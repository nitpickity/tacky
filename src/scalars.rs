//! Protobuf scalar types (as zero-sized marker types) and their encoding/decoding logic.

use std::{any::type_name, marker::PhantomData};

use bytes::BufMut;

// The protobuf types, as ZST markers.
macro_rules! protobuf_types {
    ($($name:ident)*) => {
        $(
            #[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
            pub struct $name;
        )*
    };
}
protobuf_types!(
    Int32
    Sint32
    Int64
    Sint64
    Uint32
    Uint64
    Bool
    Fixed32
    Sfixed32
    Float
    Fixed64
    Sfixed64
    Double
    PbString
    PbBytes
);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct PbEnum<T>(PhantomData<T>);

/// encode/decode on a scalar.
pub trait ProtobufScalar {
    type RustType<'a>: Copy + Default + PartialEq;
    const WIRE_TYPE: WireType;
    /// how to write the value itself.
    /// can also be used to write the value without tag.
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut);

    /// length of the value being written, exluding tag.
    fn value_len(value: Self::RustType<'_>) -> usize;

    /// writes the full field, tag + value
    fn write(field_nr: u32, value: Self::RustType<'_>, buf: &mut impl BufMut) {
        Self::write_tag(field_nr, buf);
        Self::write_value(value, buf);
    }
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError>;

    /// len on the wire, tag + value;
    fn len(field_nr: u32, value: Self::RustType<'_>) -> usize {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as u32);
        encoded_len_varint(tag as u64) + Self::value_len(value)
    }

    /// writes just tag (field nr and wiretype combo)
    fn write_tag(field_nr: u32, buf: &mut impl BufMut) {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as u32);
        write_varint(tag as u64, buf)
    }
}

// Marker trait for scalars that can be packed in packed repeated fields.
pub trait Packable: ProtobufScalar {}
impl Packable for Int32 {}
impl Packable for Sint32 {}
impl Packable for Int64 {}
impl Packable for Sint64 {}
impl Packable for Uint32 {}
impl Packable for Uint64 {}
impl Packable for Bool {}
impl Packable for Fixed32 {}
impl Packable for Sfixed32 {}
impl Packable for Float {}
impl Packable for Fixed64 {}
impl Packable for Sfixed64 {}
impl Packable for Double {}
// enums are really i32s in disguise, so they can be packed too.
impl<T: Into<i32> + TryFrom<i32> + Copy + Default + PartialEq> Packable for PbEnum<T> {}

// --- impls for the scalar types ---
impl ProtobufScalar for Int32 {
    type RustType<'a> = i32;
    const WIRE_TYPE: WireType = WireType::VARINT;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(value as u64, buf);
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(value as u64)
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let v = decode_varint(buf)?;
        Ok(v as i32)
    }
}

impl ProtobufScalar for Sint32 {
    type RustType<'a> = i32;
    const WIRE_TYPE: WireType = WireType::VARINT;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(((value << 1) ^ (value >> 31)) as u32 as u64, buf);
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(((value << 1) ^ (value >> 31)) as u32 as u64)
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let v = decode_varint(buf)? as u32;
        Ok(((v >> 1) as i32) ^ (-((v & 1) as i32)))
    }
}

impl ProtobufScalar for Int64 {
    type RustType<'a> = i64;
    const WIRE_TYPE: WireType = WireType::VARINT;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(value as u64, buf);
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(value as u64)
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let v = decode_varint(buf)?;
        Ok(v as i64)
    }
}

impl ProtobufScalar for Sint64 {
    type RustType<'a> = i64;
    const WIRE_TYPE: WireType = WireType::VARINT;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(((value << 1) ^ (value >> 63)) as u64, buf);
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(((value << 1) ^ (value >> 63)) as u64)
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let v = decode_varint(buf)?;
        Ok(((v >> 1) as i64) ^ (-((v & 1) as i64)))
    }
}

impl ProtobufScalar for Uint32 {
    type RustType<'a> = u32;
    const WIRE_TYPE: WireType = WireType::VARINT;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(value as u64, buf);
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(value as u64)
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let v = decode_varint(buf)?;
        Ok(v as u32)
    }
}

impl ProtobufScalar for Uint64 {
    type RustType<'a> = u64;
    const WIRE_TYPE: WireType = WireType::VARINT;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(value, buf);
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(value)
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        decode_varint(buf)
    }
}

impl ProtobufScalar for Bool {
    type RustType<'a> = bool;
    const WIRE_TYPE: WireType = WireType::VARINT;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        buf.put_u8(value as u8);
    }

    #[inline]
    fn value_len(_value: Self::RustType<'_>) -> usize {
        1
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let v = decode_varint(buf)?;
        Ok(v != 0)
    }
}

