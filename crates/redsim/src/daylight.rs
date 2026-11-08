//! Daylight detectors (clk-misc), read in the 26.3 source: `DaylightDetectorBlock` (its block
//! entity's ticker updates the power when the game time is a multiple of 20: the sky light at the
//! detector minus the sky darkening, inverted, or scaled by the sun's angle), the day timeline's
//! `gameplay/sky_light_level` and `visual/sun_angle` tracks (`data/minecraft/timeline/day.json`,
//! `KeyframeTrackSampler`, `EasingType.CubicBezier`) at the judge's frozen day time, and the sky
//! light itself: the light engine's fixed point (`ChunkSkyLightSources`: a column is a source down
//! to its first occluded edge; `SkyLightEngine`: 15 at the sources, minus `max(1, dampening)` per
//! step, never through occluding faces).
//!
//! Exact while the light engine has caught up: a block change within light range in the reading
//! tick or the one before (the light thread may not have run) is reported, not guessed. Blocks
//! whose light behavior is not classified here make a reading an error too.

use crate::block::{Block, Simple};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// The judge's day time: its sandboxes never advance time (`GameRules.ADVANCE_TIME` off) and a
/// new world starts at 0 (sunrise).
pub const DAY_TIME: i64 = 0;

// ------------------------------------------------------------------------- the day timeline

/// `EasingType.CubicBezier.apply`.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let curve = |v1: f32, v2: f32| (3.0f32 * v1 - 3.0f32 * v2 + 1.0f32, -6.0f32 * v1 + 3.0f32 * v2, 3.0f32 * v1);
    let (xa, xb, xc) = curve(x1, x2);
    let (ya, yb, yc) = curve(y1, y2);
    let sample = |a: f32, b: f32, c: f32, t: f32| ((a * t + b) * t + c) * t;
    let gradient = |a: f32, b: f32, c: f32, t: f32| (3.0f32 * a * t + 2.0f32 * b) * t + c;
    // solveT: 4 Newton-Raphson steps, then bisection
    let mut t = x;
    let mut solved = None;
    for _ in 0..4 {
        let error = sample(xa, xb, xc, t) - x;
        if error.abs() < 1.0e-5f32 {
            solved = Some(t);
            break;
        }
        let g = gradient(xa, xb, xc, t);
        if g < 1.0e-5f32 {
            break;
        }
        t -= (error / g).clamp(-0.25f32, 0.25f32);
    }
    let t = match solved {
        Some(t) => t,
        None => {
            let (mut t0, mut t1) = (0.0f32, 1.0f32);
            let mut t = t;
            loop {
                if t0 >= t1 {
                    break t;
                }
                let error = sample(xa, xb, xc, t) - x;
                if error.abs() < 1.0e-5f32 {
                    break t;
                }
                if error < 0.0 {
                    t0 = t;
                } else {
                    t1 = t;
                }
                t = (t1 + t0) / 2.0f32;
            }
        }
    };
    sample(ya, yb, yc, t)
}

/// `KeyframeTrackSampler.sample` of a looping track (period 24000 gt): keyframes (ticks, value).
fn sample(keyframes: &[(i32, f32)], ease: impl Fn(f32) -> f32, ticks: i64) -> f32 {
    let period = 24000i64;
    let first = keyframes[0];
    let last = keyframes[keyframes.len() - 1];
    let mut segments = vec![(last.1, last.0 - period as i32, first.1, first.0)];
    for w in keyframes.windows(2) {
        segments.push((w[0].1, w[0].0, w[1].1, w[1].0));
    }
    segments.push((last.1, last.0, first.1, first.0 + period as i32));
    let t = ticks.rem_euclid(period);
    let seg = segments.iter().find(|s| t < s.3 as i64).copied().unwrap_or(*segments.last().unwrap());
    let (from_v, from_t, to_v, to_t) = seg;
    if t <= from_t as i64 {
        return from_v;
    }
    if t >= to_t as i64 {
        return to_v;
    }
    let alpha = (t - from_t as i64) as f32 / (to_t - from_t) as f32;
    // LerpFunction.ofFloat = Mth.lerp
    from_v + ease(alpha) * (to_v - from_v)
}

