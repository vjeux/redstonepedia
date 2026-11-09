//! The iron farm's grammar as parts (vjeux 2026-11-08 11:24: "breakdown the various part of the
//! farms to optimize them individually. The villager cage, the golem spawn area, the kill chamber,
//! the loot collection system"): every part is generated parametrically from the ironfarm lane's
//! generators (crates/mobfarm `day1.rs`, `layouts.rs`), written into one module frame (the
//! villagers' bed heads at y 0, z 0; the zombie north, -z; the pad south, +z, below; the kill under
//! the pad's hole), so any cage, pad, kill and collection compose.
//!
//! - cage: the villagers' 1-cell pods, the zombie in its boat, the hide trapdoor(s) and the calm's
//!   input (U1's plate pod for the bed plate, U1's bed for a bed observer);
//! - pad: the spawn area, an n x n pad around a 4 x 4 dry hole (L2d), its floor and offset;
//! - kill: the pit corner (one lava source at the bottom of the hole's shaft, in sight or out of the
//!   follow cube by its depth) or the fire-safe blade chamber (`chamber_safe`);
//! - collection: the kill's hoppers into the chest.
//!
//! Each part writes either void cells (walls placed: glass and stone) or dug cells (`terrain:
//! cells`: only the cells it digs, `air`, and the blocks it places; natural rock is the wall).

use mobfarm::layouts::Builder;

pub const STONE: &str = "stone";
pub const GLASS: &str = "glass";
pub const AIR: &str = "air";
pub const LAVA: &str = "lava[level=0]";
pub const WATER: &str = "water[level=0]";
pub const GATE_N: &str = "oak_fence_gate[facing=north,in_wall=false,open=true,powered=false]";
pub const GATE_E: &str = "oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]";
pub const CHEST: &str = "chest[facing=north,type=single,waterlogged=false]";

/// How the module is built: in the void (every wall placed) or dug into terrain (only dug cells
/// and placed blocks listed; natural rock is the wall).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Place {
    Void,
    Dug,
}

/// The calm trigger's kind, as the cage provides its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CalmKind {
    /// U1 stands by day on a wooden plate by the hide (farm-iron.md D1): no redstone, day 1.
    Plate,
    /// U1's bed watched by an observer: the calm part (a catalog timing part) drives the hide.
    Observer,
    /// Day 1's dust fork (ironspec, catalog row 189): U1 stands by day on a wooden plate whose
    /// falling edge (U1 lies down at dusk) fires the falling-edge pulse part (torches, dust and
    /// repeaters: no quartz) into M2's hide: a brief hide at dusk, so every scared villager sleeps
    /// and wakes panicking.
    Pulse,
}

/// The villager cage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cage {
    /// Villagers that see the zombie (they panic and summon).
    pub scared: u8,
    /// Villagers that never see it, besides U1 (they sleep on their own and count for the 3).
    pub blind: u8,
    pub calm: CalmKind,
    /// Observer calm: U1 (the observer's bed) west of the row instead of east of it (the ironfarm
    /// lane's measurement: the timer then fits in front of the pods, under their sight lines).
    pub u1_west: bool,
    /// The regular cage (ironspec iteration 17, vjeux 18:26 "please optimize the villager room"):
    /// U1 in the watchers' row at their level, in a pod like theirs (open), out of the zombie's 8
    /// blocks so it never panics; the calm routed to it (never raised for a joint placement).
    pub regular: bool,
}

/// The spawn pad: n x n (even, >= 6) around the 4 x 4 dry hole, its floor `floor` below the beds,
/// moved `dx` on x from centred under the scared villagers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pad {
    pub n: u8,
    pub floor: i8,
    pub dx: i8,
}

/// The kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kill {
    /// The pit corner (D1): the hole's shaft down to golem feet `k`, one water source pushing every
    /// golem into the corner under one lava source held by 3 wall signs, one hopper.
    Pit { k: i8 },
    /// The fire-safe blade chamber (M2's `chamber_safe`) north of the shaft's bottom at feet `k`:
    /// 4 lava sources, 2 hoppers.
    Blade { k: i8 },
    /// A bare shaft down to `k` (open below): an upper module of a shared-kill stack drops its
    /// golems through the modules below into the one kill at the bottom.
    Shaft { k: i8 },
    /// The chute to the side (vjeux 18:02): the hole drops 3 into a water channel through the rock,
    /// `len` cells east at the farm's level (a step down every 7), to a pit corner beyond every
    /// villager's follow range (dug only: the rock is its walls).
    Side { len: u8, feet: i8, one: bool },
}

impl Kill {
    pub fn k(self) -> i32 {
        match self {
            Kill::Pit { k } | Kill::Blade { k } | Kill::Shaft { k } => k as i32,
            // (nominal, for a pad 7 below: the channel 4 under the pad, a step every 7, the pit 6
            // under the channel's end)
            Kill::Side { len, feet, .. } => (if feet == 0 { -11 } else { feet as i32 }) - len as i32 / 7,
        }
    }

    pub fn lava(self) -> u32 {
        match self {
            Kill::Pit { .. } | Kill::Side { .. } => 1,
            Kill::Blade { .. } => 4,
            Kill::Shaft { .. } => 0,
        }
    }
}

/// One module: a cage, its pad, its kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Module {
    pub cage: Cage,
    pub pad: Pad,
    pub kill: Kill,
}

/// What a module exposes to the assembly (module coordinates).
#[derive(Clone, Debug, Default)]
pub struct Ports {
    /// The kill's drop cell: the chest goes here (or the shared collection's input).
    pub output: [i32; 3],
    /// The cell a calm part's output must reach to work the hide (observer calm: its dust then
    /// climbs onto the stone east of the hide), and U1's bed head (the observer watches it).
    pub hide_feed: Option<[i32; 3]>,
    pub u1_bed: Option<[i32; 3]>,
    /// The cells of the scared villagers' feet (the golem searches' centres).
    pub summoners: Vec<[i32; 3]>,
    /// The pad's spawnable floor cells (each golem cell is one above).
    pub pad_cells: Vec<[i32; 3]>,
    /// The pit's cells, golem feet levels (the dwell box of an in-sight kill).
    pub pit: Option<([i32; 3], [i32; 3])>,
    /// The pods' x extent (module coordinates moved like the rest), U1's included.
    pub row: (i32, i32),
}

