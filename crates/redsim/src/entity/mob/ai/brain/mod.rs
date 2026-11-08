//! The exact villager brain (26.3 `Brain`, `Behavior`, `OneShot`, `GateBehavior` / `RunOne`,
//! `ShufflingList`, `VillagerGoalPackages`, the villager's sensors): every behaviour of the CORE,
//! IDLE, WORK, MEET, REST, PANIC and PLAY packages in the brain's slot order, with the level
//! random's draws (`Behavior.tryStart`'s duration, `SetRaidStatus`, `AcquirePoi`...), the
//! villager's own (the random positions of its strolls) and each shuffling list's (seeded as the
//! bridge seeds them: `rng::shuffle_seed`).
//!
//! It runs with the mob AI on; `villager::Brain` keeps the memories the rest of the mob layer reads
//! (`home`, `last_slept`, `last_woken`, `golem_detected`, `nearest_hostile`, the nearest living
//! entities) and holds this module's state in `ex`.

pub mod run;
pub mod sense;
pub mod tree;

use crate::entity::mob::villager::{self, Activity};
use crate::entity::mob::{Damage, mob, mob_mut};
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::rng::LegacyRandom;
use crate::world::World;
use tree::Node;

/// `Activity` (every one the villager registers).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Act {
    Core,
    Idle,
    Work,
    Play,
    Rest,
    Meet,
    Panic,
    Raid,
    PreRaid,
    Hide,
}

impl Act {
    pub fn name(self) -> &'static str {
        match self {
            Act::Core => "core",
            Act::Idle => "idle",
            Act::Work => "work",
            Act::Play => "play",
            Act::Rest => "rest",
            Act::Meet => "meet",
            Act::Panic => "panic",
            Act::Raid => "raid",
            Act::PreRaid => "pre_raid",
            Act::Hide => "hide",
        }
    }

    /// The `villager::Activity` the rest of the mob layer reads.
    pub fn simple(self) -> Activity {
        match self {
            Act::Work => Activity::Work,
            Act::Meet => Activity::Meet,
            Act::Rest => Activity::Rest,
            Act::Panic => Activity::Panic,
            _ => Activity::Idle,
        }
    }
}

/// `PositionTracker`: a block (`BlockPosTracker(BlockPos)`: its centre), a point
/// (`BlockPosTracker(Vec3)`), an entity (`EntityTracker`, at its eye or its feet).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tracker {
    Block(BlockPos),
    Point(Vec3),
    Entity { id: u32, eye: bool },
}

/// `WalkTarget`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WalkTarget {
    pub target: Tracker,
    pub speed: f32,
    pub close: i32,
}

impl WalkTarget {
    /// `new WalkTarget(Vec3, ...)`: the block containing the point.
    pub fn at_point(p: Vec3, speed: f32, close: i32) -> WalkTarget {
        WalkTarget { target: Tracker::Block(containing(p)), speed, close }
    }
}

/// `BlockPos.containing(v)`.
pub fn containing(p: Vec3) -> BlockPos {
    BlockPos::new(crate::entity::mth::floor(p.x), crate::entity::mth::floor(p.y), crate::entity::mth::floor(p.z))
}

/// The memories the exact brain adds to `villager::Brain`'s (an empty list = no value, as
/// `Brain.setMemory` clears a memory set to an empty collection).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Mem {
    pub walk_target: Option<WalkTarget>,
    pub look_target: Option<Tracker>,
    /// `PATH`: a path object (its uid tells whether it is the navigation's).
    pub path: Option<crate::entity::mob::ai::path::Path>,
    pub cant_reach_since: Option<i64>,
    /// `DOORS_TO_CLOSE`: a set once remembered (present even when emptied).
    pub doors_to_close: Option<Vec<BlockPos>>,
    pub interaction_target: Option<u32>,
    pub breed_target: Option<u32>,
    /// `HURT_BY`: the damage source (its type and attacker).
    pub hurt_by: Option<(Damage, Option<u32>)>,
    pub hurt_by_entity: Option<u32>,
    pub nearest_players: Vec<u32>,
    pub nearest_visible_player: Option<u32>,
    pub nearest_visible_attackable_player: Option<u32>,
    pub nearest_visible_attackable_players: Vec<u32>,
    pub nearest_visible_wanted_item: Option<u32>,
    pub nearest_bed: Option<BlockPos>,
    pub visible_villager_babies: Vec<u32>,
    pub secondary_job_site: Vec<BlockPos>,
    pub job_site: Option<BlockPos>,
    pub potential_job_site: Option<BlockPos>,
    pub meeting_point: Option<BlockPos>,
    pub heard_bell_time: Option<i64>,
}

