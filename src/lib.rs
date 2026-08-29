#![doc = include_str!("../README.md")]

use ahash::AHashMap;
use serde_yaml::{Value, from_str};

use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

mod plural;
pub use ezlz_macros::t;

/// Global localization store.
///
/// `init()` populates this exactly once.
static TRANSLATIONS: OnceLock<Translations> = OnceLock::new();

/// Stores all compiled [`Template`]s for all locales along
/// with the name of the fallback locale.
#[derive(Debug)]
struct Translations {
    fallback: Box<str>,
    locales: AHashMap<Box<str>, AHashMap<Box<str>, Template>>,
}

/// Errors that may occur during [`init`].
#[derive(Debug)]
pub enum Error {
    /// File system errors.
    ///
    /// Occurs if the process has no permissions
    /// to open a file or a read operation was interrupted.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// YAML syntax errors.
    ///
    /// Occurs if a file has invalid indentation
    /// or is not valid YAML.
    ParseYaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    /// Unexpected value type.
    ///
    /// Translation files must only have string values.
    InvalidYaml { path: PathBuf, message: String },
    /// Error during [`Template`] compilation.
    ///
    /// Occurs if a translation string has
    /// unclosed or invalid placeholders.
    InvalidTemplate {
        path: PathBuf,
        key: String,
        message: String,
    },
    /// Occurs if [`init`] is called after a successful initialization.
    AlreadyInitialized,
    /// Occurs if the fallback locale can't found in locales directory.
    FallbackLocaleNotFound { name: String, path: PathBuf },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Io { path, source } => {
                write!(f, "failed to read {}: {source}", path.to_string_lossy())
            }

            Self::ParseYaml { path, source } => {
                write!(f, "failed to parse {}: {}", path.to_string_lossy(), source)
            }

            Self::InvalidYaml { path, message } => {
                write!(f, "invalid YAML in {}: {message}", path.to_string_lossy())
            }

            Self::InvalidTemplate { path, key, message } => write!(
                f,
                "invalid template '{key}' in {}: {message}",
                path.to_string_lossy()
            ),

            Self::AlreadyInitialized => write!(
                f,
                "ezlz::init() can't be called again after a successful initialization"
            ),

            Self::FallbackLocaleNotFound { name, path } => write!(
                f,
                "fallback locale '{name}' not found in {}",
                path.to_string_lossy()
            ),
        }
    }
}

impl StdError for Error {}

/// Initializes the localization store from a `directory`. The `fallback`
/// locale is used if some translation is unavailable in the requested locale.
/// May return an [`Error`].
///
/// ```rust no_run
/// use ezlz::t;
///
/// // Will search "locales" for translation files
/// // and use "en" as fallback locale.
/// ezlz::init("en", "locales").unwrap();
///
/// fn get_lang() -> String {
///     "ru".to_string()
/// }
///
/// t!("en", messages.hello);
/// t!("en_GB", messages.greet, name = "Anna");
/// // If the variable name matches placeholder name,
/// // using explicit placeholder names is unnecessary:
/// let name = "Anna";
/// t!(get_lang(), messages.greet, name);
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

        compile_translation(&path, &yaml, String::new(), &mut translations)?;

        locales.insert(Box::from(locale), translations);
    }

    TRANSLATIONS
        .set(Translations {
            fallback: fallback.clone(),
            locales,
        })
        .map_err(|_| Error::AlreadyInitialized)?;

    if TRANSLATIONS
        .get()
        .map(|t| t.locales.get(&fallback))
        .unwrap()
        .is_none()
    {
        return Err(Error::FallbackLocaleNotFound {
            name: fallback.to_string(),
            path: directory.to_path_buf(),
        });
    }

    Ok(())
}

/// Recursively flattens YAML mappings into dotted translation keys
/// and compiles each translated value into a [`Template`],
/// inserting each key-template pair into `output`.
fn compile_translation(
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
                        message: "template keys must be strings".to_owned(),
                    });
                };

                let full_key = if prefix.is_empty() {
                    key.to_owned()
                } else {
                    format!("{prefix}.{key}")
                };

                compile_translation(path, value, full_key, output)?;
            }
        }

        Value::String(value) => {
            let template = match Template::compile(value) {
                Ok(template) => template,
                Err(msg) => {
                    return Err(Error::InvalidTemplate {
                        path: path.to_path_buf(),
                        key: prefix,
                        message: msg,
                    });
                }
            };

            output.insert(prefix.into_boxed_str(), template);
        }

        _ => {
            return Err(Error::InvalidYaml {
                path: path.to_path_buf(),
                message: format!("template value must be a string, got {value:?}"),
            });
        }
    }

    Ok(())
}

