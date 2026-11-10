//! The camera: an orbit around a target, framed like the real-game pictures (`mctas clock shot`:
//! looking from the west, above and south, vertical FOV 70, the box filling 85% of the view).

pub type Mat4 = [[f32; 4]; 4];

/// `crates/clock/src/shot.rs::CAM_DIR`: where the camera sits relative to the build.
pub const CAM_DIR: [f32; 3] = [-0.55, 0.6, 1.0];
pub const VFOV: f32 = 70.0;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// FxHash's step (a multiply and a rotate): [`hull_candidates`]' maps, keyed by two coordinates'
/// bits, and the picker's cell maps (the standard SipHash costs more than the work it saves: a
/// big card's camera search spent a third of its time hashing cells). Public: the terrain farm's
/// public maps (`TerrainFarm::cut_rows`, `landscape_index`) are keyed with it.
#[derive(Default)]
pub struct FxHasher(u64);

/// A map or set with [`FxHasher`] (`HashMap<K, V, FxBuild>`; made with `default()`).
pub type FxBuild = std::hash::BuildHasherDefault<FxHasher>;

impl std::hash::Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_u32(&mut self, n: u32) {
        self.write_u64(n as u64);
    }
    fn write_i32(&mut self, n: i32) {
        self.write_u64(n as u32 as u64);
    }
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

/// The points that can be extreme in a view, exactly: a point is dropped when, along one of the
/// three axis-aligned lines through it, other points lie on both sides of it (it is inside their
/// segment, so inside the points' convex hull). Every corner of the hull stays, and a projection's
/// extremes (the farthest left, right, up, down, and the nearest) are at the hull's corners, so a
/// camera fitted to these is the camera fitted to all of them. A block build's 8 corners per cell
/// (most shared, most inside) shrink to its outer shell's corners. Few points: returned as given.
pub fn hull_candidates(pts: &[[f32; 3]]) -> Vec<[f32; 3]> {
    if pts.len() <= 64 {
        return pts.to_vec();
    }
    // (the same points asked again: the camera search and the default camera fit a farm's 1.2
    // million cell corners each: the last big answer kept, with its points to compare)
    thread_local! {
        static LAST: std::cell::RefCell<Option<(Vec<[f32; 3]>, Vec<[f32; 3]>)>> = const { std::cell::RefCell::new(None) };
    }
    let big = pts.len() >= 100_000;
    if big && let Some(out) = LAST.with(|l| l.borrow().as_ref().filter(|(p, _)| bytemuck::cast_slice::<[f32; 3], u8>(p) == bytemuck::cast_slice::<[f32; 3], u8>(pts)).map(|(_, o)| o.clone())) {
        return out;
    }
    let out = match hull_candidates_dense(pts) {
        Some(out) => out,
        None if pts.iter().any(|p| p.iter().any(|c| c.is_nan())) => hull_candidates_hashed(pts),
        None => hull_candidates_sorted(pts),
    };
    if big {
        LAST.with(|l| *l.borrow_mut() = Some((pts.to_vec(), out.clone())));
    }
    out
}

/// [`hull_candidates`] for any points but NaNs, by sorting: the points once (each kept once,
/// by their bits), then along each axis by their line (the other two coordinates' bits) and
/// their place on it, its ends the first and the last; the same points as the maps' (test
/// `hull_candidates_sorted_are_the_hashed`), in a quarter of their time on a farm's 1.2 million.
fn hull_candidates_sorted(pts: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut u: Vec<[u32; 3]> = pts.iter().map(|p| p.map(f32::to_bits)).collect();
    u.sort_unstable();
    u.dedup();
    let val = |b: u32| f32::from_bits(b);
    let mut keep = vec![true; u.len()];
    let mut ord: Vec<u32> = (0..u.len() as u32).collect();
    for a in 0..3 {
        let (i, j) = match a {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        ord.sort_unstable_by(|&x, &y| {
            let (p, q) = (&u[x as usize], &u[y as usize]);
            (p[i], p[j]).cmp(&(q[i], q[j])).then(val(p[a]).total_cmp(&val(q[a])))
        });
        let mut s = 0;
        while s < ord.len() {
            let line = |k: usize| (u[ord[k] as usize][i], u[ord[k] as usize][j]);
            let mut e = s + 1;
            while e < ord.len() && line(e) == line(s) {
                e += 1;
            }
            let (lo, hi) = (val(u[ord[s] as usize][a]), val(u[ord[e - 1] as usize][a]));
            for &k in &ord[s..e] {
                let v = val(u[k as usize][a]);
                if !(v <= lo || v >= hi) {
                    keep[k as usize] = false;
                }
            }
            s = e;
        }
    }
    u.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| p.map(f32::from_bits)).collect()
}

