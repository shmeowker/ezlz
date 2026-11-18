//! This module handles the parsing, compilation and rendering of plural placeholders.

use std::iter::Peekable;
use std::mem::take;
use std::str::Chars;

use crate::Arg;

/// An operator for [`Selector`]s.
#[derive(Debug, PartialEq, Clone, Copy)]
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

/// Selector for a plural [`Rule`].
#[derive(Debug, PartialEq, Clone, Copy)]
struct Selector {
    /// The selector operator.
    op: Op,
    /// Range lower bound.
    lo: u8,
    /// Range upper bound.
    hi: u8,
}

impl Selector {
    const FALLBACK: Self = Self {
        op: Op::Fallback,
        lo: u8::MAX,
        hi: u8::MAX,
    };
    const FLOAT: Self = Self {
        op: Op::Float,
        lo: u8::MAX,
        hi: u8::MAX,
    };

    /// Matches the selector against the absolute truncated integer
    /// numeric value of an [`Arg`] according to [`Self::op`].
    ///
    /// See the Pluralization section of README.md for details.
    #[inline]
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

/// A plural rule.
#[derive(Debug)]
struct Rule {
    /// The rule selector.
    sel: Selector,
    /// The text value of the rule.
    text: Box<str>,
    /// Replacement flag.
    ///
    /// Indicates that this rule is supposed to replace the
    /// number instead of appending to it.
    replace: bool,
}

/// The ruleset of a plural placeholder.
#[derive(Debug)]
pub(crate) struct Ruleset {
    /// The list of compiled [`Rule`]s.
    rules: Box<[Rule]>,
}

impl Ruleset {
    /// Renders a [`Rule`] matching the value of an [`Arg`].
    ///
    /// The rendered strings are written directly to the `output` buffer.
    /// If the provided [`Arg`] is not numeric, writes it as is and aborts.
    pub fn render(&self, output: &mut String, arg: &Arg<'_>) {
        if !arg.is_numeric() {
            arg.write_to(output);
            return;
        }

        let Some(rule) = self.rules.iter().find(|r| r.sel.matches(arg)) else {
            arg.write_to(output);
            return;
        };

        if rule.replace {
            output.push_str(&rule.text);
            return;
        }

        arg.write_to(output);
        output.push_str(&rule.text);
    }
}

impl TryFrom<&mut Peekable<Chars<'_>>> for Ruleset {
    type Error = &'static str;

    fn try_from(iter: &mut Peekable<Chars<'_>>) -> Result<Self, Self::Error> {
        #[derive(Clone, Copy, PartialEq)]
        enum State {
            MaybeSel,
            Text,
            Escaped,
        }

        let mut out = Vec::new();
        let mut state = State::MaybeSel;
        let mut selector = None;
        let mut replace = false;
        let mut buf = String::new();

        while let Some(c) = iter.next() {
            match (state, c) {
                (State::Escaped, '`') => state = State::Text,
                (State::Escaped, _) => buf.push(c),

                (_, '`') =>
                    if iter.next_if_eq(&'`').is_some() {
                        buf.push('`');
                    } else {
                        state = State::Escaped;
                    },

                (State::MaybeSel, ':') => {
                    selector = Some(Selector::try_from(&*buf)?);
                    buf.clear();
                    replace = iter.next_if_eq(&'=').is_some();
                    state = State::Text;
                },

                (_, '|' | '}') => {
                    let mut text = take(&mut buf);

                    // A rule without a `:` is a fallback, its text may start with the marker.
                    if state == State::MaybeSel && text.starts_with('=') {
                        text.remove(0);
                        replace = true;
                    }

                    let sel = selector.take().unwrap_or(Selector::FALLBACK);

                    out.push(Rule {
                        sel,
                        text: text.into_boxed_str(),
                        replace: take(&mut replace),
                    });

                    if c == '}' {
                        return Ok(Self {
                            rules: out.into_boxed_slice(),
                        });
                    }

                    state = State::MaybeSel;
                },

                (State::MaybeSel | State::Text, '{') => return Err("unquoted { in plural rule text"),

                _ => buf.push(c),
            }
        }

        Err("unterminated plural placeholder")
    }
}

impl TryFrom<&str> for Selector {
    type Error = &'static str;

    fn try_from(body: &str) -> Result<Selector, &'static str> {
        /// Parses a range bound. 255 is rejected because it marks an open upper bound.
        fn bound(s: &str) -> Option<u8> {
            s.parse::<u8>().ok().filter(|&n| n != u8::MAX)
        }

        if body.is_empty() {
            return Ok(Selector::FALLBACK);
        }
        if body == "." {
            return Ok(Selector::FLOAT);
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
            let lo = bound(lo).ok_or("invalid rule selector open range bound")?;
            return Ok(Selector { op, lo, hi: u8::MAX });
        }

        if let Some((lo, hi)) = range.split_once('-') {
            let lo = bound(lo).ok_or("invalid rule selector range lower bound")?;
            let hi = bound(hi).ok_or("invalid rule selector range upper bound")?;

            if lo > hi {
                return Err("selector range lower bound is greater than its upper bound");
            }

            return Ok(Selector { op, lo, hi });
        }

        let n = bound(range).ok_or("invalid rule selector range")?;

        Ok(Selector { op, lo: n, hi: n })
    }
}

#[cfg(test)]
#[path = "../unit_tests/plural_tests.rs"]
mod tests;
