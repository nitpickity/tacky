use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::parser::{Field, Label, PbType, Scalar};

/// Whether a field's variant borrows from the input buffer (needs lifetime 'a).
fn field_borrows(field: &Field) -> bool {
    match field.label {
        Label::Packed => true, // packed → &'a [u8]
        _ => match &field.ty {
            PbType::Scalar(Scalar::String) | PbType::Scalar(Scalar::Bytes) => true,
            PbType::Message(_) | PbType::Map(_, _) | PbType::SimpleMap(_, _) => true,
            _ => false,
        },
    }
}

/// The Rust type carried by this field's enum variant.
fn variant_type(field: &Field) -> TokenStream {
    match field.label {
        Label::Packed => quote!(&'a [u8]),
        _ => match &field.ty {
            PbType::Scalar(s) => scalar_variant_type(s),
            PbType::Enum(_) => quote!(i32),
            PbType::Message(_) => quote!(&'a [u8]),
            PbType::Map(_, _) | PbType::SimpleMap(_, _) => quote!(&'a [u8]),
        },
    }
}

fn scalar_variant_type(s: &Scalar) -> TokenStream {
    match s {
        Scalar::Int32 => quote!(i32),
        Scalar::Sint32 => quote!(i32),
        Scalar::Int64 => quote!(i64),
        Scalar::Sint64 => quote!(i64),
        Scalar::Uint32 => quote!(u32),
        Scalar::Uint64 => quote!(u64),
        Scalar::Bool => quote!(bool),
        Scalar::Fixed32 => quote!(u32),
        Scalar::Sfixed32 => quote!(i32),
        Scalar::Float => quote!(f32),
        Scalar::Fixed64 => quote!(u64),
        Scalar::Sfixed64 => quote!(i64),
        Scalar::Double => quote!(f64),
        Scalar::String => quote!(&'a str),
        Scalar::Bytes => quote!(&'a [u8]),
    }
}

/// The WireType constant for this field.
fn wire_type_token(field: &Field) -> TokenStream {
    match field.label {
        Label::Packed => quote!(tacky::WireType::LEN),
        _ => match &field.ty {
            PbType::Scalar(s) => scalar_wire_type_token(s),
            PbType::Enum(_) => quote!(tacky::WireType::VARINT),
            PbType::Message(_) => quote!(tacky::WireType::LEN),
            PbType::Map(_, _) | PbType::SimpleMap(_, _) => quote!(tacky::WireType::LEN),
        },
    }
}

fn scalar_wire_type_token(s: &Scalar) -> TokenStream {
    match s {
        Scalar::Int32 | Scalar::Sint32 | Scalar::Int64 | Scalar::Sint64 | Scalar::Uint32
        | Scalar::Uint64 | Scalar::Bool => quote!(tacky::WireType::VARINT),
        Scalar::Fixed32 | Scalar::Sfixed32 | Scalar::Float => quote!(tacky::WireType::I32),
        Scalar::Fixed64 | Scalar::Sfixed64 | Scalar::Double => quote!(tacky::WireType::I64),
        Scalar::String | Scalar::Bytes => quote!(tacky::WireType::LEN),
    }
}

/// The decode expression for a field, assuming wire type has already been checked.
fn decode_expr(field: &Field) -> TokenStream {
    match field.label {
        Label::Packed => {
            // All packed fields just return raw bytes for user to iterate
            quote! {
                let data = tacky::decode_len(buf)?;
            }
        }
        _ => match &field.ty {
            PbType::Scalar(s) => scalar_decode_expr(s),
            PbType::Enum(_) => quote! {
                let val = tacky::decode_varint(buf)? as i32;
            },
            PbType::Message(_) => quote! {
                let data = tacky::decode_len(buf)?;
            },
            PbType::Map(_, _) | PbType::SimpleMap(_, _) => quote! {
                let data = tacky::decode_len(buf)?;
            },
        },
    }
}

fn scalar_decode_expr(s: &Scalar) -> TokenStream {
    match s {
        Scalar::Int32 => quote! { let val = tacky::decode_varint(buf)? as i32; },
        Scalar::Int64 => quote! { let val = tacky::decode_varint(buf)? as i64; },
        Scalar::Uint32 => quote! { let val = tacky::decode_varint(buf)? as u32; },
        Scalar::Uint64 => quote! { let val = tacky::decode_varint(buf)?; },
        Scalar::Sint32 => quote! {
            let val = tacky::decode_zigzag32(tacky::decode_varint(buf)? as u32);
        },
        Scalar::Sint64 => quote! {
            let val = tacky::decode_zigzag64(tacky::decode_varint(buf)?);
        },
        Scalar::Bool => quote! { let val = tacky::decode_varint(buf)? != 0; },
        Scalar::Fixed32 => quote! { let val = tacky::decode_u32(buf)?; },
        Scalar::Sfixed32 => quote! { let val = tacky::decode_i32(buf)?; },
        Scalar::Float => quote! { let val = tacky::decode_f32(buf)?; },
        Scalar::Fixed64 => quote! { let val = tacky::decode_u64(buf)?; },
        Scalar::Sfixed64 => quote! { let val = tacky::decode_i64(buf)?; },
        Scalar::Double => quote! { let val = tacky::decode_f64(buf)?; },
        Scalar::String => quote! {
            let data = tacky::decode_len(buf)?;
            let val = core::str::from_utf8(data)?;
        },
        Scalar::Bytes => quote! {
            let data = tacky::decode_len(buf)?;
        },
    }
}

/// The value expression to wrap in `Some(Self::Variant(...))`.
fn variant_value_expr(field: &Field) -> TokenStream {
    match field.label {
        Label::Packed => quote!(data),
        _ => match &field.ty {
            PbType::Scalar(Scalar::String) => quote!(val),
            PbType::Scalar(Scalar::Bytes) => quote!(data),
            PbType::Scalar(_) | PbType::Enum(_) => quote!(val),
            PbType::Message(_) | PbType::Map(_, _) | PbType::SimpleMap(_, _) => quote!(data),
        },
    }
}

pub fn write_field_enum(name: &str, fields: &[Field]) -> TokenStream {
    let enum_name = format_ident!("{name}Field");

    let needs_lifetime = fields.iter().any(field_borrows);

    // Generate variant definitions
    let variants: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let variant_name =
                format_ident!("{}", heck::AsUpperCamelCase(&f.name).to_string());
            let ty = variant_type(f);
            quote! { #variant_name(#ty) }
        })
        .collect();

    // Generate match arms for decode
    let match_arms: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let tag = f.number as u32;
            let variant_name =
                format_ident!("{}", heck::AsUpperCamelCase(&f.name).to_string());
            let field_name_str = &f.name;
            let wt = wire_type_token(f);
            let decode = decode_expr(f);
            let value = variant_value_expr(f);

            quote! {
                #tag => {
                    tacky::check_wire_type(wire_type, #wt, #field_name_str)?;
                    #decode
                    Ok(Some(Self::#variant_name(#value)))
                }
            }
        })
        .collect();

    if needs_lifetime {
        quote! {
            #[derive(Debug, Copy, Clone, PartialEq)]
            pub enum #enum_name<'a> {
                #(#variants,)*
            }

            impl<'a> #enum_name<'a> {
                /// Decode the next field from the buffer.
                /// Returns `Ok(Some(field))` for known fields, `Ok(None)` for unknown (skipped).
                pub fn decode(buf: &mut &'a [u8]) -> Result<Option<Self>, tacky::DecodeError> {
                    let (tag, wire_type) = tacky::decode_key(buf)?;
                    match tag {
                        #(#match_arms)*
                        _ => {
                            tacky::skip_field(wire_type, buf)?;
                            Ok(None)
                        }
                    }
                }
            }
        }
    } else {
        quote! {
            #[derive(Debug, Copy, Clone, PartialEq)]
            pub enum #enum_name {
                #(#variants,)*
            }

            impl #enum_name {
                /// Decode the next field from the buffer.
                /// Returns `Ok(Some(field))` for known fields, `Ok(None)` for unknown (skipped).
                pub fn decode(buf: &mut &[u8]) -> Result<Option<Self>, tacky::DecodeError> {
                    let (tag, wire_type) = tacky::decode_key(buf)?;
                    match tag {
                        #(#match_arms)*
                        _ => {
                            tacky::skip_field(wire_type, buf)?;
                            Ok(None)
                        }
                    }
                }
            }
        }
    }
}
