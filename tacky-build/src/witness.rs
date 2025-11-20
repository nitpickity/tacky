use core::panic;
use std::fmt::Write;

use crate::{
    formatter::Fmter,
    parser::{Field, Label, PbType, Scalar},
};

#[rustfmt::skip]
pub fn message_def_writer(w: &mut Fmter<'_>, name: &str){
    //write struct
    indented!(w, r"pub struct {name}Writer<'buf> {{");
    indented!(w, r"   tack: Tack<'buf>");
    indented!(w, r"}}");
    indented!(w);
    indented!(w, r"impl<'buf> {name}Writer<'buf> {{");
    w.indent();
    indented!(w, r"pub fn new(buf: &'buf mut Vec<u8>, tag: Option<u32>) -> Self {{");
    indented!(w, r"    Self {{tack: Tack::new(buf, tag)}}");
    indented!(w, r"}}");
    indented!(w, r"pub fn written(&self) -> usize {{");
    indented!(w, r"    self.tack.buffer.len()");
    indented!(w, r"}}");
    w.unindent();
    indented!(w, r"}}");
    indented!(w)
}

#[rustfmt::skip]
pub fn field_witness_type(w: &mut Fmter<'_>, field: &Field){
    let Field {
        name,
        number,
        ty,
        label,
    } = field;
    let mut wrap_label = |l: &str| match label {
        Label::Required => indented!(w, "pub {name}: Field<{number}, Required<{l}>>,"),
        Label::Optional => indented!(w, "pub {name}: Field<{number}, Optional<{l}>>,"),
        Label::Repeated => indented!(w, "pub {name}: Field<{number}, Repeated<{l}>>,"),
        Label::Packed => indented!(w, "pub {name}: Field<{number}, Packed<{l}>>,"),
        Label::Plain => indented!(w, "pub {name}: Field<{number}, Plain<{l}>>,")
    };
    match ty {
        PbType::Scalar(p) => wrap_label(p.tacky_type()),
        PbType::SimpleMap(k, v) => indented!(w,"pub {name}: Field<{number}, PbMap<{}, {}>>,",k.tacky_type(), v.tacky_type()),
        PbType::Message(m) => wrap_label(m),
        PbType::Enum((name, _fields)) => wrap_label(name),
        PbType::Map(k, v) => {
            let k = k.tacky_type();
            let v = match &**v {
                PbType::Scalar(s) => s.tacky_type(),
                PbType::Enum((name,_fields)) => name,
                PbType::Message(m) => m,
                _ => panic!("map values cant be other maps")
            };
            indented!(w,"pub {name}: Field<{number}, PbMap<{k}, {v}>>")
        },
    }
}
