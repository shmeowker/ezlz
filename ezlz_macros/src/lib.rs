use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
  parse::{Parse, ParseStream},
  parse_macro_input,
  Expr, Ident, Result, Token,
};

#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as Translation);

  let locale = input.locale;
  let key = input.key.to_string();

  let args = input.args.into_iter().map(|arg| {
    let name = arg.name;
    let value = arg.value;

    quote! {
      (#name, &#value as &dyn ::std::fmt::Display)
    }
  });


		let crate_name = proc_macro_crate::crate_name("ezlz").unwrap();
	
		let crate_ident = match crate_name {
		  proc_macro_crate::FoundCrate::Itself => quote!(crate),
		  proc_macro_crate::FoundCrate::Name(name) => {
		    let ident = syn::Ident::new(&name, Span::call_site());
		    quote!(::#ident)
		  }
		};

  quote! {
    #crate_ident::__get(
      #locale,
      #key,
      &[
      #(
        #args
      ),*
    ],
    )
  }
  .into()
}


struct Translation {
  locale: Expr,
  key: TranslationKey,
  args: Vec<Placeholder>,
}

impl Parse for Translation {
  fn parse(input: ParseStream<'_>) -> Result<Self> {
    let locale: Expr = input.parse()?;

    input.parse::<Token![,]>()?;

    let key: TranslationKey = input.parse()?;

    let mut args = Vec::new();

    while !input.is_empty() {
      input.parse::<Token![,]>()?;

      args.push(input.parse()?);
    }

    Ok(Self {
      locale,
      key,
      args,
    })
  }
}


struct TranslationKey(Vec<Ident>);

impl Parse for TranslationKey {
  fn parse(input: ParseStream<'_>) -> Result<Self> {
    let mut segments = Vec::new();

    segments.push(input.parse::<Ident>()?);

    while input.peek(Token![.]) {
      input.parse::<Token![.]>()?;
      segments.push(input.parse::<Ident>()?);
    }

    Ok(Self(segments))
  }
}

impl TranslationKey {
  fn to_string(&self) -> String {
    self.0
      .iter()
      .map(Ident::to_string)
      .collect::<Vec<_>>()
      .join(".")
  }
}


struct Placeholder {
  name: syn::LitStr,
  value: Expr,
}

impl Parse for Placeholder {
  fn parse(input: ParseStream<'_>) -> Result<Self> {
    if input.peek(Ident) {
      let ident: Ident = input.parse()?;

      if input.peek(Token![=]) {
        input.parse::<Token![=]>()?;

        let value: Expr = input.parse()?;

        return Ok(Self {
          name: syn::LitStr::new(
            &ident.to_string(),
            ident.span(),
          ),
          value,
        });
      }

      let value = Expr::Path(syn::ExprPath {
        attrs: Vec::new(),
        qself: None,
        path: ident.clone().into(),
      });

      return Ok(Self {
        name: syn::LitStr::new(
          &ident.to_string(),
          ident.span(),
        ),
        value,
      });
    }

    Err(input.error(
      "expected a placeholder identifier or `name = expression`",
    ))
  }
}