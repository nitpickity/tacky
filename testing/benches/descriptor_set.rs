//! `FileDescriptorSet` — the shape every protobuf toolchain actually encodes.
//!
//! A descriptor set is what `protoc --descriptor_set_out` produces and what every
//! gRPC reflection service, schema registry and dynamic-message runtime ships
//! around. It is the opposite of GoogleMessage1: deeply nested (file → message →
//! nested message → field → options), overwhelmingly short strings and small
//! varints, and thousands of *absent* optional fields. That makes it the densest corpus
//! in the suite in length prefixes per byte, which is where both tacky's placeholder
//! patching and prost's `encoded_len` recursion cost the most.
//!
//! Three fixtures, all checked in so `cargo bench` needs no local `protoc`
//! (regenerate with `scripts/gen_bench_fixtures.sh`):
//!
//! - `descriptor_proto` — the vendored `descriptor.proto` describing itself, 7.6 KB.
//!   Extension ranges, reserved ranges, nested enums, oneofs, custom defaults.
//! - `testing_protos` — this repo's own protos plus `descriptor.proto`, with imports,
//!   20 KB. Flatter: mostly names, field numbers and `json_name`s, plus map-entry messages.
//! - `registry` — `testing_protos`' files plus the vendored OTLP schema protos, with
//!   `--include_source_info`, 126 KB. A superset rather than an independent corpus, which
//!   makes the pair a controlled comparison: source info is the only variable. This is the
//!   one that matches reality: a schema
//!   registry, a gRPC reflection service or anything buf ships carries source info,
//!   and a real service's descriptor set runs from ~100 KB into the megabytes. The
//!   other two are an order of magnitude small and fit in L2 whole, which is the
//!   cache-resident best case this suite otherwise takes care to avoid. Most of the
//!   extra bytes are `SourceCodeInfo.location`: thousands of short packed `int32`
//!   arrays, a work mix neither of the other fixtures contains at all.
//!
//! Arms are the same four as `benches/google_message1.rs`:
//!
//! - `tacky` — one pass, no sizing.
//! - `prost` — `Message::encode`, prost's published shape. It computes `encoded_len()`
//!   itself for its capacity check, so the sizing pass is included whether or not the
//!   buffer was reserved.
//!
//! The prost side is `prost-types`, which is generated from `descriptor.proto`, so
//! this comparison costs no extra codegen. tacky's side is generated from
//! `testing/protos/descriptor.proto` (protobuf v3.20.3) by the build script.
//!
//! The writer covers every field of the descriptor messages the fixtures contain,
//! `SourceCodeInfo` included. It does *not* cover `ServiceDescriptorProto`,
//! `UninterpretedOption`, or the option messages with no set fields here
//! (`EnumOptions`, `EnumValueOptions`, `OneofOptions`, `ExtensionRangeOptions`): none
//! appear in any fixture. Services are absent because tacky does not generate service
//! definitions — RPC is not what it is for — not because they were overlooked. The rest
//! is not a silent gap either: the round-trip assert below compares whole messages, so
//! a fixture that grew one of them would fail rather than quietly measure less work.
//!
//! Wire output is checked by decoding tacky's bytes with prost and comparing messages
//! rather than by comparing byte strings, because a reverse writer emits fields in the
//! opposite order. The two encoders' byte *counts* do match — a placeholder is grown, not
//! padded — and each fixture prints both so that stays checked rather than assumed.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prost::Message;

#[cfg(feature = "cpp")]
#[path = "common/cpp_arms.rs"]
mod cpp_arms;

// The writer and the generated module live in `common/` because `benches/comparison.rs`
// needs them too, for its rotating group.
#[path = "common/fds_writer.rs"]
mod fds_writer;
use fds_writer::{tacky_encode, td};

const FDS_DESCRIPTOR_PROTO: &[u8] = include_bytes!("../data/descriptor_proto.fds");
const FDS_TESTING_PROTOS: &[u8] = include_bytes!("../data/testing_protos.fds");
const FDS_REGISTRY: &[u8] = include_bytes!("../data/registry.fds");

// ---------------------------------------------------------------------------
// Decode
// ---------------------------------------------------------------------------
//
// Decoding into prost's structs is what makes the comparison apples-to-apples:
// both arms end up owning the same `prost_types` value, so the only difference is
// the parser. Arms for sub-messages the fixtures never contain are
// `unimplemented!()` rather than ignored, so a grown fixture fails loudly.

