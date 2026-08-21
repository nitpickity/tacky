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
//! **There is no official OTLP payload dataset** — unlike pprof, where this suite
//! benches a real checked-in profile — so this batch is synthesised. That makes the
//! generator part of the result, not scaffolding, so it is spelled out below: every
//! count is a named constant, every string length is scattered by a fixed integer
//! mixer, and nothing depends on a clock or an RNG. Three traps this repo has already
//! fallen into are avoided deliberately:
//!
//! - Attribute string *values* are freshly allocated `String`s cut from a scattered
//!   offset, not one `&str` reused. Reusing one source keeps every copy in a single L1
//!   line and measures the cache-resident best case.
//! - Value lengths are scattered, not constant. A constant length makes any
//!   size-dispatch branch predict perfectly; last time that turned a real 2.1x into an
//!   apparent 4.0x. The spread is reported with the numbers.
//! - Scattering is applied only where real traffic is high-cardinality. Span names,
//!   attribute keys and event names are route templates and semconv constants, reused
//!   across a whole batch; generating a unique random string for each one invented a
//!   cache miss no exporter takes. Values (`url.full`, `db.statement`) are where the
//!   cardinality really is.
//!
//! Occurrence rates matter as much as counts. Events and links are *occasional*, because
//! real spans mostly carry neither; emitting one of each per span would inflate nested
//! messages and their length prefixes, which is the work tacky exists to make cheap.
//!
//! Two batch sizes are benched, `encode_otlp_traces` (200 spans) and
//! `encode_otlp_traces_512` (512, the SDK exporter default), because no single batch
//! size stands in for the range between an SDK and a collector.
//!
//! Arms are the same as the other targets:
//!
//! - `tacky` — one pass, no sizing.
//! - `prost` — `Message::encode`, which sizes internally. prost's published shape.
//! - `cpp` / `cpp-cached` / `cpp-noutf8…` under `--features cpp`.
//!
//! Wire output is checked by decoding tacky's bytes with prost and comparing messages
//! rather than by comparing byte strings, because a reverse writer emits fields in the
//! opposite order — legal, but not byte-comparable.
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

/// 4 resources × 2 scopes × 25 spans = 200 spans. Sized to sit well outside L1 so the
/// encode loop is not measured against a hot buffer, and to stay small enough that the
/// criterion sample count is still meaningful.
const RESOURCES: usize = 4;
const SCOPES_PER_RESOURCE: usize = 2;
const SPANS_PER_SCOPE: usize = 25;

/// 4 × 2 × 64 = 512 spans, which is the SDK `BatchSpanProcessor`'s default
/// `max_export_batch_size` and so the batch size a real exporter emits most often. The
/// collector's `batchprocessor` triggers at 8192 items instead; that is another 16×
/// beyond this and is left to the size trend rather than benched directly.
const SPANS_PER_SCOPE_BATCH: usize = 64;

/// Attribute counts, matched to what an instrumented HTTP/DB service actually emits.
///
/// `RESOURCE_ATTRS` is 18 because a real resource is the union of what the SDK, the
/// host, the process, the k8s and the cloud detectors each contribute — see
/// [`RESOURCE_KEYS`]. `SPAN_ATTRS` is 11 because that is roughly the semconv HTTP
/// server set (see [`SPAN_KEYS`]).
const RESOURCE_ATTRS: usize = 18;
const SCOPE_ATTRS: usize = 2;
const SPAN_ATTRS: usize = 11;

/// Events and links are *occasional*, not per-span. Events are overwhelmingly `exception`
/// records, so only spans that failed carry any; links only show up on messaging consumers
/// and batch fan-in.
const EVENTS_EVERY: u64 = 8;
const EVENTS_PER_SPAN: usize = 2;
const EVENT_ATTRS: usize = 3;
const LINKS_EVERY: u64 = 16;
const LINKS_PER_SPAN: usize = 1;
const LINK_ATTRS: usize = 1;

