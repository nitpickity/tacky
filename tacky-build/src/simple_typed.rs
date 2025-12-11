use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::parser::{parse_ty, Field, Label, PbType};

// generate writing methods for simple scalar fields
pub fn get_writer(field: &Field) -> TokenStream {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;

    let name_ident = format_ident!("{name}");
    let number_lit = proc_macro2::Literal::u32_unsuffixed(*number as u32);

    let ty_str = match ty {
        PbType::Scalar(s) => s.tacky_type().to_string(),
        PbType::Enum((name, _fields)) => name.to_string(),
        PbType::Message(m) => m.to_string(),
        PbType::SimpleMap(k, v) => {
            let (k, v) = (k.tacky_type(), v.tacky_type());
            let k_ident = parse_ty(k);
            let v_ident = parse_ty(v);
            
            return quote! {
                pub fn #name_ident(&mut self) -> MapWriter<'_, #number_lit, #k_ident, #v_ident> {
                    MapWriter::new(self.tack.buffer)
                }
            };
        }
        PbType::Map(k, v) => {
            let k = k.tacky_type();
            let v = match &**v {
                PbType::Scalar(s) => s.tacky_type(),
                PbType::Enum((name, _fields)) => name,
                PbType::Message(m) => m,
                _ => panic!("map values cant be other maps"),
            };
            let k_ident = parse_ty(k);
            let v_ident = parse_ty(v);

            return quote! {
                pub fn #name_ident(&mut self) -> MapWriter<'_, #number_lit, #k_ident, #v_ident> {
                    MapWriter::new(self.tack.buffer)
                }
            };
        }
    };

    let ty_ident = parse_ty(&ty_str);

    let return_type = match label {
        Label::Required => quote!(FieldWriter<'_, #number_lit, Required<#ty_ident>>),
        Label::Optional => quote!(FieldWriter<'_, #number_lit, Optional<#ty_ident>>),
        Label::Repeated => quote!(FieldWriter<'_, #number_lit, Repeated<#ty_ident>>),
        Label::Packed => quote!(FieldWriter<'_, #number_lit, Packed<#ty_ident>>),
        Label::Plain => quote!(FieldWriter<'_, #number_lit, Plain<#ty_ident>>),
    };

    quote! {
        pub fn #name_ident(&mut self) -> #return_type {
            FieldWriter::new(self.tack.buffer)
        }
    }
}
