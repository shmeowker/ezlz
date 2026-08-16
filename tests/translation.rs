use ezlz::t;

#[test]
fn translations_work() {
    ezlz::init("en", "tests/locales").unwrap();

    // Pluralization / conditional interpolation tests.

    // {n| штук| штука|2+ штуки|5+ штук}
    assert_eq!(t!("en", plurals.russian, n = 0), "0 штук");
    assert_eq!(t!("en", plurals.russian, n = 1), "1 штука");
    assert_eq!(t!("en", plurals.russian, n = 2), "2 штуки");
    assert_eq!(t!("en", plurals.russian, n = 3), "3 штуки");
    assert_eq!(t!("en", plurals.russian, n = 4), "4 штуки");
    assert_eq!(t!("en", plurals.russian, n = 5), "5 штук");
    assert_eq!(t!("en", plurals.russian, n = 100), "100 штук");

    // {n|| штука|2+ штуки|5+ штук|10+ штук}
    assert_eq!(t!("en", plurals.items, n = 0), "0");
    assert_eq!(t!("en", plurals.items, n = 1), "1 штука");
    assert_eq!(t!("en", plurals.items, n = 2), "2 штуки");
    assert_eq!(t!("en", plurals.items, n = 4), "4 штуки");
    assert_eq!(t!("en", plurals.items, n = 5), "5 штук");
    assert_eq!(t!("en", plurals.items, n = 9), "9 штук");
    assert_eq!(t!("en", plurals.items, n = 10), "10 штук");
    assert_eq!(t!("en", plurals.items, n = 100), "100 штук");

    // {n| pieces| piece|2+ pieces}
    assert_eq!(t!("en", plurals.english, n = 0), "0 pieces");
    assert_eq!(t!("en", plurals.english, n = 1), "1 piece");
    assert_eq!(t!("en", plurals.english, n = 2), "2 pieces");
    assert_eq!(t!("en", plurals.english, n = 10), "10 pieces");

    // {n||1+ item(s)}
    assert_eq!(t!("en", plurals.item, n = 0), "0");
    assert_eq!(t!("en", plurals.item, n = 1), "1 item(s)");
    assert_eq!(t!("en", plurals.item, n = 2), "2 item(s)");
    assert_eq!(t!("en", plurals.item, n = 10), "10 item(s)");

    // {n|| singular}
    assert_eq!(t!("en", plurals.singular, n = 0), "0");
    assert_eq!(t!("en", plurals.singular, n = 1), "1 singular");
    assert_eq!(t!("en", plurals.singular, n = 2), "2");

    // {n|.0000}
    assert_eq!(t!("en", plurals.decimal, n = 0), "0.0000");
    assert_eq!(t!("en", plurals.decimal, n = 1), "1");
    assert_eq!(t!("en", plurals.decimal, n = 42), "42");

    // {n|=none|2+=some}
    assert_eq!(t!("en", plurals.some, n = 0), "none");
    assert_eq!(t!("en", plurals.some, n = 1), "1");
    assert_eq!(t!("en", plurals.some, n = 2), "some");
    assert_eq!(t!("en", plurals.some, n = 100), "some");

    // {n|=zero|=one|=two|3+more that two}
    assert_eq!(t!("en", plurals.exact, n = 0), "zero");
    assert_eq!(t!("en", plurals.exact, n = 1), "one");
    assert_eq!(t!("en", plurals.exact, n = 2), "two");
    assert_eq!(t!("en", plurals.exact, n = 3), "3more that two");
    assert_eq!(t!("en", plurals.exact, n = 100), "100more that two");

    // {n||+=not null}
    assert_eq!(t!("en", plurals.non_null, n = 0), "0");
    assert_eq!(t!("en", plurals.non_null, n = 1), "not null");
    assert_eq!(t!("en", plurals.non_null, n = 2), "not null");
    assert_eq!(t!("en", plurals.non_null, n = 100), "not null");

    // {+n||+ is }
    //
    // +n puts the selected text before the number.
    // + on the rule means index 1 and above.
    assert_eq!(t!("en", plurals.before, n = 0), "0");
    assert_eq!(t!("en", plurals.before, n = 1), " is 1");
    assert_eq!(t!("en", plurals.before, n = 2), " is 2");
    assert_eq!(t!("en", plurals.before, n = 100), " is 100");

    // Basic literal locale lookup.
    assert_eq!(t!("en", messages.hello), "Hello, world!");

    // Dynamic locale.
    let lang = "de";

    assert_eq!(t!(lang, messages.hello), "Hallo Welt!");

    // Bare identifier interpolation:
    //
    // `name` means `%{name}`.
    let name = "Anna";

    assert_eq!(t!("en", messages.greet, name), "Hello, Anna!");

    // Named expression interpolation.
    //
    // `name = expression` explicitly maps the expression
    // to the `%{name}` placeholder.
    struct User {
        name: String,
    }

    let user = User {
        name: "Anna".to_owned(),
    };

    assert_eq!(t!("en", messages.greet, name = user.name), "Hello, Anna!");

    // Multiple bare identifier arguments.
    let name = "Anna";
    let count = 42;

    assert_eq!(
        t!("en", messages.count, name, count),
        "Anna has 42 messages."
    );

    // Regional locale.
    assert_eq!(t!("en_GB", messages.hello), "Hello, world!");

    // `en_GB` doesn't contain `errors.not_found`, so this
    // should fall back to the base `en` locale.
    assert_eq!(t!("en_GB", errors.not_found), "Not found");

    // Nested YAML keys.
    assert_eq!(t!("en", nested.deeply.nested.value), "Deep value");

    // Nested keys from another locale.
    assert_eq!(t!("de", nested.deeply.nested.value), "Tiefer Wert");

    // Dynamic locale + interpolation.
    let lang = "de";
    let name = "Anna";

    assert_eq!(t!(lang, messages.greet, name), "Hallo, Anna!");

    // Explicit named arguments can contain arbitrary expressions.
    let user_name = String::from("Anna");
    let message_count = 42;

    assert_eq!(
        t!(
            "en",
            messages.count,
            name = user_name,
            count = message_count
        ),
        "Anna has 42 messages."
    );
}