/// The memory types the behaviours name (for entry conditions and erasing).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum M {
    WalkTarget,
    LookTarget,
    Path,
    CantReachSince,
    DoorsToClose,
    InteractionTarget,
    BreedTarget,
    HurtBy,
    HurtByEntity,
    NearestHostile,
    NearestLiving,
    NearestVisibleLiving,
    NearestVisiblePlayer,
    NearestVisibleWantedItem,
    NearestBed,
    VisibleVillagerBabies,
    SecondaryJobSite,
    JobSite,
    PotentialJobSite,
    MeetingPoint,
    Home,
    HeardBellTime,
    LastWoken,
    LastSlept,
    ItemPickupCooldown,
    HidingPlace,
}

/// `MemoryStatus`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Present,
    Absent,
    Registered,
}

/// The exact brain's state.
#[derive(Clone, PartialEq, Debug)]
pub struct Exact {
    pub mem: Mem,
    pub nodes: Vec<Node>,
    /// `availableBehaviorsByPriority` flattened in iteration order: (priority, activity, node).
    pub slots: Vec<(i32, Act, usize)>,
    /// The active non-core activity (CORE is always active).
    pub active: Act,
    pub default_activity: Act,
    pub last_schedule_update: i64,
    /// The brain's generation (a rebuilt brain counts one more: the shuffling lists' keys).
    pub generation: u32,
    pub baby: bool,
    /// `NearestBedSensor`'s state.
    pub bed_sensor_tried: i32,
    pub bed_sensor_last_update: i64,
}

impl Exact {
    /// A new adult (or baby) brain of an unemployed villager whose own random was seeded `seed`:
    /// the packages registered in `BRAIN_PROVIDER`'s order, each shuffling list seeded.
    pub fn new(baby: bool, entity_seed: i64, generation: u32) -> Exact {
        let (nodes, slots) = tree::build(baby, entity_seed, generation);
        Exact {
            mem: Mem::default(),
            nodes,
            slots,
            active: Act::Idle,
            default_activity: Act::Idle,
            last_schedule_update: -9999,
            generation,
            baby,
            bed_sensor_tried: 0,
            bed_sensor_last_update: 0,
        }
    }
}

pub fn brain(w: &World, i: usize) -> &villager::Brain {
    mob(w, i).brain.as_deref().expect("a villager has a brain")
}

pub fn brain_mut(w: &mut World, i: usize) -> &mut villager::Brain {
    mob_mut(w, i).brain.as_deref_mut().expect("a villager has a brain")
}

pub fn ex(w: &World, i: usize) -> &Exact {
    brain(w, i).ex.as_deref().expect("an exact brain")
}

pub fn ex_mut(w: &mut World, i: usize) -> &mut Exact {
    brain_mut(w, i).ex.as_deref_mut().expect("an exact brain")
}

/// Whether villager `i` runs the exact brain.
pub fn has_exact(w: &World, i: usize) -> bool {
    matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.brain.as_deref().is_some_and(|b| b.ex.is_some()))
}

