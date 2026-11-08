//! The mob-farm runner (ironfarm): a farm built in a redsim world (blocks, its villagers, zombie
//! and vehicles, the judge's output chest), run for a number of game ticks with the mob layer
//! seeded, and every must-have of the brief (`designs/briefs/farm-iron.md`) checked as it runs:
//!
//! * no golem leaves the build, gets within melee reach of the zombie or touches a vehicle, or
//!   lives longer than [`MAX_GOLEM_LIFE`];
//! * no villager or zombie takes damage, no villager moves beyond its pod;
//! * no drop touches lava, comes within the zombie's pickup reach or ages to its despawn; at the
//!   end every iron ingot dropped is in the output chest or still in the farm's containers;
//! * the villagers start sleeping at least once every game day.
//!
//! Runs start in the steady state ([`RunOpts::steady`]): a day time in the day, every
//! villager's `LAST_SLEPT` at the end of the last REST. What the model cannot judge is refused
//! before any run ([`super::refuse::static_checks`]); the golem exit flood is
//! [`super::exit::golem_exit`]. The caller (the circuit study `mobfarm`) turns the design into a
//! [`FarmSpec`] and the reports into its card.

use super::types::MobType;
use super::walk::Walk;
use super::{MobEvent, mob};
use crate::block::Block;
use crate::entity::{Aabb, EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;

/// A golem alive longer than this (gt) fails the run: it is not being killed.
pub const MAX_GOLEM_LIFE: i64 = 1200;
/// Game ticks per hour at vanilla speed (20 gt/s).
pub const TICKS_PER_HOUR: f64 = 72000.0;

/// A farm as the runner builds it.
#[derive(Clone, Debug)]
pub struct FarmSpec {
    /// The design's blocks, placed like the judge places a design (raw, then updates).
    pub blocks: Vec<(BlockPos, Block)>,
    /// Its entities (`entity:` lines): type id, position, SNBT.
    pub entities: Vec<(String, [f64; 3], Option<String>)>,
    /// The judge's chest (an air cell of the design).
    pub output: BlockPos,
    /// The simulated world box (inclusive): the design's box with a margin.
    pub lo: BlockPos,
    pub hi: BlockPos,
    /// Its block entities (`nbt:` lines: position, SNBT), applied after the raw paste and before
    /// the updates, like the game check's placement (a sorter's filter items).
    pub block_entities: Vec<(BlockPos, String)>,
}

impl FarmSpec {
    /// The box a golem must stay in: the world box less its outer cell.
    pub fn inner(&self) -> Aabb {
        Aabb::new(self.lo.x as f64 + 1.0, self.lo.y as f64 + 1.0, self.lo.z as f64 + 1.0, self.hi.x as f64, self.hi.y as f64, self.hi.z as f64)
    }
}

/// How the golems of a run walk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WalkMode {
    /// Each new golem draws a policy ([`Walk`]: still, or one of 8 directions at the goal speeds
    /// 1.0 and 0.6) from the run's walk random.
    Random,
    /// Every golem of the run walks under this one policy (the worst-policy runs).
    Fixed(Walk),
}

/// One run's options.
#[derive(Clone, Copy, Debug)]
pub struct RunOpts {
    /// The mob layer's seed.
    pub seed: i64,
    /// Game ticks to run.
    pub ticks: u32,
    /// The day time at the start (the judge draws it in the day, 10..12000: the steady state).
    pub day0: i64,
    pub walk: WalkMode,
    /// The steady state: every villager's `LAST_SLEPT` at the end of the last REST (day time 10).
    /// `false`: none (the restart probe: villagers that missed a night).
    pub slept: bool,
}

impl RunOpts {
    /// The judge's run `seed`: the day time drawn from the seed in the day (10..12000), slept.
    pub fn steady(seed: i64, ticks: u32, walk: WalkMode) -> RunOpts {
        let mut r = crate::rng::LegacyRandom::new(seed ^ 0x6461_7930);
        RunOpts { seed, ticks, day0: 10 + r.next_int(11990) as i64, walk, slept: true }
    }
}

/// The misses of one run.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Losses {
    /// Spawn tries (one villager's 10 attempts) that found no spot.
    pub no_spot: u32,
    /// Golem sensor sightings.
    pub seen: u32,
}

