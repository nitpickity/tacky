use crate::parser::{field_ident, parse_ty, Field, Label, PbType};
use proc_macro2::TokenStream;
use quote::quote;

pub fn field_type(field: &Field) -> TokenStream {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;

    let name_ident = field_ident(name);
    let number_lit = proc_macro2::Literal::u32_unsuffixed(*number as u32);

    let wrap_label = |l: &str| {
        let ty_ident = parse_ty(l);
        match label {
            Label::Required => quote!(pub #name_ident: Field<#number_lit, Required<#ty_ident>>),
            Label::Optional => quote!(pub #name_ident: Field<#number_lit, Optional<#ty_ident>>),
            Label::Repeated => quote!(pub #name_ident: Field<#number_lit, Repeated<#ty_ident>>),
            Label::Packed => quote!(pub #name_ident: Field<#number_lit, Packed<#ty_ident>>),
            Label::Plain => quote!(pub #name_ident: Field<#number_lit, Plain<#ty_ident>>),
        }
    };

    match ty {
        PbType::Scalar(p) => wrap_label(p.tacky_type()),
        PbType::SimpleMap(k, v) => {
            let k_ident = parse_ty(k.tacky_type());
            let v_ident = parse_ty(v.tacky_type());
            quote!(pub #name_ident: Field<#number_lit, PbMap<#k_ident, #v_ident>>)
        }
        PbType::Message(m) => wrap_label(m),
        PbType::Enum((name, _fields)) => wrap_label(&format!("PbEnum<{name}>")),
        PbType::Map(k, v) => {
            let k_str = k.tacky_type();
            let v_str = match &**v {
                PbType::Scalar(s) => s.tacky_type(),
                PbType::Enum((name, _fields)) => name,
                PbType::Message(m) => m,
                _ => panic!("map values cant be other maps"),
            };
            let k_ident = parse_ty(k_str);
            let v_ident = parse_ty(v_str);
            quote!(pub #name_ident: Field<#number_lit, PbMap<#k_ident, #v_ident>>)
        }
    }
}
