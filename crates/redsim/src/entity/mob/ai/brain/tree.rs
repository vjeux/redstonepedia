//! The villager brain's behaviours as nodes (26.3 `VillagerGoalPackages`), their slots in the
//! brain's iteration order (`availableBehaviorsByPriority`: priorities ascending, each priority's
//! `HashMap<Activity, ..>` in bucket order, each activity's behaviours as registered) and their
//! debug strings.

use super::{Act, Exact, M, Status};
use crate::entity::mob::types::MobType;
use crate::pos::BlockPos;
use crate::rng::LegacyRandom;

/// A look target's filter (`SetEntityLookTarget.create(type|category, 8)`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LookFilter {
    Cat,
    Villager,
    Player,
    Creature,
    WaterCreature,
    Axolotls,
    UndergroundWaterCreature,
    WaterAmbient,
    Monster,
}

/// What an `InteractWith` looks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractKind {
    Villager,
    BreedVillager,
    Cat,
}

/// Which POIs a behaviour names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoiSel {
    /// The profession's held job site (none for an unemployed villager).
    HeldJob,
    /// Any job site (`ALL_ACQUIRABLE_JOBS`).
    AcquirableJob,
    Home,
    Meeting,
}

/// A behaviour.
#[derive(Clone, PartialEq, Debug)]
pub enum Kind {
    // ---- Behavior subclasses (a duration from the level random when they start)
    Swim,
    LookAtTargetSink,
    VillagerPanicTrigger,
    MoveToTargetSink { min: i32, max: i32, remaining_cooldown: i32, path: Option<crate::entity::mob::ai::path::Path>, last_target: Option<BlockPos>, speed: f32 },
    LookAndFollowTradingPlayerSink,
    GoToPotentialJobSite,
    SleepInBed { next_ok: i64 },
    GiveGiftToHero { time_until_next_gift: i32 },
    ShowTradesToPlayer,
    TradeWithVillager,
    VillagerMakeLove,
    JumpOnBed,
    WorkAtPoi { last_check: i64 },
    HarvestFarmland,
    UseBonemeal,
    CelebrateVillagersSurvivedRaid,
    DoNothing { min: i32, max: i32 },
    // ---- one-shots
    InteractWithDoor { last_checked: Option<BlockPos>, remaining_cooldown: i32 },
    WakeUp,
    ReactToBell,
    SetRaidStatus,
    ValidateNearbyPoi { poi: PoiSel, mem: M },
    PoiCompetitorScan,
    GoToWantedItem,
    AcquirePoi { poi: PoiSel, validate: M, acquire: M, only_adult: bool, next_scheduled: i64, batch: Vec<(BlockPos, Retry)> },
    YieldJobSite,
    AssignProfessionFromJobSite,
    ResetProfession,
    SetWalkTargetFromBlockMemory { mem: M, speed: f32, close: i32, too_far: i32, too_long: i64 },
    UpdateActivityFromSchedule,
    SetEntityLookTarget(LookFilter),
    InteractWith { kind: InteractKind, speed: f32 },
    VillageBoundRandomStroll { speed: f32, xz: i32, y: i32 },
    SetWalkTargetFromLookTarget { speed: f32, close: i32 },
    SetLookAndInteract,
    StrollAroundPoi { mem: M, max_dist: i32, next_ok: i64 },
    StrollToPoi { mem: M, speed: f32, close: i32, max_dist: i32, next_ok: i64 },
    StrollToPoiList { next_ok: i64 },
    SetClosestHomeAsWalkTarget { last_update: i64 },
    InsideBrownianWalk,
    GoToClosestVillage,
    VillagerCalmDown,
    SetWalkTargetAwayFrom { mem: M, speed: f32 },
    PlayTagWithOtherKids,
    SocializeAtBell,
    RingBell,
    ResetRaidStatus,
    RaidSequence,
    LocateHidingPlace,
    SetHiddenState,
    // ---- gates
    Gate { entry: Vec<(M, Status)>, exit_erased: Vec<M>, shuffled: bool, run_one: bool },
    TriggerGate,
}

/// `AcquirePoi.JitteredLinearRetry`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Retry {
    pub previous: i64,
    pub next: i64,
    pub delay: i32,
}

