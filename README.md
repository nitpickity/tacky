# A slightly sticky protobuf writer

Tacky is a simple "structless" protobuf serialiser (not deserializer).
Given a protobuf definition, rather than creating a rust struct to hold the data, `tacky-build` will generate a rust type-level representation of the protobuf schema, and a builder-like API to write your message.
every function in the builder api just tacks on the relevant field/message to a buffer, abstracting over the protobuf types, field numbers, and wire types. Since the builder doesnt ever need to own data,the write functions accept iterators and borrowed values.
This is a lower level library that concerns itself with providing functions to work with writing data in a protobuf compatible fashion.

# Example

Assume the following protobuf schema:

```protobuf
message SimpleMessage {
    optional string text = 1;
    optional string text2 = 5;
    repeated int32 numbers = 2;
    repeated bytes blobs = 3;
    map<string,double> map = 4;
}
```

### Strict Schema API

Sometimes as a protobuf producer, you want to make sure all fields in the schema are accounted for, and when updating the protobuf file, the compiler should make sure to check exhaustiveness (if needed).

```rust
/// lift the schema into the rust type level. the const integers are the field numbers,
/// the rest are zero-sized marker types. this makes this whole struct a ZST with no runtime cost.
/// the advantage of including the field number in the type system is that 2 fields of the same protobuf type cant be accidently interchanged.
pub struct SimpleMessageSchema {
    text: Field<1,Optional<PbString>>,
    numbers: Field<2,Repeated<Int32>>,
    blobs: Field<3,Repeated<PbBytes>>,
    map: Field<4,Map<PbString,Double>>
}
pub struct SimpleMessageWriter {/*..*/}
/// Every function here instead returns the field type that it is assigned to. those are ZST markers.
impl SimpleMessageWriter {
    fn new(buf: &mut Vec<u8>) -> Self;
    fn text<T: AsRef<str>>(&mut self, text: Option<T>)->Field<1,Optional<PbString>>;
    fn text2<T: AsRef<str>>(&mut self, text: Option<T>)->Field<5,Optional<PbString>>;
    fn numbers(&mut self, numbers: impl IntoIterator<Item = i32>) -> Field<2,Repeated<Int32>>;
    fn blobs<T: AsRef<[u8]>(&mut self, blobs: impl IntoIterator<Item = T>) -> Field<3,Repeated<PbBytes>>;
    fn map<T:AsRef<str>>(&mut self, map: impl IntoIterator<Item = (T,f64)>) -> Field<4,Map<PbString,Double>>;
    fn finish(self);
}
fn use_it() {
    let mut buf = Vec::new();
    let mut writer = SimpleMessageWriter::new(&mut buf);
    let blob_set = HashSet::from_iter([b"blob1",b"blob2"]);
    let force_schema = SimpleMessageSchema {
        text: writer.text(Some("Hello")),
        //text2: writer.text(Some("World")), <- wont compile because the wrong writer is used. both are optional strings, but differ in field number.
        text2: writer.text2(Some("World")),
        numbers: writer.numbers([1,2,3]),
        blobs: writer.blobs(&blob_set),
        // have to at the very least _mention_ the map field or else it wont compile.
        map: writer.map(&[])
    };
    // thats it, buf contains the serialized data, with compiler enforced exhaustiveness checks and some confusion protection.
}

```

### Simple Builder API - TODO (restore, existed in previous build)

using `tacky-build` in this mode will generate the following code:

```rust
pub struct SimpleMessageWriter {/*..*/}
impl SimpleMessageWriter {
    fn new(buf: &mut Vec<u8>) -> Self;
    fn text<T: AsRef<str>>(mut self, text: Option<T>)-> Self;
    fn text2<T: AsRef<str>>(mut self, text2: Option<T>)-> Self;
    fn numbers(mut self, numbers: impl IntoIterator<Item = i32>) -> Self;
    fn blobs<T: AsRef<[u8]>(mut self, blobs: impl IntoIterator<Item = T>) -> Self;
    fn map<T:AsRef<str>>(mut self, map: impl IntoIterator<Item = (T,f64)>) -> Self;
    fn finish(self);
}

fn use_it() {
    let mut buf = Vec::new();
    let blob_set = HashSet::from_iter([b"blob1",b"blob2"]);
    SimpleMessageWriter::new(&mut buf)
        .text(Some("Hello"))
        .numbers([1,2,3])
        .blobs(&blob_set)
        .finish();
    // thats it, buf contains the serialized data. note the 'map' field was not written
}

```

