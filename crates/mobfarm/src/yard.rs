//! The carousel's sleep routine (bench part: designs/briefs/farm-iron-carousel.md, "The parts",
//! 3): the one shared bed past the hold ends, reached by each napper in its own cart through a
//! yard of switched rails, so the napper never leaves its cart except to lie down.
//!
//! The hold end: past each hold slope a second braked slope (on the hold control: the hump) with a
//! flat top and a plain slope down. A returning cart stores up to ~1.4 a tick of speed over the
//! boosters and a single braked slope does not stop it (the old cap did); over two it stops on
//! them and leaves by gravity when they are powered. At its batch's return the napper's hump is
//! powered instead: its cart climbs over and runs on along its extension to its level's chain of
//! junctions. A plain rail with exactly three rail neighbours is a T: it takes a curve, never the
//! straight (`RailState.place`), and toggles with power, unpowered `south_east`, powered
//! `south_west`; with a fourth rail neighbour it does not reshape (`countPotentialConnections ==
//! 3`), so the junctions stand two apart in z. Lanes 0-3 turn west into the east chain (J_b at
//! (b-1, 3k+1, z0+2b): its west arm the shared line from lane 0, its east arm lane b's track, its
//! stem south: unpowered it joins the track, powered it passes the line), lanes 6-4 east into its
//! mirror, the west chain (unpowered it passes the line, powered it joins the track), and a Y
//! joins the two (powered: the east chain). A junction's control is its support: a redstone block
//! (powered) or a target block (unpowered; a signal source's kind, so both switches update the
//! rail: `neighborChanged` reshapes a T only for a signal source's change).
//!
//! The three levels' chains stack in one footprint; two more T's merge their outputs into the bed
//! line at level 1's height. On it, past the free cell (beside the bed's head), an activator drops
//! the napper into the free cell behind the cart (the dismount's only free floor: glass over the
//! bed, the activator's cell and the run after it) and the cart rolls on to a flat park rail
//! against a cobblestone. The napper calms, lies down; the hatch (an iron trapdoor in the cage's
//! east wall, open across the sight line while powered) shuts and the dorm zombie behind it wakes
//! the napper, which stands up in the free cell and stays there (it flees west: the bed holds no
//! one standing). 60 gt after its drop (its boarding cooldown) the park rail is powered: the cart,
//! kicked off the cobblestone (0.02, then the boost of one rail), runs 4 rails empty (0.96 a tick)
//! and meets it under 0.2 a tick (a faster step can jump the pickup's reach into its box: the
//! collision stops the cart without a pickup), takes it and runs the same junctions back, over the
//! hump (powered, braked once past its top) and onto its hold slope. No loop: each trip goes out
//! and back the same way.

use crate::hold::{LaneOpts, BATCHES};
use crate::layouts::Builder;
use anyhow::{bail, ensure, Result};
use redsim::pos::BlockPos;

/// The yard's shape.
#[derive(Clone, Copy, Debug)]
pub struct YardOpts {
    /// The first turn's z (lanes 0 and 6 turn there; the east chain's lane b 2b further, the west
    /// chain's mirrored).
    pub z0: i32,
    /// A booster on every this-many straight rail of the extensions (0: none).
    pub boost_every: i32,
}

impl YardOpts {
    /// The yard of these tracks: the first turn right past the bottom level's hump (its slope, flat
    /// top and plain slope down: `run + 1 ..= run + 3`).
    pub fn of(o: &LaneOpts) -> YardOpts {
        YardOpts { z0: o.run + 4, boost_every: 2 }
    }
}

/// What a rail cell of a path is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum R {
    Plain,
    /// A powered rail on a redstone block.
    Boost,
    /// The bed line's activator rail on its control (glass: unpowered).
    Activator,
    /// The park rail (a flat powered rail on its control, against a cobblestone stop).
    Park,
    /// The hold end's second braked slope (a powered rail rising south, on the hold control),
    /// the hump's near side.
    Hump,
    /// The hump's far side (a plain rail falling south from the hump's top).
    Fall,
    /// A cell placed elsewhere (a junction, the hold slope, a stop): it only shapes its
    /// neighbours.
    Skip,
}

