//! Currently wraps/uses pb-rs from quick-protobuf as the underlying parser, as i dont want any protoc system deps (a la prost)
//! and dont i dont to write my own (yet).

use crate::pbrs::types::{Enumerator, FieldType, FileDescriptor, Message, SymbolKind};
use crate::{field_enum::field_enum, field_type::field_type};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use std::io::Write;

pub fn parse_ty(s: &str) -> syn::Type {
    syn::parse_str(s).unwrap_or_else(|_| panic!("failed to parse type: {}", s))
}

/// Create an identifier that handles Rust keywords by using raw identifiers (e.g. `r#type`).
pub fn field_ident(name: &str) -> proc_macro2::Ident {
    syn::parse_str::<syn::Ident>(name)
        .unwrap_or_else(|_| proc_macro2::Ident::new_raw(name, proc_macro2::Span::call_site()))
}

fn read_proto_files(files: &[&str], includes: &[&str]) -> FileDescriptor {
    let roots: Vec<std::path::PathBuf> = files.iter().map(Into::into).collect();
    let search_path: Vec<std::path::PathBuf> = includes.iter().map(Into::into).collect();
    FileDescriptor::read_protos(&roots, &search_path).unwrap()
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
    Message(String),          //name
    SimpleMap(Scalar, Scalar),
    Map(Scalar, Box<PbType>),
}