/// A behaviour's node: its kind and state, its status, its end time, a gate's children (node,
/// weight) in the list's current order with their random weights, and its list's own random.
#[derive(Clone, PartialEq, Debug)]
pub struct Node {
    pub kind: Kind,
    pub running: bool,
    pub end: i64,
    pub children: Vec<(usize, i32, f64)>,
    pub shuffle: Option<LegacyRandom>,
}

impl Node {
    fn new(kind: Kind) -> Node {
        Node { kind, running: false, end: 0, children: Vec::new(), shuffle: None }
    }

    /// `Behavior`'s duration bounds (`None`: not a timed behaviour).
    pub fn durations(&self) -> Option<(i32, i32)> {
        Some(match &self.kind {
            Kind::Swim | Kind::VillagerPanicTrigger | Kind::TradeWithVillager | Kind::JumpOnBed | Kind::WorkAtPoi { .. } | Kind::HarvestFarmland | Kind::UseBonemeal | Kind::SleepInBed { .. } => (60, 60),
            Kind::LookAtTargetSink => (45, 90),
            Kind::MoveToTargetSink { min, max, .. } => (*min, *max),
            Kind::LookAndFollowTradingPlayerSink => (i32::MAX, i32::MAX),
            Kind::GoToPotentialJobSite => (1200, 1200),
            Kind::GiveGiftToHero { .. } => (100, 100),
            Kind::ShowTradesToPlayer => (400, 1600),
            Kind::VillagerMakeLove => (350, 350),
            Kind::CelebrateVillagersSurvivedRaid => (600, 600),
            Kind::DoNothing { min, max } => (*min, *max),
            _ => return None,
        })
    }

    /// The class name (`getClass().getSimpleName()`).
    pub fn class_name(&self) -> &'static str {
        match &self.kind {
            Kind::Swim => "Swim",
            Kind::LookAtTargetSink => "LookAtTargetSink",
            Kind::VillagerPanicTrigger => "VillagerPanicTrigger",
            Kind::MoveToTargetSink { .. } => "MoveToTargetSink",
            Kind::LookAndFollowTradingPlayerSink => "LookAndFollowTradingPlayerSink",
            Kind::GoToPotentialJobSite => "GoToPotentialJobSite",
            Kind::SleepInBed { .. } => "SleepInBed",
            Kind::GiveGiftToHero { .. } => "GiveGiftToHero",
            Kind::ShowTradesToPlayer => "ShowTradesToPlayer",
            Kind::TradeWithVillager => "TradeWithVillager",
            Kind::VillagerMakeLove => "VillagerMakeLove",
            Kind::JumpOnBed => "JumpOnBed",
            Kind::WorkAtPoi { .. } => "WorkAtPoi",
            Kind::HarvestFarmland => "HarvestFarmland",
            Kind::UseBonemeal => "UseBonemeal",
            Kind::CelebrateVillagersSurvivedRaid => "CelebrateVillagersSurvivedRaid",
            Kind::DoNothing { .. } => "DoNothing",
            Kind::Gate { shuffled: true, run_one: true, .. } => "RunOne",
            Kind::Gate { .. } => "GateBehavior",
            _ => "OneShot",
        }
    }
}

/// A behaviour's debug string as the bridge prints a running one (`debugString()`): a gate's is
/// `"<Class>: [<running children's classes>]"` (a `HashSet<String>`'s order).
pub fn debug_string(x: &Exact, n: usize) -> String {
    let node = &x.nodes[n];
    match &node.kind {
        Kind::Gate { .. } => {
            let mut names: Vec<&'static str> = Vec::new();
            for &(c, _, _) in &node.children {
                if x.nodes[c].running {
                    let s = x.nodes[c].class_name();
                    if !names.contains(&s) {
                        names.push(s);
                    }
                }
            }
            names.sort_by_key(|s| hash_set_order_key(s));
            format!("{}: [{}]", node.class_name(), names.join(", "))
        }
        _ => node.class_name().to_string(),
    }
}

