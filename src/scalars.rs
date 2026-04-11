//! Zero-sized marker types for protobuf scalars, plus wire format encoding and decoding.
//!
//! Each protobuf scalar type (int32, string, etc.) is represented by a ZST marker struct.
//! These carry no data — they exist so that [`Field`](`crate::Field`) can be generic over
//! the protobuf type and dispatch to the correct encoding at compile time via
//! the [`ProtobufScalar`] trait.

use std::marker::PhantomData;

use bytes::BufMut;

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

/// Constraint for types that can be used as protobuf enums.
/// Protobuf enums are i32 on the wire, so this requires bidirectional i32 conversion.
/// Generated enum types implement this automatically.
pub trait PbEnumType: Copy + Into<i32> + From<i32> + Default + PartialEq {}
impl<T: Copy + Into<i32> + From<i32> + Default + PartialEq> PbEnumType for T {}

/// ZST marker for protobuf enum fields, generic over the generated Rust enum type.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct PbEnum<T>(PhantomData<T>);

/// Defines how a protobuf scalar type is encoded and decoded on the wire.
///
/// Each scalar marker ([`Int32`], [`PbString`], etc.) implements this trait,
/// mapping it to a Rust type, a wire type, and the encoding/decoding logic.
/// This is the trait that [`Field`](`crate::Field`) dispatches on — adding a new
/// scalar type to tacky means implementing `ProtobufScalar` for a new ZST.
pub trait ProtobufScalar {
    /// The Rust type this scalar maps to. Lifetime-parameterized so that
    /// borrowed types like `&str` and `&[u8]` can be returned during decoding.
    type RustType<'a>: Copy + Default + PartialEq;
    const WIRE_TYPE: WireType;
    /// Encoded size per element for fixed-width wire types (f32, f64, fixed32, etc.).
    /// `None` for varints whose encoded size depends on the value.
    /// Used by packed field `write_exact` to bypass the Tack when the total
    /// length can be computed upfront as `count * fixed_size`.
    const FIXED_WIRE_SIZE: Option<usize> = None;
    /// Writes just the value bytes (no tag). Used by [`Field`](`crate::Field`) after
    /// writing the tag separately.
    fn write_value(value: Self::RustType<'_>, buf: &mut impl BufMut);
    /// Encoded length of the value bytes, excluding tag.
    fn value_len(value: Self::RustType<'_>) -> usize;
    /// Writes a complete field (tag + value). Convenience method used by map entry encoding
    /// where the tag isn't precomputed via [`EncodedTag`].
    fn write(field_nr: u32, value: Self::RustType<'_>, buf: &mut impl BufMut) {
        Self::write_tag(field_nr, buf);
        Self::write_value(value, buf);
    }
    /// Reads one value from the buffer, advancing the cursor past it.
    fn read<'a>(buf: &mut &'a [u8]) -> Result<Self::RustType<'a>, DecodeError>;
    /// Total wire length of a field (tag + value). Used for map entries
    /// where the entry length must be known before writing.
    fn len(field_nr: u32, value: Self::RustType<'_>) -> usize {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as u32);
        encoded_len_varint(tag as u64) + Self::value_len(value)
    }
    fn write_tag(field_nr: u32, buf: &mut impl BufMut) {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as u32);
        write_varint(tag as u64, buf)
    }
}

/// Marker for scalars that can appear in `packed` repeated fields.
/// All numeric types and enums are packable. Strings and bytes are not —
/// protobuf's wire format doesn't support packing length-delimited types.
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
impl<T: PbEnumType> Packable for PbEnum<T> {}

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
    const FIXED_WIRE_SIZE: Option<usize> = Some(4);

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
    const FIXED_WIRE_SIZE: Option<usize> = Some(4);

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
    const FIXED_WIRE_SIZE: Option<usize> = Some(4);

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
    const FIXED_WIRE_SIZE: Option<usize> = Some(8);

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
    const FIXED_WIRE_SIZE: Option<usize> = Some(8);

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
    const FIXED_WIRE_SIZE: Option<usize> = Some(8);

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

impl<T: PbEnumType> ProtobufScalar for PbEnum<T> {
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
        Ok(T::from(n))
    }
}
/// Protobuf wire types. The wire type tells the decoder how many bytes a field
/// value occupies, allowing unknown fields to be skipped without understanding
/// their schema.
///
/// See <https://protobuf.dev/programming-guides/encoding/#structure>.
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
    InvalidMapEntry,
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
            DecodeError::InvalidMapEntry => {
                write!(f, "invalid map entry, tag isnt 1 or 2")
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

/// Decodes a field key into its field number and wire type.
/// Every protobuf field on the wire starts with this key.
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

/// Advances the cursor past an unknown field value based on its wire type.
/// Used by generated deserializers to skip fields not recognized by the schema,
/// enabling forward compatibility.
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
    if buf.len() >= 10 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_varint_panics_on_8_byte_buffer() {
        // 8 bytes, all with continuation bit set — a truncated varint.
        // This should return Truncated, not panic on out-of-bounds buf[8].
        let data = [0x80u8; 8];
        let mut buf: &[u8] = &data;
        let result = skip_varint(&mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_skip_varint_panics_on_9_byte_buffer() {
        // 9 bytes, all with continuation bit set — a truncated varint.
        // This should return Truncated, not panic on out-of-bounds buf[9].
        let data = [0x80u8; 9];
        let mut buf: &[u8] = &data;
        let result = skip_varint(&mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_skip_varint_9_byte_valid() {
        // 8 continuation bytes + 1 terminating byte = valid 9-byte varint
        let mut data = [0x80u8; 9];
        data[8] = 0x01; // terminating byte
        let mut buf: &[u8] = &data;
        let result = skip_varint(&mut buf);
        assert!(result.is_ok());
        assert!(buf.is_empty());
    }

    #[test]
    fn test_skip_varint_10_byte_valid() {
        // 9 continuation bytes + 1 terminating byte = valid 10-byte varint (max)
        let mut data = [0x80u8; 10];
        data[9] = 0x01;
        let mut buf: &[u8] = &data;
        let result = skip_varint(&mut buf);
        assert!(result.is_ok());
        assert!(buf.is_empty());
    }
}

/// A field tag (field number + wire type) pre-encoded as varint bytes.
///
/// Used via `const { EncodedTag::new(N, P::WIRE_TYPE) }` in generated code so
/// the varint encoding happens at compile time. At runtime, writing a tag is
/// just a memcpy of 1-2 bytes. This matters in tight loops over repeated fields
/// where the tag is written once per element.
pub struct EncodedTag {
    bytes: [u8; 5],
    len: u8,
}

impl EncodedTag {
    #[inline]
    pub const fn new(field_nr: u32, wire_type: WireType) -> Self {
        let mut tag = (field_nr << 3) | (wire_type as u32);
        let mut bytes = [0u8; 5];
        let mut i = 0;
        loop {
            if tag < 0x80 {
                bytes[i] = tag as u8;
                i += 1;
                break;
            }
            bytes[i] = ((tag & 0x7F) | 0x80) as u8;
            tag >>= 7;
            i += 1;
        }
        EncodedTag {
            bytes,
            len: i as u8,
        }
    }

    #[inline]
    pub fn write(&self, buf: &mut impl BufMut) {
        buf.put_slice(&self.bytes[..self.len as usize]);
    }
}

/// Reads a length prefix and returns that many bytes as a sub-slice, advancing the cursor.
/// This is the building block for decoding strings, bytes, nested messages, packed fields,
/// and map entries — anything with wire type LEN.
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
