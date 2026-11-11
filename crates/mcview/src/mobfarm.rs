//! Mob farms in the viewer (mobviewer, 2026-11-08; vjeux on the iron farms' "the view shows the
//! farm as built" caveat: "Why all of these caveats? Just do the real work!?"): the farm runs in
//! the view exactly as its judge runs it, through redsim's own farm runner
//! (`redsim::entity::mob::farm::Run`, the code `circuit::studies::mobfarm` judges with), never a
//! viewer-side approximation:
//!
//! * the judge's farm of the design as written ([`spec`] = `studies::mobfarm::spec_of`: its blocks,
//!   its entities, the judge's output chest, the watch box with its margin of 4);
//! * the judge's first run of its base build ([`run_opts`]: seed [`JUDGE_SEED`], the steady state,
//!   every golem walking under the judge's random policy), run on past the judge's 5 days;
//! * the water's settle (`farm::SETTLE_TICKS`) paused between ticks in the browser
//!   (`crate::staged`), the same ticks in the same order.
//!
//! `mctas web check` holds the viewer's farm to the circuit crate's spec and its world to redsim's
//! run tick for tick, natively and in the shipped wasm. What the view adds is the game client's
//! own drawing of that state ([`MobRun::draw`]: `LivingEntityRenderer`'s render state, the walk
//! animation from position deltas, `entitymodel` draws the models) and the readout's counts.

use crate::design::{Design, Pos};
use redsim::entity::mob::farm::{FarmSpec, HandHide, HandStep, Run, RunOpts, WalkMode};
use redsim::entity::mob::types::MobType;
use redsim::entity::mob::MobEvent;
use redsim::entity::EntityKind;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;

/// The judge's first seed, its run 0 of build 0 (the design as written: no turn, offset 0):
/// redsim's `farm::judge_seed(0, 0)`, the judge's own.
pub const JUDGE_SEED: i64 = redsim::entity::mob::farm::JUDGE_SEED0;
/// The judge's days per run (`studies::mobfarm::JUDGE_DAYS`).
pub const JUDGE_DAYS: u32 = 5;
/// The judge's world box margin (`spec_of`: `run::watch_box(c, 4)`).
pub const WATCH_MARGIN: i32 = 4;

/// A mob farm (`circuit: mob-farm <item> [tier]`).
pub fn is_mob_farm(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("mob-farm"))
}