/// A string's place in a small `HashSet<String>` (16 buckets: the spread hash's low 4 bits).
fn hash_set_order_key(s: &str) -> u32 {
    let h = crate::rng::java_string_hash(s) as u32;
    (h ^ (h >> 16)) & 15
}

// --------------------------------------------------------------------------- the packages

struct Builder {
    nodes: Vec<Node>,
}

impl Builder {
    fn add(&mut self, kind: Kind) -> usize {
        self.nodes.push(Node::new(kind));
        self.nodes.len() - 1
    }

    fn gate(&mut self, entry: Vec<(M, Status)>, exit_erased: Vec<M>, shuffled: bool, run_one: bool, children: Vec<(Kind, i32)>) -> usize {
        let kids: Vec<(usize, i32, f64)> = children.into_iter().map(|(k, wt)| (self.add(k), wt, 0.0)).collect();
        let g = self.add(Kind::Gate { entry, exit_erased, shuffled, run_one });
        self.nodes[g].children = kids;
        g
    }

    fn run_one(&mut self, entry: Vec<(M, Status)>, children: Vec<(Kind, i32)>) -> usize {
        self.gate(entry, Vec::new(), true, true, children)
    }

    fn trigger_gate(&mut self, children: Vec<(Kind, i32)>) -> usize {
        let kids: Vec<(usize, i32, f64)> = children.into_iter().map(|(k, wt)| (self.add(k), wt, 0.0)).collect();
        let g = self.add(Kind::TriggerGate);
        self.nodes[g].children = kids;
        g
    }

    fn minimal_look(&mut self) -> usize {
        self.run_one(Vec::new(), vec![(Kind::SetEntityLookTarget(LookFilter::Villager), 2), (Kind::SetEntityLookTarget(LookFilter::Player), 2), (Kind::DoNothing { min: 30, max: 60 }, 8)])
    }

    fn full_look(&mut self) -> usize {
        self.run_one(
            Vec::new(),
            vec![
                (Kind::SetEntityLookTarget(LookFilter::Cat), 8),
                (Kind::SetEntityLookTarget(LookFilter::Villager), 2),
                (Kind::SetEntityLookTarget(LookFilter::Player), 2),
                (Kind::SetEntityLookTarget(LookFilter::Creature), 1),
                (Kind::SetEntityLookTarget(LookFilter::WaterCreature), 1),
                (Kind::SetEntityLookTarget(LookFilter::Axolotls), 1),
                (Kind::SetEntityLookTarget(LookFilter::UndergroundWaterCreature), 1),
                (Kind::SetEntityLookTarget(LookFilter::WaterAmbient), 1),
                (Kind::SetEntityLookTarget(LookFilter::Monster), 1),
                (Kind::DoNothing { min: 30, max: 60 }, 2),
            ],
        )
    }
}

fn acquire(poi: PoiSel, validate: M, acquire: M, only_adult: bool) -> Kind {
    Kind::AcquirePoi { poi, validate, acquire, only_adult, next_scheduled: 0, batch: Vec::new() }
}

