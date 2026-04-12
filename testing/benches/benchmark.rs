use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use prost::Message;

// import tacky generated
#[allow(dead_code)]
mod tacky_proto {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}
#[allow(dead_code)]
mod prost_proto {
    include!(concat!(env!("OUT_DIR"), "/example.rs"));
}

use prost_proto::{
    BigLevel1 as PBigLevel1, BigLevel2 as PBigLevel2, BigLevel3 as PBigLevel3,
    BigLevel4 as PBigLevel4, Level1 as PLevel1, Level2 as PLevel2, Level3 as PLevel3,
    Level4 as PLevel4, MixedLargeMessage as PMixedLargeMessage,
    MixedSmallMessage as PMixedSmallMessage, MixedUsageMessage as PMixedUsageMessage,
    SimpleMessage as PSimpleMessage,
};
use tacky_proto::example::{
    BigLevel1 as TBigLevel1, Level1 as TLevel1, MixedUsageMessage as TMixedUsageMessage,
    SimpleEnum as TSimpleEnum, SimpleMessage as TSimpleMessage,
};

fn bench_packed_repeated(c: &mut Criterion) {
    let mut group = c.benchmark_group("packed_repeated");
    let sizes = [("few", 10), ("many", 100), ("hundreds", 1000)];

    for (name, size) in sizes.iter() {
        // uniform distribution across the i32 range
        let step = (i32::MAX as f64 / *size as f64).floor() as i32;
        let manynumbers: Vec<i32> = (0..*size)
            .map(|i| (i as i32).saturating_mul(step))
            .collect();

        group.bench_with_input(BenchmarkId::new("tacky", name), size, |b, _size| {
            let mut buf = Vec::with_capacity(1024 * 10);
            b.iter(|| {
                let schema = TSimpleMessage::default();
                schema.manynumbers.write(&mut buf, &manynumbers);
                black_box(buf.clear());
            });
            black_box(buf);
        });

        group.bench_with_input(BenchmarkId::new("prost", name), size, |b, _size| {
            let msg = PSimpleMessage {
                normal_int: None,
                zigzag_int: None,
                fixed_int: None,
                manynumbers: manynumbers.clone(),
                manynumbers_unpacked: vec![],
                packed_enum: vec![],
                astring: None,
                manystrings: vec![],
                manybytes: vec![],
                abytes: None,
                yesno: None,
                packed_doubles: vec![],
                packed_floats: vec![],
                packed_fixed32: vec![],
                packed_fixed64: vec![],
                packed_sfixed32: vec![],
                packed_sfixed64: vec![],
                repeated_ints: vec![],
                repeated_floats: vec![],
            };
            let mut buf = Vec::with_capacity(1024 * 10);
            b.iter(|| {
                msg.encode(&mut buf).unwrap();
                black_box(buf.clear());
            });
            black_box(buf);
        });
    }
    group.finish();
}

fn bench_normal_repeated(c: &mut Criterion) {
    let mut group = c.benchmark_group("normal_repeated");
    let sizes = [("few", 10), ("many", 100), ("hundreds", 1000)];

    for (name, size) in sizes.iter() {
        // uniform distribution across the i32 range
        let step = (i32::MAX as f64 / *size as f64).floor() as i32;
        let manynumbers: Vec<i32> = (0..*size)
            .map(|i| (i as i32).saturating_mul(step))
            .collect();

        group.bench_with_input(BenchmarkId::new("tacky", name), size, |b, _size| {
            let mut buf = Vec::with_capacity(1024 * 10);
            b.iter(|| {
                let schema = TSimpleMessage::default();
                schema.manynumbers_unpacked.write(&mut buf, &manynumbers);
                black_box(buf.clear());
            });
            black_box(buf);
        });

        group.bench_with_input(BenchmarkId::new("prost", name), size, |b, _size| {
            let msg = PSimpleMessage {
                normal_int: None,
                zigzag_int: None,
                fixed_int: None,
                manynumbers: vec![],
                manynumbers_unpacked: manynumbers.clone(),
                packed_enum: vec![],
                astring: None,
                manystrings: vec![],
                manybytes: vec![],
                abytes: None,
                yesno: None,
                packed_doubles: vec![],
                packed_floats: vec![],
                packed_fixed32: vec![],
                packed_fixed64: vec![],
                packed_sfixed32: vec![],
                packed_sfixed64: vec![],
                repeated_ints: vec![],
                repeated_floats: vec![],
            };
            let mut buf = Vec::with_capacity(1024 * 10);
            b.iter(|| {
                msg.encode(&mut buf).unwrap();
                black_box(buf.clear());
            });
            black_box(buf);
        });
    }
    group.finish();
}

