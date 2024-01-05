//! Base module providing tools for working with protobuf scalars and maps where fields are scalars.
use crate::{scalars::*, tack::Tack, Width};
use bytes::BufMut;
use std::{fmt::Display, marker::PhantomData};

/// The protobuf types, as ZST markers.
pub struct Int32;
pub struct Sint32;
pub struct Int64;
pub struct Sint64;
pub struct Uint32;
pub struct Uint64;
pub struct Bool;
pub struct Fixed32;
pub struct Sfixed32;
pub struct Float;
pub struct Fixed64;
pub struct Sfixed64;
pub struct Double;
pub struct PbEnum;

// length-delimited
pub struct PbString;
pub struct PbBytes;
pub struct PbMessage;

// compound types
pub struct PbMap<K, V>(PhantomData<(K, V)>); // Map<PbString,Int32>
pub struct OneOf<O>(PhantomData<O>); // OneOf<(Field<1,Int32>,Field<3,PbString>)>

//field labels/modifiers that can be applied to the above (except maps and oneOfs)
pub struct Optional<P>(PhantomData<P>); // also applied to proto3 fields with no modifier
pub struct Repeated<P>(PhantomData<P>);
pub struct Required<P>(PhantomData<P>);
pub struct Packed<P>(PhantomData<P>);

// a complete field in a message, field number and type
pub struct Field<const N: usize, P>(PhantomData<P>);

// https://protobuf.dev/programming-guides/encoding/#structure
#[repr(usize)]
pub enum WireType {
    VARINT = 0, //	int32, int64, uint32, uint64, sint32, sint64, bool, enum
    I64 = 1,    //	fixed64, sfixed64, double
    LEN = 2,    //	string, bytes, embedded messages, packed repeated fields
    // SGROUP = 3, //	group start (deprecated)
    // EGROUP = 4, //	group end (deprecated)
    I32 = 5, //	fixed32, sfixed32, float
}

macro_rules! impl_wrapped {
    ($t:ident<$p:ident>) => {
        impl<$p> $t<$p> {
            pub fn new() -> $t<$p> {
                $t(PhantomData)
            }
        }
    };
}

impl_wrapped!(Optional<P>);
impl_wrapped!(Repeated<P>);
impl_wrapped!(Required<P>);
impl_wrapped!(Packed<P>);

impl<const N: usize, P> Field<N, P> {
    pub fn new() -> Field<N, P> {
        Field(PhantomData)
    }
}

impl<K, V> PbMap<K, V> {
    pub fn new() -> PbMap<K, V> {
        PbMap(PhantomData)
    }
}

/// actions on a scalar.
/// this is already exhaustively implemented as the types in this module contain all protobuf types.
/// public only because its needed for the codegen crate.
pub trait ProtobufScalar {
    type RustType<'a>: Copy;
    const WIRE_TYPE: usize;
    /// how to write the value itself.
    /// can also be used to write the value without tag.
    fn write_value<'a>(value: Self::RustType<'a>, buf: &mut impl BufMut);

    /// length of the value being written, exluding tag.
    fn value_len<'a>(value: Self::RustType<'a>) -> usize;
    //provided:

    /// writes the full field, tag + value
    fn write<'a>(field_nr: i32, value: Self::RustType<'a>, buf: &mut impl BufMut) {
        Self::write_tag(field_nr, buf);
        Self::write_value(value, buf);
    }
    /// len on the wire, tag + value;
    fn len<'a>(field_nr: i32, value: Self::RustType<'a>) -> usize {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as i32);
        encoded_len_varint(tag as u64) + Self::value_len(value)
    }

    /// writes just tag (field nr and wiretype combo)
    fn write_tag(field_nr: i32, buf: &mut impl BufMut) {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as i32);
        write_varint(tag as u64, buf)
    }
}

macro_rules! implscalar {
    ($t:ident, $rt:ty, $wt:expr, $f:expr, $fl:expr) => {
        impl ProtobufScalar for $t {
            type RustType<'a> = $rt;
            const WIRE_TYPE: usize = $wt as usize;
            fn write_value<'a>(value: Self::RustType<'a>, buf: &mut impl BufMut) {
                $f(value, buf)
            }
            fn value_len<'a>(value: Self::RustType<'a>) -> usize {
                $fl(value)
            }
        }
    };
}

