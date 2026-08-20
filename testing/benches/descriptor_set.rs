//! `FileDescriptorSet` — the shape every protobuf toolchain actually encodes.
//!
//! A descriptor set is what `protoc --descriptor_set_out` produces and what every
//! gRPC reflection service, schema registry and dynamic-message runtime ships
//! around. It is the opposite of GoogleMessage1: deeply nested (file → message →
//! nested message → field → options), overwhelmingly short strings and small
//! varints, and thousands of *absent* optional fields. That makes it the corpus
//! where tacky's fixed-width length padding costs the most and where prost's
//! `encoded_len` recursion costs the most, so it is worth measuring on its own.
//!
//! Two fixtures, both checked in so `cargo bench` needs no local `protoc`
//! (regenerate with `scripts/gen_bench_fixtures.sh`):
//!
//! - `descriptor_proto` — the vendored `descriptor.proto` describing itself.
//!   Extension ranges, reserved ranges, nested enums, oneofs, custom defaults.
//! - `testing_protos` — this repo's own protos, with imports. Flatter: mostly
//!   names, field numbers and `json_name`s, plus map-entry messages.
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
//! The writer covers every field of the descriptor messages the fixtures contain.
//! It does *not* cover `ServiceDescriptorProto`, `SourceCodeInfo`,
//! `UninterpretedOption`, or the option messages with no set fields here
//! (`EnumOptions`, `EnumValueOptions`, `OneofOptions`, `ExtensionRangeOptions`):
//! none appear in either fixture. That is not a silent gap — the round-trip assert
//! below compares whole messages, so a fixture that grew one of them would fail
//! rather than quietly measure less work.
//!
//! Wire output is checked by decoding tacky's bytes with prost and comparing
//! messages, not by comparing byte strings: tacky pads nested length prefixes to a
//! fixed width, so its output is semantically equal but longer. With this much
//! nesting the delta is the interesting number, and it is printed next to the
//! timings.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prost::Message;

#[cfg(feature = "cpp")]
#[path = "common/cpp_arms.rs"]
mod cpp_arms;

#[allow(dead_code)]
mod tacky_descriptor {
    include!(concat!(env!("OUT_DIR"), "/tacky_descriptor.rs"));
}
use tacky_descriptor::google::protobuf as td;

const FDS_DESCRIPTOR_PROTO: &[u8] = include_bytes!("../data/descriptor_proto.fds");
const FDS_TESTING_PROTOS: &[u8] = include_bytes!("../data/testing_protos.fds");

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------
//
// Every writer emits fields in ascending tag order, which is the order prost and
// protoc emit, so the two outputs differ only where tacky pads a length prefix.

fn tacky_encode<B: tacky::WriteBuf>(
    buf: &mut tacky::AnyDir<B>,
    set: &prost_types::FileDescriptorSet,
) {
    let s = td::FileDescriptorSet::schema();
    s.file
        .write_msgs(buf, &set.file, |buf, _, f| write_file(buf, f));
}

fn write_file<B: tacky::WriteBuf>(
    buf: &mut tacky::AnyDir<B>,
    f: &prost_types::FileDescriptorProto,
) {
    let s = td::FileDescriptorProto::schema();
    s.name.write(buf, f.name.as_deref());
    s.package.write(buf, f.package.as_deref());
    s.dependency.write(buf, &f.dependency);
    s.message_type
        .write_msgs(buf, &f.message_type, |buf, _, m| write_message(buf, m));
    s.enum_type
        .write_msgs(buf, &f.enum_type, |buf, _, e| write_enum(buf, e));
    s.extension
        .write_msgs(buf, &f.extension, |buf, _, x| write_field(buf, x));
    if let Some(o) = &f.options {
        s.options.write_msg(buf, |buf, t| {
            t.java_package.write(buf, o.java_package.as_deref());
            t.java_outer_classname
                .write(buf, o.java_outer_classname.as_deref());
            t.optimize_for
                .write(buf, o.optimize_for.map(td::FileOptionsOptimizeMode::from));
            t.java_multiple_files.write(buf, o.java_multiple_files);
            t.go_package.write(buf, o.go_package.as_deref());
            t.cc_generic_services.write(buf, o.cc_generic_services);
            t.java_generic_services.write(buf, o.java_generic_services);
            t.py_generic_services.write(buf, o.py_generic_services);
            t.deprecated.write(buf, o.deprecated);
            t.java_string_check_utf8
                .write(buf, o.java_string_check_utf8);
            t.cc_enable_arenas.write(buf, o.cc_enable_arenas);
            t.objc_class_prefix
                .write(buf, o.objc_class_prefix.as_deref());
            t.csharp_namespace.write(buf, o.csharp_namespace.as_deref());
            t.swift_prefix.write(buf, o.swift_prefix.as_deref());
            t.php_class_prefix.write(buf, o.php_class_prefix.as_deref());
            t.php_namespace.write(buf, o.php_namespace.as_deref());
            t.php_metadata_namespace
                .write(buf, o.php_metadata_namespace.as_deref());
            t.ruby_package.write(buf, o.ruby_package.as_deref());
        });
    }
    s.public_dependency.write(buf, &f.public_dependency);
    s.weak_dependency.write(buf, &f.weak_dependency);
    s.syntax.write(buf, f.syntax.as_deref());
}

