//! A Tack marks the start of a as-of-yet unknown length delimited quantity.
//! upon creation, it writes down the tag of thie field and a (configurable)fixed width length field.
//! Once this struct Drop's, it goes back and updates the length field with however much was written to the buffer in the mean time.
//! Since the length field is fixed width (by default 4 bytes, which allows for messages of size 2**28 -1 bits long.) it can in theory overflow.
//! currently a Tack will panic on drop if that happens. (but the full value will still be written to the buffer).

use crate::scalars::{self, encoded_len_varint};
use bytes::BufMut;
use std::num::NonZeroU32;
/// A Tack marks the start of a as-of-yet unknown length delimited quantity.
#[must_use]
pub struct Tack<'b> {
    pub buffer: &'b mut Vec<u8>,
    // if the length of data written is 0, controls if the buffer is rewound to before the tag, or keeps the tag and len of 0.
    // default true
    pub rewind: bool,
    pub tag: Option<NonZeroU32>,
    start: u32,
    width: u32,
}

fn write_wide_varint(width: usize, value: u64, buf: &mut impl BufMut) {
    assert!(width <= 5 && width > 0);
    assert!(value < 2u64.pow(7 * width as u32) - 1);
    if width == 1 {
        buf.put_u8(value as u8);
        return;
    }
    for i in 0..(width - 1) {
        buf.put_u8((((value >> (7 * i)) & 0x7F) | 0x80) as u8)
    }
    buf.put_u8(((value >> (7 * width)) & 0x7F) as u8)
}

impl<'b> Tack<'b> {
    /// creates a new tack, which marks the start of a length-delimited field of TBD length.
    /// takes a buffer, and an optional tag. for top level messages, this will be None, as they dont have a tag or length delimiter of their own.
    pub fn new(buffer: &'b mut Vec<u8>, field_nr: Option<u32>) -> Self {
        let tag = field_nr.map(|n| (n << 3) | 2);
        let tag = tag.and_then(NonZeroU32::new);
        if let Some(tag) = tag {
            // writing in a nested context, need to write down the tag, and then len.
            scalars::write_varint(tag.get() as u64, buffer);
            // since we dont know the length yet, we write a prelim 4 bytes wide varint, and fix it later
            write_wide_varint(4, 0, buffer)
        }
        // now start represents the actual start of the data buffer, excluding the tag/length prefix
        Tack {
            start: buffer.len() as u32,
            buffer,
            rewind: true,
            tag,
            width: 4,
        }
    }

    pub fn new_with_width(buffer: &'b mut Vec<u8>, field_nr: Option<u32>, width: u32) -> Self {
        let tag = field_nr.map(|n| (n << 3) | 2);
        let tag = tag.and_then(NonZeroU32::new);
        if let Some(tag) = tag {
            // writing in a nested context, need to write down the tag, and then len.
            scalars::write_varint(tag.get() as u64, buffer);
            // since we dont know the length yet, we write a prelim 4 bytes wide varint, and fix it later
            write_wide_varint(width as usize, 0, buffer)
        }
        // now start represents the actual start of the data buffer, excluding the tag/length prefix
        Tack {
            start: buffer.len() as u32,
            buffer,
            rewind: true,
            tag,
            width,
        }
    }

    fn close(&mut self) {
        // not a nested field, just go back.
        let Some(tag) = self.tag else {
            return;
        };
        let start = self.start as usize;
        let width = self.width as usize;
        let data_len = self.buffer.len() - start;

        // Data is 0, handle rewind
        if data_len == 0 {
            if self.rewind {
                let tag_len = encoded_len_varint(tag.get() as u64);
                self.buffer.truncate(start - (tag_len + width));
            }
            return;
        }

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
    fn test_tack_expansion() {
        let mut buf = Vec::new();
        {
            let t = crate::tack::Tack::new_with_width(&mut buf, Some(1), 1);
            // Write 150 bytes of data (requires 2 bytes for length varint, taking up width=1 and expanding by 1)
            for _ in 0..150 {
                t.buffer.push(0xAA);
            }
        }
        // Expected layout: tag (1 byte: field 1, wire type 2 = 0x0A), len (2 bytes: 150 = 0x96 0x01), data (150 bytes of 0xAA)
        assert_eq!(buf.len(), 1 + 2 + 150);
        assert_eq!(buf[0], 0x0A);
        assert_eq!(buf[1], 0x96);
        assert_eq!(buf[2], 0x01);
        for i in 0..150 {
            assert_eq!(buf[3 + i], 0xAA);
        }
    }
}
