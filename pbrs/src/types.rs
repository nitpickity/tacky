use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::errors::{Error, Result};
use crate::parser::file_descriptor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Syntax {
    Proto2,
    Proto3,
    Edition(String),
}

impl Default for Syntax {
    fn default() -> Syntax {
        Syntax::Proto2
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Optional,
    Repeated,
    Packed,
    Required,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Bool,
    Fixed64,
    Sfixed64,
    Double,
    String,
    Bytes,
    Fixed32,
    Sfixed32,
    Float,
    /// A type reference exactly as written in the `.proto`, before resolution. Possibly relative,
    /// possibly leading-dot absolute. [`FileDescriptor::resolve_types`] replaces every one of these
    /// with a [`FieldType::Message`] or [`FieldType::Enum`].
    Named(String),
    /// A message, by fully-qualified proto name (`package.Outer.Inner`).
    Message(String),
    /// An enum, by fully-qualified proto name.
    Enum(String),
    Map(Box<FieldType>, Box<FieldType>),
}

impl FieldType {
    /// Whether the type is a varint/fixed scalar, and so legal to pack. Enums count; messages,
    /// maps, strings and bytes do not.
    pub fn is_primitive(&self) -> bool {
        !matches!(
            *self,
            FieldType::Message(_)
                | FieldType::Named(_)
                | FieldType::Map(_, _)
                | FieldType::String
                | FieldType::Bytes
        )
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub frequency: Option<Frequency>,
    pub typ: FieldType,
    pub number: i32,
    /// Whether the field carries a `[default = …]` option. The value itself is not kept: tacky
    /// rejects custom defaults, so only presence is ever consulted.
    pub default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Message {
    pub name: String,
    pub fields: Vec<Field>,
    pub oneofs: Vec<OneOf>,
    pub reserved_nums: Option<Vec<i32>>,
    pub reserved_names: Option<Vec<String>>,
    /// The enclosing scope: the file's package plus any enclosing messages, dot-separated.
    pub package: String,
    /// Fully-qualified proto name, `package.Outer.Inner`. Set by [`FileDescriptor::flatten`].
    pub full_name: String,
    /// The flattened Rust identifier, `OuterInner`: enclosing message names concatenated, package
    /// dropped. Set by [`FileDescriptor::flatten`].
    pub rust_name: String,
    /// Nested messages, as parsed. Emptied by [`FileDescriptor::flatten`], which lifts them into
    /// the descriptor's own list — do not expect nesting to survive `read_proto`.
    pub messages: Vec<Message>,
    /// Nested enums, as parsed. Emptied by [`FileDescriptor::flatten`], as with `messages`.
    pub enums: Vec<Enumerator>,
}

impl Message {
    fn sanity_checks(&self) -> Result<()> {
        for f in self.all_fields() {
            // check reserved
            if self
                .reserved_names
                .as_ref()
                .map_or(false, |names| names.contains(&f.name))
                || self
                    .reserved_nums
                    .as_ref()
                    .map_or(false, |nums| nums.contains(&f.number))
            {
                return Err(Error::InvalidMessage(format!(
                    "Error in message {}\n\
                     Field {:?} conflict with reserved fields",
                    self.name, f
                )));
            }

            // custom field defaults
            if f.default {
                return Err(Error::InvalidDefaultEnum(format!(
                    "Error in message {}\n custom defaults are not supported in tacky",
                    self.name
                )));
            }
        }
        Ok(())
    }

    /// Stamp the enclosing scope onto this message and its nested items, so that
    /// [`FileDescriptor::flatten`] can build each fully-qualified name from `package` + `name`.
    ///
    /// A file with no package deliberately leaves `self.package` empty while still scoping its
    /// children under `self.name`: type references in such a file resolve against the bare
    /// message name.
    fn set_package(&mut self, package: &str) {
        let child_package = if package.is_empty() {
            self.name.clone()
        } else {
            self.package = package.to_string();
            format!("{}.{}", package, self.name)
        };

        for m in &mut self.messages {
            m.set_package(&child_package);
        }
        for m in &mut self.enums {
            m.set_package(&child_package);
        }
    }

    /// Return an iterator producing references to all the `Field`s of `self`,
    /// including both direct and `oneof` fields.
    pub fn all_fields(&self) -> impl Iterator<Item = &Field> {
        self.fields
            .iter()
            .chain(self.oneofs.iter().flat_map(|o| o.fields.iter()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Enumerator {
    pub name: String,
    pub fields: Vec<(String, i32)>,
    pub package: String,
    /// Fully-qualified proto name. Set by [`FileDescriptor::flatten`].
    pub full_name: String,
    /// The flattened Rust identifier. Set by [`FileDescriptor::flatten`].
    pub rust_name: String,
}

impl Enumerator {
    fn set_package(&mut self, package: &str) {
        self.package = package.to_string();
    }
}

#[derive(Debug, Clone, Default)]
pub struct OneOf {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Message,
    Enum,
}

/// One entry of the descriptor's flat symbol table, keyed by fully-qualified proto name.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: SymbolKind,
    /// The declaring file's package, which the FQN alone cannot be split back into.
    pub package: String,
    /// The flattened Rust identifier this symbol generates as.
    pub rust_name: String,
}

#[derive(Debug, Default, Clone)]
pub struct FileDescriptor {
    pub import_paths: Vec<PathBuf>,
    pub package: String,
    pub syntax: Syntax,
    /// Every message in the file and its transitive imports, flat: nesting is encoded in
    /// `full_name`/`rust_name` rather than in the structure.
    pub messages: Vec<Message>,
    /// Every enum, flat, as with `messages`.
    pub enums: Vec<Enumerator>,
    /// Messages and enums by fully-qualified proto name.
    pub symbols: HashMap<String, Symbol>,
}

impl FileDescriptor {
    /// Parse `in_file`, inline its transitive imports, and resolve every type reference.
    ///
    /// Runs in four passes: parse each file to a tree of nested messages; merge the imports into
    /// one descriptor; flatten that into `messages`/`enums`/`symbols`; resolve the type references
    /// against the merged symbol table. Resolution is last and happens once, over everything, so
    /// no name ever has to be resolved against a scope it will later be moved out of.
    pub fn read_proto(in_file: &Path, import_search_path: &[PathBuf]) -> Result<FileDescriptor> {
        let mut desc = Self::parse_tree(in_file, import_search_path)?;
        desc.flatten();
        desc.resolve_types()?;
        desc.sanity_checks()?;
        Ok(desc)
    }

    /// Parse one file and merge its imports, leaving messages nested and types unresolved. Called
    /// recursively for each import, so it must not resolve: a name is only resolvable once every
    /// file has been merged.
    fn parse_tree(in_file: &Path, import_search_path: &[PathBuf]) -> Result<FileDescriptor> {
        let file = std::fs::read_to_string(in_file)?;
        let (rem, mut desc) = file_descriptor(&file).map_err(Error::Nom)?;
        let rem = rem.trim();
        if !rem.is_empty() {
            return Err(Error::TrailingGarbage(rem.chars().take(50).collect()));
        }
        desc.fetch_imports(in_file, import_search_path)?;
        Ok(desc)
    }

    fn sanity_checks(&self) -> Result<()> {
        for m in &self.messages {
            m.sanity_checks()?;
        }
        Ok(())
    }

    /// Get messages and enums from imports
    fn fetch_imports(&mut self, in_file: &Path, import_search_path: &[PathBuf]) -> Result<()> {
        for m in &mut self.messages {
            m.set_package(&self.package);
        }
        for m in &mut self.enums {
            m.set_package(&self.package);
        }

        for import in &self.import_paths {
            // this is the same logic as the C preprocessor;
            // if the include path item is absolute, then append the filename,
            // otherwise it is always relative to the file.
            let mut matching_file = None;
            for path in import_search_path {
                let candidate = if path.is_absolute() {
                    path.join(import)
                } else {
                    in_file
                        .parent()
                        .map_or_else(|| path.join(import), |p| p.join(path).join(import))
                };
                if candidate.exists() {
                    matching_file = Some(candidate);
                    break;
                }
            }
            if matching_file.is_none() {
                return Err(Error::InvalidImport(format!(
                    "file {} not found on import path",
                    import.display()
                )));
            }
            let proto_file = matching_file.unwrap();
            let mut f = FileDescriptor::parse_tree(&proto_file, import_search_path)?;

            // if the proto has a packge then the names will be prefixed
            let package = f.package.clone();
            // A diamond import (a imports b and c, and b also imports c) reaches the
            // same file twice, so merge by fully-qualified name. Protobuf guarantees
            // those are unique, which makes a repeat the same definition and dropping
            // it lossless.
            let messages: Vec<Message> = f
                .messages
                .drain(..)
                .map(|mut m| {
                    if m.package.is_empty() {
                        m.set_package(&package);
                    }
                    m
                })
                .collect();
            for m in messages {
                if !self
                    .messages
                    .iter()
                    .any(|e| e.package == m.package && e.name == m.name)
                {
                    self.messages.push(m);
                }
            }
            let enums: Vec<Enumerator> = f
                .enums
                .drain(..)
                .map(|mut e| {
                    if e.package.is_empty() {
                        e.set_package(&package);
                    }
                    e
                })
                .collect();
            for e in enums {
                if !self
                    .enums
                    .iter()
                    .any(|x| x.package == e.package && x.name == e.name)
                {
                    self.enums.push(e);
                }
            }
        }
        Ok(())
    }

    /// Lift every nested message and enum into the descriptor's own lists, stamping each with its
    /// fully-qualified proto name and its flattened Rust identifier, and index the result by FQN.
    ///
    /// Messages come out in pre-order depth first; enums file-level first, then per message in the
    /// same order. Generated code follows these lists, so the traversal is what fixes the order of
    /// declarations in the output.
    fn flatten(&mut self) {
        fn qualify(package: &str, name: &str) -> String {
            if package.is_empty() {
                name.to_string()
            } else {
                format!("{package}.{name}")
            }
        }

        fn walk(
            mut m: Message,
            prefix: &str,
            messages: &mut Vec<Message>,
            enums: &mut Vec<Enumerator>,
        ) {
            let rust_name = format!("{prefix}{}", m.name);
            m.full_name = qualify(&m.package, &m.name);
            m.rust_name = rust_name.clone();
            let nested_messages: Vec<Message> = m.messages.drain(..).collect();
            let nested_enums: Vec<Enumerator> = m.enums.drain(..).collect();
            messages.push(m);

            for mut e in nested_enums {
                e.full_name = qualify(&e.package, &e.name);
                e.rust_name = format!("{rust_name}{}", e.name);
                enums.push(e);
            }
            for nested in nested_messages {
                walk(nested, &rust_name, messages, enums);
            }
        }

        let mut messages = Vec::new();
        let mut enums = Vec::new();

        for mut e in self.enums.drain(..) {
            e.full_name = qualify(&e.package, &e.name);
            e.rust_name = e.name.clone();
            enums.push(e);
        }
        for m in self.messages.drain(..) {
            walk(m, "", &mut messages, &mut enums);
        }

        // First definition wins on a duplicate FQN, matching the merge in `fetch_imports`.
        for m in &messages {
            self.symbols
                .entry(m.full_name.clone())
                .or_insert_with(|| Symbol {
                    kind: SymbolKind::Message,
                    package: m.package.clone(),
                    rust_name: m.rust_name.clone(),
                });
        }
        for e in &enums {
            self.symbols
                .entry(e.full_name.clone())
                .or_insert_with(|| Symbol {
                    kind: SymbolKind::Enum,
                    package: e.package.clone(),
                    rust_name: e.rust_name.clone(),
                });
        }

        self.messages = messages;
        self.enums = enums;
    }

    /// Replace every [`FieldType::Named`] with the message or enum it refers to, and downgrade a
    /// `Packed` frequency to `Repeated` for the types that cannot be packed.
    fn resolve_types(&mut self) -> Result<()> {
        let Self {
            messages, symbols, ..
        } = self;
        for m in messages.iter_mut() {
            let scope = m.full_name.clone();
            for typ in m
                .fields
                .iter_mut()
                .chain(m.oneofs.iter_mut().flat_map(|o| o.fields.iter_mut()))
                .map(|f| &mut f.typ)
                .flat_map(|typ| match *typ {
                    FieldType::Map(ref mut key, ref mut value) => {
                        vec![&mut **key, &mut **value].into_iter()
                    }
                    _ => vec![typ].into_iter(),
                })
            {
                if let FieldType::Named(name) = typ {
                    *typ = resolve_named(name, &scope, symbols)?;
                }
            }

            // Enums are primitives, so they stay Packed; messages do not.
            for f in m
                .fields
                .iter_mut()
                .chain(m.oneofs.iter_mut().flat_map(|o| o.fields.iter_mut()))
            {
                if f.frequency == Some(Frequency::Packed) && !f.typ.is_primitive() {
                    f.frequency = Some(Frequency::Repeated);
                }
            }
        }
        Ok(())
    }

    /// The flattened Rust identifier for a fully-qualified proto name.
    ///
    /// # Panics
    ///
    /// If the name is not in the symbol table. Every name reachable from a resolved `FieldType` is,
    /// by construction, so a panic here means the descriptor was built by hand.
    pub fn rust_name(&self, full_name: &str) -> &str {
        &self
            .symbols
            .get(full_name)
            .unwrap_or_else(|| panic!("{full_name} is not in the symbol table"))
            .rust_name
    }

    /// The enum with this fully-qualified proto name.
    pub fn find_enum(&self, full_name: &str) -> Option<&Enumerator> {
        self.enums.iter().find(|e| e.full_name == full_name)
    }
}

/// Resolve one type reference the way protobuf specifies: innermost scope outwards.
///
/// A leading `.` means the reference is already absolute. Otherwise try `scope.name`, then strip
/// one trailing segment off the scope and retry, and finally the bare name — so a reference inside
/// `a.b.Outer` finds `a.b.Outer.Name`, `a.b.Name`, `a.Name` or `Name`, in that order.
fn resolve_named(name: &str, scope: &str, symbols: &HashMap<String, Symbol>) -> Result<FieldType> {
    let candidates: Vec<String> = if let Some(absolute) = name.strip_prefix('.') {
        vec![absolute.to_string()]
    } else {
        let mut candidates = Vec::new();
        let mut scope = scope;
        while !scope.is_empty() {
            candidates.push(format!("{scope}.{name}"));
            match scope.rfind('.') {
                Some(i) => scope = &scope[..i],
                None => break,
            }
        }
        candidates.push(name.to_string());
        candidates
    };

    for candidate in &candidates {
        if let Some(symbol) = symbols.get(candidate) {
            return Ok(match symbol.kind {
                SymbolKind::Message => FieldType::Message(candidate.clone()),
                SymbolKind::Enum => FieldType::Enum(candidate.clone()),
            });
        }
    }
    Err(Error::MessageOrEnumNotFound(name.to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    /// Write `files` into a scratch directory of its own and return the directory. Resolution and
    /// imports can only be exercised through the filesystem, since `read_proto` reads imports off
    /// the include path.
    fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pbrs_test_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, contents) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        dir
    }

    fn field_type(desc: &FileDescriptor, message: &str, field: &str) -> FieldType {
        let m = desc
            .messages
            .iter()
            .find(|m| m.full_name == message)
            .unwrap_or_else(|| panic!("no message {message}"));
        m.fields
            .iter()
            .find(|f| f.name == field)
            .unwrap_or_else(|| panic!("no field {field} on {message}"))
            .typ
            .clone()
    }

    /// A relative reference resolves innermost-scope-first: `Dup` inside `a.b.Outer` must find the
    /// nested `a.b.Outer.Dup`, not the top-level `a.b.Dup` that shares its name.
    #[test]
    fn resolves_innermost_scope_first() {
        let dir = scratch(
            "innermost",
            &[(
                "root.proto",
                r#"syntax = "proto3";
                package a.b;
                message Dup { int32 outer_one = 1; }
                message Outer {
                    message Dup { int32 inner_one = 1; }
                    Dup shadowed = 1;
                }
                "#,
            )],
        );
        let desc = FileDescriptor::read_proto(&dir.join("root.proto"), &[dir.clone()]).unwrap();

        assert_eq!(
            FieldType::Message("a.b.Outer.Dup".to_string()),
            field_type(&desc, "a.b.Outer", "shadowed"),
        );
        // Flattening drops the package and concatenates the nesting.
        assert_eq!("OuterDup", desc.rust_name("a.b.Outer.Dup"));
        assert_eq!("Dup", desc.rust_name("a.b.Dup"));
    }

    /// When the innermost scope has no match, the search strips one trailing segment at a time.
    /// `Leaf` referenced from `a.b.Outer.Mid` is declared at `a.b`, three scopes out.
    #[test]
    fn resolves_outward_through_scopes() {
        let dir = scratch(
            "outward",
            &[(
                "root.proto",
                r#"syntax = "proto3";
                package a.b;
                enum Leaf { LEAF_ZERO = 0; }
                message Outer {
                    message Mid {
                        Leaf far = 1;
                        .a.b.Leaf absolute = 2;
                    }
                }
                "#,
            )],
        );
        let desc = FileDescriptor::read_proto(&dir.join("root.proto"), &[dir.clone()]).unwrap();

        assert_eq!(
            FieldType::Enum("a.b.Leaf".to_string()),
            field_type(&desc, "a.b.Outer.Mid", "far"),
        );
        // A leading dot short-circuits the search; same target, reached absolutely.
        assert_eq!(
            FieldType::Enum("a.b.Leaf".to_string()),
            field_type(&desc, "a.b.Outer.Mid", "absolute"),
        );
        assert_eq!("OuterMid", desc.rust_name("a.b.Outer.Mid"));
    }

    /// A diamond — root imports both `left` and `right`, which each import `common` — must merge
    /// `common`'s message exactly once, and references to it from either arm must still resolve.
    #[test]
    fn diamond_import_merges_once() {
        let dir = scratch(
            "diamond",
            &[
                (
                    "common.proto",
                    r#"syntax = "proto3";
                    package common;
                    message Shared { int32 v = 1; }
                    "#,
                ),
                (
                    "left.proto",
                    r#"syntax = "proto3";
                    package left;
                    import "common.proto";
                    message L { common.Shared s = 1; }
                    "#,
                ),
                (
                    "right.proto",
                    r#"syntax = "proto3";
                    package right;
                    import "common.proto";
                    message R { common.Shared s = 1; }
                    "#,
                ),
                (
                    "root.proto",
                    r#"syntax = "proto3";
                    package root;
                    import "left.proto";
                    import "right.proto";
                    message Root { left.L l = 1; right.R r = 2; }
                    "#,
                ),
            ],
        );
        let desc = FileDescriptor::read_proto(&dir.join("root.proto"), &[dir.clone()]).unwrap();

        let shared: Vec<_> = desc
            .messages
            .iter()
            .filter(|m| m.full_name == "common.Shared")
            .collect();
        assert_eq!(1, shared.len(), "common.Shared merged more than once");

        assert_eq!(
            FieldType::Message("common.Shared".to_string()),
            field_type(&desc, "left.L", "s"),
        );
        assert_eq!(
            FieldType::Message("common.Shared".to_string()),
            field_type(&desc, "right.R", "s"),
        );
        assert_eq!(
            FieldType::Message("left.L".to_string()),
            field_type(&desc, "root.Root", "l"),
        );
    }

    /// An unresolvable reference is an error, not a panic or a silently-kept `Named`.
    #[test]
    fn unknown_type_reference_errors() {
        let dir = scratch(
            "unknown",
            &[(
                "root.proto",
                r#"syntax = "proto3";
                package a;
                message M { Nope n = 1; }
                "#,
            )],
        );
        let err = FileDescriptor::read_proto(&dir.join("root.proto"), &[dir.clone()]).unwrap_err();
        assert!(
            matches!(err, Error::MessageOrEnumNotFound(ref n) if n == "Nope"),
            "unexpected error: {err}"
        );
    }
}