/// The villager constructor's brains as the bridge's seeded summon leaves them: the constructor's
/// brain draws its 9 sensor delays, loading the data builds the live one (9 more), each
/// `registerBrainGoals` updating the activity from the schedule at game time `t`.
pub fn attach(b: &mut villager::Brain, own: &mut LegacyRandom, entity_seed: i64, t: i64, day: i64) {
    for (k, (_, rate)) in villager::SENSORS.iter().enumerate() {
        b.sensors[k] = own.next_int(*rate as i32) as i64;
    }
    let mut x = Exact::new(false, entity_seed, 1);
    // registerBrainGoals: updateActivityFromSchedule (WORK needs a JOB_SITE, MEET a MEETING_POINT:
    // IDLE, the default, instead)
    x.last_schedule_update = t;
    let sched = scheduled(day);
    x.active = if matches!(sched, Act::Work | Act::Meet) { Act::Idle } else { sched };
    b.activity = x.active.simple();
    b.last_schedule_update = t;
    b.ex = Some(Box::new(x));
}

/// The adult villager's scheduled activity at day time `day` (`VILLAGER_ACTIVITY`).
pub fn scheduled(day: i64) -> Act {
    match villager::scheduled(day) {
        Some(Activity::Work) => Act::Work,
        Some(Activity::Meet) => Act::Meet,
        Some(Activity::Rest) => Act::Rest,
        _ => Act::Idle,
    }
}

// --------------------------------------------------------------------------- memories

/// `Brain.checkMemory(type, status)` (every type here is registered).
pub fn check(w: &World, i: usize, m: M, s: Status) -> bool {
    if s == Status::Registered {
        return true;
    }
    let present = has(w, i, m);
    match s {
        Status::Present => present,
        Status::Absent => !present,
        Status::Registered => true,
    }
}

/// `hasMemoryValue(type)`.
pub fn has(w: &World, i: usize, m: M) -> bool {
    let b = brain(w, i);
    let x = ex(w, i);
    let e = &x.mem;
    match m {
        M::WalkTarget => e.walk_target.is_some(),
        M::LookTarget => e.look_target.is_some(),
        M::Path => e.path.is_some(),
        M::CantReachSince => e.cant_reach_since.is_some(),
        M::DoorsToClose => e.doors_to_close.is_some(),
        M::InteractionTarget => e.interaction_target.is_some(),
        M::BreedTarget => e.breed_target.is_some(),
        M::HurtBy => e.hurt_by.is_some(),
        M::HurtByEntity => e.hurt_by_entity.is_some(),
        M::NearestHostile => b.nearest_hostile.is_some(),
        M::NearestLiving => !b.nearest_living.is_empty(),
        M::NearestVisibleLiving => b.visible_present,
        M::NearestVisiblePlayer => e.nearest_visible_player.is_some(),
        M::NearestVisibleWantedItem => e.nearest_visible_wanted_item.is_some(),
        M::NearestBed => e.nearest_bed.is_some(),
        M::VisibleVillagerBabies => !e.visible_villager_babies.is_empty(),
        M::SecondaryJobSite => !e.secondary_job_site.is_empty(),
        M::JobSite => e.job_site.is_some(),
        M::PotentialJobSite => e.potential_job_site.is_some(),
        M::MeetingPoint => e.meeting_point.is_some(),
        M::Home => b.home.is_some(),
        M::HeardBellTime => e.heard_bell_time.is_some(),
        M::LastWoken => b.last_woken.is_some(),
        M::LastSlept => b.last_slept.is_some(),
        M::ItemPickupCooldown | M::HidingPlace => false,
    }
}

