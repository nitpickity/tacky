//! Public benchmarks: tacky vs prost, apples-to-apples.
//!
//! These are the numbers you put in the README. Every arm encodes identical data;
//! prost and C++ messages are pre-built so we measure pure encoding/decoding
//! speed, not allocation.
//!
//! Wire output is semantically identical everywhere. At the default `Tack` width a
//! placeholder is *grown* rather than padded, so tacky's byte count matches prost's exactly;
//! each bench prints the two lengths so that stays checked rather than assumed. Byte-level
//! equality is still not assertable against a reverse writer, which emits fields in the
//! opposite order — legal, and checked by decoding instead.
//!
//! `--features cpp` adds arms for the official C++ protobuf runtime; run
//! `scripts/bench_cpp.sh`, which sets it up statically. Each C++ workload gets up to four
//! arms:
//!
//! - `cpp` — public API equivalent: size pass, then write pass.
//! - `cpp-cached` — write pass only, sizes precomputed. Not a legal steady state
//!   for a mutating producer, but it is the runtime's theoretical floor.
//! - `cpp-noutf8` / `cpp-noutf8-cached` — same, with proto3 UTF-8 validation off,
//!   which Rust gets for free from `&str`. See `derive_noutf8_proto` in build.rs.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use prost::Message;

#[allow(dead_code)]
mod tacky_proto {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}
#[allow(dead_code)]
mod prost_proto {
    include!(concat!(env!("OUT_DIR"), "/example.rs"));
}
#[allow(dead_code)]
mod tacky_pprof {
    include!(concat!(env!("OUT_DIR"), "/pprof.rs"));
}
#[allow(dead_code)]
mod prost_pprof {
    include!(concat!(env!("OUT_DIR"), "/perftools.profiles.rs"));
}
#[allow(dead_code)]
mod tacky_accesslog {
    include!(concat!(env!("OUT_DIR"), "/tacky_accesslog.rs"));
}
#[allow(dead_code)]
mod prost_accesslog {
    include!(concat!(env!("OUT_DIR"), "/accesslog.rs"));
}

// Shared with `benches/descriptor_set.rs`, which measures this writer on its own. Here it
// is one of the three unlike message types in `bench_encode_rotating`.
#[path = "common/fds_writer.rs"]
mod fds_writer;

const FDS_REGISTRY: &[u8] = include_bytes!("../data/registry.fds");

use prost_proto::{
    MixedLargeMessage as PMixedLargeMessage, MixedSmallMessage as PMixedSmallMessage,
    MixedUsageMessage as PMixedUsageMessage,
};
use tacky_proto::example::{MixedUsageMessage as TMixedUsageMessage, SimpleEnum as TSimpleEnum};

// ---------------------------------------------------------------------------
// Official C++ protobuf runtime (feature = "cpp")
// ---------------------------------------------------------------------------

#[cfg(feature = "cpp")]
use testing::cpp;

#[cfg(feature = "cpp")]
#[path = "common/cpp_arms.rs"]
mod cpp_arms;
#[cfg(feature = "cpp")]
use cpp_arms::bench_cpp_arms;

/// Calibration arm: an empty `extern "C"` call, so the FFI cost baked into the
/// C++ numbers is visible rather than assumed negligible.
#[cfg(feature = "cpp")]
fn bench_ffi_overhead(c: &mut Criterion) {
    c.benchmark_group("ffi_overhead")
        .bench_function("noop_extern_call", |b| {
            b.iter(|| cpp::noop());
        });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Encode a full MixedUsageMessage with tacky, all fields set.
fn tacky_encode_mixed_all<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>) {
    let schema = TMixedUsageMessage::schema();
    schema
        .session_id
        .write(buf, Some("sess-a]b1c2d3-e4f5-6789-abcd-ef0123456789"));
    schema.user_id.write(buf, Some(9999));
    schema
        .client_version
        .write(buf, Some("v2.14.3-beta.1+build.20240315"));
    schema.small_payload.write_msg(buf, |buf, scm| {
        scm.label
            .write(buf, Some("inventory-check-primary-warehouse-us-east"));
        scm.count.write(buf, Some(42));
        scm.active.write(buf, Some(true));
    });
    schema.large_payload.write_msg(buf, |buf, scm| {
        scm.id.write(buf, Some("entity-987654321-abcdef"));
        scm.name.write(buf, Some("Production Order Processing Pipeline - West Region"));
        scm.description.write(buf, Some("This order processing pipeline handles all incoming purchase orders from the western distribution region, including validation, inventory reservation, payment processing, and fulfillment scheduling. It integrates with the warehouse management system and the shipping provider API."));
        scm.timestamp.write(buf, Some(1678886400));
        scm.score.write(buf, Some(99.9));
        scm.is_verified.write(buf, Some(true));
        scm.tags.write(buf, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15, 20, 25, 30, 50, 100, 200, 500, 1000, 9999]);
        scm.permissions.write(buf, &["read", "write", "admin", "execute", "audit", "export", "manage-users", "configure"]);
        scm.details.write_msg(buf, |buf, scm| {
            scm.label.write(buf, Some("nested-pipeline-stage-validation-config"));
            scm.count.write(buf, Some(1));
            scm.active.write(buf, Some(false));
        });
        scm.metrics.write(buf, &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0]);
        scm.flags.write(buf, &[true, false, true, true, false, false, true, true, false, true]);
    });
    const HISTORY: [(&str, i32, bool); 8] = [
        ("order-received", 150, true),
        ("validation-passed", 148, true),
        ("inventory-reserved", 145, true),
        ("payment-processed", 143, false),
        ("fulfillment-scheduled", 140, true),
        ("shipped", 130, true),
        ("delivered", 120, false),
        ("returned", 5, true),
    ];
    schema
        .history
        .write_msgs(buf, &HISTORY, |buf, scm, (label, count, active)| {
            scm.label.write(buf, Some(*label));
            scm.count.write(buf, Some(*count));
            scm.active.write(buf, Some(*active));
        });
    schema.related_ids.write(
        buf,
        &[
            "order-2024-001",
            "order-2024-002",
            "order-2024-003",
            "order-2024-004",
            "shipment-west-100",
            "shipment-west-101",
            "shipment-west-102",
            "invoice-5500",
            "invoice-5501",
            "return-auth-300",
        ],
    );
    schema.created_at.write(buf, Some(1670000000));
    schema.updated_at.write(buf, Some(1678886400));
    schema.priority.write(buf, Some(1.0));
    schema.is_test.write(buf, Some(false));
    schema.status.write(buf, Some(TSimpleEnum::Second));
}

fn prost_mixed_all() -> PMixedUsageMessage {
    PMixedUsageMessage {
        session_id: Some("sess-a]b1c2d3-e4f5-6789-abcd-ef0123456789".to_string()),
        user_id: Some(9999),
        client_version: Some("v2.14.3-beta.1+build.20240315".to_string()),
        small_payload: Some(PMixedSmallMessage {
            label: Some("inventory-check-primary-warehouse-us-east".to_string()),
            count: Some(42),
            active: Some(true),
        }),
        large_payload: Some(PMixedLargeMessage {
            id: Some("entity-987654321-abcdef".to_string()),
            name: Some("Production Order Processing Pipeline - West Region".to_string()),
            description: Some("This order processing pipeline handles all incoming purchase orders from the western distribution region, including validation, inventory reservation, payment processing, and fulfillment scheduling. It integrates with the warehouse management system and the shipping provider API.".to_string()),
            timestamp: Some(1678886400),
            score: Some(99.9),
            is_verified: Some(true),
            tags: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15, 20, 25, 30, 50, 100, 200, 500, 1000, 9999],
            permissions: vec!["read".into(), "write".into(), "admin".into(), "execute".into(), "audit".into(), "export".into(), "manage-users".into(), "configure".into()],
            details: Some(PMixedSmallMessage {
                label: Some("nested-pipeline-stage-validation-config".to_string()),
                count: Some(1),
                active: Some(false),
            }),
            metrics: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0],
            flags: vec![true, false, true, true, false, false, true, true, false, true],
        }),
        history: vec![
            PMixedSmallMessage { label: Some("order-received".into()), count: Some(150), active: Some(true) },
            PMixedSmallMessage { label: Some("validation-passed".into()), count: Some(148), active: Some(true) },
            PMixedSmallMessage { label: Some("inventory-reserved".into()), count: Some(145), active: Some(true) },
            PMixedSmallMessage { label: Some("payment-processed".into()), count: Some(143), active: Some(false) },
            PMixedSmallMessage { label: Some("fulfillment-scheduled".into()), count: Some(140), active: Some(true) },
            PMixedSmallMessage { label: Some("shipped".into()), count: Some(130), active: Some(true) },
            PMixedSmallMessage { label: Some("delivered".into()), count: Some(120), active: Some(false) },
            PMixedSmallMessage { label: Some("returned".into()), count: Some(5), active: Some(true) },
        ],
        related_ids: vec![
            "order-2024-001".into(), "order-2024-002".into(), "order-2024-003".into(), "order-2024-004".into(),
            "shipment-west-100".into(), "shipment-west-101".into(), "shipment-west-102".into(),
            "invoice-5500".into(), "invoice-5501".into(), "return-auth-300".into(),
        ],
        created_at: Some(1670000000),
        updated_at: Some(1678886400),
        priority: Some(1.0),
        is_test: Some(false),
        status: Some(prost_proto::SimpleEnum::Second as i32),
    }
}

