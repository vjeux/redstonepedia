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
}

impl Kill {
    pub fn k(self) -> i32 {
        match self {
            Kill::Pit { k } | Kill::Blade { k } => k as i32,
        }
    }

    pub fn lava(self) -> u32 {
        match self {
            Kill::Pit { .. } => 1,
            Kill::Blade { .. } => 4,
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
fn pod(b: &mut Builder, at: [i32; 3], place: Place, x: i32, sees: bool) {
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
        (CalmKind::Observer, s) => {
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
        CalmKind::Observer => {
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
            // observer watches its bed head
            let u1 = if c.u1_west { xs[0] - 2 - 2 * c.blind as i32 } else { xs[xs.len() - 1] + 2 };
            if with_u1 {
                pod(b, at, place, u1, false);
            }
            (Some(feed), Some([at[0] + u1, at[1], at[2]]), summoners)
        }
    }
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
    (centre2 - p.n as i32) / 2 + p.dx as i32
}

/// The pad (L2d's dry hole, day1.rs `pit_kill`'s pad): x x0..x0+n-1, z 2..n+1, its floor `fl`,
/// water in its 4 corners at fl + 1, the 4 x 4 hole in its middle (ring: open gates at fl under
/// the water; centre: open gates at fl + 1, dry), 3 cells of headroom; void: glass rims 2 high.
/// Returns the hole's min corner (x, z) and the spawnable floor cells.
fn pad(b: &mut Builder, at: [i32; 3], place: Place, x0: i32, n: i32, fl: i32) -> ([i32; 2], Vec<[i32; 3]>) {
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

/// The pit corner under the hole (day1.rs `pit_kill`): the hole's 4 x 4 shaft from the pad's floor
/// down to golem feet `k` (void: a stone shell), one water source in its far corner pushing every
/// golem into the near corner, one lava source at head height there held by 3 wall signs (signs
/// do not burn), the hopper under that corner over the output.
fn pit(b: &mut Builder, at: [i32; 3], place: Place, h: [i32; 2], fl: i32, k: i32) -> [i32; 3] {
    let [hx, hz] = h;
    for y in k..fl {
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
                    if !inside || y == k - 1 {
                        set(b, at, x, y, z, STONE);
                    }
                }
            }
        }
    }
    set(b, at, hx + 3, k, hz + 3, WATER);
    set(b, at, hx, k + 2, hz, LAVA);
    set(b, at, hx, k + 1, hz, "oak_wall_sign[facing=south,waterlogged=false]");
    set(b, at, hx + 1, k + 2, hz, "oak_wall_sign[facing=south,waterlogged=false]");
    set(b, at, hx, k + 2, hz + 1, "oak_wall_sign[facing=east,waterlogged=false]");
    // the day-1 lane's shield (D1U): `LavaFluid.randomTick` lights fire only in an air cell beside
    // a block lava ignites (the oak signs), or above one; a stone button (not ignited by lava, no
    // collision: golems fall through it) fills the two cells over the side signs
    set(b, at, hx, k + 3, hz + 1, "stone_button[face=wall,facing=east,powered=false]");
    set(b, at, hx + 1, k + 3, hz, "stone_button[face=wall,facing=south,powered=false]");
    set(b, at, hx, k - 1, hz, "hopper[enabled=true,facing=down]");
    [at[0] + hx, at[1] + k - 2, at[2] + hz]
}

/// The blade (M2's open drop and `chamber_safe`, moved to the hole at `h`): the shaft (the hole's
/// 4 x 4) down to feet `k`, walled in its bottom 4 layers in the void (dug: rock), water from its
/// two far corners pushing every golem north into the chamber (x hx..hx+3, z hz-2..hz-1): lava in
/// its north row at head height over open fence gates, gates beside it under the ceiling (no air
/// cell in lava's random-tick reach touches a gate), 2 hoppers under the north row.
fn blade(b: &mut Builder, at: [i32; 3], place: Place, h: [i32; 2], fl: i32, k: i32) -> [i32; 3] {
    let [hx, hz] = h;
    let top = (k + 3).min(fl - 1);
    for y in k..fl {
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
    };
    let (xs, _) = scared_xs(&m.cage);
    let lo = xs[0] - 2 * m.cage.blind as i32 - if m.cage.u1_west && m.cage.calm == CalmKind::Observer { 2 } else { 0 };
    let hi = xs[xs.len() - 1] + if !m.cage.u1_west && m.cage.calm == CalmKind::Observer { 2 } else { 0 };
    Ports { output, hide_feed, u1_bed, summoners, pad_cells, pit: pit_box, row: (at[0] + lo, at[0] + hi) }
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
        assert_eq!(scared_xs(&Cage { scared: 4, blind: 0, calm: CalmKind::Observer, u1_west: false }), (vec![0, 2, 4, 6], 3));
        assert_eq!(scared_xs(&Cage { scared: 3, blind: 0, calm: CalmKind::Observer, u1_west: true }), (vec![0, 2, 4], 2));
    }
}
