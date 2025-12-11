//! Currently wraps/uses pb-rs from quick-protobuf as the underlying parser, as i dont want any protoc system deps (a la prost)
//! and dont i dont to write my own (yet).

use pb_rs::types::{Enumerator, FieldType, FileDescriptor, Message};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::io::Write;

use crate::{
    simple_typed::get_writer,
    witness::{field_witness_type, message_def_writer},
};

pub fn parse_ty(s: &str) -> syn::Type {
    syn::parse_str(s).unwrap_or_else(|_| panic!("failed to parse type: {}", s))
}

fn read_proto_file(file: &str, includes: &str) -> Vec<FileDescriptor> {
    let cfg = pb_rs::ConfigBuilder::new(&[file], None, None, &[includes]).unwrap();
    let cfg = cfg.build();
    let mut out = Vec::new();
    for cfg in cfg {
        let file = pb_rs::types::FileDescriptor::read_proto(&cfg.in_file, &cfg.import_search_path)
            .unwrap();
        out.push(file)
    }
    out
}

#[derive(Debug)]
pub enum Scalar {
    Int32,
    Sint32,
    Int64,
    Sint64,
    Uint32,
    Uint64,
    Bool,
    Fixed32,
    Sfixed32,
    Float,
    Fixed64,
    Sfixed64,
    Double,
    String,
    Bytes,
}

impl Scalar {
    pub const fn as_str(&self) -> &str {
        match self {
            Scalar::Int32 => "int32",
            Scalar::Sint32 => "sint32",
            Scalar::Int64 => "int64",
            Scalar::Sint64 => "sint64",
            Scalar::Uint32 => "uint32",
            Scalar::Uint64 => "uint64",
            Scalar::Bool => "bool",
            Scalar::Fixed32 => "fixed32",
            Scalar::Sfixed32 => "sfixed32",
            Scalar::Float => "float",
            Scalar::Fixed64 => "fixed64",
            Scalar::Sfixed64 => "sfixed64",
            Scalar::Double => "double",
            Scalar::String => "string",
            Scalar::Bytes => "bytes",
        }
    }

    pub const fn tacky_type(&self) -> &str {
        match self {
            Scalar::Int32 => "Int32",
            Scalar::Sint32 => "Sint32",
            Scalar::Int64 => "Int64",
            Scalar::Sint64 => "Sint64",
            Scalar::Uint32 => "Uint32",
            Scalar::Uint64 => "Uint64",
            Scalar::Bool => "Bool",
            Scalar::Fixed32 => "Fixed32",
            Scalar::Sfixed32 => "Sfixed32",
            Scalar::Float => "Float",
            Scalar::Fixed64 => "Fixed64",
            Scalar::Sfixed64 => "Sfixed64",
            Scalar::Double => "Double",
            Scalar::String => "PbString",
            Scalar::Bytes => "PbBytes",
        }
    }

    pub const fn wire_type(&self) -> u32 {
        match self {
            Scalar::Int32
            | Scalar::Sint32
            | Scalar::Int64
            | Scalar::Sint64
            | Scalar::Uint32
            | Scalar::Uint64
            | Scalar::Bool => 0,
            Scalar::Fixed32 | Scalar::Sfixed32 | Scalar::Float => 5,
            Scalar::Fixed64 | Scalar::Sfixed64 | Scalar::Double => 1,
            Scalar::String | Scalar::Bytes => 2,
        }
    }
}
impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
#[derive(Debug)]
pub enum PbType {
    Scalar(Scalar),
    Enum((String, Vec<i32>)), // name and allowed values
    Message(String),          //name
    SimpleMap(Scalar, Scalar),
    Map(Scalar, Box<PbType>),
}