/// The sun's angle in degrees (`EnvironmentAttributes.SUN_ANGLE` from the day timeline).
pub fn sun_angle(day: i64) -> f32 {
    sample(&[(6000, 360.0), (6000, 0.0)], |a| cubic_bezier(0.362, 0.241, 0.638, 0.759, a), day)
}

/// `Level.getSkyDarken`: `(int)(15 - SKY_LIGHT_LEVEL)`, the level 15 times the timeline's factor
/// (clear weather: `World::sky_darken` adds rain and thunder, `weather/mod.rs`).
pub fn sky_darken(day: i64) -> i32 {
    let level = sky_light_level(day).clamp(0.0, 15.0);
    (15.0f32 - level) as i32
}

/// The day timeline's `gameplay/sky_light_level` (`FloatModifier.MULTIPLY` of the default 15 by
/// the track's factor), before the weather layer and the clamp (mobinfra).
pub fn sky_light_level(day: i64) -> f32 {
    let factor = sample(&[(133, 1.0), (11867, 1.0), (13670, 0.26666668), (22330, 0.26666668)], |a| a, day);
    15.0f32 * factor
}

// ------------------------------------------------------------------------------ sky light

/// A face's occluding shape on a 16x16 grid: rows (v) of bits (u). For the two faces of one
/// boundary: X faces (u = z, v = y), Y faces (u = x, v = z), Z faces (u = x, v = y).
type Face = [u16; 16];
const EMPTY: Face = [0; 16];
const FULL: Face = [0xffff; 16];

/// What the light engine needs of a state: `getLightDampening` and, when it occludes by shape
/// (`canOcclude && useShapeForLightOcclusion`), its faces (`getFaceOcclusionShape`).
#[derive(Clone, Copy)]
struct Props {
    dampening: i32,
    faces: Option<[Face; 6]>,
}

fn dir_index(d: Dir) -> usize {
    match d {
        Dir::Down => 0,
        Dir::Up => 1,
        Dir::North => 2,
        Dir::South => 3,
        Dir::West => 4,
        Dir::East => 5,
    }
}

