use std::fmt::Write;

use crate::{
    formatter::Fmter,
    parser::{Field, Label, PbType, Scalar},
};

const TACKY: &'static str = "::tacky::typed_writers";
#[rustfmt::skip]
pub fn message_def_writer(w: &mut Fmter<'_>, name: &str) -> std::fmt::Result {
    //write struct
    indented!(w, r"pub struct {name}Writer<'buf> {{")?;
    indented!(w, r"   tack: ::tacky::tack::Tack<'buf>")?;
    indented!(w, r"}}")?;
    indented!(w)?;
    indented!(w, r"impl<'buf> {name}Writer<'buf> {{")?;
    w.indent();
    indented!(w, r"pub fn new(buf: &'buf mut Vec<u8>, tag: Option<u32>) -> Self {{")?;
    indented!(w, r"    Self {{tack: ::tacky::tack::Tack::new(buf, tag)}}")?;
    indented!(w, r"}}")?;
    w.unindent();
    indented!(w, r"}}")?;
    indented!(w)
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
    let tacky_type = pb_type.tacky_type();
    let writer_new = format!("{TACKY}::ScalarWriter::<'_,{TACKY}::{tacky_type}>::new");
    indented!(w, r"pub fn {name}_writer(&mut self) -> {TACKY}::ScalarWriter<'_,{TACKY}::{tacky_type}> {{")?;
    indented!(w, r"    {writer_new}(&mut self.tack.buffer, {number})")?;
    indented!(w, r"}}")
    
}

/// generate writing methods for a map whose key-values are simple scalar
/// map is exactly a "repeated" of
/// message MapEntry {
///     optional key_type key = 1;
///     optional val type val = 2;
/// }
#[rustfmt::skip]
pub fn simple_map_writer(w: &mut Fmter<'_>, field: Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label: _,
    } = field;
    let PbType::Map(k, v) = &ty else { panic!() };
    let (PbType::Scalar(k), PbType::Scalar(v)) = (k.as_ref(), v.as_ref()) else {
        panic!()
    };

    let tag = ty.tag(number as u32);
    let key_tag = (1 << 3) | k.wire_type();
    let key_write_fn = format!("::tacky::scalars::write_{k}");
    let key_len_fn = format!("::tacky::scalars::len_of_{k}");
    let val_tag = (2 << 3) | v.wire_type();
    let val_write_fn = format!("::tacky::scalars::write_{v}");
    let val_len_fn = format!("::tacky::scalars::len_of_{v}");
    let (key_type, val_type) = (k.rust_type_no_ref(), v.rust_type_no_ref());
    indented!(w,r"pub fn {name}<'rep>(&mut self, entries: impl IntoIterator<Item =(&'rep {key_type},&'rep {val_type})>) -> &mut Self {{")?;
    indented!(w,r"    for (key, value) in entries {{")?;
    indented!(w,r"        let len = 2 + {key_len_fn}(*key) + {val_len_fn}(value);")?;
    indented!(w,r"        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);")?;
    indented!(w,r"        ::tacky::scalars::write_varint(len as u64, &mut self.tack.buffer);")?;
    indented!(w,r"        ::tacky::scalars::write_varint({key_tag}, &mut self.tack.buffer);")?;
    indented!(w,r"        {key_write_fn}(*key, &mut self.tack.buffer);")?;
    indented!(w,r"        ::tacky::scalars::write_varint({val_tag}, &mut self.tack.buffer);")?;
    indented!(w,r"        {val_write_fn}(value, &mut self.tack.buffer);")?;
    indented!(w,r"    }}")?;
    indented!(w,r"    self")?;
    indented!(w,r"}}")
    
}

// generate writing method for message-type fields
#[rustfmt::skip]
pub fn simple_message_writer(
    w: &mut Fmter,
    field: Field,
) -> std::fmt::Result {
    let Field { name, number, ty, label } = field;
    let tag = ty.tag(number as u32);
    let ty = match ty {
        PbType::Message(m) => m,
        _ => panic!(),
    };
    // due to the inremental nature of this lib, its impossible to actually hold an iterator/collection of message writers,
    // so there isnt any syntactic helper for repeated (nested) message type, the user of the lib just has to hoist the write loop outside
    // for i in 0..10 {
    //   m.write_nested(|w| {
    //     w.write_field(i);
    //})
    //}
    indented!(w,r"pub fn {name}(&mut self, mut {name}: impl FnMut({ty}Writer)) -> &mut Self {{ ")?;
    indented!(w,r"    let writer = {ty}Writer::new(&mut self.tack.buffer,Some({tag}));")?;
    indented!(w,r"    {name}(writer);")?;
    indented!(w,r"    self")?;
    indented!(w,r"}}")
             
}

//genrate ate writing method for enum-type fields
// enums are just i32s, so we take anything thats Into<i32>.
#[rustfmt::skip]
fn simple_enum_writer(w: &mut impl Write, field: Field) -> std::fmt::Result {
    todo!()
}
