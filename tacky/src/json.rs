//! Decoding straight to JSON, for debugging and for handing a message to something that does not
//! speak protobuf. `tacky-build` generates `Msg::to_json(&bytes)` next to `Msg::decode`, so
//! `Msg::to_json(&bytes)?.to_string()` turns encoded bytes into a JSON string.
//!
//! One decode pass per message, with protobuf's parse semantics rather than wire order (for the
//! wire as-is, see the `Debug` impl). Each field key appears once, in order of first appearance: a
//! repeated, packed or map field collects every occurrence, a singular scalar takes the last, a
//! singular message seen twice merges, and a oneof member clears the rest of its group. Unknown
//! fields are dropped. This is not ProtoJSON: names stay as in the `.proto`, 64-bit integers are
//! numbers, bytes are arrays of numbers.

pub use serde_json;

use crate::{packed::PackedIter, DecodeError, Packable};
use alloc::string::ToString;
use serde_json::{Map, Value};

/// Plumbing for the generated code, which uses it to recurse uniformly over scalars, enums and
/// nested messages. `Msg::to_json` is the entry point.
#[doc(hidden)]
pub trait ToJson {
    fn to_json(&self) -> Result<Value, DecodeError>;
}

macro_rules! impl_to_json {
    ($($t:ty),*) => {
        $(
            impl ToJson for $t {
                fn to_json(&self) -> Result<Value, DecodeError> {
                    Ok(Value::from(*self))
                }
            }
        )*
    };
}
impl_to_json!(i32, i64, u32, u64, f32, f64, bool, &str, &[u8]);

/// Appends `value` to the array under `key`, creating it on first occurrence.
#[doc(hidden)]
pub fn push(m: &mut Map<alloc::string::String, Value>, key: &str, value: Value) {
    let Value::Array(a) = m
        .entry(key)
        .or_insert_with(|| Value::Array(alloc::vec::Vec::new()))
    else {
        unreachable!()
    };
    a.push(value);
}

/// Appends a packed chunk's values to the array under `key`.
#[doc(hidden)]
pub fn push_packed<'a, T: Packable>(
    m: &mut Map<alloc::string::String, Value>,
    key: &str,
    values: PackedIter<'a, T>,
) -> Result<(), DecodeError>
where
    T::RustType<'a>: ToJson,
{
    for v in values {
        push(m, key, v?.to_json()?);
    }
    Ok(())
}

/// Inserts a map entry into the object under `key`, creating it on first occurrence. JSON keys
/// are strings, so the protobuf key goes through `Display`.
#[doc(hidden)]
pub fn insert_entry(
    m: &mut Map<alloc::string::String, Value>,
    key: &str,
    k: impl core::fmt::Display,
    value: Value,
) {
    let Value::Object(o) = m.entry(key).or_insert_with(|| Value::Object(Map::new())) else {
        unreachable!()
    };
    o.insert(k.to_string(), value);
}