/// The light behavior of the states redsim classifies; others are an error.
fn props(b: &Block) -> Result<Props, String> {
    let opaque = Props { dampening: 15, faces: None };
    Ok(match *b {
        Block::Air => Props { dampening: 0, faces: None },
        Block::Simple(s) if s.full() && !matches!(s, Simple::Glass | Simple::TintedGlass | Simple::WhiteStainedGlass | Simple::BlackStainedGlass | Simple::SlimeBlock | Simple::HoneyBlock | Simple::WaxedCopperGrate) => opaque,
        // full, opaque (solid render) blocks; a retracted piston is a full block
        Block::Observer { .. } | Block::RedstoneBlock | Block::Lamp { .. } | Block::Note { .. } | Block::Target { .. } | Block::Dispenser { .. } | Block::Crafter { .. } => opaque,
        Block::Piston { extended: false, .. } => opaque,
        // no occlusion, light passes (propagatesSkylightDown: a shape short of the full block and
        // no fluid): crops (clk-misc)
        Block::SugarCane { .. } | Block::Bamboo { .. } | Block::BambooSapling => Props { dampening: 0, faces: None },
        Block::Stem { .. } | Block::AttachedStem { .. } => Props { dampening: 0, faces: None },
        // a cactus (column 14 wide) and its flower: not full, no fluid, light block 0
        Block::Cactus { .. } | Block::CactusFlower => Props { dampening: 0, faces: None },
        // a sign: no collision box, light block 0, propagatesSkylightDown (ironfarm)
        Block::Sign { .. } => Props { dampening: 0, faces: None },
        // beds and trapdoors (noOcclusion: light block 0, no occluding faces) and leaves (light
        // block 1, no occlusion); lava is a fluid (light block 1) (ironfarm)
        Block::Bed { .. } | Block::Trapdoor { .. } => Props { dampening: 0, faces: None },
        Block::Leaves { .. } | Block::Lava { .. } => Props { dampening: 1, faces: None },
        // repeaters (a 2 px DiodeBlock: not a full shape, so light block 0, and no shape used for
        // occlusion), torches and dust (no collision box) (ironfarm's dusk pulse)
        Block::Repeater { .. } | Block::Torch { .. } | Block::WallTorch { .. } | Block::Wire { .. } => Props { dampening: 0, faces: None },
        // glass, stained glass and copper grates, waterlogged too (`TransparentBlock`:
        // propagatesSkylightDown always, not solid render: light dampening 0, no occlusion)
        Block::Simple(Simple::Glass | Simple::WhiteStainedGlass | Simple::BlackStainedGlass | Simple::WaxedCopperGrate) | Block::WaterloggedGrate => Props { dampening: 0, faces: None },
        // a moving block (`MovingPistonBlock`: noOcclusion, an empty shape without its block
        // entity, which the state's light cache is computed without: dampening 0, no occlusion)
        Block::MovingPiston { .. } => Props { dampening: 0, faces: None },
        // a lightning rod (mobinfra): noOcclusion, not a full shape, dry: propagatesSkylightDown
        Block::LightningRod { .. } => Props { dampening: 0, faces: None },
        // fire (mobinfra): no collision, no occlusion: propagatesSkylightDown
        Block::Fire { .. } => Props { dampening: 0, faces: None },
        // placed light sources (terrain lane): the table's light dampening; full opaque cubes
        // (jack o'lanterns, froglights) occlude everything, the others no occluding shape
        Block::JackOLantern { .. } | Block::Froglight { .. } => opaque,
        Block::PlainTorch { .. } | Block::PlainWallTorch { .. } | Block::SoulLantern { .. } | Block::Campfire { .. } | Block::EndRod { .. } | Block::SeaPickle { .. } | Block::Candle { .. } => {
            Props { dampening: crate::behavior::light_row(b).map(|p| p.dampening as i32).unwrap_or(0), faces: None }
        }
        // farmland: column(16, 0, 15), useShapeForLightOcclusion: the bottom face full, the sides
        // 15 px high, the top open
        Block::Farmland { .. } => {
            let mut side = EMPTY;
            for row in side.iter_mut().take(15) {
                *row = 0xffff;
            }
            Props { dampening: 0, faces: Some([FULL, EMPTY, side, side, side, side]) }
        }
        // a fluid inside: light block 1
        Block::Water { .. } | Block::Kelp { .. } | Block::KelpPlant => Props { dampening: 1, faces: None },
        // column(16, 0, 6), useShapeForLightOcclusion: the bottom face full, the sides 6 px high
        Block::DaylightDetector { .. } => {
            let mut side = EMPTY;
            for row in side.iter_mut().take(6) {
                *row = 0xffff;
            }
            Props { dampening: 0, faces: Some([FULL, EMPTY, side, side, side, side]) }
        }
        _ => return Err(format!("the sky light through {b} is not modeled")),
    })
}

/// `Shapes.faceShapeOccludes`: either face is the full block, or their union covers the face.
fn face_occludes(a: &Face, b: &Face) -> bool {
    (0..16).all(|v| a[v] | b[v] == 0xffff)
}

fn face(p: &Props, d: Dir) -> Face {
    p.faces.map(|f| f[dir_index(d)]).unwrap_or(EMPTY)
}

/// `LightEngine.shapeOccludes(from, to, direction)`.
fn shape_occludes(from: &Props, to: &Props, d: Dir) -> bool {
    face_occludes(&face(from, d), &face(to, d.opposite()))
}

/// `ChunkSkyLightSources.isEdgeOccluded(top, bottom)`.
fn edge_occluded(top: &Props, bottom: &Props) -> bool {
    bottom.dampening != 0 || face_occludes(&face(top, Dir::Down), &face(bottom, Dir::Up))
}