/// `eraseMemory(type)`.
pub fn erase(w: &mut World, i: usize, m: M) {
    let b = brain_mut(w, i);
    match m {
        M::NearestHostile => b.nearest_hostile = None,
        M::NearestLiving => b.nearest_living.clear(),
        M::NearestVisibleLiving => {
            b.visible_present = false;
            b.visible = villager::Visible::default();
        }
        M::Home => b.home = None,
        M::LastWoken => b.last_woken = None,
        M::LastSlept => b.last_slept = None,
        _ => {
            let e = &mut b.ex.as_deref_mut().expect("an exact brain").mem;
            match m {
                M::WalkTarget => e.walk_target = None,
                M::LookTarget => e.look_target = None,
                M::Path => e.path = None,
                M::CantReachSince => e.cant_reach_since = None,
                M::DoorsToClose => e.doors_to_close = None,
                M::InteractionTarget => e.interaction_target = None,
                M::BreedTarget => e.breed_target = None,
                M::HurtBy => e.hurt_by = None,
                M::HurtByEntity => e.hurt_by_entity = None,
                M::NearestVisiblePlayer => e.nearest_visible_player = None,
                M::NearestVisibleWantedItem => e.nearest_visible_wanted_item = None,
                M::NearestBed => e.nearest_bed = None,
                M::VisibleVillagerBabies => e.visible_villager_babies.clear(),
                M::SecondaryJobSite => e.secondary_job_site.clear(),
                M::JobSite => e.job_site = None,
                M::PotentialJobSite => e.potential_job_site = None,
                M::MeetingPoint => e.meeting_point = None,
                M::HeardBellTime => e.heard_bell_time = None,
                _ => {}
            }
        }
    }
}

// --------------------------------------------------------------------------- activities

/// `Brain.activityRequirementsAreMet`: WORK needs a JOB_SITE, MEET a MEETING_POINT.
fn requirements_met(w: &World, i: usize, a: Act) -> bool {
    match a {
        Act::Work => has(w, i, M::JobSite),
        Act::Meet => has(w, i, M::MeetingPoint),
        _ => true,
    }
}

/// `Brain.isActive(activity)`.
pub fn is_active(w: &World, i: usize, a: Act) -> bool {
    a == Act::Core || ex(w, i).active == a
}

/// `Brain.setActiveActivity(activity)` (no memories to erase for a villager's activities).
fn set_active(w: &mut World, i: usize, a: Act) {
    if !is_active(w, i, a) {
        ex_mut(w, i).active = a;
        brain_mut(w, i).activity = a.simple();
        let (t, id) = (w.time, w.entities.list[i].id);
        w.mob.events.push(crate::entity::mob::MobEvent::Activity { t, id, activity: a.simple() });
    }
}

/// `Brain.setActiveActivityIfPossible(activity)`.
pub fn set_active_if_possible(w: &mut World, i: usize, a: Act) {
    if requirements_met(w, i, a) {
        set_active(w, i, a);
    } else {
        let d = ex(w, i).default_activity;
        set_active(w, i, d);
    }
}

/// `Brain.updateActivityFromSchedule(attributes, gameTime, pos)`.
pub fn update_activity_from_schedule(w: &mut World, i: usize) {
    let t = w.time;
    if t - ex(w, i).last_schedule_update > 20 {
        ex_mut(w, i).last_schedule_update = t;
        brain_mut(w, i).last_schedule_update = t;
        let a = if ex(w, i).baby { baby_scheduled(w.day_time()) } else { scheduled(w.day_time()) };
        if !is_active(w, i, a) {
            set_active_if_possible(w, i, a);
        }
    }
}

/// `BABY_VILLAGER_ACTIVITY`: 10 IDLE, 3000 PLAY, 6000 IDLE, 10000 PLAY, 12000 REST.
fn baby_scheduled(day: i64) -> Act {
    let t = day.rem_euclid(24000);
    if t < 10 {
        Act::Rest
    } else if t < 3000 {
        Act::Idle
    } else if t < 6000 {
        Act::Play
    } else if t < 10000 {
        Act::Idle
    } else if t < 12000 {
        Act::Play
    } else {
        Act::Rest
    }
}

// --------------------------------------------------------------------------- the tick

