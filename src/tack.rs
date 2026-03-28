//! A Tack marks the start of a as-of-yet unknown length delimited quantity.
//! upon creation, it writes a (configurable) fixed width length field.
//! Once this struct Drop's, it goes back and updates the length field with however much was written to the buffer in the mean time.
//! Since the length field is fixed width (by default 3 bytes, which allows for messages of size ~2Mb.) it can in theory overflow.
//! if that happens, the Tack will reallocate the buffer to make room for a larger length field, and shift the data over to make room for it, before writing the length.
//! The caller is responsible for writing the tag before creating the Tack.

use crate::scalars::{self, encoded_len_varint};
use bytes::BufMut;
/// A Tack marks the start of a as-of-yet unknown length delimited quantity.
#[must_use]
pub struct Tack<'b> {
    pub buffer: &'b mut Vec<u8>,
    start: u32,
    width: u32,
}

pub fn write_wide_varint(width: usize, value: u64, buf: &mut impl BufMut) {
    assert!(width <= 5 && width > 0);
    assert!(value < 2u64.pow(7 * width as u32));
    if width == 1 {
        buf.put_u8(value as u8);
        return;
    }
    for i in 0..(width - 1) {
        buf.put_u8((((value >> (7 * i)) & 0x7F) | 0x80) as u8)
    }
    buf.put_u8(((value >> (7 * (width - 1))) & 0x7F) as u8)
}

impl<'b> Tack<'b> {
    /// Creates a new tack, which marks the start of a length-delimited section of TBD length.
    /// The caller must write the tag before creating the Tack.
    /// Defaults to 3 bytes width for the length field (~2Mb max).
    pub fn new(buffer: &'b mut Vec<u8>) -> Self {
        Self::new_with_width(buffer, 3)
    }
    pub fn new_with_width(buffer: &'b mut Vec<u8>, width: u32) -> Self {
        // since we dont know the length yet, we write a prelim <width> bytes wide varint, and fix it later
        write_wide_varint(width as usize, 0, buffer);

        Tack {
            start: buffer.len() as u32,
            buffer,
            width,
        }
    }

    fn close(&mut self) {
        let start = self.start as usize;
        let width = self.width as usize;
        let data_len = self.buffer.len() - start;

        let required_width = encoded_len_varint(data_len as u64);

        // Hot path: data fits within the reserved width
        if required_width <= width {
            let mut len_prefix_loc = &mut self.buffer[start - width..start];
            write_wide_varint(width, data_len as u64, &mut len_prefix_loc);
        } else {
            // Cold path: data requires larger varint encoding width
            self.fix_overflow(data_len, required_width);
        }
    }

    #[inline(never)]
    #[cold]
    fn fix_overflow(&mut self, data_len: usize, required_width: usize) {
        let start = self.start as usize;
        let width = self.width as usize;
        let diff = required_width - width;
        let old_len = self.buffer.len();
        // Resize buffer to add `diff` bytes
        self.buffer.resize(old_len + diff, 0);
        // Shift data to the right by `diff`
        self.buffer.copy_within(start..old_len, start + diff);
        // Write the correct length using standard varint encoding into the expanded prefix
        let mut len_prefix_loc: &mut [u8] = &mut self.buffer[start - width..start + diff];
        scalars::write_varint(data_len as u64, &mut len_prefix_loc);
    }
}

impl<'b> Drop for Tack<'b> {
    fn drop(&mut self) {
        self.close()
    }
}


#[cfg(test)]
mod tests {
    use crate::tack::write_wide_varint;

    #[test]
    fn test_write() {
        let mut buf = Vec::new();
        {
            for i in 2..=5 {
                write_wide_varint(i, 15723, &mut buf);
                println!("{buf:?}");
                let dec = crate::scalars::decode_varint(&mut buf.as_slice());
                println!("{dec:?}");
                buf.clear()
            }
        }
    }

    #[test]
    fn test_write_wide_varint_roundtrips() {
        let cases: Vec<(usize, u64)> = vec![
            (2, 128),       // needs bit 7 — first value that uses the 2nd group
            (2, 16383),     // max for width 2: 2^14 - 1
            (3, 16384),     // needs bit 14 — first value that uses the 3rd group
            (3, 2_097_151), // max for width 3: 2^21 - 1
            (4, 2_097_152), // first value needing the 4th group
        ];
        for (width, value) in cases {
            let mut buf = Vec::new();
            write_wide_varint(width, value, &mut buf);
            let decoded = crate::scalars::decode_varint(&mut buf.as_slice()).unwrap();
            assert_eq!(
                decoded, value,
                "write_wide_varint({width}, {value}) decoded as {decoded}"
            );
        }
    }

    #[test]
    fn test_tack_expansion() {
        let mut buf = Vec::new();
        // Manually write the tag (field 1, wire type LEN = 0x0A)
        crate::scalars::write_varint(0x0A, &mut buf);
        {
            let t = crate::tack::Tack::new_with_width(&mut buf, 1);
            // Write 150 bytes of data (requires 2 bytes for length varint, taking up width=1 and expanding by 1)
            for _ in 0..150 {
                t.buffer.push(0xAA);
            }
        }
        // Expected layout: tag (1 byte: 0x0A), len (2 bytes: 150 = 0x96 0x01), data (150 bytes of 0xAA)
        assert_eq!(buf.len(), 1 + 2 + 150);
        assert_eq!(buf[0], 0x0A);
        assert_eq!(buf[1], 0x96);
        assert_eq!(buf[2], 0x01);
        for i in 0..150 {
            assert_eq!(buf[3 + i], 0xAA);
        }
    }
}
