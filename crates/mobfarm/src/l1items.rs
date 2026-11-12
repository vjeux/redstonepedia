//! (onechunk, Nov 11; farmopt 22:12/22:22, ironspec's self-contained loaders, tick-exact's code
//! read) L1‴, the item-loop loader, benched in redsim: a portal pair (overworld A, Nether B, both
//! `width` x 3 at x 0.., y 10..12, z 0, axis x: the same axis, so an item's motion carries on through
//! the exit plane), and on each side a firing dropper D2 (bottom row, one cell off the plane, facing
//! it) fed by a dropper D1 under it (facing up), and a floor of hoppers just past the frame on the
//! same side (where the other side's items come out) chained into D1. Each side fires from its own
//! column (A column 1, B column width - 2), so the other side's items come out clear of it.
//! `items` dedicated items circulate; a fresh item from a dropper has no portal cooldown.
//!
//! EMULATED (labelled): each side's clock: every `period` gt (B half a period after A) a 2-gt
//! redstone-block pulse beside D1 and D2 (D1 lifts one item into D2, D2 fires one). The real clock
//! (a hopper clock, ~8 blocks a side) is priced, not built.
//!
//! The report: each side's arrivals (an item entity id new to that world), their intervals against
//! the 300-gt PORTAL ticket, the Nether's ticking gaps, items held inside a portal (stalled on
//! cooldown: lost), the items counted (containers plus entities, both worlds), the landing spread.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use std::collections::{BTreeMap, BTreeSet};

type P = [i32; 3];

pub const ITEM: &str = "cobblestone";

/// One side: `s` (+1: its dropper and floor at z >= 1, -1: z <= -1), its dropper column, the
/// other side's column (where this side's arrivals come out).
#[derive(Clone, Debug)]
pub struct Side {
    pub s: i32,
    pub col: i32,
    pub other: i32,
}

impl Side {
    pub fn d2(&self) -> P {
        [self.col, 10, self.s]
    }
    pub fn d1(&self) -> P {
        [self.col, 9, self.s]
    }
    /// The clock's power cells beside D1 and D2 (on the side away from the floor).
    pub fn power(&self) -> [P; 2] {
        let dx = if self.other > self.col { -1 } else { 1 };
        [[self.col + dx, 9, self.s], [self.col + dx, 10, self.s]]
    }
    /// The floor hoppers: the other column and its two neighbours, two rows deep.
    pub fn floor(&self, width: i32) -> Vec<P> {
        let mut v = Vec::new();
        for x in (self.other - 1)..=(self.other + 1) {
            if x < 0 || x >= width || x == self.col {
                continue;
            }
            for k in 1..=2 {
                v.push([x, 9, self.s * k]);
            }
        }
        v
    }
    fn blocks(&self, width: i32, out: &mut BTreeMap<P, String>, be: &mut BTreeMap<P, String>, items: u32) {
        for y in 10..=12 {
            out.insert([-1, y, 0], "minecraft:obsidian".into());
            out.insert([width, y, 0], "minecraft:obsidian".into());
        }
        for x in 0..width {
            out.insert([x, 9, 0], "minecraft:obsidian".into());
            out.insert([x, 13, 0], "minecraft:obsidian".into());
            for y in 10..=12 {
                out.insert([x, y, 0], "minecraft:nether_portal[axis=x]".into());
            }
        }
        let toward = if self.s > 0 { "north" } else { "south" };
        out.insert(self.d2(), format!("minecraft:dropper[facing={toward},triggered=false]"));
        out.insert(self.d1(), "minecraft:dropper[facing=up,triggered=false]".into());
        let half = items / 2;
        be.insert(self.d2(), format!("{{Items:[{{Slot:0b,id:\"minecraft:{ITEM}\",count:1}}]}}"));
        if half > 1 {
            be.insert(self.d1(), format!("{{Items:[{{Slot:0b,id:\"minecraft:{ITEM}\",count:{}}}]}}", half - 1));
        }
        // the floor: each hopper points toward D1 along x, the back row toward the front row
        for p in self.floor(width) {
            let facing = if p[2] == self.s * 2 {
                if self.s > 0 { "north" } else { "south" }
            } else if p[0] > self.col {
                "west"
            } else {
                "east"
            };
            out.insert(p, format!("minecraft:hopper[enabled=true,facing={facing}]"));
        }
        // (a glass backstop three cells off the plane over the floor: an item comes out of the
        // portal at up to ~0.4 b/t and ~0.2 up and flew past the 2-row floor in the first run)
        for x in (self.other - 2)..=(self.other + 2) {
            if x == self.col {
                continue;
            }
            for y in 10..=12 {
                out.insert([x, y, self.s * 3], "minecraft:glass".into());
            }
        }
    }
}