fn resolve_type(value: FieldType, desc: &FileDescriptor, scope: &Scope) -> PbType {
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
            let kt: PbType = resolve_type(*k, desc, scope);
            let vt: PbType = resolve_type(*v, desc, scope);
            match (kt, vt) {
                (PbType::Scalar(k), PbType::Scalar(v)) => PbType::SimpleMap(k, v),
                (PbType::Scalar(k), v) => PbType::Map(k, Box::new(v)),
                _ => panic!("invalid map structure"),
            }
        }
        FieldType::Message(fqn) => PbType::Message(scope.name(&fqn).to_string()),
        FieldType::Enum(fqn) => {
            let name = scope.name(&fqn).to_string();
            let values = desc
                .find_enum(&fqn)
                .unwrap_or_else(|| panic!("resolved enum {fqn} is missing from the descriptor"))
                .fields
                .iter()
                .map(|(_, v)| *v)
                .collect();
            PbType::Enum((name, values))
        }
        FieldType::Named(name) => {
            unreachable!("type reference {name} survived resolution")
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

fn convert_field(field: &crate::pbrs::types::Field, desc: &FileDescriptor, scope: &Scope) -> Field {
    let crate::pbrs::types::Field {
        name,
        frequency,
        typ,
        number,
        // Presence is checked in the parser's own `sanity_checks`, which rejects custom defaults.
        default: _,
    } = field;
    let ty = resolve_type(typ.clone(), desc, scope);
    // The parser decides packedness per file, from that file's own syntax, so nothing is re-derived
    // here. Doing so from the descriptor's syntax used to pack a proto2 import's repeated scalars
    // whenever the root file happened to be proto3.
    let label: Label = frequency.map(|f| f.into()).unwrap_or(Label::Plain);

    Field {
        name: name.clone(),
        number: *number,
        ty,
        label,
    }
}
impl From<crate::pbrs::types::Frequency> for Label {
    fn from(value: crate::pbrs::types::Frequency) -> Self {
        match value {
            crate::pbrs::types::Frequency::Optional => Label::Optional,
            crate::pbrs::types::Frequency::Repeated => Label::Repeated,
            crate::pbrs::types::Frequency::Required => Label::Required,
            crate::pbrs::types::Frequency::Packed => Label::Packed,
            crate::pbrs::types::Frequency::Plain => Label::Plain,
        }
    }
}

fn write_message(
    m: &Message,
    qualified_name: &str,
    desc: &FileDescriptor,
    scope: &Scope,
) -> TokenStream {
    // Regular (non-oneof) fields
    let regular_fields: Vec<Field> = m
        .fields
        .iter()
        .map(|f| convert_field(f, desc, scope))
        .collect();

    // Oneof groups
    let oneof_groups: Vec<OneOfGroup> = m
        .oneofs
        .iter()
        .map(|o| OneOfGroup {
            name: o.name.clone(),
            fields: o
                .fields
                .iter()
                .map(|f| convert_field(f, desc, scope))
                .collect(),
        })
        .collect();

    // All fields flattened (for the decode enum)
    let all_fields: Vec<Field> = m
        .all_fields()
        .map(|f| convert_field(f, desc, scope))
        .collect();

    let struct_schema = message_schema(qualified_name, &regular_fields, &oneof_groups);
    let field_enum = field_enum(qualified_name, &all_fields);
    let oneof_impls: Vec<TokenStream> = oneof_groups
        .iter()
        .map(|g| write_oneof(qualified_name, g))
        .collect();

    quote! {
        #struct_schema
        #field_enum
        #(#oneof_impls)*
    }
}

fn write_enum(
    m: &Enumerator,
    qualified_name: &str,
    _desc: &FileDescriptor,
    _scope: &Scope,
) -> TokenStream {
    let name_ident = format_ident!("{qualified_name}");

    let variants = m.fields.iter().map(|(field, _number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! {
             #field_ident
        }
    });

    let from_i32_matches = m.fields.iter().map(|(field, number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! {
            #number => #name_ident::#field_ident
        }
    });

    let into_i32_matches = m.fields.iter().map(|(field, number)| {
        let field_ident = format_ident!("{}", heck::AsUpperCamelCase(field).to_string());
        quote! {
            #name_ident::#field_ident => #number
        }
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
        #[derive(Debug, Copy, Clone)]
        pub struct #name_ident {
            #(#field_defs,)*
            #(#oneof_defs,)*
        }
        impl MessageSchema for #name_ident {}
        impl #name_ident {
            pub fn schema() -> Self {
                <Self as MessageSchema>::schema()
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
                        pub fn #method_name<B: WriteBuf>(self, buf: &mut B, value: impl ProtoEncode<#tacky_ty>) -> Self {
                            let t = const { EncodedTag::new(#number, <#tacky_ty as ProtobufScalar>::WIRE_TYPE) };
                            if B::REVERSE {
                                <#tacky_ty as ProtobufScalar>::write_value(value.as_scalar(), buf);
                                t.write(buf);
                            } else {
                                t.write(buf);
                                <#tacky_ty as ProtobufScalar>::write_value(value.as_scalar(), buf);
                            }
                            Self
                        }
                    }
                }
                PbType::Enum((name, _)) => {
                    let enum_ident = format_ident!("{}", name);
                    quote! {
                        pub fn #method_name<B: WriteBuf>(self, buf: &mut B, value: impl ProtoEncode<PbEnum<#enum_ident>>) -> Self {
                            let t = const { EncodedTag::new(#number, WireType::VARINT) };
                            if B::REVERSE {
                                <PbEnum<#enum_ident> as ProtobufScalar>::write_value(value.as_scalar(), buf);
                                t.write(buf);
                            } else {
                                t.write(buf);
                                <PbEnum<#enum_ident> as ProtobufScalar>::write_value(value.as_scalar(), buf);
                            }
                            Self
                        }
                    }
                }
                PbType::Message(msg) => {
                    let msg_ident = parse_ty(msg);
                    let method_name = format_ident!("write_{}_msg", f.name);
                    quote! {
                        pub fn #method_name<B: WriteBuf>(self, buf: &mut B, mut f: impl FnMut(&mut B, #msg_ident)) -> Self {
                            let t = const { EncodedTag::new(#number, WireType::LEN) };
                            buf.put_msg(t, |buf| f(buf, #msg_ident::schema()));
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

/// The names a single package's generated module uses to refer to types, and the imports that
/// bring the ones from other packages into scope.
///
/// Definitions always spell a type as a bare identifier, which is what lets the generated bodies
/// read like hand-written Rust — a cross-package reference is an ordinary `use` at the top of the
/// module, exactly as you would write it yourself, rather than a path threaded through every field.
#[derive(Default)]
struct Scope {
    /// Fully-qualified proto name -> the identifier this package's definitions should spell it as.
    local: HashMap<String, String>,
    /// Module path -> the items to take from it, one `use` per module rather than per type.
    imports: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

impl Scope {
    fn name(&self, fqn: &str) -> &str {
        self.local
            .get(fqn)
            .unwrap_or_else(|| panic!("{fqn} is not in scope for this package"))
    }

    fn import_statements(&self) -> Vec<TokenStream> {
        self.imports
            .iter()
            .map(|(path, items)| {
                let items = items.iter().cloned().collect::<Vec<_>>().join(", ");
                let statement = if items.contains(", ") {
                    format!("use {path}{{{items}}};")
                } else {
                    format!("use {path}{items};")
                };
                let statement: syn::ItemUse = syn::parse_str(&statement)
                    .unwrap_or_else(|e| panic!("bad import `{statement}`: {e}"));
                quote!(#statement)
            })
            .collect()
    }
}

/// Collect the fully-qualified names a field type refers to, looking through maps.
fn referenced_types(typ: &FieldType, out: &mut Vec<String>) {
    match typ {
        FieldType::Message(fqn) | FieldType::Enum(fqn) => out.push(fqn.clone()),
        FieldType::Map(k, v) => {
            referenced_types(k, out);
            referenced_types(v, out);
        }
        _ => {}
    }
}

/// The module path to reach package `to` from inside package `from`: one `super` per level up to
/// their common ancestor, then down. Sibling packages come out as `super::super::common::v1`.
fn relative_path(from: &str, to: &str) -> String {
    let segments = |p: &str| -> Vec<String> {
        p.split('.')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    };
    let (from, to) = (segments(from), segments(to));
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();

    let mut path = String::new();
    for _ in shared..from.len() {
        path.push_str("super::");
    }
    for segment in &to[shared..] {
        path.push_str(segment);
        path.push_str("::");
    }
    path
}

/// Work out what each package's module must import, and under what identifier every type it touches
/// should be spelled.
///
/// A type declared in the same package is spelled with its own flattened name. One from elsewhere is
/// imported under that name too, unless the package already has something by that name — then it is
/// imported under an alias carrying its package, which is the only case where a generated name is
/// not simply what the `.proto` called it.
fn build_scopes(desc: &FileDescriptor) -> HashMap<String, Scope> {
    let mut scopes: HashMap<String, Scope> = HashMap::new();

    // Own declarations first, so an import can tell whether its short name is already taken.
    for (fqn, symbol) in &desc.symbols {
        let scope = scopes.entry(symbol.file_package.clone()).or_default();
        if let Some(previous) = scope
            .local
            .iter()
            .find(|(_, name)| **name == symbol.rust_name)
        {
            panic!(
                "{} and {fqn} both generate the Rust name `{}`; rename one of them, as their \
                 flattened names are indistinguishable",
                previous.0, symbol.rust_name
            );
        }
        scope.local.insert(fqn.clone(), symbol.rust_name.clone());
    }

    // Then the cross-package references, sorted so the emitted imports are stable run to run.
    let mut wanted: Vec<(String, String)> = Vec::new();
    for m in &desc.messages {
        let package = &desc.symbols[&m.full_name].file_package;
        let mut refs = Vec::new();
        for f in m.all_fields() {
            referenced_types(&f.typ, &mut refs);
        }
        for fqn in refs {
            if desc.symbols[&fqn].file_package != *package {
                wanted.push((package.clone(), fqn));
            }
        }
    }
    wanted.sort();
    wanted.dedup();

    for (package, fqn) in wanted {
        let symbol = &desc.symbols[&fqn];
        let scope = scopes.entry(package.clone()).or_default();
        if scope.local.contains_key(&fqn) {
            continue;
        }
        let taken = scope.local.values().any(|name| *name == symbol.rust_name);
        let local = if taken {
            // Two packages calling something the same is legal; only one can keep the short name.
            format!(
                "{}{}",
                symbol
                    .file_package
                    .split('.')
                    .filter(|s| !s.is_empty())
                    .map(|s| heck::AsUpperCamelCase(s).to_string())
                    .collect::<String>(),
                symbol.rust_name
            )
        } else {
            symbol.rust_name.clone()
        };

        // A message is referred to by two names: the schema struct, and the `Fields` decoder the
        // generated decode enum holds. Both have to come along.
        let path = relative_path(&package, &symbol.file_package);
        let (from, to) = (&symbol.rust_name, &local);
        let items = scope.imports.entry(path).or_default();
        let spec = |from: &str, to: &str| {
            if from == to {
                from.to_string()
            } else {
                format!("{from} as {to}")
            }
        };
        items.insert(spec(from, to));
        if symbol.kind == SymbolKind::Message {
            items.insert(spec(&format!("{from}Fields"), &format!("{to}Fields")));
        }
        scope.local.insert(fqn, local);
    }

    scopes
}

/// The generated module tree: one node per package segment, carrying that package's imports and
/// definitions.
#[derive(Default)]
struct ModTree {
    children: std::collections::BTreeMap<String, ModTree>,
    imports: Vec<TokenStream>,
    defs: Vec<TokenStream>,
}

impl ModTree {
    fn at(&mut self, package: &str) -> &mut ModTree {
        let mut node = self;
        for segment in package.split('.').filter(|s| !s.is_empty()) {
            node = node.children.entry(segment.to_string()).or_default();
        }
        node
    }

    /// `top` marks the file root, whose children carry the lint attribute: `allow` is inherited by
    /// nested items, so repeating it on every module would only add noise.
    fn emit(&self, top: bool) -> TokenStream {
        let tacky = (!self.defs.is_empty()).then(|| {
            quote!(
                use ::tacky::*;
            )
        });
        let imports = &self.imports;
        let defs = &self.defs;
        let children = self.children.iter().map(|(name, child)| {
            let mod_name = format_ident!("{name}");
            let inner = child.emit(false);
            let allow = top.then(|| quote!(#[allow(unused, dead_code)]));
            quote! {
                #allow
                pub mod #mod_name { #inner }
            }
        });
        quote! {
            #tacky
            #(#imports)*
            #(#defs)*
            #(#children)*
        }
    }
}

/// Generate the schema structs and field enums for one `.proto`, resolving its `import` paths
/// against the directory the file itself sits in — the equivalent of
/// `protoc -I<dir> <dir>/file.proto`. Use [`write_proto_with_includes`] for a tree whose imports
/// are rooted somewhere else.
pub fn write_proto(file: &str, output: &str) {
    let dir = std::path::Path::new(file)
        .parent()
        .and_then(std::path::Path::to_str)
        .filter(|dir| !dir.is_empty())
        .unwrap_or(".");
    write_proto_with_includes(file, output, &[dir])
}

/// [`write_proto`] with explicit import roots, which behave as `protoc -I` does: an `import` path
/// is joined onto each root in turn, and a relative root is taken against the working directory —
/// for a `build.rs`, the package directory. Nothing is inferred from the importing file's location.
pub fn write_proto_with_includes(file: &str, output: &str, includes: &[&str]) {
    write_protos(&[file], output, includes)
}

/// [`write_proto_with_includes`] over several `.proto` files at once, emitting one module tree
/// holding everything reachable from any of them — as `protoc a.proto b.proto` does.
///
/// Use this when the types you want are not all reachable from a single file. OpenTelemetry's logs
/// and traces service definitions are the standard example: they are siblings, neither imports the
/// other, and they share `common` and `resource`. Generating them one at a time gives two unrelated
/// Rust types for every shared message, so nothing that touches a `KeyValue` can be used with both.
///
/// A file reached from more than one of `files` is emitted once.
pub fn write_protos(files: &[&str], output: &str, includes: &[&str]) {
    let test_file = read_proto_files(files, includes);

    // Each proto package becomes a Rust module holding its own definitions, so a type is named
    // exactly what the `.proto` called it. `read_proto` returns both lists flattened, in declaration
    // order, and the traversal order here is what fixes the order of declarations in the output.
    let scopes = build_scopes(&test_file);
    let mut tree = ModTree::default();

    for (package, scope) in &scopes {
        tree.at(package).imports = scope.import_statements();
    }
    for m in &test_file.messages {
        let package = &test_file.symbols[&m.full_name].file_package;
        let def = write_message(m, &m.rust_name, &test_file, &scopes[package]);
        tree.at(package).defs.push(def);
    }
    for e in &test_file.enums {
        let package = &test_file.symbols[&e.full_name].file_package;
        let def = write_enum(e, &e.rust_name, &test_file, &scopes[package]);
        tree.at(package).defs.push(def);
    }

    let token_stream = tree.emit(true);

    // eprintln!("GENERATED CODE:\n{}", token_stream.to_string());

    let syntax_tree = syn::parse2(token_stream).unwrap();
    let formatted = prettyplease::unparse(&syntax_tree);

    let mut file = std::fs::File::create(output).unwrap();
    file.write_all(formatted.as_bytes()).unwrap();
}

#[cfg(test)]
mod test {
    /// Two messages whose flattened names collide must fail the build with a message naming both,
    /// rather than emitting two conflicting definitions and letting rustc complain about generated
    /// code. `Outer.Inner` and a top-level `OuterInner` are indistinguishable once flattened.
    #[test]
    #[should_panic(expected = "both generate the Rust name `OuterInner`")]
    fn colliding_flattened_names_fail_the_build() {
        let dir = std::env::temp_dir().join("tacky_build_test_collision");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let proto = dir.join("collide.proto");
        std::fs::write(
            &proto,
            r#"syntax = "proto3";
            package example;
            message Outer {
                message Inner { int32 x = 1; }
            }
            message OuterInner { int32 y = 1; }
            "#,
        )
        .unwrap();

        super::write_proto(
            proto.to_str().unwrap(),
            dir.join("out.rs").to_str().unwrap(),
        );
    }
}
