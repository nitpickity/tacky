use std::marker::PhantomData;

use bytes::BufMut;

use crate::scalars::{self, encoded_len_varint};

#[must_use]
pub struct Tack<'b, W: WidthImpl = Width<4>> {
    pub buffer: &'b mut Vec<u8>,
    pub tag: Option<u32>,
    start: u32,
    _w: PhantomData<W>
}

pub struct Width<const N: usize>;
trait WidthImpl {
    fn write(value: u64, buf: &mut impl BufMut);
    fn width() -> usize;
}

impl WidthImpl for Width<5> {
    fn write(value: u64, buf: &mut impl BufMut) {
        debug_assert!(encoded_len_varint(value) < 2usize.pow(35) - 1);
        buf.put_slice(&[
            ((value & 0x7F) | 0x80) as u8,
            (((value >> 7) & 0x7F) | 0x80) as u8,
            (((value >> 14) & 0x7F) | 0x80) as u8,
            (((value >> 21) & 0x7F) | 0x80) as u8,
            ((value >> 28) & 0x7F) as u8,
        ])
    }

    fn width() -> usize {
        5
    }
}
impl WidthImpl for Width<4> {
    fn write(value: u64, buf: &mut impl BufMut) {
        debug_assert!(encoded_len_varint(value) < 2usize.pow(28) - 1);
        buf.put_slice(&[
            ((value & 0x7F) | 0x80) as u8,
            (((value >> 7) & 0x7F) | 0x80) as u8,
            (((value >> 14) & 0x7F) | 0x80) as u8,
            ((value >> 21) & 0x7F) as u8,
        ])
    }

    fn width() -> usize {
        4
    }
}

impl WidthImpl for Width<3> {
    fn write(value: u64, buf: &mut impl BufMut) {
        debug_assert!(encoded_len_varint(value) < 2usize.pow(21) - 1);
        buf.put_slice(&[
            ((value & 0x7F) | 0x80) as u8,
            (((value >> 7) & 0x7F) | 0x80) as u8,
            (((value >> 14) & 0x7F)) as u8,
        ])
    }

    fn width() -> usize {
        3
    }
}

impl WidthImpl for Width<2> {
    fn write(value: u64, buf: &mut impl BufMut) {
        debug_assert!(encoded_len_varint(value) < 2usize.pow(14) - 1);
        buf.put_slice(&[
            ((value & 0x7F) | 0x80) as u8,
            (((value >> 7) & 0x7F)) as u8,
        ])
    }

    fn width() -> usize {
        2
    }
}

impl WidthImpl for Width<1> {
    fn write(value: u64, buf: &mut impl BufMut) {
        debug_assert!(encoded_len_varint(value) < 2usize.pow(7) - 1);
        buf.put_u8(value as u8)
    }

    fn width() -> usize {
        1
    }
}

impl<'b, W: WidthImpl> Tack<'b, W> {
    pub fn new(buffer: &'b mut Vec<u8>, tag: Option<u32>) -> Self {
        if let Some(tag) = tag {
            // writing in a nested context, need to write down the tag, and then len.
            scalars::write_varint(tag as u64, buffer);
            // since we dont know the length yet, we write a prelim 4 bytes wide varint, and fix it later
            W::write(42, buffer)
        }
        // now start represents the actual start of the data buffer, excluding the tag/length prefix
        Tack {
            start: buffer.len() as u32,
            buffer,
            tag,
            _w: PhantomData
        }
    }

    fn close(&mut self) {
        if self.tag.is_none() {
            return;
        }
        let start = self.start as usize;
        let data_len = self.buffer.len() - start;
        let mut len_prefix_loc = &mut self.buffer[start - W::width()..start];
        // write the correct length now
        W::write(data_len as u64, &mut len_prefix_loc);
    }
}

impl<'b, W: WidthImpl> Drop for Tack<'b, W> {
    fn drop(&mut self) {
        self.close()
    }
}