/// `Brain.tick`: forget outdated memories, the sensors, start each non-running behaviour of the
/// active activities, tick each running one; then `Villager.customServerAiStep`'s raid roll.
pub fn tick(w: &mut World, i: usize) {
    forget_outdated(w, i);
    sense::tick_sensors(w, i);
    if w.entities.list[i].removed {
        return;
    }
    let t = w.time;
    let n = ex(w, i).slots.len();
    // startEachNonRunningBehavior: each (priority, activity) entry is checked once for being
    // active, then all its behaviours tried
    let mut entry: Option<(i32, Act)> = None;
    let mut entry_active = false;
    for k in 0..n {
        let (prio, act, node) = ex(w, i).slots[k];
        if entry != Some((prio, act)) {
            entry = Some((prio, act));
            entry_active = is_active(w, i, act);
        }
        if !entry_active {
            continue;
        }
        if !ex(w, i).nodes[node].running {
            run::try_start(w, i, node, t);
        }
        if w.entities.list[i].removed {
            return;
        }
    }
    // tickEachRunningBehavior: the running list first, then each one's tickOrStop
    let running: Vec<usize> = ex(w, i).slots.iter().map(|(_, _, node)| *node).filter(|&node| ex(w, i).nodes[node].running).collect();
    for node in running {
        run::tick_or_stop(w, i, node, t);
        if w.entities.list[i].removed {
            return;
        }
    }
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_int(100);
    }
}

/// `MemorySlot.tick` of the expiring memories (`GOLEM_DETECTED_RECENTLY`).
fn forget_outdated(w: &mut World, i: usize) {
    let b = brain_mut(w, i);
    if let Some(ttl) = b.golem_detected {
        b.golem_detected = if ttl <= 0 { None } else { Some(ttl - 1) };
    }
}

// --------------------------------------------------------------------------- the frame

