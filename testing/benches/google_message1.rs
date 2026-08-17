//! GoogleMessage1 — protobuf's own benchmark corpus, in both proto2 and proto3.
//!
//! Google deleted these datasets from protobuf in 2022 (`83c499de`, superseded by
//! fleetbench); prost copied them into `third_party/old_protobuf_benchmarks/` and
//! benches them to this day. The schemas and the two `BenchmarkDataset` payloads
//! here are vendored from protobuf tag v3.20.3 so this bench runs standalone, with
//! no network and no local `protoc` beyond what the build already needs.
//!
//! **Read the arms carefully.** prost's `benches/dataset.rs` sizes its output
//! buffer with a separate `encoded_len` pass and does not count that pass in its
//! `encode` figure — it reports it as its own line item. Sizing is exactly the work
//! tacky exists to delete, so:
//!
//! - `tacky` — one pass, no sizing. What tacky does.
//! - `prost` — `Message::encode`, prost's published shape. It computes `encoded_len()`
//!   itself for its capacity check, so the sizing pass is included regardless.
//!
//! google_message2 is absent on purpose: it uses proto2 `group`, which tacky does
//! not implement (`WireType::SGROUP`/`EGROUP` are commented out in
//! `tacky/src/scalars.rs`). prost benches it. We'd be showing a gap, and hiding it
//! would be worse.
//!
//! Wire output is checked by decoding tacky's bytes with prost and comparing the
//! messages, not by comparing byte strings: tacky pads nested length prefixes to a
//! fixed width, so its output is semantically equal but can be a couple of bytes
//! longer, so wire equivalence is checked by decoding rather than by comparing bytes.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prost::Message;

#[cfg(feature = "cpp")]
#[path = "common/cpp_arms.rs"]
mod cpp_arms;

#[allow(dead_code)]
mod tacky_p2 {
    include!(concat!(env!("OUT_DIR"), "/tacky_message1_proto2.rs"));
}
#[allow(dead_code)]
mod tacky_p3 {
    include!(concat!(env!("OUT_DIR"), "/tacky_message1_proto3.rs"));
}
#[allow(dead_code)]
mod prost_p2 {
    include!(concat!(env!("OUT_DIR"), "/benchmarks.proto2.rs"));
}
#[allow(dead_code)]
mod prost_p3 {
    include!(concat!(env!("OUT_DIR"), "/benchmarks.proto3.rs"));
}
#[allow(dead_code)]
mod prost_dataset {
    include!(concat!(env!("OUT_DIR"), "/benchmarks.rs"));
}

const DATASET_PROTO2: &[u8] = include_bytes!("../data/dataset.google_message1_proto2.pb");
const DATASET_PROTO3: &[u8] = include_bytes!("../data/dataset.google_message1_proto3.pb");

/// Unwraps a vendored `BenchmarkDataset` into its payloads. protobuf's harness
/// loops over the whole list so a run isn't dominated by one message's branch
/// prediction; message1 happens to ship exactly one payload.
fn payloads(dataset: &[u8]) -> Vec<Vec<u8>> {
    prost_dataset::BenchmarkDataset::decode(dataset)
        .expect("vendored dataset decodes as BenchmarkDataset")
        .payload
}

// ---------------------------------------------------------------------------
// proto2
// ---------------------------------------------------------------------------

