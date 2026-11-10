//! `mctas circuit memory sched-gen`: the carousel's nightly scheduler, generated (day-1 lane
//! 2026-11-10; brief part 3; ironspec's rulings: the counting-free BLOCK policy, visits every 4
//! advances inside the deep night). The core, in global coordinates (x east, y up, z south):
//!
//! - the head pointer P: a 2x7 ring of droppers and hoppers (two columns along z at x = 0, 1), one
//!   stone, a loop of dust on top fired by STEP; its 7 droppers name the lanes in the +4 order
//!   0 -> 5 -> 2 -> 3 -> 4 -> 1 -> 6 (west column 0 5 2 3 north to south, east column 4 1 6 south to
//!   north), a comparator tap on each (west taps facing west, east taps facing east);
//! - each tap's signal on a wire to its lane cell (pitch 4, the west lanes west of the ring, the
//!   east lanes east of it, the northernmost tap to the farthest cell so no wire crosses another):
//!   G = line AND P (a subtract comparator: rear = the line, side = NOT P from a wall torch), onto
//!   one OR bus of repeaters running east under the cells;
//! - the visit edge V: the bus's falling edge gated by the night window (`win`);
//! - X: a two-stage off-delay (catalog off-delay-2stage-k1, L 108) of V: the hump window;
//! - STEP: X's falling edge, back on a wire to the pointer's loop and the level ring's loop;
//! - the level ring L: a 2x3 ring (catalog-style), stepped by STEP with P.
//!
//! Inputs: `line<b>` (the counter's lines), `win`. Outputs: `p<b>` (each lane cell's plain P),
//! `l0..l2`, `v`, `x`, `step`.

use anyhow::{Result, bail};
use mctas::design::Design;
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) type P3 = [i32; 3];

pub(crate) struct B {
    pub(crate) blocks: BTreeMap<P3, String>,
    pub(crate) ports: BTreeMap<String, P3>,
    pub(crate) nbt: BTreeMap<P3, String>,
}

pub(crate) const DUST: &str = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
pub(crate) const STONE: &str = "minecraft:stone";

pub(crate) fn rep(facing: &str, delay: u8) -> String {
    format!("minecraft:repeater[delay={delay},facing={facing},locked=false,powered=false]")
}

impl B {
    pub(crate) fn put(&mut self, p: P3, s: &str) -> Result<()> {
        if let Some(old) = self.blocks.get(&p) {
            if old != s {
                bail!("cell {p:?} holds {old}, now {s}");
            }
        }
        self.blocks.insert(p, s.to_string());
        Ok(())
    }
    /// A block that needs a sturdy floor: the stone under it too (unless something is there).
    pub(crate) fn on(&mut self, p: P3, s: &str) -> Result<()> {
        self.put(p, s)?;
        let below = [p[0], p[1] - 1, p[2]];
        if !self.blocks.contains_key(&below) {
            self.blocks.insert(below, STONE.into());
        }
        Ok(())
    }
    /// A design placed at `off`, turned `turns` quarter turns; its ports prefixed (`None`: dropped).
    fn stamp(&mut self, path: &Path, off: P3, turns: u8, prefix: Option<&str>) -> Result<()> {
        let d = Design::load(path)?;
        let place = |q: P3| {
            let r = clock::rotate_pos(q, turns);
            [r[0] + off[0], r[1] + off[1], r[2] + off[2]]
        };
        for (q, s) in &d.blocks {
            self.put(place(*q), &clock::canon::rotate_state(s, turns))?;
        }
        for (q, n) in &d.block_entities {
            self.nbt.insert(place(*q), n.clone());
        }
        if let Some(pre) = prefix {
            for (k, q) in &d.ports {
                self.ports.insert(format!("{k}{pre}"), place(*q));
            }
        }
        Ok(())
    }
    /// A wire of dust at y along `path` (consecutive cells adjacent), a repeater every `every`
    /// cells pointing along the path (the first cell's predecessor is `from`).
    pub(crate) fn wire(&mut self, from: P3, path: &[P3], every: usize) -> Result<()> {
        let mut prev = from;
        for (i, &c) in path.iter().enumerate() {
            if every > 0 && i > 0 && i % every == 0 && i + 1 < path.len() {
                let d = [prev[0] - c[0], prev[2] - c[2]];
                let facing = match d {
                    [1, 0] => "east",
                    [-1, 0] => "west",
                    [0, 1] => "south",
                    [0, -1] => "north",
                    _ => bail!("wire turns vertically at {c:?}"),
                };
                self.on(c, &rep(facing, 1))?;
            } else {
                self.on(c, DUST)?;
            }
            prev = c;
        }
        Ok(())
    }
}

