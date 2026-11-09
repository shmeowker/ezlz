//! Rendering, argument handling, lookup and fallback. Uses `tests/locales/extra.yml`.
mod common;

use std::borrow::Cow;

use common::init_test_locales;
use ezlz::t;

#[test]
fn plain_text_and_unicode() {
    init_test_locales();
    assert_eq!(t!("extra", r.plain), "Hello");
    assert_eq!(t!("extra", r.unicode, name = "Анна"), "Привет, Анна! 你好 🎉");
}

#[test]
fn keys_may_be_nested_flow_style_keywords_or_anchors() {
    init_test_locales();
    assert_eq!(t!("extra", r.nested.deep.key, x = 1), "deep 1");
    assert_eq!(t!("extra", r.flow.a.b, x = 1), "flow 1");
    assert_eq!(t!("extra", r.type), "keyword key");
    assert_eq!(t!("extra", r.anchor, x = 1), "anchored 1");
}

#[test]
fn integer_types() {
    init_test_locales();
    assert_eq!(t!("extra", r.var, x = 255u8), "255");
    assert_eq!(t!("extra", r.var, x = -128i8), "-128");
    assert_eq!(t!("extra", r.var, x = 65535u16), "65535");
    assert_eq!(t!("extra", r.var, x = -32768i16), "-32768");
    assert_eq!(t!("extra", r.var, x = u32::MAX), "4294967295");
    assert_eq!(t!("extra", r.var, x = i32::MIN), "-2147483648");
    assert_eq!(t!("extra", r.var, x = u64::MAX), "18446744073709551615");
    assert_eq!(t!("extra", r.var, x = i64::MIN), "-9223372036854775808");
    assert_eq!(t!("extra", r.var, x = usize::MAX), usize::MAX.to_string());
    assert_eq!(t!("extra", r.var, x = isize::MIN), isize::MIN.to_string());
    assert_eq!(t!("extra", r.var, x = 0), "0");
}

#[test]
fn float_types_use_their_own_precision() {
    init_test_locales();
    // An f32 must not be widened to f64 before printing.
    assert_eq!(t!("extra", r.var, x = 1.1_f32), "1.1");
    assert_eq!(t!("extra", r.var, x = 0.1_f32), "0.1");
    assert_eq!(t!("extra", r.var, x = 1.1_f64), "1.1");
    assert_eq!(t!("extra", r.var, x = 100.0_f32), "100.0");
    assert_eq!(t!("extra", r.var, x = -2.5_f64), "-2.5");
}

#[test]
fn string_types() {
    init_test_locales();
    let owned = String::from("owned");
    let borrowed: &str = "borrowed";
    assert_eq!(t!("extra", r.var, x = "literal"), "literal");
    assert_eq!(t!("extra", r.var, x = owned), "owned");
    assert_eq!(t!("extra", r.var, x = borrowed), "borrowed");
    assert_eq!(t!("extra", r.var, x = owned.as_str()), "owned");
    assert_eq!(t!("extra", r.var, x = ""), "");
}

#[test]
fn box_and_cow_string_arguments() {
    init_test_locales();
    let boxed: Box<str> = "boxed".into();
    let borrowed: Cow<str> = Cow::Borrowed("cow");
    let owned: Cow<str> = Cow::Owned(String::from("owned cow"));
    assert_eq!(t!("extra", r.var, x = boxed), "boxed");
    assert_eq!(t!("extra", r.var, x = borrowed), "cow");
    assert_eq!(t!("extra", r.var, x = owned), "owned cow");
    // Plural placeholders print non-numeric values as they are.
    assert_eq!(t!("extra", r.nomatch, n = Box::<str>::from("many")), "many");
}

// With `fast-floats`, `zmij::Buffer::format_finite` is used, which prints an unspecified
// number for NaN and infinity (e.g. `2.696539702293474e+308` for NaN), so this is skipped.
#[test]
#[cfg_attr(feature = "fast-floats", ignore = "format_finite renders NaN/inf as unspecified numbers")]
fn non_finite_floats_render_readably() {
    init_test_locales();
    assert_eq!(t!("extra", r.var, x = f64::NAN), "NaN");
    assert_eq!(t!("extra", r.var, x = f64::INFINITY), "inf");
    assert_eq!(t!("extra", r.var, x = f64::NEG_INFINITY), "-inf");
    assert_eq!(t!("extra", r.var, x = f32::NAN), "NaN");
    assert_eq!(t!("extra", r.var, x = f32::INFINITY), "inf");
}

