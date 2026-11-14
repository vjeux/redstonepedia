//! fdlibm's `asin` and `pow` (Sun's Freely Distributable Math Library, through FreeBSD's msun
//! and musl, as the `libm` crate renders them in Rust), so redsim computes them the same on every
//! platform: the browser's wasm has no libc, and its `f64::asin` / `f64::powf` (compiler-builtins'
//! libm) differ from the native glibc ones in the last bit (9 of the 257 `Mth.atan2` table
//! entries, 6% of the brain's shuffle weights): with the mob AI on, the viewer's run left the
//! judge's within a second. Java's `Math.asin` is `StrictMath.asin`, this fdlibm code, so the
//! table is now the game's own; `Math.pow` is a HotSpot intrinsic, which this matches in all but
//! rare last bits (the weights only order a shuffle).
//!
//! ====================================================
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//!
//! Developed at SunSoft, a Sun Microsystems, Inc. business.
//! Permission to use, copy, modify, and distribute this
//! software is freely granted, provided that this notice
//! is preserved.
//! ====================================================

fn get_high_word(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32
}

fn get_low_word(x: f64) -> u32 {
    x.to_bits() as u32
}

fn with_set_high_word(f: f64, hi: u32) -> f64 {
    f64::from_bits((f.to_bits() & 0x0000_0000_ffff_ffff) | ((hi as u64) << 32))
}

fn with_set_low_word(f: f64, lo: u32) -> f64 {
    f64::from_bits((f.to_bits() & 0xffff_ffff_0000_0000) | lo as u64)
}

/// musl's `scalbn`: `x * 2^n`.
fn scalbn(x: f64, mut n: i32) -> f64 {
    let x1p1023 = f64::from_bits(0x7fe0_0000_0000_0000);
    let x1p53 = f64::from_bits(0x4340_0000_0000_0000);
    let x1p_1022 = f64::from_bits(0x0010_0000_0000_0000);
    let mut y = x;
    if n > 1023 {
        y *= x1p1023;
        n -= 1023;
        if n > 1023 {
            y *= x1p1023;
            n -= 1023;
            if n > 1023 {
                n = 1023;
            }
        }
    } else if n < -1022 {
        y *= x1p_1022 * x1p53;
        n += 1022 - 53;
        if n < -1022 {
            y *= x1p_1022 * x1p53;
            n += 1022 - 53;
            if n < -1022 {
                n = -1022;
            }
        }
    }
    y * f64::from_bits(((0x3ff + n) as u64) << 52)
}

// (ironspec 09:26a (1)) fdlibm's pio2_hi 1.57079632679489655800e+00 (0x3FF921FB, 0x54442D18) is f64's FRAC_PI_2 bit
// for bit (the test `fdlibms_constants_are_the_std_ones_bit_for_bit`)
const PIO2_HI: f64 = std::f64::consts::FRAC_PI_2;
const PIO2_LO: f64 = 6.12323399573676603587e-17;
const P_S0: f64 = 1.66666666666666657415e-01;
const P_S1: f64 = -3.25565818622400915405e-01;
const P_S2: f64 = 2.01212532134862925881e-01;
const P_S3: f64 = -4.00555345006794114027e-02;
const P_S4: f64 = 7.91534994289814532176e-04;
const P_S5: f64 = 3.47933107596021167570e-05;
const Q_S1: f64 = -2.40339491173441421878e+00;
const Q_S2: f64 = 2.02094576023350569471e+00;
const Q_S3: f64 = -6.88283971605453293030e-01;
const Q_S4: f64 = 7.70381505559019352791e-02;

fn comp_r(z: f64) -> f64 {
    let p = z * (P_S0 + z * (P_S1 + z * (P_S2 + z * (P_S3 + z * (P_S4 + z * P_S5)))));
    let q = 1.0 + z * (Q_S1 + z * (Q_S2 + z * (Q_S3 + z * Q_S4)));
    p / q
}