/// Writes one `GoogleMessage1` in ascending tag order, which is the order prost
/// emits, so the two outputs differ only where tacky pads a length prefix.
fn tacky_encode_p2<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, m: &prost_p2::GoogleMessage1) {
    let s = tacky_p2::benchmarks::proto2::GoogleMessage1::schema();
    s.field1.write(buf, m.field1.as_str());
    s.field2.write(buf, m.field2);
    s.field3.write(buf, m.field3);
    s.field4.write(buf, m.field4.as_deref());
    s.field5.write(buf, &m.field5);
    s.field6.write(buf, m.field6);
    s.field7.write(buf, m.field7.as_deref());
    s.field9.write(buf, m.field9.as_deref());
    s.field12.write(buf, m.field12);
    s.field13.write(buf, m.field13);
    s.field14.write(buf, m.field14);
    if let Some(sub) = &m.field15 {
        s.field15.write_msg(buf, |buf, t| {
            t.field1.write(buf, sub.field1);
            t.field2.write(buf, sub.field2);
            t.field3.write(buf, sub.field3);
            t.field12.write(buf, sub.field12);
            t.field13.write(buf, sub.field13);
            t.field14.write(buf, sub.field14);
            t.field15.write(buf, sub.field15.as_deref());
            t.field16.write(buf, sub.field16);
            t.field19.write(buf, sub.field19);
            t.field20.write(buf, sub.field20);
            t.field21.write(buf, sub.field21);
            t.field22.write(buf, sub.field22);
            t.field23.write(buf, sub.field23);
            t.field28.write(buf, sub.field28);
            t.field203.write(buf, sub.field203);
            t.field204.write(buf, sub.field204);
            t.field205.write(buf, sub.field205.as_deref());
            t.field206.write(buf, sub.field206);
            t.field207.write(buf, sub.field207);
            t.field300.write(buf, sub.field300);
        });
    }
    s.field16.write(buf, m.field16);
    s.field17.write(buf, m.field17);
    s.field18.write(buf, m.field18.as_deref());
    s.field22.write(buf, m.field22);
    s.field23.write(buf, m.field23);
    s.field24.write(buf, m.field24);
    s.field25.write(buf, m.field25);
    s.field29.write(buf, m.field29);
    s.field30.write(buf, m.field30);
    s.field59.write(buf, m.field59);
    s.field60.write(buf, m.field60);
    s.field67.write(buf, m.field67);
    s.field68.write(buf, m.field68);
    s.field78.write(buf, m.field78);
    s.field80.write(buf, m.field80);
    s.field81.write(buf, m.field81);
    s.field100.write(buf, m.field100);
    s.field101.write(buf, m.field101);
    s.field102.write(buf, m.field102.as_deref());
    s.field103.write(buf, m.field103.as_deref());
    s.field104.write(buf, m.field104);
    s.field128.write(buf, m.field128);
    s.field129.write(buf, m.field129.as_deref());
    s.field130.write(buf, m.field130);
    s.field131.write(buf, m.field131);
    s.field150.write(buf, m.field150);
    s.field271.write(buf, m.field271);
    s.field272.write(buf, m.field272);
    s.field280.write(buf, m.field280);
}

