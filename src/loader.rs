//! YAML localization loader.

use std::fs;
use std::path::Path;

use yaml_serde::{Value, from_str};

use crate::{Error, HashMap, HashMapExt, Template, Translations};

/// Parses YAML files in `directory`, builds a [`Translations`] instance
/// with `fallback` locale and returns a raw pointer to it.
pub fn load(fallback: impl Into<Box<str>>, directory: impl AsRef<Path>) -> Result<*mut Translations, Error> {
    let fallback = fallback.into();
    let directory = directory.as_ref();

    let mut locales = HashMap::new();

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

        if locales.contains_key(locale) {
            return Err(Error::DuplicateLocale {
                name: locale.to_string(),
            });
        }

        let source = fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;

        let yaml: Value = from_str(&source).map_err(|source| Error::Parse {
            path: path.clone(),
            source: Box::new(source),
        })?;

        let mut translations = HashMap::<Box<str>, Template>::new();

        compile_translation(&path, &yaml, String::new(), &mut translations)?;

        locales.insert(Box::from(locale), translations);
    }

    if !locales.contains_key(&fallback) {
        return Err(Error::FallbackLocaleNotFound {
            name: fallback.to_string(),
            path: directory.to_path_buf(),
        });
    }

    let translations = Translations { fallback, locales };

    let ptr = Box::into_raw(Box::new(translations));
    Ok(ptr)
}

/// Recursively flattens YAML mappings into dotted translation keys
/// and compiles each translated value into a [`Template`],
/// inserting each key-template pair into `output`.
fn compile_translation(
    path: &Path,
    value: &Value,
    prefix: String,
    output: &mut HashMap<Box<str>, Template>,
) -> Result<(), Error> {
    match value {
        Value::Mapping(mapping) =>
            for (key, value) in mapping {
                let Some(key) = key.as_str() else {
                    return Err(Error::Value {
                        path: path.to_path_buf(),
                        message: "template keys must be strings".to_owned(),
                    });
                };

                let full_key = if prefix.is_empty() {
                    key.to_owned()
                } else {
                    format!("{prefix}.{key}")
                };

                compile_translation(path, value, full_key, output)?;
            },

        Value::String(value) => {
            let template = match Template::compile(value) {
                Ok(template) => template,
                Err(msg) => {
                    return Err(Error::InvalidTemplate {
                        path: path.to_path_buf(),
                        key: prefix,
                        message: msg.to_string(),
                    });
                },
            };

            if output.contains_key(&*prefix) {
                return Err(Error::DuplicateKey {
                    path: path.to_path_buf(),
                    key: prefix,
                });
            }

            output.insert(prefix.into_boxed_str(), template);
        },

        _ => {
            return Err(Error::Value {
                path: path.to_path_buf(),
                message: format!("template value must be a string, got {value:?}"),
            });
        },
    }

    Ok(())
}
