//! Buffer trait for protobuf serialization.
//!
//! [`WriteBuf`] covers both appending (for scalar writes) and random-access patching
//! (for [`Tack`](`crate::Tack`)'s length placeholders). Two buffers implement it:
//!
//! - `Vec<u8>`, which grows as needed. The default.
//! - [`SliceBuf`], a cursor over a caller-owned `&mut [u8]`, for `no_std`/no-alloc.

/// A contiguous byte buffer that supports both appending and random-access patching. Appending is
/// used by all scalar writers, random access by [`Tack`](`crate::Tack`) to patch length
/// placeholders.
///
/// Sealed: implemented only by `Vec<u8>` and [`SliceBuf`], and usable as a bound from any crate
/// but not implementable outside this one, for the reason on `private`.
pub trait WriteBuf: private::Sealed {
    fn put_u8(&mut self, val: u8);
    fn put_slice(&mut self, src: &[u8]);
    fn len(&self) -> usize;
    /// A mutable view whose first [`WriteBuf::len`] bytes are what has been written. It may be
    /// *longer*, since a fixed-capacity buffer hands back its whole backing store, so never treat
    /// the returned length as the written one.
    #[doc(hidden)]
    fn as_mut_slice(&mut self) -> &mut [u8];

    /// Grow the buffer by `additional` bytes. Called only on the overflow cold path.
    /// Fixed-size buffers should panic here.
    #[doc(hidden)]
    fn grow(&mut self, additional: usize);

    /// Shift bytes within the buffer. Used on the overflow cold path to make room
    /// for a wider length varint.
    #[doc(hidden)]
    fn copy_within(&mut self, src: core::ops::Range<usize>, dest: usize);

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Appends a base-128 varint. The default byte-at-a-time loop suits `Vec`; [`SliceBuf`]
    /// overrides it to avoid a bounds check per byte.
    fn put_varint(&mut self, value: u64) {
        crate::scalars::write_varint_into(value, self);
    }

    /// Writes a length-delimited submessage: the tag, the byte length of whatever `f` writes, and
    /// the payload, with the length reserved as a [`Tack`](`crate::Tack`) placeholder. Every
    /// nested-message and packed-field writer goes through here.
    // important inline
    #[inline]
    fn put_msg(&mut self, tag: crate::scalars::EncodedTag, f: impl FnOnce(&mut Self))
    where
        Self: Sized,
    {
        tag.write(self);
        let t = crate::tack::Tack::new(self);
        f(t.buffer);
    }

    /// Appends a length-delimited payload: its length as a varint, then the bytes.
    fn put_len_delimited(&mut self, payload: &[u8]) {
        self.put_varint(payload.len() as u64);
        self.put_slice(payload);
    }

    fn put_u32_le(&mut self, val: u32) {
        self.put_slice(&val.to_le_bytes());
    }
    fn put_i32_le(&mut self, val: i32) {
        self.put_slice(&val.to_le_bytes());
    }
    fn put_u64_le(&mut self, val: u64) {
        self.put_slice(&val.to_le_bytes());
    }
    fn put_i64_le(&mut self, val: i64) {
        self.put_slice(&val.to_le_bytes());
    }
    fn put_f32_le(&mut self, val: f32) {
        self.put_slice(&val.to_le_bytes());
    }
    fn put_f64_le(&mut self, val: f64) {
        self.put_slice(&val.to_le_bytes());
    }
}

/// Seals [`WriteBuf`]. Each buffer's `Sealed` impl sits next to its `WriteBuf` impl, since
/// `Vec<u8>`'s is feature-gated.
///
/// Sealed rather than `unsafe`, because [`Tack`](`crate::Tack`) patches the length prefix through
/// `get_unchecked_mut`, relying on `as_mut_slice()` being at least `len()` long. A safe
/// `pub trait` cannot require that of an impl, so an outside impl with no `unsafe` at all (large
/// `len()`, short `as_mut_slice()`) could produce an out-of-bounds write.
mod private {
    pub trait Sealed {}
}

