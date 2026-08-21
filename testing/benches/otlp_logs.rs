//! OTLP logs — `ExportLogsServiceRequest`, the highest-volume OTLP signal in practice.
//!
//! Structurally shallower than traces (`ResourceLogs` → `ScopeLogs` → `LogRecord`, and a
//! `LogRecord` is ten fields rather than a `Span`'s sixteen), but the byte mix is
//! different in ways that exercise different code:
//!
//! - **The body dominates.** In traces the payload is spread over many small attribute
//!   values; in logs one `body` string per record is usually the largest single field.
//!   That shifts weight from the length-prefix machinery onto plain `put_slice`.
//! - **Timestamps are `fixed64`, not varint.** Every record carries two of them
//!   (`time_unix_nano`, `observed_time_unix_nano`) plus a `fixed32` `flags`, so this is
//!   the only OTLP corpus that leans on the fixed-width write path.
//! - **One resource, not four.** A process exports its own logs, so the resource is
//!   amortised over the whole batch instead of over 50 spans.
//!
//! Schemas are vendored from `open-telemetry/opentelemetry-proto` tag **v1.3.2**,
//! matching the traces bench, upstream directory layout intact.
//!
//! As with traces, **there is no official OTLP payload dataset**, so this batch is
//! synthesised and the generator is part of the result. It shares its mixer and string
//! source with the traces corpus (`common/corpus.rs`), and follows the same rule about
//! where scattering is legitimate: log bodies are genuinely high-cardinality, so they
//! are scattered; severity texts, attribute keys and scope names are not, so they are
//! reused constants.
//!
//! Encode only. Correctness is still gated: prost must decode what tacky wrote, for both
//! writer directions.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prost::Message;

#[cfg(feature = "cpp")]
#[path = "common/cpp_arms.rs"]
mod cpp_arms;

#[path = "common/corpus.rs"]
mod corpus_gen;
use corpus_gen::{bytes, mix, scattered};

#[allow(dead_code)]
mod tacky_otlp_logs {
    include!(concat!(env!("OUT_DIR"), "/tacky_otlp_logs.rs"));
}
#[allow(dead_code)]
mod otlp {
    include!(concat!(env!("OUT_DIR"), "/otlp.rs"));
}

use otlp::opentelemetry::proto::collector::logs::v1 as pcol;
use otlp::opentelemetry::proto::common::v1 as pcommon;
use otlp::opentelemetry::proto::logs::v1 as plogs;
use otlp::opentelemetry::proto::resource::v1 as presource;
use tacky_otlp_logs::opentelemetry::proto::collector::logs::v1 as t;

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

/// 1 resource × 2 scopes × 256 records = 512 records, which is the SDK
/// `BatchLogRecordProcessor` default `max_export_batch_size`. Two scopes because a
/// process logs through more than one logger; the collector's 8192-item trigger is
/// another 16× beyond this and is left to the size trend rather than benched.
const RESOURCES: usize = 1;
const SCOPES_PER_RESOURCE: usize = 2;
const RECORDS_PER_SCOPE: usize = 256;

/// See the traces bench: 18 resource attributes is what the SDK, host, process, k8s and
/// cloud detectors contribute between them. Log records carry fewer attributes than
/// spans — structured logging adds a handful of its own on top of the semconv code and
/// thread keys — so 6 rather than a span's 11.
const RESOURCE_ATTRS: usize = 18;
const RECORD_ATTRS: usize = 6;

/// Log bodies: `[20, 220)` bytes, scattered. This is the one field in the corpus that is
/// genuinely high-cardinality in real traffic — a templated message with the parameters
/// substituted in — and it is also the largest, which is why the spread runs well past
/// the ~128 B point where an inline-store copy stops beating `memcpy`.
///
/// It is text, so the floor is a short-but-real line ("shutting down", "cache miss") at
/// 20 B rather than the 4 B an attribute value can be; the spread reaches past 200 B
/// because a stack trace or a request dump is one line too. Attribute values are a
/// different thing entirely and keep the traces bench's `[4, 64)`.
const BODY_LEN: (usize, usize) = (20, 200);
const VALUE_LEN: (usize, usize) = (4, 60);

