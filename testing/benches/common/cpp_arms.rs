//! The official C++ protobuf runtime's bench arms, shared by every encode bench
//! that has a `feature = "cpp"` counterpart.
//!
//! Lives in a subdirectory so cargo does not pick it up as a bench target of its
//! own; each bench pulls it in with `#[path = "common/cpp_arms.rs"] mod cpp_arms;`.

use criterion::black_box;
use testing::cpp;

/// Adds a `{label}` arm (size pass + write pass) and a `{label}-cached` arm
/// (write pass only) to `group`.
///
/// `prost_wire` must be prost's canonical encoding of the message under test; it
/// seeds the C++ message and doubles as the wire-equality gate.
pub fn bench_cpp_arms(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    label: &str,
    kind: i32,
    prost_wire: &[u8],
) {
    bench_cpp_arms_gated(group, label, kind, prost_wire, |cpp_wire| {
        assert_eq!(
            cpp_wire, prost_wire,
            "{label}: C++ re-serialization differs from prost"
        );
    });
}

/// As [`bench_cpp_arms`], but with the gate supplied by the caller.
///
/// Byte equality is the right gate almost everywhere, and it is the strongest one, but
/// it is not available for a message containing a `map` field: map entries have no
/// canonical wire order, and the C++ runtime emits them in its own hash order while
/// prost emits them in `BTreeMap` order. For those the caller decodes the C++ output and
/// compares messages instead, which is the same standard the `tacky-rev` arms are held
/// to for the same reason.
pub fn bench_cpp_arms_gated(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    label: &str,
    kind: i32,
    prost_wire: &[u8],
    gate: impl Fn(&[u8]),
) {
    let msg = cpp::Msg::parse(kind, prost_wire);

    let mut check = Vec::with_capacity(msg.byte_size() + 64);
    msg.serialize(&mut check);
    gate(&check);

    let cap = check.len();
    group.bench_function(label.to_string(), |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            msg.serialize(&mut buf);
            black_box(buf.as_slice());
            buf.clear();
        });
    });
    group.bench_function(format!("{label}-cached"), |b| {
        let mut buf = Vec::with_capacity(cap);
        b.iter(|| {
            msg.serialize_cached(&mut buf);
            black_box(buf.as_slice());
            buf.clear();
        });
    });
}
