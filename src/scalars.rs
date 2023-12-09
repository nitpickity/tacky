use bytes::BufMut;
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
pub const fn encode_zigzag32(n: i32) -> u32 {
    ((n << 1) ^ (n >> 31)) as u32
}
#[inline]
pub const fn encode_zigzag64(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}
#[inline]
pub fn write_double(value: f64, buf: &mut impl BufMut) {
    buf.put_f64(value);
}
#[inline]
pub fn write_float(value: f32, buf: &mut impl BufMut) {
    buf.put_f32(value);
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
