//! The carousel's release + hold part (vjeux 2026-11-09 22:25-22:32; brief
//! designs/briefs/farm-iron-carousel.md, "The parts", 2): seven batches of three villagers, each
//! batch a column of three stacked tracks (one villager riding one minecart on each), no switch
//! and no loop. A track is a single cart between two one-rail slopes of powered rail rising toward
//! the track's two stops: unpowered, a slope brakes the arriving cart and holds it parked on the
//! slope; powered, the parked cart rolls off by gravity and the boost. Each slope's control sits
//! beside its top rail (the bench switches it between a redstone block and glass): a batch's hold
//! slopes are powered while the counter's line for the batch is high, its stand slopes while it is
//! low, so the batch goes out when its line rises and comes back when it falls.
//!
//! Why single carts: three carts on one track, a train, bench badly. Carts in contact push each
//! other every tick (`pushOtherMinecart` averages their velocities and keeps a fifth of each: the
//! pair's momentum grows by a fifth a tick), so a train's stored speeds reach the 2.0 cap; when its
//! lead brakes, a follower bounces off the train and drifts away, a train parked on flat brakes
//! has nothing to start it again, a dip before the end traps it and a downhill pit stalls it with
//! every cart pressed against the wall. A single cart parks on a braked slope and leaves it by
//! gravity every time.
//!
//! The levels stand `level_gap` (3) apart: a rider's eyes (1.68 over its rail) stay under the
//! level above's supports. A level's two slopes are one rail in from the level below's (stand at
//! z `k`, hold at z `run - k`), so nothing sits over the head of a rider parked on a slope (lifted
//! half a block). Held villagers panic too, as the brief's part 2 requires at night: on every
//! level, zombies in boats two out from the outer lanes (out of reach), level with the riders, in
//! line with the level's two slopes (a parked rider looks along x there: the level above's slope
//! support blocks its view along the lane) and at the middle of the run (the middle lane is 5
//! across, so a pair covers 6 along the lanes).
//!
//! [`design`] writes the part's bench design; [`bench`] runs the seven batches on the 100-gt slot
//! grid with the counter's lines ideal and checks every villager every tick.

use crate::layouts::Builder;
use anyhow::Result;
use redsim::block::Block;
use redsim::entity::mob::ai::brain::{self, Act};
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// The batches (vjeux 22:27: "we should have 7 sets of villagers for max efficiency").
pub const BATCHES: i32 = 7;

/// The tracks' shape.
#[derive(Clone, Copy, Debug)]
pub struct LaneOpts {
    /// Lane spacing in x (carts side by side never push each other: `pushOtherMinecart` skips a
    /// cart across the line between them).
    pub pitch: i32,
    /// Tracks a lane, stacked: one villager each, a batch the lane's column.
    pub levels: i32,
    /// Height between levels.
    pub level_gap: i32,
    /// The bottom level's hold slope (z); level `k`'s is `run - k`.
    pub run: i32,
    /// A booster (a powered rail on a redstone block) every this many flat rails.
    pub boost_every: i32,
    /// Flat rails at each end without a booster (a booster in line with a slope would power it
    /// through the rails between).
    pub margin: i32,
    /// The hold zombies.
    pub zombies: bool,
    /// The pad's nearest cell (z): held villagers must be 19.3 past it (the golems' follow box).
    pub pad_z: f64,
}

impl Default for LaneOpts {
    fn default() -> LaneOpts {
        LaneOpts { pitch: 1, levels: 3, level_gap: 3, run: 20, boost_every: 2, margin: 1, zombies: true, pad_z: -1.0 }
    }
}

impl LaneOpts {
    /// Lane `b`'s x.
    pub fn x(&self, b: i32) -> i32 {
        b * self.pitch
    }

    /// Every control cell of a lane's stand end: one a level, beside the level's slope rail in its
    /// stop column.
    pub fn stand_ctrls(&self, b: i32) -> Vec<BlockPos> {
        (0..self.levels).map(|k| BlockPos::new(self.x(b), self.level_gap * k, k - 1)).collect()
    }

    /// Every control cell of a lane's hold end.
    pub fn hold_ctrls(&self, b: i32) -> Vec<BlockPos> {
        (0..self.levels).map(|k| BlockPos::new(self.x(b), self.level_gap * k, self.run - k + 1)).collect()
    }

    /// The stand zone: a rider with its z under this is at the station (on its level's slope).
    pub fn stand_z(&self) -> f64 {
        self.levels as f64
    }

    /// The hold zone: a held rider's z is past this (19.3 from the pad's nearest cell).
    pub fn hold_z(&self) -> f64 {
        self.pad_z + 19.3
    }

