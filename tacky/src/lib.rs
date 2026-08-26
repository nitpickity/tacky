//! Protobuf serialization and deserialization through zero-sized typed schemas.
//!
//! Instead of generating structs to hold your data (like prost), tacky generates a schema you
//! write your existing data through, in whatever form it's already in. The schema is your `.proto`
//! lifted into the type system — field numbers as const generics, labels and scalars as zero-sized
//! markers — so none of it exists at runtime and `size_of::<Schema>() == 0`.
//!
//! ```
//! use tacky::*;
//!
//! // syntax = "proto3";
//! //
//! // message SimpleMessage {
//! //     optional string text = 1;
//! //     repeated int32 numbers = 2;      // packed, since this is proto3
//! //     map<string, double> scores = 3;
//! // }
//! //
//! // `tacky-build` generates this schema struct from the above:
//! pub struct SimpleMessage {
//!     pub text: Field<1, Optional<PbString>>,
//!     pub numbers: Field<2, Packed<Int32>>,
//!     pub scores: Field<3, PbMap<PbString, Double>>,
//! }
//! impl MessageSchema for SimpleMessage {}
//!
//! let mut buf = Vec::new();
//! let schema = SimpleMessage::schema();
//!
//! schema.text.write(&mut buf, Some("hello"));
//! schema.numbers.write(&mut buf, [1, 2, 3]);
//! schema.scores.write(&mut buf, [("pi", 3.14)]);
//! ```
//!
//! Nothing is allocated for the schema itself, and the field values are read straight from whatever
//! you already have: `text` takes any `AsRef<str>`, `numbers` any iterator of `i32`, `scores` any
//! iterator of pairs, a `HashMap` included.
//!
//! Every `write` returns the field back, so listing the writes in a struct literal buys
//! compile-time exhaustiveness — add a field to the `.proto` and this stops compiling:
//!
//! ```
//! # use tacky::*;
//! # pub struct SimpleMessage {
//! #     pub text: Field<1, Optional<PbString>>,
//! #     pub numbers: Field<2, Packed<Int32>>,
//! #     pub scores: Field<3, PbMap<PbString, Double>>,
//! # }
//! # impl MessageSchema for SimpleMessage {}
//! # let mut buf = Vec::new();
//! # let schema = SimpleMessage::schema();
//! SimpleMessage {
//!     text: schema.text.write(&mut buf, Some("hello")),
//!     numbers: schema.numbers.write(&mut buf, [1, 2, 3]),
//!     scores: schema.scores,  // deliberately skipped
//! };
//! ```
//!
//! The literal constructs nothing — `SimpleMessage` is zero-sized and the writes are the side
//! effects. It exists only for the compiler to check.
//!
//! # Setup
//!
//! Schema structs and field enums come from the `tacky-build` crate: call its `write_proto` from
//! `build.rs` once per `.proto`, then `include!` the output from a module in your crate. Its own
//! docs carry the `build.rs` snippet.
//!
//! # Field labels
//!
//! A field is `Field<N, Label<Scalar>>`: `N` the field number, `Label` how presence and repetition
//! work, `Scalar` the wire type. The label is what decides the shape of the value you pass:
//!
//! | Label | Generated from | `write` takes | Skipped when |
//! | --- | --- | --- | --- |
//! | [`Optional<P>`](`Optional`) | `optional`, in proto2 or proto3 | `Option<V>` | value is `None` |
//! | [`Plain<P>`](`Plain`) | proto3 implicit presence | `V` | value is the type's default |
//! | [`Required<P>`](`Required`) | proto2 `required` | `V` | never |
//! | [`Repeated<P>`](`Repeated`) | `repeated`, where packing does not apply | `IntoIterator<Item = V>` | iterator is empty |
//! | [`Packed<P>`](`Packed`) | `repeated` numerics and enums, when packed | `IntoIterator<Item = V>` | iterator is empty |
//! | [`PbMap<K, V>`](`PbMap`) | `map<K, V>` | `IntoIterator<Item = (A, B)>` | iterator is empty |
//!
//! A numeric or enum `repeated` field is packed in proto3 by default, and in proto2 only with
//! `[packed = true]`; anything else `repeated` — strings, bytes, messages — cannot be packed and is
//! always [`Repeated`]. So the label you get for the same declaration depends on the syntax, which
//! is why the generated struct is worth reading rather than assumed.
//!
//! `V` is anything implementing [`ProtoEncode`] for that scalar, which covers the obvious Rust
//! primitives plus every `AsRef<str>` and `AsRef<[u8]>`. Implement it for your own types to write
//! them without a conversion. Message-valued fields take a closure instead — [`Field::write_msg`],
//! [`Field::write_msgs`].
//!
//! # Map of the crate
//!
//! Things you call:
//!
//! - [`Field`], whose `write`, [`write_msg`](`Field::write_msg`),
//!   [`write_msgs`](`Field::write_msgs`), [`write_single`](`Field::write_single`),
//!   [`write_exact`](`Field::write_exact`) and [`write_entry`](`Field::write_entry`) methods are the
//!   entire encode API
//! - [Buffers](`buf`): `Vec<u8>`, [`SliceBuf`] for no-alloc, [`RevBuf`], which writes backwards so
//!   nested lengths are exact and need no placeholder at all, and [`AnyDir`] for code that has not
//!   picked one
//! - [`ProtoEncode`], to write your own types through a field
//! - [`PbDisplay`] and `PbWrite`, to write a `Display` value or an `io::Write` closure as a field
//!
//! Things you read, in your own generated schema: the [labels](`field`) above, and the
//! [scalar markers](`scalars`) — [`Int32`], [`PbString`], [`Double`] and the rest, one per protobuf
//! scalar type.
//!
//! Things generated code calls, which you can ignore: [`MessageSchema`], [`EncodedTag`],
//! [`PackedIter`](`field::packed::PackedIter`), [`OrderedIter`], [`Order`] with
//! [`Forward`]/[`Reverse`]/[`Both`], [`Tack`], and
//! the wire-level helpers [`decode_key`], [`decode_len`], [`decode_varint`], [`skip_field`],
//! [`skip_varint`], [`check_wire_type`] and [`write_varint`].
//!
//! # Features
//!
//! `alloc`, on by default, enables the `Vec<u8>` buffer. `std` adds [`IoWriter`] and `PbWrite`.
//! [`SliceBuf`] and [`RevBuf`] need neither, so encoding works on bare `#![no_std]`.
//!
//! # Panics
//!
//! Encoding panics where a conventional API would return `Err`. Decoding never does, so nothing
//! a peer sends can panic. Every encode panic is a function of the message and the buffer, never
//! of an allocator, a clock or a thread, so it reproduces on every run.
//!
//! Most are caller bugs: a [`Tack`], [`FmtWriter`] or [`IoWriter`] over a [`RevBuf`], a bad
//! placeholder width, a [`PbWrite`] closure or [`PbDisplay`] value that fails mid-field. The one a
//! correct program can reach is a fixed buffer running out of room; see
//! [Running out of room](`SliceBuf#running-out-of-room`) for the three ways to handle it.

#![no_std]
#![allow(clippy::new_without_default)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod buf;
pub mod field;
pub mod scalars;
pub mod tack;
pub use buf::*;
pub use field::*;
pub use scalars::*;
pub use tack::*;
