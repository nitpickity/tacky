//! Single-pass length encoding for protobuf's length-delimited fields.
//!
//! Protobuf requires the byte length of nested messages and packed repeated fields to appear
//! *before* their contents. The standard approach is two passes, one to compute the length and
//! one to write. Tack avoids this by writing a fixed-width placeholder varint, letting the
//! caller write data past it, then patching the real length on [`Drop`].

use crate::buf::WriteBuf;
use crate::scalars::encoded_len_varint;

/// Marks the start of a length-delimited section whose size isn't known yet.
///
/// On creation, writes a placeholder varint [`DEFAULT_WIDTH`] bytes wide. The caller writes data
/// into [`Tack::buffer`], and on drop the placeholder is overwritten with the actual length. Data
/// exceeding what the placeholder can hold expands the buffer and shifts the payload, which is
/// correct but slow and so marked `#[cold]`.
///
/// The caller must write the field tag before creating the Tack.
#[must_use]
pub struct Tack<'b, B: WriteBuf> {
    /// The buffer being written to. Exposed so callers (like `write_msg` closures) can write
    /// nested data through the Tack's borrow, which also blocks writes to the outer buffer while
    /// the Tack is active.
    pub buffer: &'b mut B,
    /// Byte position in the buffer immediately after the placeholder.
    /// `buffer.len() - start` gives the data length when closing.
    ///
    /// Do not narrow to `u32`. `close` feeds this to `get_unchecked_mut`, so a truncating cast
    /// past 4 GiB of *sink* (not message) would index outside the allocation, and an assert
    /// instead costs a compare and a panic edge per `Tack`.
    start: usize,
    /// Number of bytes reserved for the length varint. See [`DEFAULT_WIDTH`] for the range each
    /// width covers.
    width: u32,
}

/// Writes a varint padded to exactly `width` bytes using continuation bits. The result decodes
/// to `value` but always occupies `width` bytes, which is what lets the placeholder be
/// overwritten in place without shifting data.
///
/// # Panics
///
/// Unless `1 <= width <= 5` and `value < 2^(7 * width)`. A value that does not fit would
/// silently drop its high bits. Both asserts fold away for a constant `width`.
#[doc(hidden)]
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

/// Width of the length placeholder every writer reserves, in bytes.
///
/// A placeholder holds lengths below `2^(7*WIDTH)`, so 1 byte covers 128 B, 2 covers 16 KB and 3
/// covers 2 MB. Anything past the limit takes `fix_overflow`, which shifts that message's whole
/// payload. 1 byte is still the default, because it is only marginally slower than a wider
/// placeholder and yields a minimally packed size.
pub const DEFAULT_WIDTH: u32 = 1;

impl<'b, B: WriteBuf> Tack<'b, B> {
    /// Creates a new Tack with a [`DEFAULT_WIDTH`]-byte placeholder.
    /// Used for nested messages. The caller must write the field tag first.
    pub fn new(buffer: &'b mut B) -> Self {
        Self::new_with_width(buffer, DEFAULT_WIDTH)
    }
    /// Creates a new Tack with a custom placeholder width, for a caller that knows its payload
    /// will not fit in [`DEFAULT_WIDTH`] bytes.
    ///
    /// This and `close` are specialised for a constant width. A computed one turns both into
    /// loops over a variable, which is slower.
    ///
    /// # Panics
    ///
    /// Unless `1 <= width <= 5`, via [`write_wide_varint`], and on a reverse buffer, which needs
    /// no placeholder and should use [`WriteBuf::put_msg`]. Both asserts are constant at every
    /// generated call site, so both fold away there.
    // important: no #[inline] here, and keep this and `close` small
    pub fn new_with_width(buffer: &'b mut B, width: u32) -> Self {
        // A Tack's `start`-relative patch would hit the payload when the buffer grows downward. Not `const assert!`, because `maps::write_msg` instantiates its forward
        // branch for RevBuf behind a runtime guard. Folds away for forward buffers.
        assert!(
            !B::REVERSE,
            "Tack is forward-only: a reverse buffer knows its lengths, use WriteBuf::put_msg"
        );
        write_wide_varint(width as usize, 0, buffer);

        Tack {
            start: buffer.len(),
            buffer,
            width,
        }
    }

