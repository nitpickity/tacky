use std::fmt::Write;

use crate::{
    formatter::Fmter,
    parser::{Field, Label, PbType, Scalar},
};
#[rustfmt::skip]
pub fn message_def_writer(w: &mut Fmter<'_>, name: &str) -> std::fmt::Result {
    //write struct
    indented!(w, r"pub struct {name}Writer<'buf> {{")?;
    indented!(w, r"   tack: Tack<'buf>")?;
    indented!(w, r"}}")?;
    indented!(w)?;
    indented!(w, r"impl<'buf> {name}Writer<'buf> {{")?;
    w.indent();
    indented!(w, r"pub fn new(buf: &'buf mut Vec<u8>, tag: Option<u32>) -> Self {{")?;
    indented!(w, r"    Self {{tack: Tack::new(buf, tag)}}")?;
    indented!(w, r"}}")?;
    w.unindent();
    indented!(w, r"}}")?;
    indented!(w)
}
// generate writing methods for simple scalar fields
#[rustfmt::skip]
pub fn simple_field_writer(w: &mut Fmter<'_>, field: Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;
    let PbType::Scalar(pb_type) = ty else {
        panic!("expected scalar type")
    };
    let rust_type = pb_type.rust_type_no_ref();
    let tacky_type = pb_type.tacky_type();
    let write_fn = format!("::tacky::scalars::write_{pb_type}");
    let mk_write_expr =
        |arg| format!("{tacky_type}::write({number}, {arg}, &mut self.tack.buffer);");
    match label {
        Label::Optional => {
            let (lf, rust_type) = match pb_type {
                Scalar::String | Scalar::Bytes => ("<'opt>", format!("&'opt {rust_type}")),
                _ => ("", rust_type.into()),
            };
            let write_expr = mk_write_expr("value");
            indented!(w,"pub fn {name}{lf}(&mut self, {name}: impl Into<Option<{rust_type}>>) -> &mut Self {{")?;
            indented!(w, r"    if let Some(value) = {name}.into() {{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"    }}")?;
            indented!(w, r"    self")?;
            indented!(w, r"}}")
        }

        Label::Repeated => {
            let (item, generics, value) = match pb_type {
                Scalar::String | Scalar::Bytes => {
                    ("T", format!("<T: AsRef<{rust_type}>>"), "value.as_ref()")
                }
                _ => (rust_type, "".into(), "value"),
            };
            let write_expr = mk_write_expr(value);
            indented!(w,r"pub fn {name}{generics}(&mut self, {name}: impl IntoIterator<Item = {item}>) -> &mut Self {{")?;
            indented!(w, r"    for value in {name} {{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"    }}")?;
            indented!(w, r"    self")?;
            indented!(w, r"}}")
        }
        Label::Required => {
            let write_expr = mk_write_expr(&name);
            indented!(w,r"pub fn {name}(&mut self, {name}: {rust_type}) -> &mut Self {{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"    self")?;
            indented!(w, r"}}")
        }
        Label::Packed => {
            // encoded using the same wide-varint approach as nested messages. this means that we "waste" a little space bit skip iterating twice.
            // need to have a check if the iterator is empty first, otherwise we will write the tag\len wrongly.
            let tag = (number << 3) | 2; // wire type 2, length delimited
            indented!(w,r"pub fn {name}<'rep>(&mut self, {name}: impl IntoIterator<Item = &'rep {rust_type}>) -> &mut Self {{")?;
            indented!(w, r"    let mut it = {name}.into_iter();")?;
            indented!(w, r"    let first = it.next();")?;
            indented!(w, r"    if let Some(value) = first {{")?;
            indented!(w,r"        let tack: Tack<Width<2>> = Tack::new(self.tack.buffer, Some({tag}));")?;
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
#[rustfmt::skip]
pub fn simple_map_writer(w: &mut Fmter<'_>, field: Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label: _,
    } = field;
    let PbType::SimpleMap(k, v) = ty else { panic!() };


    let (kt, vt) = (k.rust_type_no_ref(), v.rust_type_no_ref());
    let (pkt, pvt) = (k.tacky_type(), v.tacky_type());
    let mut generics = [String::new(),String::new(),String::new()];
    let mut types = [String::new(), String::new()];
    let mut value_adjust = [String::new(),String::new()];
    // massage key type into shape
    match k {
        Scalar::String => {
            generics[1] = format!("K: AsRef<str>, ");
            types[0] = format!("K");
            value_adjust[0] = "let key = key.as_ref();".into()
        }
        Scalar::Bytes => {panic!("Bytes not allowed as protobuf map key")},
        _ => {
            generics[0] = "'r, ".into();
            types[0] = format!("&'r {kt}");
            value_adjust[0] = "let key = *key;".into()
        }
    };

    // massage value type into shape
    match v {
        Scalar::String | Scalar::Bytes=> {
            generics[2] = format!("V: AsRef<{vt}>, ");
            types[1] = format!("V");
            value_adjust[1] = "let value = value.as_ref();".into()
        }
        _ => {
            generics[0] = "'r, ".into();
            types[1] = format!("&'r {vt}");
            value_adjust[1] = "let value = *value;".into()
        }
    };
    let generics = generics.concat();
    let types = format!("{}, {}", types[0], types[1]);
    //Most maps (std hashmap/btreemap/hashbrown, etc) give out (&key,&val) items as iterators
    indented!(w,r"pub fn {name}<{generics}>(&mut self, entries: impl IntoIterator<Item =({types})>) -> &mut Self {{")?;
    indented!(w,r"    let mut entry_writer = <::tacky::MapEntryWriter<'_,{pkt},{pvt}>>::new(self.tack.buffer, {number});")?;
    indented!(w,r"    for (key, value) in entries {{")?;
    indented!(w,r"        {}",value_adjust[0])?;
    indented!(w,r"        {}",value_adjust[1])?;
    indented!(w,r"        entry_writer.write_entry(key, value);")?;
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
