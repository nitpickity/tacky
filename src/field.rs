//! Field schema types and their serialization/deserialization logic.
//!
//! A protobuf field is represented as `Field<N, Label<Scalar>>` where:
//! - `N` is the field number (const generic, known at compile time)
//! - `Label` is one of [`Optional`], [`Repeated`], [`Packed`], [`Required`], [`Plain`], or [`PbMap`]
//! - `Scalar` is a marker type from [`scalars`](`crate::scalars`) or a [`MessageSchema`] implementor
//!
//! All of these are zero-sized. A generated message schema struct composed entirely
//! of `Field` types has `size_of::<T>() == 0`.

use crate::buf::WriteBuf;
use crate::{scalars::*, tack::Tack};
use core::marker::PhantomData;

macro_rules! impl_wrapped {
    ($($t:ident),*) => {
        $(
            #[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
            pub struct $t<P>(PhantomData<P>);
            impl<P> $t<P> {
                pub fn new() -> $t<P> {
                    $t(PhantomData)
                }
            }
        )*
    };
}

// Field label types — all zero-sized wrappers over PhantomData.
//
// - Optional<P>: present or absent. Takes Option<V>, skips the field if None.
//   Used for proto2 `optional` and proto3 explicit `optional`.
// - Repeated<P>: unpacked repeated field. Takes IntoIterator<Item=V>, writes each
//   element with its own tag. Used for length-delimited types that can't be packed.
// - Required<P>: always written. Takes a bare value. Proto2 `required` fields.
// - Packed<P>: packed repeated field. Writes all elements under a single
//   length-delimited tag. More compact for numeric types.
// - Plain<P>: implicit presence (proto3 default). Skips the field if the value
//   equals the type's default (0, false, empty string, etc.).
impl_wrapped!(Optional, Repeated, Required, Packed, Plain);

/// Map field type, generic over key and value scalar types.
/// On the wire, each map entry is a length-delimited message with field 1 = key, field 2 = value.
#[derive(Debug, PartialEq, Eq, Default)]
pub struct PbMap<K, V>(PhantomData<(K, V)>);
impl<K, V> Copy for PbMap<K, V> {}
impl<K, V> Clone for PbMap<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A single field in a protobuf message schema.
///
/// `N` is the field number and `P` is the field type (label + scalar, e.g. `Optional<Int32>`).
/// Both are compile-time information — `Field` is zero-sized and carries no runtime data.
///
/// `.write()` consumes and returns `self`, which enables the exhaustiveness pattern:
/// the return value can be assigned back into a struct literal for compile-time
/// completeness checking, while the actual serialization happens as a side effect.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct Field<const N: u32, P>(PhantomData<P>);
impl<const N: u32, P> Field<N, P> {
    pub const fn new() -> Field<N, P> {
        Field(PhantomData)
    }
}

pub mod optional {
    use super::*;
    impl<const N: u32, P: ProtobufScalar> Field<N, Optional<P>> {
        /// Writes the field if `value` is `Some`, skips it if `None`.
        pub fn write<V: ProtoEncode<P>>(self, buf: &mut impl WriteBuf, value: Option<V>) -> Self {
            if let Some(value) = value {
                let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
                t.write(buf);
                P::write_value(value.as_scalar(), buf);
            }
            Field::new()
        }
    }

    impl<const N: u32, M: MessageSchema> Field<N, Optional<M>> {
        /// Writes a nested message field. The closure receives the buffer (through the
        /// Tack's borrow) and a default schema instance for the nested message.
        /// The length prefix is patched automatically when the closure returns.
        pub fn write_msg<B: WriteBuf>(self, buf: &mut B, mut f: impl FnMut(&mut B, M)) -> Self {
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);
            let t = Tack::new(buf);
            f(t.buffer, M::default());
            Field::new()
        }
    }
}

