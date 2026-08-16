//! OTLP traces — `ExportTraceServiceRequest`, the shape tacky was built for.
//!
//! Three levels of nesting (`ResourceSpans` → `ScopeSpans` → `Span`), then two
//! more for every attribute (`KeyValue` → `AnyValue`, a oneof). Almost every field
//! is a repeated message wrapping a handful of short strings and small varints, so
//! the length-prefix work dominates and there is very little payload to hide it
//! behind. Of the three targets in this suite this is the one closest to real
//! telemetry traffic.
//!
//! Schemas are vendored from `open-telemetry/opentelemetry-proto` tag **v1.3.2**,
//! upstream directory layout intact, under `testing/protos/opentelemetry/`.
//!
//! **There is no official OTLP payload dataset**, so this batch is synthesised.
//! That makes the generator part of the result, not scaffolding, so it is spelled
//! out below: every count is a named constant, every string length is scattered by
//! a fixed integer mixer, and nothing depends on a clock or an RNG. Two traps this
//! repo has already fallen into are avoided deliberately:
//!
//! - Every string is a freshly allocated `String` cut from a scattered offset, not
//!   one `&str` reused. Reusing one source keeps every copy in a single L1 line and
//!   measures the cache-resident best case.
//! - Lengths are scattered, not constant. A constant length makes any size-dispatch
//!   branch predict perfectly; last time that turned a real 2.1x into an apparent
//!   4.0x. The spread is reported with the numbers because the answer depends on it.
//!
//! Arms are the same as the other targets:
//!
//! - `tacky` — one pass, no sizing.
//! - `prost` — `Message::encode`, which sizes internally. prost's published shape.
//! - `cpp` / `cpp-cached` / `cpp-noutf8…` under `--features cpp`.
//!
//! Wire output is checked by decoding tacky's bytes with prost and comparing
//! messages rather than by comparing byte strings: tacky pads a nested message's length
//! prefix to the placeholder width it reserved, so the two encoders agree on content but
//! not always on bytes.
//!
//! [`bench_otlp_value_len`] sweeps the attribute-value length, which is the axis this
//! corpus is most sensitive to.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use prost::Message;

#[cfg(feature = "cpp")]
#[path = "common/cpp_arms.rs"]
mod cpp_arms;

#[allow(dead_code)]
mod tacky_otlp {
    include!(concat!(env!("OUT_DIR"), "/tacky_otlp.rs"));
}
#[allow(dead_code)]
mod otlp {
    include!(concat!(env!("OUT_DIR"), "/otlp.rs"));
}

// tacky-build inlines a file's imports, so the whole tree lives in one module.
use tacky_otlp::opentelemetry::proto::collector::trace::v1 as t;
// prost keeps one module per proto package.
use otlp::opentelemetry::proto::collector::trace::v1 as pcol;
use otlp::opentelemetry::proto::common::v1 as pcommon;
use otlp::opentelemetry::proto::resource::v1 as presource;
use otlp::opentelemetry::proto::trace::v1 as ptrace;

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

/// 4 resources × 2 scopes × 25 spans = 200 spans, ~90 KB on the wire. Sized to sit
/// well outside L1 so the encode loop is not measured against a hot buffer, and to
/// stay small enough that the criterion sample count is still meaningful.
const RESOURCES: usize = 4;
const SCOPES_PER_RESOURCE: usize = 2;
const SPANS_PER_SCOPE: usize = 25;

/// Attribute counts, chosen to match what an instrumented HTTP/DB service actually
/// emits rather than to flatter either encoder.
const RESOURCE_ATTRS: usize = 8;
const SCOPE_ATTRS: usize = 2;
const SPAN_ATTRS: usize = 6;
const EVENTS_PER_SPAN: usize = 2;
const EVENT_ATTRS: usize = 2;
const LINKS_PER_SPAN: usize = 1;
const LINK_ATTRS: usize = 1;

