# Changelog

## v3.0.1 (2026-11-18)

### Fixed
- Changed visibility for `Transaltions` to private.
### Changed 
- Private items are no longer included in docs.
- Better docs for `reinit_leak` and a couple other items.

## v3.0.0 (2026-11-18)

Compared with `ezlz` 2.0.0 and `ezlz-macros` 0.3.0.

### Breaking changes

- **New escaping scheme for templates.** Backslash escapes are gone;
  backslashes are now plain text.
  - In text, `{{` is a literal `{`. A run of `n` braces gives `n / 2` literal
    braces, and if `n` is odd, the last brace opens a placeholder.
    A lone `}` is plain text.
  - Migration: `\{x}` becomes `{{x}`.
- **Plural rule text may be quoted in backticks.** Quoted text is literal, so it
  can contain `{`, `}`, `|` and `:`. A doubled backtick outside a quote is a
  literal backtick. The `=` replace marker only counts when it is unquoted.
- **An unquoted `{` in plural rule text is now a template error.** Previously
  such templates compiled and rendered incorrectly. Quote the brace
  (`` `{` ``) to keep it.
- **Range bound 255 in plural selectors is now an error** (e.g. `=255`,
  `=1-255`, `#255+`), because 255 was used internally as the open-range marker.
  It used to be accepted and silently behave incorrectly.
- **`Error` has been reworked** and is now `#[non_exhaustive]`:
  - `ParseYaml { path, message }` is now `Parse { path, source }`,
    where `source` is the boxed underlying parser error.
  - `InvalidYaml` is now `Value`.
  - `Error` implements `source()` for `Io` and `Parse`.
- **`set_cleanup_delay` is now `set_grace_period`.** The default delay before
  the old storage is freed after `reinit` is now 60 seconds instead of 1.
- **Cargo feature `missing-key-nopanic` is now `stringify-missing-keys`.**
- **YAML backend changed from `serde_yaml` to `yaml_serde`.** This is visible
  only through the `Parse` error's `source`.
- `ezlz-macros` is bumped to 0.4.0, and `ezlz` now depends on it by that
  version.

### Added

- `wt!` macro: renders a translation into an existing `&mut String` without
  allocating a new one: `wt!(&mut buf, "en", foo.bar, name)`.
- `reinit_leak`: a safe alternative to `reinit` that swaps in a new store and
  never frees the old one, so it has no timing requirements.
- Unit test modules for the template and plural parsers (`unit_tests/`),
  new integration tests for escaping, quoting and errors, and benchmarks for
  `wt!`.
- Explicit `rust-version = "1.85.0"`.

### Fixed

- Raw identifiers work as argument names: `t!("en", key, r#type = 1)` binds the
  placeholder `{type}`.

### Changed

- The template parser is rewritten as a single-pass state machine
  (`TryFrom<&str> for Template`, `TryFrom<&mut Peekable<Chars>> for Ruleset`)
  instead of splitting the source by `{` and `|`.
- Source layout: the YAML loader moved to `src/loader.rs` and the error type
  to `src/error.rs`.
- `reinit` safety documentation is rewritten to describe the stalled-reader
  case and to point to `reinit_leak`.
- README: new "Escaping and quoting" section, `wt!` documentation, an updated
  hot-reloading section and refreshed benchmarks.
- Workflow renamed from `ci.yml` to `tests.yml`.