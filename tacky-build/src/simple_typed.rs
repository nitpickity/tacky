use std::fmt::Write;

use crate::{
    formatter::Fmter,
    parser::{Field, Label, PbType},
};

// generate writing methods for simple scalar fields
#[rustfmt::skip]
pub fn get_scalar_writer(w: &mut Fmter<'_>, field: &Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label: _,
    } = field;
    let PbType::Scalar(pb_type) = ty else {
        panic!()
    };
    let full_type = format!("ScalarWriter<'_,{number},{}>", pb_type.tacky_type());
    indented!(w, r"pub fn {name}_writer(&mut self) -> {full_type} {{")?;
    indented!(w, r"    <{full_type}>::new(&mut self.tack.buffer)")?;
    indented!(w, r"}}")
}

/// generate writing methods for a map whose key-values are simple scalar
/// map is exactly a "repeated" of
/// message MapEntry {
///     optional key_type key = 1;
///     optional val type val = 2;
/// }
pub fn get_map_writer(w: &mut Fmter<'_>, field: &Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label: _,
    } = field;
    let PbType::SimpleMap(k, v) = ty else {
        panic!()
    };

    let full_type = format!(
        "MapEntryWriter<'_, {number},{}, {}>",
        k.tacky_type(),
        v.tacky_type()
    );
    indented!(w, r"pub fn {name}_writer(&mut self) -> {full_type} {{")?;
    indented!(w, r"    <{full_type}>::new(&mut self.tack.buffer)")?;
    indented!(w, r"}}")
}

// generate writing method for message-type fields
pub fn get_message_writer(w: &mut Fmter, field: &Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;
    let tag = ty.tag(*number as u32);
    let ty = match ty {
        PbType::Message(m) => m,
        _ => panic!(),
    };
    let wrap_label = |l: &str| match label {
        Label::Required => format!("Field<{number},Required<{l}>>"),
        Label::Optional | Label::Plain => format!("Field<{number},Optional<{l}>>"),
        Label::Repeated => format!("Field<{number},Repeated<{l}>>"),
        Label::Packed => panic!("messages cant be packed"),
    };
    // due to the inremental nature of this lib, its impossible to actually hold an iterator/collection of message writers,
    // so there isnt any syntactic helper for repeated (nested) message type, the user of the lib just has to hoist the write loop outside
    // for i in 0..10 {
    //   m.write_nested(|w| {
    //     w.write_field(i);
    //})
    //}
    let witness_type = wrap_label("PbMessage");
    let t = format!("MessageWriter<'_, {number}, {ty}Writer>");
    indented!(w, r"pub fn {name}_writer(&mut self) -> {ty}Writer<'_> {{ ")?;
    indented!(
        w,
        r"    <{ty}Writer>::new(&mut self.tack.buffer, Some({number}))"
    )?;
    indented!(w, r"}}")
}
//genrate ate writing method for enum-type fields
// enums are just i32s, so we take anything thats Into<i32>.
pub fn get_enum_writer(w: &mut Fmter, field: &Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label: _,
    } = field;
    let PbType::Enum((ename, valid)) = ty else {
        panic!()
    };
    let full_type = format!("EnumWriter<'_,{number}>");
    indented!(w, r"pub fn {name}_writer(&mut self) -> {full_type} {{")?;
    indented!(w, r"    <{full_type}>::new(&mut self.tack.buffer)")?;
    indented!(w, r"}}")
}
