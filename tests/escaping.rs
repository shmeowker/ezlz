//! Escaping of braces and quoting in plural rules. Uses `tests/locales/test.yml`.
mod common;

use common::init_test_locales;
use ezlz::t;

#[test]
fn a_doubled_brace_is_a_literal_brace() {
    init_test_locales();
    assert_eq!(t!("test", test.escape, x = "foo"), "{x}");
    assert_eq!(t!("test", test.escape4, x = "foo"), r"{x} bar \foo qux \{x}");
}

#[test]
fn backslashes_are_plain_text() {
    init_test_locales();
    assert_eq!(t!("test", test.escape2, x = "foo"), r"\foo");
    assert_eq!(t!("test", test.escape3, x = "foo"), r"\{x}");
}

/// No backslashes are involved, so the style of the YAML scalar doesn't matter.
#[test]
fn escapes_work_in_every_yaml_scalar_style() {
    init_test_locales();
    // Double quoted: `escape`, single quoted: `escape2`, plain and block:
    assert_eq!(t!("test", test.escape_plain, x = "foo"), "a {x} b foo");
    assert_eq!(t!("test", test.escape_block, x = "foo"), "{x}\n\\foo");
}

#[test]
fn quoted_plural_rule_texts() {
    init_test_locales();
    assert_eq!(t!("test", test.quote, i = 1), "1 {one}");
    assert_eq!(t!("test", test.quote, i = 2), "2 {many}");
    assert_eq!(t!("test", test.quote_pipe, i = 1), "1 a | b");
    assert_eq!(t!("test", test.quote_pipe, i = 5), "5 c");
}

#[test]
fn a_plural_placeholder_may_be_followed_by_a_closing_brace() {
    init_test_locales();
    assert_eq!(t!("test", test.quote_tail, i = 1), "1 x}");
    assert_eq!(t!("test", test.quote_tail, i = 2), "2}");
}
