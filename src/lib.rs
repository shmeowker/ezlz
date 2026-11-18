#![doc = include_str!("../README.md")]
#![allow(rustdoc::private_intra_doc_links)]
#![warn(missing_docs)]

use std::mem::take;
use std::num::NonZero;
use std::path::Path;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ahash::{HashMap, HashMapExt};

mod error;
mod loader;
mod plural;
pub use error::Error;
pub use ezlz_macros::{t, wt};
use loader::load;
use plural::Ruleset;

/// Global localization store.
static STORAGE: AtomicPtr<Translations> = AtomicPtr::new(null_mut());
/// How long an old storage is kept alive in memory
/// after calling [`reinit`], in seconds.
static GRACE_PERIOD: AtomicU64 = AtomicU64::new(60);

/// Sets how long the old storage is kept alive in memory after
/// calling [`reinit`]. This delay is 60 seconds by default.
pub fn set_grace_period(seconds: NonZero<u64>) {
    GRACE_PERIOD.store(seconds.get(), Ordering::Relaxed);
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
/// fn lang() -> String {
///     "ru".to_string()
/// }
///
/// t!("en", messages.hello);
/// t!("en_GB", messages.greet, name = "Anna");
/// // If the variable name matches placeholder name,
/// // using explicit placeholder names is unnecessary:
/// let name = "Anna";
/// t!(lang(), messages.greet, name);
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

/// Initializes a new storage and atomically swaps the old one with it.
///
/// # Safety
/// The old storage is dropped by a background thread after the grace period
/// (60 seconds by default, see [`set_grace_period`]). You must guarantee that,
/// by then, no thread is still inside a [`t!`] or [`wt!`] call that started
/// before this swap. Argument expressions are evaluated before the lookup
/// begins and do not count.
///
/// Such a call normally finishes in well under a millisecond, but a thread can
/// be stalled in the middle of it: preempted, starved under heavy load, blocked
/// on a page fault or paused by a debugger. If it resumes after the old storage
/// was dropped, it reads freed memory, which is undefined behavior.
///
/// Use a grace period far longer than any stall you consider possible, or use
/// [`reinit_leak`], which never frees the old storage and has no such requirement.
pub unsafe fn reinit(fallback: impl Into<Box<str>>, directory: impl AsRef<Path>) -> Result<(), Error> {
    let new = load(fallback, directory)?;
    let old = STORAGE.swap(new, Ordering::AcqRel);

    if !old.is_null() {
        let addr = old as usize;
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(GRACE_PERIOD.load(Ordering::Acquire)));
            unsafe {
                let _ = Box::from_raw(addr as *mut Translations);
            }
        });
    }

    Ok(())
}

/// Initializes a new storage and atomically swaps the old one with it,
/// leaking the latter.
pub fn reinit_leak(fallback: impl Into<Box<str>>, directory: impl AsRef<Path>) -> Result<(), Error> {
    let ptr = load(fallback, directory)?;
    STORAGE.store(ptr, Ordering::Release);

    Ok(())
}

/// Stores all compiled [`Template`]s for all locales along
/// with the name of the fallback locale.
#[derive(Debug)]
pub struct Translations {
    fallback: Box<str>,
    locales: HashMap<Box<str>, HashMap<Box<str>, Template>>,
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

/// A compiled representation of a translation value.
#[derive(Debug)]
struct Template {
    /// List of the compiled template parts.
    parts: Box<[Part]>,
    /// Estimated size of a rendered template.
    ///
    /// It is calculated during parsing.
    /// Used as the size of the string buffer in [`Template::render`].
    bufsize: usize,
}

impl TryFrom<&str> for Template {
    type Error = &'static str;