/// Where the yard's controls and parts are.
#[derive(Clone, Debug)]
pub struct Yard {
    /// Level k's junction controls (each junction's support): the east chain's J_1..J_3 (lanes
    /// 1-3, the shared line from lane 0), the west chain's J_5, J_4 (the shared line from lane 6),
    /// then the Y joining the two chains. A control is a redstone block (powered) or a target
    /// block (unpowered); which state joins a lane's track is [`Yard::junction_states`]'s.
    pub junctions: Vec<Vec<BlockPos>>,
    /// The junctions' rails (the same order).
    pub junction_rails: Vec<Vec<BlockPos>>,
    /// The level merges' controls: M1 (powered: level 0 passes, unpowered: level 1), M2
    /// (powered: levels 0-1, unpowered: level 2).
    pub merges: [BlockPos; 2],
    /// The merges' rails.
    pub merge_rails: [BlockPos; 2],
    /// The activator rail and its control (its support: a redstone block or glass).
    pub activator_rail: BlockPos,
    pub activator: BlockPos,
    /// The park rail and its control (its support: a redstone block or glass).
    pub park_rail: BlockPos,
    pub park: BlockPos,
    /// The free stand-up cell (a rail between the activator and the park slope).
    pub free_cell: BlockPos,
    /// The bed (facing north: the head is the north half).
    pub bed_head: BlockPos,
    pub bed_foot: BlockPos,
    /// The hatch: an iron trapdoor in the cage's east wall; open, the sleeper sees the dorm
    /// zombie through it.
    pub hatch: BlockPos,
    /// The dorm zombie's boat.
    pub zombie: [f64; 3],
    /// The highest z any part of the yard reaches.
    pub z_max: i32,
    /// Laid on paired lanes ([`LaneOpts::paired`]): the junctions in [`Yard::junctions`]' paired
    /// order.
    pub paired: bool,
}

impl Yard {
    /// The states (powered?) of a level's junctions, in [`Yard::junctions`]' order, routing lane
    /// `lane`'s track to the level's output. A junction the cart never reaches keeps its state
    /// (`None`).
    ///
    /// At pitch 1: on the east chain (lanes 0-3) a junction joins its own lane's track unpowered
    /// (`south_east`: east arm to the stem) and passes the shared line powered; on the west chain
    /// (lanes 4-6, mirrored) it joins its track powered (`south_west`) and passes the shared line
    /// unpowered; the Y takes the east chain powered. On paired lanes every junction has a west
    /// arm, an east arm and its stem south: powered (`south_west`) it takes the west arm,
    /// unpowered (`south_east`) the east one.
    pub fn junction_states(&self, lane: i32) -> [Option<bool>; 6] {
        if self.paired {
            // J_12, J_34, J_56 (each the pair of lanes on either side of its column), T_0 (lane 0
            // west, J_12 east), T_25 (T_0 west, J_34 east), T_f (T_25 west, J_56 east)
            let (w, e) = (Some(true), Some(false));
            return match lane {
                0 => [None, None, None, w, w, w],
                1 => [w, None, None, e, w, w],
                2 => [e, None, None, e, w, w],
                3 => [None, w, None, None, e, w],
                4 => [None, e, None, None, e, w],
                5 => [None, None, w, None, None, e],
                _ => [None, None, e, None, None, e],
            };
        }
        let east = |c: i32| if lane > 3 { None } else if c == lane { Some(false) } else if c > lane { Some(true) } else { None };
        let west = |c: i32| if lane < 4 { None } else if c == lane { Some(true) } else if c < lane { Some(false) } else { None };
        [east(1), east(2), east(3), west(5), west(4), Some(lane <= 3)]
    }
}

/// Lane b's turn: its z and its step in x into its junction. At pitch 1 the east chain's lanes 0-3
/// turn west 2 apart from z0, the west chain's lanes 6-4 east 2 apart from z0; on paired lanes
/// lanes 1-6 turn at z0 into the junction in the column beside them (odd lanes east, even west),
/// lane 0 east 2 further, into T_0.
fn turn(o: &LaneOpts, y: &YardOpts, lane: i32) -> (i32, i32) {
    if o.paired {
        return match lane {
            0 => (y.z0 + 2, 1),
            l if l % 2 == 1 => (y.z0, 1),
            _ => (y.z0, -1),
        };
    }
    (zj(y, lane), if lane <= 3 { -1 } else { 1 })
}

/// Lane b's turn z at pitch 1.
fn zj(y: &YardOpts, lane: i32) -> i32 {
    if lane <= 3 { y.z0 + 2 * lane } else { y.z0 + 2 * (BATCHES - 1 - lane) }
}

