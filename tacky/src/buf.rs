//! Buffer trait for protobuf serialization.
//!
//! [`WriteBuf`] covers both appending (for scalar writes) and random-access patching
//! (for [`Tack`](`crate::Tack`)'s length placeholders). Three buffers implement it:
//!
//! - `Vec<u8>` — grows as needed, the default.
//! - [`SliceBuf`] — a cursor over a caller-owned `&mut [u8]`, for `no_std`/no-alloc.
//! - [`RevBuf`] — fills the same kind of slice *backwards*. Every write prepends, so a
//!   nested message's length is known by the time it is written: no placeholder, no
//!   `Tack`, no overflow shift, and always a minimal-width varint. The cost is that the
//!   caller must let the writers own repeated-field iteration (see
//!   `Field::<N, Repeated<M>>::write_msgs`), because a hand-written loop would emit list
//!   elements in reverse. Map entries are exempt: their order is unspecified by protobuf,
//!   so they are written in iteration order either way.
//!
//! Direction is a compile-time property ([`WriteBuf::REVERSE`]), so the writers' two arms
//! fold away per buffer type and the forward path pays nothing for the reverse one.

/// A contiguous byte buffer that supports both appending and random-access patching.
///
/// Appending is used by all scalar writers. Random access (`len`, `as_mut_slice`) is
/// used by [`Tack`](`crate::Tack`) to patch length placeholders. `grow` and `copy_within`
/// are only called on Tack's overflow cold path — fixed-size buffers can panic there.
pub trait WriteBuf {
    /// True for buffers that grow *downward*, where every write prepends. Composite
    /// writes — tag then value, length then payload — have to be emitted in the opposite
    /// order so they land correctly, and this is what lets the writers pick. It is an
    /// associated const, so each branch folds away per buffer type at compile time.
    ///
    /// `WriteBuf` is never used as a trait object, so the const costs no flexibility.
    const REVERSE: bool = false;

    fn put_u8(&mut self, val: u8);
    fn put_slice(&mut self, src: &[u8]);
    fn len(&self) -> usize;
    /// A mutable view whose first [`WriteBuf::len`] bytes are what has been written. It may
    /// be *longer* than that: a fixed-capacity buffer hands back its whole backing store
    /// rather than reslicing to the cursor, because that reslice is bounds-checked and
    /// [`Tack`](`crate::Tack`) then checks the same range again. Never treat the returned
    /// slice's length as the written length; use `len()`.
    fn as_mut_slice(&mut self) -> &mut [u8];

    /// Grow the buffer by `additional` bytes. Called only on the overflow cold path.
    /// Fixed-size buffers should panic here.
    fn grow(&mut self, additional: usize);

    /// Shift bytes within the buffer. Used on the overflow cold path to make room
    /// for a wider length varint.
    fn copy_within(&mut self, src: core::ops::Range<usize>, dest: usize);

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Appends a base-128 varint. The default is the byte-at-a-time loop, which is right
    /// for `Vec` (`push` is a compare and a store) but leaves a cursor buffer paying its
    /// bounds check per byte — see [`SliceBuf`]'s override. [`RevBuf`] must override it for
    /// a second reason: prepending one byte at a time would reverse the varint's groups.
    fn put_varint(&mut self, value: u64) {
        crate::scalars::write_varint_into(value, self);
    }

    /// Writes a length-delimited submessage: the tag, the byte length of whatever `f`
    /// writes, and the payload.
    ///
    /// The forward default reserves a placeholder ([`Tack`](`crate::Tack`)) because the
    /// length is not known until `f` returns; a downward-growing buffer overrides this to
    /// run `f` first and prepend the exact length, which is why it needs no placeholder,
    /// no width and no overflow shift. Every nested-message and packed-field writer goes
    /// through here, so this method is the only place submessage direction lives.
    /// `#[inline]` is load-bearing: this is one call per submessage, and without it the
    /// `Tack` cannot be kept in registers across the closure — accesslog paid +80% and otlp
    /// +45% with it missing.
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
    /// Direction lives here rather than in every writer — a downward buffer prepends the
    /// payload first and the length second, so both land in the right order.
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

// --- Vec<u8> impl ---

#[cfg(feature = "alloc")]
mod alloc_impls {
    extern crate alloc;
    use alloc::vec::Vec;

    use super::*;