fn tacky_decode(wire: &[u8]) -> prost_types::FileDescriptorSet {
    let mut set = prost_types::FileDescriptorSet::default();
    for field in td::FileDescriptorSet::decode(wire) {
        match field.unwrap() {
            td::FileDescriptorSetField::File(fields) => set.file.push(read_file(fields)),
        }
    }
    set
}

fn read_file(fields: td::FileDescriptorProtoFields<'_>) -> prost_types::FileDescriptorProto {
    use td::FileDescriptorProtoField as F;

    let mut f = prost_types::FileDescriptorProto::default();
    for field in fields {
        match field.unwrap() {
            F::Name(v) => f.name = Some(v.to_string()),
            F::Package(v) => f.package = Some(v.to_string()),
            F::Dependency(v) => f.dependency.push(v.to_string()),
            F::PublicDependency(v) => f.public_dependency.push(v),
            F::WeakDependency(v) => f.weak_dependency.push(v),
            F::MessageType(m) => f.message_type.push(read_message(m)),
            F::EnumType(e) => f.enum_type.push(read_enum(e)),
            F::Extension(x) => f.extension.push(read_field(x)),
            F::Options(opts) => {
                use td::FileOptionsField as O;
                let o = f.options.get_or_insert_with(Default::default);
                for opt in opts {
                    match opt.unwrap() {
                        O::JavaPackage(v) => o.java_package = Some(v.to_string()),
                        O::JavaOuterClassname(v) => o.java_outer_classname = Some(v.to_string()),
                        O::JavaMultipleFiles(v) => o.java_multiple_files = Some(v),
                        O::JavaStringCheckUtf8(v) => o.java_string_check_utf8 = Some(v),
                        O::OptimizeFor(v) => o.optimize_for = Some(v.into()),
                        O::GoPackage(v) => o.go_package = Some(v.to_string()),
                        O::CcGenericServices(v) => o.cc_generic_services = Some(v),
                        O::JavaGenericServices(v) => o.java_generic_services = Some(v),
                        O::PyGenericServices(v) => o.py_generic_services = Some(v),
                        O::PhpGenericServices(v) => o.php_generic_services = Some(v),
                        O::Deprecated(v) => o.deprecated = Some(v),
                        O::CcEnableArenas(v) => o.cc_enable_arenas = Some(v),
                        O::ObjcClassPrefix(v) => o.objc_class_prefix = Some(v.to_string()),
                        O::CsharpNamespace(v) => o.csharp_namespace = Some(v.to_string()),
                        O::SwiftPrefix(v) => o.swift_prefix = Some(v.to_string()),
                        O::PhpClassPrefix(v) => o.php_class_prefix = Some(v.to_string()),
                        O::PhpNamespace(v) => o.php_namespace = Some(v.to_string()),
                        O::PhpMetadataNamespace(v) => {
                            o.php_metadata_namespace = Some(v.to_string())
                        }
                        O::RubyPackage(v) => o.ruby_package = Some(v.to_string()),
                        O::JavaGenerateEqualsAndHash(_) | O::UninterpretedOption(_) => {
                            unimplemented!("FileOptions field absent from both fixtures")
                        }
                    }
                }
            }
            F::Syntax(v) => f.syntax = Some(v.to_string()),
            F::SourceCodeInfo(sci) => {
                use td::SourceCodeInfoField as S;
                let info = f.source_code_info.get_or_insert_with(Default::default);
                for field in sci {
                    match field.unwrap() {
                        S::Location(loc) => {
                            use td::SourceCodeInfoLocationField as L;
                            let mut l = prost_types::source_code_info::Location::default();
                            for field in loc {
                                match field.unwrap() {
                                    L::Path(iter) => l.path.extend(iter.map(|r| r.unwrap())),
                                    L::Span(iter) => l.span.extend(iter.map(|r| r.unwrap())),
                                    L::LeadingComments(v) => {
                                        l.leading_comments = Some(v.to_string())
                                    }
                                    L::TrailingComments(v) => {
                                        l.trailing_comments = Some(v.to_string())
                                    }
                                    L::LeadingDetachedComments(v) => {
                                        l.leading_detached_comments.push(v.to_string())
                                    }
                                }
                            }
                            info.location.push(l);
                        }
                    }
                }
            }
            F::Service(_) => {
                unimplemented!("tacky does not generate service definitions; see the module docs")
            }
        }
    }
    f
}

