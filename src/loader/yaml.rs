//! The default YAML directory loader.

use std::fs::{ReadDir, read_dir, read_to_string};
use std::path::{Path, PathBuf};

use yaml_serde::mapping::IntoIter;
use yaml_serde::{Value, from_str};

use crate::Error;
use crate::loader::Loader;

/// The YAML directory loader.
///
/// Each file with `.yml` or `.yaml` extension in a directory becomes
/// a locale with dotted YAML mappings as translation keys.
/// Mapping keys and values must be strings.
///
/// Uses a stack-based lazy flattening iterator to traverse nested YAML mappings.
pub struct Yaml {
    root: PathBuf,
    path: PathBuf,
    files: ReadDir,
    fallback: Box<str>,
    lang: String,
    stack: Vec<(String, IntoIter)>,
}

impl Yaml {
    /// Sets up a new instance.
    pub fn new(fallback: impl Into<Box<str>>, root: impl AsRef<Path>) -> Result<Self, Error> {
        let fallback = fallback.into();
        let root = root.as_ref().to_path_buf();

        let files = match read_dir(&root) {
            Ok(files) => files,
            Err(source) => return Err(Error::Io { path: root, source }),
        };

        Ok(Self {
            root,
            path: PathBuf::new(),
            files,
            fallback,
            lang: String::new(),
            stack: Vec::new(),
        })
    }
}

impl Loader for Yaml {
    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    fn root(&self) -> PathBuf {
        self.root.clone()
    }

    fn fallback(&self) -> Box<str> {
        self.fallback.clone()
    }

    fn next_lang(&mut self) -> Result<Option<String>, Error> {
        while let Some(res) = self.files.next() {
            let entry = res.map_err(|source| Error::Io {
                path: self.root(),
                source,
            })?;

            let path = entry.path();

            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension() else {
                continue;
            };
            if ext != "yml" && ext != "yaml" {
                continue;
            }
            let Some(locale) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            let source = match read_to_string(&path) {
                Ok(source) => source,
                Err(source) => return Err(Error::Io { path, source }),
            };

            let value = match from_str(&source) {
                Ok(Value::Mapping(mapping)) => mapping,

                Err(source) =>
                    return Err(Error::Parse {
                        path,
                        source: Box::new(source),
                    }),

                Ok(invalid) =>
                    return Err(Error::Value {
                        path,
                        message: format!("expected mapping, found {invalid:?}"),
                    }),
            };

            self.stack.push((String::new(), value.into_iter()));
            self.lang = locale.to_owned();
            self.path = path;

            return Ok(Some(self.lang.clone()));
        }

        Ok(None)
    }

    fn next_kv(&mut self) -> Result<Option<(String, String)>, Error> {
        while let Some((prefix, iter)) = self.stack.last_mut() {
            if let Some((key, value)) = iter.next() {
                let Value::String(key) = key else {
                    return Err(Error::Value {
                        path: self.path.clone(),
                        message: format!("template keys must be strings, got {key:?}"),
                    });
                };

                let full_key = if prefix.is_empty() {
                    key
                } else {
                    format!("{}.{}", prefix, key)
                };

                match value {
                    Value::String(value) => return Ok(Some((full_key, value))),

                    Value::Mapping(map) => self.stack.push((full_key, map.into_iter())),

                    invalid =>
                        return Err(Error::Value {
                            path: self.path.clone(),
                            message: format!("template values must be strings, got {invalid:?}"),
                        }),
                }
            } else {
                // Current mapping exhausted; pop and resume parent
                self.stack.pop();
            }
        }

        Ok(None)
    }
}
