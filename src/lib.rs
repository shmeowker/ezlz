use std::{
    collections::HashMap,
    error::Error as StdError,
    fmt, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde_yaml::Value;

pub use ezlz_macros::t;

static I18N: OnceLock<I18n> = OnceLock::new();

struct I18n {
    base_locale: String,
    translations: HashMap<String, HashMap<String, String>>,
}

impl I18n {
    fn get(&self, locale: &str, key: &str) -> Option<&str> {
        self.translations
            .get(locale)
            .and_then(|translations| translations.get(key))
            .or_else(|| {
                self.translations
                    .get(&self.base_locale)
                    .and_then(|translations| translations.get(key))
            })
            .map(String::as_str)
    }
}

#[derive(Debug)]
pub enum InitError {
    AlreadyInitialized,

    DirectoryNotFound {
        path: PathBuf,
    },

    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    ParseYaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    InvalidRoot {
        path: PathBuf,
    },

    InvalidLocale {
        path: PathBuf,
    },

    InvalidTranslation {
        path: PathBuf,
        key: String,
    },

    DuplicateTranslation {
        locale: String,
        key: String,
    },
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInitialized => {
                write!(f, "ezlz has already been initialized")
            }

            Self::DirectoryNotFound { path } => {
                write!(
                    f,
                    "localization directory does not exist: {}",
                    path.display()
                )
            }

            Self::ReadDirectory { path, source } => {
                write!(
                    f,
                    "failed to read localization directory {}: {}",
                    path.display(),
                    source
                )
            }

            Self::ReadFile { path, source } => {
                write!(
                    f,
                    "failed to read localization file {}: {}",
                    path.display(),
                    source
                )
            }

            Self::ParseYaml { path, source } => {
                write!(
                    f,
                    "failed to parse localization file {}: {}",
                    path.display(),
                    source
                )
            }

            Self::InvalidRoot { path } => {
                write!(
                    f,
                    "localization file {} must contain a YAML mapping at its root",
                    path.display()
                )
            }

            Self::InvalidLocale { path } => {
                write!(
                    f,
                    "could not determine locale from file name: {}",
                    path.display()
                )
            }

            Self::InvalidTranslation { path, key } => {
                write!(
                    f,
                    "translation `{}` in {} must be a string",
                    key,
                    path.display()
                )
            }

            Self::DuplicateTranslation { locale, key } => {
                write!(
                    f,
                    "duplicate translation for locale `{}` and key `{}`",
                    locale, key
                )
            }
        }
    }
}

