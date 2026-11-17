//! `java.util.Random`, the generator behind Minecraft's `LegacyRandomSource`.
//!
//! Every player entity owns one (`Entity.random`), the enchanting table menu owns one
//! that it re-seeds from the player's enchantment seed, and both are this 48-bit LCG.

pub const MULTIPLIER: u64 = 0x5_DEEC_E66D;
pub const ADDEND: u64 = 0xB;
pub const MASK: u64 = (1 << 48) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JavaRandom {
    /// The raw 48-bit state (what `LegacyRandomSource.seed` holds).
    pub state: u64,
}

impl JavaRandom {
    /// `new Random(seed)` / `setSeed(seed)`: the seed is scrambled with the multiplier.
    #[inline]
    pub fn new(seed: i64) -> Self {
        Self { state: (seed as u64 ^ MULTIPLIER) & MASK }
    }

    /// A generator positioned at a raw 48-bit state (no scrambling).
    #[inline]
    pub fn from_state(state: u64) -> Self {
        Self { state: state & MASK }
    }

    #[inline]
    pub fn set_seed(&mut self, seed: i64) {
        self.state = (seed as u64 ^ MULTIPLIER) & MASK;
    }

    #[inline]
    pub fn step(state: u64) -> u64 {
        state.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK
    }

    /// `next(bits)`: advance once and return the top `bits` bits (as Java's `(int)` cast).
    #[inline]
    pub fn next(&mut self, bits: u32) -> i32 {
        self.state = Self::step(self.state);
        (self.state >> (48 - bits)) as i32
    }

    #[inline]
    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `nextInt(bound)` exactly as `BitRandomSource.nextInt` (power-of-two fast path, rejection loop).
    #[inline]
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        debug_assert!(bound > 0);
        if bound & (bound - 1) == 0 {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let sample = self.next(31);
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo;
            }
        }
    }

    /// `nextFloat()`: 24 random bits times 2^-24.
    #[inline]
    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    #[inline]
    pub fn next_long(&mut self) -> i64 {
        let hi = self.next(32) as i64;
        let lo = self.next(32) as i64;
        (hi << 32).wrapping_add(lo)
    }

    /// Coefficients `(a, c)` such that `n` steps take a state `x` to `a * x + c (mod 2^48)`.
    pub fn jump(n: u64) -> (u64, u64) {
        let (mut a, mut c) = (1u64, 0u64);
        let (mut ma, mut mc) = (MULTIPLIER, ADDEND);
        let mut k = n;
        while k > 0 {
            if k & 1 == 1 {
                a = a.wrapping_mul(ma) & MASK;
                c = c.wrapping_mul(ma).wrapping_add(mc) & MASK;
            }
            mc = mc.wrapping_mul(ma).wrapping_add(mc) & MASK;
            ma = ma.wrapping_mul(ma) & MASK;
            k >>= 1;
        }
        (a, c)
    }

    /// Advance `n` calls of `next(..)` in O(log n).
    pub fn advance(&mut self, n: u64) {
        let (a, c) = Self::jump(n);
        self.state = self.state.wrapping_mul(a).wrapping_add(c) & MASK;
    }

    /// The value `nextInt()` would return without consuming it.
    #[inline]
    pub fn peek_int(&self) -> i32 {
        (Self::step(self.state) >> 16) as i32
    }
}

/// Java's `Math.round(float)`: floor(x + 1/2), computed exactly.
#[inline]
pub fn java_round(x: f32) -> i32 {
    let v = (x as f64 + 0.5).floor();
    if v >= i32::MAX as f64 {
        i32::MAX
    } else if v <= i32::MIN as f64 {
        i32::MIN
    } else {
        v as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_java_values() {
        // new java.util.Random(42): nextInt() = -1170105035, nextInt(10) = 0 (then 3), nextFloat = 0.7275637
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int(), -1170105035);
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_int_bound(10), 0);
        assert_eq!(r.next_int_bound(10), 3);
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_float(), 0.727_563_7);
        let mut r = JavaRandom::new(42);
        assert_eq!(r.next_long(), -5025562857975149833);
    }

    #[test]
    fn jump_matches_stepping() {
        let mut a = JavaRandom::new(123);
        let mut b = a;
        for _ in 0..1000 {
            a.next(32);
        }
        b.advance(1000);
        assert_eq!(a, b);
    }

    #[test]
    fn rounding() {
        assert_eq!(java_round(0.49999997), 0);
        assert_eq!(java_round(0.5), 1);
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-2.5), -2);
    }
}