/// [`hull_candidates`] for any points: each axis line's ends in a map keyed by the other two
/// coordinates' bits.
fn hull_candidates_hashed(pts: &[[f32; 3]]) -> Vec<[f32; 3]> {
    type Lines = std::collections::HashMap<u64, (f32, f32), FxBuild>;
    // the line along axis `a` through `p`: the other two coordinates' bits
    let line = |p: &[f32; 3], a: usize| -> u64 {
        let (i, j) = match a {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        ((p[i].to_bits() as u64) << 32) | p[j].to_bits() as u64
    };
    let mut ends: [Lines; 3] = Default::default();
    for p in pts {
        for (a, m) in ends.iter_mut().enumerate() {
            let e = m.entry(line(p, a)).or_insert((p[a], p[a]));
            e.0 = e.0.min(p[a]);
            e.1 = e.1.max(p[a]);
        }
    }
    let mut out: Vec<[f32; 3]> = pts
        .iter()
        .filter(|p| {
            ends.iter().enumerate().all(|(a, m)| {
                let (lo, hi) = m[&line(p, a)];
                p[a] <= lo || p[a] >= hi
            })
        })
        .copied()
        .collect();
    out.sort_by(|a, b| a.map(f32::to_bits).cmp(&b.map(f32::to_bits)));
    out.dedup_by(|a, b| a.map(f32::to_bits) == b.map(f32::to_bits));
    out
}

/// [`hull_candidates`] for whole-number points in a modest box (a build's cell corners, 8 a cell:
/// 1.6 million for a farm in its terrain, whose maps took 1.4 s natively): each axis line's ends
/// in a dense grid over the other two coordinates, no hashing; the same points (a line is the
/// same line: whole numbers, no negative zero, one to one with their bits). None: not such points.
fn hull_candidates_dense(pts: &[[f32; 3]]) -> Option<Vec<[f32; 3]>> {
    let (mut lo, mut hi) = ([i64::MAX; 3], [i64::MIN; 3]);
    for p in pts {
        for k in 0..3 {
            let c = p[k];
            if c != c.trunc() || c.abs() > 1e6 || c.to_bits() == (-0.0f32).to_bits() {
                return None;
            }
            lo[k] = lo[k].min(c as i64);
            hi[k] = hi[k].max(c as i64);
        }
    }
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    // the line along x through a point: its (y, z); along y: (x, z); along z: (x, y)
    let sizes = [d[1] * d[2], d[0] * d[2], d[0] * d[1]];
    if sizes.iter().any(|&s| s > 1 << 24) {
        return None;
    }
    let at = |p: &[f32; 3], a: usize| -> usize {
        let c = [p[0] as i64 - lo[0], p[1] as i64 - lo[1], p[2] as i64 - lo[2]];
        (match a {
            0 => c[1] * d[2] + c[2],
            1 => c[0] * d[2] + c[2],
            _ => c[0] * d[1] + c[1],
        }) as usize
    };
    let mut ends: [Vec<(f32, f32)>; 3] = sizes.map(|s| vec![(f32::INFINITY, f32::NEG_INFINITY); s as usize]);
    for p in pts {
        for (a, e) in ends.iter_mut().enumerate() {
            let e = &mut e[at(p, a)];
            e.0 = e.0.min(p[a]);
            e.1 = e.1.max(p[a]);
        }
    }
    let mut out: Vec<[f32; 3]> = pts
        .iter()
        .filter(|p| {
            ends.iter().enumerate().all(|(a, e)| {
                let (l, h) = e[at(p, a)];
                p[a] <= l || p[a] >= h
            })
        })
        .copied()
        .collect();
    out.sort_by(|a, b| a.map(f32::to_bits).cmp(&b.map(f32::to_bits)));
    out.dedup_by(|a, b| a.map(f32::to_bits) == b.map(f32::to_bits));
    Some(out)
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub target: [f32; 3],
    /// Horizontal angle of the eye around the target (radians, 0 = south of it, +x = east).
    pub yaw: f32,
    /// Elevation of the eye (radians).
    pub pitch: f32,
    pub dist: f32,
    pub vfov: f32,
}

impl Camera {
    /// Unit vector from the target to the eye.
    pub fn dir(&self) -> [f32; 3] {
        [self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos()]
    }

    pub fn eye(&self) -> [f32; 3] {
        let d = self.dir();
        [self.target[0] + d[0] * self.dist, self.target[1] + d[1] * self.dist, self.target[2] + d[2] * self.dist]
    }

    /// Camera basis: forward, right, up.
    pub fn basis(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let d = self.dir();
        let f = [-d[0], -d[1], -d[2]];
        let right = norm(cross(f, [0.0, 1.0, 0.0]));
        let up = cross(right, f);
        (f, right, up)
    }

    /// The camera at `dir` (target -> eye), framing the box `lo..hi` so it fills `fill` of the
    /// view (`mctas::video::frame_box`).
    pub fn frame(lo: [f32; 3], hi: [f32; 3], dir: [f32; 3], vfov_deg: f32, aspect: f32, fill: f32) -> Camera {
        let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let d = norm(dir);
        let yaw = d[0].atan2(d[2]);
        let pitch = d[1].clamp(-1.0, 1.0).asin();
        let mut cam = Camera { target: center, yaw, pitch, dist: 10.0, vfov: vfov_deg };
        cam.fit(lo, hi, aspect, fill);
        cam
    }

    /// Keep the direction, move along it until the box fills `fill` of the view.
    pub fn fit(&mut self, lo: [f32; 3], hi: [f32; 3], aspect: f32, fill: f32) {
        let tan_v = (self.vfov.to_radians() / 2.0).tan();
        let tan_h = tan_v * aspect;
        let corners: Vec<[f32; 3]> = (0..8).map(|i| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }]).collect();
        let (f, right, up) = self.basis();
        let d = self.dir();
        let center = self.target;
        let extent = |dist: f32| -> f32 {
            let eye = [center[0] + d[0] * dist, center[1] + d[1] * dist, center[2] + d[2] * dist];
            let mut m: f32 = 0.0;
            for c in &corners {
                let v = sub(*c, eye);
                let z = dot(v, f);
                if z <= 0.1 {
                    return f32::INFINITY;
                }
                m = m.max((dot(v, right) / z).abs() / tan_h).max((dot(v, up) / z).abs() / tan_v);
            }
            m
        };
        let (mut a, mut b) = (0.5f32, 2000.0f32);
        for _ in 0..60 {
            let mid = (a + b) / 2.0;
            if extent(mid) > fill {
                a = mid
            } else {
                b = mid
            }
        }
        self.dist = b;
    }

    /// Keep the direction; move the target and the distance so that every point projects inside
    /// the view window `x` in [-wx, wx], `y` in [y0, y1] (normalized view coordinates, y up)
    /// and the points' projected box is centred in that window (the build fills the window's
    /// limiting side). Used to frame a build tightly: its cells' corners, not its bounding box.
    pub fn fit_points(&mut self, pts: &[[f32; 3]], aspect: f32, wx: f32, y0: f32, y1: f32) {
        self.fit_points_min(pts, aspect, wx, y0, y1, 0.0)
    }

    /// [`Camera::fit_points`], at least `min_dist` away (a tiny build framed as if it were
    /// bigger, still centred in the window).
    pub fn fit_points_min(&mut self, pts: &[[f32; 3]], aspect: f32, wx: f32, y0: f32, y1: f32, min_dist: f32) {
        self.fit_points_with(pts, aspect, wx, y0, y1, min_dist, true)
    }

    /// [`Camera::fit_points_min`]; `reduce`: on the points that can be extreme only
    /// ([`hull_candidates`]: the same camera, a test compares).
    #[allow(clippy::too_many_arguments)]
    fn fit_points_with(&mut self, pts: &[[f32; 3]], aspect: f32, wx: f32, y0: f32, y1: f32, min_dist: f32, reduce: bool) {
        if pts.is_empty() {
            return;
        }
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for p in pts {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        // the search below projects every point ~180 times: only the ones that can be extreme
        // (the same camera, exactly; a 997-block lock's card spent 280 ms here)
        let shell = if reduce { hull_candidates(pts) } else { pts.to_vec() };
        let pts: &[[f32; 3]] = &shell;
        self.target = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let tan_v = (self.vfov.to_radians() / 2.0).tan();
        let tan_h = tan_v * aspect;
        let (f, right, up) = self.basis();
        let d = self.dir();
        let (hw, hh, yc) = (wx.max(0.05), ((y1 - y0) / 2.0).max(0.05), (y0 + y1) / 2.0);
        // the projected box (in view coordinates) of the points from `dist` behind `target`
        let bbox = |target: [f32; 3], dist: f32| -> Option<(f32, f32, f32, f32)> {
            let eye = [target[0] + d[0] * dist, target[1] + d[1] * dist, target[2] + d[2] * dist];
            let (mut x0, mut x1, mut ya, mut yb) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for p in pts {
                let v = sub(*p, eye);
                let z = dot(v, f);
                if z <= 0.1 {
                    return None;
                }
                let (x, y) = (dot(v, right) / z / tan_h, dot(v, up) / z / tan_v);
                x0 = x0.min(x);
                x1 = x1.max(x);
                ya = ya.min(y);
                yb = yb.max(y);
            }
            Some((x0, x1, ya, yb))
        };
        for _ in 0..3 {
            // the distance at which the projected box's halves fit the window's
            let fits = |dist: f32| bbox(self.target, dist).is_some_and(|(x0, x1, ya, yb)| (x1 - x0) / 2.0 <= hw && (yb - ya) / 2.0 <= hh);
            let (mut a, mut b) = (0.5f32, 4000.0f32);
            for _ in 0..60 {
                let mid = (a + b) / 2.0;
                if fits(mid) {
                    b = mid
                } else {
                    a = mid
                }
            }
            self.dist = b.max(min_dist);
            // centre the projected box in the window: move the target across the view
            let Some((x0, x1, ya, yb)) = bbox(self.target, self.dist) else { break };
            let (cx, cy) = ((x0 + x1) / 2.0, (ya + yb) / 2.0 - yc);
            let (sx, sy) = (cx * tan_h * self.dist, cy * tan_v * self.dist);
            for k in 0..3 {
                self.target[k] += right[k] * sx + up[k] * sy;
            }
        }
    }

    /// Projection x view (wgpu clip space: depth 0..1).
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let eye = self.eye();
        let (f, r, u) = self.basis();
        // view: rows r, u, -f
        let view: Mat4 = [
            [r[0], u[0], -f[0], 0.0],
            [r[1], u[1], -f[1], 0.0],
            [r[2], u[2], -f[2], 0.0],
            [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
        ];
        let near = (self.dist * 0.02).clamp(0.02, 1.0);
        let far = self.dist * 8.0 + 200.0;
        let t = 1.0 / (self.vfov.to_radians() / 2.0).tan();
        let proj: Mat4 = [[t / aspect, 0.0, 0.0, 0.0], [0.0, t, 0.0, 0.0], [0.0, 0.0, far / (near - far), -1.0], [0.0, 0.0, near * far / (near - far), 0.0]];
        mul(&proj, &view)
    }

    /// The ray through a point of the view (x, y in -1..1, y up): origin, unit direction.
    pub fn ray(&self, x: f32, y: f32, aspect: f32) -> ([f32; 3], [f32; 3]) {
        let (f, r, u) = self.basis();
        let t = (self.vfov.to_radians() / 2.0).tan();
        let d = [f[0] + r[0] * x * t * aspect + u[0] * y * t, f[1] + r[1] * x * t * aspect + u[1] * y * t, f[2] + r[2] * x * t * aspect + u[2] * y * t];
        (self.eye(), norm(d))
    }

    /// Turn around the target.
    pub fn orbit(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(-1.5, 1.5);
    }

    /// Keep the eye above `floor` (the ground's top): the target no lower than it, the pitch no
    /// lower than what holds the eye a little over it.
    pub fn keep_above(&mut self, floor: f32) {
        let margin = 0.05;
        if self.target[1] < floor {
            self.target[1] = floor;
        }
        let h = self.target[1] - (floor + margin);
        let min_pitch = (-h / self.dist.max(0.01)).clamp(-1.0, 1.0).asin().max(-1.5);
        if self.pitch < min_pitch {
            self.pitch = min_pitch;
        }
    }

    /// Move the target in the view plane (pan), by fractions of the view height.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (_, r, u) = self.basis();
        let s = self.dist * (self.vfov.to_radians() / 2.0).tan() * 2.0;
        for k in 0..3 {
            self.target[k] += (-r[k] * dx + u[k] * dy) * s;
        }
    }
}

