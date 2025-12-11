use bytes::BufMut;

/// The protobuf types, as ZST markers.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Int32;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Sint32;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Int64;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Sint64;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Uint32;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Uint64;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Bool;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Fixed32;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Sfixed32;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Float;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Fixed64;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Sfixed64;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Double;

// length-delimited
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct PbString;
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct PbBytes;

#[inline]
pub fn write_varint(mut value: u64, buf: &mut impl BufMut) {
    loop {
        if value < 0x80 {
            buf.put_u8(value as u8);
            break;
        } else {
            buf.put_u8(((value & 0x7F) | 0x80) as u8);
            value >>= 7;
        }
    }
}

#[inline]
pub const fn encoded_len_varint(value: u64) -> usize {
    // Based on [VarintSize64][1].
    // [1]: https://github.com/google/protobuf/blob/3.3.x/src/google/protobuf/io/coded_stream.h#L1301-L1309
    ((((value | 1).leading_zeros() ^ 63) * 9 + 73) / 64) as usize
}

#[inline]
pub const fn encode_zigzag32(n: i32) -> u32 {
    ((n << 1) ^ (n >> 31)) as u32
}
#[inline]
pub const fn encode_zigzag64(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}
#[inline]
pub fn write_double(value: f64, buf: &mut impl BufMut) {
    buf.put_f64_le(value);
}
#[inline]
pub fn write_float(value: f32, buf: &mut impl BufMut) {
    buf.put_f32_le(value);
}
#[inline]
pub fn write_int32(value: i32, buf: &mut impl BufMut) {
    write_varint(value as u64, buf);
}
#[inline]
pub fn write_int64(value: i64, buf: &mut impl BufMut) {
    write_varint(value as u64, buf);
}
#[inline]
pub fn write_uint32(value: u32, buf: &mut impl BufMut) {
    write_varint(value as u64, buf);
}
#[inline]
pub fn write_uint64(value: u64, buf: &mut impl BufMut) {
    write_varint(value, buf);
}
#[inline]
pub fn write_sint32(value: i32, buf: &mut impl BufMut) {
    write_varint(encode_zigzag32(value) as u64, buf);
}
#[inline]
pub fn write_sint64(value: i64, buf: &mut impl BufMut) {
    write_varint(encode_zigzag64(value), buf);
}
#[inline]
pub fn write_fixed32(value: u32, buf: &mut impl BufMut) {
    buf.put_u32_le(value);
}
#[inline]
pub fn write_fixed64(value: u64, buf: &mut impl BufMut) {
    buf.put_u64_le(value);
}
#[inline]
pub fn write_sfixed32(value: i32, buf: &mut impl BufMut) {
    buf.put_i32_le(value);
}
#[inline]
pub fn write_sfixed64(value: i64, buf: &mut impl BufMut) {
    buf.put_i64_le(value);
}
#[inline]
pub fn write_bytes(value: &[u8], buf: &mut impl BufMut) {
    write_varint(value.len() as u64, buf);
    buf.put(value);
}
#[inline]
pub fn write_string(value: &str, buf: &mut impl BufMut) {
    write_bytes(value.as_bytes(), buf);
}
#[inline]
pub fn write_bool(value: bool, buf: &mut impl BufMut) {
    buf.put_u8(value as u8);
}

// lengths
#[inline]
pub const fn len_of_value<T: Copy>(_: T) -> usize {
    std::mem::size_of::<T>()
}
#[inline]
pub const fn len_of_string(value: &str) -> usize {
    encoded_len_varint(value.len() as u64) + value.len()
}
#[inline]
pub const fn len_of_bytes(value: &[u8]) -> usize {
    encoded_len_varint(value.len() as u64) + value.len()
}
#[inline]
pub const fn len_of_int32(value: i32) -> usize {
    encoded_len_varint(value as u64)
}
#[inline]
pub const fn len_of_int64(value: i64) -> usize {
    encoded_len_varint(value as u64)
}
#[inline]
pub const fn len_of_uint32(value: u32) -> usize {
    encoded_len_varint(value as u64)
}
#[inline]
pub const fn len_of_uint64(value: u64) -> usize {
    encoded_len_varint(value)
}
#[inline]
pub const fn len_of_sint32(value: i32) -> usize {
    encoded_len_varint(((value << 1) ^ (value >> 31)) as u64)
}
#[inline]
pub const fn len_of_sint64(value: i64) -> usize {
    encoded_len_varint(((value << 1) ^ (value >> 63)) as u64)
}

/// actions on a scalar.
/// this is already exhaustively implemented as the types in this module contain all protobuf types.
/// public only because its needed for the codegen crate.
pub trait ProtobufScalar {
    type RustType<'a>: Copy;
    const WIRE_TYPE: WireType;
    /// how to write the value itself.
    /// can also be used to write the value without tag.
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut);

    /// length of the value being written, exluding tag.
    fn value_len(value: Self::RustType<'_>) -> usize;
    //provided:

    /// writes the full field, tag + value
    fn write(field_nr: i32, value: Self::RustType<'_>, buf: &mut impl BufMut) {
        Self::write_tag(field_nr, buf);
        Self::write_value(value, buf);
    }
    /// len on the wire, tag + value;
    fn len(field_nr: i32, value: Self::RustType<'_>) -> usize {
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
            const WIRE_TYPE: WireType = $wt;
            fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
                $f(value, buf)
            }
            fn value_len(value: Self::RustType<'_>) -> usize {
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

// https://protobuf.dev/programming-guides/encoding/#structure
#[repr(usize)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WireType {
    VARINT = 0, //	int32, int64, uint32, uint64, sint32, sint64, bool, enum
    I64 = 1,    //	fixed64, sfixed64, double
    LEN = 2,    //	string, bytes, embedded messages, packed repeated fields
    // SGROUP = 3, //	group start (deprecated)
    // EGROUP = 4, //	group end (deprecated)
    I32 = 5, //	fixed32, sfixed32, float
}
