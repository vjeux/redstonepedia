//! Full-size sugar cane farms (track clk-rt, designs/briefs/farm-sugar-cane.md): `circuit cane-gen`
//! writes THE OBVIOUS DESIGN, the bar of the brief: double rows of cane on mud, an observer and a
//! piston per cane, hoppers under every mud and water cell into one chest.
//!
//! Columns along x, repeated every 5 (`modules` double rows), canes along z (`length`):
//!
//! ```text
//!   x:  R  wB mB mA wA R  wB mB mA wA R ...
//! y=3:  d  o> .  .  <o d  ...          o = observer watching the cane's 3rd block
//! y=2:  s  p> c  c  <p s               p = piston facing the cane (breaks its 2nd block)
//! y=1:  .  .  c  c  .  .               c = sugar cane (planted by the judge)
//! y=0:  s  w  M  M  w  s               M = mud, w = water source, s = stone
//! y=-1: .  h  h  h  h  .               h = hopper (south), into a collector row at z = length
//! ```
//!
//! An observer's back powers the relay dust (R: dust on stone), the dust powers its stone, the stone
//! powers the pistons on both sides of it. A piston breaks the 2nd block (the 3rd pops a tick later)
//! and pushes the 2nd block's item into the neighbouring row; every item lands on mud (14/16 tall:
//! a hopper under it is not blocked, `HopperBlockEntity::suckInItems`) or in water (a hopper under
//! it too). One mud catcher (with its hopper) at each end of every cane row takes items that fly
//! off the row's ends.

use mctas::design::Design;
use std::collections::BTreeMap;

type Pos = [i32; 3];

/// LAYOUT 5 (designs/briefs/farm-sugar-cane.md): the layout-4 field (lattice, hopper floor, ring,
/// two collectors) with no pistons, dust or clock: ONE flying machine sweeps the 2nd-block height.
/// The machine is the docking flying machine's 6-block core (two halves of observer facing down,
/// slime, sticky piston; track fly) at y = 2..3, travelling along z between a north and a south
/// station (an observer that sees it arrive, a stone it powers, a cross of dust on the stone behind
/// the leading piston; a wall), with a WING on each half: a row of slime at y = 2 (the blade, one
/// block per column, stuck to the half's observer), up to 9 per half (12 blocks per push), so one
/// machine sweeps 20 columns. Canes are destroyed by a moving block (POPPED) and slime does not
/// drag them sideways. It starts when the judge plants the cane under its first observer (the
/// other observer stands over water). Slime blocks conduct redstone; honey (an alternative for a
/// second machine) does not, so the core's observer would not reach its piston through it.
pub fn sweeper(w: i32, l: i32) -> Design {
    let mut d = field(w, l);
    // the core columns: h1's observer at cx (wing west to x = 0), h2's at cx + 1 (wing east)
    let cx = w / 2 - 1;
    // the start row: h1's observer over a cane, h2's over water
    let water = |x: i32, z: i32| (x + 2 * z).rem_euclid(5) == 0;
    let z0 = (1..l - 2).find(|&z| !water(cx, z) && water(cx + 1, z + 1)).unwrap_or(1);
    add_sweeper(&mut d, w, l, z0);
    d.name = format!("cane-sweeper-{w}x{l}");
    tiled(d)
}

