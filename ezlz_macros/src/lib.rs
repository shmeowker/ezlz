use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, Ident, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};
extern crate self as ezlz;

struct Translation {
    locale: Expr,
    key: String,
    args: Vec<(String, Expr)>,
}

impl Parse for Translation {
    fn parse(input: ParseStream) -> Result<Self> {
        let locale: Expr = input.parse()?;

        input.parse::<Token![,]>()?;

        let mut key = String::new();

        loop {
            let ident: Ident = input.parse()?;

            if !key.is_empty() {
                key.push('.');
            }

            key.push_str(&ident.to_string());

            if !input.peek(Token![.]) {
                break;
            }

            input.parse::<Token![.]>()?;
        }

        let mut args = Vec::new();

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;

            let expr: Expr = input.parse()?;

            match &expr {
                /*
                 * Explicit:
                 *
                 * name = expression
                 *
                 * n = 42
                 */
                Expr::Assign(assign) => {
                    let Expr::Path(path) = &*assign.left else {
                        return Err(syn::Error::new_spanned(
                            &assign.left,
                            "expected a placeholder identifier",
                        ));
                    };

                    let Some(ident) = path.path.get_ident() else {
                        return Err(syn::Error::new_spanned(
                            &assign.left,
                            "expected a placeholder identifier",
                        ));
                    };

                    args.push((ident.to_string(), (*assign.right).clone()));
                }

                /*
                 * Bare identifier:
                 *
                 * name
                 *
                 * becomes:
                 *
                 * ("name", __display(&name))
                 */
                Expr::Path(path) => {
                    let Some(ident) = path.path.get_ident() else {
                        return Err(syn::Error::new_spanned(
                            &expr,
                            "expected a placeholder identifier or `name = expression`",
                        ));
                    };

                    args.push((ident.to_string(), expr));
                }

                /*
                 * Numeric/literal expressions without a name
                 * are deliberately rejected.
                 *
                 * The intended syntax is:
                 *
                 * n = 42
                 *
                 * rather than:
                 *
                 * 42
                 */
                _ => {
                    return Err(syn::Error::new_spanned(
                        &expr,
                        "expected a placeholder identifier or `name = expression`",
                    ));
                }
            }
        }

        Ok(Self { locale, key, args })
    }
}

#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    let Translation { locale, key, args } = parse_macro_input!(input as Translation);

    let arguments = args.iter().map(|(name, expr)| {
        if name == "n" {
            quote! {
                (
                    #name,
                    ::ezlz::__number(
                        &(#expr)
                    )
                )
            }
        } else {
            quote! {
                (
                    #name,
                    ::ezlz::__display(
                        &(#expr)
                    )
                )
            }
        }
    });

    quote! {
        ::ezlz::__get(
            &#locale,
            #key,
            &[
                #(#arguments),*
            ],
        )
    }
    .into()
}
