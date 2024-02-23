use std::fmt::{format, Write};

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
        label,
    } = field;
    let ty = match ty {
        PbType::Scalar(s) => s.tacky_type(),
        PbType::Enum(e) => &e.0,
        PbType::Message(m) => m,
        PbType::SimpleMap(_, _) => todo!(),
        PbType::Map(_, _) => todo!(),
    };

    let generics = format!("<'_,{number},{ty}>");
    let (return_type, field_type) = match label {
        Label::Required =>(
            format!("RequiredValueWriter{generics}"),
            format!("Field<{number},Required<{ty}>>")
        ),
        Label::Optional => (
            format!("OptionalValueWriter{generics}"),
            format!("Field<{number},Optional<{ty}>>")
        ),

        Label::Repeated => (
            format!("RepeatedValueWriter{generics}"),
            format!("Field<{number},Repeated<{ty}>>")
        ),
        Label::Packed => (
            format!("PackedValueWriter{generics}"),
            format!("Field<{number},Packed<{ty}>>")
        ),
        Label::Plain => (
            format!("PlainValueWriter{generics}"),
            format!("Field<{number},Plain<{ty}>>")
        )
    };
    indented!(w, r"pub fn {name}_writer(&mut self) -> {return_type} {{")?;
    indented!(w, r"    <{field_type}>::get_writer(&mut self.tack.buffer)")?;
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
