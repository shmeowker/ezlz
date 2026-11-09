#![doc = include_str!("../README.md")]

use std::error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::time::Duration;
use std::{fs, thread};

use ahash::{HashMap, HashMapExt};
use serde_yaml::{Value, from_str};

mod plural;
pub use ezlz_macros::t;

/// Global localization store.
static STORAGE: AtomicPtr<Translations> = AtomicPtr::new(null_mut());
/// How long an old storage is kept alive in memory
/// after calling [`reinit`], in seconds.
static CLEANUP_DELAY: AtomicU64 = AtomicU64::new(1);

/// Stores all compiled [`Template`]s for all locales along
/// with the name of the fallback locale.
#[derive(Debug)]
struct Translations {
    fallback: Box<str>,
    locales: HashMap<Box<str>, HashMap<Box<str>, Template>>,
}

/// Errors that may occur during [`init`].
#[derive(Debug)]
pub enum Error {
    /// File system errors.
    ///
    /// Occurs if the process has no permissions
    /// to open a file or a read operation was interrupted.
    Io { path: PathBuf, source: std::io::Error },
    /// YAML syntax errors.
    ///
    /// Occurs if a file has invalid indentation
    /// or is not valid YAML.
    ParseYaml { path: PathBuf, message: String },
    /// Unexpected value type.
    ///
    /// Translation files must only have string values.
    InvalidYaml { path: PathBuf, message: String },
    /// Error during [`Template`] compilation.
    ///
    /// Occurs if a translation value has
    /// unclosed or invalid placeholders.
    InvalidTemplate { path: PathBuf, key: String, message: String },
    /// Occurs if [`init`] is called after a successful initialization.
    AlreadyInitialized,
    /// Occurs if the fallback locale can't be found in the provided directory.
    FallbackLocaleNotFound { name: String, path: PathBuf },
    /// Occurs when multiple identical keys are encountered in one file.
    DuplicateKey { path: PathBuf, key: String },
    /// Occurs when multiple files with identical locale names are encountered.
    DuplicateLocale { name: String },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Io { path, source } => write!(f, "failed to read {}: {source}", path.to_string_lossy()),

            Self::ParseYaml { path, message } => write!(f, "failed to parse {}: {message}", path.to_string_lossy()),

            Self::InvalidYaml { path, message } => write!(f, "invalid YAML in {}: {message}", path.to_string_lossy()),

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

impl error::Error for Error {}

/// Sets how long the old storage is kept alive in memory after 
/// calling [`reinit`]. This delay is 1 second by default.
pub fn set_cleanup_delay(seconds: NonZero<u64>) {
    CLEANUP_DELAY.store(seconds.get(), Ordering::Relaxed);
}

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
    if !STORAGE.load(Ordering::Acquire).is_null() {
        return Err(Error::AlreadyInitialized);
    }

    let ptr = load(fallback, directory)?;

    match STORAGE.compare_exchange(null_mut(), ptr, Ordering::Release, Ordering::Relaxed) {
        Ok(_) => Ok(()),
        Err(_) => {
            unsafe { drop(Box::from_raw(ptr)) };
            Err(Error::AlreadyInitialized)
        },
    }
}

/// Initializes a new storage and swaps the old one with it.
/// The old storage will be dropped after [`CLEANUP_DELAY`] seconds.
///
/// That delay can be changed with [`set_cleanup_delay`] and is 1 second by default.
///
/// # Safety
/// You must ensure that all other threads reading the old storage finish accessing it
/// within the [`CLEANUP_DELAY`] window. This is very unlikely though, because individual
/// storage lookups usually take no more than 1 millisecond and return an owned [`String`]
/// without using the storage longer than that.
pub unsafe fn reinit(fallback: impl Into<Box<str>>, directory: impl AsRef<Path>) -> Result<(), Error> {
    let new = load(fallback, directory)?;
    let old = STORAGE.swap(new, Ordering::AcqRel);

    if !old.is_null() {
        let addr = old as usize;
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(CLEANUP_DELAY.load(Ordering::Acquire)));
            unsafe {
                let _ = Box::from_raw(addr as *mut Translations);
            }
        });
    }

    Ok(())
}