/// The same sweeper over a BAMBOO field (w86's fuel-farm brief, layout C2, 2026-10-22): mud over a
/// hopper under every cell (bamboo needs no water), a stalk on every field cell but the one under
/// the second half's observer (so only the first half's observer sees the planting and starts the
/// machine). Bamboo is destroyed by a moving block like cane; its stump keeps a collision column,
/// so items land on stumps, and the next pass's stone blade pushes them on.
pub fn bamboo_sweeper(w: i32, l: i32) -> Design {
    let cx = w / 2 - 1;
    let z0 = 1;
    let mut d = field_of(w, l, "minecraft:bamboo", &|_, _| false, Some([cx + 1, 1, z0 + 1]));
    add_sweeper(&mut d, w, l, z0);
    // walls along both long sides up to y = 4 instead of a curb: a popped bamboo item pushed out of
    // a block by the machine can leave sideways at 0.2 block/gt as high as y ~3.2
    for z in -3..=l + 2 {
        for x in [-2, w + 1] {
            for y in 1..=4 {
                d.blocks.insert([x, y, z], STONE.into());
            }
        }
    }
    // a stone over the catcher row in front of each station's hopper pair: an item that came in at
    // y ~1 slipped under the upper hopper's bowl, where neither hopper takes it (the lower one pulls
    // from the hopper above it instead of sucking items), and despawned
    d.blocks.insert([cx + 1, 1, -2], STONE.into());
    d.blocks.insert([cx, 1, l + 1], STONE.into());
    // the south station's wall beside the dust: stone does (`circuit material`: no slime touches it)
    d.blocks.insert([cx + 1, 2, l + 2], STONE.into());
    // one more row on both end walls (y = 5): an item launched by the machine's slime from on top of
    // it (y ~4) flew over the y = 4 row at the north end and out of the world (10x64's tiled run at
    // rotation 3, once in about 30,000 items)
    for x in -2..=w + 1 {
        for z in [-3, l + 2] {
            d.blocks.insert([x, 5, z], STONE.into());
        }
    }
    // a catcher behind each end wall: an item riding on top of the machine is squeezed into the
    // end wall as the machine docks, and an item inside a block moves to the closest free space
    // (`ItemEntity.tick` -> `moveTowardsClosestSpace`), which can be behind the wall (10x64's tiled
    // run at rotation 3, once in about 30,000 items: out of the world). Mud over hoppers one cell
    // behind the wall passes such an item under the wall into the catcher row, a curb behind it.
    for x in -1..=w {
        for (zb, dz) in [(-4, 1), (l + 3, -1)] {
            b_insert_catcher(&mut d.blocks, x, zb, dz);
        }
    }
    // (the station pockets already hold a hopper column there: `b_insert_catcher` leaves them)
    d.name = format!("bamboo-sweeper-{w}x{l}");
    tiled(d)
}

/// One cell of a catcher behind an end wall (`zb`: the cell behind the wall, `dz`: toward the field):
/// mud over a hopper, a hopper under the wall into the catcher row, a curb behind. A station
/// pocket's hopper column (already there) is left as it is.
fn b_insert_catcher(b: &mut BTreeMap<Pos, String>, x: i32, zb: i32, dz: i32) {
    if b.get(&[x, 0, zb]).is_some_and(|s| s.starts_with("minecraft:hopper")) {
        return;
    }
    let facing = if dz > 0 { "south" } else { "north" };
    b.entry([x, 0, zb]).or_insert_with(|| MUD.into());
    b.entry([x, -1, zb]).or_insert_with(|| hopper(facing));
    b.entry([x, -1, zb + dz]).or_insert_with(|| hopper(facing));
    // a back wall up to y = 3 (a 1-high curb caught an expelled item on its top, where it despawned)
    for y in 1..=3 {
        b.entry([x, y, zb - dz]).or_insert_with(|| STONE.into());
    }
}