fn resolve_type(value: FieldType, desc: &FileDescriptor) -> PbType {
    match value {
        FieldType::Int32 => PbType::Scalar(Scalar::Int32),
        FieldType::Int64 => PbType::Scalar(Scalar::Int64),
        FieldType::Uint32 => PbType::Scalar(Scalar::Uint32),
        FieldType::Uint64 => PbType::Scalar(Scalar::Uint64),
        FieldType::Sint32 => PbType::Scalar(Scalar::Sint32),
        FieldType::Sint64 => PbType::Scalar(Scalar::Sint64),
        FieldType::Bool => PbType::Scalar(Scalar::Bool),
        FieldType::Fixed64 => PbType::Scalar(Scalar::Fixed64),
        FieldType::Sfixed64 => PbType::Scalar(Scalar::Sfixed64),
        FieldType::Double => PbType::Scalar(Scalar::Double),
        FieldType::String => PbType::Scalar(Scalar::String),
        FieldType::Bytes => PbType::Scalar(Scalar::Bytes),
        FieldType::Fixed32 => PbType::Scalar(Scalar::Fixed32),
        FieldType::Sfixed32 => PbType::Scalar(Scalar::Sfixed32),
        FieldType::Float => PbType::Scalar(Scalar::Float),
        FieldType::Map(k, v) => {
            let kt: PbType = resolve_type(*k, desc);
            let vt: PbType = resolve_type(*v, desc);
            match (kt, vt) {
                (PbType::Scalar(k), PbType::Scalar(v)) => PbType::SimpleMap(k, v),
                (PbType::Scalar(k), v) => PbType::Map(k, Box::new((v).into())),
                _ => panic!("invalid map structure"),
            }
        }
        FieldType::Message(m) => {
            let name = &m.get_message(desc).name;
            PbType::Message(name.clone())
        }
        FieldType::Enum(e) => {
            let e = e.get_enum(desc);
            let name = e.name.clone();
            let values = e.fields.iter().map(|(_, v)| *v).collect();
            PbType::Enum((name, values))
        }
        FieldType::MessageOrEnum(s) => unreachable!(),
    }
}

impl PbType {
    pub const fn wire_type(&self) -> u32 {
        match self {
            PbType::Scalar(s) => s.wire_type(),
            PbType::Enum(_) => 0, //varint
            PbType::Message(_) | PbType::Map(_, _) | PbType::SimpleMap(_, _) => 2,
        }
    }
}

impl std::fmt::Display for PbType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PbType::Scalar(s) => f.write_str(s.as_str()),
            PbType::Enum(e) => f.write_str(&e.0),
            PbType::Message(m) => f.write_str(m),
            PbType::Map(k, v) => write!(f, "map<{},{}>", k.as_str(), v.to_string()),
            PbType::SimpleMap(k, v) => write!(f, "map<{},{}>", k.as_str(), v.as_str()),
        }
    }
}

pub enum Label {
    Required, //proto2 required fields, N/A to proto3
    Optional, //Proto2 and Proto3 fields with "optional" label
    Repeated, //"repeated" fields in both proto2/3
    Packed,   // packable fields with packed = true in proto2 or by default in proto3
    Plain,    //field with no label in proto3, not written to the wire if equals to default
}

pub struct Field {
    pub name: String,
    pub number: i32,
    pub ty: PbType,
    pub label: Label,
}

fn convert_field(field: &pb_rs::types::Field, desc: &FileDescriptor) -> Field {
    let pb_rs::types::Field {
        name,
        frequency,
        typ,
        number,
        default,
        deprecated,
    } = field;
    Field {
        name: name.clone(),
        number: *number,
        ty: resolve_type(typ.clone(), desc),
        label: frequency.map(|f| f.into()).unwrap_or(Label::Plain),
    }
}
impl From<pb_rs::types::Frequency> for Label {
    fn from(value: pb_rs::types::Frequency) -> Self {
        match value {
            pb_rs::types::Frequency::Optional => Label::Optional,
            pb_rs::types::Frequency::Repeated => Label::Repeated,
            pb_rs::types::Frequency::Required => Label::Required,
            pb_rs::types::Frequency::Packed => Label::Packed,
            pb_rs::types::Frequency::Plain => Label::Plain,
        }
    }
}

