//! `mctas mobfarm timeline`: a mob farm run's tick-level timing view, for the carousel's parts and
//! assembly (the parent and ironspec, 2026-11-09; vjeux 22:30: "The timings are tight so need to
//! be precise"). One run of the judge's runner (`farm::Run`, `mobfarm run`'s options: the same run
//! tick for tick) is sampled after every tick:
//! - each farm villager: its position class (asleep, at its bed, travelling, a tagged region such
//!   as the station, held in a still cart, loose), its activity, its panic trigger
//!   (`VillagerPanicTrigger` running: it summons on the slots), GOLEM_DETECTED_RECENTLY with its
//!   expiry, whether it wants a golem, its cart, HOME, LAST_SLEPT and CANT_REACH_WALK_TARGET_SINCE;
//! - each farm cart: its position, speed and rider;
//! - each bed's OCCUPIED and its sleeper;
//! - the design's tagged blocks (a counter's bulbs, a release line: `timeline: signal NAME x y z`)
//!   and regions (`timeline: region NAME x0 y0 z0 x1 y1 z1`: a villager in one is at NAME);
//! - the gameTime % 100 slots: each summoner, the villagers in its 10-box (redsim's own box test:
//!   the 10.6 rule), who wants a golem, and its attempt's result (redsim's `GolemTry`);
//! - the golems: each spawn (who summoned it, where) and exit (died and removed, or removed
//!   without dying: a portal);
//! - every violation: a villager outside the summoner's group locked by a spawn (10.6); a villager
//!   awake in REST within 16 of its home bed while the bed is OCCUPIED (16: `ValidateNearbyPoi`
//!   drops its HOME); CANT_REACH older than 1200 gt in REST (the nightly HOME release); a waiting
//!   villager's golem sensor seeing a golem (its follow box).
//!
//! Written as a page (`<stem>-seed<S>.timeline.html`: lanes on a canvas, zoom and pan by wheel,
//! drag and pinch, a tap reads every lane at that tick) and a text dump (`.timeline.txt`: one line
//! per change, each starting `t=<tick after the arrival> gt=<game time> day=<n> dt=<day time>`, the
//! violations as `VIOLATION <rule>` lines, for grep).

use anyhow::{Result, anyhow, bail};
use redsim::entity::mob::ai::brain::tree::Kind;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts};
use redsim::entity::mob::types::MobType;
use redsim::entity::mob::MobEvent;
use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::{BlockPos, World};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A tag of the design's (`timeline: ...` lines) or the command line's (`--tag`).
#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    /// `signal NAME x y z`: a block's state as a lane (a counter's bulb, a release line's dust).
    Signal { name: String, at: [i32; 3] },
    /// `region NAME x0 y0 z0 x1 y1 z1`: a box of cells (inclusive); a villager whose feet are in it
    /// is at NAME (the station, a hold, the dormitory). A region named `station...` is the
    /// summoners' group for the 10.6 rule.
    Region { name: String, lo: [i32; 3], hi: [i32; 3] },
}

impl Tag {
    pub fn parse(s: &str) -> Result<Tag> {
        let w: Vec<&str> = s.split_whitespace().collect();
        let num = |k: usize| -> Result<i32> { w[k].parse::<i32>().map_err(|_| anyhow!("timeline tag {s:?}: {} is not a whole number", w[k])) };
        match w.first().copied() {
            Some("signal") if w.len() == 5 => Ok(Tag::Signal { name: w[1].to_string(), at: [num(2)?, num(3)?, num(4)?] }),
            Some("region") if w.len() == 8 => {
                let (a, b) = ([num(2)?, num(3)?, num(4)?], [num(5)?, num(6)?, num(7)?]);
                Ok(Tag::Region { name: w[1].to_string(), lo: std::array::from_fn(|k| a[k].min(b[k])), hi: std::array::from_fn(|k| a[k].max(b[k])) })
            }
            _ => bail!("timeline tag {s:?}: expected `signal NAME x y z` or `region NAME x0 y0 z0 x1 y1 z1`"),
        }
    }
}

/// The design's tags: its `timeline:` lines.
pub fn tags_in(text: &str) -> Result<Vec<Tag>> {
    text.lines().filter_map(|l| l.trim().strip_prefix("timeline:")).map(|t| Tag::parse(t.trim())).collect()
}

/// A lane's values over the run: each change (tick, value).
#[derive(Clone, Debug, Default)]
struct Ch<T> {
    v: Vec<(u32, T)>,
}

impl<T: PartialEq + Clone> Ch<T> {
    /// Record `x` at tick `k`; true when it changed.
    fn set(&mut self, k: u32, x: T) -> bool {
        if self.v.last().is_some_and(|(_, y)| *y == x) {
            return false;
        }
        self.v.push((k, x));
        true
    }

    /// The changes in the window `[from, to]`, the value at `from` first (at `from`).
    fn window(&self, from: u32, to: u32) -> Vec<(u32, T)> {
        let i = self.v.partition_point(|(k, _)| *k <= from);
        let mut out: Vec<(u32, T)> = Vec::new();
        if i > 0 {
            out.push((from, self.v[i - 1].1.clone()));
        }
        out.extend(self.v[i..].iter().take_while(|(k, _)| *k <= to).cloned());
        out
    }
}

/// A farm villager's lane.
struct VLane {
    id: u32,
    label: String,
    first_cart: String,
    class: Ch<String>,
    act: Ch<String>,
    panic: Ch<bool>,
    sleep: Ch<bool>,
    /// GOLEM_DETECTED_RECENTLY's expiry (tick; -1: none).
    lock: Ch<i64>,
    willing: Ch<bool>,
    cart: Ch<String>,
    home: Ch<String>,
    /// LAST_SLEPT (tick; i64::MIN: none).
    slept: Ch<i64>,
    /// CANT_REACH_WALK_TARGET_SINCE (tick; i64::MIN: none).
    cant: Ch<i64>,
    /// Its feet (hundredths), each change of a hundredth or more.
    pos: Vec<(u32, [i32; 3])>,
    alive: bool,
    prev: Option<Vec3>,
    prev_ttl: Option<i64>,
    /// The open spans of the 16 rule and of CANT_REACH over 1200 (their start ticks).
    over16: Option<(u32, f64)>,
    over1200: Option<u32>,
}

/// A farm cart's lane.
struct CLane {
    id: u32,
    label: String,
    rider: Ch<String>,
    pos: Vec<(u32, [i32; 3])>,
    prev: Option<Vec3>,
    moving: Option<(u32, Vec3, f64)>,
}

/// A bed's lane.
struct BLane {
    head: BlockPos,
    label: String,
    occ: Ch<bool>,
    by: Ch<String>,
    /// The open nap (its start, its sleeper, the nearest each other claimant of the bed came).
    nap: Option<(u32, String, HashMap<String, f64>)>,
}

/// A tagged block's lane.
struct SLane {
    name: String,
    /// Its cell (a run's tagged block); none for a dump's lane.
    at: Option<BlockPos>,
    /// Its short value (power, lit, powered...) and its full block state.
    val: Ch<String>,
    full: Ch<String>,
}

/// A lane of point events (a dump's `mark NAME text` lines: a bench's input pulses, its missed
/// slots).
struct MLane {
    name: String,
    ev: Vec<(u32, String)>,
}

/// A golem of the run.
struct Golem {
    id: u32,
    label: String,
    t: u32,
    by: String,
    at: [i32; 3],
    died: Option<(u32, String)>,
    gone: Option<(u32, [f64; 3])>,
    /// The summoning batch: the villagers its spawn locked (in the summoner's 10-box, the
    /// summoner included), by lane index.
    batch: Vec<usize>,
}

/// One summoner on a slot: its attempt's result and its 10-box.
struct Summoner {
    who: String,
    result: String,
    golem: Option<String>,
    willing: u32,
    /// (villager, dx, dy, dz, wants a golem, in its 10-box)
    others: Vec<(String, f64, f64, f64, bool, bool)>,
}

struct Slot {
    k: u32,
    summoners: Vec<Summoner>,
}

struct Flag {
    k: u32,
    until: Option<u32>,
    rule: String,
    lane: String,
    text: String,
}

