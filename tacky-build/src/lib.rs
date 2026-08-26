//! Build-time code generation for the `tacky` crate.
//!
//! Call [`write_proto`] (or [`write_proto_with_includes`]) from `build.rs`, once per `.proto` file.
//! Each call emits a zero-sized schema struct per message, an enum per message for decoding, and a
//! Rust enum per proto enum.

#![allow(unused, dead_code)]
mod field_enum;
mod field_type;
mod parser;
pub use parser::{write_proto, write_proto_with_includes};
