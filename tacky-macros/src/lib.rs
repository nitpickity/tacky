use quote::{format_ident, quote};
use syn::{
    braced, parse::Parse, parse_macro_input, punctuated::Punctuated, Expr, Ident, Path, Token,
};

#[derive(Debug)]
struct Input {
    exhaustive: bool,
    writer: Ident,
    schema: Path,
    fields: Punctuated<WriteExpr, Token![,]>,
}

#[derive(Debug)]
enum WriteExpr {
    Witness(Ident),
    Scope(Ident),       //foo, assumes value in scope, writer.write(foo)
    Write(Ident, Expr), //foo: 42, implies -> foo: writer.write_foo(42)
    With(Ident, Expr),  // foo: with { expr} -> foo: { raw expr }
    Fmt(Ident, Expr),
}

mod kw {
    syn::custom_keyword!(exhaustive);
    syn::custom_keyword!(with);
    syn::custom_keyword!(witness);
    syn::custom_keyword!(fmt);
}
impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let writer = input.parse::<Ident>()?;
        let _comma: Token![,] = input.parse()?;
        let exhaustive_token: Option<kw::exhaustive> = input.parse()?;
        let schema = input.parse::<Path>()?;
        let content;
        let _brace_token = braced!(content in input);
        let fields = content.parse_terminated(WriteExpr::parse, Token![,])?;
        Ok(Input {
            writer,
            schema,
            fields,
            exhaustive: exhaustive_token.is_some(),
        })
    }
}
impl Parse for WriteExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let wit: Option<kw::witness> = input.parse()?;
        let name = input.parse::<Ident>()?;
        let colon_token: Option<Token![:]> = input.parse()?;
        let format: Option<kw::fmt> = input.parse()?;
        let with_token: Option<kw::with> = input.parse()?;
        let out = if colon_token.is_some() {
            let expr = input.parse::<Expr>()?;
            if with_token.is_some() {
                Self::With(name, expr)
            } else if format.is_some() {
                Self::Fmt(name, expr)
            } else {
                Self::Write(name, expr)
            }
        } else if wit.is_some() {
            Self::Witness(name)
        } else {
            Self::Scope(name)
        };

        Ok(out)
    }
}
#[proc_macro]
pub fn write_proto(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let content = parse_macro_input!(input as Input);
    let Input {
        writer,
        schema,
        fields,
        exhaustive,
    } = content;
    let fields = fields.iter().map(|f| match f {
        WriteExpr::Fmt(s, w) => {
            let ss = format_ident!("{s}_writer");
            quote! {
                #s: #writer.#ss().write_fmt((#w), false)
            }
        }
        WriteExpr::Witness(s) => quote! {
            #s
        },
        WriteExpr::Scope(s) => quote! {
            #s: #writer.#s(#s)
        },
        WriteExpr::Write(s, w) => quote! {
            #s: #writer.#s(#w)
        },
        WriteExpr::With(s, w) => quote! {
            #s: #w
        },
    });
    let fields = quote! {
        #( #fields ),*
    };
    let q = if exhaustive {
        quote!(
            #schema {
                #fields
            }
        )
    } else {
        quote!(
            #schema {
                #fields,
                ..Default::default()
            }
        )
    };
    println!("{q}");
    q.into()
}
