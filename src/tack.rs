use bytes::BufMut;

use crate::scalars;

pub struct Tack<'b> {
    buffer: &'b mut Vec<u8>,
    tag: Option<u32>,
    start: u32,
}

fn write_wide_varint(value: u64, buf: &mut impl BufMut) {
    buf.put_slice(&[
        ((value & 0x7F) | 0x80) as u8,
        (((value >> 7) & 0x7F) | 0x80) as u8,
        (((value >> 14) & 0x7F) | 0x80) as u8,
        ((value >> 21) & 0x7F) as u8,
    ])
}

impl<'b> Tack<'b> {
    pub fn new(buffer: &'b mut Vec<u8>, tag: Option<u32>) -> Self {
        if let Some(tag) = tag {
            // writing in a nested context, need to write down the tag, and then len.
            scalars::write_varint(tag as u64, buffer);
            // since we dont know the length yet, we write a prelim 4 bytes wide varint, and fix it later
            write_wide_varint(42, buffer)
        }
        // now start represents the actual start of the data buffer, excluding the tag/length prefix
        Tack {
            start: buffer.len() as u32,
            buffer,
            tag,
        }
    }

    fn close(&mut self) {
        if self.tag.is_none() {
            return;
        }
        let start = self.start as usize;
        let data_len = self.buffer.len() - start;
        let mut len_prefix_loc = &mut self.buffer[start - 4..start];
        // write the correct length now
        write_wide_varint(data_len as u64, &mut len_prefix_loc);
    }
}

impl<'b> Drop for Tack<'b> {
    fn drop(&mut self) {
        self.close()
    }
}