/// String-length spreads, reported with the results. Span names land in
/// `[8, 48)` bytes and attribute string values in `[4, 64)`, both scattered by
/// [`mix`]. Every one of these is below the ~128 B point where an inline-store copy
/// stops beating `memcpy`, which is exactly the regime this corpus is meant to probe.
const NAME_LEN: (usize, usize) = (8, 40);
const VALUE_LEN: (usize, usize) = (4, 60);

/// Every 10th span carries an error `Status`; the rest leave it unset, as
/// instrumentation does.
const ERROR_EVERY: u64 = 10;

/// splitmix64. Deterministic on purpose — the corpus has to be byte-identical on
/// every machine and every run, so no clock and no RNG.
fn mix(i: u64) -> u64 {
    let mut x = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// 128 printable ASCII bytes to cut scattered-length strings out of. Slicing at a
/// scattered *offset* as well as a scattered length keeps the contents distinct, so
/// two strings of the same length are still different bytes.
const SRC: &str = "GET/api/v2/orders?status=open&limit=50 POST/api/v2/orders/12345/items \
                   svc-checkout-7f9c4d8b6-x2qlm eu-central-1b node-14 build-2f8a1c";

/// A freshly allocated `String` of scattered length in `[min, min + spread)`.
///
/// Wraps around [`SRC`] rather than slicing it, so the value-length sweep can ask for
/// lengths past its 128 B. ASCII throughout, so any byte window is valid UTF-8.
fn scattered(i: u64, (min, spread): (usize, usize)) -> String {
    let len = min + mix(i) as usize % spread;
    let off = mix(i ^ 0x5BF0_3635) as usize % SRC.len();
    SRC.bytes()
        .cycle()
        .skip(off)
        .take(len)
        .map(char::from)
        .collect()
}

/// Realistic attribute keys. Reused by name across spans — that part *is* how OTLP
/// looks, and both encoders pay the same copy for it.
const ATTR_KEYS: [&str; 12] = [
    "http.request.method",
    "http.route",
    "http.response.status_code",
    "url.full",
    "server.address",
    "server.port",
    "db.system",
    "db.statement",
    "rpc.service",
    "rpc.method",
    "messaging.destination.name",
    "user_agent.original",
];

/// One attribute, cycling string / int / bool / double so no single `AnyValue`
/// variant dominates. `value_len` is a parameter rather than [`VALUE_LEN`] so
/// [`bench_otlp_value_len`] can sweep it.
fn attr(i: u64, value_len: (usize, usize)) -> pcommon::KeyValue {
    use pcommon::any_value::Value;
    let value = match mix(i ^ 1) % 4 {
        0 => Value::StringValue(scattered(i, value_len)),
        1 => Value::IntValue((mix(i ^ 2) % 100_000) as i64),
        2 => Value::BoolValue(mix(i ^ 3) % 2 == 0),
        _ => Value::DoubleValue(mix(i ^ 4) as f64 / 1e15),
    };
    pcommon::KeyValue {
        key: ATTR_KEYS[mix(i) as usize % ATTR_KEYS.len()].to_string(),
        value: Some(pcommon::AnyValue { value: Some(value) }),
    }
}

fn bytes(i: u64, n: usize) -> Vec<u8> {
    (0..n as u64).map(|k| mix(i ^ k) as u8).collect()
}

/// Builds the whole batch. `ArrayValue` and `KeyValueList` each appear exactly
/// once per resource and per scope respectively — rare in real traffic, but the
/// nested-oneof path is worth exercising at all rather than not at all.
fn corpus(value_len: (usize, usize)) -> pcol::ExportTraceServiceRequest {
    use pcommon::any_value::Value;

    let mut resource_spans = Vec::with_capacity(RESOURCES);
    for r in 0..RESOURCES as u64 {
        let mut attributes: Vec<pcommon::KeyValue> = (0..RESOURCE_ATTRS as u64)
            .map(|a| attr(r * 1_000 + a, value_len))
            .collect();
        attributes.push(pcommon::KeyValue {
            key: "process.command_args".to_string(),
            value: Some(pcommon::AnyValue {
                value: Some(Value::ArrayValue(pcommon::ArrayValue {
                    values: (0..3)
                        .map(|k| pcommon::AnyValue {
                            value: Some(Value::StringValue(scattered(r * 97 + k, value_len))),
                        })
                        .collect(),
                })),
            }),
        });

        let mut scope_spans = Vec::with_capacity(SCOPES_PER_RESOURCE);
        for s in 0..SCOPES_PER_RESOURCE as u64 {
            let seed = r * 10_000 + s * 1_000;
            let mut scope_attrs: Vec<pcommon::KeyValue> = (0..SCOPE_ATTRS as u64)
                .map(|a| attr(seed + a, value_len))
                .collect();
            scope_attrs.push(pcommon::KeyValue {
                key: "otel.scope.config".to_string(),
                value: Some(pcommon::AnyValue {
                    value: Some(Value::KvlistValue(pcommon::KeyValueList {
                        values: (0..2).map(|k| attr(seed + 500 + k, value_len)).collect(),
                    })),
                }),
            });

            let spans = (0..SPANS_PER_SCOPE as u64)
                .map(|i| {
                    let seed = r * 1_000_000 + s * 10_000 + i * 100;
                    let start = 1_700_000_000_000_000_000u64 + mix(seed) % 1_000_000_000;
                    ptrace::Span {
                        trace_id: bytes(seed, 16),
                        span_id: bytes(seed ^ 0xA5, 8),
                        trace_state: String::new(),
                        parent_span_id: if i == 0 {
                            Vec::new()
                        } else {
                            bytes(seed ^ 0x5A, 8)
                        },
                        flags: 1,
                        name: scattered(seed, NAME_LEN),
                        kind: (1 + mix(seed ^ 7) % 5) as i32,
                        start_time_unix_nano: start,
                        end_time_unix_nano: start + mix(seed ^ 8) % 50_000_000,
                        attributes: (0..SPAN_ATTRS as u64)
                            .map(|a| attr(seed + a, value_len))
                            .collect(),
                        dropped_attributes_count: 0,
                        events: (0..EVENTS_PER_SPAN as u64)
                            .map(|e| ptrace::span::Event {
                                time_unix_nano: start + e * 1_000_000,
                                name: scattered(seed + 40 + e, NAME_LEN),
                                attributes: (0..EVENT_ATTRS as u64)
                                    .map(|a| attr(seed + 50 + e * 10 + a, value_len))
                                    .collect(),
                                dropped_attributes_count: 0,
                            })
                            .collect(),
                        dropped_events_count: 0,
                        links: (0..LINKS_PER_SPAN as u64)
                            .map(|l| ptrace::span::Link {
                                trace_id: bytes(seed ^ 0x11 ^ l, 16),
                                span_id: bytes(seed ^ 0x22 ^ l, 8),
                                trace_state: String::new(),
                                attributes: (0..LINK_ATTRS as u64)
                                    .map(|a| attr(seed + 70 + l * 10 + a, value_len))
                                    .collect(),
                                dropped_attributes_count: 0,
                                flags: 1,
                            })
                            .collect(),
                        dropped_links_count: 0,
                        status: (i % ERROR_EVERY == 0).then(|| ptrace::Status {
                            message: scattered(seed + 90, NAME_LEN),
                            code: 2,
                        }),
                    }
                })
                .collect();

            scope_spans.push(ptrace::ScopeSpans {
                scope: Some(pcommon::InstrumentationScope {
                    name: format!("io.opentelemetry.instrumentation.scope-{r}-{s}"),
                    version: "1.32.0".to_string(),
                    attributes: scope_attrs,
                    dropped_attributes_count: 0,
                }),
                spans,
                schema_url: "https://opentelemetry.io/schemas/1.24.0".to_string(),
            });
        }

        resource_spans.push(ptrace::ResourceSpans {
            resource: Some(presource::Resource {
                attributes,
                dropped_attributes_count: 0,
            }),
            scope_spans,
            schema_url: "https://opentelemetry.io/schemas/1.24.0".to_string(),
        });
    }

    pcol::ExportTraceServiceRequest { resource_spans }
}

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------
//
// Fields go out in ascending tag order, which is the order prost emits, so the two
// outputs differ only where tacky pads a length prefix.

fn tacky_encode(buf: &mut impl tacky::WriteBuf, req: &pcol::ExportTraceServiceRequest) {
    let s = t::ExportTraceServiceRequest::schema();
    s.resource_spans
        .write_msgs(buf, &req.resource_spans, |buf, s, rs| {
            if let Some(r) = &rs.resource {
                s.resource.write_msg(buf, |buf, s| {
                    s.attributes
                        .write_msgs(buf, &r.attributes, |buf, _, a| write_kv(buf, a));
                    s.dropped_attributes_count
                        .write(buf, r.dropped_attributes_count);
                });
            }
            s.scope_spans
                .write_msgs(buf, &rs.scope_spans, |buf, s, ss| {
                    if let Some(sc) = &ss.scope {
                        s.scope.write_msg(buf, |buf, s| {
                            s.name.write(buf, sc.name.as_str());
                            s.version.write(buf, sc.version.as_str());
                            s.attributes
                                .write_msgs(buf, &sc.attributes, |buf, _, a| write_kv(buf, a));
                            s.dropped_attributes_count
                                .write(buf, sc.dropped_attributes_count);
                        });
                    }
                    s.spans
                        .write_msgs(buf, &ss.spans, |buf, _, span| write_span(buf, span));
                    s.schema_url.write(buf, ss.schema_url.as_str());
                });
            s.schema_url.write(buf, rs.schema_url.as_str());
        });
}

fn write_span(buf: &mut impl tacky::WriteBuf, span: &ptrace::Span) {
    let s = t::Span::schema();
    s.trace_id.write(buf, span.trace_id.as_slice());
    s.span_id.write(buf, span.span_id.as_slice());
    s.trace_state.write(buf, span.trace_state.as_str());
    s.parent_span_id.write(buf, span.parent_span_id.as_slice());
    s.name.write(buf, span.name.as_str());
    s.kind.write(buf, t::SpanSpanKind::from(span.kind));
    s.start_time_unix_nano.write(buf, span.start_time_unix_nano);
    s.end_time_unix_nano.write(buf, span.end_time_unix_nano);
    s.attributes
        .write_msgs(buf, &span.attributes, |buf, _, a| write_kv(buf, a));
    s.dropped_attributes_count
        .write(buf, span.dropped_attributes_count);
    s.events.write_msgs(buf, &span.events, |buf, s, e| {
        s.time_unix_nano.write(buf, e.time_unix_nano);
        s.name.write(buf, e.name.as_str());
        s.attributes
            .write_msgs(buf, &e.attributes, |buf, _, a| write_kv(buf, a));
        s.dropped_attributes_count
            .write(buf, e.dropped_attributes_count);
    });
    s.dropped_events_count.write(buf, span.dropped_events_count);
    s.links.write_msgs(buf, &span.links, |buf, s, l| {
        s.trace_id.write(buf, l.trace_id.as_slice());
        s.span_id.write(buf, l.span_id.as_slice());
        s.trace_state.write(buf, l.trace_state.as_str());
        s.attributes
            .write_msgs(buf, &l.attributes, |buf, _, a| write_kv(buf, a));
        s.dropped_attributes_count
            .write(buf, l.dropped_attributes_count);
        s.flags.write(buf, l.flags);
    });
    s.dropped_links_count.write(buf, span.dropped_links_count);
    if let Some(st) = &span.status {
        s.status.write_msg(buf, |buf, s| {
            s.message.write(buf, st.message.as_str());
            s.code.write(buf, t::StatusStatusCode::from(st.code));
        });
    }
    s.flags.write(buf, span.flags);
}

fn write_kv(buf: &mut impl tacky::WriteBuf, kv: &pcommon::KeyValue) {
    let s = t::KeyValue::schema();
    s.key.write(buf, kv.key.as_str());
    if let Some(v) = &kv.value {
        s.value.write_msg(buf, |buf, _| write_any(buf, v));
    }
}

/// Recursive through `ArrayValue`/`KeyValueList`. An unset `AnyValue.value` writes
/// nothing, matching prost.
fn write_any(buf: &mut impl tacky::WriteBuf, v: &pcommon::AnyValue) {
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
        Some(Value::BytesValue(x)) => {
            s.value.write_bytes_value(buf, x.as_slice());
        }
    }
}