/// The sweeper machine, its stations, walls, curbs and pockets, added to a field (`z0`: the machine's
/// start row).
fn add_sweeper(d: &mut Design, w: i32, l: i32, z0: i32) {
    let slime = "minecraft:slime_block";
    let obs = "minecraft:observer[facing=down,powered=false]";
    let cx = w / 2 - 1;
    let b = &mut d.blocks;
    b.insert([cx, 2, z0], obs.into());
    b.insert([cx, 3, z0], slime.into());
    b.insert([cx + 1, 3, z0], "minecraft:sticky_piston[extended=false,facing=south]".into());
    b.insert([cx, 3, z0 + 1], "minecraft:sticky_piston[extended=false,facing=north]".into());
    b.insert([cx + 1, 3, z0 + 1], slime.into());
    b.insert([cx + 1, 2, z0 + 1], obs.into());
    // the wings: slime at y = 3 stuck to the half's slime (a piston moves only what sticks to a
    // sticky block of the structure: `PistonStructureResolver.addBranchingBlocks`), a stone blade
    // under each (a moving slime block LAUNCHES the items it hits at 1 block/gt along its motion,
    // `PistonMovingBlockEntity.moveCollidedEntities`; stone just pushes them)
    for x in 0..cx {
        b.insert([x, 3, z0], slime.into());
        b.insert([x, 2, z0], STONE.into());
    }
    for x in cx + 2..w {
        b.insert([x, 3, z0 + 1], slime.into());
        b.insert([x, 2, z0 + 1], STONE.into());
    }
    // the north station (the docking machine's (1,0,-1) observer facing south, (1,0,-2) stone,
    // dust on it): it sees h2's observer arrive over the ring row z = -1 (no cane: only the
    // machine changes that cell). The block under the dust is MUD over a hopper (mud conducts
    // like stone, and a hopper sucks through its 14/16 height as on the field): an item the
    // docking piston pushes off the station observer's top into the dust's cell landed on a stone
    // there and stayed (judge's 10x32 build at rotation 1)
    b.insert([cx + 1, 2, -2], "minecraft:observer[facing=south,powered=false]".into());
    b.insert([cx + 1, 2, -3], MUD.into());
    b.insert([cx + 1, 3, -3], CROSS.into());
    // the south station: an observer facing north at z = l + 1 (it sees h1's observer arrive over
    // the ring row z = l), mud (over a hopper) and a wall behind it, dust on the mud behind h2's
    // north-facing piston
    b.insert([cx, 2, l + 1], "minecraft:observer[facing=north,powered=false]".into());
    b.insert([cx, 2, l + 2], MUD.into());
    b.insert([cx + 1, 2, l + 2], "minecraft:obsidian".into());
    b.insert([cx, 3, l + 2], CROSS.into());
    // A moving slime block launches the items its box sweeps at 1 block/gt along z
    // (`PistonMovingBlockEntity.moveCollidedEntities`): a cane item pops with 0.2 up and rises
    // into the wing's slime layer. They fly ~9 blocks at y ~3: obsidian walls (immovable, so the
    // slime never drags them) at y = 2..3 past both ends stop them (a launched item's box tops out
    // near y = 3.7), and a catcher row under each end (mud over a hopper into the ring) takes what
    // falls there.
    // (y = 1..3, x = -1..=w: a launched item that falls short skims the catcher row's mud at y ~1
    // and slid off the end under a wall that started at y = 2, in 9 of the judge's 16 builds)
    // Obsidian only where the machine's slime touches the wall when it docks (y = 3: h1's slime
    // and wing at the north end, h2's at the south end; a stone there would be dragged off when
    // the machine leaves); stone everywhere else (vjeux 2026-10-21: obsidian only where needed).
    let obsidian = "minecraft:obsidian";
    for x in -1..=w {
        for (z, skip, touched) in [(-3, cx + 1, (0..=cx).contains(&x)), (l + 2, cx, (cx + 1..w).contains(&x))] {
            if x != skip {
                b.insert([x, 3, z], if touched { obsidian } else { STONE }.into());
            }
            b.entry([x, 2, z]).or_insert_with(|| STONE.into());
            b.insert([x, 1, z], STONE.into());
            // y = 4: an item that ends up on top of the machine (pushed out of a block the machine
            // moved into it) is launched at y ~4 (one flew over a y = 1..3 wall on the twin)
            b.insert([x, 4, z], STONE.into());
        }
    }
    // a curb along both long sides (y = 1, one cell outside the ring): a launched item keeps the
    // sideways 0.1 block/gt it popped with and can land just past the ring (one did, on a water
    // containment stone, in the judge's build at rotation 1); by the time it has drifted that far
    // it is below y = 2, so one block high stops it and drops it into the ring
    for z in -3..=l + 2 {
        for x in [-2, w + 1] {
            b.insert([x, 1, z], STONE.into());
        }
    }
    // a hopper over each output chest (the corners past the catcher row): an item that lands there
    // went into the chest instead of resting on its lid (judge's build at rotation 1)
    b.insert([-1, 0, l + 1], hopper("down"));
    b.insert([w, 0, l + 1], hopper("down"));
    // a pocket behind each station's dust: the core's slime launches its own columns' items along
    // the station's lane, through the dust's cell (no collision); with obsidian right behind the
    // dust one fell back onto the stone under the dust and stayed (judge's build at rotation 3).
    // Now they fly on into the pocket, hit obsidian one cell further, and drop onto a hopper that
    // passes them down into the catcher row.
    for (x, z, dz, into) in [(cx + 1, -4, -1, "south"), (cx, l + 3, 1, "north")] {
        b.insert([x, 3, z + dz], STONE.into());
        b.insert([x - 1, 3, z], STONE.into());
        b.insert([x + 1, 3, z], STONE.into());
        b.insert([x, 4, z], STONE.into());
        for y in 0..=2 {
            b.insert([x, y, z], hopper("down"));
        }
        b.insert([x, -1, z], hopper(into));
        b.insert([x, -1, z - dz], hopper(into));
        // under the station's mud: two hoppers down into that one
        b.insert([x, 1, z - dz], hopper("down"));
        b.insert([x, 0, z - dz], hopper("down"));
    }
    for x in -1..=w {
        b.insert([x, -1, -2], hopper("south"));
        b.insert([x, 0, -2], MUD.into());
    }
    for x in 0..w {
        b.insert([x, -1, l + 1], hopper("north"));
        b.insert([x, 0, l + 1], MUD.into());
    }
}