/// The judge's farm of a design as written (`circuit::studies::mobfarm::spec_of`): its blocks
/// (sorted by x, y, z), its entities in file order, the chest at `port output`, the world box:
/// the circuit's bounds (blocks and ports) plus [`WATCH_MARGIN`], and its Nether part
/// (`World::nether`: blocks, output chest and data relative to the anchor's Nether image).
pub fn spec(d: &Design) -> Result<FarmSpec, String> {
    let out = d.ports.get("output").map(|(p, _)| *p).ok_or("no `port output`")?;
    let mut pts = d.blocks.keys().copied().chain(d.ports.values().map(|(p, _)| *p));
    let first = pts.next().ok_or("an empty design")?;
    let (mut lo, mut hi) = (first, first);
    for p in pts {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let m = WATCH_MARGIN;
    let mut blocks = Vec::with_capacity(d.blocks.len());
    for (q, s) in &d.blocks {
        blocks.push((bp(*q), farm_block(s).map_err(|e| format!("not modeled by redsim: {e}"))?));
    }
    blocks.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    // (the judge's `port home`, `studies::mobfarm::spec_of`: every villager holds HOME = that
    // bed, loaded as a memory, a cart's rider in its own compound)
    let home = d.ports.get("home").map(|(h, _)| format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}", h[0], h[1], h[2]));
    let mut entities = Vec::with_capacity(d.entities.len());
    for e in &d.entities {
        let mut nbt = e.nbt.clone();
        if let Some(h) = &home
            && e.kind.trim_start_matches("minecraft:") == "villager"
        {
            nbt = Some(match nbt.as_deref().map(str::trim) {
                Some(n) if n.contains("Brain:") => return Err(format!("a villager with its own Brain data and `port home`: {n}")),
                Some(n) if n.len() > 2 => format!("{},{h}}}", n.strip_suffix('}').unwrap_or(n)),
                _ => format!("{{{h}}}"),
            });
        }
        if let Some(h) = &home
            && let Some(n) = nbt.as_deref()
            && n.contains("Passengers:[")
        {
            nbt = Some(with_passenger_home(n, h)?);
        }
        entities.push((e.kind.clone(), e.pos, nbt));
    }
    let block_entities = d.block_entities.iter().map(|(p, s)| (bp(*p), s.clone())).collect();
    Ok(FarmSpec { blocks, entities, output: bp(out), lo: BlockPos::new(lo[0] - m, lo[1] - m, lo[2] - m), hi: BlockPos::new(hi[0] + m, hi[1] + m, hi[2] + m), block_entities, rock: d.meta.get("terrain").map(|t| t.trim()) == Some("rock"), nether: nether_spec(d)?, land: d.meta.contains_key("grounded") })
}

/// The judge's `port home` memory put into each villager passenger's own compound of a vehicle's
/// NBT (`studies::mobfarm::with_passenger_home`).
fn with_passenger_home(n: &str, home: &str) -> Result<String, String> {
    let key = "id:\"minecraft:villager\"";
    let mut out = String::new();
    let mut rest = n;
    while let Some(i) = rest.find(key) {
        let after = &rest[i + key.len()..];
        // the passenger's compound from its id to its closing brace (depth 0 again)
        let mut depth = 0i32;
        let mut end = after.len();
        for (k, c) in after.char_indices() {
            match c {
                '{' | '[' => depth += 1,
                '}' | ']' => {
                    if depth == 0 {
                        end = k;
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        if after[..end].contains("Brain:") {
            return Err(format!("a villager passenger with its own Brain data and `port home`: {n}"));
        }
        out.push_str(&rest[..i + key.len()]);
        out.push(',');
        out.push_str(home);
        rest = after;
    }
    out.push_str(rest);
    Ok(out)
}

/// The iron a Nether world's containers hold (ingots, an iron block as 9).
pub fn nether_iron(n: &World) -> u32 {
    let (ingot, block) = (redsim::redstone::container::item_id("iron_ingot"), redsim::redstone::container::item_id("iron_block"));
    n.containers.map.values().flat_map(|c| c.slots.iter().flatten()).map(|s| if Some(s.item) == ingot { s.count as u32 } else if Some(s.item) == block { 9 * s.count as u32 } else { 0 }).sum()
}

/// A farm block as the judge reads it (`studies::mobfarm::farm_block`): a nether portal through
/// redsim's terrain conversion (its axis), every other block through [`Block::parse`].
pub fn farm_block(s: &str) -> Result<Block, String> {
    if s.starts_with("minecraft:nether_portal[") {
        return redsim::terrain::land::block_of(s);
    }
    Block::parse(s)
}

/// The quarter turns of a design's Nether part (`mctas::design::Design::nether_turns`): its
/// `nether-axis` one step from its `nether-anchor` along +x (0), +z (1), -x (2) or -z (3).
pub fn nether_turns(d: &Design) -> Option<u8> {
    let a = d.ports.get("nether-anchor")?.0;
    let b = d.ports.get("nether-axis")?.0;
    match [b[0] - a[0], b[1] - a[1], b[2] - a[2]] {
        [1, 0, 0] => Some(0),
        [0, 0, 1] => Some(1),
        [-1, 0, 0] => Some(2),
        [0, 0, -1] => Some(3),
        _ => None,
    }
}

/// The judge's Nether part of a design as written (`studies::mobfarm::spec_of`'s): its blocks,
/// its output chest (`port nether-output`) and its data (`nether nbt:`) relative to the anchor's
/// Nether image, the anchor at `port nether-anchor`; None without one.
pub fn nether_spec(d: &Design) -> Result<Option<redsim::entity::mob::farm::NetherSpec>, String> {
    if d.nether_blocks.is_empty() {
        return Ok(None);
    }
    let turns = nether_turns(d).ok_or("a Nether part whose `nether-axis` is not one step along x or z from `nether-anchor`")?;
    // (every published design's part is as written, turns 0; a turned one would need the
    // judge's state rotation, `clock::canon::rotate_state`, which the view does not carry)
    if turns != 0 {
        return Err(format!("a Nether part turned {turns} quarter turn(s) from +x"));
    }
    let anchor = d.ports.get("nether-anchor").map(|(p, _)| *p).ok_or("a Nether part without `port nether-anchor`")?;
    let mut nb = Vec::with_capacity(d.nether_blocks.len());
    for (q, s) in &d.nether_blocks {
        nb.push((bp(*q), farm_block(s).map_err(|e| format!("the Nether part: not modeled by redsim: {e}"))?));
    }
    nb.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    let output = d.nether_ports.get("nether-output").map(|p| bp(*p));
    let block_entities = d.nether_block_entities.iter().map(|(p, s)| (bp(*p), s.clone())).collect();
    Ok(Some(redsim::entity::mob::farm::NetherSpec { anchor: bp(anchor), blocks: nb, output, block_entities }))
}

/// The judge's run of the base build with seed `seed` (steady state, random walking policies): for
/// [`JUDGE_SEED`] its first run (`studies::mobfarm::build_runs`' first job); in a terrain sample
/// (`site`) the judge's terrain run (`farm::judge_opts_in`: the site, the weather's cycle).
pub fn run_opts_in(seed: i64, site: Option<&'static redsim::entity::mob::site::Site>) -> RunOpts {
    // the judge's run options, from redsim (the circuit crate builds every run from them)
    redsim::entity::mob::farm::judge_opts_in(seed, JUDGE_DAYS * 24000, WalkMode::Random, site)
}

/// [`run_opts_in`] in the void.
pub fn run_opts_seeded(seed: i64) -> RunOpts {
    run_opts_in(seed, None)
}

/// The judge's first run of the base build ([`JUDGE_SEED`]).
pub fn run_opts() -> RunOpts {
    run_opts_seeded(JUDGE_SEED)
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// `WalkAnimationState` as the client keeps it for a living entity (`LivingEntity
/// .calculateEntityAnimation(false)` after each tick: the horizontal distance moved, x4, capped at
/// 1, eased by 0.4; stopped for a passenger or a dead mob).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WalkAnim {
    pub speed_old: f32,
    pub speed: f32,
    pub position: f32,
    /// A bat's animation state (`Bat.setupAnimationStates` each tick: the resting or the flying
    /// `AnimationState`, started at the tick it changed): whether it rests, since which tick.
    pub bat: Option<(bool, i32)>,
    /// A chicken's wings (`Chicken.aiStep` each tick from its `onGround`): oFlap, flap,
    /// oFlapSpeed, flapSpeed, flapping.
    pub chicken: Option<[f32; 5]>,
}

impl WalkAnim {
    fn update(&mut self, distance: f32) {
        let target = (distance * 4.0).min(1.0);
        self.speed_old = self.speed;
        self.speed += (target - self.speed) * 0.4;
        self.position += self.speed;
    }

    fn stop(&mut self) {
        *self = WalkAnim { bat: self.bat, chicken: self.chicken, ..WalkAnim::default() };
    }

    /// `position(partialTicks)`.
    pub fn position_at(&self, partial: f32) -> f32 {
        self.position - self.speed * (1.0 - partial)
    }

    /// `speed(partialTicks)`.
    pub fn speed_at(&self, partial: f32) -> f32 {
        (self.speed_old + (self.speed - self.speed_old) * partial).min(1.0)
    }
}

/// A mob farm's run in the view: redsim's runner (its world lives in `Session::world` between
/// ticks and is swapped in for each tick, [`MobRun::tick`]) and what the readout counts.
#[derive(Clone)]
pub struct MobRun {
    pub run: Run,
    /// The judge's output chest and every chest of the design (a scaled farm's copies have their
    /// own): where the readout counts the iron.
    pub chests: Vec<BlockPos>,
    /// Golems summoned since the farm's mobs arrived, and the iron and poppies dropped.
    pub golems: u32,
    pub iron_dropped: u32,
    pub poppies_dropped: u32,
    /// The game time the mobs arrived (the end of the settle).
    pub arrived: i64,
    /// Each living mob's walk animation (the client's, from its moves), by entity id.
    pub walk: HashMap<u32, WalkAnim>,
    /// The golem spawn spots now ([`spawn_spots`], found again when a villager changes cell or a
    /// block changes), and the golems spawned on each cell so far.
    pub spots: Vec<(BlockPos, Result<(), &'static str>)>,
    spots_key: Vec<BlockPos>,
    pub spawned: HashMap<BlockPos, u32>,
    /// The cells golems spawned on in the last tick.
    pub last_spawns: Vec<BlockPos>,
    /// Its farm as built (moved into its terrain, if any) and the farm's own mobs (the design's
    /// entities, by id, once they arrived).
    spec: FarmSpec,
    own: std::collections::HashSet<u32>,
    /// The card's hand step, as the judge's T run performs it (`farm::run_t`; the parent
    /// 2026-11-08: "Does the view perform the card's hand steps? If not, the view shows a night the
    /// player wouldn't have"): a farm whose design does not claim `restart: yes` has its zombie
    /// hidden by hand the first night ([`hand_hide_of`]). And what it did, at which game time (the
    /// timeline's marks).
    pub hand: Option<HandHide>,
    pub hand_events: Vec<(i64, HandStep)>,
    /// A part's bench run (`redsim::entity::mob::drive`: the page's `data-drive`): the bench's
    /// own start and its acts before each tick instead of the judge's run.
    pub drive: Option<Driven>,
    /// The iron the Nether part's containers held when the mobs arrived (the design's own stock:
    /// not the farm's).
    pub nether_seed_iron: u32,
    /// What a driven run did, for its check against the bench's expectations: each golem
    /// (`t` after t0, its id, its cell), each crossing to the Nether (`t`, the golem's id), each
    /// golem's death in the Nether (`t`, its Nether id).
    pub log: DriveLog,
}

/// A bench's drive in the view's run.
#[derive(Clone)]
pub struct Driven {
    pub drive: std::rc::Rc<redsim::entity::mob::drive::Drive>,
    /// Each entity line's entity, once spawned; the world's time then.
    pub lines: Vec<u32>,
    pub t0: i64,
    pub cursor: redsim::entity::mob::drive::Cursor,
}

/// A driven run's record ([`MobRun::log`]).
#[derive(Clone, Debug, Default)]
pub struct DriveLog {
    pub golems: Vec<(i64, u32, BlockPos)>,
    pub crossings: Vec<(i64, u32, u32)>,
    pub nether_deaths: Vec<(i64, u32)>,
    /// The golems' Nether copies still alive there.
    pub in_nether: Vec<u32>,
}

thread_local! {
    /// The drives a page gave its views (`data-drive`, by the design's `name:`): [`provide_drive`].
    static DRIVES: std::cell::RefCell<HashMap<String, std::rc::Rc<redsim::entity::mob::drive::Drive>>> = std::cell::RefCell::new(HashMap::new());
}

/// A part's bench drive for the design named `name` (a page's `data-drive`, fetched by the view;
/// a native check's file): its view's run is that bench's run ([`MobRun::start_driven`]).
pub fn provide_drive(name: &str, text: &str) -> Result<(), String> {
    let d = redsim::entity::mob::drive::Drive::parse(text)?;
    DRIVES.with(|m| m.borrow_mut().insert(name.to_string(), std::rc::Rc::new(d)));
    Ok(())
}

/// The drive given for the design named `name`, if any.
pub fn drive_for(name: &str) -> Option<std::rc::Rc<redsim::entity::mob::drive::Drive>> {
    DRIVES.with(|m| m.borrow().get(name).cloned())
}

/// A farm's card prints "hide the zombie by hand the first night" (its design does not claim
/// `restart: yes`: `circuit::mobfarm::claims_restart`): the view performs it like the T run.
pub fn hand_hide_of(d: &Design) -> bool {
    d.meta.get("restart").map(|v| v.trim()) != Some("yes")
}

/// The words of a hand step for the timeline and the badge.
pub fn hand_words(s: HandStep) -> &'static str {
    match s {
        HandStep::Hid => "you hide the zombie",
        HandStep::Restored => "you open the trapdoors again",
    }
}

impl MobRun {
    /// Start the judge's run of `d` with `seed` ([`JUDGE_SEED`]: its first): blocks placed, the
    /// chest, the settle left to run ([`MobRun::settle_tick`]); then [`MobRun::arrive`]. In its
    /// terrain (`terrain`): the judge's site, the farm moved into the sample's world.
    pub fn start(d: &Design, seed: i64, terrain: Option<&crate::terrain::TerrainFarm>) -> Result<MobRun, String> {
        let spec = spec(d)?;
        // (its void judge's static checks; a farm in its terrain is walled by the sample's rock)
        let refused = if terrain.is_none() { redsim::entity::mob::refuse::static_checks(&spec) } else { Vec::new() };
        if !refused.is_empty() {
            return Err(format!("refused by its judge: {}", refused.join("; ")));
        }
        let opts = run_opts_in(seed, terrain.map(|t| t.site));
        let spec = match terrain {
            Some(t) => t.site.spec(&spec),
            None => spec,
        };
        let run = Run::start(&spec, &opts)?;
        let mut chests = vec![spec.output];
        for (p, b) in &spec.blocks {
            if matches!(b, Block::Chest { .. }) && !chests.contains(p) {
                chests.push(*p);
            }
        }
        let hand = hand_hide_of(d).then(HandHide::default);
        Ok(MobRun { run, chests, golems: 0, iron_dropped: 0, poppies_dropped: 0, arrived: 0, walk: HashMap::new(), spots: Vec::new(), spots_key: Vec::new(), spawned: HashMap::new(), last_spawns: Vec::new(), spec, own: Default::default(), hand, hand_events: Vec::new(), drive: None, nether_seed_iron: 0, log: DriveLog::default() })
    }

    /// Start a part's bench run of `d` (`drive`: the bench's `--drive-out`): the bench's options
    /// (`Drive::opts`), no judge's static checks (the bench brings its own batches, memories and
    /// controls), the settle left to run; then [`MobRun::arrive`] spawns the entity lines in file
    /// order and checks the start time against the bench's.
    pub fn start_driven(d: &Design, drive: std::rc::Rc<redsim::entity::mob::drive::Drive>) -> Result<MobRun, String> {
        let spec = spec(d)?;
        let run = Run::start(&spec, &drive.opts())?;
        let mut chests = vec![spec.output];
        for (p, b) in &spec.blocks {
            if matches!(b, Block::Chest { .. }) && !chests.contains(p) {
                chests.push(*p);
            }
        }
        Ok(MobRun { run, chests, golems: 0, iron_dropped: 0, poppies_dropped: 0, arrived: 0, walk: HashMap::new(), spots: Vec::new(), spots_key: Vec::new(), spawned: HashMap::new(), last_spawns: Vec::new(), spec, own: Default::default(), hand: None, hand_events: Vec::new(), drive: Some(Driven { drive, lines: Vec::new(), t0: 0, cursor: Default::default() }), nether_seed_iron: 0, log: DriveLog::default() })
    }

    /// One tick of the water's settle; false once none is left.
    pub fn settle_tick(&mut self) -> bool {
        self.run.settle_tick()
    }

    /// The id of the design's first entity once it arrived (its entity lines, each followed by
    /// its passengers): a drive's first line, else `Run::own`'s first (what the world held before,
    /// a terrain sample's village and natural mobs, took the ids before it).
    pub fn first_design_id(&self) -> Option<u32> {
        match &self.drive {
            Some(dv) => dv.lines.first().copied(),
            None => self.run.own.first().copied(),
        }
    }

    /// The mobs arrive (the design's entities, their beds, the steady state's last sleep); a
    /// driven run's arrive as its bench's: the entity lines spawned in file order, the start
    /// time checked against the bench's `t0`.
    pub fn arrive(&mut self) -> Result<(), String> {
        if let Some(dv) = self.drive.as_mut() {
            dv.lines = redsim::entity::mob::drive::spawn_lines(&mut self.run, &self.spec)?;
            dv.t0 = self.run.world.time;
            if let Some(t0) = dv.drive.t0.filter(|t0| *t0 != dv.t0) {
                return Err(format!("its bench's drive starts at time {t0}, the view's run at {}", dv.t0));
            }
        } else {
            self.run.arrive()?;
        }
        self.arrived = self.run.world.time;
        self.nether_seed_iron = self.run.world.nether.as_deref().map(nether_iron).unwrap_or(0);
        self.own = self.run.own.iter().copied().collect();
        let w = std::mem::replace(&mut self.run.world, World::new(BlockPos::new(0, 0, 0), [1, 1, 1]));
        self.refresh_spots(&w, true);
        self.run.world = w;
        Ok(())
    }

    /// Take the run's world out (the session holds it between ticks); a 1-cell world stays.
    pub fn take_world(&mut self) -> World {
        let min = self.run.world.min;
        std::mem::replace(&mut self.run.world, World::new(min, [1, 1, 1]))
    }

    /// One judged tick of the farm on `w` (the session's world: swapped into the runner for the
    /// tick), then the record taken: the counts, each mob's walk animation, the spawn spots.
    pub fn tick(&mut self, w: &mut World) {
        std::mem::swap(w, &mut self.run.world);
        // (the card's hand step due before the tick, as the T run does it)
        if let Some(h) = self.hand.as_mut()
            && let Some(step) = h.before_tick(&mut self.run.world, &self.spec, &self.own)
        {
            self.hand_events.push((self.run.world.time, step));
        }
        // (a bench's acts due before the tick, in its order)
        if let Some(dv) = self.drive.as_mut() {
            let (lines, t0) = (&dv.lines, dv.t0);
            dv.cursor.apply_due(&mut self.run.world, lines, &dv.drive, t0);
        }
        let crossed = self.run.world.crossings.len();
        self.run.tick();
        std::mem::swap(w, &mut self.run.world);
        if let Some(dv) = self.drive.as_ref() {
            let t = w.time - dv.t0;
            for (_, from, copy) in &w.crossings[crossed.min(w.crossings.len())..] {
                self.log.crossings.push((t, *from, *copy));
                self.log.in_nether.push(*copy);
            }
            // (a golem's Nether death as the benches count it: the first tick its copy is no
            // longer in the Nether's world, its death animation over)
            if let Some(n) = w.nether.as_deref() {
                let gone: Vec<u32> = self.log.in_nether.iter().copied().filter(|id| !n.entities.list.iter().any(|e| e.id == *id && !e.removed)).collect();
                for id in gone {
                    self.log.nether_deaths.push((t, id));
                    self.log.in_nether.retain(|x| *x != id);
                }
            }
        }
        self.absorb(w);
        // (a block written this tick: the session's write log, taken after the tick)
        let changed = !w.writes.is_empty();
        self.refresh_spots(w, changed);
    }

    /// The spawn spots found again when a villager moved to another cell or `changed` (a block
    /// changed): the judge's own search around each villager ([`spawn_spots`]).
    pub fn refresh_spots(&mut self, w: &World, changed: bool) {
        let key: Vec<BlockPos> = villager_cells(w);
        if !changed && key == self.spots_key && !self.spots.is_empty() {
            return;
        }
        self.spots = spawn_spots(w);
        self.spots_key = key;
    }

    /// The mob layer's record of the tick (it keeps growing unless taken: the judge's runs take it
    /// every tick too): golems summoned, drops; and the client's walk animations.
    fn absorb(&mut self, w: &mut World) {
        self.last_spawns.clear();
        let t = self.drive.as_ref().map(|dv| w.time - dv.t0);
        // (the Nether's events too: its level ticks second; a golem dying there is the kill)
        if let Some(n) = w.nether.as_deref_mut() {
            for ev in std::mem::take(&mut n.mob.events) {
                match ev {
                    MobEvent::Drop { what, count, .. } => match what {
                        "minecraft:iron_ingot" => self.iron_dropped += count,
                        "minecraft:poppy" => self.poppies_dropped += count,
                        _ => {}
                    },
                    _ => {}
                }
            }
            n.mob.spawn_tries.clear();
        }
        for ev in std::mem::take(&mut w.mob.events) {
            match ev {
                MobEvent::GolemTry { golem: Some(g), at, .. } => {
                    self.golems += 1;
                    if let (Some(t), Some(a)) = (t, at) {
                        self.log.golems.push((t, g, BlockPos::new(a[0], a[1], a[2])));
                    }
                    if let Some(a) = at {
                        let c = BlockPos::new(a[0], a[1], a[2]);
                        *self.spawned.entry(c).or_default() += 1;
                        self.last_spawns.push(c);
                    }
                }
                MobEvent::Drop { what, count, .. } => match what {
                    "minecraft:iron_ingot" => self.iron_dropped += count,
                    "minecraft:poppy" => self.poppies_dropped += count,
                    _ => {}
                },
                _ => {}
            }
        }
        w.mob.spawn_tries.clear();
        self.follow(w);
    }

    /// The client's walk animation of each living mob of `w` brought up to its last move (a
    /// tick's absorb; a page's Nether scene for the world it shows, `Session::show_world`).
    pub fn follow(&mut self, w: &World) {
        let mut seen: Vec<u32> = Vec::new();
        for e in w.entities.list.iter().filter(|e| !e.removed) {
            let EntityKind::Mob(m) = &e.kind else { continue };
            seen.push(e.id);
            let a = self.walk.entry(e.id).or_default();
            if e.vehicle.is_none() && !m.dead && m.health > 0.0 {
                let (dx, dz) = (e.pos.x - e.xo, e.pos.z - e.zo);
                a.update((dx * dx + dz * dz).sqrt() as f32);
            } else {
                a.stop();
            }
            if m.ty.def().id == "bat" && a.bat.is_none_or(|(r, _)| r != m.bat_resting) {
                a.bat = Some((m.bat_resting, e.tick_count as i32));
            }
            if matches!(m.ty.def().id, "chicken" | "baby_chicken") {
                // Chicken.aiStep: the wings beat while it is off the ground
                let [_, flap, _, speed, flapping] = a.chicken.unwrap_or([0.0, 0.0, 0.0, 0.0, 1.0]);
                let new_speed = (speed + (if e.on_ground { -1.0 } else { 4.0 }) * 0.3).clamp(0.0, 1.0);
                let flapping = (if !e.on_ground && flapping < 1.0 { 1.0 } else { flapping }) * 0.9;
                a.chicken = Some([flap, flap + flapping * 2.0, speed, new_speed, flapping]);
            }
        }
        self.walk.retain(|id, _| seen.contains(id));
    }

    /// Iron ingots in the farm's chests now; a farm with a Nether part also the iron its Nether
    /// containers gained (ingots, an iron block as 9, beyond the design's own stock: the judge's
    /// and the benches' count): where its golems die.
    pub fn iron_in_chests(&self, w: &World) -> u32 {
        let iron = redsim::redstone::container::item_id("iron_ingot");
        let over: u32 = self
            .chests
            .iter()
            .filter_map(|p| w.containers.map.get(p))
            .flat_map(|c| c.slots.iter().flatten())
            .filter(|s| Some(s.item) == iron)
            .map(|s| s.count as u32)
            .sum();
        over + w.nether.as_deref().map(|n| nether_iron(n).saturating_sub(self.nether_seed_iron)).unwrap_or(0)
    }

    /// How the client draws a mob (its `LivingEntityRenderState`) at `partial`: [`draw_mob`] with
    /// this run's walk animations.
    pub fn draw(&self, w: &World, v: &redsim::entity::mob::view::MobView, partial: f32) -> crate::mesh::MobDraw {
        draw_mob(w, v, partial, self.walk.get(&v.id).copied().unwrap_or_default())
    }
}

/// The villagers' cells, in entity order: the golem search starts at each.
fn villager_cells(w: &World) -> Vec<BlockPos> {
    w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager)).map(|e| e.block_position()).collect()
}

/// The cells a golem can spawn on by the game's rule, read afresh from `w` (the converse of
/// [`spawn_check_seeded`]; vjeux 2026-11-08, "can golems really spawn in the air?"): for every
/// villager (any of them summons when it panics, whatever scared it: the day-1 lane found D1U's
/// blind villager, which never sees the zombie, panicking at a natural hostile), each of the
/// 17 x 17 columns around its cell
/// scanned from 5 above it down to 7 below (`moveToPossibleSpawnPosition`): the first cell whose
/// block below is solid or powder snow and none of `LEGACY_IRON_GOLEM`'s excluded blocks (glass,
/// panes, leaves, ice, cobweb, cactus, TNT, glowstone, beacon, conduit, sea lantern) with air or a
/// fluid in it (`blocks::legacy_golem_can_spawn_on`), its golem's box clear of blocks
/// (`checkSpawnObstruction`; an entity in it only for now). The view draws exactly these.
pub fn spots_by_rule(w: &World) -> std::collections::HashSet<BlockPos> {
    use redsim::entity::mob::{blocks, spawn};
    let mut out = std::collections::HashSet::new();
    for e in w.entities.list.iter() {
        if e.removed || !matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager) {
            continue;
        }
        let start = e.block_position();
        for dz in -8..=8 {
            for dx in -8..=8 {
                // from the cell 6 above down: each cell below it and the one above it
                let mut above = start.offset(dx, 6, dz);
                for _ in 0..13 {
                    let here = above.below();
                    if blocks::legacy_golem_can_spawn_on(&w.get(here), &w.get(above)) {
                        let pos = redsim::entity::Vec3::new(above.x as f64 + 0.5, above.y as f64, above.z as f64 + 0.5);
                        if spot_open(&without_golems(w, above, spawn::obstruction(w, above, pos))) {
                            out.insert(above);
                        }
                        break;
                    }
                    above = here;
                }
            }
        }
    }
    out
}

/// The spots the view draws against [`spots_by_rule`] over the judge's run `seed` for `ticks`
/// ticks, every `every` ticks: how many times compared, the most cells drawn at once, and each
/// difference (its tick, cell, and whether it is drawn but not by the rule or the converse).
pub fn spots_rule_check_seeded(text: &str, seed: i64, ticks: u32, every: u32) -> Result<(u32, usize, Vec<String>, Option<String>), String> {
    let mut s = crate::session::Session::new(text, &crate::session::Options { mob_seed: Some(seed), ..crate::session::Options::live() })?;
    if let Some(why) = &s.unsimulated {
        return Err(format!("not simulated: {why}"));
    }
    let (mut checks, mut most, mut bad) = (0u32, 0usize, Vec::new());
    for t in 0..=ticks {
        if t % every.max(1) == 0 {
            let m = s.mob.as_ref().ok_or("not a mob farm")?;
            let drawn: std::collections::HashSet<BlockPos> = m.spots.iter().filter(|(_, r)| spot_open(r)).map(|(c, _)| *c).collect();
            let rule = spots_by_rule(&s.world);
            most = most.max(drawn.len());
            checks += 1;
            for c in drawn.difference(&rule) {
                bad.push(format!("tick {t}: {} {} {} drawn but no golem can spawn there by the rule", c.x, c.y, c.z));
            }
            for c in rule.difference(&drawn) {
                bad.push(format!("tick {t}: {} {} {} a spawn spot by the rule but not drawn", c.x, c.y, c.z));
            }
        }
        if t < ticks {
            s.step();
            // (the run stopped where redsim does: the looks before it stand, the stop is said)
            if let Some(h) = &s.halted {
                return Ok((checks, most, bad, Some(format!("stopped: {h}"))));
            }
        }
    }
    Ok((checks, most, bad, None))
}

/// Every cell the golem search reaches around every villager of `w` now (any villager summons
/// when it panics: [`spots_by_rule`]), each once (in villager, then
/// column order: `trySpawnMob(IRON_GOLEM, .., 10, 8, 6, LEGACY_IRON_GOLEM)`'s 17 x 17
/// columns from the villager's cell, each scanned from y+5 down to y-7 to the cell it stops at),
/// with the judge's own verdict on it (`IronGolem.checkSpawnObstruction`, redsim's
/// `spawn::golem_spawn_cells`): `Ok` or why a golem cannot spawn there. A column the scan finds no
/// cell in is left out; a cell two villagers reach keeps a passing verdict.
pub fn spawn_spots(w: &World) -> Vec<(BlockPos, Result<(), &'static str>)> {
    let mut out: Vec<(BlockPos, Result<(), &'static str>)> = Vec::new();
    let mut at: HashMap<BlockPos, usize> = HashMap::new();
    for (i, e) in w.entities.list.iter().enumerate() {
        if e.removed || !matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager) {
            continue;
        }
        for (c, r) in redsim::entity::mob::spawn::golem_spawn_cells(w, i) {
            if !spot_reached(&r) {
                continue;
            }
            let r = without_golems(w, c, r);
            match at.get(&c) {
                Some(&k) => {
                    if spot_open(&r) && !spot_open(&out[k].1) {
                        out[k].1 = r;
                    }
                }
                None => {
                    at.insert(c, out.len());
                    out.push((c, r));
                }
            }
        }
    }
    out
}

/// The reason the judge gives a column its scan finds no cell in (`golem_spawn_cells`).
const NO_CELL: &str = "no cell in the column";
/// The reason it gives a cell a mob (a golem, a villager, the zombie) or a boat stands in.
const ENTITY_IN_BOX: &str = "an entity is in the golem's box";

/// The search reaches a cell (its column's scan stopped there).
pub fn spot_reached(r: &Result<(), &'static str>) -> bool {
    !matches!(r, Err(why) if why.starts_with(NO_CELL))
}

/// A golem can spawn on the cell: the judge's checks pass (`IronGolem.checkSpawnObstruction`),
/// with no leniency ([`without_golems`]: only golems are taken out of the way).
pub fn spot_open(r: &Result<(), &'static str>) -> bool {
    r.is_ok()
}

/// The judge's verdict `r` on a golem spawning at `cell` with the golems themselves out of the way
/// (vjeux 2026-11-08: "The UI shows a spawning spot next to the zombie but no iron golem spawns
/// there"): a golem standing on the pad comes and goes, so the area stays put as it does; the
/// farm's own entities (its villagers, its zombie, the zombie's boat) are always in place, and a
/// cell one of them reaches into is no spawn spot (iron-day1's boat, 1.375 wide, in the 1.4-wide
/// box of the golem beside the zombie: the game never spawns one there).
pub fn without_golems(w: &World, cell: BlockPos, r: Result<(), &'static str>) -> Result<(), &'static str> {
    match r {
        Err(why) if why.starts_with(ENTITY_IN_BOX) => {
            let d = MobType::IronGolem.def();
            let pos = redsim::entity::Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5);
            let bb = redsim::entity::make_bounding_box(pos, d.width, d.height);
            // (`spawn::is_unobstructed`'s blocking entities, but golems)
            let others = w.get_entities(bb, |e| match &e.kind {
                EntityKind::Mob(m) => m.ty != MobType::IronGolem,
                EntityKind::Boat(_) | EntityKind::Minecart(_) | EntityKind::ArmorStand(_) | EntityKind::PrimedTnt(_) => true,
                _ => false,
            });
            let falling = w.gravity.entities.iter().any(|f| !f.removed && f.bb().intersects(bb));
            if others.is_empty() && !falling { Ok(()) } else { Err(why) }
        }
        r => r,
    }
}

/// How the client draws a mob (its `LivingEntityRenderState`) at `partial`: from the mob layer's
/// view of it ([`redsim::entity::mob::view::MobView`]: pose, yaw, death, hurt, fire), its core
/// entity (position, pitch, age), its state's health (a golem's crackiness) and box (the flames'
/// size), and the client's walk animation `a` (a still before it plays: none, the mobs as placed).
pub fn draw_mob(w: &World, v: &redsim::entity::mob::view::MobView, partial: f32, a: WalkAnim) -> crate::mesh::MobDraw {
    use redsim::entity::mob::view::MobPose;
    let e = &w.entities.list[v.index];
    let EntityKind::Mob(m) = &e.kind else { return crate::mesh::MobDraw::default() };
    let def = v.ty.def();
    let lerp_deg = |a: f32, b: f32| a + wrap_degrees(b - a) * partial;
    let mut body = lerp_deg(v.body_yaw_o, v.body_yaw);
    let mut head = wrap_degrees(lerp_deg(v.head_yaw_o, v.head_yaw) - body);
    let riding = matches!(v.pose, MobPose::Riding { .. });
    // a mob in a boat: `AbstractBoat.positionRider` keeps its body at the boat's yaw and its head
    // within 105 degrees of it (`clampRotation`); the mob layer keeps one yaw for the mob
    if let MobPose::Riding { vehicle } = v.pose
        && let Some(b) = w.entities.list.iter().find(|x| x.id == vehicle && !x.removed)
        && matches!(b.kind, EntityKind::Boat(_))
    {
        let boat = lerp_deg(b.y_rot_o, b.y_rot);
        head = wrap_degrees(body + head - boat).clamp(-105.0, 105.0);
        body = boat;
    }
    let sleeping = match v.pose {
        MobPose::Sleeping { facing, .. } => Some(facing),
        _ => None,
    };
    let alive = !v.dead && m.health > 0.0;
    // a wandering trader's or a trader llama's own state (redsim's `TraderState`)
    let trader = m.ai.as_ref().and_then(|a| a.trader.as_ref());
    crate::mesh::MobDraw {
        body_yaw: body,
        head_yaw: head,
        head_pitch: e.x_rot_o + (e.x_rot - e.x_rot_o) * partial,
        walk_pos: if alive && !riding { a.position_at(partial) } else { 0.0 },
        walk_speed: if alive && !riding { a.speed_at(partial) } else { 0.0 },
        death_time: if v.death_time > 0 { v.death_time as f32 + partial } else { 0.0 },
        red: v.hurt_time > 0 || v.death_time > 0,
        fire: v.on_fire && !def.fire_immune,
        sleeping,
        eye_standing: def.eye,
        riding,
        crack: if v.ty == MobType::IronGolem { crackiness(m.health / def.max_health) } else { 0 },
        age: e.tick_count as f32 + partial,
        width: m.dims().0,
        height: m.dims().1,
        held: m.mainhand.as_ref().map(|h| h.item.clone()),
        armor: std::array::from_fn(|s| (s < m.armor.pieces as usize && (s != 0 || m.helmet.is_some())).then_some(m.armor.material)),
        left_handed: m.left_handed,
        // a trader llama's `Llama.Variant` (CREAMY, WHITE, BROWN, GRAY: its spawn roll's order)
        variant: m.variant.or_else(|| trader.filter(|t| t.llama).map(|t| ["creamy", "white", "brown", "gray"][t.variant.clamp(0, 3) as usize])),
        invisible: trader.is_some_and(|t| t.invisible),
        aggressive: m.ai.as_ref().is_some_and(|a| a.aggressive),
        color: m.color,
        resting: m.bat_resting,
        anim_start: a.bat.map_or(0, |(_, since)| since),
        entity_id: e.id,
        flap: a.chicken.map_or([0.0; 2], |[of, f, os, s, _]| [of + (f - of) * partial, os + (s - os) * partial]),
        eat: sheep_eat(m, partial),
        carried: m.ai.as_ref().and_then(|a| a.enderman.as_ref()).and_then(|s| s.carried.as_ref()).map(crate::session::drawn_state),
    }
}

/// A sheep eating grass at `partial` (its `EatBlockGoal`'s `eatAnimationTick`, the client's
/// `Sheep.eatAnimationTick` that the goal's start event sets): its head's drop
/// (`getHeadEatPositionScale`) and angle (`getHeadEatAngleScale`); None while it does not eat.
fn sheep_eat(m: &redsim::entity::mob::MobState, partial: f32) -> Option<[f32; 2]> {
    use redsim::entity::mob::ai::goal::{Goal, animal::AnimalGoal};
    let tick = m.ai.as_ref()?.goals.goals.iter().find_map(|g| match &g.goal {
        Goal::Animal(AnimalGoal::EatBlock { tick }) => Some(*tick),
        _ => None,
    })?;
    if tick <= 0 {
        return None;
    }
    let t = tick as f32;
    let drop = if (4..=36).contains(&tick) {
        1.0
    } else if tick < 4 {
        (t - partial) / 4.0
    } else {
        -(t - 40.0 - partial) / 4.0
    };
    let angle = if tick > 4 && tick <= 36 { PI_F / 5.0 + 0.21991149 * ((t - 4.0 - partial) / 32.0 * 28.7).sin() } else { PI_F / 5.0 };
    Some([drop, angle])
}

const PI_F: f32 = std::f32::consts::PI;

/// `Crackiness.GOLEM.byFraction` (0.75, 0.5, 0.25): 0 none, 1 low, 2 medium, 3 high.
pub fn crackiness(fraction: f32) -> u8 {
    if fraction < 0.25 {
        3
    } else if fraction < 0.5 {
        2
    } else if fraction < 0.75 {
        1
    } else {
        0
    }
}

/// `Mth.wrapDegrees` (float).
pub fn wrap_degrees(a: f32) -> f32 {
    let mut r = a % 360.0;
    if r >= 180.0 {
        r -= 360.0;
    }
    if r < -180.0 {
        r += 360.0;
    }
    r
}

/// Everything of a farm's world a tick can change, as one number (the same on every platform,
/// so the wasm's equals the native's): the world's state key (cells, scheduled ticks, block
/// events, containers...), every entity (kind, position, motion, rotation, vehicle) and a mob's
/// whole state (health, fire, sleep, brain...), the game time, the day offset and the mob layer's
/// random.
pub fn world_digest(w: &mut World) -> u64 {
    let mut h = crate::session::H128::default();
    // the world's state key without its entities (redsim's `state_key_into`), then its entities
    // keyed the way it keys them, each path's identity renumbered by its first appearance
    // ([`canonical_path_uids`])
    let list = std::mem::take(&mut w.entities.list);
    w.state_key_into(&mut h);
    w.entities.list = list;
    let mut x = crate::session::entity_hash(h.a ^ h.b.rotate_left(1), w);
    let mut eat = |s: &str| {
        for b in s.bytes() {
            x ^= b as u64;
            x = x.wrapping_mul(0x100000001b3);
        }
    };
    eat(&format!("{} {} {:?}", w.time, w.mob.day_offset, w.mob.rng));
    let mut uids: Vec<u64> = Vec::new();
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        let mut k = e.clone();
        k.id = 0;
        k.tick_count = 0;
        k.section_seq = w.entities.list.iter().filter(|o| !o.removed && o.section == e.section && o.section_seq < e.section_seq).count() as u64;
        eat(&canonical_path_uids(&format!("{k:?}"), &mut uids));
        eat(&format!("{} {} {} {:?} {:?}", e.id, e.y_rot.to_bits(), e.x_rot.to_bits(), e.vehicle, e.passengers));
    }
    x
}

/// An entity's state as text with its paths' `uid`s renumbered by their first appearance in
/// `seen` (shared across a world's entities): a path's uid is its identity (Java's object
/// identity: the brain's PATH memory is the navigation's path or not), numbered process-wide, so
/// two runs side by side in one process number the same paths differently; the relation stays.
fn canonical_path_uids(s: &str, seen: &mut Vec<u64>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("uid: ") {
        out.push_str(&rest[..i + 5]);
        let after = &rest[i + 5..];
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let n: u64 = after[..digits].parse().unwrap_or(0);
        let k = seen.iter().position(|u| *u == n).unwrap_or_else(|| {
            seen.push(n);
            seen.len() - 1
        });
        out.push_str(&k.to_string());
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

/// The judge's run of a design's farm on redsim alone (`farm::Run::new` on the circuit-free
/// spec, [`run_opts`]; in its terrain `site`: the spec moved into it, the run built through it),
/// `ticks` judged ticks: the digest after placement and after every tick ([`world_digest`]), and
/// the golems summoned. The reference the view is held to.
pub fn reference_digests(d: &Design, ticks: u32, site: Option<&'static redsim::entity::mob::site::Site>) -> Result<(Vec<u64>, u32), String> {
    let spec = spec(d)?;
    let opts = run_opts_in(JUDGE_SEED, site);
    let spec = match site {
        Some(s) => s.spec(&spec),
        None => spec,
    };
    let mut r = Run::new(&spec, &opts)?;
    let mut v = vec![world_digest(&mut r.world)];
    let mut golems = 0;
    // (the card's hand step, as the view and the T run perform it)
    let mut hand = hand_hide_of(d).then(HandHide::default);
    let own: std::collections::HashSet<u32> = r.own.iter().copied().collect();
    for _ in 0..ticks {
        if let Some(h) = hand.as_mut() {
            h.before_tick(&mut r.world, &spec, &own);
        }
        r.tick();
        golems += std::mem::take(&mut r.world.mob.events).iter().filter(|e| matches!(e, MobEvent::GolemTry { golem: Some(_), .. })).count() as u32;
        v.push(world_digest(&mut r.world));
    }
    Ok((v, golems))
}

/// The view's run of a design's farm (`Session::new` with the browser's live options, then
/// `Session::step`): the same digests and golems as [`reference_digests`] when the view is exact.
pub fn view_digests(text: &str, ticks: u32) -> Result<(Vec<u64>, u32), String> {
    let mut s = crate::session::Session::new(text, &crate::session::Options::live())?;
    if let Some(why) = &s.unsimulated {
        return Err(format!("not simulated: {why}"));
    }
    let mut v = vec![world_digest(&mut s.world)];
    for _ in 0..ticks {
        s.step();
        if let Some(h) = &s.halted {
            return Err(format!("stopped: {h}"));
        }
        v.push(world_digest(&mut s.world));
    }
    Ok((v, s.mob.as_ref().map(|m| m.golems).unwrap_or(0)))
}

/// The view's run of a design's farm for `ticks` judged ticks: the golems summoned, the iron
/// dropped and the iron in the farm's chests at the end (the readout's numbers).
pub fn view_counts(text: &str, ticks: u32) -> Result<(u32, u32, u32), String> {
    view_counts_seeded(text, JUDGE_SEED, ticks)
}

/// [`view_counts`] of the judge's run with seed `seed` (`mctas web mobrate`: the judge's other
/// runs, played through the view's own session).
pub fn view_counts_seeded(text: &str, seed: i64, ticks: u32) -> Result<(u32, u32, u32), String> {
    let mut s = crate::session::Session::new(text, &crate::session::Options { mob_seed: Some(seed), ..crate::session::Options::live() })?;
    if let Some(why) = &s.unsimulated {
        return Err(format!("not simulated: {why}"));
    }
    for _ in 0..ticks {
        s.step();
        if let Some(h) = &s.halted {
            return Err(format!("stopped: {h}"));
        }
    }
    let m = s.mob.as_ref().ok_or("not a mob farm")?;
    Ok((m.golems, m.iron_dropped, m.iron_in_chests(&s.world)))
}

/// The view's run of the judge's run `seed` for `ticks` judged ticks, each golem's spawn checked
/// against the spawn spots the view shows (vjeux 2026-11-08, `mctas web check`): it must land on a
/// cell shown as a spawn spot in the frame before it or the frame it appears in
/// ([`MobRun::spots`], [`spot_open`]). The golems spawned and the cells shown open (the most at
/// once), and each spawn outside them (its tick, cell and the judge's verdict on the cell).
pub fn spawn_check_seeded(text: &str, seed: i64, ticks: u32) -> Result<(u32, usize, Vec<String>), String> {
    let mut s = crate::session::Session::new(text, &crate::session::Options { mob_seed: Some(seed), ..crate::session::Options::live() })?;
    if let Some(why) = &s.unsimulated {
        return Err(format!("not simulated: {why}"));
    }
    let open = |s: &crate::session::Session| -> std::collections::HashSet<BlockPos> { s.mob.as_ref().map(|m| m.spots.iter().filter(|(_, r)| spot_open(r)).map(|(c, _)| *c).collect()).unwrap_or_default() };
    let (mut golems, mut most, mut outside) = (0u32, 0usize, Vec::new());
    let mut before = open(&s);
    for _ in 0..ticks {
        s.step();
        if let Some(h) = &s.halted {
            return Err(format!("stopped: {h}"));
        }
        let after = open(&s);
        most = most.max(after.len());
        if let Some(m) = s.mob.as_ref() {
            for c in &m.last_spawns {
                golems += 1;
                if !before.contains(c) && !after.contains(c) {
                    let why = m.spots.iter().find(|(p, _)| p == c).map(|(_, r)| format!("{r:?}")).unwrap_or_else(|| "not reached".into());
                    outside.push(format!("tick {}: a golem at {} {} {} outside the spawn spots ({why})", s.world.time, c.x, c.y, c.z));
                }
            }
        }
        before = after;
    }
    Ok((golems, most, outside))
}

/// A run's digests and golems as one number (the shipped wasm's answer: `mcview_mob_digest`).
pub fn chain(digests: &[u64], golems: u32) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for d in digests.iter().copied().chain(std::iter::once(golems as u64)) {
        h = (h ^ d).wrapping_mul(0x100000001b3).rotate_left(23);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archived(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../../designs/circuits/mobfarm/archive/{name}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// Every archived mob farm's name (the archive's `circuit: mob-farm` designs, sorted): the
    /// print tests follow the archive as farms come and go.
    fn archived_farms() -> Vec<String> {
        let dir = format!("{}/../../designs/circuits/mobfarm/archive", env!("CARGO_MANIFEST_DIR"));
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "txt"))
            .filter(|p| std::fs::read_to_string(p).ok().and_then(|t| Design::parse(&t).ok()).is_some_and(|d| is_mob_farm(&d)))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        names
    }

    /// The spawn spots of a farm's run (`MCVIEW_DESIGN`, default iron-day1; `--nocapture`): each
    /// cell the search reaches after the mobs arrive and at noon, its verdict, and where run 0's
    /// golems spawned over a day.
    #[test]
    #[ignore]
    fn print_spawn_spots() {
        let path = std::env::var("MCVIEW_DESIGN").unwrap_or_else(|_| format!("{}/../../designs/circuits/mobfarm/archive/iron-day1.txt", env!("CARGO_MANIFEST_DIR")));
        let text = std::fs::read_to_string(&path).unwrap();
        let d = Design::parse(&text).unwrap();
        let mut run = MobRun::start(&d, JUDGE_SEED, None).unwrap();
        while run.settle_tick() {}
        run.arrive().unwrap();
        let mut w = run.take_world();
        let show = |w: &World, label: &str| {
            let spots = spawn_spots(w);
            let open: Vec<String> = spots.iter().filter(|(_, r)| spot_open(r)).map(|(c, _)| format!("{} {} {}", c.x, c.y, c.z)).collect();
            let mut why: HashMap<&str, u32> = HashMap::new();
            for (_, r) in spots.iter().filter(|(_, r)| !spot_open(r)) {
                *why.entry(r.err().unwrap_or("")).or_default() += 1;
            }
            eprintln!("{label}: villagers at {:?}; {} reached, {} open: {}; refused: {why:?}", villager_cells(w), spots.len(), open.len(), open.join(", "));
        };
        show(&w, "arrived");
        for _ in 0..24000 {
            run.tick(&mut w);
            w.writes.clear();
        }
        show(&w, "a day later");
        let mut spawned: Vec<_> = run.spawned.iter().map(|(c, n)| (format!("{} {} {}", c.x, c.y, c.z), *n)).collect();
        spawned.sort();
        eprintln!("{} golems spawned on: {spawned:?}", run.golems);
    }

    /// vjeux 2026-11-08 ("show the places where golems spawn"): every golem of the judge's run 0's
    /// first day spawns on a cell the view shows as a spawn spot, and the view's verdict on each
    /// cell is the judge's (`checkSpawnObstruction`).
    #[test]
    fn every_golem_spawns_on_a_shown_spawn_spot() {
        let text = archived("iron-day1");
        let (golems, most, outside) = spawn_check_seeded(&text, JUDGE_SEED, 24000).unwrap();
        assert!(golems > 10, "{golems} golems");
        assert!(most > 0);
        assert!(outside.is_empty(), "{outside:?}");
        // the verdicts against the judge's own check, cell by cell, as the run goes: on the world
        // without its golems (the area stays put as they come and go), everything else in place
        let d = Design::parse(&text).unwrap();
        let mut run = MobRun::start(&d, JUDGE_SEED, None).unwrap();
        while run.settle_tick() {}
        run.arrive().unwrap();
        let mut w = run.take_world();
        let mut checked = 0;
        for t in 0..3000 {
            run.tick(&mut w);
            if t % 100 == 0 {
                let mut bare = w.clone();
                for e in bare.entities.list.iter_mut() {
                    if matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::IronGolem) {
                        e.removed = true;
                    }
                }
                for (c, r) in spawn_spots(&w) {
                    let pos = redsim::entity::Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5);
                    assert_eq!(r.is_ok(), redsim::entity::mob::spawn::check_spawn_obstruction(&bare, c, pos), "tick {t}: {c:?} {r:?}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 100, "{checked} verdicts");
        // the cell beside the zombie, its boat in the golem's box: never a spawn spot (vjeux
        // 2026-11-08, "The UI shows a spawning spot next to the zombie but no iron golem spawns
        // there"; the judge's list, `exit::spawn_spots`, has 8 cells)
        let open: Vec<BlockPos> = run.spots.iter().filter(|(_, r)| spot_open(r)).map(|(c, _)| *c).collect();
        let judge = redsim::entity::mob::exit::spawn_spots(&w);
        assert_eq!(open.len(), 8, "{open:?} vs the judge's {judge:?}");
    }

    /// vjeux 2026-11-08 ("can golems really spawn in the air?"): every spot the view draws is one
    /// by the game's rule (a solid floor under the feet, air or a fluid in them, the golem's box
    /// clear), read afresh as the run goes, and every such spot is drawn, from every villager (a
    /// blind one summons too when something else scares it: D1U's third villager's search reaches
    /// the surface above the farm, the day-1 lane's escape).
    #[test]
    fn the_drawn_spawn_spots_are_the_rules() {
        let text = archived("iron-day1");
        let (checks, most, bad, stopped) = spots_rule_check_seeded(&text, JUDGE_SEED, 3000, 100).unwrap();
        assert_eq!(stopped, None);
        assert!(checks >= 30 && most > 0, "{checks} checks, at most {most} cells");
        assert!(bad.is_empty(), "{bad:?}");
        // D1U in its terrain: every look its run reaches (redsim may stop it on a natural mob it
        // does not model yet: that stop is web check's and the view's to report, not this rule's)
        let d1u = std::fs::read_to_string(format!("{}/../../designs/circuits/mobfarm/tracks/ironfarm/d1u-underground.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let (checks, most, bad, stopped) = spots_rule_check_seeded(&d1u, JUDGE_SEED, 600, 100).unwrap();
        if let Some(why) = &stopped {
            assert!(why.contains("not modeled"), "{why}");
            println!("D1U's run: {why} (the looks before it checked)");
        }
        // (its arrival's look at least: a natural mob's model gap can stop it at its first tick)
        let least = if stopped.is_some() { 1 } else { 6 };
        assert!(checks >= least && most > 0, "{checks} checks, at most {most} cells");
        assert!(bad.is_empty(), "{bad:?}");
    }

    /// vjeux 2026-11-08 ("Can you make areas continuous for spawn targets"): the spawn area is one
    /// uniform sheet: each cell of the union of every villager's cells drawn once (its floor, from
    /// above and from under), all in one colour, magenta (the blue read as water: d1-bed-plate's
    /// water sources, translucent blue cubes in its pad's corners, were taken for the area), the
    /// outline only where the union ends.
    #[test]
    fn the_spawn_area_is_one_uniform_sheet() {
        let text = std::fs::read_to_string(format!("{}/../../designs/farms/iron/day-1/d1-bed-plate.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let atlas = crate::atlas::Atlas::build();
        let mut look = crate::scene::viewer_look(&s);
        look.spawn_spots = true;
        let with = crate::scene::build(&atlas, &s, 0.0, &look);
        look.spawn_spots = false;
        let without = crate::scene::build(&atlas, &s, 0.0, &look);
        let sheet: Vec<&crate::mesh::Quad> = with.translucent.iter().filter(|q| q.v[0].layer & crate::mesh::XRAY != 0).collect();
        assert_eq!(with.translucent.len() - without.translucent.len(), sheet.len(), "the area adds only its sheet");
        let open: std::collections::HashSet<[i32; 3]> = s.mob.as_ref().unwrap().spots.iter().filter(|(_, r)| spot_open(r)).map(|(c, _)| c.arr()).collect();
        assert!(open.len() >= 16, "{} cells", open.len());
        // the floor quads: one colour, two per cell (up and down), at the cell's floor
        let floors: Vec<&&crate::mesh::Quad> = sheet.iter().filter(|q| q.v[0].color[3] < 200).collect();
        let colour = floors[0].v[0].color;
        // (magenta: no farm block's colour, not water's blue)
        assert_eq!(colour, crate::mesh::SPAWN_SHEET);
        assert!(colour[0] > 200 && colour[1] < 100 && colour[2] > 150, "{colour:?}");
        assert!(floors.iter().all(|q| q.v.iter().all(|v| v.color == colour)), "one colour");
        let mut per_cell: HashMap<[i32; 3], u32> = HashMap::new();
        for q in &floors {
            let c = [q.center[0].floor() as i32, q.center[1].floor() as i32, q.center[2].floor() as i32];
            *per_cell.entry(c).or_default() += 1;
        }
        assert_eq!(per_cell.len(), open.len(), "every cell once");
        assert!(per_cell.values().all(|n| *n == 2), "{per_cell:?}");
        // the outline: only on the union's outer boundary
        let edges = sheet.len() - floors.len();
        let boundary: usize = open.iter().map(|c| [[0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]].iter().filter(|d| !open.contains(&[c[0] + d[0], c[1], c[2] + d[2]])).count()).sum();
        assert_eq!(edges, 2 * boundary, "the outline on the boundary only");
    }

    /// vjeux 2026-11-08 ("a visual indication for the day night cycle since it's important for how
    /// the farm works"): a mob farm's run shows the game's day (its sky and light from its day
    /// time), the clock's sun or moon turns with it, and the scrubber's night spans run from dusk
    /// to dawn; a view without the game's clock keeps the pictures' noon.
    #[test]
    fn a_mob_farms_view_shows_the_games_day() {
        let text = archived("iron-day1");
        let mut p = crate::player::Player::new_web(&text, 1.5).unwrap();
        let env = p.daylight().expect("a mob farm's run shows the day");
        let day = p.session.world.day_time();
        assert_eq!(env, crate::daylight::at(day, 0.0, 0.0));
        assert!(env.is_day(), "the judge's run starts by day ({day})");
        // over the five days of the judge's run: five nights, each from dusk to dawn
        let (floor, high) = (p.session.tick, p.session.tick + 5 * 24000);
        let spans = p.night_spans(floor, high);
        assert!(spans.len() >= 5 && spans.len() <= 6, "{spans:?}");
        let off = p.session.world.mob.day_offset;
        for (a, b) in &spans {
            if *a > floor {
                assert_eq!((off + *a as i64).rem_euclid(24000), crate::daylight::DUSK);
            }
            if *b < high {
                assert_eq!((off + *b as i64).rem_euclid(24000), 24000 + crate::daylight::DAWN);
            }
        }
        // in the first night the clock shows the moon
        let night = spans[0].0 + 100;
        while p.session.tick < night {
            p.session.step();
        }
        assert!(!p.daylight().unwrap().is_day());
        // a clock (no game time of its own): the pictures' noon
        let clock = std::fs::read_to_string(format!("{}/../../designs/clocks/archive/census-p129-calibrated_sculk_sensor-dispenser-note_block-observer-sculk_sensor-sticky_piston-6b.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let q = crate::player::Player::new_web(&clock, 1.5).unwrap();
        assert!(q.daylight().is_none() && q.night_spans(0, 50000).is_empty());
    }

    /// vjeux 2026-11-08 ("I don't see any zombie, why are they panicking?"): a boat's passenger is
    /// drawn as itself, not as the design's next entity (d1-bed-plate: a villager line follows the
    /// boat), and every drawn entity is redsim's, of its kind.
    #[test]
    fn a_boats_zombie_is_drawn_as_a_zombie() {
        let text = std::fs::read_to_string(format!("{}/../../designs/circuits/mobfarm/tracks/ironfarm/d1-bed-plate.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let kinds = |s: &crate::session::Session| s.entities(0.0).iter().map(|e| e.kind.clone()).collect::<Vec<_>>();
        let k = kinds(&s);
        assert_eq!(k.iter().filter(|k| *k == "minecraft:zombie").count(), 1, "{k:?}");
        assert_eq!(k.iter().filter(|k| *k == "minecraft:villager").count(), 3, "{k:?}");
        assert_eq!(k.iter().filter(|k| *k == "minecraft:oak_boat").count(), 1, "{k:?}");
        for _ in 0..40 {
            s.drawn_matches_simulation().unwrap();
            s.step();
        }
    }

    /// vjeux 2026-11-08 ("Just do the real work!?"): the view runs the farm as its judge does,
    /// tick for tick (redsim's own runner, the judge's spec and first run), and golems spawn.
    #[test]
    fn a_mob_farm_runs_like_its_judge() {
        let text = archived("iron-day1");
        let d = Design::parse(&text).unwrap();
        let (want, golems) = reference_digests(&d, 6000, None).unwrap();
        let (got, view_golems) = view_digests(&text, 6000).unwrap();
        let first = want.iter().zip(&got).position(|(a, b)| a != b);
        assert_eq!(first, None, "the view leaves the judge's run at tick {first:?}");
        assert_eq!(golems, view_golems);
        assert!(golems >= 2, "{golems} golems in 6000 ticks");
    }

    /// The first tick the view's run leaves the judge's, and what differs there (a diagnostic:
    /// `cargo test -p mcview print_first_divergence -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_first_divergence() {
        let text = archived("iron-day1");
        let d = Design::parse(&text).unwrap();
        let spec = spec(&d).unwrap();
        let opts = run_opts_in(JUDGE_SEED, None);
        let mut r = Run::new(&spec, &opts).unwrap();
        let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let describe = |w: &mut World| -> Vec<String> {
            let mut h = crate::session::H128::default();
            let list = std::mem::take(&mut w.entities.list);
            w.state_key_into(&mut h);
            w.entities.list = list;
            let mut out = vec![format!("time {} offset {} rng {:?} state {:x}/{:x} entities {:x}", w.time, w.mob.day_offset, w.mob.rng, h.a, h.b, crate::session::entity_hash(0, w))];
            let mut uids = Vec::new();
            for e in w.entities.list.iter().filter(|e| !e.removed) {
                out.push(format!("{} {}", e.id, canonical_path_uids(&format!("{e:?}"), &mut uids)));
            }
            out
        };
        for t in 0..200 {
            if world_digest(&mut r.world) != world_digest(&mut s.world) {
                let (a, b) = (describe(&mut r.world), describe(&mut s.world));
                println!("tick {t}: the judge's {} lines, the view's {}", a.len(), b.len());
                for (x, y) in a.iter().zip(&b) {
                    if x != y {
                        println!("judge: {}\nview:  {}", &x[..x.len().min(3000)], &y[..y.len().min(3000)]);
                    }
                }
                return;
            }
            r.tick();
            s.step();
        }
        println!("no divergence in 200 ticks");
    }

    /// The spawn cells a design's still and its live view draw (a diagnostic:
    /// `MCTAS_DESIGN=<path> cargo test -p mcview print_drawn_spawn_cells -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_drawn_spawn_cells() {
        let path = std::env::var("MCTAS_DESIGN").unwrap_or_else(|_| format!("{}/../../designs/circuits/mobfarm/iron-day1.txt", env!("CARGO_MANIFEST_DIR")));
        let text = std::fs::read_to_string(&path).unwrap();
        let still = crate::session::Session::new(&text, &crate::session::Options::default()).unwrap();
        let spots = spawn_spots(&still.world);
        println!("still: {} villagers, {} cells reached, {} open: {:?}", villager_cells(&still.world).len(), spots.len(), spots.iter().filter(|(_, r)| spot_open(r)).count(), spots.iter().map(|(c, r)| format!("{} {} {} {:?}", c.x, c.y, c.z, r)).take(12).collect::<Vec<_>>());
        let live = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let m = live.mob.as_ref().unwrap();
        println!("live: {} cells reached, {} open", m.spots.len(), m.spots.iter().filter(|(_, r)| spot_open(r)).count());
    }

    /// The parent 2026-11-08 ("Does the view perform the card's hand steps (the judge's T run
    /// does)? If not, the view shows a night the player wouldn't have"): iron-day1's card says
    /// "hide the zombie by hand the first night", and the view does it as the T run does: at day
    /// time 12000 of its first night the trapdoors flip so no villager sees the zombie, and once the
    /// three villagers sleep they flip back; the view stays the judge's run with that step, tick
    /// for tick.
    #[test]
    fn the_view_hides_the_zombie_the_first_night_like_the_t_run() {
        let text = archived("iron-day1");
        let d = Design::parse(&text).unwrap();
        assert!(hand_hide_of(&d));
        let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let mut hid = None;
        let mut restored = None;
        for _ in 0..30000 {
            s.step();
            let m = s.mob.as_ref().unwrap();
            if hid.is_none() && let Some((t, HandStep::Hid)) = m.hand_events.first() {
                hid = Some(*t);
                assert_eq!((s.world.mob.day_offset + t).rem_euclid(24000), 12000, "the hide at the first day time 12000");
                assert!(status_words(&s).starts_with("you hid the zombie"), "{}", status_words(&s));
            }
            if let Some((t, HandStep::Restored)) = m.hand_events.get(1) {
                restored = Some(*t);
                break;
            }
        }
        let (hid, restored) = (hid.expect("the zombie hidden"), restored.expect("the trapdoors back"));
        assert!(restored - hid >= 20 && restored - hid < 600, "{hid} -> {restored}");
        // every villager asleep when the trapdoors flip back
        assert_eq!(status_words(&s), "villagers: 3 asleep");
        // and the view is the judge's run with the same step, tick for tick, through it
        let n = (restored + 100 - s.mob.as_ref().unwrap().arrived) as u32;
        let (want, _) = reference_digests(&d, n, None).unwrap();
        let (got, _) = view_digests(&text, n).unwrap();
        assert_eq!(want.iter().zip(&got).position(|(a, b)| a != b), None);
    }

    /// What the villagers do through the first two nights of a farm's view (a diagnostic:
    /// `MCTAS_DESIGN=<path> cargo test -p mcview print_nights -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn print_nights() {
        let path = std::env::var("MCTAS_DESIGN").unwrap_or_else(|_| format!("{}/../../designs/circuits/mobfarm/archive/iron-day1.txt", env!("CARGO_MANIFEST_DIR")));
        let text = std::fs::read_to_string(&path).unwrap();
        let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        // (MCTAS_NO_HAND=1: without the card's hand step, as the view ran before)
        if std::env::var("MCTAS_NO_HAND").is_ok() {
            s.mob.as_mut().unwrap().hand = None;
        }
        // each night's first sleep and its panics, and the golems at each dawn
        let (mut first_sleep, mut panics, mut was_panic): (Option<String>, u32, bool) = (None, 0, false);
        for _ in 0..60000 {
            s.step();
            let day = s.world.day_time();
            let w = status_words(&s);
            let d = day.rem_euclid(24000);
            if (12000..23000).contains(&d) {
                if first_sleep.is_none() && w.contains("asleep") {
                    first_sleep = Some(clock_words(day));
                }
                let p = w.contains("panicking");
                if p && !was_panic {
                    panics += 1;
                }
                was_panic = p;
            }
            if d == 23000 {
                println!("night ending {}: first asleep at {}, {panics} panics; {}", clock_words(day), first_sleep.take().unwrap_or_else(|| "never".into()), readout(&s));
                panics = 0;
            }
        }
    }

    /// A terrain farm's slowest ticks natively with the mob AI on (a diagnostic for the browser's
    /// frame budget: `MCTAS_DESIGN=<path> cargo test -p mcview print_slow_ticks -- --ignored
    /// --nocapture`): each tick over 15 ms, its path searches and their time, the mobs alive.
    #[test]
    #[ignore]
    fn print_slow_ticks() {
        use redsim::entity::mob::ai::path::navigation::{SEARCHES, SEARCH_NANOS};
        use std::sync::atomic::Ordering::Relaxed;
        let path = std::env::var("MCTAS_DESIGN").unwrap_or_else(|_| format!("{}/../../designs/circuits/mobfarm/tracks/ironfarm/d1u-underground.txt", env!("CARGO_MANIFEST_DIR")));
        let text = std::fs::read_to_string(&path).unwrap();
        let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let (mut total, mut slow) = (0.0f64, 0u32);
        let n = 8000;
        for _ in 0..n {
            let (s0, n0) = (SEARCHES.load(Relaxed), SEARCH_NANOS.load(Relaxed));
            let t0 = std::time::Instant::now();
            s.step();
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            total += ms;
            if ms > 15.0 {
                slow += 1;
                let mut kinds: HashMap<String, u32> = HashMap::new();
                for e in s.world.entities.list.iter().filter(|e| !e.removed) {
                    if let EntityKind::Mob(m) = &e.kind {
                        *kinds.entry(format!("{:?}", m.ty)).or_default() += 1;
                    }
                }
                let mut k: Vec<_> = kinds.into_iter().collect();
                k.sort();
                println!("tick {}: {ms:.1} ms, {} path searches ({:.1} ms), mobs {k:?}", s.tick, SEARCHES.load(Relaxed) - s0, (SEARCH_NANOS.load(Relaxed) - n0) as f64 / 1e6);
            }
            if s.halted.is_some() {
                println!("stopped: {:?}", s.halted);
                break;
            }
        }
        println!("{n} ticks: {:.2} ms a tick on average, {slow} over 15 ms", total / n as f64);
    }

    /// The staged camera search finds the very camera of the search at once on every archived
    /// circuit and door, desktop and phone (`--ignored`: a few hundred designs).
    #[test]
    #[ignore]
    fn every_archived_design_keeps_its_live_camera() {
        let atlas = crate::atlas::Atlas::build();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs");
        let mut files = Vec::new();
        for dir in ["circuits", "piston-door-3x3"] {
            let mut stack = vec![root.join(dir)];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        if p.file_name().is_some_and(|n| n != "tracks") {
                            stack.push(p);
                        }
                    } else if p.extension().is_some_and(|x| x == "txt") && p.parent().is_some_and(|q| q.ends_with("archive")) {
                        files.push(p);
                    }
                }
            }
        }
        files.sort();
        let (mut n, mut bad) = (0, Vec::new());
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            let Ok(s) = crate::session::Session::new(&text, &crate::session::Options::still()) else { continue };
            let look = crate::scene::viewer_look(&s);
            for (w, h) in [(1232.0, 480.0), (399.0, 480.0)] {
                let once = crate::scene::live_camera_at_once(&atlas, &s, &look, w, h);
                let mut search = crate::scene::LiveCameraSearch::new(&atlas, &s, &look, w, h);
                let staged = loop {
                    if let Some(r) = search.step(&atlas, &s, &look, &mut || true) {
                        break r;
                    }
                };
                n += 1;
                if format!("{once:?}") != format!("{staged:?}") {
                    bad.push(format!("{} at {w}x{h}", f.display()));
                }
            }
        }
        eprintln!("{n} cameras, {} differ", bad.len());
        assert!(bad.is_empty(), "{bad:?}");
    }

    /// A big design's first frame is built over several frames ([`crate::scene::Staged`]) and gives
    /// the very mesh of one build.
    #[test]
    fn a_staged_live_camera_is_the_one_found_at_once() {
        // the 2x2 farm's camera search took 80-90 ms in one task in a software GL; staged, it
        // weighs one candidate a call and must find the same camera, on a desktop and a phone
        let atlas = crate::atlas::Atlas::build();
        for name in ["iron-day1"] {
            let text = archived(name);
            let s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
            let look = crate::scene::viewer_look(&s);
            for (w, h) in [(1232.0, 480.0), (399.0, 480.0)] {
                let once = crate::scene::live_camera_at_once(&atlas, &s, &look, w, h);
                let mut search = crate::scene::LiveCameraSearch::new(&atlas, &s, &look, w, h);
                let mut calls = 0;
                let staged = loop {
                    calls += 1;
                    if let Some(r) = search.step(&atlas, &s, &look, &mut || true) {
                        break r;
                    }
                };
                assert!(calls > 10, "{calls} calls");
                assert_eq!(format!("{once:?}"), format!("{staged:?}"), "{name} at {w}x{h}");
            }
        }
    }

    /// The page's view before it plays (a still: nothing ticks) draws the mobs as placed with the
    /// client's models (it drew the zombie as the stand-in box).
    #[test]
    fn a_staged_still_is_the_one_shot_mesh() {
        // a still built 16 blocks a call (a big farm's first frame no longer one 45-60 ms task)
        // gives the very mesh of one call
        let atlas = crate::atlas::Atlas::build();
        let text = archived("iron-day1");
        let s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
        let look = crate::scene::viewer_look(&s);
        let whole = crate::scene::build(&atlas, &s, 0.0, &look);
        let mut st = crate::scene::Staged::new(&s, 0.0, &look, crate::scene::Part::All);
        let mut calls = 0;
        let parts = loop {
            calls += 1;
            if let Some(m) = st.step(&atlas, &s, 0.0, &look, &mut || true) {
                break m;
            }
        };
        assert!(calls > 10, "{calls} calls");
        assert_eq!(bytemuck::cast_slice::<_, u8>(&whole.solid), bytemuck::cast_slice::<_, u8>(&parts.solid));
        assert_eq!(format!("{:?}", whole.translucent), format!("{:?}", parts.translucent));
        assert_eq!((whole.lo, whole.hi), (parts.lo, parts.hi));
    }

    /// The page's view before it plays (a still: nothing ticks) draws the mobs as placed with the
    /// client's models (it drew the zombie as the stand-in box).
    #[test]
    fn a_mob_farm_still_draws_its_mobs_with_their_models() {
        let text = archived("iron-day1");
        let s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
        let mobs: Vec<_> = s.entities(0.0).into_iter().filter(|e| e.kind.ends_with("villager") || e.kind.ends_with("zombie")).collect();
        assert_eq!(mobs.len(), 4, "{:?}", mobs.iter().map(|e| &e.kind).collect::<Vec<_>>());
        assert!(mobs.iter().all(|e| e.mob.is_some()));
        assert!(mobs.iter().any(|e| e.mob.as_ref().is_some_and(|m| m.riding)), "the zombie rides its boat");
    }

    /// What the readout and the left badge say every 500 ticks of two farms (`--nocapture`).
    #[test]
    #[ignore]
    fn print_villager_states() {
        for name in archived_farms() {
            let text = archived(&name);
            let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
            for k in 0..=48 {
                if k > 0 {
                    s.run(500);
                }
                eprintln!("{name} tick {}: {} · {}", s.tick, readout(&s), status_words(&s));
            }
        }
    }

    /// The still's mesh of the smallest and the biggest farm, natively (`--nocapture`).
    #[test]
    #[ignore]
    fn print_camera_costs() {
        let mut atlas = crate::atlas::Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, w, h) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, w, h, &idx, &defs, &models).unwrap();
        let loops: u32 = std::env::var("MCVIEW_MESH_LOOPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
        for name in archived_farms() {
            let text = archived(&name);
            let s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
            let look = crate::scene::viewer_look(&s);
            let t = std::time::Instant::now();
            for _ in 0..loops {
                std::hint::black_box(crate::scene::live_camera(&atlas, &s, &look, 1232.0, 480.0));
            }
            eprintln!("{name}: the live camera {:.1} ms", t.elapsed().as_secs_f64() * 1000.0 / loops as f64);
        }
    }

    /// The still's mesh of the smallest and the biggest farm, natively (`--nocapture`).
    #[test]
    #[ignore]
    fn print_still_mesh_costs() {
        let mut atlas = crate::atlas::Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, w, h) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, w, h, &idx, &defs, &models).unwrap();
        for name in archived_farms() {
            let text = archived(&name);
            let t = std::time::Instant::now();
            let s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
            let t_new = t.elapsed().as_secs_f64() * 1000.0;
            let look = crate::scene::viewer_look(&s);
            let t = std::time::Instant::now();
            let blocks = s.render_blocks(0.0);
            let t_rb = t.elapsed().as_secs_f64() * 1000.0;
            let t = std::time::Instant::now();
            let m = crate::scene::build(&atlas, &s, 0.0, &look);
            let t_build = t.elapsed().as_secs_f64() * 1000.0;
            // (more builds for a profiler: MCVIEW_MESH_LOOPS=n)
            let loops: u32 = std::env::var("MCVIEW_MESH_LOOPS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            for _ in 0..loops {
                std::hint::black_box(crate::scene::build(&atlas, &s, 0.0, &look));
            }
            eprintln!("{name}: session {t_new:.1} ms, {} blocks drawn ({t_rb:.1} ms), mesh {t_build:.1} ms: {} solid, {} translucent vertices", blocks.len(), m.solid.len(), m.translucent.len());
        }
    }

    /// Each archived farm's view: a tick's and a frame's cost natively (`--nocapture`).
    #[test]
    #[ignore]
    fn print_frame_costs() {
        // the browser's atlas: the game's textures, block models and entity textures
        let mut atlas = crate::atlas::Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, w, h) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, w, h, &idx, &defs, &models).unwrap();
        let ((ep, ei), (ip, ii)) = crate::tools::vanilla_entity_assets().unwrap();
        let (er, ew, eh) = crate::tools::decode_png(&ep).unwrap();
        let (ir, iw, ih) = crate::tools::decode_png(&ip).unwrap();
        atlas.add_game_entities((&er, ew, eh, &ei), (&ir, iw, ih, &ii)).unwrap();
        for name in archived_farms() {
            let text = archived(&name);
            let mut p = crate::player::Player::new_web(&text, 1.5).unwrap();
            let t = std::time::Instant::now();
            let _ = p.triangles(&atlas);
            let first = t.elapsed().as_secs_f64() * 1000.0;
            let (mut ticks, mut frames, mut rebuilt) = (0.0, 0.0, 0);
            let mut version = p.session.blocks_version;
            for _ in 0..600 {
                let t = std::time::Instant::now();
                p.session.step();
                ticks += t.elapsed().as_secs_f64() * 1000.0;
                if p.session.blocks_version != version {
                    rebuilt += 1;
                    version = p.session.blocks_version;
                }
                let t = std::time::Instant::now();
                let _ = p.triangles(&atlas);
                frames += t.elapsed().as_secs_f64() * 1000.0;
            }
            eprintln!("{name}: first frame {first:.1} ms; 600 ticks: {:.3} ms a tick, {:.2} ms a frame, the blocks' mesh rebuilt {rebuilt} times", ticks / 600.0, frames / 600.0);
        }
    }

    /// When each archived farm's view summons its golems (a picture's moments: `--nocapture`).
    #[test]
    #[ignore]
    fn print_golem_ticks() {
        for name in archived_farms() {
            let mut s = crate::session::Session::new(&archived(&name), &crate::session::Options::live()).unwrap();
            let (mut seen, mut ticks) = (0, Vec::new());
            for _ in 0..3000 {
                s.step();
                let g = s.mob.as_ref().unwrap().golems;
                if g > seen {
                    seen = g;
                    ticks.push(s.tick - s.rest_tick);
                }
            }
            eprintln!("{name}: golems at {ticks:?}");
        }
        // where the day-1 farm's first golem goes, every 10 ticks of its life
        let mut s = crate::session::Session::new(&archived("iron-day1"), &crate::session::Options::live()).unwrap();
        for _ in 0..100 {
            s.step();
        }
        for k in 0..30 {
            for _ in 0..10 {
                s.step();
            }
            for e in s.world.entities.list.iter().filter(|e| !e.removed) {
                if let EntityKind::Mob(m) = &e.kind
                    && m.ty == MobType::IronGolem
                {
                    eprintln!("t+{} golem {} at {:.2} {:.2} {:.2} health {:.1} fire {} hurt {} dead {} {}", 100 + 10 * (k + 1), e.id, e.pos.x, e.pos.y, e.pos.z, m.health, m.fire_ticks, m.hurt_time, m.dead, m.death_time);
                }
            }
        }
    }
}

