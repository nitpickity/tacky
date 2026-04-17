use std::collections::HashMap;
use std::fmt;
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

#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct MessageIndex {
    indexes: Vec<usize>,
}

impl fmt::Debug for MessageIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        f.debug_set().entries(self.indexes.iter()).finish()
    }
}

impl MessageIndex {
    pub fn get_message<'a>(&self, desc: &'a FileDescriptor) -> &'a Message {
        let first_message = self.indexes.first().and_then(|i| desc.messages.get(*i));
        self.indexes
            .iter()
            .skip(1)
            .fold(first_message, |cur, next| {
                cur.and_then(|msg| msg.messages.get(*next))
            })
            .expect("Message index not found")
    }

    /// Compute a flattened qualified name (e.g. `OuterInner`) by walking the index path.
    pub fn qualified_name(&self, desc: &FileDescriptor) -> String {
        let mut name = String::new();
        let mut current_messages = &desc.messages;
        for &idx in &self.indexes {
            let msg = &current_messages[idx];
            name.push_str(&msg.name);
            current_messages = &msg.messages;
        }
        name
    }

    /// Access the raw index path.
    pub fn indexes(&self) -> &[usize] {
        &self.indexes
    }

    fn push(&mut self, i: usize) {
        self.indexes.push(i);
    }

    fn pop(&mut self) {
        self.indexes.pop();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct EnumIndex {
    msg_index: MessageIndex,
    index: usize,
}

impl EnumIndex {
    /// Access the parent message index path (empty for top-level enums).
    pub fn msg_indexes(&self) -> &[usize] {
        self.msg_index.indexes()
    }

    pub fn get_enum<'a>(&self, desc: &'a FileDescriptor) -> &'a Enumerator {
        let enums = if self.msg_index.indexes.is_empty() {
            &desc.enums
        } else {
            &self.msg_index.get_message(desc).enums
        };
        enums.get(self.index).expect("Enum index not found")
    }

    /// Compute a flattened qualified name (e.g. `OuterStatus`) by walking the parent path.
    pub fn qualified_name(&self, desc: &FileDescriptor) -> String {
        let enum_name = &self.get_enum(desc).name;
        if self.msg_index.indexes.is_empty() {
            enum_name.clone()
        } else {
            format!("{}{}", self.msg_index.qualified_name(desc), enum_name)
        }
    }
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
    Enum(EnumIndex),
    Fixed64,
    Sfixed64,
    Double,
    String,
    Bytes,
    Message(MessageIndex),
    MessageOrEnum(String),
    Fixed32,
    Sfixed32,
    Float,
    Map(Box<FieldType>, Box<FieldType>),
}

