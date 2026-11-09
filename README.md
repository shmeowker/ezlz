[![crates.io version](https://shields.io/crates/v/ezlz)](https://crates.io/crates/ezlz)
[![docs.rs](https://img.shields.io/docsrs/ezlz)](https://docs.rs/ezlz)
[![CI](https://github.com/shmeowker/ezlz/actions/workflows/ci.yml/badge.svg)](https://github.com/shmeowker/ezlz/actions/workflows/ci.yml)

# ezlz

A compact internationalization crate optimized for hot loops.

## Overview

The goal is to support all natural languages while keeping the crate
simple and fast enough for use in web templates and UIs that render
in hot loops, where the full [CLDR](https://cldr.unicode.org)
functionality is not required.

### Features

- **Fast**: Translations are compiled into RAM ahead of time. A single-placeholder
  render takes roughly 50-90 ns on a mid-range phone CPU
  (see [Benchmarks](#benchmarks)).
- **Simple**: The basic API is one function (`init`) and one macro (`t!`).
- **Pluralization**: Plural rules are compiled from placeholder syntax.
- **Hot reloading**: Translations can be swapped at runtime with the
  `reinit` function.

## Quick start

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

```rust no_run
use ezlz::t;

fn main() {
    ezlz::init("en", "locales").unwrap();

    let name = "Anna";
    let n = 4;

    assert_eq!(t!("fr", messages.hello, name), "Bonjour, Anna!");
    assert_eq!(t!("en", messages.items, n), "You have 4 items.");

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

```rust no_run
ezlz::init("en", "locales").unwrap();
```

Only the top level of the directory is scanned (subdirectories are ignored),
and a locale must be defined in exactly one file: having both `en.yml` and
`en.yaml` is an error (`DuplicateLocale`).

Everything else in the directory is **silently skipped**: files without a
`.yml`/`.yaml` extension, subdirectories, and files whose name is not valid
UTF-8 (a locale name must be a valid string). A file skipped this way does not
produce an error, so a locale file with an unusual name simply won't be found.

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

Every leaf value must be a **string**. Numbers, booleans, `null` (including an
empty file) and sequences are rejected by `init` with an error, so quote
values like `"404"` or `"true"`.

The flattened keys must be unique: defining `a.b` both directly and as
`a: { b: ... }` is an error (`DuplicateKey`), as is repeating a key in one mapping.

Once `init` is called, each translation is parsed for
placeholders and stored in memory as a compiled `Template` object.

If a requested locale does not contain a translation,
ezlz tries to find it in the fallback locale and **panics
if the translation is not found there either**.

```rust ignore
// Falls back to 'en'
t!("jp", foo.bar);
// Panic: translation 'nonexistent.key' not found in locale 'en' and fallback locale
t!("en", nonexistent.key);
```

Panic can be changed to returning the translation key instead
by enabling the `stringify-missing-keys` feature.

```shell
cargo add ezlz -F stringify-missing-keys
```

Locale names are matched exactly: `en_GB` does **not** fall back to `en`,
only to the configured fallback locale.

### Errors

`init` and `reinit` return an `Error` instead of panicking:

| Variant                  | Cause                                                            |
| ------------------------ | ---------------------------------------------------------------- |
| `Io`                     | The directory or a file could not be read.                       |
| `ParseYaml`              | A file is not valid YAML.                                        |
| `InvalidYaml`            | A key is not a string, or a value is not a string (see above).   |
| `InvalidTemplate`        | A translation has an unclosed or invalid placeholder.            |
| `AlreadyInitialized`     | `init` was called after a successful initialization.             |
| `FallbackLocaleNotFound` | No file for the fallback locale was found in the directory.      |
| `DuplicateKey`           | The same flattened translation key is defined twice in one file. |
| `DuplicateLocale`        | Two files (`en.yml` and `en.yaml`) define the same locale.       |

Calling `t!` before a successful `init` panics.

## The `t!` macro

Usage:

```text
t!(<locale>, <key>[, arguments...])
```

### Locale

The `locale` can be any Rust expression that can be borrowed as a `&str`,
such as `&str`, `String`, `Box<str>` or `Cow<str>`.

```rust ignore
t!("en", foo.bar);
t!(current_locale(), foo.bar.baz);
```

### Translation key

A translation `key` is the YAML path separated by dots:

```rust ignore
t!("en", menu.login);
t!("en", store.cart.total);
```

Each segment is parsed as a Rust identifier, so keywords like `type` or `match`
work, but YAML keys that start with a digit or contain characters such as `-`
can be defined in YAML and cannot be referenced with `t!`. Keys are not checked
at compile time: a typo is found at runtime (see the fallback behavior above).

### Arguments

All arguments are **named**, but you can pass a bare variable
whose name matches the placeholder name:

```rust ignore
let name = "Anna";
let items = 5;

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
A trailing comma after the last argument is allowed.

Arguments are matched to placeholders by name at runtime and **not validated**:
a placeholder without a matching argument renders as an empty string,
and arguments that match no placeholder are ignored. If the same name is
passed twice, the first one is used.

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
escape: '\{not a placeholder}'
double: 'but \\{this} is still a placeholder'
```

Placeholder names must not be empty and may contain only ASCII letters, digits and `_`.
You can have multiple placeholders in a template, and names can repeat.

### Escaping

A backslash escapes `{` only when it is directly in front of it:

- `\{` renders as a literal `{` (the backslash is removed) and does not
  start a placeholder.
- Backslashes anywhere else are plain text and are kept as they are.
- If `{` is preceded by an **even** number of backslashes, it starts a
  placeholder and **all** of those backslashes are kept in the output,
  so `\\{name}` renders as `\\Anna`. There is currently no way to render a
  single backslash directly in front of a placeholder.

In YAML, `\{` is an invalid escape sequence in double-quoted strings.
Use single quotes (`'\{x}'`), or double the backslash (`"\\{x}"`).

Placeholders cannot be nested, and the first `}` always closes a placeholder,
so plural rule text cannot contain `}`. A stray `}` outside a placeholder is
plain text.

### Supported types

Placeholders accept the following types:

- Unsigned: `u8`, `u16`, `u32`, `u64`, `usize`
- Signed: `i8`, `i16`, `i32`, `i64`, `isize`
- Float: `f32`, `f64`
- Text: `String`, `&str`, `Cow<str>`, `Box<str>`

`u128`, `i128`, `char`, `bool` and other types are not supported out of the box;
You can pass any custom type whose reference implements the
[`ToArg`](https://docs.rs/ezlz/latest/ezlz/trait.ToArg.html)
trait. Check out the source code for reference.

Arguments are passed by reference internally, so a value of type `&String` or `&i32`
(for example `name = &user.name`, or a bare `name` that is itself a reference to a
number or `String`) does not compile. Pass the value itself, or for text
`name = user.name.as_str()`. A `&str` works as it is.

Floats are rendered with their shortest round-trip representation, and `f32`
values keep their own precision: `1.1_f32` renders as `1.1`.

### Pluralization

A plural placeholder has an identifier and a set of rules that are
compiled during initialization. The rule syntax is designed to be capable
of implementing any pluralization rule for any natural language.
Matching rules are defined explicitly by the locale author.

```text
{id|selector:text|selector:text|...|text}
```

Rules are evaluated from left to right. The first matching rule is selected.
If no rule matches, only the number is rendered.
**Float inputs will not match any selectors except `.`, `~`, and fallback.**

**Numeric values are matched using their absolute integer value
with the fractional part truncated.** This does not affect the
rendered number. The `.` selector matches the original argument
type and therefore distinguishes floating-point inputs.

The rule text is appended to the rendered number.
If rule text starts with `=`, the rendered number is replaced
instead of prepended. If a plural placeholder receives a non-numeric
value it renders only the value itself and returns without rule matching.

Range bounds are written as unsigned decimal integers up to `255` (`255` is
reserved internally and acts as "no upper bound", which is irrelevant for
real plural rules); values like `=1000` are rejected at initialization.
Because the bounds are small, use `#` or `%` to match numbers by their last digits.

Each rule is split only once, at its first `:`, so rule text may itself contain
colons as long as the rule has a selector. For a fallback rule whose text contains
a colon, start it with an empty selector:

```yaml
items: "{n|=1: item|: items: all}"
```

Rule text cannot contain `|` or `}`.

#### Selectors

| Selector | Description                           | Syntax              |
| -------- | ------------------------------------- | ------------------- |
| `~`      | Numeric value/range, including floats | `~1` `~0-1` `~2+`   |
| `.`      | Float input type                      | `.`                 |
| `#`      | Modulo 100 value/range                | `#0` `#11-19` `#3+` |
| `%`      | Modulo 10 value/range                 | `%1` `%2-4` `%5+`   |
| `=`      | Absolute integer value/range          | `=0` `=0-1` `=9+`   |
| _(none)_ | Unconditional fallback                | `text`              |

Selectors can use `+` for an open-ended range.
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

Special float values are handled by the same rules: `NaN` truncates to `0`
and infinities saturate, so they are matched as `0` and as a very large number.

#### Plural placeholder examples

```yaml
en: "{i|=1: fox| foxes}"
fr: "{i|~0-1: article| articles}"
ro: "{i|=0: vulpi|=1: vulpe|#1-19: vulpi| de vulpi}"
ru: "{i|.: стола|#11-14: столов|%1: стол|%2-4: стола| столов}"
ar: "{i|.: other|#11-99: many|=0: zero|%1: one|%2: two|#3-10: few|#0: other}"
```

A rule that has a leading `=` in the text will replace the number:
```yaml
count: "{n|=0:=no items|=1:=one item| items}"
```

## Hot reloading

`reinit` loads a new set of translations and atomically swaps it in:

```rust no_run
// SAFETY: translation lookups are short-lived, see below.
unsafe { ezlz::reinit("en", "locales").unwrap(); }
```

The previous translations are not freed immediately. A background thread
drops them after the grace period. Using this is mostly safe because in normal conditions
the readers only access the storage for no longer than a millisecond, however
during a severe lag spike a thread can theoretically be frozen for long enough
to outlive the drop timer, and in that case the behavior is undefined.
You should either avoid using this completely or set the grace period to a 
very long duration if memory safety is crucial in your codebase.
If the new translations fail to load, the old ones stay in place and an `Error` is returned.
`reinit` also works as an initial `init`. The grace period is 60 seconds by default, configurable with
`set_grace_period`.

```rust no_run
use std::num::NonZero;

// 20 minutes
let delay = NonZero::new(1200).unwrap();
ezlz::set_grace_period(delay);
```

The delay must be non-zero because dropping the old translations immediately
will almost certainly cause severe undefined behavior.

## How it works

### Initialization

1. Search the provided directory for `.yml` and `.yaml` files.
   The filename without its extension is used as the locale name.
2. Parse each YAML file and recursively flatten its mappings into
   dotted translation keys. String values are compiled into `Template`
   objects containing `Part`s for text, regular placeholders, and plural
   placeholders. Plural placeholders are compiled into `Ruleset`s.
3. Store the compiled templates in a `HashMap` for each locale and
   store all locales together with the fallback locale in `Translations`.
4. Publish `Translations` through a global `AtomicPtr`. `init` can only be
   called successfully once, and will return an error if called again or if the
   configured fallback locale does not exist. `reinit` replaces the pointer
   (see [Hot reloading](#hot-reloading)).

### Runtime lookup

1. `t!` expands to a call that passes the locale, hardcoded translation key,
   and converted arguments to the runtime lookup function.
2. Look up the `Template` by key in the requested locale. If it is not found,
   try the same key in the fallback locale. Panic if it is not found there either.
3. Render the template by walking through its precompiled `Part`s. Text is appended
   directly to the output buffer, while regular and plural placeholders look up their
   arguments (by a linear search over the few passed arguments) and render them into
   the same buffer.
4. Return the resulting `String`.

### Summary

This keeps YAML parsing, template compilation, and plural rule compilation out of
the rendering path. Combined with [`ahash`](https://crates.io/crates/ahash) for
translation mapping, [`itoap`](https://crates.io/crates/itoap) and
[`zmij`](https://crates.io/crates/zmij) for number-to-string conversion, this makes
translation lookup and placeholder rendering fast enough for use in
hot loops. See the [Benchmarks](#benchmarks) section for detailed statistics.

## Benchmarks

Benchmarks were run in the following conditions:

- Harness: [Criterion](https://crates.io/crates/criterion)
- Allocator: [mimalloc](https://github.com/microsoft/mimalloc)
- Environment: [Termux](https://github.com/termux/termux-app)
- OS: Android 14
- CPU: [MediaTek Dimensity 8050](https://www.mediatek.com/products/smartphones/mediatek-dimensity-8050)

Results:

| Benchmark          | Description            | Average |
| ------------------ | ---------------------- | ------: |
| `text`             | No placeholders        |   47 ns |
| `simple`           | Single integer         |   50 ns |
| `simple<-string`   | Single string          |   50 ns |
| `simple<-float`    | Single float           |   67 ns |
| `simple (x10)`     | 10 integers            |  109 ns |
| `plural_en`        | English integer plural |   60 ns |
| `plural_en<-float` | English float plural   |   79 ns |
| `plural_en (x10)`  | 10 plural integers     |   99 ns |
| `plural_fr<-float` | French float plural    |   79 ns |
| `plural_ru`        | Russian integer plural |   64 ns |
| `plural_ru<-float` | Russian float plural   |   78 ns |

Every call allocates and returns one `String`, so the allocator matters.

### Running the benchmarks

Clone the repository and run:

```bash
cargo bench
```

For development, the benchmark source is in `benches/benchmarks.rs`.

## Minimum supported Rust version

Rust 1.85 (the first release with edition 2024).

## Development

```bash
cargo test --workspace                              # default features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features
```

Because the translation store is global and `init` succeeds once per process,
the integration tests are split into separate files (each is its own process):
`errors.rs` (failing initializations), `init.rs` (racing `init` calls),
`reinit.rs` (hot reloading), `rendering.rs` (rendering, arguments, fallback),
`pluralization.rs`, `escaping.rs`, `uninit.rs` and `features.rs`.
Internals are covered by unit tests in `src/lib.rs` and `src/plural.rs`.

## Cargo features

| Feature                  | Description                                                                |
| ------------------------ | -------------------------------------------------------------------------- |
| `stringify-missing-keys` | `t!` returns the translation key instead of panicking when it isn't found. |
| `fast-floats`            | Renders floats without checking for NaN and infinity (see below).          |

### `fast-floats`

By default, floats are formatted with `zmij::Buffer::format`, which prints `NaN`,
`inf` and `-inf` for non-finite values. With `fast-floats`, `Buffer::format_finite`
is used instead, which skips that check and is slightly faster. The catch is that
for NaN and infinity it prints an **unspecified number** (for example NaN renders as
`2.696539702293474e+308`) instead of failing, so enable it only if your floats are
always finite. Cargo features are additive: if any crate in a dependency tree enables
it, it is enabled for all of them.

## License

MIT