/// The game's day time as the readout says it: the day (from 1) and the clock (day time 0 is
/// 6:00, 1000 a game hour).
pub fn clock_words(day_time: i64) -> String {
    let d = day_time.rem_euclid(24000);
    let day = day_time.div_euclid(24000) + 1;
    let minutes = (d * 60 / 1000 + 6 * 60) % (24 * 60);
    format!("day {day}, {:02}:{:02}", minutes / 60, minutes % 60)
}

/// The view's readout for a mob farm (the time badge, vjeux's brief: the in-game time, the golems
/// so far, the iron in the chest): `day 2, 18:30 · 12 golems, 47 iron` (one part per line on a
/// phone). Before it plays (a still): the judge's start, nothing yet.
pub fn readout(s: &crate::session::Session) -> String {
    let (day_time, golems, iron) = match &s.mob {
        Some(m) => (s.world.day_time(), m.golems, m.iron_in_chests(&s.world)),
        None => (run_opts().day0, 0, 0),
    };
    let plural = |n: u32, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
    format!("{} · {}, {} iron", clock_words(day_time), plural(golems, "golem", "golems"), iron)
}

/// What the villagers do now, from redsim's state (the left badge): `villagers: 3 asleep`,
/// `villagers: 2 panicking, 2 calm`.
pub fn status_words(s: &crate::session::Session) -> String {
    let (mut asleep, mut panic, mut calm) = (0, 0, 0);
    for e in s.world.entities.list.iter().filter(|e| !e.removed) {
        let EntityKind::Mob(m) = &e.kind else { continue };
        if m.ty != MobType::Villager {
            continue;
        }
        if m.sleeping_pos.is_some() {
            asleep += 1;
        } else if m.brain.as_deref().is_some_and(|b| b.activity == redsim::entity::mob::villager::Activity::Panic) {
            panic += 1;
        } else {
            calm += 1;
        }
    }
    // the card's hand step under way (the zombie hidden until the villagers sleep)
    if s.mob.as_ref().is_some_and(|m| m.hand.as_ref().is_some_and(|h| h.hidden.is_some())) {
        return hand_status_text(asleep, panic, calm);
    }
    status_text(asleep, panic, calm)
}