// ---------------------------------------------------------------------------
// Decode
// ---------------------------------------------------------------------------
//
// Both arms end up owning the same `prost` value, so the only difference measured
// is the parser.

fn tacky_decode(wire: &[u8]) -> pcol::ExportTraceServiceRequest {
    let mut req = pcol::ExportTraceServiceRequest::default();
    for field in t::ExportTraceServiceRequest::decode(wire) {
        match field.unwrap() {
            t::ExportTraceServiceRequestField::ResourceSpans(fields) => {
                req.resource_spans.push(read_resource_spans(fields))
            }
        }
    }
    req
}

fn read_resource_spans(fields: t::ResourceSpansFields<'_>) -> ptrace::ResourceSpans {
    use t::ResourceSpansField as F;

    let mut rs = ptrace::ResourceSpans::default();
    for field in fields {
        match field.unwrap() {
            F::Resource(res) => {
                use t::ResourceField as R;
                let r = rs.resource.get_or_insert_with(Default::default);
                for f in res {
                    match f.unwrap() {
                        R::Attributes(kv) => r.attributes.push(read_kv(kv)),
                        R::DroppedAttributesCount(v) => r.dropped_attributes_count = v,
                    }
                }
            }
            F::ScopeSpans(ss) => rs.scope_spans.push(read_scope_spans(ss)),
            F::SchemaUrl(v) => rs.schema_url = v.to_string(),
        }
    }
    rs
}