/// Parses YAML files in `directory`, builds a [`Translations`] instance
/// with `fallback` locale and returns a raw pointer to it.
fn load(fallback: impl Into<Box<str>>, directory: impl AsRef<Path>) -> Result<*mut Translations, Error> {
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

        let yaml: Value = from_str(&source).map_err(|source| Error::ParseYaml {
            path: path.clone(),
            message: source.to_string(),
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
            return Err(Error::InvalidYaml {
                path: path.to_path_buf(),
                message: format!("template value must be a string, got {value:?}"),
            });
        },
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
    Plural { name: Box<str>, rules: plural::Ruleset },
}

impl Part {
    /// Parse a placeholder from a string slice containing its body.
    fn parse_placeholder(source: &str) -> Result<Self, &'static str> {
        if is_identifier(source) {
            return Ok(Self::Variable { name: source.into() });
        }

        let (name, rules) = plural::compile(source)?;

        Ok(Self::Plural { name, rules })
    }

    /// Parse the first valid [`Part`] from a string slice and
    /// return it along with the rest of that string slice.
    fn parse_next(source: &str) -> Result<(Self, &str), &'static str> {
        /// Checks if the byte at index `i` has an odd number of `\` bytes before it.
        fn is_escaped(bytes: &[u8], i: usize) -> bool {
            let backslashes = bytes[..i].iter().rev().take_while(|&&b| b == b'\\').count();
            backslashes % 2 == 1
        }

        /// Turns the escaped `\{` sequences of a text part into plain `{`.
        fn unescape(text: &str) -> Box<str> {
            text.replace(r"\{", "{").into_boxed_str()
        }

        let bytes = source.as_bytes();

        // Index of the first `{` that is not escaped. It is ASCII, so
        // slicing the string at it is always on a character boundary.
        let open = source.match_indices('{').map(|(i, _)| i).find(|&i| !is_escaped(bytes, i));

        match open {
            // The source starts with a placeholder.
            Some(0) => {
                let end = source[1..].find('}').map(|end| 1 + end).ok_or("unclosed placeholder")?;
                let part = Self::parse_placeholder(&source[1..end])?;
                Ok((part, &source[end + 1..]))
            },
            // Text up to the next placeholder.
            Some(i) => Ok((Self::Text(unescape(&source[..i])), &source[i..])),
            // Only text is left.
            None => Ok((Self::Text(unescape(source)), "")),
        }
    }
}

/// A compiled representation of a translation value.
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
    fn compile(translation: &str) -> Result<Self, &'static str> {
        let mut parts = Vec::new();
        let mut source = translation;
        let mut bufsize = 0;

        while !source.is_empty() {
            let (part, rest) = match Part::parse_next(source) {
                Ok((part, rest)) => {
                    match &part {
                        Part::Text(text) => bufsize += text.len(),
                        Part::Variable { .. } => {
                            bufsize += Self::ESTIMATED_ARG_LEN;
                        },
                        Part::Plural { .. } => {
                            bufsize += Self::ESTIMATED_ARG_LEN;
                        },
                    }
                    (part, rest)
                },
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
            args.iter().find_map(|(arg_name, value)| (*arg_name == name).then_some(value))
        }

        let mut output = String::with_capacity(self.bufsize);

        for part in &self.parts {
            match part {
                Part::Text(text) => {
                    output.push_str(text);
                },

                Part::Variable { name } =>
                    if let Some(arg) = find_arg(args, name) {
                        arg.write_to(&mut output);
                    },

                Part::Plural { name, rules } =>
                    if let Some(arg) = find_arg(args, name) {
                        rules.render(&mut output, arg);
                    },
            }
        }

        output
    }
}

/// Checks if a string slice is a valid placeholder identifier.
fn is_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// An argument passed to a translation template.
///
/// Wraps the input value to determine the rendering approach.
#[derive(PartialEq, Debug)]
#[repr(u8)]
pub enum Arg<'a> {
    /// Signed integers.
    Int(i64),
    /// Unsigned integers.
    Uint(u64),
    /// 32 bit floating point numbers.
    Float32(f32),
    /// 64 bit floating point numbers.
    Float64(f64),
    /// String values.
    String(&'a str),
}

impl<'a> Arg<'a> {
    /// Checks if `self` is a numeric argument.
    #[inline]
    fn is_numeric(&self) -> bool {
        !matches!(self, Self::String(..))
    }

    /// Checks if `self` is [`Arg::Float32`] or [`Arg::Float64`].
    #[inline]
    fn is_float(&self) -> bool {
        matches!(self, Self::Float32(..) | Self::Float64(..))
    }