/// Decodes with tacky into prost's struct, so both decoders do the same work:
/// walk the wire bytes and materialize owned `String`s and `Vec`s.
fn tacky_decode_p2(wire: &[u8]) -> prost_p2::GoogleMessage1 {
    use tacky_p2::benchmarks::proto2::{
        GoogleMessage1 as T, GoogleMessage1Field as F, GoogleMessage1SubMessageField as SF,
    };

    let mut m = prost_p2::GoogleMessage1::default();
    for field in T::decode(wire) {
        match field.unwrap() {
            F::Field1(v) => m.field1 = v.to_string(),
            F::Field2(v) => m.field2 = v,
            F::Field3(v) => m.field3 = v,
            F::Field4(v) => m.field4 = Some(v.to_string()),
            F::Field5(v) => m.field5.push(v),
            F::Field6(v) => m.field6 = Some(v),
            F::Field7(v) => m.field7 = Some(v.to_string()),
            F::Field9(v) => m.field9 = Some(v.to_string()),
            F::Field12(v) => m.field12 = Some(v),
            F::Field13(v) => m.field13 = Some(v),
            F::Field14(v) => m.field14 = Some(v),
            F::Field15(fields) => {
                let sub = m.field15.get_or_insert_with(Default::default);
                for f in fields {
                    match f.unwrap() {
                        SF::Field1(v) => sub.field1 = Some(v),
                        SF::Field2(v) => sub.field2 = Some(v),
                        SF::Field3(v) => sub.field3 = Some(v),
                        SF::Field12(v) => sub.field12 = Some(v),
                        SF::Field13(v) => sub.field13 = Some(v),
                        SF::Field14(v) => sub.field14 = Some(v),
                        SF::Field15(v) => sub.field15 = Some(v.to_string()),
                        SF::Field16(v) => sub.field16 = Some(v),
                        SF::Field19(v) => sub.field19 = Some(v),
                        SF::Field20(v) => sub.field20 = Some(v),
                        SF::Field21(v) => sub.field21 = Some(v),
                        SF::Field22(v) => sub.field22 = Some(v),
                        SF::Field23(v) => sub.field23 = Some(v),
                        SF::Field28(v) => sub.field28 = Some(v),
                        SF::Field203(v) => sub.field203 = Some(v),
                        SF::Field204(v) => sub.field204 = Some(v),
                        SF::Field205(v) => sub.field205 = Some(v.to_string()),
                        SF::Field206(v) => sub.field206 = Some(v),
                        SF::Field207(v) => sub.field207 = Some(v),
                        SF::Field300(v) => sub.field300 = Some(v),
                    }
                }
            }
            F::Field16(v) => m.field16 = Some(v),
            F::Field17(v) => m.field17 = Some(v),
            F::Field18(v) => m.field18 = Some(v.to_string()),
            F::Field22(v) => m.field22 = Some(v),
            F::Field23(v) => m.field23 = Some(v),
            F::Field24(v) => m.field24 = Some(v),
            F::Field25(v) => m.field25 = Some(v),
            F::Field29(v) => m.field29 = Some(v),
            F::Field30(v) => m.field30 = Some(v),
            F::Field59(v) => m.field59 = Some(v),
            F::Field60(v) => m.field60 = Some(v),
            F::Field67(v) => m.field67 = Some(v),
            F::Field68(v) => m.field68 = Some(v),
            F::Field78(v) => m.field78 = Some(v),
            F::Field80(v) => m.field80 = Some(v),
            F::Field81(v) => m.field81 = Some(v),
            F::Field100(v) => m.field100 = Some(v),
            F::Field101(v) => m.field101 = Some(v),
            F::Field102(v) => m.field102 = Some(v.to_string()),
            F::Field103(v) => m.field103 = Some(v.to_string()),
            F::Field104(v) => m.field104 = Some(v),
            F::Field128(v) => m.field128 = Some(v),
            F::Field129(v) => m.field129 = Some(v.to_string()),
            F::Field130(v) => m.field130 = Some(v),
            F::Field131(v) => m.field131 = Some(v),
            F::Field150(v) => m.field150 = Some(v),
            F::Field271(v) => m.field271 = Some(v),
            F::Field272(v) => m.field272 = Some(v),
            F::Field280(v) => m.field280 = Some(v),
        }
    }
    m
}

// ---------------------------------------------------------------------------
// proto3
// ---------------------------------------------------------------------------