fn read_message(fields: td::DescriptorProtoFields<'_>) -> prost_types::DescriptorProto {
    use td::DescriptorProtoField as F;

    let mut m = prost_types::DescriptorProto::default();
    for field in fields {
        match field.unwrap() {
            F::Name(v) => m.name = Some(v.to_string()),
            F::Field(f) => m.field.push(read_field(f)),
            F::Extension(x) => m.extension.push(read_field(x)),
            F::NestedType(n) => m.nested_type.push(read_message(n)),
            F::EnumType(e) => m.enum_type.push(read_enum(e)),
            F::ExtensionRange(r) => {
                use td::DescriptorProtoExtensionRangeField as R;
                let mut range = prost_types::descriptor_proto::ExtensionRange::default();
                for f in r {
                    match f.unwrap() {
                        R::Start(v) => range.start = Some(v),
                        R::End(v) => range.end = Some(v),
                        R::Options(_) => {
                            unimplemented!("ExtensionRangeOptions absent from both fixtures")
                        }
                    }
                }
                m.extension_range.push(range);
            }
            F::OneofDecl(d) => {
                use td::OneofDescriptorProtoField as D;
                let mut decl = prost_types::OneofDescriptorProto::default();
                for f in d {
                    match f.unwrap() {
                        D::Name(v) => decl.name = Some(v.to_string()),
                        D::Options(_) => {
                            unimplemented!("OneofOptions absent from both fixtures")
                        }
                    }
                }
                m.oneof_decl.push(decl);
            }
            F::Options(opts) => {
                use td::MessageOptionsField as O;
                let o = m.options.get_or_insert_with(Default::default);
                for opt in opts {
                    match opt.unwrap() {
                        O::MessageSetWireFormat(v) => o.message_set_wire_format = Some(v),
                        O::NoStandardDescriptorAccessor(v) => {
                            o.no_standard_descriptor_accessor = Some(v)
                        }
                        O::Deprecated(v) => o.deprecated = Some(v),
                        O::MapEntry(v) => o.map_entry = Some(v),
                        O::UninterpretedOption(_) => {
                            unimplemented!("UninterpretedOption absent from both fixtures")
                        }
                    }
                }
            }
            F::ReservedRange(r) => {
                use td::DescriptorProtoReservedRangeField as R;
                let mut range = prost_types::descriptor_proto::ReservedRange::default();
                for f in r {
                    match f.unwrap() {
                        R::Start(v) => range.start = Some(v),
                        R::End(v) => range.end = Some(v),
                    }
                }
                m.reserved_range.push(range);
            }
            F::ReservedName(v) => m.reserved_name.push(v.to_string()),
        }
    }
    m
}

fn read_field(fields: td::FieldDescriptorProtoFields<'_>) -> prost_types::FieldDescriptorProto {
    use td::FieldDescriptorProtoField as F;

    let mut f = prost_types::FieldDescriptorProto::default();
    for field in fields {
        match field.unwrap() {
            F::Name(v) => f.name = Some(v.to_string()),
            F::Number(v) => f.number = Some(v),
            F::Label(v) => f.label = Some(v.into()),
            F::Type(v) => f.r#type = Some(v.into()),
            F::TypeName(v) => f.type_name = Some(v.to_string()),
            F::Extendee(v) => f.extendee = Some(v.to_string()),
            F::DefaultValue(v) => f.default_value = Some(v.to_string()),
            F::OneofIndex(v) => f.oneof_index = Some(v),
            F::JsonName(v) => f.json_name = Some(v.to_string()),
            F::Proto3Optional(v) => f.proto3_optional = Some(v),
            F::Options(opts) => {
                use td::FieldOptionsField as O;
                let o = f.options.get_or_insert_with(Default::default);
                for opt in opts {
                    match opt.unwrap() {
                        O::Ctype(v) => o.ctype = Some(v.into()),
                        O::Packed(v) => o.packed = Some(v),
                        O::Jstype(v) => o.jstype = Some(v.into()),
                        O::Lazy(v) => o.lazy = Some(v),
                        O::Deprecated(v) => o.deprecated = Some(v),
                        O::Weak(v) => o.weak = Some(v),
                        // `unverified_lazy` landed in protobuf 3.21, after the
                        // descriptor.proto prost-types is generated from, so prost has
                        // nowhere to put it. Neither fixture sets it.
                        O::UnverifiedLazy(_) | O::UninterpretedOption(_) => {
                            unimplemented!("FieldOptions field absent from both fixtures")
                        }
                    }
                }
            }
        }
    }
    f
}

