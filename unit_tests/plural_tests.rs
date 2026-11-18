use super::*;

/// Parses the rules of a placeholder from the text that follows `name|`.
///
/// Returns the result and whatever is left of the input afterwards.
fn parse_rest(src: &str) -> (Result<Ruleset, &'static str>, String) {
    let mut iter = src.chars().peekable();
    let result = Ruleset::try_from(&mut iter);
    (result, iter.collect())
}

/// Parses `rules` followed by the closing brace and expects nothing to be left over.
fn compile(rules: &str) -> Result<Ruleset, &'static str> {
    let (result, rest) = parse_rest(&format!("{rules}}}"));
    if result.is_ok() {
        assert_eq!(rest, "", "{rules:?}");
    }
    result
}

fn render(rules: &str, arg: Arg) -> String {
    let ruleset = compile(rules).unwrap_or_else(|e| panic!("{rules:?}: {e}"));
    let mut output = String::new();
    ruleset.render(&mut output, &arg);
    output
}

// Selectors.

#[test]
fn selectors_parse_to_op_and_bounds() {
    assert_eq!(
        Selector::try_from("").unwrap(),
        Selector {
            op: Op::Fallback,
            lo: !0,
            hi: !0
        }
    );
    assert_eq!(
        Selector::try_from(".").unwrap(),
        Selector {
            op: Op::Float,
            lo: !0,
            hi: !0
        }
    );
    assert_eq!(
        Selector::try_from("=1").unwrap(),
        Selector {
            op: Op::Equals,
            lo: 1,
            hi: 1
        }
    );
    assert_eq!(
        Selector::try_from("=0-1").unwrap(),
        Selector {
            op: Op::Equals,
            lo: 0,
            hi: 1
        }
    );
    assert_eq!(
        Selector::try_from("=9+").unwrap(),
        Selector {
            op: Op::Equals,
            lo: 9,
            hi: !0
        }
    );
    assert_eq!(
        Selector::try_from("~2+").unwrap(),
        Selector {
            op: Op::FloatRange,
            lo: 2,
            hi: !0
        }
    );
    assert_eq!(
        Selector::try_from("%2-4").unwrap(),
        Selector {
            op: Op::Mod10,
            lo: 2,
            hi: 4
        }
    );
    assert_eq!(
        Selector::try_from("#11-19").unwrap(),
        Selector {
            op: Op::Mod100,
            lo: 11,
            hi: 19
        }
    );
    assert_eq!(
        Selector::try_from("=254").unwrap(),
        Selector {
            op: Op::Equals,
            lo: 254,
            hi: 254
        }
    );
}

#[test]
fn invalid_selectors_are_rejected() {
    for selector in [
        "?", "?1", "x", " ", "=", "=a", "=-", "=1-", "=-1", "=+", "=1+2", "=256", "=1000", "=1-256", "=1.5", "=-1-2",
    ] {
        assert!(Selector::try_from(selector).is_err(), "{selector:?} should be rejected");
    }

    assert!(Selector::try_from(".5").is_err());
    assert!(Selector::try_from(".1-2").is_err());
}

/// 255 doubles as the "no upper bound" marker, so it can't be a real bound.
#[test]
fn bound_255_is_rejected() {
    for selector in [
        "=255", "=254-255", "=255+", "=1-255", "%255", "#255", "~255", "~0-255", "=255-255",
    ] {
        assert!(Selector::try_from(selector).is_err(), "{selector:?} should be rejected");
    }
    assert!(Selector::try_from("=254-254").is_ok());
    assert!(Selector::try_from("=254+").is_ok());
}

// Rule structure.

#[test]
fn rules_are_split_at_pipes() {
    let rules = compile("=1: a|%2: b| c").unwrap().rules;
    assert_eq!(rules.len(), 3);

    assert_eq!((&rules[0].sel.op, rules[0].sel.lo, rules[0].sel.hi), (&Op::Equals, 1, 1));
    assert_eq!((&*rules[0].text, rules[0].replace), (" a", false));

    assert_eq!((&rules[1].sel.op, rules[1].sel.lo, rules[1].sel.hi), (&Op::Mod10, 2, 2));
    assert_eq!((&*rules[1].text, rules[1].replace), (" b", false));

    assert_eq!((&rules[2].sel.op, rules[2].sel.lo, rules[2].sel.hi), (&Op::Fallback, !0, !0));
    assert_eq!((&*rules[2].text, rules[2].replace), (" c", false));
}

