//! Per-write cost of each `WriteBuf`, at the instruction level.
//!
//! `SliceBuf` measured *slower* per write than `Vec`, which is backwards from the intuition
//! that a fixed buffer should be cheaper — it cannot reallocate, so what is there to check?
//! This isolates the answer: three identical writes (a tag byte, a varint, a slice) into
//! each buffer, each in an `#[inline(never)]` function so it gets its own symbol. `Vec::push`
//! pays one compare then stores through a raw pointer; indexing a slice after an assert
//! checks the same bound twice.
//!
//!   cargo build --release -p testing --example buf_asm
//!   scripts/fn_asm.sh -v target/release/examples/buf_asm writes_vec writes_slice
//!
//! Read the *count of compares and panic edges*, not the total: the question is how many
//! bounds checks each write pays.

use std::hint::black_box;
use tacky::{SliceBuf, WriteBuf};

#[inline(never)]
pub fn writes_vec(buf: &mut Vec<u8>, n: u64, s: &[u8]) {
    buf.put_u8(0x0a);
    buf.put_varint(n);
    buf.put_slice(s);
}

#[inline(never)]
pub fn writes_slice(buf: &mut SliceBuf<'_>, n: u64, s: &[u8]) {
    buf.put_u8(0x0a);
    buf.put_varint(n);
    buf.put_slice(s);
}

fn main() {
    let payload = black_box(b"hello world".as_slice());
    let n = black_box(300u64);

    let mut v = Vec::with_capacity(64);
    writes_vec(&mut v, n, payload);

    let mut backing = [0u8; 64];
    let mut sb = SliceBuf::new(&mut backing);
    writes_slice(&mut sb, n, payload);

    println!("vec {} slice {}", v.len(), sb.written().len());
}
