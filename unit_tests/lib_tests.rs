use super::*;

fn render(template: &str, args: &[(&str, Arg)]) -> String {
    Template::try_from(template)
        .unwrap_or_else(|e| panic!("{template:?}: {e}"))
        .render(args)
}

fn error(template: &str) -> &'static str {
    Template::try_from(template).expect_err(template)
}

// Identifiers and text.

#[test]
fn identifiers() {
    for valid in ["x", "_", "_x", "x1", "X", "a_b_c", "1"] {
        assert!(is_identifier(valid), "{valid:?}");
    }
    for invalid in ["", " ", "a b", "a-b", "a.b", "é", "x|y", "{x}", "a\n"] {
        assert!(!is_identifier(invalid), "{invalid:?}");
    }
}

#[test]
fn text_and_placeholders() {
    assert_eq!(render("", &[]), "");
    assert_eq!(render("plain", &[]), "plain");
    assert_eq!(render("{x}", &[("x", Arg::Int(1))]), "1");
    assert_eq!(render("a{x}b{y}c", &[("x", Arg::Int(1)), ("y", Arg::String("z"))]), "a1bzc");
    assert_eq!(render("{x}{x}", &[("x", Arg::Uint(7))]), "77");
    assert_eq!(render("é{x}ü 你好{x}🎉", &[("x", Arg::Int(1))]), "é1ü 你好1🎉");
}

#[test]
fn leading_and_trailing_text_is_kept() {
    let x = [("x", Arg::String("v"))];
    assert_eq!(render("a{x}", &x), "av");
    assert_eq!(render("{x}b", &x), "vb");
    assert_eq!(render("a{x}b", &x), "avb");
    assert_eq!(render("{x}{x}z", &x), "vvz");
    assert_eq!(render("{n|=1: a}z", &[("n", Arg::Int(1))]), "1 az");
}

#[test]
fn stray_closing_brace_is_text() {
    assert_eq!(render("a}b", &[]), "a}b");
    assert_eq!(render("}{x}}", &[("x", Arg::Int(1))]), "}1}");
    assert_eq!(render("}}", &[]), "}}");
}

#[test]
fn missing_arguments_render_as_nothing() {
    assert_eq!(render("[{x}|{n|=1: a}]", &[]), "[|]");
    assert_eq!(render("[{x}]", &[("y", Arg::Int(1))]), "[]");
}

// Escaping.

#[test]
fn a_doubled_opening_brace_is_a_literal_brace() {
    let x = [("x", Arg::String("v"))];
    assert_eq!(render("{{x}", &x), "{x}");
    assert_eq!(render("a {{x} b", &x), "a {x} b");
    assert_eq!(render("{{", &x), "{");
    assert_eq!(render("{{{{", &x), "{{");
    assert_eq!(render("é{{x}", &x), "é{x}");
    // `}}` is not folded, a closing brace is never special in text.
    assert_eq!(render("{{}}", &x), "{}}");
}

#[test]
fn braces_are_resolved_from_left_to_right() {
    let x = [("x", Arg::String("v"))];
    // A run of `k` braces is `k / 2` literal braces and, if `k` is odd, a placeholder.
    assert_eq!(render("{{{x}", &x), "{v");
    assert_eq!(render("{{{{{x}", &x), "{{v");
    assert_eq!(render("{x}{{x}", &x), "v{x}");
    assert_eq!(render("{{x}{x}", &x), "{x}v");
    assert_eq!(render("{{{x}}", &x), "{v}");
}