pub mod repeated {
    use super::*;
    impl<const N: u32, P: ProtobufScalar> Field<N, Repeated<P>> {
        /// Writes each element with its own tag. For non-packed repeated fields.
        #[inline]
        pub fn write<V: ProtoEncode<P>>(
            self,
            buf: &mut impl WriteBuf,
            values: impl IntoIterator<Item = V>,
        ) -> Field<N, Repeated<P>> {
            let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
            for value in values {
                t.write(buf);
                P::write_value(value.as_scalar(), buf);
            }
            Field::new()
        }
        /// Writes a single element to a repeated field. Convenience for appending
        /// one value without wrapping it in an iterator.
        #[inline]
        pub fn write_single<V: ProtoEncode<P>>(self, buf: &mut impl WriteBuf, value: V) -> Self {
            let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
            t.write(buf);
            P::write_value(value.as_scalar(), buf);
            Field::new()
        }
    }

    impl<const N: u32, M: MessageSchema> Field<N, Repeated<M>> {
        /// Writes one nested message to a repeated message field. Call multiple times
        /// to write multiple messages — each call produces one entry.
        pub fn write_msg<B: WriteBuf>(self, buf: &mut B, mut f: impl FnMut(&mut B, M)) -> Self {
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);
            let t = Tack::new(buf);
            f(t.buffer, M::default());
            Field::new()
        }
    }
}

pub mod packed {
    use super::*;
    impl<const N: u32, P: Packable> Field<N, Packed<P>> {
        /// Writes all elements under a single length-delimited tag.
        /// Uses a [`Tack`] with width 2 (~16KB) to avoid a two-pass length calculation.
        /// Skips the field entirely if the iterator is empty.
        #[inline]
        pub fn write<V: ProtoEncode<P>>(
            self,
            buf: &mut impl WriteBuf,
            values: impl IntoIterator<Item = V>,
        ) -> Field<N, Packed<P>> {
            let mut iter = values.into_iter();
            let Some(first) = iter.next() else {
                return Field::new();
            };
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);
            let t = Tack::new_with_width(buf, 2);
            P::write_value(first.as_scalar(), t.buffer);
            for value in iter {
                P::write_value(value.as_scalar(), t.buffer);
            }
            Field::new()
        }

        /// Like `write`, but requires an `ExactSizeIterator`. For fixed-size types (float, double,
        /// fixed32, etc.), this bypasses the Tack entirely and writes the length prefix
        /// directly since `count * fixed_size` gives the exact byte length upfront.
        /// For varint types, falls back to the Tack since encoded size depends on values.
        #[inline]
        pub fn write_exact<I>(self, buf: &mut impl WriteBuf, values: I) -> Field<N, Packed<P>>
        where
            I: IntoIterator<Item: ProtoEncode<P>>,
            I::IntoIter: ExactSizeIterator,
        {
            let it = values.into_iter();
            if it.len() == 0 {
                return Field::new();
            }
            if let Some(fixed_size) = P::FIXED_WIRE_SIZE {
                let data_len = it.len() * fixed_size;
                let tag = const { EncodedTag::new(N, WireType::LEN) };
                tag.write(buf);
                write_varint(data_len as u64, buf);
                for value in it {
                    P::write_value(value.as_scalar(), buf);
                }
                return Field::new();
            }
            // Varint types: still need Tack since encoded size depends on values
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);
            let t = Tack::new_with_width(buf, 2);
            for value in it {
                P::write_value(value.as_scalar(), t.buffer);
            }
            Field::new()
        }
    }
    /// Iterator over values in a packed repeated field during deserialization.
    /// Yields one decoded scalar per call to `next()`. Borrows the packed
    /// byte slice — no allocation needed.
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct PackedIter<'a, T: Packable> {
        buf: &'a [u8],
        _t: PhantomData<T>,
    }

    impl<'a, T: Packable> PackedIter<'a, T> {
        pub fn new(buf: &'a [u8]) -> Self {
            Self {
                buf,
                _t: PhantomData,
            }
        }
    }

    impl<'a, T: Packable> Iterator for PackedIter<'a, T> {
        type Item = Result<T::RustType<'a>, DecodeError>;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if self.buf.is_empty() {
                return None;
            }
            match T::read(&mut self.buf) {
                Ok(v) => Some(Ok(v)),
                Err(e) => {
                    self.buf = &[];
                    Some(Err(e))
                }
            }
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            // Each element is at least 1 byte.
            (0, Some(self.buf.len()))
        }
    }
}

pub mod required {
    use super::*;

