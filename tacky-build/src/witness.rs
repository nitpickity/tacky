use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::parser::{parse_ty, Field, Label, PbType, Scalar};

pub fn message_def_writer(name: &str) -> TokenStream {
    let name_ident = format_ident!("{name}");
    let writer_name = format_ident!("{name}Writer");

    quote! {
        pub struct #writer_name<'buf> {
            tack: Tack<'buf>
        }

        impl<'buf> #writer_name<'buf> {
            pub fn new(buf: &'buf mut Vec<u8>, tag: Option<u32>) -> Self {
                Self { tack: Tack::new(buf, tag) }
            }
            pub fn written(&self) -> usize {
                self.tack.buffer.len()
            }
        }
    }
}

pub fn field_witness_type(field: &Field) -> TokenStream {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;
    
    let name_ident = format_ident!("{name}");
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
        },
        PbType::Message(m) => wrap_label(m),
        PbType::Enum((name, _fields)) => wrap_label(name),
        PbType::Map(k, v) => {
            let k_str = k.tacky_type();
            let v_str = match &**v {
                PbType::Scalar(s) => s.tacky_type(),
                PbType::Enum((name, _fields)) => name,
                PbType::Message(m) => m,
                _ => panic!("map values cant be other maps")
            };
             let k_ident = parse_ty(k_str);
             let v_ident = parse_ty(v_str);
             quote!(pub #name_ident: Field<#number_lit, PbMap<#k_ident, #v_ident>>)
        },
    }
}