/// One run's record and verdict.
#[derive(Clone, Default, Debug)]
pub struct RunReport {
    pub ticks: u32,
    pub golems: u32,
    pub deaths: u32,
    pub iron_dropped: u32,
    pub poppies_dropped: u32,
    pub iron_in_chest: u32,
    pub iron_in_containers: u32,
    /// Every container holding iron at the end (position, iron as ingots: blocks x 9).
    pub iron_where: Vec<([i32; 3], u32)>,
    /// Every non-empty container's contents at the end (`x y z: count item, ...`).
    pub contents: Vec<String>,
    /// The game time of every golem spawn.
    pub spawn_times: Vec<i64>,
    /// The cell of every golem spawn.
    pub spawn_cells: Vec<[i32; 3]>,
    /// Spawn-to-death times.
    pub lifetimes: Vec<i64>,
    pub losses: Losses,
    /// The longest a villager went without starting to sleep (gt), over the run.
    pub max_sleep_gap: i64,
    /// Sleeps started.
    pub sleeps: u32,
    /// Must-have failures (first ones), each with its game time.
    pub failures: Vec<String>,
    /// Model errors (the world's), each once.
    pub errors: Vec<String>,
}

impl RunReport {
    pub fn hours(&self) -> f64 {
        self.ticks as f64 / TICKS_PER_HOUR
    }

    pub fn iron_per_hour(&self) -> f64 {
        self.iron_dropped as f64 / self.hours()
    }

    pub fn golems_per_hour(&self) -> f64 {
        self.golems as f64 / self.hours()
    }

    pub fn passed(&self) -> bool {
        self.failures.is_empty() && self.errors.is_empty()
    }

    pub(crate) fn fail(&mut self, msg: String) {
        if self.failures.len() < 8 && !self.failures.contains(&msg) {
            self.failures.push(msg);
        }
    }
}

/// The judge's first seed: its run k of build b is seeded [`judge_seed`]`(b, k)`.
pub const JUDGE_SEED0: i64 = 0x6972_6f6e;

/// The seed of the judge's run `k` of build `build` (circuit::mobfarm; the 3D viewer plays run 0
/// of build 0).
pub fn judge_seed(build: u32, k: u32) -> i64 {
    JUDGE_SEED0 + build as i64 * 1000 + k as i64
}

/// The judge's options for one base run (circuit::mobfarm builds every run from it, and the 3D
/// viewer, which cannot link the circuit crate, calls it too): today the steady state of `seed`.
pub fn judge_opts(seed: i64, ticks: u32, walk: WalkMode) -> RunOpts {
    RunOpts::steady(seed, ticks, walk)
}

/// Game ticks the built farm runs before its entities arrive, so its water has spread (a
/// player's farm is built long before it runs; each flowing step takes 5 gt).
pub const SETTLE_TICKS: i64 = 200;

/// One farm run, tick by tick: the judge's runner and the 3D viewer's (one runner, so the view
/// cannot drift from the judge; mobviewer 2026-11-08). [`Run::start`] builds the farm up to its
/// water's settle, [`Run::settle_tick`] runs one of the [`SETTLE_TICKS`] (a browser spreads them
/// over frames), [`Run::arrive`] brings the daylight detectors, the entities, each villager's
/// HOME and LAST_SLEPT and the golems' walking; [`Run::tick`] is one game tick (a new walking
/// policy first when golems walk at random). [`Run::new`] is the three in order, [`build`] its
/// world. Clone: the viewer snapshots a run (the world with its walk random) for rewind and reset.
#[derive(Clone)]
pub struct Run {
    pub world: World,
    spec: FarmSpec,
    o: RunOpts,
    detectors: Vec<(BlockPos, Block)>,
    settle_left: i64,
    walk_rng: crate::rng::LegacyRandom,
}