    impl<const N: u32, P: ProtobufScalar> Field<N, Required<P>> {
        pub fn write<V: ProtoEncode<P>>(
            self,
            buf: &mut impl WriteBuf,
            value: V,
        ) -> Field<N, Required<P>> {
            let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
            t.write(buf);
            P::write_value(value.as_scalar(), buf);
            Field::new()
        }
    }

    impl<const N: u32, M: MessageSchema> Field<N, Required<M>> {
        pub fn write_msg<B: WriteBuf>(
            self,
            buf: &mut B,
            mut func: impl FnMut(&mut B, M),
        ) -> Field<N, Required<M>> {
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);
            let t = Tack::new(buf);
            func(t.buffer, M::default());
            Field::new()
        }
    }
}

pub mod plain {
    use super::*;

    impl<const N: u32, P: ProtobufScalar> Field<N, Plain<P>> {
        /// Writes the field only if the value differs from the protobuf default
        /// (0, false, empty string, etc.). This is proto3's implicit presence behavior.
        pub fn write<V: ProtoEncode<P>>(
            self,
            buf: &mut impl WriteBuf,
            value: V,
        ) -> Field<N, Plain<P>> {
            if value.is_default() {
                return Field::new();
            }
            let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
            t.write(buf);
            P::write_value(value.as_scalar(), buf);
            Field::new()
        }
    }
    impl<const N: u32, M: MessageSchema> Field<N, Plain<M>> {
        pub fn write_msg<B: WriteBuf>(
            self,
            buf: &mut B,
            mut func: impl FnMut(&mut B, M),
        ) -> Field<N, Plain<M>> {
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);
            let t = Tack::new(buf);
            func(t.buffer, M::default());
            Field::new()
        }
    }
}

impl<K, V> PbMap<K, V> {
    pub fn new() -> PbMap<K, V> {
        PbMap(PhantomData)
    }
}

impl<K: ProtobufScalar, M: MessageSchema> PbMap<K, M> {
    /// Decodes a map entry where the value is a nested message.
    /// The decoder closure receives the raw message bytes and returns the parsed result.
    pub fn read_msg<'a, T>(
        buf: &mut &'a [u8],
        decoder: impl Fn(&'a [u8]) -> T,
    ) -> Result<(K::RustType<'a>, Option<T>), DecodeError> {
        let mut key = None;
        let mut val = None;
        let mut entry_buf = decode_len(buf)?;
        while !entry_buf.is_empty() {
            match decode_key(&mut entry_buf)? {
                (1, wt) => {
                    check_wire_type(wt, K::WIRE_TYPE, "key")?;
                    key = Some(K::read(&mut entry_buf)?);
                }
                (2, wt) => {
                    check_wire_type(wt, WireType::LEN, "value")?;
                    let msg_buf = decode_len(&mut entry_buf)?;
                    val = Some(decoder(&msg_buf));
                }
                (_, wt) => {
                    skip_field(wt, &mut entry_buf)?;
                }
            }
        }
        let Some(key) = key else {
            return Err(DecodeError::InvalidMapEntry);
        };
        Ok((key, val))
    }
}
impl<K: ProtobufScalar, V: ProtobufScalar> PbMap<K, V> {
    /// Decodes a single map entry into `(key, Option<value>)`.
    ///
    /// The value is `Option` because protobuf allows entries with a key but no value
    /// (proto3 treats this as the default value; tacky surfaces the absence explicitly).
    /// A missing key is an error since there's no meaningful default for map keys.
    pub fn read<'a>(
        buf: &mut &'a [u8],
    ) -> Result<(K::RustType<'a>, Option<V::RustType<'a>>), DecodeError> {
        let mut key = None;
        let mut val = None;
        let mut entry_buf = decode_len(buf)?;
        while !entry_buf.is_empty() {
            match decode_key(&mut entry_buf)? {
                (1, wt) => {
                    check_wire_type(wt, K::WIRE_TYPE, "key")?;
                    key = Some(K::read(&mut entry_buf)?);
                }
                (2, wt) => {
                    check_wire_type(wt, V::WIRE_TYPE, "value")?;
                    val = Some(V::read(&mut entry_buf)?);
                }
                (_, wt) => {
                    skip_field(wt, &mut entry_buf)?;
                }
            }
        }
        let Some(key) = key else {
            return Err(DecodeError::InvalidMapEntry);
        };
        Ok((key, val))
    }
}