/// A compiled segment of a translation text.
#[derive(Debug)]
enum Part {
    /// Plain text part.
    Text(Box<str>),
    /// Regular placeholder.
    Variable { name: Box<str> },
    /// Plural placeholder.
    Plural {
        name: Box<str>,
        rules: plural::Ruleset,
    },
}

impl Part {
    /// Parse a placeholder from a string slice containing its body.
    fn parse_placeholder(source: &str) -> Result<Self, String> {
        if is_identifier(source) {
            return Ok(Self::Variable {
                name: source.into(),
            });
        }
        if let Some((name, rules)) = plural::compile(source) {
            return Ok(Self::Plural { name, rules });
        }
        Err("unable to parse placeholder".to_string())
    }

    /// Parse the first valid [`Part`] from a string slice and
    /// return it along with the rest of that string slice.
    fn parse_next(source: &str) -> Result<(Self, &str), String> {
        /// Converts a slice of bytes to a string slice.
        fn str_from_bytes(bytes: &[u8]) -> &str {
            unsafe { str::from_utf8_unchecked(bytes) }
        }
        /// Checks if the byte at index `i` has an odd number of `\` bytes before it.
        fn is_escaped(bytes: &[u8], i: usize) -> bool {
            let mut backslashes = 0;
            for b in bytes[..i].iter().rev() {
                if *b == b'\\' {
                    backslashes += 1;
                } else {
                    break;
                }
            }
            backslashes % 2 == 1
        }
        let bytes = source.as_bytes();
        let size = bytes.len();
        let mut text = String::with_capacity(size);
        let mut i = 0;
        while i < size {
            match bytes[i] {
                b'{' if !is_escaped(bytes, i) => {
                    if text.is_empty() {
                        let Some(end) = source[i + 1..].find('}').map(|r_end| i + 1 + r_end) else {
                            return Err("unclosed placeholder".to_string());
                        };
                        let body = str_from_bytes(&bytes[i + 1..end]);
                        let part = Self::parse_placeholder(body)?;
                        let rest = &source[end + 1..];
                        return Ok((part, rest));
                    } else {
                        text = text.replace("\\{", "{");
                        let part = Self::Text(text.into_boxed_str());
                        let rest = str_from_bytes(&bytes[i..]);
                        return Ok((part, rest));
                    }
                }
                byte => unsafe {
                    text.as_mut_vec().push(byte);
                    i += 1;
                },
            }
        }
        text = text.replace("\\{", "{");
        Ok((Self::Text(text.into_boxed_str()), ""))
    }
}

/// A compiled representation of a translation string.
#[derive(Debug)]
struct Template {
    /// List of the compiled template parts.
    parts: Box<[Part]>,
    /// Estimated size of a rendered template.
    ///
    /// Calculated in [`Template::compile`].
    /// Used as the size of the string buffer in [`Template::render`].
    bufsize: usize,
}

impl Template {
    /// Estimated size of a placeholder's rendered value.
    const ESTIMATED_ARG_LEN: usize = 32;
    /// Parse a translation string and compile its segments
    /// into a list of [`Part`]s.
    ///
    /// Calculates the approximate size of the rendered template by
    /// adding the total size of text parts to the number of placeholders
    /// multiplied by [`Template::ESTIMATED_ARG_LEN`].
    fn compile(translation: &str) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut source = translation;
        let mut bufsize = 0;

        while !source.is_empty() {
            let (part, rest) = match Part::parse_next(source) {
                Ok((part, rest)) => {
                    match &part {
                        Part::Text(text) => bufsize += text.len(),
                        Part::Variable { name: _ } => {
                            bufsize += Self::ESTIMATED_ARG_LEN;
                        }
                        Part::Plural { name: _, rules: _ } => {
                            bufsize += Self::ESTIMATED_ARG_LEN;
                        }
                    }
                    (part, rest)
                }
                Err(msg) => return Err(msg),
            };
            parts.push(part);
            source = rest;
        }