    fn try_from(src: &str) -> Result<Self, Self::Error> {
        #[derive(PartialEq, Eq)]
        enum State {
            Text,
            Ident,
        }

        let mut out = Vec::new();
        let mut state = State::Text;
        let mut iter = src.chars().peekable();
        let mut buf = String::new();
        let mut bufsize = 0;

        while let Some(c) = iter.next() {
            match state {
                State::Text => match c {
                    '{' => {
                        if iter.peek() == Some(&'{') {
                            buf.push('{');
                            bufsize += 1;
                            iter.next();
                            continue;
                        }

                        if !buf.is_empty() {
                            let text = take(&mut buf).into_boxed_str();
                            let part = Part::Text(text);
                            out.push(part);
                        }

                        state = State::Ident;
                    },

                    _ => {
                        bufsize += c.len_utf8();
                        buf.push(c);
                    },
                },

                State::Ident => match c {
                    '}' => {
                        if !is_identifier(&buf) {
                            return Err("invalid placeholder identifier");
                        }

                        let name = take(&mut buf).into_boxed_str();
                        let part = Part::Variable { name };
                        bufsize += Self::ESTIMATED_VAR_LEN;
                        out.push(part);
                        state = State::Text;
                    },

                    '|' => {
                        if !is_identifier(&buf) {
                            return Err("invalid plural placeholder identifier");
                        }

                        let rules = Ruleset::try_from(&mut iter)?;
                        let name = take(&mut buf).into_boxed_str();
                        let part = Part::Plural { name, rules };

                        bufsize += Self::ESTIMATED_VAR_LEN;
                        out.push(part);
                        state = State::Text;
                    },

                    _ => buf.push(c),
                },
            }
        }

        if state != State::Text {
            return Err("unterminated placeholder");
        }

        if !buf.is_empty() {
            let text = take(&mut buf).into_boxed_str();
            let part = Part::Text(text);
            out.push(part);
        }

        Ok(Self {
            parts: out.into_boxed_slice(),
            bufsize,
        })
    }
}

impl Template {
    /// Estimated size of a placeholder's rendered value.
    const ESTIMATED_VAR_LEN: usize = 32;

    /// Renders a template into a new [`String`] and returns it.
    fn render(&self, args: &[(&str, Arg<'_>)]) -> String {
        let mut output = String::with_capacity(self.bufsize);
        self.render_to(&mut output, args);

        output
    }

    /// Iterates over the [`Part`]s of a template and
    /// renders each part in a way corresponding to its
    /// variant into the `output` buffer.
    #[inline(always)]
    fn render_to(&self, output: &mut String, args: &[(&str, Arg<'_>)]) {
        /// Find an [`Arg`] by its `name` in a list of `args`.
        #[inline]
        fn find_arg<'a>(args: &'a [(&str, Arg<'a>)], name: &str) -> Option<&'a Arg<'a>> {
            args.iter().find_map(|(arg_name, value)| (*arg_name == name).then_some(value))
        }

        for part in self.parts.iter() {
            match part {
                Part::Text(text) => output.push_str(text),

                Part::Variable { name } =>
                    if let Some(arg) = find_arg(args, name) {
                        arg.write_to(output);
                    },

                Part::Plural { name, rules } =>
                    if let Some(arg) = find_arg(args, name) {
                        rules.render(output, arg);
                    },
            }
        }
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

/// Attempts to get the requested template from the requested locale and,
/// if not found, fallback locale.
///
/// Panics if the storage is not initialized.
#[inline(always)]
fn fetch_template(locale: &str, key: &str) -> Option<&'static Template> {
    let storage = unsafe { STORAGE.load(Ordering::Acquire).as_ref() }.expect("ezlz::init() must be called before using t!");

    storage
        .locales
        .get(locale)
        .and_then(|locale| locale.get(key))
        .or_else(|| storage.locales.get(&storage.fallback).and_then(|locale| locale.get(key)))
}

/// Defines the missing key render behavior.
///
/// Panics if a translation doesn't exist in either the requested or fallback
/// locale, unless the `stringify-missing-keys` feature is enabled, in which case
/// it returns the translation key.
#[cold]
fn missing_key(locale: &str, key: &str) -> String {
    #[cfg(feature = "stringify-missing-keys")]
    {
        let _ = locale;
        key.to_string()
    }
    #[cfg(not(feature = "stringify-missing-keys"))]
    panic!("translation '{key}' not found in locale '{locale}' and fallback locale");
}

/// Used by the proc-macros to convert a
/// compatible input to an [`Arg`].
#[doc(hidden)]
#[inline(always)]
pub fn __arg<'a, T>(value: &'a T) -> Arg<'a>
where
    T: ?Sized,
    &'a T: ToArg<'a>,
{
    value.to_arg()
}

/// Renders the requested template into a new [`String`] and returns it.
#[doc(hidden)]
pub fn __get(locale: &str, key: &str, args: &[(&str, Arg<'_>)]) -> String {
    match fetch_template(locale, key) {
        Some(template) => template.render(args),
        None => missing_key(locale, key),
    }
}

/// Renders the requested template into the `output` buffer.
#[doc(hidden)]
pub fn __write(output: &mut String, locale: &str, key: &str, args: &[(&str, Arg<'_>)]) {
    match fetch_template(locale, key) {
        Some(template) => {
            output.reserve(template.bufsize);
            template.render_to(output, args);
        },
        None => output.push_str(&missing_key(locale, key)),
    }
}

#[cfg(test)]
#[path = "../unit_tests/lib_tests.rs"]
mod tests;
