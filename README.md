# Tacky

A protobuf serializer and deserializer for Rust that gets out of the way of your domain types.

Note: this is work-in-progress, APIs may change. the basic idea will not.

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

The usual assumption is that skipping the generated struct means losing safety — forget to write a field and nothing tells you. Tacky sidesteps this with a small trick: every `.write()` call returns the field schema value back. This means you can use the generated schema as a literal to "fill in". and get compile-time exhaustiveness for free:

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
}
```
// ---- OR ----
```rust
let events = ["scroll", "click"];
Message {
    events: {
        for e in events {
            schema.events.write_msg(&mut buf, |buf, scm| {
            EventSchema {
                name: scm.name.write(buf, Some(e)),
                ..scm
            }}
        }
        schema.events //gotta mark this as written explicitly as a for loop returns (), not the written field. 
        },
        ..Message::default()
}

```

## Performance

Protobuf has two cases where the length of a field must be written before its contents — packed repeated fields and nested messages. The conventional approach is two passes: iterate to calculate the length, write the length, then iterate again to write the data.

Tacky instead writes a placeholder length, writes the data in a single pass, and patches the real length in place when the scope closes. This applies to both packed fields and nested messages. for varints this is much faster. 
prost wants to allocate a vec for both repeated and packed values, the calculating the length of this vec in the repeat unpacked case where the tag length needs to be calculated as well results in particularly bad performance.


| Benchmark Suite | Variant | Tacky Time | Prost Time | Performance Difference |
| :--- | :--- | :--- | :--- | :--- |
| **Tiny nested Messages** | Default | ~29 ns | ~26 ns | Prost is ~1.1x faster |
| **Big Nested Messages** | Default | ~35 ns | ~80 ns | Tacky is ~2.3x faster |
| **Packed Repeated** | Few (10) | ~36 ns | ~52 ns | Tacky is ~1.4x faster |
| **Packed Repeated** | Many (100) | ~363 ns | ~565 ns | Tacky is ~1.5x faster |
| **Packed Repeated** | Hundreds (1000) | ~3.80 µs| ~5.51 µs | Tacky is ~1.4x faster |
| **Normal Repeated** | Few (10) | ~26 ns | ~62 ns | Tacky is ~2.4x faster |
| **Normal Repeated** | Many (100) | ~281 ns | ~618 ns | Tacky is ~2.2x faster |
| **Normal Repeated** | Hundreds (1000) | ~2.57 µs | ~6.58 µs | Tacky is ~2.5x faster |
| **Mixed Usage** | All fields set | ~121 ns | ~187 ns | Tacky is ~1.5x faster |
| **Mixed Usage** | Half fields set | ~60 ns | ~87 ns | Tacky is ~1.4x faster |
| **Mixed Usage** | Few fields set (1-2) | ~1.7 ns | ~12.1 ns | Tacky is ~7.1x faster |

## Deserialization

`tacky-build` generates an enum with a variant per field, and an iterator that yields them one at a time. You match on variants and build your domain object from primitives. you can either exhaustively match all the fields or just select what you care about at this point. unknown fields are skipped by the iterator. if you need to keep unknown fields, let me know.

```rust
for field in SimpleMessageDecoder::new(&buf) {
    match field? {
        SimpleMessageField::Text(s) => { /* s is a &str */ },
        SimpleMessageField::Numbers(n) => { /* n is an i32 */ },
        _ => {}
    }
}
```

Fields come back as basic Rust primitives — `&str`, `i32`, `f64`, etc. Mapping those to your domain types is up to you. Only one enum variant lives on the stack at a time, regardless of how many fields the message has. the struct approach prost and co use can lead to just the size on the stack of the message before any data is filled in to be much larger than the message itself, and grows with more fields.


## Limitations

**Imports and nested definitions are not yet supported.** 
All message definitions must be flat within a single file.

**protobuf merge semantics dont work**
Due to the design of the deserializer as a-field-at-a-time, it doesnt automatically merge repeated instances of a "singular" messages. if that is required for correctness in your case, you can implement it in your code.

**OneOf fields are flattened into the schema.** For a serializer this is fine — the OneOf constraint is more meaningful during deserialization. If you need to enforce OneOf semantics you'll need to do that in your own code.

**The exhaustiveness pattern is verbose.** A message with many fields means a long struct literal with repetitive `field: schema.field.write(&mut buf, data)` lines. This is opt-in — you only pay the verbosity cost if you want the compile-time exhaustiveness check. For partial writes, just call `.write()` on the fields you need.

## How It Works

*TODO: Tack primitive, zero-sized types, const generics, the drop trick.*
