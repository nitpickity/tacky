use std::fmt::Write;

use crate::{parser::{Field, Label, PbType, Scalar}, formatter::Fmter};

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
pub fn simple_field_writer(w: &mut Fmter<'_>, field: Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;
    let tag = ty.tag(number as u32);
    let PbType::Scalar(pb_type) = ty else {
        panic!()
    };
    let rust_type = pb_type.rust_type();
    let write_fn = format!("::tacky::scalars::write_{pb_type}");
    
    match label {
        Label::Optional => match pb_type {
            Scalar::String | Scalar::Bytes => {
                let rust_type = pb_type.rust_type_no_ref();
                indented!(w, r"pub fn {name}<'opt>(&mut self, {name}: impl Into<Option<&'opt {rust_type}>>) -> &mut Self {{")?;
                indented!(w, r"    if let Some(value) = {name}.into() {{")?;
                indented!(w, r"        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);")?;
                indented!(w, r"        {write_fn}(value, &mut self.tack.buffer);")?;
                indented!(w, r"    }}")?;
                indented!(w, r"    self")?;
                indented!(w, r"}}")
            }
            _ => {
                indented!(w, r"pub fn {name}(&mut self, {name}: impl Into<Option<{rust_type}>>) -> &mut Self {{")?;
                indented!(w, r"    if let Some(value) = {name}.into() {{")?;
                indented!(w, r"        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);")?;
                indented!(w, r"        {write_fn}(value, &mut self.tack.buffer);")?;
                indented!(w, r"    }}")?;
                indented!(w, r"    self")?;
                indented!(w, r"}}")
            }
        },

        Label::Repeated => match pb_type {
            Scalar::String | Scalar::Bytes => {
                let rust_type = pb_type.rust_type_no_ref();
                indented!(w, r"pub fn {name}<T: AsRef<{rust_type}>>(&mut self, {name}: impl IntoIterator<Item = T>) -> &mut Self {{")?;
                indented!(w, r"    for value in {name} {{")?;
                indented!(w, r"        let value = value.as_ref();")?;                       
                indented!(w, r"        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);")?; 
                indented!(w, r"        {write_fn}(value, &mut self.tack.buffer);")?; 
                indented!(w, r"    }}")?; 
                indented!(w, r"    self")?; 
                indented!(w, r"}}")
            }
            _ => {
                indented!(w, r"pub fn {name}<'rep>(&mut self, {name}: impl IntoIterator<Item = &'rep {rust_type}>) -> &mut Self {{")?;
                indented!(w, r"    for value in {name} {{")?;
                indented!(w, r"        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);")?;
                indented!(w, r"        {write_fn}(*value, &mut self.tack.buffer);")?;
                indented!(w, r"    }}")?; 
                indented!(w, r"    self")?; 
                indented!(w, r"}}")
            }
        },
        Label::Required => {
            indented!(w, r"pub fn {name}(&mut self, {name}: {rust_type}) -> &mut Self {{")?;
            indented!(w, r"    ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);")?;
            indented!(w, r"    {write_fn}({name}, &mut self.tack.buffer);")?;
            indented!(w, r"    self")?; 
            indented!(w, r"}}")
        }
        Label::Packed => {
            // encoded using the same wide-varint approach as nested messages. this means that we "waste" a little space bit skip iterating twice.
            // need to have a check if the iterator is empty first, otherwise we will write the tag\len wrongly.
            let tag = (number << 3) | 2; // wire type 2, length delimited
            indented!(w, r"pub fn {name}<'rep>(&mut self, {name}: impl IntoIterator<Item = &'rep {rust_type}>) -> &mut Self {{")?;
            indented!(w, r"    let mut it = {name}.into_iter();")?;
            indented!(w, r"    let first = it.next();")?; 
            indented!(w, r"    if let Some(value) = first {{")?;
            indented!(w, r"        let tack = ::tacky::tack::Tack::new(self.tack.buffer, Some({tag}));")?;
            indented!(w, r"        {write_fn}(*value, tack.buffer);")?;
            indented!(w, r"        for value in it {{")?;
            indented!(w, r"            {write_fn}(*value, tack.buffer);")?;
            indented!(w, r"        }}")?;
            indented!(w, r"        drop(tack);")?;
            indented!(w, r"    }}")?;
            indented!(w, r"    self")?;
            indented!(w, r"}}")
        }
    }
}

/// generate writing methods for a map whose key-values are simple scalar
/// map is exactly a "repeated" of
/// message MapEntry {
///     optional key_type key = 1;
///     optional val type val = 2;
/// }
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
fn simple_enum_writer(w: &mut impl Write, field: Field) -> std::fmt::Result {
    todo!()
}