/// The timeline of one run.
pub struct Timeline {
    pub title: String,
    pub seed: i64,
    /// Game time and day time at tick 0 (the arrival).
    pub t0: i64,
    pub day0: i64,
    pub ticks: u32,
    offset: [i32; 3],
    regions: Vec<(String, [i32; 3], [i32; 3])>,
    villagers: Vec<VLane>,
    carts: Vec<CLane>,
    beds: Vec<BLane>,
    signals: Vec<SLane>,
    golems: Vec<Golem>,
    slots: Vec<Slot>,
    flags: Vec<Flag>,
    /// The text dump's lines (tick, line).
    lines: Vec<(u32, String)>,
    /// The checks that need the rest of the run (a villager's role: arriving at or leaving the
    /// station), resolved by [`Timeline::finish`].
    pending: Vec<Pending>,
    /// The judge's counters, by tick: tries without a spot, golem sightings, sleeps, POI releases.
    counts: Vec<(u32, &'static str)>,
    /// The open follow-box exposures: (golem, villager) → the tick it began.
    exposed: HashMap<(usize, usize), u32>,
    /// Each villager's sleeps since the last dawn, and whether the run saw that whole night.
    night_sleeps: Vec<u32>,
    whole_night: bool,
    /// A dump's point-event lanes.
    marks: Vec<MLane>,
    /// The dump it was read from (`--from-dump`), none for a run.
    source: Option<String>,
}

/// A check resolved once the run is over.
enum Pending {
    /// A villager in summoner `who`'s 10-box at the spawn of golem `g`.
    Spawn { k: u32, who: String, g: String, vi: usize, d: [f64; 3] },
    /// A villager's golem sensor saw golems `seen` (its lock before: `prev`, an expiry tick).
    Follow { k: u32, vi: usize, seen: String, follow: f64, prev: i64 },
}

/// A block state's short value for a lane: its power level, and its true flags (lit, powered,
/// open, triggered, occupied), else "off".
fn signal_value(state: &str) -> String {
    let props = state.split_once('[').map(|(_, p)| p.trim_end_matches(']')).unwrap_or("");
    let mut out: Vec<String> = Vec::new();
    for kv in props.split(',') {
        let Some((k, v)) = kv.split_once('=') else { continue };
        match (k, v) {
            ("power", v) => out.push(format!("p{v}")),
            ("powered" | "lit" | "open" | "triggered" | "occupied" | "enabled" | "attached", "true") if k != "attached" => out.push(k.to_string()),
            _ => {}
        }
    }
    if out.is_empty() { "off".to_string() } else { out.join(" ") }
}

fn fmt_pos(p: Vec3, off: [i32; 3]) -> String {
    format!("{:.2},{:.2},{:.2}", p.x - off[0] as f64, p.y - off[1] as f64, p.z - off[2] as f64)
}

fn cell(b: BlockPos, off: [i32; 3]) -> String {
    format!("{},{},{}", b.x - off[0], b.y - off[1], b.z - off[2])
}

/// `Vec3i.closerToCenterThan`: the distance from the cell's centre.
fn dist_center(b: BlockPos, p: Vec3) -> f64 {
    let (dx, dy, dz) = (b.x as f64 + 0.5 - p.x, b.y as f64 + 0.5 - p.y, b.z as f64 + 0.5 - p.z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// What a villager is: its brain's readings.
struct Read {
    pos: Vec3,
    bb: Aabb,
    vehicle: Option<u32>,
    sleeping: bool,
    act: String,
    panic: bool,
    ttl: Option<i64>,
    willing: bool,
    home: Option<BlockPos>,
    slept: Option<i64>,
    cant: Option<i64>,
    follow: f64,
    /// The iron golems in its nearest-living list (what its GolemSensor reads).
    golems_listed: Vec<u32>,
}

fn read_villager(w: &World, i: usize) -> Option<Read> {
    let e = &w.entities.list[i];
    let EntityKind::Mob(m) = &e.kind else { return None };
    let b = m.brain.as_deref()?;
    let ex = b.ex.as_deref();
    let act = match ex {
        Some(x) => x.active.name().to_string(),
        None => b.activity.name().to_string(),
    };
    let panic = match ex {
        Some(x) => x.nodes.iter().any(|n| matches!(n.kind, Kind::VillagerPanicTrigger) && n.running),
        None => b.panic_running,
    };
    Some(Read {
        pos: e.pos,
        bb: e.bb,
        vehicle: e.vehicle,
        sleeping: m.sleeping_pos.is_some(),
        act,
        panic,
        ttl: b.golem_detected,
        willing: b.wants_golem(w.time),
        home: b.home,
        slept: b.last_slept,
        cant: ex.and_then(|x| x.mem.cant_reach_since),
        follow: m.follow_range,
        golems_listed: b.nearest_living.iter().filter(|(_, ty)| *ty == MobType::IronGolem).map(|(id, _)| *id).collect(),
    })
}

/// Run the design's judge run (`spec`, `o`: already in the run's world; `offset`: design to world)
/// for `ticks` ticks after the arrival, sampling every tick.
pub fn record(title: &str, spec: &FarmSpec, o: &RunOpts, offset: [i32; 3], tags: &[Tag], ticks: u32) -> Result<Timeline> {
    let mut r = Run::new(spec, o).map_err(|e| anyhow!(e))?;
    let w = &r.world;
    let t0 = w.time;
    let day0 = w.day_time();
    let own = r.own.clone();
    let mut villagers: Vec<VLane> = Vec::new();
    let mut carts: Vec<CLane> = Vec::new();
    // the carts first (their riders' labels follow them), in the design's order
    for e in w.entities.list.iter().filter(|e| own.contains(&e.id)) {
        if matches!(e.kind, EntityKind::Minecart(_)) {
            carts.push(CLane { id: e.id, label: format!("C{}", carts.len() + 1), rider: Ch::default(), pos: Vec::new(), prev: None, moving: None });
        }
    }
    let cart_label = |carts: &[CLane], id: Option<u32>| -> String { id.and_then(|v| carts.iter().find(|c| c.id == v)).map(|c| c.label.clone()).unwrap_or_default() };
    for e in w.entities.list.iter().filter(|e| own.contains(&e.id)) {
        if matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager) {
            let first_cart = cart_label(&carts, e.vehicle);
            villagers.push(VLane {
                id: e.id,
                label: format!("V{}", villagers.len() + 1),
                first_cart,
                class: Ch::default(),
                act: Ch::default(),
                panic: Ch::default(),
                sleep: Ch::default(),
                lock: Ch::default(),
                willing: Ch::default(),
                cart: Ch::default(),
                home: Ch::default(),
                slept: Ch::default(),
                cant: Ch::default(),
                pos: Vec::new(),
                alive: true,
                prev: None,
                prev_ttl: None,
                over16: None,
                over1200: None,
            });
        }
    }
    // (a design with no villager still shows its carts, beds, tagged blocks and golems: a part's
    // bench, the counter's)
    // the beds: every bed head of the design, in its order
    let mut beds: Vec<BLane> = Vec::new();
    for (p, b) in &spec.blocks {
        let s = b.to_string();
        if s.contains("_bed[") && s.contains("part=head") {
            beds.push(BLane { head: *p, label: format!("B{} {}", beds.len() + 1, cell(*p, offset)), occ: Ch::default(), by: Ch::default(), nap: None });
        }
    }
    let at = |p: [i32; 3]| BlockPos::new(p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]);
    let signals: Vec<SLane> = tags.iter().filter_map(|t| if let Tag::Signal { name, at: p } = t { Some(SLane { name: name.clone(), at: Some(at(*p)), val: Ch::default(), full: Ch::default() }) } else { None }).collect();
    let regions: Vec<(String, [i32; 3], [i32; 3])> = tags
        .iter()
        .filter_map(|t| if let Tag::Region { name, lo, hi } = t { Some((name.clone(), std::array::from_fn(|k| lo[k] + offset[k]), std::array::from_fn(|k| hi[k] + offset[k]))) } else { None })
        .collect();
    let nv = villagers.len();
    // (the first dawn closes a whole night only when the run began before that night's dusk)
    let whole_night = day0.rem_euclid(24000) < 12000;
    let mut tl = Timeline { title: title.to_string(), seed: o.seed, t0, day0, ticks, offset, regions, villagers, carts, beds, signals, golems: Vec::new(), slots: Vec::new(), flags: Vec::new(), lines: Vec::new(), pending: Vec::new(), counts: Vec::new(), exposed: HashMap::new(), night_sleeps: vec![0; nv], whole_night, marks: Vec::new(), source: None };
    tl.sample(&r.world, 0, Vec::new());
    for k in 1..=ticks {
        r.tick();
        let events = std::mem::take(&mut r.world.mob.events);
        r.world.weather.events.clear();
        if let Some(e) = r.world.errors.first() {
            bail!("gt {}: the world went wrong: {e}", r.world.time);
        }
        tl.sample(&r.world, k, events);
    }
    tl.finish();
    Ok(tl)
}

impl Timeline {
    /// A timeline read from a text dump (`--from-dump`: a part's bench that is not a mob farm run,
    /// the counter's, writes its trace in the dump's format): each line `t=<tick> gt=<game time>
    /// [key=value...]` then `signal NAME=VALUE [(full state)]` (a lane per NAME), `mark NAME
    /// [text]` (a point event on lane NAME) or `VIOLATION <rule> <lane>: <text>` (a flag); any
    /// other line is skipped. Without `gt=`, the game time is the tick.
    pub fn from_dump(title: &str, path: &str, text: &str) -> Result<Timeline> {
        let mut tl = Timeline {
            title: title.to_string(),
            seed: 0,
            t0: 0,
            day0: 0,
            ticks: 0,
            offset: [0, 0, 0],
            regions: Vec::new(),
            villagers: Vec::new(),
            carts: Vec::new(),
            beds: Vec::new(),
            signals: Vec::new(),
            golems: Vec::new(),
            slots: Vec::new(),
            flags: Vec::new(),
            lines: Vec::new(),
            pending: Vec::new(),
            counts: Vec::new(),
            exposed: HashMap::new(),
            night_sleeps: Vec::new(),
            whole_night: false,
            marks: Vec::new(),
            source: Some(path.to_string()),
        };
        let mut t0: Option<i64> = None;
        let mut last = 0u32;
        for (n, line) in text.lines().enumerate() {
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some(k) = words.first().and_then(|w| w.strip_prefix("t=")).and_then(|v| v.parse::<u32>().ok()) else { continue };
            if k < last {
                bail!("{path}:{}: t={k} after t={last}: the dump's lines go in tick order", n + 1);
            }
            last = k;
            tl.ticks = tl.ticks.max(k);
            if t0.is_none()
                && let Some(gt) = words.iter().find_map(|w| w.strip_prefix("gt=")).and_then(|v| v.parse::<i64>().ok())
            {
                t0 = Some(gt - k as i64);
            }
            let Some(vi) = words.iter().position(|w| matches!(*w, "signal" | "mark" | "VIOLATION")) else { continue };
            match words[vi] {
                "signal" => {
                    let Some((name, value)) = words.get(vi + 1).and_then(|w| w.split_once('=')) else { bail!("{path}:{}: expected `signal NAME=VALUE`", n + 1) };
                    let full = line.split_once(" (").map(|(_, f)| f.trim_end().trim_end_matches(')').to_string()).unwrap_or_else(|| value.to_string());
                    let si = match tl.signals.iter().position(|s| s.name == name) {
                        Some(si) => si,
                        None => {
                            tl.signals.push(SLane { name: name.to_string(), at: None, val: Ch::default(), full: Ch::default() });
                            tl.signals.len() - 1
                        }
                    };
                    tl.signals[si].val.set(k, value.to_string());
                    tl.signals[si].full.set(k, full);
                }
                "mark" => {
                    let Some(name) = words.get(vi + 1) else { bail!("{path}:{}: expected `mark NAME [text]`", n + 1) };
                    let text = words[vi + 2..].join(" ");
                    match tl.marks.iter_mut().find(|m| m.name == *name) {
                        Some(m) => m.ev.push((k, text)),
                        None => tl.marks.push(MLane { name: name.to_string(), ev: vec![(k, text)] }),
                    }
                }
                _ => {
                    let rest = words[vi + 1..].join(" ");
                    let (head, text) = rest.split_once(": ").unwrap_or((rest.as_str(), ""));
                    let (rule, lane) = head.split_once(' ').unwrap_or((head, ""));
                    tl.flags.push(Flag { k, until: None, rule: rule.to_string(), lane: lane.to_string(), text: text.to_string() });
                }
            }
            tl.lines.push((k, line.to_string()));
        }
        if tl.lines.is_empty() {
            bail!("{path}: no `t=<tick> ... signal|mark|VIOLATION` line");
        }
        tl.t0 = t0.unwrap_or(0);
        // (a bench's game time from a fresh world: its day time is its game time)
        tl.day0 = tl.t0;
        Ok(tl)
    }