/// The chains' output z (the first cell past the Y's stem at pitch 1; past T_f's stem on paired
/// lanes).
fn zout(o: &LaneOpts, y: &YardOpts) -> i32 {
    y.z0 + if o.paired { 8 } else { 9 }
}

/// The chains' output x (the Y's at pitch 1, T_f's on paired lanes); the merges and the bed line
/// stand this far east of where they stand at pitch 1, less 3.
fn xout(o: &LaneOpts) -> i32 {
    if o.paired { 5 } else { 3 }
}

/// The yard's parts (positions only).
pub fn layout(o: &LaneOpts, y: &YardOpts) -> Yard {
    let zo = zout(o, y);
    let dx = xout(o) - 3;
    let p = |x: i32, yy: i32, z: i32| BlockPos::new(x, yy, z);
    // each level's junctions: at pitch 1 J_1..J_3 at (lane - 1, z0 + 2 lane), J_5 at (6, z0 + 2),
    // J_4 at (5, z0 + 4), the Y at (3, z0 + 8); on paired lanes J_12, J_34, J_56 in the columns
    // at z0, T_0 at (1, z0 + 2), T_25 at (3, z0 + 4), T_f at (5, z0 + 6)
    let at = |k: i32, dy: i32| -> Vec<BlockPos> {
        let h = o.level_gap * k + dy;
        if o.paired {
            let z = y.z0;
            return vec![p(2, h, z), p(5, h, z), p(8, h, z), p(1, h, z + 2), p(3, h, z + 4), p(5, h, z + 6)];
        }
        let mut v: Vec<BlockPos> = (1..=3).map(|l| p(l - 1, h, zj(y, l))).collect();
        v.push(p(6, h, zj(y, 5)));
        v.push(p(5, h, zj(y, 4)));
        v.push(p(3, h, y.z0 + 8));
        v
    };
    Yard {
        junctions: (0..o.levels).map(|k| at(k, 0)).collect(),
        junction_rails: (0..o.levels).map(|k| at(k, 1)).collect(),
        merges: [p(2 + dx, 3, zo + 4), p(4 + dx, 3, zo + 6)],
        merge_rails: [p(2 + dx, 4, zo + 4), p(4 + dx, 4, zo + 6)],
        activator_rail: p(4 + dx, 4, zo + 10),
        activator: p(4 + dx, 3, zo + 10),
        park_rail: p(4 + dx, 4, zo + 14),
        park: p(4 + dx, 3, zo + 14),
        free_cell: p(4 + dx, 4, zo + 9),
        bed_head: p(3 + dx, 4, zo + 9),
        bed_foot: p(3 + dx, 4, zo + 10),
        hatch: p(5 + dx, 5, zo + 9),
        zombie: [(7 + dx) as f64 + 0.5, 5.0, (zo + 9) as f64 + 0.5],
        z_max: zo + 15,
        paired: o.paired,
    }
}

fn dir_of(c: [i32; 3], n: [i32; 3]) -> Result<&'static str> {
    Ok(match (n[0] - c[0], n[2] - c[2]) {
        (0, -1) => "north",
        (0, 1) => "south",
        (1, 0) => "east",
        (-1, 0) => "west",
        d => bail!("rail cells {c:?} and {n:?} are not neighbours ({d:?})"),
    })
}

fn opposite(d: &str) -> &'static str {
    match d {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        _ => "east",
    }
}

/// A rail's shape from its path neighbours: ascending toward a neighbour one higher, else the
/// straight or the curve joining them.
fn shape(prev: Option<[i32; 3]>, c: [i32; 3], next: Option<[i32; 3]>) -> Result<String> {
    let nbs: Vec<[i32; 3]> = [prev, next].into_iter().flatten().collect();
    for n in &nbs {
        if n[1] == c[1] + 1 {
            return Ok(format!("ascending_{}", dir_of(c, *n)?));
        }
    }
    let mut ds: Vec<&str> = nbs.iter().map(|n| dir_of(c, *n)).collect::<Result<_>>()?;
    if ds.len() == 1 {
        ds.push(opposite(ds[0]));
    }
    let order = |d: &&str| ["north", "south", "east", "west"].iter().position(|x| x == d);
    ds.sort_by_key(order);
    Ok(match (ds[0], ds[1]) {
        ("north", "south") => "north_south",
        ("east", "west") => "east_west",
        ("north", "east") => "north_east",
        ("north", "west") => "north_west",
        ("south", "east") => "south_east",
        ("south", "west") => "south_west",
        pair => bail!("no rail shape joins {pair:?} at {c:?}"),
    }
    .to_string())
}

