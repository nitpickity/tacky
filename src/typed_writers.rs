//! Base module providing tools for working with protobuf scalars and maps where fields are scalars.

use crate::{scalars::*, tack::Tack};
use std::{fmt::Display, marker::PhantomData};

// compound types
// OneOf<(Field<1,Int32>,Field<3,PbString>)>
// currently unused as Oneof fields just get flattened into the main message
pub struct OneOf<O>(PhantomData<O>);
pub struct PbMap<K, V>(PhantomData<(K, V)>); // Map<PbString,Int32>

//field labels/modifiers that can be applied to the above (except maps and oneOfs)
pub struct Optional<P>(PhantomData<P>); // also applied to proto3 fields with no modifier
pub struct Repeated<P>(PhantomData<P>);
pub struct Required<P>(PhantomData<P>);
pub struct Packed<P>(PhantomData<P>);
pub struct Plain<P>(PhantomData<P>);

// a complete field in a message, field number and type
#[derive(Debug, Copy, Clone)]
pub struct Field<const N: usize, P>(PhantomData<P>);

impl<const N: usize, P> Default for Field<N, P> {
    fn default() -> Self {
        Field::new()
    }
}

macro_rules! impl_wrapped {
    ($t:ident<$p:ident>) => {
        impl<$p> $t<$p> {
            pub fn new() -> $t<$p> {
                $t(PhantomData)
            }
        }
    };
}

impl_wrapped!(Optional<P>);
impl_wrapped!(Repeated<P>);
impl_wrapped!(Required<P>);
impl_wrapped!(Packed<P>);

impl<const N: usize, P> Field<N, P> {
    fn new() -> Field<N, P> {
        Field(PhantomData)
    }
}

pub mod optional {
    use super::*;
    pub struct OptionalValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }

    impl<'b, const N: usize, P> OptionalValueWriter<'b, N, P> {
        pub const fn new(buf: &'b mut Vec<u8>) -> Self {
            Self {
                buf,
                _m: PhantomData,
            }
        }

        #[inline]
        pub fn write<V: ProtoEncode<P>>(self, value: Option<V>) -> Field<N, Optional<P>> {
            if let Some(value) = value {
                <V as ProtoEncode<P>>::encode_tag(N as i32, self.buf);
                <V as ProtoEncode<P>>::encode(self.buf, value);
            }
            Field::new()
        }
    }

    impl<'b, const N: usize> OptionalValueWriter<'b, N, PbString> {
        #[inline]
        pub fn write_fmt<V: Display>(
            self,
            value: Option<V>,
            incl_empty: bool,
        ) -> Field<N, Optional<PbString>> {
            use std::io::Write;
            if let Some(value) = value {
                let mut t = Tack::new_with_width(self.buf, Some(N as u32), 2);
                t.rewind = !incl_empty;
                write!(t.buffer, "{value}").unwrap();
            }
            Field::new()
        }
    }
    impl<'b, const N: usize, M: MessageSchema> OptionalValueWriter<'b, N, M> {
        #[inline]
        pub fn write_msg(self, mut f: impl FnMut(M::Writer<'_>)) -> Field<N, Optional<M>> {
            let w = M::new_writer(self.buf, Some(N as i32));
            f(w);
            Field::new()
        }
    }
}

pub mod repeated {
    use super::*;
    impl<'b, const N: usize, P> RepeatedValueWriter<'b, N, P> {
        pub fn new(buf: &'b mut Vec<u8>) -> Self {
            Self {
                buf,
                _m: PhantomData,
            }
        }

        #[inline]
        pub fn write<V: ProtoEncode<P>>(
            self,
            values: impl IntoIterator<Item = V>,
        ) -> Field<N, Repeated<P>> {
            for value in values {
                <V as ProtoEncode<P>>::encode_tag(N as i32, self.buf);
                <V as ProtoEncode<P>>::encode(self.buf, value);
            }
            Field::new()
        }
        #[inline]
        pub fn append<V: ProtoEncode<P>>(&mut self, value: V) -> &mut Self {
            <V as ProtoEncode<P>>::encode_tag(N as i32, self.buf);
            <V as ProtoEncode<P>>::encode(self.buf, value);
            self
        }
        pub fn close(self) -> Field<N, Repeated<P>> {
            Field::new()
        }
    }

