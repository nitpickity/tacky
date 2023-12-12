use std::{collections::HashSet, fmt::Write};

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

// generate writing methods for a map whose key-values are simple scalar
pub fn simple_map_writer(
    w: &mut impl Write,
    field_name: &str,
    pb_type: &PbType,
    field_number: u32,
) -> std::fmt::Result {
    let tag = pb_type.tag(field_number);
    let (key_type, val_type) = match pb_type {
        PbType::Map(k, v) => match (k.as_ref(), v.as_ref()) {
            (PbType::Scalar(ref k), PbType::Scalar(v)) => (k.rust_type(), v.rust_type()),
            _ => panic!(),
        },
        _ => panic!(),
    };

    writeln!(
        w,
        r#"pub fn {field_name}(&mut self, (key,value): ({key_type},{val_type})) -> &mut Self {{
            todo!()
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
