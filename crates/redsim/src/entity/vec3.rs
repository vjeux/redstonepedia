//! `Vec3` and `AABB` (net.minecraft.world.phys) with the game's exact double arithmetic and
//! operation order, and Java's `Math.min` / `Math.max` (a NaN wins, -0.0 < 0.0).

/// Java's `Math.min(double, double)`.
#[inline]
pub fn jmin(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && (a.is_sign_negative() || b.is_sign_negative()) {
        return -0.0;
    }
    if a <= b { a } else { b }
}

/// Java's `Math.max(double, double)`.
#[inline]
pub fn jmax(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && (a.is_sign_positive() || b.is_sign_positive()) {
        return 0.0;
    }
    if a >= b { a } else { b }
}

/// `Vec3`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    /// `normalize`: ZERO under 1.0E-5F (the float literal, widened).
    pub fn normalize(self) -> Vec3 {
        let dist = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if dist < 1.0e-5f32 as f64 { Vec3::ZERO } else { Vec3::new(self.x / dist, self.y / dist, self.z / dist) }
    }

    pub fn dot(self, v: Vec3) -> f64 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    pub fn cross(self, v: Vec3) -> Vec3 {
        Vec3::new(self.y * v.z - self.z * v.y, self.z * v.x - self.x * v.z, self.x * v.y - self.y * v.x)
    }

    /// `add(x, y, z)`.
    pub fn add(self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(self.x + x, self.y + y, self.z + z)
    }

    pub fn add_v(self, v: Vec3) -> Vec3 {
        self.add(v.x, v.y, v.z)
    }

    /// `subtract(x, y, z)` = `add(-x, -y, -z)`.
    pub fn subtract(self, x: f64, y: f64, z: f64) -> Vec3 {
        self.add(-x, -y, -z)
    }

    pub fn subtract_v(self, v: Vec3) -> Vec3 {
        self.subtract(v.x, v.y, v.z)
    }

    /// `multiply(x, y, z)`.
    pub fn multiply(self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(self.x * x, self.y * y, self.z * z)
    }

    pub fn scale(self, s: f64) -> Vec3 {
        self.multiply(s, s, s)
    }

    /// `Vec3.yRot(float)`: Mth's float sine/cosine, widened for the double products.
    pub fn y_rot(self, radians: f32) -> Vec3 {
        let cos = super::mth::cos(radians as f64) as f64;
        let sin = super::mth::sin(radians as f64) as f64;
        Vec3::new(self.x * cos + self.z * sin, self.y, self.z * cos - self.x * sin)
    }

    pub fn reverse(self) -> Vec3 {
        self.scale(-1.0)
    }

    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn length_sqr(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn horizontal_distance(self) -> f64 {
        (self.x * self.x + self.z * self.z).sqrt()
    }

    pub fn horizontal_distance_sqr(self) -> f64 {
        self.x * self.x + self.z * self.z
    }

    pub fn distance_to_sqr(self, v: Vec3) -> f64 {
        let (dx, dy, dz) = (v.x - self.x, v.y - self.y, v.z - self.z);
        dx * dx + dy * dy + dz * dz
    }

    /// `with(axis, value)`: 0 = x, 1 = y, 2 = z.
    pub fn with_axis(self, axis: usize, v: f64) -> Vec3 {
        match axis {
            0 => Vec3::new(v, self.y, self.z),
            1 => Vec3::new(self.x, v, self.z),
            _ => Vec3::new(self.x, self.y, v),
        }
    }

    pub fn get(self, axis: usize) -> f64 {
        match axis {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
}

/// `AABB`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl Aabb {
    /// `new AABB(...)`: min/max sorted per axis.
    #[inline]
    pub fn new(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Aabb {
        Aabb { min_x: jmin(x0, x1), min_y: jmin(y0, y1), min_z: jmin(z0, z1), max_x: jmax(x0, x1), max_y: jmax(y0, y1), max_z: jmax(z0, z1) }
    }

    /// `new AABB(BlockPos)`.
    pub fn of_block(p: crate::pos::BlockPos) -> Aabb {
        Aabb::new(p.x as f64, p.y as f64, p.z as f64, (p.x + 1) as f64, (p.y + 1) as f64, (p.z + 1) as f64)
    }

    pub fn move_by(self, x: f64, y: f64, z: f64) -> Aabb {
        Aabb::new(self.min_x + x, self.min_y + y, self.min_z + z, self.max_x + x, self.max_y + y, self.max_z + z)
    }

    pub fn move_v(self, v: Vec3) -> Aabb {
        self.move_by(v.x, v.y, v.z)
    }

    pub fn inflate(self, x: f64, y: f64, z: f64) -> Aabb {
        Aabb::new(self.min_x - x, self.min_y - y, self.min_z - z, self.max_x + x, self.max_y + y, self.max_z + z)
    }

    pub fn deflate(self, x: f64, y: f64, z: f64) -> Aabb {
        self.inflate(-x, -y, -z)
    }

    pub fn expand_towards(self, x: f64, y: f64, z: f64) -> Aabb {
        let (mut x0, mut y0, mut z0, mut x1, mut y1, mut z1) = (self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z);
        if x < 0.0 {
            x0 += x;
        } else if x > 0.0 {
            x1 += x;
        }
        if y < 0.0 {
            y0 += y;
        } else if y > 0.0 {
            y1 += y;
        }
        if z < 0.0 {
            z0 += z;
        } else if z > 0.0 {
            z1 += z;
        }
        Aabb::new(x0, y0, z0, x1, y1, z1)
    }

    pub fn contract(self, x: f64, y: f64, z: f64) -> Aabb {
        let (mut x0, mut y0, mut z0, mut x1, mut y1, mut z1) = (self.min_x, self.min_y, self.min_z, self.max_x, self.max_y, self.max_z);
        if x < 0.0 {
            x0 -= x;
        } else if x > 0.0 {
            x1 -= x;
        }
        if y < 0.0 {
            y0 -= y;
        } else if y > 0.0 {
            y1 -= y;
        }
        if z < 0.0 {
            z0 -= z;
        } else if z > 0.0 {
            z1 -= z;
        }
        Aabb::new(x0, y0, z0, x1, y1, z1)
    }

    pub fn intersect(self, o: Aabb) -> Aabb {
        Aabb::new(jmax(self.min_x, o.min_x), jmax(self.min_y, o.min_y), jmax(self.min_z, o.min_z), jmin(self.max_x, o.max_x), jmin(self.max_y, o.max_y), jmin(self.max_z, o.max_z))
    }

    pub fn minmax(self, o: Aabb) -> Aabb {
        Aabb::new(jmin(self.min_x, o.min_x), jmin(self.min_y, o.min_y), jmin(self.min_z, o.min_z), jmax(self.max_x, o.max_x), jmax(self.max_y, o.max_y), jmax(self.max_z, o.max_z))
    }

    pub fn intersects(self, o: Aabb) -> bool {
        self.min_x < o.max_x && self.max_x > o.min_x && self.min_y < o.max_y && self.max_y > o.min_y && self.min_z < o.max_z && self.max_z > o.min_z
    }

    pub fn x_size(self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn y_size(self) -> f64 {
        self.max_y - self.min_y
    }

    pub fn z_size(self) -> f64 {
        self.max_z - self.min_z
    }

    /// `getCenter`.
    pub fn center(self) -> Vec3 {
        Vec3::new(crate::entity::mth::lerp(0.5, self.min_x, self.max_x), crate::entity::mth::lerp(0.5, self.min_y, self.max_y), crate::entity::mth::lerp(0.5, self.min_z, self.max_z))
    }

    /// `min(axis)` / `max(axis)`: 0 = x, 1 = y, 2 = z.
    pub fn min(self, axis: usize) -> f64 {
        match axis {
            0 => self.min_x,
            1 => self.min_y,
            _ => self.min_z,
        }
    }

    pub fn max(self, axis: usize) -> f64 {
        match axis {
            0 => self.max_x,
            1 => self.max_y,
            _ => self.max_z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_min_max() {
        assert!(jmin(0.0, -0.0).is_sign_negative());
        assert!(jmax(-0.0, 0.0).is_sign_positive());
        assert!(jmin(f64::NAN, 1.0).is_nan());
        assert_eq!(jmin(1.0, 2.0), 1.0);
    }

    #[test]
    fn boxes() {
        let b = Aabb::new(1.0, 2.0, 3.0, 0.0, 0.0, 0.0);
        assert_eq!((b.min_x, b.max_z), (0.0, 3.0));
        let e = b.expand_towards(-0.5, 0.0, 0.25);
        assert_eq!((e.min_x, e.max_z), (-0.5, 3.25));
        assert!(b.intersects(Aabb::new(0.5, 0.5, 0.5, 2.0, 2.0, 2.0)));
        assert!(!b.intersects(Aabb::new(1.0, 0.0, 0.0, 2.0, 1.0, 1.0)));
        assert_eq!(Vec3::new(3.0, 0.0, 4.0).normalize(), Vec3::new(0.6, 0.0, 0.8));
        assert_eq!(Vec3::new(1e-6, 0.0, 0.0).normalize(), Vec3::ZERO);
    }
}
