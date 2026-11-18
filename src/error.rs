use std::error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

/// Errors that may occur during initialization.
#[non_exhaustive]
#[derive(Debug)]
#[allow(missing_docs)]
pub enum Error {
    /// File system errors.
    ///
    /// Occurs if the process has no permissions
    /// to open a file or a read operation was interrupted.
    Io { path: PathBuf, source: std::io::Error },
    /// Locale syntax errors.
    ///
    /// Occurs if a translation file has invalid syntax.
    Parse {
        path: PathBuf,
        source: Box<dyn error::Error + Send + Sync>,
    },
    /// Unexpected value type.
    ///
    /// Translation files must only have string values.
    Value { path: PathBuf, message: String },
    /// Error during template compilation.
    ///
    /// Occurs if a translation value has
    /// unclosed or invalid placeholders.
    InvalidTemplate { path: PathBuf, key: String, message: String },
    /// Occurs if `init` is called after a successful initialization.
    AlreadyInitialized,
    /// Occurs if the fallback locale can't be found in the provided directory.
    FallbackLocaleNotFound { name: String, path: PathBuf },
    /// Occurs when multiple identical keys are encountered in one file.
    DuplicateKey { path: PathBuf, key: String },
    /// Occurs when multiple files with identical locale names are encountered.
    DuplicateLocale { name: String },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "failed to read {}: {source}", path.to_string_lossy()),

            Self::Parse { path, source } => write!(f, "failed to parse {}: {source}", path.to_string_lossy()),

            Self::Value { path, message } => write!(f, "invalid value in {}: {message}", path.to_string_lossy()),

            Self::InvalidTemplate { path, key, message } =>
                write!(f, "invalid template '{key}' in {}: {message}", path.to_string_lossy()),

            Self::AlreadyInitialized => write!(f, "ezlz::init() can't be called again after a successful initialization"),

            Self::FallbackLocaleNotFound { name, path } =>
                write!(f, "fallback locale '{name}' not found in {}", path.to_string_lossy()),
            Self::DuplicateKey { path, key } =>
                write!(f, "key '{key}' already appears more than once in {}", path.to_string_lossy()),
            Self::DuplicateLocale { name } => write!(f, "found more than one file for locale {name}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(&**source),
            _ => None,
        }
    }
}
