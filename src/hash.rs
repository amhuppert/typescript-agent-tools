//! Hashing for internal lookup tables. `FastMap` uses rustc's FxHash, which
//! hashes the compact keys of binding tables several times faster than the
//! standard library's SipHash. SipHash resists keys chosen to collide; the
//! keys here come from the user's own project, and a table that is only
//! looked up, never iterated, behaves the same with either hasher.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A `HashMap` with `FxHasher`, for tables that are only looked up: their
/// iteration order is the hasher's.
pub(crate) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// rustc's FxHash: each word is mixed into the state by a rotate, an xor,
/// and a multiplication.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FxHasher {
    hash: u64,
}

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.add(u64::from_le_bytes(
                chunk.try_into().expect("an 8-byte chunk"),
            ));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    fn write_u16(&mut self, value: u16) {
        self.add(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }

    fn finish(&self) -> u64 {
        self.hash
    }
}