/// The sky light at `p` (`getBrightness(LightLayer.SKY, pos)`): the fixed point over the cube of
/// radius 15 around it (outside redsim's box: air under the open sky). A cell under nothing but
/// air up to the top of the box is under the open sky: 15 (`ChunkSkyLightSources`: full sky light
/// down to the first block that is not air), whatever is classified around it (ironfarm: the
/// daylight detector of a mob farm).
pub fn sky_light(w: &World, p: BlockPos) -> Result<i32, String> {
    let top = w.min.y + w.size[1];
    if (p.y + 1..top).all(|y| matches!(w.get(BlockPos::new(p.x, y, p.z)), Block::Air)) && props(&w.get(p)).is_ok_and(|q| q.dampening == 0) {
        return Ok(15);
    }
    sky_light_with(w, p, None, 15)
}

/// Whether `p` is a sky light source (`ChunkSkyLightSources`: down its column from the top of the
/// world to its first occluded edge; above the box: air), which is exactly a sky light of 15
/// (`canSeeSky`): the engine gives 15 to the sources and every step away costs at least 1 (mobinfra:
/// a column scan, not the flood fill).
pub fn sky_source(w: &World, p: BlockPos) -> Result<bool, String> {
    let world_top = w.min.y + w.size[1] - 1;
    let mut top = Props { dampening: 0, faces: None };
    let mut y = world_top;
    while y >= p.y {
        let bottom = props(&w.get(BlockPos::new(p.x, y, p.z)))?;
        if edge_occluded(&top, &bottom) {
            return Ok(false);
        }
        top = bottom;
        y -= 1;
    }
    Ok(true)
}

/// [`sky_light`] bounds when blocks near `p` are not classified (clk-misc, bamboo's growth check):
/// (every unclassified block opaque, every one air). Light only drops as blocks turn opaque, so
/// the game's value lies between them.
pub fn sky_light_bounds(w: &World, p: BlockPos) -> (i32, i32) {
    sky_light_bounds_from(w, p, 0)
}

/// [`sky_light_bounds`] where only whether each bound reaches `need` matters (a crop's light check:
/// `need` 9): computed over the cube of radius `15 - need` around `p` instead of 15 (clk-misc
/// 2026-10-21: a 64-stem melon farm spent 90% of its twin run in the radius-15 flood fill). Exact
/// for that question: the light at `p` is 15 minus the cheapest path's cost from a sky source (each
/// step costs at least 1), so a reading of `need` or more comes from a path of cost at most
/// `15 - need`, which never leaves that cube; the sources are the same (each column is scanned from
/// the world's top). A bound below `need` is a lower bound only.
pub fn sky_light_bounds_from(w: &World, p: BlockPos, need: i32) -> (i32, i32) {
    let r = (15 - need).clamp(0, 15);
    let opaque = Props { dampening: 15, faces: None };
    let clear = Props { dampening: 0, faces: None };
    let lo = sky_light_with(w, p, Some(opaque), r).unwrap_or(0);
    let hi = sky_light_with(w, p, Some(clear), r).unwrap_or(15);
    (lo, hi)
}

/// The unclassified block closest to `p` within the sky light's reach (15), for a model note.
pub fn unclassified_near(w: &World, p: BlockPos) -> Option<(BlockPos, String)> {
    let mut best: Option<(i32, BlockPos, String)> = None;
    for dy in -15..=15i32 {
        for dz in -15..=15i32 {
            for dx in -15..=15i32 {
                let d = dx.abs() + dy.abs() + dz.abs();
                if best.as_ref().is_some_and(|(b, _, _)| *b <= d) {
                    continue;
                }
                let q = p.offset(dx, dy, dz);
                let b = w.get(q);
                if props(&b).is_err() {
                    best = Some((d, q, b.to_string()));
                }
            }
        }
    }
    best.map(|(_, q, s)| (q, s))
}