/// Sets a block where nothing stands yet.
fn put(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("the yard's {s} at {c:?} meets {old}");
    }
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

/// Lays a path of rails, each on its support (kept where one stands already: the hold control).
fn lay(b: &mut Builder, cells: &[([i32; 3], R)]) -> Result<()> {
    for i in 0..cells.len() {
        let (c, r) = cells[i];
        if r == R::Skip {
            continue;
        }
        let prev = (i > 0).then(|| cells[i - 1].0);
        let next = cells.get(i + 1).map(|x| x.0);
        let sh = match r {
            R::Fall => "ascending_north".to_string(),
            _ => shape(prev, c, next)?,
        };
        let straight = sh == "north_south" || sh == "east_west" || sh.starts_with("ascending");
        ensure!(matches!(r, R::Plain | R::Fall) || straight, "a powered or activator rail cannot curve ({sh} at {c:?})");
        let (rail, support) = match r {
            R::Plain | R::Fall => (format!("rail[shape={sh},waterlogged=false]"), "glass"),
            R::Boost => (format!("powered_rail[powered=true,shape={sh},waterlogged=false]"), "redstone_block"),
            R::Activator => (format!("activator_rail[powered=false,shape={sh},waterlogged=false]"), "glass"),
            R::Park | R::Hump => (format!("powered_rail[powered=false,shape={sh},waterlogged=false]"), "glass"),
            R::Skip => unreachable!(),
        };
        put(b, c, &rail)?;
        let s = [c[0], c[1] - 1, c[2]];
        if !b.d.blocks.contains_key(&s) {
            b.set(s[0], s[1], s[2], support);
        }
    }
    Ok(())
}

/// One level's chains at pitch 1, at rail height `ye`: the east chain (J_0 a plain curve, then
/// for lanes 1-3 the previous junction's stem (a booster), the junction's west arm, the junction;
/// J_3's stem and its turn east into the Y's west arm) and the west chain, mirrored (J_6 a plain
/// curve, its stem, J_5's east arm, J_5, its stem, J_4's east arm, J_4, its stem on south and its
/// turn west into the Y's east arm).
fn chains_pitch1(b: &mut Builder, ye: i32, y: &YardOpts) -> Result<()> {
    let mut east = vec![([0, ye, zj(y, 0)], R::Skip), ([-1, ye, zj(y, 0)], R::Plain)];
    for lane in 1..=3 {
        east.push(([lane - 2, ye, zj(y, lane) - 1], R::Boost));
        east.push(([lane - 2, ye, zj(y, lane)], R::Plain));
        east.push(([lane - 1, ye, zj(y, lane)], R::Skip));
    }
    east.extend([([2, ye, y.z0 + 7], R::Boost), ([2, ye, y.z0 + 8], R::Plain), ([3, ye, y.z0 + 8], R::Skip)]);
    lay(b, &east)?;
    let z0 = y.z0;
    lay(
        b,
        &[
            ([6, ye, z0], R::Skip),
            ([7, ye, z0], R::Plain),
            ([7, ye, z0 + 1], R::Boost),
            ([7, ye, z0 + 2], R::Plain),
            ([6, ye, z0 + 2], R::Skip),
            ([6, ye, z0 + 3], R::Boost),
            ([6, ye, z0 + 4], R::Plain),
            ([5, ye, z0 + 4], R::Skip),
            ([5, ye, z0 + 5], R::Boost),
            ([5, ye, z0 + 6], R::Plain),
            ([5, ye, z0 + 7], R::Plain),
            ([5, ye, z0 + 8], R::Plain),
            ([4, ye, z0 + 8], R::Plain),
            ([3, ye, z0 + 8], R::Skip),
        ],
    )
}

