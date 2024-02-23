//! Base module providing tools for working with protobuf scalars and maps where fields are scalars.

use crate::{scalars::*, tack::Tack};
use std::{fmt::Display, marker::PhantomData};

pub struct PbMessage;
pub struct PbEnum;
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
    pub fn new() -> Field<N, P> {
        Field(PhantomData)
    }
}
pub trait FieldWriter
where
    Self: Sized,
{
    type Writer<'a>;
    fn get_writer<'a>(buf: &'a mut Vec<u8>) -> Self::Writer<'a>;
}

pub mod optional {
    use super::*;
    pub struct OptionalValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }

    impl<'b, const N: usize, P: ProtobufScalar> OptionalValueWriter<'b, N, P> {
        pub fn write(self, value: Option<P::RustType<'_>>) -> Field<N, Optional<P>> {
            if let Some(value) = value {
                P::write(N as i32, value, self.buf);
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
            if let Some(value) = value {
                use std::io::Write;
                let tag = ((N as i32) << 3) | (PbString::WIRE_TYPE as i32);
                let mut t = Tack::new_with_width(self.buf, Some(tag as u32), 1);
                t.rewind = !incl_empty;
                write!(t.buffer, "{value}").unwrap();
            }
            Field::new()
        }
    }
    impl<'b, const N: usize, M: MessageSchema> OptionalValueWriter<'b, N, M> {
        pub fn write_msg<T: ProtoWrite<M>>(self, value: Option<T>) -> Field<N, Optional<M>> {
            if let Some(value) = value {
                let w = M::new_writer(self.buf, Some(N as i32));
                value.write_msg(w)
            }
            Field::new()
        }
        pub fn write_msg_with(self, mut f: impl FnMut(M::Writer<'_>)) -> Field<N, Optional<M>> {
            let w = M::new_writer(self.buf, Some(N as i32));
            f(w);
            Field::new()
        }
    }

    impl<const N: usize, P> FieldWriter for Field<N, Optional<P>> {
        type Writer<'a> = OptionalValueWriter<'a, N, P>;
        fn get_writer<'a>(buf: &'a mut Vec<u8>) -> Self::Writer<'a> {
            OptionalValueWriter {
                buf,
                _m: PhantomData,
            }
        }
    }
}

pub mod repeated {
    use super::*;
    impl<'b, const N: usize, P: ProtobufScalar> RepeatedValueWriter<'b, N, P> {
        pub fn write<'a>(
            self,
            values: impl IntoIterator<Item = P::RustType<'a>>,
        ) -> Field<N, Repeated<P>> {
            for value in values {
                P::write(N as i32, value, self.buf)
            }
            Field::new()
        }
    }

    impl<'b, const N: usize, M: MessageSchema> RepeatedValueWriter<'b, N, M> {
        pub fn write_msg<'a, T: ProtoWrite<M>>(
            self,
            values: impl IntoIterator<Item = T>,
        ) -> Field<N, Repeated<M>> {
            for value in values {
                let writer = M::new_writer(self.buf, Some(N as i32));
                value.write_msg(writer)
            }
            Field::new()
        }
        pub fn append_msg<'a, T: ProtoWrite<M>>(&mut self, value: T) -> Field<N, Repeated<M>> {
            let writer = M::new_writer(self.buf, Some(N as i32));
            value.write_msg(writer);
            Field::new()
        }
        pub fn append_msg_with(
            &mut self,
            mut func: impl FnMut(M::Writer<'_>),
        ) -> Field<N, Repeated<M>> {
            let writer = M::new_writer(self.buf, Some(N as i32));
            func(writer);
            Field::new()
        }
        pub fn close(self) -> Field<N, Repeated<M>> {
            Field::new()
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

    impl<const N: usize, P> FieldWriter for Field<N, Repeated<P>> {
        type Writer<'a> = RepeatedValueWriter<'a, N, P>;
        fn get_writer<'a>(buf: &'a mut Vec<u8>) -> Self::Writer<'a> {
            RepeatedValueWriter {
                buf,
                _m: PhantomData,
            }
        }
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
    impl<'b, const N: usize, P: ProtobufScalar> PackedValueWriter<'b, N, P> {
        pub fn write<'a>(
            self,
            values: impl IntoIterator<Item = P::RustType<'a>>,
        ) -> Field<N, Packed<P>> {
            for value in values {
                P::write(N as i32, value, self.buf)
            }
            Field::new()
        }
    }

    impl<const N: usize, P> FieldWriter for Field<N, Packed<P>> {
        type Writer<'a> = PackedValueWriter<'a, N, P>;
        fn get_writer<'a>(buf: &'a mut Vec<u8>) -> Self::Writer<'a> {
            PackedValueWriter {
                buf,
                _m: PhantomData,
            }
        }
    }
}

