//! Wraps pb-rs as the underlying protobuf parser. No protoc system dependency needed.

use crate::{field_enum::field_enum, field_type::field_type};
use pb_rs::types::{EnumIndex, Enumerator, FieldType, FileDescriptor, Message, MessageIndex};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use std::io::Write;

pub fn parse_ty(s: &str) -> syn::Type {
    syn::parse_str(s).unwrap_or_else(|_| panic!("failed to parse type: {}", s))
}

/// Create an identifier that handles Rust keywords by using raw identifiers (e.g. `r#type`).
pub fn field_ident(name: &str) -> proc_macro2::Ident {
    syn::parse_str::<syn::Ident>(name)
        .unwrap_or_else(|_| proc_macro2::Ident::new_raw(name, proc_macro2::Span::call_site()))
}

/// Parse a potentially path-containing name (e.g. `"outer::Inner"`) into tokens.
/// Simple names produce an ident, paths produce a qualified path.
pub fn name_tokens(name: &str) -> TokenStream {
    if name.contains("::") {
        let ty: syn::Type = syn::parse_str(name)
            .unwrap_or_else(|_| panic!("failed to parse name as path: {}", name));
        quote!(#ty)
    } else {
        let ident = format_ident!("{}", name);
        quote!(#ident)
    }
}

fn read_proto_file(file: &str, includes: &[&str]) -> FileDescriptor {
    let cfg = pb_rs::ConfigBuilder::new(&[file], includes).unwrap();
    let mut cfgs = cfg.build();
    let cfg = cfgs.pop().unwrap();
    pb_rs::types::FileDescriptor::read_proto(&cfg.in_file, &cfg.import_search_path).unwrap()
}

/// Compute a prost-like Rust path for a message index within a descriptor.
/// Top-level `Foo` → `"Foo"`, nested `Outer.Inner` → `"outer::Inner"`.
fn message_rust_path(idx: &MessageIndex, desc: &FileDescriptor) -> String {
    let mut parts = Vec::new();
    let mut current_messages = &desc.messages;
    let indexes = idx.indexes();
    for (i, &index) in indexes.iter().enumerate() {
        let m = &current_messages[index];
        if i < indexes.len() - 1 {
            parts.push(heck::AsSnakeCase(&m.name).to_string());
        } else {
            parts.push(m.name.clone());
        }
        current_messages = &m.messages;
    }
    parts.join("::")
}

/// Compute a prost-like Rust path for an enum index within a descriptor.
/// Top-level `Status` → `"Status"`, nested inside `Outer` → `"outer::Status"`.
fn enum_rust_path(idx: &EnumIndex, desc: &FileDescriptor) -> String {
    let enum_name = &idx.get_enum(desc).name;
    let parent_indexes = idx.msg_indexes();
    if parent_indexes.is_empty() {
        enum_name.clone()
    } else {
        let mut parts = Vec::new();
        let mut current_messages = &desc.messages;
        for &index in parent_indexes {
            let m = &current_messages[index];
            parts.push(heck::AsSnakeCase(&m.name).to_string());
            current_messages = &m.messages;
        }
        parts.push(enum_name.clone());
        parts.join("::")
    }
}

/// Given a type's package and the current (owning) package, compute the Rust
/// module prefix needed to reach the type.  When packages match, returns "".
/// When they differ, returns "super::<target_pkg_module_path>::".
fn cross_package_prefix(type_package: &str, current_package: &str) -> String {
    if type_package == current_package || type_package.is_empty() {
        return String::new();
    }
    // The target type lives in a sibling package module.  From inside
    // `mod current_pkg { ... }`, we reach sibling `mod target_pkg` via
    // `super::target_pkg`.  For dotted packages like "perftools.profiles",
    // the module nesting is `mod perftools { mod profiles { ... } }`, so
    // the prefix becomes `super::perftools::profiles::`.
    let parts: Vec<&str> = type_package.split('.').collect();
    // How many `super::` do we need?  One to escape the current package's
    // innermost module, then one more for each additional nesting level.
    let current_depth = if current_package.is_empty() {
        0
    } else {
        current_package.split('.').count()
    };
    let mut prefix = String::new();
    for _ in 0..current_depth {
        prefix.push_str("super::");
    }
    for part in &parts {
        prefix.push_str(part);
        prefix.push_str("::");
    }
    prefix
}

#[derive(Debug)]
pub enum Scalar {
    Int32,
    Sint32,
    Int64,
    Sint64,
    Uint32,
    Uint64,
    Bool,
    Fixed32,
    Sfixed32,
    Float,
    Fixed64,
    Sfixed64,
    Double,
    String,
    Bytes,
}

impl Scalar {
    pub const fn as_str(&self) -> &str {
        match self {
            Scalar::Int32 => "int32",
            Scalar::Sint32 => "sint32",
            Scalar::Int64 => "int64",
            Scalar::Sint64 => "sint64",
            Scalar::Uint32 => "uint32",
            Scalar::Uint64 => "uint64",
            Scalar::Bool => "bool",
            Scalar::Fixed32 => "fixed32",
            Scalar::Sfixed32 => "sfixed32",
            Scalar::Float => "float",
            Scalar::Fixed64 => "fixed64",
            Scalar::Sfixed64 => "sfixed64",
            Scalar::Double => "double",
            Scalar::String => "string",
            Scalar::Bytes => "bytes",
        }
    }

    pub const fn tacky_type(&self) -> &str {
        match self {
            Scalar::Int32 => "Int32",
            Scalar::Sint32 => "Sint32",
            Scalar::Int64 => "Int64",
            Scalar::Sint64 => "Sint64",
            Scalar::Uint32 => "Uint32",
            Scalar::Uint64 => "Uint64",
            Scalar::Bool => "Bool",
            Scalar::Fixed32 => "Fixed32",
            Scalar::Sfixed32 => "Sfixed32",
            Scalar::Float => "Float",
            Scalar::Fixed64 => "Fixed64",
            Scalar::Sfixed64 => "Sfixed64",
            Scalar::Double => "Double",
            Scalar::String => "PbString",
            Scalar::Bytes => "PbBytes",
        }
    }
}
impl std::fmt::Display for Scalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
#[derive(Debug)]
pub enum PbType {
    Scalar(Scalar),
    Enum((String, Vec<i32>)), // name and allowed values
    Message(String),          // name (may contain :: for nested/cross-pkg types)
    SimpleMap(Scalar, Scalar),
    Map(Scalar, Box<PbType>),
}

/// Resolve a pb-rs `FieldType` to a `PbType`, with cross-package awareness.
/// `current_package` is the file-level package of the message that owns this field.
fn resolve_type(value: FieldType, desc: &FileDescriptor, current_package: &str) -> PbType {
    match value {
        FieldType::Int32 => PbType::Scalar(Scalar::Int32),
        FieldType::Int64 => PbType::Scalar(Scalar::Int64),
        FieldType::Uint32 => PbType::Scalar(Scalar::Uint32),
        FieldType::Uint64 => PbType::Scalar(Scalar::Uint64),
        FieldType::Sint32 => PbType::Scalar(Scalar::Sint32),
        FieldType::Sint64 => PbType::Scalar(Scalar::Sint64),
        FieldType::Bool => PbType::Scalar(Scalar::Bool),
        FieldType::Fixed64 => PbType::Scalar(Scalar::Fixed64),
        FieldType::Sfixed64 => PbType::Scalar(Scalar::Sfixed64),
        FieldType::Double => PbType::Scalar(Scalar::Double),
        FieldType::String => PbType::Scalar(Scalar::String),
        FieldType::Bytes => PbType::Scalar(Scalar::Bytes),
        FieldType::Fixed32 => PbType::Scalar(Scalar::Fixed32),
        FieldType::Sfixed32 => PbType::Scalar(Scalar::Sfixed32),
        FieldType::Float => PbType::Scalar(Scalar::Float),
        FieldType::Map(k, v) => {
            let kt = resolve_type(*k, desc, current_package);
            let vt = resolve_type(*v, desc, current_package);
            match (kt, vt) {
                (PbType::Scalar(k), PbType::Scalar(v)) => PbType::SimpleMap(k, v),
                (PbType::Scalar(k), v) => PbType::Map(k, Box::new(v)),
                _ => panic!("invalid map structure"),
            }
        }
        FieldType::Message(m) => {
            let target_msg = m.get_message(desc);
            let prefix = if target_msg.imported {
                // Imported type — find its file-level package from its top-level
                // ancestor's package field (top-level imported messages have
                // package = their original file's package).
                let top_msg = desc.messages.get(m.indexes()[0]).unwrap();
                cross_package_prefix(&top_msg.package, current_package)
            } else {
                String::new()
            };
            let local_path = message_rust_path(&m, desc);
            PbType::Message(format!("{}{}", prefix, local_path))
        }
        FieldType::Enum(e) => {
            let target_enum = e.get_enum(desc);
            let prefix = if target_enum.imported {
                let parent_indexes = e.msg_indexes();
                let target_file_pkg = if parent_indexes.is_empty() {
                    target_enum.package.clone()
                } else {
                    desc.messages[parent_indexes[0]].package.clone()
                };
                cross_package_prefix(&target_file_pkg, current_package)
            } else {
                String::new()
            };
            let local_path = enum_rust_path(&e, desc);
            let name = format!("{}{}", prefix, local_path);
            let values = target_enum.fields.iter().map(|(_, v)| *v).collect();
            PbType::Enum((name, values))
        }
        FieldType::MessageOrEnum(_) => unreachable!(),
    }
}

impl PbType {
    /// Whether this type is a scalar that supports packed encoding (everything except string/bytes).
    pub fn is_packable_scalar(&self) -> bool {
        match self {
            PbType::Scalar(Scalar::String) | PbType::Scalar(Scalar::Bytes) => false,
            PbType::Scalar(_) | PbType::Enum(_) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for PbType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PbType::Scalar(s) => f.write_str(s.as_str()),
            PbType::Enum(e) => f.write_str(&e.0),
            PbType::Message(m) => f.write_str(m),
            PbType::Map(k, v) => write!(f, "map<{},{}>", k.as_str(), v),
            PbType::SimpleMap(k, v) => write!(f, "map<{},{}>", k.as_str(), v.as_str()),
        }
    }
}

pub enum Label {
    Required, //proto2 required fields, N/A to proto3
    Optional, //Proto2 and Proto3 fields with "optional" label
    Repeated, //"repeated" fields in both proto2/3
    Packed,   // packable fields with packed = true in proto2 or by default in proto3
    Plain,    //field with no label in proto3, not written to the wire if equals to default
}

pub struct Field {
    pub name: String,
    pub number: i32,
    pub ty: PbType,
    pub label: Label,
}

pub struct OneOfGroup {
    pub name: String,
    pub fields: Vec<Field>,
}

fn convert_field(
    field: &pb_rs::types::Field,
    desc: &FileDescriptor,
    current_package: &str,
) -> Field {
    let pb_rs::types::Field {
        name,
        frequency,
        typ,
        number,
        ..
    } = field;
    let ty = resolve_type(typ.clone(), desc, current_package);
    let mut label: Label = frequency.map(|f| f.into()).unwrap_or(Label::Plain);

    // pb-rs's scan_syntax fails on files with leading comments, misdetecting
    // proto3 as proto2. This causes repeated scalar fields to get Repeated
    // instead of Packed. Fix it here using the correctly-parsed syntax.
    if matches!(desc.syntax, pb_rs::types::Syntax::Proto3)
        && matches!(label, Label::Repeated)
        && ty.is_packable_scalar()
    {
        label = Label::Packed;
    }

    Field {
        name: name.clone(),
        number: *number,
        ty,
        label,
    }
}
impl From<pb_rs::types::Frequency> for Label {
    fn from(value: pb_rs::types::Frequency) -> Self {
        match value {
            pb_rs::types::Frequency::Optional => Label::Optional,
            pb_rs::types::Frequency::Repeated => Label::Repeated,
            pb_rs::types::Frequency::Required => Label::Required,
            pb_rs::types::Frequency::Packed => Label::Packed,
            pb_rs::types::Frequency::Plain => Label::Plain,
        }
    }
}

fn write_message(
    m: &Message,
    name: &str,
    desc: &FileDescriptor,
    current_package: &str,
) -> TokenStream {
    let regular_fields: Vec<Field> = m
        .fields
        .iter()
        .map(|f| convert_field(f, desc, current_package))
        .collect();

    let oneof_groups: Vec<OneOfGroup> = m
        .oneofs
        .iter()
        .map(|o| OneOfGroup {
            name: o.name.clone(),
            fields: o
                .fields
                .iter()
                .map(|f| convert_field(f, desc, current_package))
                .collect(),
        })
        .collect();

    let all_fields: Vec<Field> = m
        .all_fields()
        .map(|f| convert_field(f, desc, current_package))
        .collect();

    let struct_schema = message_schema(name, &regular_fields, &oneof_groups);
    let field_enum = field_enum(name, &all_fields);
    let oneof_impls: Vec<TokenStream> = oneof_groups.iter().map(|g| write_oneof(name, g)).collect();

    quote! {
        #struct_schema
        #field_enum
        #(#oneof_impls)*
    }
}

fn write_enum(m: &Enumerator, name: &str) -> TokenStream {
    let name_ident = format_ident!("{name}");

    let variants = m.fields.iter().map(|(field, _number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! { #field_ident }
    });

    let from_i32_matches = m.fields.iter().map(|(field, number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! { #number => #name_ident::#field_ident }
    });

    let into_i32_matches = m.fields.iter().map(|(field, number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! { #name_ident::#field_ident => #number }
    });

    quote! {
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
        pub enum #name_ident {
            #[default]
            #(#variants,)*
            __Unrecognized(i32),
        }

        impl std::convert::From<i32> for #name_ident {
            fn from(value: i32) -> Self {
                match value {
                    #(#from_i32_matches,)*
                    v => #name_ident::__Unrecognized(v),
                }
            }
        }
        impl std::convert::From<#name_ident> for i32 {
            fn from(value: #name_ident) -> i32 {
                match value {
                    #(#into_i32_matches,)*
                    #name_ident::__Unrecognized(v) => v,
                }
            }
        }
    }
}

/// Recursively generate code for a message and its nested types.
fn write_message_tree(m: &Message, desc: &FileDescriptor, current_package: &str) -> TokenStream {
    let msg_code = write_message(m, &m.name, desc, current_package);

    let nested_msgs: Vec<TokenStream> = m
        .messages
        .iter()
        .map(|n| write_message_tree(n, desc, current_package))
        .collect();

    let nested_enums: Vec<TokenStream> = m.enums.iter().map(|e| write_enum(e, &e.name)).collect();

    if nested_msgs.is_empty() && nested_enums.is_empty() {
        msg_code
    } else {
        let mod_name = field_ident(&heck::AsSnakeCase(&m.name).to_string());
        quote! {
            #msg_code
            pub mod #mod_name {
                use super::*;
                #(#nested_msgs)*
                #(#nested_enums)*
            }
        }
    }
}

fn message_schema(name: &str, fields: &[Field], oneofs: &[OneOfGroup]) -> TokenStream {
    let name_ident = format_ident!("{name}");
    let field_defs = fields.iter().map(field_type);
    let oneof_defs = oneofs.iter().map(|o| {
        let field_name = format_ident!("{}", o.name);
        let marker_name = format_ident!("{}{}", name, heck::AsUpperCamelCase(&o.name).to_string());
        quote!(pub #field_name: #marker_name)
    });
    let k = format_ident!("{name}Fields");
    quote! {
        #[derive(Default, Debug, Copy, Clone)]
        pub struct #name_ident {
            #(#field_defs,)*
            #(#oneof_defs,)*
        }
        impl MessageSchema for #name_ident {}
        impl #name_ident {
            pub fn new() -> Self {
                Self::default()
            }
            pub fn decode(buf: &[u8])-> #k<'_> {
                #k::new(buf)
            }
        }
    }
}

