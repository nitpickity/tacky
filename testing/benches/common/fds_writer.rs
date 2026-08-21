//! The `FileDescriptorSet` writer, shared by `benches/descriptor_set.rs` (which measures
//! it against prost and C++) and `benches/comparison.rs` (which needs it for the rotating
//! group, where three unlike message types take turns).
//!
//! Extracted so the two benches cannot drift: a writer that covers a field in one place
//! and not the other would silently change what the numbers mean. It also owns the
//! generated tacky module, so both callers see the same `td` types.
//!
//! Lives in a subdirectory so cargo does not pick it up as a bench target of its own; each
//! bench pulls it in with `#[path = "common/fds_writer.rs"] mod fds_writer;`.
//!
//! Every writer emits fields in ascending tag order, which is the order prost and protoc
//! emit.

#[allow(dead_code)]
pub mod tacky_descriptor {
    include!(concat!(env!("OUT_DIR"), "/tacky_descriptor.rs"));
}
pub use tacky_descriptor::google::protobuf as td;

pub fn tacky_encode<B: tacky::WriteBuf>(
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
    // Only the `registry` fixture has this, and it is most of that fixture: one
    // `Location` per syntactic element, each a pair of short packed int32 arrays plus
    // whatever comments the author wrote.
    if let Some(sci) = &f.source_code_info {
        s.source_code_info.write_msg(buf, |buf, t| {
            t.location.write_msgs(buf, &sci.location, |buf, t, l| {
                t.path.write(buf, &l.path);
                t.span.write(buf, &l.span);
                t.leading_comments.write(buf, l.leading_comments.as_deref());
                t.trailing_comments
                    .write(buf, l.trailing_comments.as_deref());
                t.leading_detached_comments
                    .write(buf, &l.leading_detached_comments);
            });
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
