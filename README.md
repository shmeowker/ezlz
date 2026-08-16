<!-- i vibecoded this in 30 minutes -->

# ezlz

Easy Localization — a tiny, runtime YAML-based localization crate for Rust.

`ezlz` loads localization files from a directory at startup and provides a small `t!` macro for convenient translations with locale fallback and named interpolation.

## Features

* Simple YAML localization files
* Nested translation keys
* Locale fallback
* Runtime locale selection
* Named interpolation
* Bare identifier interpolation shorthand
* No build script or generated files
* Minimal API

## Installation

Add `ezlz` to your `Cargo.toml`:

```toml
[dependencies]
ezlz = "0.1"
```

## Quick start

Create a `locales` directory:

```text
locales/
├── en.yml
├── en_GB.yml
└── de.yml
```

### `locales/en.yml`

```yaml
messages:
  hello: "Hello, world!"
  greet: "Hello, %{name}!"
  count: "%{name} has %{count} messages."

errors:
  not_found: "Not found"
```

### `locales/de.yml`

```yaml
messages:
  hello: "Hallo Welt!"
  greet: "Hallo, %{name}!"
```

Initialize `ezlz` once:

```rust
fn main() {
    ezlz::init("en", "locales").unwrap();

    let lang = "de";

    println!("{}", ezlz::t!(lang, messages.hello));
}
```

Output:

```text
Hallo Welt!
```

## Translation keys

Nested YAML mappings are flattened into dot-separated keys.

Given:

```yaml
messages:
  greeting:
    short: "Hi!"
    long: "Hello!"
```

you can write:

```rust
ezlz::t!("en", messages.greeting.short);
ezlz::t!("en", messages.greeting.long);
```

The macro converts the key into the equivalent runtime lookup:

```text
messages.greeting.short
```

## Interpolation

Translations can contain named placeholders:

```yaml
messages:
  greet: "Hello, %{name}!"
  count: "%{name} has %{count} messages."
```

A bare identifier is used as its own placeholder name:

```rust
let name = "Anna";

ezlz::t!("en", messages.greet, name);
```

This corresponds to:

```text
%{name} → Anna
```

Multiple placeholders can be supplied:

```rust
let name = "Anna";
let count = 42;

ezlz::t!("en", messages.count, name, count);
```

For arbitrary expressions, use an explicitly named argument:

```rust
let user = User {
    name: "Anna".to_owned(),
};

ezlz::t!(
    "en",
    messages.greet,
    name = &user.name
);
```

You can also use expressions directly:

```rust
ezlz::t!(
    "en",
    messages.count,
    name = &user.name,
    count = &items.len()
);
```

## Locale fallback

The first argument to `init` specifies the base locale:

```rust
ezlz::init("en", "locales").unwrap();
```

If a translation doesn't exist in the requested locale, `ezlz` looks for it in the base locale.

For example, if `en.yml` contains:

```yaml
errors:
  not_found: "Not found"
```

but `en_GB.yml` doesn't, this:

```rust
ezlz::t!("en_GB", errors.not_found);
```

falls back to:

```text
en/errors.not_found
```

and returns:

```text
Not found
```

A translation in the requested locale always takes precedence over the fallback locale.

## Supported files

`ezlz` loads files ending in:

```text
.yml
.yaml
```

The filename determines the locale:

```text
en.yml       → en
en_GB.yml    → en_GB
de.yml       → de
pt-BR.yml    → pt-BR
```

Other files in the directory are ignored.

## Initialization

Call `init` once during application startup:

```rust
ezlz::init("en", "locales")?;
```

The localization data is loaded into memory and remains available for the lifetime of the application.

`init` returns an error if:

* the localization directory doesn't exist
* a YAML file cannot be read
* a YAML file cannot be parsed
* a translation value isn't a string
* `ezlz` has already been initialized

Because initialization uses a global `OnceLock`, calling `init` more than once is not supported.

## Dynamic locales

The locale passed to `t!` can be any Rust expression:

```rust
let current_language = get_current_language();

let message = ezlz::t!(
    current_language,
    messages.hello
);
```

This makes it possible to select the language per request, user, session, or other application context without changing the global translation data.

## API

### `ezlz::init`

```rust
pub fn init(
    base_locale: impl Into<String>,
    directory: impl AsRef<Path>,
) -> Result<(), InitError>
```

Loads all `.yml` and `.yaml` files from `directory`.

### `ezlz::try_get`

```rust
pub fn try_get(
    locale: &str,
    key: &str,
    args: &[(&str, &dyn std::fmt::Display)],
) -> Option<String>
```

Performs a non-panicking translation lookup.

Returns `None` when the translation cannot be found in either the requested locale or the base locale.

### `ezlz::t!`

```rust
ezlz::t!(locale, key)
ezlz::t!(locale, key, argument)
ezlz::t!(locale, key, name = expression)
```

The macro converts the translation key into a string and forwards the lookup to the runtime.

## Error handling

Initialization should normally be handled explicitly:

```rust
ezlz::init("en", "locales")?;
```

The `t!` macro is intended for translations that are expected to exist. If `ezlz::init` has not been called, or a translation cannot be found, `t!` will panic with a descriptive error.

For code that needs to handle missing translations without panicking, use `try_get`:

```rust
match ezlz::try_get("de", "messages.hello", &[]) {
    Some(message) => println!("{message}"),
    None => println!("Translation missing"),
}
```

## Design

`ezlz` intentionally keeps the architecture simple:

```text
                 locales/*.yml
                       │
                       ▼
                 ezlz::init()
                       │
                       ▼
              ┌─────────────────┐
              │ Parse YAML      │
              │ Flatten keys    │
              │ Store in memory │
              └────────┬────────┘
                       │
                       ▼
          HashMap<locale, HashMap<key, value>>
                       │
                       ▼
                 t!(locale, key)
                       │
                       ▼
                    __get()
                       │
                 ┌─────┴─────┐
                 ▼           ▼
              locale       base locale
                 │           │
                 └─────┬─────┘
                       ▼
                  translation
```

The procedural macro deliberately doesn't know anything about YAML files or the available locales. It only transforms:

```rust
ezlz::t!(lang, messages.greet, name)
```

into a call to the runtime lookup function.

This keeps the proc-macro implementation small and keeps all localization loading in the normal library.