impl Run {
    /// The farm's blocks (raw, then updates; the daylight detectors held back: they would read
    /// the sky light while the water spreads, which the light engine's timing check refuses), its
    /// output chest (the design's own stays as built; an air output gets the judge's), emptied;
    /// the mob layer seeded with `o.seed` at day time `o.day0 - SETTLE_TICKS`.
    pub fn start(spec: &FarmSpec, o: &RunOpts) -> Result<Run, String> {
        let size = [spec.hi.x - spec.lo.x + 1, spec.hi.y - spec.lo.y + 1, spec.hi.z - spec.lo.z + 1];
        let mut w = World::new(spec.lo, size);
        w.mob.seed(o.seed, o.day0 - SETTLE_TICKS);
        // the level random (`Level.random`): a composter's chance, a dropper's slot (the shared
        // collection's), drawn from a stand-in seeded from the run like the mob layer's
        w.level_random = Some(crate::rng::LegacyRandom::new(o.seed ^ 0x6c76_6c72));
        let (detectors, rest): (Vec<_>, Vec<_>) = spec.blocks.iter().cloned().partition(|(_, b)| matches!(b, Block::DaylightDetector { .. }));
        w.place_raw(&rest);
        crate::redstone::container::apply_block_entities(&mut w, spec.block_entities.iter().map(|(p, s)| (*p, s.as_str())))?;
        w.place_updates(&rest);
        let chest = Block::parse("minecraft:chest[facing=north,type=single,waterlogged=false]")?;
        if !matches!(w.get(spec.output), Block::Chest { .. }) {
            w.set_block(spec.output, chest, crate::world::flags::RAW);
        }
        crate::redstone::container::apply_block_entities(&mut w, [(spec.output, "{Items:[]}")])?;
        Ok(Run { world: w, spec: spec.clone(), o: *o, detectors, settle_left: SETTLE_TICKS, walk_rng: crate::rng::LegacyRandom::new(o.seed ^ 0x5741_4c4b) })
    }

    /// The settle's ticks left.
    pub fn settle_left(&self) -> i64 {
        self.settle_left
    }

    /// One of the settle's ticks; false once none is left (nothing ran).
    pub fn settle_tick(&mut self) -> bool {
        if self.settle_left <= 0 {
            return false;
        }
        self.world.tick();
        self.settle_left -= 1;
        true
    }

    /// The rest of the build (any settle tick left first): the daylight detectors, the entities,
    /// each villager's HOME (the bed head nearest its feet within 2.5) and, in the steady state, a
    /// LAST_SLEPT at the end of the last REST (day time 10); a fixed walking policy.
    pub fn arrive(&mut self) -> Result<(), String> {
        while self.settle_tick() {}
        let w = &mut self.world;
        w.place_raw(&self.detectors);
        w.place_updates(&self.detectors);
        for (ty, pos, nbt) in &self.spec.entities {
            crate::entity::spawn(w, ty, *pos, nbt.as_deref())?;
        }
        let slept = w.time - (self.o.day0 - 10).rem_euclid(24000);
        let villagers: Vec<usize> = (0..w.entities.list.len()).filter(|&i| matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ty == MobType::Villager)).collect();
        let mut taken: Vec<BlockPos> = Vec::new();
        for i in villagers {
            let feet = w.entities.list[i].pos;
            let cell = w.entities.list[i].block_position();
            let heads = super::blocks::bed_heads(w, cell.offset(-3, -2, -3), cell.offset(3, 2, 3));
            let home = heads
                .into_iter()
                .filter(|h| !taken.contains(h))
                .map(|h| (Vec3::new(h.x as f64 + 0.5, h.y as f64 + 0.5, h.z as f64 + 0.5).distance_to_sqr(feet), h))
                .filter(|(d, _)| *d < 2.5 * 2.5)
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, h)| h);
            let Some(home) = home else { return Err(format!("the villager at {:.2} {:.2} {:.2} has no bed of its own within reach", feet.x, feet.y, feet.z)) };
            taken.push(home);
            if let EntityKind::Mob(m) = &mut w.entities.list[i].kind
                && let Some(b) = m.brain.as_deref_mut()
            {
                b.home = Some(home);
                b.last_slept = if self.o.slept { Some(slept) } else { None };
            }
        }
        if let WalkMode::Fixed(p) = self.o.walk {
            w.mob.golem_walk = p;
        }
        Ok(())
    }

    /// [`Run::start`], every settle tick, [`Run::arrive`].
    pub fn new(spec: &FarmSpec, o: &RunOpts) -> Result<Run, String> {
        let mut r = Run::start(spec, o)?;
        r.arrive()?;
        Ok(r)
    }

    /// One game tick: a new walking policy for the next golem first when golems walk at random
    /// (drawn from the run's walk random, seeded `o.seed ^ 0x5741_4c4b`), then the world's tick.
    pub fn tick(&mut self) {
        if self.o.walk == WalkMode::Random {
            self.world.mob.golem_walk = pick_walk(&mut self.walk_rng);
        }
        self.world.tick();
    }
}