fn bench_encode_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_realistic");

    let mut ref_buf = Vec::with_capacity(2048);
    tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut ref_buf));
    let size = ref_buf.len() as u64;
    group.throughput(Throughput::Bytes(size));

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut buf));
            black_box(buf.as_slice());
            buf.clear();
        });
    });
    let prost_msg = prost_mixed_all();
    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            prost_msg.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // Only the C++ arms consume this now: they are gated on re-serializing prost's bytes
    // exactly, which is what proves both runtimes encode the same message.
    #[cfg(feature = "cpp")]
    let prost_wire = prost_msg.encode_to_vec();

    // Field order differs — a downward buffer emits fields in the reverse of the order they
    // are written, which is legal — so this is checked by decoding, not by comparing bytes.
    // `test_revbuf_descending_matches_prost` pins the byte-level encoding separately.
    let mut rev_backing = vec![0u8; size as usize + 1024];
    let mut rb = tacky::RevBuf::new(&mut rev_backing);
    tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut rb));
    assert_eq!(
        PMixedUsageMessage::decode(rb.written()).unwrap(),
        prost_msg,
        "reverse writer output does not decode back to the same message"
    );
    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; size as usize + 1024];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut rb));
            black_box(rb.written());
        });
    });
    #[cfg(feature = "cpp")]
    bench_cpp_arms(&mut group, "cpp", cpp::MIXED, &prost_wire);

    group.finish();
}

// ---------------------------------------------------------------------------
// Decode: realistic message
// ---------------------------------------------------------------------------

/// Decode tacky wire bytes into a prost MixedUsageMessage struct, so both sides
/// do the same work: parse wire bytes and materialize owned Strings/Vecs.
fn tacky_decode_into_prost(wire: &[u8]) -> PMixedUsageMessage {
    use tacky_proto::example::{
        MixedLargeMessageField, MixedSmallMessageField, MixedUsageMessageField,
    };

    let mut msg = PMixedUsageMessage::default();
    for field in TMixedUsageMessage::decode(wire) {
        match field.unwrap() {
            MixedUsageMessageField::SessionId(v) => msg.session_id = Some(v.to_string()),
            MixedUsageMessageField::UserId(v) => msg.user_id = Some(v),
            MixedUsageMessageField::ClientVersion(v) => msg.client_version = Some(v.to_string()),
            MixedUsageMessageField::SmallPayload(fields) => {
                let mut sm = PMixedSmallMessage::default();
                for f in fields {
                    match f.unwrap() {
                        MixedSmallMessageField::Label(v) => sm.label = Some(v.to_string()),
                        MixedSmallMessageField::Count(v) => sm.count = Some(v),
                        MixedSmallMessageField::Active(v) => sm.active = Some(v),
                    }
                }
                msg.small_payload = Some(sm);
            }
            MixedUsageMessageField::LargePayload(fields) => {
                let mut lm = PMixedLargeMessage::default();
                for f in fields {
                    match f.unwrap() {
                        MixedLargeMessageField::Id(v) => lm.id = Some(v.to_string()),
                        MixedLargeMessageField::Name(v) => lm.name = Some(v.to_string()),
                        MixedLargeMessageField::Description(v) => {
                            lm.description = Some(v.to_string())
                        }
                        MixedLargeMessageField::Timestamp(v) => lm.timestamp = Some(v),
                        MixedLargeMessageField::Score(v) => lm.score = Some(v),
                        MixedLargeMessageField::IsVerified(v) => lm.is_verified = Some(v),
                        MixedLargeMessageField::Tags(iter) => {
                            lm.tags.extend(iter.map(|r| r.unwrap()));
                        }
                        MixedLargeMessageField::Permissions(v) => {
                            lm.permissions.push(v.to_string());
                        }
                        MixedLargeMessageField::Details(fields) => {
                            let mut sm = PMixedSmallMessage::default();
                            for f in fields {
                                match f.unwrap() {
                                    MixedSmallMessageField::Label(v) => {
                                        sm.label = Some(v.to_string())
                                    }
                                    MixedSmallMessageField::Count(v) => sm.count = Some(v),
                                    MixedSmallMessageField::Active(v) => sm.active = Some(v),
                                }
                            }
                            lm.details = Some(sm);
                        }
                        MixedLargeMessageField::Metrics(iter) => {
                            lm.metrics.extend(iter.map(|r| r.unwrap()));
                        }
                        MixedLargeMessageField::Flags(iter) => {
                            lm.flags.extend(iter.map(|r| r.unwrap()));
                        }
                    }
                }
                msg.large_payload = Some(lm);
            }
            MixedUsageMessageField::History(fields) => {
                let mut sm = PMixedSmallMessage::default();
                for f in fields {
                    match f.unwrap() {
                        MixedSmallMessageField::Label(v) => sm.label = Some(v.to_string()),
                        MixedSmallMessageField::Count(v) => sm.count = Some(v),
                        MixedSmallMessageField::Active(v) => sm.active = Some(v),
                    }
                }
                msg.history.push(sm);
            }
            MixedUsageMessageField::RelatedIds(v) => msg.related_ids.push(v.to_string()),
            MixedUsageMessageField::CreatedAt(v) => msg.created_at = Some(v),
            MixedUsageMessageField::UpdatedAt(v) => msg.updated_at = Some(v),
            MixedUsageMessageField::Priority(v) => msg.priority = Some(v),
            MixedUsageMessageField::IsTest(v) => msg.is_test = Some(v),
            MixedUsageMessageField::Status(v) => msg.status = Some(v.into()),
        }
    }
    msg
}

/// Parse-only counterpart to `tacky_decode_into_prost`. See `tacky_walk_pprof`.
fn tacky_walk_mixed(wire: &[u8]) -> u64 {
    use tacky_proto::example::{
        MixedLargeMessageField, MixedSmallMessageField, MixedUsageMessageField,
    };

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }
    macro_rules! small {
        ($fields:expr) => {
            for f in $fields {
                match f.unwrap() {
                    MixedSmallMessageField::Label(v) => add!(v.len()),
                    MixedSmallMessageField::Count(v) => add!(v),
                    MixedSmallMessageField::Active(v) => add!(v),
                }
            }
        };
    }

    for field in TMixedUsageMessage::decode(wire) {
        match field.unwrap() {
            MixedUsageMessageField::SessionId(v) => add!(v.len()),
            MixedUsageMessageField::UserId(v) => add!(v),
            MixedUsageMessageField::ClientVersion(v) => add!(v.len()),
            MixedUsageMessageField::SmallPayload(fields) => small!(fields),
            MixedUsageMessageField::LargePayload(fields) => {
                for f in fields {
                    match f.unwrap() {
                        MixedLargeMessageField::Id(v) => add!(v.len()),
                        MixedLargeMessageField::Name(v) => add!(v.len()),
                        MixedLargeMessageField::Description(v) => add!(v.len()),
                        MixedLargeMessageField::Timestamp(v) => add!(v),
                        MixedLargeMessageField::Score(v) => add!(v.to_bits()),
                        MixedLargeMessageField::IsVerified(v) => add!(v),
                        MixedLargeMessageField::Tags(iter) => {
                            for r in iter {
                                add!(r.unwrap());
                            }
                        }
                        MixedLargeMessageField::Permissions(v) => add!(v.len()),
                        MixedLargeMessageField::Details(fields) => small!(fields),
                        MixedLargeMessageField::Metrics(iter) => {
                            for r in iter {
                                add!(r.unwrap().to_bits());
                            }
                        }
                        MixedLargeMessageField::Flags(iter) => {
                            for r in iter {
                                add!(r.unwrap());
                            }
                        }
                    }
                }
            }
            MixedUsageMessageField::History(fields) => small!(fields),
            MixedUsageMessageField::RelatedIds(v) => add!(v.len()),
            MixedUsageMessageField::CreatedAt(v) => add!(v),
            MixedUsageMessageField::UpdatedAt(v) => add!(v),
            MixedUsageMessageField::Priority(v) => add!(v),
            MixedUsageMessageField::IsTest(v) => add!(v),
            MixedUsageMessageField::Status(v) => add!(i32::from(v)),
        }
    }
    acc
}

