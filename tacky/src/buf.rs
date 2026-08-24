//! Buffer trait for protobuf serialization.
//!
//! [`WriteBuf`] covers both appending (for scalar writes) and random-access patching
//! (for [`Tack`](`crate::Tack`)'s length placeholders). Three buffers implement it:
//!
//! - `Vec<u8>`, which grows as needed. The default.
//! - [`SliceBuf`], a cursor over a caller-owned `&mut [u8]`, for `no_std`/no-alloc.
//! - [`RevBuf`], which fills the same kind of slice *backwards*, so nested lengths are exact and
//!   need no placeholder. Comes with an ordering contract for repeated fields, see its docs.
//!
//! Direction is a compile-time property, [`WriteBuf::REVERSE`] as a value and
//! [`WriteBuf::Order`] as a type, so the writers' two arms fold away per buffer type. Code that
//! has not picked a buffer writes through [`AnyDir`]. See [`OrderedIter`].

/// A contiguous byte buffer that supports both appending and random-access patching. Appending is
/// used by all scalar writers, random access by [`Tack`](`crate::Tack`) to patch length
/// placeholders.
///
/// Sealed: implemented only by `Vec<u8>`, [`SliceBuf`], [`RevBuf`] and [`AnyDir`], and usable as a
/// bound from any crate but not implementable outside this one, for the reason on `private`.
pub trait WriteBuf: private::Sealed {
    /// This buffer's direction as a type, either [`Forward`] or [`Reverse`]. What
    /// [`OrderedIter`] dispatches on.
    type Order: Order;

    /// True for buffers that grow *downward*, where every write prepends. Composite writes, tag
    /// then value or length then payload, have to be emitted in the opposite order to come out
    /// correct. An associated const, so each branch folds away per buffer type.
    ///
    /// Derived from [`WriteBuf::Order`]. Set the type, never this.
    const REVERSE: bool = <Self::Order as Order>::REVERSE;

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

    /// Reserves `n` contiguous bytes to be filled in **wire order**, or `None` if this
    /// buffer cannot hand out such a window.
    ///
    /// Only a downward-growing buffer says yes, and only where the total size is known before
    /// writing, as in a scalar map entry. Prepending a whole block leaves the bytes *inside* it in
    /// order, so the caller can wrap the window in a [`SliceBuf`] and run the forward path
    /// verbatim. `Vec` says no, since a window over its uninitialised capacity needs
    /// `MaybeUninit`.
    #[doc(hidden)]
    fn claim_block(&mut self, _n: usize) -> Option<&mut [u8]> {
        None
    }

    /// Appends a base-128 varint. The default byte-at-a-time loop suits `Vec`; [`SliceBuf`]
    /// overrides it to avoid a bounds check per byte, and [`RevBuf`] must, since prepending one
    /// byte at a time reverses the varint's groups.
    fn put_varint(&mut self, value: u64) {
        crate::scalars::write_varint_into(value, self);
    }

    /// Writes a length-delimited submessage: the tag, the byte length of whatever `f` writes, and
    /// the payload. The forward default reserves a [`Tack`](`crate::Tack`); a downward-growing
    /// buffer runs `f` first and prepends the exact length. Every nested-message and packed-field
    /// writer goes through here, so this is the only place submessage direction lives.
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

