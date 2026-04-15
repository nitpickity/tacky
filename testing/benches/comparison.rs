//! Public benchmarks: tacky vs prost, apples-to-apples.
//!
//! These are the numbers you put in the README. Both sides encode identical data
//! and produce identical wire output. Prost messages are pre-built so we measure
//! pure encoding/decoding speed, not allocation.

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

use prost_proto::{
    MixedLargeMessage as PMixedLargeMessage, MixedSmallMessage as PMixedSmallMessage,
    MixedUsageMessage as PMixedUsageMessage,
};
use tacky_proto::example::{MixedUsageMessage as TMixedUsageMessage, SimpleEnum as TSimpleEnum};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Encode a full MixedUsageMessage with tacky, all fields set.
fn tacky_encode_mixed_all(buf: &mut Vec<u8>) {
    let schema = TMixedUsageMessage::default();
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
    for (label, count, active) in [
        ("order-received", 150, true),
        ("validation-passed", 148, true),
        ("inventory-reserved", 145, true),
        ("payment-processed", 143, false),
        ("fulfillment-scheduled", 140, true),
        ("shipped", 130, true),
        ("delivered", 120, false),
        ("returned", 5, true),
    ] {
        schema.history.write_msg(buf, |buf, scm| {
            scm.label.write(buf, Some(label));
            scm.count.write(buf, Some(count));
            scm.active.write(buf, Some(active));
        });
    }
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
    tacky_encode_mixed_all(&mut ref_buf);
    let size = ref_buf.len() as u64;
    group.throughput(Throughput::Bytes(size));

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            tacky_encode_mixed_all(&mut buf);
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

fn bench_decode_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_realistic");

    // Encode a reference message (all fields) — wire bytes are identical
    let mut wire = Vec::with_capacity(512);
    tacky_encode_mixed_all(&mut wire);
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
        TRepeatedStrings::default()
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
                TRepeatedStrings::default().values.write(&mut buf, &data);
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
// Pprof: realistic profiling data (~10KB+ messages)
// ---------------------------------------------------------------------------

const NUM_FUNCTIONS: usize = 30;
const NUM_LOCATIONS: usize = 50;
const NUM_SAMPLES: usize = 200;
const LOCS_PER_SAMPLE: usize = 4;

fn pprof_string_table() -> Vec<String> {
    let mut st = Vec::with_capacity(7 + NUM_FUNCTIONS + 5);
    st.push(String::new()); // 0: required empty
    st.push("cpu".into()); // 1
    st.push("nanoseconds".into()); // 2
    st.push("thread".into()); // 3
    st.push("id".into()); // 4
    st.push("/usr/bin/myapp".into()); // 5: mapping filename
    st.push("abc123def456".into()); // 6: build id
    for i in 0..NUM_FUNCTIONS {
        st.push(format!("github.com/myorg/myapp/pkg.Function{i}"));
    }
    for i in 0..5 {
        st.push(format!("/home/user/src/myapp/pkg/file{i}.go"));
    }
    st
}

/// Pre-computed data for the pprof encode benchmark, built once outside the hot loop.
struct PprofEncodeData {
    /// Per-sample location_id arrays, pre-computed.
    sample_locs: Vec<[u64; LOCS_PER_SAMPLE]>,
    /// Per-sample value arrays, pre-computed.
    sample_values: Vec<[i64; 2]>,
    /// String table as owned strings (built once).
    string_table: Vec<String>,
}

fn pprof_encode_data() -> PprofEncodeData {
    let sample_locs: Vec<[u64; LOCS_PER_SAMPLE]> = (0..NUM_SAMPLES)
        .map(|i| std::array::from_fn(|j| ((i * LOCS_PER_SAMPLE + j) % NUM_LOCATIONS + 1) as u64))
        .collect();
    let sample_values: Vec<[i64; 2]> = (0..NUM_SAMPLES)
        .map(|i| [(i as i64 + 1) * 10_000_000, 1])
        .collect();
    PprofEncodeData {
        sample_locs,
        sample_values,
        string_table: pprof_string_table(),
    }
}

fn tacky_encode_pprof(buf: &mut Vec<u8>, data: &PprofEncodeData) {
    use tacky_pprof::perftools::profiles::Profile;

    let s = Profile::default();

    // sample_type: cpu / nanoseconds
    s.sample_type.write_msg(buf, |buf, vt| {
        vt.r#type.write(buf, 1i64);
        vt.unit.write(buf, 2i64);
    });

    // samples
    for i in 0..NUM_SAMPLES {
        s.sample.write_msg(buf, |buf, sample| {
            sample.location_id.write(buf, &data.sample_locs[i]);
            sample.value.write(buf, &data.sample_values[i]);
            sample.label.write_msg(buf, |buf, l| {
                l.key.write(buf, 3i64);
                l.num.write(buf, (i % 8) as i64);
                l.num_unit.write(buf, 4i64);
            });
        });
    }

    // mapping
    s.mapping.write_msg(buf, |buf, m| {
        m.id.write(buf, 1u64);
        m.memory_start.write(buf, 0x400000u64);
        m.memory_limit.write(buf, 0x800000u64);
        m.file_offset.write(buf, 0u64); // won't be written (proto3 default)
        m.filename.write(buf, 5i64);
        m.build_id.write(buf, 6i64);
        m.has_functions.write(buf, true);
        m.has_filenames.write(buf, true);
        m.has_line_numbers.write(buf, true);
        m.has_inline_frames.write(buf, true);
    });

    // locations
    for i in 1..=NUM_LOCATIONS {
        s.location.write_msg(buf, |buf, loc| {
            loc.id.write(buf, i as u64);
            loc.mapping_id.write(buf, 1u64);
            loc.address.write(buf, (0x400000 + i * 16) as u64);
            loc.line.write_msg(buf, |buf, line| {
                line.function_id.write(buf, (i % NUM_FUNCTIONS + 1) as u64);
                line.line.write(buf, (i * 10) as i64);
            });
            if i % 3 == 0 {
                loc.line.write_msg(buf, |buf, line| {
                    line.function_id
                        .write(buf, ((i + 1) % NUM_FUNCTIONS + 1) as u64);
                    line.line.write(buf, (i * 10 + 5) as i64);
                });
            }
        });
    }

    // functions
    for i in 1..=NUM_FUNCTIONS {
        s.function.write_msg(buf, |buf, f| {
            f.id.write(buf, i as u64);
            f.name.write(buf, (7 + i - 1) as i64);
            f.system_name.write(buf, (7 + i - 1) as i64);
            f.filename
                .write(buf, (7 + NUM_FUNCTIONS + (i - 1) % 5) as i64);
            f.start_line.write(buf, (i * 100) as i64);
        });
    }

    // string_table — iterate and write each string individually to avoid intermediate Vec
    for st in &data.string_table {
        s.string_table.write(buf, &[st.as_str()]);
    }

    // scalar fields
    s.time_nanos.write(buf, 1_678_886_400_000_000_000i64);
    s.duration_nanos.write(buf, 30_000_000_000i64);
    s.period_type.write_msg(buf, |buf, vt| {
        vt.r#type.write(buf, 1i64);
        vt.unit.write(buf, 2i64);
    });
    s.period.write(buf, 10_000_000i64);
}

fn prost_pprof_profile() -> prost_pprof::Profile {
    let strings = pprof_string_table();

    let sample_type = vec![prost_pprof::ValueType { r#type: 1, unit: 2 }];

    let sample: Vec<prost_pprof::Sample> = (0..NUM_SAMPLES)
        .map(|i| {
            let location_id: Vec<u64> = (0..LOCS_PER_SAMPLE)
                .map(|j| ((i * LOCS_PER_SAMPLE + j) % NUM_LOCATIONS + 1) as u64)
                .collect();
            prost_pprof::Sample {
                location_id,
                value: vec![(i as i64 + 1) * 10_000_000, 1],
                label: vec![prost_pprof::Label {
                    key: 3,
                    str: 0,
                    num: (i % 8) as i64,
                    num_unit: 4,
                }],
            }
        })
        .collect();

    let mapping = vec![prost_pprof::Mapping {
        id: 1,
        memory_start: 0x400000,
        memory_limit: 0x800000,
        file_offset: 0,
        filename: 5,
        build_id: 6,
        has_functions: true,
        has_filenames: true,
        has_line_numbers: true,
        has_inline_frames: true,
    }];

    let location: Vec<prost_pprof::Location> = (1..=NUM_LOCATIONS)
        .map(|i| {
            let mut lines = vec![prost_pprof::Line {
                function_id: (i % NUM_FUNCTIONS + 1) as u64,
                line: (i * 10) as i64,
                column: 0,
            }];
            if i % 3 == 0 {
                lines.push(prost_pprof::Line {
                    function_id: ((i + 1) % NUM_FUNCTIONS + 1) as u64,
                    line: (i * 10 + 5) as i64,
                    column: 0,
                });
            }
            prost_pprof::Location {
                id: i as u64,
                mapping_id: 1,
                address: (0x400000 + i * 16) as u64,
                line: lines,
                is_folded: false,
            }
        })
        .collect();

    let function: Vec<prost_pprof::Function> = (1..=NUM_FUNCTIONS)
        .map(|i| prost_pprof::Function {
            id: i as u64,
            name: (7 + i - 1) as i64,
            system_name: (7 + i - 1) as i64,
            filename: (7 + NUM_FUNCTIONS + (i - 1) % 5) as i64,
            start_line: (i * 100) as i64,
        })
        .collect();

    prost_pprof::Profile {
        sample_type,
        sample,
        mapping,
        location,
        function,
        string_table: strings,
        drop_frames: 0,
        keep_frames: 0,
        time_nanos: 1_678_886_400_000_000_000,
        duration_nanos: 30_000_000_000,
        period_type: Some(prost_pprof::ValueType { r#type: 1, unit: 2 }),
        period: 10_000_000,
        comment: vec![],
        default_sample_type: 0,
        doc_url: 0,
    }
}

fn bench_encode_pprof(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_pprof");

    let data = pprof_encode_data();
    let mut ref_buf = Vec::with_capacity(16384);
    tacky_encode_pprof(&mut ref_buf, &data);
    let size = ref_buf.len() as u64;
    group.throughput(Throughput::Bytes(size));

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            tacky_encode_pprof(&mut buf, &data);
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    let prost_msg = prost_pprof_profile();
    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            prost_msg.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
        });
    });

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

fn bench_decode_pprof(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_pprof");

    // Use prost-encoded bytes so both decoders parse identical packed data
    let prost_msg = prost_pprof_profile();
    let wire = prost_msg.encode_to_vec();
    group.throughput(Throughput::Bytes(wire.len() as u64));

    // Verify both decoders produce the same result
    let tacky_result = tacky_decode_pprof_into_prost(&wire);
    let prost_result = prost_pprof::Profile::decode(wire.as_slice()).unwrap();
    assert_eq!(
        tacky_result, prost_result,
        "pprof decode mismatch between tacky and prost"
    );

    group.bench_function("tacky", |b| {
        b.iter(|| {
            let msg = tacky_decode_pprof_into_prost(black_box(&wire));
            black_box(&msg);
        });
    });

    group.bench_function("prost", |b| {
        b.iter(|| {
            let msg = prost_pprof::Profile::decode(black_box(wire.as_slice())).unwrap();
            black_box(&msg);
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Access log: string-heavy realistic messages (~100 entries per batch)
// ---------------------------------------------------------------------------

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

/// Pre-computed data for the access log encode benchmark.
struct AccessLogEncodeData {
    remote_addrs: Vec<String>,
    methods: Vec<i32>,
    statuses: Vec<i32>,
    response_bytes: Vec<i64>,
    durations: Vec<i64>,
    timestamps: Vec<i64>,
}

fn accesslog_encode_data() -> AccessLogEncodeData {
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
        durations: (0..NUM_LOG_ENTRIES)
            .map(|i| 500 + (i as i64 * 317) % 50_000) // 0.5ms to 50ms
            .collect(),
        timestamps: (0..NUM_LOG_ENTRIES)
            .map(|i| 1_700_000_000_000_000 + i as i64 * 15_000) // ~15µs apart
            .collect(),
    }
}

fn tacky_encode_accesslog(buf: &mut Vec<u8>, data: &AccessLogEncodeData) {
    use tacky_accesslog::accesslog::{AccessLog, HttpMethod};

    let s = AccessLog::default();

    for i in 0..NUM_LOG_ENTRIES {
        s.entries.write_msg(buf, |buf, e| {
            e.remote_addr.write(buf, data.remote_addrs[i].as_str());
            e.method.write(buf, HttpMethod::from(data.methods[i]));
            e.path.write(buf, PATHS[i % PATHS.len()]);
            let q = QUERIES[i % QUERIES.len()];
            if !q.is_empty() {
                e.query.write(buf, q);
            }
            e.status.write(buf, data.statuses[i]);
            e.response_bytes.write(buf, data.response_bytes[i]);
            e.duration_micros.write(buf, data.durations[i]);
            e.user_agent.write(buf, USER_AGENTS[i % USER_AGENTS.len()]);
            let r = REFERERS[i % REFERERS.len()];
            if !r.is_empty() {
                e.referer.write(buf, r);
            }
            e.timestamp.write(buf, data.timestamps[i]);
            e.host.write(buf, "myapp.example.com");
            e.protocol.write(buf, "HTTP/2");
            // 2-3 headers per request
            e.request_headers.write_msg(buf, |buf, h| {
                h.name.write(buf, "Accept");
                h.value.write(buf, "application/json");
            });
            e.request_headers.write_msg(buf, |buf, h| {
                h.name.write(buf, "Authorization");
                h.value.write(
                    buf,
                    "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
                );
            });
            if i % 3 == 0 {
                e.request_headers.write_msg(buf, |buf, h| {
                    h.name.write(buf, "X-Request-Id");
                    h.value.write(buf, "a]b1c2d3-e4f5-6789-abcd-ef0123456789");
                });
            }
        });
    }

    s.server_id.write(buf, "web-prod-us-east-1a-i-0abc123def");
    s.batch_timestamp.write(buf, 1_700_000_000_000_000i64);
}

fn prost_accesslog_msg(data: &AccessLogEncodeData) -> prost_accesslog::AccessLog {
    let entries: Vec<prost_accesslog::Entry> = (0..NUM_LOG_ENTRIES)
        .map(|i| {
            let mut headers = vec![
                prost_accesslog::Header {
                    name: "Accept".into(),
                    value: "application/json".into(),
                },
                prost_accesslog::Header {
                    name: "Authorization".into(),
                    value:
                        "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0"
                            .into(),
                },
            ];
            if i % 3 == 0 {
                headers.push(prost_accesslog::Header {
                    name: "X-Request-Id".into(),
                    value: "a]b1c2d3-e4f5-6789-abcd-ef0123456789".into(),
                });
            }

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
                request_headers: headers,
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
    tacky_encode_accesslog(&mut ref_buf, &data);
    let size = ref_buf.len() as u64;
    group.throughput(Throughput::Bytes(size));

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(size as usize);
        b.iter(|| {
            tacky_encode_accesslog(&mut buf, &data);
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

    group.finish();
}

fn tacky_decode_accesslog_into_prost(wire: &[u8]) -> prost_accesslog::AccessLog {
    use tacky_accesslog::accesslog::{AccessLog, AccessLogField, EntryField, HeaderField};

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
                        EntryField::RequestHeaders(fields) => {
                            let mut h = prost_accesslog::Header::default();
                            for f in fields {
                                match f.unwrap() {
                                    HeaderField::Name(v) => h.name = v.to_string(),
                                    HeaderField::Value(v) => h.value = v.to_string(),
                                }
                            }
                            e.request_headers.push(h);
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

    group.finish();
}

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
);
criterion_main!(benches);