fn bench_nested_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_messages");

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            let schema = TLevel1::default();
            schema.num.write(&mut buf, Some(1));
            schema.next.write_msg(&mut buf, |buf, scm| {
                scm.num.write(buf, Some(2));
                scm.next.write_msg(buf, |buf, scm| {
                    scm.num.write(buf, Some(3));
                    scm.next.write_msg(buf, |buf, scm| {
                        scm.data.write(buf, Some("hello world"));
                        scm.num.write(buf, Some(42));
                    });
                });
            });
            black_box(buf.clear());
        });
        black_box(buf);
    });

    let msg = PLevel1 {
        num: Some(1),
        next: Some(PLevel2 {
            num: Some(2),
            next: Some(PLevel3 {
                num: Some(3),
                next: Some(PLevel4 {
                    num: Some(42),
                    data: Some("hello world".to_string()),
                }),
            }),
        }),
    };

    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            msg.encode(&mut buf).unwrap();
            black_box(buf.clear());
        });
        black_box(buf);
    });

    group.finish();
}

fn bench_big_nested_messages(c: &mut Criterion) {
    let mut group = c.benchmark_group("big_nested_messages");

    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            let schema = TBigLevel1::default();
            schema.next.write_msg(&mut buf, |buf, scm| {
                scm.next.write_msg(buf, |buf, scm| {
                    scm.next.write_msg(buf, |buf, scm| {
                        scm.data1.write(buf, Some("level4 data 1"));
                        scm.data2.write(buf, Some("level4 data 2"));
                        scm.num1.write(buf, Some(41));
                        scm.num2.write(buf, Some(42));
                        scm.flag.write(buf, Some(true));
                        scm.many_nums.write(buf, &[1, 2, 3, 4, 5]);
                    });
                    scm.data.write(buf, Some("level3 data"));
                    scm.num.write(buf, Some(30));
                    scm.flag.write(buf, Some(true));
                });
                scm.data.write(buf, Some("level2 data"));
                scm.num.write(buf, Some(20));
                scm.flag.write(buf, Some(false));
            });
            schema.data.write(&mut buf, Some("level1 data"));
            schema.num.write(&mut buf, Some(10));
            schema.flag.write(&mut buf, Some(true));
            black_box(buf.clear());
        });
        black_box(buf);
    });

    let msg = PBigLevel1 {
        next: Some(PBigLevel2 {
            next: Some(PBigLevel3 {
                next: Some(PBigLevel4 {
                    data1: Some("level4 data 1".to_string()),
                    data2: Some("level4 data 2".to_string()),
                    num1: Some(41),
                    num2: Some(42),
                    flag: Some(true),
                    many_nums: vec![1, 2, 3, 4, 5],
                }),
                data: Some("level3 data".to_string()),
                num: Some(30),
                flag: Some(true),
            }),
            data: Some("level2 data".to_string()),
            num: Some(20),
            flag: Some(false),
        }),
        data: Some("level1 data".to_string()),
        num: Some(10),
        flag: Some(true),
    };

    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            msg.encode(&mut buf).unwrap();
            black_box(buf.clear());
        });
        black_box(buf);
    });

    group.finish();
}

