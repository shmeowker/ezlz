use ezlz::t;

#[test]
fn translations_work() {
  ezlz::init("en", "tests/locales").unwrap();

  // Basic literal locale lookup.
  assert_eq!(
    t!("en", messages.hello),
    "Hello, world!"
  );

  // Dynamic locale.
  let lang = "de";

  assert_eq!(
    t!(lang, messages.hello),
    "Hallo Welt!"
  );

  // Bare identifier interpolation:
  //
  // `name` means `%{name}`.
  let name = "Anna";

  assert_eq!(
    t!("en", messages.greet, name),
    "Hello, Anna!"
  );

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

  assert_eq!(
    t!(
      "en",
      messages.greet,
      name = user.name
    ),
    "Hello, Anna!"
  );

  // Multiple bare identifier arguments.
  let name = "Anna";
  let count = 42;

  assert_eq!(
    t!(
      "en",
      messages.count,
      name,
      count
    ),
    "Anna has 42 messages."
  );

  // Regional locale.
  assert_eq!(
    t!("en_GB", messages.hello),
    "Hello, world!"
  );

  // `en_GB` doesn't contain `errors.not_found`, so this
  // should fall back to the base `en` locale.
  assert_eq!(
    t!("en_GB", errors.not_found),
    "Not found"
  );

  // Nested YAML keys.
  assert_eq!(
    t!("en", nested.deeply.nested.value),
    "Deep value"
  );

  // Nested keys from another locale.
  assert_eq!(
    t!("de", nested.deeply.nested.value),
    "Tiefer Wert"
  );

  // Dynamic locale + interpolation.
  let lang = "de";
  let name = "Anna";

  assert_eq!(
    t!(
      lang,
      messages.greet,
      name
    ),
    "Hallo, Anna!"
  );

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