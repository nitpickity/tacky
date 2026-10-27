use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::parser::{parse_ty, Field, Label, PbType};

/// `Debug` for a message's `{name}Fields` iterator: walks a copy of the cursor and prints
/// fields in wire order. Consecutive occurrences of a repeated, packed or map field are
/// grouped into one `[..]` / `{..}` entry; anything else prints once per occurrence.
pub fn field_debug(name: &str, fields: &[Field]) -> TokenStream {
    let enum_name = format_ident!("{name}Field");
    let fields_iterator_name = format_ident!("{name}Fields");

    let arms: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let variant_name = format_ident!("{}", heck::AsUpperCamelCase(&f.name).to_string());
            let field_name_str = &f.name;

            // How a run of this field is printed: the collection builder and the per-value body.
            let (builder, entry) = match (&f.ty, &f.label) {
                (PbType::Map(_, m), _) => {
                    let PbType::Message(msg_name) = &**m else {
                        panic!("Map value type must be a message");
                    };
                    let fields_ty = parse_ty(&format!("{}Fields", msg_name));
                    (
                        quote!(debug_map),
                        quote! {
                            let (k, v) = v;
                            l.entry(&k, &v.unwrap_or(<#fields_ty>::new(&[])));
                        },
                    )
                }
                (PbType::SimpleMap(_, _), _) => (
                    quote!(debug_map),
                    quote! {
                        let (k, v) = v;
                        l.entry(&k, &v.unwrap_or_default());
                    },
                ),
                (_, Label::Packed) => (
                    quote!(debug_list),
                    quote! {
                        for x in v {
                            match x {
                                Ok(x) => l.entry(&x),
                                Err(e) => l.entry(&e),
                            };
                        }
                    },
                ),
                (_, Label::Repeated) => (quote!(debug_list), quote!(l.entry(&v);)),
                _ => {
                    return quote! {
                        Ok(#enum_name::#variant_name(v)) => {
                            s.field(#field_name_str, &v);
                        }
                    };
                }
            };

            quote! {
                Ok(#enum_name::#variant_name(_)) => {
                    let mut n = 1usize;
                    let mut peek = it;
                    while let Some(Ok(#enum_name::#variant_name(_))) = peek.next() {
                        n += 1;
                        it = peek;
                    }
                    s.field(
                        #field_name_str,
                        &core::fmt::from_fn(move |f| {
                            let mut l = f.#builder();
                            for x in start.take(n) {
                                if let Ok(#enum_name::#variant_name(v)) = x {
                                    #entry
                                }
                            }
                            l.finish()
                        }),
                    );
                }
            }
        })
        .collect();

    quote! {
        impl core::fmt::Debug for #fields_iterator_name<'_> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut s = f.debug_struct(#name);
                let mut it = *self;
                loop {
                    #[allow(unused_variables)]
                    let start = it;
                    let Some(item) = it.next() else { break };
                    match item {
                        #(#arms)*
                        Err(e) => {
                            s.field("error", &e);
                            break;
                        }
                    }
                }
                s.finish()
            }
        }
    }
}
