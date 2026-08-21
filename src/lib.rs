use ahash::AHashMap;
use serde_yaml::{Value, from_str};
use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult, Write},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};
mod plural;

pub use ezlz_macros::t;

/// Global localization store.
///
/// `init()` populates this exactly once.
static TRANSLATIONS: OnceLock<Templates> = OnceLock::new();

#[derive(Debug)]
struct Templates {
    fallback: Box<str>,
    locales: AHashMap<Box<str>, AHashMap<Box<str>, Template>>,
}

#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    ParseYaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    InvalidYaml {
        path: PathBuf,
        message: String,
    },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Io { path, source } => {
                write!(f, "Failed to read {}: {source}", path.to_string_lossy())
            }

            Self::ParseYaml { path, source } => {
                write!(
                    f,
                    "Failed to parse {}: {}",
                    path.to_string_lossy(),
                    source.to_string()
                )
            }

            Self::InvalidYaml { path, message } => {
                write!(f, "Invalid YAML in {}: {message}.", path.to_string_lossy())
            }
        }
    }
}

impl StdError for Error {}

/// Initialize `ezlz` from a localization directory, using `fallback`
/// locale if some translation is not present in provided language.
///
/// Each `.yml`/`.yaml` file represents a locale.
///
/// ```text
/// locales/
/// ├── en.yml
/// ├── en_GB.yml
/// └── ru.yml
/// ```
///
/// ```yaml
/// # locales/en.yml
/// messages:
///   hello: "Hello"
///   greet: "Hello, {name}!"
/// ui:
///   plural: "You have {i|=1: item| items}"
/// ```
///
/// YAML mappings become dotted translation keys.
/// These can be then used in the proc macro:
/// ```rust ignore
/// use ezlz::t;
/// ezlz::init("en", "locales").unwrap();
///
/// fn get_lang() -> String {
///     "ru".to_string()
/// }
///
/// t!("en", messages.hello);
/// t!("en_GB", messages.greet, name = "Anna");
/// // If the variable name matches placeholder name,
/// // using explicit placeholder names is unneccesary:
/// let i: u8 = 7;
/// t!(get_lang(), ui.plural, i);
/// ```
pub fn init(fallback: impl Into<Box<str>>, directory: impl AsRef<Path>) -> Result<(), Error> {
    let fallback = fallback.into();
    let directory = directory.as_ref();

    let mut locales = AHashMap::new();

    let entries = fs::read_dir(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;

        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let Some(extension) = path.extension() else {
            continue;
        };

        if extension != "yml" && extension != "yaml" {
            continue;
        }

        let Some(locale) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let source = fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;

        let yaml: Value = from_str(&source).map_err(|source| Error::ParseYaml {
            path: path.clone(),
            source,
        })?;

        let mut translations = AHashMap::<Box<str>, Template>::new();

        flatten_yaml(&path, &yaml, String::new(), &mut translations)?;

        locales.insert(Box::from(locale), translations);
    }

    TRANSLATIONS
        .set(Templates { fallback, locales })
        .map_err(|_| Error::InvalidYaml {
            path: directory.to_path_buf(),
            message: "ezlz::init() can't called more than once.".to_owned(),
        })?;

    if TRANSLATIONS
        .get()
        .map(|t| t.locales.get(&t.fallback))
        .unwrap()
        .is_none()
    {
        panic!("Fallback locale not found.");
    }

    Ok(())
}

/// Flatten YAML mappings into dotted translation keys and compile
/// each translation into AHashMap.
fn flatten_yaml(
    path: &Path,
    value: &Value,
    prefix: String,
    output: &mut AHashMap<Box<str>, Template>,
) -> Result<(), Error> {
    match value {
        Value::Mapping(mapping) => {
            for (key, value) in mapping {
                let Some(key) = key.as_str() else {
                    return Err(Error::InvalidYaml {
                        path: path.to_path_buf(),
                        message: "Template keys must be strings".to_owned(),
                    });
                };

                let full_key = if prefix.is_empty() {
                    key.to_owned()
                } else {
                    format!("{prefix}.{key}")
                };

                flatten_yaml(path, value, full_key, output)?;
            }
        }

        Value::String(value) => {
            let translation = Template::compile(value);

            output.insert(prefix.into_boxed_str(), translation);
        }

        _ => {
            return Err(Error::InvalidYaml {
                path: path.to_path_buf(),
                message: format!("Template value must be a string, got {value:?}"),
            });
        }
    }

    Ok(())
}

#[derive(Debug)]
enum Part {
    Text(Box<str>),

    Variable {
        name: Box<str>,
    },

    Plural {
        name: Box<str>,
        rules: plural::Ruleset,
    },
}

#[derive(Debug)]
struct Template {
    parts: Box<[Part]>,
}

