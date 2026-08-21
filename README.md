# Tacky

A [fast](#performance) protobuf serializer and deserializer for Rust that gets out of the way of your domain types.

AI disclaimer: the concept and implementation are home-grown artisanal code. 
Benches, tests and docs were expanded upon with Claude Opus 4.8/5 because unlike code, they are not fun to write.

## Why this exists

Every protobuf library for Rust works the same way: generate a Rust struct that mirrors your schema, fill it in, serialize it. The problem is that serializing data and representing data in your domain are two different concerns — and this approach couples them together whether you like it or not.

Prost will tell you a `repeated string` field must be a `Vec<String>`. But your domain type might be a `HashSet<SnakeCase>`, or a database row, or an iterator. Now you're cloning and reallocating just to satisfy a generated struct that exists only to be immediately thrown away. All you actually needed was something that can produce a `&str`.

Tacky keeps these concerns separate. Instead of generating a struct to hold your data, it generates a typed schema you write your existing data through — in whatever form it's already in.

## Basic Usage

Given this proto definition:

```protobuf
message SimpleMessage {
    optional string text = 1;
    repeated int32 numbers = 2;
    repeated bytes blobs = 3;
    map<string,double> map = 4;
}
```

`tacky-build` generates this schema:

```rust
pub struct SimpleMessage {
    text: Field<1, Optional<PbString>>,
    numbers: Field<2, Repeated<Int32>>,
    blobs: Field<3, Repeated<PbBytes>>,
    map: Field<4, Map<PbString, Double>>,
}
```

Which you use like this:

```rust
let mut buffer = Vec::new();
let schema = SimpleMessage::schema();

schema.text.write(&mut buffer, Some("hello world"));
schema.numbers.write(&mut buffer, [1, 2, 3, 4]);
```

`Optional` fields take an `Option`, `Repeated` fields take anything iterable. String fields accept any `AsRef<str>`, so your own string types work without conversion.

## Project Setup

Add `tacky` as a dependency and `tacky-build` as a build dependency:

```toml
[dependencies]
tacky = { git = "ssh://git@github.com/nitpickity/tacky.git" }

[build-dependencies]
tacky-build = { git = "ssh://git@github.com/nitpickity/tacky.git" }
```

A minimal `build.rs` invokes `tacky_build::write_proto` per `.proto` file:

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

## Exhaustiveness Checking

The usual assumption is that skipping the generated struct means losing safety — forget to write a field and nothing tells you. Tacky sidesteps this with a small trick: every `.write()` call returns the field schema value back. This means you can use the generated schema as a literal to "fill in" and get compile-time exhaustiveness for free:

```rust
let mut buffer = Vec::new();
let schema = SimpleMessage::schema();

SimpleMessage {
    text: schema.text.write(&mut buffer, Some("hello world")),
    numbers: schema.numbers.write(&mut buffer, [1, 2, 3, 4]),
    blobs: schema.blobs,  // explicitly skipped
    ..schema              // skip the rest
};
```

`SimpleMessage` is zero-sized — nothing is being constructed here. The `.write()` calls are the side effects, filling the buffer. The struct literal is purely a compile-time exhaustiveness check. Add a field to your proto schema and this stops compiling. Same safety as a generated data struct, none of the allocation.

## Nested Messages

Nested message fields use a closure API. The closure receives a buffer and the nested schema, and the length is patched in automatically when it returns:

```rust
MsgWithNesting {
    header: schema.header.write_msg(&mut buf, |buf, scm| {
        scm.title.write(buf, Some("hello"));
        scm.version.write(buf, Some(1));
    }),
    ..schema
};
```

For repeated message fields, hand the whole list to `write_msgs`. The closure runs once per element and receives it as a third argument, and the call returns the field, so it drops straight into a struct literal like any other write:

```rust
let events = ["scroll", "click"];
Message {
    events: schema.events.write_msgs(&mut buf, events, |buf, scm, e| {
        scm.name.write(buf, Some(e));
    }),
    ..Message::schema()
};
```

`scm` is the nested schema, so the same struct-literal exhaustiveness check works inside the closure if you want it. Letting the writer own the iteration is what keeps the list in order for every buffer: `RevBuf` fills backwards, so it has to emit the elements back-to-front, and only a call that owns the loop can do that.

When the entries have no single Rust type, `write_msg` and `write_single` write one at a time instead — see their API docs, and the ordering caveat under [Buffers and write direction](#buffers-and-write-direction).

## Maps

Map fields accept anything iterable over key-value pairs:

```rust
schema.str_int.write(&mut buf, [("a", 1), ("b", 2)]);
schema.str_int.write(&mut buf, &my_hashmap);
```

For maps with message values, there's a closure API similar to nested messages:

```rust
schema.str_msg.write_msg(&mut buf, "key1", |buf, scm| {
    scm.label.write(buf, "nested");
    scm.value.write(buf, 42);
});
```

On the read side, each map entry comes back as a `(key, Option<value>)` tuple:

```rust
for field in MsgWithMaps::decode(&buf) {
    match field? {
        MsgWithMapsField::StrInt((k, v)) => {
            map.insert(k, v.unwrap_or_default());
        },
        _ => {}
    }
}
```

The value is `Option` because protobuf technically allows a map entry with a key but no value — in proto3 that means the default value, but tacky gives you explicit presence and lets you decide. `write_entry` takes the same `Option`, so you can write a key-only entry deliberately.

## Performance

Tacky encodes in a single pass. Pre-computed tags and the [tack](#the-tack-primitive) length-patching strategy eliminate the size-calculation pass that prost and similar libraries need for nested messages and packed fields.

Encode, measured in one run on an M3 (ARM) across four real-world schemas. `tacky` writes into a `Vec<u8>`; `tacky-rev` into a caller-provided fixed slice, filled backwards, which needs no length placeholders at all — see [Buffers and write direction](#buffers-and-write-direction) for what that buys and what it costs. The C++ column is the fair arm: `cpp-noutf8` for proto3, `cpp` for proto2.

Both ratio columns are for the default forward writer; `tacky-rev`'s own ratio against C++ is in the rightmost column.

| Corpus | Size | tacky | tacky-rev | prost | C++ | tacky vs prost | tacky vs C++ | rev vs C++ |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **pprof** — real Go heap profile | 847 KB | 490 µs | 449 | 1314 | 772 | **2.7x** | **1.6x** | **1.7x** |
| **Descriptor set** — with source info | 126 KB | 48.4 µs | 38.8 | 174 | 63.0 | **3.6x** | **1.3x** | **1.6x** |
| **Descriptor set** — schemas only | 20 KB | 11.2 µs | 8.9 | 24.5 | 13.5 | **2.2x** | **1.2x** | **1.5x** |
| **OTLP traces** — 512 spans | 355 KB | 109 µs | 85.5 | 235 | 136 | **2.2x** | **1.3x** | **1.6x** |
| **OTLP traces** — 200 spans | 145 KB | 42.8 µs | 34.0 | 90.4 | 51.5 | **2.1x** | **1.2x** | **1.5x** |
| **OTLP logs** — 512 records | 233 KB | 66.8 µs | 48.3 | 120 | 68.2 | **1.8x** | **1.0x** | **1.4x** |
| **Access log** — 100 entries, map headers | 62 KB | 11.4 µs | 12.3 | 28.7 | 19.2 | **2.5x** | **1.7x** | **1.6x** |

Two things the spread shows, both worth knowing before you believe the good numbers:

**The win tracks length prefixes per byte, not size.** A descriptor set with source info is thousands of tiny messages each carrying two packed `int32` arrays — three length prefixes per ~15 bytes of payload — and that is where the sizing pass costs prost most and tacky nothing. OTLP logs are the opposite: one ~120-byte body per record, so copying dominates and there is barely any prefix work to skip. Hence 3.6x down to 1.8x against prost on the same encoder.

**`tacky-rev` is faster almost everywhere, but it asks for something.** It needs an upper bound on the output size and it constrains how you write repeated fields — see [Buffers and write direction](#buffers-and-write-direction). The access log is where it loses: that corpus is dominated by a scalar-valued `map`, whose entry lengths are computable in advance in *both* directions, so there is no placeholder for a reverse writer to eliminate.

Output is byte-for-byte the same length as prost's on every corpus above; each bench prints both lengths so that stays checked.

How to run these, which arms are a fair comparison, and where the corpora come from: [`testing/benches/README.md`](testing/benches/README.md).

Decoding is roughly on par with prost when materializing into owned structs. Tacky's decode model is zero-copy for strings, bytes, and sub-messages, so real-world decode performance depends on how much copying your application actually needs.

## Buffers and write direction

Writes go through the `WriteBuf` trait, and three buffers implement it:

| Buffer | Direction | Capacity | Use for |
| :--- | :--- | :--- | :--- |
| `Vec<u8>` | forward | grows | the default |
| `SliceBuf` | forward | fixed | `no_std`, no allocator |
| `RevBuf` | **backwards** | fixed | the fastest path, when you can bound the output |

Direction is a compile-time property, not a runtime flag — it is an associated type on the buffer plus a `const REVERSE: bool` — so each writer's unused arm folds away and the forward path pays nothing for the reverse one.

**Why backwards is faster.** A forward writer meets a nested message's length prefix *before* it knows the length, so it reserves a placeholder and patches it afterwards, widening and memmoving the payload if the guess was too small. A backwards writer runs the body first and prepends the length once it is known: no placeholder, no reserved width, no shift, and always a minimal-width varint. That is where the 1.4–1.7x over the C++ runtime comes from, and it grows with how many length prefixes your messages have per byte of payload.

**What `RevBuf` asks of you in return:**

- **An upper bound on the output size.** It cannot grow; exceeding the buffer panics. Over-provisioning costs only address space, and a bounded batch is normal for an exporter, but it is a real requirement. This is also why there is no cold-buffer story for it — you always hand it memory you already own.
- **The output lives at the tail.** `written()` returns a slice from the middle of your buffer, not from index 0. If a sink demands an owned `Vec` starting at 0, that is one extra copy.
- **Let the writers own repeated-field iteration.** Elements have to be emitted back-to-front, which only a call that owns the loop can do — so use `write` and `write_msgs` rather than a hand-rolled loop of `write_single`/`write_msg`. Those single-entry escape hatches still work, but through a reverse buffer you must call them tail-first. The iterator must also be double-ended, which rules out a `HashSet`'s (a forward buffer accepts it).
- **`Display`-based writes are forward-only.** `PbDisplay`, `FmtWriter` and the `io::Write` adapter all stream in chunks, and chunks would land reversed, so they panic on a `RevBuf`. Format into a `String` first if you need this.
- **Maps gain nothing.** A map entry's length is two scalar lengths, computable in advance, so the forward path needs no placeholder either — there is nothing for the reverse writer to eliminate, and it can come out slower on a map-dominated message. Map entry *order* is unspecified by protobuf, so entries are written in iteration order in both directions; the only visible consequence is that an iterator yielding a duplicate key resolves last-one-wins to the other value than it would forwards.

`SliceBuf` shares the fixed-capacity constraint but nothing else: it appends, so every ordering caveat above is irrelevant to it, and a placeholder that needs widening still works as long as the buffer has room.

Code that has not picked a buffer writes through `AnyDir`, which erases the direction. It asks for double-ended iterators on repeated fields, because the buffer it wraps might turn out to be a `RevBuf`.

## Deserialization

`tacky-build` generates an enum with a variant per field, and an iterator that yields them one at a time. You match on variants and build your domain object from primitives. You can either exhaustively match all the fields or just select what you care about. Unknown fields are skipped by the iterator.

```rust
for field in SimpleMessage::decode(&buf) {
    match field? {
        SimpleMessageField::Text(s) => { /* s is a &str */ },
        SimpleMessageField::Numbers(n) => { /* n is an i32 */ },
        _ => {}
    }
}
```

Fields come back as basic Rust primitives — `&str`, `i32`, `f64` — and mapping them to your domain types is up to you. Only one variant is on the stack at a time, however many fields the message has, where a generated struct costs you all of them at once.

### Repeated fields

Unpacked repeated fields appear as one variant per occurrence — match in the loop and append:

```rust
let mut tags: Vec<String> = Vec::new();
for field in Message::decode(&buf) {
    match field? {
        MessageField::Tag(s) => tags.push(s.to_owned()),
        _ => {}
    }
}
```

Packed repeated fields (and `repeated` scalars in proto3, which are packed by default) come back as a single variant carrying an iterator over the elements. Each element is a `Result`, since varint decoding can fail mid-stream:

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

### Enums and nested messages

Proto enums come back as a Rust enum with an extra `__Unrecognized(i32)` variant, so a value added by a newer producer is something you handle rather than something that breaks you:

```rust
UserField::Tier(t) => user.tier = match t {
    proto::Tier::Free => Some(Tier::Free),
    proto::Tier::Pro => Some(Tier::Pro),
    proto::Tier::__Unrecognized(_) => None,
},
```

Nested messages give you a sub-decoder you iterate exactly like the outer one, so a domain object gets built field-by-field the whole way down, without the proto's intermediate struct ever existing.

## Limitations

Tacky focuses on dumping data to the wire fast, and reading it as its presented. As a result, several protobuf features are either irrelevant or unsupported.

**Extensions, RPC, custom defaults** are not supported.

**Protobuf merge semantics are not implemented.**
Due to the design of the deserializer as a-field-at-a-time, it cannot automatically merge repeated instances of a singular message. If that is required for correctness in your case, you can implement it in your code.

**OneOf is not enforced.** The serializer generates a OneOf type that groups the variants together, but nothing prevents you from writing more than one (except common sense). The deserializer flattens OneOf variants into individual fields like any other. If you need to enforce mutual exclusivity, you can implement it in your code.

## How It Works

Tacky is built on a few ideas that work together.

### Zero-sized schemas

Every type in the schema system — scalar markers like `Int32` and `PbString`, label wrappers like `Optional<P>` and `Repeated<P>`, and the `Field<N, P>` struct itself — is a zero-sized type (ZST). They exist only to carry type information through the generic system; at runtime they occupy no memory at all.

A generated message schema is just a struct of these ZSTs:

```rust
pub struct SimpleMessage {
    text: Field<1, Optional<PbString>>,     // 0 bytes
    numbers: Field<2, Repeated<Int32>>,     // 0 bytes
}
// size_of::<SimpleMessage>() == 0
```

This is why the exhaustiveness pattern works without overhead. When you write a struct literal for compile-time field checking, you're not constructing anything — the compiler verifies every field is accounted for, and the generated code is identical to calling `.write()` on each field individually.

The `.write()` method consumes `self` (a zero-sized value) and returns `Self` (another zero-sized value of the same type). The returned value slots back into the struct literal, satisfying the type checker. The actual work — writing bytes to the buffer — happens as a side effect.

### Const generics for field numbers

The field number is a const generic parameter on `Field<const N: u32, P>`. This means the protobuf tag — which combines the field number and wire type — can be computed entirely at compile time:

```rust
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
```

`const { EncodedTag::new(N, P::WIRE_TYPE) }` pre-computes the varint-encoded tag bytes at compile time and stores them as a `[u8; 5]` plus a length. At runtime, writing a tag is just copying 1-2 bytes — no varint encoding loop. In a repeated field with thousands of elements, that loop would otherwise run on every single element.

### The Tack primitive

Protobuf's wire format requires the byte length of nested messages and packed repeated fields to be written *before* their contents. The standard approach is two passes: iterate once to calculate the length, then iterate again to write the data.

Tacky uses a different strategy. The `Tack` struct reserves a byte, lets you write data past it, and then patches the real length in when it's done. if the length if greater than 1 byte can fit (128 bytes), you pay a memmove of that data to extend the prefix space. While this sounds wasteful, memmove is still order of magnitude faster than the serialization work, thus the good performance regardless.

```
Buffer before Tack:  [... tag]
After Tack::new():   [... tag | 00 ]  ← 1-byte placeholder
After writing data:  [... tag | 00 | actual data bytes... ]
After Tack closes:   [... tag | len len len | actual data bytes... ]
```

`Tack` implements `Drop`, so the length is patched automatically when it goes out of scope. This is what makes the nested message closure API work — the caller never has to finalize anything:

```rust
pub fn write_msg(self, buf: &mut Vec<u8>, mut f: impl FnMut(&mut Vec<u8>, M)) -> Self {
    let t = const { EncodedTag::new(N, WireType::LEN) };
    t.write(buf);
    let t = Tack::new(buf);       // placeholder written, t borrows buf
    f(t.buffer, M::default());    // user writes nested fields into the tack's buffer
    // t drops here → length patched
    Field::new()
}
```

The borrow through `t.buffer` also prevents the caller from accidentally writing to the outer buffer while the Tack is active, since `Tack` holds the `&mut Vec<u8>`.

## Acknowledgements

`tacky-build` vendors a heavily modified copy of [pb-rs](https://github.com/tafia/quick-protobuf/tree/master/pb-rs) (from the [quick-protobuf](https://github.com/tafia/quick-protobuf) project, MIT-licensed), used purely as a `.proto` parser and validator at build time. pb-rs parses and validates `.proto` files in pure Rust, which lets tacky avoid the `protoc` system dependency that prost and other libraries pull in. The vendored copy has been stripped down and adapted to that role — none of pb-rs's own code generation is used; the schema and decoder code is all generated by tacky.