    /// Appends a length-delimited payload: its length as a varint, then the bytes. A downward
    /// buffer prepends the payload first and the length second, to get the order right.
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

// --- Direction ---

/// Marker for a buffer that grows upward. Writes append, so a repeated field's elements keep the
/// order they were written in.
pub struct Forward;
/// Marker for a buffer that grows downward. Writes prepend, so a repeated field's elements have
/// to be emitted back-to-front.
pub struct Reverse;
/// Marker for a buffer whose direction is not known where the write is type-checked, which is
/// the direction [`AnyDir`] presents. Repeated fields then require a [`DoubleEndedIterator`],
/// since the buffer may turn out to grow downward.
///
/// A `WriteBuf` using this **must** override [`WriteBuf::REVERSE`]. Inheriting `Both`'s
/// placeholder value would give a wrapper around a [`RevBuf`] forward ordering.
pub struct Both;

/// A buffer's direction, as a type. Carried by [`WriteBuf::Order`] so that [`OrderedIter`] can
/// select on it. Sealed: the writers branch on exactly [`Forward`], [`Reverse`] and [`Both`].
pub trait Order: private::Sealed {
    /// Mirrors [`WriteBuf::REVERSE`], which is derived from this.
    #[doc(hidden)]
    const REVERSE: bool;
}

/// Seals both [`Order`] and [`WriteBuf`]. The direction markers are impl'd here; each buffer's
/// `Sealed` impl sits next to its `WriteBuf` impl, since `Vec<u8>`'s is feature-gated.
///
/// Sealed rather than `unsafe`, because [`Tack`](`crate::Tack`) patches the length prefix through
/// `get_unchecked_mut`, relying on `as_mut_slice()` being at least `len()` long. A safe
/// `pub trait` cannot require that of an impl, so an outside impl with no `unsafe` at all (large
/// `len()`, short `as_mut_slice()`) could produce an out-of-bounds write.
mod private {
    pub trait Sealed {}
    impl Sealed for super::Forward {}
    impl Sealed for super::Reverse {}
    impl Sealed for super::Both {}
}

impl Order for Forward {
    const REVERSE: bool = false;
}

impl Order for Reverse {
    const REVERSE: bool = true;
}

impl Order for Both {
    /// A placeholder. See [`Both`].
    const REVERSE: bool = false;
}

/// What the repeated and packed writers take: an iterable they can walk in wire order, given
/// the buffer's direction. One impl per direction.
///
/// - [`Forward`] takes **any** [`IntoIterator`], since appending reorders nothing. A `HashSet`,
///   a `take_while`, a hand-written one-way `Iterator`.
/// - [`Reverse`] requires a [`DoubleEndedIterator`] and walks it backwards. A one-way iterator
///   there is a compile error rather than a silently reversed list.
///
/// So the direction must be *known* where the call is type-checked, from a concrete buffer or a
/// `WriteBuf<Order = ..>` bound. A body generic over the buffer has neither, and no impl can cover
/// it, because a blanket impl over the direction would have to be strict and coherence rejects
/// that beside the lax [`Forward`] impl. Such a body writes through [`AnyDir`] and its [`Both`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be written to a buffer whose direction is `{O}`",
    label = "not writable in `{O}` order",
    note = "a `Reverse` buffer walks a repeated field's elements backwards, so it needs a `DoubleEndedIterator`",
    note = "if `{O}` is a generic parameter or projection, this body has not picked a direction. take a `&mut AnyDir<B>` instead, or bound the buffer as `WriteBuf<Order = Forward>`"
)]
pub trait OrderedIter<O>: IntoIterator {
    type Ordered: Iterator<Item = Self::Item>;
    /// The elements in the order the buffer needs them written. Walk it front-to-back either
    /// way, since for a downward buffer, whose writes prepend, that yields list order.
    ///
    /// `reverse` is [`WriteBuf::REVERSE`]. Only [`Both`] reads it, and it is a compile-time
    /// constant at every call site, so the arm it selects folds away.
    fn ordered(self, reverse: bool) -> Self::Ordered;
}

impl<I: IntoIterator> OrderedIter<Forward> for I {
    type Ordered = I::IntoIter;
    #[inline]
    fn ordered(self, _reverse: bool) -> Self::Ordered {
        self.into_iter()
    }
}

impl<I: IntoIterator> OrderedIter<Reverse> for I
where
    I::IntoIter: DoubleEndedIterator,
{
    type Ordered = core::iter::Rev<I::IntoIter>;
    #[inline]
    fn ordered(self, _reverse: bool) -> Self::Ordered {
        self.into_iter().rev()
    }
}

impl<I: IntoIterator> OrderedIter<Both> for I
where
    I::IntoIter: DoubleEndedIterator,
{
    type Ordered = EitherIter<I::IntoIter>;
    #[inline]
    fn ordered(self, reverse: bool) -> Self::Ordered {
        if reverse {
            EitherIter::Reverse(self.into_iter().rev())
        } else {
            EitherIter::Forward(self.into_iter())
        }
    }
}

/// [`Both`]'s ordered iterator: the caller's, or [`Rev`](`core::iter::Rev`) of it, decided by
/// the [`WriteBuf::REVERSE`] passed to [`OrderedIter::ordered`]. That is a `const` per buffer
/// type, so the match folds. The tag exists only because the *type* cannot name a direction the
/// body has not picked.
pub enum EitherIter<I> {
    Forward(I),
    Reverse(core::iter::Rev<I>),
}

impl<I: DoubleEndedIterator> Iterator for EitherIter<I> {
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        match self {
            EitherIter::Forward(i) => i.next(),
            EitherIter::Reverse(i) => i.next(),
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            EitherIter::Forward(i) => i.size_hint(),
            EitherIter::Reverse(i) => i.size_hint(),
        }
    }
}

