//! (onechunk, Nov 11-12; farmopt 22:12/22:22, ironspec's self-contained loaders, tick-exact's code
//! reads; G1's CHANGES 4 on 1c902beb5) L1‴, the item-loop loader, benched in redsim: a portal pair
//! (overworld A, Nether B, both `width` x 3 at x 0.., y 10..12, z 0, axis x: the same axis, so an
//! item's motion carries on through the exit plane). On each side:
//! - a firing dropper D2 in the TOP row's height, one cell off the plane, facing it (tick-exact:
//!   a zombified piglin standing in the bottom row reaches items up to 11.95 = 1.95 over the frame's
//!   obsidian; the items fly at 12.3 and up), fed by a dropper D1 under it facing up;
//! - a hopper floor at y 11 (tops at 12) just past the frame where the other side's items come out
//!   (each side fires from its own column, so they come out clear of its dropper), chained into D1;
//!   the cells (x, 11, near) are all solid, so the exit side's clearance over the bottom row is 1
//!   (a piglin, 1.95 tall, can't leave the portal); a glass wall on the far face;
//! - a glass backstop three cells off the plane over the floor;
//! - a REAL clock: two observers facing each other behind D2 (one's output into D2: D2 and, through
//!   it, D1 fire every 4 gt; an empty dropper only clicks), so items fire as soon as they reach D2.
//!
//! The rows: seeds (the dropper's spread is random; the brief's rule: >= 40 seeds, 0 failures),
//! redsim's level-32 freeze per world (`reload::unload_some`: entities skip, blocks and block
//! entities run), a login landing on every dispense, a full stop (`reload::unload_all` /
//! `load_all`, scheduled ticks kept), a zombified piglin standing in A's bottom row (AI on, loot
//! pickup on, empty hand). Measures: each side's arrivals (B: the Nether's PORTAL tickets; A: a
//! crossing's copy, a new item with a cooldown), the worst gap against 300, items lost, items held
//! in a portal, the landing spread against the floor's and backstop's edges, the flight's clearance
//! to the frame and to a piglin's reach.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use std::collections::{BTreeMap, BTreeSet};

type P = [i32; 3];

/// The loop's item (ironspec 01:36: leather: baby piglins ignore it, armed mobs keep their weapons).
pub const ITEM: &str = "leather";
/// The piglin's reach: its box (1.95) over the frame's bottom obsidian (y 10).
pub const PIGLIN_REACH_Y: f64 = 11.95;
/// A jumping baby's (tick-exact: a Nether baby piglin that crossed and zombified, empty-handed with
/// loot pickup): its 0.975 box at the jump's apex reaches 2.227 over the portal's floor.
pub const PIGLIN_JUMP_REACH_Y: f64 = 12.227;

/// One side: `s` (+1: its dropper and floor at z >= 1, -1: z <= -1), its dropper column, the
/// other side's column (where this side's arrivals come out).
#[derive(Clone, Debug)]
pub struct Side {
    pub s: i32,
    pub col: i32,
    pub other: i32,
    /// The firing dropper's y (12, or 13 in a 4-row portal: items start a row higher), fed from
    /// y 11 by up-facing droppers.
    pub fire_y: i32,
    /// The far face's walled rows from the bottom (2: the bottom two, the rule; the height: all).
    pub far_rows: i32,
    /// The block shielding the portal and the pocket (glass in the overworld; cobblestone in the
    /// Nether: a ghast's power-1 fireball breaks glass, and a portal block has no blast resistance).
    pub shell: &'static str,
}

