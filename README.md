# ezlz
A compact internationalization crate optimized for hot loops.
##


## Overview
The goal is to support all natural languages while keeping the crate
simple and fast enough for use in web templates and UIs that render
in hot loops where the full [CLDR](https://cldr.unicode.org)
functionality is not required.

### Features
 - **Fast**: Translations are compiled at runtime and can be rendered over 10 million times per second for simple templates.
 - **Simple**: The basic API is just a single function and a macro.
 - **Pluralization**: Plural rules are compiled from placeholder syntax.
##


## Quick start

### Installation
Add with Cargo:
```bash
cargo add ezlz
```
Or manually to `Cargo.toml`:
```toml
[dependencies]
ezlz = "1"
```

### Setup
Create a directory for your locales and YAML files containing the translations.
For example:
```text
locales/
├── en.yml
├── fr.yml
└── ru.yml
```
```yaml
# locales/en.yml
messages:
  hello: "Hello, {name}!"
  items: "You have {n|=1: item| items}."

# locales/fr.yml
messages:
  hello: "Bonjour, {name}!"
  items: "Vous avez {n|~0-1: object| objects}."

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

### Translations
Each `.yml` or `.yaml` file in the locales directory contains
translations for one language.
The filename without the extension is the locale name.
You must initialize ezlz with a fallback locale:
```rust ignore
ezlz::init("en", "locales").unwrap();
```


YAML key mappings become dotted translation keys. For example:
```yaml
foo:
  bar:
    baz: "corge"
    qux: "grault"
```
can be referenced as:
```rust ignore
t!(lang, foo.bar.baz);
t!(lang, foo.bar.qux);
```


Once `init` is called, each translation is parsed for 
placeholders and stored in memory as a compiled `Template` object.


If a requested locale does not contain a translation, 
ezlz tries to find it in the fallback locale and **panics
if the translation is not found there either**.
```rust ignore
// Falls back to 'en'
t!("cn", foo.bar);
// Panic: Translation 'nonexistent.key' not found for locale 'en' and fallback locale 'en'.
t!("en", nonexistent.key);
```
##


## The `t!` macro
Usage:
```text
t!(<locale>, <key>[, arguments...])
```

### Locale
The `locale` can be any Rust expression whose value can be converted to `Box<str>`.
```rust ignore
t!("en", foo.bar);
t!(current_locale(), foo.bar.baz);
```

### Translation key
A translation `key` is basically the YAML path separated by dots:
```rust ignore
t!("en", menu.login);
t!("en", store.cart.total);
```

### Arguments
All arguments are **named**, but you can pass a bare variable 
whose name matches the placeholder name:

```rust ignore
let name = "Anna";
let items = 5_u32;

t!("en", examples.stats, name, items);
```

For an explicit placeholder name or expression:
```rust ignore
t!("en", foo, name = user_name);
t!("en", foo, count = some_expression());
```


Expressions must be explicitly named. You can pass multiple different arguments.
If a template has multiple placeholders with the same name, they will all take 
the supplied value, so you don't need to repeat the argument for each placeholder.
##


## Placeholders
Regular placeholder:
```yaml
hello: "Hello, {name}!"
```
Plural placeholder:
```yaml
items: "You have {items|=1: item| items}."
```
Escaping:
```yaml
escape: 'this is \{not a placeholder}'
double: 'but this is a \\{placeholder} with a backslash before it'
```
You can have multiple placeholders in a template. 
Placeholder names can repeat.

### Supported types
You can pass the following types into the placeholders:
- Unsigned: `u8`, `u16`, `u32`, `u64`, `usize`
- Signed: `i8`, `i16`, `i32`, `i64`, `isize`
- Float: `f32`, `f64`
- Text: `String`, `&str`

Or any custom type that implements `ezlz::ToArg` trait.
Check out [docs.rs](https://docs.rs/ezlz) or source code for reference.

### Pluralization
A plural placeholder has an identifier and a set of rules that are
compiled during initialization. If rule text starts with `=`, the rendered number
is replaced instead of prepended.

The rule syntax is designed to be capable of implementing
any pluralization rule for any natural language.
Matching rules are defined explicitly by the locale author.
```text
{id|selector:text|selector:text|...|text}
```


**Numeric values are matched using their absolute integer value with the fractional part truncated.**
This does not affect the rendered number. The `.` selector is an exception:
it matches the original argument type and therefore distinguishes floating-point inputs.


Rules are evaluated from left to right. The first matching rule is selected.
**Float inputs will not match any selectors except `.`, `~`, and fallback.**

| Selector | Description                           | Syntax              |
| -------- | ------------------------------------- | ------------------- |
| `~`      | Numeric value/range, including floats | `~1` `~0-1` `~2+`   |
| `.`      | Float input type                      | `.`                 |
| `#`      | Modulo 100 value/range                | `#0` `#11-19` `#3+` |
| `%`      | Modulo 10 value/range                 | `%1` `%2-4` `%5+`   |
| `=`      | Absolute integer value/range          | `=0` `=0-1` `=9+`   |
| *(none)* | Unconditional fallback                | `text`              |


Rules can use `+` for an open-ended range.
Here, `N` is an absolute truncated integer value, as mentioned above.
```text
%1+   modulo 10 of N is 1 or greater
#11+  modulo 100 of N is 11 or greater
=1+   N is 1 or greater
~1+   N is 1 or greater, including float inputs
```


The `.` selector matches arguments that were originally passed 
as floating-point types, e.g. `1.0_f64` **does** match it even 
though its numerical value is an integer.

#### Plural placeholder examples
```yaml
en: "{i|=1: fox| foxes}"
fr: "{i|~0-1: article| articles}"
ro: "{i|=0: vulpi|=1: vulpe|#1-19: vulpi| de vulpi}"
ru: "{i|.: стола|#11-14: столов|%1: стол|%2-4: стола| столов}"
ar: "{i|.: other|#11-99: many|=0: zero|%1: one|%2: two|#3-10: few|#0: other}"
```
##


## How it works

### Initialization
1. Search the provided directory for `.yml` and `.yaml` files.
   The filename without its extension is used as the locale name.
2. Parse each YAML file and recursively flatten its mappings into
   dotted translation keys. String values are compiled into `Template`
   objects containing `Part`s for text, regular placeholders, and plural
   placeholders. Plural placeholders are compiled into `Ruleset`s.
3. Store the compiled templates in an `AHashMap` for each locale and
   store all locales together with the fallback locale in `Translations`.
4. Store `Translations` in a global `OnceLock`. `init` can only be called
   successfully once, and will return an error if called again or if the configured
   fallback locale does not exist.

### Runtime lookup
1. `t!` expands to a call that passes the locale, hardcoded translation key,
   and converted arguments to the runtime lookup function.
2. Look up the `Template` by key in the requested locale. If it is not found,
   try the same key in the fallback locale. Panic if it is not found there either.
3. Render the template by walking through its precompiled `Part`s. Text is appended
   directly to the output buffer, while regular and plural placeholders look up their 
   arguments and render them into the same buffer.
4. Return the resulting `String`.

### Summary
This keeps YAML parsing, template compilation, and plural rule compilation out of 
the rendering path. Combined with [`ahash`](https://crates.io/crates/ahash) for
translation mapping, [`itoa`](https://crates.io/crates/itoa) and 
[`zmij`](https://crates.io/crates/zmij) for number-to-string conversion, this makes 
translation lookup and placeholder rendering fast enough for use in
hot loops. See the [Benchmarks](#benchmarks) section for detailed statistics.
##


## Benchmarks

Benchmarks were run with Criterion harness using:

* Environment: [Termux](https://github.com/termux/termux-app) 0.118.3
* OS: Android 14
* CPU: [MediaTek Dimensity 8050](https://www.mediatek.com/products/smartphones/mediatek-dimensity-8050)


Results:

| Benchmark          | Description             |   Average |
| ------------------ | ----------------------- | --------: |
| `text`             | No placeholders         |     80 ns |
| `simple`           | Single integer          |     91 ns |
| `simple<-string`   | Single string           |     87 ns |
| `simple<-float`    | Single float            |    113 ns |
| `simple (x10)`     | 10 `simple` in one      |    162 ns |
| `plural_en`        | English integer plural  |    104 ns |
| `plural_en<-float` | English float plural    |    126 ns |
| `plural_en (x10)`  | 10 `plural_en` in one   |    139 ns |
| `plural_fr<-float` | French float plural     |    126 ns |
| `plural_ru`        | Russian integer plural  |    108 ns |
| `plural_ru<-float` | Russian float plural    |    125 ns |

### Running the benchmarks

Clone the repository and run:

```bash
cargo bench
```
For development, the benchmark source is in `benches/benchmarks.rs`.
##


## License

MIT