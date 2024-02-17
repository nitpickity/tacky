use core::panic;
use std::fmt::Write;

use crate::{
    formatter::Fmter,
    parser::{Field, Label, PbType, Scalar},
};

#[rustfmt::skip]
pub fn message_builder(w: &mut Fmter<'_>, name: &str, fields: &[Field] ) -> std::fmt::Result {
    //write struct
    indented!(w, r"pub struct {name}Builder<'buf> {{")?;
    indented!(w, r"   tack: Tack<'buf>")?;
    indented!(w, r"}}")?;
    indented!(w)?;
    indented!(w, r"impl<'buf> {name}Builder<'buf> {{")?;
    w.indent();
    indented!(w, r"pub fn new(buf: &'buf mut Vec<u8>, tag: Option<u32>) -> Self {{")?;
    indented!(w, r"    Self {{tack: Tack::new(buf, tag)}}")?;
    indented!(w, r"}}")?;
        for f in fields {
            match f.ty {
                PbType::SimpleMap(_, _) => {
                    simple_map_build(w, &f).unwrap();
                }
                PbType::Scalar(_) => {
                    simple_field_build(w, &f).unwrap();
                }
                PbType::Message(_) => {
                    // the closure based API for nested messages is already generated
                    simple_message_build(w, &f).unwrap()
                }
                PbType::Enum(_) => simple_enum_build(w, &f).unwrap(),
                _ => todo!(),
            }
        }
    w.unindent();
    indented!(w, r"}}")?;
    indented!(w)
}

// #[rustfmt::skip]
// pub fn field_witness_type(w: &mut Fmter<'_>, field: &Field) -> std::fmt::Result {
//     let Field {
//         name,
//         number,
//         ty,
//         label,
//     } = field;
//     let mut wrap_label = |l: &str| match label {
//         Label::Required => indented!(w, "pub {name}: Field<{number}, Required<{l}>>,"),
//         Label::Optional => indented!(w, "pub {name}: Field<{number}, Optional<{l}>>,"),
//         Label::Repeated => indented!(w, "pub {name}: Field<{number}, Repeated<{l}>>,"),
//         Label::Packed => indented!(w, "pub {name}: Field<{number}, Packed<{l}>>,"),
//         Label::Plain => indented!(w, "pub {name}: Field<{number}, Plain<{l}>>,")
//     };
//     match ty {
//         PbType::Scalar(p) => wrap_label(p.tacky_type()),
//         PbType::SimpleMap(k, v) => indented!(w,"pub {name}: Field<{number}, PbMap<{}, {}>>,",k.tacky_type(), v.tacky_type()),
//         PbType::Message(m) => wrap_label(&format!("PbMessage<{m}>")),
//         PbType::Enum(e) => wrap_label("PbEnum"),
//         PbType::Map(_, _) => todo!(),
//     }
// }
// generate writing methods for simple scalar fields
#[rustfmt::skip]
pub fn simple_field_build(w: &mut Fmter<'_>, field: &Field) -> std::fmt::Result {
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
            let witness_type = format!("Field<{number},Optional<{tacky_type}>>");
            let (item, generics, value) = match pb_type {
                Scalar::String | Scalar::Bytes => {
                    ("T", format!("<T: AsRef<{rust_type}>>"), "value.as_ref()")
                }
                _ => (rust_type, "".into(), "value"),
            };
            let write_expr = mk_write_expr(value);
            indented!(w,r"pub fn {name}{generics}(mut self, {name}: Option<{item}>) -> Self {{")?;
            indented!(w, r"    if let Some(value) = {name}{{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"    }}")?;
            indented!(w, r"     self")?;
            indented!(w, r"}}")
        }

        Label::Repeated => {
            let witness_type = format!("Field<{number},Repeated<{tacky_type}>>");
            let (item, generics, value) = match pb_type {
                Scalar::String | Scalar::Bytes => {
                    ("T", format!("<T: AsRef<{rust_type}>>"), "value.as_ref()")
                }
                _ => (rust_type, "".into(), "value"),
            };
            let write_expr = mk_write_expr(value);
            indented!(w,r"pub fn {name}{generics}(mut self, {name}: impl IntoIterator<Item = {item}>) -> Self {{")?;
            indented!(w, r"    for value in {name} {{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"    }}")?;
            indented!(w, r"     self")?;
            indented!(w, r"}}")
        }
        Label::Required => {
            let witness_type = format!("Field<{number},Required<{tacky_type}>>");
            let write_expr = mk_write_expr(&name);
            indented!(w,r"pub fn {name}(mut self, {name}: {rust_type}) -> Self {{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"   self")?;
            indented!(w, r"}}")
        }
        Label::Packed => {
            // encoded using the same wide-varint approach as nested messages. this means that we "waste" a little (3 bytes at worst) space but skip iterating twice.
            let witness_type = format!("Field<{number}, Packed<{tacky_type}>>");
            let tag = (number << 3) | 2; // wire type 2, length delimited
            indented!(w, r"pub fn {name}<'rep>(mut self, {name}: impl IntoIterator<Item = &'rep {rust_type}>)-> Self {{")?;
            indented!(w, r"    let tack = Tack::new_with_width(self.tack.buffer, Some({tag}), 2);")?;
            indented!(w, r"    for value in {name} {{")?;
            indented!(w, r"        {write_fn}(*value, tack.buffer);")?;
            indented!(w, r"    }}")?;
            indented!(w, r"    drop(tack);")?;
            indented!(w, r"   self")?;
            indented!(w, r"}}")
        }
        Label::Plain => {
            let witness_type = format!("Field<{number},Plain<{tacky_type}>>");
            let write_expr = mk_write_expr(&name);
            indented!(w,r"pub fn {name}(mut self, {name}: {rust_type})-> Self {{")?;
            indented!(w, r"        if {name} != Default::default() {{")?;
            indented!(w, r"            {write_expr}")?;
            indented!(w, r"        }}")?;
            indented!(w, r"   self")?;
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
pub fn simple_map_build(w: &mut Fmter<'_>, field: &Field) -> std::fmt::Result {
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
            types[0] = format!("{kt}");
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
            types[1] = format!("{vt}");
        }
    };
    let generics = generics.concat();
    let types = format!("{}, {}", types[0], types[1]);
    let witness_type = format!("Field<{number}, PbMap<{pkt},{pvt}>>");
    //Most maps (std hashmap/btreemap/hashbrown, etc) give out (&key,&val) items as iterators
    indented!(w,r"pub fn {name}<{generics}>(mut self, entries: impl IntoIterator<Item =({types})>)-> Self {{")?;
    indented!(w,r"    let mut entry_writer = <::tacky::MapEntryWriter<'_,{number},{pkt},{pvt}>>::new(self.tack.buffer);")?;
    indented!(w,r"    for (key, value) in entries {{")?;
    indented!(w,r"        {}",value_adjust[0])?;
    indented!(w,r"        {}",value_adjust[1])?;
    indented!(w,r"        entry_writer.write_entry(key, value);")?;
    indented!(w,r"    }}")?;
    indented!(w,r"   self")?;
    indented!(w,r"}}")
}