fn read_enum(fields: td::EnumDescriptorProtoFields<'_>) -> prost_types::EnumDescriptorProto {
    use td::EnumDescriptorProtoField as F;

    let mut e = prost_types::EnumDescriptorProto::default();
    for field in fields {
        match field.unwrap() {
            F::Name(v) => e.name = Some(v.to_string()),
            F::Value(vals) => {
                use td::EnumValueDescriptorProtoField as V;
                let mut val = prost_types::EnumValueDescriptorProto::default();
                for f in vals {
                    match f.unwrap() {
                        V::Name(v) => val.name = Some(v.to_string()),
                        V::Number(v) => val.number = Some(v),
                        V::Options(_) => {
                            unimplemented!("EnumValueOptions absent from both fixtures")
                        }
                    }
                }
                e.value.push(val);
            }
            F::ReservedRange(r) => {
                use td::EnumDescriptorProtoEnumReservedRangeField as R;
                let mut range = prost_types::enum_descriptor_proto::EnumReservedRange::default();
                for f in r {
                    match f.unwrap() {
                        R::Start(v) => range.start = Some(v),
                        R::End(v) => range.end = Some(v),
                    }
                }
                e.reserved_range.push(range);
            }
            F::ReservedName(v) => e.reserved_name.push(v.to_string()),
            F::Options(_) => unimplemented!("EnumOptions absent from both fixtures"),
        }
    }
    e
}

/// Parse-only counterpart to `tacky_decode`: visits every field, folds each value into an
/// accumulator, allocates nothing. The `tacky` arm measures parse *plus* building
/// prost-types' owned structs, and that allocation dominates; this isolates the iterator
/// and its tag dispatch, which for descriptor.proto is the widest match in the corpus.
///
/// No prost counterpart exists by construction. Correctness is gated by `tacky_decode`.
/// The `unimplemented!` arms mirror that function's — those fields are absent from both
/// fixtures, so the traversal is identical.
fn tacky_walk(wire: &[u8]) -> u64 {
    let mut acc = 0u64;
    for field in td::FileDescriptorSet::decode(wire) {
        match field.unwrap() {
            td::FileDescriptorSetField::File(fields) => acc = acc.wrapping_add(walk_file(fields)),
        }
    }
    acc
}

fn walk_file(fields: td::FileDescriptorProtoFields<'_>) -> u64 {
    use td::FileDescriptorProtoField as F;

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }
    for field in fields {
        match field.unwrap() {
            F::Name(v) => add!(v.len()),
            F::Package(v) => add!(v.len()),
            F::Dependency(v) => add!(v.len()),
            F::PublicDependency(v) => add!(v),
            F::WeakDependency(v) => add!(v),
            F::MessageType(m) => add!(walk_message(m)),
            F::EnumType(e) => add!(walk_enum(e)),
            F::Extension(x) => add!(walk_field(x)),
            F::Options(opts) => {
                use td::FileOptionsField as O;
                for opt in opts {
                    match opt.unwrap() {
                        O::JavaPackage(v) => add!(v.len()),
                        O::JavaOuterClassname(v) => add!(v.len()),
                        O::JavaMultipleFiles(v) => add!(v),
                        O::JavaStringCheckUtf8(v) => add!(v),
                        O::OptimizeFor(v) => add!(i32::from(v)),
                        O::GoPackage(v) => add!(v.len()),
                        O::CcGenericServices(v) => add!(v),
                        O::JavaGenericServices(v) => add!(v),
                        O::PyGenericServices(v) => add!(v),
                        O::PhpGenericServices(v) => add!(v),
                        O::Deprecated(v) => add!(v),
                        O::CcEnableArenas(v) => add!(v),
                        O::ObjcClassPrefix(v) => add!(v.len()),
                        O::CsharpNamespace(v) => add!(v.len()),
                        O::SwiftPrefix(v) => add!(v.len()),
                        O::PhpClassPrefix(v) => add!(v.len()),
                        O::PhpNamespace(v) => add!(v.len()),
                        O::PhpMetadataNamespace(v) => add!(v.len()),
                        O::RubyPackage(v) => add!(v.len()),
                        O::JavaGenerateEqualsAndHash(_) | O::UninterpretedOption(_) => {
                            unimplemented!("FileOptions field absent from both fixtures")
                        }
                    }
                }
            }
            F::Syntax(v) => add!(v.len()),
            F::SourceCodeInfo(sci) => {
                use td::SourceCodeInfoField as S;
                for field in sci {
                    match field.unwrap() {
                        S::Location(loc) => {
                            use td::SourceCodeInfoLocationField as L;
                            for field in loc {
                                match field.unwrap() {
                                    L::Path(iter) | L::Span(iter) => {
                                        for r in iter {
                                            add!(r.unwrap());
                                        }
                                    }
                                    L::LeadingComments(v)
                                    | L::TrailingComments(v)
                                    | L::LeadingDetachedComments(v) => add!(v.len()),
                                }
                            }
                        }
                    }
                }
            }
            F::Service(_) => {
                unimplemented!("tacky does not generate service definitions; see the module docs")
            }
        }
    }
    acc
}

