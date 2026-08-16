use std::{
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub use ezlz_macros::t;

/// Global localization store.
///
/// `init()` populates this exactly once.
static TRANSLATIONS: OnceLock<Translations> = OnceLock::new();

struct Translations {
    fallback: String,
    locales: HashMap<String, HashMap<String, Translation>>,
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }

            Self::ParseYaml { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            }

            Self::InvalidYaml { path, message } => {
                write!(f, "invalid YAML in {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Initialize ezlz from a localization directory.
///
/// Each `.yml` file represents a locale:
///
/// ```text
/// locales/
/// ├── en.yml
/// ├── en_GB.yml
/// └── ru.yml
/// ```
///
/// Nested YAML mappings become dotted translation keys:
///
/// ```yaml
/// messages:
///   hello: "Hello"
///   greet: "Hello, %{name}!"
/// ```
///
/// becomes:
///
/// ```text
/// messages.hello
/// messages.greet
/// ```
pub fn init(fallback: impl Into<String>, directory: impl AsRef<Path>) -> Result<(), Error> {
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

        if extension != "yml" {
            continue;
        }

        let Some(locale) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let source = fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;

        let yaml: serde_yaml::Value =
            serde_yaml::from_str(&source).map_err(|source| Error::ParseYaml {
                path: path.clone(),
                source,
            })?;

        let mut translations = HashMap::new();

        flatten_yaml(&path, &yaml, String::new(), &mut translations)?;

        locales.insert(locale.to_owned(), translations);
    }

    TRANSLATIONS
        .set(Translations { fallback, locales })
        .map_err(|_| Error::InvalidYaml {
            path: directory.to_path_buf(),
            message: "ezlz::init() was called more than once".to_owned(),
        })?;

    Ok(())
}

/// Flatten YAML mappings into dotted translation keys and compile
/// each translation at initialization time.
fn flatten_yaml(
    path: &Path,
    value: &serde_yaml::Value,
    prefix: String,
    output: &mut HashMap<String, Translation>,
) -> Result<(), Error> {
    match value {
        serde_yaml::Value::Mapping(mapping) => {
            for (key, value) in mapping {
                let Some(key) = key.as_str() else {
                    return Err(Error::InvalidYaml {
                        path: path.to_path_buf(),
                        message: "translation keys must be strings".to_owned(),
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

        serde_yaml::Value::String(value) => {
            let translation = Translation::compile(value);

            output.insert(prefix, translation);
        }

        _ => {
            return Err(Error::InvalidYaml {
                path: path.to_path_buf(),
                message: format!("translation value must be a string, got {value:?}"),
            });
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------------
// Compiled translations
// -----------------------------------------------------------------------------

struct Translation {
    parts: Vec<Part>,
}

enum Part {
    Text(String),

    Argument {
        name: String,
    },

    Plural {
        name: String,
        prepend: bool,
        rules: Vec<PluralRule>,
    },
}

struct PluralRule {
    threshold: usize,
    thresholded: bool,
    replace: bool,
    value: String,
}

impl Translation {
    fn compile(template: &str) -> Self {
        let mut parts = Vec::new();
        let mut text_start = 0;
        let bytes = template.as_bytes();

        let mut i = 0;

        while i < bytes.len() {
            // `%{name}`
            if bytes[i] == b'%' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                if let Some(end) = find_closing_brace(template, i + 2) {
                    if text_start < i {
                        parts.push(Part::Text(template[text_start..i].to_owned()));
                    }

                    let name = &template[i + 2..end];

                    parts.push(Part::Argument {
                        name: name.to_owned(),
                    });

                    i = end + 1;
                    text_start = i;
                    continue;
                }
            }

            // `{...}`
            if bytes[i] == b'{' {
                if let Some(end) = find_closing_brace(template, i + 1) {
                    let content = &template[i + 1..end];

                    // A plural expression contains `|`.
                    if let Some(pipe) = content.find('|') {
                        let name_part = &content[..pipe];

                        let prepend = name_part.starts_with('+');

                        let name = if prepend { &name_part[1..] } else { name_part };

                        if !name.is_empty() {
                            if text_start < i {
                                parts.push(Part::Text(template[text_start..i].to_owned()));
                            }

                            let rules_text = &content[pipe + 1..];

                            let rules = compile_plural_rules(rules_text);

                            parts.push(Part::Plural {
                                name: name.to_owned(),
                                prepend,
                                rules,
                            });

                            i = end + 1;
                            text_start = i;
                            continue;
                        }
                    }
                }
            }

            i += 1;
        }

        if text_start < template.len() {
            parts.push(Part::Text(template[text_start..].to_owned()));
        }

        Self { parts }
    }

    fn render(&self, args: &[(&str, Arg<'_>)]) -> String {
        let capacity = self.estimated_capacity(args);

        let mut output = String::with_capacity(capacity);

        for part in &self.parts {
            match part {
                Part::Text(text) => {
                    output.push_str(text);
                }

                Part::Argument { name } => {
                    if let Some(arg) = find_arg(args, name) {
                        arg.write_to(&mut output);
                    }
                }

                Part::Plural {
                    name,
                    prepend,
                    rules,
                } => {
                    if let Some(arg) = find_arg(args, name) {
                        render_plural(&mut output, arg, *prepend, rules);
                    }
                }
            }
        }

        output
    }

    fn estimated_capacity(&self, args: &[(&str, Arg<'_>)]) -> usize {
        let text_size = self
            .parts
            .iter()
            .map(|part| match part {
                Part::Text(text) => text.len(),
                _ => 0,
            })
            .sum::<usize>();

        let arg_count = self
            .parts
            .iter()
            .filter(|part| matches!(part, Part::Argument { .. } | Part::Plural { .. }))
            .count();

        // Avoid a zero-capacity String for translations that contain
        // only arguments.
        text_size + arg_count * 8 + args.len() * 4
    }
}

fn find_closing_brace(template: &str, start: usize) -> Option<usize> {
    template[start..].find('}').map(|offset| start + offset)
}

// -----------------------------------------------------------------------------
// Plural compilation
// -----------------------------------------------------------------------------

fn compile_plural_rules(rules: &str) -> Vec<PluralRule> {
    rules
        .split('|')
        .enumerate()
        .map(|(implicit_index, rule)| compile_plural_rule(implicit_index, rule))
        .collect()
}

fn compile_plural_rule(implicit_index: usize, rule: &str) -> PluralRule {
    if let Some(rest) = rule.strip_prefix('+') {
        let replace = rest.starts_with('=');

        let value = rest.strip_prefix('=').unwrap_or(rest);

        return PluralRule {
            threshold: implicit_index,
            thresholded: true,
            replace,
            value: value.to_owned(),
        };
    }

    if let Some(plus) = rule.find('+') {
        if plus > 0 {
            if let Ok(threshold) = rule[..plus].parse::<usize>() {
                let rest = &rule[plus + 1..];

                let replace = rest.starts_with('=');

                let value = rest.strip_prefix('=').unwrap_or(rest);

                return PluralRule {
                    threshold,
                    thresholded: true,
                    replace,
                    value: value.to_owned(),
                };
            }
        }
    }

    if let Some(value) = rule.strip_prefix('=') {
        return PluralRule {
            threshold: implicit_index,
            thresholded: false,
            replace: true,
            value: value.to_owned(),
        };
    }

    PluralRule {
        threshold: implicit_index,
        thresholded: false,
        replace: false,
        value: rule.to_owned(),
    }
}

// -----------------------------------------------------------------------------
// Runtime arguments
// -----------------------------------------------------------------------------

pub enum Arg<'a> {
    Display(&'a dyn fmt::Display),
    Number(Number),
}

pub struct Number {
    value: f64,
    text: NumberText,
}

enum NumberText {
    Integer(String),
    Float(String),
}

impl Number {
    fn write_to(&self, output: &mut String) {
        match &self.text {
            NumberText::Integer(text) | NumberText::Float(text) => {
                output.push_str(text);
            }
        }
    }

    fn value(&self) -> f64 {
        self.value
    }
}

/// Used by the proc macro for ordinary placeholders.
#[doc(hidden)]
pub fn __display<'a, T>(value: &'a T) -> Arg<'a>
where
    T: fmt::Display,
{
    Arg::Display(value)
}

/// Used by the proc macro for `n = ...`.
///
/// Numeric values are captured without converting them through
/// `Display` and then parsing them again.
#[doc(hidden)]
pub fn __number<'a, T>(value: &'a T) -> Arg<'a>
where
    T: NumberArg,
{
    value.to_ezlz_number()
}

/// Numeric values supported by `{n|...}` expressions.
pub trait NumberArg {
    fn to_ezlz_number<'a>(&'a self) -> Arg<'a>;
}

macro_rules! impl_integer_number_arg {
    ($($ty:ty),* $(,)?) => {
        $(
            impl NumberArg for $ty {
                fn to_ezlz_number<'a>(
                    &'a self,
                ) -> Arg<'a> {
                    Arg::Number(Number {
                        value: *self as f64,
                        text: NumberText::Integer(
                            self.to_string(),
                        ),
                    })
                }
            }
        )*
    };
}

impl_integer_number_arg!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize,
);

impl NumberArg for f32 {
    fn to_ezlz_number<'a>(&'a self) -> Arg<'a> {
        Arg::Number(Number {
            value: *self as f64,
            text: NumberText::Float(format_number(*self as f64)),
        })
    }
}

impl NumberArg for f64 {
    fn to_ezlz_number<'a>(&'a self) -> Arg<'a> {
        Arg::Number(Number {
            value: *self,
            text: NumberText::Float(format_number(*self)),
        })
    }
}

// -----------------------------------------------------------------------------
// Runtime lookup
// -----------------------------------------------------------------------------

#[doc(hidden)]
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
        Some(template) => template.render(args),
        None => key.to_owned(),
    }
}

fn find_arg<'a>(args: &'a [(&str, Arg<'a>)], name: &str) -> Option<&'a Arg<'a>> {
    args.iter()
        .find(|(arg_name, _)| *arg_name == name)
        .map(|(_, value)| value)
}

// -----------------------------------------------------------------------------
// Rendering
// -----------------------------------------------------------------------------

impl Arg<'_> {
    fn write_to(&self, output: &mut String) {
        match self {
            Arg::Display(value) => {
                use std::fmt::Write;

                let _ = write!(output, "{value}");
            }

            Arg::Number(value) => {
                value.write_to(output);
            }
        }
    }

    fn number(&self) -> Option<f64> {
        match self {
            Arg::Number(number) => Some(number.value()),

            // Kept as a fallback for manually constructed Args.
            //
            // The proc macro always uses Arg::Number for `n`, so
            // normal plural calls do not pay this cost.
            Arg::Display(value) => value.to_string().parse::<f64>().ok(),
        }
    }
}

fn render_plural(output: &mut String, arg: &Arg<'_>, prepend: bool, rules: &[PluralRule]) {
    let Some(number) = arg.number() else {
        arg.write_to(output);
        return;
    };

    let rendered_number = match arg {
        Arg::Number(number) => {
            let mut s = String::new();

            number.write_to(&mut s);

            s
        }

        Arg::Display(value) => value.to_string(),
    };

    let Some(rule) = select_plural_rule(number, rules) else {
        output.push_str(&rendered_number);
        return;
    };

    if rule.replace {
        if prepend {
            output.push_str(&rule.value);
            output.push_str(&rendered_number);
        } else {
            output.push_str(&rule.value);
        }

        return;
    }

    if prepend {
        output.push_str(&rule.value);
        output.push_str(&rendered_number);
    } else {
        output.push_str(&rendered_number);
        output.push_str(&rule.value);
    }
}

fn select_plural_rule<'a>(number: f64, rules: &'a [PluralRule]) -> Option<&'a PluralRule> {
    if number.fract() != 0.0 || number < 0.0 {
        return None;
    }

    let index = number as usize;

    // Exact positional rule wins.
    if let Some(rule) = rules.get(index) {
        if !rule.thresholded {
            return Some(rule);
        }
    }

    // Otherwise select the highest matching threshold.
    rules
        .iter()
        .filter(|rule| rule.thresholded && rule.threshold <= index)
        .max_by_key(|rule| rule.threshold)
}

// -----------------------------------------------------------------------------
// Formatting
// -----------------------------------------------------------------------------

fn format_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}