pub mod maps {
    use super::*;
    impl<const N: u32, K: ProtobufScalar, V: ProtobufScalar> Field<N, PbMap<K, V>> {
        /// Writes all key-value pairs from an iterator. Accepts anything that yields
        /// pairs of encodable types — `HashMap`, `BTreeMap`, arrays of tuples, etc.
        pub fn write<I: IntoIterator<Item = (A, B)>, A: ProtoEncode<K>, B: ProtoEncode<V>>(
            self,
            buf: &mut impl WriteBuf,
            values: I,
        ) -> Field<N, PbMap<K, V>> {
            for (k, v) in values {
                self.write_entry(buf, k, Some(v));
            }
            Field::new()
        }
        /// Writes a single map entry. The value is `Option` so that key-only entries
        /// can represent deletions in update messages.
        pub fn write_entry<A: ProtoEncode<K>, B: ProtoEncode<V>>(
            self,
            buf: &mut impl WriteBuf,
            key: A,
            value: Option<B>,
        ) -> Field<N, PbMap<K, V>> {
            // the tag and wire type for the map field itself
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);

            let k = key.as_scalar();
            let v = value.as_ref().map(|v| v.as_scalar());
            let len = K::len(1, k) + v.map(|v| V::len(2, v)).unwrap_or(0);
            write_varint(len as u64, buf);
            let t = const { EncodedTag::new(1, K::WIRE_TYPE) };
            t.write(buf);

            A::encode(buf, &key);
            if let Some(value) = value {
                let t = const { EncodedTag::new(2, V::WIRE_TYPE) };
                t.write(buf);
                B::encode(buf, &value);
            }
            Field::new()
        }
    }
    impl<const N: u32, K: ProtobufScalar, M: MessageSchema> Field<N, PbMap<K, M>> {
        /// Writes a map entry where the value is a nested message, written via closure.
        pub fn write_msg<B: WriteBuf, A: ProtoEncode<K>>(
            self,
            buf: &mut B,
            key: A,
            mut value: impl FnMut(&mut B, M),
        ) -> Field<N, PbMap<K, M>> {
            let tag = const { EncodedTag::new(N, WireType::LEN) };
            tag.write(buf);
            let t = Tack::new_with_width(buf, 2);
            let tag = const { EncodedTag::new(1, K::WIRE_TYPE) };
            tag.write(t.buffer);
            A::encode(t.buffer, &key);
            {
                let tag = const { EncodedTag::new(2, WireType::LEN) };
                tag.write(t.buffer);
                let tt = Tack::new_with_width(t.buffer, 2);
                value(tt.buffer, M::default());
            }
            Field::new()
        }
    }
}

/// Marker trait for generated message schema types. Implemented by `tacky-build`
/// on every generated schema struct. Used as a bound on `Field`'s `write_msg`
/// methods to distinguish nested message fields from scalar fields.
pub trait MessageSchema: Default {}

/// Bridges domain types to protobuf scalars for serialization.
///
/// Implement this for your own types to make them directly writable through tacky.
/// For example, a `UserId(u64)` could implement `ProtoEncode<Uint64>` by returning
/// the inner value from `as_scalar()`.
///
/// The default implementations cover the standard Rust primitives: `i32` encodes as
/// `Int32`/`Sint32`/`Sfixed32`, anything `AsRef<str>` encodes as `PbString`, etc.
pub trait ProtoEncode<P: ProtobufScalar> {
    /// Returns the value as the protobuf scalar's Rust type for encoding.
    /// Must be implemented. Packed fields rely on this to compute element sizes.
    fn as_scalar(&self) -> P::RustType<'_>;
    /// Returns true if this value equals the protobuf default.
    /// Used by [`Plain`] fields to skip writing default values (proto3 semantics).
    fn is_default(&self) -> bool {
        self.as_scalar() == P::RustType::default()
    }
    /// Writes the encoded value to the buffer. The default implementation calls
    /// `as_scalar()` and delegates to `P::write_value`. Override this directly
    /// if your type can't cheaply produce the scalar's Rust type — but note that
    /// packed fields won't work without `as_scalar()`.
    fn encode(buf: &mut impl WriteBuf, value: &Self) {
        let value = value.as_scalar();
        P::write_value(value, buf);
    }
}