/// Build the farm's world, ready to run: [`Run::new`]'s world.
pub fn build(spec: &FarmSpec, o: &RunOpts) -> Result<World, String> {
    Run::new(spec, o).map(|r| r.world)
}

/// The tracked state of a run.
struct Track {
    villager_home: Vec<(u32, Vec3)>,
    golem_born: Vec<(u32, i64)>,
    drops: Vec<u32>,
    last_sleep: Vec<(u32, i64)>,
    /// The iron the farm's containers hold when it starts (a sorter's filter items).
    seed_iron: u32,
}

/// The iron in the farm's containers (`ingots + 9 x blocks`): (in the output chest, in the others).
fn container_iron(w: &World, output: BlockPos) -> (u32, u32) {
    use crate::redstone::container as rc;
    let (ingot, block) = (rc::item_id("iron_ingot"), rc::item_id("iron_block"));
    let count = |c: &rc::ContainerBe| c.slots.iter().flatten().map(|s| if Some(s.item) == ingot { s.count as u32 } else if Some(s.item) == block { 9 * s.count as u32 } else { 0 }).sum::<u32>();
    let (mut out, mut rest) = (0, 0);
    for (p, c) in &w.containers.map {
        if *p == output {
            out += count(c);
        } else {
            rest += count(c);
        }
    }
    for c in w.containers.entities.values() {
        rest += count(c);
    }
    (out, rest)
}

/// Run a farm: `o.ticks` game ticks, every check as it goes.
pub fn run(spec: &FarmSpec, o: &RunOpts) -> Result<RunReport, String> {
    let mut r = Run::new(spec, o)?;
    let mut rep = RunReport { ticks: o.ticks, ..Default::default() };
    let mut tr = Track { villager_home: Vec::new(), golem_born: Vec::new(), drops: Vec::new(), last_sleep: Vec::new(), seed_iron: container_iron(&r.world, spec.output).1 };
    for e in &r.world.entities.list {
        if let EntityKind::Mob(m) = &e.kind
            && m.ty == MobType::Villager
        {
            tr.villager_home.push((e.id, e.pos));
            tr.last_sleep.push((e.id, m.brain.as_deref().and_then(|b| b.last_slept).unwrap_or(0)));
        }
    }
    for _ in 0..o.ticks {
        r.tick();
        let t = r.world.time;
        absorb_events(&mut r.world, &mut rep, &mut tr, t);
        check_tick(&r.world, &mut rep, &mut tr, spec, t);
        if rep.failures.len() >= 4 {
            break;
        }
    }
    finish(&r.world, &mut rep, &tr, spec);
    Ok(rep)
}

/// The player away for days in the middle of a run (brief, Stages: Middle).
#[derive(Clone, Copy, Debug)]
pub struct AwayPlan {
    /// Game ticks the farm runs before the player leaves (then up to 200 more, to a tick with no
    /// block tick pending).
    pub before: i64,
    /// At least this many days away.
    pub days: i64,
    /// The day time the player comes back at (6000: noon; 18000: midnight).
    pub back_at: i64,
    pub how: super::absence::Away,
    /// Game ticks the farm runs after the return.
    pub after: i64,
}

/// What the farm did after the player came back.
#[derive(Clone, Debug, Default)]
pub struct AwayReport {
    /// Game ticks from the return to the first golem (`None`: none in [`AwayPlan::after`]).
    pub first_golem: Option<i64>,
    /// Golems after the return.
    pub golems: u32,
}