/// Same message with implicit presence. `Field<_, Plain<_>>::write` skips
/// default-valued fields, which is exactly what prost does, so no `if` needed.
fn tacky_encode_p3<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, m: &prost_p3::GoogleMessage1) {
    let s = tacky_p3::benchmarks::proto3::GoogleMessage1::schema();
    s.field1.write(buf, m.field1.as_str());
    s.field2.write(buf, m.field2);
    s.field3.write(buf, m.field3);
    s.field4.write(buf, m.field4.as_str());
    s.field5.write(buf, &m.field5);
    s.field6.write(buf, m.field6);
    s.field7.write(buf, m.field7.as_str());
    s.field9.write(buf, m.field9.as_str());
    s.field12.write(buf, m.field12);
    s.field13.write(buf, m.field13);
    s.field14.write(buf, m.field14);
    if let Some(sub) = &m.field15 {
        s.field15.write_msg(buf, |buf, t| {
            t.field1.write(buf, sub.field1);
            t.field2.write(buf, sub.field2);
            t.field3.write(buf, sub.field3);
            t.field12.write(buf, sub.field12);
            t.field13.write(buf, sub.field13);
            t.field14.write(buf, sub.field14);
            t.field15.write(buf, sub.field15.as_str());
            t.field16.write(buf, sub.field16);
            t.field19.write(buf, sub.field19);
            t.field20.write(buf, sub.field20);
            t.field21.write(buf, sub.field21);
            t.field22.write(buf, sub.field22);
            t.field23.write(buf, sub.field23);
            t.field28.write(buf, sub.field28);
            t.field203.write(buf, sub.field203);
            t.field204.write(buf, sub.field204);
            t.field205.write(buf, sub.field205.as_str());
            t.field206.write(buf, sub.field206);
            t.field207.write(buf, sub.field207);
            t.field300.write(buf, sub.field300);
        });
    }
    s.field16.write(buf, m.field16);
    s.field17.write(buf, m.field17);
    s.field18.write(buf, m.field18.as_str());
    s.field22.write(buf, m.field22);
    s.field23.write(buf, m.field23);
    s.field24.write(buf, m.field24);
    s.field25.write(buf, m.field25);
    s.field29.write(buf, m.field29);
    s.field30.write(buf, m.field30);
    s.field59.write(buf, m.field59);
    s.field60.write(buf, m.field60);
    s.field67.write(buf, m.field67);
    s.field68.write(buf, m.field68);
    s.field78.write(buf, m.field78);
    s.field80.write(buf, m.field80);
    s.field81.write(buf, m.field81);
    s.field100.write(buf, m.field100);
    s.field101.write(buf, m.field101);
    s.field102.write(buf, m.field102.as_str());
    s.field103.write(buf, m.field103.as_str());
    s.field104.write(buf, m.field104);
    s.field128.write(buf, m.field128);
    s.field129.write(buf, m.field129.as_str());
    s.field130.write(buf, m.field130);
    s.field131.write(buf, m.field131);
    s.field150.write(buf, m.field150);
    s.field271.write(buf, m.field271);
    s.field272.write(buf, m.field272);
    s.field280.write(buf, m.field280);
}

