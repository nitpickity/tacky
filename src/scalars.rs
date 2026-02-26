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

// --- Decode support ---

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
pub const fn decode_zigzag32(n: u32) -> i32 {
    ((n >> 1) as i32) ^ (-((n & 1) as i32))
}

#[inline]
pub const fn decode_zigzag64(n: u64) -> i64 {
    ((n >> 1) as i64) ^ (-((n & 1) as i64))
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
#[inline]
pub fn skip_field(wire_type: WireType, buf: &mut &[u8]) -> Result<(), DecodeError> {
    match wire_type {
        WireType::VARINT => {
            decode_varint(buf)?;
        }
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

// --- Packed iterators ---

/// Zero-copy iterator over packed varint-encoded values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PackedVarints<'a>(pub &'a [u8]);

impl<'a> PackedVarints<'a> {
    pub fn int32s(self) -> impl Iterator<Item = Result<i32, DecodeError>> + 'a {
        self.map(|r| r.map(|v| v as i32))
    }
    pub fn sint32s(self) -> impl Iterator<Item = Result<i32, DecodeError>> + 'a {
        self.map(|r| r.map(|v| decode_zigzag32(v as u32)))
    }
    pub fn int64s(self) -> impl Iterator<Item = Result<i64, DecodeError>> + 'a {
        self.map(|r| r.map(|v| v as i64))
    }
    pub fn sint64s(self) -> impl Iterator<Item = Result<i64, DecodeError>> + 'a {
        self.map(|r| r.map(decode_zigzag64))
    }
    pub fn uint32s(self) -> impl Iterator<Item = Result<u32, DecodeError>> + 'a {
        self.map(|r| r.map(|v| v as u32))
    }
    pub fn uint64s(self) -> impl Iterator<Item = Result<u64, DecodeError>> + 'a {
        self.map(|r| r.map(|v| v))
    }
    pub fn bools(self) -> impl Iterator<Item = Result<bool, DecodeError>> + 'a {
        self.map(|r| r.map(|v| v != 0))
    }
    pub fn enums<E: TryFrom<i32>>(self) -> impl Iterator<Item = Result<E, DecodeError>> + 'a {
        self.map(|r| {
            r.and_then(|v| {
                E::try_from(v as i32).map_err(|_| DecodeError::InvalidEnumValue {
                    field: "<packed>",
                    value: v as i32,
                })
            })
        })
    }
}

impl<'a> Iterator for PackedVarints<'a> {
    type Item = Result<u64, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        Some(decode_varint(&mut self.0))
    }
}

/// Zero-copy iterator over packed fixed 32-bit values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PackedFixed32s<'a>(pub &'a [u8]);

impl<'a> PackedFixed32s<'a> {
    pub fn u32s(self) -> impl Iterator<Item = Result<u32, DecodeError>> + 'a {
        self.map(|r| r.map(u32::from_le_bytes))
    }
    pub fn i32s(self) -> impl Iterator<Item = Result<i32, DecodeError>> + 'a {
        self.map(|r| r.map(i32::from_le_bytes))
    }
    pub fn f32s(self) -> impl Iterator<Item = Result<f32, DecodeError>> + 'a {
        self.map(|r| r.map(f32::from_le_bytes))
    }
}

impl<'a> Iterator for PackedFixed32s<'a> {
    type Item = Result<[u8; 4], DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        if self.0.len() < 4 {
            self.0 = &[];
            return Some(Err(DecodeError::Truncated));
        }
        let (val, rest) = self.0.split_at(4);
        self.0 = rest;
        Some(Ok(val.try_into().unwrap()))
    }
}

/// Zero-copy iterator over packed fixed 64-bit values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PackedFixed64s<'a>(pub &'a [u8]);

impl<'a> PackedFixed64s<'a> {
    pub fn u64s(self) -> impl Iterator<Item = Result<u64, DecodeError>> + 'a {
        self.map(|r| r.map(u64::from_le_bytes))
    }
    pub fn i64s(self) -> impl Iterator<Item = Result<i64, DecodeError>> + 'a {
        self.map(|r| r.map(i64::from_le_bytes))
    }
    pub fn f64s(self) -> impl Iterator<Item = Result<f64, DecodeError>> + 'a {
        self.map(|r| r.map(f64::from_le_bytes))
    }
}

impl<'a> Iterator for PackedFixed64s<'a> {
    type Item = Result<[u8; 8], DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        if self.0.len() < 8 {
            self.0 = &[];
            return Some(Err(DecodeError::Truncated));
        }
        let (val, rest) = self.0.split_at(8);
        self.0 = rest;
        Some(Ok(val.try_into().unwrap()))
    }
}

// Fixed-width decode functions

#[inline]
pub fn decode_i64(buf: &mut &[u8]) -> Result<i64, DecodeError> {
    if buf.len() < 8 {
        return Err(DecodeError::Truncated);
    }
    let val = i64::from_le_bytes(buf[..8].try_into().unwrap());
    *buf = &buf[8..];
    Ok(val)
}

#[inline]
pub fn decode_u64(buf: &mut &[u8]) -> Result<u64, DecodeError> {
    if buf.len() < 8 {
        return Err(DecodeError::Truncated);
    }
    let val = u64::from_le_bytes(buf[..8].try_into().unwrap());
    *buf = &buf[8..];
    Ok(val)
}

#[inline]
pub fn decode_f64(buf: &mut &[u8]) -> Result<f64, DecodeError> {
    if buf.len() < 8 {
        return Err(DecodeError::Truncated);
    }
    let val = f64::from_le_bytes(buf[..8].try_into().unwrap());
    *buf = &buf[8..];
    Ok(val)
}

#[inline]
pub fn decode_i32(buf: &mut &[u8]) -> Result<i32, DecodeError> {
    if buf.len() < 4 {
        return Err(DecodeError::Truncated);
    }
    let val = i32::from_le_bytes(buf[..4].try_into().unwrap());
    *buf = &buf[4..];
    Ok(val)
}

#[inline]
pub fn decode_u32(buf: &mut &[u8]) -> Result<u32, DecodeError> {
    if buf.len() < 4 {
        return Err(DecodeError::Truncated);
    }
    let val = u32::from_le_bytes(buf[..4].try_into().unwrap());
    *buf = &buf[4..];
    Ok(val)
}

#[inline]
pub fn decode_f32(buf: &mut &[u8]) -> Result<f32, DecodeError> {
    if buf.len() < 4 {
        return Err(DecodeError::Truncated);
    }
    let val = f32::from_le_bytes(buf[..4].try_into().unwrap());
    *buf = &buf[4..];
    Ok(val)
}