#[test]
fn a_rule_without_a_colon_is_a_fallback() {
    // It must not inherit the selector of the rule in front of it.
    assert_eq!(render("=1: a| b", Arg::Int(5)), "5 b");
    assert_eq!(render("=1: a| b", Arg::Int(1)), "1 a");
    assert_eq!(compile("=1: a| b").unwrap().rules[1].sel.op, Op::Fallback);
    // Also at the very start.
    assert_eq!(render(" c", Arg::Int(3)), "3 c");
    assert_eq!(render("=1: a| b|=2: c", Arg::Int(2)), "2 b", "the first fallback wins");
}

#[test]
fn rule_text_is_split_at_the_first_colon_only() {
    assert_eq!(render("=1: a: b", Arg::Int(1)), "1 a: b");
    assert_eq!(render(": a: b", Arg::Int(1)), "1 a: b");
    assert_eq!(render("=1:=a:b", Arg::Int(1)), "a:b");
}

#[test]
fn a_fallback_text_with_a_colon_needs_an_explicit_selector() {
    // ` b` is read as a selector here.
    assert!(compile("=1: a| b: c").is_err());
    assert_eq!(render("=1: a|: b: c", Arg::Int(2)), "2 b: c");
}

#[test]
fn rules_are_evaluated_left_to_right() {
    assert_eq!(render("=1: a|=1: b| c", Arg::Int(1)), "1 a");
    assert_eq!(
        render(" c|=1: a", Arg::Int(1)),
        "1 c",
        "an early fallback shadows later rules"
    );
}

#[test]
fn no_rules_and_empty_text() {
    assert_eq!(render("", Arg::Int(5)), "5");
    assert_eq!(render("=5:", Arg::Int(5)), "5");
    assert_eq!(render("=5:=", Arg::Int(5)), "");
}

#[test]
fn the_replacement_marker() {
    assert_eq!(render("=0:=zero|=1:=one| many", Arg::Int(0)), "zero");
    assert_eq!(render("=0:=zero|=1:=one| many", Arg::Int(1)), "one");
    assert_eq!(render("=0:=zero|=1:=one| many", Arg::Int(7)), "7 many");
    // A fallback without a selector can replace the number as well.
    assert_eq!(render("=1: a|=b", Arg::Int(2)), "b");
    assert_eq!(render("=1: a|=b", Arg::Int(1)), "1 a");
}

// Where parsing stops.

#[test]
fn parsing_stops_after_the_closing_brace() {
    let (result, rest) = parse_rest("=1: a| b} tail {x}");
    assert!(result.is_ok());
    assert_eq!(rest, " tail {x}");

    // A second brace belongs to the text after the placeholder.
    let (result, rest) = parse_rest("=1: a}}");
    assert!(result.is_ok());
    assert_eq!(rest, "}");

    let (result, rest) = parse_rest("}x");
    assert!(result.is_ok());
    assert_eq!(rest, "x");
}

// Matching.

#[test]
fn integer_matching() {
    assert_eq!(render("=0-1: a| b", Arg::Uint(1)), "1 a");
    assert_eq!(render("=0-1: a| b", Arg::Uint(2)), "2 b");
    assert_eq!(render("=2+: a| b", Arg::Uint(u64::MAX)), "18446744073709551615 a");
    assert_eq!(render("=1: a| b", Arg::Int(-1)), "-1 a");
    assert_eq!(render("=1: a| b", Arg::Int(i64::MIN)), "-9223372036854775808 b");
    assert_eq!(render("%0: a| b", Arg::Uint(1000)), "1000 a");
    assert_eq!(render("#0: a| b", Arg::Uint(1000)), "1000 a");
    assert_eq!(render("%5+: a| b", Arg::Uint(17)), "17 a");
}