/// The sweeper's machines and stations alone, as a clock (no `circuit:`, the field removed), kicked
/// by a stone placed for 1 gt under the first machine's first observer: `mctas clock sim` shows
/// whether they shuttle (the period is the round trip).
pub fn sweeper_machines(w: i32, l: i32) -> Design {
    sweeper_test(w, l, 0)
}

/// [`sweeper_machines`] with the field kept (`canes` >= 1: a cane on every crop port; 2: and a 2nd
/// block on each), to see the machine work over a planted field.
pub fn sweeper_test(w: i32, l: i32, canes: i32) -> Design {
    let mut d = sweeper(w, l);
    if canes == 0 {
        d.blocks.retain(|p, _| p[1] >= 2);
    } else {
        let crops: Vec<Pos> = d.ports.iter().filter(|(k, _)| k.starts_with("crop.")).map(|(_, p)| *p).collect();
        for p in crops {
            d.blocks.insert(p, "minecraft:sugar_cane[age=0]".into());
            if canes >= 2 && !d.blocks.contains_key(&[p[0], p[1] + 1, p[2]]) {
                d.blocks.insert([p[0], p[1] + 1, p[2]], "minecraft:sugar_cane[age=0]".into());
            }
        }
    }
    d.ports.clear();
    d.meta.clear();
    let first = d.blocks.iter().find(|(_, s)| s.starts_with("minecraft:observer[facing=down")).map(|(p, _)| *p).unwrap_or([0, 2, 0]);
    d.ports.insert("start".into(), [first[0], first[1] - 1, first[2]]);
    d.meta.insert("port start".into(), if canes == 0 { "minecraft:stone 1" } else { "last" }.into());
    d.name = format!("cane-sweeper-machines-{w}x{l}");
    d
}

