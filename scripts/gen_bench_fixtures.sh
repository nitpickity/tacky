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
#
# `descriptor.proto` is in the list, so this fixture — like the other two — contains its
# own schema. That is what a real registry holds, and it is also the densest file
# available: extension ranges, reserved ranges, nested enums, oneofs, custom defaults.
protoc -Itesting/protos --include_imports \
    --descriptor_set_out="$OUT/testing_protos.fds" \
    simple_message.proto \
    importing.proto \
    proto3_message.proto \
    pprof.proto \
    accesslog.proto \
    benchmarks.proto \
    benchmark_message1_proto2.proto \
    benchmark_message1_proto3.proto \
    descriptor.proto

# Arm 3: registry scale. Everything above plus the vendored OTLP tree, with
# `--include_source_info`, which is what `protoc` emits for a schema registry or a gRPC
# reflection service and what buf ships. That is the real-world shape: 130 KB rather
# than 8, and most of the extra is `SourceCodeInfo.location` — packed int32 `path` and
# `span` arrays, thousands of them, a work mix the other two fixtures do not have at
# all. Both of those are also too small to leave L2, which is the trap this arm exists
# to avoid.
protoc -Itesting/protos --include_imports --include_source_info \
    --descriptor_set_out="$OUT/registry.fds" \
    simple_message.proto \
    importing.proto \
    proto3_message.proto \
    pprof.proto \
    accesslog.proto \
    benchmarks.proto \
    benchmark_message1_proto2.proto \
    benchmark_message1_proto3.proto \
    descriptor.proto \
    opentelemetry/proto/trace/v1/trace.proto

# Note `trace.proto`, not `collector/.../trace_service.proto`: that one declares a
# `service`, tacky does not generate service definitions, and so the writer would drop
# it and the round-trip assert would fail. Its imports pull in common and resource
# anyway, which is the bulk of the OTLP schema.

wc -c "$OUT"/*.fds

# ---------------------------------------------------------------------------
# pprof
# ---------------------------------------------------------------------------
# `testing/data/pprof_go_heap.pb` is a real Go heap profile, not a generated one, so
# there is nothing here to regenerate — only to re-fetch if it is ever lost. It comes
# from grafana/pyroscope's test fixtures, gunzipped:
#
#   curl -sL -o - \
#     https://raw.githubusercontent.com/grafana/pyroscope/main/pkg/pprof/testdata/heap \
#     | gunzip -c > testing/data/pprof_go_heap.pb
#
# That file is AGPL-3.0 testdata from pyroscope. It is vendored as data only — nothing
# links against it — and it stays under its own licence, not this repo's.
#
# Do not swap it for a synthesised profile: stack depth, section proportions, values per
# sample and label rate all have to be right together, and getting any of them wrong
# flatters the encoder. The real profile's measured shape is documented on `PPROF_FIXTURE`
# in benches/comparison.rs.