impl ProtobufScalar for Fixed32 {
    type RustType<'a> = u32;
    const WIRE_TYPE: WireType = WireType::I32;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        buf.put_u32_le(value);
    }

    #[inline]
    fn value_len(_value: Self::RustType<'_>) -> usize {
        4
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let Some((val, rest)) = buf.split_first_chunk::<4>() else {
            return Err(DecodeError::Truncated);
        };
        let val = u32::from_le_bytes(*val);
        *buf = rest;
        Ok(val)
    }
}

impl ProtobufScalar for Sfixed32 {
    type RustType<'a> = i32;
    const WIRE_TYPE: WireType = WireType::I32;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        buf.put_i32_le(value);
    }

    #[inline]
    fn value_len(_value: Self::RustType<'_>) -> usize {
        4
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let Some((val, rest)) = buf.split_first_chunk::<4>() else {
            return Err(DecodeError::Truncated);
        };
        let val = i32::from_le_bytes(*val);
        *buf = rest;
        Ok(val)
    }
}

impl ProtobufScalar for Float {
    type RustType<'a> = f32;
    const WIRE_TYPE: WireType = WireType::I32;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        buf.put_f32_le(value);
    }

    #[inline]
    fn value_len(_value: Self::RustType<'_>) -> usize {
        4
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let Some((val, rest)) = buf.split_first_chunk::<4>() else {
            return Err(DecodeError::Truncated);
        };
        let val = f32::from_le_bytes(*val);
        *buf = rest;
        Ok(val)
    }
}

impl ProtobufScalar for Fixed64 {
    type RustType<'a> = u64;
    const WIRE_TYPE: WireType = WireType::I64;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        buf.put_u64_le(value);
    }

    #[inline]
    fn value_len(_value: Self::RustType<'_>) -> usize {
        8
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let Some((val, rest)) = buf.split_first_chunk::<8>() else {
            return Err(DecodeError::Truncated);
        };
        let val = u64::from_le_bytes(*val);
        *buf = rest;
        Ok(val)
    }
}

impl ProtobufScalar for Sfixed64 {
    type RustType<'a> = i64;
    const WIRE_TYPE: WireType = WireType::I64;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        buf.put_i64_le(value);
    }

    #[inline]
    fn value_len(_value: Self::RustType<'_>) -> usize {
        8
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let Some((val, rest)) = buf.split_first_chunk::<8>() else {
            return Err(DecodeError::Truncated);
        };
        let val = i64::from_le_bytes(*val);
        *buf = rest;
        Ok(val)
    }
}

impl ProtobufScalar for Double {
    type RustType<'a> = f64;
    const WIRE_TYPE: WireType = WireType::I64;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        buf.put_f64_le(value);
    }

    #[inline]
    fn value_len(_value: Self::RustType<'_>) -> usize {
        8
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let Some((val, rest)) = buf.split_first_chunk::<8>() else {
            return Err(DecodeError::Truncated);
        };
        let val = f64::from_le_bytes(*val);
        *buf = rest;
        Ok(val)
    }
}

impl ProtobufScalar for PbString {
    type RustType<'a> = &'a str;
    const WIRE_TYPE: WireType = WireType::LEN;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(value.len() as u64, buf);
        buf.put(value.as_bytes());
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(value.len() as u64) + value.len()
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let bytes = decode_len(buf)?;
        let s = std::str::from_utf8(bytes)?;
        Ok(s)
    }
}

impl ProtobufScalar for PbBytes {
    type RustType<'a> = &'a [u8];
    const WIRE_TYPE: WireType = WireType::LEN;

    #[inline]
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(value.len() as u64, buf);
        buf.put(value);
    }

    #[inline]
    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(value.len() as u64) + value.len()
    }

    #[inline]
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        decode_len(buf)
    }
}

impl<T: Copy + Into<i32> + TryFrom<i32> + Default + PartialEq> ProtobufScalar for PbEnum<T> {
    type RustType<'a> = T;

    const WIRE_TYPE: WireType = WireType::VARINT;

    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut) {
        write_varint(value.into() as u64, buf);
    }

    fn value_len(value: Self::RustType<'_>) -> usize {
        encoded_len_varint(value.into() as u64)
    }

    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError> {
        let n = decode_varint(buf)? as i32;
        T::try_from(n).map_err(|_| DecodeError::InvalidEnumValue {
            field: type_name::<T>(),
            value: n,
        })
    }
}
// https://protobuf.dev/programming-guides/encoding/#structure
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WireType {
    VARINT = 0, //	int32, int64, uint32, uint64, sint32, sint64, bool, enum
    I64 = 1,    //	fixed64, sfixed64, double
    LEN = 2,    //	string, bytes, embedded messages, packed repeated fields
    // SGROUP = 3, //	group start (deprecated)
    // EGROUP = 4, //	group end (deprecated)
    I32 = 5, //	fixed32, sfixed32, float
}