/// Column-major 4x4 product a * b.
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0f32; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            out[c][r] = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_box_projects_inside_the_view() {
        let cam = Camera::frame([0.0, 0.0, 0.0], [3.0, 1.0, 1.0], CAM_DIR, VFOV, 16.0 / 9.0, 0.85);
        let m = cam.view_proj(16.0 / 9.0);
        for c in [[0.0, 0.0, 0.0], [3.0, 1.0, 1.0], [3.0, 0.0, 0.0], [0.0, 1.0, 1.0]] {
            let v = [c[0], c[1], c[2], 1.0];
            let clip: Vec<f32> = (0..4).map(|r| (0..4).map(|k| m[k][r] * v[k]).sum()).collect();
            let (x, y, z) = (clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]);
            assert!(x.abs() <= 0.86 && y.abs() <= 0.86 && (0.0..1.0).contains(&z), "{c:?} -> {x} {y} {z}");
        }
    }

    /// The sorted candidates are the maps' (`hull_candidates_sorted` against
    /// `hull_candidates_hashed`): a farm's cell corners grown and moved off the whole numbers (the
    /// camera's points: 1.4 apart, at a fraction), scattered floats with repeats, zeros of both
    /// signs on one line; and asked twice, the same answer (the last big one kept).
    #[test]
    fn hull_candidates_sorted_are_the_hashed() {
        let mut seed = 0x51f1_5e7a_1b2c_3d4eu64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for fill in [3u64, 30, 80, 100] {
            let mut pts: Vec<[f32; 3]> = Vec::new();
            for x in -10..20 {
                for y in -4..8 {
                    for z in -6..15 {
                        if rnd() % 100 < fill {
                            for k in 0..8 {
                                let c = [(x + (k & 1)) as f32, (y + ((k >> 1) & 1)) as f32, (z + ((k >> 2) & 1)) as f32];
                                pts.push([c[0] * 1.4 + 0.8296325 - 0.2, c[1], c[2] * 1.4 - 0.41903 - 0.2]);
                            }
                        }
                    }
                }
            }
            assert_eq!(hull_candidates_sorted(&pts), hull_candidates_hashed(&pts), "fill {fill}: {} points", pts.len());
        }
        let mut floats: Vec<[f32; 3]> = (0..3000).map(|_| [(rnd() % 40) as f32 * 0.25 - 5.0, (rnd() % 9) as f32 * 0.5, (rnd() % 30) as f32 * 0.75]).collect();
        floats.extend_from_slice(&floats.clone()[..500]);
        floats.extend([[0.0, 1.0, 2.0], [-0.0, 1.0, 2.0], [3.0, 1.0, 2.0], [-0.0, 7.5, 0.0], [0.0, 7.5, 0.0]]);
        assert_eq!(hull_candidates_sorted(&floats), hull_candidates_hashed(&floats));
        let mut big: Vec<[f32; 3]> = Vec::new();
        for x in 0..60 {
            for y in 0..10 {
                for z in 0..30 {
                    for k in 0..8 {
                        big.push([(x + (k & 1)) as f32 * 1.4 + 0.3, (y + ((k >> 1) & 1)) as f32, (z + ((k >> 2) & 1)) as f32 * 1.4 + 0.7]);
                    }
                }
            }
        }
        let first = hull_candidates(&big);
        assert_eq!(first, hull_candidates_hashed(&big));
        assert_eq!(hull_candidates(&big), first);
        big[0][1] -= 1.0;
        assert_eq!(hull_candidates(&big), hull_candidates_hashed(&big), "a point moved: answered anew");
    }

    /// The dense grids' candidates are the maps' (`hull_candidates_dense` against
    /// `hull_candidates_hashed`): whole-number cell corners with negative coordinates and repeats,
    /// sparse to dense; and points it must leave to the maps (a fraction, a negative zero).
    #[test]
    fn hull_candidates_dense_are_the_hashed() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for fill in [2u64, 15, 50, 90, 100] {
            let mut pts: Vec<[f32; 3]> = Vec::new();
            for x in -12..18 {
                for y in -3..9 {
                    for z in -7..14 {
                        if rnd() % 100 < fill {
                            for k in 0..8 {
                                pts.push([(x + (k & 1)) as f32, (y + ((k >> 1) & 1)) as f32, (z + ((k >> 2) & 1)) as f32]);
                            }
                        }
                    }
                }
            }
            let dense = hull_candidates_dense(&pts).expect("whole numbers in a small box");
            assert_eq!(dense, hull_candidates_hashed(&pts), "fill {fill}: {} points", pts.len());
            assert_eq!(hull_candidates(&pts), dense);
        }
        let mut odd: Vec<[f32; 3]> = (0..100).map(|i| [i as f32, (i % 7) as f32, (i % 5) as f32]).collect();
        odd.push([0.5, 1.0, 1.0]);
        assert!(hull_candidates_dense(&odd).is_none());
        odd.pop();
        odd.push([-0.0, 1.0, 1.0]);
        assert!(hull_candidates_dense(&odd).is_none());
        assert_eq!(hull_candidates(&odd), hull_candidates_hashed(&odd));
    }

    /// The camera fitted to [`hull_candidates`] is the camera fitted to every point, bit for bit:
    /// sparse and dense cell builds (8 corners a cell, shared corners repeated), several
    /// directions, windows and aspects.
    #[test]
    fn hull_candidates_fit_the_same_camera() {
        // a deterministic scatter of cells in a 30x8x20 box, from sparse to dense
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for fill in [3u64, 20, 60, 100] {
            let mut pts: Vec<[f32; 3]> = Vec::new();
            for x in 0..30 {
                for y in 0..8 {
                    for z in 0..20 {
                        if rnd() % 100 < fill {
                            for k in 0..8 {
                                pts.push([(x + (k & 1)) as f32, (y + ((k >> 1) & 1)) as f32, (z + ((k >> 2) & 1)) as f32]);
                            }
                        }
                    }
                }
            }
            let shell = hull_candidates(&pts);
            assert!(shell.len() * 4 < pts.len(), "fill {fill}: {} of {} points kept", shell.len(), pts.len());
            for dir in [CAM_DIR, [0.55, 0.6, -1.0], [1.0, 0.2, 0.1], [-0.3, 1.0, 0.4]] {
                for (aspect, window) in [(16.0 / 9.0, (0.9, -0.85, 0.85)), (286.0 / 215.0, (0.8, -0.7, 0.9)), (0.46, (0.95, -0.6, 0.95))] {
                    let base = Camera::frame([0.0; 3], [30.0, 8.0, 20.0], dir, VFOV, aspect, 0.85);
                    let (mut a, mut b) = (base, base);
                    a.fit_points_with(&pts, aspect, window.0, window.1, window.2, 0.0, true);
                    b.fit_points_with(&pts, aspect, window.0, window.1, window.2, 0.0, false);
                    assert_eq!((a.dist.to_bits(), a.target.map(f32::to_bits)), (b.dist.to_bits(), b.target.map(f32::to_bits)), "fill {fill}, dir {dir:?}, aspect {aspect}");
                }
            }
        }
    }
}