/// One level's chains on paired lanes, at rail height `ye`: J_12, J_34, J_56 in the columns at x
/// 2, 5, 8 (z0), each joining the lanes on either side of its column; T_0 at (1, z0 + 2) joins
/// lane 0 (west) and J_12's stem (east), T_25 at (3, z0 + 4) T_0's stem (west) and J_34's (east),
/// T_f at (5, z0 + 6) T_25's stem (west) and J_56's (east). Every stem starts with a booster.
fn chains_paired(b: &mut Builder, ye: i32, z: i32) -> Result<()> {
    // J_12's stem, west into T_0's east arm
    lay(b, &[([2, ye, z], R::Skip), ([2, ye, z + 1], R::Boost), ([2, ye, z + 2], R::Plain), ([1, ye, z + 2], R::Skip)])?;
    // J_34's stem, west into T_25's east arm
    lay(b, &[([5, ye, z], R::Skip), ([5, ye, z + 1], R::Boost), ([5, ye, z + 2], R::Plain), ([5, ye, z + 3], R::Boost), ([5, ye, z + 4], R::Plain), ([4, ye, z + 4], R::Plain), ([3, ye, z + 4], R::Skip)])?;
    // J_56's stem, west into T_f's east arm
    lay(
        b,
        &[
            ([8, ye, z], R::Skip),
            ([8, ye, z + 1], R::Boost),
            ([8, ye, z + 2], R::Plain),
            ([8, ye, z + 3], R::Boost),
            ([8, ye, z + 4], R::Plain),
            ([8, ye, z + 5], R::Boost),
            ([8, ye, z + 6], R::Plain),
            ([7, ye, z + 6], R::Plain),
            ([6, ye, z + 6], R::Plain),
            ([5, ye, z + 6], R::Skip),
        ],
    )?;
    // T_0's stem, east into T_25's west arm
    lay(b, &[([1, ye, z + 2], R::Skip), ([1, ye, z + 3], R::Boost), ([1, ye, z + 4], R::Plain), ([2, ye, z + 4], R::Plain), ([3, ye, z + 4], R::Skip)])?;
    // T_25's stem, east into T_f's west arm
    lay(b, &[([3, ye, z + 4], R::Skip), ([3, ye, z + 5], R::Boost), ([3, ye, z + 6], R::Plain), ([4, ye, z + 6], R::Plain), ([5, ye, z + 6], R::Skip)])
}