#[derive(Debug)]
pub enum DecodeError {
    Truncated,
    InvalidWireType(u32),
    WireTypeMismatch {
        field: &'static str,
        expected: WireType,
        actual: WireType,
    },
    InvalidUtf8,
    InvalidEnumValue {
        field: &'static str,
        value: i32,
    },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Truncated => f.write_str("unexpected end of input"),
            DecodeError::InvalidWireType(wt) => write!(f, "invalid wire type: {wt}"),
            DecodeError::WireTypeMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "wire type mismatch for field \"{field}\": expected {expected:?}, got {actual:?}"
            ),
            DecodeError::InvalidUtf8 => f.write_str("invalid UTF-8 in string field"),
            DecodeError::InvalidEnumValue { field, value } => {
                write!(f, "invalid enum value {value} for field \"{field}\"")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<core::str::Utf8Error> for DecodeError {
    fn from(_: core::str::Utf8Error) -> Self {
        DecodeError::InvalidUtf8
    }
}

#[inline]
pub fn decode_key(buf: &mut &[u8]) -> Result<(u32, WireType), DecodeError> {
    let v = decode_varint(buf)?;
    let tag = (v >> 3) as u32;
    let wire = (v & 0x07) as u32;
    let wire_type = match wire {
        0 => WireType::VARINT,
        1 => WireType::I64,
        2 => WireType::LEN,
        5 => WireType::I32,
        other => return Err(DecodeError::InvalidWireType(other)),
    };
    Ok((tag, wire_type))
}

#[inline]
pub fn check_wire_type(
    actual: WireType,
    expected: WireType,
    field: &'static str,
) -> Result<(), DecodeError> {
    if actual != expected {
        return Err(DecodeError::WireTypeMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

/// Skip an unknown field value based on wire type.
#[cold]
pub fn skip_field(wire_type: WireType, buf: &mut &[u8]) -> Result<(), DecodeError> {
    match wire_type {
        WireType::VARINT => skip_varint(buf)?,
        WireType::I64 => {
            if buf.len() < 8 {
                return Err(DecodeError::Truncated);
            }
            *buf = &buf[8..];
        }
        WireType::LEN => {
            decode_len(buf)?;
        }
        WireType::I32 => {
            if buf.len() < 4 {
                return Err(DecodeError::Truncated);
            }
            *buf = &buf[4..];
        }
    }
    Ok(())
}

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
    ((((value | 1).leading_zeros() ^ 63) * 9 + 73) / 64) as usize
}

#[inline]
pub fn decode_varint(buf: &mut &[u8]) -> Result<u64, DecodeError> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        let &b = buf.first().ok_or(DecodeError::Truncated)?;
        *buf = &buf[1..];
        result |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 64 {
            return Err(DecodeError::Truncated);
        }
    }
}

#[inline]
pub fn skip_varint(buf: &mut &[u8]) -> Result<(), DecodeError> {
    if buf.len() >= 8 {
        let word = u64::from_le_bytes(buf[..8].try_into().unwrap());
        let msbs = !word & 0x8080808080808080;
        if msbs != 0 {
            *buf = &buf[(msbs.trailing_zeros() / 8 + 1) as usize..];
            return Ok(());
        }
        if buf[8] & 0x80 == 0 {
            *buf = &buf[9..];
            return Ok(());
        }
        if buf[9] & 0x80 == 0 {
            *buf = &buf[10..];
            return Ok(());
        }
        return Err(DecodeError::Truncated);
    }
    let len = buf
        .iter()
        .position(|&b| b & 0x80 == 0)
        .map(|i| i + 1)
        .ok_or(DecodeError::Truncated)?;
    *buf = &buf[len..];
    Ok(())
}

/// Decode a length-delimited field, returning a sub-slice of the input.
#[inline]
pub fn decode_len<'a>(buf: &mut &'a [u8]) -> Result<&'a [u8], DecodeError> {
    let len = decode_varint(buf)? as usize;
    if buf.len() < len {
        return Err(DecodeError::Truncated);
    }
    let (data, rest) = buf.split_at(len);
    *buf = rest;
    Ok(data)
}
