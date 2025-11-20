use std::fmt::{format, Write};

use crate::{
    formatter::Fmter,
    parser::{Field, Label, PbType},
};

// generate writing methods for simple scalar fields
#[rustfmt::skip]
pub fn get_writer(w: &mut Fmter<'_>, field: &Field) {
    let Field {
        name,
        number,
        ty,
        label,
    } = field;


    let ty = match ty {
        PbType::Scalar(s) => s.tacky_type().to_string(),
        PbType::Enum((name,_fields)) => name.to_string(),
        PbType::Message(m) => m.to_string(),
        PbType::SimpleMap(k,v) => {
            let (k,v) = (k.tacky_type(), v.tacky_type());
            {
                let return_type = format!("MapWriter<'_,{number},{k},{v}>");
                indented!(w, r"pub fn {name}(&mut self) -> {return_type} {{");
                indented!(w, r"    <{return_type}>::new(self.tack.buffer)");
                indented!(w, r"}}");
                return;
                
            }
        }

        PbType::Map(k,v) => {
            let k = k.tacky_type();
            let v = match &**v {
                PbType::Scalar(s) => s.tacky_type(),
                PbType::Enum((name,_fields)) => name,
                PbType::Message(m) => m,
                _ => panic!("map values cant be other maps")
            };

            {
                let return_type = format!("MapWriter<'_,{number},{k},{v}>");
                indented!(w, r"pub fn {name}(&mut self) -> {return_type} {{");
                indented!(w, r"    <{return_type}>::new(self.tack.buffer)");
                indented!(w, r"}}");
                return;
               
            }
        }
    };

    let mk_it = |s| {
        format!("{s}ValueWriter<'_,{number},{ty}>")
    };

    let return_type = match label {
        Label::Required => mk_it("Required"),
        Label::Optional => mk_it("Optional"),
        Label::Repeated => mk_it("Repeated"),
        Label::Packed =>   mk_it("Packed"),
        Label::Plain => mk_it("Plain")
    };
    indented!(w, r"pub fn {name}(&mut self) -> {return_type} {{");
    indented!(w, r"    <{return_type}>::new(self.tack.buffer)");
    indented!(w, r"}}")
}