    impl<'b, const N: usize, M: MessageSchema> RepeatedValueWriter<'b, N, M> {
        #[inline]
        pub fn append_msg_with(&mut self, mut func: impl FnMut(M::Writer<'_>)) -> &mut Self {
            let writer = M::new_writer(self.buf, Some(N as i32));
            func(writer);
            self
        }
    }

    impl<'b, const N: usize> RepeatedValueWriter<'b, N, PbString> {
        #[inline]
        pub fn write_fmt<V: Display>(
            self,
            values: impl IntoIterator<Item = V>,
            incl_empty: bool,
        ) -> Field<N, Repeated<PbString>> {
            for value in values {
                use std::io::Write;
                let mut t = Tack::new_with_width(self.buf, Some(N as u32), 2);
                t.rewind = !incl_empty;
                write!(t.buffer, "{value}").unwrap();
            }
            Field::new()
        }
    }
    pub struct RepeatedValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }
}

pub mod packed {
    //todo: not all scalars can be packed (strings, bytes),
    // make this more typesafe by not implementing it on those.
    use super::*;
    pub struct PackedValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }

    impl<'b, const N: usize, P> PackedValueWriter<'b, N, P> {
        pub fn new(buf: &'b mut Vec<u8>) -> Self {
            Self {
                buf,
                _m: PhantomData,
            }
        }
        #[inline]
        pub fn write<V: ProtoEncode<P>>(
            self,
            values: impl IntoIterator<Item = V>,
        ) -> Field<N, Packed<P>> {
            let vv = values.into_iter();
            let (_, n) = vv.size_hint();
            let width = n.map(|n| encoded_len_varint(n as u64)).unwrap_or(4);
            let t = Tack::new_with_width(self.buf, Some(N as u32), width as u32);
            for value in vv {
                <V as ProtoEncode<P>>::encode(t.buffer, value)
            }
            drop(t);
            Field::new()
        }
    }
}

pub mod required {
    use super::*;
    pub struct RequiredValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }
    impl<'b, const N: usize, P> RequiredValueWriter<'b, N, P> {
        pub fn new(buf: &'b mut Vec<u8>) -> Self {
            Self {
                buf,
                _m: PhantomData,
            }
        }

        pub fn write<V: ProtoEncode<P>>(self, value: V) -> Field<N, Required<P>> {
            <V as ProtoEncode<P>>::encode_tag(N as i32, self.buf);
            <V as ProtoEncode<P>>::encode(self.buf, value);
            Field::new()
        }
    }

    impl<'b, const N: usize, M: MessageSchema> RequiredValueWriter<'b, N, M> {
        pub fn write_with(self, mut func: impl FnMut(M::Writer<'_>)) -> Field<N, Required<M>> {
            let w = M::new_writer(self.buf, Some(N as i32));
            func(w);
            Field::new()
        }
    }
}

pub mod plain {
    use super::*;
    pub struct PlainValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }

    impl<'b, const N: usize, P> PlainValueWriter<'b, N, P> {
        pub fn new(buf: &'b mut Vec<u8>) -> Self {
            Self {
                buf,
                _m: PhantomData,
            }
        }

        pub fn write<V: ProtoEncode<P>>(self, value: V) -> Field<N, Plain<P>> {
            <V as ProtoEncode<P>>::encode_tag(N as i32, self.buf);
            <V as ProtoEncode<P>>::encode(self.buf, value);
            Field::new()
        }
    }
}

impl<K, V> PbMap<K, V> {
    pub fn new() -> PbMap<K, V> {
        PbMap(PhantomData)
    }
}