    /// The shared bed's head, past the hold ends: every villager's HOME through the R1 double
    /// claim (the sleep routine's part moves it).
    pub fn bed(&self) -> BlockPos {
        BlockPos::new(self.x(BATCHES / 2), 0, self.run + 3)
    }
}

/// The part's bench design: the seven lanes of stacked tracks, every cart with its villager
/// parked on its hold slope, the hold zombies, the shared bed.
pub fn design(o: &LaneOpts) -> Result<String> {
    anyhow::ensure!(o.pitch >= 1 && o.levels >= 1 && o.level_gap >= 3, "a pitch of 1 or more and levels 3 or more apart (a rider's eyes under the level above's supports)");
    anyhow::ensure!(o.boost_every >= 0 && o.margin >= 1, "boosters every 0 (none) or more, at least one flat rail from each slope");
    let hold_first = o.run - (o.levels - 1);
    anyhow::ensure!((hold_first as f64) + 0.5 > o.hold_z(), "the top level holds at z {hold_first} but held riders must be past {:.1} (19.3 from the pad)", o.hold_z());
    anyhow::ensure!(hold_first - (o.levels - 1) > 2 * o.margin + 1, "the top level's track is too short for its margins");
    let mut b = Builder::new("carousel-hold-7");
    let glass = "glass";
    for lane in 0..BATCHES {
        let x = o.x(lane);
        for k in 0..o.levels {
            let h = o.level_gap * k;
            let rail = |b: &mut Builder, z: i32, block: &str, support: &str| {
                b.set(x, h, z, block);
                b.set(x, h - 1, z, support);
            };
            let (zs, zh) = (k, o.run - k);
            // the stand slope, rising toward its stop (-z); its control a redstone block at the
            // start (every line low: the stand ends powered, the carts at the hold); glass over
            // the control where the cart would climb on
            rail(&mut b, zs, "powered_rail[powered=false,shape=ascending_north,waterlogged=false]", glass);
            b.set(x, h, zs - 1, "redstone_block");
            b.set(x, h + 1, zs - 1, glass);
            let n = zh - zs - 1;
            for i in 0..n {
                let boost = o.boost_every > 0 && i >= o.margin && i < n - o.margin && i % o.boost_every == o.boost_every / 2;
                if boost {
                    rail(&mut b, zs + 1 + i, "powered_rail[powered=true,shape=north_south,waterlogged=false]", "redstone_block");
                } else {
                    rail(&mut b, zs + 1 + i, "rail[shape=north_south,waterlogged=false]", glass);
                }
            }
            // the hold slope, rising toward its stop (+z); its control glass at the start
            rail(&mut b, zh, "powered_rail[powered=false,shape=ascending_south,waterlogged=false]", glass);
            b.set(x, h, zh + 1, glass);
            b.set(x, h + 1, zh + 1, glass);
            // the cart parked on the hold slope (half a block up its cell), facing along the lane
            // (a cart that rolled there faces its motion)
            b.entity("minecart", [x as f64 + 0.5, h as f64 + 0.5625, zh as f64 + 0.5], Some("{Rotation:[90f,0f],Passengers:[{id:\"minecraft:villager\"}]}"));
        }
    }
    if o.zombies {
        let east = o.x(BATCHES - 1) + 2;
        let roof = o.level_gap * o.levels;
        for k in 0..o.levels {
            let h = o.level_gap * k;
            for z in [k, o.run / 2, o.run - k] {
                for x in [-2, east] {
                    b.set(x, h - 1, z, glass);
                    b.set(x, roof, z, "oak_leaves[distance=1,persistent=true,waterlogged=false]");
                    b.entity("oak_boat", [x as f64 + 0.5, h as f64, z as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"));
                }
            }
        }
    }
    // the shared bed (facing north: its head the north half), on glass
    let bed = o.bed();
    b.set(bed.x, bed.y, bed.z, "red_bed[facing=north,occupied=false,part=head]");
    b.set(bed.x, bed.y, bed.z + 1, "red_bed[facing=north,occupied=false,part=foot]");
    b.set(bed.x, bed.y - 1, bed.z, glass);
    b.set(bed.x, bed.y - 1, bed.z + 1, glass);
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("hold".into(), format!("pitch={} levels={} level_gap={} run={} boost={} margin={} zombies={}", o.pitch, o.levels, o.level_gap, o.run, o.boost_every, o.margin, o.zombies as i32));
    b.d.ports.insert("output".into(), [bed.x + 1, 0, bed.z]);
    Ok(b.text(&[
        &format!("The carousel's release + hold part (bench): {BATCHES} lanes at pitch {}, each {} stacked single-cart tracks {} apart (one villager a track, a batch a lane),", o.pitch, o.levels, o.level_gap),
        "each track a cart between two braked one-rail slopes, each slope's control switched by the bench; zombies beside every level, one shared bed.",
    ]))
}

/// The tracks' shape from a bench design's `hold:` line.
pub fn lanes_of(meta: &std::collections::BTreeMap<String, String>) -> Result<LaneOpts> {
    let line = meta.get("hold").ok_or_else(|| anyhow::anyhow!("not a release + hold design (no `hold:` line)"))?;
    let mut o = LaneOpts::default();
    for kv in line.split_whitespace() {
        let (k, v) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("bad `hold:` entry {kv:?}"))?;
        let n: i32 = v.parse()?;
        match k {
            "pitch" => o.pitch = n,
            "levels" => o.levels = n,
            "level_gap" => o.level_gap = n,
            "run" => o.run = n,
            "boost" => o.boost_every = n,
            "margin" => o.margin = n,
            "zombies" => o.zombies = n != 0,
            _ => anyhow::bail!("unknown `hold:` entry {k:?}"),
        }
    }
    Ok(o)
}

/// The bench's options.
#[derive(Clone, Copy, Debug)]
pub struct BenchOpts {
    pub seed: i64,
    pub days: u32,
    /// Each line switch comes 1 + a draw in 0..=jitter gt after its slot (the station's pulse
    /// delay and the counter's, d_s + T_c).
    pub jitter: u32,
    /// Print each slot's line and the first calm villagers at night.
    pub verbose: bool,
}

/// What the bench measured.
#[derive(Clone, Debug, Default)]
pub struct BenchReport {
    pub slots: u32,
    /// Violations (the first few, with their game time), and how many in all.
    pub violations: Vec<String>,
    pub violation_count: u32,
    /// The smallest arrival margin over the slots (gt from the batch's riders all in its stand
    /// zone to its slot), and its mean.
    pub min_margin: i64,
    pub mean_margin: f64,
    /// The riders' z at the station at slot ticks (min, max) and in the hold (min).
    pub stand_z: (f64, f64),
    pub hold_min_z: f64,
    /// Villager-ticks in REST, the oldest CANT_REACH at a REST tick, villager-ticks not panicking.
    pub rest_ticks: u64,
    pub max_cant_reach_at_rest: i64,
    pub calm_ticks: u64,
    /// Calm villager-ticks by cell (x, y, z) and whether it was night: the zombies' gaps.
    pub calm_at: std::collections::BTreeMap<(i32, i32, i32, bool), u64>,
    calm_printed: u32,
    /// Villager-ticks off their carts.
    pub off_cart: u64,
    /// Cells where a golem could spawn from any position a villager took.
    pub spawn_spots: Vec<BlockPos>,
    /// Model errors.
    pub errors: Vec<String>,
}

/// The cells a golem could spawn on for a villager at `cell` (`spawn_spots_of`'s scan, one start).
fn spots_from(w: &redsim::world::World, cell: BlockPos) -> Vec<BlockPos> {
    use redsim::entity::mob::spawn::{check_spawn_obstruction, possible_spawn_position};
    let mut out = Vec::new();
    for start in [cell, cell.above()] {
        for dz in -8..=8 {
            for dx in -8..=8 {
                let top = BlockPos::new(start.x + dx, start.y + 6, start.z + dz);
                if let Some(c) = possible_spawn_position(w, top, 6)
                    && !out.contains(&c)
                    && check_spawn_obstruction(w, c, Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5))
                {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Every zombie within 10 of villager `i`: its distance and whether the villager's eyes see its.
fn zombies_near(w: &redsim::world::World, i: usize) -> String {
    let e = &w.entities.list[i];
    let eye = Vec3::new(e.pos.x, e.pos.y + 1.62, e.pos.z);
    let mut out = String::new();
    for z in &w.entities.list {
        let EntityKind::Mob(m) = &z.kind else { continue };
        if m.ty != redsim::entity::mob::types::MobType::Zombie {
            continue;
        }
        let d = ((z.pos.x - e.pos.x).powi(2) + (z.pos.y - e.pos.y).powi(2) + (z.pos.z - e.pos.z).powi(2)).sqrt();
        if d < 10.0 {
            let sight = redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(z.pos.x, z.pos.y + 1.74, z.pos.z));
            out.push_str(&format!(" [zombie at {:.1} {:.2} {:.1}: {d:.2} away, sight {sight}]", z.pos.x, z.pos.y, z.pos.z));
        }
    }
    out
}

/// The seven batches cycling on the 100-gt slot grid: the counter's lines ideal (each switch
/// 1 + jitter gt after a slot: the next batch's hold ends powered and its stand ends braked, the
/// last batch's the other way), every villager checked every tick.
pub fn bench(spec: &FarmSpec, lanes: &LaneOpts, o: &BenchOpts) -> Result<BenchReport, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, o.days * 24000, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let bed = lanes.bed();
    // every villager: slept last night, HOME the shared bed (the R1 claims: the bed's one ticket
    // taken, every villager's memory on it)
    let t0 = run.world.time;
    redsim::entity::mob::ai::poi::take(&mut run.world, bed);
    let mut riders: Vec<(u32, i32)> = Vec::new();
    for i in 0..run.world.entities.list.len() {
        let e = &run.world.entities.list[i];
        let EntityKind::Mob(m) = &e.kind else { continue };
        if m.ty != redsim::entity::mob::types::MobType::Villager {
            continue;
        }
        let lane = (0..BATCHES).find(|&k| (e.pos.x - (lanes.x(k) as f64 + 0.5)).abs() < 0.4).ok_or_else(|| format!("a villager at x {:.2} is in no lane", e.pos.x))?;
        riders.push((e.id, lane));
        let b = brain::brain_mut(&mut run.world, i);
        b.home = Some(bed);
        b.last_slept = Some(t0 - 100);
    }
    if riders.len() != (lanes.levels * BATCHES) as usize {
        return Err(format!("{} villagers, not {}", riders.len(), lanes.levels * BATCHES));
    }
    // the slot grid and the line switches
    let mut rng = redsim::rng::LegacyRandom::new(o.seed ^ 0x686f_6c64);
    let s0 = ((t0 + 150) / 100 + 1) * 100;
    let end = t0 + o.days as i64 * 24000;
    let mut slots: Vec<i64> = Vec::new();
    let mut s = s0;
    while s < end {
        slots.push(s);
        s += 100;
    }
    let switch_at = |n: usize, rng: &mut redsim::rng::LegacyRandom| -> i64 {
        let base = if n == 0 { slots[0] - 99 } else { slots[n - 1] + 1 };
        base + if o.jitter > 0 { rng.next_int(o.jitter as i32 + 1) as i64 } else { 0 }
    };
    let switches: Vec<i64> = (0..slots.len()).map(|n| switch_at(n, &mut rng)).collect();
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let set_line = |w: &mut redsim::world::World, k: i32, high: bool| {
        let fl = redsim::world::flags::ALL;
        for p in lanes.hold_ctrls(k) {
            w.set_block(p, if high { on.clone() } else { off.clone() }, fl);
        }
        for p in lanes.stand_ctrls(k) {
            w.set_block(p, if high { off.clone() } else { on.clone() }, fl);
        }
    };
    let mut rep = BenchReport { min_margin: i64::MAX, stand_z: (f64::MAX, f64::MIN), hold_min_z: f64::MAX, ..Default::default() };
    let violate = |rep: &mut BenchReport, t: i64, what: String| {
        rep.violation_count += 1;
        if rep.violations.len() < 12 {
            rep.violations.push(format!("gt {t}: {what}"));
        }
    };
    let mut seen_cells: std::collections::HashSet<BlockPos> = Default::default();
    let mut next_switch = 0usize;
    let mut next_slot = 0usize;
    let mut arrived: Option<i64> = None;
    let mut margins: Vec<i64> = Vec::new();
    let mut selected: Option<i32> = None;
    let index_of = |w: &redsim::world::World, id: u32| w.entities.list.iter().position(|e| e.id == id);
    while run.world.time < end {
        let t_next = run.world.time + 1;
        // the line switch due this tick
        while next_switch < switches.len() && switches[next_switch] <= t_next {
            let k = (next_switch % BATCHES as usize) as i32;
            if let Some(p) = selected {
                set_line(&mut run.world, p, false);
            }
            set_line(&mut run.world, k, true);
            selected = Some(k);
            arrived = None;
            next_switch += 1;
        }
        // the slot about to tick: the selected batch at its stand cells, every other rider held
        if next_slot < slots.len() && slots[next_slot] == t_next {
            let k = (next_slot % BATCHES as usize) as i32;
            let w = &run.world;
            let mut stand: Vec<Vec3> = Vec::new();
            for (id, lane) in &riders {
                let Some(i) = index_of(w, *id) else { continue };
                let p = w.entities.list[i].pos;
                if *lane == k {
                    if p.z >= lanes.stand_z() {
                        violate(&mut rep, t_next, format!("batch {k}'s rider {id} not at the station (z {:.2})", p.z));
                    }
                    rep.stand_z = (rep.stand_z.0.min(p.z), rep.stand_z.1.max(p.z));
                    stand.push(p);
                }
            }
            for (id, lane) in &riders {
                if *lane == k {
                    continue;
                }
                let Some(i) = index_of(w, *id) else { continue };
                let p = w.entities.list[i].pos;
                rep.hold_min_z = rep.hold_min_z.min(p.z);
                if p.z < lanes.hold_z() {
                    violate(&mut rep, t_next, format!("batch {lane}'s rider {id} at z {:.2}, inside the golems' follow box (z < {:.1})", p.z, lanes.hold_z()));
                }
                if stand.iter().any(|q| (q.x - p.x).abs() <= 10.6 && (q.z - p.z).abs() <= 10.6 && (q.y - p.y).abs() <= 11.95) {
                    violate(&mut rep, t_next, format!("batch {lane}'s rider {id} inside the summoner's 10-box"));
                }
                if (p.x - (lanes.x(*lane) as f64 + 0.5)).abs() > 0.4 {
                    violate(&mut rep, t_next, format!("rider {id} out of its lane {lane} (x {:.2})", p.x));
                }
            }
            match arrived {
                Some(a) => margins.push(t_next - a),
                None => violate(&mut rep, t_next, format!("batch {k} not arrived")),
            }
            if o.verbose {
                let at: Vec<String> = stand.iter().map(|p| format!("({:.2} {:.2} {:.2})", p.x, p.y, p.z)).collect();
                println!("slot {} gt {t_next}: batch {k}, arrived {:?}, riders {}", next_slot, arrived.map(|a| t_next - a), at.join(" "));
            }
            rep.slots += 1;
            next_slot += 1;
        }
        run.tick();
        let w = &run.world;
        let t = w.time;
        // every rider, every tick
        let mut at_station: std::collections::BTreeSet<i32> = Default::default();
        let mut all_in = true;
        for (id, lane) in &riders {
            let Some(i) = index_of(w, *id) else {
                violate(&mut rep, t, format!("rider {id} gone"));
                continue;
            };
            let e = &w.entities.list[i];
            if e.vehicle.is_none() {
                rep.off_cart += 1;
                if rep.off_cart == 1 {
                    violate(&mut rep, t, format!("rider {id} of batch {lane} off its cart at {:.2} {:.2} {:.2}", e.pos.x, e.pos.y, e.pos.z));
                }
            }
            if e.pos.z < lanes.stand_z() {
                at_station.insert(*lane);
            } else if Some(*lane) == selected {
                all_in = false;
            }
            if brain::has_exact(w, i) {
                let x = brain::ex(w, i);
                if x.active != Act::Panic {
                    rep.calm_ticks += 1;
                    let c = e.block_position();
                    let night = (t % 24000) >= 12000;
                    *rep.calm_at.entry((c.x, c.y, c.z, night)).or_insert(0) += 1;
                    if o.verbose && night && rep.calm_printed < 8 {
                        rep.calm_printed += 1;
                        println!("calm gt {t}: rider {} ({:?}) at {:.2} {:.2} {:.2}, hostile memory {:?};{}", e.id, x.active, e.pos.x, e.pos.y, e.pos.z, brain::brain(w, i).nearest_hostile, zombies_near(w, i));
                    }
                }
                if x.active == Act::Rest {
                    rep.rest_ticks += 1;
                    if let Some(since) = x.mem.cant_reach_since {
                        rep.max_cant_reach_at_rest = rep.max_cant_reach_at_rest.max(t - since);
                    }
                }
            }
            let cell = e.block_position();
            if seen_cells.insert(cell) {
                for c in spots_from(w, cell) {
                    if !rep.spawn_spots.contains(&c) {
                        rep.spawn_spots.push(c);
                    }
                }
            }
        }
        if at_station.len() > 1 {
            violate(&mut rep, t, format!("batches {at_station:?} at the station at once"));
        }
        if selected.is_some() && all_in && arrived.is_none() {
            arrived = Some(t);
        }
        if selected.is_some() && !all_in {
            arrived = None;
        }
    }
    rep.mean_margin = if margins.is_empty() { 0.0 } else { margins.iter().sum::<i64>() as f64 / margins.len() as f64 };
    rep.min_margin = margins.iter().copied().min().unwrap_or(0);
    rep.errors = run.world.errors.clone();
    Ok(rep)
}