/// Recursive: `nested_type` is a `DescriptorProto` again, which is most of what
/// makes this corpus different from a flat message.
fn write_message<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, m: &prost_types::DescriptorProto) {
    let s = td::DescriptorProto::schema();
    s.name.write(buf, m.name.as_deref());
    s.field
        .write_msgs(buf, &m.field, |buf, _, f| write_field(buf, f));
    s.nested_type
        .write_msgs(buf, &m.nested_type, |buf, _, n| write_message(buf, n));
    s.enum_type
        .write_msgs(buf, &m.enum_type, |buf, _, e| write_enum(buf, e));
    s.extension_range
        .write_msgs(buf, &m.extension_range, |buf, t, r| {
            t.start.write(buf, r.start);
            t.end.write(buf, r.end);
        });
    s.extension
        .write_msgs(buf, &m.extension, |buf, _, x| write_field(buf, x));
    if let Some(o) = &m.options {
        s.options.write_msg(buf, |buf, t| {
            t.message_set_wire_format
                .write(buf, o.message_set_wire_format);
            t.no_standard_descriptor_accessor
                .write(buf, o.no_standard_descriptor_accessor);
            t.deprecated.write(buf, o.deprecated);
            t.map_entry.write(buf, o.map_entry);
        });
    }
    s.oneof_decl.write_msgs(buf, &m.oneof_decl, |buf, t, d| {
        t.name.write(buf, d.name.as_deref());
    });
    s.reserved_range
        .write_msgs(buf, &m.reserved_range, |buf, t, r| {
            t.start.write(buf, r.start);
            t.end.write(buf, r.end);
        });
    s.reserved_name.write(buf, &m.reserved_name);
}

fn write_field<B: tacky::WriteBuf>(
    buf: &mut tacky::AnyDir<B>,
    f: &prost_types::FieldDescriptorProto,
) {
    let s = td::FieldDescriptorProto::schema();
    s.name.write(buf, f.name.as_deref());
    s.extendee.write(buf, f.extendee.as_deref());
    s.number.write(buf, f.number);
    s.label
        .write(buf, f.label.map(td::FieldDescriptorProtoLabel::from));
    s.r#type
        .write(buf, f.r#type.map(td::FieldDescriptorProtoType::from));
    s.type_name.write(buf, f.type_name.as_deref());
    s.default_value.write(buf, f.default_value.as_deref());
    if let Some(o) = &f.options {
        s.options.write_msg(buf, |buf, t| {
            t.ctype.write(buf, o.ctype.map(td::FieldOptionsCType::from));
            t.packed.write(buf, o.packed);
            t.deprecated.write(buf, o.deprecated);
            t.lazy.write(buf, o.lazy);
            t.jstype
                .write(buf, o.jstype.map(td::FieldOptionsJSType::from));
            t.weak.write(buf, o.weak);
        });
    }
    s.oneof_index.write(buf, f.oneof_index);
    s.json_name.write(buf, f.json_name.as_deref());
    s.proto3_optional.write(buf, f.proto3_optional);
}

fn write_enum<B: tacky::WriteBuf>(
    buf: &mut tacky::AnyDir<B>,
    e: &prost_types::EnumDescriptorProto,
) {
    let s = td::EnumDescriptorProto::schema();
    s.name.write(buf, e.name.as_deref());
    s.value.write_msgs(buf, &e.value, |buf, t, v| {
        t.name.write(buf, v.name.as_deref());
        t.number.write(buf, v.number);
    });
    s.reserved_range
        .write_msgs(buf, &e.reserved_range, |buf, t, r| {
            t.start.write(buf, r.start);
            t.end.write(buf, r.end);
        });
    s.reserved_name.write(buf, &e.reserved_name);
}

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
            F::Service(_) | F::SourceCodeInfo(_) => {
                unimplemented!("FileDescriptorProto field absent from both fixtures")
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
            F::Service(_) | F::SourceCodeInfo(_) => {
                unimplemented!("FileDescriptorProto field absent from both fixtures")
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

    // Forward writer into a fixed slice, so `tacky-rev` vs `tacky-slice` isolates the write
    // *direction* from the buffer kind.
    group.bench_function("tacky-slice", |b| {
        let mut backing = vec![0u8; cap + 1024];
        b.iter(|| {
            let mut sb = tacky::SliceBuf::new(&mut backing);
            tacky_encode(tacky::AnyDir::from_mut(&mut sb), &set);
            black_box(sb.written());
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

    // Handing the result over as an owned, index-0 buffer: the reverse output lives at the
    // tail, so a `Vec<u8>`-shaped sink forces one compaction.
    group.bench_function("tacky-rev-owned", |b| {
        let mut backing = vec![0u8; cap + 1024];
        let mut out = Vec::with_capacity(cap + 1024);
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode(tacky::AnyDir::from_mut(&mut rb), &set);
            out.clear();
            out.extend_from_slice(rb.written());
            black_box(out.as_slice());
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
}

criterion_group!(benches, bench_descriptor_set);
criterion_main!(benches);