fn bench_decode_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_realistic");

    // Encode a reference message (all fields) — wire bytes are identical
    let mut wire = Vec::with_capacity(512);
    tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut wire));
    group.throughput(Throughput::Bytes(wire.len() as u64));

    // Verify both decoders produce the same result
    let tacky_result = tacky_decode_into_prost(&wire);
    let prost_result = PMixedUsageMessage::decode(wire.as_slice()).unwrap();
    assert_eq!(
        tacky_result, prost_result,
        "decode mismatch between tacky and prost"
    );

    group.bench_function("tacky", |b| {
        b.iter(|| {
            let msg = tacky_decode_into_prost(black_box(&wire));
            black_box(&msg);
        });
    });

    group.bench_function("prost", |b| {
        b.iter(|| {
            let msg = PMixedUsageMessage::decode(black_box(wire.as_slice())).unwrap();
            black_box(&msg);
        });
    });

    assert!(tacky_walk_mixed(&wire) != 0, "walker folded nothing");
    group.bench_function("tacky-walk", |b| {
        b.iter(|| black_box(tacky_walk_mixed(black_box(&wire))));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Encode: repeated strings (no nesting, no tack — pure tag+len+data writes)
// ---------------------------------------------------------------------------

fn bench_encode_repeated_strings(c: &mut Criterion) {
    use tacky_proto::example::RepeatedStrings as TRepeatedStrings;

    let mut group = c.benchmark_group("encode_repeated_strings");
    let sizes: &[(&str, usize)] = &[("10", 10), ("100", 100), ("1000", 1000)];

    for (name, size) in sizes {
        let data: Vec<&str> = (0..*size)
            .map(|i| match i % 5 {
                0 => "/api/v1/users/12345/profile",
                1 => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
                2 => "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
                3 => "hello",
                _ => "https://example.com/very/long/path/to/some/resource?query=param&foo=bar",
            })
            .collect();

        let prost_msg = prost_proto::RepeatedStrings {
            values: data.iter().map(|s| s.to_string()).collect(),
        };

        // Verify wire compatibility
        let mut tacky_wire = Vec::with_capacity(size * 64);
        TRepeatedStrings::schema()
            .values
            .write(&mut tacky_wire, &data);
        let mut prost_wire = Vec::new();
        prost_msg.encode(&mut prost_wire).unwrap();
        assert_eq!(
            tacky_wire, prost_wire,
            "repeated strings wire mismatch at size {name}"
        );

        group.throughput(Throughput::Bytes(prost_wire.len() as u64));

        group.bench_with_input(BenchmarkId::new("tacky", name), size, |b, _| {
            let mut buf = Vec::with_capacity(tacky_wire.len());
            b.iter(|| {
                TRepeatedStrings::schema().values.write(&mut buf, &data);
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        group.bench_with_input(BenchmarkId::new("prost", name), size, |b, _| {
            let mut buf = Vec::with_capacity(prost_wire.len());
            b.iter(|| {
                prost_msg.encode(&mut buf).unwrap();
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        #[cfg(feature = "cpp")]
        bench_cpp_arms(
            &mut group,
            &format!("cpp/{name}"),
            cpp::REPEATED_STRINGS,
            &prost_wire,
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Decode: repeated strings (no nesting — pure tag+len+data reads)
// ---------------------------------------------------------------------------

fn bench_decode_repeated_strings(c: &mut Criterion) {
    use tacky_proto::example::{RepeatedStrings as TRepeatedStrings, RepeatedStringsField};

    let mut group = c.benchmark_group("decode_repeated_strings");
    let sizes: &[(&str, usize)] = &[("10", 10), ("100", 100), ("1000", 1000)];

    for (name, size) in sizes {
        let data: Vec<String> = (0..*size)
            .map(|i| {
                match i % 5 {
                    0 => "/api/v1/users/12345/profile",
                    1 => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
                    2 => "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
                    3 => "hello",
                    _ => "https://example.com/very/long/path/to/some/resource?query=param&foo=bar",
                }
                .to_string()
            })
            .collect();

        let prost_msg = prost_proto::RepeatedStrings {
            values: data.clone(),
        };
        let wire = prost_msg.encode_to_vec();

        group.throughput(Throughput::Bytes(wire.len() as u64));

        group.bench_with_input(BenchmarkId::new("tacky", name), size, |b, _| {
            b.iter(|| {
                let mut strings = Vec::<String>::with_capacity(*size);
                for field in TRepeatedStrings::decode(black_box(&wire)) {
                    match field.unwrap() {
                        RepeatedStringsField::Values(v) => strings.push(v.to_string()),
                    }
                }
                black_box(&strings);
            });
        });

        group.bench_with_input(BenchmarkId::new("prost", name), size, |b, _| {
            b.iter(|| {
                let msg = prost_proto::RepeatedStrings::decode(black_box(wire.as_slice())).unwrap();
                black_box(&msg.values);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Pprof: a real profile, not a synthesised one
// ---------------------------------------------------------------------------

/// A real Go heap profile, checked in ungzipped (`testing/data/pprof_go_heap.pb`).
///
/// Vendored from `grafana/pyroscope`, `pkg/pprof/testdata/heap`, which is
/// AGPL-3.0-licensed testdata; it is used here as data only, not linked, and stays
/// under its own licence. Refresh instructions are in `scripts/gen_bench_fixtures.sh`.
///
/// Measured shape: 847 KB, 4 `sample_type`s and 4 values per sample, 11,951 samples
/// at a mean stack depth of **17.5** (median 19, max 32), 4,676 locations averaging
/// 1.22 `Line`s each (inlined frames, max 7) and all carrying an `address`, 2,405
/// functions, one mapping, and a 3,006-entry string table of 155 KB — mean symbol
/// length 52 B, max 117 B. Every sample carries exactly one numeric `bytes` label,
/// which is what Go's heap profiler emits.
///
/// Two things that follow from the size and are worth stating rather than implying:
/// 847 KB does not fit in the ~1.25 MB per-core L2 of the CI runners alongside an
/// output buffer, so this is not a cache-resident best case; and `runtime/pprof`
/// gzips its output, so proto encoding is a minority of the cost of writing a real
/// profile. What it is *not* a minority of is interesting on its own: Go's
/// `profileBuilder` hand-writes the wire format field by field rather than
/// populating a `Profile` message, because materialising one is too expensive.
/// That is tacky's premise, shipped in the Go standard library.
const PPROF_FIXTURE: &[u8] = include_bytes!("../data/pprof_go_heap.pb");

fn prost_pprof_profile() -> prost_pprof::Profile {
    prost_pprof::Profile::decode(PPROF_FIXTURE).expect("vendored pprof fixture does not decode")
}

/// Encodes a `Profile` from prost's owned structs, so both arms start from the same
/// value and only the writer differs. Fields go out in ascending tag order, which is
/// the order prost emits.
fn tacky_encode_pprof<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, p: &prost_pprof::Profile) {
    use tacky_pprof::perftools::profiles::Profile;

    let s = Profile::schema();

    s.sample_type.write_msgs(buf, &p.sample_type, |buf, vt, t| {
        vt.r#type.write(buf, t.r#type);
        vt.unit.write(buf, t.unit);
    });

    s.sample.write_msgs(buf, &p.sample, |buf, sample, sm| {
        sample.location_id.write(buf, &sm.location_id);
        sample.value.write(buf, &sm.value);
        sample.label.write_msgs(buf, &sm.label, |buf, l, lb| {
            l.key.write(buf, lb.key);
            l.str.write(buf, lb.str);
            l.num.write(buf, lb.num);
            l.num_unit.write(buf, lb.num_unit);
        });
    });

    s.mapping.write_msgs(buf, &p.mapping, |buf, m, mp| {
        m.id.write(buf, mp.id);
        m.memory_start.write(buf, mp.memory_start);
        m.memory_limit.write(buf, mp.memory_limit);
        m.file_offset.write(buf, mp.file_offset);
        m.filename.write(buf, mp.filename);
        m.build_id.write(buf, mp.build_id);
        m.has_functions.write(buf, mp.has_functions);
        m.has_filenames.write(buf, mp.has_filenames);
        m.has_line_numbers.write(buf, mp.has_line_numbers);
        m.has_inline_frames.write(buf, mp.has_inline_frames);
    });

    s.location.write_msgs(buf, &p.location, |buf, loc, lc| {
        loc.id.write(buf, lc.id);
        loc.mapping_id.write(buf, lc.mapping_id);
        loc.address.write(buf, lc.address);
        loc.line.write_msgs(buf, &lc.line, |buf, line, ln| {
            line.function_id.write(buf, ln.function_id);
            line.line.write(buf, ln.line);
            line.column.write(buf, ln.column);
        });
        loc.is_folded.write(buf, lc.is_folded);
    });

    s.function.write_msgs(buf, &p.function, |buf, f, fun| {
        f.id.write(buf, fun.id);
        f.name.write(buf, fun.name);
        f.system_name.write(buf, fun.system_name);
        f.filename.write(buf, fun.filename);
        f.start_line.write(buf, fun.start_line);
    });

    // One call with the whole table rather than one call per string: the repeated writer
    // owns element order, which is what a downward-growing buffer needs.
    s.string_table
        .write(buf, p.string_table.iter().map(|st| st.as_str()));

    s.drop_frames.write(buf, p.drop_frames);
    s.keep_frames.write(buf, p.keep_frames);
    s.time_nanos.write(buf, p.time_nanos);
    s.duration_nanos.write(buf, p.duration_nanos);
    if let Some(pt) = &p.period_type {
        s.period_type.write_msg(buf, |buf, vt| {
            vt.r#type.write(buf, pt.r#type);
            vt.unit.write(buf, pt.unit);
        });
    }
    s.period.write(buf, p.period);
    s.comment.write(buf, &p.comment);
    s.default_sample_type.write(buf, p.default_sample_type);
    s.doc_url.write(buf, p.doc_url);
}

fn bench_encode_pprof(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_pprof");

    let prost_msg = prost_pprof_profile();
    let mut ref_buf = Vec::with_capacity(PPROF_FIXTURE.len() + 4096);
    tacky_encode_pprof(tacky::AnyDir::from_mut(&mut ref_buf), &prost_msg);
    let size = ref_buf.len() as u64;
    group.throughput(Throughput::Bytes(size));

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            tacky_encode_pprof(tacky::AnyDir::from_mut(&mut buf), &prost_msg);
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            prost_msg.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // Only the C++ arms consume this now: they are gated on re-serializing prost's bytes
    // exactly, which is what proves both runtimes encode the same message.
    #[cfg(feature = "cpp")]
    let prost_wire = prost_msg.encode_to_vec();

    // This group is the single home for the two buffer diagnostics, because they report the
    // same thing on every corpus and 847 KB is where they report it most clearly.
    //
    // Forward into a fixed slice, so `tacky-slice` vs `tacky-rev` isolates the write
    // *direction* from the buffer kind, and `tacky-slice` vs `tacky` isolates the buffer kind
    // from everything else.
    group.bench_function("tacky-slice", |b| {
        let mut backing = vec![0u8; size as usize + 1024];
        b.iter(|| {
            let mut sb = tacky::SliceBuf::new(&mut backing);
            tacky_encode_pprof(tacky::AnyDir::from_mut(&mut sb), &prost_msg);
            black_box(sb.written());
        });
    });

    // Field order differs — a downward buffer emits fields in the reverse of the order they
    // are written, which is legal — so this is checked by decoding, not by comparing bytes.
    // `test_revbuf_descending_matches_prost` pins the byte-level encoding separately.
    let mut rev_backing = vec![0u8; size as usize + 1024];
    let mut rb = tacky::RevBuf::new(&mut rev_backing);
    tacky_encode_pprof(tacky::AnyDir::from_mut(&mut rb), &prost_msg);
    assert_eq!(
        prost_pprof::Profile::decode(rb.written()).unwrap(),
        prost_msg,
        "reverse writer output does not decode back to the same message"
    );
    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; size as usize + 1024];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode_pprof(tacky::AnyDir::from_mut(&mut rb), &prost_msg);
            black_box(rb.written());
        });
    });

    // What handing the result over as an owned, index-0 buffer costs: the reverse output
    // lives at the tail, so a `Vec<u8>`-shaped sink forces one compaction.
    group.bench_function("tacky-rev-owned", |b| {
        let mut backing = vec![0u8; size as usize + 1024];
        let mut out = Vec::with_capacity(size as usize + 1024);
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode_pprof(tacky::AnyDir::from_mut(&mut rb), &prost_msg);
            out.clear();
            out.extend_from_slice(rb.written());
            black_box(out.as_slice());
        });
    });
    #[cfg(feature = "cpp")]
    bench_cpp_arms(&mut group, "cpp-noutf8", cpp::PPROF_NO_UTF8, &prost_wire);

    group.finish();
}

fn tacky_decode_pprof_into_prost(wire: &[u8]) -> prost_pprof::Profile {
    use tacky_pprof::perftools::profiles::{
        FunctionField, LabelField, LineField, LocationField, MappingField, Profile, ProfileField,
        SampleField, ValueTypeField,
    };

    let mut msg = prost_pprof::Profile::default();
    for field in Profile::decode(wire) {
        match field.unwrap() {
            ProfileField::SampleType(fields) => {
                let mut vt = prost_pprof::ValueType::default();
                for f in fields {
                    match f.unwrap() {
                        ValueTypeField::Type(v) => vt.r#type = v,
                        ValueTypeField::Unit(v) => vt.unit = v,
                    }
                }
                msg.sample_type.push(vt);
            }
            ProfileField::Sample(fields) => {
                let mut s = prost_pprof::Sample::default();
                for f in fields {
                    match f.unwrap() {
                        SampleField::LocationId(iter) => {
                            s.location_id.extend(iter.map(|r| r.unwrap()));
                        }
                        SampleField::Value(iter) => {
                            s.value.extend(iter.map(|r| r.unwrap()));
                        }
                        SampleField::Label(fields) => {
                            let mut l = prost_pprof::Label::default();
                            for f in fields {
                                match f.unwrap() {
                                    LabelField::Key(v) => l.key = v,
                                    LabelField::Str(v) => l.str = v,
                                    LabelField::Num(v) => l.num = v,
                                    LabelField::NumUnit(v) => l.num_unit = v,
                                }
                            }
                            s.label.push(l);
                        }
                    }
                }
                msg.sample.push(s);
            }
            ProfileField::Mapping(fields) => {
                let mut m = prost_pprof::Mapping::default();
                for f in fields {
                    match f.unwrap() {
                        MappingField::Id(v) => m.id = v,
                        MappingField::MemoryStart(v) => m.memory_start = v,
                        MappingField::MemoryLimit(v) => m.memory_limit = v,
                        MappingField::FileOffset(v) => m.file_offset = v,
                        MappingField::Filename(v) => m.filename = v,
                        MappingField::BuildId(v) => m.build_id = v,
                        MappingField::HasFunctions(v) => m.has_functions = v,
                        MappingField::HasFilenames(v) => m.has_filenames = v,
                        MappingField::HasLineNumbers(v) => m.has_line_numbers = v,
                        MappingField::HasInlineFrames(v) => m.has_inline_frames = v,
                    }
                }
                msg.mapping.push(m);
            }
            ProfileField::Location(fields) => {
                let mut loc = prost_pprof::Location::default();
                for f in fields {
                    match f.unwrap() {
                        LocationField::Id(v) => loc.id = v,
                        LocationField::MappingId(v) => loc.mapping_id = v,
                        LocationField::Address(v) => loc.address = v,
                        LocationField::Line(fields) => {
                            let mut line = prost_pprof::Line::default();
                            for f in fields {
                                match f.unwrap() {
                                    LineField::FunctionId(v) => line.function_id = v,
                                    LineField::Line(v) => line.line = v,
                                    LineField::Column(v) => line.column = v,
                                }
                            }
                            loc.line.push(line);
                        }
                        LocationField::IsFolded(v) => loc.is_folded = v,
                    }
                }
                msg.location.push(loc);
            }
            ProfileField::Function(fields) => {
                let mut func = prost_pprof::Function::default();
                for f in fields {
                    match f.unwrap() {
                        FunctionField::Id(v) => func.id = v,
                        FunctionField::Name(v) => func.name = v,
                        FunctionField::SystemName(v) => func.system_name = v,
                        FunctionField::Filename(v) => func.filename = v,
                        FunctionField::StartLine(v) => func.start_line = v,
                    }
                }
                msg.function.push(func);
            }
            ProfileField::StringTable(v) => msg.string_table.push(v.to_string()),
            ProfileField::DropFrames(v) => msg.drop_frames = v,
            ProfileField::KeepFrames(v) => msg.keep_frames = v,
            ProfileField::TimeNanos(v) => msg.time_nanos = v,
            ProfileField::DurationNanos(v) => msg.duration_nanos = v,
            ProfileField::PeriodType(fields) => {
                let mut vt = prost_pprof::ValueType::default();
                for f in fields {
                    match f.unwrap() {
                        ValueTypeField::Type(v) => vt.r#type = v,
                        ValueTypeField::Unit(v) => vt.unit = v,
                    }
                }
                msg.period_type = Some(vt);
            }
            ProfileField::Period(v) => msg.period = v,
            ProfileField::Comment(iter) => {
                msg.comment.extend(iter.map(|r| r.unwrap()));
            }
            ProfileField::DefaultSampleType(v) => msg.default_sample_type = v,
            ProfileField::DocUrl(v) => msg.doc_url = v,
        }
    }
    msg
}

/// Walks every field of a pprof profile without materializing anything, folding each
/// value into an accumulator so nothing can be optimized away.
///
/// The arm above measures parse *plus* building prost's owned structs, and that
/// allocation dominates — which is why tacky and prost land within a few percent of
/// each other there. This one isolates the iterator and its tag dispatch, the part an
/// optimization would actually move. Borrowed strings are only measured, never copied.
///
/// No prost counterpart exists by construction: prost cannot decode without
/// materializing. Correctness is gated by `tacky_decode_pprof_into_prost`; this walker
/// only has to visit the same fields.
fn tacky_walk_pprof(wire: &[u8]) -> u64 {
    use tacky_pprof::perftools::profiles::{
        FunctionField, LabelField, LineField, LocationField, MappingField, Profile, ProfileField,
        SampleField, ValueTypeField,
    };

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }

    for field in Profile::decode(wire) {
        match field.unwrap() {
            ProfileField::SampleType(fields) | ProfileField::PeriodType(fields) => {
                for f in fields {
                    match f.unwrap() {
                        ValueTypeField::Type(v) => add!(v),
                        ValueTypeField::Unit(v) => add!(v),
                    }
                }
            }
            ProfileField::Sample(fields) => {
                for f in fields {
                    match f.unwrap() {
                        SampleField::LocationId(iter) => {
                            for r in iter {
                                add!(r.unwrap());
                            }
                        }
                        SampleField::Value(iter) => {
                            for r in iter {
                                add!(r.unwrap());
                            }
                        }
                        SampleField::Label(fields) => {
                            for f in fields {
                                match f.unwrap() {
                                    LabelField::Key(v) => add!(v),
                                    LabelField::Str(v) => add!(v),
                                    LabelField::Num(v) => add!(v),
                                    LabelField::NumUnit(v) => add!(v),
                                }
                            }
                        }
                    }
                }
            }
            ProfileField::Mapping(fields) => {
                for f in fields {
                    match f.unwrap() {
                        MappingField::Id(v) => add!(v),
                        MappingField::MemoryStart(v) => add!(v),
                        MappingField::MemoryLimit(v) => add!(v),
                        MappingField::FileOffset(v) => add!(v),
                        MappingField::Filename(v) => add!(v),
                        MappingField::BuildId(v) => add!(v),
                        MappingField::HasFunctions(v) => add!(v),
                        MappingField::HasFilenames(v) => add!(v),
                        MappingField::HasLineNumbers(v) => add!(v),
                        MappingField::HasInlineFrames(v) => add!(v),
                    }
                }
            }
            ProfileField::Location(fields) => {
                for f in fields {
                    match f.unwrap() {
                        LocationField::Id(v) => add!(v),
                        LocationField::MappingId(v) => add!(v),
                        LocationField::Address(v) => add!(v),
                        LocationField::Line(fields) => {
                            for f in fields {
                                match f.unwrap() {
                                    LineField::FunctionId(v) => add!(v),
                                    LineField::Line(v) => add!(v),
                                    LineField::Column(v) => add!(v),
                                }
                            }
                        }
                        LocationField::IsFolded(v) => add!(v),
                    }
                }
            }
            ProfileField::Function(fields) => {
                for f in fields {
                    match f.unwrap() {
                        FunctionField::Id(v) => add!(v),
                        FunctionField::Name(v) => add!(v),
                        FunctionField::SystemName(v) => add!(v),
                        FunctionField::Filename(v) => add!(v),
                        FunctionField::StartLine(v) => add!(v),
                    }
                }
            }
            // Borrowed: measured, not copied.
            ProfileField::StringTable(v) => add!(v.len()),
            ProfileField::DropFrames(v) => add!(v),
            ProfileField::KeepFrames(v) => add!(v),
            ProfileField::TimeNanos(v) => add!(v),
            ProfileField::DurationNanos(v) => add!(v),
            ProfileField::Period(v) => add!(v),
            ProfileField::Comment(iter) => {
                for r in iter {
                    add!(r.unwrap());
                }
            }
            ProfileField::DefaultSampleType(v) => add!(v),
            ProfileField::DocUrl(v) => add!(v),
        }
    }
    acc
}

fn bench_decode_pprof(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_pprof");

    // The fixture's own bytes, as Go wrote them, rather than a prost re-encoding: the
    // field order and packing choices of the real producer are part of what a decoder
    // has to deal with. Both arms parse the same buffer either way.
    let wire = PPROF_FIXTURE;
    group.throughput(Throughput::Bytes(wire.len() as u64));

    // Verify both decoders produce the same result
    let tacky_result = tacky_decode_pprof_into_prost(wire);
    let prost_result = prost_pprof::Profile::decode(wire).unwrap();
    assert_eq!(
        tacky_result, prost_result,
        "pprof decode mismatch between tacky and prost"
    );

    group.bench_function("tacky", |b| {
        b.iter(|| {
            let msg = tacky_decode_pprof_into_prost(black_box(wire));
            black_box(&msg);
        });
    });

    group.bench_function("prost", |b| {
        b.iter(|| {
            let msg = prost_pprof::Profile::decode(black_box(wire)).unwrap();
            black_box(&msg);
        });
    });

    // Parse only, no materialization — see `tacky_walk_pprof`. Not comparable to the
    // arms above; it is the baseline a dispatch change would move.
    assert!(
        tacky_walk_pprof(wire) != 0,
        "walker folded nothing; it is not visiting the profile"
    );
    group.bench_function("tacky-walk", |b| {
        b.iter(|| black_box(tacky_walk_pprof(black_box(wire))));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Access log: string-heavy messages, 100 entries per batch
// ---------------------------------------------------------------------------
//
// This corpus is *plausible*, not real, and unlike pprof there is no fixture to
// replace it with — nobody publishes access-log payloads. The schema is shaped after
// Envoy's `envoy.data.accesslog.v3.HTTPAccessLogEntry`, which is the one access-log
// proto that is actually deployed at scale: headers in a `map`, per-connection detail
// in a `Common` submessage rather than inline, TLS detail below that. What it does not
// borrow from Envoy is the well-known types — real ALS keeps timings in
// `google.protobuf.Duration` and `Timestamp` — because tacky has no specialisation for
// a message that wraps a single scalar, and benching one would measure a gap this crate
// has not claimed to close. Timings here are plain `int64` micros.
//
// It earns its place for one thing the other corpora do not have: `request_headers` is
// the only `map` field in the comparative suite, so this is where the map-entry writer
// is measured against prost and C++ rather than only against itself in
// `benches/regression.rs`.

const NUM_LOG_ENTRIES: usize = 100;

static PATHS: &[&str] = &[
    "/",
    "/api/v1/users",
    "/api/v1/users/12345/profile",
    "/api/v1/orders",
    "/api/v1/orders/98765/status",
    "/api/v1/search?q=rust+protobuf",
    "/static/js/app.bundle.min.js",
    "/static/css/main.css",
    "/health",
    "/api/v2/inventory/items",
    "/login",
    "/api/v1/notifications",
    "/favicon.ico",
    "/api/v1/products/categories/electronics",
    "/robots.txt",
];

static QUERIES: &[&str] = &[
    "",
    "page=1&limit=20",
    "sort=created_at&order=desc",
    "q=search+term&lang=en",
    "filter=active&category=3",
    "",
    "",
    "v=2.1.0",
    "",
    "warehouse=us-east-1&sku=ABC",
    "redirect=/dashboard",
    "since=2024-01-01&unread=true",
    "",
    "brand=acme&min_price=10&max_price=100",
    "",
];

static USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/120.0.0.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 Mobile",
    "curl/8.4.0",
    "Go-http-client/2.0",
    "python-requests/2.31.0",
];

static REFERERS: &[&str] = &[
    "",
    "https://www.google.com/",
    "https://myapp.example.com/dashboard",
    "https://myapp.example.com/api/docs",
    "",
    "",
];

/// Request headers, in the proportion a browser actually sends them. An entry takes the
/// first `5 + i % 8` of these, so header counts run 5..12 — a real request carries
/// roughly that many.
static HEADERS: &[(&str, &str)] = &[
    ("accept", "application/json, text/plain, */*"),
    ("accept-encoding", "gzip, deflate, br"),
    ("accept-language", "en-GB,en;q=0.9"),
    ("host", "myapp.example.com"),
    ("connection", "keep-alive"),
    (
        "authorization",
        "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
    ),
    ("x-request-id", "a]b1c2d3-e4f5-6789-abcd-ef0123456789"),
    ("x-forwarded-for", "203.0.113.42, 10.0.0.7"),
    ("x-forwarded-proto", "https"),
    ("cache-control", "no-cache"),
    ("cookie", "session=8f2c1a7b4e9d; consent=1; theme=dark"),
    ("origin", "https://myapp.example.com"),
];

/// Pre-computed data for the access log encode benchmark.
struct AccessLogEncodeData {
    remote_addrs: Vec<String>,
    methods: Vec<i32>,
    statuses: Vec<i32>,
    response_bytes: Vec<i64>,
    durations: Vec<i64>,
    timestamps: Vec<i64>,
    /// One header map per entry, built outside the hot loop like everything else here.
    /// `BTreeMap` on both sides — see the `btree_map` note in `build.rs`.
    headers: Vec<std::collections::BTreeMap<String, String>>,
    /// The `Common` submessages, prebuilt for the same reason: tacky's writer reads them
    /// field by field, so building one per iteration would put `String` allocation into
    /// the encode measurement.
    commons: Vec<prost_accesslog::Common>,
}

fn accesslog_encode_data() -> AccessLogEncodeData {
    let durations: Vec<i64> = (0..NUM_LOG_ENTRIES)
        .map(|i| 500 + (i as i64 * 317) % 50_000) // 0.5ms to 50ms
        .collect();
    let remote_addrs: Vec<String> = (0..NUM_LOG_ENTRIES)
        .map(|i| {
            format!(
                "10.{}.{}.{}",
                (i / 256) % 256,
                (i * 7) % 256,
                (i * 13 + 1) % 256
            )
        })
        .collect();

    // Weighted: mostly 200, some 301/304/404/500
    let status_pattern: &[i32] = &[
        200, 200, 200, 200, 200, 200, 200, 301, 304, 304, 404, 404, 500,
    ];
    let method_pattern: &[i32] = &[1, 1, 1, 1, 1, 2, 2, 4, 1, 6]; // mostly GET, some POST/DELETE/HEAD

    AccessLogEncodeData {
        remote_addrs,
        methods: (0..NUM_LOG_ENTRIES)
            .map(|i| method_pattern[i % method_pattern.len()])
            .collect(),
        statuses: (0..NUM_LOG_ENTRIES)
            .map(|i| status_pattern[i % status_pattern.len()])
            .collect(),
        response_bytes: (0..NUM_LOG_ENTRIES)
            .map(|i| match i % 5 {
                0 => 45_000,  // HTML page
                1 => 256,     // API JSON
                2 => 1_200,   // small JSON
                3 => 350_000, // JS bundle
                _ => 0,       // redirect/empty
            })
            .collect(),
        timestamps: (0..NUM_LOG_ENTRIES)
            .map(|i| 1_700_000_000_000_000 + i as i64 * 15_000) // ~15µs apart
            .collect(),
        commons: (0..NUM_LOG_ENTRIES)
            .map(|i| accesslog_common(i, durations[i]))
            .collect(),
        durations,
        headers: (0..NUM_LOG_ENTRIES)
            .map(|i| {
                HEADERS[..5 + i % 8]
                    .iter()
                    .map(|(n, v)| (n.to_string(), v.to_string()))
                    .collect()
            })
            .collect(),
    }
}

/// The `Common` submessage, identical on both sides. Values vary with `i` so the
/// nested-message path is not encoding the same bytes 100 times over.
fn accesslog_common(i: usize, duration: i64) -> prost_accesslog::Common {
    prost_accesslog::Common {
        upstream_host: format!("10.4.{}.{}:8080", i % 8, 20 + i % 40),
        upstream_cluster: "orders-v2-canary".into(),
        route_name: "orders_route".into(),
        upstream_connect_micros: 300 + (i as i64 * 71) % 4_000,
        time_to_first_byte_micros: duration / 2,
        request_bytes: 180 + (i as i64 * 37) % 2_000,
        sampled: i % 4 == 0,
        // TLS is only reported on the connections that had it: HTTP/2 over plaintext
        // to a sidecar is common enough that leaving it unset on some entries is the
        // realistic shape, not an omission.
        tls: (i % 3 != 0).then(|| prost_accesslog::TlsProperties {
            version: "TLSv1.3".into(),
            cipher_suite: "TLS_AES_128_GCM_SHA256".into(),
            sni: "myapp.example.com".into(),
            resumed: i % 5 == 0,
        }),
    }
}

fn tacky_encode_accesslog<B: tacky::WriteBuf>(
    buf: &mut tacky::AnyDir<B>,
    data: &AccessLogEncodeData,
) {
    use tacky_accesslog::accesslog::{AccessLog, HttpMethod};

    let s = AccessLog::schema();
    s.entries.write_msgs(buf, 0..NUM_LOG_ENTRIES, |buf, e, i| {
        e.remote_addr.write(buf, data.remote_addrs[i].as_str());
        e.method.write(buf, HttpMethod::from(data.methods[i]));
        e.path.write(buf, PATHS[i % PATHS.len()]);
        e.query.write(buf, QUERIES[i % QUERIES.len()]);
        e.status.write(buf, data.statuses[i]);
        e.response_bytes.write(buf, data.response_bytes[i]);
        e.duration_micros.write(buf, data.durations[i]);
        e.user_agent.write(buf, USER_AGENTS[i % USER_AGENTS.len()]);
        e.referer.write(buf, REFERERS[i % REFERERS.len()]);
        e.timestamp.write(buf, data.timestamps[i]);
        e.host.write(buf, "myapp.example.com");
        e.protocol.write(buf, "HTTP/2");
        e.request_headers.write(buf, &data.headers[i]);
        let c = &data.commons[i];
        e.common.write_msg(buf, |buf, cm| {
            cm.upstream_host.write(buf, c.upstream_host.as_str());
            cm.upstream_cluster.write(buf, c.upstream_cluster.as_str());
            cm.route_name.write(buf, c.route_name.as_str());
            cm.upstream_connect_micros
                .write(buf, c.upstream_connect_micros);
            cm.time_to_first_byte_micros
                .write(buf, c.time_to_first_byte_micros);
            cm.request_bytes.write(buf, c.request_bytes);
            cm.sampled.write(buf, c.sampled);
            if let Some(t) = &c.tls {
                cm.tls.write_msg(buf, |buf, tp| {
                    tp.version.write(buf, t.version.as_str());
                    tp.cipher_suite.write(buf, t.cipher_suite.as_str());
                    tp.sni.write(buf, t.sni.as_str());
                    tp.resumed.write(buf, t.resumed);
                });
            }
        });
    });
    s.server_id.write(buf, "web-prod-us-east-1a-i-0abc123def");
    s.batch_timestamp.write(buf, 1_700_000_000_000_000i64);
}

fn prost_accesslog_msg(data: &AccessLogEncodeData) -> prost_accesslog::AccessLog {
    let entries: Vec<prost_accesslog::Entry> = (0..NUM_LOG_ENTRIES)
        .map(|i| {
            let q = QUERIES[i % QUERIES.len()];
            let r = REFERERS[i % REFERERS.len()];
            prost_accesslog::Entry {
                remote_addr: data.remote_addrs[i].clone(),
                method: data.methods[i],
                path: PATHS[i % PATHS.len()].into(),
                query: q.into(),
                status: data.statuses[i],
                response_bytes: data.response_bytes[i],
                duration_micros: data.durations[i],
                user_agent: USER_AGENTS[i % USER_AGENTS.len()].into(),
                referer: r.into(),
                timestamp: data.timestamps[i],
                host: "myapp.example.com".into(),
                protocol: "HTTP/2".into(),
                request_headers: data.headers[i].clone(),
                common: Some(data.commons[i].clone()),
            }
        })
        .collect();

    prost_accesslog::AccessLog {
        entries,
        server_id: "web-prod-us-east-1a-i-0abc123def".into(),
        batch_timestamp: 1_700_000_000_000_000,
    }
}

fn bench_encode_accesslog(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_accesslog");

    let data = accesslog_encode_data();
    let mut ref_buf = Vec::with_capacity(32768);
    tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut ref_buf), &data);
    let size = ref_buf.len() as u64;
    group.throughput(Throughput::Bytes(size));

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut buf), &data);
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    let prost_msg = prost_accesslog_msg(&data);
    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            prost_msg.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // Only the C++ arms consume this now: they are gated on re-serializing prost's bytes
    // exactly, which is what proves both runtimes encode the same message.
    #[cfg(feature = "cpp")]
    let prost_wire = prost_msg.encode_to_vec();

    // Field order differs — a downward buffer emits fields in the reverse of the order they
    // are written, which is legal — so this is checked by decoding, not by comparing bytes.
    // `test_revbuf_descending_matches_prost` pins the byte-level encoding separately.
    let mut rev_backing = vec![0u8; size as usize + 1024];
    let mut rb = tacky::RevBuf::new(&mut rev_backing);
    tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut rb), &data);
    assert_eq!(
        prost_accesslog::AccessLog::decode(rb.written()).unwrap(),
        prost_msg,
        "reverse writer output does not decode back to the same message"
    );
    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; size as usize + 1024];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut rb), &data);
            black_box(rb.written());
        });
    });

    // The cold-buffer pair, and the only place in the suite that measures it: every other
    // arm reuses a warm buffer, which is the right steady state for an exporter but hides
    // what the first export costs. A fresh `Vec` per iteration pays the reallocation path
    // from zero capacity, plus one deallocation.
    //
    // Both encoders run it because they reach a cold buffer differently: prost reserves
    // `encoded_len()` up front and allocates once, exactly right, while tacky refuses to
    // compute that length and doubles its way there. There is no reverse counterpart —
    // `SliceBuf` and `RevBuf` are fixed-capacity and panic in `grow`.
    group.bench_function("tacky-grow", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut buf), &data);
            black_box(buf.as_slice());
        });
    });
    group.bench_function("prost-grow", |b| {
        b.iter(|| {
            let mut buf = Vec::new();
            prost_msg.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
        });
    });
    // `request_headers` is a map, so the C++ arm is gated on decoding back to the same
    // message rather than on byte equality; see `bench_cpp_arms_gated`.
    #[cfg(feature = "cpp")]
    cpp_arms::bench_cpp_arms_gated(
        &mut group,
        "cpp-noutf8",
        cpp::ACCESSLOG_NO_UTF8,
        &prost_wire,
        |cpp_wire| {
            assert_eq!(
                prost_accesslog::AccessLog::decode(cpp_wire).unwrap(),
                prost_msg,
                "cpp-noutf8: C++ re-serialization does not decode to the same message"
            );
        },
    );

    group.finish();
}

