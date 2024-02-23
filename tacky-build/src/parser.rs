//! Currently wraps/uses pb-rs from quick-protobuf as the underlying parser, as i dont want any protoc system deps (a la prost)
//! and dont i dont to write my own (yet).

use std::io::Write;

use pb_rs::types::{FieldType, FileDescriptor, Message};

use crate::{
    formatter::Fmter,
    simple_typed::{get_enum_writer, get_map_writer, get_scalar_writer},
    witness::{
        field_witness_type, message_def_writer, simple_enum_witness, simple_field_witness,
        simple_map_witness, simple_message_witness,
    },
};

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
    pub const fn rust_type_no_ref(&self) -> &str {
        match self {
            Scalar::String => "str",
            Scalar::Bytes => "[u8]",
            _ => self.rust_type(),
        }
    }
    pub const fn rust_type(&self) -> &str {
        match self {
            Scalar::Int32 => "i32",
            Scalar::Sint32 => "i32",
            Scalar::Int64 => "i64",
            Scalar::Sint64 => "i64",
            Scalar::Uint32 => "u32",
            Scalar::Uint64 => "u64",
            Scalar::Bool => "bool",
            Scalar::Fixed32 => "u32",
            Scalar::Sfixed32 => "i32",
            Scalar::Float => "f32",
            Scalar::Fixed64 => "u64",
            Scalar::Sfixed64 => "i64",
            Scalar::Double => "f64",
            Scalar::String => "&str",
            Scalar::Bytes => "&[u8]",
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
    pub const fn tag(&self, field_nr: u32) -> u32 {
        (field_nr << 3) | self.wire_type()
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

fn write_writer_api<'a>(w: &mut Fmter<'_>, fields: impl IntoIterator<Item = &'a Field>) {
    for f in fields {
        match f.ty {
            PbType::SimpleMap(_, _) => {
                get_map_writer(w, &f).unwrap();
            }
            // PbType::Scalar(_) => {
            //     get_scalar_writer(w, &f).unwrap();
            // }
            // PbType::Message(_) => {
            //     // the closure based API for nested messages is already generated
            //     get_message_writer(w, &f).unwrap()
            // }
            PbType::Enum(_) => get_enum_writer(w, &f).unwrap(),
            _ => get_scalar_writer(w, &f).unwrap(),
        }
    }
}

fn write_witness_api<'a>(w: &mut Fmter<'_>, fields: impl IntoIterator<Item = &'a Field>) {
    for f in fields {
        match &f.ty {
            PbType::SimpleMap(_, _) => {
                simple_map_witness(w, &f).unwrap();
            }
            PbType::Scalar(_) => {
                simple_field_witness(w, &f).unwrap();
            }
            PbType::Message(_) => simple_message_witness(w, &f).unwrap(),
            PbType::Enum(_) => simple_enum_witness(w, &f).unwrap(),
            _ => todo!(),
        }
    }
}

fn write_simple_message(w: &mut Fmter<'_>, m: &Message, desc: &FileDescriptor) {
    let name = &m.name;
    let fields = m
        .fields
        .iter()
        .map(|f| convert_field(f, desc))
        .collect::<Vec<_>>();

    //write struct

    indented!(w).unwrap();
    write_simple_message_schema(w, name, &fields);
    write_trait_impl(w, name);
    message_def_writer(w, &name).unwrap();
    indented!(w, r#"impl<'buf> {name}Writer<'buf> {{"#).unwrap();
    w.indent();
    write_writer_api(w, &fields);
    write_witness_api(w, &fields);
    w.unindent();
    indented!(w, "}}").unwrap();
}

fn write_trait_impl(w: &mut Fmter<'_>, name: &str) {
    indented!(w, r#"impl MessageSchema for {name} {{"#).unwrap();
    w.indent();
    indented!(w, "type Writer<'a> = {name}Writer<'a>; ").unwrap();
    indented!(
        w,
        "fn new_writer<'a>(buffer: &'a mut Vec<u8>, tag: Option<i32>) -> Self::Writer<'a> {{"
    )
    .unwrap();
    w.indent();
    indented!(w, " <Self::Writer<'_>>::new(buffer, tag.map(|t| t as u32))").unwrap();
    w.unindent();
    indented!(w, "}}").unwrap();
    w.unindent();
    indented!(w, "}}").unwrap();
}

fn write_simple_message_schema(w: &mut Fmter<'_>, name: &str, fields: &[Field]) {
    //write struct
    indented!(w, r"#[derive(Default)]");
    indented!(w, r"pub struct {name} {{").unwrap();
    w.indent();
    for f in fields {
        field_witness_type(w, &f).unwrap()
    }
    w.unindent();
    indented!(w, "}}").unwrap();
}

pub fn write_proto(file: &str, output: &str) {
    let mut files = read_proto_file(file, ".");
    let test_file = files.pop().unwrap();
    let mut buf = String::new();
    let mut fmter = Fmter::new(&mut buf);
    let mod_name = test_file.package.clone();
    indented!(fmter, "#[allow(unused, dead_code)]").unwrap();
    indented!(fmter, "pub mod {mod_name} {{").unwrap();
    fmter.indent();
    indented!(fmter, "use ::tacky::*;").unwrap();
    for m in &test_file.messages {
        write_simple_message(&mut fmter, m, &test_file);
    }
    fmter.unindent();
    indented!(fmter, "}}").unwrap();
    let mut file = std::fs::File::create(output).unwrap();
    file.write_all(buf.as_bytes()).unwrap();
}