fn bench_mixed_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_usage_message");

    // All fields set
    group.bench_function("tacky_all_set", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            let schema = TMixedUsageMessage::default();
            schema.session_id.write(&mut buf, Some("session-12345"));
            schema.user_id.write(&mut buf, Some(9999));
            schema.client_version.write(&mut buf, Some("v1.2.3"));
            schema.small_payload.write_msg(&mut buf, |buf, scm| {
                scm.label.write(buf, Some("small-label"));
                scm.count.write(buf, Some(42));
                scm.active.write(buf, Some(true));
            });
            schema.large_payload.write_msg(&mut buf, |buf, scm| {
                scm.id.write(buf, Some("large-id-987"));
                scm.name.write(buf, Some("Large Payload Entity"));
                scm.description.write(buf, Some("This is a very long description to test the performance of large string fields in protobufs."));
                scm.timestamp.write(buf, Some(1678886400));
                scm.score.write(buf, Some(99.9));
                scm.is_verified.write(buf, Some(true));
                scm.tags.write(buf, &[1, 2, 3, 4, 10, 20]);
                scm.permissions.write(buf, &["read", "write", "admin"]);
                scm.details.write_msg(buf, |buf, scm| {
                    scm.label.write(buf, Some("nested-small"));
                    scm.count.write(buf, Some(1));
                    scm.active.write(buf, Some(false));
                });
                scm.metrics.write(buf, &[0.1, 0.2, 0.3, 0.4, 0.5]);
                scm.flags.write(buf, &[true, false, true, true]);
            });
            schema.history.write_msg(&mut buf, |buf, scm| {
                scm.label.write(buf, Some("hist-1"));
                scm.count.write(buf, Some(10));
                scm.active.write(buf, Some(true));
            });
            schema.history.write_msg(&mut buf, |buf, scm| {
                scm.label.write(buf, Some("hist-2"));
                scm.count.write(buf, Some(20));
                scm.active.write(buf, Some(false));
            });
            schema.related_ids.write(&mut buf, &["rel-1", "rel-2", "rel-3"]);
            schema.created_at.write(&mut buf, Some(1670000000));
            schema.updated_at.write(&mut buf, Some(1678886400));
            schema.priority.write(&mut buf, Some(1.0));
            schema.is_test.write(&mut buf, Some(false));
            schema.status.write(&mut buf, Some(TSimpleEnum::Second));
            black_box(buf.clear());
        });
        black_box(buf);
    });

    let prost_all = PMixedUsageMessage {
        session_id: Some("session-12345".to_string()),
        user_id: Some(9999),
        client_version: Some("v1.2.3".to_string()),
        small_payload: Some(PMixedSmallMessage {
            label: Some("small-label".to_string()),
            count: Some(42),
            active: Some(true),
        }),
        large_payload: Some(PMixedLargeMessage {
            id: Some("large-id-987".to_string()),
            name: Some("Large Payload Entity".to_string()),
            description: Some("This is a very long description to test the performance of large string fields in protobufs.".to_string()),
            timestamp: Some(1678886400),
            score: Some(99.9),
            is_verified: Some(true),
            tags: vec![1, 2, 3, 4, 10, 20],
            permissions: vec!["read".to_string(), "write".to_string(), "admin".to_string()],
            details: Some(PMixedSmallMessage {
                label: Some("nested-small".to_string()),
                count: Some(1),
                active: Some(false),
            }),
            metrics: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            flags: vec![true, false, true, true],
        }),
        history: vec![
            PMixedSmallMessage {
                label: Some("hist-1".to_string()),
                count: Some(10),
                active: Some(true),
            },
            PMixedSmallMessage {
                label: Some("hist-2".to_string()),
                count: Some(20),
                active: Some(false),
            },
        ],
        related_ids: vec!["rel-1".to_string(), "rel-2".to_string(), "rel-3".to_string()],
        created_at: Some(1670000000),
        updated_at: Some(1678886400),
        priority: Some(1.0),
        is_test: Some(false),
        status: Some(prost_proto::SimpleEnum::Second as i32),
    };

    group.bench_function("prost_all_set", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            prost_all.encode(&mut buf).unwrap();
            black_box(buf.clear());
        });
        black_box(buf);
    });

    // half set fields
    group.bench_function("tacky_half_set", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            let schema = TMixedUsageMessage::default();
            schema.session_id.write(&mut buf, Some("session-12345"));
            schema.client_version.write(&mut buf, Some("v1.2.3"));
            schema.large_payload.write_msg(&mut buf, |buf, scm| {
                scm.id.write(buf, Some("large-id-987"));
                scm.description.write(buf, Some("This is a very long description to test the performance of large string fields in protobufs."));
                scm.score.write(buf, Some(99.9));
                scm.tags.write(buf, &[1, 2, 3, 4, 10, 20]);
                scm.details.write_msg(buf, |buf, scm| {
                    scm.label.write(buf, Some("nested-small"));
                });
                scm.flags.write(buf, &[true, false, true, true]);
            });
            schema.related_ids.write(&mut buf, &["rel-1", "rel-2", "rel-3"]);
            schema.updated_at.write(&mut buf, Some(1678886400));
            schema.is_test.write(&mut buf, Some(false));
            black_box(buf.clear());
        });
        black_box(buf);
    });
    let prost_half = PMixedUsageMessage {
        session_id: Some("session-12345".to_string()),
        user_id: None,
        client_version: Some("v1.2.3".to_string()),
        small_payload: None,
        large_payload: Some(PMixedLargeMessage {
            id: Some("large-id-987".to_string()),
            name: None,
            description: Some("This is a very long description to test the performance of large string fields in protobufs.".to_string()),
            timestamp: None,
            score: Some(99.9),
            is_verified: None,
            tags: vec![1, 2, 3, 4, 10, 20],
            permissions: vec![],
            details: Some(PMixedSmallMessage {
                label: Some("nested-small".to_string()),
                count: None,
                active: None,
            }),
            metrics: vec![],
            flags: vec![true, false, true, true],
        }),
        history: vec![],
        related_ids: vec!["rel-1".to_string(), "rel-2".to_string(), "rel-3".to_string()],
        created_at: None,
        updated_at: Some(1678886400),
        priority: None,
        is_test: Some(false),
        status: None,
    };

    group.bench_function("prost_half_set", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            prost_half.encode(&mut buf).unwrap();
            black_box(buf.clear());
        });
        black_box(buf);
    });

    // 1-2 fields set
    group.bench_function("tacky_few_set", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            let schema = TMixedUsageMessage::default();
            schema.session_id.write(&mut buf, Some("session-12345"));
            schema.is_test.write(&mut buf, Some(true));
            black_box(buf.clear());
        });
        black_box(buf);
    });
    let prost_few = PMixedUsageMessage {
        session_id: Some("session-12345".to_string()),
        user_id: None,
        client_version: None,
        small_payload: None,
        large_payload: None,
        history: vec![],
        related_ids: vec![],
        created_at: None,
        updated_at: None,
        priority: None,
        is_test: Some(true),
        status: None,
    };

    group.bench_function("prost_few_set", |b| {
        let mut buf = Vec::with_capacity(1024 * 8);
        b.iter(|| {
            prost_few.encode(&mut buf).unwrap();
            black_box(buf.clear());
        });
        black_box(buf);
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_nested_messages,
    bench_packed_repeated,
    bench_normal_repeated,
    bench_big_nested_messages,
    bench_mixed_usage
);
criterion_main!(benches);