#[test]
fn argument_forms() {
    init_test_locales();
    let x = 5;
    let a = "A";
    let b = 2;

    // Bare identifier, explicit name, expression, trailing comma.
    assert_eq!(t!("extra", r.var, x), "5");
    assert_eq!(t!("extra", r.var, x = x + 1), "6");
    assert_eq!(t!("extra", r.var, x = format!("<{x}>")), "<5>");
    assert_eq!(t!("extra", r.var, x,), "5");
    assert_eq!(t!("extra", r.two, a, b), "A and 2");
    assert_eq!(t!("extra", r.two, b, a), "A and 2", "order of arguments is irrelevant");
    assert_eq!(t!("extra", r.two, a = "left", b = "right",), "left and right");
}

#[test]
fn arguments_are_matched_by_name() {
    init_test_locales();
    // A repeated placeholder takes the same value everywhere.
    assert_eq!(t!("extra", r.twice, x = 3), "3-3");
    // Unknown arguments are ignored.
    assert_eq!(t!("extra", r.plain, unused = 1, other = "x"), "Hello");
    assert_eq!(t!("extra", r.var, x = 1, y = 2), "1");
    // A missing argument renders as nothing.
    assert_eq!(t!("extra", r.var), "");
    assert_eq!(t!("extra", r.two, a = "A"), "A and ");
    assert_eq!(t!("extra", r.open), "");
    // When a name is passed twice, the first one wins.
    assert_eq!(t!("extra", r.var, x = 1, x = 2), "1");
}

#[test]
fn arguments_are_evaluated_once() {
    init_test_locales();
    let mut calls = 0;
    let mut next = || {
        calls += 1;
        calls
    };
    assert_eq!(t!("extra", r.twice, x = next()), "1-1");
    assert_eq!(calls, 1);
}

#[test]
fn locale_expression_types() {
    init_test_locales();
    let string = String::from("extra");
    let boxed: Box<str> = "extra".into();
    let cow: Cow<str> = Cow::Borrowed("extra");
    let borrowed: &str = "extra";
    let pick = |yes: bool| if yes { "extra" } else { "test" };

    assert_eq!(t!(string, shared.greeting), "extra greeting");
    assert_eq!(t!(&string, shared.greeting), "extra greeting");
    assert_eq!(t!(boxed, shared.greeting), "extra greeting");
    assert_eq!(t!(cow, shared.greeting), "extra greeting");
    assert_eq!(t!(borrowed, shared.greeting), "extra greeting");
    assert_eq!(t!(pick(true), shared.greeting), "extra greeting");
    assert_eq!(t!(pick(false), shared.greeting), "fallback greeting");
    assert_eq!(
        t!(if string.is_empty() { "test" } else { "extra" }, shared.greeting),
        "extra greeting"
    );
    assert_eq!(t!(format!("{}ra", "ext"), shared.greeting), "extra greeting");
}

#[test]
fn fallback_locale() {
    init_test_locales();
    // Present in the requested locale: it wins over the fallback.
    assert_eq!(t!("extra", shared.greeting), "extra greeting");
    assert_eq!(t!("test", shared.greeting), "fallback greeting");
    // Missing in the requested locale: the fallback is used.
    assert_eq!(t!("partial", shared.greeting), "fallback greeting");
    assert_eq!(t!("partial", test.en, i = 1), "1 fox");
    // The requested locale is used for the keys it has.
    assert_eq!(t!("partial", only.here), "only in partial");
    // Unknown locales, including regional variants, go to the fallback.
    assert_eq!(t!("zz", shared.greeting), "fallback greeting");
    assert_eq!(t!("extra_GB", shared.greeting), "fallback greeting");
    assert_eq!(t!("", shared.greeting), "fallback greeting");
    // Keys that exist only in a non-fallback locale are not found in others.
    assert_eq!(t!("partial", only.here), "only in partial");
}

#[test]
#[cfg(not(feature = "stringify-missing-keys"))]
#[should_panic(expected = "translation 'nope.key' not found in locale 'extra' and fallback locale")]
fn missing_key_panics() {
    init_test_locales();
    let _ = t!("extra", nope.key);
}

#[test]
#[cfg(not(feature = "stringify-missing-keys"))]
#[should_panic(expected = "translation 'only.here' not found in locale 'test'")]
fn key_missing_from_fallback_panics_even_if_another_locale_has_it() {
    init_test_locales();
    let _ = t!("test", only.here);
}

#[test]
fn plural_without_matching_rule_prints_just_the_number() {
    init_test_locales();
    assert_eq!(t!("extra", r.nomatch, n = 1), "1 one");
    assert_eq!(t!("extra", r.nomatch, n = 5), "5");
    assert_eq!(t!("extra", r.nomatch, n = 1.5), "1.5");
}

