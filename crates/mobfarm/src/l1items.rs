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

pub const ITEM: &str = "cobblestone";
/// The piglin's reach: its box (1.95) over the frame's bottom obsidian (y 10).
pub const PIGLIN_REACH_Y: f64 = 11.95;

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
}

impl Side {
    pub fn d2(&self) -> P {
        [self.col, self.fire_y, self.s]
    }
    pub fn d1(&self) -> P {
        [self.col, 11, self.s]
    }
    /// The observer pair: O1 (output into D2) and O2.
    pub fn observers(&self) -> [P; 2] {
        [[self.col, 12, 2 * self.s], [self.col, 12, 3 * self.s]]
    }
    /// The floor columns: the other side's column and its two neighbours (in the portal, not D2's).
    pub fn floor_cols(&self, width: i32) -> Vec<i32> {
        ((self.other - 1)..=(self.other + 1)).filter(|x| *x >= 0 && *x < width && *x != self.col).collect()
    }
    /// The collecting hoppers (y 11, two off the plane: beyond a bottom-row piglin's 1-block reach,
    /// z 0.2..0.8 + 1), under the flowing water; the one under O1 feeds D1.
    pub fn floor(&self, width: i32) -> Vec<P> {
        let mut v: Vec<P> = self.floor_cols(width).into_iter().map(|x| [x, 11, 2 * self.s]).collect();
        v.push([self.col, 11, 2 * self.s]);
        v
    }
    fn blocks(&self, width: i32, height: i32, out: &mut BTreeMap<P, String>, be: &mut BTreeMap<P, String>, items: u32) {
        let glass = "minecraft:glass".to_string();
        let (s, c) = (self.s, self.col);
        // (the portal's interior rows y 10 .. 9 + height; the frame's top at 10 + height: with 4
        // rows a slow item fired in the third row arcs under the top before it reaches the face)
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
                // the far face walled: nothing leaves the plane there, and a piglin in the bottom
                // row can't step out that way
                out.insert([x, y, -s], glass.clone());
            }
        }
        let toward = if s > 0 { "north" } else { "south" };
        let away = if s > 0 { "south" } else { "north" };
        out.insert(self.d2(), format!("minecraft:dropper[facing={toward},triggered=false]"));
        // (the feeders: up-facing droppers from y 11 to under D2; O1 powers the one at y 12 (D2
        // itself when it fires at 12), which as a conductor powers the droppers next to it)
        for y in 11..self.fire_y {
            out.insert([c, y, s], "minecraft:dropper[facing=up,triggered=false]".into());
        }
        let half = items / 2;
        be.insert(self.d2(), format!("{{Items:[{{Slot:0b,id:\"minecraft:{ITEM}\",count:1}}]}}"));
        if half > 1 {
            be.insert(self.d1(), format!("{{Items:[{{Slot:0b,id:\"minecraft:{ITEM}\",count:{}}}]}}", half - 1));
        }
        let cols = self.floor_cols(width);
        // the near row (z s): the portal's bottom two rows closed on this face (y 10 glass, y 11
        // full blocks: D1, ice or glass; ironspec 00:30: a baby or a chicken jockey fits a 1-high
        // gap), so a mob in the bottom rows never leaves; the items come out of the top row onto
        // ICE in the floor columns (top 12, out of a bottom-row mob's reach; ice: no spawns but a
        // polar bear's; slippery, so an item that lands slides on to the hoppers), glass elsewhere
        for x in -1..=width {
            out.insert([x, 10, s], glass.clone());
            if x == c {
                continue;
            }
            // (packed ice: redsim models its friction, 0.98, not plain ice's; plain ice takes no
            // spawn but a polar bear's, packed ice takes them: the build's floor is tick-exact's
            // cells, this one benches the clock and the flow)
            out.insert([x, 11, s], if cols.contains(&x) { "minecraft:packed_ice".into() } else { glass.clone() });
            if !cols.contains(&x) {
                for y in 12..=top {
                    out.insert([x, y, s], glass.clone());
                }
            }
        }
        // the hopper row (z 2s): the floor columns' hoppers point along x toward the one under O1,
        // which points into D1; glass in the other columns at y 11 and 12 (the water's bounds)
        for p in self.floor(width) {
            let facing = if p[0] == c {
                toward
            } else if p[0] > c {
                "west"
            } else {
                "east"
            };
            out.insert(p, format!("minecraft:hopper[enabled=true,facing={facing}]"));
        }
        for x in -1..=width {
            if x == c || cols.contains(&x) {
                continue;
            }
            out.insert([x, 11, 2 * s], glass.clone());
            out.insert([x, 12, 2 * s], glass.clone());
        }
        // the backstop (z 3s, y 12..13) over the whole width, O2's cell aside; the enclosure: a
        // glass roof at y 13 over the near and hopper rows, walls at x -1 and x width (y 10..13),
        // so nothing walks in and no floor inside takes a spawn (ice, hoppers, glass; the
        // droppers' and observers' tops under the roof)
        for x in -1..=width {
            for y in 12..=cap {
                if x == c && y == 12 {
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
        // the clock: O1 faces O2 (its back, the output, into D2), O2 faces O1
        let [o1, o2] = self.observers();
        out.insert(o1, format!("minecraft:observer[facing={away},powered=false]"));
        out.insert(o2, format!("minecraft:observer[facing={toward},powered=false]"));
    }
}

pub fn sides(width: i32, fire_y: i32) -> (Side, Side) {
    (Side { s: 1, col: 1, other: width - 2, fire_y }, Side { s: -1, col: width - 2, other: 1, fire_y })
}

pub fn design_text(width: i32, height: i32, fire_y: i32, items: u32) -> String {
    let (a, b) = sides(width, fire_y);
    let mut d = Design { name: "l1-item-loop-bench".into(), ..Default::default() };
    a.blocks(width, height, &mut d.blocks, &mut d.block_entities, items);
    b.blocks(width, height, &mut d.nether_blocks, &mut d.nether_block_entities, items);
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    d.ports.insert("output".into(), [width + 6, 10, 8]);
    d.ports.insert("nether-anchor".into(), [0, 0, 0]);
    d.ports.insert("nether-axis".into(), [1, 0, 0]);
    let mut t = format!("# L1''' item-loop loader bench (onechunk, Nov 12): portals {width} x {height}; on each side a firing dropper at y 12 (above a bottom-row mob's reach) fed by an up-facing dropper, the exit side's bottom two rows closed, packed ice and a hopper row past the frame at y 11, a far-face wall, a backstop, and the clock: two observers facing each other behind the firing dropper (REAL, not emulated); {items} items\n");
    t.push_str(&d.to_text());
    t
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub width: i32,
    /// The portals' interior rows (3 or 4).
    pub height: i32,
    /// The firing dropper's y (12, or 13 with 4 rows).
    pub fire_y: i32,
    pub items: u32,
    pub ticks: i64,
    /// (from, len, worlds: 1 overworld, 2 Nether, 3 both): redsim's level-32 freeze.
    pub freezes: Vec<(i64, i64, u8)>,
    /// A 3-gt freeze of both worlds every N gt.
    pub login_each: Option<i64>,
    /// A 3-gt freeze of both worlds starting at every dispense (the item crosses after the thaw).
    pub login_before_arrival: bool,
    /// Both worlds unloaded at T and loaded at T + 1 (`reload::unload_all` / `load_all`).
    pub stops: Vec<i64>,
    /// A mob placed in a bottom row at column X at time T (AI on), on side bits (1 A, 2 B, 3 both),
    /// kind 0 an adult zombified piglin, 1 a baby, 2 a chicken jockey.
    pub piglins: Vec<(i64, i32, u8, u8)>,
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
    pub stuck: usize,
    pub nether_dead: i64,
    pub nether_dead_from: Option<i64>,
    /// Landing spread per side: x min, x max, |z| min, |z| max over every arrival's positions
    /// outside the plane until a hopper took it.
    pub spread: [[f64; 4]; 2],
    /// The flight's least clearance per side: to the frame's sides (x), to the top frame (y 13),
    /// and to a piglin's reach (11.95) while within 2.3 of the plane on the arrival side.
    pub clear: [[f64; 3]; 2],
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
    let (a, b) = sides(width, o.fire_y);
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
    let mut out = RunOut { seed, spread: [[f64::MAX, f64::MIN, f64::MAX, f64::MIN]; 2], clear: [[f64::MAX; 3]; 2], ..Default::default() };
    let mut seen: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    let mut arrivals: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    let mut arriving: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    let mut inside_since: BTreeMap<(usize, u32), i64> = BTreeMap::new();
    let mut last_ticket: i64 = i64::MIN;
    let mut frozen = [false; 2];
    let mut login_until: i64 = i64::MIN;
    let mut min_count = u32::MAX;
    let mut mobs_out: BTreeSet<(usize, u32)> = BTreeSet::new();
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
                    let (id, what) = match kind {
                        1 => (redsim::entity::spawn(w, "baby_zombified_piglin", at, loot)?, "a baby zombified piglin"),
                        2 => {
                            let ch = redsim::entity::spawn(w, "minecraft:chicken", at, Some("{IsChickenJockey:1b,PortalCooldown:300}"))?;
                            let rider = redsim::entity::spawn(w, "baby_zombified_piglin", at, loot)?;
                            redsim::entity::mob::mount(w, rider, ch)?;
                            (rider, "a chicken jockey (a baby zombified piglin on a chicken)")
                        }
                        _ => (redsim::entity::spawn(w, "minecraft:zombified_piglin", at, loot)?, "an adult zombified piglin"),
                    };
                    // (the positive control: a stone item at its feet, inside its reach, which it
                    // must take: the pickup is modeled)
                    redsim::entity::spawn(w, "minecraft:item", [x as f64 + 0.5, 10.1, 0.5], Some("{Item:{id:\"minecraft:stone\",count:1},PickupDelay:0}"))?;
                    out.control.push(format!("t {rel}: {what} (id {id}, loot pickup on, empty hand) placed in {}'s bottom row at column {x}, a stone item at its feet (the control)", if wi == 0 { "A" } else { "B" }));
                }
            }
        }
        for &st in &o.stops {
            if rel == st {
                redsim::reload::unload_all(&mut run.world);
                redsim::reload::unload_all(run.world.nether.as_deref_mut().unwrap());
            }
            if rel == st + 1 {
                redsim::reload::load_all(&mut run.world);
                redsim::reload::load_all(run.world.nether.as_deref_mut().unwrap());
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
                    let az = e.1[2] * side.s as f64;
                    let c = &mut out.clear[wi];
                    // in the plane (its box within z 0..1): the frame's sides and top
                    if e.1[2] > -0.125 && e.1[2] < 1.125 {
                        c[0] = c[0].min((e.1[0] - 0.125).min(width as f64 - (e.1[0] + 0.125)));
                        c[1] = c[1].min((10 + o.height) as f64 - (e.1[1] + 0.25));
                    }
                    // a piglin standing anywhere in the bottom row (its box z 0.2..0.8, 1.95 tall)
                    // takes items inside its box inflated 1 on x and z (0 on y): an item whose box
                    // (half 0.125) meets z -0.8..1.8, against y 11.95
                    if e.1[2] > -0.925 && e.1[2] < 1.925 {
                        c[2] = c[2].min(e.1[1] - PIGLIN_REACH_Y);
                    }
                    if az > 1.125 {
                        let sp = &mut out.spread[wi];
                        sp[0] = sp[0].min(e.1[0]);
                        sp[1] = sp[1].max(e.1[0]);
                        sp[2] = sp[2].min(az);
                        sp[3] = sp[3].max(az);
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
    if !o.piglins.is_empty() {
        let mut piglins = 0;
        let mut with_stone = 0;
        for w in [&run.world, n] {
            for i in 0..w.entities.list.len() {
                if w.entities.list[i].removed || !matches!(w.entities.list[i].kind, EntityKind::Mob(_)) {
                    continue;
                }
                piglins += 1;
                if redsim::entity::mob::mob(w, i).mainhand.as_ref().is_some_and(|h| h.item.trim_start_matches("minecraft:") == "stone") {
                    with_stone += 1;
                }
            }
        }
        out.control.push(format!("the control at the end: {with_stone} of {piglins} piglins hold the stone placed at their feet (the pickup modeled{}); {} hold a loop item", if with_stone > 0 { "" } else { ": NOT SEEN" }, out.held_by_mob));
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
        rows.push(format!("{} full stops (both worlds unloaded and loaded again)", o.stops.len()));
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
        let cl = runs.iter().fold([f64::MAX; 3], |a, r| [a[0].min(r.clear[wi][0]), a[1].min(r.clear[wi][1]), a[2].min(r.clear[wi][2])]);
        let side = if wi == 0 { sides(o.width, o.fire_y).0 } else { sides(o.width, o.fire_y).1 };
        let (fx0, fx1) = ((side.other - 1).max(0) as f64, ((side.other + 1).min(o.width - 1) + 1) as f64);
        s.push_str(&format!(
            "side {name}: arrivals a seed {}..{}; worst gap {} gt (margin {} to 300)\n  landing spread x {:.2}..{:.2}, |z| {:.2}..{:.2} against the floor x {:.0}..{:.0} (margins {:.2} / {:.2}) and the backstop at |z| 3 (margin {:.2}); the flight's least clearance: frame sides {:.3}, top frame {:.3}, a piglin's reach {:.3}\n",
            arr.iter().min().unwrap_or(&0),
            arr.iter().max().unwrap_or(&0),
            worst,
            300 - worst,
            sp[0], sp[1], sp[2], sp[3],
            fx0, fx1,
            sp[0] - fx0, fx1 - sp[1],
            3.0 - (sp[3] + 0.125),
            cl[0], cl[1], cl[2]
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