impl<I: DoubleEndedIterator + ExactSizeIterator> ExactSizeIterator for EitherIter<I> {
    #[inline]
    fn len(&self) -> usize {
        match self {
            EitherIter::Forward(i) => i.len(),
            EitherIter::Reverse(i) => i.len(),
        }
    }
}

/// A view of any buffer for code that has not picked a direction, i.e. what
/// `fn encode(buf: &mut impl WriteBuf)` wanted to be:
///
/// ```ignore
/// fn write_file<B: WriteBuf>(buf: &mut AnyDir<B>, f: &FileDescriptorProto) {
///     schema.dependency.write(buf, &f.dependency);          // iterators stay bare
/// }
/// write_file(AnyDir::from_mut(&mut vec), &f);               // wrapped once, here
/// write_file(AnyDir::from_mut(&mut rev_buf), &f);
/// ```
///
/// It forwards every write to `B` unchanged, including [`WriteBuf::REVERSE`], so the bytes and the
/// codegen are the buffer's own. Only [`WriteBuf::Order`] changes, to [`Both`], so repeated fields
/// take any double-ended iterator and are walked in whichever order `B` needs. A one-way iterator,
/// which a forward buffer accepts bare, is rejected here.
#[repr(transparent)]
pub struct AnyDir<B>(B);

impl<B: WriteBuf> AnyDir<B> {
    /// Views `buf` as direction-erased. Free, a `repr(transparent)` reference cast.
    ///
    /// Takes `&mut B` rather than `B` so the view has no lifetime of its own, because `put_msg`
    /// builds a `&mut Self` from a shorter-lived `&mut B` and `&mut AnyDir<'a, B>` would be
    /// invariant in `'a`.
    #[inline]
    pub fn from_mut(buf: &mut B) -> &mut AnyDir<B> {
        // SAFETY: `repr(transparent)` gives `AnyDir<B>` the layout of `B`, and the view adds
        // no invariants of its own, so the two references are interchangeable.
        unsafe { &mut *(buf as *mut B as *mut AnyDir<B>) }
    }
}

impl<B: WriteBuf> private::Sealed for AnyDir<B> {}
impl<B: WriteBuf> WriteBuf for AnyDir<B> {
    type Order = Both;
    /// `B`'s own direction, still a compile-time constant. This view erases only the *iterator*
    /// bound, never the tag/value ordering the writers branch on.
    const REVERSE: bool = B::REVERSE;

    #[inline]
    fn put_u8(&mut self, val: u8) {
        self.0.put_u8(val);
    }
    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        self.0.put_slice(src);
    }
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
    #[inline]
    fn grow(&mut self, additional: usize) {
        self.0.grow(additional);
    }
    #[inline]
    fn copy_within(&mut self, src: core::ops::Range<usize>, dest: usize) {
        self.0.copy_within(src, dest);
    }
    #[inline]
    fn claim_block(&mut self, n: usize) -> Option<&mut [u8]> {
        self.0.claim_block(n)
    }
    #[inline]
    fn put_varint(&mut self, value: u64) {
        self.0.put_varint(value);
    }
    #[inline]
    fn put_msg(&mut self, tag: crate::scalars::EncodedTag, f: impl FnOnce(&mut Self)) {
        self.0.put_msg(tag, |inner| f(AnyDir::from_mut(inner)));
    }
    #[inline]
    fn put_len_delimited(&mut self, payload: &[u8]) {
        self.0.put_len_delimited(payload);
    }
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
        type Order = Forward;

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

// --- Reverse (downward-growing) buffer ---

/// A buffer that fills from the end backwards, so a nested message's length is known by the time
/// it has to be written. Doesnt use a [`Tack`](`crate::Tack`), so no overflow shifts.
///
/// The cost is that **every write prepends**, so fields appear on the wire in the reverse of the
/// order they are written. Protobuf allows any field order, with two exceptions the caller owns.
///
/// - **Repeated fields must be written back-to-front**, since their wire order is their list
///   order. One writer call handles this for you, because [`OrderedIter`] walks the iterator
///   backwards. Across calls it cannot, because each write prepends its whole block, so two
///   `write_msgs` calls, or a loop of `write_msg`/`write_single`, come out in reverse call order.
///   Let one call own the whole list where you can, otherwise call them tail-first.
/// - **Duplicate map keys** follow last-one-wins, so their relative order matters too.
///
/// Writing a message's fields in descending field order therefore reproduces exactly the bytes an
/// ascending forward writer produces.
///
/// Fixed capacity, so `grow` panics as it does on [`SliceBuf`]. [`RevBuf::written`] returns the
/// bytes, which sit at the *tail* of the backing slice.
pub struct RevBuf<'a> {
    buf: &'a mut [u8],
    /// Index of the first written byte. Writes move it down, and `buf.len() - pos` is the length
    /// written so far.
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
        // SAFETY: `pos + n` is the old `pos`, which is `<= buf.len()` for the buffer's whole
        // life, and the assert is what stops the subtraction wrapping.
        unsafe { self.buf.get_unchecked_mut(self.pos..self.pos + n) }
    }
}