fn read_scope_spans(fields: t::ScopeSpansFields<'_>) -> ptrace::ScopeSpans {
    use t::ScopeSpansField as F;

    let mut ss = ptrace::ScopeSpans::default();
    for field in fields {
        match field.unwrap() {
            F::Scope(scope) => {
                use t::InstrumentationScopeField as S;
                let sc = ss.scope.get_or_insert_with(Default::default);
                for f in scope {
                    match f.unwrap() {
                        S::Name(v) => sc.name = v.to_string(),
                        S::Version(v) => sc.version = v.to_string(),
                        S::Attributes(kv) => sc.attributes.push(read_kv(kv)),
                        S::DroppedAttributesCount(v) => sc.dropped_attributes_count = v,
                    }
                }
            }
            F::Spans(span) => ss.spans.push(read_span(span)),
            F::SchemaUrl(v) => ss.schema_url = v.to_string(),
        }
    }
    ss
}

fn read_span(fields: t::SpanFields<'_>) -> ptrace::Span {
    use t::SpanField as F;

    let mut span = ptrace::Span::default();
    for field in fields {
        match field.unwrap() {
            F::TraceId(v) => span.trace_id = v.to_vec(),
            F::SpanId(v) => span.span_id = v.to_vec(),
            F::TraceState(v) => span.trace_state = v.to_string(),
            F::ParentSpanId(v) => span.parent_span_id = v.to_vec(),
            F::Flags(v) => span.flags = v,
            F::Name(v) => span.name = v.to_string(),
            F::Kind(v) => span.kind = v.into(),
            F::StartTimeUnixNano(v) => span.start_time_unix_nano = v,
            F::EndTimeUnixNano(v) => span.end_time_unix_nano = v,
            F::Attributes(kv) => span.attributes.push(read_kv(kv)),
            F::DroppedAttributesCount(v) => span.dropped_attributes_count = v,
            F::Events(ev) => {
                use t::SpanEventField as E;
                let mut e = ptrace::span::Event::default();
                for f in ev {
                    match f.unwrap() {
                        E::TimeUnixNano(v) => e.time_unix_nano = v,
                        E::Name(v) => e.name = v.to_string(),
                        E::Attributes(kv) => e.attributes.push(read_kv(kv)),
                        E::DroppedAttributesCount(v) => e.dropped_attributes_count = v,
                    }
                }
                span.events.push(e);
            }
            F::DroppedEventsCount(v) => span.dropped_events_count = v,
            F::Links(ln) => {
                use t::SpanLinkField as L;
                let mut l = ptrace::span::Link::default();
                for f in ln {
                    match f.unwrap() {
                        L::TraceId(v) => l.trace_id = v.to_vec(),
                        L::SpanId(v) => l.span_id = v.to_vec(),
                        L::TraceState(v) => l.trace_state = v.to_string(),
                        L::Attributes(kv) => l.attributes.push(read_kv(kv)),
                        L::DroppedAttributesCount(v) => l.dropped_attributes_count = v,
                        L::Flags(v) => l.flags = v,
                    }
                }
                span.links.push(l);
            }
            F::DroppedLinksCount(v) => span.dropped_links_count = v,
            F::Status(st) => {
                use t::StatusField as S;
                let s = span.status.get_or_insert_with(Default::default);
                for f in st {
                    match f.unwrap() {
                        S::Message(v) => s.message = v.to_string(),
                        S::Code(v) => s.code = v.into(),
                    }
                }
            }
        }
    }
    span
}

