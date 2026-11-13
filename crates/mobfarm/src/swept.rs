//! (9fc3155f, ironspec Nov 12 19:49 (2)) The swept-box scan: every rail cell of a design (every
//! rider path: a rider rides a cart on a rail) where the cart's box or its rider's box, swept along
//! the rail, crosses the module column's faces, with the cells outside it the boxes reach.
//!
//! The model (vanilla `OldMinecartBehavior.moveAlongTrack`): each tick the cart's centre is snapped
//! onto its rail's chord (the segment from one exit's edge midpoint to the other's, rising one on a
//! slope), then moved by its speed (at most 0.4 b/gt on rails) along the chord's own direction, so
//! within a tick the centre runs up to one step past the chord's end before the next rail snaps it:
//! a curve's last tick carries the cart diagonally out of its cell (day-1's napper curve (9,-2,4),
//! measured to x 10.16 on the ride). Both ends are swept (a cart may run either way). The cart's
//! box: 0.98 wide (x, z) and 0.7 tall, its floor 0.0625 over the rail surface; the rider's (a
//! villager seated in it): 0.6 wide and 1.95 tall, its feet taken at the cart's floor (a passenger
//! moves with its cart and never collides on its own: a block in a cell only the rider's box
//! reaches does not slow the ride, but the rider's box is what meets entities and boxes there).
use redsim::pos::BlockPos;
use std::collections::BTreeSet;

pub const CART_HALF: f64 = 0.49;
pub const CART_H: f64 = 0.7;
pub const CART_FLOOR: f64 = 0.0625;
pub const RIDER_HALF: f64 = 0.3;
pub const RIDER_H: f64 = 1.95;