/// The packages of an unemployed villager (`BRAIN_PROVIDER`'s order: WORK or PLAY, CORE, MEET,
/// REST, IDLE, PANIC, PRE_RAID, RAID, HIDE), as (activity, [(priority, node)]).
fn packages(b: &mut Builder, baby: bool) -> Vec<(Act, Vec<(i32, usize)>)> {
    let s = 0.5f32;
    let mut out = Vec::new();
    if baby {
        let mut v = vec![(0, b.add(Kind::MoveToTargetSink { min: 80, max: 120, remaining_cooldown: 0, path: None, last_target: None, speed: 0.0 }))];
        v.push((5, b.full_look()));
        v.push((5, b.add(Kind::PlayTagWithOtherKids)));
        let r = b.run_one(
            vec![(M::VisibleVillagerBabies, Status::Absent)],
            vec![
                (Kind::InteractWith { kind: InteractKind::Villager, speed: s }, 2),
                (Kind::InteractWith { kind: InteractKind::Cat, speed: s }, 1),
                (Kind::VillageBoundRandomStroll { speed: s, xz: 10, y: 7 }, 1),
                (Kind::SetWalkTargetFromLookTarget { speed: s, close: 2 }, 1),
                (Kind::JumpOnBed, 2),
                (Kind::DoNothing { min: 20, max: 40 }, 2),
            ],
        );
        v.push((5, r));
        v.push((99, b.add(Kind::UpdateActivityFromSchedule)));
        out.push((Act::Play, v));
    } else {
        let mut v = vec![(5, b.minimal_look())];
        let r = b.run_one(
            Vec::new(),
            vec![
                (Kind::WorkAtPoi { last_check: 0 }, 7),
                (Kind::StrollAroundPoi { mem: M::JobSite, max_dist: 4, next_ok: 0 }, 2),
                (Kind::StrollToPoi { mem: M::JobSite, speed: 0.4, close: 1, max_dist: 10, next_ok: 0 }, 5),
                (Kind::StrollToPoiList { next_ok: 0 }, 5),
                (Kind::HarvestFarmland, 5),
                (Kind::UseBonemeal, 7),
            ],
        );
        v.push((5, r));
        v.push((10, b.add(Kind::ShowTradesToPlayer)));
        v.push((10, b.add(Kind::SetLookAndInteract)));
        v.push((2, b.add(Kind::SetWalkTargetFromBlockMemory { mem: M::JobSite, speed: s, close: 9, too_far: 100, too_long: 1200 })));
        v.push((3, b.add(Kind::GiveGiftToHero { time_until_next_gift: 600 })));
        v.push((99, b.add(Kind::UpdateActivityFromSchedule)));
        out.push((Act::Work, v));
    }
    // CORE
    {
        let v = vec![
            (0, b.add(Kind::Swim)),
            (0, b.add(Kind::InteractWithDoor { last_checked: None, remaining_cooldown: 0 })),
            (0, b.add(Kind::LookAtTargetSink)),
            (0, b.add(Kind::VillagerPanicTrigger)),
            (0, b.add(Kind::WakeUp)),
            (0, b.add(Kind::ReactToBell)),
            (0, b.add(Kind::SetRaidStatus)),
            (0, b.add(Kind::ValidateNearbyPoi { poi: PoiSel::HeldJob, mem: M::JobSite })),
            (0, b.add(Kind::ValidateNearbyPoi { poi: PoiSel::AcquirableJob, mem: M::PotentialJobSite })),
            (1, b.add(Kind::MoveToTargetSink { min: 150, max: 250, remaining_cooldown: 0, path: None, last_target: None, speed: 0.0 })),
            (2, b.add(Kind::PoiCompetitorScan)),
            (3, b.add(Kind::LookAndFollowTradingPlayerSink)),
            (5, b.add(Kind::GoToWantedItem)),
            (6, b.add(acquire(PoiSel::AcquirableJob, M::JobSite, M::PotentialJobSite, true))),
            (7, b.add(Kind::GoToPotentialJobSite)),
            (8, b.add(Kind::YieldJobSite)),
            (10, b.add(acquire(PoiSel::Home, M::Home, M::Home, false))),
            (10, b.add(acquire(PoiSel::Meeting, M::MeetingPoint, M::MeetingPoint, true))),
            (10, b.add(Kind::AssignProfessionFromJobSite)),
            (10, b.add(Kind::ResetProfession)),
        ];
        out.push((Act::Core, v));
    }
    // MEET
    {
        let tg = b.trigger_gate(vec![(Kind::StrollAroundPoi { mem: M::MeetingPoint, max_dist: 40, next_ok: 0 }, 2), (Kind::SocializeAtBell, 2)]);
        let mut v = vec![(2, tg)];
        v.push((10, b.add(Kind::ShowTradesToPlayer)));
        v.push((10, b.add(Kind::SetLookAndInteract)));
        v.push((2, b.add(Kind::SetWalkTargetFromBlockMemory { mem: M::MeetingPoint, speed: s, close: 6, too_far: 100, too_long: 200 })));
        v.push((3, b.add(Kind::GiveGiftToHero { time_until_next_gift: 600 })));
        v.push((3, b.add(Kind::ValidateNearbyPoi { poi: PoiSel::Meeting, mem: M::MeetingPoint })));
        let g = b.gate(Vec::new(), vec![M::InteractionTarget], false, true, vec![(Kind::TradeWithVillager, 1)]);
        v.push((3, g));
        v.push((5, b.full_look()));
        v.push((99, b.add(Kind::UpdateActivityFromSchedule)));
        out.push((Act::Meet, v));
    }
    // REST
    {
        let mut v = vec![(2, b.add(Kind::SetWalkTargetFromBlockMemory { mem: M::Home, speed: s, close: 1, too_far: 150, too_long: 1200 }))];
        v.push((3, b.add(Kind::ValidateNearbyPoi { poi: PoiSel::Home, mem: M::Home })));
        v.push((3, b.add(Kind::SleepInBed { next_ok: 0 })));
        let r = b.run_one(
            vec![(M::Home, Status::Absent)],
            vec![(Kind::SetClosestHomeAsWalkTarget { last_update: 0 }, 1), (Kind::InsideBrownianWalk, 4), (Kind::GoToClosestVillage, 2), (Kind::DoNothing { min: 20, max: 40 }, 2)],
        );
        v.push((5, r));
        v.push((5, b.minimal_look()));
        v.push((99, b.add(Kind::UpdateActivityFromSchedule)));
        out.push((Act::Rest, v));
    }
    // IDLE
    {
        let r = b.run_one(
            Vec::new(),
            vec![
                (Kind::InteractWith { kind: InteractKind::Villager, speed: s }, 2),
                (Kind::InteractWith { kind: InteractKind::BreedVillager, speed: s }, 1),
                (Kind::InteractWith { kind: InteractKind::Cat, speed: s }, 1),
                (Kind::VillageBoundRandomStroll { speed: s, xz: 10, y: 7 }, 1),
                (Kind::SetWalkTargetFromLookTarget { speed: s, close: 2 }, 1),
                (Kind::JumpOnBed, 1),
                (Kind::DoNothing { min: 30, max: 60 }, 1),
            ],
        );
        let mut v = vec![(2, r)];
        v.push((3, b.add(Kind::GiveGiftToHero { time_until_next_gift: 600 })));
        v.push((3, b.add(Kind::SetLookAndInteract)));
        v.push((3, b.add(Kind::ShowTradesToPlayer)));
        let g1 = b.gate(Vec::new(), vec![M::InteractionTarget], false, true, vec![(Kind::TradeWithVillager, 1)]);
        v.push((3, g1));
        let g2 = b.gate(Vec::new(), vec![M::BreedTarget], false, true, vec![(Kind::VillagerMakeLove, 1)]);
        v.push((3, g2));
        v.push((5, b.full_look()));
        v.push((99, b.add(Kind::UpdateActivityFromSchedule)));
        out.push((Act::Idle, v));
    }
    // PANIC
    {
        let run = s * 1.5f32;
        let v = vec![
            (0, b.add(Kind::VillagerCalmDown)),
            (1, b.add(Kind::SetWalkTargetAwayFrom { mem: M::NearestHostile, speed: run })),
            (1, b.add(Kind::SetWalkTargetAwayFrom { mem: M::HurtByEntity, speed: run })),
            (3, b.add(Kind::VillageBoundRandomStroll { speed: run, xz: 2, y: 2 })),
            (5, b.minimal_look()),
        ];
        out.push((Act::Panic, v));
    }
    // PRE_RAID, RAID, HIDE (a raid or a bell: refused when they would act)
    {
        let tg = b.trigger_gate(vec![(Kind::SetWalkTargetFromBlockMemory { mem: M::MeetingPoint, speed: s * 1.5, close: 2, too_far: 150, too_long: 200 }, 6), (Kind::VillageBoundRandomStroll { speed: s * 1.5, xz: 10, y: 7 }, 2)]);
        let v = vec![(0, b.add(Kind::RingBell)), (0, tg), (5, b.minimal_look()), (99, b.add(Kind::ResetRaidStatus))];
        out.push((Act::PreRaid, v));
        let rs = b.trigger_gate(vec![(Kind::RaidSequence, 5), (Kind::VillageBoundRandomStroll { speed: s * 1.1, xz: 10, y: 7 }, 2)]);
        let v = vec![(0, rs), (0, b.add(Kind::CelebrateVillagersSurvivedRaid)), (2, b.add(Kind::LocateHidingPlace)), (5, b.minimal_look()), (99, b.add(Kind::ResetRaidStatus))];
        out.push((Act::Raid, v));
        let v = vec![(0, b.add(Kind::SetHiddenState)), (1, b.add(Kind::LocateHidingPlace)), (5, b.minimal_look())];
        out.push((Act::Hide, v));
    }
    out
}