/// [`run`] with the player away in the middle ([`AwayPlan`]): every check applies before and after
/// the absence, and every villager gets [`super::villager::SLEPT_WITHIN`] + 6000 gt from the
/// return to sleep again (the farm restarts by itself, or the run fails on the night it misses).
pub fn run_away(spec: &FarmSpec, o: &RunOpts, plan: AwayPlan) -> Result<(RunReport, AwayReport), String> {
    let mut r = Run::new(spec, o)?;
    let mut rep = RunReport { ticks: (plan.before + plan.after) as u32, ..Default::default() };
    let mut tr = Track { villager_home: Vec::new(), golem_born: Vec::new(), drops: Vec::new(), last_sleep: Vec::new(), seed_iron: container_iron(&r.world, spec.output).1 };
    for e in &r.world.entities.list {
        if let EntityKind::Mob(m) = &e.kind
            && m.ty == MobType::Villager
        {
            tr.villager_home.push((e.id, e.pos));
            tr.last_sleep.push((e.id, m.brain.as_deref().and_then(|b| b.last_slept).unwrap_or(0)));
        }
    }
    let step = |r: &mut Run, rep: &mut RunReport, tr: &mut Track| {
        r.tick();
        let t = r.world.time;
        absorb_events(&mut r.world, rep, tr, t);
        check_tick(&r.world, rep, tr, spec, t);
    };
    for _ in 0..plan.before {
        step(&mut r, &mut rep, &mut tr);
    }
    let mut quiet = 0;
    while r.world.ticks.count() + r.world.fluid_ticks.count() > 0 {
        if quiet == 200 {
            return Err(format!("no tick without a pending block tick in the 200 after gt {}", plan.before));
        }
        step(&mut r, &mut rep, &mut tr);
        quiet += 1;
    }
    let lands = r.world.day_time() + plan.days * 24000;
    let len = plan.days * 24000 + (plan.back_at - lands).rem_euclid(24000);
    super::absence::away(&mut r.world, len, plan.how)?;
    let back = r.world.time;
    for (_, last) in tr.last_sleep.iter_mut() {
        *last = back;
    }
    // a golem alive when the player left lived no game ticks while away
    for (_, born) in tr.golem_born.iter_mut() {
        *born += len;
    }
    for _ in 0..plan.after {
        step(&mut r, &mut rep, &mut tr);
        if rep.failures.len() >= 4 {
            break;
        }
    }
    finish(&r.world, &mut rep, &tr, spec);
    let after: Vec<i64> = rep.spawn_times.iter().copied().filter(|t| *t > back).collect();
    Ok((rep, AwayReport { first_golem: after.first().map(|t| t - back), golems: after.len() as u32 }))
}

/// A random walking policy for the next golem (`WalkMode::Random`).
fn pick_walk(r: &mut crate::rng::LegacyRandom) -> Walk {
    let k = r.next_int(17);
    if k == 0 {
        Walk::Still
    } else {
        let modifier = if k <= 8 { 1.0 } else { 0.6 };
        Walk::Toward { yaw: ((k - 1) % 8) as f32 * 45.0, modifier }
    }
}

fn absorb_events(w: &mut World, rep: &mut RunReport, tr: &mut Track, t: i64) {
    let events = std::mem::take(&mut w.mob.events);
    for ev in events {
        match ev {
            MobEvent::GolemTry { golem: Some(id), at, .. } => {
                rep.golems += 1;
                rep.spawn_times.push(t);
                if let Some(a) = at {
                    rep.spawn_cells.push(a);
                }
                tr.golem_born.push((id, t));
            }
            MobEvent::GolemTry { golem: None, .. } => rep.losses.no_spot += 1,
            MobEvent::GolemSeen { .. } => rep.losses.seen += 1,
            MobEvent::Hurt { id, ty, src, amount, .. } if ty != MobType::IronGolem => {
                rep.fail(format!("gt {t}: the {} (entity {id}) took {amount} {} damage", ty.def().id, src.name()));
            }
            MobEvent::Death { id, ty: MobType::IronGolem, .. } => {
                rep.deaths += 1;
                if let Some(k) = tr.golem_born.iter().position(|(g, _)| *g == id) {
                    rep.lifetimes.push(t - tr.golem_born[k].1);
                    tr.golem_born.remove(k);
                }
            }
            MobEvent::Death { id, ty, .. } => rep.fail(format!("gt {t}: the {} (entity {id}) died", ty.def().id)),
            MobEvent::Drop { item, what, count, .. } => {
                match what {
                    "minecraft:iron_ingot" => rep.iron_dropped += count,
                    "minecraft:poppy" => rep.poppies_dropped += count,
                    _ => {}
                }
                tr.drops.push(item);
            }
            MobEvent::Slept { id, .. } => {
                rep.sleeps += 1;
                if let Some(s) = tr.last_sleep.iter_mut().find(|(v, _)| *v == id) {
                    rep.max_sleep_gap = rep.max_sleep_gap.max(t - s.1);
                    s.1 = t;
                }
            }
            _ => {}
        }
    }
}

