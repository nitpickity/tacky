#!/usr/bin/env bash
# Regenerates the checked-in `FileDescriptorSet` fixtures for
# `testing/benches/descriptor_set.rs`.
#
# The payloads are checked in rather than generated at build time so `cargo bench`
# works in CI without a matching local protoc. Run this only when the input protos
# change, and re-run the bench afterwards: the harness asserts that prost can read
# back what tacky writes, so a stale fixture shows up as a failure, not as a wrong
# number.
#
# protoc must match the vendored `testing/protos/descriptor.proto`, otherwise the
# fixture can carry fields that schema doesn't describe.
set -euo pipefail

cd "$(dirname "$0")/.."
OUT=testing/data

echo "protoc: $(protoc --version)"

# Arm 1: the vendored descriptor.proto describing itself. Denser than anything we
# write by hand — extension ranges, reserved ranges, nested enums, every file
# option. Deliberately compiled from `testing/protos`, not from protoc's own
# include dir: the fixture must be described by the schema tacky generates from,
# and that one is pinned to protobuf v3.20.3.
protoc -Itesting/protos --include_imports \
    --descriptor_set_out="$OUT/descriptor_proto.fds" \
    descriptor.proto

# Arm 2: this repo's own protos. Same order of magnitude, flatter shape: mostly
# names, field numbers and json_names. Keep this list in sync by hand — a proto
# added here and forgotten there just means the fixture covers less.
protoc -Itesting/protos --include_imports \
    --descriptor_set_out="$OUT/testing_protos.fds" \
    simple_message.proto \
    importing.proto \
    proto3_message.proto \
    pprof.proto \
    accesslog.proto \
    benchmarks.proto \
    benchmark_message1_proto2.proto \
    benchmark_message1_proto3.proto

wc -c "$OUT"/*.fds
