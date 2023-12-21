use std::fmt::Write;

use crate::{
    formatter::Fmter,
    parser::{Field, PbType, Scalar},
};

const TACKY: &'static str = "::tacky::typed_writers";
fn get_writer(t: &Scalar) -> String {
    format!("{TACKY}::ScalarWriter<'_,{TACKY}::{}>",t.tacky_type())
}
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
    let full_type = get_writer(&pb_type);
    indented!(w, r"pub fn {name}_writer(&mut self) -> {full_type} {{")?;
    indented!(w, r"    <{full_type}>::new(&mut self.tack.buffer, {number})")?;
    indented!(w, r"}}")
    
}

/// generate writing methods for a map whose key-values are simple scalar
/// map is exactly a "repeated" of
/// message MapEntry {
///     optional key_type key = 1;
///     optional val type val = 2;
/// }

pub fn get_map_writer(w: &mut Fmter<'_>, field: Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label: _,
    } = field;
todo!()
    
}

// generate writing method for message-type fields
pub fn get_message_writer(
    w: &mut Fmter,
    field: Field,
) -> std::fmt::Result {
todo!()
             
}
//genrate ate writing method for enum-type fields
// enums are just i32s, so we take anything thats Into<i32>.
fn get_enum_writer(w: &mut impl Write, field: Field) -> std::fmt::Result {
    todo!()
}