/// [`sky_light`], unclassified blocks taking `unknown`'s light behavior (or an error), over the
/// cube of radius `r` around `p` (15: every cell that can light it).
fn sky_light_with(w: &World, p: BlockPos, unknown: Option<Props>, r: i32) -> Result<i32, String> {
    let props = |b: &Block| match (props(b), unknown) {
        (Ok(v), _) => Ok(v),
        (Err(_), Some(u)) => Ok(u),
        (Err(e), None) => Err(e),
    };
    let n = 2 * r + 1;
    let lo = p.offset(-r, -r, -r);
    let idx = |x: i32, y: i32, z: i32| ((y * n + z) * n + x) as usize;
    let mut cell_props: Vec<Props> = Vec::with_capacity((n * n * n) as usize);
    for y in 0..n {
        for z in 0..n {
            for x in 0..n {
                cell_props.push(props(&w.get(lo.offset(x, y, z)))?);
            }
        }
    }
    let mut level = vec![0i8; (n * n * n) as usize];
    let mut queue: Vec<Vec<(i32, i32, i32)>> = vec![Vec::new(); 16];
    // sources: down each column to its first occluded edge (above the region and outside the
    // box: air)
    let world_top = w.min.y + w.size[1] - 1;
    for z in 0..n {
        for x in 0..n {
            let mut top = Props { dampening: 0, faces: None };
            let mut lowest = 0;
            let start = (world_top - lo.y).max(n - 1);
            let mut y = start;
            while y >= 0 {
                let bottom = props(&w.get(lo.offset(x, y, z)))?;
                if edge_occluded(&top, &bottom) {
                    lowest = y + 1;
                    break;
                }
                top = bottom;
                y -= 1;
            }
            for y in lowest..n {
                level[idx(x, y, z)] = 15;
                queue[15].push((x, y, z));
            }
        }
    }
    for l in (2..=15).rev() {
        while let Some((x, y, z)) = queue[l].pop() {
            if level[idx(x, y, z)] as usize != l {
                continue;
            }
            for d in [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East] {
                let [dx, dy, dz] = d.step();
                let (qx, qy, qz) = (x + dx, y + dy, z + dz);
                if !(0..n).contains(&qx) || !(0..n).contains(&qy) || !(0..n).contains(&qz) {
                    continue;
                }
                let (from, to) = (&cell_props[idx(x, y, z)], &cell_props[idx(qx, qy, qz)]);
                let new = l as i32 - to.dampening.max(1);
                if new > level[idx(qx, qy, qz)] as i32 && !shape_occludes(from, to, d) {
                    level[idx(qx, qy, qz)] = new as i8;
                    if new > 1 {
                        queue[new as usize].push((qx, qy, qz));
                    }
                }
            }
        }
    }
    Ok(level[idx(r, r, r)] as i32)
}

// ----------------------------------------------------------------------------- the detector

/// Whether a block change can change the sky light (different dampening or occluding faces; an
/// unclassified state counts as a change).
pub fn light_differs(a: &Block, b: &Block) -> bool {
    match (props(a), props(b)) {
        (Ok(x), Ok(y)) => x.dampening != y.dampening || x.faces != y.faces,
        _ => true,
    }
}

/// The detector's block entity appears: its ticker joins the block-entity tickers, and block
/// changes start being watched for the light engine's timing.
pub fn create(w: &mut World, pos: BlockPos) {
    w.watch_light = true;
    crate::redstone::container::add_ticker(w, pos);
}

/// `DaylightDetectorBlock.tickEntity`.
pub fn tick(w: &mut World, pos: BlockPos) {
    if w.time % 20 == 0 {
        update_signal_strength(w, pos);
    }
}

/// Java's `Math.round(float)`.
fn round_f32(x: f32) -> i32 {
    (x as f64 + 0.5).floor() as i32
}