## Why not Prost/quick-protobuf/rust-protobuf

The above libs codegen rust structs which are used to hold data before serializing, which means they need to make choices about the appropriate Rust type representation, which can be limiting. for example, Prost requires that the protobuf type `repeated string` be represented as `Vec<String>` in rust, which is not actually needed to serialise that data where a simple iterator over `&str` will do.
In other words when you have a lot of data that doesnt fit nicely into the structs generated by other protobuf libs, or you want control over how things are written for performance/semantics reasons, this library might do the trick.

## Some special considerations.

There are 2 cases where some finess is needed to make an incremental/immidiate writer like Tacky work. Both of those concern length delimited fields of size that isnt known up front.

The first case are packed repeated fields (repeated integer fields in protobuf 3, repeated integer fields in proto2 if the 'packed = true' attribute is set.) those are written as a length delimited field (tag, length, values) as opposed to non-packed "repeated" fields which are multiples of (tag, value).
the approach used by prost and quick-protobuf (and probably most other impls) is to iterate over the input values, calculate their length, and then iterate again to write them. We dont like that.

Sthe second case is fields in messages that are themselves Message types. those are also length-delimited, and since its up to the caller to decide which fields to write and how, we cant know up front the length of that message.

In both of these cases, what we do here is a slight abuse of the LEB128 (varint) encoding. we allocate a fixed width place-holder length up front. once the message writer drops, it goes back to that place in the buffer and writes the correct length (backpatching, see the Tack module in this lib for detail).

for packed fields this is transparent to users, the generated API takes care of it. encoding packed varints this way is also vastly faster than the iterate-twice approach commontly used (almost twice as fast, for obvious reasons).
note that currently this limits the length of a single packed field to 16kb. if you need longer, use the lower-level writer API to configure this.

for message-type fields,the generated API is closure based, to make sure the lengths are backfilled correctly on `Drop`.
This API does not currently explicitly track optional/repeated qualifiers, but instead leaves it up to the user to write the message zero, once, or many times.
the generated API for example:

```protobuf
message TopLevel {
    string text = 1;
    optional Nested nest = 2;
}

message Nested {
    int32 num = 1;
    repeated Nested rec = 2; //recursive message, for fun.
}
```

would generate writers for both messages:

```rust
pub struct TopLevelWriter {..}
impl TopLevelWriter {
    pub fn text(&mut self, text: Option<&str>)->Field<1,Plain<PbString>>;
    pub fn nest(&mut self, write: impl FnMut(NestedWriter)) -> Field<2,Optional<PbMessage>>;

pub struct NestedWriter {..}
impl NestedWriter  {
    pub fn num(&mut self, num: i32) -> Field<1,Plain<Int32>>,
    pub fn rec(&mut self, rec: impl FnMut(NestedWriter)) -> Field<2, Optional<PbMessage>>;
}
}
fn useage() {
    let mut buf = Vec::new();
    let mut writer = TopLevelWriter::new(&mut buf);
    writer.text(Some("hello"));
    writer.nest(|mut nested_writer| {
        nested_writer.num(42);
        nested_writer.rec(|mut deeper| {..})
    })
}

```

## Current limitations

OneOf type fields are not handled, TODO.

imports/includes/nested defs are not handled. that is, all messages must be flat within a file. TODO.

Deserializing isnt really in scope, unless i find a nice way to do it that fits what this library wants to do. use prost maybe, as that what i test against right now.

Would be nice to add a Derive for existing messages to serialize via Tacky, in case all or part of the fields match in name,type and order to a certain schema. something like:

```rust
#[derive(ToProto)]
#[tacky(proto_path = "../data.proto")]
struct Data {
    ips: BTreeSet<IpAddr>,
    paths: BTreeSet<Arc<str>>,
    timings: Vec<Duration>
}

```
