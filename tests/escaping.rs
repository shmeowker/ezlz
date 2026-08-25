use ezlz::t;

#[test]
fn test_escaping() {
    ezlz::init("test", "tests/locales").unwrap();

    assert_eq!(t!("test", test.escape, x = "foo"), "{x}");
    assert_eq!(t!("test", test.escape2, x = "foo"), r"\\foo");
    assert_eq!(t!("test", test.escape3, x = "foo"), r"\\{x}");
    assert_eq!(
        t!("test", test.escape4, x = "foo"),
        r"{x} bar \\foo qux \\{x}"
    );
}