/// The checks on one golem at game time `t` (also the exit flood's): inside the build, out of
/// the zombie's reach, off the vehicles. `None`: fine.
pub fn golem_breaks_rule(w: &World, g: usize, inner: Aabb, t: i64) -> Option<String> {
    let e = &w.entities.list[g];
    if mob(w, g).dead {
        return None;
    }
    if !inner.intersects(e.bb) || e.bb.min_y < inner.min_y {
        return Some(format!("gt {t}: a golem left the build at {:.1} {:.1} {:.1}", e.pos.x, e.pos.y, e.pos.z));
    }
    for z in w.entities.list.iter().filter(|z| matches!(&z.kind, EntityKind::Mob(m) if m.ty == MobType::Zombie)) {
        if attack_box(e.bb, None).intersects(z.bb) {
            return Some(format!("gt {t}: a golem at {:.1} {:.1} {:.1} can reach the zombie", e.pos.x, e.pos.y, e.pos.z));
        }
    }
    for v in &w.entities.list {
        if matches!(v.kind, EntityKind::Boat(_) | EntityKind::Minecart(_)) && v.bb.intersects(e.bb) {
            return Some(format!("gt {t}: a golem touches a vehicle at {:.1} {:.1} {:.1}", v.pos.x, v.pos.y, v.pos.z));
        }
    }
    None
}

/// The per-tick checks.
fn check_tick(w: &World, rep: &mut RunReport, tr: &mut Track, spec: &FarmSpec, t: i64) {
    let zombie = w.entities.list.iter().find(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Zombie));
    let zbox = zombie.map(|z| z.bb);
    let inner = spec.inner();
    for (g, e) in w.entities.list.iter().enumerate() {
        let EntityKind::Mob(m) = &e.kind else { continue };
        match m.ty {
            MobType::IronGolem => {
                if let Some(why) = golem_breaks_rule(w, g, inner, t) {
                    rep.fail(why);
                }
                if let Some((_, born)) = tr.golem_born.iter().find(|(id, _)| *id == e.id)
                    && t - born > MAX_GOLEM_LIFE
                {
                    rep.fail(format!("gt {t}: a golem spawned at gt {born} is still alive (not killed within {MAX_GOLEM_LIFE} gt)"));
                }
            }
            MobType::Villager => {
                if let Some((_, home)) = tr.villager_home.iter().find(|(v, _)| *v == e.id)
                    && m.sleeping_pos.is_none()
                    && (e.pos.x - home.x).abs().max((e.pos.z - home.z).abs()) > 1.0
                {
                    rep.fail(format!("gt {t}: a villager moved from {:.2} {:.2} to {:.2} {:.2} (outside its pod)", home.x, home.z, e.pos.x, e.pos.z));
                }
            }
            MobType::Zombie => {}
            // (a witch, a struck villager, and natural mobs: the weather and terrain tracks'
            // checks)
            MobType::Witch | MobType::Natural(_) => {}
        }
    }
    // the drops: lava, the zombie's pickup reach, despawning
    tr.drops.retain(|id| w.entities.list.iter().any(|e| e.id == *id && !e.removed));
    for id in &tr.drops {
        let Some(e) = w.entities.list.iter().find(|e| e.id == *id) else { continue };
        if let Some(z) = zbox
            && z.inflate(1.0, 0.0, 1.0).intersects(e.bb)
        {
            rep.fail(format!("gt {t}: a drop at {:.1} {:.1} {:.1} is within the zombie's pickup reach", e.pos.x, e.pos.y, e.pos.z));
        }
        if touches_lava(w, e.bb) {
            rep.fail(format!("gt {t}: a drop touched lava at {:.1} {:.1} {:.1} (it burns)", e.pos.x, e.pos.y, e.pos.z));
        }
        if let EntityKind::Item(s) = &e.kind
            && s.age > 5800
        {
            rep.fail(format!("gt {t}: a drop at {:.1} {:.1} {:.1} is about to despawn (never collected)", e.pos.x, e.pos.y, e.pos.z));
        }
    }
    // the night's sleep: a villager that sleeps every night, a few ticks later than the night
    // before, is unwilling for those ticks only (`golemSpawnConditionsMet`, which the rate counts);
    // the run fails when one has missed a night (half a day past the 24000 gt)
    if t % 1000 == 0 {
        for (id, last) in &tr.last_sleep {
            if t - last > super::villager::SLEPT_WITHIN + 6000 {
                rep.fail(format!("gt {t}: villager {id} has not slept for {} gt (no golems: golemSpawnConditionsMet)", t - last));
            }
        }
    }
}

