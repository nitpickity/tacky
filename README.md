# Tacky

Every protobuf library for Rust asks you to build a second copy of your data. Generate a struct, fill it in field by field, serialize it, throw it away. Your `HashSet<SnakeCase>` becomes a `Vec<String>` first, because the generated struct says it must.

Tacky generates a *schema* instead of a struct, and you write whatever you already have through it.

> AI disclaimer: the concept and implementation are home-grown artisanal code.
> Benches, tests and docs were expanded upon with Claude Opus 4.8/5 because unlike code, they are not fun to write.

```protobuf
syntax = "proto3";

message SimpleMessage {
    optional string text = 1;
    repeated int32 numbers = 2;
    map<string, double> scores = 3;
}
```

`tacky-build` turns that into a schema you write through:

```rust
pub struct SimpleMessage {
    pub text: Field<1, Optional<PbString>>,
    pub numbers: Field<2, Packed<Int32>>,
    pub scores: Field<3, PbMap<PbString, Double>>,
}

let schema = SimpleMessage::schema();

schema.text.write(&mut buf, Some(&my_arc_str));   // Arc<str>
schema.numbers.write(&mut buf, &my_btree_set);    // BTreeSet<i32>
schema.scores.write(&mut buf, &my_hash_map);      // HashMap<String, f64>
```

No conversion, no intermediate value. The schema itself is zero-sized — `size_of::<SimpleMessage>() == 0` — so none of it exists at runtime; it is your `.proto` expressed in the type system and nothing more.

Supports proto2, proto3, and editions 2023/2024. Parses `.proto` files in pure Rust, so there is no `protoc` system dependency.

API documentation is in the rustdoc, which is where the per-field details live: `cargo doc -p tacky --open`.

## Won't I forget a field?

This is the usual objection to skipping the generated struct, and the answer is that every `.write()` returns the field back to you. So the schema doubles as a struct literal you fill in, and the compiler checks it:

```rust
SimpleMessage {
    text: schema.text.write(&mut buf, Some("hello world")),
    numbers: schema.numbers.write(&mut buf, [1, 2, 3, 4]),
    scores: schema.scores,   // deliberately skipped
};
```

Nothing is constructed — the type is zero-sized and the writes are the side effects. The literal exists only so that adding a field to the `.proto` stops this compiling. You get the exhaustiveness a generated struct gives you, without the allocation.

## Writing nested messages and maps

Nested messages take a closure, which receives the nested schema. The length prefix is filled in when it returns:

```rust
schema.header.write_msg(&mut buf, |buf, hdr| {
    hdr.title.write(buf, Some("report"));
    hdr.version.write(buf, Some(2));
});
```

For a repeated message field, hand the whole list to `write_msgs` and the closure runs once per element:

```rust
schema.events.write_msgs(&mut buf, ["scroll", "click"], |buf, ev, name| {
    ev.name.write(buf, Some(name));
});
```