fn read_kv(fields: t::KeyValueFields<'_>) -> pcommon::KeyValue {
    use t::KeyValueField as F;

    let mut kv = pcommon::KeyValue::default();
    for field in fields {
        match field.unwrap() {
            F::Key(v) => kv.key = v.to_string(),
            F::Value(v) => kv.value = Some(read_any(v)),
        }
    }
    kv
}

fn read_any(fields: t::AnyValueFields<'_>) -> pcommon::AnyValue {
    use pcommon::any_value::Value;
    use t::AnyValueField as F;

    let mut any = pcommon::AnyValue::default();
    for field in fields {
        any.value = Some(match field.unwrap() {
            F::StringValue(v) => Value::StringValue(v.to_string()),
            F::BoolValue(v) => Value::BoolValue(v),
            F::IntValue(v) => Value::IntValue(v),
            F::DoubleValue(v) => Value::DoubleValue(v),
            F::BytesValue(v) => Value::BytesValue(v.to_vec()),
            F::ArrayValue(inner) => {
                use t::ArrayValueField as A;
                let mut a = pcommon::ArrayValue::default();
                for f in inner {
                    match f.unwrap() {
                        A::Values(v) => a.values.push(read_any(v)),
                    }
                }
                Value::ArrayValue(a)
            }
            F::KvlistValue(inner) => {
                use t::KeyValueListField as K;
                let mut kvl = pcommon::KeyValueList::default();
                for f in inner {
                    match f.unwrap() {
                        K::Values(v) => kvl.values.push(read_kv(v)),
                    }
                }
                Value::KvlistValue(kvl)
            }
        });
    }
    any
}