    impl WriteBuf for Vec<u8> {
        #[inline]
        fn put_u8(&mut self, val: u8) {
            self.push(val);
        }
        #[inline]
        fn put_slice(&mut self, src: &[u8]) {
            self.extend_from_slice(src);
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

// --- Reverse (downward-growing) buffer ---

/// A buffer that fills from the end backwards, so a nested message's length is known by
/// the time it has to be written — no placeholder, no [`Tack`](`crate::Tack`), no overflow
/// shift, and always a minimal-width varint.
///
/// The cost is that **every write prepends**, so fields land on the wire in the reverse of
/// the order they are written. Protobuf allows any field order, with two exceptions that
/// are the caller's responsibility:
///
/// - **elements of a repeated field must be written in reverse**, since their wire order is
///   their list order;
/// - **duplicate map keys** follow last-one-wins, so their relative order matters too.
///
/// Writing a message's fields in descending field order therefore reproduces exactly the
/// bytes an ascending forward writer produces — which is how the spike is checked.
///
/// Fixed capacity: `grow` panics, like [`SliceBuf`]. Use [`RevBuf::written`] to get the
/// bytes, which live at the *tail* of the backing slice.
pub struct RevBuf<'a> {
    buf: &'a mut [u8],
    /// Index of the first written byte. Writes move it down; `buf.len() - pos` is the
    /// length written so far.
    pos: usize,
}

impl<'a> RevBuf<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        let pos = buf.len();
        RevBuf { buf, pos }
    }

    /// The bytes written so far, at the tail of the backing slice.
    pub fn written(&self) -> &[u8] {
        &self.buf[self.pos..]
    }

    #[inline]
    fn claim(&mut self, n: usize) -> &mut [u8] {
        assert!(self.pos >= n, "RevBuf exhausted");
        self.pos -= n;
        // SAFETY: `pos + n` is the old `pos`, which is `<= buf.len()` for the buffer's
        // whole life, and the assert is what makes the subtraction not wrap.
        unsafe { self.buf.get_unchecked_mut(self.pos..self.pos + n) }
    }
}

impl WriteBuf for RevBuf<'_> {
    const REVERSE: bool = true;

    /// Runs `f`, then prepends the exact length and the tag — [`Tack`](`crate::Tack`)'s job
    /// without any of its machinery, since the length is already known when it is needed.
    #[inline]
    fn put_msg(&mut self, tag: crate::scalars::EncodedTag, f: impl FnOnce(&mut Self)) {
        let before = self.len();
        f(self);
        let payload = (self.len() - before) as u64;

        // Both parts are known here — the payload length was just measured and the tag is a
        // compile-time constant — so claim once and store. Writing them as two separate
        // appends costs two asserts, two cursor updates, and an out-of-line `memcpy` for
        // the tag's one to five bytes, which is the per-message overhead that made this
        // buffer lose to the forward path on `Tack`-dense corpora.
        let (tag_bytes, tag_len) = tag.raw();
        // Single-byte lengths are the overwhelming majority — every submessage under 128 B —
        // and this arm mirrors the forward path's, where a width-1 close is one compare and
        // one store. `encoded_len_varint` is a `clz` plus a multiply and divide; the forward
        // path only pays a compare against `0x80`.
        if payload < 0x80 {
            let dst = self.claim(tag_len + 1);
            // SAFETY: `dst.len() == tag_len + 1`, and `tag_len <= 5` per `EncodedTag::new`.
            unsafe {
                for i in 0..tag_len {
                    *dst.get_unchecked_mut(i) = *tag_bytes.get_unchecked(i);
                }
                *dst.get_unchecked_mut(tag_len) = payload as u8;
            }
            return;
        }
        let vn = crate::scalars::encoded_len_varint(payload);
        let dst = self.claim(tag_len + vn);
        // SAFETY: `dst.len() == tag_len + vn` by construction, and `tag_len <= 5` is
        // `EncodedTag::new`'s invariant, so every index below is in range.
        unsafe {
            for i in 0..tag_len {
                *dst.get_unchecked_mut(i) = *tag_bytes.get_unchecked(i);
            }
            let mut v = payload;
            for i in 0..vn - 1 {
                *dst.get_unchecked_mut(tag_len + i) = ((v & 0x7F) | 0x80) as u8;
                v >>= 7;
            }
            *dst.get_unchecked_mut(tag_len + vn - 1) = v as u8;
        }
    }