impl Side {
    pub fn d2(&self) -> P {
        [self.col, self.fire_y, self.s]
    }
    /// The pocket floor's cells' y (its top 1 above): 2 under the firing row (5 rows, fire 14:
    /// floor 12, its top 3.0 over the portal's floor (y 10), out of a jumping baby's reach, 2.227).
    pub fn floor_y(&self) -> i32 {
        self.fire_y - 2
    }
    /// The bottom feeder (the hopper row feeds it).
    pub fn d1(&self) -> P {
        [self.col, self.floor_y(), self.s]
    }
    /// The observer pair: O1 (its output into the feeder under D2) and O2.
    pub fn observers(&self) -> [P; 2] {
        [[self.col, self.fire_y - 1, 2 * self.s], [self.col, self.fire_y - 1, 3 * self.s]]
    }
    /// The floor columns: the other side's column and its two neighbours (in the portal, not D2's).
    pub fn floor_cols(&self, width: i32) -> Vec<i32> {
        ((self.other - 1)..=(self.other + 1)).filter(|x| *x >= 0 && *x < width && *x != self.col).collect()
    }
    fn blocks(&self, width: i32, height: i32, hopper_floor: bool, out: &mut BTreeMap<P, String>, be: &mut BTreeMap<P, String>, items: u32) {
        let glass = "minecraft:glass".to_string();
        let (s, c) = (self.s, self.col);
        let fy = self.floor_y();
        // (the portal's interior rows y 10 .. 9 + height; the frame's top at 10 + height)
        let (top, cap) = (9 + height, 10 + height);
        for y in 10..=top {
            out.insert([-1, y, 0], "minecraft:obsidian".into());
            out.insert([width, y, 0], "minecraft:obsidian".into());
        }
        for x in 0..width {
            out.insert([x, 9, 0], "minecraft:obsidian".into());
            out.insert([x, cap, 0], "minecraft:obsidian".into());
            for y in 10..=top {
                out.insert([x, y, 0], "minecraft:nether_portal[axis=x]".into());
                // the far face walled in its bottom `far_rows` rows: a mob in the bottom rows
                // can't step out that way (nothing leaves the plane there: the side's own items
                // cross at once, the arrivals move to the exit face)
                if y < 10 + self.far_rows {
                    out.insert([x, y, -s], glass.clone());
                }
            }
        }
        let toward = if s > 0 { "north" } else { "south" };
        let away = if s > 0 { "south" } else { "north" };
        out.insert(self.d2(), format!("minecraft:dropper[facing={toward},triggered=false]"));
        // (the feeders: up-facing droppers from the floor's y to under D2; O1 powers the one under
        // D2, which as a conductor powers D2 and the one under it, and O1's front update reaches
        // both on the falling edge)
        for y in fy..self.fire_y {
            out.insert([c, y, s], "minecraft:dropper[facing=up,triggered=false]".into());
        }
        let half = items / 2;
        be.insert(self.d2(), format!("{{Items:[{{Slot:0b,id:\"minecraft:{ITEM}\",count:1}}]}}"));
        if half > 1 {
            be.insert(self.d1(), format!("{{Items:[{{Slot:0b,id:\"minecraft:{ITEM}\",count:{}}}]}}", half - 1));
        }
        let cols = self.floor_cols(width);
        let along = |x: i32| if x > c { "west" } else { "east" };
        // the near row (z s): the exit face closed below the floor (y 10 .. fy - 1 glass) and at
        // the floor (hoppers in the floor columns, glass elsewhere, the feeder in c), so a mob in
        // the bottom rows never leaves; the items come out of the rows over the floor onto it: with
        // `hopper_floor` hoppers along x into the bottom feeder (no spawns on a hopper; an item in
        // the bowl sits at fy + 0.6875, over a jumping baby's reach), else packed ice (the 4-row
        // variant: an item slides on to the hopper row)
        for x in -1..=width {
            for y in 10..fy {
                out.insert([x, y, s], glass.clone());
            }
            if x == c {
                continue;
            }
            if cols.contains(&x) {
                out.insert([x, fy, s], if hopper_floor { format!("minecraft:hopper[enabled=true,facing={}]", along(x)) } else { "minecraft:packed_ice".into() });
            } else {
                out.insert([x, fy, s], glass.clone());
                for y in fy + 1..=top {
                    out.insert([x, y, s], glass.clone());
                }
            }
        }
        // the hopper row (z 2s): with a hopper floor, its hoppers point back into the near row's;
        // else they point along x toward the one in c, which points into the bottom feeder; glass in
        // the other columns at fy and fy + 1 (the row's bounds)
        for x in cols.iter().copied() {
            let facing = if hopper_floor { toward } else { along(x) };
            out.insert([x, fy, 2 * s], format!("minecraft:hopper[enabled=true,facing={facing}]"));
        }
        if hopper_floor {
            out.insert([c, fy, 2 * s], glass.clone());
        } else {
            out.insert([c, fy, 2 * s], format!("minecraft:hopper[enabled=true,facing={toward}]"));
        }
        for x in -1..=width {
            if x == c || cols.contains(&x) {
                continue;
            }
            out.insert([x, fy, 2 * s], glass.clone());
            out.insert([x, fy + 1, 2 * s], glass.clone());
        }
        // the backstop (z 3s, fy + 1 .. cap) over the whole width, O2's cell aside; the enclosure:
        // a glass roof at the cap over the near and hopper rows, walls at x -1 and x width (y 10 ..
        // cap), so nothing walks in and no floor inside takes a spawn (hoppers, glass; the droppers'
        // and observers' tops under the roof)
        let [o1, o2] = self.observers();
        for x in -1..=width {
            for y in fy + 1..=cap {
                if [x, y, 3 * s] == o2 {
                    continue;
                }
                out.insert([x, y, 3 * s], glass.clone());
            }
            for z in [s, 2 * s] {
                out.insert([x, cap, z], glass.clone());
            }
        }
        for z in [s, 2 * s, 3 * s] {
            for y in 10..=cap {
                out.insert([-1, y, z], glass.clone());
                out.insert([width, y, z], glass.clone());
            }
        }
        // the clock: O1 faces O2 (its back, the output, into the feeder under D2), O2 faces O1;
        // glass over O1 (its top, a full face, would take a 1-high spawn under the roof)
        out.insert([c, self.fire_y, 2 * s], "minecraft:glass".into());
        out.insert(o1, format!("minecraft:observer[facing={away},powered=false]"));
        out.insert(o2, format!("minecraft:observer[facing={toward},powered=false]"));
        // O2's two faces on the outside (G1, ironspec 02:31: an observer's 3.0 under a ghast's
        // fireball): its back (its output: the block there is powered with each pulse and powers
        // nothing, no redstone beside it) and its bottom, below the backstop; glass here, the
        // Nether side's outer layer turns both to cobblestone
        out.insert([c, self.fire_y - 1, 4 * s], "minecraft:glass".into());
        out.insert([c, fy, 3 * s], "minecraft:glass".into());
    }
}

pub fn sides(width: i32, height: i32, fire_y: i32, far_rows: i32) -> (Side, Side) {
    // (the Nether side (ironspec 02:02, ghasts): every block shielding the portal or the pocket
    // at least cobblestone's blast resistance, and its far face closed in every row)
    (Side { s: 1, col: 1, other: width - 2, fire_y, far_rows, shell: "minecraft:glass" }, Side { s: -1, col: width - 2, other: 1, fire_y, far_rows: height, shell: "minecraft:cobblestone" })
}

/// The air cells outside a cell map: flooded from its bounding box grown by 1 through cells it
/// doesn't hold (portal blocks count as held).
fn outside_air(cells: &BTreeMap<P, String>) -> BTreeSet<P> {
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in cells.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k] - 1);
            hi[k] = hi[k].max(p[k] + 1);
        }
    }
    let mut seen: BTreeSet<P> = BTreeSet::new();
    let mut stack = vec![lo];
    while let Some(p) = stack.pop() {
        if (0..3).any(|k| p[k] < lo[k] || p[k] > hi[k]) || seen.contains(&p) || cells.contains_key(&p) {
            continue;
        }
        seen.insert(p);
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            stack.push([p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
        }
    }
    seen
}

/// Every glass block with a face on the outside air becomes `shell`.
pub fn harden_outer(cells: &mut BTreeMap<P, String>, shell: &str) {
    let out = outside_air(cells);
    let outer: Vec<P> = cells
        .iter()
        .filter(|(p, s)| s.as_str() == "minecraft:glass" && [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].iter().any(|d| out.contains(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]])))
        .map(|(p, _)| *p)
        .collect();
    for p in outer {
        cells.insert(p, shell.to_string());
    }
}

/// (ironspec 02:14) Every interior air cell (not reachable from the outside air, not a portal cell)
/// over a block a Nether mob can spawn on (a full top that isn't glass, a hopper, a slab or a
/// portal), with its air height: a magma cube (0.52) fits 1, a piglin (1.95) 2, an enderman (2.9) 3.
pub fn interior_spawn_cells(cells: &BTreeMap<P, String>) -> Vec<(P, String, i32)> {
    let out = outside_air(cells);
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in cells.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let unspawnable = |s: &str| s.contains("glass") || s.contains("hopper") || s.contains("_slab") || s.contains("nether_portal") || s.contains("tripwire") || s.contains("rail");
    let mut v = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let p = [x, y, z];
                if cells.contains_key(&p) || out.contains(&p) {
                    continue;
                }
                let Some(below) = cells.get(&[x, y - 1, z]) else { continue };
                if unspawnable(below) {
                    continue;
                }
                let mut h = 0;
                while h < 4 && !cells.contains_key(&[x, y + h, z]) && !out.contains(&[x, y + h, z]) {
                    h += 1;
                }
                v.push((p, below.clone(), h));
            }
        }
    }
    v
}

/// A block's blast resistance (26.3's values for the blocks these files use; anything else counts
/// as 0, the weak side).
pub fn blast_resistance(state: &str) -> f64 {
    let n = state.trim_start_matches("minecraft:");
    let n = n.split('[').next().unwrap_or(n);
    match n {
        "obsidian" | "crying_obsidian" => 1200.0,
        "cobblestone" | "stone" | "smooth_stone_slab" | "stone_bricks" | "cobblestone_slab" | "stone_slab" => 6.0,
        "hopper" => 4.8,
        "dropper" | "dispenser" => 3.5,
        "observer" => 3.0,
        "glass" | "glass_pane" => 0.3,
        "packed_ice" | "ice" => 0.5,
        _ => 0.0,
    }
}