fn write_oneof(msg_name: &str, group: &OneOfGroup) -> TokenStream {
    let marker_name = format_ident!(
        "{}{}",
        msg_name,
        heck::AsUpperCamelCase(&group.name).to_string()
    );

    let write_methods: Vec<TokenStream> = group
        .fields
        .iter()
        .map(|f| {
            let method_name = format_ident!("write_{}", f.name);
            let number = f.number as u32;

            match &f.ty {
                PbType::Scalar(s) => {
                    let tacky_ty = parse_ty(s.tacky_type());
                    quote! {
                        pub fn #method_name(self, buf: &mut Vec<u8>, value: impl ProtoEncode<#tacky_ty>) -> Self {
                            let t = const { EncodedTag::new(#number, <#tacky_ty as ProtobufScalar>::WIRE_TYPE) };
                            t.write(buf);
                            <#tacky_ty as ProtobufScalar>::write_value(value.as_scalar(), buf);
                            Self
                        }
                    }
                }
                PbType::Enum((name, _)) => {
                    let enum_ty = name_tokens(name);
                    quote! {
                        pub fn #method_name(self, buf: &mut Vec<u8>, value: impl ProtoEncode<PbEnum<#enum_ty>>) -> Self {
                            let t = const { EncodedTag::new(#number, WireType::VARINT) };
                            t.write(buf);
                            <PbEnum<#enum_ty> as ProtobufScalar>::write_value(value.as_scalar(), buf);
                            Self
                        }
                    }
                }
                PbType::Message(msg) => {
                    let msg_ty = parse_ty(msg);
                    let method_name = format_ident!("write_{}_msg", f.name);
                    quote! {
                        pub fn #method_name(self, buf: &mut Vec<u8>, mut f: impl FnMut(&mut Vec<u8>, #msg_ty)) -> Self {
                            let t = const { EncodedTag::new(#number, WireType::LEN) };
                            t.write(buf);
                            let t = tack::Tack::new(buf);
                            f(t.buffer, #msg_ty::default());
                            Self
                        }
                    }
                }
                _ => panic!("oneof fields cannot be maps or repeated"),
            }
        })
        .collect();

    quote! {
        #[derive(Default, Debug, Copy, Clone)]
        pub struct #marker_name;

        impl #marker_name {
            #(#write_methods)*
        }
    }
}

/// Generate code for a single package module from its non-imported types.
fn generate_package(package: &str, desc: &FileDescriptor) -> TokenStream {
    let messages: Vec<TokenStream> = desc
        .messages
        .iter()
        .filter(|m| !m.imported)
        .map(|m| write_message_tree(m, desc, package))
        .collect();

    let enums: Vec<TokenStream> = desc
        .enums
        .iter()
        .filter(|e| !e.imported)
        .map(|e| write_enum(e, &e.name))
        .collect();

    let inner = quote! {
        use ::tacky::*;
        #(#messages)*
        #(#enums)*
    };

    inner
}

fn format_and_write(tokens: TokenStream, path: &std::path::Path) {
    let syntax_tree = syn::parse2(tokens).unwrap();
    let formatted = prettyplease::unparse(&syntax_tree);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(formatted.as_bytes()).unwrap();
}

/// Compile multiple proto files into an output directory.
///
/// Produces one `.rs` file per protobuf package (e.g. `example.rs`,
/// `perftools.profiles.rs`) plus a `_includes.rs` that wires them together
/// as sibling `pub mod` blocks.  Cross-package field references use `super::`
/// paths, so the sibling layout is required.
///
/// In your `lib.rs` / `main.rs`:
/// ```ignore
/// mod protos {
///     include!(concat!(env!("OUT_DIR"), "/_includes.rs"));
/// }
/// ```
pub fn compile_protos(files: &[&str], output_dir: &str, includes: &[&str]) {
    std::fs::create_dir_all(output_dir).unwrap();
    let out = std::path::Path::new(output_dir);

    // Compute the relative suffix from OUT_DIR to output_dir, so the generated
    // _includes.rs can use `env!("OUT_DIR")` based paths.
    let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
    let rel_prefix = if out_dir.is_empty() {
        String::new()
    } else {
        let abs_out = std::fs::canonicalize(out).unwrap();
        let abs_base = std::fs::canonicalize(&out_dir).unwrap();
        abs_out
            .strip_prefix(&abs_base)
            .map(|p| format!("/{}", p.display()))
            .unwrap_or_default()
    };

    let mut packages: BTreeMap<String, FileDescriptor> = BTreeMap::new();
    for file in files {
        let desc = read_proto_file(file, includes);
        let pkg = desc.package.clone();
        packages.insert(pkg, desc);
    }

    let mut include_lines: Vec<String> = Vec::new();

    for (package, desc) in &packages {
        let content = generate_package(package, desc);
        let token_stream = quote! { #[allow(unused, dead_code)] #content };

        let file_name = if package.is_empty() {
            "_root.rs".to_string()
        } else {
            format!("{}.rs", package)
        };

        format_and_write(token_stream, &out.join(&file_name));

        let inc_path = format!("{}/{}", rel_prefix, file_name);
        if package.is_empty() {
            include_lines.push(format!(
                "include!(concat!(env!(\"OUT_DIR\"), \"{}\"));",
                inc_path
            ));
        } else {
            let parts: Vec<&str> = package.split('.').collect();
            let mut line = String::new();
            for part in &parts {
                line.push_str(&format!("pub mod {} {{ ", part));
            }
            line.push_str(&format!(
                "include!(concat!(env!(\"OUT_DIR\"), \"{}\")); ",
                inc_path
            ));
            for _ in &parts {
                line.push_str("} ");
            }
            include_lines.push(line);
        }
    }

    let includes_content = include_lines.join("\n");
    std::fs::write(out.join("_includes.rs"), includes_content).unwrap();
}

/// Compile a single proto file to a standalone output file.
pub fn write_proto(file: &str, output: &str) {
    write_proto_with_includes(file, output, &["."])
}

/// Compile a single proto file to a standalone output file with custom include paths.
pub fn write_proto_with_includes(file: &str, output: &str, includes: &[&str]) {
    let desc = read_proto_file(file, includes);
    let package = desc.package.clone();
    let content = generate_package(&package, &desc);

    let mut inner = content;
    if !package.is_empty() {
        for part in package.rsplit('.') {
            let mod_name = format_ident!("{}", part);
            inner = quote! {
                pub mod #mod_name {
                    #inner
                }
            };
        }
    }

    let token_stream = quote! { #[allow(unused, dead_code)] #inner };
    format_and_write(token_stream, std::path::Path::new(output));
}