    #[inline]
    fn put_u8(&mut self, val: u8) {
        self.claim(1)[0] = val;
    }
    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        // One block prepend: the bytes keep their order, only the block moves.
        self.claim(src.len()).copy_from_slice(src);
    }
    #[inline]
    fn len(&self) -> usize {
        self.buf.len() - self.pos
    }
    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        let pos = self.pos;
        &mut self.buf[pos..]
    }
    fn grow(&mut self, _additional: usize) {
        panic!("RevBuf has a fixed capacity and cannot grow")
    }
    fn copy_within(&mut self, _src: core::ops::Range<usize>, _dest: usize) {
        panic!("RevBuf never shifts: lengths are known before they are written")
    }
    #[inline]
    fn put_varint(&mut self, value: u64) {
        // Claim the exact width and store into it. Staging into a `[u8; 10]` and handing
        // that to `put_slice` instead costs an out-of-line `memcpy` per varint — ~4.5 ns
        // of call overhead for one or two bytes, and this is the most frequent write there
        // is, since every length prefix goes through it.
        // No `value < 0x80` fast path here, deliberately: field values are mixed magnitude
        // (pprof interleaves small ids with 4-byte addresses), so that branch mispredicts
        // and costs 40% on pprof, while `encoded_len_varint`'s `clz` is branchless. The
        // opposite holds for *message lengths* in `put_msg`, which are locally uniform.
        let n = crate::scalars::encoded_len_varint(value);
        let dst = self.claim(n);
        let mut v = value;
        for i in 0..n - 1 {
            dst[i] = ((v & 0x7F) | 0x80) as u8;
            v >>= 7;
        }
        dst[n - 1] = v as u8;
    }
    #[inline]
    fn put_len_delimited(&mut self, payload: &[u8]) {
        // One claim for length and payload together: the length's width is known from the
        // payload's, so splitting this into two appends only buys a second assert.
        if payload.len() < 0x80 {
            let dst = self.claim(1 + payload.len());
            // SAFETY: `dst.len() == 1 + payload.len()` by construction.
            unsafe {
                *dst.get_unchecked_mut(0) = payload.len() as u8;
                dst.get_unchecked_mut(1..).copy_from_slice(payload);
            }
            return;
        }
        let vn = crate::scalars::encoded_len_varint(payload.len() as u64);
        let dst = self.claim(vn + payload.len());
        // SAFETY: `dst.len() == vn + payload.len()` by construction.
        unsafe {
            let mut v = payload.len() as u64;
            for i in 0..vn - 1 {
                *dst.get_unchecked_mut(i) = ((v & 0x7F) | 0x80) as u8;
                v >>= 7;
            }
            *dst.get_unchecked_mut(vn - 1) = v as u8;
            dst.get_unchecked_mut(vn..).copy_from_slice(payload);
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

/// Adapter that implements [`core::fmt::Write`] for any [`WriteBuf`].
///
/// Allows writing `Display` types directly into a protobuf buffer via `write!`.
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
        let t = crate::Tack::new_with_width(buf, 2);
        write!(FmtWriter(t.buffer), "{}", value.0).unwrap();
    }
}

/// Adapter that implements [`std::io::Write`] for any [`WriteBuf`].
///
/// Useful for integrations like `serde_json::to_writer` that expect an `io::Write` sink.
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
#[cfg(feature = "std")]
pub struct PbWrite<F>(pub F);

#[cfg(feature = "std")]
impl<F, E> crate::ProtoEncode<crate::PbBytes> for PbWrite<F>
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
        (value.0)(&mut IoWriter(t.buffer)).ok();
    }
}

#[cfg(feature = "std")]
impl<F, E> crate::ProtoEncode<crate::PbString> for PbWrite<F>
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
        (value.0)(&mut IoWriter(t.buffer)).ok();
    }
}

impl WriteBuf for SliceBuf<'_> {
    #[inline]
    fn put_u8(&mut self, val: u8) {
        assert!(self.pos < self.buf.len(), "SliceBuf overflow");
        // SAFETY: the assert above is exactly the bound. Indexing instead re-checks it —
        // `Vec::push` pays one compare and then stores through a raw pointer, and paying
        // two is why this buffer measured *slower* than `Vec` despite never reallocating.
        unsafe { *self.buf.get_unchecked_mut(self.pos) = val };
        self.pos += 1;
    }
    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        let end = self.pos + src.len();
        assert!(end <= self.buf.len(), "SliceBuf overflow");
        // SAFETY: `pos <= end <= buf.len()` — the first from the invariant that `pos` only
        // ever advances to a previously checked `end`, the second from the assert.
        unsafe {
            self.buf
                .get_unchecked_mut(self.pos..end)
                .copy_from_slice(src)
        };
        self.pos = end;
    }
    #[inline]
    fn put_varint(&mut self, value: u64) {
        // Claim once and store, rather than the trait default's byte-at-a-time loop: that
        // is fine for `Vec` (`push` is a compare and a store) but costs this buffer a
        // bounds check per byte.
        // One byte covers most varints — tags, small ints, and every length under 128 —
        // and taking it early skips `encoded_len_varint`'s `clz` chain, which otherwise
        // feeds the store address and serialises with it.
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
        // The whole backing store, not `..pos`: the trait only promises the first `len()`
        // bytes are written, and reslicing here costs a bounds check that `Tack::close`
        // then pays a second time.
        self.buf
    }
    /// Cannot allocate, but `Tack`'s overflow path needs `additional` bytes made
    /// writable past `len()` — which a fixed buffer can serve out of the room it
    /// already has. Panics only when it genuinely has none.
    ///
    /// `#[inline]` because `Vec`'s equivalents have it: without it, a width-1 overflow
    /// costs two out-of-line calls here and none there.
    #[inline]
    fn grow(&mut self, additional: usize) {
        assert!(
            self.pos + additional <= self.buf.len(),
            "SliceBuf cannot grow — message exceeded fixed buffer capacity"
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