    fn stamp(&self, k: u32) -> String {
        let gt = self.t0 + k as i64;
        let dt = self.day0 + k as i64;
        format!("t={k:06} gt={gt} day={} dt={:05}", dt.div_euclid(24000) - self.day0.div_euclid(24000), dt.rem_euclid(24000))
    }

    fn line(&mut self, k: u32, s: String) {
        let st = self.stamp(k);
        self.lines.push((k, format!("{st} {s}")));
    }

    fn flag(&mut self, k: u32, rule: &str, lane: &str, text: String) {
        self.line(k, format!("VIOLATION {rule} {lane}: {text}"));
        self.flags.push(Flag { k, until: None, rule: rule.to_string(), lane: lane.to_string(), text });
    }

    /// The tick of game time `gt` (relative to the arrival).
    fn tick_of(&self, gt: i64) -> i64 {
        gt - self.t0
    }

    fn villager_label(&self, id: u32) -> String {
        self.villagers.iter().find(|v| v.id == id).map(|v| v.label.clone()).unwrap_or_else(|| format!("villager {id}"))
    }

    fn region_of(&self, p: Vec3) -> Option<&str> {
        let c = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        self.regions.iter().find(|(_, lo, hi)| (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k])).map(|(n, _, _)| n.as_str())
    }

    fn has_station(&self) -> bool {
        self.regions.iter().any(|(n, _, _)| n.starts_with("station"))
    }