fn walk_message(fields: td::DescriptorProtoFields<'_>) -> u64 {
    use td::DescriptorProtoField as F;

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }
    for field in fields {
        match field.unwrap() {
            F::Name(v) => add!(v.len()),
            F::Field(f) => add!(walk_field(f)),
            F::Extension(x) => add!(walk_field(x)),
            F::NestedType(n) => add!(walk_message(n)),
            F::EnumType(e) => add!(walk_enum(e)),
            F::ExtensionRange(r) => {
                use td::DescriptorProtoExtensionRangeField as R;
                for f in r {
                    match f.unwrap() {
                        R::Start(v) => add!(v),
                        R::End(v) => add!(v),
                        R::Options(_) => {
                            unimplemented!("ExtensionRangeOptions absent from both fixtures")
                        }
                    }
                }
            }
            F::OneofDecl(d) => {
                use td::OneofDescriptorProtoField as D;
                for f in d {
                    match f.unwrap() {
                        D::Name(v) => add!(v.len()),
                        D::Options(_) => {
                            unimplemented!("OneofOptions absent from both fixtures")
                        }
                    }
                }
            }
            F::Options(opts) => {
                use td::MessageOptionsField as O;
                for opt in opts {
                    match opt.unwrap() {
                        O::MessageSetWireFormat(v) => add!(v),
                        O::NoStandardDescriptorAccessor(v) => add!(v),
                        O::Deprecated(v) => add!(v),
                        O::MapEntry(v) => add!(v),
                        O::UninterpretedOption(_) => {
                            unimplemented!("UninterpretedOption absent from both fixtures")
                        }
                    }
                }
            }
            F::ReservedRange(r) => {
                use td::DescriptorProtoReservedRangeField as R;
                for f in r {
                    match f.unwrap() {
                        R::Start(v) => add!(v),
                        R::End(v) => add!(v),
                    }
                }
            }
            F::ReservedName(v) => add!(v.len()),
        }
    }
    acc
}

fn walk_field(fields: td::FieldDescriptorProtoFields<'_>) -> u64 {
    use td::FieldDescriptorProtoField as F;

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }
    for field in fields {
        match field.unwrap() {
            F::Name(v) => add!(v.len()),
            F::Number(v) => add!(v),
            F::Label(v) => add!(i32::from(v)),
            F::Type(v) => add!(i32::from(v)),
            F::TypeName(v) => add!(v.len()),
            F::Extendee(v) => add!(v.len()),
            F::DefaultValue(v) => add!(v.len()),
            F::OneofIndex(v) => add!(v),
            F::JsonName(v) => add!(v.len()),
            F::Proto3Optional(v) => add!(v),
            F::Options(opts) => {
                use td::FieldOptionsField as O;
                for opt in opts {
                    match opt.unwrap() {
                        O::Ctype(v) => add!(i32::from(v)),
                        O::Packed(v) => add!(v),
                        O::Jstype(v) => add!(i32::from(v)),
                        O::Lazy(v) => add!(v),
                        O::Deprecated(v) => add!(v),
                        O::Weak(v) => add!(v),
                        O::UnverifiedLazy(_) | O::UninterpretedOption(_) => {
                            unimplemented!("FieldOptions field absent from both fixtures")
                        }
                    }
                }
            }
        }
    }
    acc
}

