//! Every way `init` can fail. None of these may succeed, so the global store stays untouched.
mod common;

use common::locale_dir;
use ezlz::Error;

/// Initializes from a fresh directory with the given files and returns the error.
fn fail(name: &str, files: &[(&str, &str)]) -> Error {
    ezlz::init("en", locale_dir(name, files)).expect_err("init should have failed")
}

#[test]
fn error_implements_std_error() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<Error>();
}

#[test]
fn missing_directory() {
    let err = ezlz::init("en", "tests/does-not-exist").unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "{err:?}");
    assert!(err.to_string().contains("does-not-exist"), "{err}");
}

#[test]
fn invalid_yaml_syntax() {
    let err = fail("syntax", &[("en.yml", "k: [unclosed\n")]);
    assert!(matches!(err, Error::ParseYaml { .. }), "{err:?}");
    assert!(err.to_string().contains("en.yml"), "{err}");
}

#[test]
fn values_must_be_strings() {
    for (name, content) in [
        ("number", "k: 5\n"),
        ("bool", "k: true\n"),
        ("null", "k:\n"),
        ("sequence", "k:\n  - a\n  - b\n"),
        ("nested", "a:\n  b:\n    c: 1.5\n"),
        ("empty", ""),
    ] {
        let err = fail(&format!("value-{name}"), &[("en.yml", content)]);
        assert!(matches!(err, Error::InvalidYaml { .. }), "{name}: {err:?}");
    }
}

#[test]
fn keys_must_be_strings() {
    let err = fail("key-number", &[("en.yml", "5: x\n")]);
    assert!(matches!(err, Error::InvalidYaml { .. }), "{err:?}");
}

#[test]
fn quoted_scalars_are_strings_not_errors() {
    // The same values as above are fine when quoted, so these must get past YAML
    // validation and fail only because the fallback locale is missing.
    let dir = locale_dir("quoted", &[("de.yml", "a: \"5\"\nb: 'true'\nc: \"\"\n")]);
    let err = ezlz::init("en", dir).unwrap_err();
    assert!(matches!(err, Error::FallbackLocaleNotFound { .. }), "{err:?}");
}

#[test]
fn invalid_templates() {
    for (name, template) in [
        ("unclosed", "{x"),
        ("empty-placeholder", "a {} b"),
        ("space-in-name", "{ x }"),
        ("plural-bad-name", "{a b|=1: x}"),
        ("bad-selector", "{n|?1: x}"),
        ("bad-bound", "{n|=a: x}"),
        ("missing-bound", "{n|=: x}"),
        ("reversed-range", "{n|=5-1: x}"),
        ("too-large", "{n|=1000: x}"),
        ("open-too-large", "{n|=1000+: x}"),
        ("colon-in-fallback", "{n|=1: item| items: all}"),
    ] {
        let yaml = format!("a:\n  b: \"{template}\"\n");
        let err = fail(&format!("template-{name}"), &[("en.yml", &yaml)]);
        match &err {
            Error::InvalidTemplate { key, message, .. } => {
                assert_eq!(key, "a.b", "{name}");
                assert!(!message.is_empty(), "{name}");
            },
            other => panic!("{name}: expected InvalidTemplate, got {other:?}"),
        }
        assert!(err.to_string().contains("a.b"), "{err}");
    }
}

#[test]
fn fallback_locale_must_exist() {
    let err = fail("no-fallback", &[("de.yml", "k: x\n")]);
    match &err {
        Error::FallbackLocaleNotFound { name, .. } => assert_eq!(name, "en"),
        other => panic!("{other:?}"),
    }
    assert!(err.to_string().contains("'en'"), "{err}");
}

#[test]
fn empty_directory_has_no_fallback() {
    let err = fail("empty-dir", &[]);
    assert!(matches!(err, Error::FallbackLocaleNotFound { .. }), "{err:?}");
}

#[test]
fn duplicate_locale_files() {
    let err = fail("dup-locale", &[("en.yml", "k: a\n"), ("en.yaml", "k: b\n")]);
    match &err {
        Error::DuplicateLocale { name } => assert_eq!(name, "en"),
        other => panic!("{other:?}"),
    }
    assert!(err.to_string().contains("en"), "{err}");
}

#[test]
fn duplicate_keys_after_flattening() {
    // `a.b` is defined both directly and through nesting.
    let err = fail("dup-key", &[("en.yml", "a.b: x\na:\n  b: y\n")]);
    match &err {
        Error::DuplicateKey { key, .. } => assert_eq!(key, "a.b"),
        other => panic!("{other:?}"),
    }
    assert!(err.to_string().contains("a.b"), "{err}");
}

#[test]
fn duplicate_keys_in_one_mapping() {
    // Rejected by the YAML parser itself.
    let err = fail("dup-key-same-map", &[("en.yml", "a: x\na: y\n")]);
    assert!(matches!(err, Error::ParseYaml { .. } | Error::DuplicateKey { .. }), "{err:?}");
}

#[test]
fn error_in_any_file_fails_everything() {
    let err = fail("one-bad-file", &[("en.yml", "k: fine\n"), ("de.yml", "k: \"{broken\"\n")]);
    assert!(matches!(err, Error::InvalidTemplate { .. }), "{err:?}");
    assert!(err.to_string().contains("de.yml"), "{err}");
}