impl Ports {
    /// The ports moved by `d`.
    pub fn moved(&self, d: [i32; 3]) -> Ports {
        let mv = |p: [i32; 3]| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        Ports {
            output: mv(self.output),
            hide_feed: self.hide_feed.map(mv),
            u1_bed: self.u1_bed.map(mv),
            summoners: self.summoners.iter().map(|p| mv(*p)).collect(),
            pad_cells: self.pad_cells.iter().map(|p| mv(*p)).collect(),
            pit: self.pit.map(|(a, b)| (mv(a), mv(b))),
            row: (self.row.0 + d[0], self.row.1 + d[0]),
        }
    }
}

fn set(b: &mut Builder, at: [i32; 3], x: i32, y: i32, z: i32, s: &str) {
    b.set(at[0] + x, at[1] + y, at[2] + z, s);
}

/// A cell that must be air: dug (listed `air`) in terrain, nothing to write in the void.
fn open(b: &mut Builder, at: [i32; 3], place: Place, x: i32, y: i32, z: i32) {
    if place == Place::Dug {
        set(b, at, x, y, z, AIR);
    }
}

/// A wall cell: glass in the void, natural rock (nothing listed) when dug.
fn wall(b: &mut Builder, at: [i32; 3], place: Place, x: i32, y: i32, z: i32, s: &str) {
    if place == Place::Void {
        set(b, at, x, y, z, s);
    }
}

fn villager(b: &mut Builder, at: [i32; 3], x: f64, y: f64, z: f64) {
    b.entity("villager", [at[0] as f64 + x, at[1] as f64 + y, at[2] as f64 + z], None);
}

/// A villager on the head of its bed (facing north) in a closed 1-cell pod at (x, 0, 0): `sees`,
/// its north face at y + 1 is an open trapdoor (it sees over it, it cannot pass), else its north
/// face is a wall up to its eyes. Void: glass walls, a stone roof capped with glass (day1.rs
/// `pod`); dug: the pod's two air cells over the bed, rock around.
pub(crate) fn pod(b: &mut Builder, at: [i32; 3], place: Place, x: i32, sees: bool) {
    set(b, at, x, 0, 0, "red_bed[facing=north,occupied=false,part=head]");
    set(b, at, x, 0, 1, "red_bed[facing=north,occupied=false,part=foot]");
    if sees {
        set(b, at, x, 1, 0, "oak_trapdoor[facing=south,half=bottom,open=true,powered=false,waterlogged=false]");
    } else {
        open(b, at, place, x, 1, 0);
        wall(b, at, place, x, 1, -1, GLASS);
        wall(b, at, place, x, 2, -1, GLASS);
    }
    open(b, at, place, x, 2, 0);
    if place == Place::Void {
        for z in 0..2 {
            set(b, at, x, -1, z, STONE);
        }
        set(b, at, x, 1, 1, GLASS);
        set(b, at, x, 2, 1, GLASS);
        for w in [x - 1, x + 1] {
            set(b, at, w, 1, 0, GLASS);
            set(b, at, w, 2, 0, GLASS);
        }
        for dx in -1..=1 {
            for z in 0..2 {
                set(b, at, x + dx, 3, z, STONE);
                set(b, at, x + dx, 4, z, GLASS);
            }
        }
    }
    villager(b, at, x as f64 + 0.5, 0.5625, 0.5);
}

/// The scared pods' x positions and the zombie's column (day1.rs: D1 sees from x 2, D4 from 0 and
/// 2, the zombie at x 2; M2 from 0, 2, 4, the zombie at 2; with 4 scared the zombie is centred
/// at x 3).
pub fn scared_xs(c: &Cage) -> (Vec<i32>, i32) {
    match (c.calm, c.scared) {
        (CalmKind::Plate, 1) => (vec![2], 2),
        (CalmKind::Plate, _) => (vec![0, 2], 2),
        (CalmKind::Observer | CalmKind::Pulse, s) => {
            let xs: Vec<i32> = (0..s as i32).map(|i| 2 * i).collect();
            let zx = xs[xs.len() / 2].min(xs[(xs.len() - 1) / 2] + 1);
            (xs.clone(), if xs.len() % 2 == 0 { xs[xs.len() / 2 - 1] + 1 } else { zx })
        }
    }
}

