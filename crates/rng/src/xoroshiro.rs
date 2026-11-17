//! Xoroshiro128++ as `XoroshiroRandomSource`, plus the world random sequences seeded from it.
//!
//! In 26.3 every villager trade set names a `random_sequence` (for example
//! `minecraft:trade_set/librarian/level_1`). `LootContext.Builder.create` then takes its randomness
//! from `MinecraftServer.getRandomSequence(key)`: one world-global stream per key, seeded from the
//! world seed and the key, saved in `data/random_sequences.dat`.

use md5::{Digest, Md5};

pub const GOLDEN_RATIO_64: u64 = 0x9E37_79B9_7F4A_7C15; // -7046029254386353131
pub const SILVER_RATIO_64: u64 = 0x6A09_E667_F3BC_C909; // 7640891576956012809

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Xoroshiro {
    pub lo: u64,
    pub hi: u64,
}

impl Xoroshiro {
    pub fn new(lo: u64, hi: u64) -> Self {
        if lo | hi == 0 {
            Self { lo: GOLDEN_RATIO_64, hi: SILVER_RATIO_64 }
        } else {
            Self { lo, hi }
        }
    }

    #[inline]
    pub fn next_long(&mut self) -> u64 {
        let s0 = self.lo;
        let mut s1 = self.hi;
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.lo = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.hi = s1.rotate_left(28);
        result
    }

    /// `nextInt()`: the low 32 bits of one output.
    #[inline]
    pub fn next_int(&mut self) -> i32 {
        self.next_long() as i32
    }

    /// `XoroshiroRandomSource.nextInt(bound)` (Lemire's method, as decompiled from 26.3).
    #[inline]
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        debug_assert!(bound > 0);
        let b = bound as u64;
        let mut m = (self.next_int() as u32 as u64) * b;
        let mut frac = m & 0xFFFF_FFFF;
        if frac < b {
            let threshold = (bound.wrapping_neg() as u32 % bound as u32) as u64;
            while frac < threshold {
                m = (self.next_int() as u32 as u64) * b;
                frac = m & 0xFFFF_FFFF;
            }
        }
        (m >> 32) as i32
    }

    #[inline]
    pub fn next_float(&mut self) -> f32 {
        (self.next_long() >> 40) as f32 * 5.960_464_5e-8_f32
    }

    /// `Mth.nextInt(random, min, max)`.
    #[inline]
    pub fn next_int_between(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            min
        } else {
            self.next_int_bound(max - min + 1) + min
        }
    }
}

pub fn mix_stafford13(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `RandomSupport.seedFromHashOf`: MD5 of the string, as two big-endian longs.
pub fn seed_from_hash_of(s: &str) -> (u64, u64) {
    let digest = Md5::digest(s.as_bytes());
    let lo = u64::from_be_bytes(digest[0..8].try_into().unwrap());
    let hi = u64::from_be_bytes(digest[8..16].try_into().unwrap());
    (lo, hi)
}

/// The sequence settings stored in `random_sequences.dat` (`/random reset` can change them).
#[derive(Clone, Copy, Debug)]
pub struct SequenceSettings {
    pub salt: i32,
    pub include_world_seed: bool,
    pub include_sequence_id: bool,
}

impl Default for SequenceSettings {
    fn default() -> Self {
        Self { salt: 0, include_world_seed: true, include_sequence_id: true }
    }
}

/// A freshly created world random sequence (`RandomSequences.createSequence`).
pub fn random_sequence(world_seed: i64, key: &str, settings: SequenceSettings) -> Xoroshiro {
    let seed = (if settings.include_world_seed { world_seed } else { 0 }) ^ settings.salt as i64;
    let mut lo = seed as u64 ^ SILVER_RATIO_64;
    let mut hi = lo.wrapping_add(GOLDEN_RATIO_64);
    if settings.include_sequence_id {
        let (klo, khi) = seed_from_hash_of(key);
        lo ^= klo;
        hi ^= khi;
    }
    Xoroshiro::new(mix_stafford13(lo), mix_stafford13(hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_known() {
        // md5("") = d41d8cd98f00b204e9800998ecf8427e
        let (lo, hi) = seed_from_hash_of("");
        assert_eq!(lo, 0xd41d8cd98f00b204);
        assert_eq!(hi, 0xe9800998ecf8427e);
    }

    #[test]
    fn bounded_ints_in_range() {
        let mut x = random_sequence(1, "minecraft:test", SequenceSettings::default());
        for bound in 1..200 {
            let v = x.next_int_bound(bound);
            assert!(v >= 0 && v < bound);
        }
    }
}
