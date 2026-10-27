use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::parser::{parse_ty, Field, Label, OneOfGroup, PbType};

/// `tacky::json::ToJson` for a message's `{name}Fields` iterator, built in one decode pass.
/// See `tacky::json` for the shape of the output.
///
/// The work happens in `merge_json`, which decodes into an existing object. That is how a
/// singular message field seen twice gets protobuf's merge: the second instance decodes into the
/// first one's object, so its own fields follow the same rules recursively.
pub fn field_json(name: &str, fields: &[Field], oneofs: &[OneOfGroup]) -> TokenStream {
    let enum_name = format_ident!("{name}Field");
    let fields_iterator_name = format_ident!("{name}Fields");

    let arms: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let variant_name = format_ident!("{}", heck::AsUpperCamelCase(&f.name).to_string());
            let field_name_str = &f.name;

            let body = match (&f.ty, &f.label) {
                (PbType::Map(_, m), _) => {
                    let PbType::Message(msg_name) = &**m else {
                        panic!("Map value type must be a message");
                    };
                    let fields_ty = parse_ty(&format!("{}Fields", msg_name));
                    quote! {
                        let (k, v) = v;
                        let v = v.unwrap_or(<#fields_ty>::new(&[]));
                        tacky::json::insert_entry(m, #field_name_str, k, v.to_json()?);
                    }
                }
                (PbType::SimpleMap(_, _), _) => quote! {
                    let (k, v) = v;
                    let v = v.unwrap_or_default();
                    tacky::json::insert_entry(m, #field_name_str, k, v.to_json()?);
                },
                (_, Label::Packed) => quote! {
                    tacky::json::push_packed(m, #field_name_str, v)?;
                },
                (_, Label::Repeated) => quote! {
                    tacky::json::push(m, #field_name_str, v.to_json()?);
                },
                (PbType::Message(_), _) => quote! {
                    match m.get_mut(#field_name_str) {
                        Some(tacky::json::serde_json::Value::Object(o)) => v.merge_json(o)?,
                        _ => {
                            m.insert(#field_name_str.into(), v.to_json()?);
                        }
                    }
                },
                _ => quote! {
                    m.insert(#field_name_str.into(), v.to_json()?);
                },
            };

            // Setting a oneof member clears the rest of its group.
            let siblings: Vec<&str> = oneofs
                .iter()
                .find(|o| o.fields.iter().any(|g| g.name == f.name))
                .map(|o| {
                    o.fields
                        .iter()
                        .filter(|g| g.name != f.name)
                        .map(|g| g.name.as_str())
                        .collect()
                })
                .unwrap_or_default();

            quote! {
                #enum_name::#variant_name(v) => {
                    #(m.shift_remove(#siblings);)*
                    #body
                }
            }
        })
        .collect();

    quote! {
        tacky::if_json! {
            impl #fields_iterator_name<'_> {
                #[doc(hidden)]
                #[allow(unused_variables)]
                pub fn merge_json(
                    &self,
                    m: &mut tacky::json::serde_json::Map<String, tacky::json::serde_json::Value>,
                ) -> Result<(), tacky::DecodeError> {
                    use tacky::json::ToJson as _;
                    for item in *self {
                        match item? {
                            #(#arms)*
                        }
                    }
                    Ok(())
                }
            }

            impl tacky::json::ToJson for #fields_iterator_name<'_> {
                fn to_json(&self) -> Result<tacky::json::serde_json::Value, tacky::DecodeError> {
                    let mut m = tacky::json::serde_json::Map::new();
                    self.merge_json(&mut m)?;
                    Ok(tacky::json::serde_json::Value::Object(m))
                }
            }
        }
    }
}

/// `tacky::json::ToJson` for a generated enum: the proto value name, or the raw number if
/// unrecognized.
pub fn enum_json(name: &str, values: &[(String, i32)]) -> TokenStream {
    let name_ident = format_ident!("{name}");
    let arms = values.iter().map(|(value, _number)| {
        let value_ident = format_ident!("{}", heck::AsUpperCamelCase(value).to_string());
        quote!(#name_ident::#value_ident => #value.into())
    });

    quote! {
        tacky::if_json! {
            impl tacky::json::ToJson for #name_ident {
                fn to_json(&self) -> Result<tacky::json::serde_json::Value, tacky::DecodeError> {
                    Ok(match self {
                        #(#arms,)*
                        #name_ident::__Unrecognized(n) => (*n).into(),
                    })
                }
            }
        }
    }
}