pub fn sides(width: i32) -> (Side, Side) {
    (Side { s: 1, col: 1, other: width - 2 }, Side { s: -1, col: width - 2, other: 1 })
}

pub fn design_text(width: i32, items: u32) -> String {
    let (a, b) = sides(width);
    let mut d = Design { name: "l1-item-loop-bench".into(), ..Default::default() };
    a.blocks(width, &mut d.blocks, &mut d.block_entities, items);
    b.blocks(width, &mut d.nether_blocks, &mut d.nether_block_entities, items);
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    d.ports.insert("output".into(), [width + 6, 10, 8]);
    d.ports.insert("nether-anchor".into(), [0, 0, 0]);
    d.ports.insert("nether-axis".into(), [1, 0, 0]);
    let mut t = format!("# L1''' item-loop loader bench (onechunk, Nov 11): portals {width} x 3, a firing dropper fed from below and a hopper floor on each side; {items} items; the clocks EMULATED by mobfarm l1-item-bench\n");
    t.push_str(&d.to_text());
    t
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub width: i32,
    pub items: u32,
    pub ticks: i64,
    pub period: i64,
    pub freezes: Vec<(i64, i64)>,
    pub seed: i64,
    pub trace: bool,
}

fn items_in(w: &redsim::world::World) -> (u32, Vec<(u32, [f64; 3], [f64; 3], i32)>) {
    use redsim::redstone::container as rc;
    let id = rc::item_id(ITEM);
    let inv: u32 = w.containers.map.values().map(|c| c.slots.iter().flatten().filter(|s| Some(s.item) == id).map(|s| s.count as u32).sum::<u32>()).sum();
    let ents: Vec<(u32, [f64; 3], [f64; 3], i32)> = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed && matches!(&e.kind, EntityKind::Item(s) if s.item.trim_start_matches("minecraft:") == ITEM))
        .map(|e| (e.id, [e.pos.x, e.pos.y, e.pos.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z], e.portal_cooldown))
        .collect();
    let n = ents.iter().map(|e| {
        w.entities.list.iter().find(|x| x.id == e.0).map_or(0, |x| match &x.kind {
            EntityKind::Item(s) => s.count as u32,
            _ => 0,
        })
    });
    (inv + n.sum::<u32>(), ents)
}