/// Writer for simple maps where the key/values are scalars
pub struct MapEntryWriter<'b, const N: usize, K, V> {
    buf: &'b mut Vec<u8>,
    _pbtype: PhantomData<(K, V)>,
}

impl<'b, const N: usize, K: ProtobufScalar, V: ProtobufScalar> MapEntryWriter<'b, N, K, V> {
    pub fn new(buf: &'b mut Vec<u8>) -> Self {
        Self {
            buf,
            _pbtype: PhantomData,
        }
    }

    pub fn write<'a>(&mut self, key: K::RustType<'a>, value: V::RustType<'a>) {
        let tag = (N << 3) | 2;
        write_varint(tag as u64, self.buf);
        let len = K::len(1, key) + V::len(2, value);
        write_varint(len as u64, self.buf);
        K::write(1, key, self.buf);
        V::write(2, value, self.buf);
    }
}

pub struct MapWriter<'b, const N: usize, K, V> {
    buf: &'b mut Vec<u8>,
    _pbtype: PhantomData<(K, V)>,
}

impl<'b, const N: usize, K, V> MapWriter<'b, N, K, V> {
    pub fn new(buf: &'b mut Vec<u8>) -> Self {
        Self {
            buf,
            _pbtype: PhantomData,
        }
    }

    pub fn insert<A: ProtoEncode<K>, B: ProtoEncode<V>>(&mut self, key: A, value: B) {
        let t = Tack::new(self.buf, Some(N as u32));
        <A as ProtoEncode<K>>::encode_tag(1, t.buffer);
        A::encode(t.buffer, key);
        <B as ProtoEncode<V>>::encode_tag(2, t.buffer);
        B::encode(t.buffer, value);
    }

    pub fn close(self) -> Field<N, PbMap<K, V>> {
        Field::new()
    }
    pub fn write<I: IntoIterator<Item = (A, B)>, A: ProtoEncode<K>, B: ProtoEncode<V>>(
        &mut self,
        values: I,
    ) -> Field<N, PbMap<K, V>> {
        for (k, v) in values {
            self.insert(k, v);
        }
        Field::new()
    }
}

// automatically codegened for a message in a proto file.
pub trait MessageSchema {
    type Writer<'a>;
    fn new_writer(buffer: &mut Vec<u8>, tag: Option<i32>) -> Self::Writer<'_>;
}

pub trait ProtoEncode<P> {
    const WIRE_TYPE: WireType;
    #[inline]
    fn encode_tag(field_nr: i32, buf: &mut Vec<u8>) {
        let tag = (field_nr << 3) | (Self::WIRE_TYPE as i32);
        write_varint(tag as u64, buf)
    }

    fn encode(buf: &mut Vec<u8>, value: Self);
}

impl<T: AsRef<str>> ProtoEncode<PbString> for T {
    const WIRE_TYPE: WireType = PbString::WIRE_TYPE;
    #[inline]
    fn encode(buf: &mut Vec<u8>, value: Self) {
        PbString::write_value(value.as_ref(), buf)
    }
}

impl<T: AsRef<[u8]>> ProtoEncode<PbBytes> for T {
    const WIRE_TYPE: WireType = PbBytes::WIRE_TYPE;
    #[inline]
    fn encode(buf: &mut Vec<u8>, value: Self) {
        PbBytes::write_value(value.as_ref(), buf)
    }
}