/// Longest copy the [`copy_small`] ladder handles, the reach of its three overlapping 16-byte
/// stores.
pub(crate) const SMALL_COPY_MAX: usize = 48;

/// Copies `n` bytes with overlapping fixed-width stores instead of memcpy. LLVM only does this
/// itself when it can prove the length is constant and short, which it usually cannot. Short
/// strings are very common, so this is a solid win. Past 64 bytes memcpy is cheaper.
///
/// # Safety
/// `n` must be in `1..=SMALL_COPY_MAX`, and `dst` and `src` must have `n` bytes to write to and
/// read from without overlapping each other. The copies below overlap *each other*, which is fine,
/// since the requirement is per copy between its own two arguments.
#[inline(always)]
pub(crate) unsafe fn copy_small(dst: *mut u8, src: *const u8, n: usize) {
    use core::ptr::copy_nonoverlapping;
    debug_assert!(n >= 1 && n <= SMALL_COPY_MAX);
    if n >= 16 {
        // Branchless. The ends cover [0,16) and [n-16,n), the middle closes the gap. 48 is the
        // reach of this form (n/2-8 <= 16 and n/2+8 >= n-16), and n >= 16 keeps it in bounds.
        // Redundant below 32, but a branch would cost more.
        let mid = n / 2 - 8;
        copy_nonoverlapping(src, dst, 16);
        copy_nonoverlapping(src.add(mid), dst.add(mid), 16);
        copy_nonoverlapping(src.add(n - 16), dst.add(n - 16), 16);
    } else if n >= 8 {
        copy_nonoverlapping(src, dst, 8);
        copy_nonoverlapping(src.add(n - 8), dst.add(n - 8), 8);
    } else if n >= 4 {
        copy_nonoverlapping(src, dst, 4);
        copy_nonoverlapping(src.add(n - 4), dst.add(n - 4), 4);
    } else {
        *dst = *src;
        *dst.add(n / 2) = *src.add(n / 2);
        *dst.add(n - 1) = *src.add(n - 1);
    }
}

// --- Vec<u8> impl ---

#[cfg(feature = "alloc")]
mod alloc_impls {
    extern crate alloc;
    use alloc::vec::Vec;

    use super::*;

    impl private::Sealed for Vec<u8> {}
    impl WriteBuf for Vec<u8> {
        #[inline]
        fn put_u8(&mut self, val: u8) {
            self.push(val);
        }
        #[inline]
        fn put_slice(&mut self, src: &[u8]) {
            let n = src.len();
            if n == 0 || n > SMALL_COPY_MAX {
                self.extend_from_slice(src);
                return;
            }
            // Overlapping fixed-width stores for short slices,
            self.reserve(n);
            let len = self.len();
            // SAFETY: `reserve` guarantees `n` writable bytes at `len`, `n` is in range, and
            // `copy_small` stays inside them.
            unsafe {
                copy_small(self.as_mut_ptr().add(len), src.as_ptr(), n);
                self.set_len(len + n);
            }
        }
        #[inline]
        fn len(&self) -> usize {
            self.len()
        }
        #[inline]
        fn as_mut_slice(&mut self) -> &mut [u8] {
            self.as_mut_slice()
        }
        #[inline]
        fn grow(&mut self, additional: usize) {
            self.resize(self.len() + additional, 0);
        }
        #[inline]
        fn copy_within(&mut self, src: core::ops::Range<usize>, dest: usize) {
            self.as_mut_slice().copy_within(src, dest);
        }
    }
}

// --- Fixed-size slice buffer ---

/// A fixed-size buffer for `no_std` / no-alloc environments.
/// Wraps a `&mut [u8]` with a write cursor. Panics if the buffer is exhausted.
pub struct SliceBuf<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> SliceBuf<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        SliceBuf { buf, pos: 0 }
    }

    /// Returns the written portion of the buffer.
    pub fn written(&self) -> &[u8] {
        &self.buf[..self.pos]
    }
}

