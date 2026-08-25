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

#[derive(Debug)]
struct Translations {
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

    InvalidTemplate {
        path: PathBuf,
        key: String,
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
                write!(f, "Failed to parse {}: {}", path.to_string_lossy(), source)
            }

            Self::InvalidYaml { path, message } => {
                write!(f, "Invalid YAML in {}: {message}.", path.to_string_lossy())
            }

            Self::InvalidTemplate { path, key, message } => {
                write!(
                    f,
                    "Invalid template '{key}' in {}: {message}.",
                    path.to_string_lossy()
                )
            }
        }
    }
}

impl StdError for Error {}

/// Initialize from a localization `directory`, using `fallback`
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
/// These can be then used in the proc-macro syntax:
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
        .set(Translations { fallback, locales })
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

impl Part {
    fn parse_placeholder(source: &str) -> Result<Self, String> {
        if is_identifier(source) {
            return Ok(Self::Variable {
                name: source.into(),
            });
        }
        if let Some((name, rules)) = plural::compile(source) {
            return Ok(Self::Plural { name, rules });
        }
        Err(format!("Unable to parse placeholder"))
    }
    fn parse_next(source: &str) -> Result<(Self, &str), String> {
        fn str_from_bytes(bytes: &[u8]) -> &str {
            unsafe { str::from_utf8_unchecked(bytes) }
        }
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
                            return Err(format!("Unclosed placeholder"));
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

#[derive(Debug)]
struct Template {
    parts: Box<[Part]>,
}

impl Template {
    fn compile(translation: &str) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut source = translation;

        while !source.is_empty() {
            let (part, rest) = match Part::parse_next(source) {
                Ok((part, rest)) => (part, rest),
                Err(msg) => return Err(msg),
            };
            parts.push(part);
            source = rest;
        }

        Ok(Self {
            parts: parts.into_boxed_slice(),
        })
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

/// The kind of type an [`Arg`] is constructed from.
#[derive(PartialEq)]
pub enum ArgKind {
    Int,
    Uint,
    Float,
    String,
}

/// An argument passed to a translation template.
pub struct Arg<'a> {
    pub value: ArgValue<'a>,
    pub kind: ArgKind,
}

/// The value stored in an [`Arg`].
///
/// Only the field corresponding to [`Arg::kind`] may be read.
pub union ArgValue<'a> {
    pub int: i64,
    pub uint: u64,
    pub float: f64,
    pub string: &'a str,
}

impl<'a> Arg<'a> {
    #[inline]
    fn is_numberic(&self) -> bool {
        self.kind != ArgKind::String
    }
    #[inline]
    fn abs_trunc(&self) -> u64 {
        unsafe {
            match self.kind {
                ArgKind::Int => self.value.int.unsigned_abs(),
                ArgKind::Uint => self.value.uint,
                ArgKind::Float => self.value.float.abs() as u64,
                _ => std::hint::unreachable_unchecked(),
            }
        }
    }
    #[inline]
    fn is_float(&self) -> bool {
        self.kind == ArgKind::Float
    }
    #[inline]
    fn write_to(&self, out: &mut String) {
        unsafe {
            match self.kind {
                ArgKind::Int => {
                    let mut buf = itoa::Buffer::new();
                    let n = buf.format(self.value.int);
                    out.push_str(n);
                }
                ArgKind::Uint => {
                    let mut buf = itoa::Buffer::new();
                    let n = buf.format(self.value.uint);
                    out.push_str(n);
                }
                ArgKind::Float => {
                    let mut buf = zmij::Buffer::new();
                    let n = buf.format(self.value.float);
                    out.push_str(n);
                }
                ArgKind::String => {
                    out.push_str(self.value.string);
                }
            }
        }
    }
}

/// Converts the type reference to an [`Arg`].
///
/// Must set the [`Arg::kind`] to the
/// variant of [`ArgKind`] that matches the
/// input type group and assign the value
/// to a corresponding field of [`ArgValue`].
pub trait ToArg<'a> {
    fn to_arg(self) -> Arg<'a>;
}

impl<'a> ToArg<'a> for &'a &str {
    #[inline]
    fn to_arg(self) -> Arg<'a> {
        Arg {
            value: ArgValue { string: self },
            kind: ArgKind::String,
        }
    }
}

impl<'a> ToArg<'a> for &'a String {
    #[inline]
    fn to_arg(self) -> Arg<'a> {
        Arg {
            value: ArgValue { string: self },
            kind: ArgKind::String,
        }
    }
}

macro_rules! impl_to_arg {
    ($($ty:ty => $field:ident, $kind:ident, $cast:ty),* $(,)?) => {
        $(
            impl<'a> ToArg<'a> for &'a $ty {
                #[inline(always)]
                fn to_arg(self) -> Arg<'a> {
                    Arg {
                        value: ArgValue { $field: *self as $cast },
                        kind: ArgKind::$kind,
                    }
                }
            }
        )*
    };
}

impl_to_arg!(
    i8    => int,   Int, i64,
    i16   => int,   Int, i64,
    i32   => int,   Int, i64,
    i64   => int,   Int, i64,
    isize => int,   Int, i64,

    u8    => uint,  Uint, u64,
    u16   => uint,  Uint, u64,
    u32   => uint,  Uint, u64,
    u64   => uint,  Uint, u64,
    usize => uint,  Uint, u64,

    f32   => float, Float, f64,
    f64   => float, Float, f64,
);

/// Used by the [`t!`] proc-macro to convert a
/// compatible input to [`Arg`].
///
/// Types that implement [`ToArg`] «out of the box»:
/// [`u8`], [`u16`], [`u32`], [`u64`], [`usize`],
/// [`i8`], [`i16`], [`i32`], [`i64`], [`isize`],
/// [`f32`], [`f64`],
/// [`String`], &[`str`]
pub fn __arg<'a, T: ?Sized>(value: &'a T) -> Arg<'a>
where
    &'a T: ToArg<'a>,
{
    <&'a T as ToArg<'a>>::to_arg(value)
}

/// Generated in place of the [`t!`] proc-macro.
///
/// Panics if `ezlz` has not been initialized
/// or if a translation cannot be found in requested
/// and fallback locale.
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
        None => panic!("Translation '{key}' not found in locale '{locale}' and fallback locale."),
    }
}
