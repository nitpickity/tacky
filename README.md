# Tacky

A protobuf serializer and deserializer for Rust that gets out of the way of your domain types.

Note: this is work-in-progress, APIs may change. The basic idea will not.

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
pub struct SimpleMessageSchema {
    text: Field<1, Optional<PbString>>,
    numbers: Field<2, Repeated<Int32>>,
    blobs: Field<3, Repeated<PbBytes>>,
    map: Field<4, Map<PbString, Double>>,
}
```

Which you use like this:

```rust
let mut buffer = Vec::new();
let schema = SimpleMessageSchema::default();

schema.text.write(&mut buffer, Some("hello world"));
schema.numbers.write(&mut buffer, [1, 2, 3, 4]);
```

`Optional` fields take an `Option`, `Repeated` fields take anything iterable. String fields accept any `AsRef<str>`, so your own string types work without conversion.

## Exhaustiveness Checking

The usual assumption is that skipping the generated struct means losing safety — forget to write a field and nothing tells you. Tacky sidesteps this with a small trick: every `.write()` call returns the field schema value back. This means you can use the generated schema as a literal to "fill in" and get compile-time exhaustiveness for free:

```rust
let mut buffer = Vec::new();
let schema = SimpleMessageSchema::default();

SimpleMessageSchema {
    text: schema.text.write(&mut buffer, Some("hello world")),
    numbers: schema.numbers.write(&mut buffer, [1, 2, 3, 4]),
    blobs: schema.blobs,  // explicitly skipped
    ..schema              // skip the rest
};
```

`SimpleMessageSchema` is zero-sized — nothing is being constructed here. The `.write()` calls are the side effects, filling the buffer. The struct literal is purely a compile-time exhaustiveness check. Add a field to your proto schema and this stops compiling. Same safety as a generated data struct, none of the allocation.

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

For repeated message fields, call `write_msg` multiple times — once per message you want to write. The nested schema works the same way as the outer one, including exhaustiveness checks if you want them:

```rust
schema.events.write_msg(&mut buf, |buf, scm| {
    EventSchema {
        name: scm.name.write(buf, Some("click")),
        ..scm
    }
});
schema.events.write_msg(&mut buf, |buf, scm| {
    EventSchema {
        name: scm.name.write(buf, Some("scroll")),
        ..scm
    }
});
```

Or, if you're writing from a collection:

```rust
let events = ["scroll", "click"];
Message {
    events: {
        for e in events {
            schema.events.write_msg(&mut buf, |buf, scm| {
                EventSchema {
                    name: scm.name.write(buf, Some(e)),
                    ..scm
                }
            });
        }
        schema.events // mark as written; a for loop returns (), not the field
    },
    ..Message::default()
};
```

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

You can also write individual entries with an explicit `None` value, which is useful for representing deletions in update messages:

```rust
schema.str_int.write_entry(&mut buf, "deleted_key", None::<i32>);
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

The value is `Option` because protobuf technically allows a map entry with a key but no value — in proto3 that means the default value, but tacky gives you explicit presence and lets you decide.

## Performance

Protobuf has two cases where the length of a field must be written before its contents — packed repeated fields and nested messages. The conventional approach is two passes: iterate to calculate the length, write the length, then iterate again to write the data.

Tacky instead writes a placeholder length, writes the data in a single pass, and patches the real length in place when the scope closes. This applies to both packed fields and nested messages. For varints this is much faster. Prost wants to allocate a Vec for both repeated and packed values; calculating the length of this Vec in the unpacked repeated case where the tag length also needs to be calculated results in particularly bad performance.

Benched on an M3 Macbook Pro, against Prost 0.14.
| Benchmark Suite | Variant | Tacky Time | Prost Time | Performance Difference |
| :--- | :--- | :--- | :--- | :--- |
| **Tiny Nested Messages** | Default | ~12 ns | ~26 ns | Tacky is ~2.1x faster |
| **Big Nested Messages** | Default | ~21 ns | ~80 ns | Tacky is ~3.8x faster |
| **Packed Repeated** | Few (10) | ~26 ns | ~40 ns | Tacky is ~1.5x faster |
| **Packed Repeated** | Many (100) | ~248 ns | ~397 ns | Tacky is ~1.6x faster |
| **Packed Repeated** | Hundreds (1000) | ~2.45 µs | ~3.89 µs | Tacky is ~1.6x faster |
| **Normal Repeated** | Few (10) | ~28 ns | ~47 ns | Tacky is ~1.7x faster |
| **Normal Repeated** | Many (100) | ~275 ns | ~476 ns | Tacky is ~1.7x faster |
| **Normal Repeated** | Hundreds (1000) | ~2.77 µs | ~4.85 µs | Tacky is ~1.8x faster |
| **Mixed Usage** | All fields set | ~57 ns | ~172 ns | Tacky is ~3.0x faster |
| **Mixed Usage** | Half fields set | ~27 ns | ~90 ns | Tacky is ~3.3x faster |
| **Mixed Usage** | Few fields set (1-2) | ~2.0 ns | ~12.1 ns | Tacky is ~6.0x faster |

## Deserialization

`tacky-build` generates an enum with a variant per field, and an iterator that yields them one at a time. You match on variants and build your domain object from primitives. You can either exhaustively match all the fields or just select what you care about. Unknown fields are skipped by the iterator.

```rust
for field in SimpleMessageDecoder::new(&buf) {
    match field? {
        SimpleMessageField::Text(s) => { /* s is a &str */ },
        SimpleMessageField::Numbers(n) => { /* n is an i32 */ },
        _ => {}
    }
}
```

Fields come back as basic Rust primitives — `&str`, `i32`, `f64`, etc. Mapping those to your domain types is up to you. Only one enum variant lives on the stack at a time, regardless of how many fields the message has. The struct approach that prost and others use can lead to the stack size of the message alone being much larger than the serialized data, and it grows with every field.


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
pub struct SimpleMessageSchema {
    text: Field<1, Optional<PbString>>,     // 0 bytes
    numbers: Field<2, Repeated<Int32>>,     // 0 bytes
}
// size_of::<SimpleMessageSchema>() == 0
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

Tacky uses a different strategy. The `Tack` struct reserves a fixed-width placeholder (3 bytes by default, enough for messages up to ~2MB), lets you write data past it, and then patches the real length in when it's done:

```
Buffer before Tack:  [... tag]
After Tack::new():   [... tag | 00 00 00 ]  ← 3-byte placeholder
After writing data:  [... tag | 00 00 00 | actual data bytes... ]
After Tack closes:   [... tag | len len len | actual data bytes... ]
```

The placeholder is a fixed-width varint — padded with continuation bits so that any length up to 2^21 fits without moving data. If the data happens to exceed that (the cold path), Tack shifts the data right and expands the length field. This almost never happens, and is marked `#[cold]` so the optimizer keeps it out of the hot path.

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