fn write_writer_api<'a>(fields: impl IntoIterator<Item = &'a Field>) -> TokenStream {
    let methods = fields.into_iter().map(|f| get_writer(f));
    quote! {
        #(#methods)*
    }
}

fn write_simple_message(m: &Message, desc: &FileDescriptor) -> TokenStream {
    let name = &m.name;
    let name_ident = format_ident!("{name}");
    let writer_name_ident = format_ident!("{name}Writer");

    let fields = m
        .all_fields()
        .map(|f| convert_field(f, desc))
        .collect::<Vec<_>>();

    let struct_schema = write_simple_message_schema(name, &fields);
    let trait_impl = write_trait_impl(name);
    let writer_def = message_def_writer(name);
    let writer_api = write_writer_api(&fields);

    quote! {
        #struct_schema
        #trait_impl
        #writer_def
        impl<'buf> #writer_name_ident<'buf> {
            #writer_api
        }
    }
}

fn write_simple_enum(m: &Enumerator, desc: &FileDescriptor) -> TokenStream {
    let name = &m.name;
    let name_ident = format_ident!("{name}");

    let variants = m.fields.iter().map(|(field, number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! {
             #field_ident = #number
        }
    });

    let from_matches = m.fields.iter().map(|(field, number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! {
            #number => Ok(#name_ident::#field_ident)
        }
    });

    quote! {
        #[repr(i32)]
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum #name_ident {
            #(#variants,)*
        }

        impl ProtoEncode<#name_ident> for #name_ident {
            const WIRE_TYPE: tacky::WireType = tacky::WireType::VARINT;
            fn encode(buf: &mut Vec<u8>, value: Self) {
                tacky::Int32::write_value(value as i32, buf)
            }
        }

        impl std::convert::TryFrom<i32> for #name_ident {
            type Error = ();
            fn try_from(value: i32) -> Result<Self, Self::Error> {
                match value {
                    #(#from_matches,)*
                    _ => Err(()),
                }
            }
        }
    }
}

fn write_trait_impl(name: &str) -> TokenStream {
    let name_ident = format_ident!("{name}");
    let writer_name_ident = format_ident!("{name}Writer");

    quote! {
        impl MessageSchema for #name_ident {
            type Writer<'a> = #writer_name_ident<'a>;
            fn new_writer<'a>(buffer: &'a mut Vec<u8>, tag: Option<i32>) -> Self::Writer<'a> {
                <Self::Writer<'_>>::new(buffer, tag.map(|t| t as u32))
            }
        }
    }
}

fn write_simple_message_schema(name: &str, fields: &[Field]) -> TokenStream {
    let name_ident = format_ident!("{name}");
    let field_defs = fields.iter().map(|f| field_witness_type(f));

    quote! {
        #[derive(Default, Debug, Copy, Clone)]
        pub struct #name_ident {
            #(#field_defs,)*
        }
    }
}

pub fn write_proto(file: &str, output: &str) {
    let mut files = read_proto_file(file, ".");
    let test_file = files.pop().unwrap();
    let mod_name = format_ident!("{}", test_file.package);

    let messages = test_file
        .messages
        .iter()
        .map(|m| write_simple_message(m, &test_file));
    let enums = test_file
        .enums
        .iter()
        .map(|m| write_simple_enum(m, &test_file));

    let token_stream = quote! {
        #[allow(unused, dead_code)]
        pub mod #mod_name {
            use ::tacky::*;
            #(#messages)*
            #(#enums)*
        }
    };

    // eprintln!("GENERATED CODE:\n{}", token_stream.to_string());

    let syntax_tree = syn::parse2(token_stream).unwrap();
    let formatted = prettyplease::unparse(&syntax_tree);

    let mut file = std::fs::File::create(output).unwrap();
    file.write_all(formatted.as_bytes()).unwrap();
}