    /// Returns the truncated absolute value of a numeric argument.
    #[inline]
    fn abs_trunc(&self) -> u64 {
        match self {
            Self::Int(value) => value.unsigned_abs(),
            Self::Uint(value) => *value,
            Self::Float32(value) => value.abs() as u64,
            Self::Float64(value) => value.abs() as u64,
            Self::String(_) => unreachable!(),
        }
    }

    /// Writes the value to the `output` buffer
    /// using the method corresponding to its variant.
    #[inline]
    fn write_to(&self, output: &mut String) {
        match self {
            Self::Int(value) => {
                itoap::write_to_string(output, *value);
            },
            Self::Uint(value) => {
                itoap::write_to_string(output, *value);
            },
            Self::Float32(value) => {
                let mut buf = zmij::Buffer::new();
                #[cfg(feature = "fast-floats")]
                let n = buf.format_finite(*value);
                #[cfg(not(feature = "fast-floats"))]
                let n = buf.format(*value);
                output.push_str(n);
            },
            Self::Float64(value) => {
                let mut buf = zmij::Buffer::new();
                #[cfg(feature = "fast-floats")]
                let n = buf.format_finite(*value);
                #[cfg(not(feature = "fast-floats"))]
                let n = buf.format(*value);
                output.push_str(n);
            },
            Self::String(value) => {
                output.push_str(value);
            },
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

impl<'a> ToArg<'a> for &'a Box<str> {
    #[inline]
    fn to_arg(self) -> Arg<'a> {
        Arg::String(self)
    }
}

impl<'a, 'c: 'a> ToArg<'a> for &'a std::borrow::Cow<'c, str> {
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

    f32   => Float32, f32,
    f64   => Float64, f64,
);

/// Used by the [`t!`] proc-macro to convert a
/// compatible input to an [`Arg`].
#[doc(hidden)]
#[inline]
pub fn __arg<'a, T>(value: &'a T) -> Arg<'a>
where
    T: ?Sized,
    &'a T: ToArg<'a>,
{
    value.to_arg()
}

