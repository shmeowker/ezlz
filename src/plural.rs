//! This module handles the parsing, compilation and rendering
//! of plural placeholders.
use crate::{Arg, is_identifier};

/// A plural rule.
#[derive(Debug)]
struct Rule {
    /// The selector operator.
    ///
    /// Can be `.`, `=`, `~`, `%`, or `#`.
    /// Other values trigger fallback.
    op: u8,
    /// Lower range bound of the selector.
    lo: u8,
    /// Upper range bound of the selector.
    hi: u8,
    /// The text value of the rule.
    text: Box<str>,
    /// Replacement flag.
    ///
    /// Indicates that this rule is supposed to replace the
    /// number instead of appending to it.
    replace: bool,
}

impl Rule {
    /// Matches the rule against the absolute truncated integer
    /// numeric value of an [`Arg`] according to the value of
    /// [`Rule::op`].
    ///
    /// See the Pluralization section of README.md for details.
    fn matches(&self, n: &Arg) -> bool {
        let i = n.abs_trunc();
        let x = match self.op {
            b'.' => return n.is_float(),
            b'=' | b'~' => i,
            b'%' => i % 10,
            b'#' => i % 100,
            _ => return true,
        };
        if n.is_float() && self.op != b'~' {
            return false;
        }
        if self.hi == u8::MAX {
            self.lo as u64 <= x
        } else {
            self.lo as u64 <= x && x <= self.hi as u64
        }
    }
}

/// The plural rules of a plural placeholder.
#[derive(Debug)]
pub struct Ruleset {
    /// The list of compiled [`Rule`]s.
    rules: Box<[Rule]>,
}

impl Ruleset {
    /// Returns the first [`Rule`] that matches a number.
    #[inline]
    fn select(&self, n: &Arg) -> Option<&Rule> {
        self.rules.iter().find(|r| r.matches(n))
    }

    /// Renders a [`Rule`] matching the value of an [`Arg`].
    ///
    /// The rendered strings are written directly to the `output` buffer.
    /// If the provided [`Arg`] is not numeric, writes it as is and aborts.
    pub fn render(&self, output: &mut String, arg: &Arg<'_>) {
        if !arg.is_numeric() {
            arg.write_to(&mut *output);
            return;
        }

        let Some(rule) = self.select(arg) else {
            arg.write_to(&mut *output);
            return;
        };

        if rule.replace {
            output.push_str(&rule.text);
            return;
        }

        arg.write_to(&mut *output);
        output.push_str(&rule.text);
    }
}

/// Parses a plural rule selector.
///
/// Returns 3 values corresponding to
/// [`Rule::op`], [`Rule::lo`] and [`Rule::hi`].
fn selector(s: &str) -> (u8, u8, u8) {
    if s.is_empty() {
        return (u8::MAX, u8::MAX, u8::MAX);
    }
    if s == "." {
        return (b'.', u8::MAX, u8::MAX);
    }

    let b = s.as_bytes();
    let (op, range) = unsafe { (b[0], str::from_utf8_unchecked(&b[1..])) };
    if let Some(lo) = range.strip_suffix('+') {
        return (op, lo.parse().unwrap(), u8::MAX);
    }
    if let Some((lo, hi)) = range.split_once('-') {
        return (op, lo.parse().unwrap(), hi.parse().unwrap());
    };

    let n = range.parse().unwrap();
    (op, n, n)
}

/// Compiles a plural placeholder from a string slice containing its body.
///
/// On success, returns the identifier and a compiled [`Ruleset`].
/// Returns [`None`] if the provided string slice doesn't contain
/// a valid plural placeholder body.
pub fn compile(input: &str) -> Option<(Box<str>, Ruleset)> {
    let mut parts = input.split('|');

    let name = parts.next()?;
    if !is_identifier(name) {
        return None;
    }

    let mut rules = Vec::new();

    for part in parts {
        let (op, lo, hi, text) = if let Some((sel, text)) = part.split_once(':') {
            let (op, lo, hi) = selector(sel);
            (op, lo, hi, text)
        } else {
            (0xff, 0xff, 0xff, part)
        };
        let replace = text.starts_with('=');
        let text = if replace {
            Box::from(&text[1..])
        } else {
            Box::from(text)
        };
        rules.push(Rule {
            op,
            lo,
            hi,
            text,
            replace,
        });
    }

    Some((
        name.into(),
        Ruleset {
            rules: rules.into_boxed_slice(),
        },
    ))
}
