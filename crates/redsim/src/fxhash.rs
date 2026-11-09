//! A fast, deterministic hasher for redsim's hot sets and maps of small keys (block positions,
//! chunk coordinates) (tickexact 2026-11-08, the optimizer's evaluation: SipHash was a tenth of a
//! terrain run). Only for collections whose iteration order changes no result: with the standard
//! library's random hasher such an order would already differ from run to run.

use std::hash::{BuildHasher, Hasher};

/// The rustc / Firefox "Fx" hash: rotate, xor, multiply per word.
#[derive(Clone, Copy, Default)]
pub struct FxHasher {
    hash: u64,
}

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    #[inline]
    fn add(&mut self, w: u64) {
        self.hash = (self.hash.rotate_left(5) ^ w).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut b = [0u8; 8];
            b[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(b));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }
    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.add(i as u32 as u64);
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// Builds [`FxHasher`]s.
#[derive(Clone, Copy, Default)]
pub struct FxBuild;

impl BuildHasher for FxBuild {
    type Hasher = FxHasher;
    #[inline]
    fn build_hasher(&self) -> FxHasher {
        FxHasher::default()
    }
}

pub type FxHashSet<T> = std::collections::HashSet<T, FxBuild>;
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuild>;