/// Severity distribution, by count out of 16: 11 INFO, 2 DEBUG, 2 WARN, 1 ERROR. Real
/// log volume is overwhelmingly INFO, and the number/text pair is a low-cardinality
/// constant, not something to scatter.
const SEVERITIES: [(i32, &str); 16] = [
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (9, "INFO"),
    (5, "DEBUG"),
    (5, "DEBUG"),
    (13, "WARN"),
    (13, "WARN"),
    (17, "ERROR"),
];

/// Log-record attribute keys: the semconv code and thread keys an instrumented logger
/// attaches, plus the two an application typically adds itself.
const RECORD_KEYS: [&str; RECORD_ATTRS] = [
    "code.function",
    "code.filepath",
    "code.lineno",
    "thread.name",
    "log.record.uid",
    "enduser.id",
];

/// As the traces bench. Kept as its own list rather than shared, because the two signals
/// are free to disagree about what a resource looks like and pretending otherwise would
/// hide it.
const RESOURCE_KEYS: [&str; RESOURCE_ATTRS] = [
    "service.name",
    "service.version",
    "service.namespace",
    "service.instance.id",
    "telemetry.sdk.name",
    "telemetry.sdk.language",
    "telemetry.sdk.version",
    "telemetry.distro.version",
    "host.name",
    "host.arch",
    "os.type",
    "os.version",
    "process.pid",
    "process.runtime.name",
    "process.runtime.version",
    "k8s.namespace.name",
    "k8s.pod.name",
    "k8s.node.name",
];

/// A log record is correlated with a span 5 times in 8: inside a request handler it is,
/// outside one (startup, background jobs, the reconcile loop) it is not, and then
/// `trace_id`/`span_id` are absent rather than zeroed.
const CORRELATED_IN_8: u64 = 5;

/// One attribute under `key`. 75% strings, as in the traces corpus.
fn attr(key: &str, i: u64) -> pcommon::KeyValue {
    use pcommon::any_value::Value;
    let value = match mix(i ^ 1) % 16 {
        0..=11 => Value::StringValue(scattered(i, VALUE_LEN)),
        12 | 13 => Value::IntValue((mix(i ^ 2) % 100_000) as i64),
        14 => Value::BoolValue(mix(i ^ 3) % 2 == 0),
        _ => Value::DoubleValue(mix(i ^ 4) as f64 / 1e15),
    };
    pcommon::KeyValue {
        key: key.to_string(),
        value: Some(pcommon::AnyValue { value: Some(value) }),
    }
}

fn corpus() -> pcol::ExportLogsServiceRequest {
    use pcommon::any_value::Value;

    let mut resource_logs = Vec::with_capacity(RESOURCES);
    for r in 0..RESOURCES as u64 {
        let attributes: Vec<pcommon::KeyValue> = RESOURCE_KEYS
            .iter()
            .enumerate()
            .map(|(a, k)| attr(k, r * 1_000 + a as u64))
            .collect();

        let mut scope_logs = Vec::with_capacity(SCOPES_PER_RESOURCE);
        for s in 0..SCOPES_PER_RESOURCE as u64 {
            let records = (0..RECORDS_PER_SCOPE as u64)
                .map(|i| {
                    let seed = r * 1_000_000 + s * 10_000 + i * 100;
                    let t = 1_700_000_000_000_000_000u64 + mix(seed) % 1_000_000_000;
                    let (severity_number, severity_text) =
                        SEVERITIES[mix(seed ^ 5) as usize % SEVERITIES.len()];
                    let correlated = mix(seed ^ 6) % 8 < CORRELATED_IN_8;
                    plogs::LogRecord {
                        time_unix_nano: t,
                        // The gap between emit and collection: microseconds, not zero.
                        observed_time_unix_nano: t + mix(seed ^ 9) % 500_000,
                        severity_number,
                        severity_text: severity_text.to_string(),
                        body: Some(pcommon::AnyValue {
                            value: Some(Value::StringValue(scattered(seed, BODY_LEN))),
                        }),
                        attributes: RECORD_KEYS
                            .iter()
                            .enumerate()
                            .map(|(a, k)| attr(k, seed + a as u64))
                            .collect(),
                        dropped_attributes_count: 0,
                        flags: if correlated { 1 } else { 0 },
                        trace_id: if correlated {
                            bytes(seed, 16)
                        } else {
                            Vec::new()
                        },
                        span_id: if correlated {
                            bytes(seed ^ 0xA5, 8)
                        } else {
                            Vec::new()
                        },
                    }
                })
                .collect();

            scope_logs.push(plogs::ScopeLogs {
                scope: Some(pcommon::InstrumentationScope {
                    name: format!("com.example.orders.Handler-{s}"),
                    version: "1.32.0".to_string(),
                    attributes: Vec::new(),
                    dropped_attributes_count: 0,
                }),
                log_records: records,
                schema_url: "https://opentelemetry.io/schemas/1.24.0".to_string(),
            });
        }

        resource_logs.push(plogs::ResourceLogs {
            resource: Some(presource::Resource {
                attributes,
                dropped_attributes_count: 0,
            }),
            scope_logs,
            schema_url: "https://opentelemetry.io/schemas/1.24.0".to_string(),
        });
    }

    pcol::ExportLogsServiceRequest { resource_logs }
}

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------
//
// Fields go out in ascending tag order, which is the order prost emits.