// ---------------------------------------------------------------------------
// Benches
// ---------------------------------------------------------------------------

fn bench_otlp(c: &mut Criterion) {
    let req = corpus(VALUE_LEN);
    let spans = RESOURCES * SCOPES_PER_RESOURCE * SPANS_PER_SCOPE;

    let mut prost_wire = Vec::with_capacity(req.encoded_len());
    req.encode(&mut prost_wire).unwrap();
    let mut tacky_wire = Vec::with_capacity(prost_wire.len() * 2);
    tacky_encode(&mut tacky_wire, &req);

    // Tacky's padded length prefixes rule out a byte compare, so check the
    // stronger thing: prost must decode tacky's output back to the same message.
    assert_eq!(
        pcol::ExportTraceServiceRequest::decode(tacky_wire.as_slice()).unwrap(),
        req,
        "otlp_traces: prost cannot read back what tacky wrote"
    );
    assert_eq!(
        tacky_decode(&prost_wire),
        req,
        "otlp_traces: tacky and prost decode differently"
    );

    println!(
        "otlp_traces: {spans} spans, span names {}..{} B, attribute strings {}..{} B",
        NAME_LEN.0,
        NAME_LEN.0 + NAME_LEN.1,
        VALUE_LEN.0,
        VALUE_LEN.0 + VALUE_LEN.1,
    );

    let cap = tacky_wire.len().max(prost_wire.len());
    let mut group = c.benchmark_group("encode_otlp_traces");
    group.throughput(Throughput::Bytes(prost_wire.len() as u64));
    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            tacky_encode(&mut buf, &req);
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

    // Forward writer into a fixed slice, so `tacky-rev` vs `tacky-slice` isolates the write
    // *direction* from the buffer kind.
    group.bench_function("tacky-slice", |b| {
        let mut backing = vec![0u8; cap + 4096];
        b.iter(|| {
            let mut sb = tacky::SliceBuf::new(&mut backing);
            tacky_encode(&mut sb, &req);
            black_box(sb.written());
        });
    });

    // A downward buffer emits fields in the reverse of the order they are written, which is
    // legal, so this is checked by decoding rather than by comparing bytes.
    let mut rev_backing = vec![0u8; cap + 4096];
    let mut rb = tacky::RevBuf::new(&mut rev_backing);
    tacky_encode(&mut rb, &req);
    assert_eq!(
        pcol::ExportTraceServiceRequest::decode(rb.written()).unwrap(),
        req,
        "reverse writer output does not decode back to the same message"
    );
    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; cap + 4096];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode(&mut rb, &req);
            black_box(rb.written());
        });
    });

    // Handing the result over as an owned, index-0 buffer: the reverse output lives at the
    // tail, so a `Vec<u8>`-shaped sink forces one compaction.
    group.bench_function("tacky-rev-owned", |b| {
        let mut backing = vec![0u8; cap + 4096];
        let mut out = Vec::with_capacity(cap + 4096);
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode(&mut rb, &req);
            out.clear();
            out.extend_from_slice(rb.written());
            black_box(out.as_slice());
        });
    });
    #[cfg(feature = "cpp")]
    for (label, kind) in [
        ("cpp", testing::cpp::OTLP_TRACES),
        ("cpp-noutf8", testing::cpp::OTLP_TRACES_NO_UTF8),
    ] {
        cpp_arms::bench_cpp_arms(&mut group, label, kind, &prost_wire);
    }
    group.finish();

    let mut group = c.benchmark_group("decode_otlp_traces");
    group.throughput(Throughput::Bytes(prost_wire.len() as u64));
    group.bench_function("tacky", |b| {
        b.iter(|| black_box(tacky_decode(black_box(prost_wire.as_slice()))));
    });
    group.bench_function("prost", |b| {
        b.iter(|| {
            black_box(
                pcol::ExportTraceServiceRequest::decode(black_box(prost_wire.as_slice())).unwrap(),
            )
        });
    });
    group.finish();
}

