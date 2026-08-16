// Minimal C API over the official C++ protobuf runtime, so criterion can time it
// in the exact same harness as tacky and prost.
//
// Messages are built once, at setup, by parsing wire bytes produced by prost.
// That guarantees the C++ message tree holds byte-for-byte the same field values
// as the Rust side without hand-transcribing the data into C++.

#include <cstddef>
#include <cstdint>

#include <google/protobuf/descriptor.pb.h>

#include "accesslog.pb.h"
#include "benchmark_message1_proto2.pb.h"
#include "benchmark_message1_proto3.pb.h"
#include "noutf8/accesslog.pb.h"
#include "noutf8/benchmark_message1_proto3.pb.h"
#include "noutf8/opentelemetry/proto/collector/trace/v1/trace_service.pb.h"
#include "noutf8/pprof.pb.h"
#include "opentelemetry/proto/collector/trace/v1/trace_service.pb.h"
#include "pprof.pb.h"
#include "simple_message.pb.h"

using google::protobuf::MessageLite;

namespace {
MessageLite* make(int kind) {
    switch (kind) {
        case 0:
            return new example::MixedUsageMessage();
        case 1:
            return new perftools::profiles::Profile();
        case 2:
            return new accesslog::AccessLog();
        case 3:
            return new example::RepeatedStrings();
        case 4:
            return new noutf8::accesslog::AccessLog();
        case 5:
            return new noutf8::perftools::profiles::Profile();
        case 6:
            return new benchmarks::proto2::GoogleMessage1();
        case 7:
            return new benchmarks::proto3::GoogleMessage1();
        case 8:
            return new google::protobuf::FileDescriptorSet();
        case 9:
            return new opentelemetry::proto::collector::trace::v1::ExportTraceServiceRequest();
        case 10:
            return new noutf8::benchmarks::proto3::GoogleMessage1();
        case 11:
            return new noutf8::opentelemetry::proto::collector::trace::v1::
                ExportTraceServiceRequest();
        default:
            return nullptr;
    }
}
}  // namespace

extern "C" {

/// Parse `wire` into a heap-allocated message of the given kind.
/// Returns null on unknown kind or parse failure.
void* tacky_cpp_new(int kind, const uint8_t* wire, size_t len) {
    MessageLite* msg = make(kind);
    if (msg == nullptr) {
        return nullptr;
    }
    if (!msg->ParseFromArray(wire, static_cast<int>(len))) {
        delete msg;
        return nullptr;
    }
    return msg;
}

void tacky_cpp_free(void* handle) { delete static_cast<MessageLite*>(handle); }

/// Runs the size pass and caches per-submessage sizes. Also used at setup to
/// prime the cache for `tacky_cpp_serialize_cached`.
size_t tacky_cpp_byte_size(const void* handle) {
    return static_cast<const MessageLite*>(handle)->ByteSizeLong();
}

/// Full public-API serialize: size pass, then write pass. This is exactly what
/// `Message::SerializeToArray` does internally (ByteSizeLong, bounds check,
/// SerializeWithCachedSizesToArray); it is split out only so we can return the
/// length instead of a bool. Analogous to prost's `msg.encode(&mut buf)`.
size_t tacky_cpp_serialize(const void* handle, uint8_t* out, size_t cap) {
    const MessageLite* msg = static_cast<const MessageLite*>(handle);
    size_t len = msg->ByteSizeLong();
    if (len > cap) {
        return 0;
    }
    msg->SerializeWithCachedSizesToArray(out);
    return len;
}

/// Write pass only, reusing sizes cached by an earlier `tacky_cpp_byte_size`.
/// Not a legal steady state for a mutating producer, but it is the theoretical
/// floor for the C++ runtime: one pass, with all lengths already known.
size_t tacky_cpp_serialize_cached(const void* handle, uint8_t* out, size_t cap) {
    const MessageLite* msg = static_cast<const MessageLite*>(handle);
    size_t len = static_cast<size_t>(msg->GetCachedSize());
    if (len > cap) {
        return 0;
    }
    msg->SerializeWithCachedSizesToArray(out);
    return len;
}

/// Empty call, for measuring the FFI overhead included in the numbers above.
void tacky_cpp_noop(void) {}
}