    fn close(&mut self) {
        let start = self.start;
        let width = self.width as usize;
        let data_len = self.buffer.len() - start;

        let required_width = encoded_len_varint(data_len as u64);

        // Hot path, data fits within the reserved width.
        if required_width <= width {
            // SAFETY: `new_with_width` writes exactly `width` bytes before recording
            // `start = buffer.len()`, and `WriteBuf` cannot shrink, so
            // `width <= start <= buffer.len()` holds for the Tack's whole life. LLVM cannot see
            // that across three unrelated loads, so a checked index costs a compare per close.
            let len_prefix_loc = unsafe {
                self.buffer
                    .as_mut_slice()
                    .get_unchecked_mut(start - width..start)
            };
            write_wide_varint_slice(width, data_len as u64, len_prefix_loc);
        } else {
            // Cold path, the data needs a wider length varint.
            self.fix_overflow(data_len, required_width);
        }
    }

    #[inline(never)]
    #[cold]
    fn fix_overflow(&mut self, data_len: usize, required_width: usize) {
        // `grow` panics when a fixed-size buffer has no room for the repair, and this runs from
        // `Drop`. While unwinding that second panic aborts the process and kills the caller's
        // `catch_unwind`, so give the repair up instead. Those bytes are being discarded anyway.
        #[cfg(feature = "std")]
        if std::thread::panicking() {
            return;
        }
        let start = self.start;
        let width = self.width as usize;
        let diff = required_width - width;
        let old_len = self.buffer.len();
        self.buffer.grow(diff);
        self.buffer.copy_within(start..old_len, start + diff);
        // SAFETY: as in `close`, plus `grow(diff)` has just made `len` at least
        // `old_len + diff`, and `start <= old_len`, so `start + diff <= len`.
        let len_prefix_loc = unsafe {
            self.buffer
                .as_mut_slice()
                .get_unchecked_mut(start - width..start + diff)
        };
        // Padding to exactly `required_width` is the minimal encoding, since that is what
        // `encoded_len_varint` returned, so these are the bytes an ordinary varint loop writes.
        write_wide_varint_slice(required_width, data_len as u64, len_prefix_loc);
    }
}

/// Write a wide varint directly into a mutable slice (for patching in-place).
///
/// The length assert has to stay an `assert!`. Downgrading it to `debug_assert!` reintroduces the
/// per-byte bounds checks, since it is the only thing making `buf[i]` provable.
// important inline: both callers pass a slice built to exactly `width`, so inlining folds the
// asserts and the bounds checks away
#[inline]
fn write_wide_varint_slice(width: usize, value: u64, buf: &mut [u8]) {
    // 10 rather than 5, because `fix_overflow` passes the width a varint actually needs, which
    // for a buffer length only reaches 6 or more past 32 GB and is not bounded by `Tack`'s.
    debug_assert!(width <= 10 && width > 0);
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
    /// May panic, but only when the message is unencodable anyway. The width-fits path cannot,
    /// and the overflow repair fails only when the sink is out of capacity. Never panics while
    /// unwinding, because `fix_overflow` gives up instead.
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

    #[cfg(feature = "alloc")]
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

    #[cfg(feature = "alloc")]
    #[test]
    fn test_write_wide_varint_roundtrips() {
        let cases: Vec<(usize, u64)> = vec![
            (2, 128),       // needs bit 7, first value using the 2nd group
            (2, 16383),     // max for width 2: 2^14 - 1
            (3, 16384),     // needs bit 14, first value using the 3rd group
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

    /// A `SliceBuf` overflow panics inside a live Tack, so the overflow repair in `Tack::drop`
    /// runs while unwinding. A second panic there aborts instead of letting the caller's
    /// `catch_unwind` see the first.
    #[cfg(feature = "std")]
    #[test]
    fn overflow_repair_gives_up_while_unwinding() {
        let mut backing = [0u8; 152];
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut buf = crate::SliceBuf::new(&mut backing);
            let t = crate::tack::Tack::<crate::SliceBuf>::new_with_width(&mut buf, 1);
            t.buffer.put_slice(&[0xAA; 150]);
            t.buffer.put_u8(1); // fills the buffer exactly
            t.buffer.put_u8(2); // overflows: panics with the Tack still open
        }));
        assert!(caught.is_err(), "expected the first panic to propagate");
    }

    #[cfg(feature = "alloc")]
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
