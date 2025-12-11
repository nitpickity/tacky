use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use crate::errors::{Error, Result};
use crate::parser::file_descriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Proto2,
    Proto3,
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

    // fn get_message_mut<'a>(&self, desc: &'a mut FileDescriptor) -> &'a mut Message {
    //     let first_message = self
    //         .indexes
    //         .first()
    //         .and_then(move |i| desc.messages.get_mut(*i));
    //     self.indexes
    //         .iter()
    //         .skip(1)
    //         .fold(first_message, |cur, next| {
    //             cur.and_then(|msg| msg.messages.get_mut(*next))
    //         })
    //         .expect("Message index not found")
    // }

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
    pub fn get_enum<'a>(&self, desc: &'a FileDescriptor) -> &'a Enumerator {
        let enums = if self.msg_index.indexes.is_empty() {
            &desc.enums
        } else {
            &self.msg_index.get_message(desc).enums
        };
        enums.get(self.index).expect("Enum index not found")
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

    pub fn proto_type(&self) -> &str {
        match *self {
            FieldType::Int32 => "int32",
            FieldType::Sint32 => "sint32",
            FieldType::Int64 => "int64",
            FieldType::Sint64 => "sint64",
            FieldType::Uint32 => "uint32",
            FieldType::Uint64 => "uint64",
            FieldType::Bool => "bool",
            FieldType::Enum(_) => "enum",
            FieldType::Fixed32 => "fixed32",
            FieldType::Sfixed32 => "sfixed32",
            FieldType::Float => "float",
            FieldType::Fixed64 => "fixed64",
            FieldType::Sfixed64 => "sfixed64",
            FieldType::Double => "double",
            FieldType::String => "string",
            FieldType::Bytes => "bytes",
            FieldType::Message(_) => "message",
            FieldType::Map(_, _) => "map",
            FieldType::MessageOrEnum(_) => unreachable!("Message / Enum not resolved"),
        }
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

// fn get_modules(module: &str, imported: bool, desc: &FileDescriptor) -> String {
//     let skip = usize::from(desc.package.is_empty() && !imported);
//     module
//         .split('.')
//         .filter(|p| !p.is_empty())
//         .skip(skip)
//         .map(|p| format!("{}::", p))
//         .collect()
// }

#[derive(Debug, Clone, Default)]
pub struct Extend {
    /// The message being extended.
    pub name: String,
    /// All fields that are being added to the extended message.
    pub fields: Vec<Field>,
}

impl Extend {}

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
    pub module: String,         // 'package' corresponding to actual generated Rust module
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

    // fn get_modules(&self, desc: &FileDescriptor) -> String {
    //     get_modules(&self.module, self.imported, desc)
    // }

    // fn is_unit(&self) -> bool {
    //     self.fields.is_empty()
    //         && self.oneofs.is_empty()
    //         && self.messages.iter().all(|m| m.is_unit())
    // }

    fn sanity_checks(&self, desc: &FileDescriptor) -> Result<()> {
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
            if let Some(var) = f.default.as_ref() {
                if let FieldType::Enum(ref e) = f.typ {
                    let e = e.get_enum(desc);
                    e.fields.iter().find(|&(ref name, _)| name == var)
                    .ok_or_else(|| Error::InvalidDefaultEnum(format!(
                                "Error in message {}\n\
                                Enum field {:?} has a default value '{}' which is not valid for enum index {:?}",
                                self.name, f, var, e)))?;
                }
            }
        }
        Ok(())
    }

    fn set_package(&mut self, package: &str, module: &str) {
        // The complication here is that the _package_ (as declared in the proto file) does
        // not directly map to the _module_. For example, the package 'a.A' where A is a
        // message will be the module 'a.mod_A', since we can't reuse the message name A as
        // the submodule containing nested items. Also, protos with empty packages always
        // have a module corresponding to the file name.
        let (child_package, child_module) = if package.is_empty() {
            self.module = module.to_string();
            (self.name.clone(), format!("{}.mod_{}", module, self.name))
        } else {
            self.package = package.to_string();
            self.module = module.to_string();
            (
                format!("{}.{}", package, self.name),
                format!("{}.mod_{}", module, self.name),
            )
        };

        for m in &mut self.messages {
            m.set_package(&child_package, &child_module);
        }
        for m in &mut self.enums {
            m.set_package(&child_package, &child_module);
        }
        for m in &mut self.oneofs {
            m.set_package(&child_package, &child_module);
        }
    }

    /// Return an iterator producing references to all the `Field`s of `self`,
    /// including both direct and `oneof` fields.
    pub fn all_fields(&self) -> impl Iterator<Item = &Field> {
        self.fields
            .iter()
            .chain(self.oneofs.iter().flat_map(|o| o.fields.iter()))
    }

    // /// Return an iterator producing mutable references to all the `Field`s of
    // /// `self`, including both direct and `oneof` fields.
    // fn all_fields_mut(&mut self) -> impl Iterator<Item = &mut Field> {
    //     self.fields
    //         .iter_mut()
    //         .chain(self.oneofs.iter_mut().flat_map(|o| o.fields.iter_mut()))
    // }
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

