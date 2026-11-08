#![cfg(feature = "missing-key-nopanic")]

use ezlz::t;

#[test]
fn missing_key_returns_the_key() {
    ezlz::init("test", "tests/locales").unwrap();

    // Missing in every locale, with and without arguments, for known and unknown locales.
    assert_eq!(t!("test", nonexistent.key), "nonexistent.key");
    assert_eq!(t!("extra", nonexistent.key, x = 1), "nonexistent.key");
    assert_eq!(t!("zz", a.b.c), "a.b.c");

    // Existing keys are unaffected.
    assert_eq!(t!("extra", shared.greeting), "extra greeting");
}