/// How the standing against C++ moves as attribute strings grow, one group per
/// value-length spread (`encode_otlp_vlen_<mean>`, arms named as in the main group).
///
/// This is the axis the corpus is most sensitive to, and not because of the
/// inline-store/`memcpy` crossover: a string's own length prefix is known before it is
/// written, so strings never widen a prefix. What they widen is the *enclosing*
/// messages. At `Tack` width 1 every nested message whose payload reaches 128 B has its
/// one-byte placeholder grown on drop, which memmoves that payload — so as values grow
/// past ~100 B the innermost `KeyValue`/`AnyValue` pair starts overflowing too, and the
/// bytes moved per payload byte goes up by two levels of nesting. C++ pays a sizing
/// pass instead, whose cost is per *field*, not per byte, so it dilutes as strings grow.
fn bench_otlp_value_len(c: &mut Criterion) {
    for spread in [(4, 60), (48, 96), (160, 128)] {
        let req = corpus(spread);
        let mut prost_wire = Vec::with_capacity(req.encoded_len());
        req.encode(&mut prost_wire).unwrap();
        let mut tacky_wire = Vec::with_capacity(prost_wire.len() * 2);
        tacky_encode(&mut tacky_wire, &req);
        assert_eq!(
            pcol::ExportTraceServiceRequest::decode(tacky_wire.as_slice()).unwrap(),
            req,
            "otlp_vlen {spread:?}: prost cannot read back what tacky wrote"
        );

        let name = format!("encode_otlp_vlen_{}", spread.0 + spread.1 / 2);

        let cap = tacky_wire.len().max(prost_wire.len());
        let mut group = c.benchmark_group(&name);
        group.throughput(Throughput::Bytes(prost_wire.len() as u64));
        group.bench_function("tacky", |b| {
            let mut buf = Vec::with_capacity(cap);
            b.iter(|| {
                tacky_encode(&mut buf, &req);
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

        // Forward writer into a fixed slice, so `tacky-rev` vs `tacky-slice` isolates the write
        // *direction* from the buffer kind.
        group.bench_function("tacky-slice", |b| {
            let mut backing = vec![0u8; cap + 4096];
            b.iter(|| {
                let mut sb = tacky::SliceBuf::new(&mut backing);
                tacky_encode(&mut sb, &req);
                black_box(sb.written());
            });
        });

        // A downward buffer emits fields in the reverse of the order they are written, which is
        // legal, so this is checked by decoding rather than by comparing bytes.
        let mut rev_backing = vec![0u8; cap + 4096];
        let mut rb = tacky::RevBuf::new(&mut rev_backing);
        tacky_encode(&mut rb, &req);
        assert_eq!(
            pcol::ExportTraceServiceRequest::decode(rb.written()).unwrap(),
            req,
            "reverse writer output does not decode back to the same message"
        );
        group.bench_function("tacky-rev", |b| {
            let mut backing = vec![0u8; cap + 4096];
            b.iter(|| {
                let mut rb = tacky::RevBuf::new(&mut backing);
                tacky_encode(&mut rb, &req);
                black_box(rb.written());
            });
        });

        // Handing the result over as an owned, index-0 buffer: the reverse output lives at the
        // tail, so a `Vec<u8>`-shaped sink forces one compaction.
        group.bench_function("tacky-rev-owned", |b| {
            let mut backing = vec![0u8; cap + 4096];
            let mut out = Vec::with_capacity(cap + 4096);
            b.iter(|| {
                let mut rb = tacky::RevBuf::new(&mut backing);
                tacky_encode(&mut rb, &req);
                out.clear();
                out.extend_from_slice(rb.written());
                black_box(out.as_slice());
            });
        });
        #[cfg(feature = "cpp")]
        for (label, kind) in [
            ("cpp", testing::cpp::OTLP_TRACES),
            ("cpp-noutf8", testing::cpp::OTLP_TRACES_NO_UTF8),
        ] {
            cpp_arms::bench_cpp_arms(&mut group, label, kind, &prost_wire);
        }
        group.finish();
    }
}

criterion_group!(benches, bench_otlp, bench_otlp_value_len);
criterion_main!(benches);
