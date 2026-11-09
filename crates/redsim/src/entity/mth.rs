//! `net.minecraft.util.Mth`: the game's float sine table and helpers, bit for bit (Java casts:
//! `(int)`/`(long)` of a double truncate toward zero and saturate, NaN -> 0, as Rust's `as`).
//!
//! The SIN table is `(float)Math.sin(i / 10430.378350470453)` with the JVM's own `Math.sin`
//! ([`crate::entity::jmath`], checked against the game's JVM on every one of the 65536 arguments).

use std::sync::OnceLock;

fn sin_table() -> &'static [f32; 65536] {
    static T: OnceLock<Box<[f32; 65536]>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([0f32; 65536]);
        for (i, v) in t.iter_mut().enumerate() {
            *v = crate::entity::jmath::sin(i as f64 / 10430.378350470453) as f32;
        }
        t
    })
}

/// `Mth.sin(double)`.
pub fn sin(i: f64) -> f32 {
    sin_table()[((i * 10430.378350470453) as i64 & 65535) as usize]
}

/// `Mth.cos(double)`.
pub fn cos(i: f64) -> f32 {
    sin_table()[((i * 10430.378350470453 + 16384.0) as i64 & 65535) as usize]
}

/// `Mth.sqrt(float)`.
pub fn sqrt(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

/// `Mth.floor(double)` (as `v.floor() as i32` for every input, NaN and the infinities included,
/// without the libm call: tickexact 2026-11-08, it was a hundredth of a run).
#[inline]
pub fn floor(v: f64) -> i32 {
    let i = v as i32;
    if (i as f64) > v { i.saturating_sub(1) } else { i }
}

/// `Mth.lfloor(double)` (as `v.floor() as i64`, without the libm call).
#[inline]
pub fn lfloor(v: f64) -> i64 {
    let i = v as i64;
    if (i as f64) > v { i.saturating_sub(1) } else { i }
}

/// `Mth.frac(double)`.
pub fn frac(v: f64) -> f64 {
    v - lfloor(v) as f64
}

/// `Mth.clamp(double, double, double)`.
pub fn clamp(v: f64, min: f64, max: f64) -> f64 {
    if v < min { min } else { crate::entity::vec3::jmin(v, max) }
}

/// Java's `Math.min(float, float)`.
pub fn jminf(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && (a.is_sign_negative() || b.is_sign_negative()) {
        return -0.0;
    }
    if a <= b { a } else { b }
}

/// Java's `Math.max(float, float)`.
pub fn jmaxf(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && (a.is_sign_positive() || b.is_sign_positive()) {
        return 0.0;
    }
    if a >= b { a } else { b }
}

/// `Mth.clamp(float, float, float)`.
pub fn clampf(v: f32, min: f32, max: f32) -> f32 {
    if v < min { min } else { jminf(v, max) }
}

/// `Mth.lerp(double, double, double)`.
pub fn lerp(a: f64, p0: f64, p1: f64) -> f64 {
    p0 + a * (p1 - p0)
}

/// `Mth.lerp(float, float, float)`.
pub fn lerpf(a: f32, p0: f32, p1: f32) -> f32 {
    p0 + a * (p1 - p0)
}

/// `Mth.wrapDegrees(float)`.
pub fn wrap_degrees(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `Mth.wrapDegrees(double)`.
pub fn wrap_degrees_d(angle: f64) -> f64 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `Mth.rotLerp(float, float, float)`.
pub fn rot_lerp(a: f32, from: f32, to: f32) -> f32 {
    from + a * wrap_degrees(to - from)
}

/// `Mth.equal(double, double)`.
pub fn equal(a: f64, b: f64) -> bool {
    (b - a).abs() < 1.0e-5f32 as f64
}

/// `Mth.sign(double)`.
pub fn sign(v: f64) -> i32 {
    if v == 0.0 {
        0
    } else if v > 0.0 {
        1
    } else {
        -1
    }
}

/// `Mth.fastInvSqrt(double)`.
pub fn fast_inv_sqrt(x: f64) -> f64 {
    let xhalf = 0.5 * x;
    let i = 6910469410427058090i64.wrapping_sub((x.to_bits() as i64) >> 1);
    let x = f64::from_bits(i as u64);
    x * (1.5 - xhalf * x * x)
}

fn asin_tabs() -> &'static ([f64; 257], [f64; 257]) {
    static T: OnceLock<([f64; 257], [f64; 257])> = OnceLock::new();
    T.get_or_init(|| {
        // `Math.asin` (`StrictMath.asin`: fdlibm) and `Math.cos` (HotSpot's), the same on every
        // platform (the wasm viewer's own `f64::asin` and `f64::cos` differ in the last bit)
        let mut asin = [0f64; 257];
        let mut cos = [0f64; 257];
        for i in 0..257 {
            let v = i as f64 / 256.0;
            let a = super::fdlibm::asin(v);
            cos[i] = super::jmath::cos(a);
            asin[i] = a;
        }
        (asin, cos)
    })
}

/// `Mth.atan2(double, double)`: the game's table approximation, not `Math.atan2`.
pub fn atan2(y: f64, x: f64) -> f64 {
    let d2 = x * x + y * y;
    if d2.is_nan() {
        return f64::NAN;
    }
    let (mut y, mut x) = (y, x);
    let neg_y = y < 0.0;
    if neg_y {
        y = -y;
    }
    let neg_x = x < 0.0;
    if neg_x {
        x = -x;
    }
    let steep = y > x;
    if steep {
        std::mem::swap(&mut x, &mut y);
    }
    let rinv = fast_inv_sqrt(d2);
    x *= rinv;
    y *= rinv;
    let frac_bias = f64::from_bits(4805340802404319232u64);
    let yp = frac_bias + y;
    let index = yp.to_bits() as i32 as usize;
    let (asin, cos) = asin_tabs();
    let phi = asin[index];
    let c_phi = cos[index];
    let s_phi = yp - frac_bias;
    let sd = y * c_phi - x * s_phi;
    let d = (6.0 + sd * sd) * sd * 0.16666666666666666;
    let mut theta = phi + d;
    if steep {
        theta = std::f64::consts::FRAC_PI_2 - theta;
    }
    if neg_x {
        theta = std::f64::consts::PI - theta;
    }
    if neg_y {
        theta = -theta;
    }
    theta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-190.0), 170.0);
        assert_eq!(floor(-0.5), -1);
        // the game's atan2 is an approximation (fastInvSqrt: one Newton step)
        assert!((atan2(1.0, 1.0) - std::f64::consts::FRAC_PI_4).abs() < 1e-2);
        assert!((atan2(-1.0, -2.0) - (-1.0f64).atan2(-2.0)).abs() < 1e-2);
    }

    /// `floor` and `lfloor` answer as `v.floor() as i32` / `as i64` everywhere: integers, halves,
    /// signed zeros, NaN, the infinities, the edges of the integer ranges, tiny offsets.
    #[test]
    fn floor_is_the_libm_floor() {
        let mut vs = vec![0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 1.0e-7, -1.0e-7, 2.999999, -2.999999, 1.0e300, -1.0e300, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MIN_POSITIVE, -f64::MIN_POSITIVE];
        for b in [i32::MAX as f64, i32::MIN as f64, i64::MAX as f64, i64::MIN as f64, 4503599627370496.0, -4503599627370496.0] {
            for d in [-1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5] {
                vs.push(b + d);
            }
        }
        let mut r = crate::rng::LegacyRandom::new(7);
        for _ in 0..100_000 {
            vs.push((r.next_double() - 0.5) * 1.0e4);
        }
        for v in vs {
            assert_eq!(floor(v), v.floor() as i32, "floor({v})");
            assert_eq!(lfloor(v), v.floor() as i64, "lfloor({v})");
        }
    }
}