impl private::Sealed for RevBuf<'_> {}
impl WriteBuf for RevBuf<'_> {
    type Order = Reverse;

    /// Runs `f`, then prepends the exact length and the tag. [`Tack`](`crate::Tack`)'s job without
    /// any of its machinery, since the length is known by the time it is needed.
    #[inline]
    fn put_msg(&mut self, tag: crate::scalars::EncodedTag, f: impl FnOnce(&mut Self)) {
        let before = self.len();
        f(self);
        let payload = (self.len() - before) as u64;

        // Both parts are known here, so claim once and store. Two separate appends instead cost
        // two asserts, two cursor updates and an out-of-line `memcpy` for the tag, per message.
        let (tag_bytes, tag_len) = tag.raw();
        // Single-byte lengths are the overwhelming majority, every submessage under 128 B, so
        // take them on a compare rather than `encoded_len_varint`'s clz/multiply/divide.
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
        // SAFETY: `dst.len() == tag_len + vn` by construction, and `tag_len <= 5` per
        // `EncodedTag::new`, so every index below is in range.
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
        // One block prepend, so the bytes keep their order and only the block moves.
        //
        // The length test comes *first*, before `claim` touches `pos`, and the short path then
        // stores through a raw pointer rather than building the `&mut [u8]` `claim` returns.
        // Keep that order. `claim`'s assert and cursor update ahead of the length branch is
        // measurably worse on varint-heavy inputs, where most calls exceed the cap.
        let n = src.len();
        if n == 0 || n > SMALL_COPY_MAX {
            self.claim(n).copy_from_slice(src);
            return;
        }
        assert!(self.pos >= n, "RevBuf exhausted");
        self.pos -= n;
        // SAFETY: as in `claim`. `pos + n` is the old `pos`, which is `<= buf.len()` for the
        // buffer's whole life, and the assert is what keeps the subtraction from wrapping. That
        // is `n` writable bytes at `pos`, which is all `copy_small` requires.
        unsafe { copy_small(self.buf.as_mut_ptr().add(self.pos), src.as_ptr(), n) };
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
    /// `None` when the remaining room is short, as the trait promises; `claim` would assert
    /// instead.
    #[inline]
    fn claim_block(&mut self, n: usize) -> Option<&mut [u8]> {
        if self.pos < n {
            return None;
        }
        Some(self.claim(n))
    }
    fn grow(&mut self, _additional: usize) {
        panic!("RevBuf has a fixed capacity and cannot grow")
    }
    fn copy_within(&mut self, _src: core::ops::Range<usize>, _dest: usize) {
        panic!("RevBuf never shifts: lengths are known before they are written")
    }
    #[inline]
    fn put_varint(&mut self, value: u64) {
        // Claim the exact width and store into it; staging into a `[u8; 10]` for `put_slice`
        // costs an out-of-line `memcpy` on the most frequent write there is.
        //
        // No `value < 0x80` fast path, deliberately. Field values are mixed magnitude, so that
        // branch mispredicts where `encoded_len_varint`'s `clz` is branchless. The opposite
        // holds for *message lengths* in `put_msg`, which are locally uniform.
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
        // One claim for length and payload together. The length's width follows from the
        // payload's, so two appends would only buy a second assert.
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

/// Adapter that implements [`core::fmt::Write`] for any [`WriteBuf`], so `Display` types can be
/// written into a protobuf buffer with `write!`.
///
/// Forward buffers only. A multi-chunk write into a prepending buffer comes out chunk-reversed.
pub struct FmtWriter<'a, B: WriteBuf + ?Sized>(pub &'a mut B);

impl<B: WriteBuf + ?Sized> core::fmt::Write for FmtWriter<'_, B> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // Folds away for a forward buffer. Not `debug_assert!`, which leaves release emitting
        // chunks backwards, and not `const { assert!(..) }`, which as at `Tack::new_with_width`
        // would refuse to compile an instantiation the caller guards at runtime.
        assert!(
            !B::REVERSE,
            "FmtWriter appends; a reverse buffer would emit chunks backwards"
        );
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
/// Panics on a [`RevBuf`], since it streams through a [`Tack`](`crate::Tack`) placeholder, and if
/// `Display::fmt` errors, rather than emit a half-formatted field under a correct length prefix.
/// Unusable as a map key or value too, because `as_scalar` returns `""` while `encode` writes the
/// real bytes, so `write_entry`'s precomputed length understates it.
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
        // `FmtWriter::write_str` never fails, so the only `Err` is the value's own `Display::fmt`.
        // Named, because `unwrap` on a `fmt::Error` prints nothing diagnosable.
        write!(FmtWriter(t.buffer), "{}", value.0).expect("PbDisplay: Display::fmt failed");
    }
}

