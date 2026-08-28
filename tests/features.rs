#[test]
#[cfg(feature = "missing-key-nopanic")]
fn missing_key_nopanic() {
    use ezlz::t;
    
    ezlz::init("test", "tests/locales").unwrap();

    assert_eq!(t!("test", nonexistent.key), "nonexistent.key");
}