// generate writing method for message-type fields
#[rustfmt::skip]
pub fn simple_message_build(
    w: &mut Fmter,
    field: &Field,
) -> std::fmt::Result {
    let Field { name, number, ty, label } = field;
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
    let witness_type = wrap_label(&format!("PbMessage<{ty}>"));
    indented!(w,r"pub fn {name}(mut self, mut {name}: impl FnMut({ty}Builder))-> Self {{ ")?;
    indented!(w,r"    let writer = {ty}Builder::new(&mut self.tack.buffer,Some({tag}));")?;
    indented!(w,r"    {name}(writer);")?;
    indented!(w,r"   self")?;
    indented!(w,r"}}")
}

// genrate ate writing method for enum-type fields
// enums are just i32s, so we take anything thats Into<i32>.
// generate writing methods for simple scalar fields
#[rustfmt::skip]
pub fn simple_enum_build(w: &mut Fmter<'_>, field: &Field) -> std::fmt::Result {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;
    let PbType::Enum((ename, valid)) = ty else {
        panic!("expected enum type")
    };
    let rust_type ="i32";
    let tacky_type = "PbEnum";
    let mk_write_expr =
        |arg| format!("Int32::write({number}, {arg}, &mut self.tack.buffer);");
    match label {
        Label::Optional => {
            let witness_type = format!("Field<{number},Optional<{tacky_type}>>");
            let write_expr = mk_write_expr("value");
            indented!(w,r"pub fn {name}(mut self, {name}: Option<i32>)-> Self {{")?;
            indented!(w, r"    if let Some(value) = {name}{{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"    }}")?;
            indented!(w, r"    self")?;
            indented!(w, r"}}")
        }

        Label::Repeated => {
            let witness_type = format!("Field<{number},Repeated<{tacky_type}>>");
            let write_expr = mk_write_expr("value");
            indented!(w,r"pub fn {name}(mut self, {name}: impl IntoIterator<Item = {rust_type}>)-> Self {{")?;
            indented!(w, r"    for value in {name} {{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"    }}")?;
            indented!(w, r"    self")?;
            indented!(w, r"}}")
        }
        Label::Required => {
            let witness_type = format!("Field<{number},Required<{tacky_type}>>");
            let write_expr = mk_write_expr(&name);
            indented!(w,r"pub fn {name}(mut self, {name}: {rust_type})-> Self {{")?;
            indented!(w, r"        {write_expr}")?;
            indented!(w, r"   self")?;
            indented!(w, r"}}")
        }
        Label::Packed => {
            panic!("enums cant be packed")
        }
        Label::Plain => {
            let witness_type = format!("Field<{number},Plain<{tacky_type}>>");
            let write_expr = mk_write_expr(&name);
            indented!(w,r"pub fn {name}(mut self, {name}: {rust_type})-> Self {{")?;
            indented!(w, r"        if {name} != Default::default() {{")?;
            indented!(w, r"            {write_expr}")?;
            indented!(w, r"        }}")?;
            indented!(w, r"   self")?;
            indented!(w, r"}}")
        }
    }
}