/// The cage: pods, zombie, hide, U1. Returns (hide feed, observer output, summoner cells).
fn cage(b: &mut Builder, at: [i32; 3], place: Place, c: &Cage, with_u1: bool) -> (Option<[i32; 3]>, Option<[i32; 3]>, Vec<[i32; 3]>) {
    let (xs, zx) = scared_xs(c);
    for &x in &xs {
        pod(b, at, place, x, true);
    }
    // the blind villagers west of the row (D1's blind U2 at x 0 when one villager is scared)
    let first = xs[0];
    for i in 0..c.blind as i32 {
        pod(b, at, place, first - 2 - 2 * i, false);
        // the blind pod's sides at bed height closed too: two sleeping villagers' eyes (bed +
        // 0.2) see each other across the open cell between their beds, and 5 villagers in a box
        // that see each other are refused (gossip, not modeled)
        let x = first - 2 - 2 * i;
        for w in [x - 1, x + 1] {
            let q = [at[0] + w, at[1], at[2]];
            if !b.d.blocks.contains_key(&q) {
                wall(b, at, place, w, 0, 0, GLASS);
            }
        }
    }
    // the sight tunnel (dug): the cells every scared line of sight crosses, x from the leftmost
    // scared pod to the zombie's column, y 1..2, z -1..-5
    if place == Place::Dug {
        let (x0, x1) = (xs[0].min(zx), xs[xs.len() - 1].max(zx));
        for z in -5..=-1 {
            for x in x0..=x1 {
                for y in 1..=2 {
                    set(b, at, x, y, z, AIR);
                }
            }
        }
        // the sleeping eyes' rays to the zombie (the day-1 lane's D1U fix: a sleeping villager's
        // eye at bed + 0.7625 rises through y 1 only 1.8 blocks north of its bed): the floor cells
        // in front of each scared pod dug two deep under top slabs (above the sleeping ray, below
        // the standing one; no standing room for a villager or a golem under them), else the
        // natural floor hides the zombie at night and golems come only by day
        for &x in &xs {
            for z in [-1, -2] {
                set(b, at, x, 0, z, AIR);
                set(b, at, x, 1, z, "cobblestone_slab[type=top,waterlogged=false]");
            }
        }
        // a torch on the tunnel's west wall lights the tunnel, the pods and the cells under the
        // slabs (no monster spawns at block light 1 or more); no collision: the rays pass
        set(b, at, x0, 2, -3, "wall_torch[facing=east]");
    }
    // the zombie: a boat on a slab 7 north, a roof over its eyes (void: stone capped with glass;
    // dug: rock), the cell north of its eye box dug (its eye box reaches z -7.04)
    set(b, at, zx, 0, -7, "stone_slab[type=bottom,waterlogged=false]");
    if place == Place::Void {
        set(b, at, zx, 2, -7, STONE);
        set(b, at, zx, 3, -7, GLASS);
    } else {
        set(b, at, zx, 1, -7, AIR);
        set(b, at, zx, 1, -8, AIR);
    }
    b.entity("oak_boat", [at[0] as f64 + zx as f64 + 0.5, at[1] as f64 + 0.5, at[2] as f64 - 6.8], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
    let summoners: Vec<[i32; 3]> = xs.iter().map(|&x| [at[0] + x, at[1], at[2]]).collect();
    match c.calm {
        CalmKind::Plate => {
            // the hide: a top-half trapdoor shut across the standing lines (D1 facing east with one
            // scared villager, D4 facing west with two); U1's plate pod east of it holds it open
            let facing = if c.scared == 1 { "east" } else { "west" };
            set(b, at, zx, 1, -6, &format!("oak_trapdoor[facing={facing},half=top,open=false,powered=false,waterlogged=false]"));
            plate_pod(b, at, place, zx);
            (None, None, summoners)
        }
        CalmKind::Observer | CalmKind::Pulse => {
            // the hide (M2): a closed bottom-half trapdoor the lines pass over; opened (powered
            // by the dust on the stone east of it, pointing into it) it stands across them. Every
            // scared line crosses one cell when the zombie is centred on the row (`hide_cells`)
            let cells = hide_cells(&xs, zx);
            let hx = cells[0];
            set(b, at, hx, 1, -6, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
            set(b, at, hx + 1, 0, -6, STONE);
            set(b, at, hx + 1, 1, -6, "redstone_wire[east=side,north=none,power=0,south=none,west=side]");
            // the feed: the calm's dust ends on the cell east of that stone (and climbs onto it)
            let feed = [at[0] + hx + 2, at[1], at[2] - 6];
            // U1, blind: east of the row, or west of it (past the blind pods); the calm part's
            // observer watches its bed head. The regular cage: U1's pod like the watchers' (open)
            // in their row, far enough along it that the zombie is beyond its 8 blocks (feet to
            // feet: the zombie 7.3 north of the row, so 4 or more columns from the zombie's)
            let u1 = if c.regular && c.calm == CalmKind::Observer {
                if c.u1_west { (xs[0] - 2).min(zx - 4) } else { (xs[xs.len() - 1] + 2).max(zx + 4) }
            } else if c.u1_west {
                xs[0] - 2 - 2 * c.blind as i32
            } else {
                xs[xs.len() - 1] + 2
            };
            if with_u1 {
                if c.calm == CalmKind::Pulse {
                    plate_pod_row(b, at, place, u1, if c.regular { 0 } else { 1 });
                } else {
                    pod(b, at, place, u1, c.regular);
                }
            }
            // (the dust fork's U1 sleeps one level up, its bed head at y 1, unless regular: in the
            // row at the watchers' level, walled, its plate feeding the dust)
            let u1_y = if c.calm == CalmKind::Pulse && !c.regular { 1 } else { 0 };
            (Some(feed), Some([at[0] + u1, at[1] + u1_y, at[2]]), summoners)
        }
    }
}

/// U1's plate pod at the row's end (the dust fork), one level up so the part works at the routes'
/// level: U1's bed (head at y 1, z 0, facing north), a wooden pressure plate in front of the head
/// (y 1, z -1) where U1 stands when awake, on the stone that is the falling-edge part's input
/// (x, 0, -1); the plate's cell enclosed (no sight of the zombie, no way out), its headroom open.
fn plate_pod_row(b: &mut Builder, at: [i32; 3], place: Place, x: i32, y0: i32) {
    // (y0 1: U1 a level up, the routes' level; y0 0: the regular row, U1 at the watchers' level)
    let at = [at[0], at[1] + y0 - 1, at[2]];
    set(b, at, x, 1, 0, "red_bed[facing=north,occupied=false,part=head]");
    set(b, at, x, 1, 1, "red_bed[facing=north,occupied=false,part=foot]");
    set(b, at, x, 0, -1, STONE);
    set(b, at, x, 1, -1, "oak_pressure_plate[powered=false]");
    open(b, at, place, x, 2, -1);
    open(b, at, place, x, 2, 0);
    open(b, at, place, x, 3, 0);
    if place == Place::Void {
        for z in 0..2 {
            set(b, at, x, 0, z, STONE);
        }
        set(b, at, x, 2, 1, GLASS);
        set(b, at, x, 3, 1, GLASS);
        for w in [x - 1, x + 1] {
            for z in -1..=1 {
                for y in 1..=3 {
                    if !b.d.blocks.contains_key(&[at[0] + w, at[1] + y, at[2] + z]) {
                        set(b, at, w, y, z, GLASS);
                    }
                }
            }
        }
        for y in 1..=3 {
            set(b, at, x, y, -2, GLASS);
        }
        set(b, at, x, 3, -1, GLASS);
        for dx in -1..=1 {
            for z in -1..2 {
                set(b, at, x + dx, 4, z, STONE);
            }
        }
    }
    // U1 starts standing on its plate (D1's way), so its first lying down at dusk is a falling
    // edge too
    villager(b, at, x as f64 + 0.5, 1.0, -0.5);
}

/// The hide's columns: every x at which a scared villager's standing line of sight (eye at bed +
/// 0.5625 + 1.62, the zombie's eye at boat 0.5 + 0.1875 - 0.7 + 1.74) crosses the open trapdoor's
/// slab (z -6.0 .. -5.8125).
pub fn hide_cells(xs: &[i32], zx: i32) -> Vec<i32> {
    let (ez, zz) = (0.5, -6.8);
    let mut cells: Vec<i32> = Vec::new();
    for &x in xs {
        let (vx, tx) = (x as f64 + 0.5, zx as f64 + 0.5);
        for z in [-6.0, -5.8125] {
            let t = (ez - z) / (ez - zz);
            let cx = (vx + (tx - vx) * t).floor() as i32;
            if !cells.contains(&cx) {
                cells.push(cx);
            }
        }
    }
    cells.sort();
    let (lo, hi) = (cells[0], cells[cells.len() - 1]);
    (lo..=hi).collect()
}

/// U1's pod east of the hide (day1.rs `plate_pod`): a wooden pressure plate at (zx + 1, 2, -6) on
/// the stone next to the trapdoor, U1's bed head east of the plate (facing west).
fn plate_pod(b: &mut Builder, at: [i32; 3], place: Place, zx: i32) {
    let x = zx + 1;
    set(b, at, x, 1, -6, STONE);
    set(b, at, x, 2, -6, "oak_pressure_plate[powered=false]");
    set(b, at, x + 1, 2, -6, "red_bed[facing=west,occupied=false,part=head]");
    set(b, at, x + 2, 2, -6, "red_bed[facing=west,occupied=false,part=foot]");
    if place == Place::Void {
        for dx in 1..=2 {
            set(b, at, x + dx, 1, -6, STONE);
        }
        for y in 2..=3 {
            for dx in 0..=2 {
                set(b, at, x + dx, y, -7, GLASS);
                set(b, at, x + dx, y, -5, GLASS);
            }
            set(b, at, x - 1, y, -6, GLASS);
            set(b, at, x + 3, y, -6, GLASS);
        }
        for dx in 0..=2 {
            set(b, at, x + dx, 4, -6, STONE);
            set(b, at, x + dx, 5, -6, STONE);
            set(b, at, x + dx, 6, -6, GLASS);
        }
    } else {
        for dx in 0..=2 {
            set(b, at, x + dx, 3, -6, AIR);
        }
    }
    villager(b, at, x as f64 + 0.5, 2.0, -5.5);
}

/// The pad's min x: centred under the scared villagers, moved by `dx`.
pub fn pad_x0(c: &Cage, p: &Pad) -> i32 {
    let (xs, _) = scared_xs(c);
    let centre2 = xs[0] + xs[xs.len() - 1] + 1; // twice the scared row's centre
    // (the quadrant pad, n 9, is 8 wide)
    let n = if p.n == 9 { 8 } else { p.n as i32 };
    (centre2 - n) / 2 + p.dx as i32
}

/// The pad (L2d's dry hole, day1.rs `pit_kill`'s pad): x x0..x0+n-1, z 2..n+1, its floor `fl`,
/// water in its 4 corners at fl + 1, the 4 x 4 hole in its middle (ring: open gates at fl under
/// the water; centre: open gates at fl + 1, dry), 3 cells of headroom; void: glass rims 2 high.
/// Returns the hole's min corner (x, z) and the spawnable floor cells.
fn pad(b: &mut Builder, at: [i32; 3], place: Place, x0: i32, n: i32, fl: i32) -> ([i32; 2], Vec<[i32; 3]>) {
    if n == 9 {
        return pad_quads(b, at, place, x0, fl);
    }
    let (x1, z0, z1) = (x0 + n - 1, 2, n + 1);
    let (hx, hz) = (x0 + n / 2 - 2, z0 + n / 2 - 2);
    let hole = |x: i32, z: i32| (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z);
    let centre = |x: i32, z: i32| (hx + 1..=hx + 2).contains(&x) && (hz + 1..=hz + 2).contains(&z);
    let mut cells = Vec::new();
    for x in x0..=x1 {
        for z in z0..=z1 {
            let corner = (x == x0 || x == x1) && (z == z0 || z == z1);
            if centre(x, z) {
                open(b, at, place, x, fl, z);
                set(b, at, x, fl + 1, z, GATE_N);
            } else if hole(x, z) {
                set(b, at, x, fl, z, GATE_N);
                open(b, at, place, x, fl + 1, z);
            } else {
                set(b, at, x, fl, z, STONE);
                cells.push([at[0] + x, at[1] + fl, at[2] + z]);
                if corner {
                    set(b, at, x, fl + 1, z, WATER);
                } else {
                    open(b, at, place, x, fl + 1, z);
                }
            }
            open(b, at, place, x, fl + 2, z);
            open(b, at, place, x, fl + 3, z);
        }
    }
    if place == Place::Void {
        for y in fl + 1..=fl + 2 {
            for x in (x0 - 1)..=(x1 + 1) {
                set(b, at, x, y, z0 - 1, GLASS);
                set(b, at, x, y, z1 + 1, GLASS);
            }
            for z in z0..=z1 {
                set(b, at, x0 - 1, y, z, GLASS);
                set(b, at, x1 + 1, y, z, GLASS);
            }
        }
    }
    ([hx, hz], cells)
}

/// The top of a kill's opened shaft: the pad's floor, or under the quadrant pad's collector (its
/// dry hole's gates at fl - 4 stay).
fn shaft_top(b: &Builder, at: [i32; 3], h: [i32; 2], fl: i32) -> i32 {
    if b.d.blocks.get(&[at[0] + h[0], at[1] + fl - 4, at[2] + h[1]]).is_some_and(|s| s.contains("fence_gate")) { fl - 4 } else { fl }
}

/// The quadrant pad (ironspec iteration 20's dwell lever, the pad key's `9`): the 8 x 8 floor split
/// into four 4 x 4 quadrants, each a 2 x 2 hole (open gates) ringed by its 12 spawn cells with
/// water in the quadrant's 4 corners flowing in (every spawn cell borders its hole or is one step
/// from it), so a golem falls almost at once; under the pad a 2-high collector over the whole
/// floor, water in its corners flowing to the 4 x 4 shaft in its middle (the 8 x 8's shaft: the
/// kill parts see the same hole). Returns the shaft's min corner and the spawn cells.
fn pad_quads(b: &mut Builder, at: [i32; 3], place: Place, x0: i32, fl: i32) -> ([i32; 2], Vec<[i32; 3]>) {
    let n = 8;
    let (x1, z0, z1) = (x0 + n - 1, 2, n + 1);
    let (hx, hz) = (x0 + n / 2 - 2, z0 + n / 2 - 2);
    let mut cells = Vec::new();
    for x in x0..=x1 {
        for z in z0..=z1 {
            let (qx, qz) = ((x - x0) % 4, (z - z0) % 4);
            let in_hole = (1..=2).contains(&qx) && (1..=2).contains(&qz);
            let q_corner = (qx == 0 || qx == 3) && (qz == 0 || qz == 3);
            if in_hole {
                set(b, at, x, fl, z, GATE_N);
                open(b, at, place, x, fl + 1, z);
            } else {
                set(b, at, x, fl, z, STONE);
                cells.push([at[0] + x, at[1] + fl, at[2] + z]);
                if q_corner {
                    set(b, at, x, fl + 1, z, WATER);
                } else {
                    open(b, at, place, x, fl + 1, z);
                }
            }
            open(b, at, place, x, fl + 2, z);
            open(b, at, place, x, fl + 3, z);
            // the collector under the pad: 3 high (a golem is 2.7 tall), its floor at fl - 4,
            // water in its corners at fl - 3; the shaft's 4 x 4 the pad's own dry hole (its ring
            // open gates at the floor, under the water, its centre dry under open gates at the
            // water's level), so the current pushes every golem onto a gate and it falls through
            let shaft = (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z);
            let centre = (hx + 1..=hx + 2).contains(&x) && (hz + 1..=hz + 2).contains(&z);
            let corner = (x == x0 || x == x1) && (z == z0 || z == z1);
            if centre {
                open(b, at, place, x, fl - 4, z);
                set(b, at, x, fl - 3, z, GATE_N);
            } else if shaft {
                set(b, at, x, fl - 4, z, GATE_N);
                open(b, at, place, x, fl - 3, z);
            } else {
                set(b, at, x, fl - 4, z, STONE);
                if corner {
                    set(b, at, x, fl - 3, z, WATER);
                } else {
                    open(b, at, place, x, fl - 3, z);
                }
            }
            open(b, at, place, x, fl - 2, z);
            open(b, at, place, x, fl - 1, z);
        }
    }
    if place == Place::Void {
        for y in (fl - 3..=fl - 1).chain(fl + 1..=fl + 2) {
            for x in (x0 - 1)..=(x1 + 1) {
                set(b, at, x, y, z0 - 1, GLASS);
                set(b, at, x, y, z1 + 1, GLASS);
            }
            for z in z0..=z1 {
                set(b, at, x0 - 1, y, z, GLASS);
                set(b, at, x1 + 1, y, z, GLASS);
            }
        }
    }
    ([hx, hz], cells)
}

/// The pit corner under the hole (day1.rs `pit_kill`): the hole's 4 x 4 shaft from the pad's floor
/// down to golem feet `k` (void: a stone shell), one water source in its far corner pushing every
/// golem into the near corner, one lava source at head height there held by 3 wall signs (signs
/// do not burn), the hopper under that corner over the output.
fn pit(b: &mut Builder, at: [i32; 3], place: Place, h: [i32; 2], fl: i32, k: i32) -> [i32; 3] {
    pit_turned(b, at, place, h, fl, k, false)
}

/// `pit`, or turned half a turn (`turned`: the lava corner south-east, the water north-west): a
/// side chute's golems arrive moving east and land on the far side, where a water source (no
/// current in its own cell) would pin them; turned, they land by the lava and the water behind
/// them pushes them into it.
fn pit_turned(b: &mut Builder, at: [i32; 3], place: Place, h: [i32; 2], fl: i32, k: i32, turned: bool) -> [i32; 3] {
    let [hx, hz] = h;
    // the cell (x, z) of the unturned pit, turned around the pit's centre when `turned`
    let t = |x: i32, z: i32| if turned { (2 * hx + 3 - x, 2 * hz + 3 - z) } else { (x, z) };
    let face = |f: &str| -> String {
        if !turned {
            return f.to_string();
        }
        match f {
            "south" => "north",
            "north" => "south",
            "east" => "west",
            _ => "east",
        }
        .to_string()
    };
    for y in k..shaft_top(b, at, h, fl) {
        for x in hx..=hx + 3 {
            for z in hz..=hz + 3 {
                open(b, at, place, x, y, z);
            }
        }
    }
    if place == Place::Void {
        for y in (k - 1)..fl {
            for x in (hx - 1)..=(hx + 4) {
                for z in (hz - 1)..=(hz + 4) {
                    let inside = (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z);
                    // (the quadrant pad's collector already holds the cells right under the pad)
                    let collector = y >= fl - 4 && b.d.blocks.contains_key(&[at[0] + x, at[1] + y, at[2] + z]);
                    if (!inside || y == k - 1) && !collector {
                        set(b, at, x, y, z, STONE);
                    }
                }
            }
        }
    }
    let put = |b: &mut Builder, x: i32, y: i32, z: i32, s: &str| {
        let (x, z) = t(x, z);
        set(b, at, x, y, z, s);
    };
    put(b, hx + 3, k, hz + 3, WATER);
    put(b, hx, k + 2, hz, LAVA);
    put(b, hx, k + 1, hz, &format!("oak_wall_sign[facing={},waterlogged=false]", face("south")));
    put(b, hx + 1, k + 2, hz, &format!("oak_wall_sign[facing={},waterlogged=false]", face("south")));
    put(b, hx, k + 2, hz + 1, &format!("oak_wall_sign[facing={},waterlogged=false]", face("east")));
    // the day-1 lane's shield (D1U): `LavaFluid.randomTick` lights fire only in an air cell beside
    // a block lava ignites (the oak signs), or above one; an oak button (not ignited by lava, no
    // collision: golems fall through it; 1 plank, where a stone button needs a smelt) fills the two
    // cells over the side signs
    put(b, hx, k + 3, hz + 1, &format!("oak_button[face=wall,facing={},powered=false]", face("east")));
    put(b, hx + 1, k + 3, hz, &format!("oak_button[face=wall,facing={},powered=false]", face("south")));
    put(b, hx, k - 1, hz, "hopper[enabled=true,facing=down]");
    let (ox, oz) = t(hx, hz);
    [at[0] + ox, at[1] + k - 2, at[2] + oz]
}

/// The chute to the side (vjeux 18:02, "put the chute to the side"; ironspec iteration 16: leave
/// the follow cubes first; dug: the rock is every wall): the hole's 4 x 4 drops straight down
/// below every villager's cube (the arm's start at feet y -24 or lower: villager y 0.56 - 20.54
/// is -20), then a 4-wide water arm runs `len` cells east through the rock (its floor stepping
/// down every 7, the steps counted from the kill so its last 5 cells are flat: the ironfarm lane's
/// lesson), into the kill beside the farm: its last 2 columns under open gates holding lava at
/// feet + 2 (gates at the arm's mouth beside the lava, the rock as ceiling: the ironfarm lane's
/// cross junction), a water source along the far column carrying drops to one hopper and chest.
/// Returns the output chest.
fn side(b: &mut Builder, at: [i32; 3], place: Place, h: [i32; 2], fl: i32, len: i32, start_feet: i32, one: bool) -> [i32; 3] {
    let [hx, hz] = h;
    // the arm runs south, away from the pods (the villagers stand north of the pad, so the kill
    // beyond 18.84 of every villager needs the shortest arm that way): rows z hz (under the hole's
    // north side) to ze (the kill's far row), 4 wide (x hx..hx+3)
    let ze = hz + 3 + len;
    // the steps counted from the source (the arm's water restarts at each step): the source's
    // level-5 cell spills over the first step (6 cells on), then one step every 7, so the falling
    // water always feeds the next stretch (a step 7 cells from its water is dry: level 7 spreads
    // no further, and a dry lip pins a still golem; a step right under the drop pinned the landing
    // golems too). The kill's last row ends the last stretch (len = 7 s + 2), so no step sits
    // within 5 cells of the kill (the ironfarm lane's lesson)
    let steps = |z: i32| if z < hz + 6 { 0 } else { (z.min(ze) - hz + 1) / 7 };
    // the arm's start: `start_feet` (the cube's floor -20: the lowest villager's feet 0.56 - 20.54;
    // or -24), or 0 for the shallow arm, 3 under the pad (no drop out of the cubes: its golems
    // ride inside them, the judge measures the dwell)
    let f0 = if start_feet == 0 { fl - 4 } else { start_feet };
    let fj = f0 - steps(ze);
    let feet = |z: i32| f0 - steps(z);
    for z in hz..=ze {
        let f = feet(z);
        // 4 cells of air like the ironfarm lane's arm (a golem rides up in the water), 3 at the
        // kill's mouth and rows (the rock over the lava: no air in its random tick's reach)
        let top = if z > hz { feet(z - 1).max(f) + 3 } else { f + 3 };
        let top = if z >= ze - 2 { f + 2 } else { top };
        for y in f..=top {
            for x in hx..=hx + 3 {
                open(b, at, place, x, y, z);
            }
        }
        if place == Place::Void {
            for x in hx..=hx + 3 {
                set(b, at, x, f - 1, z, STONE);
                set(b, at, x, top + 1, z, STONE);
            }
            for y in (f - 1)..=(top + 1) {
                set(b, at, hx - 1, y, z, STONE);
                set(b, at, hx + 4, y, z, STONE);
            }
        }
    }
    // the drop: the hole's 4 x 4 from under the pad down into the arm
    for y in (feet(hz) + 4)..fl {
        for x in hx..=hx + 3 {
            for z in hz..=hz + 3 {
                open(b, at, place, x, y, z);
            }
        }
    }
    for x in hx..=hx + 3 {
        set(b, at, x, feet(hz), hz, WATER);
    }
    // the kill: lava at feet + 2 over open gates in the last two rows, gates at the arm's mouth
    // beside the lava
    for z in [ze - 1, ze] {
        for x in hx..=hx + 3 {
            set(b, at, x, fj + 1, z, GATE_E);
            set(b, at, x, fj + 2, z, LAVA);
        }
    }
    for x in hx..=hx + 3 {
        set(b, at, x, fj + 2, ze - 2, GATE_E);
    }
    // the collection, `one`: a trough one lower past the kill's far row, its water carrying every
    // drop west onto ONE hopper (ironspec: a hopper is 154 s on day 1), lava over it too for the
    // golem the arm pushes in; else a hopper under every cell of the far row (the arm's flow
    // carries every drop to the far wall, anywhere across it; a water source along that row
    // pushed drops back north against the arm's flow, where they stalled), chained east into one
    // chest
    if one {
        let zt = ze + 1;
        for x in hx..=hx + 3 {
            open(b, at, place, x, fj - 1, zt);
            open(b, at, place, x, fj, zt);
            set(b, at, x, fj, zt, GATE_E);
            set(b, at, x, fj + 1, zt, LAVA);
            if place == Place::Void {
                set(b, at, x, fj - 2, zt, STONE);
                set(b, at, x, fj + 2, zt, STONE);
                for y in (fj - 2)..=(fj + 2) {
                    set(b, at, x, y, zt + 1, STONE);
                }
            }
        }
        set(b, at, hx + 3, fj - 1, zt, WATER);
        set(b, at, hx, fj - 2, zt, "hopper[enabled=true,facing=down]");
        return [at[0] + hx, at[1] + fj - 3, at[2] + zt];
    }
    for x in hx..hx + 3 {
        set(b, at, x, fj - 1, ze, "hopper[enabled=true,facing=east]");
    }
    set(b, at, hx + 3, fj - 1, ze, "hopper[enabled=true,facing=down]");
    [at[0] + hx + 3, at[1] + fj - 2, at[2] + ze]
}

/// The blade (M2's open drop and `chamber_safe`, moved to the hole at `h`): the shaft (the hole's
/// 4 x 4) down to feet `k`, walled in its bottom 4 layers in the void (dug: rock), water from its
/// two far corners pushing every golem north into the chamber (x hx..hx+3, z hz-2..hz-1): lava in
/// its north row at head height over open fence gates, gates beside it under the ceiling (no air
/// cell in lava's random-tick reach touches a gate), 2 hoppers under the north row.
/// An upper module's shaft in a shared-kill stack: the hole's 4 x 4 column open from the pad's
/// floor down to `k` (walled in the void; the terrain's rock when dug), open below into the lower
/// module's pad space, which its golems fall through into the lower hole and on to the one kill.
/// Returns the shaft's bottom (no collection of its own).
fn shaft(b: &mut Builder, at: [i32; 3], place: Place, h: [i32; 2], fl: i32, k: i32) -> [i32; 3] {
    let [hx, hz] = h;
    for y in k..shaft_top(b, at, h, fl) {
        for x in hx..=hx + 3 {
            for z in hz..=hz + 3 {
                open(b, at, place, x, y, z);
            }
        }
        // in the void a ring every third layer: a golem (2.7 tall) cannot leave through a gap two
        // blocks high, so the open layers between the rings hold it as walls would (a third of
        // the blocks); the rings stop above the lower villagers' spawn scan (k), where a ring's
        // top would end their scan above their own pad (SpawnUtil takes the first surface from
        // the top), and the golem falls the last layers straight into the lower hole
        if place == Place::Void && (fl - 1 - y) % 3 == 0 {
            for x in (hx - 1)..=(hx + 4) {
                for z in (hz - 1)..=(hz + 4) {
                    let inside = (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z);
                    if !inside {
                        set(b, at, x, y, z, STONE);
                    }
                }
            }
        }
    }
    [at[0] + hx, at[1] + k, at[2] + hz]
}

fn blade(b: &mut Builder, at: [i32; 3], place: Place, h: [i32; 2], fl: i32, k: i32) -> [i32; 3] {
    let [hx, hz] = h;
    let top = (k + 3).min(fl - 1);
    for y in k..shaft_top(b, at, h, fl) {
        for x in hx..=hx + 3 {
            for z in hz..=hz + 3 {
                open(b, at, place, x, y, z);
            }
        }
    }
    if place == Place::Void {
        for y in (k - 1)..top {
            for x in (hx - 1)..=(hx + 4) {
                for z in (hz - 1)..=(hz + 4) {
                    let inside = (hx..=hx + 3).contains(&x) && (hz..=hz + 3).contains(&z) && y >= k;
                    let mouth = (hx..=hx + 3).contains(&x) && z == hz - 1 && (k..=k + 2).contains(&y);
                    if !inside && !mouth {
                        set(b, at, x, y, z, STONE);
                    }
                }
            }
        }
    }
    for x in [hx, hx + 3] {
        set(b, at, x, k, hz + 3, WATER);
    }
    // the chamber, z hz-2 (lava row) and hz-1 (gates beside it), the north wall hz-3
    let (cn, cs) = (hz - 2, hz - 1);
    for x in hx..=hx + 3 {
        if place == Place::Void {
            for y in k..=k + 3 {
                set(b, at, x, y, cn - 1, STONE);
            }
            set(b, at, x, k + 3, cn, STONE);
            set(b, at, x, k + 3, cs, STONE);
            set(b, at, x, k - 1, cs, STONE);
        } else {
            open(b, at, place, x, k, cn);
            open(b, at, place, x, k, cs);
            open(b, at, place, x, k + 1, cs);
        }
        set(b, at, x, k + 1, cn, GATE_E);
        set(b, at, x, k + 2, cn, LAVA);
        set(b, at, x, k + 2, cs, GATE_E);
        if x == hx || x == hx + 3 {
            if place == Place::Void {
                set(b, at, x, k - 1, cn, STONE);
            }
        }
    }
    if place == Place::Void {
        for z in (cn - 1)..=cs {
            for y in (k - 1)..=(k + 3) {
                set(b, at, hx - 1, y, z, STONE);
                set(b, at, hx + 4, y, z, STONE);
            }
        }
    }
    set(b, at, hx + 1, k - 1, cn, "hopper[enabled=true,facing=east]");
    set(b, at, hx + 2, k - 1, cn, "hopper[enabled=true,facing=down]");
    [at[0] + hx + 2, at[1] + k - 2, at[2] + cn]
}

/// One module at `at`. `with_u1`: an observer cage keeps U1 (its pod and villager); a module
/// driven by another module's shared calm needs none.
pub fn module(b: &mut Builder, at: [i32; 3], place: Place, m: &Module, with_u1: bool) -> Ports {
    let (hide_feed, u1_bed, summoners) = cage(b, at, place, &m.cage, with_u1);
    let x0 = pad_x0(&m.cage, &m.pad);
    let fl = m.pad.floor as i32;
    let (h, pad_cells) = pad(b, at, place, x0, m.pad.n as i32, fl);
    let k = m.kill.k();
    let (output, pit_box) = match m.kill {
        Kill::Pit { .. } => (pit(b, at, place, h, fl, k), Some(([at[0] + h[0], at[1] + k, at[2] + h[1]], [at[0] + h[0] + 3, at[1] + fl - 1, at[2] + h[1] + 3]))),
        Kill::Blade { .. } => (blade(b, at, place, h, fl, k), None),
        Kill::Shaft { .. } => (shaft(b, at, place, h, fl, k), None),
        Kill::Side { len, feet, one } => (side(b, at, place, h, fl, len as i32, feet as i32, one), None),
    };
    let (xs, _) = scared_xs(&m.cage);
    let lo = xs[0] - 2 * m.cage.blind as i32 - if m.cage.u1_west && m.cage.calm != CalmKind::Plate { 2 } else { 0 };
    let hi = xs[xs.len() - 1] + if !m.cage.u1_west && m.cage.calm != CalmKind::Plate { 2 } else { 0 };
    Ports { output, hide_feed, u1_bed, summoners, pad_cells, pit: pit_box, row: (at[0] + lo, at[0] + hi) }
}

/// The cheap sorter and iron-block crafter (ironspec iteration 20's storage: the classic sorter's
/// filter stock only, its fillers plain, no lock stock): the ironfarm lane's collect_bottom_at1 laid
/// out the same way (the line hopper L at `l`, the iron slice's filter under it, the poppy slice
/// east of L, the iron into a chest over the crafter's hopper, the crafter packing 9 ingots a
/// block into the blocks' chest), but each filter hopper holds 41 of its item and 4 sticks (plain
/// sticks, stackable and never in the stream: no anvil, no renamed paper) and each lock hopper
/// starts EMPTY (it passes one item a time once its torch lets go; the archive's 64 + 4 lock stock
/// only keeps a full chest from backing into the filter, which the overflow onward covers). Returns
/// the blocks' chest (the judge's port).
pub fn sorter_crafter_cheap(b: &mut Builder, l: [i32; 3]) -> [i32; 3] {
    // collect_bottom_at1's frame: L at (1, yl, -1)
    let (dx, yl, dz) = (l[0] - 1, l[1], l[2] + 1);
    let dust = "redstone_wire[east=side,north=side,power=0,south=side,west=side]";
    let stock = |item: &str, n: i32| format!("{{Items:[{{Slot:0b,id:\"minecraft:{item}\",count:{n}}},{}]}}", (1..=4).map(|k| format!("{{Slot:{k}b,id:\"minecraft:stick\",count:1}}")).collect::<Vec<_>>().join(","));
    let mut set = |b: &mut Builder, x: i32, y: i32, z: i32, s: &str| b.set(x + dx, y, z + dz, s);
    let slice = |b: &mut Builder, set: &mut dyn FnMut(&mut Builder, i32, i32, i32, &str), a: [i32; 3], dzs: i32, item: &str| {
        let (n, s_) = if dzs > 0 { ("north", "south") } else { ("south", "north") };
        let mut s = |b: &mut Builder, sx: i32, sy: i32, sz: i32, st: &str| set(b, a[0] + sx, a[1] - 2 + sy, a[2] + dzs * sz, st);
        s(b, 0, 2, 0, "hopper[enabled=true,facing=down]");
        s(b, 0, 1, 0, "hopper[enabled=false,facing=down]");
        s(b, 0, 2, 1, &format!("comparator[facing={n},mode=compare,powered=false]"));
        for (sx, sy, sz) in [(0, 1, 1), (0, 1, 2), (0, 1, 3), (0, 0, 2), (0, 0, 4), (0, -1, 3)] {
            s(b, sx, sy, sz, "cobblestone");
        }
        s(b, 0, 0, 1, &format!("redstone_wall_torch[facing={n},lit=true]"));
        s(b, 0, 0, 3, &format!("repeater[delay=1,facing={s_},locked=false,powered=false]"));
        for (sx, sy, sz) in [(0, 2, 2), (0, 2, 3), (0, 1, 4)] {
            s(b, sx, sy, sz, dust);
        }
        b.d.block_entities.insert([a[0] + dx, a[1], a[2] + dz], stock(item, 41));
    };
    set(b, 1, yl, -1, "hopper[enabled=true,facing=east]");
    slice(b, &mut set, [1, yl - 1, -1], 1, "iron_ingot");
    slice(b, &mut set, [2, yl, -1], -1, "poppy");
    set(b, 1, yl - 3, -1, "chest[facing=north,type=single,waterlogged=false]");
    set(b, 2, yl - 2, -1, "chest[facing=north,type=single,waterlogged=false]");
    set(b, 2, yl - 3, -1, "hopper[enabled=true,facing=east]");
    set(b, 3, yl - 3, -1, "chest[facing=north,type=single,waterlogged=false]");
    // the crafter (ironblock-v24) as collect_bottom_at1 places it
    let mut c = |b: &mut Builder, cx: i32, cy: i32, cz: i32, st: &str| set(b, cx + 2, cy + yl - 4, -cz - 1, st);
    c(b, -1, 0, 0, "hopper[enabled=true,facing=east]");
    c(b, 0, 0, 0, "crafter[crafting=false,orientation=down_south,triggered=false]");
    c(b, 1, 0, 0, "comparator[facing=west,mode=compare,powered=false]");
    c(b, 2, 0, 0, dust);
    c(b, 0, 0, -1, "cobblestone");
    c(b, 1, 0, -1, "repeater[delay=1,facing=east,locked=false,powered=false]");
    c(b, 2, 0, -1, dust);
    c(b, -1, 0, 1, "crafter[crafting=false,orientation=south_up,triggered=false]");
    c(b, 0, 0, 1, "comparator[facing=west,mode=compare,powered=false]");
    c(b, 1, 0, 1, dust);
    for (cx, cz) in [(1, -1), (2, -1), (1, 0), (2, 0), (0, 1), (1, 1)] {
        c(b, cx, -1, cz, "cobblestone");
    }
    b.d.block_entities.insert([1 + dx, yl - 4, -2 + dz], "{disabled_slots:[I;0,1,2,3,4,5,6,7,8]}".into());
    let port = [2 + dx, yl - 5, -1 + dz];
    b.set(port[0], port[1], port[2], "chest[facing=north,type=single,waterlogged=false]");
    port
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hide_cells_follow_the_lines() {
        // M2: 3 scared at 0, 2, 4 and the zombie at 2 cross one cell; 4 scared at 0..6 with the
        // zombie at 3 cross one cell too (the rays converge on the zombie)
        assert_eq!(hide_cells(&[0, 2, 4], 2), vec![2]);
        assert_eq!(hide_cells(&[0, 2, 4, 6], 3), vec![3]);
        assert_eq!(scared_xs(&Cage { scared: 4, blind: 0, calm: CalmKind::Observer, u1_west: false, regular: false }), (vec![0, 2, 4, 6], 3));
        assert_eq!(scared_xs(&Cage { scared: 3, blind: 0, calm: CalmKind::Observer, u1_west: true, regular: false }), (vec![0, 2, 4], 2));
    }
}