/// Adapter that implements [`core::fmt::Write`] for any [`WriteBuf`], so `Display` types can be
/// written into a protobuf buffer with `write!`.
pub struct FmtWriter<'a, B: WriteBuf + ?Sized>(pub &'a mut B);

impl<B: WriteBuf + ?Sized> core::fmt::Write for FmtWriter<'_, B> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.put_slice(s.as_bytes());
        Ok(())
    }
}

/// Wraps a reference to a [`Display`](`core::fmt::Display`) type so it can be written
/// directly as a protobuf string field. The formatted output becomes the field's UTF-8 value.
///
/// ```ignore
/// schema.name.write(&mut buf, Some(PbDisplay(&my_ip)));
/// ```
///
/// Panics if `Display::fmt` errors, rather than emit a half-formatted field under a correct length
/// prefix. Unusable as a map key or value too, because `as_scalar` returns `""` while `encode`
/// writes the real bytes, so `write_entry`'s precomputed length understates it.
pub struct PbDisplay<'a, T: core::fmt::Display + ?Sized>(pub &'a T);

impl<T: core::fmt::Display> crate::ProtoEncode<crate::PbString> for PbDisplay<'_, T> {
    fn as_scalar(&self) -> &str {
        ""
    }

    fn is_default(&self) -> bool {
        false
    }

    fn encode(buf: &mut impl WriteBuf, value: &Self) {
        use core::fmt::Write;
        // big enough for most things (ips, urls, uuids, etc)
        let t = crate::Tack::new_with_width(buf, 1);
        // `FmtWriter::write_str` never fails, so the only `Err` is the value's own `Display::fmt`.
        // Named, because `unwrap` on a `fmt::Error` prints nothing diagnosable.
        write!(FmtWriter(t.buffer), "{}", value.0).expect("PbDisplay: Display::fmt failed");
    }
}

/// Adapter that implements [`std::io::Write`] for any [`WriteBuf`], for sinks like
/// `serde_json::to_writer`.
#[cfg(feature = "std")]
pub struct IoWriter<'a, B: WriteBuf + ?Sized>(pub &'a mut B);

#[cfg(feature = "std")]
impl<B: WriteBuf + ?Sized> std::io::Write for IoWriter<'_, B> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.put_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Wraps a closure that writes bytes into an [`IoWriter`] so it can be used directly
/// as a protobuf bytes or string field. The closure receives an `&mut impl io::Write`.
///
/// ```ignore
/// schema.json_field.write(&mut buf, Some(PbWrite(|w| serde_json::to_writer(w, &val))));
/// ```
///
/// Unusable as a map key or value. See [`PbDisplay`].
/// Panics if the closure errors rather than emit a truncated field.
/// ```
#[cfg(feature = "std")]
pub struct PbWrite<F>(pub F);

#[cfg(feature = "std")]
impl<F, E: core::fmt::Debug> crate::ProtoEncode<crate::PbBytes> for PbWrite<F>
where
    F: Fn(&mut dyn std::io::Write) -> Result<(), E>,
{
    fn as_scalar(&self) -> &[u8] {
        &[]
    }

    fn is_default(&self) -> bool {
        false
    }

    fn encode(buf: &mut impl WriteBuf, value: &Self) {
        let t = crate::Tack::new_with_width(buf, 2);
        (value.0)(&mut IoWriter(t.buffer)).expect("PbWrite closure failed mid-field");
    }
}

#[cfg(feature = "std")]
impl<F, E: core::fmt::Debug> crate::ProtoEncode<crate::PbString> for PbWrite<F>
where
    F: Fn(&mut dyn std::io::Write) -> Result<(), E>,
{
    fn as_scalar(&self) -> &str {
        ""
    }

    fn is_default(&self) -> bool {
        false
    }

    fn encode(buf: &mut impl WriteBuf, value: &Self) {
        let t = crate::Tack::new_with_width(buf, 2);
        (value.0)(&mut IoWriter(t.buffer)).expect("PbWrite closure failed mid-field");
    }
}