impl Template {
    fn compile(template: &str) -> Self {
        let mut parts = Vec::new();
        let mut text_start = 0;
        let bytes = template.as_bytes();

        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] != b'{' {
                i += 1;
                continue;
            }

            // Find the closing `}`.
            let Some(relative_end) = template[i + 1..].find('}') else {
                i += 1;
                continue;
            };

            let end = i + 1 + relative_end;

            // Text preceding the placeholder.
            if text_start < i {
                parts.push(Part::Text(template[text_start..i].into()));
            }

            let placeholder = &template[i + 1..end];

            // Ordinary `{name}` placeholder.
            if is_identifier(placeholder) {
                parts.push(Part::Variable {
                    name: placeholder.into(),
                });

                i = end + 1;
                text_start = i;
                continue;
            }

            if let Some((name, rules)) = plural::compile(placeholder) {
                parts.push(Part::Plural { name, rules });

                i = end + 1;
                text_start = i;
                continue;
            }

            parts.push(Part::Text(template[i..=end].into()));

            i = end + 1;
            text_start = i;
        }

        // Remaining text.
        if text_start < template.len() {
            parts.push(Part::Text(template[text_start..].into()));
        }

        Self {
            parts: parts.into_boxed_slice(),
        }
    }

    fn render(&self, args: &[(&str, Arg<'_>)]) -> String {
        let mut output = String::new();

        for part in &self.parts {
            match part {
                Part::Text(text) => {
                    output.push_str(text);
                }

                Part::Variable { name } => {
                    if let Some(arg) = find_arg(args, name) {
                        arg.write_to(&mut output);
                    }
                }

                Part::Plural { name, rules } => {
                    if let Some(arg) = find_arg(args, name) {
                        rules.render(&mut output, arg);
                    }
                }
            }
        }

        output
    }
}

#[inline]
fn find_arg<'a>(args: &'a [(&str, Arg<'a>)], name: &str) -> Option<&'a Arg<'a>> {
    args.iter()
        .find(|(arg_name, _)| *arg_name == name)
        .map(|(_, value)| value)
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

#[derive(Debug)]
pub struct Number(f64, bool);

impl Number {
    fn new(n: &dyn Display, is_float: bool) -> Self {
        Self(n.to_string().parse::<f64>().unwrap_or(f64::NAN), is_float)
    }
    #[inline]
    fn abs_trunc(&self) -> u64 {
        self.0.abs().trunc() as u64
    }
    #[inline]
    fn is_float(&self) -> bool {
        self.1
    }
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for Number {
    fn to_string(&self) -> String {
        if self.1 {
            let mut buf = zmij::Buffer::new();
            return buf.format(self.0).to_owned();
        }
        unsafe { self.0.to_int_unchecked::<i64>().to_string() }
    }
}

/// (value, is_float)
///
/// `is_float` is only used during plural placeholder
/// rendering and doesn't matter for any non-numberic value.
pub struct Arg<'a>(&'a dyn Display, bool);

impl Arg<'_> {
    fn write_to(&self, output: &mut String) {
        let _ = write!(output, "{}", self.0);
    }
    fn number(&self) -> Number {
        Number::new(self.0, self.1)
    }
}

pub trait ToArg<'a> {
    fn to_arg(self) -> Arg<'a>;
}

macro_rules! impl_to_arg {
    ($float:expr; $($ty:ty),* $(,)?) => {
        $(
            impl<'a> ToArg<'a> for &'a $ty {
                #[inline(always)]
                fn to_arg(self) -> Arg<'a> {
                    Arg(self, $float)
                }
            }
        )*
    };
}

impl_to_arg!(
    false; u8, u16, u32, u64, usize, i8, i16, i32, i64, isize,
);
impl_to_arg!(
    true; f32, f64, String, &str
);

/// Supported types:
/// u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64, String, &str
#[doc(hidden)]
pub fn __arg<'a, T: ?Sized>(value: &'a T) -> Arg<'a>
where
    &'a T: ToArg<'a>,
{
    <&'a T as ToArg<'a>>::to_arg(value)
}

/// Used by the `t!` proc-macro.
///
/// Panics if `ezlz` has not been initialized
/// or if a translation cannot be found.
#[doc(hidden)]
pub fn __get(locale: &str, key: &str, args: &[(&str, Arg<'_>)]) -> String {
    let translations = TRANSLATIONS
        .get()
        .expect("ezlz::init() must be called before using t!");

    let template = translations
        .locales
        .get(locale)
        .and_then(|locale| locale.get(key))
        .or_else(|| {
            translations
                .locales
                .get(&translations.fallback)
                .and_then(|locale| locale.get(key))
        });

    match template {
        Some(translation) => translation.render(args),
        None => panic!("Translation '{key}' not found in locale '{locale}' and fallback locale.",),
    }
}