/// The 26 neighbour offsets (faces, edges and corners).
fn n26() -> Vec<P> {
    let mut v = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if (dx, dy, dz) != (0, 0, 0) {
                    v.push([dx, dy, dz]);
                }
            }
        }
    }
    v
}

/// (G1 / ironspec 02:31; G1 03:35: a power-1 ray samples every 0.3 blocks, so it crosses a cell
/// edge or corner without sampling either face neighbour) Every loop block a ray from the outside
/// reaches: a 26-connected flood from the outside air through air and every block under 4.034,
/// listing the observers (3.0) and portal blocks (no resistance) it meets; a dropper (3.5) costs a
/// ray's second sample more than it has left (G1), so it isn't listed.
pub fn exposed_loop_blocks(cells: &BTreeMap<P, String>) -> Vec<(P, String)> {
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in cells.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k] - 1);
            hi[k] = hi[k].max(p[k] + 1);
        }
    }
    let weak = |p: &P| cells.get(p).is_none_or(|s| blast_resistance(s) < 4.034);
    let nb = n26();
    let mut seen: BTreeSet<P> = BTreeSet::new();
    let mut stack = vec![lo];
    while let Some(p) = stack.pop() {
        if (0..3).any(|k| p[k] < lo[k] || p[k] > hi[k]) || seen.contains(&p) || !weak(&p) {
            continue;
        }
        seen.insert(p);
        for d in &nb {
            stack.push([p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
        }
    }
    seen.iter()
        .filter_map(|p| cells.get(p).filter(|s| s.contains("observer") || s.contains("nether_portal")).map(|s| (*p, s.clone())))
        .collect()
}

/// (G1 03:35) Close the casing's 26-neighbourhood: while a 26-connected ray flood from the outside
/// reaches an observer or a portal block, every outside air cell (6-connected) with an edge or a
/// corner on a weak cell inside the casing (under 4.034, air included) becomes `shell`. The cells
/// added.
pub fn close_26(cells: &mut BTreeMap<P, String>, shell: &str) -> Vec<P> {
    let mut added = Vec::new();
    let nb = n26();
    for _ in 0..8 {
        if exposed_loop_blocks(cells).is_empty() {
            break;
        }
        let out = outside_air(cells);
        let inside_weak = |p: &P| !out.contains(p) && cells.get(p).is_none_or(|s| blast_resistance(s) < 4.034);
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for p in cells.keys() {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let inside_box = |p: &P| (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]);
        let fill: Vec<P> = out
            .iter()
            .filter(|p| nb.iter().any(|d| {
                let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
                inside_box(&q) && inside_weak(&q)
            }))
            .copied()
            .collect();
        if fill.is_empty() {
            break;
        }
        for p in fill {
            cells.insert(p, shell.to_string());
            added.push(p);
        }
    }
    added
}

pub fn design_text(width: i32, height: i32, fire_y: i32, far_rows: i32, hopper_floor: bool, items: u32) -> Result<String, String> {
    let (a, b) = sides(width, height, fire_y, far_rows);
    let mut d = Design { name: "l1-item-loop-bench".into(), ..Default::default() };
    a.blocks(width, height, hopper_floor, &mut d.blocks, &mut d.block_entities, items);
    b.blocks(width, height, hopper_floor, &mut d.nether_blocks, &mut d.nether_block_entities, items);
    // (the Nether side's outer layer, ironspec 02:14: every glass block facing the open Nether
    // becomes the shell's cobblestone; a power-1 ray loses 1.89 a step through it, so the
    // interior stays glass)
    harden_outer(&mut d.nether_blocks, b.shell);
    // (G1 03:35: the casing closed in the 26-neighbourhood: the edges and corners a ray crosses)
    let closed = close_26(&mut d.nether_blocks, b.shell);
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    d.ports.insert("output".into(), [width + 6, 10, 8]);
    d.ports.insert("nether-anchor".into(), [0, 0, 0]);
    d.ports.insert("nether-axis".into(), [1, 0, 0]);
    let mut t = format!("# L1''' item-loop loader bench (onechunk, Nov 12): portals {width} x {height}; on each side a firing dropper at y {fire_y} (the top row) fed by up-facing droppers, the exit face closed below and at the pocket floor (its top at y {}), the floor {} and a hopper row past the frame, a backstop and an enclosure; the overworld side glass, its far face closed in its bottom {far_rows} rows; the Nether side's outer layer cobblestone (ghasts), its far face closed in every row, its interior glass; the clock: two observers facing each other behind the firing column (REAL, not emulated); {items} items ({ITEM})\n", fire_y - 1, if hopper_floor { "hoppers" } else { "packed ice" });
    for (wn, cells) in [("overworld", &d.blocks), ("Nether", &d.nether_blocks)] {
        let sp = interior_spawn_cells(cells);
        t.push_str(&format!("# {wn} side: interior spawn floors (a full top under air inside the enclosure): {}{}\n", sp.len(), sp.iter().map(|(p, b, h)| format!(" ({},{},{}) over {} with {h} air", p[0], p[1], p[2], b.trim_start_matches("minecraft:"))).collect::<String>()));
        if wn == "Nether" {
            let ex = exposed_loop_blocks(cells);
            t.push_str(&format!("# Nether side: the casing closed in the 26-neighbourhood (G1): {} cobblestone added at its edges and corners{}\n", closed.len(), closed.iter().map(|p| format!(" ({},{},{})", p[0], p[1], p[2])).collect::<String>()));
            t.push_str(&format!("# Nether side: loop blocks (observers, portal blocks) a ray from the open Nether reaches (a 26-connected flood through blocks under 4.034): {}{}\n", ex.len(), ex.iter().map(|(p, b)| format!(" ({},{},{}) {}", p[0], p[1], p[2], b.trim_start_matches("minecraft:"))).collect::<String>()));
        }
    }
    t.push_str(&d.to_text());
    // (ironspec 02:32: every design writer re-parses what it wrote and compares every cell)
    let back = Design::parse(&t).map_err(|e| format!("the written design doesn't parse: {e}"))?;
    let same = back.blocks == d.blocks && back.nether_blocks == d.nether_blocks && back.block_entities == d.block_entities && back.nether_block_entities == d.nether_block_entities && back.ports == d.ports;
    if !same {
        return Err(format!(
            "the written design doesn't round-trip: overworld {} vs {} cells, Nether {} vs {}, block entities {} vs {}, Nether block entities {} vs {}, ports {} vs {}",
            back.blocks.len(),
            d.blocks.len(),
            back.nether_blocks.len(),
            d.nether_blocks.len(),
            back.block_entities.len(),
            d.block_entities.len(),
            back.nether_block_entities.len(),
            d.nether_block_entities.len(),
            back.ports.len(),
            d.ports.len()
        ));
    }
    Ok(t)
}

/// (ironspec 02:14) The interior spawn floors of both sides, as `design_text` lays them.
pub fn spawn_report(width: i32, height: i32, fire_y: i32, far_rows: i32, hopper_floor: bool) -> String {
    let (a, b) = sides(width, height, fire_y, far_rows);
    let (mut ow, mut ne, mut be) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    a.blocks(width, height, hopper_floor, &mut ow, &mut be, 4);
    b.blocks(width, height, hopper_floor, &mut ne, &mut be, 4);
    harden_outer(&mut ne, b.shell);
    let closed = close_26(&mut ne, b.shell);
    let mut s = String::new();
    for (wn, cells) in [("overworld", &ow), ("Nether", &ne)] {
        let sp = interior_spawn_cells(cells);
        let shell = cells.values().filter(|v| v.as_str() == "minecraft:cobblestone").count();
        s.push_str(&format!("{wn} side: {} placed blocks ({} cobblestone, the outer layer); interior spawn floors (a full top under air inside the enclosure; a magma cube fits 1 air, a piglin 2, an enderman 3): {}{}{}\n", cells.values().filter(|v| !v.contains("nether_portal")).count(), shell, sp.len(), if sp.is_empty() { " (PASS)" } else { " (FAIL)" }, sp.iter().map(|(p, b, h)| format!(" ({},{},{}) over {} with {h} air", p[0], p[1], p[2], b.trim_start_matches("minecraft:"))).collect::<String>()));
        // (the ghast rule binds the Nether side; the overworld's glass is printed for the record)
        let ex = exposed_loop_blocks(cells);
        if wn == "Nether" {
            s.push_str(&format!("Nether side: the casing closed in the 26-neighbourhood (G1: a ray crosses edges and corners): {} cobblestone added at its edges and corners\n", closed.len()));
        }
        s.push_str(&format!(
            "{wn} side: loop blocks (observers 3.0, portal blocks; a dropper's 3.5 outlasts a ray's second sample) a ray from the outside reaches, a 26-connected flood through blocks under 4.034{}: {}{}\n",
            if wn == "Nether" { "" } else { " (glass: not ruled on this side)" },
            ex.len(),
            if wn != "Nether" { String::new() } else if ex.is_empty() { " (PASS)".into() } else { format!(" (FAIL){}", ex.iter().map(|(p, b)| format!(" ({},{},{}) {}", p[0], p[1], p[2], b.trim_start_matches("minecraft:").split('[').next().unwrap_or(""))).collect::<String>()) }
        ));
    }
    s
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub width: i32,
    /// The portals' interior rows (3 or 4).
    pub height: i32,
    /// The firing dropper's y (12, or 13 with 4 rows).
    pub fire_y: i32,
    /// The far face's walled rows from the bottom.
    pub far_rows: i32,
    /// The pocket floor: hoppers (the ruling: no spawnable floor) or packed ice.
    pub hopper_floor: bool,
    pub items: u32,
    pub ticks: i64,
    /// (from, len, worlds: 1 overworld, 2 Nether, 3 both): redsim's level-32 freeze.
    pub freezes: Vec<(i64, i64, u8)>,
    /// A 3-gt freeze of both worlds every N gt.
    pub login_each: Option<i64>,
    /// A 3-gt freeze of both worlds starting at every dispense (the item crosses after the thaw).
    pub login_before_arrival: bool,
    /// A restart at T: `liveness::restart` on both worlds between two ticks (no time passing).
    pub stops: Vec<i64>,
    /// The restarts' entity load order (liveness::Restart: save, reverse, seed:K).
    pub restart: crate::liveness::Restart,
    /// A mob placed in a bottom row at column X at time T (AI on), on side bits (1 A, 2 B, 3 both),
    /// kind 0 an adult zombified piglin, 1 a baby, 2 a chicken jockey.
    pub piglins: Vec<(i64, i32, u8, u8)>,
    /// Every N gt each mob on the ground jumps (its box toward the jump's reach).
    pub mob_jump: Option<i64>,
    pub seeds: Vec<i64>,
    pub trace: bool,
}

type Ent = (u32, [f64; 3], [f64; 3], i32);

fn items_in(w: &redsim::world::World) -> (u32, Vec<Ent>) {
    use redsim::redstone::container as rc;
    let id = rc::item_id(ITEM);
    let inv: u32 = w.containers.map.values().map(|c| c.slots.iter().flatten().filter(|s| Some(s.item) == id).map(|s| s.count as u32).sum::<u32>()).sum();
    let mut n = 0u32;
    let mut ents: Vec<Ent> = Vec::new();
    for e in &w.entities.list {
        if e.removed {
            continue;
        }
        match &e.kind {
            EntityKind::Item(s) if s.item.trim_start_matches("minecraft:") == ITEM => {
                n += s.count as u32;
                ents.push((e.id, [e.pos.x, e.pos.y, e.pos.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z], e.portal_cooldown));
            }
            _ => {}
        }
    }
    // (an item in a mob's hand counts too: a piglin that took one)
    (inv + n + held_in(w), ents)
}

/// The loop's items held in mobs' main hands (a piglin with loot pickup takes one for good).
fn held_in(w: &redsim::world::World) -> u32 {
    (0..w.entities.list.len())
        .filter(|&i| !w.entities.list[i].removed && matches!(w.entities.list[i].kind, EntityKind::Mob(_)))
        .map(|i| redsim::entity::mob::mob(w, i).mainhand.as_ref().filter(|h| h.item.trim_start_matches("minecraft:") == ITEM).map_or(0, |h| h.count as u32))
        .sum()
}

/// One seed's run.
#[derive(Clone, Debug, Default)]
pub struct RunOut {
    pub seed: i64,
    pub arrivals: [usize; 2],
    pub worst_gap: [i64; 2],
    pub over_300: [usize; 2],
    pub lost: u32,
    pub held_by_mob: u32,
    /// Mobs whose centre left the portal's cells (a placed mob must stay in the bottom rows).
    pub mobs_out: usize,
    /// The placed mobs' highest box top over the portal's floor (y 10): adults (box >= 1.5 tall),
    /// babies on foot, and the jockey stack (a rider or its chicken): the jumps' own control.
    pub mob_top: [f64; 3],
    pub stuck: usize,
    pub nether_dead: i64,
    pub nether_dead_from: Option<i64>,
    /// Landing spread per side: x min, x max, |z| min, |z| max over every arrival's positions
    /// outside the plane until a hopper took it.
    pub spread: [[f64; 4]; 2],
    /// The flight's least clearance per side: to the frame's sides (x), to the top frame (y 13),
    /// and to a piglin's reach (11.95) while within 2.3 of the plane on the arrival side.
    pub clear: [[f64; 4]; 2],
    /// Each arrival's largest drift from its column's centre outside the plane, per side: the
    /// arrivals tracked, past 0.375 (a 1-cell pocket's edge), past 0.875, at a wall (1.37+).
    pub drift: [[usize; 4]; 2],
    pub drift_max: [f64; 2],
    /// At the face crossing (the first tick an arrival's box reaches the exit face; G1: a 1-cell
    /// pocket loses it if its box meets the glass beside the opening then): per side, the arrivals
    /// seen crossing, those past 0.375 from the column's centre, the largest offset, and the
    /// flight time from the arrival (min, max, sum).
    pub face: [(usize, usize, f64, i64, i64, i64); 2],
    pub errors: Vec<String>,
    pub log: Vec<String>,
    pub stuck_log: Vec<String>,
    pub control: Vec<String>,
}

fn world_mut(run: &mut Run, wi: usize) -> &mut redsim::world::World {
    if wi == 0 { &mut run.world } else { run.world.nether.as_deref_mut().unwrap() }
}

pub fn run_one(spec: &FarmSpec, o: &Opts, seed: i64) -> Result<RunOut, String> {
    let ai = !o.piglins.is_empty();
    let ro = RunOpts { ai, ..RunOpts::steady(seed, (o.ticks + 100) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let width = o.width;
    // (a dropper's item draws its motion from the level random and its own random from the drop
    // seed: both worlds get them, seeded from the run's seed; the forced box grown by a ring of
    // chunks so a partial unload of the bench's chunks leaves them at level 32)
    for wi in 0..2 {
        let w = world_mut(&mut run, wi);
        let k = if wi == 0 { 0 } else { 0x4e45 };
        w.level_random = Some(redsim::rng::LegacyRandom::new(seed ^ k));
        w.drop_seed = Some(seed ^ k);
        w.entity_chunks = Some([-2, -2, 1, 1]);
    }
    let (a, b) = sides(width, o.height, o.fire_y, o.far_rows);
    let air = Block::parse("minecraft:air")?;
    // the clocks' start: O2 taken and put back (O1 sees it; in the build, O2 placed last)
    for (wi, side) in [(0usize, &a), (1usize, &b)] {
        let w = world_mut(&mut run, wi);
        let o2 = side.observers()[1];
        let p = BlockPos::new(o2[0], o2[1], o2[2]);
        let st = w.get(p).clone();
        w.set_block(p, air.clone(), redsim::world::flags::ALL);
        w.set_block(p, st, redsim::world::flags::ALL);
    }
    let start = run.world.time;
    let mut out = RunOut { seed, spread: [[f64::MAX, f64::MIN, f64::MAX, f64::MIN]; 2], clear: [[f64::MAX; 4]; 2], ..Default::default() };
    let mut seen: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    let mut arrivals: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    let mut arriving: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    let mut inside_since: BTreeMap<(usize, u32), i64> = BTreeMap::new();
    let mut last_ticket: i64 = i64::MIN;
    let mut frozen = [false; 2];
    let mut login_until: i64 = i64::MIN;
    let mut min_count = u32::MAX;
    let mut mobs_out: BTreeSet<(usize, u32)> = BTreeSet::new();
    let mut drift: BTreeMap<(usize, u32), f64> = BTreeMap::new();
    let mut arrived_at: BTreeMap<(usize, u32), i64> = BTreeMap::new();
    let mut crossed: BTreeSet<(usize, u32)> = BTreeSet::new();
    for _ in 0..o.ticks {
        let t = run.world.time;
        let rel = t - start;
        // the rows' acts
        for &(pt, x, side, kind) in &o.piglins {
            if rel == pt {
                for wi in 0..2usize {
                    if side & (1 << wi) == 0 {
                        continue;
                    }
                    let w = world_mut(&mut run, wi);
                    let at = [x as f64 + 0.5, 10.0, 0.5];
                    let loot = Some("{CanPickUpLoot:1b,PortalCooldown:300}");
                    let armed = kind & 4 != 0;
                    let (id, what) = match kind & 3 {
                        1 => (redsim::entity::spawn(w, "baby_zombified_piglin", at, loot)?, "a baby zombified piglin"),
                        2 => {
                            let ch = redsim::entity::spawn(w, "minecraft:chicken", at, Some("{IsChickenJockey:1b,PortalCooldown:300}"))?;
                            let rider = redsim::entity::spawn(w, "baby_zombified_piglin", at, loot)?;
                            redsim::entity::mob::mount(w, rider, ch)?;
                            (rider, "a chicken jockey (a baby zombified piglin on a chicken)")
                        }
                        _ => (redsim::entity::spawn(w, "minecraft:zombified_piglin", at, loot)?, "an adult zombified piglin"),
                    };
                    // (armed: as a portal piglin spawns, a golden sword in its main hand
                    // (populateDefaultEquipmentSlots), loot pickup forced on)
                    if armed {
                        redsim::entity::mob::set_hand(w, id, true, Some(("minecraft:golden_sword", false)), &[]);
                    }
                    // (the positive control at its feet, inside its reach: an empty-handed mob must
                    // take a stone; an armed one an iron sword (a better weapon): the pickup is
                    // modeled and in reach)
                    let ctl = if armed { "iron_sword" } else { "stone" };
                    redsim::entity::spawn(w, "minecraft:item", [x as f64 + 0.5, 10.1, 0.5], Some(&format!("{{Item:{{id:\"minecraft:{ctl}\",count:1}},PickupDelay:0}}")))?;
                    out.control.push(format!("t {rel}: {what} (id {id}, loot pickup on, {}) placed in {}'s bottom row at column {x}, a {ctl} item at its feet (the control)", if armed { "a golden sword in hand" } else { "empty hand" }, if wi == 0 { "A" } else { "B" }));
                }
            }
        }
        // (forced jumps: every `mob_jump` gt each mob on the ground jumps, its box lifted toward the
        // jump's reach)
        if let Some(n) = o.mob_jump
            && rel % n == 0
        {
            for wi in 0..2usize {
                let w = world_mut(&mut run, wi);
                for e in w.entities.list.iter_mut().filter(|e| !e.removed && e.on_ground && matches!(e.kind, EntityKind::Mob(_))) {
                    e.delta_movement.y = 0.42;
                }
            }
        }
        for &st in &o.stops {
            if rel == st {
                // (G1 on 0596acead: liveness::restart, cc767b78's helper: a save/stop/start
                // between two ticks in the run's load order, no time passing, on both worlds;
                // every entity back under a new id, so the trackers follow the id map)
                for wi in 0..2usize {
                    let m = crate::liveness::restart(world_mut(&mut run, wi), o.restart)?;
                    let tr = |id: &u32| m.get(id).copied();
                    seen[wi] = seen[wi].iter().filter_map(tr).collect();
                    arriving[wi] = arriving[wi].iter().filter_map(tr).collect();
                    inside_since = inside_since.iter().map(|(&(w, id), &v)| ((w, if w == wi { tr(&id).unwrap_or(id) } else { id }), v)).collect();
                    mobs_out = mobs_out.iter().map(|&(w, id)| (w, if w == wi { tr(&id).unwrap_or(id) } else { id })).collect();
                    drift = drift.iter().map(|(&(w, id), &v)| ((w, if w == wi { tr(&id).unwrap_or(id) } else { id }), v)).collect();
                    arrived_at = arrived_at.iter().map(|(&(w, id), &v)| ((w, if w == wi { tr(&id).unwrap_or(id) } else { id }), v)).collect();
                    crossed = crossed.iter().map(|&(w, id)| (w, if w == wi { tr(&id).unwrap_or(id) } else { id })).collect();
                }
            }
        }
        let mut want = [false; 2];
        for &(f, d, wsel) in &o.freezes {
            if rel >= f && rel < f + d {
                if wsel & 1 != 0 {
                    want[0] = true;
                }
                if wsel & 2 != 0 {
                    want[1] = true;
                }
            }
        }
        if let Some(n) = o.login_each
            && rel > 0
            && rel % n < 3
        {
            want = [true, true];
        }
        if t < login_until {
            want = [true, true];
        }
        for wi in 0..2 {
            if want[wi] != frozen[wi] {
                let w = world_mut(&mut run, wi);
                if want[wi] {
                    redsim::reload::unload_some(w, [-1, -1, 0, 0]);
                } else {
                    redsim::reload::load_some(w, [-1, -1, 0, 0]);
                }
                frozen[wi] = want[wi];
            }
        }
        // the measures (before the tick)
        for wi in 0..2usize {
            let w: &redsim::world::World = if wi == 0 { &run.world } else { run.world.nether.as_deref().unwrap() };
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Mob(_))) {
                // (the jumps' own control: each placed mob's highest box top over the portal's
                // floor: adults (a box 1.5 tall or more), babies on foot, and the jockey stack (a
                // rider or its chicken) apart)
                let tall = e.bb.max_y - e.bb.min_y;
                let k = if tall >= 1.5 {
                    0
                } else if e.vehicle.is_some() || !e.passengers.is_empty() {
                    2
                } else {
                    1
                };
                out.mob_top[k] = out.mob_top[k].max(e.bb.max_y - 10.0);
                let d = (-e.pos.x).max(e.pos.x - width as f64).max(-e.pos.z).max(e.pos.z - 1.0).max(0.0);
                if d > 0.05 && !mobs_out.contains(&(wi, e.id)) {
                    mobs_out.insert((wi, e.id));
                    out.control.push(format!("t {rel}: MOB OUT in {}: id {} at ({:.2},{:.2},{:.2}), {:.2} off the portal's cells", if wi == 0 { "A" } else { "B" }, e.id, e.pos.x, e.pos.y, e.pos.z, d));
                }
            }
        }
        for (wi, side) in [(0usize, &a), (1usize, &b)] {
            let w: &redsim::world::World = if wi == 0 { &run.world } else { run.world.nether.as_deref().unwrap() };
            let (_, ents) = items_in(w);
            let present: BTreeSet<u32> = ents.iter().map(|e| e.0).collect();
            seen[wi].retain(|id| present.contains(id));
            arriving[wi].retain(|id| present.contains(id));
            for e in &ents {
                if !seen[wi].contains(&e.0) {
                    seen[wi].insert(e.0);
                    if e.3 > 0 {
                        // a crossing's copy
                        arriving[wi].insert(e.0);
                        arrived_at.insert((wi, e.0), t);
                        if wi == 0 {
                            arrivals[0].push(rel);
                        }
                        if o.trace && out.log.len() < 60 {
                            out.log.push(format!("t {rel}: arrival in {} (item {}) at ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3})", if wi == 0 { "A" } else { "B" }, e.0, e.1[0], e.1[1], e.1[2], e.2[0], e.2[1], e.2[2]));
                        }
                    } else if o.login_before_arrival && t >= login_until {
                        // a dispense: the login lands on it (the item crosses after the thaw)
                        login_until = t + 3;
                    }
                }
                let inside = e.1[0] > -0.13 && e.1[0] < width as f64 + 0.13 && e.1[2] > -0.13 && e.1[2] < 1.13 && e.1[1] > 9.9 && e.1[1] < (10 + o.height) as f64;
                if inside {
                    let since = *inside_since.entry((wi, e.0)).or_insert(t);
                    if t - since == 40 {
                        out.stuck += 1;
                        out.stuck_log.push(format!("t {rel}: item {} inside {}'s portal 40 gt at ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3}), cooldown {}", e.0, if wi == 0 { "A" } else { "B" }, e.1[0], e.1[1], e.1[2], e.2[0], e.2[1], e.2[2], e.3));
                    }
                } else {
                    inside_since.remove(&(wi, e.0));
                }
                if arriving[wi].contains(&e.0) {
                    // the face crossing: the first tick its box reaches the exit face (A's at z 1,
                    // B's at z 0)
                    let at_face = if side.s > 0 { e.1[2] + 0.125 >= 1.0 } else { e.1[2] - 0.125 <= 0.0 };
                    if at_face && !crossed.contains(&(wi, e.0)) {
                        crossed.insert((wi, e.0));
                        let off = (e.1[0] - (side.other as f64 + 0.5)).abs();
                        let ft = t - arrived_at.get(&(wi, e.0)).copied().unwrap_or(t);
                        let f = &mut out.face[wi];
                        f.0 += 1;
                        if off > 0.375 {
                            f.1 += 1;
                        }
                        f.2 = f.2.max(off);
                        f.3 = if f.0 == 1 { ft } else { f.3.min(ft) };
                        f.4 = f.4.max(ft);
                        f.5 += ft;
                    }
                    let az = e.1[2] * side.s as f64;
                    let c = &mut out.clear[wi];
                    // in the plane (its box within z 0..1): the frame's sides and top
                    if e.1[2] > -0.125 && e.1[2] < 1.125 {
                        c[0] = c[0].min((e.1[0] - 0.125).min(width as f64 - (e.1[0] + 0.125)));
                        c[1] = c[1].min((10 + o.height) as f64 - (e.1[1] + 0.25));
                    }
                    // a mob standing anywhere in the bottom row (its box anywhere in the cell's
                    // depth z 0..1, 1.95 tall) takes items inside its box inflated 1 on x and z (0
                    // on y): an item whose box (half 0.125) meets z -1..2, against y 11.95; a
                    // jumping one's box reaches ~13.2 (the jump's 1.25 over the floor)
                    if e.1[2] > -1.125 && e.1[2] < 2.125 {
                        c[2] = c[2].min(e.1[1] - PIGLIN_REACH_Y);
                        c[3] = c[3].min(e.1[1] - PIGLIN_JUMP_REACH_Y);
                    }
                    if az > 1.125 {
                        let sp = &mut out.spread[wi];
                        sp[0] = sp[0].min(e.1[0]);
                        sp[1] = sp[1].max(e.1[0]);
                        sp[2] = sp[2].min(az);
                        sp[3] = sp[3].max(az);
                        let d = (e.1[0] - (side.other as f64 + 0.5)).abs();
                        let m = drift.entry((wi, e.0)).or_insert(0.0);
                        *m = m.max(d);
                    }
                }
            }
        }
        run.tick();
        // B's arrivals: the Nether's PORTAL tickets (each crossing into it places one)
        let n = run.world.nether.as_deref().unwrap();
        if let Some(m) = n.portal_tickets.iter().map(|x| x.1).max()
            && m > last_ticket
        {
            last_ticket = m;
            arrivals[1].push(rel);
        }
        if n.portal_tickets.is_empty() && !arrivals[1].is_empty() {
            out.nether_dead += 1;
            out.nether_dead_from.get_or_insert(rel);
        }
        // (the count skips a tick with a world unloaded: its entities are saved away, not lost)
        if run.world.away.is_none() && n.away.is_none() {
            let total = items_in(&run.world).0 + items_in(n).0;
            min_count = min_count.min(total);
        }
    }
    let n = run.world.nether.as_deref().unwrap();
    let end_total = items_in(&run.world).0 + items_in(n).0;
    out.held_by_mob = held_in(&run.world) + held_in(n);
    out.mobs_out = mobs_out.len();
    for ((wi, _), d) in &drift {
        let k = &mut out.drift[*wi];
        k[0] += 1;
        if *d > 0.375 {
            k[1] += 1;
        }
        if *d > 0.875 {
            k[2] += 1;
        }
        if *d >= 1.37 {
            k[3] += 1;
        }
        out.drift_max[*wi] = out.drift_max[*wi].max(*d);
    }
    if !o.piglins.is_empty() {
        let mut piglins = 0;
        let mut with_stone = 0;
        for w in [&run.world, n] {
            for i in 0..w.entities.list.len() {
                if w.entities.list[i].removed || !matches!(w.entities.list[i].kind, EntityKind::Mob(_)) {
                    continue;
                }
                piglins += 1;
                if redsim::entity::mob::mob(w, i).mainhand.as_ref().is_some_and(|h| matches!(h.item.trim_start_matches("minecraft:"), "stone" | "iron_sword")) {
                    with_stone += 1;
                }
            }
        }
        out.control.push(format!("the control at the end: {with_stone} of {piglins} mobs hold the control item placed at their feet (a stone, or an iron sword for an armed one: the pickup modeled{}); {} hold a loop item", if with_stone > 0 { "" } else { ": NOT SEEN" }, out.held_by_mob));
    }
    // (lost: gone from the loop, an item a mob holds included)
    out.lost = o.items.saturating_sub(end_total.min(min_count)) + out.held_by_mob;
    for wi in 0..2 {
        let ar = &arrivals[wi];
        out.arrivals[wi] = ar.len();
        let mut gaps: Vec<i64> = ar.windows(2).map(|w| w[1] - w[0]).collect();
        if let Some(l) = ar.last() {
            gaps.push(o.ticks - 1 - l);
        }
        out.worst_gap[wi] = gaps.iter().copied().max().unwrap_or(o.ticks);
        out.over_300[wi] = gaps.iter().filter(|g| **g > 300).count();
    }
    for (wn, w) in [("overworld", &run.world), ("Nether", n)] {
        for e in &w.errors {
            out.errors.push(format!("{wn}: {e}"));
        }
    }
    Ok(out)
}

pub fn failed(r: &RunOut) -> bool {
    r.lost > 0 || r.mobs_out > 0 || r.stuck > 0 || r.nether_dead > 0 || r.over_300[0] > 0 || r.over_300[1] > 0 || r.arrivals[0] < 3 || r.arrivals[1] < 3 || !r.errors.is_empty()
}

/// The Wilson score interval (95%) of `k` failures in `n` runs.
pub fn wilson(k: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let (z, nf) = (1.96f64, n as f64);
    let p = k as f64 / nf;
    let den = 1.0 + z * z / nf;
    let c = (p + z * z / (2.0 * nf)) / den;
    let h = z * (p * (1.0 - p) / nf + z * z / (4.0 * nf * nf)).sqrt() / den;
    ((c - h).max(0.0), (c + h).min(1.0))
}

pub fn bench(spec: &FarmSpec, o: &Opts) -> Result<String, String> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8).min(o.seeds.len().max(1));
    let chunks: Vec<Vec<i64>> = (0..threads).map(|k| o.seeds.iter().copied().skip(k).step_by(threads).collect()).collect();
    let results: Vec<Result<RunOut, String>> = std::thread::scope(|sc| {
        let hs: Vec<_> = chunks.iter().map(|ch| sc.spawn(move || ch.iter().map(|&s| run_one(spec, o, s)).collect::<Vec<_>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap_or_else(|_| vec![Err("a bench thread panicked".to_string())])).collect()
    });
    let mut runs: Vec<RunOut> = Vec::new();
    for r in results {
        runs.push(r?);
    }
    runs.sort_by_key(|r| r.seed);
    let mut s = format!(
        "l1-item-bench (L1''' item loop, redsim; bench, tool unreviewed): portals {} x {}, items fired at y 12.34 (above a bottom-row mob's reach), the exit faces' bottom two rows closed, {} items, the clocks REAL (an observer pair behind each firing dropper), {} gt a seed, {} seeds; the droppers' item motion from the level random as the game draws it; crossings, the cooldown restart, the closest-portal search and the Nether's PORTAL ticket as redsim models them\n",
        o.width, o.height, o.items, o.ticks, runs.len()
    );
    let mut rows: Vec<String> = Vec::new();
    if !o.freezes.is_empty() {
        rows.push(format!("{} level-32 freezes", o.freezes.len()));
    }
    if let Some(n) = o.login_each {
        rows.push(format!("a 3-gt login freeze (both worlds) every {n} gt"));
    }
    if o.login_before_arrival {
        rows.push("a 3-gt login freeze (both worlds) on every dispense".into());
    }
    if !o.stops.is_empty() {
        rows.push(format!("{} restarts at every phase (liveness::restart on both worlds between two ticks, order {})", o.stops.len(), o.restart.label()));
    }
    if !o.piglins.is_empty() {
        rows.push(format!("{} zombified piglin placements in the bottom rows (AI on, loot pickup on, empty hand, a stone at each one's feet as the control)", o.piglins.len()));
    }
    if !rows.is_empty() {
        s.push_str(&format!("rows: {}\n", rows.join("; ")));
    }
    let fails: Vec<&RunOut> = runs.iter().filter(|r| failed(r)).collect();
    let (lo, hi) = wilson(fails.len(), runs.len());
    if !o.piglins.is_empty() {
        let unseen = runs.iter().filter(|r| r.control.iter().any(|l| l.contains("NOT SEEN"))).count();
        s.push_str(&format!("the piglins' positive control (a stone at each one's feet, taken): {} of {} seeds{}\n", runs.len() - unseen, runs.len(), if unseen == 0 { " (PASS: the pickup is modeled, in reach)" } else { " (FAIL: NOT SEEN in some seeds)" }));
    }
    s.push_str(&format!("failures: {} of {} seeds (a failure: an item lost or held by a mob, a mob out of the portal's cells, an item held in a portal 40 gt, the Nether's ticket lapsing, a gap over 300 on either side, or a model error); failure rate {:.3}, Wilson 95% [{:.3}, {:.3}]{}\n", fails.len(), runs.len(), fails.len() as f64 / runs.len().max(1) as f64, lo, hi, if fails.is_empty() { " (PASS)" } else { " (FAIL)" }));
    for (wi, name) in [(0usize, "A (overworld; arrivals: a crossing's copy)"), (1usize, "B (Nether; arrivals: its PORTAL tickets)")] {
        let worst = runs.iter().map(|r| r.worst_gap[wi]).max().unwrap_or(0);
        let arr: Vec<usize> = runs.iter().map(|r| r.arrivals[wi]).collect();
        let sp = runs.iter().fold([f64::MAX, f64::MIN, f64::MAX, f64::MIN], |a, r| [a[0].min(r.spread[wi][0]), a[1].max(r.spread[wi][1]), a[2].min(r.spread[wi][2]), a[3].max(r.spread[wi][3])]);
        let cl = runs.iter().fold([f64::MAX; 4], |a, r| [a[0].min(r.clear[wi][0]), a[1].min(r.clear[wi][1]), a[2].min(r.clear[wi][2]), a[3].min(r.clear[wi][3])]);
        let dr = runs.iter().fold([0usize; 4], |a, r| [a[0] + r.drift[wi][0], a[1] + r.drift[wi][1], a[2] + r.drift[wi][2], a[3] + r.drift[wi][3]]);
        let dmax = runs.iter().map(|r| r.drift_max[wi]).fold(0.0f64, f64::max);
        let side = if wi == 0 { sides(o.width, o.height, o.fire_y, o.far_rows).0 } else { sides(o.width, o.height, o.fire_y, o.far_rows).1 };
        let (fx0, fx1) = ((side.other - 1).max(0) as f64, ((side.other + 1).min(o.width - 1) + 1) as f64);
        let pct = |k: usize| if dr[0] == 0 { 0.0 } else { 100.0 * k as f64 / dr[0] as f64 };
        s.push_str(&format!(
            "side {name}: arrivals a seed {}..{}; worst gap {} gt (margin {} to 300)\n  landing spread x {:.2}..{:.2}, |z| {:.2}..{:.2} against the floor x {:.0}..{:.0} (margins {:.2} / {:.2}) and the backstop at |z| 3 (margin {:.2}); the flight's least clearance: frame sides {:.3}, top frame {:.3}, a standing bottom-row mob's reach (11.95) {:.3}, a jumping baby's (12.227) {:.3}\n  each arrival's largest drift from its column's centre (x {}.5) outside the plane: {} arrivals; past 0.375 (a 1-cell pocket's edge) {} ({:.2}%), past 0.875 {} ({:.2}%), at a wall (1.37+) {} ({:.2}%); the largest {:.3}\n",
            arr.iter().min().unwrap_or(&0),
            arr.iter().max().unwrap_or(&0),
            worst,
            300 - worst,
            sp[0], sp[1], sp[2], sp[3],
            fx0, fx1,
            sp[0] - fx0, fx1 - sp[1],
            3.0 - (sp[3] + 0.125),
            cl[0], cl[1], cl[2], cl[3],
            side.other, dr[0], dr[1], pct(dr[1]), dr[2], pct(dr[2]), dr[3], pct(dr[3]), dmax
        ));
        let fc = runs.iter().fold((0usize, 0usize, 0.0f64, i64::MAX, 0i64, 0i64), |a, r| {
            let f = r.face[wi];
            (a.0 + f.0, a.1 + f.1, a.2.max(f.2), if f.0 > 0 { a.3.min(f.3) } else { a.3 }, a.4.max(f.4), a.5 + f.5)
        });
        s.push_str(&format!(
            "  at the face crossing (the first tick an arrival's box reaches the exit face): {} arrivals; offset from the column's centre past 0.375 {} ({:.3}%); the largest {:.3}; flight time from the arrival {}..{} gt (mean {:.1})\n",
            fc.0, fc.1, if fc.0 == 0 { 0.0 } else { 100.0 * fc.1 as f64 / fc.0 as f64 }, fc.2, if fc.3 == i64::MAX { 0 } else { fc.3 }, fc.4, if fc.0 == 0 { 0.0 } else { fc.5 as f64 / fc.0 as f64 }
        ));
    }
    for r in fails.iter().take(6) {
        s.push_str(&format!(
            "  FAIL seed {}: lost {} (held by a mob {}), mobs out {}, held in a portal {}, Nether unticketed {} gt{}, worst gaps A {} B {}, arrivals A {} B {}{}\n",
            r.seed, r.lost, r.held_by_mob, r.mobs_out, r.stuck, r.nether_dead, r.nether_dead_from.map_or(String::new(), |f| format!(" from t {f}")), r.worst_gap[0], r.worst_gap[1], r.arrivals[0], r.arrivals[1],
            if r.errors.is_empty() { String::new() } else { format!("; model errors: {}", r.errors.join("; ")) }
        ));
        for l in r.stuck_log.iter().take(3) {
            s.push_str(&format!("    {l}\n"));
        }
        for l in r.control.iter().filter(|l| l.contains("MOB OUT")).take(3) {
            s.push_str(&format!("    {l}\n"));
        }
    }
    if !o.piglins.is_empty() {
        let out_total: usize = runs.iter().map(|r| r.mobs_out).sum();
        s.push_str(&format!("mobs out of the portal's cells over every seed: {out_total}{}\n", if out_total == 0 { " (PASS)" } else { " (FAIL)" }));
        let top = runs.iter().fold([0.0f64; 3], |a, r| [a[0].max(r.mob_top[0]), a[1].max(r.mob_top[1]), a[2].max(r.mob_top[2])]);
        s.push_str(&format!(
            "the mobs' highest box top over the portal's floor (y 10), the jumps' own control{}: adults {:.3}, babies on foot {:.3}, the jockey stack (a rider and its chicken) {:.3}; the pocket floor's top is {:.1} over it, the hopper bowl's items {:.4}\n",
            if o.mob_jump.is_some() { "" } else { " (no forced jumps)" },
            top[0],
            top[1],
            top[2],
            (o.fire_y - 1 - 10) as f64,
            (o.fire_y - 2 - 10) as f64 + 0.6875
        ));
    }
    if let Some(r) = runs.first() {
        for l in r.control.iter().take(6) {
            s.push_str(&format!("  seed {}: {l}\n", r.seed));
        }
        if o.trace {
            for l in r.log.iter().take(40) {
                s.push_str(&format!("  seed {}: {l}\n", r.seed));
            }
        }
    }
    Ok(s)
}
