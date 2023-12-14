use std::{collections::{HashSet, HashMap}, fmt::Write};

use pb_rs::types::Frequency;

use crate::parser::{self, PbType, Scalar};

pub fn message_def_writer(w: &mut impl Write, name: &str) -> std::fmt::Result {
    //write struct
    writeln!(
        w,
        r#"pub struct {name}Writer<'buf> {{
    tack: ::tacky::tack::Tack<'buf>,
        }}"#
    )
}
// generate writing methods for simple scalar fields
pub fn simple_field_writer(
    w: &mut impl Write,
    field_name: &str,
    pb_type: &PbType,
    field_number: u32,
) -> std::fmt::Result {
    let tag = pb_type.tag(field_number);
    let PbType::Scalar(pb_type) = pb_type else {
        panic!()
    };
    let rust_type = pb_type.rust_type();
    let write_fn = format!("::tacky::scalars::write_{}", pb_type.as_str());
    writeln!(
        w,
        r#"pub fn {field_name}(&mut self, {field_name}: {rust_type}) -> &mut Self {{
        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);
        {write_fn}({field_name}, &mut self.tack.buffer);
        self
    }}"#
    )
}

// generate writing methods for simple scalar fields
pub fn simple_field_writer_label(
    w: &mut impl Write,
    field_name: &str,
    label: Frequency,
    pb_type: &PbType,
    field_number: u32,
) -> std::fmt::Result {
    let tag = pb_type.tag(field_number);
    let PbType::Scalar(pb_type) = pb_type else {
        panic!()
    };
    let rust_type = pb_type.rust_type();
    let write_fn = format!("::tacky::scalars::write_{}", pb_type.as_str());
    match label {
        Frequency::Optional => match pb_type {
            Scalar::String | Scalar::Bytes => {
                let rust_type = pb_type.rust_type_no_ref();
                writeln!(
                    w,
                    r#"pub fn {field_name}<'opt>(&mut self, {field_name}: impl Into<Option<&'opt {rust_type}>>) -> &mut Self {{
                        if let Some(value) = {field_name}.into() {{
                            ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);
                            {write_fn}(value, &mut self.tack.buffer);
                        }}
                    self
                }}"#
                )
            }
            _ => {
                writeln!(
                    w,
                    r#"pub fn {field_name}(&mut self, {field_name}: impl Into<Option<{rust_type}>>) -> &mut Self {{
                    if let Some(value) = {field_name}.into() {{
                        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);
                        {write_fn}(value, &mut self.tack.buffer);
                    }}
                self
            }}"#
                )
            }
        },

        Frequency::Repeated => match pb_type {
            Scalar::String | Scalar::Bytes => {
                let rust_type = pb_type.rust_type_no_ref();
                writeln!(
                    w,
                    r#"pub fn {field_name}<T: AsRef<{rust_type}>>(&mut self, {field_name}: impl IntoIterator<Item = T>) -> &mut Self {{    
                    for value in {field_name} {{    
                        let value = value.as_ref();
                        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);
                        {write_fn}(value, &mut self.tack.buffer);
                    }}
                self
            }}"#
                )
            }
            _ => {
                writeln!(
                    w,
                    r#"pub fn {field_name}<'rep>(&mut self, {field_name}: impl IntoIterator<Item = &'rep {rust_type}>) -> &mut Self {{    
                    for value in {field_name} {{
                        ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);
                        {write_fn}(value, &mut self.tack.buffer);
                    }}
                self
            }}"#
                )
            }
        },
        Frequency::Required => {
            writeln!(
                w,
                r#"pub fn {field_name}(&mut self, {field_name}: {rust_type}) -> &mut Self {{
                ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);
                {write_fn}({field_name}, &mut self.tack.buffer);
                self
            }}"#
            )
        }
    }
}

/// generate writing methods for a map whose key-values are simple scalar
/// map is exactly a "repeated" of
/// message MapEntry {
///     optional key_type key = 1;
///     optional val type val = 2;
/// }
pub fn simple_map_writer(
    w: &mut impl Write,
    field_name: &str,
    pb_type: &PbType,
    field_number: u32,
) -> std::fmt::Result {
    let PbType::Map(k, v) = pb_type else { panic!() };
    let (PbType::Scalar(k), PbType::Scalar(v)) = (k.as_ref(), v.as_ref()) else {
        panic!()
    };

    let tag = pb_type.tag(field_number);
    let key_tag = (1 << 3) | k.wire_type();
    let key_write_fn = format!("::tacky::scalars::write_{}", k.as_str());
    let key_len_fn = format!("::tacky::scalars::len_of_{}", k.as_str());
    let val_tag = (2 << 3) | v.wire_type();
    let val_write_fn = format!("::tacky::scalars::write_{}", v.as_str());
    let val_len_fn = format!("::tacky::scalars::len_of_{}", v.as_str());
    let (key_type, val_type) = (k.rust_type_no_ref(), v.rust_type_no_ref());

    writeln!(
        w,
        r#"pub fn {field_name}<'rep>(&mut self, entries: impl IntoIterator<Item =({key_type},{val_type}>)) -> &mut Self {{
            for (key, value) in entries {{
                //calc message length
                let len = 2 + {key_len_fn}(key) + {val_len_fn}(value);
                ::tacky::scalars::write_varint({tag}, &mut self.tack.buffer);
                //write message len
                ::tacky::scalars::write_varint(len as u64, &mut self.tack.buffer);
                //write key
                ::tacky::scalars::write_varint({key_tag}, &mut self.tack.buffer);
                {key_write_fn}(key, &mut self.tack.buffer);

                //write value
                ::tacky::scalars::write_varint({val_tag}, &mut self.tack.buffer);
                {val_write_fn}(value, &mut self.tack.buffer);
            }}

    }}"#
    )
}

// generate writing method for message-type fields
fn simple_message_writer(
    w: &mut impl Write,
    field_name: &str,
    pb_type: &PbType,
    field_number: u32,
) -> std::fmt::Result {
    let ty = match pb_type {
        PbType::Message(m) => m,
        _ => panic!(),
    };

    writeln!(
        w,
        r#"fn write_{field_name}(&mut self, mut {field_name}: impl FnMut({ty})) {{
            
    }}"#
    )
}

//genrate ate writing method for enum-type fields
fn simple_enum_writer(
    w: &mut impl Write,
    field_name: &str,
    pb_type: &str,
    field_number: u32,
) -> std::fmt::Result {
    todo!()
}


#[test]
fn it_works() {
    let mut buff = String::new();
    simple_field_writer(&mut buff, "field_name", &PbType::Scalar(Scalar::Sint32), 4).unwrap();
    println! {"{buff}"}
}