fn tacky_encode<B: tacky::WriteBuf>(
    buf: &mut tacky::AnyDir<B>,
    req: &pcol::ExportLogsServiceRequest,
) {
    let s = t::ExportLogsServiceRequest::schema();
    s.resource_logs
        .write_msgs(buf, &req.resource_logs, |buf, s, rl| {
            if let Some(r) = &rl.resource {
                s.resource.write_msg(buf, |buf, s| {
                    s.attributes
                        .write_msgs(buf, &r.attributes, |buf, _, a| write_kv(buf, a));
                    s.dropped_attributes_count
                        .write(buf, r.dropped_attributes_count);
                });
            }
            s.scope_logs.write_msgs(buf, &rl.scope_logs, |buf, s, sl| {
                if let Some(sc) = &sl.scope {
                    s.scope.write_msg(buf, |buf, s| {
                        s.name.write(buf, sc.name.as_str());
                        s.version.write(buf, sc.version.as_str());
                        s.attributes
                            .write_msgs(buf, &sc.attributes, |buf, _, a| write_kv(buf, a));
                        s.dropped_attributes_count
                            .write(buf, sc.dropped_attributes_count);
                    });
                }
                s.log_records
                    .write_msgs(buf, &sl.log_records, |buf, _, rec| write_record(buf, rec));
                s.schema_url.write(buf, sl.schema_url.as_str());
            });
            s.schema_url.write(buf, rl.schema_url.as_str());
        });
}

fn write_record<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, rec: &plogs::LogRecord) {
    let s = t::LogRecord::schema();
    s.time_unix_nano.write(buf, rec.time_unix_nano);
    s.severity_number
        .write(buf, t::SeverityNumber::from(rec.severity_number));
    s.severity_text.write(buf, rec.severity_text.as_str());
    if let Some(b) = &rec.body {
        s.body.write_msg(buf, |buf, _| write_any(buf, b));
    }
    s.attributes
        .write_msgs(buf, &rec.attributes, |buf, _, a| write_kv(buf, a));
    s.dropped_attributes_count
        .write(buf, rec.dropped_attributes_count);
    s.flags.write(buf, rec.flags);
    s.trace_id.write(buf, rec.trace_id.as_slice());
    s.span_id.write(buf, rec.span_id.as_slice());
    s.observed_time_unix_nano
        .write(buf, rec.observed_time_unix_nano);
}

fn write_kv<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, kv: &pcommon::KeyValue) {
    let s = t::KeyValue::schema();
    s.key.write(buf, kv.key.as_str());
    if let Some(v) = &kv.value {
        s.value.write_msg(buf, |buf, _| write_any(buf, v));
    }
}