impl FieldType {
    pub fn is_primitive(&self) -> bool {
        !matches!(
            *self,
            FieldType::Message(_)
                | FieldType::MessageOrEnum(_)
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
    pub default: Option<String>,
    pub deprecated: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Extend {
    /// The message being extended.
    pub name: String,
    /// All fields that are being added to the extended message.
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default)]
pub struct Message {
    pub name: String,
    pub fields: Vec<Field>,
    pub oneofs: Vec<OneOf>,
    pub reserved_nums: Option<Vec<i32>>,
    pub reserved_names: Option<Vec<String>>,
    pub imported: bool,
    pub package: String,        // package from imports + nested items
    pub messages: Vec<Message>, // nested messages
    pub enums: Vec<Enumerator>, // nested enums
    pub path: PathBuf,
    pub import: PathBuf,
    pub index: MessageIndex,
    /// Allowed extensions for this message, None if no extensions.
    pub extensions: Option<Extensions>,
}

impl Message {
    fn set_imported(&mut self) {
        self.imported = true;
        for o in self.oneofs.iter_mut() {
            o.imported = true;
        }
        for m in self.messages.iter_mut() {
            m.set_imported();
        }
        for e in self.enums.iter_mut() {
            e.imported = true;
        }
    }

    fn sanity_checks(&self, _desc: &FileDescriptor) -> Result<()> {
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

            // check default enums
            if let Some(_) = f.default.as_ref() {
                return Err(Error::InvalidDefaultEnum(format!(
                    "Error in message {}\n custom defaults are not supported in tacky",
                    self.name
                )));
            }
        }
        Ok(())
    }

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
pub struct RpcFunctionDeclaration {
    pub name: String,
    pub arg: String,
    pub ret: String,
}

#[derive(Debug, Clone, Default)]
pub struct RpcService {
    pub service_name: String,
    pub functions: Vec<RpcFunctionDeclaration>,
}

#[derive(Debug, Clone, Default)]
pub struct Extensions {
    pub from: i32,
    /// Max number is 536,870,911 (2^29 - 1), as defined in the
    /// protobuf docs
    pub to: i32,
}

impl Extensions {
    /// The max field number that can be used as an extension.
    pub fn max() -> i32 {
        536870911
    }
}

#[derive(Debug, Clone, Default)]
pub struct Enumerator {
    pub name: String,
    pub fields: Vec<(String, i32)>,
    pub imported: bool,
    pub package: String,
    pub path: PathBuf,
    pub import: PathBuf,
    pub index: EnumIndex,
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
    pub package: String,
    pub imported: bool,
}

pub struct Config {
    pub in_file: PathBuf,
    pub import_search_path: Vec<PathBuf>,
}

#[derive(Debug, Default, Clone)]
pub struct FileDescriptor {
    pub import_paths: Vec<PathBuf>,
    pub package: String,
    pub syntax: Syntax,
    pub messages: Vec<Message>,
    pub message_extends: Vec<Extend>,
    pub enums: Vec<Enumerator>,
    pub rpc_services: Vec<RpcService>,
}

impl FileDescriptor {
    /// Opens a proto file, reads it and returns raw parsed data
    pub fn read_proto(in_file: &Path, import_search_path: &[PathBuf]) -> Result<FileDescriptor> {
        let file = std::fs::read_to_string(in_file)?;
        let (rem, mut desc) = file_descriptor(&file).map_err(Error::Nom)?;
        let rem = rem.trim();
        if !rem.is_empty() {
            return Err(Error::TrailingGarbage(rem.chars().take(50).collect()));
        }
        for m in &mut desc.messages {
            if m.path.as_os_str().is_empty() {
                m.path = in_file.to_path_buf();
                if !import_search_path.is_empty() {
                    if let Ok(p) = m.path.clone().strip_prefix(&import_search_path[0]) {
                        m.import = p.to_path_buf();
                    }
                }
            }
        }

        desc.fetch_imports(in_file, import_search_path)?;
        desc.resolve_types()?;
        desc.sanity_checks()?;
        Ok(desc)
    }

    fn sanity_checks(&self) -> Result<()> {
        for m in &self.messages {
            m.sanity_checks(self)?;
        }
        Ok(())
    }

    /// Reset all resolved field types (Message/Enum indices) back to unresolved
    /// MessageOrEnum strings. This must be called on imported descriptors before
    /// merging, because MessageIndex/EnumIndex values are local to each descriptor
    /// and become stale after merging.
    fn unresolve_types(&mut self) {
        let (fwd_msgs, fwd_enums) = self.get_full_names();
        let msg_names: HashMap<MessageIndex, String> =
            fwd_msgs.into_iter().map(|(k, v)| (v, k)).collect();
        let enum_names: HashMap<EnumIndex, String> =
            fwd_enums.into_iter().map(|(k, v)| (v, k)).collect();

        fn unresolve_field_type(
            typ: &mut FieldType,
            msg_names: &HashMap<MessageIndex, String>,
            enum_names: &HashMap<EnumIndex, String>,
        ) {
            match typ {
                FieldType::Message(idx) => {
                    if let Some(name) = msg_names.get(idx) {
                        *typ = FieldType::MessageOrEnum(name.clone());
                    }
                }
                FieldType::Enum(idx) => {
                    if let Some(name) = enum_names.get(idx) {
                        *typ = FieldType::MessageOrEnum(name.clone());
                    }
                }
                FieldType::Map(ref mut k, ref mut v) => {
                    unresolve_field_type(k, msg_names, enum_names);
                    unresolve_field_type(v, msg_names, enum_names);
                }
                _ => {}
            }
        }

        fn unresolve_message(
            m: &mut Message,
            msg_names: &HashMap<MessageIndex, String>,
            enum_names: &HashMap<EnumIndex, String>,
        ) {
            for f in m
                .fields
                .iter_mut()
                .chain(m.oneofs.iter_mut().flat_map(|o| o.fields.iter_mut()))
            {
                unresolve_field_type(&mut f.typ, msg_names, enum_names);
            }
            for nested in m.messages.iter_mut() {
                unresolve_message(nested, msg_names, enum_names);
            }
        }

        for m in &mut self.messages {
            unresolve_message(m, &msg_names, &enum_names);
        }
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
            let mut f = FileDescriptor::read_proto(&proto_file, import_search_path)?;

            // Reset resolved indices before merging — they reference the imported
            // file's local descriptor and would be stale in the combined one.
            f.unresolve_types();

            // if the proto has a package then the names will be prefixed
            let package = f.package.clone();
            self.messages.extend(f.messages.drain(..).map(|mut m| {
                if m.package.is_empty() {
                    m.set_package(&package);
                }
                if m.path.as_os_str().is_empty() {
                    m.path = proto_file.clone();
                }
                if m.import.as_os_str().is_empty() {
                    m.import = import.clone();
                }
                m.set_imported();
                m
            }));
            self.enums.extend(f.enums.drain(..).map(|mut e| {
                if e.package.is_empty() {
                    e.set_package(&package);
                }
                if e.path.as_os_str().is_empty() {
                    e.path = proto_file.clone();
                }
                if e.import.as_os_str().is_empty() {
                    e.import = import.clone();
                }
                e.imported = true;
                e
            }));
        }
        Ok(())
    }

    fn get_full_names(&mut self) -> (HashMap<String, MessageIndex>, HashMap<String, EnumIndex>) {
        fn rec_full_names(
            m: &mut Message,
            index: &mut MessageIndex,
            full_msgs: &mut HashMap<String, MessageIndex>,
            full_enums: &mut HashMap<String, EnumIndex>,
        ) {
            m.index = index.clone();
            if m.package.is_empty() {
                full_msgs
                    .entry(m.name.clone())
                    .or_insert_with(|| index.clone());
            } else {
                full_msgs
                    .entry(format!("{}.{}", m.package, m.name))
                    .or_insert_with(|| index.clone());
            }
            for (i, e) in m.enums.iter_mut().enumerate() {
                let index = EnumIndex {
                    msg_index: index.clone(),
                    index: i,
                };
                e.index = index.clone();
                full_enums
                    .entry(format!("{}.{}", e.package, e.name))
                    .or_insert(index);
            }
            for (i, m) in m.messages.iter_mut().enumerate() {
                index.push(i);
                rec_full_names(m, index, full_msgs, full_enums);
                index.pop();
            }
        }

        let mut full_msgs = HashMap::new();
        let mut full_enums = HashMap::new();
        let mut index = MessageIndex { indexes: vec![] };
        for (i, m) in self.messages.iter_mut().enumerate() {
            index.push(i);
            rec_full_names(m, &mut index, &mut full_msgs, &mut full_enums);
            index.pop();
        }
        for (i, e) in self.enums.iter_mut().enumerate() {
            let index = EnumIndex {
                msg_index: index.clone(),
                index: i,
            };
            e.index = index.clone();
            if e.package.is_empty() {
                full_enums
                    .entry(e.name.clone())
                    .or_insert_with(|| index.clone());
            } else {
                full_enums
                    .entry(format!("{}.{}", e.package, e.name))
                    .or_insert_with(|| index.clone());
            }
        }
        (full_msgs, full_enums)
    }

    fn resolve_types(&mut self) -> Result<()> {
        let (full_msgs, full_enums) = self.get_full_names();

        fn rec_resolve_types(
            m: &mut Message,
            full_msgs: &HashMap<String, MessageIndex>,
            full_enums: &HashMap<String, EnumIndex>,
        ) -> Result<()> {
            // Interestingly, we can't call all_fields_mut to iterate over the
            // fields here: writing out the field traversal as below lets Rust
            // split m's mutable borrow, permitting the loop body to use fields
            // of `m` other than `fields` and `oneofs`.
            'types: for typ in m
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
                if let FieldType::MessageOrEnum(name) = typ.clone() {
                    let test_names: Vec<String> = if name.starts_with('.') {
                        vec![name.clone().split_off(1)]
                    } else if m.package.is_empty() {
                        vec![format!("{}.{}", m.name, name), name.clone()]
                    } else {
                        let mut v = vec![
                            format!("{}.{}.{}", m.package, m.name, name),
                            format!("{}.{}", m.package, name),
                        ];
                        for (index, _) in m.package.match_indices('.').rev() {
                            v.push(format!("{}.{}", &m.package[..index], name));
                        }
                        v.push(name.clone());
                        v
                    };
                    for name in &test_names {
                        if let Some(msg) = full_msgs.get(name) {
                            *typ = FieldType::Message(msg.clone());
                            continue 'types;
                        } else if let Some(e) = full_enums.get(name) {
                            *typ = FieldType::Enum(e.clone());
                            continue 'types;
                        }
                    }
                    return Err(Error::MessageOrEnumNotFound(name));
                }
            }

            // Downgrade 'Packed' frequency to 'Repeated' for non-primitive types
            // (like Messages) now that types are fully resolved.
            // Enums are primitives so they remain Packed.
            for f in m
                .fields
                .iter_mut()
                .chain(m.oneofs.iter_mut().flat_map(|o| o.fields.iter_mut()))
            {
                if f.frequency == Some(Frequency::Packed) && !f.typ.is_primitive() {
                    f.frequency = Some(Frequency::Repeated);
                }
            }

            for m in m.messages.iter_mut() {
                rec_resolve_types(m, full_msgs, full_enums)?;
            }
            Ok(())
        }

        for m in self.messages.iter_mut() {
            rec_resolve_types(m, &full_msgs, &full_enums)?;
        }
        Ok(())
    }
}
