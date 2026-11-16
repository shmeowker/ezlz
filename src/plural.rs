//! This module handles the parsing, compilation and rendering
//! of plural placeholders.
use crate::{Arg, is_identifier};

/// Selector operator for [`Rule`]s.
#[derive(Debug, PartialEq)]
#[repr(u8)]
enum Op {
    // The `.` selector.
    Float,
    // The `=` selector.
    Equals,
    // The `~` selector.
    FloatRange,
    // The `%` selector.
    Mod10,
    // The `#` selector.
    Mod100,
    // Unconditional.
    Fallback,
}

impl TryFrom<u8> for Op {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let op = match value {
            b'.' => Self::Float,
            b'=' => Self::Equals,
            b'~' => Self::FloatRange,
            b'%' => Self::Mod10,
            b'#' => Self::Mod100,
            _ => return Err("invalid plural rule selector operator"),
        };

        Ok(op)
    }
}

/// A plural rule.
#[derive(Debug)]
struct Rule {
    /// The selector operator.
    op: Op,
    /// Range lower bound.
    lo: u8,
    /// Range upper bound.
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
            Op::Float => return n.is_float(),
            Op::Equals | Op::FloatRange => i,
            Op::Mod10 => i % 10,
            Op::Mod100 => i % 100,
            Op::Fallback => return true,
        };
        if n.is_float() && self.op != Op::FloatRange {
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
pub(crate) struct Ruleset {
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
/// [`Rule::op`], [`Rule::lo`] and [`Rule::hi`]
/// or an error message if the selector is invalid.
fn parse_selector(body: &str) -> Result<(Op, u8, u8), &'static str> {
    if body.is_empty() {
        return Ok((Op::Fallback, u8::MAX, u8::MAX));
    }
    if body == "." {
        return Ok((Op::Float, u8::MAX, u8::MAX));
    }

    let bytes = body.as_bytes();
    let op = Op::try_from(bytes[0])?;

    if op == Op::Float {
        return Err("the `.` selector does not take a range");
    }

    let Ok(range) = std::str::from_utf8(&bytes[1..]) else {
        return Err("invalid UTF-8 in rule selector range body");
    };

    if let Some(lo) = range.strip_suffix('+') {
        if let Ok(lo) = lo.parse::<u8>()
            && lo != u8::MAX
        {
            return Ok((op, lo, u8::MAX));
        };

        return Err("invalid rule selector open range bound");
    }

    if let Some((lo, hi)) = range.split_once('-') {
        if let Ok(lo) = lo.parse::<u8>()
            && lo != u8::MAX
        {
            if let Ok(hi) = hi.parse::<u8>()
                && hi != u8::MAX
            {
                return Ok((op, lo, hi));
            };
            return Err("invalid rule selector range upper bound");
        };
        return Err("invalid rule selector range lower bound");
    };

    if let Ok(n) = range.parse::<u8>()
        && n != u8::MAX
    {
        return Ok((op, n, n));
    };

    Err("invalid rule selector range")
}

/// Compiles a plural placeholder from a string slice containing its body.
///
/// On success, returns the identifier and a compiled [`Ruleset`].
/// Returns an error message if the provided string slice doesn't contain
/// a valid plural placeholder body.
pub(crate) fn compile(input: &str) -> Result<(Box<str>, Ruleset), &'static str> {
    let mut parts = input.split('|');

    let Some(name) = parts.next() else {
        return Err("placeholder without an identifier");
    };
    if !is_identifier(name) {
        return Err("invalid plural placeholder identifier");
    }

    let mut rules = Vec::new();

    for part in parts {
        let (op, lo, hi, text) = if let Some((sel, text)) = part.split_once(':') {
            let (op, lo, hi) = parse_selector(sel)?;
            (op, lo, hi, text)
        } else {
            (Op::Fallback, u8::MAX, u8::MAX, part)
        };

        if lo > hi {
            return Err("selector range lower bound is greater than its upper bound");
        }
        if text.contains('{') {
            return Err("`{` is not allowed in plural rule text");
        }

        let replace = text.starts_with('=');
        let text = if replace { Box::from(&text[1..]) } else { Box::from(text) };

        rules.push(Rule {
            op,
            lo,
            hi,
            text,
            replace,
        });
    }

    Ok((name.into(), Ruleset { rules: rules.into() }))
}

#[cfg(test)]
#[path = "../unit_tests/plural_tests.rs"]
mod tests;