/// Recursive through `ArrayValue`/`KeyValueList`. An unset `AnyValue.value` writes
/// nothing, matching prost.
fn write_any<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, v: &pcommon::AnyValue) {
    use pcommon::any_value::Value;
    let s = t::AnyValue::schema();
    match &v.value {
        None => {}
        Some(Value::StringValue(x)) => {
            s.value.write_string_value(buf, x.as_str());
        }
        Some(Value::BoolValue(x)) => {
            s.value.write_bool_value(buf, *x);
        }
        Some(Value::IntValue(x)) => {
            s.value.write_int_value(buf, *x);
        }
        Some(Value::DoubleValue(x)) => {
            s.value.write_double_value(buf, *x);
        }
        Some(Value::BytesValue(x)) => {
            s.value.write_bytes_value(buf, x.as_slice());
        }
        Some(Value::ArrayValue(a)) => {
            s.value.write_array_value_msg(buf, |buf, s| {
                s.values
                    .write_msgs(buf, &a.values, |buf, _, inner| write_any(buf, inner));
            });
        }
        Some(Value::KvlistValue(kvl)) => {
            s.value.write_kvlist_value_msg(buf, |buf, s| {
                s.values
                    .write_msgs(buf, &kvl.values, |buf, _, inner| write_kv(buf, inner));
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Bench
// ---------------------------------------------------------------------------

fn bench_otlp_logs(c: &mut Criterion) {
    let req = corpus();
    let records = RESOURCES * SCOPES_PER_RESOURCE * RECORDS_PER_SCOPE;

    let mut prost_wire = Vec::with_capacity(req.encoded_len());
    req.encode(&mut prost_wire).unwrap();
    let mut tacky_wire = Vec::with_capacity(prost_wire.len() * 2);
    tacky_encode(tacky::AnyDir::from_mut(&mut tacky_wire), &req);

    // Tacky's padded length prefixes rule out a byte compare, so check the stronger
    // thing: prost must decode tacky's output back to the same message.
    assert_eq!(
        pcol::ExportLogsServiceRequest::decode(tacky_wire.as_slice()).unwrap(),
        req,
        "otlp_logs: prost cannot read back what tacky wrote"
    );

    println!(
        "otlp_logs: {records} records, prost {} B, tacky {} B (+{:.2}%), bodies {}..{} B, \
         {RECORD_ATTRS} record attrs / {RESOURCE_ATTRS} resource attrs, \
         {CORRELATED_IN_8}/8 correlated with a span",
        prost_wire.len(),
        tacky_wire.len(),
        (tacky_wire.len() as f64 / prost_wire.len() as f64 - 1.0) * 100.0,
        BODY_LEN.0,
        BODY_LEN.0 + BODY_LEN.1,
    );

    let cap = tacky_wire.len().max(prost_wire.len());
    let mut group = c.benchmark_group("encode_otlp_logs");
    group.throughput(Throughput::Bytes(prost_wire.len() as u64));
    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            tacky_encode(tacky::AnyDir::from_mut(&mut buf), &req);
            black_box(buf.as_slice());
            buf.clear();
        });
    });
    group.bench_function("prost", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            req.encode(&mut buf).unwrap();
            black_box(buf.as_slice());
            buf.clear();
        });
    });

    // A downward buffer emits fields in the reverse of the order they are written, which
    // is legal, so this is checked by decoding rather than by comparing bytes.
    let mut rev_backing = vec![0u8; cap + 4096];
    let mut rb = tacky::RevBuf::new(&mut rev_backing);
    tacky_encode(tacky::AnyDir::from_mut(&mut rb), &req);
    assert_eq!(
        pcol::ExportLogsServiceRequest::decode(rb.written()).unwrap(),
        req,
        "reverse writer output does not decode back to the same message"
    );
    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; cap + 4096];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode(tacky::AnyDir::from_mut(&mut rb), &req);
            black_box(rb.written());
        });
    });
    // proto3, so the fair arm is `cpp-noutf8`; see the note on `encode_arms` in
    // `benches/otlp_traces.rs`.
    #[cfg(feature = "cpp")]
    cpp_arms::bench_cpp_arms(
        &mut group,
        "cpp-noutf8",
        testing::cpp::OTLP_LOGS_NO_UTF8,
        &prost_wire,
    );
    group.finish();
}

criterion_group!(benches, bench_otlp_logs);
criterion_main!(benches);