fn tacky_decode_accesslog_into_prost(wire: &[u8]) -> prost_accesslog::AccessLog {
    use tacky_accesslog::accesslog::{
        AccessLog, AccessLogField, CommonField, EntryField, TlsPropertiesField,
    };

    let mut msg = prost_accesslog::AccessLog::default();
    for field in AccessLog::decode(wire) {
        match field.unwrap() {
            AccessLogField::Entries(fields) => {
                let mut e = prost_accesslog::Entry::default();
                for f in fields {
                    match f.unwrap() {
                        EntryField::RemoteAddr(v) => e.remote_addr = v.to_string(),
                        EntryField::Method(v) => e.method = i32::from(v),
                        EntryField::Path(v) => e.path = v.to_string(),
                        EntryField::Query(v) => e.query = v.to_string(),
                        EntryField::Status(v) => e.status = v,
                        EntryField::ResponseBytes(v) => e.response_bytes = v,
                        EntryField::DurationMicros(v) => e.duration_micros = v,
                        EntryField::UserAgent(v) => e.user_agent = v.to_string(),
                        EntryField::Referer(v) => e.referer = v.to_string(),
                        EntryField::Timestamp(v) => e.timestamp = v,
                        EntryField::Host(v) => e.host = v.to_string(),
                        EntryField::Protocol(v) => e.protocol = v.to_string(),
                        // One map entry per yield. The value is an `Option` because a
                        // map entry may legally omit it, in which case the type's
                        // default applies.
                        EntryField::RequestHeaders((k, v)) => {
                            e.request_headers
                                .insert(k.to_string(), v.unwrap_or_default().to_string());
                        }
                        EntryField::Common(fields) => {
                            let c = e.common.get_or_insert_with(Default::default);
                            for f in fields {
                                match f.unwrap() {
                                    CommonField::UpstreamHost(v) => c.upstream_host = v.to_string(),
                                    CommonField::UpstreamCluster(v) => {
                                        c.upstream_cluster = v.to_string()
                                    }
                                    CommonField::RouteName(v) => c.route_name = v.to_string(),
                                    CommonField::UpstreamConnectMicros(v) => {
                                        c.upstream_connect_micros = v
                                    }
                                    CommonField::TimeToFirstByteMicros(v) => {
                                        c.time_to_first_byte_micros = v
                                    }
                                    CommonField::RequestBytes(v) => c.request_bytes = v,
                                    CommonField::Sampled(v) => c.sampled = v,
                                    CommonField::Tls(fields) => {
                                        let t = c.tls.get_or_insert_with(Default::default);
                                        for f in fields {
                                            match f.unwrap() {
                                                TlsPropertiesField::Version(v) => {
                                                    t.version = v.to_string()
                                                }
                                                TlsPropertiesField::CipherSuite(v) => {
                                                    t.cipher_suite = v.to_string()
                                                }
                                                TlsPropertiesField::Sni(v) => t.sni = v.to_string(),
                                                TlsPropertiesField::Resumed(v) => t.resumed = v,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                msg.entries.push(e);
            }
            AccessLogField::ServerId(v) => msg.server_id = v.to_string(),
            AccessLogField::BatchTimestamp(v) => msg.batch_timestamp = v,
        }
    }
    msg
}

/// Parse-only counterpart to `tacky_decode_accesslog_into_prost`. See `tacky_walk_pprof`.
fn tacky_walk_accesslog(wire: &[u8]) -> u64 {
    use tacky_accesslog::accesslog::{
        AccessLog, AccessLogField, CommonField, EntryField, TlsPropertiesField,
    };

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }

    for field in AccessLog::decode(wire) {
        match field.unwrap() {
            AccessLogField::Entries(fields) => {
                for f in fields {
                    match f.unwrap() {
                        EntryField::RemoteAddr(v) => add!(v.len()),
                        EntryField::Method(v) => add!(i32::from(v)),
                        EntryField::Path(v) => add!(v.len()),
                        EntryField::Query(v) => add!(v.len()),
                        EntryField::Status(v) => add!(v),
                        EntryField::ResponseBytes(v) => add!(v),
                        EntryField::DurationMicros(v) => add!(v),
                        EntryField::UserAgent(v) => add!(v.len()),
                        EntryField::Referer(v) => add!(v.len()),
                        EntryField::Timestamp(v) => add!(v),
                        EntryField::Host(v) => add!(v.len()),
                        EntryField::Protocol(v) => add!(v.len()),
                        EntryField::RequestHeaders((k, v)) => {
                            add!(k.len());
                            add!(v.map_or(0, str::len));
                        }
                        EntryField::Common(fields) => {
                            for f in fields {
                                match f.unwrap() {
                                    CommonField::UpstreamHost(v) => add!(v.len()),
                                    CommonField::UpstreamCluster(v) => add!(v.len()),
                                    CommonField::RouteName(v) => add!(v.len()),
                                    CommonField::UpstreamConnectMicros(v) => add!(v),
                                    CommonField::TimeToFirstByteMicros(v) => add!(v),
                                    CommonField::RequestBytes(v) => add!(v),
                                    CommonField::Sampled(v) => add!(v),
                                    CommonField::Tls(fields) => {
                                        for f in fields {
                                            match f.unwrap() {
                                                TlsPropertiesField::Version(v) => add!(v.len()),
                                                TlsPropertiesField::CipherSuite(v) => add!(v.len()),
                                                TlsPropertiesField::Sni(v) => add!(v.len()),
                                                TlsPropertiesField::Resumed(v) => add!(v),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            AccessLogField::ServerId(v) => add!(v.len()),
            AccessLogField::BatchTimestamp(v) => add!(v),
        }
    }
    acc
}

fn bench_decode_accesslog(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_accesslog");

    let data = accesslog_encode_data();
    let prost_msg = prost_accesslog_msg(&data);
    let wire = prost_msg.encode_to_vec();
    group.throughput(Throughput::Bytes(wire.len() as u64));

    // Verify both decoders produce the same result
    let tacky_result = tacky_decode_accesslog_into_prost(&wire);
    let prost_result = prost_accesslog::AccessLog::decode(wire.as_slice()).unwrap();
    assert_eq!(
        tacky_result, prost_result,
        "accesslog decode mismatch between tacky and prost"
    );

    group.bench_function("tacky", |b| {
        b.iter(|| {
            let msg = tacky_decode_accesslog_into_prost(black_box(&wire));
            black_box(&msg);
        });
    });

    group.bench_function("prost", |b| {
        b.iter(|| {
            let msg = prost_accesslog::AccessLog::decode(black_box(wire.as_slice())).unwrap();
            black_box(&msg);
        });
    });

    assert!(tacky_walk_accesslog(&wire) != 0, "walker folded nothing");
    group.bench_function("tacky-walk", |b| {
        b.iter(|| black_box(tacky_walk_accesslog(black_box(&wire))));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Rotating: three message types per iteration, not one hammered
// ---------------------------------------------------------------------------
//
// Every other group in this suite encodes one message type in a tight loop, so its
// writer, its branch history and its inlined code stay resident for the whole
// measurement. That is close to what a telemetry exporter really does — it ships
// `ResourceSpans` over and over — so this group is **not** here as a realism fix. It is
// here as a control: if the isolated numbers were partly an artifact of one hot writer,
// the ratios between arms will move when three writers take turns.
//
// Fleetbench does the same thing for a stronger reason (Google's fleet runs thousands of
// message types in one process, and its `ProtoLifecycle` deliberately interleaves twenty
// so nothing stays hot). Three is what this file can offer, and the three are deliberately
// unalike in both shape and writer:
//
// - `mixed` — 1.2 KB of scalars and small submessages.
// - `accesslog` — 62 KB of strings, a map and a nested submessage.
// - `fds registry` — 126 KB of short strings, deep nesting and packed `int32` arrays,
//   through the writer shared with `benches/descriptor_set.rs`.
//
// **pprof is deliberately excluded**: at 847 KB it would be 93% of the blend, leaving the
// group unable to detect a rotation effect on anything else. These three run 1 : 33 : 67,
// which is as balanced as this suite's corpora get without shrinking one of them.
//
// One iteration encodes all three and throughput is the summed bytes. The number to read
// is not the blended rate but whether each arm's *ratio* to the others matches what the
// isolated groups report.
fn bench_encode_rotating(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_rotating");

    let log_data = accesslog_encode_data();
    let prost_mixed = prost_mixed_all();
    let prost_log = prost_accesslog_msg(&log_data);
    let fds = prost_types::FileDescriptorSet::decode(FDS_REGISTRY)
        .expect("checked-in fixture decodes as FileDescriptorSet");

    // Size each type once, so the throughput denominator is the real blend.
    let mut probe = Vec::new();
    tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut probe));
    let mixed_len = probe.len();
    probe.clear();
    tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut probe), &log_data);
    let log_len = probe.len();
    probe.clear();
    fds_writer::tacky_encode(tacky::AnyDir::from_mut(&mut probe), &fds);
    let fds_len = probe.len();
    let total = mixed_len + log_len + fds_len;
    let cap = total + 4096;
    println!(
        "encode_rotating: mixed {mixed_len} B + accesslog {log_len} B + fds {fds_len} B \
         = {total} B per iteration"
    );
    group.throughput(Throughput::Bytes(total as u64));

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut buf));
            black_box(buf.as_slice());
            buf.clear();
            tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut buf), &log_data);
            black_box(buf.as_slice());
            buf.clear();
            fds_writer::tacky_encode(tacky::AnyDir::from_mut(&mut buf), &fds);
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; cap];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode_mixed_all(tacky::AnyDir::from_mut(&mut rb));
            black_box(rb.written());
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode_accesslog(tacky::AnyDir::from_mut(&mut rb), &log_data);
            black_box(rb.written());
            let mut rb = tacky::RevBuf::new(&mut backing);
            fds_writer::tacky_encode(tacky::AnyDir::from_mut(&mut rb), &fds);
            black_box(rb.written());
        });
    });

    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            prost_mixed.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
            prost_log.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
            fds.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // The C++ arms cannot go through `bench_cpp_arms`, which times one kind; the point here
    // is three kinds in one iteration. Each handle is seeded from prost's bytes exactly as
    // that helper does, and `byte_size` is called once up front to prime the size cache the
    // `-cached` arm reuses.
    #[cfg(feature = "cpp")]
    {
        let handles = [
            cpp::Msg::parse(cpp::MIXED, &prost_mixed.encode_to_vec()),
            cpp::Msg::parse(cpp::ACCESSLOG_NO_UTF8, &prost_log.encode_to_vec()),
            cpp::Msg::parse(cpp::FILE_DESCRIPTOR_SET, &fds.encode_to_vec()),
        ];
        for h in &handles {
            h.byte_size();
        }
        group.bench_function("cpp", |b| {
            let mut buf = Vec::with_capacity(cap);
            b.iter(|| {
                for h in &handles {
                    h.serialize(&mut buf);
                    black_box(buf.as_slice());
                    buf.clear();
                }
            });
        });
        group.bench_function("cpp-cached", |b| {
            let mut buf = Vec::with_capacity(cap);
            b.iter(|| {
                for h in &handles {
                    h.serialize_cached(&mut buf);
                    black_box(buf.as_slice());
                    buf.clear();
                }
            });
        });
    }

    group.finish();
}

#[cfg(not(feature = "cpp"))]
criterion_group!(
    benches,
    bench_encode_realistic,
    bench_encode_repeated_strings,
    bench_decode_realistic,
    bench_decode_repeated_strings,
    bench_encode_pprof,
    bench_decode_pprof,
    bench_encode_accesslog,
    bench_decode_accesslog,
    bench_encode_rotating,
);

#[cfg(feature = "cpp")]
criterion_group!(
    benches,
    bench_ffi_overhead,
    bench_encode_realistic,
    bench_encode_repeated_strings,
    bench_decode_realistic,
    bench_decode_repeated_strings,
    bench_encode_pprof,
    bench_decode_pprof,
    bench_encode_accesslog,
    bench_decode_accesslog,
    bench_encode_rotating,
);
criterion_main!(benches);