/// The memories with a value as the bridge prints them (`Exact.memoryValue`), by name.
pub fn frame_memories(w: &World, i: usize) -> std::collections::BTreeMap<String, String> {
    let b = brain(w, i);
    let x = ex(w, i);
    let e = &x.mem;
    let mut m = std::collections::BTreeMap::new();
    let r = |id: u32| format!("#{id}");
    let p = |q: BlockPos| format!("{} {} {}", q.x, q.y, q.z);
    let list = |l: &[u32]| format!("[{}]", l.iter().map(|id| r(*id)).collect::<Vec<_>>().join(" "));
    let tracker = |t: &Tracker| match t {
        Tracker::Block(q) => format!("block {}", p(*q)),
        Tracker::Point(v) => format!("block {}", p(containing(*v))),
        Tracker::Entity { id, .. } => format!("entity {}", r(*id)),
    };
    if let Some(h) = b.home {
        m.insert("home".into(), p(h));
    }
    if let Some(t) = b.last_slept {
        m.insert("last_slept".into(), t.to_string());
    }
    if let Some(t) = b.last_woken {
        m.insert("last_woken".into(), t.to_string());
    }
    if let Some(ttl) = b.golem_detected {
        m.insert("golem_detected_recently".into(), format!("true ttl {ttl}"));
    }
    if let Some(id) = b.nearest_hostile {
        m.insert("nearest_hostile".into(), r(id));
    }
    if !b.nearest_living.is_empty() {
        let ids: Vec<u32> = b.nearest_living.iter().map(|(id, _)| *id).collect();
        m.insert("mobs".into(), list(&ids));
    }
    if b.visible_present {
        let ids: Vec<u32> = b.visible.entities.iter().map(|(id, _)| *id).collect();
        m.insert("visible_mobs".into(), list(&ids));
    }
    if let Some(wt) = &e.walk_target {
        m.insert("walk_target".into(), format!("walk {} {} {}", tracker(&wt.target), java_float(wt.speed), wt.close));
    }
    if let Some(t) = &e.look_target {
        m.insert("look_target".into(), tracker(t));
    }
    if let Some(pa) = &e.path {
        let live = mob(w, i).ai.as_deref().and_then(|a| a.nav.path.as_ref()).filter(|n| n.uid == pa.uid);
        let q = live.unwrap_or(pa);
        m.insert("path".into(), format!("path {} {} {} {}/{} {}", q.target.x, q.target.y, q.target.z, q.next_node_index, q.node_count(), if q.reached { "reach" } else { "partial" }));
    }
    if let Some(t) = e.cant_reach_since {
        m.insert("cant_reach_walk_target_since".into(), t.to_string());
    }
    if let Some(d) = &e.doors_to_close {
        // a set: its positions sorted (the bridge's canonical form for sets)
        let mut v: Vec<String> = d.iter().map(|q| p(*q)).collect();
        v.sort();
        m.insert("doors_to_close".into(), format!("[{}]", v.join(" ")));
    }
    if let Some(id) = e.interaction_target {
        m.insert("interaction_target".into(), r(id));
    }
    if let Some(id) = e.breed_target {
        m.insert("breed_target".into(), r(id));
    }
    if let Some((d, by)) = e.hurt_by {
        m.insert("hurt_by".into(), format!("damage {}{}", d.msg_id(), by.map(|id| format!(" {}", r(id))).unwrap_or_default()));
    }
    if let Some(id) = e.hurt_by_entity {
        m.insert("hurt_by_entity".into(), r(id));
    }
    if !e.nearest_players.is_empty() {
        m.insert("nearest_players".into(), list(&e.nearest_players));
    }
    if let Some(id) = e.nearest_visible_player {
        m.insert("nearest_visible_player".into(), r(id));
    }
    if let Some(id) = e.nearest_visible_wanted_item {
        m.insert("nearest_visible_wanted_item".into(), r(id));
    }
    if let Some(q) = e.nearest_bed {
        m.insert("nearest_bed".into(), p(q));
    }
    if !e.visible_villager_babies.is_empty() {
        m.insert("visible_villager_babies".into(), list(&e.visible_villager_babies));
    }
    if !e.secondary_job_site.is_empty() {
        m.insert("secondary_job_site".into(), format!("[{}]", e.secondary_job_site.iter().map(|q| p(*q)).collect::<Vec<_>>().join(" ")));
    }
    if let Some(q) = e.job_site {
        m.insert("job_site".into(), p(q));
    }
    if let Some(q) = e.potential_job_site {
        m.insert("potential_job_site".into(), p(q));
    }
    if let Some(q) = e.meeting_point {
        m.insert("meeting_point".into(), p(q));
    }
    m
}

/// Every memory name the exact brain models (any other the game reports is a gap).
pub const MODELED: &[&str] = &[
    "golem_detected_recently",
    "home",
    "last_slept",
    "last_woken",
    "nearest_hostile",
    "mobs",
    "visible_mobs",
    "walk_target",
    "look_target",
    "path",
    "cant_reach_walk_target_since",
    "interaction_target",
    "breed_target",
    "hurt_by",
    "hurt_by_entity",
    "nearest_players",
    "nearest_visible_player",
    "nearest_visible_wanted_item",
    "nearest_bed",
    "visible_villager_babies",
    "secondary_job_site",
    "job_site",
    "potential_job_site",
    "meeting_point",
    "doors_to_close",
    "heard_bell_time",
];

/// The active activities as the bridge prints them (sorted names).
pub fn frame_activities(w: &World, i: usize) -> String {
    let mut v = vec!["core", ex(w, i).active.name()];
    v.sort();
    v.join(" ")
}

/// The running behaviours' debug strings, sorted.
pub fn frame_behaviors(w: &World, i: usize) -> String {
    let x = ex(w, i);
    let mut v: Vec<String> = x.slots.iter().filter(|(_, _, n)| x.nodes[*n].running).map(|(_, _, n)| tree::debug_string(x, *n)).collect();
    v.sort();
    v.join(" ")
}

/// `Float.toString(f)` for the speeds the brain uses.
pub fn java_float(f: f32) -> String {
    let s = format!("{f:?}");
    if s.contains('.') || s.contains('e') { s } else { format!("{s}.0") }
}