    fn sample(&mut self, w: &World, k: u32, events: Vec<MobEvent>) {
        let t = w.time;
        let off = self.offset;
        let index: HashMap<u32, usize> = w.entities.list.iter().enumerate().filter(|(_, e)| !e.removed).map(|(i, e)| (e.id, i)).collect();
        // the events: attempts, golems, sensor locks, releases
        let mut tries: HashMap<u32, (u32, Option<u32>, Option<[i32; 3]>)> = HashMap::new();
        let mut seen: Vec<u32> = Vec::new();
        for ev in &events {
            match ev {
                MobEvent::GolemTry { villager, willing, golem, at, .. } => {
                    tries.insert(*villager, (*willing, *golem, *at));
                    if golem.is_none() {
                        self.counts.push((k, "fail"));
                    }
                    if let (Some(g), Some(p)) = (golem, at) {
                        let by = self.villager_label(*villager);
                        let label = format!("G{}", self.golems.len() + 1);
                        let p = [p[0] - off[0], p[1] - off[1], p[2] - off[2]];
                        self.line(k, format!("golem {label} spawned at {},{},{} (summoned by {by})", p[0], p[1], p[2]));
                        self.golems.push(Golem { id: *g, label, t: k, by, at: p, died: None, gone: None, batch: Vec::new() });
                    }
                }
                MobEvent::GolemSeen { villager, .. } => {
                    self.counts.push((k, "sight"));
                    seen.push(*villager);
                }
                MobEvent::Slept { id, .. } => {
                    self.counts.push((k, "sleep"));
                    if let Some(vi) = self.villagers.iter().position(|v| v.id == *id) {
                        self.night_sleeps[vi] += 1;
                    }
                }
                MobEvent::Death { id, src, pos, .. } => {
                    if let Some(gi) = self.golems.iter().position(|g| g.id == *id && g.died.is_none()) {
                        let how = format!("{src:?}").to_lowercase();
                        let label = self.golems[gi].label.clone();
                        self.golems[gi].died = Some((k, how.clone()));
                        self.line(k, format!("golem {label} died ({how}) at {:.2},{:.2},{:.2}", pos[0] - off[0] as f64, pos[1] - off[1] as f64, pos[2] - off[2] as f64));
                    }
                }
                MobEvent::Removed { id, .. } => {
                    if let Some(gi) = self.golems.iter().position(|g| g.id == *id && g.gone.is_none()) {
                        // (removed this tick: its last position is the previous tick's; a portal's
                        // removal keeps it in the list until the end of the tick)
                        let p = index.get(id).map(|&i| w.entities.list[i].pos).or_else(|| w.entities.list.iter().find(|e| e.id == *id).map(|e| e.pos));
                        let p = p.map(|p| [p.x - off[0] as f64, p.y - off[1] as f64, p.z - off[2] as f64]).unwrap_or([f64::NAN; 3]);
                        let g = &mut self.golems[gi];
                        g.gone = Some((k, p));
                        let (label, how) = (g.label.clone(), if g.died.is_some() { "removed (dead)" } else { "left the world without dying (a portal)" });
                        let life = k - g.t;
                        self.line(k, format!("golem {label} {how} at {:.2},{:.2},{:.2}, {life} gt after its spawn", p[0], p[1], p[2]));
                    }
                }
                MobEvent::Released { id, .. } => {
                    self.counts.push((k, "release"));
                    let l = self.villager_label(*id);
                    self.line(k, format!("{l} released a POI (Villager.releasePoi)"));
                }
                _ => {}
            }
        }
        // the villagers
        let mut reads: Vec<Option<Read>> = Vec::new();
        for vi in 0..self.villagers.len() {
            let id = self.villagers[vi].id;
            let rd = index.get(&id).and_then(|&i| read_villager(w, i));
            if rd.is_none() && self.villagers[vi].alive {
                self.villagers[vi].alive = false;
                let l = self.villagers[vi].label.clone();
                self.line(k, format!("{l} is gone (died, converted or removed)"));
            }
            reads.push(rd);
        }
        let cart_labels: HashMap<u32, String> = self.carts.iter().map(|c| (c.id, c.label.clone())).collect();
        for vi in 0..self.villagers.len() {
            let Some(rd) = &reads[vi] else { continue };
            let moved = self.villagers[vi].prev.map(|p| (rd.pos.x - p.x).hypot(rd.pos.z - p.z).hypot(rd.pos.y - p.y)).unwrap_or(0.0);
            let class = if rd.sleeping {
                "asleep".to_string()
            } else if moved >= 0.005 {
                "travelling".to_string()
            } else if let Some(n) = self.region_of(rd.pos) {
                n.to_string()
            } else if rd.vehicle.is_none() && rd.home.is_some_and(|h| dist_center(h, rd.pos) < 2.0) {
                "at-bed".to_string()
            } else if rd.vehicle.is_some() {
                "held".to_string()
            } else {
                "loose".to_string()
            };
            let cart = rd.vehicle.map(|v| cart_labels.get(&v).cloned().unwrap_or_else(|| format!("entity {v}"))).unwrap_or_default();
            let home = rd.home.map(|h| cell(h, off)).unwrap_or_default();
            let slept = rd.slept.map(|s| self.tick_of(s)).unwrap_or(i64::MIN);
            let cant = rd.cant.map(|s| self.tick_of(s)).unwrap_or(i64::MIN);
            // the lock's end: the tick it is free again (its time to live after this tick, counted
            // down one a tick: gone the tick after it reaches 0); a fresh lock (or a refresh) when
            // the time to live did not count down
            let lock_exp = match rd.ttl {
                None => -1,
                Some(ttl) => {
                    let fresh = self.villagers[vi].prev_ttl.is_none_or(|p| ttl > p - 1);
                    let last = self.villagers[vi].lock.v.last().map(|(_, e)| *e).unwrap_or(-1);
                    if fresh || last < 0 { k as i64 + ttl + 1 } else { last }
                }
            };
            let v = &mut self.villagers[vi];
            let mut said: Vec<String> = Vec::new();
            if v.class.set(k, class.clone()) {
                said.push(format!("class={class} at {}", fmt_pos(rd.pos, off)));
            }
            if v.act.set(k, rd.act.clone()) {
                said.push(format!("act={}", rd.act));
            }
            if v.panic.set(k, rd.panic) {
                said.push(format!("panic={}", if rd.panic { "on" } else { "off" }));
            }
            if v.sleep.set(k, rd.sleeping) {
                said.push(format!("sleeping={}", if rd.sleeping { "yes" } else { "no" }));
            }
            if v.lock.set(k, lock_exp) {
                said.push(if lock_exp < 0 { "lock=off".to_string() } else { format!("lock=on free-at=t={lock_exp:06}") });
            }
            if v.willing.set(k, rd.willing) {
                said.push(format!("wants-golem={}", if rd.willing { "yes" } else { "no" }));
            }
            if v.cart.set(k, cart.clone()) {
                said.push(format!("cart={}", if cart.is_empty() { "-" } else { &cart }));
            }
            if v.home.set(k, home.clone()) {
                said.push(format!("home={}", if home.is_empty() { "-" } else { &home }));
            }
            if v.slept.set(k, slept) {
                said.push(if slept == i64::MIN { "last-slept=-".to_string() } else { format!("last-slept=t={slept:06}") });
            }
            if v.cant.set(k, cant) {
                said.push(if cant == i64::MIN { "cant-reach=-".to_string() } else { format!("cant-reach-since=t={cant:06}") });
            }
            let q = [(rd.pos.x * 100.0).round() as i32, (rd.pos.y * 100.0).round() as i32, (rd.pos.z * 100.0).round() as i32];
            if v.pos.last().is_none_or(|(_, p)| *p != q) {
                v.pos.push((k, q));
            }
            v.prev = Some(rd.pos);
            v.prev_ttl = rd.ttl;
            if !said.is_empty() {
                let l = v.label.clone();
                self.line(k, format!("{l} {}", said.join(" ")));
            }
        }
        // the carts
        for ci in 0..self.carts.len() {
            let id = self.carts[ci].id;
            let Some(&i) = index.get(&id) else { continue };
            let e = &w.entities.list[i];
            let rider = e.passengers.first().map(|p| self.villager_label(*p)).unwrap_or_default();
            let c = &mut self.carts[ci];
            let mut said: Vec<String> = Vec::new();
            if c.rider.set(k, rider.clone()) {
                said.push(format!("rider={}", if rider.is_empty() { "-" } else { &rider }));
            }
            let step = c.prev.map(|p| (e.pos.x - p.x).hypot(e.pos.z - p.z)).unwrap_or(0.0);
            let q = [(e.pos.x * 100.0).round() as i32, (e.pos.y * 100.0).round() as i32, (e.pos.z * 100.0).round() as i32];
            if c.pos.last().is_none_or(|(_, p)| *p != q) {
                c.pos.push((k, q));
            }
            match c.moving {
                None if step >= 0.005 => {
                    let from = c.prev.unwrap_or(e.pos);
                    c.moving = Some((k, from, step));
                    said.push(format!("departs from {}", fmt_pos(from, off)));
                }
                Some((k0, from, top)) if step < 0.005 => {
                    c.moving = None;
                    let d = (e.pos.x - from.x).hypot(e.pos.z - from.z);
                    said.push(format!("stops at {} ({} gt, {:.2} blocks from its start, top speed {:.3})", fmt_pos(e.pos, off), k - k0, d, top));
                }
                Some((k0, from, top)) => c.moving = Some((k0, from, top.max(step))),
                None => {}
            }
            c.prev = Some(e.pos);
            if !said.is_empty() {
                let l = c.label.clone();
                self.line(k, format!("{l} {}", said.join(", ")));
            }
        }
        // the beds, and the naps' claimants (the 16 rule's distances)
        for bi in 0..self.beds.len() {
            let head = self.beds[bi].head;
            let occ = w.get(head).to_string().contains("occupied=true");
            let sleeper = reads.iter().zip(&self.villagers).find(|(r, _)| r.as_ref().is_some_and(|r| r.sleeping && r.pos.distance_to_sqr(Vec3::new(head.x as f64 + 0.5, r.pos.y, head.z as f64 + 0.5)) < 1.0)).map(|(_, v)| v.label.clone()).unwrap_or_default();
            let homes: Vec<(String, f64)> = reads.iter().zip(&self.villagers).filter_map(|(r, v)| r.as_ref().filter(|r| r.home == Some(head) && !r.sleeping).map(|r| (v.label.clone(), dist_center(head, r.pos)))).collect();
            let b = &mut self.beds[bi];
            let mut said: Vec<String> = Vec::new();
            if b.occ.set(k, occ) {
                said.push(format!("occupied={}", if occ { "yes" } else { "no" }));
            }
            if b.by.set(k, sleeper.clone()) && !sleeper.is_empty() {
                said.push(format!("sleeper={sleeper}"));
            }
            match (&mut b.nap, occ) {
                (None, true) => b.nap = Some((k, sleeper.clone(), homes.into_iter().collect())),
                (Some((_, _, near)), true) => {
                    for (l, d) in homes {
                        let e = near.entry(l).or_insert(d);
                        *e = e.min(d);
                    }
                }
                (Some((k0, who, near)), false) => {
                    let mut ns: Vec<String> = near.iter().map(|(l, d)| format!("{l} {d:.1}")).collect();
                    ns.sort();
                    said.push(format!("nap of {} gt by {} over; its other claimants came nearest: {}", k - *k0, if who.is_empty() { "?" } else { who.as_str() }, if ns.is_empty() { "none".to_string() } else { ns.join(", ") }));
                    b.nap = None;
                }
                (None, false) => {}
            }
            if !said.is_empty() {
                let l = b.label.clone();
                self.line(k, format!("bed {l} {}", said.join(", ")));
            }
        }
        // the tagged blocks
        for si in 0..self.signals.len() {
            let Some(at) = self.signals[si].at else { continue };
            let s = w.get(at).to_string();
            if self.signals[si].full.set(k, s.clone()) {
                let v = signal_value(&s);
                self.signals[si].val.set(k, v.clone());
                let n = self.signals[si].name.clone();
                self.line(k, format!("signal {n}={v} ({s})"));
            }
        }
        // the 16 rule and CANT_REACH over 1200, villager by villager
        let occupied = |h: BlockPos| w.get(h).to_string().contains("occupied=true");
        for vi in 0..self.villagers.len() {
            let Some(rd) = &reads[vi] else { continue };
            let label = self.villagers[vi].label.clone();
            let in_rest = rd.act == "rest";
            let near16 = match rd.home {
                Some(h) if in_rest && !rd.sleeping && occupied(h) => {
                    let d = dist_center(h, rd.pos);
                    (d < 16.0).then_some(d)
                }
                _ => None,
            };
            match (self.villagers[vi].over16, near16) {
                (None, Some(d)) => {
                    self.villagers[vi].over16 = Some((k, d));
                    let h = cell(rd.home.unwrap(), off);
                    self.flag(k, "16", &label, format!("awake in REST {d:.2} from its home bed {h} while the bed is OCCUPIED: ValidateNearbyPoi drops its HOME"));
                }
                (Some((k0, dmin)), Some(d)) => self.villagers[vi].over16 = Some((k0, dmin.min(d))),
                (Some((k0, dmin)), None) => {
                    self.villagers[vi].over16 = None;
                    if let Some(f) = self.flags.iter_mut().rev().find(|f| f.rule == "16" && f.lane == label && f.k == k0) {
                        f.until = Some(k);
                    }
                    self.line(k, format!("{label} 16-rule span over ({} gt, nearest {dmin:.2})", k - k0));
                }
                (None, None) => {}
            }
            let age = rd.cant.map(|s| t - s);
            let over = in_rest && rd.home.is_some() && age.is_some_and(|a| a > 1200);
            match (self.villagers[vi].over1200, over) {
                (None, true) => {
                    self.villagers[vi].over1200 = Some(k);
                    self.flag(k, "1200", &label, format!("CANT_REACH_WALK_TARGET_SINCE {} gt old in REST with its HOME: SetWalkTargetFromBlockMemory releases the HOME at its next run", age.unwrap()));
                }
                (Some(k0), false) => {
                    self.villagers[vi].over1200 = None;
                    if let Some(f) = self.flags.iter_mut().rev().find(|f| f.rule == "1200" && f.lane == label && f.k == k0) {
                        f.until = Some(k);
                    }
                }
                _ => {}
            }
        }
        // the golem sensor's locks: the golems in the villager's nearest-living list (what its
        // GolemSensor read: the list of the living-entity sensor's last scan, up to 20 gt old)
        for id in seen {
            let Some(vi) = self.villagers.iter().position(|v| v.id == id) else { continue };
            let Some(rd) = &reads[vi] else { continue };
            let gs: Vec<String> = rd
                .golems_listed
                .iter()
                .map(|gid| {
                    let label = self.golems.iter().find(|g| g.id == *gid).map(|g| g.label.clone()).unwrap_or_else(|| format!("golem {gid}"));
                    match index.get(gid).map(|&i| &w.entities.list[i]) {
                        Some(e) => format!("{label} at {} (dy {:+.1})", fmt_pos(e.pos, off), e.pos.y - rd.pos.y),
                        None => format!("{label} (gone since the list's scan)"),
                    }
                })
                .collect();
            let label = self.villagers[vi].label.clone();
            let class = self.villagers[vi].class.v.last().map(|(_, c)| c.clone()).unwrap_or_default();
            // (its lock before this tick's sighting: the lock lane's entry before this tick's)
            let lk = &self.villagers[vi].lock.v;
            let prev = if lk.last().is_some_and(|(kk, _)| *kk == k) { lk.iter().rev().nth(1).map(|(_, e)| *e) } else { lk.last().map(|(_, e)| *e) }.unwrap_or(-1);
            let seen = if gs.is_empty() { "a golem".to_string() } else { gs.join(", ") };
            self.line(k, format!("{label} golem sensor saw {seen} (follow range {:.2}) while {class}: locked 599 gt", rd.follow));
            self.pending.push(Pending::Follow { k, vi, seen, follow: rd.follow, prev });
        }
        // the slot (gameTime % 100 == 0): every villager whose panic trigger runs summons
        // (`VillagerPanicTrigger.tick`); a try off the slots is the gossip path's
        // (`Villager.gossip`: `spawnGolemIfNeeded(5)`), flagged
        let slot_tick = t % 100 == 0;
        let tried_here: Vec<usize> = (0..self.villagers.len()).filter(|&vi| tries.contains_key(&self.villagers[vi].id)).collect();
        if slot_tick || !tried_here.is_empty() {
            let mut summoners: Vec<usize> = if slot_tick { (0..self.villagers.len()).filter(|&vi| reads[vi].as_ref().is_some_and(|r| r.panic)).collect() } else { Vec::new() };
            for vi in tried_here {
                if !summoners.contains(&vi) && reads[vi].is_some() {
                    summoners.push(vi);
                }
            }
            summoners.sort();
            let mut slot = Slot { k, summoners: Vec::new() };
            for &si in &summoners {
                let s = reads[si].as_ref().unwrap();
                let bx = s.bb.inflate(10.0, 10.0, 10.0);
                let mut others = Vec::new();
                let mut in_box_willing = s.willing as u32;
                for (vi, r) in reads.iter().enumerate() {
                    let Some(r) = r else { continue };
                    if vi == si {
                        continue;
                    }
                    let inside = r.bb.intersects(bx);
                    let (dx, dy, dz) = (r.pos.x - s.pos.x, r.pos.y - s.pos.y, r.pos.z - s.pos.z);
                    if inside {
                        in_box_willing += r.willing as u32;
                    }
                    // (inside, or within 2 blocks of the box: the margins)
                    if inside || (dx.abs() < 12.6 && dz.abs() < 12.6 && dy.abs() < 13.95) {
                        others.push((self.villagers[vi].label.clone(), dx, dy, dz, r.willing, inside));
                    }
                }
                let who = self.villagers[si].label.clone();
                let (mut result, golem, willing) = match tries.get(&self.villagers[si].id) {
                    Some((wl, Some(g), _)) => {
                        let gl = self.golems.iter().find(|x| x.id == *g).map(|x| x.label.clone()).unwrap_or_default();
                        (format!("spawned {gl} ({wl} willing)"), Some(gl), *wl)
                    }
                    Some((wl, None, _)) => (format!("no spot in 10 tries ({wl} willing)"), None, *wl),
                    None if !s.willing => (if s.ttl.is_some() { "no try: it is locked".to_string() } else { "no try: it has not slept within 24000 gt".to_string() }, None, 0),
                    None => (format!("no try: {} willing in its 10-box (3 needed)", in_box_willing.min(5)), None, in_box_willing.min(5)),
                };
                if slot_tick && !s.panic {
                    result.push_str(" (its panic trigger is not running after the tick)");
                }
                slot.summoners.push(Summoner { who, result, golem, willing, others });
            }
            if !slot_tick {
                for s in &slot.summoners {
                    let text = format!("a summon off the 100 gt grid (the gossip path, spawnGolemIfNeeded(5)): {}", s.result);
                    self.flag(k, "gossip", &s.who, text);
                }
            }
            if !slot.summoners.is_empty() {
                for s in &slot.summoners {
                    let box_: Vec<String> = s.others.iter().map(|(l, dx, _, dz, wl, inside)| format!("{l} {dx:+.2},{dz:+.2}{}{}", if *inside { " in" } else { " out" }, if *wl { " willing" } else { "" })).collect();
                    let line = format!("slot {} {}; box: {}", s.who, s.result, if box_.is_empty() { "nobody near".to_string() } else { box_.join("; ") });
                    self.line(k, line);
                }
                // the 10.6 rule: at a spawn, every villager in the summoner's 10-box (its role,
                // in the summoner's group or not, needs the rest of the run: `finish`)
                let spawns: Vec<(String, String, Vec<(String, f64, f64, f64, bool, bool)>)> = slot.summoners.iter().filter_map(|s| s.golem.clone().map(|g| (s.who.clone(), g, s.others.clone()))).collect();
                for (who, g, others) in spawns {
                    let mut batch: Vec<usize> = self.villagers.iter().position(|v| v.label == who).into_iter().collect();
                    for (l, dx, dy, dz, _, inside) in others {
                        if inside {
                            let vi = self.villagers.iter().position(|v| v.label == l).unwrap();
                            batch.push(vi);
                            self.pending.push(Pending::Spawn { k, who: who.clone(), g: g.clone(), vi, d: [dx, dy, dz] });
                        }
                    }
                    if let Some(gl) = self.golems.iter_mut().find(|x| x.label == g) {
                        gl.batch = batch;
                    }
                }
                self.slots.push(slot);
            }
        }
        // the follow-box exposure (the brief's timing diagram: during each golem's overworld dwell
        // every villager outside its summoning batch, the arriving batch included, stays out of
        // the golem's follow box): the villager's box inflated by its follow range (what its
        // NearestLivingEntitySensor lists, no sight test) meeting the golem's box, at any tick,
        // whatever its sensors' phase
        let mut now: Vec<(usize, usize)> = Vec::new();
        for gi in 0..self.golems.len() {
            let g = &self.golems[gi];
            if g.gone.is_some() {
                continue;
            }
            let Some(&ei) = index.get(&g.id) else { continue };
            let gb = w.entities.list[ei].bb;
            for (vi, r) in reads.iter().enumerate() {
                let Some(r) = r else { continue };
                if g.batch.contains(&vi) {
                    continue;
                }
                if r.bb.inflate(r.follow, r.follow, r.follow).intersects(gb) {
                    now.push((gi, vi));
                }
            }
        }
        for &(gi, vi) in &now {
            if !self.exposed.contains_key(&(gi, vi)) {
                self.exposed.insert((gi, vi), k);
                let g = &self.golems[gi];
                let r = reads[vi].as_ref().unwrap();
                let gp = index.get(&g.id).map(|&i| w.entities.list[i].pos).unwrap_or(r.pos);
                let class = self.villagers[vi].class.v.last().map(|(_, c)| c.clone()).unwrap_or_default();
                let text = format!(
                    "{} (spawned t={:06} by {}) in its follow box (range {:.2}) {} gt into its life: the golem at {}, the villager at {} ({class}; dx {:+.2} dy {:+.2} dz {:+.2})",
                    g.label,
                    g.t,
                    g.by,
                    r.follow,
                    k - g.t,
                    fmt_pos(gp, off),
                    fmt_pos(r.pos, off),
                    gp.x - r.pos.x,
                    gp.y - r.pos.y,
                    gp.z - r.pos.z
                );
                let l = self.villagers[vi].label.clone();
                self.flag(k, "follow-box", &l, text);
            }
        }
        let ended: Vec<((usize, usize), u32)> = self.exposed.iter().filter(|(key, _)| !now.contains(key)).map(|(key, k0)| (*key, *k0)).collect();
        for ((gi, vi), k0) in ended {
            self.exposed.remove(&(gi, vi));
            let l = self.villagers[vi].label.clone();
            let tag = format!("{} (spawned", self.golems[gi].label);
            if let Some(f) = self.flags.iter_mut().rev().find(|f| f.rule == "follow-box" && f.lane == l && f.k == k0 && f.text.starts_with(&tag)) {
                f.until = Some(k);
            }
        }
        // every dawn and midday: every villager holds a HOME (the brief's judge: HOME holders at
        // every dawn and midday); every dawn: each slept exactly once since the last dawn
        let dt = (self.day0 + k as i64).rem_euclid(24000);
        if k > 0 && (dt == 0 || dt == 6000) {
            for vi in 0..self.villagers.len() {
                if let Some(r) = &reads[vi]
                    && r.home.is_none()
                {
                    let l = self.villagers[vi].label.clone();
                    self.flag(k, "home", &l, format!("no HOME at {}", if dt == 0 { "dawn" } else { "midday" }));
                }
            }
        }
        if k > 0 && dt == 0 {
            if self.whole_night {
                for vi in 0..self.villagers.len() {
                    let n = self.night_sleeps[vi];
                    if n != 1 && reads[vi].is_some() {
                        let l = self.villagers[vi].label.clone();
                        self.flag(k, "sleep", &l, format!("slept {n} times since the last dawn (LAST_SLEPT advances once a night)"));
                    }
                }
            }
            self.night_sleeps.iter_mut().for_each(|n| *n = 0);
            self.whole_night = true;
        }
    }

