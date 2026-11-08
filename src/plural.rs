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
        if self.hi == !0 {
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
        return Ok((Op::Fallback, !0, !0));
    }
    if body == "." {
        return Ok((Op::Float, !0, !0));
    }

    let bytes = body.as_bytes();
    let op = Op::try_from(bytes[0])?;
    let Ok(range) = std::str::from_utf8(&bytes[1..]) else {
        return Err("invalid UTF-8 in rule selector range body");
    };

    if let Some(lo) = range.strip_suffix('+') {
        let Ok(lo) = lo.parse::<u8>() else {
            return Err("invalid rule selector open range bound");
        };
        return Ok((op, lo, !0));
    }

    if let Some((lo, hi)) = range.split_once('-') {
        let Ok(lo) = lo.parse::<u8>() else {
            return Err("invalid rule selector range lower bound");
        };
        let Ok(hi) = hi.parse::<u8>() else {
            return Err("invalid rule selector range upper bound");
        };
        return Ok((op, lo, hi));
    };

    let Ok(n) = range.parse::<u8>() else {
        return Err("invalid rule selector range");
    };

    Ok((op, n, n))
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
            (Op::Fallback, !0, !0, part)
        };

        if lo > hi {
            return Err("selector range lower bound is greater than its upper bound");
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
mod tests {
    use super::*;

    fn render(body: &str, arg: Arg) -> String {
        let (_, ruleset) = compile(body).unwrap();
        let mut output = String::new();
        ruleset.render(&mut output, &arg);
        output
    }

    #[test]
    fn selectors_parse_to_op_and_bounds() {
        assert_eq!(parse_selector("").unwrap(), (Op::Fallback, !0, !0));
        assert_eq!(parse_selector(".").unwrap(), (Op::Float, !0, !0));
        assert_eq!(parse_selector("=1").unwrap(), (Op::Equals, 1, 1));
        assert_eq!(parse_selector("=0-1").unwrap(), (Op::Equals, 0, 1));
        assert_eq!(parse_selector("=9+").unwrap(), (Op::Equals, 9, !0));
        assert_eq!(parse_selector("~2+").unwrap(), (Op::FloatRange, 2, !0));
        assert_eq!(parse_selector("%2-4").unwrap(), (Op::Mod10, 2, 4));
        assert_eq!(parse_selector("#11-19").unwrap(), (Op::Mod100, 11, 19));
        assert_eq!(parse_selector("=254").unwrap(), (Op::Equals, 254, 254));
    }

    #[test]
    fn invalid_selectors_are_rejected() {
        for selector in [
            "?", "?1", "x", " ", "=", "=a", "=-", "=1-", "=-1", "=+", "=1+2", "=256", "=1000", "=1-256", "=1.5", "=-1-2",
        ] {
            assert!(parse_selector(selector).is_err(), "{selector:?} should be rejected");
        }
    }

    #[test]
    fn reversed_ranges_are_rejected() {
        assert!(compile("n|=5-1: x").is_err());
        assert!(compile("n|=1-1: x").is_ok());
        assert!(compile("n|=5+: x").is_ok());
    }

    #[test]
    fn placeholder_names_are_validated() {
        assert!(compile("").is_err());
        assert!(compile(" n|=1: x").is_err());
        assert!(compile("a-b|=1: x").is_err());
        assert!(compile("é|=1: x").is_err());
        assert_eq!(&*compile("n_1|=1: x").unwrap().0, "n_1");
    }

    #[test]
    fn rule_text_is_split_at_the_first_colon_only() {
        assert_eq!(render("n|=1: a: b", Arg::Int(1)), "1 a: b");
        assert_eq!(render("n|: a: b", Arg::Int(1)), "1 a: b");
        assert_eq!(render("n|=1:=a:b", Arg::Int(1)), "a:b");
    }

    #[test]
    fn rules_are_evaluated_left_to_right() {
        assert_eq!(render("n|=1: a|=1: b| c", Arg::Int(1)), "1 a");
        assert_eq!(
            render("n| c|=1: a", Arg::Int(1)),
            "1 c",
            "an early fallback shadows later rules"
        );
    }

    #[test]
    fn no_rules_and_empty_text() {
        assert_eq!(render("n", Arg::Int(5)), "5");
        assert_eq!(render("n|", Arg::Int(5)), "5");
        assert_eq!(render("n|=5:", Arg::Int(5)), "5");
        assert_eq!(render("n|=5:=", Arg::Int(5)), "");
    }

    #[test]
    fn integer_matching() {
        assert_eq!(render("n|=0-1: a| b", Arg::Uint(1)), "1 a");
        assert_eq!(render("n|=0-1: a| b", Arg::Uint(2)), "2 b");
        assert_eq!(render("n|=2+: a| b", Arg::Uint(u64::MAX)), "18446744073709551615 a");
        assert_eq!(render("n|=1: a| b", Arg::Int(-1)), "-1 a");
        assert_eq!(render("n|=1: a| b", Arg::Int(i64::MIN)), "-9223372036854775808 b");
        assert_eq!(render("n|%0: a| b", Arg::Uint(1000)), "1000 a");
        assert_eq!(render("n|#0: a| b", Arg::Uint(1000)), "1000 a");
        assert_eq!(render("n|%5+: a| b", Arg::Uint(17)), "17 a");
    }

    #[test]
    fn floats_only_match_float_aware_selectors() {
        for value in [1.0_f64, 1.5, 21.0] {
            for arg in [|v: f64| Arg::Float64(v), |v: f64| Arg::Float32(v as f32)] {
                let arg = arg(value);
                assert!(arg.is_float());
                let ends_with = |body: &str, suffix: &str| render(body, arg_clone(&arg)).ends_with(suffix);
                assert!(ends_with("n|=1: a| b", " b"), "`=` must not match {value}");
                assert!(ends_with("n|%1: a| b", " b"), "`%` must not match {value}");
                assert!(ends_with("n|#1: a| b", " b"), "`#` must not match {value}");
                assert!(ends_with("n|.: a| b", " a"), "`.` must match {value}");
                assert!(ends_with("n|~0-99: a| b", " a"), "`~` must match {value}");
            }
        }
        assert!(!Arg::Int(1).is_float() && !Arg::Uint(1).is_float() && !Arg::String("1").is_float());
    }

    /// `Arg` is not `Clone`; rebuild it from its numeric value.
    fn arg_clone<'a>(arg: &Arg<'a>) -> Arg<'a> {
        match arg {
            Arg::Float64(v) => Arg::Float64(*v),
            Arg::Float32(v) => Arg::Float32(*v),
            _ => unreachable!(),
        }
    }

    #[test]
    fn strings_bypass_rules() {
        assert_eq!(render("n|=1: a| b", Arg::String("one")), "one");
        assert_eq!(render("n|: a", Arg::String("one")), "one");
        assert_eq!(render("n|=1:=x", Arg::String("")), "");
    }
}