implscalar!(Int32, i32, WireType::VARINT, write_int32, len_of_int32);
implscalar!(Sint32, i32, WireType::VARINT, write_sint32, len_of_sint32);
implscalar!(Int64, i64, WireType::VARINT, write_int64, len_of_int64);
implscalar!(Sint64, i64, WireType::VARINT, write_sint64, len_of_sint64);
implscalar!(Uint32, u32, WireType::VARINT, write_uint32, len_of_uint32);
implscalar!(Uint64, u64, WireType::VARINT, write_uint64, len_of_uint64);
implscalar!(Bool, bool, WireType::VARINT, write_bool, len_of_value);
implscalar!(Fixed32, u32, WireType::I32, write_fixed32, len_of_value);
implscalar!(Sfixed32, i32, WireType::I32, write_sfixed32, len_of_value);
implscalar!(Float, f32, WireType::I32, write_float, len_of_value);
implscalar!(Fixed64, u64, WireType::I64, write_fixed64, len_of_value);
implscalar!(Sfixed64, i64, WireType::I64, write_sfixed64, len_of_value);
implscalar!(Double, f64, WireType::I64, write_double, len_of_value);
implscalar!(
    PbString,
    &'a str,
    WireType::LEN,
    write_string,
    len_of_string
);
implscalar!(PbBytes, &'a [u8], WireType::LEN, write_bytes, len_of_bytes);

pub struct ScalarWriter<'b, const N: usize, P> {
    buf: &'b mut Vec<u8>,
    _pbtype: PhantomData<P>,
}

impl<'b, const N: usize, P: ProtobufScalar> ScalarWriter<'b, N, P> {
    pub fn new(buf: &'b mut Vec<u8>) -> Self {
        Self {
            buf,
            _pbtype: PhantomData,
        }
    }

    pub fn write_field<'a>(&mut self, value: P::RustType<'a>) {
        P::write(N as i32, value, self.buf)
    }

    pub fn write_untagged<'a>(&mut self, value: P::RustType<'a>) {
        P::write_value(value, self.buf)
    }
    pub fn write_tag<'a>(&mut self) {
        P::write_tag(N as i32, self.buf)
    }
}

impl<'b, const N: usize> ScalarWriter<'b, N, PbString> {
    /// Writes values to string via their Display impl.
    /// the max length of the string here is 127 bytes, which should cover most cases
    pub fn write_display(&mut self, d: impl Display) {
        use std::io::Write;
        let tag = ((N as i32) << 3) | (PbString::WIRE_TYPE as i32);
        let t: Tack<Width<1>> = Tack::new(self.buf, Some(tag as u32));
        write!(t.buffer, "{d}").unwrap();
    }
}

/// Writer for simple maps where the key/values are scalars
pub struct MapEntryWriter<'b, const N: usize, K, V> {
    buf: &'b mut Vec<u8>,
    _pbtype: PhantomData<(K, V)>,
}

impl<'b, const N: usize, K: ProtobufScalar, V: ProtobufScalar> MapEntryWriter<'b, N, K, V> {
    pub fn new(buf: &'b mut Vec<u8>) -> Self {
        Self {
            buf,
            _pbtype: PhantomData,
        }
    }

    pub fn write_entry<'a>(&mut self, key: K::RustType<'a>, value: V::RustType<'a>) {
        let tag = (N << 3) | 2;
        write_varint(tag as u64, self.buf);
        let len = K::len(1, key) + V::len(2, value);
        write_varint(len as u64, self.buf);
        K::write(1, key, self.buf);
        V::write(2, value, self.buf);
    }
}

pub trait MessageSchema {
    type Writer<'a>;
    fn new_writer<'a>(buffer: &'a mut Vec<u8>, tag: Option<i32>) -> Self::Writer<'a>;
}

pub struct MessageWriter<'b, const N: usize, M> {
    buf: &'b mut Vec<u8>,
    _m: PhantomData<M>,
}

impl<'b, const N: usize, M: MessageSchema> MessageWriter<'b, N, M> {
    pub fn new(buf: &'b mut Vec<u8>) -> Self {
        Self {
            buf,
            _m: PhantomData,
        }
    }
    #[cfg(feature = "prost-compat")]
    pub fn write_prost<P: prost::Message>(&mut self, m: P) -> Result<(), prost::EncodeError> {
        let tag = (N << 3) | 2;
        write_varint(tag as u64, self.buf);
        let len = m.encoded_len();
        write_varint(len as u64, self.buf);
        m.encode(self.buf)
    }
}

impl<'b, const N: usize, M: MessageSchema> MessageWriter<'b, N, M> {
    pub fn write_msg(&'b mut self, mut f: impl FnMut(M::Writer<'_>)) {
        let tag = (N << 3) | 2;
        let m = M::new_writer(self.buf, Some(tag as i32));
        f(m)
    }
}
impl<'a, 'b, const N: usize, M: MessageSchema> MessageWriter<'b, N, M> {
    pub fn close<P>(self) -> Field<N, P> {
        Field::new()
    }
}
