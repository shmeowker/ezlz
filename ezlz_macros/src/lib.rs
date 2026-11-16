//! Procedural macros for `ezlz` crate.

use proc_macro::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, Ident, Result, Token, parse_macro_input};

/// Template parameters: `<locale>, <key>[, arguments...]`.
struct Translation {
    locale: Expr,
    key: String,
    args: Punctuated<Argument, Token![,]>,
}

/// A single `name` or `name = expression` argument.
struct Argument {
    name: String,
    value: Expr,
}

/// Returns the identifier if `expr` is just a bare identifier like `name`.
fn bare_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => path.path.get_ident().map(|ident| ident.unraw().to_string()),
        _ => None,
    }
}

impl Parse for Argument {
    fn parse(input: ParseStream) -> Result<Self> {
        let expr: Expr = input.parse()?;

        match expr {
            // Explicit: `name = expression`
            Expr::Assign(assign) => match bare_ident(&assign.left) {
                Some(name) => Ok(Self {
                    name,
                    value: *assign.right,
                }),
                None => Err(syn::Error::new_spanned(&assign.left, "expected a placeholder identifier")),
            },

            // Shorthand: `name` becomes `name = name`.
            // Other expressions without a name are deliberately rejected.
            expr => match bare_ident(&expr) {
                Some(name) => Ok(Self { name, value: expr }),
                None => Err(syn::Error::new_spanned(
                    &expr,
                    "expected a placeholder identifier or `name = expression`",
                )),
            },
        }
    }
}

impl Parse for Translation {
    fn parse(input: ParseStream) -> Result<Self> {
        let locale: Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        // Dotted key. `parse_any` also accepts Rust keywords like `type`.
        let mut key = Ident::parse_any(input)?.unraw().to_string();
        while input.parse::<Option<Token![.]>>()?.is_some() {
            key.push('.');
            key.push_str(&Ident::parse_any(input)?.unraw().to_string());
        }

        // Optional `, arg, arg = expr`, with an optional trailing comma.
        let args = if input.parse::<Option<Token![,]>>()?.is_some() {
            Punctuated::parse_terminated(input)?
        } else {
            Punctuated::new()
        };

        Ok(Self { locale, key, args })
    }
}

/// The body of `wt!` macro.
struct WriteTranslation {
    output: Expr,
    translation: Translation,
}

impl Parse for WriteTranslation {
    fn parse(input: ParseStream) -> Result<Self> {
        let output: Expr = input.parse()?;
        input.parse::<Token![,]>()?;
        Ok(Self {
            output,
            translation: input.parse()?,
        })
    }
}

/// Turns the parsed arguments into `("name", Arg)` tuple expressions.
fn arguments(args: &Punctuated<Argument, Token![,]>) -> impl Iterator<Item = proc_macro2::TokenStream> + '_ {
    args.iter()
        .map(|Argument { name, value }| quote! { (#name, ::ezlz::__arg(&(#value))) })
}

/// Generates a translation getter with provided parameters.
///
/// Usage:
/// ```text
/// t!(<locale>, <key>[, arguments...]);
/// ```
///
/// Examples:
/// ```rust ignore
/// t!("ex", foo.corge);
/// t!(lang, foo.bar.baz, qux);
/// t!(get_lang(), foo.pets, birds = birds(), cats, dogs = 0);
/// ```
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    let Translation { locale, key, args } = parse_macro_input!(input as Translation);

    let arguments = arguments(&args);

    quote! {
        ::ezlz::__get(&(#locale), #key, &[#(#arguments),*])
    }
    .into()
}

/// Same as `t!`, but renders into a buffer instead of
/// allocating a new `String`.
///
/// Usage:
/// ```text
/// wt!(<output>, <locale>, <key>[, arguments...]);
/// ```
///
/// Examples:
/// ```rust ignore
/// let mut buf = String::new();
/// wt!(&mut buf, "en", foo.bar);
/// wt!(&mut buf, "cn", qux.corge);
/// ```
#[proc_macro]
pub fn wt(input: TokenStream) -> TokenStream {
    let WriteTranslation {
        output,
        translation: Translation { locale, key, args },
    } = parse_macro_input!(input as WriteTranslation);
    let arguments = arguments(&args);

    quote! { ::ezlz::__write(#output, &(#locale), #key, &[#(#arguments),*]) }.into()
}