#[test]
fn plural_with_non_numeric_argument_prints_the_value() {
    init_test_locales();
    assert_eq!(t!("extra", r.nomatch, n = "abc"), "abc");
    assert_eq!(t!("extra", r.open, n = String::from("many apples")), "many apples");
}

#[test]
fn plural_open_ranges() {
    init_test_locales();
    assert_eq!(t!("extra", r.open, n = 0), "0 few");
    assert_eq!(t!("extra", r.open, n = 1), "1 few");
    assert_eq!(t!("extra", r.open, n = 2), "2 many");
    assert_eq!(t!("extra", r.open, n = 1_000_000u64), "1000000 many");
    assert_eq!(t!("extra", r.open, n = -2), "-2 many", "the sign is ignored when matching");
    // `=` never matches floats, so they reach the fallback.
    assert_eq!(t!("extra", r.open, n = 5.5), "5.5 few");
}

#[test]
fn plural_float_selector() {
    init_test_locales();
    assert_eq!(t!("extra", r.float_dot, n = 1.0_f64), "1.0 float");
    assert_eq!(t!("extra", r.float_dot, n = 1.0_f32), "1.0 float");
    assert_eq!(t!("extra", r.float_dot, n = 1), "1 int");
    assert_eq!(t!("extra", r.float_dot, n = 1u64), "1 int");
}

#[test]
fn plural_float_range_selector() {
    init_test_locales();
    // `~` matches the truncated absolute value of floats and integers alike.
    assert_eq!(t!("extra", r.tilde, n = 0), "0 few");
    assert_eq!(t!("extra", r.tilde, n = 1), "1 few");
    assert_eq!(t!("extra", r.tilde, n = 0.5), "0.5 few");
    assert_eq!(t!("extra", r.tilde, n = -1.9), "-1.9 few");
    assert_eq!(t!("extra", r.tilde, n = 2), "2 many");
    assert_eq!(t!("extra", r.tilde, n = 2.0), "2.0 many");
}

#[test]
fn plural_modulo_selectors() {
    init_test_locales();
    assert_eq!(t!("extra", r.mod10, n = 1), "1 one");
    assert_eq!(t!("extra", r.mod10, n = 21), "21 one");
    assert_eq!(t!("extra", r.mod10, n = -21), "-21 one");
    assert_eq!(t!("extra", r.mod10, n = 24), "24 few");
    assert_eq!(t!("extra", r.mod10, n = 10), "10 many");
    // Modulo selectors do not match floats.
    assert_eq!(t!("extra", r.mod10, n = 21.0), "21.0 many");

    assert_eq!(t!("extra", r.mod100, n = 11), "11 teen");
    assert_eq!(t!("extra", r.mod100, n = 111), "111 teen");
    assert_eq!(t!("extra", r.mod100, n = 114), "114 teen");
    assert_eq!(t!("extra", r.mod100, n = 15), "15 other");
    assert_eq!(t!("extra", r.mod100, n = 211u32), "211 teen");
}

#[test]
fn plural_replace_rules() {
    init_test_locales();
    assert_eq!(t!("extra", r.replace, n = 0), "none");
    assert_eq!(t!("extra", r.replace, n = 1), "one");
    assert_eq!(t!("extra", r.replace, n = 7), "7 many");
}

#[test]
fn plural_rule_text_may_contain_colons() {
    init_test_locales();
    assert_eq!(t!("extra", r.colon, n = 1), "1 item");
    assert_eq!(t!("extra", r.colon, n = 3), "3 items: all");
}

#[test]
fn plural_special_floats_do_not_panic() {
    init_test_locales();
    assert!(t!("extra", r.open, n = f64::NAN).ends_with(" few"));
    assert!(t!("extra", r.open, n = f64::INFINITY).ends_with(" few"));
    assert!(t!("extra", r.open, n = f64::NEG_INFINITY).ends_with(" few"));
    assert!(t!("extra", r.open, n = f32::NAN).ends_with(" few"));
    assert!(t!("extra", r.open, n = -0.0_f64).ends_with(" few"));
}

#[test]
fn threads_can_render_concurrently() {
    init_test_locales();
    let handles: Vec<_> = (0..8)
        .map(|id| {
            std::thread::spawn(move || {
                for n in 0..2_000 {
                    assert_eq!(
                        t!("test", test.en, i = n),
                        if n == 1 { "1 fox".to_owned() } else { format!("{n} foxes") }
                    );
                    assert_eq!(t!("extra", r.var, x = id), id.to_string());
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}
