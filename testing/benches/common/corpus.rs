//! The synthetic-corpus primitives shared by the OTLP benches.
//!
//! One definition, so "the corpora are generated the same way" is a fact rather than a
//! claim: both signals scatter with the same mixer, cut from the same source bytes, and
//! depend on no clock and no RNG.
//!
//! Lives in a subdirectory so cargo does not pick it up as a bench target of its own;
//! each bench pulls it in with `#[path = "common/corpus.rs"] mod corpus;`.

/// splitmix64. Deterministic on purpose — a corpus has to be byte-identical on every
/// machine and every run.
pub fn mix(i: u64) -> u64 {
    let mut x = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// 128 printable ASCII bytes to cut scattered-length strings out of. Slicing at a
/// scattered *offset* as well as a scattered length keeps the contents distinct, so two
/// strings of the same length are still different bytes.
const SRC: &str = "GET/api/v2/orders?status=open&limit=50 POST/api/v2/orders/12345/items \
                   svc-checkout-7f9c4d8b6-x2qlm eu-central-1b node-14 build-2f8a1c";

/// A freshly allocated `String` of scattered length in `[min, min + spread)`.
///
/// Wraps around [`SRC`] rather than slicing it, so a length sweep can ask for lengths
/// past its 128 B. ASCII throughout, so any byte window is valid UTF-8.
///
/// Freshly allocated, never one `&str` reused: reusing one source keeps every copy in a
/// single L1 line and measures the cache-resident best case. Apply this only where real
/// traffic is high-cardinality — see the note on the callers' length constants.
pub fn scattered(i: u64, (min, spread): (usize, usize)) -> String {
    let len = min + mix(i) as usize % spread;
    let off = mix(i ^ 0x5BF0_3635) as usize % SRC.len();
    SRC.bytes()
        .cycle()
        .skip(off)
        .take(len)
        .map(char::from)
        .collect()
}

/// `n` scattered bytes, for trace and span ids.
pub fn bytes(i: u64, n: usize) -> Vec<u8> {
    (0..n as u64).map(|k| mix(i ^ k) as u8).collect()
}
