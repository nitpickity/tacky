//! Thin binding to `cpp/shim.cc`, the official C++ protobuf runtime.
//!
//! Lives in the lib rather than the bench so the bench inherits the build
//! script's native-library link flags through this crate's rlib.
//!
//! The C++ message is built once by parsing prost's wire bytes, so all arms of
//! the comparison bench encode identical field values.

pub const MIXED: i32 = 0;
pub const PPROF: i32 = 1;
pub const ACCESSLOG: i32 = 2;
pub const REPEATED_STRINGS: i32 = 3;
/// Same wire format as [`ACCESSLOG`], from an edition-2023 copy of the proto with
/// `features.utf8_validation = NONE`. See `derive_noutf8_proto` in `build.rs`.
pub const ACCESSLOG_NO_UTF8: i32 = 4;
/// Same, for [`PPROF`].
pub const PPROF_NO_UTF8: i32 = 5;
pub const MESSAGE1_PROTO2: i32 = 6;
pub const MESSAGE1_PROTO3: i32 = 7;
/// `google.protobuf.FileDescriptorSet`, taken from the C++ runtime's own built-in
/// `descriptor.pb.h` rather than from our vendored copy of `descriptor.proto`:
/// compiling that copy would register a second `google/protobuf/descriptor.proto`
/// in the C++ descriptor pool, which aborts at startup.
pub const FILE_DESCRIPTOR_SET: i32 = 8;
pub const OTLP_TRACES: i32 = 9;
/// Same wire format as [`MESSAGE1_PROTO3`], UTF-8 validation off.
pub const MESSAGE1_PROTO3_NO_UTF8: i32 = 10;
/// Same, for [`OTLP_TRACES`].
pub const OTLP_TRACES_NO_UTF8: i32 = 11;
pub const OTLP_LOGS: i32 = 12;
/// Same, for [`OTLP_LOGS`].
pub const OTLP_LOGS_NO_UTF8: i32 = 13;

extern "C" {
    fn tacky_cpp_new(kind: i32, wire: *const u8, len: usize) -> *mut core::ffi::c_void;
    fn tacky_cpp_free(handle: *mut core::ffi::c_void);
    fn tacky_cpp_byte_size(handle: *const core::ffi::c_void) -> usize;
    fn tacky_cpp_serialize(handle: *const core::ffi::c_void, out: *mut u8, cap: usize) -> usize;
    fn tacky_cpp_serialize_cached(
        handle: *const core::ffi::c_void,
        out: *mut u8,
        cap: usize,
    ) -> usize;
    fn tacky_cpp_noop();
}

/// An empty `extern "C"` call, for measuring the FFI overhead baked into the
/// C++ timings.
pub fn noop() {
    unsafe { tacky_cpp_noop() }
}

pub struct Msg(*mut core::ffi::c_void);

impl Msg {
    /// Parses `wire` into a C++ message and primes its size cache.
    pub fn parse(kind: i32, wire: &[u8]) -> Self {
        let handle = unsafe { tacky_cpp_new(kind, wire.as_ptr(), wire.len()) };
        assert!(
            !handle.is_null(),
            "C++ ParseFromArray failed for kind {kind}"
        );
        let msg = Msg(handle);
        msg.byte_size();
        msg
    }

    pub fn byte_size(&self) -> usize {
        unsafe { tacky_cpp_byte_size(self.0) }
    }

    /// Size pass + write pass into `buf`'s spare capacity. Never reallocates:
    /// the caller pre-sizes the buffer, matching the tacky and prost arms.
    pub fn serialize(&self, buf: &mut Vec<u8>) -> usize {
        let cap = buf.capacity();
        let n = unsafe { tacky_cpp_serialize(self.0, buf.as_mut_ptr(), cap) };
        assert_ne!(n, 0, "C++ serialize overflowed the {cap}-byte buffer");
        unsafe { buf.set_len(n) };
        n
    }

    /// Write pass only, reusing the primed size cache.
    pub fn serialize_cached(&self, buf: &mut Vec<u8>) -> usize {
        let cap = buf.capacity();
        let n = unsafe { tacky_cpp_serialize_cached(self.0, buf.as_mut_ptr(), cap) };
        assert_ne!(n, 0, "C++ serialize overflowed the {cap}-byte buffer");
        unsafe { buf.set_len(n) };
        n
    }
}

impl Drop for Msg {
    fn drop(&mut self) {
        unsafe { tacky_cpp_free(self.0) }
    }
}
