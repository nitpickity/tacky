//! Base module providing tools for working with protobuf scalars and maps where fields are scalars.

use crate::{scalars::*, tack::Tack};
use std::{fmt::Display, marker::PhantomData};

// compound types
pub struct PbMap<K, V>(PhantomData<(K, V)>); // Map<PbString,Int32>
pub struct OneOf<O>(PhantomData<O>); // OneOf<(Field<1,Int32>,Field<3,PbString>)>

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
        pub fn new(buf: &'b mut Vec<u8>) -> Self {
            Self {
                buf,
                _m: PhantomData,
            }
        }

        pub fn write<K: ProtoEncode<P>>(self, value: Option<K>) -> Field<N, Optional<P>> {
            if let Some(value) = value {
                <K as ProtoEncode<P>>::encode(N as i32, self.buf, value);
            }
            Field::new()
        }
    }
    impl<'b, const N: usize> OptionalValueWriter<'b, N, PbString> {
        pub fn write_fmt<T: Display>(
            self,
            value: Option<T>,
            incl_empty: bool,
        ) -> Field<N, Optional<PbString>> {
            use std::io::Write;
            if let Some(value) = value {
                let tag = ((N as i32) << 3) | (PbString::WIRE_TYPE as i32);
                let mut t = Tack::new_with_width(self.buf, Some(tag as u32), 1);
                t.rewind = !incl_empty;
                write!(t.buffer, "{value}").unwrap();
            }

            Field::new()
        }
    }
    impl<'b, const N: usize, M: MessageSchema> OptionalValueWriter<'b, N, M> {
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

        pub fn write<K: ProtoEncode<P>>(
            self,
            values: impl IntoIterator<Item = K>,
        ) -> Field<N, Repeated<P>> {
            for value in values {
                K::encode(N as i32, self.buf, value)
            }
            Field::new()
        }
        pub fn append<K: ProtoEncode<P>>(&mut self, value: K) -> &mut Self {
            K::encode(N as i32, self.buf, value);
            self
        }
        pub fn close(self) -> Field<N, Repeated<P>> {
            Field::new()
        }
    }

    impl<'b, const N: usize, M: MessageSchema> RepeatedValueWriter<'b, N, M> {
        pub fn append_msg_with(&mut self, mut func: impl FnMut(M::Writer<'_>)) -> &mut Self {
            let writer = M::new_writer(self.buf, Some(N as i32));
            func(writer);
            self
        }
    }

    impl<'b, const N: usize> RepeatedValueWriter<'b, N, PbString> {
        pub fn write_fmt<'a, T: Display>(
            self,
            values: impl IntoIterator<Item = T>,
            incl_empty: bool,
        ) -> Field<N, Repeated<PbString>> {
            for value in values {
                use std::io::Write;
                let tag = ((N as i32) << 3) | (PbString::WIRE_TYPE as i32);
                let mut t = Tack::new_with_width(self.buf, Some(tag as u32), 1);
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
        pub fn write<'a, K: ProtoEncode<P>>(
            self,
            values: impl IntoIterator<Item = K>,
        ) -> Field<N, Packed<P>> {
            for value in values {
                K::encode(N as i32, self.buf, value)
            }
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

        pub fn write<T: ProtoEncode<P>>(self, value: T) -> Field<N, Required<P>> {
            T::encode(N as i32, self.buf, value);
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

        pub fn write<T: ProtoEncode<P>>(self, value: T) -> Field<N, Plain<P>> {
            T::encode(N as i32, self.buf, value);
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

    pub fn write_entry(&mut self, key: K::RustType<'_>, value: V::RustType<'_>) {
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

    pub fn insert<'a, A: ProtoEncode<K>, B: ProtoEncode<V>>(&mut self, key: A, value: B) {
        let t = Tack::new(self.buf, Some(N as u32));
        A::encode(1, t.buffer, key);
        B::encode(2, t.buffer, value);
    }

    pub fn close(self) -> Field<N, PbMap<K, V>> {
        Field::new()
    }
    pub fn write<'a, I: IntoIterator<Item = (A, B)>, A: ProtoEncode<K>, B: ProtoEncode<V>>(
        self,
        values: I,
    ) -> Field<N, PbMap<K, V>> {
        ProtoEncode::<PbMap<K, V>>::encode(N as i32, self.buf, values);
        Field::new()
    }
}

// automatically codegened for a message in a proto file.
pub trait MessageSchema {
    type Writer<'a>;
    fn new_writer(buffer: &mut Vec<u8>, tag: Option<i32>) -> Self::Writer<'_>;
}

pub trait ProtoEncode<P> {
    fn encode(field_nr: i32, buf: &mut Vec<u8>, value: Self);
}

impl<T: AsRef<str>> ProtoEncode<PbString> for T {
    fn encode(field_nr: i32, buf: &mut Vec<u8>, value: Self) {
        PbString::write(field_nr, value.as_ref(), buf)
    }
}

impl<T: AsRef<[u8]>> ProtoEncode<PbBytes> for T {
    fn encode(field_nr: i32, buf: &mut Vec<u8>, value: Self) {
        PbBytes::write(field_nr, value.as_ref(), buf)
    }
}

macro_rules! gen_encodes {
    ($src:ty => $($dst:ty),*) => {
        $(
            impl ProtoEncode<$dst> for $src {
                fn encode(field_nr: i32, buf: &mut Vec<u8>, value: Self) {
                    <$dst>::write(field_nr, value, buf)
                }
            }
            impl<'a> ProtoEncode<$dst> for &'a $src {
                fn encode(field_nr: i32, buf: &mut Vec<u8>, value: Self) {
                    <$dst>::write(field_nr, *value, buf)
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

impl<A, B, K, V, I> ProtoEncode<PbMap<K, V>> for I
where
    I: IntoIterator<Item = (A, B)>,
    A: ProtoEncode<K>,
    B: ProtoEncode<V>,
{
    fn encode(field_nr: i32, buf: &mut Vec<u8>, value: Self) {
        for (k, v) in value {
            let t = Tack::new(buf, Some(field_nr as u32));
            A::encode(1, t.buffer, k);
            B::encode(2, t.buffer, v);
        }
    }
}