    /// A villager's role at tick `k`, and whether it belongs with the station's group there: at
    /// the station (a region `station...`), or travelling into or out of it (its class before and
    /// after the travel, read from the rest of the run). With no station region, the group is the
    /// villagers whose panic trigger runs within 100 gt of `k`.
    fn role(&self, vi: usize, k: u32) -> (String, bool) {
        let v = &self.villagers[vi];
        let c = &v.class.v;
        let i = c.partition_point(|(kk, _)| *kk <= k);
        let here = if i > 0 { c[i - 1].1.clone() } else { String::new() };
        if !self.has_station() {
            let near = v.panic.v.iter().any(|(kk, on)| *on && kk.abs_diff(k) <= 100) || v.panic.window(k, k).first().is_some_and(|(_, on)| *on);
            return (here, near);
        }
        if here.starts_with("station") {
            return (here, true);
        }
        if here == "travelling" {
            let next = c[i.min(c.len())..].iter().find(|(_, x)| x != "travelling");
            let prev = c[..i.saturating_sub(1)].iter().rev().find(|(_, x)| x != "travelling");
            if let Some((kn, n)) = next
                && n.starts_with("station")
            {
                return (format!("arriving at the {n} (t={kn:06})"), true);
            }
            if let Some((_, p)) = prev
                && p.starts_with("station")
            {
                return ("leaving the station".to_string(), false);
            }
            let name = |x: Option<&(u32, String)>| x.map(|(_, s)| s.clone()).unwrap_or_else(|| "?".into());
            return (format!("travelling from {} to {}", name(prev), name(next)), false);
        }
        (here, false)
    }