pub mod required {
    use super::*;
    pub struct RequiredValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }
    impl<'b, const N: usize, P: ProtobufScalar> RequiredValueWriter<'b, N, P> {
        pub fn write(self, value: P::RustType<'_>) -> Field<N, Required<P>> {
            P::write(N as i32, value, self.buf);
            Field::new()
        }
    }
    impl<'b, const N: usize, M: MessageSchema> RequiredValueWriter<'b, N, M> {
        pub fn write_msg<T: ProtoWrite<M>>(self, value: T) -> Field<N, Required<M>> {
            let w = M::new_writer(self.buf, Some(N as i32));
            value.write_msg(w);
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

    impl<const N: usize, P> FieldWriter for Field<N, Required<P>> {
        type Writer<'a> = RequiredValueWriter<'a, N, P>;
        fn get_writer<'a>(buf: &'a mut Vec<u8>) -> Self::Writer<'a> {
            RequiredValueWriter {
                buf,
                _m: PhantomData,
            }
        }
    }
}

pub mod plain {
    use super::*;
    pub struct PlainValueWriter<'b, const N: usize, P> {
        buf: &'b mut Vec<u8>,
        _m: PhantomData<P>,
    }

    impl<'b, const N: usize, P: ProtobufScalar> PlainValueWriter<'b, N, P> {
        pub fn write(self, value: P::RustType<'_>) -> Field<N, Plain<P>> {
            P::write(N as i32, value, self.buf);

            Field::new()
        }
    }
    //plain messages are treated as optional
    impl<'b, const N: usize, M: MessageSchema> PlainValueWriter<'b, N, M> {
        pub fn write_msg<T: ProtoWrite<M>>(self, value: Option<T>) -> Field<N, Plain<M>> {
            if let Some(value) = value {
                let w = M::new_writer(self.buf, Some(N as i32));
                value.write_msg(w)
            }
            Field::new()
        }
    }

    impl<const N: usize, P> FieldWriter for Field<N, Plain<P>> {
        type Writer<'a> = PlainValueWriter<'a, N, P>;
        fn get_writer<'a>(buf: &'a mut Vec<u8>) -> Self::Writer<'a> {
            PlainValueWriter {
                buf,
                _m: PhantomData,
            }
        }
    }
}

impl<K, V> PbMap<K, V> {
    pub fn new() -> PbMap<K, V> {
        PbMap(PhantomData)
    }
}

pub struct EnumWriter<'b, const N: usize> {
    buf: &'b mut Vec<u8>,
}

impl<'b, const N: usize> EnumWriter<'b, N> {
    pub fn new(buf: &'b mut Vec<u8>) -> Self {
        Self { buf }
    }

    pub fn write_field(&mut self, value: i32) {
        Int32::write(N as i32, value, self.buf)
    }

    pub fn write_untagged(&mut self, value: i32) {
        Int32::write_value(value, self.buf)
    }
    pub fn write_tag(&mut self) {
        Int32::write_tag(N as i32, self.buf)
    }
}
pub struct ScalarWriter<'b, const N: usize, P> {
    buf: &'b mut Vec<u8>,
    _pbtype: PhantomData<P>,
}

impl<'b, const N: usize, P: ProtobufScalar> ScalarWriter<'b, N, P> {
    pub fn new(buf: &'b mut Vec<u8>) -> Self {
        Self {
            buf,
            _pbtype: PhantomData,
        }
    }

    pub fn write_field(&mut self, value: P::RustType<'_>) {
        P::write(N as i32, value, self.buf)
    }

    pub fn write_untagged(&mut self, value: P::RustType<'_>) {
        P::write_value(value, self.buf)
    }
    pub fn write_tag(&mut self) {
        P::write_tag(N as i32, self.buf)
    }
}

impl<'b, const N: usize> ScalarWriter<'b, N, PbString> {
    /// Writes values to string via their Display impl.
    /// the max length of the string here is 127 bytes, which should cover most cases this is designed for.
    /// NOTE: incl_empty controls wether the write will have proto3 or proto2 semantics.
    /// if false, if the written length ends up being 0 ("", empty string), the tag/len wont be written either (proto3)
    /// if true, the empty value will still be written, like proto2 or proto3 with explicit presence
    pub fn write_display(&mut self, d: impl Display, incl_empty: bool) {
        use std::io::Write;
        let tag = ((N as i32) << 3) | (PbString::WIRE_TYPE as i32);
        let mut t = Tack::new_with_width(self.buf, Some(tag as u32), 1);
        t.rewind = !incl_empty;
        write!(t.buffer, "{d}").unwrap();
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

    pub fn write_entry<'a>(&mut self, key: K::RustType<'a>, value: V::RustType<'a>) {
        let tag = (N << 3) | 2;
        write_varint(tag as u64, self.buf);
        let len = K::len(1, key) + V::len(2, value);
        write_varint(len as u64, self.buf);
        K::write(1, key, self.buf);
        V::write(2, value, self.buf);
    }
}

// automatically codegened for a message in a proto file.
pub trait MessageSchema {
    type Writer<'a>;
    fn new_writer(buffer: &mut Vec<u8>, tag: Option<i32>) -> Self::Writer<'_>;
}

pub trait ProtoWrite<M>
where
    M: MessageSchema,
{
    fn write_msg(&self, writer: M::Writer<'_>);
}

impl<T: ProtoWrite<M>, M: MessageSchema> ProtoWrite<M> for &T {
    fn write_msg(&self, writer: <M as MessageSchema>::Writer<'_>) {
        T::write_msg(self, writer)
    }
}