        Ok(Self {
            parts: parts.into_boxed_slice(),
            bufsize,
        })
    }

    /// Render a template.
    ///
    /// Iterates over the [`Part`]s of a template and
    /// renders each part in a way corresponding to its
    /// variant to a [`String`] buffer, returning the buffer.
    fn render(&self, args: &[(&str, Arg<'_>)]) -> String {
        /// Find an [`Arg`] by its `name` in a list of `args`.
        #[inline]
        fn find_arg<'a>(args: &'a [(&str, Arg<'a>)], name: &str) -> Option<&'a Arg<'a>> {
            args.iter()
                .find(|(arg_name, _)| *arg_name == name)
                .map(|(_, value)| value)
        }

        let mut output = String::with_capacity(self.bufsize);

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

/// Checks if a string slice is a valid placeholder identifier.
fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// An argument passed to a translation template.
///
/// Wraps the input value to determine the rendering approach.
#[derive(PartialEq, Debug)]
pub enum Arg<'a> {
    /// Signed integers.
    Int(i64),
    /// Unsigned integers.
    Uint(u64),
    /// Floating point numbers.
    Float(f64),
    /// String values.
    String(&'a str),
}

impl<'a> Arg<'a> {
    /// Checks if `self` is a numeric argument.
    #[inline]
    fn is_numeric(&self) -> bool {
        !matches!(self, Self::String(..))
    }

    /// Checks if `self` is an [`Arg::Float`].
    #[inline]
    fn is_float(&self) -> bool {
        matches!(self, Self::Float(..))
    }

    /// Returns the truncated absolute value of a numeric argument.
    #[inline]
    fn abs_trunc(&self) -> u64 {
        match self {
            Self::Int(value) => value.unsigned_abs(),
            Self::Uint(value) => *value,
            Self::Float(value) => value.abs() as u64,
            // Because this function is only called after making sure
            // an arg is numeric, the string arm is not needed.
            Self::String(_) => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    /// Writes the value to the `output` buffer
    /// using the method corresponding to its variant.
    #[inline]
    fn write_to(&self, output: &mut String) {
        match self {
            Self::Int(value) => {
                let mut buf = itoa::Buffer::new();
                let n = buf.format(*value);
                output.push_str(n);
            }
            Self::Uint(value) => {
                let mut buf = itoa::Buffer::new();
                let n = buf.format(*value);
                output.push_str(n);
            }
            Self::Float(value) => {
                let mut buf = zmij::Buffer::new();
                let n = buf.format(*value);
                output.push_str(n);
            }
            Self::String(value) => {
                output.push_str(value);
            }
        }
    }
}

/// Trait for types whose references can be converted to an [`Arg`].
pub trait ToArg<'a> {
    /// Converts the type reference to an [`Arg`].
    ///
    /// Must convert the referenced value to the corresponding [`Arg`] variant.
    /// Should be marked `#[inline]` for optimization in hot loops.
    fn to_arg(self) -> Arg<'a>;
}

impl<'a> ToArg<'a> for &'a &str {
    #[inline]
    fn to_arg(self) -> Arg<'a> {
        Arg::String(self)
    }
}

impl<'a> ToArg<'a> for &'a String {
    #[inline]
    fn to_arg(self) -> Arg<'a> {
        Arg::String(self)
    }
}

/// Blanket-implements [`ToArg`] on numeric types.
macro_rules! impl_to_arg {
    ($($ty:ty => $field:ident, $cast:ty),* $(,)?) => {
        $(
            impl<'a> ToArg<'a> for &'a $ty {
                #[inline]
                fn to_arg(self) -> Arg<'a> {
                    Arg::$field(*self as $cast)
                }
            }
        )*
    };
}

impl_to_arg!(
    i8    => Int,   i64,
    i16   => Int,   i64,
    i32   => Int,   i64,
    i64   => Int,   i64,
    isize => Int,   i64,

    u8    => Uint,  u64,
    u16   => Uint,  u64,
    u32   => Uint,  u64,
    u64   => Uint,  u64,
    usize => Uint,  u64,

    f32   => Float, f64,
    f64   => Float, f64,
);

/// Used by the [`t!`] proc-macro to convert a
/// compatible input to an [`Arg`].
#[inline]
pub fn __arg<'a, T: ?Sized>(value: &'a T) -> Arg<'a>
where
    &'a T: ToArg<'a>,
{
    <&'a T as ToArg<'a>>::to_arg(value)
}

/// Generated in place of the [`t!`] proc-macro.
///
/// Panics if [`init`] has not been called.
///
/// Panics if a translation doesn't exist in either the requested or fallback
/// locale, unless the `missing-key-nopanic` feature is enabled, in which case
/// it returns the translation key.
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
        None => {
            #[cfg(not(feature = "missing-key-nopanic"))]
            {
                panic!("translation '{key}' not found in locale '{locale}' and fallback locale")
            }
            #[cfg(feature = "missing-key-nopanic")]
            {
                return key.to_string();
            }
        }
    }
}