/// The swept chord of a rail cell: its two ends (the centre's positions, the rail surface's y) with
/// one `step` past each along the chord's direction (the exits: `fleecheck::rail_exits`).
pub fn swept_chord(c: BlockPos, state: &str, step: f64) -> Option<([f64; 3], [f64; 3])> {
    let e = crate::fleecheck::rail_exits(state);
    if e.len() != 2 {
        return None;
    }
    let end = |(dx, dy, dz): (i32, i32, i32)| [c.x as f64 + 0.5 + dx as f64 * 0.5, c.y as f64 + dy as f64, c.z as f64 + 0.5 + dz as f64 * 0.5];
    let (p0, p1) = (end(e[0]), end(e[1]));
    let h = ((p1[0] - p0[0]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt();
    let u = [(p1[0] - p0[0]) / h, (p1[1] - p0[1]) / h, (p1[2] - p0[2]) / h];
    Some(([p0[0] - u[0] * step, p0[1] - u[1] * step, p0[2] - u[2] * step], [p1[0] + u[0] * step, p1[1] + u[1] * step, p1[2] + u[2] * step]))
}

/// One crossing: the rail cell, its shape, which box, the face, how far past it, and the outside
/// cells the box reaches there.
pub struct Cross {
    pub cell: BlockPos,
    pub shape: String,
    pub rider: bool,
    pub face: String,
    pub extent: f64,
    pub outside: BTreeSet<(i32, i32, i32)>,
}

/// The column: x0..=x1 and z0..=z1 (cells), and optionally y0..=y1; its faces sit at x0 and x1 + 1
/// (likewise z, y). Each rail cell's chord is sampled every 1/32 of its length: at each sample the
/// cart's and the rider's boxes; a box past a face adds the outside cells it overlaps.
pub fn scan(rails: &[(BlockPos, String)], col: (i32, i32, i32, i32), y: Option<(i32, i32)>, step: f64) -> (usize, Vec<Cross>) {
    let (x0, z0, x1, z1) = col;
    let mut out = Vec::new();
    let mut n = 0usize;
    for (c, state) in rails {
        let Some((q0, q1)) = swept_chord(*c, state, step) else { continue };
        n += 1;
        let shape = state.split("shape=").nth(1).and_then(|s| s.split([',', ']']).next()).unwrap_or("").to_string();
        for rider in [false, true] {
            let (half, lo, hi) = if rider { (RIDER_HALF, CART_FLOOR, CART_FLOOR + RIDER_H) } else { (CART_HALF, CART_FLOOR, CART_FLOOR + CART_H) };
            // (each face crossed: its name, whether its coordinate is the box's least (west, north,
            // bottom) or greatest, the farthest coordinate reached, the outside cells)
            let mut faces: Vec<(String, bool, f64, BTreeSet<(i32, i32, i32)>)> = Vec::new();
            for k in 0..=32 {
                let t = k as f64 / 32.0;
                let p = [q0[0] + (q1[0] - q0[0]) * t, q0[1] + (q1[1] - q0[1]) * t, q0[2] + (q1[2] - q0[2]) * t];
                let bx = [p[0] - half, p[0] + half];
                let bz = [p[2] - half, p[2] + half];
                let by = [p[1] + lo, p[1] + hi];
                let cells = |xr: (f64, f64), yr: (f64, f64), zr: (f64, f64)| -> Vec<(i32, i32, i32)> {
                    let r = |a: f64, b: f64| (a.floor() as i32)..=((b - 1e-9).floor() as i32);
                    r(xr.0, xr.1).flat_map(|x| r(yr.0, yr.1).flat_map(move |y| r(zr.0, zr.1).map(move |z| (x, y, z)))).collect()
                };
                let mut add = |name: String, least: bool, at: f64, cs: Vec<(i32, i32, i32)>| {
                    if let Some(f) = faces.iter_mut().find(|f| f.0 == name) {
                        f.2 = if least { f.2.min(at) } else { f.2.max(at) };
                        f.3.extend(cs);
                    } else {
                        faces.push((name, least, at, cs.into_iter().collect()));
                    }
                };
                if bx[0] < x0 as f64 {
                    add(format!("x {x0} (west)"), true, bx[0], cells((bx[0], x0 as f64), (by[0], by[1]), (bz[0], bz[1])));
                }
                if bx[1] > (x1 + 1) as f64 {
                    add(format!("x {} (east)", x1 + 1), false, bx[1], cells(((x1 + 1) as f64, bx[1]), (by[0], by[1]), (bz[0], bz[1])));
                }
                if bz[0] < z0 as f64 {
                    add(format!("z {z0} (north)"), true, bz[0], cells((bx[0], bx[1]), (by[0], by[1]), (bz[0], z0 as f64)));
                }
                if bz[1] > (z1 + 1) as f64 {
                    add(format!("z {} (south)", z1 + 1), false, bz[1], cells((bx[0], bx[1]), (by[0], by[1]), ((z1 + 1) as f64, bz[1])));
                }
                if let Some((y0, y1)) = y {
                    if by[0] < y0 as f64 {
                        add(format!("y {y0} (bottom)"), true, by[0], cells((bx[0], bx[1]), (by[0], y0 as f64), (bz[0], bz[1])));
                    }
                    if by[1] > (y1 + 1) as f64 {
                        add(format!("y {} (top)", y1 + 1), false, by[1], cells((bx[0], bx[1]), ((y1 + 1) as f64, by[1]), (bz[0], bz[1])));
                    }
                }
            }
            for (face, _, extent, outside) in faces {
                out.push(Cross { cell: *c, shape: shape.clone(), rider, face, extent, outside });
            }
        }
    }
    (n, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// day-1's napper curve (9,-2,4) south_west and slope (9,-2,5): the curve's last tick toward the
    /// slope swings the cart's box past x 10 (measured 10.16 on the ride; the chord model with a
    /// 0.4 step bounds it at 10.27), the slope alone stays inside (9.99), and a 0 step (no swing)
    /// keeps the curve inside too.
    #[test]
    fn napper_curve_swing() {
        let rails = vec![(BlockPos::new(9, -2, 4), "minecraft:rail[shape=south_west,waterlogged=false]".to_string()), (BlockPos::new(9, -2, 5), "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]".to_string())];
        let (n, hits) = scan(&rails, (-6, -6, 9, 9), None, 0.4);
        assert_eq!(n, 2);
        let cart: Vec<&Cross> = hits.iter().filter(|h| !h.rider).collect();
        assert_eq!(cart.len(), 1, "only the curve's cart box crosses");
        let h = cart[0];
        assert_eq!(h.cell, BlockPos::new(9, -2, 4));
        assert!(h.face.starts_with("x 10") && h.extent > 10.16 && h.extent < 10.3, "{} {}", h.face, h.extent);
        assert!(h.outside.contains(&(10, -2, 5)) && h.outside.iter().all(|c| c.0 == 10 && c.1 == -2), "{:?}", h.outside);
        let (_, none) = scan(&rails, (-6, -6, 9, 9), None, 0.0);
        assert!(none.iter().all(|h| h.rider || h.extent <= 10.0), "no step, no swing past the face");
    }
}