fn walk_enum(fields: td::EnumDescriptorProtoFields<'_>) -> u64 {
    use td::EnumDescriptorProtoField as F;

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }
    for field in fields {
        match field.unwrap() {
            F::Name(v) => add!(v.len()),
            F::Value(vals) => {
                use td::EnumValueDescriptorProtoField as V;
                for f in vals {
                    match f.unwrap() {
                        V::Name(v) => add!(v.len()),
                        V::Number(v) => add!(v),
                        V::Options(_) => {
                            unimplemented!("EnumValueOptions absent from both fixtures")
                        }
                    }
                }
            }
            F::ReservedRange(r) => {
                use td::EnumDescriptorProtoEnumReservedRangeField as R;
                for f in r {
                    match f.unwrap() {
                        R::Start(v) => add!(v),
                        R::End(v) => add!(v),
                    }
                }
            }
            F::ReservedName(v) => add!(v.len()),
            F::Options(_) => unimplemented!("EnumOptions absent from both fixtures"),
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Benches
// ---------------------------------------------------------------------------

fn bench_fixture(c: &mut Criterion, name: &str, fixture: &[u8]) {
    let set = prost_types::FileDescriptorSet::decode(fixture)
        .expect("checked-in fixture decodes as FileDescriptorSet");

    // Tacky's padded length prefixes rule out a byte compare, so check the
    // stronger thing: prost must decode tacky's output back to the same message.
    let mut tacky_wire = Vec::with_capacity(fixture.len() * 2);
    tacky_encode(tacky::AnyDir::from_mut(&mut tacky_wire), &set);
    assert_eq!(
        prost_types::FileDescriptorSet::decode(tacky_wire.as_slice()).unwrap(),
        set,
        "{name}: prost cannot read back what tacky wrote"
    );
    let mut prost_wire = Vec::with_capacity(fixture.len() + 64);
    set.encode(&mut prost_wire).unwrap();

    assert_eq!(
        tacky_decode(fixture),
        set,
        "{name}: tacky and prost decode differently"
    );

    // Throughput below is computed on prost's length for every arm, so the arms stay
    // comparable. The percentage is what tacky's length prefixes cost in bytes over prost's.
    println!(
        "{name}: {} files, prost {} B, tacky {} B (+{:.2}%)",
        set.file.len(),
        prost_wire.len(),
        tacky_wire.len(),
        (tacky_wire.len() as f64 / prost_wire.len() as f64 - 1.0) * 100.0,
    );

    let cap = tacky_wire.len().max(prost_wire.len());
    let mut group = c.benchmark_group(format!("encode_{name}"));
    group.throughput(Throughput::Bytes(prost_wire.len() as u64));
    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            tacky_encode(tacky::AnyDir::from_mut(&mut buf), &set);
            black_box(buf.as_slice());
            buf.clear();
        });
    });
    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            set.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // A downward buffer emits fields in the reverse of the order they are written, which is
    // legal, so this is checked by decoding rather than by comparing bytes.
    let mut rev_backing = vec![0u8; cap + 1024];
    let mut rb = tacky::RevBuf::new(&mut rev_backing);
    tacky_encode(tacky::AnyDir::from_mut(&mut rb), &set);
    assert_eq!(
        prost_types::FileDescriptorSet::decode(rb.written()).unwrap(),
        set,
        "reverse writer output does not decode back to the same message"
    );
    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; cap + 1024];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode(tacky::AnyDir::from_mut(&mut rb), &set);
            black_box(rb.written());
        });
    });
    // descriptor.proto is proto2, so the C++ runtime never validates its strings
    // and `cpp-cached` already is that runtime's floor — no no-UTF8 arm to add.
    #[cfg(feature = "cpp")]
    cpp_arms::bench_cpp_arms(
        &mut group,
        "cpp",
        testing::cpp::FILE_DESCRIPTOR_SET,
        &prost_wire,
    );
    group.finish();

    let mut group = c.benchmark_group(format!("decode_{name}"));
    group.throughput(Throughput::Bytes(fixture.len() as u64));
    group.bench_function("tacky", |b| {
        b.iter(|| black_box(tacky_decode(black_box(fixture))));
    });
    group.bench_function("prost", |b| {
        b.iter(|| black_box(prost_types::FileDescriptorSet::decode(black_box(fixture)).unwrap()));
    });
    assert!(tacky_walk(fixture) != 0, "walker folded nothing");
    group.bench_function("tacky-walk", |b| {
        b.iter(|| black_box(tacky_walk(black_box(fixture))));
    });
    group.finish();
}

fn bench_descriptor_set(c: &mut Criterion) {
    bench_fixture(c, "fds_descriptor_proto", FDS_DESCRIPTOR_PROTO);
    bench_fixture(c, "fds_testing_protos", FDS_TESTING_PROTOS);
    bench_fixture(c, "fds_registry", FDS_REGISTRY);
}

criterion_group!(benches, bench_descriptor_set);
criterion_main!(benches);