/// `DaylightDetectorBlock.updateSignalStrength`.
fn update_signal_strength(w: &mut World, pos: BlockPos) {
    let Block::DaylightDetector { inverted, power } = w.get(pos) else { return };
    // the light engine may lag a change of this tick or the previous one
    if w.light_changes.iter().any(|&(t, q)| t >= w.time - 1 && (q.x - pos.x).abs() + (q.y - pos.y).abs() + (q.z - pos.z).abs() <= 15) {
        w.errors.push(format!("daylight detector at {pos} reads the sky light right after a block change near it (the light engine's timing is not modeled)"));
        return;
    }
    let sky = match sky_light(w, pos) {
        Ok(l) => l,
        Err(e) => {
            w.errors.push(format!("daylight detector at {pos}: {e}"));
            return;
        }
    };
    // getEffectiveSkyBrightness = sky - getSkyDarken (the day time is the mob layer's when it
    // runs, a mob farm's day passes, else the frozen DAY_TIME every other study's sandbox has;
    // mobinfra: with the level's rain and thunder)
    let day = if w.mob.rng.is_some() { w.day_time() } else { DAY_TIME };
    let mut target = (sky - w.sky_darken()).max(0);
    let mut sun = sun_angle(day) * (std::f64::consts::PI / 180.0) as f32;
    if inverted {
        target = 15 - target;
    } else if target > 0 {
        let offset = if sun < std::f64::consts::PI as f32 { 0.0f32 } else { (std::f64::consts::PI * 2.0) as f32 };
        sun += (offset - sun) * 0.2f32;
        target = round_f32(target as f32 * crate::entity::mth::cos(sun as f64));
    }
    let target = target.clamp(0, 15) as u8;
    if power != target {
        w.set_block_and_update(pos, Block::DaylightDetector { inverted, power: target });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_time_values() {
        // the judge's facts: a normal detector reads 7 under the open sky and under a 1-block
        // cover (sky light 15 / 14), an inverted one 0 / 1: no darkening at day time 0
        assert_eq!(sky_darken(0), 0);
        let sun = sun_angle(0) * (std::f64::consts::PI / 180.0) as f32;
        let sun = sun + ((std::f64::consts::PI * 2.0) as f32 - sun) * 0.2f32;
        let c = crate::entity::mth::cos(sun as f64);
        assert_eq!(round_f32(15.0 * c), 7);
        assert_eq!(round_f32(14.0 * c), 7);
    }

    /// The crops' light check over the radius-6 cube answers "at least 9?" like the radius-15
    /// one (`sky_light_bounds_from`), both bounds, at every cell of random worlds of opaque
    /// blocks, glass, water, farmland and an unclassified block (a moving piston's base).
    #[test]
    fn the_short_light_check_matches_the_full_one() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let kinds = [
            Block::Air,
            Block::Simple(crate::block::Simple::Stone),
            Block::Simple(crate::block::Simple::Glass),
            Block::Water { level: 0 },
            Block::Farmland { moisture: 7 },
            Block::Piston { sticky: false, facing: Dir::Up, extended: true },
        ];
        let mut seen = [0usize; 2];
        for _ in 0..3 {
            let mut w = World::new(BlockPos::new(-8, -8, -8), [17, 17, 17]);
            let mut cells = Vec::new();
            for x in -8..=8 {
                for y in -8..=8 {
                    for z in -8..=8 {
                        let r = next() % 100;
                        let b = if r < 60 { 0 } else if r < 82 { 1 } else { 2 + (next() % 4) as usize };
                        cells.push((BlockPos::new(x, y, z), kinds[b]));
                    }
                }
            }
            w.place_raw(&cells);
            for x in -3..=3 {
                for y in -3..=3 {
                    for z in -3..=3 {
                        let p = BlockPos::new(x, y, z);
                        let (lo, hi) = sky_light_bounds(&w, p);
                        let (lo6, hi6) = sky_light_bounds_from(&w, p, 9);
                        seen[(lo >= 9) as usize] += 1;
                        assert_eq!(lo >= 9, lo6 >= 9, "{p}: low bound {lo} vs {lo6}");
                        assert_eq!(hi >= 9, hi6 >= 9, "{p}: high bound {hi} vs {hi6}");
                        if lo6 >= 9 {
                            assert_eq!(lo, lo6, "{p}");
                        }
                        if hi6 >= 9 {
                            assert_eq!(hi, hi6, "{p}");
                        }
                    }
                }
            }
        }
        assert!(seen[0] > 50 && seen[1] > 50, "both answers tested: {seen:?}");
    }
}
