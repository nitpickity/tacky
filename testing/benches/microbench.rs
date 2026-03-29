use criterion::{black_box, criterion_group, criterion_main, Criterion};

// ──────────────────────────────────────────────────
//  packed fields: tacky vs prost (fixed + varint, short + medium)
// ──────────────────────────────────────────────────
fn bench_packed_vs_prost(c: &mut Criterion) {
    use prost::Message;
    let mut group = c.benchmark_group("packed_vs_prost");

    let sizes: &[(&str, usize)] = &[("3", 3), ("20", 20), ("50", 50)];

    for (name, count) in sizes {
        let ints: Vec<i32> = (0..*count as i32).map(|i| i * 137).collect();
        let floats: Vec<f32> = (0..*count).map(|i| i as f32 * 0.5).collect();
        let doubles: Vec<f64> = (0..*count).map(|i| i as f64 * 0.5).collect();
        let fixed32s: Vec<u32> = (0..*count as u32).map(|i| i * 7).collect();
        let fixed64s: Vec<u64> = (0..*count as u64).map(|i| i * 7).collect();

        // --- packed varint (Int32) ---
        group.bench_function(format!("tacky_varint_{name}"), |b| {
            use tacky_proto::example::SimpleMessage as TSimpleMessage;
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                let schema = TSimpleMessage::default();
                schema.manynumbers.write(&mut buf, &ints);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_function(format!("prost_varint_{name}"), |b| {
            let msg = prost_proto::SimpleMessage {
                manynumbers: ints.clone(),
                ..Default::default()
            };
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                msg.encode(&mut buf).unwrap();
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        // --- packed float ---
        group.bench_function(format!("tacky_float_{name}"), |b| {
            use tacky_proto::example::SimpleMessage as TSimpleMessage;
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                let schema = TSimpleMessage::default();
                schema.packed_floats.write_exact(&mut buf, &floats);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_function(format!("prost_float_{name}"), |b| {
            let msg = prost_proto::SimpleMessage {
                packed_floats: floats.clone(),
                ..Default::default()
            };
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                msg.encode(&mut buf).unwrap();
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        // --- packed double ---
        group.bench_function(format!("tacky_double_{name}"), |b| {
            use tacky_proto::example::SimpleMessage as TSimpleMessage;
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                let schema = TSimpleMessage::default();
                schema.packed_doubles.write_exact(&mut buf, &doubles);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_function(format!("prost_double_{name}"), |b| {
            let msg = prost_proto::SimpleMessage {
                packed_doubles: doubles.clone(),
                ..Default::default()
            };
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                msg.encode(&mut buf).unwrap();
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        // --- packed fixed32 ---
        group.bench_function(format!("tacky_fixed32_{name}"), |b| {
            use tacky_proto::example::SimpleMessage as TSimpleMessage;
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                let schema = TSimpleMessage::default();
                schema.packed_fixed32.write_exact(&mut buf, &fixed32s);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_function(format!("prost_fixed32_{name}"), |b| {
            let msg = prost_proto::SimpleMessage {
                packed_fixed32: fixed32s.clone(),
                ..Default::default()
            };
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                msg.encode(&mut buf).unwrap();
                black_box(buf.as_slice());
                buf.clear();
            });
        });

        // --- packed fixed64 ---
        group.bench_function(format!("tacky_fixed64_{name}"), |b| {
            use tacky_proto::example::SimpleMessage as TSimpleMessage;
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                let schema = TSimpleMessage::default();
                schema.packed_fixed64.write_exact(&mut buf, &fixed64s);
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_function(format!("prost_fixed64_{name}"), |b| {
            let msg = prost_proto::SimpleMessage {
                packed_fixed64: fixed64s.clone(),
                ..Default::default()
            };
            let mut buf = Vec::with_capacity(4096);
            b.iter(|| {
                msg.encode(&mut buf).unwrap();
                black_box(buf.as_slice());
                buf.clear();
            });
        });
    }
    group.finish();
}

// Need the generated proto for benchmarks
#[allow(dead_code)]
mod tacky_proto {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}
#[allow(dead_code)]
mod prost_proto {
    include!(concat!(env!("OUT_DIR"), "/example.rs"));
}

criterion_group!(benches, bench_packed_vs_prost,);
criterion_main!(benches);
