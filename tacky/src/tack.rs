//! Single-pass length encoding for protobuf's length-delimited fields.
//!
//! Protobuf requires the byte length of nested messages and packed repeated fields
//! to appear *before* their contents. The standard approach is two passes: iterate
//! to compute the length, then iterate again to write. Tack avoids this by writing
//! a fixed-width placeholder varint, letting the caller write data past it, then
//! patching the real length on [`Drop`].

use crate::buf::WriteBuf;
use crate::scalars::{self, encoded_len_varint};

/// Marks the start of a length-delimited section whose size isn't known yet.
///
/// On creation, writes a fixed-width placeholder varint (default 3 bytes, ~2MB max).
/// The caller writes data into [`Tack::buffer`]. On drop, the placeholder is overwritten
/// with the actual length. If the data exceeds the placeholder's capacity, the buffer
/// is expanded and data shifted — correct but slow, and marked `#[cold]`.
///
/// The caller is responsible for writing the field tag before creating the Tack.
#[must_use]
pub struct Tack<'b, B: WriteBuf> {
    /// The buffer being written to. Exposed so callers (like `write_msg` closures)
    /// can write nested data through the Tack's borrow, which also prevents
    /// accidental writes to the outer buffer while the Tack is active.
    pub buffer: &'b mut B,
    /// Byte position in the buffer immediately after the placeholder.
    /// `buffer.len() - start` gives the data length when closing.
    start: u32,
    /// Number of bytes reserved for the length varint.
    /// 2 bytes = ~16KB, 3 bytes = ~2MB.
    width: u32,
}

/// Writes a varint padded to exactly `width` bytes using continuation bits.
/// This produces a valid varint that decodes to `value`, but always occupies
/// the specified number of bytes — needed so the placeholder can be overwritten
/// in-place without shifting data.
pub fn write_wide_varint(width: usize, value: u64, buf: &mut impl WriteBuf) {
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

impl<'b, B: WriteBuf> Tack<'b, B> {
    /// Creates a new Tack with a 3-byte placeholder (~2MB max).
    /// Used for nested messages. The caller must write the field tag first.
    pub fn new(buffer: &'b mut B) -> Self {
        Self::new_with_width(buffer, 3)
    }
    /// Creates a new Tack with a custom placeholder width.
    /// Used for packed fields and map entries (width=2, ~16KB max).
    pub fn new_with_width(buffer: &'b mut B, width: u32) -> Self {
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
            let len_prefix_loc = &mut self.buffer.as_mut_slice()[start - width..start];
            write_wide_varint_slice(width, data_len as u64, len_prefix_loc);
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
        // Grow buffer to add `diff` bytes
        self.buffer.grow(diff);
        // Shift data to the right by `diff`
        self.buffer.copy_within(start..old_len, start + diff);
        // Write the correct length using standard varint encoding into the expanded prefix
        let len_prefix_loc = &mut self.buffer.as_mut_slice()[start - width..start + diff];
        scalars::write_varint_slice(data_len as u64, len_prefix_loc);
    }
}

/// Write a wide varint directly into a mutable slice (for patching in-place).
fn write_wide_varint_slice(width: usize, value: u64, buf: &mut [u8]) {
    assert!(width <= 5 && width > 0);
    assert!(buf.len() >= width);
    if width == 1 {
        buf[0] = value as u8;
        return;
    }
    for i in 0..(width - 1) {
        buf[i] = (((value >> (7 * i)) & 0x7F) | 0x80) as u8;
    }
    buf[width - 1] = ((value >> (7 * (width - 1))) & 0x7F) as u8;
}

impl<B: WriteBuf> Drop for Tack<'_, B> {
    fn drop(&mut self) {
        self.close()
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use crate::buf::WriteBuf;
    use crate::tack::write_wide_varint;
    use alloc::{vec, vec::Vec};

    #[test]
    fn test_write() {
        let mut buf = Vec::new();
        {
            for i in 2..=5 {
                write_wide_varint(i, 15723, &mut buf);
                let dec = crate::scalars::decode_varint(&mut buf.as_slice());
                assert_eq!(dec.unwrap(), 15723);
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
            let t = crate::tack::Tack::<Vec<u8>>::new_with_width(&mut buf, 1);
            // Write 150 bytes of data (requires 2 bytes for length varint, taking up width=1 and expanding by 1)
            for _ in 0..150 {
                t.buffer.put_u8(0xAA);
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