/// The layout-4 field alone: the lattice of mud and water, a hopper under every cell, the ring of
/// mud (water where the lattice continues, held in by stones), two collectors into two chests.
fn field(w: i32, l: i32) -> Design {
    let water = |x: i32, z: i32| (x + 2 * z).rem_euclid(5) == 0;
    field_of(w, l, "minecraft:sugar_cane", &water, None)
}

/// [`field`] for any crop: `water(x, z)` cells hold water (held in by stones outside the ring),
/// the rest mud, a hopper under every cell; every field mud cell is a crop port except `skip`.
fn field_of(w: i32, l: i32, crop: &str, water: &dyn Fn(i32, i32) -> bool, skip: Option<Pos>) -> Design {
    let mut b: BTreeMap<Pos, String> = BTreeMap::new();
    let mut ports: BTreeMap<String, Pos> = BTreeMap::new();
    let mut n = 0;
    for z in -1..=l {
        for x in -1..=w {
            let field = (0..w).contains(&x) && (0..l).contains(&z);
            let facing = if x == -1 || x == w { "south" } else if x < w / 2 { "west" } else { "east" };
            b.insert([x, -1, z], hopper(facing));
            if water(x, z) {
                b.insert([x, 0, z], WATER.into());
                if !field {
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (nx, nz) = (x + dx, z + dz);
                        if !(-1..=w).contains(&nx) || !(-1..=l).contains(&nz) {
                            b.insert([nx, 0, nz], STONE.into());
                        }
                    }
                }
            } else {
                b.insert([x, 0, z], MUD.into());
                if field && skip != Some([x, 1, z]) {
                    ports.insert(format!("crop.{n:03}"), [x, 1, z]);
                    n += 1;
                }
            }
        }
    }
    ports.insert("output.a".into(), [-1, -1, l + 1]);
    ports.insert("output.b".into(), [w, -1, l + 1]);
    let mut meta = BTreeMap::new();
    let short = crop.trim_start_matches("minecraft:");
    meta.insert("circuit".to_string(), format!("harvester {short}"));
    let name = if short == "sugar_cane" { format!("cane-field-{w}x{l}") } else { format!("{short}-field-{w}x{l}") };
    Design { name, meta, ports, blocks: b, entities: Vec::new(), block_entities: BTreeMap::new(), ..Default::default() }
}