/// A Java `HashMap`'s bucket of an activity among 16 (`hash ^ hash >>> 16`, low 4 bits).
fn bucket(a: Act) -> u32 {
    let h = crate::rng::java_string_hash(a.name()) as u32;
    (h ^ (h >> 16)) & 15
}

/// The nodes and the slots in iteration order, each gate's list seeded
/// (`shuffle_seed(entity_seed, "<generation>/<activity>/<priority>/<index>")`, nested gates
/// `"/<k>"`, a trigger gate's list `"#0"`).
pub fn build(baby: bool, entity_seed: i64, generation: u32) -> (Vec<Node>, Vec<(i32, Act, usize)>) {
    let mut b = Builder { nodes: Vec::new() };
    let pk = packages(&mut b, baby);
    // priority -> activities in insertion order -> behaviours
    let mut prios: std::collections::BTreeMap<i32, Vec<(Act, Vec<usize>)>> = std::collections::BTreeMap::new();
    for (act, list) in &pk {
        for &(p, n) in list {
            let e = prios.entry(p).or_default();
            match e.iter_mut().find(|(a, _)| a == act) {
                Some((_, v)) => v.push(n),
                None => e.push((*act, vec![n])),
            }
        }
    }
    let mut slots = Vec::new();
    for (p, acts) in &prios {
        let mut ordered: Vec<(usize, &(Act, Vec<usize>))> = acts.iter().enumerate().collect();
        ordered.sort_by_key(|(k, (a, _))| (bucket(*a), *k));
        for (_, (a, v)) in ordered {
            for (index, &n) in v.iter().enumerate() {
                slots.push((*p, *a, n));
                let key = format!("{generation}/{}/{p}/{index}", a.name());
                seed_lists(&mut b.nodes, n, &key, entity_seed);
            }
        }
    }
    (b.nodes, slots)
}

