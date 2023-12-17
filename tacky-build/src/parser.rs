//! Currently wraps/uses pb-rs from quick-protobuf as the underlying parser, as i dont want any protoc system deps an (a la prost)
//! and dont i dont to write my own (yet).

use std::{fmt::Write, io::Write as ioWrite};

use pb_rs::types::{FieldType, FileDescriptor, Message};

use crate::{simple::{message_def_writer, simple_field_writer_label, simple_map_writer}, formatter::Fmter};

fn read_proto_file(file: &str, includes: &str) -> Vec<FileDescriptor> {
    let cfg = pb_rs::ConfigBuilder::new(&[file], None, None, &[includes]).unwrap();
    let cfg = cfg.dont_use_cow(true).build();
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
    pub const fn rust_type_no_ref(&self) -> &str {
        match self {
            Scalar::String => "str",
            Scalar::Bytes => "[u8]",
            _ => self.rust_type(),
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
    Enum(String),    // by name, type is technically just an i32
    Message(String), //name
    Map(Box<PbType>, Box<PbType>),
}
impl From<FieldType> for PbType {
    fn from(value: FieldType) -> Self {
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
            FieldType::StringCow => PbType::Scalar(Scalar::String), //redundant from using pb-rs to parse..
            FieldType::BytesCow => PbType::Scalar(Scalar::Bytes), //redundant from using pb-rs to parse..
            FieldType::String_ => PbType::Scalar(Scalar::String),
            FieldType::Bytes_ => PbType::Scalar(Scalar::Bytes),
            FieldType::Fixed32 => PbType::Scalar(Scalar::Fixed32),
            FieldType::Sfixed32 => PbType::Scalar(Scalar::Sfixed32),
            FieldType::Float => PbType::Scalar(Scalar::Float),
            FieldType::Map(k, v) => {
                let k = *k;
                let v = *v;
                PbType::Map(Box::new((k).into()), Box::new(v.into()))
            }
            FieldType::MessageOrEnum(_) => todo!(),
            FieldType::Message(_) => todo!(),
            FieldType::Enum(_) => todo!(), //technically int32 according to spec
        }
    }
}
impl PbType {
    pub const fn wire_type(&self) -> u32 {
        match self {
            PbType::Scalar(s) => s.wire_type(),
            PbType::Enum(_) => 0, //varint
            PbType::Message(_) | PbType::Map(_, _) => 2,
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
            PbType::Enum(_) => todo!(),
            PbType::Message(_) => todo!(),
            PbType::Map(_, _) => todo!(),
        }
    }
}

pub enum Label {
    Required,
    Optional,
    Repeated,
    Packed,
}

pub struct Field {
    pub name: String,
    pub number: i32,
    pub ty: PbType,
    pub label: Label,
}

impl From<pb_rs::types::Field> for Field {
    fn from(value: pb_rs::types::Field) -> Self {
        let label = if let Some(true) = value.packed {
            Label::Packed
        } else {
            match value.frequency {
                pb_rs::types::Frequency::Optional => Label::Optional,
                pb_rs::types::Frequency::Repeated => Label::Repeated,
                pb_rs::types::Frequency::Required => Label::Required,
            }
        };
        Field {
            name: value.name,
            number: value.number,
            ty: value.typ.into(),
            label,
        }
    }
}

fn write_simple_message(w: &mut Fmter<'_>, m: Message) {
    let name = &m.name;
    //write struct
    message_def_writer(w, &name).unwrap();
    indented!(w, r#"impl<'buf> {name}Writer<'buf> {{"#).unwrap();
    w.indent();
    for f in m.fields {
        let field: Field = f.into();
        match field.ty {
            PbType::Map(_, _) => {
                simple_map_writer(w, field).unwrap();
            }
            PbType::Scalar(_) => {
                simple_field_writer_label(w, field).unwrap();
            }
            _ => todo!(),
        }
    }
    w.unindent();
    indented!(w, "}}").unwrap();
}

pub fn write_proto(file: &str, output: &str) {
    let mut files = read_proto_file(file, ".");
    let test_file = files.pop().unwrap();
    let simple = test_file.messages[0].clone();
    let mut file = std::fs::File::create(output).unwrap();
    let mut buf = String::new();
    let mut fmter = Fmter::new(&mut buf);
    write_simple_message(&mut fmter, simple);
    file.write_all(buf.as_bytes()).unwrap();
}
#[test]
fn test_read() {
    let mut files = read_proto_file("src/simple_message.proto", ".");
    let test_file = files.pop().unwrap();
    let simple = test_file.messages[0].clone();
    let mut file = std::fs::File::create("simple_output.rs").unwrap();
    let mut buf = String::new();
    let mut fmter = Fmter::new(&mut buf);
    write_simple_message(&mut fmter, simple);
    file.write_all(buf.as_bytes()).unwrap();
}

mod t {
    use std::{borrow::Cow, collections::HashMap};
    include!("../simple_output.rs");
    #[test]
    fn testme() {
        let mut buf = Vec::new();
        let map = HashMap::from([(1, "one".to_string()), (2, "two".to_string())]);
        let mut m = MySimpleMessageWriter::new(&mut buf, None);
        let moo = Cow::Borrowed("foo");
        m.abytes(&b"hello"[..])
            .anumber(42)
            .manystrings((&map).values())
            .manystrings(&["this", "works"])
            .astring(&*moo);
    }
}
