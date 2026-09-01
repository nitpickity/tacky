//! A `.proto` parser, vendored and stripped down from **pb-rs**, part of the
//! [quick-protobuf](https://github.com/tafia/quick-protobuf) project by Johann Tuffe, used here
//! under its MIT licence. See `tacky-build/THIRD-PARTY.md` for the notice.
//!
//! [`types::FileDescriptor::read_protos`] is the entry point: it parses one or more `.proto` files,
//! inlines their transitive imports and resolves every type reference. Parsing `.proto` in pure Rust
//! is what lets tacky avoid a `protoc` system dependency.
//!
//! Only the parsing and validation half of pb-rs survives. Its code generation — which emitted
//! quick-protobuf-compatible Rust — is gone, along with the CLI, the module writer, and the
//! `MessageIndex`/`EnumIndex` machinery that generation needed to navigate a nested descriptor.
//! tacky generates its own schema and decoder code from the flat symbol table this produces.

pub mod errors;
mod parser;
pub mod types;