impl RpcService {
    pub fn write_definition<W: Write>(&self, _w: &mut W, _config: &Config) -> Result<()> {
        Ok(())
    }
}

pub type RpcGeneratorFunction = Box<dyn Fn(&RpcService, &mut dyn Write) -> Result<()>>;

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
    pub fully_qualified_fields: Vec<(String, i32)>,
    pub partially_qualified_fields: Vec<(String, i32)>,
    pub imported: bool,
    pub package: String,
    pub module: String,
    pub path: PathBuf,
    pub import: PathBuf,
    pub index: EnumIndex,
}

impl Enumerator {
    fn set_package(&mut self, package: &str, module: &str) {
        self.package = package.to_string();
        self.module = module.to_string();
        self.partially_qualified_fields = self
            .fields
            .iter()
            .map(|f| (format!("{}::{}", &self.name, f.0), f.1))
            .collect();
        self.fully_qualified_fields = self
            .partially_qualified_fields
            .iter()
            .map(|pqf| {
                let fqf = if self.module.is_empty() {
                    pqf.0.clone()
                } else {
                    format!("{}::{}", self.module.replace('.', "::"), pqf.0)
                };
                (fqf, pqf.1)
            })
            .collect();
    }

    // fn get_modules(&self, desc: &FileDescriptor) -> String {
    //     get_modules(&self.module, self.imported, desc)
    // }
}

#[derive(Debug, Clone, Default)]
pub struct OneOf {
    pub name: String,
    pub fields: Vec<Field>,
    pub package: String,
    pub module: String,
    pub imported: bool,
}

impl OneOf {
    fn set_package(&mut self, package: &str, module: &str) {
        self.package = package.to_string();
        self.module = module.to_string();
    }

    // fn get_modules(&self, desc: &FileDescriptor) -> String {
    //     get_modules(&self.module, self.imported, desc)
    // }
}

pub struct Config {
    pub in_file: PathBuf,
    pub out_file: PathBuf,
    pub single_module: bool,
    pub import_search_path: Vec<PathBuf>,
    pub no_output: bool,
    pub error_cycle: bool,
    pub add_deprecated_fields: bool,
}

#[derive(Debug, Default, Clone)]
pub struct FileDescriptor {
    pub import_paths: Vec<PathBuf>,
    pub package: String,
    pub syntax: Syntax,
    pub messages: Vec<Message>,
    pub message_extends: Vec<Extend>,
    pub enums: Vec<Enumerator>,
    pub module: String,
    pub rpc_services: Vec<RpcService>,
}