fn tacky_decode_p3(wire: &[u8]) -> prost_p3::GoogleMessage1 {
    use tacky_p3::benchmarks::proto3::{
        GoogleMessage1 as T, GoogleMessage1Field as F, GoogleMessage1SubMessageField as SF,
    };

    let mut m = prost_p3::GoogleMessage1::default();
    for field in T::decode(wire) {
        match field.unwrap() {
            F::Field1(v) => m.field1 = v.to_string(),
            F::Field2(v) => m.field2 = v,
            F::Field3(v) => m.field3 = v,
            F::Field4(v) => m.field4 = v.to_string(),
            // proto3 packs `repeated fixed64` by default, so this arrives as one
            // length-delimited run rather than a value per tag.
            F::Field5(iter) => m.field5.extend(iter.map(|r| r.unwrap())),
            F::Field6(v) => m.field6 = v,
            F::Field7(v) => m.field7 = v.to_string(),
            F::Field9(v) => m.field9 = v.to_string(),
            F::Field12(v) => m.field12 = v,
            F::Field13(v) => m.field13 = v,
            F::Field14(v) => m.field14 = v,
            F::Field15(fields) => {
                let sub = m.field15.get_or_insert_with(Default::default);
                for f in fields {
                    match f.unwrap() {
                        SF::Field1(v) => sub.field1 = v,
                        SF::Field2(v) => sub.field2 = v,
                        SF::Field3(v) => sub.field3 = v,
                        SF::Field12(v) => sub.field12 = v,
                        SF::Field13(v) => sub.field13 = v,
                        SF::Field14(v) => sub.field14 = v,
                        SF::Field15(v) => sub.field15 = v.to_string(),
                        SF::Field16(v) => sub.field16 = v,
                        SF::Field19(v) => sub.field19 = v,
                        SF::Field20(v) => sub.field20 = v,
                        SF::Field21(v) => sub.field21 = v,
                        SF::Field22(v) => sub.field22 = v,
                        SF::Field23(v) => sub.field23 = v,
                        SF::Field28(v) => sub.field28 = v,
                        SF::Field203(v) => sub.field203 = v,
                        SF::Field204(v) => sub.field204 = v,
                        SF::Field205(v) => sub.field205 = v.to_string(),
                        SF::Field206(v) => sub.field206 = v,
                        SF::Field207(v) => sub.field207 = v,
                        SF::Field300(v) => sub.field300 = v,
                    }
                }
            }
            F::Field16(v) => m.field16 = v,
            F::Field17(v) => m.field17 = v,
            F::Field18(v) => m.field18 = v.to_string(),
            F::Field22(v) => m.field22 = v,
            F::Field23(v) => m.field23 = v,
            F::Field24(v) => m.field24 = v,
            F::Field25(v) => m.field25 = v,
            F::Field29(v) => m.field29 = v,
            F::Field30(v) => m.field30 = v,
            F::Field59(v) => m.field59 = v,
            F::Field60(v) => m.field60 = v,
            F::Field67(v) => m.field67 = v,
            F::Field68(v) => m.field68 = v,
            F::Field78(v) => m.field78 = v,
            F::Field80(v) => m.field80 = v,
            F::Field81(v) => m.field81 = v,
            F::Field100(v) => m.field100 = v,
            F::Field101(v) => m.field101 = v,
            F::Field102(v) => m.field102 = v.to_string(),
            F::Field103(v) => m.field103 = v.to_string(),
            F::Field104(v) => m.field104 = v,
            F::Field128(v) => m.field128 = v,
            F::Field129(v) => m.field129 = v.to_string(),
            F::Field130(v) => m.field130 = v,
            F::Field131(v) => m.field131 = v,
            F::Field150(v) => m.field150 = v,
            F::Field271(v) => m.field271 = v,
            F::Field272(v) => m.field272 = v,
            F::Field280(v) => m.field280 = v,
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Benches
// ---------------------------------------------------------------------------

/// One encode group per syntax. `$encode` writes the whole batch back to back,
/// which is protobuf's own harness shape and keeps a single-payload dataset from
/// being measured as one branch-predictable message.
macro_rules! encode_group {
    ($c:expr, $name:literal, $dataset:expr, $msg:ty, $encode:expr, $cpp_kinds:expr) => {{
        let msgs: Vec<$msg> = payloads($dataset)
            .iter()
            .map(|p| <$msg>::decode(p.as_slice()).expect("payload decodes"))
            .collect();

        // Tacky's padded length prefixes rule out a byte compare, so check the
        // stronger thing per message: prost must decode tacky's output back to
        // the message it came from.
        let mut tacky_wire = Vec::with_capacity(4096);
        let mut prost_wire = Vec::with_capacity(4096);
        for m in &msgs {
            let mut one = Vec::with_capacity(1024);
            $encode(tacky::AnyDir::from_mut(&mut one), m);
            assert_eq!(
                &<$msg>::decode(one.as_slice()).unwrap(),
                m,
                concat!($name, ": prost cannot read back what tacky wrote")
            );
            tacky_wire.extend_from_slice(&one);
            m.encode(&mut prost_wire).unwrap();
        }

        let mut group = $c.benchmark_group(concat!("encode_", $name));
        group.throughput(Throughput::Bytes(prost_wire.len() as u64));
        let cap = tacky_wire.len().max(prost_wire.len());

        // One message at a time, so the check is per message rather than over a
        // concatenation whose order a prepending buffer reverses.
        {
            let mut backing = vec![0u8; cap + 1024];
            for m in &msgs {
                let mut rb = tacky::RevBuf::new(&mut backing);
                $encode(tacky::AnyDir::from_mut(&mut rb), m);
                assert_eq!(
                    &<$msg>::decode(rb.written()).unwrap(),
                    m,
                    concat!($name, ": reverse writer output does not decode back")
                );
            }
        }
        group.bench_function("tacky-slice", |b| {
            let mut backing = vec![0u8; cap + 1024];
            b.iter(|| {
                let mut sb = tacky::SliceBuf::new(&mut backing);
                for m in &msgs {
                    $encode(tacky::AnyDir::from_mut(&mut sb), m);
                }
                black_box(sb.written());
            });
        });
        group.bench_function("tacky-rev", |b| {
            let mut backing = vec![0u8; cap + 1024];
            b.iter(|| {
                let mut rb = tacky::RevBuf::new(&mut backing);
                // Reversed, so the concatenation lands in the same order as the forward
                // arm's — each message prepends ahead of the previous one.
                for m in msgs.iter().rev() {
                    $encode(tacky::AnyDir::from_mut(&mut rb), m);
                }
                black_box(rb.written());
            });
        });

        group.bench_function("tacky", |b| {
            let mut buf = Vec::with_capacity(cap);
            b.iter(|| {
                for m in &msgs {
                    $encode(tacky::AnyDir::from_mut(&mut buf), m);
                }
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        group.bench_function("prost", |b| {
            let mut buf = Vec::with_capacity(cap);
            b.iter(|| {
                for m in &msgs {
                    m.encode(&mut buf).unwrap();
                }
                black_box(buf.as_slice());
                buf.clear();
            });
        });
        #[cfg(feature = "cpp")]
        {
            assert_eq!(
                msgs.len(),
                1,
                concat!($name, ": C++ arm assumes a single-payload dataset")
            );
            for (label, kind) in $cpp_kinds {
                cpp_arms::bench_cpp_arms(&mut group, label, kind, &prost_wire);
            }
        }
        group.finish();
    }};
}

macro_rules! decode_group {
    ($c:expr, $name:literal, $dataset:expr, $msg:ty, $decode:expr) => {{
        let wires = payloads($dataset);

        for w in &wires {
            assert_eq!(
                $decode(w),
                <$msg>::decode(w.as_slice()).unwrap(),
                concat!($name, ": tacky and prost decode differently")
            );
        }

        let mut group = $c.benchmark_group(concat!("decode_", $name));
        group.throughput(Throughput::Bytes(
            wires.iter().map(|w| w.len() as u64).sum::<u64>(),
        ));

        group.bench_function("tacky", |b| {
            b.iter(|| {
                for w in &wires {
                    black_box($decode(black_box(w)));
                }
            });
        });
        group.bench_function("prost", |b| {
            b.iter(|| {
                for w in &wires {
                    black_box(<$msg>::decode(black_box(w.as_slice())).unwrap());
                }
            });
        });
        group.finish();
    }};
}

fn bench_message1(c: &mut Criterion) {
    encode_group!(
        c,
        "google_message1_proto2",
        DATASET_PROTO2,
        prost_p2::GoogleMessage1,
        tacky_encode_p2,
        // proto2: the C++ runtime never validates proto2 strings, so `cpp-cached`
        // already is its floor and there is no no-UTF8 arm to add.
        [("cpp", testing::cpp::MESSAGE1_PROTO2)]
    );
    encode_group!(
        c,
        "google_message1_proto3",
        DATASET_PROTO3,
        prost_p3::GoogleMessage1,
        tacky_encode_p3,
        [
            ("cpp", testing::cpp::MESSAGE1_PROTO3),
            ("cpp-noutf8", testing::cpp::MESSAGE1_PROTO3_NO_UTF8),
        ]
    );
    decode_group!(
        c,
        "google_message1_proto2",
        DATASET_PROTO2,
        prost_p2::GoogleMessage1,
        tacky_decode_p2
    );
    decode_group!(
        c,
        "google_message1_proto3",
        DATASET_PROTO3,
        prost_p3::GoogleMessage1,
        tacky_decode_p3
    );
}

criterion_group!(benches, bench_message1);
criterion_main!(benches);