/// String-length spread for attribute *values*, reported with the results: `[4, 64)`
/// bytes, scattered by [`mix`]. Below the ~128 B point where an inline-store copy stops
/// beating `memcpy`, which is the regime this corpus is meant to probe.
///
/// Only values are scattered. Span names, attribute keys and event names are all
/// low-cardinality in real traffic — route templates and semconv constants, reused
/// across every span in a batch — and pretending otherwise measures a cache miss that
/// a real exporter does not take. High cardinality is real for values (`url.full`,
/// `db.statement`), and that is where it is kept.
const VALUE_LEN: (usize, usize) = (4, 60);

/// Length spread for the one genuinely per-span free-text field, an error
/// `Status.message`.
const NAME_LEN: (usize, usize) = (8, 40);

/// Every 10th span carries an error `Status`; the rest leave it unset, as
/// instrumentation does.
const ERROR_EVERY: u64 = 10;

#[path = "common/corpus.rs"]
mod corpus_gen;
use corpus_gen::{bytes, mix, scattered};

/// Semconv span attribute keys, roughly the HTTP-server set plus the DB and RPC keys a
/// service that talks downstream also emits. Reused by name across spans — that part
/// *is* how OTLP looks, and both encoders pay the same copy for it.
///
/// Keys are handed out by ordinal, not picked by [`mix`]. Picking randomly let one span
/// carry the same key twice, which is not a payload any SDK produces: attributes are a
/// map.
const SPAN_KEYS: [&str; SPAN_ATTRS] = [
    "http.request.method",
    "http.route",
    "http.response.status_code",
    "url.full",
    "url.scheme",
    "server.address",
    "server.port",
    "network.protocol.version",
    "user_agent.original",
    "client.address",
    "db.statement",
];

/// What the SDK, host, process, k8s and cloud resource detectors put on a real
/// `Resource` between them.
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

/// Events are `exception` records, and these are the three attributes that carries.
const EVENT_KEYS: [&str; EVENT_ATTRS] = [
    "exception.type",
    "exception.message",
    "exception.stacktrace",
];

/// Low-cardinality span names: route templates, DB operations and RPC methods, exactly
/// as instrumentation names spans. A batch reuses the same handful over and over.
const SPAN_NAMES: [&str; 12] = [
    "GET /api/v2/orders",
    "POST /api/v2/orders",
    "GET /api/v2/orders/{id}",
    "GET /api/v2/orders/{id}/items",
    "POST /api/v2/checkout",
    "DELETE /api/v2/carts/{id}",
    "GET /health",
    "SELECT orders",
    "INSERT order_items",
    "orders.created publish",
    "orders.created process",
    "checkout.v2.Checkout/PlaceOrder",
];

/// One attribute under `key`. Values are 75% strings, which is the real mix; the remaining
/// quarter covers int, bool and double so no `AnyValue` variant goes untested. `value_len` is
/// a parameter rather than [`VALUE_LEN`] so [`bench_otlp_value_len`] can sweep it.
fn attr(key: &str, i: u64, value_len: (usize, usize)) -> pcommon::KeyValue {
    use pcommon::any_value::Value;
    let value = match mix(i ^ 1) % 16 {
        0..=11 => Value::StringValue(scattered(i, value_len)),
        12 | 13 => Value::IntValue((mix(i ^ 2) % 100_000) as i64),
        14 => Value::BoolValue(mix(i ^ 3) % 2 == 0),
        _ => Value::DoubleValue(mix(i ^ 4) as f64 / 1e15),
    };
    pcommon::KeyValue {
        key: key.to_string(),
        value: Some(pcommon::AnyValue { value: Some(value) }),
    }
}