/// `Mob.getAttackBoundingBox(DEFAULT_ATTACK_REACH)`: the horizontal union of the mob's box and
/// its vehicle's (if it rides one), over the mob's own vertical span, inflated horizontally by
/// `sqrt(2.04) - 0.6` (0.828), the reach of a melee attack (`isWithinMeleeAttackRange`).
pub fn attack_box(own: Aabb, vehicle: Option<Aabb>) -> Aabb {
    let r = (2.04f32 as f64).sqrt() - 0.6f32 as f64;
    let b = match vehicle {
        Some(v) => Aabb::new(own.min_x.min(v.min_x), own.min_y, own.min_z.min(v.min_z), own.max_x.max(v.max_x), own.max_y, own.max_z.max(v.max_z)),
        None => own,
    };
    b.inflate(r, 0.0, r)
}

/// Whether a box meets a lava cell's fluid (its height, 8/9 for a source).
pub fn touches_lava(w: &World, bb: Aabb) -> bool {
    let (x0, y0, z0) = (bb.min_x.floor() as i32, bb.min_y.floor() as i32, bb.min_z.floor() as i32);
    let (x1, y1, z1) = (bb.max_x.floor() as i32, bb.max_y.floor() as i32, bb.max_z.floor() as i32);
    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                let p = BlockPos::new(x, y, z);
                if let Some(h) = super::blocks::lava_height(w, p)
                    && Aabb::new(x as f64, y as f64, z as f64, (x + 1) as f64, y as f64 + h, (z + 1) as f64).intersects(bb)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// The end of a run: the iron in the chest and in the farm's containers against what dropped.
fn finish(w: &World, rep: &mut RunReport, tr: &Track, spec: &FarmSpec) {
    let (out, rest) = container_iron(w, spec.output);
    rep.iron_in_chest += out;
    rep.iron_in_containers += rest.saturating_sub(tr.seed_iron);
    {
        use crate::redstone::container as rc;
        let (ingot, block) = (rc::item_id("iron_ingot"), rc::item_id("iron_block"));
        for (p, c) in &w.containers.map {
            let n: u32 = c.slots.iter().flatten().map(|s| if Some(s.item) == ingot { s.count as u32 } else if Some(s.item) == block { 9 * s.count as u32 } else { 0 }).sum();
            if n > 0 || c.slots.iter().flatten().count() > 0 {
                rep.iron_where.push(([p.x, p.y, p.z], n));
                rep.contents.push(format!("{} {} {}: {}", p.x, p.y, p.z, c.slots.iter().flatten().map(|s| format!("{} {}", s.count, rc::item_name(s.item))).collect::<Vec<_>>().join(", ")));
            }
        }
        rep.iron_where.sort();
    }
    let loose: u32 = tr
        .drops
        .iter()
        .filter_map(|id| w.entities.list.iter().find(|e| e.id == *id && !e.removed))
        .filter_map(|e| if let EntityKind::Item(s) = &e.kind { Some(s) } else { None })
        .filter(|s| s.item.ends_with("iron_ingot"))
        .map(|s| s.count as u32)
        .sum();
    let accounted = rep.iron_in_chest + rep.iron_in_containers + loose;
    if accounted != rep.iron_dropped && rep.failures.is_empty() {
        rep.fail(format!("{} iron ingots dropped, {} in the chest, {} in the farm's containers, {} loose: {} lost", rep.iron_dropped, rep.iron_in_chest, rep.iron_in_containers, loose, rep.iron_dropped as i64 - accounted as i64));
    }
    for e in &w.errors {
        if !rep.errors.contains(e) && rep.errors.len() < 8 {
            rep.errors.push(e.clone());
        }
    }
}