/// Generated in place of the [`t!`] proc-macro.
///
/// Panics if [`init`] has not been called.
///
/// Panics if a translation doesn't exist in either the requested or fallback
/// locale, unless the `missing-key-nopanic` feature is enabled, in which case
/// it returns the translation key.
#[doc(hidden)]
pub fn __get(locale: &str, key: &str, args: &[(&str, Arg<'_>)]) -> String {
    let storage = unsafe { STORAGE.load(Ordering::Acquire).as_ref() }.expect("ezlz::init() must be called before using t!");

    let template = storage
        .locales
        .get(locale)
        .and_then(|locale| locale.get(key))
        .or_else(|| storage.locales.get(&storage.fallback).and_then(|locale| locale.get(key)));

    match template {
        Some(translation) => translation.render(args),
        None => {
            #[cfg(feature = "missing-key-nopanic")]
            return key.to_string();
            #[cfg(not(feature = "missing-key-nopanic"))]
            panic!("translation '{key}' not found in locale '{locale}' and fallback locale");
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(template: &str, args: &[(&str, Arg)]) -> String {
        Template::compile(template).unwrap().render(args)
    }

    #[test]
    fn identifiers() {
        for valid in ["x", "_", "_x", "x1", "X", "a_b_c", "1"] {
            assert!(is_identifier(valid), "{valid:?}");
        }
        for invalid in ["", " ", "a b", "a-b", "a.b", "é", "x|y", "{x}", "a\n"] {
            assert!(!is_identifier(invalid), "{invalid:?}");
        }
    }

    #[test]
    fn text_and_placeholders() {
        assert_eq!(render("", &[]), "");
        assert_eq!(render("plain", &[]), "plain");
        assert_eq!(render("{x}", &[("x", Arg::Int(1))]), "1");
        assert_eq!(render("a{x}b{y}c", &[("x", Arg::Int(1)), ("y", Arg::String("z"))]), "a1bzc");
        assert_eq!(render("{x}{x}", &[("x", Arg::Uint(7))]), "77");
        assert_eq!(render("é{x}ü 你好{x}🎉", &[("x", Arg::Int(1))]), "é1ü 你好1🎉");
    }

    #[test]
    fn stray_closing_brace_is_text() {
        assert_eq!(render("a}b", &[]), "a}b");
        assert_eq!(render("}{x}}", &[("x", Arg::Int(1))]), "}1}");
    }

    #[test]
    fn escapes() {
        let x = [("x", Arg::String("v"))];
        // An odd number of backslashes escapes the brace and removes one backslash.
        assert_eq!(render(r"\{x}", &x), "{x}");
        assert_eq!(render(r"a \{x} b", &x), "a {x} b");
        assert_eq!(render(r"\\\{x}", &x), r"\\{x}");
        // An even number leaves the placeholder active and keeps every backslash.
        assert_eq!(render(r"\\{x}", &x), r"\\v");
        assert_eq!(render(r"\\\\{x}", &x), r"\\\\v");
        // Backslashes elsewhere are plain text.
        assert_eq!(render(r"a\b", &x), r"a\b");
        assert_eq!(render(r"\", &x), r"\");
        assert_eq!(render(r"{x}\", &x), r"v\");
        assert_eq!(render(r"{x}\{x}", &x), r"v{x}");
        assert_eq!(render(r"\{\{", &x), "{{");
        assert_eq!(render(r"é\{x}", &x), "é{x}");
    }

    #[test]
    fn compile_errors() {
        for (template, message) in [
            ("{x", "unclosed placeholder"),
            ("abc {", "unclosed placeholder"),
            ("{", "unclosed placeholder"),
            ("{}", "invalid plural placeholder identifier"),
            ("{ x }", "invalid plural placeholder identifier"),
            ("{n|?: x}", "invalid plural rule selector operator"),
            ("{n|=5-1: x}", "selector range lower bound is greater than its upper bound"),
        ] {
            assert_eq!(Template::compile(template).unwrap_err(), message, "{template:?}");
        }
        // A closing brace can never be escaped: the first `}` always ends the placeholder.
        assert_eq!(
            Template::compile(r"{x\}}").unwrap_err(),
            "invalid plural placeholder identifier"
        );
    }

    #[test]
    fn buffer_size_estimate() {
        assert_eq!(Template::compile("").unwrap().bufsize, 0);
        assert_eq!(Template::compile("abc").unwrap().bufsize, 3);
        assert_eq!(Template::compile("abc{x}").unwrap().bufsize, 3 + Template::ESTIMATED_ARG_LEN);
        assert_eq!(
            Template::compile("{x}{n|=1: a| b}").unwrap().bufsize,
            2 * Template::ESTIMATED_ARG_LEN
        );
    }

    #[test]
    fn abs_trunc() {
        assert_eq!(Arg::Int(-5).abs_trunc(), 5);
        assert_eq!(Arg::Int(i64::MIN).abs_trunc(), 1 << 63);
        assert_eq!(Arg::Uint(u64::MAX).abs_trunc(), u64::MAX);
        assert_eq!(Arg::Float64(-1.9).abs_trunc(), 1);
        assert_eq!(Arg::Float64(0.99).abs_trunc(), 0);
        assert_eq!(Arg::Float32(-2.5).abs_trunc(), 2);
        assert_eq!(Arg::Float64(f64::NAN).abs_trunc(), 0);
        assert_eq!(Arg::Float64(f64::INFINITY).abs_trunc(), u64::MAX);
        assert_eq!(Arg::Float32(f32::NEG_INFINITY).abs_trunc(), u64::MAX);
        assert_eq!(Arg::Float64(-0.0).abs_trunc(), 0);
    }

    #[test]
    fn writing_values() {
        fn write(arg: Arg) -> String {
            let mut output = String::new();
            arg.write_to(&mut output);
            output
        }
        assert_eq!(write(Arg::Int(-7)), "-7");
        assert_eq!(write(Arg::Uint(7)), "7");
        assert_eq!(write(Arg::Float64(1.5)), "1.5");
        assert_eq!(write(Arg::Float32(1.1)), "1.1");
        assert_eq!(write(Arg::Float64(f64::from(1.1_f32))), "1.100000023841858");
        assert_eq!(write(Arg::String("é")), "é");
    }

    #[test]
    fn to_arg_conversions() {
        assert_eq!(__arg(&-3_i8), Arg::Int(-3));
        assert_eq!(__arg(&3_u16), Arg::Uint(3));
        assert_eq!(__arg(&usize::MAX), Arg::Uint(usize::MAX as u64));
        assert_eq!(__arg(&1.5_f32), Arg::Float32(1.5));
        assert_eq!(__arg(&1.5_f64), Arg::Float64(1.5));
        assert_eq!(__arg(&"s"), Arg::String("s"));
        assert_eq!(__arg(&String::from("s")), Arg::String("s"));
    }
}
