//! A `.proto` parser, vendored and stripped down from pb-rs (of quick-protobuf).
//!
//! [`types::FileDescriptor::read_proto`] is the only entry point: it parses one `.proto`, inlines
//! its transitive imports and resolves every type reference. None of pb-rs's own code generation
//! is retained — tacky-build generates its own.

pub mod errors;
mod parser;
pub mod types;