impl StdError for InitError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::ReadDirectory { source, .. } => Some(source),
            Self::ReadFile { source, .. } => Some(source),
            Self::ParseYaml { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Initialize ezlz from a directory containing locale YAML files.
///
/// For example:
///
/// ```ignore
/// ezlz::init("en", "locales")?;
/// ```
///
/// Given:
///
/// ```text
/// locales/
/// ├── en.yml
/// ├── en_GB.yml
/// └── de.yml
/// ```
///
/// the file names become the locale names.
///
/// This function should normally only be called once.
pub fn init(base_locale: impl Into<String>, directory: impl AsRef<Path>) -> Result<(), InitError> {
    let base_locale = base_locale.into();
    let directory = directory.as_ref();

    if !directory.is_dir() {
        return Err(InitError::DirectoryNotFound {
            path: directory.to_path_buf(),
        });
    }

    let translations = load_directory(directory)?;

    let i18n = I18n {
        base_locale,
        translations,
    };

    I18N.set(i18n).map_err(|_| InitError::AlreadyInitialized)
}

/// Non-panicking translation lookup.
///
/// `locale` is the requested locale and `key` is the flattened
/// translation key, e.g. `messages.greet`.
///
/// If the key doesn't exist in `locale`, the base locale is tried.
///
/// Returns `None` if neither locale contains the translation.
pub fn try_get(locale: &str, key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> Option<String> {
    let i18n = I18N.get()?;

    let template = i18n.get(locale, key)?;

    Some(interpolate(template, args))
}

/// Internal function used by the `t!` proc macro.
///
/// This intentionally panics if ezlz has not been initialized or if
/// a translation cannot be found.
#[doc(hidden)]
pub fn __get(locale: &str, key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let i18n = I18N.get().unwrap_or_else(|| {
        panic!(
            "ezlz::init() has not been called before \
       attempting to translate `{key}`"
        )
    });

    let template = i18n.get(locale, key).unwrap_or_else(|| {
        panic!(
            "translation `{key}` not found for locale `{locale}` \
       or fallback locale `{}`",
            i18n.base_locale
        )
    });

    interpolate(template, args)
}

fn load_directory(directory: &Path) -> Result<HashMap<String, HashMap<String, String>>, InitError> {
    let entries = fs::read_dir(directory).map_err(|source| InitError::ReadDirectory {
        path: directory.to_path_buf(),
        source,
    })?;

    let mut translations = HashMap::new();

    for entry in entries {
        let entry = entry.map_err(|source| InitError::ReadDirectory {
            path: directory.to_path_buf(),
            source,
        })?;

        let path = entry.path();

        // Ignore directories and non-YAML files.
        if !path.is_file() {
            continue;
        }

        let Some(extension) = path.extension().and_then(|x| x.to_str()) else {
            continue;
        };

        if extension != "yml" && extension != "yaml" {
            continue;
        }

        let locale = path
            .file_stem()
            .and_then(|x| x.to_str())
            .ok_or_else(|| InitError::InvalidLocale { path: path.clone() })?
            .to_owned();

        let contents = fs::read_to_string(&path).map_err(|source| InitError::ReadFile {
            path: path.clone(),
            source,
        })?;

        let yaml: Value =
            serde_yaml::from_str(&contents).map_err(|source| InitError::ParseYaml {
                path: path.clone(),
                source,
            })?;

        let mut locale_translations = HashMap::new();

        flatten_yaml(&yaml, "", &path, &mut locale_translations)?;

        translations.insert(locale, locale_translations);
    }

    Ok(translations)
}

fn flatten_yaml(
    value: &Value,
    prefix: &str,
    path: &Path,
    output: &mut HashMap<String, String>,
) -> Result<(), InitError> {
    match value {
        Value::Mapping(mapping) => {
            for (key, value) in mapping {
                let Some(key) = key.as_str() else {
                    return Err(InitError::InvalidRoot {
                        path: path.to_path_buf(),
                    });
                };

                let full_key = if prefix.is_empty() {
                    key.to_owned()
                } else {
                    format!("{prefix}.{key}")
                };

                flatten_yaml(value, &full_key, path, output)?;
            }
        }

        Value::String(value) => {
            insert_translation(output, prefix, value.clone(), path)?;
        }

        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Tagged(_) | Value::Sequence(_) => {
            return Err(InitError::InvalidTranslation {
                path: path.to_path_buf(),
                key: prefix.to_owned(),
            });
        }
    }

    Ok(())
}

fn insert_translation(
    output: &mut HashMap<String, String>,
    key: &str,
    value: String,
    path: &Path,
) -> Result<(), InitError> {
    if output.insert(key.to_owned(), value).is_some() {
        // This normally won't be reachable with serde_yaml's Mapping
        // semantics, but keeping the check makes the invariant explicit.
        let locale = path
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or("<unknown>");

        return Err(InitError::DuplicateTranslation {
            locale: locale.to_owned(),
            key: key.to_owned(),
        });
    }

    Ok(())
}

fn interpolate(template: &str, args: &[(&str, &dyn fmt::Display)]) -> String {
    if args.is_empty() {
        return template.to_owned();
    }

    let mut result = template.to_owned();

    for &(name, value) in args {
        let placeholder = format!("%{{{name}}}");

        result = result.replace(&placeholder, &value.to_string());
    }

    result
}