/// Builds the whole batch. `ArrayValue` and `KeyValueList` each appear exactly
/// once per resource and per scope respectively — rare in real traffic, but the
/// nested-oneof path is worth exercising at all rather than not at all.
///
/// `spans_per_scope` is a parameter so the 200-span and 512-span batches come out of the
/// same generator; see [`SPANS_PER_SCOPE_BATCH`].
fn corpus(value_len: (usize, usize), spans_per_scope: usize) -> pcol::ExportTraceServiceRequest {
    use pcommon::any_value::Value;

    let mut resource_spans = Vec::with_capacity(RESOURCES);
    for r in 0..RESOURCES as u64 {
        let mut attributes: Vec<pcommon::KeyValue> = RESOURCE_KEYS
            .iter()
            .enumerate()
            .map(|(a, k)| attr(k, r * 1_000 + a as u64, value_len))
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
            const SCOPE_KEYS: [&str; SCOPE_ATTRS] = ["otel.scope.build_id", "otel.library.name"];
            let mut scope_attrs: Vec<pcommon::KeyValue> = SCOPE_KEYS
                .iter()
                .enumerate()
                .map(|(a, k)| attr(k, seed + a as u64, value_len))
                .collect();
            scope_attrs.push(pcommon::KeyValue {
                key: "otel.scope.config".to_string(),
                value: Some(pcommon::AnyValue {
                    value: Some(Value::KvlistValue(pcommon::KeyValueList {
                        values: (0..2)
                            .map(|k| attr(SPAN_KEYS[k as usize], seed + 500 + k, value_len))
                            .collect(),
                    })),
                }),
            });

            let spans = (0..spans_per_scope as u64)
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
                        name: SPAN_NAMES[mix(seed) as usize % SPAN_NAMES.len()].to_string(),
                        kind: (1 + mix(seed ^ 7) % 5) as i32,
                        start_time_unix_nano: start,
                        end_time_unix_nano: start + mix(seed ^ 8) % 50_000_000,
                        attributes: SPAN_KEYS
                            .iter()
                            .enumerate()
                            .map(|(a, k)| attr(k, seed + a as u64, value_len))
                            .collect(),
                        dropped_attributes_count: 0,
                        // Only failed spans carry events, and what they carry is an
                        // exception record.
                        events: if i % EVENTS_EVERY == 0 {
                            (0..EVENTS_PER_SPAN as u64)
                                .map(|e| ptrace::span::Event {
                                    time_unix_nano: start + e * 1_000_000,
                                    name: "exception".to_string(),
                                    attributes: EVENT_KEYS
                                        .iter()
                                        .enumerate()
                                        .map(|(a, k)| {
                                            attr(k, seed + 50 + e * 10 + a as u64, value_len)
                                        })
                                        .collect(),
                                    dropped_attributes_count: 0,
                                })
                                .collect()
                        } else {
                            Vec::new()
                        },
                        dropped_events_count: 0,
                        links: if i % LINKS_EVERY == 0 {
                            (0..LINKS_PER_SPAN as u64)
                                .map(|l| ptrace::span::Link {
                                    trace_id: bytes(seed ^ 0x11 ^ l, 16),
                                    span_id: bytes(seed ^ 0x22 ^ l, 8),
                                    trace_state: String::new(),
                                    attributes: (0..LINK_ATTRS as u64)
                                        .map(|a| {
                                            attr(
                                                "messaging.batch.message_id",
                                                seed + 70 + l * 10 + a,
                                                value_len,
                                            )
                                        })
                                        .collect(),
                                    dropped_attributes_count: 0,
                                    flags: 1,
                                })
                                .collect()
                        } else {
                            Vec::new()
                        },
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
// Fields go out in ascending tag order, which is the order prost emits.

fn tacky_encode<B: tacky::WriteBuf>(
    buf: &mut tacky::AnyDir<B>,
    req: &pcol::ExportTraceServiceRequest,
) {
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

fn write_span<B: tacky::WriteBuf>(buf: &mut tacky::AnyDir<B>, span: &ptrace::Span) {
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

/// Parse-only counterpart to `tacky_decode`: visits every field, folds each value into
/// an accumulator, allocates nothing. The `tacky` arm measures parse *plus* building
/// prost's owned structs, and that allocation dominates; this isolates the iterator and
/// its tag dispatch. Borrowed strings and byte slices are measured, never copied.
///
/// No prost counterpart exists by construction. Correctness is gated by `tacky_decode`.
fn tacky_walk(wire: &[u8]) -> u64 {
    let mut acc = 0u64;
    for field in t::ExportTraceServiceRequest::decode(wire) {
        match field.unwrap() {
            t::ExportTraceServiceRequestField::ResourceSpans(fields) => {
                acc = acc.wrapping_add(walk_resource_spans(fields))
            }
        }
    }
    acc
}

fn walk_resource_spans(fields: t::ResourceSpansFields<'_>) -> u64 {
    use t::ResourceSpansField as F;

    let mut acc = 0u64;
    for field in fields {
        match field.unwrap() {
            F::Resource(res) => {
                use t::ResourceField as R;
                for f in res {
                    match f.unwrap() {
                        R::Attributes(kv) => acc = acc.wrapping_add(walk_kv(kv)),
                        R::DroppedAttributesCount(v) => acc = acc.wrapping_add(v as u64),
                    }
                }
            }
            F::ScopeSpans(ss) => acc = acc.wrapping_add(walk_scope_spans(ss)),
            F::SchemaUrl(v) => acc = acc.wrapping_add(v.len() as u64),
        }
    }
    acc
}

fn walk_scope_spans(fields: t::ScopeSpansFields<'_>) -> u64 {
    use t::ScopeSpansField as F;

    let mut acc = 0u64;
    for field in fields {
        match field.unwrap() {
            F::Scope(scope) => {
                use t::InstrumentationScopeField as S;
                for f in scope {
                    match f.unwrap() {
                        S::Name(v) => acc = acc.wrapping_add(v.len() as u64),
                        S::Version(v) => acc = acc.wrapping_add(v.len() as u64),
                        S::Attributes(kv) => acc = acc.wrapping_add(walk_kv(kv)),
                        S::DroppedAttributesCount(v) => acc = acc.wrapping_add(v as u64),
                    }
                }
            }
            F::Spans(span) => acc = acc.wrapping_add(walk_span(span)),
            F::SchemaUrl(v) => acc = acc.wrapping_add(v.len() as u64),
        }
    }
    acc
}

fn walk_span(fields: t::SpanFields<'_>) -> u64 {
    use t::SpanField as F;

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }
    for field in fields {
        match field.unwrap() {
            F::TraceId(v) => add!(v.len()),
            F::SpanId(v) => add!(v.len()),
            F::TraceState(v) => add!(v.len()),
            F::ParentSpanId(v) => add!(v.len()),
            F::Flags(v) => add!(v),
            F::Name(v) => add!(v.len()),
            F::Kind(v) => add!(i32::from(v)),
            F::StartTimeUnixNano(v) => add!(v),
            F::EndTimeUnixNano(v) => add!(v),
            F::Attributes(kv) => add!(walk_kv(kv)),
            F::DroppedAttributesCount(v) => add!(v),
            F::Events(ev) => {
                use t::SpanEventField as E;
                for f in ev {
                    match f.unwrap() {
                        E::TimeUnixNano(v) => add!(v),
                        E::Name(v) => add!(v.len()),
                        E::Attributes(kv) => add!(walk_kv(kv)),
                        E::DroppedAttributesCount(v) => add!(v),
                    }
                }
            }
            F::DroppedEventsCount(v) => add!(v),
            F::Links(ln) => {
                use t::SpanLinkField as L;
                for f in ln {
                    match f.unwrap() {
                        L::TraceId(v) => add!(v.len()),
                        L::SpanId(v) => add!(v.len()),
                        L::TraceState(v) => add!(v.len()),
                        L::Attributes(kv) => add!(walk_kv(kv)),
                        L::DroppedAttributesCount(v) => add!(v),
                        L::Flags(v) => add!(v),
                    }
                }
            }
            F::DroppedLinksCount(v) => add!(v),
            F::Status(st) => {
                use t::StatusField as S;
                for f in st {
                    match f.unwrap() {
                        S::Message(v) => add!(v.len()),
                        S::Code(v) => add!(i32::from(v)),
                    }
                }
            }
        }
    }
    acc
}

fn walk_kv(fields: t::KeyValueFields<'_>) -> u64 {
    use t::KeyValueField as F;

    let mut acc = 0u64;
    for field in fields {
        match field.unwrap() {
            F::Key(v) => acc = acc.wrapping_add(v.len() as u64),
            F::Value(v) => acc = acc.wrapping_add(walk_any(v)),
        }
    }
    acc
}

fn walk_any(fields: t::AnyValueFields<'_>) -> u64 {
    use t::AnyValueField as F;

    let mut acc = 0u64;
    macro_rules! add {
        ($v:expr) => {
            acc = acc.wrapping_add($v as u64)
        };
    }
    for field in fields {
        match field.unwrap() {
            F::StringValue(v) => add!(v.len()),
            F::BoolValue(v) => add!(v),
            F::IntValue(v) => add!(v),
            F::DoubleValue(v) => add!(v.to_bits()),
            F::BytesValue(v) => add!(v.len()),
            F::ArrayValue(inner) => {
                use t::ArrayValueField as A;
                for f in inner {
                    match f.unwrap() {
                        A::Values(v) => add!(walk_any(v)),
                    }
                }
            }
            F::KvlistValue(inner) => {
                use t::KeyValueListField as K;
                for f in inner {
                    match f.unwrap() {
                        K::Values(v) => add!(walk_kv(v)),
                    }
                }
            }
        }
    }
    acc
}

// ---------------------------------------------------------------------------
// Benches
// ---------------------------------------------------------------------------

/// The encode arms, shared by the two batch sizes and by the value-length sweep.
///
/// Four arms, matching what the README publishes: `tacky` (forward, into a `Vec`),
/// `tacky-rev` (backwards into a caller-sized slice), `prost`, and the fair C++ arm.
/// `cpp-noutf8` is that arm for proto3 — the plain `cpp` arm also validates UTF-8, which
/// Rust gets free from `&str` — and `bench_cpp_arms` adds its `-cached` floor alongside.
/// The buffer-kind and hand-off diagnostics (`tacky-slice`, `tacky-rev-owned`) live on
/// `encode_pprof` in `benches/comparison.rs`; they report the same thing on every corpus,
/// so one home is enough.
///
/// The `tacky-rev` round-trip is asserted here rather than at each call site: a downward
/// buffer emits fields in the reverse of the order they are written, which is legal, so
/// it is checked by decoding rather than by comparing bytes.
fn encode_arms(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    req: &pcol::ExportTraceServiceRequest,
    prost_wire: &[u8],
    cap: usize,
) {
    group.throughput(Throughput::Bytes(prost_wire.len() as u64));
    group.bench_function("tacky", |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            tacky_encode(tacky::AnyDir::from_mut(&mut buf), req);
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

    let mut rev_backing = vec![0u8; cap + 4096];
    let mut rb = tacky::RevBuf::new(&mut rev_backing);
    tacky_encode(tacky::AnyDir::from_mut(&mut rb), req);
    assert_eq!(
        &pcol::ExportTraceServiceRequest::decode(rb.written()).unwrap(),
        req,
        "reverse writer output does not decode back to the same message"
    );
    group.bench_function("tacky-rev", |b| {
        let mut backing = vec![0u8; cap + 4096];
        b.iter(|| {
            let mut rb = tacky::RevBuf::new(&mut backing);
            tacky_encode(tacky::AnyDir::from_mut(&mut rb), req);
            black_box(rb.written());
        });
    });
    #[cfg(feature = "cpp")]
    cpp_arms::bench_cpp_arms(
        group,
        "cpp-noutf8",
        testing::cpp::OTLP_TRACES_NO_UTF8,
        prost_wire,
    );
}

/// Encodes `req` with both writers and checks they agree, returning prost's wire bytes
/// and a capacity that fits either output. Tacky's padded length prefixes rule out a
/// byte compare, so this checks the stronger thing: prost must decode tacky's output
/// back to the same message.
fn wire_and_cap(req: &pcol::ExportTraceServiceRequest, what: &str) -> (Vec<u8>, usize) {
    let mut prost_wire = Vec::with_capacity(req.encoded_len());
    req.encode(&mut prost_wire).unwrap();
    let mut tacky_wire = Vec::with_capacity(prost_wire.len() * 2);
    tacky_encode(tacky::AnyDir::from_mut(&mut tacky_wire), req);
    assert_eq!(
        &pcol::ExportTraceServiceRequest::decode(tacky_wire.as_slice()).unwrap(),
        req,
        "{what}: prost cannot read back what tacky wrote"
    );
    println!(
        "{what}: prost {} B, tacky {} B (+{:.2}%)",
        prost_wire.len(),
        tacky_wire.len(),
        (tacky_wire.len() as f64 / prost_wire.len() as f64 - 1.0) * 100.0,
    );
    let cap = tacky_wire.len().max(prost_wire.len());
    (prost_wire, cap)
}

fn bench_otlp(c: &mut Criterion) {
    let req = corpus(VALUE_LEN, SPANS_PER_SCOPE);
    let spans = RESOURCES * SCOPES_PER_RESOURCE * SPANS_PER_SCOPE;
    let (prost_wire, cap) = wire_and_cap(&req, "otlp_traces");

    assert_eq!(
        tacky_decode(&prost_wire),
        req,
        "otlp_traces: tacky and prost decode differently"
    );

    println!(
        "otlp_traces: {spans} spans, {} B on the wire, attribute strings {}..{} B, \
         {SPAN_ATTRS} span attrs / {RESOURCE_ATTRS} resource attrs, events on 1 span in \
         {EVENTS_EVERY}, links on 1 in {LINKS_EVERY}",
        prost_wire.len(),
        VALUE_LEN.0,
        VALUE_LEN.0 + VALUE_LEN.1,
    );

    let mut group = c.benchmark_group("encode_otlp_traces");
    encode_arms(&mut group, &req, &prost_wire, cap);
    group.finish();

    // The SDK's default export batch, 512 spans. Same corpus, more of it: the point is
    // the size trend, since one batch size cannot stand in for the whole range between
    // an SDK exporter and a collector's 8192-item trigger.
    let big = corpus(VALUE_LEN, SPANS_PER_SCOPE_BATCH);
    let (big_wire, big_cap) = wire_and_cap(&big, "otlp_traces_512");
    println!(
        "otlp_traces_512: {} spans, {} B on the wire",
        RESOURCES * SCOPES_PER_RESOURCE * SPANS_PER_SCOPE_BATCH,
        big_wire.len(),
    );
    let mut group = c.benchmark_group("encode_otlp_traces_512");
    encode_arms(&mut group, &big, &big_wire, big_cap);
    group.finish();

    let mut group = c.benchmark_group("decode_otlp_traces");
    group.throughput(Throughput::Bytes(prost_wire.len() as u64));
    group.bench_function("tacky", |b| {
        b.iter(|| black_box(tacky_decode(black_box(prost_wire.as_slice()))));
    });
    assert!(tacky_walk(&prost_wire) != 0, "walker folded nothing");
    group.bench_function("tacky-walk", |b| {
        b.iter(|| black_box(tacky_walk(black_box(prost_wire.as_slice()))));
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
        let req = corpus(spread, SPANS_PER_SCOPE);
        let (prost_wire, cap) = wire_and_cap(&req, &format!("otlp_vlen {spread:?}"));
        let name = format!("encode_otlp_vlen_{}", spread.0 + spread.1 / 2);
        let mut group = c.benchmark_group(&name);
        encode_arms(&mut group, &req, &prost_wire, cap);
        group.finish();
    }
}

criterion_group!(benches, bench_otlp, bench_otlp_value_len);
criterion_main!(benches);