/// `StrictMath.asin` (fdlibm's `e_asin.c`).
pub fn asin(mut x: f64) -> f64 {
    let hx = get_high_word(x);
    let ix = hx & 0x7fff_ffff;
    // |x| >= 1 or NaN
    if ix >= 0x3ff0_0000 {
        let lx = get_low_word(x);
        if ((ix - 0x3ff0_0000) | lx) == 0 {
            // asin(1) = +-pi/2 with inexact
            return x * PIO2_HI + f64::from_bits(0x3870_0000_0000_0000);
        }
        #[expect(clippy::eq_op, reason = "fdlibm's NaN idiom, kept verbatim in the StrictMath port")]
        return 0.0 / (x - x);
    }
    // |x| < 0.5
    if ix < 0x3fe0_0000 {
        // 0x1p-1022 <= |x| < 0x1p-26: x itself
        if (0x0010_0000..0x3e50_0000).contains(&ix) {
            return x;
        }
        return x + x * comp_r(x * x);
    }
    // 1 > |x| >= 0.5
    let z = (1.0 - x.abs()) * 0.5;
    let s = z.sqrt();
    let r = comp_r(z);
    if ix >= 0x3fef_3333 {
        // |x| > 0.975
        x = PIO2_HI - (2. * (s + s * r) - PIO2_LO);
    } else {
        // f + c = sqrt(z)
        let f = with_set_low_word(s, 0);
        let c = (z - f * f) / (s + f);
        x = 0.5 * PIO2_HI - (2.0 * s * r - (PIO2_LO - 2.0 * c) - (0.5 * PIO2_HI - 2.0 * f));
    }
    if hx >> 31 != 0 { -x } else { x }
}

const BP: [f64; 2] = [1.0, 1.5];
const DP_H: [f64; 2] = [0.0, 5.84962487220764160156e-01];
const DP_L: [f64; 2] = [0.0, 1.35003920212974897128e-08];
const TWO53: f64 = 9007199254740992.0;
const HUGE: f64 = 1.0e300;
const TINY: f64 = 1.0e-300;
const L1: f64 = 5.99999999999994648725e-01;
const L2: f64 = 4.28571428578550184252e-01;
const L3: f64 = 3.33333329818377432918e-01;
const L4: f64 = 2.72728123808534006489e-01;
const L5: f64 = 2.30660745775561754067e-01;
const L6: f64 = 2.06975017800338417784e-01;
const P1: f64 = 1.66666666666666019037e-01;
const P2: f64 = -2.77777777770155933842e-03;
const P3: f64 = 6.61375632143793436117e-05;
const P4: f64 = -1.65339022054652515390e-06;
const P5: f64 = 4.13813679705723846039e-08;
// (ironspec 09:26a (1)) fdlibm's lg2 6.93147180559945286227e-01 (0x3FE62E42, 0xFEFA39EF) is f64's LN_2 bit for bit
const LG2: f64 = std::f64::consts::LN_2;
const LG2_H: f64 = 6.93147182464599609375e-01;
const LG2_L: f64 = -1.90465429995776804525e-09;
const OVT: f64 = 8.0085662595372944372e-017;
const CP: f64 = 9.61796693925975554329e-01;
const CP_H: f64 = 9.61796700954437255859e-01;
const CP_L: f64 = -7.02846165095275826516e-09;
// (ironspec 09:26a (1)) fdlibm's ivln2 1.44269504088896338700e+00 (0x3FF71547, 0x652B82FE, 1/ln2) is f64's LOG2_E bit
// for bit
const IVLN2: f64 = std::f64::consts::LOG2_E;
const IVLN2_H: f64 = 1.44269502162933349609e+00;
const IVLN2_L: f64 = 1.92596299112661746887e-08;

