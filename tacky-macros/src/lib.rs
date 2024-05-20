use quote::quote;
use syn::{
    braced, parse::Parse, parse_macro_input, punctuated::Punctuated, Expr, ExprCall, Ident, Path,
    Token,
};

#[derive(Debug)]
struct Input {
    writer: Ident,
    schema: Path,
    fields: Punctuated<WriteExpr, Token![,]>,
}

#[derive(Debug)]
enum WriteExpr {
    Scope(Ident),       // foo, assumes value in scope, writer.write(foo)
    Write(Ident, Expr), // foo: 42, implies -> foo: writer.write_foo(42)
    Block(Ident, Expr), // foo: with { expr} -> foo: { raw expr }
    Fmt(Ident, Expr),
}

impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let writer = input.parse::<Ident>()?;
        let _comma: Token![,] = input.parse()?;
        let schema = input.parse::<Path>()?;
        let content;
        let _brace_token = braced!(content in input);
        let fields = content.parse_terminated(WriteExpr::parse, Token![,])?;
        Ok(Input {
            writer,
            schema,
            fields,
        })
    }
}

impl Parse for WriteExpr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse::<Ident>()?;
        let colon_token: Option<Token![:]> = input.parse()?;
        if colon_token.is_none() {
            return Ok(Self::Scope(name));
        };
        let exp = input.parse::<Expr>()?;
        let ee = match &exp {
            e @ Expr::Call(ExprCall { func, args, .. }) => match &**func {
                Expr::Path(p) => {
                    if p.path.is_ident("write_fmt") {
                        let arg = args.first().unwrap();
                        Self::Fmt(name, arg.clone())
                    } else {
                        Self::Write(name, e.clone())
                    }
                }
                _ => Self::Write(name, e.clone()),
            },
            e @ Expr::Block(_) => Self::Block(name, e.clone()),
            _ => Self::Write(name, exp),
        };
        Ok(ee)
    }
}

#[proc_macro]
pub fn write_proto(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let content = parse_macro_input!(input as Input);
    let Input {
        writer,
        schema,
        fields,
    } = content;
    let fields = fields.iter().map(|f| match f {
        WriteExpr::Scope(s) => quote! {
            #s: #writer.#s().write(#s)
        },
        WriteExpr::Write(s, w) => quote! {
            #s: #writer.#s().write(#w)
        },
        WriteExpr::Fmt(s, w) => {
            quote! {
                #s: #writer.#s().write_fmt((#w), false)
            }
        }
        WriteExpr::Block(s, w) => {
            quote! {
                #s: #w
            }
        }
    });
    let q = quote!(
        #schema {
            #( #fields ),*
        }
    );
    // println!("{q}");
    q.into()
}