/// Builds the yard onto the release + hold design (its hold caps left out: each extension's
/// first rail stands on its hold control).
pub fn build(b: &mut Builder, o: &LaneOpts, y: &YardOpts) -> Result<Yard> {
    ensure!(o.levels == 3 && o.level_gap == 3 && (o.paired || o.pitch == 1), "the yard is laid for 3 levels 3 apart, on paired lanes or at pitch 1");
    ensure!(y.z0 >= o.run + 4, "the first turn (z {}) must stand past the bottom level's hump (z {})", y.z0, o.run + 3);
    let yd = layout(o, y);
    let zo = zout(o, y);
    let (xo, dx) = (xout(o), xout(o) - 3);
    for k in 0..o.levels {
        let ye = o.level_gap * k + 1;
        // each lane's way out: the hold slope (shaping only), the hump (a second braked slope on
        // the hold control, so a returning cart brakes over two slopes and stops on them; a flat
        // top, a plain slope down), then the extension straight on to its turn into its junction
        for lane in 0..BATCHES {
            let x = o.x(lane);
            let hz = o.hold_slope_z(lane, k);
            let (zt, step) = turn(o, y, lane);
            let mut cells = vec![([x, ye - 1, hz], R::Skip), ([x, ye, hz + 1], R::Hump), ([x, ye + 1, hz + 2], R::Plain), ([x, ye, hz + 3], R::Fall)];
            let n = zt - (hz + 4);
            ensure!(n >= 0, "lane {lane}'s turn (z {zt}) stands before its level-{k} hump's foot (z {})", hz + 3);
            for i in 0..n {
                // (boosters from the third rail on, none next to the turn)
                let boost = y.boost_every > 0 && i >= 2 && i < n - 1 && (i - 2) % y.boost_every == 0;
                cells.push(([x, ye, hz + 4 + i], if boost { R::Boost } else { R::Plain }));
            }
            cells.push(([x, ye, zt], R::Plain));
            cells.push(([x + step, ye, zt], R::Skip));
            lay(b, &cells)?;
        }
        let mut line = if o.paired {
            chains_paired(b, ye, y.z0)?;
            // T_f's stem
            vec![([xo, ye, y.z0 + 6], R::Skip), ([xo, ye, y.z0 + 7], R::Boost)]
        } else {
            chains_pitch1(b, ye, y)?;
            vec![([xo, ye, y.z0 + 8], R::Skip)]
        };
        // the output's stem on to the merges
        let c = |x: i32, yy: i32, z: i32, r: R| ([x + dx, yy, z], r);
        match k {
            // level 0: west, up three slopes south, east into M1's west arm
            0 => line.extend([
                c(3, 1, zo, R::Plain),
                c(2, 1, zo, R::Plain),
                c(1, 1, zo, R::Plain),
                c(1, 1, zo + 1, R::Plain),
                c(1, 2, zo + 2, R::Plain),
                c(1, 3, zo + 3, R::Plain),
                c(1, 4, zo + 4, R::Plain),
                c(2, 4, zo + 4, R::Skip),
            ]),
            // level 1: straight south, west into M1's east arm
            1 => line.extend([c(3, 4, zo, R::Plain), c(3, 4, zo + 1, R::Boost), c(3, 4, zo + 2, R::Plain), c(3, 4, zo + 3, R::Plain), c(3, 4, zo + 4, R::Plain), c(2, 4, zo + 4, R::Skip)]),
            // level 2: east, down three slopes south, west into M2's east arm
            _ => line.extend([
                c(3, 7, zo, R::Plain),
                c(4, 7, zo, R::Plain),
                c(5, 7, zo, R::Plain),
                c(6, 7, zo, R::Plain),
                c(6, 6, zo + 1, R::Plain),
                c(6, 5, zo + 2, R::Plain),
                c(6, 4, zo + 3, R::Plain),
                c(6, 4, zo + 4, R::Plain),
                c(6, 4, zo + 5, R::Plain),
                c(6, 4, zo + 6, R::Plain),
                c(5, 4, zo + 6, R::Plain),
                c(4, 4, zo + 6, R::Skip),
            ]),
        }
        lay(b, &line)?;
        // the junctions, unpowered (`south_east`: lane b's track to the stem) on target blocks
        for (r, s) in yd.junction_rails[k as usize].iter().zip(&yd.junctions[k as usize]) {
            put(b, [r.x, r.y, r.z], "rail[shape=south_east,waterlogged=false]")?;
            put(b, [s.x, s.y, s.z], "target[power=0]")?;
        }
    }
    // M1's stem into M2's west arm, the bed line: M2's stem, the rail under the walkers' glass,
    // the free cell, the activator, the slow run, the park slope (its top's stop: the control,
    // glass over it)
    lay(b, &[([2 + dx, 4, zo + 4], R::Skip), ([2 + dx, 4, zo + 5], R::Plain), ([2 + dx, 4, zo + 6], R::Plain), ([3 + dx, 4, zo + 6], R::Plain), ([4 + dx, 4, zo + 6], R::Skip)])?;
    let lx = yd.free_cell.x;
    lay(
        b,
        &[
            ([lx, 4, zo + 6], R::Skip),
            ([lx, 4, zo + 7], R::Boost),
            ([lx, 4, zo + 8], R::Plain),
            ([lx, 4, zo + 9], R::Plain),
            ([lx, 4, zo + 10], R::Activator),
            ([lx, 4, zo + 11], R::Plain),
            ([lx, 4, zo + 12], R::Plain),
            ([lx, 4, zo + 13], R::Plain),
            ([lx, 4, zo + 14], R::Park),
            ([lx, 4, zo + 15], R::Skip),
        ],
    )?;
    for (r, s) in yd.merge_rails.iter().zip(&yd.merges) {
        put(b, [r.x, r.y, r.z], "rail[shape=south_east,waterlogged=false]")?;
        put(b, [s.x, s.y, s.z], "target[power=0]")?;
    }
    // the park rail's stop: a cobblestone (a conductor: the stopped cart, powered, is kicked off
    // it northward, slow: `moveAlongTrack`'s 0.02 then the boost on one rail), glass over it
    put(b, [lx, 4, zo + 15], "cobblestone")?;
    put(b, [lx, 5, zo + 15], "glass")?;
    // the bed beside the activator (its foot) and the free cell (its head), on glass
    let (f, h) = (yd.bed_foot, yd.bed_head);
    put(b, [f.x, f.y, f.z], "red_bed[facing=north,occupied=false,part=foot]")?;
    put(b, [h.x, h.y, h.z], "red_bed[facing=north,occupied=false,part=head]")?;
    put(b, [f.x, f.y - 1, f.z], "glass")?;
    put(b, [h.x, h.y - 1, h.z], "glass")?;
    // the cage: glass two high west of the bed, north of its head, south of its foot, east of
    // the line (the hatch over the hatch's redstone block beside the free cell); glass over the
    // rail before the free cell (riders pass under it: glass does not suffocate)
    let (wx, ex) = (h.x - 1, lx + 1);
    let mut walls: Vec<[i32; 3]> = Vec::new();
    for yy in 4..=5 {
        walls.extend([[wx, yy, f.z], [wx, yy, h.z], [h.x, yy, zo + 11], [ex, yy, zo + 8], [ex, yy, f.z], [ex, yy, zo + 11]]);
    }
    // the bed head's front (north of it) open, on glass, joined to the line before the walkers'
    // glass: a walking path from anywhere on the way out ends there, within reach 1 of the head,
    // so a napper that released HOME at its first REST tick in its cart (its CANT_REACH older than
    // 1200 gt) re-takes the bed from the cart at AcquirePoi's next try (no failed path, no retry
    // marker's 40..400 gt back-off); the woken napper in the free cell cannot reach it (the
    // walkers' glass, the bed's glass)
    walls.extend([[h.x, 3, zo + 7], [h.x, 3, zo + 8]]);
    walls.extend([[lx, 5, zo + 8]]);
    // glass over the activator's cell: the napper stands only in the free cell (dropped there,
    // standing up there), so its cart meets it after a fixed run
    // glass over the run from the activator to the park slope too: the released cart, empty, slows
    // there (0.96 a tick) to under 0.2 a tick before it meets the napper, so its step cannot jump
    // the pickup's 0.2 reach into the napper's box (a collision stops it without the pickup), and no
    // napper walks into that run
    walls.extend([[lx, 5, zo + 10], [lx, 5, zo + 11], [lx, 5, zo + 12]]);
    // the east wall a third block high, glass over the hatch too (the shut hatch's plate over the
    // hatch's redstone block is a stair onto the wall's top otherwise)
    walls.extend([[ex, 6, zo + 8], [ex, 6, f.z], [ex, 6, h.z], [ex, 6, zo + 11]]);
    // glass over the bed (y 6): no villager stands on it (the drop and the stand-up both find the
    // line, and the woken napper fleeing the dorm zombie stays on the line for its cart)
    walls.extend([[h.x, 6, f.z], [h.x, 6, h.z]]);
    for c in walls {
        put(b, c, "glass")?;
    }
    // the hatch: an iron trapdoor open (its plate on the cell's west face, across the sleeper's
    // sight line) while the redstone block under it (the wall beside the free cell) powers it;
    // unpowered (glass) it shuts to its bottom half and the line passes over it
    put(b, [yd.hatch.x, yd.hatch.y, yd.hatch.z], "iron_trapdoor[facing=east,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(b, [yd.hatch.x, yd.hatch.y - 1, yd.hatch.z], "redstone_block")?;
    // the dorm zombie in a boat on glass east of the hatch (its eyes over the hatch's plate seen
    // from the bed's head), roofed against the sun
    let zc = [yd.zombie[0].floor() as i32, yd.zombie[1] as i32, yd.zombie[2].floor() as i32];
    put(b, [zc[0], zc[1] - 1, zc[2]], "glass")?;
    put(b, [zc[0], zc[1] + 3, zc[2]], "oak_leaves[distance=1,persistent=true,waterlogged=false]")?;
    b.entity("oak_boat", yd.zombie, Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    Ok(yd)
}

// ---------------------------------------------------------------------------- the nap scheduler

/// Where a visit stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Its cart on the way out (the hold slope powered until it is past).
    Out,
    /// Dropped beside the bed, not yet asleep.
    AtBed,
    /// Asleep (the hatch open).
    Asleep,
    /// Awake, waiting for its cart (the park slope powered).
    Woken,
    /// Back in its cart, on the way home.
    Back,
}

/// One visit.
#[derive(Clone, Debug)]
pub struct Visit {
    pub id: u32,
    /// The napper's own cart.
    pub cart: Option<u32>,
    pub lane: i32,
    pub level: i32,
    pub stage: Stage,
    pub start: i64,
    /// Ticks at each stage's start.
    pub at: Vec<(Stage, i64)>,
    /// When its cart was parked back on its hold slopes.
    pub parked: Option<i64>,
    /// When its returning cart was back on its own extension (past every shared junction).
    pub clear: Option<i64>,
}

pub use crate::naps::Naps;
