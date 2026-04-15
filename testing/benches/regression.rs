//! Internal regression benchmarks: tacky only, no prost.
//!
//! Catches performance regressions in tacky's encoding and decoding primitives.
//! Organized by feature area rather than by comparison.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[allow(dead_code)]
mod tacky_proto {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}

use tacky_proto::example::{
    ApiResponse, BigLevel1 as TBigLevel1, MapsWithMsg, MixedUsageMessage as TMixedUsageMessage,
    MsgWithMaps, SimpleEnum as TSimpleEnum, SimpleMessage as TSimpleMessage, SimpleMessageField,
};

// ---------------------------------------------------------------------------
// Encoding: scalar field types
// ---------------------------------------------------------------------------

fn bench_encode_scalars(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_scalars");

    // varint: int64 (small value vs large value)
    group.bench_function("int64/small", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.normal_int.write(&mut buf, Some(42i64));
            black_box(buf.as_slice());
            buf.clear();
        });
    });
    group.bench_function("int64/large", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.normal_int.write(&mut buf, Some(i64::MAX));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // zigzag: sint64
    group.bench_function("sint64/negative", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.zigzag_int.write(&mut buf, Some(-123456i64));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // fixed: sfixed64
    group.bench_function("sfixed64", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.fixed_int.write(&mut buf, Some(999i64));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // string
    group.bench_function("string/short", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.astring.write(&mut buf, Some("hello"));
            black_box(buf.as_slice());
            buf.clear();
        });
    });
    group.bench_function("string/long", |b| {
        let long = "a]".repeat(500);
        let mut buf = Vec::with_capacity(1200);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.astring.write(&mut buf, Some(long.as_str()));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // bytes
    group.bench_function("bytes", |b| {
        let data = vec![0xABu8; 256];
        let mut buf = Vec::with_capacity(300);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.abytes.write(&mut buf, Some(data.as_slice()));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // bool
    group.bench_function("bool", |b| {
        let mut buf = Vec::with_capacity(16);
        b.iter(|| {
            let scm = TSimpleMessage::default();
            scm.yesno.write(&mut buf, Some(true));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // enum
    group.bench_function("enum", |b| {
        let mut buf = Vec::with_capacity(16);
        b.iter(|| {
            let scm = tacky_proto::example::MsgWithEnums::default();
            scm.enum1.write(&mut buf, Some(TSimpleEnum::Second));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Encoding: packed fields — write vs write_exact, various types
// ---------------------------------------------------------------------------

fn bench_encode_packed(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_packed");

    let sizes: &[(&str, usize)] = &[("10", 10), ("100", 100), ("1000", 1000)];

    for (name, count) in sizes {
        let ints: Vec<i32> = (0..*count as i32).map(|i| i * 137).collect();
        let floats: Vec<f32> = (0..*count).map(|i| i as f32 * 0.5).collect();
        let doubles: Vec<f64> = (0..*count).map(|i| i as f64 * 0.5).collect();
        let fixed32s: Vec<u32> = (0..*count as u32).map(|i| i * 7).collect();
        let fixed64s: Vec<u64> = (0..*count as u64).map(|i| i * 7).collect();

        group.throughput(Throughput::Elements(*count as u64));

        // packed varint (int32) — write (iterator, uses Tack)
        group.bench_with_input(BenchmarkId::new("varint/write", name), count, |b, _| {
            let mut buf = Vec::with_capacity(count * 6);
            b.iter(|| {
                TSimpleMessage::default().manynumbers.write(&mut buf, &ints);
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        // packed float — write vs write_exact
        group.bench_with_input(BenchmarkId::new("float/write", name), count, |b, _| {
            let mut buf = Vec::with_capacity(count * 5);
            b.iter(|| {
                TSimpleMessage::default()
                    .packed_floats
                    .write(&mut buf, &floats);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_with_input(
            BenchmarkId::new("float/write_exact", name),
            count,
            |b, _| {
                let mut buf = Vec::with_capacity(count * 5);
                b.iter(|| {
                    TSimpleMessage::default()
                        .packed_floats
                        .write_exact(&mut buf, &floats);
                    black_box(buf.as_slice());
                    buf.clear();
                });
            },
        );

        // packed double — write vs write_exact
        group.bench_with_input(BenchmarkId::new("double/write", name), count, |b, _| {
            let mut buf = Vec::with_capacity(count * 9);
            b.iter(|| {
                TSimpleMessage::default()
                    .packed_doubles
                    .write(&mut buf, &doubles);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_with_input(
            BenchmarkId::new("double/write_exact", name),
            count,
            |b, _| {
                let mut buf = Vec::with_capacity(count * 9);
                b.iter(|| {
                    TSimpleMessage::default()
                        .packed_doubles
                        .write_exact(&mut buf, &doubles);
                    black_box(buf.as_slice());
                    buf.clear();
                });
            },
        );

        // packed fixed32 — write_exact (bypasses Tack)
        group.bench_with_input(
            BenchmarkId::new("fixed32/write_exact", name),
            count,
            |b, _| {
                let mut buf = Vec::with_capacity(count * 5);
                b.iter(|| {
                    TSimpleMessage::default()
                        .packed_fixed32
                        .write_exact(&mut buf, &fixed32s);
                    black_box(buf.as_slice());
                    buf.clear();
                });
            },
        );

        // packed fixed64 — write_exact (bypasses Tack)
        group.bench_with_input(
            BenchmarkId::new("fixed64/write_exact", name),
            count,
            |b, _| {
                let mut buf = Vec::with_capacity(count * 9);
                b.iter(|| {
                    TSimpleMessage::default()
                        .packed_fixed64
                        .write_exact(&mut buf, &fixed64s);
                    black_box(buf.as_slice());
                    buf.clear();
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Encoding: unpacked repeated
// ---------------------------------------------------------------------------

fn bench_encode_unpacked_repeated(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_unpacked_repeated");

    let sizes: &[(&str, usize)] = &[("10", 10), ("100", 100), ("1000", 1000)];

    for (name, count) in sizes {
        let ints: Vec<i32> = (0..*count as i32).map(|i| i * 137).collect();
        let strings: Vec<&str> = (0..*count).map(|_| "hello world").collect();

        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(BenchmarkId::new("int32", name), count, |b, _| {
            let mut buf = Vec::with_capacity(count * 8);
            b.iter(|| {
                TSimpleMessage::default()
                    .manynumbers_unpacked
                    .write(&mut buf, &ints);
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        group.bench_with_input(BenchmarkId::new("string", name), count, |b, _| {
            let mut buf = Vec::with_capacity(count * 16);
            b.iter(|| {
                TSimpleMessage::default()
                    .manystrings
                    .write(&mut buf, &strings);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Encoding: nested message depth (Tack overhead)
// ---------------------------------------------------------------------------

fn bench_encode_nesting_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_nesting_depth");

    // depth 1: just BigLevel1 scalars
    group.bench_function("depth_1", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = TBigLevel1::default();
            scm.data.write(&mut buf, Some("level1"));
            scm.num.write(&mut buf, Some(1));
            scm.flag.write(&mut buf, Some(true));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // depth 2
    group.bench_function("depth_2", |b| {
        let mut buf = Vec::with_capacity(128);
        b.iter(|| {
            let scm = TBigLevel1::default();
            scm.data.write(&mut buf, Some("level1"));
            scm.num.write(&mut buf, Some(1));
            scm.next.write_msg(&mut buf, |buf, scm| {
                scm.data.write(buf, Some("level2"));
                scm.num.write(buf, Some(2));
            });
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // depth 3
    group.bench_function("depth_3", |b| {
        let mut buf = Vec::with_capacity(192);
        b.iter(|| {
            let scm = TBigLevel1::default();
            scm.data.write(&mut buf, Some("level1"));
            scm.next.write_msg(&mut buf, |buf, scm| {
                scm.data.write(buf, Some("level2"));
                scm.next.write_msg(buf, |buf, scm| {
                    scm.data.write(buf, Some("level3"));
                    scm.num.write(buf, Some(3));
                });
            });
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // depth 4 (max for BigLevel)
    group.bench_function("depth_4", |b| {
        let mut buf = Vec::with_capacity(256);
        b.iter(|| {
            let scm = TBigLevel1::default();
            scm.data.write(&mut buf, Some("level1"));
            scm.next.write_msg(&mut buf, |buf, scm| {
                scm.data.write(buf, Some("level2"));
                scm.next.write_msg(buf, |buf, scm| {
                    scm.data.write(buf, Some("level3"));
                    scm.next.write_msg(buf, |buf, scm| {
                        scm.data1.write(buf, Some("level4"));
                        scm.num1.write(buf, Some(4));
                    });
                });
            });
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Encoding: maps
// ---------------------------------------------------------------------------

fn bench_encode_maps(c: &mut Criterion) {
    use std::collections::BTreeMap;

    let mut group = c.benchmark_group("encode_maps");

    let sizes: &[(&str, usize)] = &[("5", 5), ("50", 50)];

    for (name, count) in sizes {
        let map: BTreeMap<&str, i32> = (0..*count as i32)
            .map(|i| {
                // Leak a string so we get &'static str. Fine for benchmarks.
                let s: &str = Box::leak(format!("key_{i}").into_boxed_str());
                (s, i * 10)
            })
            .collect();

        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(BenchmarkId::new("string_int32", name), count, |b, _| {
            let mut buf = Vec::with_capacity(count * 20);
            b.iter(|| {
                MsgWithMaps::default().map1.write(&mut buf, &map);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
    }

    // Map with message values
    group.bench_function("string_message/5", |b| {
        let mut buf = Vec::with_capacity(512);
        b.iter(|| {
            let scm = MapsWithMsg::default();
            for i in 0..5 {
                let key: &str = match i {
                    0 => "alpha",
                    1 => "bravo",
                    2 => "charlie",
                    3 => "delta",
                    _ => "echo",
                };
                scm.map1.write_msg(&mut buf, key, |buf, scm| {
                    scm.normal_int.write(buf, Some(i * 10));
                    scm.astring.write(buf, Some("map-value"));
                });
            }
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Encoding: oneof
// ---------------------------------------------------------------------------

fn bench_encode_oneof(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_oneof");

    group.bench_function("string_variant", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = ApiResponse::default();
            scm.request_id.write(&mut buf, Some("req-123"));
            scm.result.write_error(&mut buf, "something went wrong");
            scm.cached.write(&mut buf, Some(false));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.bench_function("int_variant", |b| {
        let mut buf = Vec::with_capacity(64);
        b.iter(|| {
            let scm = ApiResponse::default();
            scm.request_id.write(&mut buf, Some("req-123"));
            scm.result.write_code(&mut buf, 404);
            scm.cached.write(&mut buf, Some(true));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.bench_function("message_variant", |b| {
        let mut buf = Vec::with_capacity(128);
        b.iter(|| {
            let scm = ApiResponse::default();
            scm.request_id.write(&mut buf, Some("req-123"));
            scm.result.write_data_msg(&mut buf, |buf, scm| {
                scm.normal_int.write(buf, Some(42));
                scm.astring.write(buf, Some("response payload"));
            });
            scm.cached.write(&mut buf, Some(true));
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Decoding: field iteration
// ---------------------------------------------------------------------------

fn bench_decode_fields(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_fields");

    // Encode a SimpleMessage with many field types populated
    let mut wire = Vec::with_capacity(256);
    {
        let scm = TSimpleMessage::default();
        scm.normal_int.write(&mut wire, Some(42i64));
        scm.zigzag_int.write(&mut wire, Some(-7i64));
        scm.fixed_int.write(&mut wire, Some(999i64));
        scm.manynumbers.write(&mut wire, &[10, 20, 30, 40, 50]);
        scm.astring.write(&mut wire, Some("hello world"));
        scm.manystrings.write(&mut wire, &["foo", "bar", "baz"]);
        scm.abytes.write(&mut wire, Some(b"raw bytes".as_slice()));
        scm.yesno.write(&mut wire, Some(true));
        scm.packed_doubles.write(&mut wire, &[1.5, 2.5, 3.5]);
        scm.packed_floats.write(&mut wire, &[0.5f32, 1.0, 2.0]);
        scm.packed_fixed32.write(&mut wire, &[100u32, 200, 300]);
    }
    group.throughput(Throughput::Bytes(wire.len() as u64));

    group.bench_function("simple_message", |b| {
        b.iter(|| {
            for field in TSimpleMessage::decode(&wire) {
                let field = field.unwrap();
                match field {
                    SimpleMessageField::Manynumbers(iter) => {
                        for v in iter {
                            black_box(v.unwrap());
                        }
                    }
                    SimpleMessageField::PackedDoubles(iter) => {
                        for v in iter {
                            black_box(v.unwrap());
                        }
                    }
                    SimpleMessageField::PackedFloats(iter) => {
                        for v in iter {
                            black_box(v.unwrap());
                        }
                    }
                    SimpleMessageField::PackedFixed32(iter) => {
                        for v in iter {
                            black_box(v.unwrap());
                        }
                    }
                    other => {
                        black_box(other);
                    }
                }
            }
        });
    });

    // Decode the realistic MixedUsageMessage
    let mut mixed_wire = Vec::with_capacity(512);
    {
        let schema = TMixedUsageMessage::default();
        schema
            .session_id
            .write(&mut mixed_wire, Some("session-12345"));
        schema.user_id.write(&mut mixed_wire, Some(9999));
        schema.client_version.write(&mut mixed_wire, Some("v1.2.3"));
        schema.small_payload.write_msg(&mut mixed_wire, |buf, scm| {
            scm.label.write(buf, Some("small-label"));
            scm.count.write(buf, Some(42));
            scm.active.write(buf, Some(true));
        });
        schema.large_payload.write_msg(&mut mixed_wire, |buf, scm| {
            scm.id.write(buf, Some("large-id-987"));
            scm.name.write(buf, Some("Large Payload Entity"));
            scm.description.write(buf, Some("A description string."));
            scm.timestamp.write(buf, Some(1678886400));
            scm.score.write(buf, Some(99.9));
            scm.is_verified.write(buf, Some(true));
            scm.tags.write(buf, &[1, 2, 3, 4, 10, 20]);
            scm.permissions.write(buf, &["read", "write", "admin"]);
            scm.metrics.write(buf, &[0.1, 0.2, 0.3, 0.4, 0.5]);
            scm.flags.write(buf, &[true, false, true, true]);
        });
        schema.history.write_msg(&mut mixed_wire, |buf, scm| {
            scm.label.write(buf, Some("hist-1"));
            scm.count.write(buf, Some(10));
        });
        schema
            .related_ids
            .write(&mut mixed_wire, &["rel-1", "rel-2"]);
        schema.created_at.write(&mut mixed_wire, Some(1670000000));
        schema
            .status
            .write(&mut mixed_wire, Some(TSimpleEnum::Second));
    }
    group.throughput(Throughput::Bytes(mixed_wire.len() as u64));

    group.bench_function("mixed_usage_message", |b| {
        b.iter(|| {
            for field in TMixedUsageMessage::decode(&mixed_wire) {
                black_box(field.unwrap());
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Decoding: packed iteration at various sizes
// ---------------------------------------------------------------------------

fn bench_decode_packed(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_packed");

    let sizes: &[(&str, usize)] = &[("10", 10), ("100", 100), ("1000", 1000)];

    for (name, count) in sizes {
        let data: Vec<i32> = (0..*count as i32).map(|i| i * 137).collect();

        // Encode packed varint field
        let mut wire_varint = Vec::new();
        TSimpleMessage::default()
            .manynumbers
            .write(&mut wire_varint, &data);

        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(BenchmarkId::new("varint_iter", name), count, |b, _| {
            b.iter(|| {
                for field in TSimpleMessage::decode(&wire_varint) {
                    match field.unwrap() {
                        SimpleMessageField::Manynumbers(iter) => {
                            for v in iter {
                                black_box(v.unwrap());
                            }
                        }
                        _ => {}
                    }
                }
            });
        });

        // Encode packed doubles
        let doubles: Vec<f64> = (0..*count).map(|i| i as f64 * 0.5).collect();
        let mut wire_doubles = Vec::new();
        TSimpleMessage::default()
            .packed_doubles
            .write(&mut wire_doubles, &doubles);

        group.bench_with_input(BenchmarkId::new("double_iter", name), count, |b, _| {
            b.iter(|| {
                for field in TSimpleMessage::decode(&wire_doubles) {
                    match field.unwrap() {
                        SimpleMessageField::PackedDoubles(iter) => {
                            for v in iter {
                                black_box(v.unwrap());
                            }
                        }
                        _ => {}
                    }
                }
            });
        });

        // Encode packed fixed32
        let fixed32s: Vec<u32> = (0..*count as u32).map(|i| i * 7).collect();
        let mut wire_fixed32 = Vec::new();
        TSimpleMessage::default()
            .packed_fixed32
            .write(&mut wire_fixed32, &fixed32s);

        group.bench_with_input(BenchmarkId::new("fixed32_iter", name), count, |b, _| {
            b.iter(|| {
                for field in TSimpleMessage::decode(&wire_fixed32) {
                    match field.unwrap() {
                        SimpleMessageField::PackedFixed32(iter) => {
                            for v in iter {
                                black_box(v.unwrap());
                            }
                        }
                        _ => {}
                    }
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encode_scalars,
    bench_encode_packed,
    bench_encode_unpacked_repeated,
    bench_encode_nesting_depth,
    bench_encode_maps,
    bench_encode_oneof,
    bench_decode_fields,
    bench_decode_packed,
);
criterion_main!(benches);
