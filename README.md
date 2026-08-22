# ezlz

A compact and fast localization engine for Rust with language-agnostic procedural pluralization.

## Quick start

Create a directory for your locales and YAML files for the translations:

```text
locales/
├── en.yml
├── fr.yml
└── ru.yml
```

For example:

```yaml
# locales/en.yml
messages:
  hello: "Hello, {name}!"
  items: "You have {n|=1: item| items}."

# locales/fr.yml
messages:
  hello: "Bonjour, {name}!"
  items: "Vous avez {n|~0: object|=1: object| objects}."

# locales/ru.yml
messages:
  hello: "Привет, {name}!"
  items: "У тебя {n|.: предмета|#11-14: предметов|%1: предмет|%2-4: предмета| предметов}."
```


```rust ignore
use ezlz::t;

fn main() {
    ezlz::init("en", "locales").unwrap();
    
    let name = "Anna";
    let n = 4u32;
    
    assert_eq!(
        t!("en", messages.hello, name),
        "Hello, Anna!"
    );
    
    assert_eq!(
        t!("en", messages.items, n),
        "You have 4 items."
    );
    let text = t!(current_locale(), messages.items, n);
    assert_eq!(text, "У тебя 4 предмета.");
}

fn current_locale() -> String {
    "ru".to_owned()
}
```

### Templates

```yaml
examples:
  # Plain text
  hello: "Hello!"

  # Named interpolation
  greeting: "Hello, {name}!"

  # Multiple placeholders
  stats: "{name} has {items} items."

  # Pluralization examples
  en: "{i|=1: fox| foxes}"
  fr: "{i|~0-1: article| articles}"
  ro: "{i|=0: vulpi|=1: vulpe|#1-19: vulpi| de vulpi}"
  ru: "{i|.: стола|#11-14: столов|%1: стол|%2-4: стола| столов}"
  ar: "{i|.: other|#11-99: many|=0: zero|%1: one|%2: two|#3-10: few|#0: other}"
```

YAML mappings become dotted translation keys:

```yaml
ui:
  auth:
    login: "Log in"
    register: "Register"
```

which can be referenced as:

```rust ignore
t!("en", ui.auth.login);
```

## Installation

Add with Cargo:

```bash
cargo add ezlz
```

Or manually to `Cargo.toml`:

```toml
[dependencies]
ezlz = "1"
```

## Translation files

Each `.yml` or `.yaml` file in the localization directory represents one locale.
The filename without the extension is the locale name.
Initialize ezlz with the fallback locale:

```rust ignore
ezlz::init("en", "locales").unwrap();
```

If a requested locale does not contain the translation, 
ezlz tries to find it in the fallback locale, and **panics
if the fallback locale doesn't have it**.

```rust ignore
// Falls back to 'en'
t!("cn", messages.hello);
// Panic: Translation 'nonexistant.key' not found for locale 'en' or fallback locale 'en'.
t!("en", nonexistant.key);
```

## The `t!` macro
Usage:
```text
t!(<locale: Into<Box<str>>>, <mapping[.key]...>[, ident: ezlz::ToArg | ident = expr: ezlz::IntoArg]...)
```

### Locale
The first argument is any Rust expression which value can be converted to `Box<str>`.

```rust ignore
t!("en", messages.hello);
t!(current_locale(), messages.hello);
```

### Translation key
A translation key is basically the YAML path separated by dots:

```rust ignore
t!("en", menu.login);
t!("en", store.cart.total);
```

### Arguments
All arguments are **named**, but you can pass a bare variable 
which name matches the placeholder name:

```rust ignore
let name = "Anna";
let count = 5u32;

t!("en", message, name, count);
```

For an explicit placeholder name or expression:

```rust ignore
t!("en", message, name = user_name);
t!("en", message, count = some_expression());
```

Expressions must be named. You can pass multiple different arguments.
If a template has multiple placeholders with same names,
they will all take the supplied value, 
so you shall not repeat the argument per each placeholder.

## Placeholders
Regular placeholder:
```yaml
hello: "Hello, {name}!"
```
Plural placeholder:
```yaml
items: "You have {items|=1: item| items}."
```
You can have multiple placeholders in a template. 
Placeholder names can repeat.

### Supported types
You can pass the following types into the placeholders:
- Unsigned: `u8`, `u16`, `u32`, `u64`, `usize`
- Signed: `i8`, `i16`, `i32`, `i64`, `isize`
- Float: `f32`, `f64`
- Text: `String`, `&str`

### Pluralization
Plural placeholders have an identifier and set of rules that is 
compiled at run time. If rule text starts with `=`, the rendered number
is replaced instead of prepended.
\
The syntax is deliberately language-agnostic: ezlz does not infer plural categories
or implement CLDR rules. The author of a locale defines the matching rules explicitly.
```text
{id|selector:text|selector:text|...|text}
```

Rules are evaluated from left to right. The first matching rule is selected.
**Float inputs will not match any selectors except `.` and `~`!**

| Selector | Description                           | Syntax              |
| -------- | ------------------------------------- | ------------------- |
| `~`      | Numeric value/range, including floats | `~1` `~0-1` `~2+`   |
| `.`      | Float                                 | `.`                 |
| `#`      | Modulo 100 value/range                | `#0` `#11-19` `#3+` |
| `%`      | Modulo 10 value/range                 | `%1` `%2-4` `%5+`   |
| `=`      | Absolute integer value/range          | `=0` `=0-1` `=9+`   |
| *(none)* | Unconditional fallback                | `text`              |

For `%` and `#`, the rule is applied to the absolute truncated integer value:

```text
21  matches %1
22  matches %2
125 matches #25
```

Rules can use `+` for an open-ended range:

```text
%5+   modulo 10 is 5 or greater
#11+  modulo 100 is 11 or greater
=9+   absolute integer value is 9 or greater
~1+   truncated absolute integer value is 1 or greater, including float inputs
```


The `.` selector matches arguments that were 
originally passed as floating-point types,
e.g. `1.0_f64` **does** match it even though it is integer.


You can check out some plural placeholder examples
for popular languages in the `Quick Strart Templates` section.

## Benchmarks

Benchmarks were run with Criterion harness using:

* Environment: **Termux 0.118.3**
* OS: **Android 14**
* CPU: **MediaTek Dimensity 8050**

> Differences of roughly 5–10 ns should not be considered significant
> because the benchmarks were run on an actively used phone without
> dedicated cooling or background-process isolation.

Results:

| Benchmark          |   Average |
| ------------------ | --------: |
| `text`             |     84 ns |
| `simple`           |     93 ns |
| `simple<-string`   |     87 ns |
| `simple<-float`    |    112 ns |
| `simple (x10)`     |    259 ns |
| `plural_en`        |    128 ns |
| `plural_en<-float` |    148 ns |
| `plural_en (x10)`  |    175 ns |
| `plural_fr`        |    150 ns |
| `plural_ru`        |    140 ns |
| `plural_ru<-float` |    146 ns |

### Running benchmarks

Clone the repository and run:

```bash
cargo bench
```
For development, the benchmark source is in `benches/benchmarks.rs`.

## License

MIT