impl<T: PbEnumType> ProtoEncode<PbEnum<T>> for T {
    #[inline]
    fn as_scalar(&self) -> <PbEnum<T> as ProtobufScalar>::RustType<'_> {
        *self
    }
}

impl<T: PbEnumType> ProtoEncode<PbEnum<T>> for &T {
    #[inline]
    fn as_scalar(&self) -> <PbEnum<T> as ProtobufScalar>::RustType<'_> {
        **self
    }
}

impl<T: AsRef<str>> ProtoEncode<PbString> for T {
    #[inline]
    fn as_scalar(&self) -> <PbString as ProtobufScalar>::RustType<'_> {
        self.as_ref()
    }

    fn is_default(&self) -> bool {
        self.as_ref().is_empty()
    }
}

impl<T: AsRef<[u8]>> ProtoEncode<PbBytes> for T {
    #[inline]
    fn as_scalar(&self) -> <PbBytes as ProtobufScalar>::RustType<'_> {
        self.as_ref()
    }

    fn is_default(&self) -> bool {
        self.as_ref().is_empty()
    }
}

macro_rules! gen_encodes {
    ($src:ty => $($dst:ty),*) => {
        $(
            impl ProtoEncode<$dst> for $src {
                #[inline]
                fn as_scalar(&self) -> <$dst as ProtobufScalar>::RustType<'_> {
                    *self
                }
                fn is_default(&self) -> bool {
                    *self == Self::default()
                }

            }
            impl<'a> ProtoEncode<$dst> for &'a $src {
                #[inline]
                fn as_scalar(&self) -> <$dst as ProtobufScalar>::RustType<'_> {
                    **self
                }
                fn is_default(&self) -> bool {
                    <$src as ProtoEncode<$dst>>::is_default(*self)
                }
            }
        )*
    };
}