fn seed_lists(nodes: &mut [Node], n: usize, key: &str, entity_seed: i64) {
    match nodes[n].kind {
        Kind::Gate { .. } => {
            nodes[n].shuffle = Some(LegacyRandom::new(crate::rng::shuffle_seed(entity_seed, key)));
            let kids: Vec<usize> = nodes[n].children.iter().map(|c| c.0).collect();
            for (k, c) in kids.into_iter().enumerate() {
                seed_lists(nodes, c, &format!("{key}/{k}"), entity_seed);
            }
        }
        Kind::TriggerGate => {
            nodes[n].shuffle = Some(LegacyRandom::new(crate::rng::shuffle_seed(entity_seed, &format!("{key}#0"))));
        }
        _ => {}
    }
}

/// The modeled mob types a look filter accepts (`MobCategory` from the terrain lane's table).
pub fn look_accepts(f: LookFilter, ty: Option<MobType>) -> bool {
    use crate::entity::mob::natural::{Category, category_of};
    match (f, ty) {
        (LookFilter::Player, None) => true,
        (LookFilter::Villager, Some(MobType::Villager)) => true,
        (LookFilter::Monster, Some(t)) => category_of(t) == Category::Monster,
        (LookFilter::Creature, Some(t)) => category_of(t) == Category::Creature,
        (LookFilter::WaterCreature, Some(t)) => category_of(t) == Category::WaterCreature,
        (LookFilter::Axolotls, Some(t)) => category_of(t) == Category::Axolotls,
        (LookFilter::UndergroundWaterCreature, Some(t)) => category_of(t) == Category::UndergroundWaterCreature,
        (LookFilter::WaterAmbient, Some(t)) => category_of(t) == Category::WaterAmbient,
        (LookFilter::Cat, Some(t)) => t.def().id == "cat",
        _ => false,
    }
}