Letting the writer own the iteration is what keeps the list in order for every buffer — see [Choosing a buffer](#choosing-a-buffer). Maps take anything iterable over pairs, and have a closure form for message values. Full API, with worked examples for every field kind: `cargo doc -p tacky --open`.

## Performance

Tacky encodes in a single pass. Precomputed tags and a length-patching primitive remove the size-calculation pass that prost and similar libraries need for nested messages and packed fields.

Measured in one run on an M3 (ARM) across four real-world schemas. `tacky` writes into a `Vec<u8>`; `tacky-rev` into a caller-provided fixed slice, filled backwards, which needs no length placeholders at all. The C++ column is the fair arm: `cpp-noutf8` for proto3, `cpp` for proto2. Both ratio columns are for the default forward writer; `tacky-rev`'s own ratio against C++ is rightmost.

| Corpus | Size | tacky | tacky-rev | prost | C++ | tacky vs prost | tacky vs C++ | rev vs C++ |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **pprof** — real Go heap profile | 847 KB | 490 µs | 449 | 1314 | 772 | **2.7x** | **1.6x** | **1.7x** |
| **Descriptor set** — with source info | 126 KB | 48.4 µs | 38.8 | 174 | 63.0 | **3.6x** | **1.3x** | **1.6x** |
| **Descriptor set** — schemas only | 20 KB | 11.2 µs | 8.9 | 24.5 | 13.5 | **2.2x** | **1.2x** | **1.5x** |
| **OTLP traces** — 512 spans | 355 KB | 109 µs | 85.5 | 235 | 136 | **2.2x** | **1.3x** | **1.6x** |
| **OTLP traces** — 200 spans | 145 KB | 42.8 µs | 34.0 | 90.4 | 51.5 | **2.1x** | **1.2x** | **1.5x** |
| **OTLP logs** — 512 records | 233 KB | 66.8 µs | 48.3 | 120 | 68.2 | **1.8x** | **1.0x** | **1.4x** |
| **Access log** — 100 entries, map headers | 62 KB | 11.4 µs | 12.3 | 28.7 | 19.2 | **2.5x** | **1.7x** | **1.6x** |

**These numbers flatter prost.** Every arm starts from an already-built prost struct, so the table measures writing cost alone. An application whose domain types are its own pays to construct and populate that struct as well — allocating the `String`s and `Vec`s the generated shape demands — and tacky skips that entirely. The gap above is the floor, not the ceiling.

**The win tracks length prefixes per byte, not size.** A descriptor set with source info is thousands of tiny messages each carrying two packed `int32` arrays, three length prefixes per ~15 bytes of payload, and that is where the sizing pass costs prost most and tacky nothing. OTLP logs are the opposite: one ~120-byte body per record, so copying dominates and there is barely any prefix work to skip. Hence 3.6x down to 1.8x against prost on the same encoder.

Output is byte-for-byte the same length as prost's on every corpus above, and each bench prints both lengths so that stays checked.

How to run these, which arms are a fair comparison, and where the corpora come from: [`testing/benches/README.md`](testing/benches/README.md).

Decoding is roughly on par with prost when materializing into owned structs. Tacky's decode model is zero-copy for strings, bytes and sub-messages, so real decode performance depends on how much copying your application actually needs.

## Limitations

Tacky focuses on dumping data to the wire fast, and reading it as it's presented. As a result, several protobuf features are either irrelevant or unsupported.

**Extensions, RPC and custom defaults** are not supported.

**Merge semantics are not implemented.** Because the deserializer yields one field at a time, it cannot merge repeated instances of a singular message for you.

**OneOf is not enforced.** The serializer generates a OneOf type grouping the variants, but nothing stops you writing more than one. The deserializer flattens OneOf variants into individual fields like any other.

## Setup

Add `tacky` as a dependency and `tacky-build` as a build dependency:

```toml
[dependencies]
tacky = { git = "https://github.com/nitpickity/tacky.git" }

[build-dependencies]
tacky-build = { git = "https://github.com/nitpickity/tacky.git" }
```

A minimal `build.rs` calls `write_proto` per `.proto` file:

```rust
fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    tacky_build::write_proto("protos/my_message.proto", &format!("{out_dir}/my_message.rs"));
    println!("cargo:rerun-if-changed=protos/my_message.proto");
}
```

Then `include!` the generated file from a module in your crate:

```rust
include!(concat!(env!("OUT_DIR"), "/my_message.rs"));
```

For protos that import others, use `write_proto_with_includes` and pass the include paths.

## Decoding

`tacky-build` generates an enum with a variant per field, and an iterator that yields them one at a time. You match on variants and build your domain object from primitives, either exhaustively or selecting only what you care about. Unknown fields are skipped by the iterator.

```rust
for field in SimpleMessage::decode(&buf) {
    match field? {
        SimpleMessageField::Text(s) => { /* s is a &str */ },
        SimpleMessageField::Numbers(n) => { /* n is an i32 */ },
        _ => {}
    }
}
```

Fields come back as `&str`, `i32`, `f64` and the like, borrowed from the input buffer; mapping them to your domain types is up to you. Only one variant is on the stack at a time, however many fields the message has, where a generated struct costs you all of them at once.

### Repeated fields

Unpacked repeated fields appear as one variant per occurrence, so match in the loop and append:

```rust
let mut tags: Vec<String> = Vec::new();
for field in Message::decode(&buf) {
    match field? {
        MessageField::Tag(s) => tags.push(s.to_owned()),
        _ => {}
    }
}
```

Packed fields, which includes `repeated` numerics in proto3, come back as a single variant carrying an iterator. Each element is a `Result`, since a varint can be malformed part-way through the run:

```rust
for field in Message::decode(&buf) {
    match field? {
        MessageField::Numbers(iter) => {
            for n in iter {
                numbers.push(n?);
            }
        }
        _ => {}
    }
}
```

Protobuf allows the same repeated field to appear more than once in a message, and the loop above accumulates across every occurrence without extra bookkeeping.

### Maps, enums and nested messages

Each map entry comes back as a `(key, Option<value>)` tuple. The value is `Option` because protobuf allows an entry with a key but no value; proto3 reads that as the default, and tacky hands you the absence so you can decide. `write_entry` takes the same `Option`, so a key-only entry can be written deliberately.

```rust
MsgWithMapsField::StrInt((k, v)) => { map.insert(k, v.unwrap_or_default()); },
```

Proto enums come back as a Rust enum with an extra `__Unrecognized(i32)` variant, so a value added by a newer producer is something you handle rather than something that breaks you:

```rust
UserField::Tier(t) => user.tier = match t {
    proto::Tier::Free => Some(Tier::Free),
    proto::Tier::Pro => Some(Tier::Pro),
    proto::Tier::__Unrecognized(_) => None,
},
```

Nested messages give you a sub-decoder you iterate exactly like the outer one, so a domain object gets built field by field the whole way down without the proto's intermediate struct ever existing.

## Choosing a buffer

Writes go through the `WriteBuf` trait, and three buffers implement it:

| Buffer | Direction | Capacity | Use for |
| :--- | :--- | :--- | :--- |
| `Vec<u8>` | forward | grows | the default |
| `SliceBuf` | forward | fixed | `no_std`, no allocator |
| `RevBuf` | **backwards** | fixed | the fastest path, when you can bound the output |

Direction is a compile-time property rather than a runtime flag, so each writer's unused arm folds away and the forward path pays nothing for the reverse one.

**Why backwards is faster.** A forward writer reaches a nested message's length prefix before it knows the length, so it reserves a placeholder and patches it afterwards, widening and memmoving the payload if the guess was too small. A backwards writer runs the body first and prepends the length once it is known: no placeholder, no reserved width, no shift, and always a minimal-width varint. That is where the 1.4–1.7x over the C++ runtime comes from, and it grows with the number of length prefixes per byte of payload.

`RevBuf` asks for two things in return: an upper bound on the output size, since it cannot grow, and that you let the writers own repeated-field iteration, since elements have to be emitted back-to-front. Its rustdoc carries the full ordering contract, which is worth reading before you use it. Maps gain nothing from it — a map entry's length is computable in advance in both directions — which is why the access log is the one corpus above where it loses.

Code that has not picked a buffer writes through `AnyDir`, which erases the direction.

## Acknowledgements

`tacky-build` vendors a heavily modified copy of [pb-rs](https://github.com/tafia/quick-protobuf/tree/master/pb-rs) (from [quick-protobuf](https://github.com/tafia/quick-protobuf), MIT-licensed), used purely as a `.proto` parser and validator at build time. Parsing `.proto` in pure Rust is what lets tacky avoid the `protoc` system dependency that prost and others pull in. The vendored copy is stripped down and adapted to that role; none of pb-rs's own code generation is used, and the schema and decoder code is all generated by tacky.
