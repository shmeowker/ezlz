use super::*;

fn render(body: &str, arg: Arg) -> String {
    let (_, ruleset) = compile(body).unwrap();
    let mut output = String::new();
    ruleset.render(&mut output, &arg);
    output
}

#[test]
fn selectors_parse_to_op_and_bounds() {
    assert_eq!(parse_selector("").unwrap(), (Op::Fallback, u8::MAX, u8::MAX));
    assert_eq!(parse_selector(".").unwrap(), (Op::Float, u8::MAX, u8::MAX));
    assert_eq!(parse_selector("=1").unwrap(), (Op::Equals, 1, 1));
    assert_eq!(parse_selector("=0-1").unwrap(), (Op::Equals, 0, 1));
    assert_eq!(parse_selector("=9+").unwrap(), (Op::Equals, 9, u8::MAX));
    assert_eq!(parse_selector("~2+").unwrap(), (Op::FloatRange, 2, u8::MAX));
    assert_eq!(parse_selector("%2-4").unwrap(), (Op::Mod10, 2, 4));
    assert_eq!(parse_selector("#11-19").unwrap(), (Op::Mod100, 11, 19));
    assert_eq!(parse_selector("=254").unwrap(), (Op::Equals, 254, 254));
}

#[test]
fn invalid_selectors_are_rejected() {
    for selector in [
        "?", "?1", "x", " ", "=", "=a", "=-", "=1-", "=-1", "=+", "=1+2", "=256", "=1000", "=1-256", "=1.5", "=-1-2",
    ] {
        assert!(parse_selector(selector).is_err(), "{selector:?} should be rejected");
    }

    assert!(parse_selector(".5").is_err());
    assert!(parse_selector(".1-2").is_err());
}

#[test]
fn reversed_ranges_are_rejected() {
    assert!(compile("n|=5-1: x").is_err());
    assert!(compile("n|=1-1: x").is_ok());
    assert!(compile("n|=5+: x").is_ok());
}

#[test]
fn placeholder_names_are_validated() {
    assert!(compile("").is_err());
    assert!(compile(" n|=1: x").is_err());
    assert!(compile("a-b|=1: x").is_err());
    assert!(compile("é|=1: x").is_err());
    assert_eq!(&*compile("n_1|=1: x").unwrap().0, "n_1");
}

#[test]
fn rule_text_is_split_at_the_first_colon_only() {
    assert_eq!(render("n|=1: a: b", Arg::Int(1)), "1 a: b");
    assert_eq!(render("n|: a: b", Arg::Int(1)), "1 a: b");
    assert_eq!(render("n|=1:=a:b", Arg::Int(1)), "a:b");
}

#[test]
fn rules_are_evaluated_left_to_right() {
    assert_eq!(render("n|=1: a|=1: b| c", Arg::Int(1)), "1 a");
    assert_eq!(
        render("n| c|=1: a", Arg::Int(1)),
        "1 c",
        "an early fallback shadows later rules"
    );
}

#[test]
fn no_rules_and_empty_text() {
    assert_eq!(render("n", Arg::Int(5)), "5");
    assert_eq!(render("n|", Arg::Int(5)), "5");
    assert_eq!(render("n|=5:", Arg::Int(5)), "5");
    assert_eq!(render("n|=5:=", Arg::Int(5)), "");
}

#[test]
fn integer_matching() {
    assert_eq!(render("n|=0-1: a| b", Arg::Uint(1)), "1 a");
    assert_eq!(render("n|=0-1: a| b", Arg::Uint(2)), "2 b");
    assert_eq!(render("n|=2+: a| b", Arg::Uint(u64::MAX)), "18446744073709551615 a");
    assert_eq!(render("n|=1: a| b", Arg::Int(-1)), "-1 a");
    assert_eq!(render("n|=1: a| b", Arg::Int(i64::MIN)), "-9223372036854775808 b");
    assert_eq!(render("n|%0: a| b", Arg::Uint(1000)), "1000 a");
    assert_eq!(render("n|#0: a| b", Arg::Uint(1000)), "1000 a");
    assert_eq!(render("n|%5+: a| b", Arg::Uint(17)), "17 a");
}

#[test]
fn floats_only_match_float_aware_selectors() {
    for value in [1.0_f64, 1.5, 21.0] {
        for arg in [|v: f64| Arg::Float64(v), |v: f64| Arg::Float32(v as f32)] {
            let arg = arg(value);
            assert!(arg.is_float());
            let ends_with = |body: &str, suffix: &str| render(body, arg_clone(&arg)).ends_with(suffix);
            assert!(ends_with("n|=1: a| b", " b"), "`=` must not match {value}");
            assert!(ends_with("n|%1: a| b", " b"), "`%` must not match {value}");
            assert!(ends_with("n|#1: a| b", " b"), "`#` must not match {value}");
            assert!(ends_with("n|.: a| b", " a"), "`.` must match {value}");
            assert!(ends_with("n|~0-99: a| b", " a"), "`~` must match {value}");
        }
    }
    assert!(!Arg::Int(1).is_float() && !Arg::Uint(1).is_float() && !Arg::String("1").is_float());
}

/// `Arg` is not `Clone`; rebuild it from its numeric value.
fn arg_clone<'a>(arg: &Arg<'a>) -> Arg<'a> {
    match arg {
        Arg::Float64(v) => Arg::Float64(*v),
        Arg::Float32(v) => Arg::Float32(*v),
        _ => unreachable!(),
    }
}

#[test]
fn strings_bypass_rules() {
    assert_eq!(render("n|=1: a| b", Arg::String("one")), "one");
    assert_eq!(render("n|: a", Arg::String("one")), "one");
    assert_eq!(render("n|=1:=x", Arg::String("")), "");
}