/// What a part's bench drives in its view's run, in a player's words (its drive's `# caption`,
/// vjeux 07:29: "write down these details when you show me things"): the view's caption line.
pub fn caption(s: &crate::session::Session) -> Option<String> {
    s.mob.as_ref()?.drive.as_ref()?.drive.caption.clone()
}

fn status_text(asleep: u32, panic: u32, calm: u32) -> String {
    let parts: Vec<String> = [(asleep, "asleep"), (panic, "panicking"), (calm, "calm")].iter().filter(|(n, _)| *n > 0).map(|(n, w)| format!("{n} {w}")).collect();
    if parts.is_empty() { String::new() } else { format!("villagers: {}", parts.join(", ")) }
}

/// The badge while the card's hand step is under way: `you hid the zombie: 2 asleep, 1 calm`.
fn hand_status_text(asleep: u32, panic: u32, calm: u32) -> String {
    let v = status_text(asleep, panic, calm);
    format!("{}: {}", hand_words(HandStep::Hid).replace("hide", "hid"), v.strip_prefix("villagers: ").unwrap_or(&v))
}

/// The widest texts [`status_words`] can say for a farm of `n` villagers (the badge is sized once
/// from them, never resizing as the night comes).
pub fn status_texts(n: u32) -> Vec<String> {
    let mut v = vec![status_text(n, 0, 0), status_text(0, n, 0), status_text(0, 0, n)];
    if n >= 3 {
        let a = n / 3;
        v.push(status_text(a, a, n - 2 * a));
    }
    if n >= 2 {
        v.push(status_text(n / 2, n - n / 2, 0));
        v.push(status_text(0, n / 2, n - n / 2));
    }
    // (the card's hand step under way: the zombie hidden while they fall asleep)
    v.push(hand_status_text(n, 0, 0));
    if n >= 2 {
        v.push(hand_status_text(n / 2, n - n / 2, 0));
        v.push(hand_status_text(n / 2, 0, n - n / 2));
    }
    v
}