    /// The tick villager `vi` reached the station for the visit holding slot `k` (a cart's one- or
    /// two-tick jitter at the station does not count as leaving it), if it was at the station or
    /// travelling into it at `k`.
    fn arrival(&self, vi: usize, k: u32) -> Option<u32> {
        let c = &self.villagers[vi].class.v;
        let i = c.partition_point(|(kk, _)| *kk <= k).checked_sub(1)?;
        if c[i].1.starts_with("station") {
            let mut j = i;
            while j >= 2 && c[j - 1].1 == "travelling" && c[j].0 - c[j - 1].0 <= 2 && c[j - 2].1.starts_with("station") {
                j -= 2;
            }
            return Some(c[j].0);
        }
        if c[i].1 == "travelling" {
            return c.get(i + 1).filter(|(_, x)| x.starts_with("station")).map(|(kk, _)| *kk);
        }
        None
    }

    /// Resolve the checks that needed the rest of the run, and order the flags and lines.
    fn finish(&mut self) {
        // the arrival margins (the brief's timing diagram: a batch reaches the station at least 39
        // gt before its slot, for the panic onset; none needed by a villager that arrives already
        // panicking)
        if self.has_station() {
            for gi in 0..self.golems.len() {
                let (k, label, by, batch) = (self.golems[gi].t, self.golems[gi].label.clone(), self.golems[gi].by.clone(), self.golems[gi].batch.clone());
                let mut parts: Vec<String> = Vec::new();
                let mut short: Vec<String> = Vec::new();
                for &vi in &batch {
                    let Some(a) = self.arrival(vi, k) else { continue };
                    let m = k as i64 - a as i64;
                    let panicking = a > 0 && self.villagers[vi].panic.window(a - 1, a - 1).first().is_some_and(|(_, p)| *p);
                    let l = self.villagers[vi].label.clone();
                    parts.push(format!("{l} {m:+}{}", if panicking { " (arrived panicking)" } else { "" }));
                    if m < 39 && !panicking {
                        short.push(format!("{l} {m:+}"));
                    }
                }
                if parts.is_empty() {
                    continue;
                }
                self.line(k, format!("margin of {label}'s batch (gt from its arrival at the station to the slot): {}", parts.join(", ")));
                if !short.is_empty() {
                    self.flag(k, "arrival", &by, format!("{label}'s batch: {} reached the station less than 39 gt before the slot (the panic onset)", short.join(", ")));
                }
            }
        }
        for p in std::mem::take(&mut self.pending) {
            match p {
                Pending::Spawn { k, who, g, vi, d } => {
                    let (role, grouped) = self.role(vi, k);
                    let l = self.villagers[vi].label.clone();
                    let text = format!("{role}, in {who}'s 10-box at the spawn of {g} (dx {:+.2} dy {:+.2} dz {:+.2}): locked 600 gt", d[0], d[1], d[2]);
                    if grouped {
                        self.line(k, format!("{l} locked by the spawn of {g}: {text}"));
                    } else {
                        self.flag(k, "10.6", &l, text);
                    }
                }
                Pending::Follow { k, vi, seen, follow, prev } => {
                    let (role, grouped) = self.role(vi, k);
                    // a sighting at the station is the station's own business; a villager of the
                    // station's group arriving or leaving that its group's spawn already locked
                    // only has its lock extended (a line); anywhere else it locks (or keeps
                    // locked) a villager the next slots need
                    if role.starts_with("station") {
                        continue;
                    }
                    let l = self.villagers[vi].label.clone();
                    let locked = prev > k as i64;
                    let free = |e: i64| format!("free at t={e:06}");
                    let now = self.villagers[vi].lock.window(k, k).first().map(|(_, e)| *e).unwrap_or(-1);
                    let before = if locked { format!("already locked ({}), now {}", free(prev), free(now)) } else { format!("locked, {}", free(now)) };
                    let text = format!("its golem sensor saw {seen} (follow range {follow:.2}) while {role}: {before}");
                    if grouped && locked {
                        self.line(k, format!("{l} lock extended: {text}"));
                    } else {
                        self.flag(k, "follow", &l, text);
                    }
                }
            }
        }
        // each CANT_REACH span: did the HOME go?
        for fi in 0..self.flags.len() {
            if self.flags[fi].rule != "1200" {
                continue;
            }
            let (k0, until) = (self.flags[fi].k, self.flags[fi].until.unwrap_or(self.ticks));
            let Some(v) = self.villagers.iter().find(|v| v.label == self.flags[fi].lane) else { continue };
            let gone = v.home.v.iter().find(|(kk, h)| *kk >= k0 && *kk <= until + 1 && h.is_empty()).map(|(kk, _)| *kk);
            let note = match gone {
                Some(kk) => format!(" (released at t={kk:06})"),
                None => format!(" (kept: the span ended at t={until:06})"),
            };
            self.flags[fi].text.push_str(&note);
            let tag = format!("VIOLATION 1200 {}: ", self.flags[fi].lane);
            if let Some((_, line)) = self.lines.iter_mut().find(|(kk, l)| *kk == k0 && l.contains(&tag)) {
                line.push_str(&note);
            }
        }
        self.flags.sort_by_key(|f| f.k);
        self.lines.sort_by_key(|(k, _)| *k);
    }

