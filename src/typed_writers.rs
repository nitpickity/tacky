use std::{marker::PhantomData, fmt::Display};
use crate::scalars::*;
use bytes::BufMut;


pub trait ProtobufScalar {
    type RustType<'a>;
    const WIRE_TYPE: usize;
    //writes just the value, minus the tag. can be useful.
    fn write_value<'a>(value: Self::RustType<'a>, buf:&mut impl BufMut);
    // writes tag (field nr and wiretype combo)
    fn write_tag(field_nr: i32, buf:&mut impl BufMut) {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as i32);
        write_varint(tag as u64, buf)
    }
    //writes the full thing, tag + value
    fn write<'a>(field_nr: i32, value: Self::RustType<'a>, buf: &mut impl BufMut) {
        Self::write_tag(field_nr, buf);
        Self::write_value(value, buf);
    }

    fn value_len<'a>(value: &Self::RustType<'a>) -> usize;
    fn len<'a>(field_nr: i32, value: &Self::RustType<'a>) -> usize {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as i32);
        encoded_len_varint(tag as u64) + Self::value_len(value)
    }
}

macro_rules! implscalar {
    ($t:ident, $rt:ty, $wt:expr, $f:expr, $fl:expr) => {
        pub struct $t;
        impl ProtobufScalar for $t {
            type RustType<'a> = $rt;
            const WIRE_TYPE: usize = $wt;
            fn write_value<'a>(value: Self::RustType<'a>, buf:&mut impl BufMut) {
                $f(value, buf)
            }
            fn value_len<'a>(value: &Self::RustType<'a>) -> usize {
                $fl(*value)
            }
        }
    };
}

struct MapEntryWriter<'b,K,V> {
    buf: &'b mut Vec<u8>,
    field_nr: i32,
    _pbtype: PhantomData<(K,V)>
}

impl<'b, K: ProtobufScalar,V:ProtobufScalar> MapEntryWriter<'b, K,V> {
    pub fn new(buf: &'b mut Vec<u8>, field_nr: i32) -> Self {
        Self {
            buf,
            field_nr,
            _pbtype: PhantomData,
        }
    }

    pub fn write_entry<'a>(&mut self, key: K::RustType<'a>, value: V::RustType<'a> ) {
        let tag = (self.field_nr << 3) | 2;
        write_varint(tag as u64, self.buf);
        let len = K::len(1, &key) + V::len(2, &value);
        write_varint(len as u64, self.buf);
        K::write(1, key, self.buf);
        V::write(2, value, self.buf);
    }
}
implscalar!(Int32, i32, 0, write_int32, len_of_int32);
implscalar!(Sint32, i32, 0, write_sint32, len_of_sint32);
implscalar!(Int64, i64, 0, write_int64, len_of_int64);
implscalar!(Sint64, i64, 0, write_sint64,len_of_sint64);
implscalar!(Uint32, u32, 0, write_uint32,len_of_uint32);
implscalar!(Uint64, u64, 0, write_uint64,len_of_uint64);
implscalar!(Bool, bool, 0, write_bool,len_of_bool);
implscalar!(Fixed32, u32, 5, write_fixed32,len_of_fixed32);
implscalar!(Sfixed32, i32, 5, write_sfixed32,len_of_sfixed32);
implscalar!(Float, f32, 5, write_float,len_of_float);
implscalar!(Fixed64, u64, 1, write_fixed64,len_of_fixed64);
implscalar!(Sfixed64, i64, 1, write_sfixed64,len_of_sfixed64);
implscalar!(Double, f64, 1, write_double,len_of_double);
implscalar!(PbString, &'a str, 2, write_string,len_of_string);
implscalar!(PbBytes, &'a [u8], 2, write_bytes, len_of_bytes);

pub struct ScalarWriter<'b,P> {
    buf: &'b mut Vec<u8>,
    field_nr: i32,
    _pbtype: PhantomData<P>
}

impl<'b, P:ProtobufScalar> ScalarWriter<'b,P> {
    pub fn new(buf: &'b mut Vec<u8>, field_nr: i32) -> Self {
        Self {
            buf,
            field_nr,
            _pbtype: PhantomData,
        }
    }

    pub fn write_field<'a>(&mut self, value: P::RustType<'a> ) {
        P::write(self.field_nr, value, self.buf)
    }

    pub fn write_untagged<'a>(&mut self, value: P::RustType<'a> ) {
        P::write_value(value, self.buf)
    }
}

impl<'b,> ScalarWriter<'b,PbString> {
    pub fn write_display(&mut self, d: impl Display) {
        use std::io::Write;
        write!(self.buf, "{d}").unwrap();
    }
}

fn wat() {
    let mut buf = Vec::new();
    let p = ScalarWriter::<'_,PbString>::new(&mut buf, 42).write_field("hello");
}