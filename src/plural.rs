use crate::{Arg, is_identifier};

#[derive(Debug)]
struct Rule {
    op: u8,
    lo: u8,
    hi: u8,
    text: Box<str>,
    replace: bool,
}

impl Rule {
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
        if self.hi == 0xff {
            self.lo as u64 <= x
        } else {
            self.lo as u64 <= x && x <= self.hi as u64
        }
    }
}

#[derive(Debug)]
pub struct Ruleset {
    rules: Box<[Rule]>,
}

impl Ruleset {
    #[inline]
    fn select(&self, n: &Arg) -> Option<&Rule> {
        self.rules.iter().find(|r| r.matches(n))
    }

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

fn selector(s: &str) -> (u8, u8, u8) {
    if s.is_empty() {
        return (0xff, 0xff, 0xff);
    }
    if s == "." {
        return (b'.', 0xff, 0xff);
    }

    let b = s.as_bytes();
    let (op, range) = unsafe { (b[0], str::from_utf8_unchecked(&b[1..])) };
    if let Some(lo) = range.strip_suffix('+') {
        return (op, lo.parse().unwrap(), 0xff);
    }
    if let Some((lo, hi)) = range.split_once('-') {
        return (op, lo.parse().unwrap(), hi.parse().unwrap());
    };

    let n = range.parse().unwrap();
    (op, n, n)
}

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
