//! Base module providing tools for working with protobuf scalars and maps where fields are scalars.

use bytes::Buf;

use crate::{scalars::*, tack::Tack};
use std::marker::PhantomData;

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
impl_wrapped!(Optional, Repeated, Required, Packed, Plain);

/// currently Oneof fields just get flattened into the main schema, so this is unused.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct OneOf<O>(PhantomData<O>);

#[derive(Debug, PartialEq, Eq, Default)]
pub struct PbMap<K, V>(PhantomData<(K, V)>); // Map<PbString,Int32>
impl<K, V> Copy for PbMap<K, V> {}
impl<K, V> Clone for PbMap<K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

//field labels/modifiers that can be applied to the above (except maps and oneOfs)

/// a complete field in a message, field number and type
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
        pub fn write<V: ProtoEncode<P>>(self, buf: &mut Vec<u8>, value: Option<V>) -> Self {
            if let Some(value) = value {
                let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
                t.write(buf);
                P::write_value(value.as_scalar(), buf);
            }
            Field::new()
        }
    }

    impl<const N: u32, M: MessageSchema> Field<N, Optional<M>> {
        pub fn write_msg(self, buf: &mut Vec<u8>, mut f: impl FnMut(&mut Vec<u8>, M)) -> Self {
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
        #[inline]
        pub fn write<V: ProtoEncode<P>>(
            self,
            buf: &mut Vec<u8>,
            values: impl IntoIterator<Item = V>,
        ) -> Field<N, Repeated<P>> {
            let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
            for value in values {
                t.write(buf);
                P::write_value(value.as_scalar(), buf);
            }
            Field::new()
        }
        #[inline]
        pub fn write_single<V: ProtoEncode<P>>(self, buf: &mut Vec<u8>, value: V) -> Self {
            let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
            t.write(buf);
            P::write_value(value.as_scalar(), buf);
            Field::new()
        }
    }

    impl<const N: u32, M: MessageSchema> Field<N, Repeated<M>> {
        pub fn write_msg(self, buf: &mut Vec<u8>, mut f: impl FnMut(&mut Vec<u8>, M)) -> Self {
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
        #[inline]
        pub fn write<V: ProtoEncode<P>>(
            self,
            buf: &mut Vec<u8>,
            values: impl IntoIterator<Item = V>,
        ) -> Field<N, Packed<P>> {
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);
            let t = Tack::new_with_width(buf, 2);
            for value in values {
                let value = value.as_scalar();
                P::write_value(value, t.buffer);
            }
            Field::new()
        }

        /// Like `write`, but requires an ExactSizeIterator. For fixed-size types (float, double,
        /// fixed32, fixed64, sfixed32, sfixed64), this bypasses the Tack and writes the length
        /// prefix directly, which is significantly faster.
        #[inline]
        pub fn write_exact<I>(self, buf: &mut Vec<u8>, values: I) -> Field<N, Packed<P>>
        where
            I: IntoIterator<Item: ProtoEncode<P>>,
            I::IntoIter: ExactSizeIterator,
        {
            let it = values.into_iter();
            if let Some(fixed_size) = P::FIXED_WIRE_SIZE {
                let count = it.len();
                if count == 0 {
                    return Field::new();
                }
                let data_len = count * fixed_size;
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

        fn next(&mut self) -> Option<Self::Item> {
            if self.buf.is_empty() {
                return None;
            }
            let mut buf = self.buf;
            let out = T::read(&mut buf);
            self.buf = if out.is_ok() { buf } else { &[] };
            Some(out)
        }
    }
}

pub mod required {
    use super::*;

    impl<const N: u32, P: ProtobufScalar> Field<N, Required<P>> {
        pub fn write<V: ProtoEncode<P>>(
            self,
            buf: &mut Vec<u8>,
            value: V,
        ) -> Field<N, Required<P>> {
            let t = const { EncodedTag::new(N, P::WIRE_TYPE) };
            t.write(buf);
            P::write_value(value.as_scalar(), buf);
            Field::new()
        }
    }

    impl<const N: u32, M: MessageSchema> Field<N, Required<M>> {
        pub fn write_msg(
            self,
            buf: &mut Vec<u8>,
            mut func: impl FnMut(&mut Vec<u8>, M),
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
        pub fn write<V: ProtoEncode<P>>(self, buf: &mut Vec<u8>, value: V) -> Field<N, Plain<P>> {
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
        pub fn write_msg(
            self,
            buf: &mut Vec<u8>,
            mut func: impl FnMut(&mut Vec<u8>, M),
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
    pub fn read_msg<'a, T>(
        buf: &mut &'a [u8],
        decoder: impl Fn(&'a [u8]) -> T,
    ) -> Result<(K::RustType<'a>, Option<T>), DecodeError> {
        let mut key = None;
        let mut val = None;
        let mut entry_buf = decode_len(buf)?;
        while entry_buf.has_remaining() {
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
    pub fn read<'a>(
        buf: &mut &'a [u8],
    ) -> Result<(K::RustType<'a>, Option<V::RustType<'a>>), DecodeError> {
        let mut key = None;
        let mut val = None;
        let mut entry_buf = decode_len(buf)?;
        // we expect exactly two fields, with tags 1 and 2, but protobuf doesnt enforce any order, so we loop until we find both or run out of data.
        // maps are technically {optional X key = 1; optional Y value = 2}, so we would have to handle the case where they can be missing
        // the official docs do cover the "key present, value missing" case. the "value present, key missing" case seems to be silently regarded as an error.
        // in proto3 this would return the defaults. here return with explicit presence, codegen can decide how to handle that.
        while entry_buf.has_remaining() {
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
        pub fn write<I: IntoIterator<Item = (A, B)>, A: ProtoEncode<K>, B: ProtoEncode<V>>(
            self,
            buf: &mut Vec<u8>,
            values: I,
        ) -> Field<N, PbMap<K, V>> {
            for (k, v) in values {
                self.write_entry(buf, k, Some(v));
            }
            Field::new()
        }
        // writing {key; none} is useful as a way to delete entries in an update message
        pub fn write_entry<A: ProtoEncode<K>, B: ProtoEncode<V>>(
            self,
            buf: &mut Vec<u8>,
            key: A,
            value: Option<B>,
        ) -> Field<N, PbMap<K, V>> {
            // the tag and wire type for the map field itself
            let t = const { EncodedTag::new(N, WireType::LEN) };
            t.write(buf);

            let k = key.as_scalar();
            let v = value.as_ref().map(|v| v.as_scalar());
            // len of the entry message, which is 1 (for the key) + len of the key + (0 if value is None else 1 + len of value)
            let len = K::len(1, k) + v.map(|v| V::len(1, v)).unwrap_or(0);
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
        pub fn write_msg<A: ProtoEncode<K>>(
            self,
            buf: &mut Vec<u8>,
            key: A,
            mut value: impl FnMut(&mut Vec<u8>, M),
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

// Marker trait for a protobuf schema generated by tacky-build
pub trait MessageSchema: Default {}

/// Trait for allowing a wide array of types to be encoded as protobuf scalars
/// if you have a custom type that you want to be able to write as a protobuf scalar, implement this trait for it.
/// if you can implement as_scalar, everything works as expected.
/// if you cant implement as_scalar, you can still implement encode directly, at the cost of being unable to Packed<your type> fields.
pub trait ProtoEncode<P: ProtobufScalar> {
    /// this should return the value to be encoded as a protobuf scalar, which will then be passed to the appropriate write_value function for the scalar type.
    /// if your type can be converted to the protobuf scalar type, this is straightforward.
    /// if not, you can implement encode directly, but you wont be able to use Packed<your type> fields, since those rely on as_scalar for calculating the length of the packed field.
    /// in that case, leave this as unimplemented!()
    fn as_scalar(&self) -> P::RustType<'_>;
    /// checks if a field is equal to its protobuf default value.
    /// used to skip writing default values when using the Plain field modifier.
    fn is_default(&self) -> bool {
        self.as_scalar() == P::RustType::default()
    }
    /// default implementation of encode, which works for all types that can be converted to protobuf scalars.
    /// if your type cant be converted to the protobuf scalar type, you can implement encode directly.
    fn encode(buf: &mut Vec<u8>, value: &Self) {
        let value = value.as_scalar();
        P::write_value(value, buf);
    }
}

impl<T: Copy + Into<i32> + TryFrom<i32> + Default + PartialEq> ProtoEncode<PbEnum<T>> for T {
    #[inline]
    fn as_scalar(&self) -> <PbEnum<T> as ProtobufScalar>::RustType<'_> {
        *self
    }
}

impl<T: Copy + Into<i32> + TryFrom<i32> + Default + PartialEq> ProtoEncode<PbEnum<T>> for &T {
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
    use super::*;

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