    /// The text dump of the window `[from, to]`.
    pub fn text(&self, from: u32, to: u32) -> String {
        let mut s = String::new();
        s.push_str(&format!("# {} seed {}: {} ticks from the arrival at gt {} (day time {}); window t={from:06}..t={to:06}\n", self.title, self.seed, self.ticks, self.t0, self.day0.rem_euclid(24000)));
        s.push_str("# t = ticks after the arrival, gt = game time (the slots are gt % 100 == 0), day = days since the arrival's day, dt = day time\n");
        s.push_str(&format!("# {}\n", self.summary(from, to)));
        let names: Vec<String> = self.villagers.iter().map(|v| format!("{} (id {}, starts in {})", v.label, v.id, if v.first_cart.is_empty() { "no cart" } else { &v.first_cart })).collect();
        s.push_str(&format!("# villagers: {}\n", names.join(", ")));
        if !self.regions.is_empty() {
            let rs: Vec<String> = self.regions.iter().map(|(n, lo, hi)| format!("{n} {} to {}", cell(BlockPos::new(lo[0], lo[1], lo[2]), self.offset), cell(BlockPos::new(hi[0], hi[1], hi[2]), self.offset))).collect();
            s.push_str(&format!("# regions: {}\n", rs.join(", ")));
        }
        for (k, l) in &self.lines {
            if *k >= from && *k <= to {
                s.push_str(l);
                s.push('\n');
            }
        }
        s
    }

    /// The window's counts: slots, spawns, failed tries, no tries, violations by rule.
    pub fn summary(&self, from: u32, to: u32) -> String {
        if let Some(src) = &self.source {
            let marks: usize = self.marks.iter().map(|m| m.ev.iter().filter(|(k, _)| *k >= from && *k <= to).count()).sum();
            let flags = self.flags.iter().filter(|f| f.k >= from && f.k <= to).count();
            return format!("from {src}: {} signal lanes, {} mark lanes ({marks} marks), {flags} violations", self.signals.len(), self.marks.len());
        }
        let slots: Vec<&Slot> = self.slots.iter().filter(|s| s.k >= from && s.k <= to).collect();
        let spawned = slots.iter().filter(|s| s.summoners.iter().any(|m| m.golem.is_some())).count();
        let tried = slots.iter().filter(|s| s.summoners.iter().all(|m| m.golem.is_none()) && s.summoners.iter().any(|m| m.result.starts_with("no spot"))).count();
        let none = slots.len() - spawned - tried;
        let mut by: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for f in self.flags.iter().filter(|f| f.k >= from && f.k <= to) {
            *by.entry(f.rule.as_str()).or_insert(0) += 1;
        }
        let v: Vec<String> = by.iter().map(|(r, n)| format!("{r} x{n}")).collect();
        let golems = self.golems.iter().filter(|g| g.t >= from && g.t <= to).count();
        let n = |what: &str| self.counts.iter().filter(|(k, c)| *k >= from && *k <= to && *c == what).count();
        format!(
            "{} slots with summoners: {spawned} spawned, {tried} tried without a spot, {none} without a try; violations: {}; the judge's counters: {golems} golems, {} tries without a spot, {} sightings, {} sleeps, {} POI releases",
            slots.len(),
            if v.is_empty() { "none".to_string() } else { v.join(", ") },
            n("fail"),
            n("sight"),
            n("sleep"),
            n("release")
        )
    }