#[test]
fn backslashes_are_plain_text() {
    let x = [("x", Arg::String("v"))];
    assert_eq!(render(r"\{x}", &x), r"\v");
    assert_eq!(render(r"\\{x}", &x), r"\\v");
    assert_eq!(render(r"\{{x}", &x), r"\{x}");
    assert_eq!(render(r"a\b\\", &x), r"a\b\\");
    assert_eq!(render(r"C:\dir\{x}\", &x), r"C:\dir\v\");
    assert_eq!(render(r"{x}\{x}", &x), r"v\v");
}

// Plural placeholders.

#[test]
fn plural_placeholders() {
    let n = |v: i64| [("n", Arg::Int(v))];
    assert_eq!(render("{n|=1: item| items}", &n(1)), "1 item");
    assert_eq!(render("{n|=1: item| items}", &n(5)), "5 items");
    assert_eq!(render("Got {n|=1: fox| foxes} today", &n(1)), "Got 1 fox today");
    assert_eq!(render("Got {n|=1: fox| foxes} today", &n(3)), "Got 3 foxes today");
    assert_eq!(render("{n|=0:=none|=1:=one| many}", &n(0)), "none");
    assert_eq!(render("{n|=0:=none|=1:=one| many}", &n(1)), "one");
    assert_eq!(render("{n|=0:=none|=1:=one| many}", &n(9)), "9 many");
    assert_eq!(
        render("{a|=1: x| y}-{b}", &[("a", Arg::Int(1)), ("b", Arg::String("B"))]),
        "1 x-B"
    );
    assert_eq!(render("{a|=1: x| y}{a|=1: x| y}", &[("a", Arg::Int(2))]), "2 y2 y");
}

#[test]
fn a_plural_placeholder_may_be_followed_by_a_closing_brace() {
    let n = |v: i64| [("n", Arg::Int(v))];
    assert_eq!(render("{n|=1: a}}", &n(1)), "1 a}");
    assert_eq!(render("{n|=1: a}}", &n(2)), "2}");
    assert_eq!(render("{n|=1: a}}}", &n(1)), "1 a}}");
}

#[test]
fn braces_in_quoted_rule_texts() {
    let n = |v: i64| [("n", Arg::Int(v))];
    assert_eq!(render("{n|=1: `{x}`|: `}`}", &n(1)), "1 {x}");
    assert_eq!(render("{n|=1: `{x}`|: `}`}", &n(2)), "2 }");
    assert_eq!(render("<{n|=1: `a}`}>", &n(1)), "<1 a}>");
    assert_eq!(render("{n|=1: `a | b`| c}", &n(1)), "1 a | b");
    assert_eq!(render("{n|=1: `a | b`| c}", &n(2)), "2 c");
    assert_eq!(render("{n|=1: x``y}", &n(1)), "1 x`y");
}

#[test]
fn plural_names_are_validated() {
    // The name is checked before the rules are parsed.
    for template in [
        "{a b|=1: x}",
        "{|=1: x}",
        "{ n|=1: x}",
        "{a-b|=1: x}",
        "{é|=1: x}",
        "{n{|=1: x}",
    ] {
        assert!(Template::try_from(template).is_err(), "{template:?}");
    }
    assert!(Template::try_from("{n_1|=1: x}").is_ok());
    assert!(Template::try_from("{1|=1: x}").is_ok());
}

// Errors.

#[test]
fn unquoted_opening_brace_in_rule_text_is_an_error() {
    const MSG: &str = "unquoted { in plural rule text";

    // Nested placeholders and stray braces, in a selector rule or a fallback.
    for template in [
        "{n|=1: {x}| y}",
        "{n|=1: a{b| y}",
        "{n|=1:={x}}",
        "{n|=1: x|{y}",
        "{n| {y}}",
        "{n|=1: x|=2: y{}",
        "{n|=1: {{x}}",
    ] {
        assert_eq!(error(template), MSG, "{template:?}");
    }

    // Quoted braces stay literal, including in the replace and fallback forms.
    for template in [
        "{n|=1: `{`}",
        "{n|=1: `{x}`| y}",
        "{n|=1:=`{x}`}",
        "{n| `{y}`}",
        "{n|=`{y}`}",
        "{n|=1: a `{` b}",
    ] {
        assert!(Template::try_from(template).is_ok(), "{template:?} should compile");
    }

    // A `{` outside any plural placeholder is not affected.
    assert!(Template::try_from("a {{ b {n|=1: x}").is_ok());
}

#[test]
fn unterminated_placeholders() {
    for template in ["{x", "abc {", "{", "a{x", "{x}{y", "{x} {"] {
        assert_eq!(error(template), "unterminated placeholder", "{template:?}");
    }
}

#[test]
fn unterminated_plural_placeholders() {
    for template in ["{n|", "{n|=1: x", "{n|=1: x|", "{n|=1: `x}", "{n|=1: x}{m|", "a {n|=1: `}"] {
        assert_eq!(error(template), "unterminated plural placeholder", "{template:?}");
    }
}

#[test]
fn invalid_identifiers() {
    for template in ["{}", "{ x }", "{x y}", "{x{y}", "{x-y}", "{é}", "a {} b"] {
        assert_eq!(error(template), "invalid placeholder identifier", "{template:?}");
    }
    // A closing brace can't be escaped: the first `}` always ends a variable placeholder.
    assert_eq!(error(r"{x\}}"), "invalid placeholder identifier");
}

#[test]
fn rule_errors_are_passed_through() {
    assert_eq!(error("{n|?: x}"), "invalid plural rule selector operator");
    assert_eq!(
        error("{n|=5-1: x}"),
        "selector range lower bound is greater than its upper bound"
    );
    assert!(Template::try_from("{n|=1000: x}").is_err());
    assert!(Template::try_from("{n|=255: x}").is_err());
    assert!(Template::try_from("{n|=1: item| items: all}").is_err());
    // An error in a later placeholder is found as well.
    assert!(Template::try_from("{x} {n|?: x}").is_err());
}

// Structure and size.

#[test]
fn templates_have_no_empty_text_parts() {
    for (template, parts) in [
        ("", 0),
        ("{x}", 1),
        ("{x}{x}", 2),
        ("a{x}", 2),
        ("{x}a", 2),
        ("a{x}b", 3),
        ("{{", 1),
    ] {
        let compiled = Template::try_from(template).unwrap();
        assert_eq!(compiled.parts.len(), parts, "{template:?}: {:?}", compiled.parts);
        for part in compiled.parts.iter() {
            if let Part::Text(text) = part {
                assert!(!text.is_empty(), "{template:?}");
            }
        }
    }
}

#[test]
fn parts_are_compiled_in_order() {
    let compiled = Template::try_from("a{x}b{n|=1: c}d").unwrap();
    let parts = &*compiled.parts;
    assert_eq!(parts.len(), 5);
    assert!(matches!(&parts[0], Part::Text(t) if &**t == "a"));
    assert!(matches!(&parts[1], Part::Variable { name } if &**name == "x"));
    assert!(matches!(&parts[2], Part::Text(t) if &**t == "b"));
    assert!(matches!(&parts[3], Part::Plural { name, .. } if &**name == "n"));
    assert!(matches!(&parts[4], Part::Text(t) if &**t == "d"));
}

#[test]
fn buffer_size_estimate() {
    let size = |template: &str| Template::try_from(template).unwrap().bufsize;
    assert_eq!(size(""), 0);
    assert_eq!(size("abc"), 3);
    assert_eq!(size("é"), 2);
    assert_eq!(size("abc{x}"), 3 + Template::ESTIMATED_VAR_LEN);
    assert_eq!(size("{x}{n|=1: a| b}"), 2 * Template::ESTIMATED_VAR_LEN);
    assert_eq!(size("é{x}ü"), 4 + Template::ESTIMATED_VAR_LEN);
    // A doubled brace is one byte of text.
    assert_eq!(size("{{x}"), 3);
    assert_eq!(size("{{{x}"), 1 + Template::ESTIMATED_VAR_LEN);
}

// Arguments.

#[test]
fn abs_trunc() {
    assert_eq!(Arg::Int(-5).abs_trunc(), 5);
    assert_eq!(Arg::Int(i64::MIN).abs_trunc(), 1 << 63);
    assert_eq!(Arg::Uint(u64::MAX).abs_trunc(), u64::MAX);
    assert_eq!(Arg::Float64(-1.9).abs_trunc(), 1);
    assert_eq!(Arg::Float64(0.99).abs_trunc(), 0);
    assert_eq!(Arg::Float32(-2.5).abs_trunc(), 2);
    assert_eq!(Arg::Float64(f64::NAN).abs_trunc(), 0);
    assert_eq!(Arg::Float64(f64::INFINITY).abs_trunc(), u64::MAX);
    assert_eq!(Arg::Float32(f32::NEG_INFINITY).abs_trunc(), u64::MAX);
    assert_eq!(Arg::Float64(-0.0).abs_trunc(), 0);
}

#[test]
fn writing_values() {
    fn write(arg: Arg) -> String {
        let mut output = String::new();
        arg.write_to(&mut output);
        output
    }
    assert_eq!(write(Arg::Int(-7)), "-7");
    assert_eq!(write(Arg::Uint(7)), "7");
    assert_eq!(write(Arg::Float64(1.5)), "1.5");
    assert_eq!(write(Arg::Float32(1.1)), "1.1");
    assert_eq!(write(Arg::Float64(f64::from(1.1_f32))), "1.100000023841858");
    assert_eq!(write(Arg::String("é")), "é");
}

#[test]
fn to_arg_conversions() {
    assert_eq!(__arg(&-3_i8), Arg::Int(-3));
    assert_eq!(__arg(&3_u16), Arg::Uint(3));
    assert_eq!(__arg(&usize::MAX), Arg::Uint(usize::MAX as u64));
    assert_eq!(__arg(&1.5_f32), Arg::Float32(1.5));
    assert_eq!(__arg(&1.5_f64), Arg::Float64(1.5));
    assert_eq!(__arg(&"s"), Arg::String("s"));
    assert_eq!(__arg(&String::from("s")), Arg::String("s"));
}
