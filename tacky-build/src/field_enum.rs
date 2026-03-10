use core::panic;

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
        Label::Packed => packed_variant_type(field),
        _ => match &field.ty {
            PbType::Scalar(s) => scalar_variant_type(s),
            PbType::Enum((name, _)) => {
                let ident = format_ident!("{}", name);
                quote!(#ident)
            }
            PbType::Message(_) => quote!(&'a [u8]),
            PbType::Map(_, _) | PbType::SimpleMap(_, _) => quote!(&'a [u8]),
        },
    }
}

fn packed_variant_type(field: &Field) -> TokenStream {
    match &field.ty {
        PbType::Scalar(s) => {
            let t = s.tacky_type();
            let ty_ident = format_ident!("{t}");
            quote!(tacky::packed::PackedIter::<'a, #ty_ident>)
        }
        PbType::Enum(_) => quote!(tacky::packed::PackedIter::<'a, Int32>),
        _ => quote!(tacky::packed::PackedIter::<'a, Int32>),
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
        Scalar::Int32
        | Scalar::Sint32
        | Scalar::Int64
        | Scalar::Sint64
        | Scalar::Uint32
        | Scalar::Uint64
        | Scalar::Bool => quote!(tacky::WireType::VARINT),
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
            PbType::Enum((name, _)) => {
                let ident = format_ident!("{}", name);
                let field_name_str = &field.name;
                quote! {
                    let raw = <Int32 as tacky::ProtobufScalar>::read(buf)?; // enums are always varint-encoded
                    let val = #ident::try_from(raw).map_err(|_| tacky::DecodeError::InvalidEnumValue {
                        field: #field_name_str,
                        value: raw,
                    })?;
                }
            }
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
    let t = s.tacky_type();
    let ty_ident = format_ident!("{t}");
    quote! {
        let val = <#ty_ident as tacky::ProtobufScalar>::read(buf)?;
    }
}

/// The value expression to wrap in `Some(Self::Variant(...))`.
fn packed_value_expr(field: &Field) -> TokenStream {
    match &field.ty {
        PbType::Scalar(s) => {
            let t = s.tacky_type();
            let ty_ident = format_ident!("{t}");
            quote!(tacky::packed::PackedIter::<#ty_ident>::new(data))
        }
        PbType::Enum(_) => quote!(tacky::packed::PackedIter::<'a, Int32>::new(data)),
        _ => panic!("Only scalar and enum fields can be packed"),
    }
}

fn variant_value_expr(field: &Field) -> TokenStream {
    match field.label {
        Label::Packed => packed_value_expr(field),
        _ => match &field.ty {
            PbType::Scalar(Scalar::String) => quote!(val),
            PbType::Scalar(Scalar::Bytes) => quote!(val),
            PbType::Scalar(_) | PbType::Enum(_) => quote!(val),
            PbType::Message(_) | PbType::Map(_, _) | PbType::SimpleMap(_, _) => quote!(data),
        },
    }
}

pub fn field_enum(name: &str, fields: &[Field]) -> TokenStream {
    let enum_name = format_ident!("{name}Field");

    let needs_lifetime = fields.iter().any(field_borrows);

    // Generate variant definitions
    let variants: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let variant_name = format_ident!("{}", heck::AsUpperCamelCase(&f.name).to_string());
            let ty = variant_type(f);
            quote! { #variant_name(#ty) }
        })
        .collect();

    // Generate match arms for decode
    let match_arms: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let tag = f.number as u32;
            let variant_name = format_ident!("{}", heck::AsUpperCamelCase(&f.name).to_string());
            let field_name_str = &f.name;
            let wt = wire_type_token(f);
            let decode = decode_expr(f);
            let value = variant_value_expr(f);

            quote! {
                #tag => {
                    Some((
                        || {
                         tacky::check_wire_type(wire_type, #wt, #field_name_str)?;
                    #decode
                    Ok(#enum_name::#variant_name(#value))
                    })())


                }
            }
        })
        .collect();

    let fields_iterator_name = format_ident!("{name}Fields");

    let (lt_token, lt_name) = if needs_lifetime {
        (quote! {<'a>}, (quote! {'a}))
    } else {
        (quote! {}, quote! {})
    };

    quote! {
        #[derive(Debug, Copy, Clone, PartialEq)]
        pub enum #enum_name #lt_token {
            #(#variants,)*
        }
        pub struct #fields_iterator_name<'a> {
            buf: &'a [u8],
        }

        impl<'a> #fields_iterator_name<'a> {
            pub fn new(buf: &'a [u8]) -> Self {
                Self { buf }
            }
        }
        impl<'a> Iterator for #fields_iterator_name<'a> {
            type Item = Result<#enum_name #lt_token, tacky::DecodeError>;

            fn next(&mut self) -> Option<Self::Item> {
                if self.buf.is_empty() {
                    return None;
                }
                let buf = &mut self.buf;
                let (tag, wire_type) = match tacky::decode_key(buf) {
                    Ok(t) => t,
                    Err(e) => return Some(Err(e)),
                };
                match tag {
                    #(#match_arms)*
                    _ => {
                        match tacky::skip_field(wire_type, buf) {
                            Ok(()) => Self::next(self), // recursively call next to find the next known field
                            Err(e) => Some(Err(e)),
                        }

                    }
                }
            }
        }
    }
}