impl FileDescriptor {
    pub fn run(configs: &[Config]) -> Result<()> {
        for config in configs {
            Self::print_proto(config)?
        }
        Ok(())
    }
    pub fn print_proto(config: &Config) -> Result<()> {
        let mut desc = FileDescriptor::read_proto(&config.in_file, &config.import_search_path)?;

        if desc.messages.is_empty() && desc.enums.is_empty() {
            // There could had been unsupported structures, so bail early
            return Err(Error::EmptyRead);
        }

        desc.resolve_types()?;
        desc.sanity_checks()?;

        if config.single_module {
            desc.package = "".to_string();
        }

        let (prefix, file_package) = split_package(&desc.package);

        let file_stem = if file_package.is_empty() {
            get_file_stem(&config.out_file)?
        } else {
            file_package.to_string()
        };

        let mut out_file = config.out_file.with_file_name(format!("{file_stem}.rs"));

        if !prefix.is_empty() {
            use std::fs::create_dir_all;
            // e.g. package is a.b; we need to create directory 'a' and insert it into the path
            let file = PathBuf::from(out_file.file_name().unwrap());
            out_file.pop();
            for p in prefix.split('.') {
                out_file.push(p);

                if !out_file.exists() {
                    create_dir_all(&out_file)?;
                    update_mod_file(&out_file)?;
                }
            }
            out_file.push(file);
        }

        let imported = |b| if b { " imported" } else { "" };
        println!("source will be written to {}\n", out_file.display());
        for m in &desc.messages {
            println!(
                "message {} module {}{}",
                m.name,
                m.module,
                imported(m.imported)
            );
        }
        for e in &desc.enums {
            println!(
                "enum {} module {}{}",
                e.name,
                e.module,
                imported(e.imported)
            );
        }
        return Ok(());
    }

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
        // proto files with no packages are given an implicit module,
        // since every generated Rust source file represents a module
        desc.module = if desc.package.is_empty() {
            get_file_stem(in_file)?
        } else {
            desc.package.clone()
        };

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

    /// Get messages and enums from imports
    fn fetch_imports(&mut self, in_file: &Path, import_search_path: &[PathBuf]) -> Result<()> {
        for m in &mut self.messages {
            m.set_package(&self.package, &self.module);
        }
        for m in &mut self.enums {
            m.set_package(&self.package, &self.module);
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

            // if the proto has a packge then the names will be prefixed
            let package = f.package.clone();
            let module = f.module.clone();
            self.messages.extend(f.messages.drain(..).map(|mut m| {
                if m.package.is_empty() {
                    m.set_package(&package, &module);
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
                    e.set_package(&package, &module);
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

/// "" is ("",""), "a" is ("","a"), "a.b" is ("a"."b"), and so forth.
fn split_package(package: &str) -> (&str, &str) {
    if package.is_empty() {
        ("", "")
    } else if let Some(i) = package.rfind('.') {
        (&package[0..i], &package[i + 1..])
    } else {
        ("", package)
    }
}

const MAGIC_HEADER: &str = "// Automatically generated mod.rs";

/// Given a file path, create or update the mod.rs file within its folder
fn update_mod_file(path: &Path) -> Result<()> {
    let mut file = path.to_path_buf();
    use std::fs::OpenOptions;
    use std::io::prelude::*;

    let name = file.file_stem().unwrap().to_string_lossy().to_string();
    file.pop();
    file.push("mod.rs");
    let matches = "pub mod ";
    let mut present = false;
    let mut exists = false;
    if let Ok(f) = File::open(&file) {
        exists = true;
        let mut first = true;
        for line in BufReader::new(f).lines() {
            let line = line?;
            if first {
                if !line.contains(MAGIC_HEADER) {
                    // it is NOT one of our generated mod.rs files, so don't modify it!
                    present = true;
                    break;
                }
                first = false;
            }
            if let Some(i) = line.find(matches) {
                let rest = &line[i + matches.len()..line.len() - 1];
                if rest == name {
                    // we already have a reference to this module...
                    present = true;
                    break;
                }
            }
        }
    }
    if !present {
        let mut f = if exists {
            OpenOptions::new().append(true).open(&file)?
        } else {
            let mut f = File::create(&file)?;
            writeln!(f, "{}", MAGIC_HEADER)?;
            f
        };

        writeln!(f, "pub mod {};", name)?;
    }
    Ok(())
}

/// get the proper sanitized file stem from an input file path
fn get_file_stem(path: &Path) -> Result<String> {
    let mut file_stem = path
        .file_stem()
        .and_then(|f| f.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::OutputFile(format!("{}", path.display())))?;

    file_stem = file_stem.replace(|c: char| !c.is_alphanumeric(), "_");
    Ok(file_stem)
}