/// The villagers a design brings (its `entity:` lines).
pub fn villagers(d: &Design) -> u32 {
    // (a villager riding a vehicle too: `Passengers:[{id:"minecraft:villager"...}]`, the carousel's
    // carts: the badge sized for 0 cut "villagers: 21 panicking" on the siding's page)
    d.entities
        .iter()
        .map(|e| (e.kind.trim_start_matches("minecraft:") == "villager") as u32 + e.nbt.as_deref().map(|n| n.matches("id:\"minecraft:villager\"").count() as u32).unwrap_or(0))
        .sum()
}

/// The speed a mob farm plays at first (game ticks a second; vjeux's brief: a golem every few
/// seconds): its card's seconds per golem (the archived `# CIRCUIT:` line) over about 4 s, the
/// nearest of `speeds` (log scale, at least real time); 10x when the card says nothing.
pub fn default_tps(d: &Design, speeds: &[f32]) -> f32 {
    let secs = d.comments.iter().find_map(|c| {
        let rest = c.split("seconds per golem ").nth(1)?;
        rest.split([',', ' ']).next()?.parse::<f32>().ok()
    });
    let want = match secs {
        Some(s) if s > 0.0 => (s / 4.0).max(1.0) * 20.0,
        _ => 200.0,
    };
    speeds.iter().copied().filter(|t| *t >= 20.0).min_by(|a, b| (a.ln() - want.ln()).abs().total_cmp(&(b.ln() - want.ln()).abs())).unwrap_or(200.0)
}