/// `StrictMath.pow` (fdlibm's `e_pow.c`).
pub fn pow(x: f64, y: f64) -> f64 {
    let t1: f64;
    let t2: f64;
    let (hx, lx): (i32, u32) = ((x.to_bits() >> 32) as i32, x.to_bits() as u32);
    let (hy, ly): (i32, u32) = ((y.to_bits() >> 32) as i32, y.to_bits() as u32);
    let mut ix: i32 = hx & 0x7fff_ffff;
    let iy: i32 = hy & 0x7fff_ffff;
    // x**0 = 1, even if x is NaN
    if ((iy as u32) | ly) == 0 {
        return 1.0;
    }
    // 1**y = 1, even if y is NaN
    if hx == 0x3ff0_0000 && lx == 0 {
        return 1.0;
    }
    // NaN if either argument is NaN
    if ix > 0x7ff0_0000 || (ix == 0x7ff0_0000 && lx != 0) || iy > 0x7ff0_0000 || (iy == 0x7ff0_0000 && ly != 0) {
        return x + y;
    }
    // x < 0: yisint 0 (not an integer), 1 (odd), 2 (even)
    let mut yisint: i32 = 0;
    let mut k: i32;
    let mut j: i32;
    if hx < 0 {
        if iy >= 0x4340_0000 {
            yisint = 2;
        } else if iy >= 0x3ff0_0000 {
            k = (iy >> 20) - 0x3ff;
            if k > 20 {
                j = (ly >> (52 - k)) as i32;
                if (j << (52 - k)) == (ly as i32) {
                    yisint = 2 - (j & 1);
                }
            } else if ly == 0 {
                j = iy >> (20 - k);
                if (j << (20 - k)) == iy {
                    yisint = 2 - (j & 1);
                }
            }
        }
    }
    // special values of y
    if ly == 0 {
        if iy == 0x7ff0_0000 {
            // y is +-inf
            return if ((ix - 0x3ff0_0000) | (lx as i32)) == 0 {
                1.0
            } else if ix >= 0x3ff0_0000 {
                if hy >= 0 { y } else { 0.0 }
            } else if hy >= 0 {
                0.0
            } else {
                -y
            };
        }
        if iy == 0x3ff0_0000 {
            // y is +-1
            return if hy >= 0 { x } else { 1.0 / x };
        }
        if hy == 0x4000_0000 {
            return x * x;
        }
        if hy == 0x3fe0_0000 && hx >= 0 {
            // y is 0.5, x >= +0
            return x.sqrt();
        }
    }
    let mut ax: f64 = x.abs();
    // special values of x
    if lx == 0 && (ix == 0x7ff0_0000 || ix == 0 || ix == 0x3ff0_0000) {
        // x is +-0, +-inf, +-1
        let mut z: f64 = ax;
        if hy < 0 {
            z = 1.0 / z;
        }
        if hx < 0 {
            if ((ix - 0x3ff0_0000) | yisint) == 0 {
                #[expect(clippy::eq_op, reason = "fdlibm's NaN idiom, kept verbatim in the StrictMath port")]
                let nan = (z - z) / (z - z); // (-1)**non-int is NaN
                z = nan;
            } else if yisint == 1 {
                z = -z; // (x<0)**odd = -(|x|**odd)
            }
        }
        return z;
    }
    let mut s: f64 = 1.0; // the result's sign
    if hx < 0 {
        if yisint == 0 {
            #[expect(clippy::eq_op, reason = "fdlibm's NaN idiom, kept verbatim in the StrictMath port")]
            return (x - x) / (x - x); // (x<0)**(non-int) is NaN
        }
        if yisint == 1 {
            s = -1.0; // (x<0)**(odd int)
        }
    }
    if iy > 0x41e0_0000 {
        // |y| > 2**31
        if iy > 0x43f0_0000 {
            // |y| > 2**64: must o/uflow
            if ix <= 0x3fef_ffff {
                return if hy < 0 { HUGE * HUGE } else { TINY * TINY };
            }
            if ix >= 0x3ff0_0000 {
                return if hy > 0 { HUGE * HUGE } else { TINY * TINY };
            }
        }
        // over/underflow if x is not close to one
        if ix < 0x3fef_ffff {
            return if hy < 0 { s * HUGE * HUGE } else { s * TINY * TINY };
        }
        if ix > 0x3ff0_0000 {
            return if hy > 0 { s * HUGE * HUGE } else { s * TINY * TINY };
        }
        // now |1-x| is tiny <= 2**-20: log(x) by x-x^2/2+x^3/3-x^4/4
        let t: f64 = ax - 1.0;
        let w: f64 = (t * t) * (0.5 - t * (0.3333333333333333333333 - t * 0.25));
        let u: f64 = IVLN2_H * t;
        let v: f64 = t * IVLN2_L - w * IVLN2;
        t1 = with_set_low_word(u + v, 0);
        t2 = v - (t1 - u);
    } else {
        let mut n: i32 = 0;
        // a subnormal x
        if ix < 0x0010_0000 {
            ax *= TWO53;
            n -= 53;
            ix = get_high_word(ax) as i32;
        }
        n += (ix >> 20) - 0x3ff;
        j = ix & 0x000f_ffff;
        // its interval
        let k: usize;
        ix = j | 0x3ff0_0000;
        if j <= 0x3988e {
            k = 0; // |x| < sqrt(3/2)
        } else if j < 0xbb67a {
            k = 1; // |x| < sqrt(3)
        } else {
            k = 0;
            n += 1;
            ix -= 0x0010_0000;
        }
        ax = with_set_high_word(ax, ix as u32);
        // ss = s_h + s_l = (x-1)/(x+1) or (x-1.5)/(x+1.5)
        let u: f64 = ax - BP[k];
        let v: f64 = 1.0 / (ax + BP[k]);
        let ss: f64 = u * v;
        let s_h = with_set_low_word(ss, 0);
        // t_h = ax + bp[k], high
        let t_h: f64 = with_set_high_word(0.0, ((ix as u32 >> 1) | 0x2000_0000) + 0x0008_0000 + ((k as u32) << 18));
        let t_l: f64 = ax - (t_h - BP[k]);
        let s_l: f64 = v * ((u - s_h * t_h) - s_h * t_l);
        // log(ax)
        let s2: f64 = ss * ss;
        let mut r: f64 = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
        r += s_l * (s_h + ss);
        let s2: f64 = s_h * s_h;
        let t_h: f64 = with_set_low_word(3.0 + s2 + r, 0);
        let t_l: f64 = r - ((t_h - 3.0) - s2);
        // u + v = ss * (1 + ...)
        let u: f64 = s_h * t_h;
        let v: f64 = s_l * t_h + t_l * ss;
        // 2/(3log2) * (ss + ...)
        let p_h: f64 = with_set_low_word(u + v, 0);
        let p_l = v - (p_h - u);
        let z_h: f64 = CP_H * p_h;
        let z_l: f64 = CP_L * p_h + p_l * CP + DP_L[k];
        // log2(ax) = (ss + ..) * 2/(3*log2) = n + dp_h + z_h + z_l
        let t: f64 = n as f64;
        t1 = with_set_low_word(((z_h + z_l) + DP_H[k]) + t, 0);
        t2 = z_l - (((t1 - t) - DP_H[k]) - z_h);
    }
    // split y into y1 + y2 and (y1 + y2) * (t1 + t2)
    let y1: f64 = with_set_low_word(y, 0);
    let p_l: f64 = (y - y1) * t1 + y * t2;
    let mut p_h: f64 = y1 * t1;
    let z: f64 = p_l + p_h;
    let mut j: i32 = (z.to_bits() >> 32) as i32;
    let i: i32 = z.to_bits() as i32;
    if j >= 0x4090_0000 {
        // z >= 1024
        if (j - 0x4090_0000) | i != 0 {
            return s * HUGE * HUGE;
        }
        if p_l + OVT > z - p_h {
            return s * HUGE * HUGE;
        }
    } else if (j & 0x7fff_ffff) >= 0x4090_cc00 {
        // z <= -1075
        if ((j as u32).wrapping_sub(0xc090_cc00) | (i as u32)) != 0 {
            return s * TINY * TINY;
        }
        if p_l <= z - p_h {
            return s * TINY * TINY;
        }
    }
    // 2**(p_h + p_l)
    let i: i32 = j & 0x7fff_ffff;
    k = (i >> 20) - 0x3ff;
    let mut n: i32 = 0;
    if i > 0x3fe0_0000 {
        // |z| > 0.5: n = [z + 0.5]
        n = j + (0x0010_0000 >> (k + 1));
        k = ((n & 0x7fff_ffff) >> 20) - 0x3ff;
        let t: f64 = with_set_high_word(0.0, (n & !(0x000f_ffff >> k)) as u32);
        n = ((n & 0x000f_ffff) | 0x0010_0000) >> (20 - k);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    let t: f64 = with_set_low_word(p_l + p_h, 0);
    let u: f64 = t * LG2_H;
    let v: f64 = (p_l - (t - p_h)) * LG2 + t * LG2_L;
    let mut z: f64 = u + v;
    let w: f64 = v - (z - u);
    let t: f64 = z * z;
    let t1: f64 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    let r: f64 = (z * t1) / (t1 - 2.0) - (w + z * w);
    z = 1.0 - (r - z);
    j = get_high_word(z) as i32;
    j = j.wrapping_add(n << 20);
    if (j >> 20) <= 0 {
        // a subnormal result
        z = scalbn(z, n);
    } else {
        z = with_set_high_word(z, j as u32);
    }
    s * z
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (ironspec 09:26a (1)) the three fdlibm literals clippy took for std constants ARE them, bit for bit (fdlibm's own
    /// hex comments): the port stays bit-exact with the std names
    #[test]
    fn fdlibms_constants_are_the_std_ones_bit_for_bit() {
        assert_eq!(PIO2_HI.to_bits(), 0x3FF9_21FB_5444_2D18, "e_asin.c's pio2_hi");
        assert_eq!(LG2.to_bits(), 0x3FE6_2E42_FEFA_39EF, "e_pow.c's lg2");
        assert_eq!(IVLN2.to_bits(), 0x3FF7_1547_652B_82FE, "e_pow.c's ivln2");
    }

    /// Within a last bit of the platform's own (the native glibc functions are correctly rounded
    /// or nearly), exact where both are exact.
    #[test]
    fn asin_and_pow_are_the_math_functions() {
        let ulps = |a: f64, b: f64| (a.to_bits() as i64 - b.to_bits() as i64).abs();
        for i in 0..=4096 {
            let v = i as f64 / 4096.0;
            assert!(ulps(asin(v), v.asin()) <= 1, "asin({v}) = {} vs {}", asin(v), v.asin());
            assert!(ulps(asin(-v), (-v).asin()) <= 1);
        }
        for k in 1..=2000 {
            let f = k as f64 / 2001.0;
            for w in 1..=12 {
                let e = (1.0f32 / w as f32) as f64;
                assert!(ulps(pow(f, e), f.powf(e)) <= 1, "pow({f}, {e}) = {} vs {}", pow(f, e), f.powf(e));
            }
        }
        assert_eq!(asin(1.0), std::f64::consts::FRAC_PI_2);
        assert_eq!(asin(0.0), 0.0);
        assert_eq!(pow(0.25, 0.5), 0.5);
        assert_eq!(pow(2.0, 10.0), 1024.0);
        assert_eq!(pow(0.0, 0.5), 0.0);
        assert!(pow(-2.0, 0.5).is_nan());
        assert_eq!(pow(-2.0, 3.0), -8.0);
    }
}