    /// The page's data (JSON) of the window `[from, to]`.
    pub fn json(&self, from: u32, to: u32) -> serde_json::Value {
        use serde_json::json;
        let ch_s = |c: &Ch<String>| -> serde_json::Value { json!(c.window(from, to).into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>()) };
        let ch_b = |c: &Ch<bool>| -> serde_json::Value { json!(c.window(from, to).into_iter().map(|(k, v)| json!([k, v as u8])).collect::<Vec<_>>()) };
        let ch_i = |c: &Ch<i64>| -> serde_json::Value { json!(c.window(from, to).into_iter().map(|(k, v)| json!([k, if v == i64::MIN { serde_json::Value::Null } else { json!(v) }])).collect::<Vec<_>>()) };
        // a position track, flat and delta coded (t, x, y, z in hundredths), the window's
        let track = |p: &[(u32, [i32; 3])]| -> serde_json::Value {
            let i = p.partition_point(|(k, _)| *k <= from);
            let mut pts: Vec<(u32, [i32; 3])> = Vec::new();
            if i > 0 {
                pts.push((from, p[i - 1].1));
            }
            pts.extend(p[i..].iter().take_while(|(k, _)| *k <= to).cloned());
            let mut flat: Vec<i64> = Vec::with_capacity(pts.len() * 4);
            let mut last = (0i64, [0i64; 3]);
            for (k, q) in pts {
                flat.push(k as i64 - last.0);
                for a in 0..3 {
                    flat.push(q[a] as i64 - last.1[a]);
                }
                last = (k as i64, [q[0] as i64, q[1] as i64, q[2] as i64]);
            }
            json!(flat)
        };
        let off = self.offset;
        let mut lanes: Vec<serde_json::Value> = Vec::new();
        for v in &self.villagers {
            lanes.push(json!({"k": "v", "id": v.label, "sub": v.first_cart, "class": ch_s(&v.class), "act": ch_s(&v.act), "panic": ch_b(&v.panic), "sleep": ch_b(&v.sleep), "lock": ch_i(&v.lock), "willing": ch_b(&v.willing), "cart": ch_s(&v.cart), "home": ch_s(&v.home), "slept": ch_i(&v.slept), "cant": ch_i(&v.cant), "pos": track(&v.pos)}));
        }
        for c in &self.carts {
            lanes.push(json!({"k": "c", "id": c.label, "rider": ch_s(&c.rider), "pos": track(&c.pos)}));
        }
        for b in &self.beds {
            lanes.push(json!({"k": "b", "id": b.label, "occ": ch_b(&b.occ), "by": ch_s(&b.by)}));
        }
        for s in &self.signals {
            let id = match s.at {
                Some(p) => format!("{} {}", s.name, cell(p, off)),
                None => s.name.clone(),
            };
            let ch = |c: &Ch<String>| c.window(from, to).into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>();
            lanes.push(json!({"k": "s", "id": id, "val": ch(&s.val), "full": ch(&s.full)}));
        }
        for m in &self.marks {
            lanes.push(json!({"k": "m", "id": m.name, "ev": m.ev.iter().filter(|(k, _)| *k >= from && *k <= to).map(|(k, t)| json!([k, t])).collect::<Vec<_>>()}));
        }
        let golems: Vec<serde_json::Value> = self
            .golems
            .iter()
            .filter(|g| g.t <= to && g.gone.is_none_or(|(k, _)| k >= from))
            .map(|g| json!({"id": g.label, "t": g.t, "by": g.by, "at": g.at, "died": g.died.as_ref().map(|(k, h)| json!([k, h])), "gone": g.gone.map(|(k, p)| json!([k, (p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0, (p[2] * 100.0).round() / 100.0]))}))
            .collect();
        let slots: Vec<serde_json::Value> = self
            .slots
            .iter()
            .filter(|s| s.k >= from && s.k <= to)
            .map(|s| {
                json!({"t": s.k, "s": s.summoners.iter().map(|m| json!({"v": m.who, "res": m.result, "g": m.golem, "w": m.willing, "box": m.others.iter().map(|(l, dx, dy, dz, wl, inside)| json!([l, (dx * 100.0).round() / 100.0, (dy * 100.0).round() / 100.0, (dz * 100.0).round() / 100.0, *wl as u8, *inside as u8])).collect::<Vec<_>>()})).collect::<Vec<_>>()})
            })
            .collect();
        let flags: Vec<serde_json::Value> = self.flags.iter().filter(|f| f.k >= from && f.k <= to).map(|f| json!({"t": f.k, "u": f.until, "r": f.rule, "lane": f.lane, "text": f.text})).collect();
        let regions: Vec<serde_json::Value> = self.regions.iter().map(|(n, lo, hi)| json!([n, [lo[0] - off[0], lo[1] - off[1], lo[2] - off[2]], [hi[0] - off[0], hi[1] - off[1], hi[2] - off[2]]])).collect();
        json!({"title": self.title, "seed": self.seed, "t0": self.t0, "day0": self.day0, "ticks": self.ticks, "from": from, "to": to, "summary": self.summary(from, to), "regions": regions, "lanes": lanes, "golems": golems, "slots": slots, "flags": flags})
    }

    /// The page of the window `[from, to]`: one self-contained HTML file.
    pub fn html(&self, from: u32, to: u32) -> String {
        let data = self.json(from, to).to_string().replace("</", "<\\/");
        let title = if self.source.is_some() { format!("{}: timeline", self.title) } else { format!("{} seed {}: timeline", self.title, self.seed) }.replace('<', "&lt;");
        PAGE.replace("__TITLE__", &title).replace("__DATA__", &data)
    }
}

const PAGE: &str = include_str!("timeline.html");

/// Write the page and the text dump of the window into `dir` as `<stem>-seed<S>.timeline.{html,txt}`.
pub fn write(tl: &Timeline, dir: &Path, stem: &str, from: u32, to: u32) -> Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    // (a dump's page: `<stem>.timeline.html` beside nothing else, the dump being its text)
    let base = if tl.source.is_some() { dir.join(format!("{stem}.timeline")) } else { dir.join(format!("{stem}-seed{}.timeline", tl.seed)) };
    let (html, txt) = (base.with_extension("timeline.html"), base.with_extension("timeline.txt"));
    std::fs::write(&html, tl.html(from, to))?;
    if tl.source.is_none() {
        std::fs::write(&txt, tl.text(from, to))?;
    }
    Ok((html, txt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_parse_and_refuse() {
        assert_eq!(Tag::parse("signal counterA 1 -9 5").unwrap(), Tag::Signal { name: "counterA".into(), at: [1, -9, 5] });
        assert_eq!(Tag::parse("region station 4 -2 1 -4 1 -7").unwrap(), Tag::Region { name: "station".into(), lo: [-4, -2, -7], hi: [4, 1, 1] });
        assert!(Tag::parse("signal a 1 2").is_err());
        assert!(Tag::parse("box a 1 2 3").is_err());
        let text = "name: x\ntimeline: signal a 1 2 3\ntimeline: region s 0 0 0 1 1 1\nport output: 1 2 3\n";
        assert_eq!(tags_in(text).unwrap().len(), 2);
    }

    #[test]
    fn signal_values() {
        assert_eq!(signal_value("minecraft:redstone_wire[east=side,north=none,power=7,south=none,west=side]"), "p7");
        assert_eq!(signal_value("minecraft:copper_bulb[lit=true,powered=false]"), "lit");
        assert_eq!(signal_value("minecraft:tripwire_hook[attached=true,facing=east,powered=true]"), "powered");
        assert_eq!(signal_value("minecraft:stone"), "off");
    }

    #[test]
    fn a_window_starts_with_its_value() {
        let mut c: Ch<bool> = Ch::default();
        c.set(0, false);
        c.set(10, true);
        c.set(20, false);
        assert_eq!(c.window(15, 30), vec![(15, true), (20, false)]);
        assert_eq!(c.window(0, 12), vec![(0, false), (10, true)]);
    }

    /// A bench's own trace in the dump's format (the counter's: day-1 2026-11-09) reads back as
    /// signal lanes, mark lanes and flags.
    #[test]
    fn a_dump_reads_back() {
        let text = "# the counter bench\nt=0 gt=1200 signal line1=on (minecraft:redstone_lamp[lit=true])\nt=0 gt=1200 signal line2=off\nt=100 gt=1300 day=0 dt=1300 mark pulse golem\nt=105 gt=1305 signal line1=off\nt=105 gt=1305 signal line2=on\nt=200 gt=1400 mark slot missed\nt=300 gt=1500 VIOLATION skip line2: line3 never went high\n";
        let tl = Timeline::from_dump("bench", "bench.txt", text).unwrap();
        assert_eq!((tl.t0, tl.ticks), (1200, 300));
        assert_eq!(tl.signals.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["line1", "line2"]);
        assert_eq!(tl.signals[0].val.v, vec![(0, "on".to_string()), (105, "off".to_string())]);
        assert_eq!(tl.signals[0].full.v[0].1, "minecraft:redstone_lamp[lit=true]");
        assert_eq!(tl.marks.iter().map(|m| (m.name.as_str(), m.ev.len())).collect::<Vec<_>>(), vec![("pulse", 1), ("slot", 1)]);
        assert_eq!((tl.flags[0].k, tl.flags[0].rule.as_str(), tl.flags[0].lane.as_str(), tl.flags[0].text.as_str()), (300, "skip", "line2", "line3 never went high"));
        assert!(tl.summary(0, 300).contains("2 signal lanes, 2 mark lanes (2 marks), 1 violations"));
        assert!(Timeline::from_dump("x", "x", "t=5 gt=5 mark a\nt=3 gt=3 signal a=b\n").is_err(), "out of tick order");
        assert!(Timeline::from_dump("x", "x", "nothing here\n").is_err());
    }

    /// The timeline is the judge's run tick for tick: on the carousel's first 3000 ticks (its first
    /// golems, sightings and POI releases), its counters are the judge's report's.
    #[test]
    fn the_timeline_is_the_judges_run() {
        use redsim::entity::mob::farm::WalkMode;
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/farms/iron/middle/carousel-gate.txt");
        let c = circuit::Circuit::load(&path).unwrap();
        let spec = circuit::mobfarm::spec_of(&c).unwrap();
        let o = RunOpts::steady(1, 3000, WalkMode::Random);
        let tl = record("carousel-gate", &spec, &o, [0, 0, 0], &[Tag::parse("region station -4 -2 -7 4 1 1").unwrap()], 3000).unwrap();
        let rep = redsim::entity::mob::farm::run(&spec, &o).unwrap();
        let n = |what: &str| tl.counts.iter().filter(|(_, c)| *c == what).count();
        assert!(rep.golems > 0 && rep.losses.seen > 0, "the window shows golems and sightings");
        assert_eq!(tl.golems.len() as u32, rep.golems);
        assert_eq!(n("fail") as u32, rep.losses.no_spot);
        assert_eq!(n("sight") as u32, rep.losses.seen);
        assert_eq!(n("sleep") as u32, rep.sleeps);
        assert_eq!(n("release"), rep.releases.len());
        // every golem summoned on a slot, by a summoner the slot lists
        for g in &tl.golems {
            let s = tl.slots.iter().find(|s| s.k == g.t).expect("a golem's slot");
            assert!(s.summoners.iter().any(|m| m.golem.as_deref() == Some(g.label.as_str())));
        }
    }
}
