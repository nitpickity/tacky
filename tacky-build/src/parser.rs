//! Currently wraps/uses pb-rs from quick-protobuf as the underlying parser, as i dont want any protoc system deps an (a la prost)
//! and dont i dont to write my own (yet).

use std::{fmt::Write, io::Write as ioWrite};

use pb_rs::types::{FieldType, FileDescriptor, Message};

use crate::simple::{
    message_def_writer, simple_field_writer, simple_field_writer_label, simple_map_writer,
};

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
            Scalar::String => "str",
            Scalar::Bytes => "[u8]",
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
#[derive(Debug)]
pub enum PbType {
    Scalar(Scalar),
    Enum(String),    //name
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
            FieldType::Enum(_) => todo!(),
            FieldType::Fixed64 => PbType::Scalar(Scalar::Fixed64),
            FieldType::Sfixed64 => PbType::Scalar(Scalar::Sfixed64),
            FieldType::Double => PbType::Scalar(Scalar::Double),
            FieldType::StringCow => PbType::Scalar(Scalar::String),
            FieldType::BytesCow => PbType::Scalar(Scalar::Bytes),
            FieldType::String_ => PbType::Scalar(Scalar::String),
            FieldType::Bytes_ => PbType::Scalar(Scalar::Bytes),
            FieldType::Message(_) => todo!(),
            FieldType::MessageOrEnum(_) => todo!(),
            FieldType::Fixed32 => PbType::Scalar(Scalar::Fixed32),
            FieldType::Sfixed32 => PbType::Scalar(Scalar::Sfixed32),
            FieldType::Float => PbType::Scalar(Scalar::Float),
            FieldType::Map(k, v) => {
                let k = *k;
                let v = *v;
                PbType::Map(Box::new((k).into()), Box::new(v.into()))
            }
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

fn write_simple_message(w: &mut impl Write, m: &Message) {
    let name = &m.name;
    println!("{name}");
    //write struct
    message_def_writer(w, &name).unwrap();
    writeln!(w, r#"impl<'buf> {name}Writer<'buf> {{"#).unwrap();
    writeln!(w, r#"fn new(buf: &'buf mut Vec<u8>, tag: Option<u32>) -> Self {{
        Self {{tack: ::tacky::tack::Tack::new(buf, tag)}}    
    }}"#).unwrap();
    for f in &m.fields {
        let name = &f.name;
        let number = f.number;
        let label = f.frequency.clone();
        let ty: PbType = (f.typ.clone()).into();
        match ty {
            PbType::Map(_, _) => {
                simple_map_writer(w, &name, &ty, number as u32).unwrap();
            }
            PbType::Scalar(_) => {
                simple_field_writer_label(w, &name, label, &ty, number as u32).unwrap();
            }
            _ => todo!(),
        }
    }
    writeln!(w, "}}").unwrap();
}
#[test]
fn test_read() {
    let mut files = read_proto_file("src/simple_message.proto", ".");
    let test_file = files.pop().unwrap();
    let simple = &test_file.messages[0];
    let mut file = std::fs::File::create("simple_output.rs").unwrap();
    let mut buf = String::new();
    write_simple_message(&mut buf, &simple);
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
            .manystrings(&["this","works"])
            .astring(&*moo);
    }
}
