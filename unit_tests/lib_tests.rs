use super::*;

fn render(template: &str, args: &[(&str, Arg)]) -> String {
    let mut buf = String::new();
    Template::compile(template).unwrap().render_to(&mut buf, args);

    buf
}

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
fn stray_closing_brace_is_text() {
    assert_eq!(render("a}b", &[]), "a}b");
    assert_eq!(render("}{x}}", &[("x", Arg::Int(1))]), "}1}");
}

#[test]
fn escapes() {
    let x = [("x", Arg::String("v"))];
    // An odd number of backslashes escapes the brace and removes one backslash.
    assert_eq!(render(r"\{x}", &x), "{x}");
    assert_eq!(render(r"a \{x} b", &x), "a {x} b");
    assert_eq!(render(r"\\\{x}", &x), r"\\{x}");
    // An even number leaves the placeholder active and keeps every backslash.
    assert_eq!(render(r"\\{x}", &x), r"\\v");
    assert_eq!(render(r"\\\\{x}", &x), r"\\\\v");
    // Backslashes elsewhere are plain text.
    assert_eq!(render(r"a\b", &x), r"a\b");
    assert_eq!(render(r"\", &x), r"\");
    assert_eq!(render(r"{x}\", &x), r"v\");
    assert_eq!(render(r"{x}\{x}", &x), r"v{x}");
    assert_eq!(render(r"\{\{", &x), "{{");
    assert_eq!(render(r"é\{x}", &x), "é{x}");
}

#[test]
fn compile_errors() {
    for (template, message) in [
        ("{x", "unclosed placeholder"),
        ("abc {", "unclosed placeholder"),
        ("{", "unclosed placeholder"),
        ("{}", "invalid plural placeholder identifier"),
        ("{ x }", "invalid plural placeholder identifier"),
        ("{n|?: x}", "invalid plural rule selector operator"),
        ("{n|=5-1: x}", "selector range lower bound is greater than its upper bound"),
    ] {
        assert_eq!(Template::compile(template).unwrap_err(), message, "{template:?}");
    }
    // A closing brace can never be escaped: the first `}` always ends the placeholder.
    assert_eq!(
        Template::compile(r"{x\}}").unwrap_err(),
        "invalid plural placeholder identifier"
    );
}

#[test]
fn buffer_size_estimate() {
    assert_eq!(Template::compile("").unwrap().bufsize, 0);
    assert_eq!(Template::compile("abc").unwrap().bufsize, 3);
    assert_eq!(Template::compile("abc{x}").unwrap().bufsize, 3 + Template::ESTIMATED_ARG_LEN);
    assert_eq!(
        Template::compile("{x}{n|=1: a| b}").unwrap().bufsize,
        2 * Template::ESTIMATED_ARG_LEN
    );
}

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