pub fn bench(spec: &FarmSpec, o: &Opts) -> Result<String, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(o.seed, (o.ticks + 100) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let width = o.width;
    let in_portal = |p: [f64; 3]| p[0] > -0.13 && p[0] < width as f64 + 0.13 && p[2] > -0.13 && p[2] < 1.13 && p[1] > 9.9 && p[1] < 13.0;
    // (a dropper's item draws its motion from the level random and its own random from the drop
    // seed: both worlds get them, seeded from the bench's seed)
    run.world.level_random = Some(redsim::rng::LegacyRandom::new(o.seed));
    run.world.drop_seed = Some(o.seed);
    {
        let n = run.world.nether.as_deref_mut().ok_or("no Nether part")?;
        n.level_random = Some(redsim::rng::LegacyRandom::new(o.seed ^ 0x4e45));
        n.drop_seed = Some(o.seed ^ 0x4e45);
    }
    let (a, b) = sides(width);
    let rsrc = Block::parse("minecraft:redstone_block")?;
    let air = Block::parse("minecraft:air")?;
    let start = run.world.time;
    let mut seen: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    let mut arrivals: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    let mut landings: [Vec<[f64; 2]>; 2] = [Vec::new(), Vec::new()];
    let mut fired = [0usize; 2];
    let mut stuck: Vec<String> = Vec::new();
    let mut inside_since: BTreeMap<(usize, u32), i64> = BTreeMap::new();
    let mut flying: BTreeMap<(usize, u32), bool> = BTreeMap::new();
    let mut nether_dead = 0i64;
    let mut nether_dead_from: Option<i64> = None;
    let mut counts_min = u32::MAX;
    let mut log: Vec<String> = Vec::new();
    let mut frozen: Option<[Vec<(u32, [f64; 3], [f64; 3], i32)>; 2]> = None;
    let mut off_until = [i64::MIN / 4; 2];
    for _ in 0..o.ticks {
        let t = run.world.time;
        let rel = t - start;
        for (wi, side) in [(0usize, &a), (1usize, &b)] {
            let world: &mut redsim::world::World = if wi == 0 { &mut run.world } else { run.world.nether.as_deref_mut().unwrap() };
            // the emulated clock
            let phase = if wi == 0 { 0 } else { o.period / 2 };
            if rel % o.period == phase && rel > 0 {
                for p in side.power() {
                    world.set_block(BlockPos::new(p[0], p[1], p[2]), rsrc.clone(), redsim::world::flags::ALL);
                }
                off_until[wi] = t + 2;
                fired[wi] += 1;
            }
            if t == off_until[wi] {
                // (the cell beside D2 cleared first: D1 also reads D2's cell (quasi-connectivity)
                // and only re-reads it on its own neighbour's update, so clearing D1's cell last
                // is what lets D1 see the pulse end; the other order left D1 triggered for good,
                // the first runs' stall)
                for p in side.power().iter().rev() {
                    world.set_block(BlockPos::new(p[0], p[1], p[2]), air.clone(), redsim::world::flags::ALL);
                }
            }
            if o.trace && wi == 0 && (rel % o.period == 0 || rel % o.period == 8) && rel > 0 && rel < 1300 {
                let c = |p: P| world.containers.map.get(&BlockPos::new(p[0], p[1], p[2])).map_or(0, |c| c.slots.iter().flatten().map(|x| x.count as u32).sum::<u32>());
                log.push(format!("t {rel}: A D2 {} {:?} D1 {} {:?}", c(side.d2()), world.get(BlockPos::new(side.d2()[0], side.d2()[1], side.d2()[2])), c(side.d1()), world.get(BlockPos::new(side.d1()[0], side.d1()[1], side.d1()[2]))));
            }
            let (_, ents) = items_in(world);
            // (ids leave the seen set when their item is gone: an id is reused across the two
            // worlds' counters, and a reused id is a new item)
            let present: BTreeSet<u32> = ents.iter().map(|e| e.0).collect();
            seen[wi].retain(|id| present.contains(id));
            for e in &ents {
                // an arrival: an item new to this world with a portal cooldown (a crossing's copy;
                // a dropper's own item has none) wherever it is by now (a fast one can be out of
                // the frame before the first look)
                if !seen[wi].contains(&e.0) {
                    seen[wi].insert(e.0);
                    if e.3 > 0 {
                        arrivals[wi].push(rel);
                        flying.insert((wi, e.0), true);
                        if o.trace {
                            log.push(format!("t {rel}: arrival in {} (item {}) at ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3})", if wi == 0 { "A" } else { "B" }, e.0, e.1[0], e.1[1], e.1[2], e.2[0], e.2[1], e.2[2]));
                        }
                    }
                }
                if in_portal(e.1) {
                    let since = *inside_since.entry((wi, e.0)).or_insert(t);
                    if t - since == 40 {
                        stuck.push(format!("t {rel}: item {} inside {}'s portal 40 gt at ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3}), cooldown {}", e.0, if wi == 0 { "A" } else { "B" }, e.1[0], e.1[1], e.1[2], e.2[0], e.2[1], e.2[2], e.3));
                    }
                } else {
                    inside_since.remove(&(wi, e.0));
                    // its landing: the first tick out of the portal on the ground (vertical speed ~0)
                    if flying.get(&(wi, e.0)) == Some(&true) && e.2[1].abs() < 1e-3 {
                        landings[wi].push([e.1[0], e.1[2]]);
                        flying.insert((wi, e.0), false);
                    }
                }
            }
        }
        let freeze_now = o.freezes.iter().any(|(f, d)| rel >= *f && rel < f + d);
        if freeze_now && frozen.is_none() {
            frozen = Some([items_in(&run.world).1, items_in(run.world.nether.as_deref().unwrap()).1]);
        }
        run.tick();
        if let Some(snap) = &frozen {
            if freeze_now {
                for (wi, sn) in snap.iter().enumerate() {
                    let world: &mut redsim::world::World = if wi == 0 { &mut run.world } else { run.world.nether.as_deref_mut().unwrap() };
                    for (id, p, v, cd) in sn {
                        if let Some(i) = redsim::entity::index_of(world, *id) {
                            let e = &mut world.entities.list[i];
                            e.set_pos(redsim::entity::Vec3::new(p[0], p[1], p[2]));
                            e.delta_movement = redsim::entity::Vec3::new(v[0], v[1], v[2]);
                            e.portal_cooldown = *cd;
                        }
                    }
                }
            } else {
                frozen = None;
            }
        }
        let n = run.world.nether.as_deref().unwrap();
        if n.portal_tickets.is_empty() && !arrivals[1].is_empty() {
            nether_dead += 1;
            nether_dead_from.get_or_insert(rel);
        }
        let total = items_in(&run.world).0 + items_in(n).0;
        counts_min = counts_min.min(total);
    }
    let mut s = format!("l1-item-bench (L1''' item loop, redsim): portals {width} x 3, {} items, each side's clock EMULATED every {} gt (B half a period after A), {} gt, seed {}; the droppers' item motion from the level random as the game draws it; crossings, the closest-portal search and the Nether's PORTAL ticket as redsim models them\n", o.items, o.period, o.ticks, o.seed);
    for (wi, name) in [(0usize, "A (overworld)"), (1usize, "B (Nether)")] {
        let ar = &arrivals[wi];
        let iv: Vec<i64> = ar.windows(2).map(|w| w[1] - w[0]).collect();
        let max = iv.iter().copied().max().unwrap_or(0);
        let over = iv.iter().filter(|v| **v > 300).count();
        let la = &landings[wi];
        let (mut x0, mut x1, mut z0, mut z1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for l in la {
            x0 = x0.min(l[0]);
            x1 = x1.max(l[0]);
            z0 = z0.min(l[1]);
            z1 = z1.max(l[1]);
        }
        s.push_str(&format!(
            "side {name}: fired {} times; {} arrivals; intervals {}..{} gt (each side needs <= 300): {} over 300{}; landings {}\n",
            fired[wi],
            ar.len(),
            iv.iter().copied().min().unwrap_or(0),
            max,
            over,
            if over == 0 && iv.len() > 2 { " (PASS)" } else { " (FAIL)" },
            if la.is_empty() { "none seen on the ground (taken by the hoppers first)".to_string() } else { format!("x {x0:.2}..{x1:.2} z {z0:.2}..{z1:.2}") }
        ));
    }
    let n = run.world.nether.as_deref().unwrap();
    // (the A -> B crossings as the overworld records them: every copy placed in the Nether)
    let cr: Vec<i64> = run.world.crossings.iter().filter(|c| c.0 >= start).map(|c| c.0 - start).collect();
    let civ: Vec<i64> = cr.windows(2).map(|w| w[1] - w[0]).collect();
    s.push_str(&format!("A -> B crossings recorded by the overworld: {}; intervals {}..{} gt; {} over 300{}\n", cr.len(), civ.iter().copied().min().unwrap_or(0), civ.iter().copied().max().unwrap_or(0), civ.iter().filter(|v| **v > 300).count(), if civ.iter().all(|v| *v <= 300) && civ.len() > 2 { " (PASS)" } else { " (FAIL)" }));
    let end_total = items_in(&run.world).0 + items_in(n).0;
    s.push_str(&format!("items counted (containers + entities, both worlds): {} at the end, at least {} at any tick ({} placed){}\n", end_total, counts_min, o.items, if end_total == o.items { " (PASS)" } else { " (FAIL)" }));
    s.push_str(&format!("items held inside a portal 40 gt or more (stalled on cooldown: lost to the loop): {}{}\n", stuck.len(), if stuck.is_empty() { " (PASS)" } else { " (FAIL)" }));
    for l in stuck.iter().take(6) {
        s.push_str(&format!("  {l}\n"));
    }
    for (wn, w) in [("A", &run.world), ("B", n)] {
        for e in items_in(w).1 {
            s.push_str(&format!("  item entity in {wn} at the end: ({:.2},{:.2},{:.2}) v ({:.3},{:.3},{:.3})\n", e.1[0], e.1[1], e.1[2], e.2[0], e.2[1], e.2[2]));
        }
        let mut cs: Vec<String> = w.containers.map.iter().filter_map(|(p, c)| {
            let k: u32 = c.slots.iter().flatten().map(|x| x.count as u32).sum();
            (k > 0).then(|| format!("({},{},{}) {k}", p.x, p.y, p.z))
        }).collect();
        cs.sort();
        s.push_str(&format!("  containers holding items in {wn} at the end: {}\n", cs.join(", ")));
    }
    s.push_str(&format!("the Nether unticketed after its first arrival: {nether_dead} gt{}\n", nether_dead_from.map_or(" (PASS)".to_string(), |f| format!(" (FAIL: from t {f})"))));
    for (wn, w) in [("overworld", &run.world), ("Nether", n)] {
        if !w.errors.is_empty() {
            s.push_str(&format!("{wn} model errors: {}\n", w.errors.join("; ")));
        }
    }
    if o.trace {
        for l in log.iter().take(40) {
            s.push_str(&format!("  {l}\n"));
        }
    }
    Ok(s)
}
