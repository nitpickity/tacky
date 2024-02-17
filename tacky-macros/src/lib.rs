use quote::quote;
use syn::{
    braced, parse::Parse, parse_macro_input, punctuated::Punctuated, token::Colon, DeriveInput,
    Expr, ExprPath, Ident, MetaNameValue, Path, Token,
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
    Scope(Ident),       //foo, assumes value in scope, writer.write(foo)
    Write(Ident, Expr), //foo: 42, implies -> foo: writer.write_foo(42)
    With(Ident, Expr),  // foo: with { expr} -> foo: { raw expr }
}

mod kw {
    syn::custom_keyword!(exhaustive);
    syn::custom_keyword!(with);
}
impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let writer = input.parse::<Ident>()?;
        let _comma: Token![,] = input.parse()?;
        let exhaustive_token: Option<kw::with> = input.parse()?;
        let schema = input.parse::<Path>()?;
        let content;
        let _brace_token = braced!(content in input);
        let fields = content.parse_terminated(WriteExpr::parse, Token![,])?;
        Ok(Input {
            writer,
            schema,
            fields,
            exhaustive: exhaustive_token.is_some()
        })
    }
}
impl Parse for WriteExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse::<Ident>()?;
        let colon_token: Option<Token![:]> = input.parse()?;
        let with_token: Option<kw::with> = input.parse()?;
        let out = if colon_token.is_some() {
            let expr = input.parse::<Expr>()?;
            if with_token.is_some() {
                Self::With(name, expr)
            } else {
                Self::Write(name, expr)
            }
        } else {
            Self::Scope(name)
        };

        Ok(out)
    }
}
#[proc_macro]
pub fn mk_me(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let content = parse_macro_input!(input as Input);
    let Input {
        writer,
        schema,
        fields,
        exhaustive,
    } = content;
    let fields = fields.iter().map(|f| match f {
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
    let q = if exhaustive {
        quote!(
            #schema {
                #( #fields ),*
            };
        );
    } else {
        quote!(
            
            #schema {
                #( #fields ),*
                ..#schema::values()
            };
        );
    };
    println!("{q}");
    q.into()
}