macro_rules! gen_encodes {
    ($src:ty => $($dst:ty),*) => {
        $(
            impl ProtoEncode<$dst> for $src {
                const WIRE_TYPE: WireType = <$dst>::WIRE_TYPE;
                #[inline]
                fn encode(buf: &mut Vec<u8>, value: Self) {
                    <$dst>::write_value(value, buf)
                }

            }
            impl<'a> ProtoEncode<$dst> for &'a $src {
                const WIRE_TYPE: WireType = <$dst>::WIRE_TYPE;
                #[inline]
                fn encode( buf: &mut Vec<u8>, value: Self) {
                    <$dst>::write_value(*value, buf)
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
    #[test]
    fn test_map_int_string() {
        let mut buf = Vec::new();
        let mut writer = MapEntryWriter::<1, Int32, PbString>::new(&mut buf);
        writer.write(1, "one");
        writer.write(2, "two");

        // Each entry: tag=1, wire=2, 4-byte length, key, value
        // 1 -> "one": length = 8, encoded as 88 80 80 00
        // 2 -> "two": length = 8, encoded as 88 80 80 00
        assert_eq!(
            hex(&buf),
            "0a 07 08 01 12 03 6f 6e 65 0a 07 08 02 12 03 74 77 6f"
        );
    }

    #[test]
    fn test_map_string_string() {
        let mut buf = Vec::new();
        let mut writer = MapEntryWriter::<1, PbString, PbString>::new(&mut buf);
        writer.write("a", "alpha");
        writer.write("b", "beta");
        // Each entry: tag=1, wire=2, 4-byte length, key, value
        // "a"->"alpha": length = 11,
        // "b"->"beta": length = 10,
        assert_eq!(
            hex(&buf),
            "0a 0a 0a 01 61 12 05 61 6c 70 68 61 0a 09 0a 01 62 12 04 62 65 74 61"
        );
    }

    #[test]
    fn test_map_int_float() {
        let mut buf = Vec::new();
        let mut writer = MapEntryWriter::<1, Int32, Float>::new(&mut buf);
        writer.write(1, 1.5f32);
        writer.write(2, 2.5f32);
        // 1->1.5: 0a 09 08 01 15 00 00 c0 3f
        // 2->2.5: 0a 09 08 02 15 00 00 20 40
        assert_eq!(
            hex(&buf),
            "0a 07 08 01 15 00 00 c0 3f 0a 07 08 02 15 00 00 20 40"
        );
    }
    #[test]
    fn test_required_string_and_bytes() {
        let mut buf = Vec::new();
        // String
        let _ = required::RequiredValueWriter::<1, PbString>::new(&mut buf).write("hello");
        assert_eq!(hex(&buf), "0a 05 68 65 6c 6c 6f"); // tag=1, wire=2, len=5, "hello"
        buf.clear();
        // Bytes
        let _ = required::RequiredValueWriter::<2, PbBytes>::new(&mut buf).write(b"abc");
        assert_eq!(hex(&buf), "12 03 61 62 63"); // tag=2, wire=2, len=3, "abc"
    }

    #[test]
    fn test_optional_string_and_bytes() {
        let mut buf = Vec::new();
        // String Some
        let _ = optional::OptionalValueWriter::<1, PbString>::new(&mut buf).write(Some("hello"));
        assert_eq!(hex(&buf), "0a 05 68 65 6c 6c 6f");
        buf.clear();
        // String None
        let _ = optional::OptionalValueWriter::<1, PbString>::new(&mut buf).write(None::<&str>);
        assert_eq!(hex(&buf), "");
        buf.clear();
        // Bytes Some
        let _ = optional::OptionalValueWriter::<2, PbBytes>::new(&mut buf).write(Some(b"abc"));
        assert_eq!(hex(&buf), "12 03 61 62 63");
        buf.clear();
        // Bytes None
        let _ = optional::OptionalValueWriter::<2, PbBytes>::new(&mut buf).write(None::<&[u8]>);
        assert_eq!(hex(&buf), "");
    }

    #[test]
    fn test_repeated_string_and_bytes() {
        let mut buf = Vec::new();
        // String
        let _ = repeated::RepeatedValueWriter::<1, PbString>::new(&mut buf).write(vec!["a", "b"]);
        assert_eq!(hex(&buf), "0a 01 61 0a 01 62");
        buf.clear();
        // Bytes
        let _ = repeated::RepeatedValueWriter::<2, PbBytes>::new(&mut buf).write(vec![b"x", b"y"]);
        assert_eq!(hex(&buf), "12 01 78 12 01 79");
    }

    use super::*;

    // Helper to get hex string of buffer for assertions
    fn hex(buf: &[u8]) -> String {
        buf.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn test_required_numeric_types() {
        let mut buf = Vec::new();
        // i32
        let _ = required::RequiredValueWriter::<1, Int32>::new(&mut buf).write(42);
        assert_eq!(hex(&buf), "08 2a"); // tag=1, wire=0, value=42
        buf.clear();
        // u32
        let _ = required::RequiredValueWriter::<2, Uint32>::new(&mut buf).write(123u32);
        assert_eq!(hex(&buf), "10 7b");
        buf.clear();
        // i64
        let _ = required::RequiredValueWriter::<3, Int64>::new(&mut buf).write(1000i64);
        assert_eq!(hex(&buf), "18 e8 07");
        buf.clear();
        // u64
        let _ = required::RequiredValueWriter::<4, Uint64>::new(&mut buf).write(1000u64);
        assert_eq!(hex(&buf), "20 e8 07");
        buf.clear();
        // f32
        let _ = required::RequiredValueWriter::<5, Float>::new(&mut buf).write(1.5f32);
        assert_eq!(hex(&buf), "2d 00 00 c0 3f"); // tag=5, wire=5, value=1.5f32
        buf.clear();
        // f64
        let _ = required::RequiredValueWriter::<6, Double>::new(&mut buf).write(2.5f64);
        assert_eq!(hex(&buf), "31 00 00 00 00 00 00 04 40"); // tag=6, wire=1, value=2.5f64
        buf.clear();
        // bool
        let _ = required::RequiredValueWriter::<7, Bool>::new(&mut buf).write(true);
        assert_eq!(hex(&buf), "38 01");
        buf.clear();
        let _ = required::RequiredValueWriter::<8, Bool>::new(&mut buf).write(false);
        assert_eq!(hex(&buf), "40 00");
    }

    #[test]
    fn test_optional_numeric_types() {
        let mut buf = Vec::new();
        // i32
        let _ = optional::OptionalValueWriter::<1, Int32>::new(&mut buf).write(Some(42));
        assert_eq!(hex(&buf), "08 2a");
        buf.clear();
        let _ = optional::OptionalValueWriter::<1, Int32>::new(&mut buf).write(None::<i32>);
        assert_eq!(hex(&buf), "");
        buf.clear();
        // u32
        let _ = optional::OptionalValueWriter::<2, Uint32>::new(&mut buf).write(Some(123u32));
        assert_eq!(hex(&buf), "10 7b");
        buf.clear();
        // bool
        let _ = optional::OptionalValueWriter::<3, Bool>::new(&mut buf).write(Some(true));
        assert_eq!(hex(&buf), "18 01");
        buf.clear();
        let _ = optional::OptionalValueWriter::<3, Bool>::new(&mut buf).write(None::<bool>);
        assert_eq!(hex(&buf), "");
    }

    #[test]
    fn test_repeated_numeric_types() {
        let mut buf = Vec::new();
        // i32
        let _ = repeated::RepeatedValueWriter::<1, Int32>::new(&mut buf).write(vec![1, 2, 3]);
        assert_eq!(hex(&buf), "08 01 08 02 08 03");
        buf.clear();
        // bool
        let _ =
            repeated::RepeatedValueWriter::<2, Bool>::new(&mut buf).write(vec![true, false, true]);
        assert_eq!(hex(&buf), "10 01 10 00 10 01");
    }

    #[test]
    fn test_packed_numeric_types() {
        let mut buf = Vec::new();
        // i32
        let _ = packed::PackedValueWriter::<1, Int32>::new(&mut buf).write(vec![1, 2, 3]);
        // tag=1, wire=2, length=3, values=1,2,3
        assert_eq!(hex(&buf), "0a 03 01 02 03");
        buf.clear();
        // bool
        let _ = packed::PackedValueWriter::<2, Bool>::new(&mut buf).write(vec![true, false, true]);
        assert_eq!(hex(&buf), "12 03 01 00 01");
    }
}