/// LAYOUT 4 (designs/briefs/farm-sugar-cane.md): a `w` x `l` field, water on `x + 2z = 0 (mod 5)`
/// (every other cell is cane on mud: 4/5 of the floor), a hopper under every floor cell (rows east
/// into the collector), a ring of mud on hoppers around the field (items land up to ~1 block from
/// their cane), a piston facing down at each cane's 3rd-block height (the 3rd block never grows;
/// the head breaks the 2nd and pushes its item down), dust on every piston top (a carpet), fed by
/// repeaters on the west and east edges from distribution lines one cell further out, joined north
/// of the field and driven by `clock` (a design whose `port output` cell is the clock's on-mostly
/// output block: a wall torch on its north face makes the short pulse).
pub fn lattice(w: i32, l: i32, clock: &Design) -> Design {
    let mut b: BTreeMap<Pos, String> = BTreeMap::new();
    let mut ports: BTreeMap<String, Pos> = BTreeMap::new();
    let water = |x: i32, z: i32| (x + 2 * z).rem_euclid(5) == 0;
    let mut n = 0;
    // floor, hoppers, ring
    for z in -1..=l {
        for x in -1..=w {
            let field = (0..w).contains(&x) && (0..l).contains(&z);
            // two collectors (the ring's west and east columns, each into its own chest): the
            // judge's speed-up (rts 30 for 200 canes: ~1.9 items/s) would load one hopper line
            // (2.5/s) to 75%, and its stray-item batches would back the rows up for minutes
            let facing = if x == -1 || x == w { "south" } else if x < w / 2 { "west" } else { "east" };
            b.insert([x, -1, z], hopper(facing));
            if water(x, z) {
                // the lattice continues into the ring: an edge cane's water may lie there; a stone
                // outside the ring holds that water in
                b.insert([x, 0, z], WATER.into());
                if !field {
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (nx, nz) = (x + dx, z + dz);
                        if !(-1..=w).contains(&nx) || !(-1..=l).contains(&nz) {
                            b.insert([nx, 0, nz], STONE.into());
                        }
                    }
                }
            } else {
                b.insert([x, 0, z], MUD.into());
            }
            if field && !water(x, z) {
                ports.insert(format!("crop.{n:03}"), [x, 1, z]);
                n += 1;
                b.insert([x, 3, z], piston("down"));
                b.insert([x, 4, z], DUST.into());
            }
        }
    }
    ports.insert("output.a".into(), [-1, -1, l + 1]);
    ports.insert("output.b".into(), [w, -1, l + 1]);
    // a repeater's `facing` is its INPUT side: `rep(d)` outputs toward d
    let rep = |d: &str| {
        let f = match d {
            "east" => "west",
            "west" => "east",
            "north" => "south",
            _ => "north",
        };
        format!("minecraft:repeater[delay=1,facing={f},locked=false,powered=false]")
    };
    let mut wire = |b: &mut BTreeMap<Pos, String>, p: Pos, s: String| {
        b.insert([p[0], p[1] - 1, p[2]], STONE.into());
        b.insert(p, s);
    };
    // distribution lines (x = -2 and x = w + 1) with a repeater at their top and middle, entry
    // repeaters (x = -1 and x = w) into the carpet's edge every other cane-edged row
    let mid = l / 2 - 1;
    for (line, entry, into, edge) in [(-2, -1, "east", 0), (w + 1, w, "west", w - 1)] {
        for z in -1..l {
            let s = if z == -1 || z == mid { rep("south") } else { DUST.into() };
            wire(&mut b, [line, 4, z], s);
        }
        let mut last = -9;
        for z in 0..l {
            if !water(edge, z) && z != mid && z - last >= 2 {
                wire(&mut b, [entry, 4, z], rep(into));
                last = z;
            }
        }
    }
    // the north connector (z = -2) from x = -2 to w + 1, fed at x = jx from the clock's route
    let jx = w / 2;
    for x in -2..=w + 1 {
        let s = if x == jx - 1 { rep("west") } else if x == jx + 1 { rep("east") } else { DUST.into() };
        wire(&mut b, [x, 4, -2], s);
    }
    // the clock north of it: its output block at (jx + 2, 4, -6), the torch on that block's north
    // face, a repeater west of the torch, then dust south down x = jx to the connector
    let out = clock.ports.get("output").copied().unwrap_or([0, 0, 0]);
    let a = [out[0] + 1, out[1], out[2]];
    let off = [jx + 2 - a[0], 4 - a[1], -6 - a[2]];
    for (p, s) in &clock.blocks {
        b.insert([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s.clone());
    }
    let mut block_entities = BTreeMap::new();
    for (p, s) in &clock.block_entities {
        block_entities.insert([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s.clone());
    }
    b.insert([jx + 2, 4, -7], "minecraft:redstone_wall_torch[facing=north,lit=true]".into());
    wire(&mut b, [jx + 1, 4, -7], rep("west"));
    for z in -7..=-3 {
        wire(&mut b, [jx, 4, z], DUST.into());
    }
    let mut meta = BTreeMap::new();
    meta.insert("circuit".to_string(), "harvester sugar_cane".to_string());
    tiled(Design { name: format!("cane-lattice-{w}x{l}"), meta, ports, blocks: b, entities: Vec::new(), block_entities, ..Default::default() })
}

/// Tileable (a must-have): copies side by side along x with one cell of air between them (the
/// farm judge's `tile:` run checks they do not disturb each other).
fn tiled(mut d: Design) -> Design {
    let (lo, hi) = d.blocks.keys().fold((i32::MAX, i32::MIN), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    d.meta.insert("tile".to_string(), format!("{} 0 0", hi - lo + 2));
    d
}

const STONE: &str = "minecraft:stone";
const MUD: &str = "minecraft:mud";
const WATER: &str = "minecraft:water[level=0]";
const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
/// An isolated dust as a player places it: a cross, powering its four sides (an all-`none` dust is
/// a dot, which powers only the block under it).
const CROSS: &str = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";

fn hopper(facing: &str) -> String {
    format!("minecraft:hopper[enabled=true,facing={facing}]")
}

fn piston(facing: &str) -> String {
    format!("minecraft:piston[extended=false,facing={facing}]")
}

fn observer(facing: &str) -> String {
    format!("minecraft:observer[facing={facing},powered=false]")
}

/// The obvious design: `modules` double rows of `length` canes each.
pub fn obvious(modules: i32, length: i32) -> Design {
    let mut b: BTreeMap<Pos, String> = BTreeMap::new();
    let mut ports: BTreeMap<String, Pos> = BTreeMap::new();
    let mut put = |p: Pos, s: &str| {
        b.insert(p, s.to_string());
    };
    let relays: Vec<i32> = (0..=modules).map(|m| 5 * m).collect();
    for z in 0..length {
        for &r in &relays {
            put([r, 0, z], STONE);
            put([r, 2, z], STONE);
            put([r, 3, z], DUST);
        }
        for m in 0..modules {
            let base = 5 * m;
            // B side: piston and observer face east, A side: west
            for (w, cane, face) in [(base + 1, base + 2, "east"), (base + 4, base + 3, "west")] {
                put([w, 0, z], WATER);
                put([w, -1, z], &hopper("south"));
                put([w, 2, z], &piston(face));
                put([w, 3, z], &observer(face));
                // a stone on each observer: a 3rd block's item lands on the next column's
                // observer top otherwise and stays there (the farm judge's tiled run, 2026-10-22)
                put([w, 4, z], STONE);
                put([cane, 0, z], MUD);
                put([cane, -1, z], &hopper("south"));
            }
        }
    }
    let mut n = 0;
    for m in 0..modules {
        let base = 5 * m;
        for cane in [base + 2, base + 3] {
            for z in 0..length {
                ports.insert(format!("crop.{n:03}"), [cane, 1, z]);
                n += 1;
            }
            // catchers at both ends of the row (no cane), the water channels walled
            put([cane, 0, -1], MUD);
            put([cane, -1, -1], &hopper("south"));
            put([cane, 0, length], MUD);
        }
        for w in [base + 1, base + 4] {
            // the water channels' ends: mud (holds the water) over a hopper, so an item that
            // lands there is taken too
            put([w, 0, -1], MUD);
            put([w, -1, -1], &hopper("south"));
            put([w, 0, length], MUD);
        }
        // a second catcher row at both ends (a 3rd block's item flies up to ~1.3 blocks: one
        // escaped past z = length + 0.75 in a 48000 gt run), into the first
        for x in base + 1..=base + 4 {
            put([x, 0, -2], MUD);
            put([x, -1, -2], &hopper("south"));
            put([x, 0, length + 1], MUD);
            put([x, -1, length + 1], &hopper("north"));
        }
    }
    // the collector row under z = length, west into the judge's chest
    let east = 5 * modules - 1;
    for x in 1..=east {
        put([x, -1, length], &hopper("west"));
    }
    ports.insert("output".into(), [0, -1, length]);
    let mut meta = BTreeMap::new();
    meta.insert("circuit".to_string(), "harvester sugar_cane".to_string());
    tiled(Design { name: format!("cane-obvious-{}x{}", 5 * modules + 1, length), meta, ports, blocks: b, entities: Vec::new(), block_entities: BTreeMap::new(), ..Default::default() })
}