/// Adapter that implements [`std::io::Write`] for any [`WriteBuf`], for sinks like
/// `serde_json::to_writer`.
///
/// Forward buffers only, as [`FmtWriter`].
#[cfg(feature = "std")]
pub struct IoWriter<'a, B: WriteBuf + ?Sized>(pub &'a mut B);

#[cfg(feature = "std")]
impl<B: WriteBuf + ?Sized> std::io::Write for IoWriter<'_, B> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // As in `FmtWriter::write_str`.
        assert!(
            !B::REVERSE,
            "IoWriter appends; a reverse buffer would emit chunks backwards"
        );
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
/// Panics on a [`RevBuf`], and unusable as a map key or value. See [`PbDisplay`].
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
    type Order = Forward;

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

    // --- Reverse-buffer guards. Without them each of these silently corrupts output ---

    #[test]
    #[should_panic(expected = "Tack is forward-only")]
    fn pb_display_into_rev_buf_panics() {
        let mut backing = [0u8; 64];
        let mut rb = crate::RevBuf::new(&mut backing);
        <PbDisplay<'_, u32> as ProtoEncode<PbString>>::encode(&mut rb, &PbDisplay(&42u32));
    }

    #[cfg(feature = "std")]
    #[test]
    #[should_panic(expected = "Tack is forward-only")]
    fn pb_write_into_rev_buf_panics() {
        let mut backing = [0u8; 64];
        let mut rb = crate::RevBuf::new(&mut backing);
        let w = PbWrite(|w: &mut dyn std::io::Write| w.write_all(b"payload"));
        <PbWrite<_> as ProtoEncode<PbBytes>>::encode(&mut rb, &w);
    }

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

    #[test]
    #[should_panic(expected = "FmtWriter appends")]
    fn fmt_writer_into_rev_buf_panics() {
        let mut backing = [0u8; 64];
        let mut rb = crate::RevBuf::new(&mut backing);
        write!(FmtWriter(&mut rb), "{}", 42).unwrap();
    }

    #[cfg(feature = "std")]
    #[test]
    #[should_panic(expected = "IoWriter appends")]
    fn io_writer_into_rev_buf_panics() {
        use std::io::Write as _;
        let mut backing = [0u8; 64];
        let mut rb = crate::RevBuf::new(&mut backing);
        IoWriter(&mut rb).write_all(b"chunk").unwrap();
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
    /// all-lengths sweep. Both cases write next to existing bytes on purpose, since with
    /// overlapping stores a check that only compares the payload misses a write past `n`.
    #[test]
    fn put_slice_ladder_all_lengths_slice_and_rev() {
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

            // Reverse prepends, so the payload goes *before* the earlier write and the bytes
            // below `pos` are the ones that must stay untouched.
            let mut backing = [0xCCu8; 200];
            let mut rb = RevBuf::new(&mut backing);
            rb.put_slice(b"suffix");
            rb.put_slice(&src);
            let written = rb.written().to_vec();
            assert_eq!(written.len(), 6 + n, "RevBuf len wrong at n={n}");
            assert_eq!(&written[..n], &src[..], "RevBuf payload wrong at n={n}");
            assert_eq!(&written[n..], b"suffix", "RevBuf suffix clobbered at n={n}");
            assert!(
                backing[..200 - (6 + n)].iter().all(|&b| b == 0xCC),
                "RevBuf wrote below pos at n={n}"
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

    /// The trait promises `None` when the window will not fit; `claim` on its own asserts.
    #[test]
    fn rev_buf_claim_block_refuses_rather_than_panics() {
        let mut backing = [0u8; 4];
        let mut rb = RevBuf::new(&mut backing);
        assert!(rb.claim_block(100).is_none());
        assert_eq!(rb.claim_block(4).map(|w| w.len()), Some(4));
        // Consumed the whole buffer, so a second claim of any size is refused.
        assert!(rb.claim_block(1).is_none());
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