impl private::Sealed for SliceBuf<'_> {}
impl WriteBuf for SliceBuf<'_> {
    #[inline]
    fn put_u8(&mut self, val: u8) {
        assert!(self.pos < self.buf.len(), "SliceBuf overflow");
        // SAFETY: the assert above is exactly the bound. Indexing instead re-checks it, and two
        // compares per byte where `Vec::push` pays one makes this buffer slower than `Vec`.
        unsafe { *self.buf.get_unchecked_mut(self.pos) = val };
        self.pos += 1;
    }
    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        let n = src.len();
        let end = self.pos + n;
        assert!(end <= self.buf.len(), "SliceBuf overflow");
        // SAFETY: `pos <= end <= buf.len()`, the first from the invariant that `pos` only ever
        // advances to a previously checked `end`, the second from the assert. That is `n`
        // writable bytes at `pos`, which is `copy_small`'s whole requirement.
        unsafe {
            let dst = self.buf.get_unchecked_mut(self.pos..end);
            if n == 0 || n > SMALL_COPY_MAX {
                dst.copy_from_slice(src);
            } else {
                copy_small(dst.as_mut_ptr(), src.as_ptr(), n);
            }
        };
        self.pos = end;
    }
    #[inline]
    fn put_varint(&mut self, value: u64) {
        // Claim once and store. The trait default's byte-at-a-time loop is fine for `Vec` but
        // costs this buffer a bounds check per byte. One byte covers most varints, meaning tags,
        // small ints and every length under 128, and taking it early skips
        // `encoded_len_varint`'s `clz` chain, which otherwise feeds the store address.
        if value < 0x80 {
            self.put_u8(value as u8);
            return;
        }
        let n = crate::scalars::encoded_len_varint(value);
        let end = self.pos + n;
        assert!(end <= self.buf.len(), "SliceBuf overflow");
        // SAFETY: as in `put_slice`, and `n >= 1` for every `u64`.
        let dst = unsafe { self.buf.get_unchecked_mut(self.pos..end) };
        let mut v = value;
        for i in 0..n - 1 {
            dst[i] = ((v & 0x7F) | 0x80) as u8;
            v >>= 7;
        }
        dst[n - 1] = v as u8;
        self.pos = end;
    }
    #[inline]
    fn len(&self) -> usize {
        self.pos
    }
    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        // The whole backing store, not `..pos`. The trait only promises the first `len()` bytes
        // are written, and reslicing costs a bounds check `Tack::close` then pays again.
        self.buf
    }
    /// Cannot allocate, but `Tack`'s overflow path needs `additional` bytes writable past `len()`,
    /// which a fixed buffer serves out of the room it already has. Panics only when it has none.
    // important inline
    #[inline]
    fn grow(&mut self, additional: usize) {
        assert!(
            self.pos + additional <= self.buf.len(),
            "SliceBuf cannot grow, message exceeded fixed buffer capacity"
        );
        self.pos += additional;
    }
    #[inline]
    fn copy_within(&mut self, src: core::ops::Range<usize>, dest: usize) {
        self.buf[..self.pos].copy_within(src, dest);
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::vec::Vec;
    use core::fmt::Write;

    use crate::tack::Tack;
    use crate::{scalars::*, ProtoEncode};

    /// A failing closure must not leave a silently truncated field. The `Tack` would patch a
    /// length over the partial bytes and the message would still parse.
    #[cfg(feature = "std")]
    #[test]
    #[should_panic(expected = "PbWrite closure failed mid-field")]
    fn pb_write_closure_error_panics() {
        let mut buf = alloc::vec::Vec::new();
        let w = PbWrite(|w: &mut dyn std::io::Write| {
            w.write_all(b"partial")?;
            Err(std::io::Error::other("serializer gave up"))
        });
        <PbWrite<_> as ProtoEncode<PbBytes>>::encode(&mut buf, &w);
    }

    #[cfg(feature = "alloc")]
    /// The ladder writes overlapping fixed-width blocks, not `n` bytes, so every arm boundary
    /// needs pinning. Appends onto a non-empty buffer so a wrong offset shows as corruption.
    #[test]
    fn put_slice_ladder_all_lengths() {
        for n in 0..=80usize {
            let src: Vec<u8> = (0..n).map(|i| (i as u8) ^ 0xA5).collect();
            let mut buf = Vec::with_capacity(0);
            buf.extend_from_slice(b"prefix");
            buf.put_slice(&src);
            assert_eq!(buf.len(), 6 + n, "len wrong at n={n}");
            assert_eq!(&buf[..6], b"prefix", "prefix clobbered at n={n}");
            assert_eq!(&buf[6..], &src[..], "payload wrong at n={n}");
        }
    }

    /// Every buffer type routes short copies through the same ladder, so every one gets the same
    /// all-lengths sweep. Writes next to existing bytes on purpose, since with overlapping stores
    /// a check that only compares the payload misses a write past `n`.
    #[test]
    fn put_slice_ladder_all_lengths_slice_buf() {
        for n in 0..=80usize {
            let src: Vec<u8> = (0..n).map(|i| (i as u8) ^ 0x5A).collect();

            // Forward into a fixed slice. The payload goes after a prefix, and the bytes past
            // `end` must be untouched.
            let mut backing = [0xCCu8; 200];
            let mut sb = SliceBuf::new(&mut backing);
            sb.put_slice(b"prefix");
            sb.put_slice(&src);
            let written = sb.written().to_vec();
            assert_eq!(written.len(), 6 + n, "SliceBuf len wrong at n={n}");
            assert_eq!(
                &written[..6],
                b"prefix",
                "SliceBuf prefix clobbered at n={n}"
            );
            assert_eq!(&written[6..], &src[..], "SliceBuf payload wrong at n={n}");
            assert!(
                backing[6 + n..].iter().all(|&b| b == 0xCC),
                "SliceBuf wrote past n={n}"
            );
        }
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn fmt_writer_basic() {
        let mut buf = Vec::new();
        write!(FmtWriter(&mut buf), "hello {}", 42).unwrap();
        assert_eq!(&buf, b"hello 42");
    }

    #[test]
    fn fmt_writer_with_slice_buf() {
        let mut backing = [0u8; 64];
        let mut sb = SliceBuf::new(&mut backing);
        write!(FmtWriter(&mut sb), "pi={:.2}", 3.14159).unwrap();
        assert_eq!(sb.written(), b"pi=3.14");
    }

    /// At the width-1 default any nested message of 128 B or more takes `Tack`'s
    /// overflow path, which calls `grow`. A fixed buffer with room to spare must
    /// serve that rather than panic.
    #[test]
    fn slice_buf_survives_tack_overflow() {
        use crate::{Field, Optional};
        let mut backing = [0u8; 512];
        let mut sb = SliceBuf::new(&mut backing);
        let long = "x".repeat(300);
        Field::<1, Optional<PbString>>::new().write(&mut sb, Some(long.as_str()));

        let mut slice = sb.written();
        let (field_nr, wire) = decode_key(&mut slice).unwrap();
        assert_eq!((field_nr, wire), (1, WireType::LEN));
        assert_eq!(PbString::read(&mut slice).unwrap(), long);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn pb_display_std_ip() {
        let mut buf = Vec::new();
        let ip = core::net::Ipv4Addr::new(192, 168, 1, 42);
        <PbDisplay<core::net::Ipv4Addr> as ProtoEncode<PbString>>::encode(
            &mut buf,
            &PbDisplay(&ip),
        );

        let mut slice = buf.as_slice();
        let decoded = PbString::read(&mut slice).unwrap();
        assert_eq!(decoded, "192.168.1.42");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn pb_display_std_socket_addr() {
        use crate::{Field, Optional};
        let mut buf = Vec::new();
        let addr = core::net::SocketAddr::from(([127, 0, 0, 1], 8080));
        Field::<1, Optional<PbString>>::new().write(&mut buf, Some(PbDisplay(&addr)));

        let mut slice = buf.as_slice();
        let (field_nr, wire) = decode_key(&mut slice).unwrap();
        assert_eq!(field_nr, 1);
        assert_eq!(wire, WireType::LEN);
        let decoded = PbString::read(&mut slice).unwrap();
        assert_eq!(decoded, "127.0.0.1:8080");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn pb_display_nested_in_tack() {
        let mut buf = Vec::new();
        let tag = EncodedTag::new(1, WireType::LEN);
        tag.write(&mut buf);
        {
            let t = Tack::new(&mut buf);
            let ip = core::net::Ipv4Addr::new(10, 0, 0, 1);
            <PbDisplay<core::net::Ipv4Addr> as ProtoEncode<PbString>>::encode(
                t.buffer,
                &PbDisplay(&ip),
            );
        }
        let mut slice = buf.as_slice();
        let (field_nr, wire) = decode_key(&mut slice).unwrap();
        assert_eq!(field_nr, 1);
        assert_eq!(wire, WireType::LEN);
        let inner = decode_len(&mut slice).unwrap();
        let mut inner_slice = inner;
        let decoded = PbString::read(&mut inner_slice).unwrap();
        assert_eq!(decoded, "10.0.0.1");
    }

    #[cfg(feature = "std")]
    #[test]
    fn io_writer_basic() {
        use std::io::Write;
        let mut buf = Vec::new();
        let mut w = IoWriter(&mut buf);
        w.write_all(b"hello ").unwrap();
        w.write_all(b"world").unwrap();
        assert_eq!(&buf, b"hello world");
    }

    #[cfg(feature = "std")]
    #[test]
    fn io_writer_through_tack() {
        use std::io::Write;
        let mut buf = Vec::new();
        let tag = EncodedTag::new(1, WireType::LEN);
        tag.write(&mut buf);
        {
            let t = Tack::new(&mut buf);
            let start = t.buffer.len();
            t.buffer.put_u8(0);
            IoWriter(t.buffer).write_all(b"payload").unwrap();
            let str_len = t.buffer.len() - start - 1;
            t.buffer.as_mut_slice()[start] = str_len as u8;
        }
        let mut slice = buf.as_slice();
        let (field_nr, wire) = decode_key(&mut slice).unwrap();
        assert_eq!(field_nr, 1);
        assert_eq!(wire, WireType::LEN);
        let inner = decode_len(&mut slice).unwrap();
        let mut inner_slice = inner;
        let decoded = PbBytes::read(&mut inner_slice).unwrap();
        assert_eq!(decoded, b"payload");
    }

    #[cfg(feature = "std")]
    #[test]
    fn pb_write_as_string_field() {
        use crate::{Field, Optional};
        let mut buf = Vec::new();
        let addr = core::net::SocketAddr::from(([192, 168, 0, 1], 443));
        let writer = PbWrite(|w: &mut dyn std::io::Write| write!(w, "endpoint={}", addr));
        Field::<1, Optional<PbString>>::new().write(&mut buf, Some(writer));

        let mut slice = buf.as_slice();
        let (field_nr, wire) = decode_key(&mut slice).unwrap();
        assert_eq!(field_nr, 1);
        assert_eq!(wire, WireType::LEN);
        let decoded = PbString::read(&mut slice).unwrap();
        assert_eq!(decoded, "endpoint=192.168.0.1:443");
    }

    #[cfg(feature = "std")]
    #[test]
    fn pb_write_as_bytes_field() {
        use crate::{Field, Optional};
        let data = [1u8, 2, 3, 4, 5];
        let writer = PbWrite(|w: &mut dyn std::io::Write| w.write_all(&data));
        let mut buf = Vec::new();
        Field::<1, Optional<PbBytes>>::new().write(&mut buf, Some(writer));

        let mut slice = buf.as_slice();
        let (field_nr, wire) = decode_key(&mut slice).unwrap();
        assert_eq!(field_nr, 1);
        assert_eq!(wire, WireType::LEN);
        let decoded = PbBytes::read(&mut slice).unwrap();
        assert_eq!(decoded, &[1, 2, 3, 4, 5]);
    }
}
