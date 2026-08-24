# ezlz
A compact and fast localization engine for Rust with language-independent procedural pluralization.

## Overview
The idea is to support all natural languages while keeping
the crate small and fast so it can be integrated into web-templates 
and UIs where full CLDR functionality is not required. If you
really need extensive and complex formatting, consider
some other crates like [icu](https://crates.io/crates/icu).

### Features
 - **Fast**: Translations are compiled at runtime and can be rendered under 100 ns.
 - **Simple**: The basic API is just a single function and a macro.
 - **No CLDR pluralization**: Plural rules are compiled from placeholder syntax.


## Quick start
Create a directory for your locales and YAML files for the translations.
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

  # Text with placeholder
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


YAML key mappings become dotted translation keys, for example:
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


## How it works

### Initialization
1. Ezlz looks up the provided directory for YAML files
   and stores each name as a key into the `Translations` hashmap.
   The file contents are parsed and each YAML key/value pair is processed:
    - The keys become hashmap keys for the corresponding template.
    - The string values are parsed and each is compiled into a `Template`, that can contain multiple
       strings, regular and plural placeholders (which are also parsed and compiled into rulesets).
2. Sets the provided fallback locale for `Translations`.
3. Assigns the `Translations` to a static `OnceLock` for future access.
4. Checks if the fallback locale exists in the hashmap and panics if it doesn't.

### Runtime lookup
1. Get the template hashmap by the provided locale name.
2. Try getting a `Template` by the hardcoded key from that hashmap, or fallback locale
   hashmap if not found, and panic if that doesn't have it.
3. Create a new string buffer.
4. Walk through parts of the `Template`, render each one, writing directly to the buffer.
5. Return the buffer.

### Summary
This approach combined with `itoa` and `zmij` for number-to-string conversion
makes translation key lookups and placeholder rendering very fast. 
See the [Benchmarks](#benchmarks) section for detailed statistics.


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
t!("cn", foo.bar);
// Panic: Translation 'nonexistant.key' not found for locale 'en' and fallback locale 'en'.
t!("en", nonexistant.key);
```

## The `t!` macro
Usage:
```text
t!(<locale>, <key>[, arguments...])
```

### Locale
The `locale` is any Rust expression which value can be converted to `Box<str>`.
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
which name matches the placeholder name:

```rust ignore
let name = "Anna";
let count = 5u32;

t!("en", examples.stats, name, count);
```

For an explicit placeholder name or expression:
```rust ignore
t!("en", foo, name = user_name);
t!("en", foo, count = some_expression());
```


Expressions must be explicitly named. You can pass multiple different arguments.
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

Or any custom type that implements `ezlz::ToArg` trait.
Check out docs.rs or source code for reference.

### Pluralization
Plural placeholders have an identifier and set of rules that is 
compiled at run time. If rule text starts with `=`, the rendered number
is replaced instead of prepended.

The rule syntax is designed to be capable of implementing
any pluralization rule for any nutural language.
The author of a locale defines the matching rules explicitly.
```text
{id|selector:text|selector:text|...|text}
```


**Numeric values are matched using their absolute value with the fractional part truncated.**
This does not affect the rendered number. The `.` selector is the exception:
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
N is input.abs().trunc() as u64, as mentioned above.
```text
%1+   modulo 10 of N is 1 or greater
#11+  modulo 100 of N is 11 or greater
=1+   N is 1 or greater
~1+   N is 1 or greater, including float inputs
```


The `.` selector matches arguments that were originally passed 
as floating-point types, e.g. `1.0_f64` **does** match it even 
though its numerical value is integer.


You can check out some plural placeholder examples
for popular languages in the [Quick Start ⟩ Templates](#templates) section.

## Benchmarks

Benchmarks were run with Criterion harness using:

* Environment: Termux 0.118.3
* OS: Android 14
* CPU: MediaTek Dimensity 8050


Results:

| Benchmark          | Description             |   Average |
| ------------------ | ----------------------- | --------: |
| `text`             | No placeholders         |     87 ns |
| `simple`           | Single integer          |     93 ns |
| `simple<-string`   | Single string           |     87 ns |
| `simple<-float`    | Single float            |    112 ns |
| `simple (x10)`     | 10 `simple` in one      |    259 ns |
| `plural_en`        | English integer plural  |    128 ns |
| `plural_en<-float` | English float plural    |    148 ns |
| `plural_en (x10)`  | 10 `plural_en` in one   |    163 ns |
| `plural_fr<-float` | French float plural     |    148 ns |
| `plural_ru`        | Russian integer plural  |    133 ns |
| `plural_ru<-float` | Russian float plural    |    146 ns |

100 ns is 10M iterations per second.

> The noise threshold is roughly 5%
> because the benchmarks were run on an actively used phone without
> dedicated cooling or background-process isolation. If you have better
> benchmark results, please open an issue and paste the Criterion output
> and your system specs so I can update this table for more realistic
> numbers on desktop systems.

### Running the benchmarks

Clone the repository and run:

```bash
cargo bench
```
For development, the benchmark source is in `benches/benchmarks.rs`.

## License

MIT