#[test]
fn floats_only_match_float_aware_selectors() {
    for value in [1.0_f64, 1.5, 21.0] {
        for arg in [|v: f64| Arg::Float64(v), |v: f64| Arg::Float32(v as f32)] {
            let arg = arg(value);
            assert!(arg.is_float());
            let ends_with = |rules: &str, suffix: &str| render(rules, arg_clone(&arg)).ends_with(suffix);
            assert!(ends_with("=1: a| b", " b"), "`=` must not match {value}");
            assert!(ends_with("%1: a| b", " b"), "`%` must not match {value}");
            assert!(ends_with("#1: a| b", " b"), "`#` must not match {value}");
            assert!(ends_with(".: a| b", " a"), "`.` must match {value}");
            assert!(ends_with("~0-99: a| b", " a"), "`~` must match {value}");
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
    assert_eq!(render("=1: a| b", Arg::String("one")), "one");
    assert_eq!(render(": a", Arg::String("one")), "one");
    assert_eq!(render("=1:=x", Arg::String("")), "");
}

#[test]
fn unicode_rule_texts() {
    let ru = ".: стола|#11-14: столов|%1: стол|%2-4: стола| столов";
    for (n, expected) in [
        (1, "1 стол"),
        (2, "2 стола"),
        (5, "5 столов"),
        (11, "11 столов"),
        (21, "21 стол"),
        (22, "22 стола"),
    ] {
        assert_eq!(render(ru, Arg::Uint(n)), expected);
    }
    assert_eq!(render(ru, Arg::Float64(1.5)), "1.5 стола");
    assert_eq!(render("=1: 🦊| 🦊🦊", Arg::Int(2)), "2 🦊🦊");
}

// Quoting.

#[test]
fn quoted_text_is_literal() {
    // Braces, pipes and colons lose their meaning between backticks.
    assert_eq!(render("=1: `a | b }`| other", Arg::Int(1)), "1 a | b }");
    assert_eq!(render("=1: `a | b }`| other", Arg::Int(2)), "2 other");
    assert_eq!(render("=1: `{x}: y`|: `z}`", Arg::Int(1)), "1 {x}: y");
    assert_eq!(render("=1: `{x}: y`|: `z}`", Arg::Int(2)), "2 z}");
    // Spaces inside the quotes are kept.
    assert_eq!(render("=1:=` a `", Arg::Int(1)), " a ");
}

#[test]
fn the_replacement_marker_goes_in_front_of_a_quote() {
    assert_eq!(render("=1:=`x}`| other", Arg::Int(1)), "x}");
    assert_eq!(render("=1:=`x}`| other", Arg::Int(2)), "2 other");
}

#[test]
fn a_quote_may_start_anywhere_in_the_text() {
    // The backticks themselves are removed.
    assert_eq!(render("=1: a`b c`d", Arg::Int(1)), "1 ab cd");
    assert_eq!(render("=1: a`}`b", Arg::Int(1)), "1 a}b");
    assert_eq!(render("=1: `a`|: `b`", Arg::Int(2)), "2 b");
}

#[test]
fn a_fallback_without_a_selector_may_be_quoted() {
    assert_eq!(render("=1: a| `b | c }`", Arg::Int(2)), "2 b | c }");
    assert_eq!(render("=1: a|`{b}`", Arg::Int(2)), "2{b}");
    // A colon after a quote is plain text.
    assert_eq!(render("=1: a|`b`: c", Arg::Int(2)), "2b: c");
}

#[test]
fn a_doubled_backtick_is_a_literal_backtick() {
    assert_eq!(render("=1:=``x", Arg::Int(1)), "`x");
    assert_eq!(render("=1: a``b", Arg::Int(1)), "1 a`b");
    assert_eq!(render("=1:=a``", Arg::Int(1)), "a`");
    assert_eq!(render("=1:=````", Arg::Int(1)), "``");
    // A literal backtick followed by a quote.
    assert_eq!(render("=1:=```a`", Arg::Int(1)), "`a");
    // In the position of a selector, for a fallback.
    assert_eq!(render("=1: a|``b", Arg::Int(2)), "2`b");
}

#[test]
fn the_first_unquoted_brace_closes_the_placeholder() {
    let (result, rest) = parse_rest("=1: `}` a} tail");
    assert!(result.is_ok());
    assert_eq!(rest, " tail");

    let (result, rest) = parse_rest("=1: `a`}} tail");
    assert!(result.is_ok());
    assert_eq!(rest, "} tail");
}

// Errors.

#[test]
fn unterminated_rules_are_rejected() {
    for src in [
        "",
        "=1:",
        "=1: a",
        "=1: a|",
        "=1: a| b",
        "=1: a``",
        "=1: `a`",
        // The brace is inside the quote.
        "=1: `a}",
        "`",
        "=1: a|`b}",
    ] {
        assert_eq!(parse_rest(src).0.unwrap_err(), "unterminated plural placeholder", "{src:?}");
    }
}

#[test]
fn selector_errors_are_passed_through() {
    assert_eq!(compile("?: x").unwrap_err(), "invalid plural rule selector operator");
    assert_eq!(
        compile("=5-1: x").unwrap_err(),
        "selector range lower bound is greater than its upper bound"
    );
    assert!(compile("=a: x").is_err());
    assert!(compile("=1-1: x").is_ok());
    assert!(compile("=5+: x").is_ok());
    // Rules after a valid one are checked too.
    assert!(compile("=1: a|?: b").is_err());
}