/// Cells from `a` to `b` (inclusive) along one axis.
pub(crate) fn seg(a: P3, b: P3) -> Vec<P3> {
    let mut v = Vec::new();
    let (dx, dy, dz) = ((b[0] - a[0]).signum(), (b[1] - a[1]).signum(), (b[2] - a[2]).signum());
    let mut c = a;
    loop {
        v.push(c);
        if c == b {
            break;
        }
        c = [c[0] + dx, c[1] + dy, c[2] + dz];
    }
    v
}

/// Build the scheduler core into `out`; `modules` is the directory of the module designs
/// (vcell.txt, vedge-c2.txt, fedge-c.txt, l-ring.txt) and `offdelay` the catalog's
/// off-delay-2stage-k1.
pub fn generate(out: &Path, modules: &Path, offdelay: &Path) -> Result<()> {
    let mut b = B { blocks: BTreeMap::new(), ports: BTreeMap::new(), nbt: BTreeMap::new() };
    // ---- the pointer ring (west column x = 0, east column x = 1, z 0..6)
    let ring_lane = [0usize, 5, 2, 3, 4, 1, 6];
    for k in 0..14i32 {
        let (x, z) = if k < 7 { (0, k) } else { (1, 13 - k) };
        let dropper = k % 2 == 0;
        let facing = match k {
            0..=5 => "south",
            6 => "east",
            7..=12 => "north",
            _ => "west",
        };
        let s = if dropper { format!("minecraft:dropper[facing={facing},triggered=false]") } else { format!("minecraft:hopper[enabled=true,facing={facing}]") };
        b.put([x, 0, z], &s)?;
        b.on([x, 1, z], DUST)?; // the loop (stands on the ring)
    }
    b.nbt.insert([0, 0, 0], "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}".into());
    // ---- taps and their wires: the west taps (z 0 2 4 6, lanes 0 5 2 3) to the west cells, the
    // southernmost to the nearest; the east taps (z 5 3 1, lanes 4 1 6) to the east cells
    let zc = 8; // the cells' first row (each cell's wire enters at (x0, 0, zc))
    let west_x0 = [-12, -9, -6, -3]; // P columns for the west taps z = 0, 2, 4, 6
    let east_x0 = [4, 7, 10]; // P columns for the east taps z = 5, 3, 1
    let mut cell_of_lane = [0i32; 7];
    for (i, z) in [0, 2, 4, 6].iter().enumerate() {
        let lane = ring_lane[i];
        b.on([-1, 0, *z], "minecraft:comparator[facing=east,mode=compare,powered=false]")?;
        b.on([-2, 0, *z], &rep("east", 1))?;
        b.ports.insert(format!("output.p{lane}"), [-3, 0, *z]);
        let x0 = west_x0[i];
        let mut path = seg([-3, 0, *z], [x0, 0, *z]);
        path.extend(seg([x0, 0, z + 1], [x0, 0, zc - 1]));
        b.wire([-2, 0, *z], &path, 8)?;
        cell_of_lane[lane] = x0;
    }
    for (i, z) in [5, 3, 1].iter().enumerate() {
        let lane = ring_lane[4 + i];
        b.on([2, 0, *z], "minecraft:comparator[facing=west,mode=compare,powered=false]")?;
        b.on([3, 0, *z], &rep("west", 1))?;
        b.ports.insert(format!("output.p{lane}"), [4, 0, *z]);
        let x0 = east_x0[i];
        let mut path = seg([4, 0, *z], [x0, 0, *z]);
        path.extend(seg([x0, 0, z + 1], [x0, 0, zc - 1]));
        b.wire([3, 0, *z], &path, 8)?;
        cell_of_lane[lane] = x0;
    }
    // ---- the lane cells: the west ones extend east of their P column (bus east-bound), the
    // east ones west of it (mirrored, bus west-bound); the two buses meet at (0..1, 0, zc + 4)
    for (lane, x0) in cell_of_lane.iter().enumerate() {
        let west = *x0 < 0;
        let file = if west { "vcell3.txt" } else { "vcell3w.txt" };
        b.stamp(&modules.join(file), [*x0, -1, zc], 0, None)?;
        b.ports.insert(format!("input.line{lane}"), [if west { x0 + 2 } else { x0 - 2 }, 0, zc + 1]);
    }
    let zb = zc + 4;
    b.on([0, 0, zb], DUST)?;
    b.on([1, 0, zb], DUST)?;
    // ---- the arm, turned to run south from the junction: (0, 0, zb+1) a repeater (the plain-dust
    // buses arrive weak), the module's first dust at (0, 0, zb+2), its pulse into the window's
    // input stone at (0, 0, zb+8): the window opens while the named batch is still at the station
    b.on([0, 0, zb + 1], &rep("north", 1))?;
    b.stamp(&modules.join("varm.txt"), [0, -1, zb + 2], 1, None)?;
    b.ports.insert("input.win".into(), [3, 0, zb + 3]);
    // ---- X (the hump window, >= 100 gt past the line's fall): the 220-gt off-delay turned half
    // round below the input stone; its stage-2 stone A2 at (7, 0, zb+11) counts its level down,
    // so a repeater squares it: X = (9, 0, zb+11)
    b.stamp(offdelay, [1, -1, zb + 11], 2, None)?;
    b.on([8, 0, zb + 11], &rep("west", 1))?;
    b.ports.insert("output.v".into(), [9, 0, zb + 11]);
    b.ports.insert("output.x".into(), [9, 0, zb + 11]);
    // ---- STEP: X's falling edge, its repeater into a stone at (13, 0, zb+11); the level ring
    // east of that stone (turned half round, its loop beside it)
    b.stamp(&modules.join("fedge-n.txt"), [9, -1, zb + 11], 0, None)?;
    let st = [13, 0, zb + 11];
    b.put(st, STONE)?;
    b.stamp(&modules.join("l-ring.txt"), [16, -1, zb + 11], 2, None)?;
    b.ports.insert("output.l0".into(), [16, -1, zb + 13]);
    b.ports.insert("output.l1".into(), [14, -1, zb + 13]);
    b.ports.insert("output.l2".into(), [15, -1, zb + 8]);
    // the wire to the pointer loop: up on the stone, west at y = 2 over the window, north at
    // y = 2 along x = 1 over the arm and the corridor, down into the loop's south-east cell
    let z = zb + 10;
    b.on([13, 1, zb + 11], DUST)?;
    b.wire([13, 1, zb + 11], &[[13, 2, z]], 0)?;
    b.wire([13, 2, z], &seg([12, 2, z], [7, 2, z]), 0)?;
    b.on([6, 2, z], &rep("east", 1))?;
    b.wire([6, 2, z], &[seg([5, 2, z], [1, 2, z]), seg([1, 2, z - 1], [1, 2, 18])].concat(), 0)?;
    b.on([1, 2, 17], &rep("south", 1))?;
    b.wire([1, 2, 17], &seg([1, 2, 16], [1, 2, 10]), 0)?;
    b.on([1, 2, 9], &rep("south", 1))?;
    b.wire([1, 2, 9], &seg([1, 2, 8], [1, 2, 7]), 0)?;
    // ---- write
    let mut d = Design::parse("name: carousel-scheduler-core\n")?;
    d.blocks = b.blocks;
    d.ports = b.ports;
    d.block_entities = b.nbt;
    d.save(out)?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}