gen_encodes!(i32 => Int32, Sint32, Sfixed32);
gen_encodes!(u32 => Uint32, Fixed32);
gen_encodes!(i64 => Int64, Sint64, Sfixed64);
gen_encodes!(u64 => Uint64, Fixed64);
gen_encodes!(f32 => Float);
gen_encodes!(f64 => Double);
gen_encodes!(bool => Bool);

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::{string::ToString, vec, vec::Vec};

    #[test]
    fn test_map_int_string() {
        let mut buf = Vec::new();

        let input = [(1, "one"), (2, "two")];
        let writer = Field::<1, PbMap<Int32, PbString>>::new();
        writer.write(&mut buf, input);
        let mut slice = buf.as_slice();
        let mut results = Vec::new();
        while !slice.is_empty() {
            let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 1);
            assert_eq!(wire, crate::scalars::WireType::LEN);
            let (k, v) = crate::field::PbMap::<Int32, PbString>::read(&mut slice).unwrap();
            results.push((k, v));
        }
        assert_eq!(results, vec![(1, Some("one")), (2, Some("two"))]);
    }

    #[test]
    fn test_map_string_string() {
        let mut buf = Vec::new();
        let input = [("a", "alpha"), ("b", "beta")];
        let writer = Field::<1, PbMap<PbString, PbString>>::new();
        writer.write(&mut buf, input);
        writer.write_entry(&mut buf, "c", None::<&str>);
        let mut slice = buf.as_slice();
        let mut results = Vec::new();
        while !slice.is_empty() {
            let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 1);
            assert_eq!(wire, crate::scalars::WireType::LEN);

            let (k, v) = crate::field::PbMap::<PbString, PbString>::read(&mut slice).unwrap();
            results.push((k, v));
        }
        assert_eq!(
            results,
            vec![("a", Some("alpha")), ("b", Some("beta")), ("c", None)]
        );
    }

    #[test]
    fn test_map_int_float() {
        let mut buf = Vec::new();
        let input = [(1, 1.5f32), (2, 2.5f32)];
        let writer = Field::<1, PbMap<Int32, Float>>::new();
        writer.write(&mut buf, input);
        let mut slice = buf.as_slice();
        let mut results = Vec::new();
        while !slice.is_empty() {
            let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 1);
            assert_eq!(wire, crate::scalars::WireType::LEN);
            let (k, v) = crate::field::PbMap::<Int32, Float>::read(&mut slice).unwrap();
            results.push((k, v));
        }
        assert_eq!(results, vec![(1, Some(1.5f32)), (2, Some(2.5f32))]);
    }
    #[test]
    fn test_required_string_and_bytes() {
        let mut buf = Vec::new();
        // String
        let _ = Field::<1, Required<PbString>>::new().write(&mut buf, "hello");
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 1);
        assert_eq!(wire, crate::scalars::WireType::LEN);
        let s = PbString::read(&mut slice).unwrap();
        assert_eq!(s, "hello");
        buf.clear();
        // Bytes
        let _ = Field::<2, Required<PbBytes>>::new().write(&mut buf, b"abc");
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 2);
        assert_eq!(wire, crate::scalars::WireType::LEN);
        let b = PbBytes::read(&mut slice).unwrap();
        assert_eq!(b, b"abc");
    }

    #[test]
    fn test_optional_string_and_bytes() {
        let mut buf = Vec::new();
        // String Some
        let _ = Field::<1, Optional<PbString>>::new().write(&mut buf, Some("hello"));
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 1);
        assert_eq!(wire, crate::scalars::WireType::LEN);
        let s = PbString::read(&mut slice).unwrap();
        assert_eq!(s, "hello");
        buf.clear();
        // String None
        let _ = Field::<1, Optional<PbString>>::new().write(&mut buf, None::<&str>);
        assert!(buf.is_empty());
        buf.clear();
        // Bytes Some
        let _ = Field::<2, Optional<PbBytes>>::new().write(&mut buf, Some(b"abc"));
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 2);
        assert_eq!(wire, crate::scalars::WireType::LEN);
        let b = PbBytes::read(&mut slice).unwrap();
        assert_eq!(b, b"abc");
        buf.clear();
        // Bytes None
        let _ = Field::<2, Optional<PbBytes>>::new().write(&mut buf, None::<&[u8]>);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_repeated_string_and_bytes() {
        let mut buf = Vec::new();
        // String
        let _ = Field::<1, Repeated<PbString>>::new().write(&mut buf, vec!["a", "b"]);
        let mut slice = buf.as_slice();
        let mut results = Vec::new();
        while !slice.is_empty() {
            let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 1);
            assert_eq!(wire, crate::scalars::WireType::LEN);
            let s = PbString::read(&mut slice).unwrap();
            results.push(s.to_string());
        }
        assert_eq!(results, vec!["a", "b"]);
        buf.clear();
        // Bytes
        let _ = Field::<2, Repeated<PbBytes>>::new().write(&mut buf, vec![b"x", b"y"]);
        let mut slice = buf.as_slice();
        let mut results = Vec::new();
        while !slice.is_empty() {
            let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 2);
            assert_eq!(wire, crate::scalars::WireType::LEN);
            let b = PbBytes::read(&mut slice).unwrap();
            results.push(b.to_vec());
        }
        assert_eq!(results, vec![b"x".to_vec(), b"y".to_vec()]);
    }

    #[test]
    fn test_required_numeric_types() {
        let mut buf = Vec::new();
        // i32
        let _ = Field::<1, Required<Int32>>::new().write(&mut buf, 42);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 1);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Int32::read(&mut slice).unwrap();
        assert_eq!(v, 42);
        buf.clear();
        // u32
        let _ = Field::<2, Required<Uint32>>::new().write(&mut buf, 123u32);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 2);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Uint32::read(&mut slice).unwrap();
        assert_eq!(v, 123u32);
        buf.clear();
        let _ = Field::<3, Required<Int64>>::new().write(&mut buf, 1000i64);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 3);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Int64::read(&mut slice).unwrap();
        assert_eq!(v, 1000i64);
        buf.clear();
        let _ = Field::<4, Required<Uint64>>::new().write(&mut buf, 1000u64);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 4);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Uint64::read(&mut slice).unwrap();
        assert_eq!(v, 1000u64);
        buf.clear();
        let _ = Field::<5, Required<Float>>::new().write(&mut buf, 1.5f32);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 5);
        assert_eq!(wire, crate::scalars::WireType::I32);
        let v = Float::read(&mut slice).unwrap();
        assert_eq!(v, 1.5f32);
        buf.clear();
        // f64
        let _ = Field::<6, Required<Double>>::new().write(&mut buf, 2.5f64);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 6);
        assert_eq!(wire, crate::scalars::WireType::I64);
        let v = Double::read(&mut slice).unwrap();
        assert_eq!(v, 2.5f64);
        buf.clear();
        // bool
        let _ = Field::<7, Required<Bool>>::new().write(&mut buf, true);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 7);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Bool::read(&mut slice).unwrap();
        assert_eq!(v, true);
        buf.clear();
        let _ = Field::<8, Required<Bool>>::new().write(&mut buf, false);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 8);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Bool::read(&mut slice).unwrap();
        assert_eq!(v, false);
    }

    #[test]
    fn test_optional_numeric_types() {
        let mut buf = Vec::new();
        // i32
        let _ = Field::<1, Optional<Int32>>::new().write(&mut buf, Some(42));
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 1);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Int32::read(&mut slice).unwrap();
        assert_eq!(v, 42);
        buf.clear();
        let _ = Field::<1, Optional<Int32>>::new().write(&mut buf, None::<i32>);
        assert!(buf.is_empty());
        buf.clear();
        // u32
        let _ = Field::<2, Optional<Uint32>>::new().write(&mut buf, Some(123u32));
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 2);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Uint32::read(&mut slice).unwrap();
        assert_eq!(v, 123u32);
        buf.clear();
        // bool
        let _ = Field::<3, Optional<Bool>>::new().write(&mut buf, Some(true));
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 3);
        assert_eq!(wire, crate::scalars::WireType::VARINT);
        let v = Bool::read(&mut slice).unwrap();
        assert_eq!(v, true);
        buf.clear();
        let _ = Field::<3, Optional<Bool>>::new().write(&mut buf, None::<bool>);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_repeated_numeric_types() {
        let mut buf = Vec::new();
        let _ = Field::<1, Repeated<Int32>>::new().write(&mut buf, vec![1, 2, 3]);
        let mut slice = buf.as_slice();
        let mut results = Vec::new();
        while !slice.is_empty() {
            let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 1);
            assert_eq!(wire, crate::scalars::WireType::VARINT);
            let v = Int32::read(&mut slice).unwrap();
            results.push(v);
        }
        assert_eq!(results, vec![1, 2, 3]);
        buf.clear();
        let _ = Field::<2, Repeated<Bool>>::new().write(&mut buf, vec![true, false, true]);
        let mut slice = buf.as_slice();
        let mut results = Vec::new();
        while !slice.is_empty() {
            let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
            assert_eq!(tag, 2);
            assert_eq!(wire, crate::scalars::WireType::VARINT);
            let v = Bool::read(&mut slice).unwrap();
            results.push(v);
        }
        assert_eq!(results, vec![true, false, true]);
    }

    #[test]
    fn test_packed_numeric_types() {
        let mut buf = Vec::new();
        let _ = Field::<1, Packed<Int32>>::new().write(&mut buf, vec![1, 2, 3]);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 1);
        assert_eq!(wire, crate::scalars::WireType::LEN);
        let slice = decode_len(&mut slice).unwrap();
        let packed = crate::field::packed::PackedIter::<Int32>::new(slice);
        let values: Result<Vec<_>, _> = packed.collect();
        assert_eq!(values.unwrap(), vec![1, 2, 3]);
        buf.clear();
        let _ = Field::<2, Packed<Bool>>::new().write(&mut buf, vec![true, false, true]);
        let mut slice = buf.as_slice();
        let (tag, wire) = crate::scalars::decode_key(&mut slice).unwrap();
        assert_eq!(tag, 2);
        assert_eq!(wire, crate::scalars::WireType::LEN);
        let slice = decode_len(&mut slice).unwrap();
        let packed = crate::field::packed::PackedIter::<Bool>::new(slice);
        let values: Result<Vec<_>, _> = packed.collect();
        assert_eq!(values.unwrap(), vec![true, false, true]);
    }
}
