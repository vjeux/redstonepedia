//! `mctas mobai fixture`: a mob-AI scene run in the game (a bridge sandbox, every random seeded
//! from the sandbox seed: bridge `Exact.java`) and in redsim (the mob AI on, the same streams),
//! compared tick for tick with `redsim::exact` (track tickexact's frames: position, motion,
//! rotations, health, the own random's state, the head and body yaw, the running goals, the path
//! and the target of every mob).
//!
//! A scene is a few blocks (raw, in order), the mobs (a seeded summon with their data), the
//! difficulty, the game rules and the day time; nothing else happens in the sandbox (natural
//! spawning and the weather off).

use anyhow::{Result, anyhow, bail};
use mctas::sim::{PlaceMode, SandboxOptions, Worker, WorkerConfig};
use redsim::block::Block;
use redsim::entity::EntityKind;
use redsim::entity::mob::natural::Difficulty;
use redsim::exact::{Compare, Watch};
use redsim::pos::BlockPos;
use redsim::world::World;
use serde_json::json;

/// A scene.
#[derive(Clone, Debug)]
pub struct Scene {
    pub name: String,
    /// Chunk box `[min_cx, min_cz, max_cx, max_cz]`.
    pub chunks: [i32; 4],
    /// Blocks (game state strings), placed raw in this order.
    pub blocks: Vec<([i32; 3], String)>,
    /// Mobs: type, feet, data.
    pub mobs: Vec<(String, [f64; 3], Option<String>)>,
    pub day_time: i64,
    pub difficulty: Difficulty,
    pub mob_griefing: bool,
    /// The watched box (block changes), inclusive.
    pub watch: ([i32; 3], [i32; 3]),
    /// A survival player hanging at this position (the bridge's player: no gravity, no moves).
    pub player: Option<[f64; 3]>,
    /// The player in creative mode (no mob targets it; it still keeps mobs from idling: the
    /// despawn check's 32 blocks).
    pub player_creative: bool,
    /// `gamerule random_tick_speed` (3; 0 in a scene with grass blocks, whose random ticks need
    /// the terrain worlds' light).
    pub random_tick_speed: u32,
}

/// (tickexact 2026-11-10) A scene with monsters spawning on (`spawn_mobs`, `spawn_monsters`: a
/// nether portal's zombified piglins need `isSpawningMonsters`); every other scene has them off.
fn scene_spawns(name: &str) -> bool {
    matches!(name, "portal-piglin")
}

fn difficulty_name(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Peaceful => "peaceful",
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}

/// (tickexact 2026-11-11, farmopt's hold H: its release width and its shut on a passing cart)
/// scene gate-hold-w's lanes: (pressed carts, ridden, H's open ticks). The first 8: a pressed train
/// of 3 released for W = 1, 3, 5, 7 gt, ridden and empty; then one pressed cart, ridden and empty,
/// with H shut 1..10 gt after it opens.
fn gate_hold_lanes() -> Vec<(usize, bool, u32)> {
    let mut v = Vec::new();
    for ridden in [true, false] {
        for w in [1, 3, 5, 7] {
            v.push((3, ridden, w));
        }
    }
    for ridden in [true, false] {
        for s in 1..=10 {
            v.push((1, ridden, s));
        }
    }
    v
}

/// (tickexact 2026-11-11, ironspec's junction-settle) scene junction-settle's lanes: (J, the missing
/// arm (0 N, 1 E, 2 S, 3 W), the settle (0 unpowered; 1 placed, then powered by a repeater (its
/// input a redstone block at t3: the repeater's own update re-settles it, as router S's does); 2
/// placed onto power (a redstone block under it from t0); 3 placed, then a redstone block put under
/// it at t3 (Level.setBlock updates the neighbours with the OLD block, stone, not a signal source:
/// RailBlock.updateState does not re-settle)), the carts: (entry arm, stored speed, ridden, the
/// follower's gap in gt or 0), router S's arms (its west arm turning south as built)). The generic
/// lanes: each missing arm, each settle, one empty cart at 0.4 from each arm; router S (missing E,
/// entered from the south): unpowered and repeater-powered after placement, stored 0.3, 0.6, 1.0,
/// ridden and empty, a follower 11 or 12 gt behind.
fn junction_lanes() -> Vec<([i32; 3], u8, u8, (u8, f64, bool, u32), bool)> {
    let mut v = Vec::new();
    let mut i = 0;
    let at = |i: i32| [6 + 12 * (i % 10), 64, 6 + 16 * (i / 10)];
    for missing in 0..4u8 {
        for settle in 0..4u8 {
            for entry in (0..4u8).filter(|a| *a != missing) {
                v.push((at(i), missing, settle, (entry, 0.4, false, 0), false));
                i += 1;
            }
        }
    }
    for settle in [0u8, 1] {
        for sp in [0.3, 0.6, 1.0] {
            for ridden in [true, false] {
                for gap in [11u32, 12] {
                    // (router S's lanes below the generic ones, 28 apart in z: its exits 12 long, so
                    // no leader comes back off a stop while its follower passes)
                    let k = v.len() as i32 - 48;
                    v.push(([6 + 12 * (k % 10), 64, 110 + 28 * (k / 10)], 1, settle, (2, sp, ridden, gap), true));
                    i += 1;
                }
            }
        }
    }
    v
}

/// The unit step of arm `a` (0 N, 1 E, 2 S, 3 W).
fn arm_step(a: u8) -> [i32; 2] {
    [[0, -1], [1, 0], [0, 1], [-1, 0]][a as usize]
}

/// (tickexact 2026-11-11, farmopt's and ironspec's H rest-place contacts) scene h-contact's lanes:
/// (x, dz, flat cells before the slope, the waiting carts (z, ridden), the arrival (stored speed,
/// ridden), label). Each lane a closed gate at (x,64,3+dz), `flat` flat rails (0 sloped, 1 the
/// hybrid: place 1 on a flat cell against the gate, place 2 on the last slope; 2 flat: places 1 and
/// 2 flat), 3 slope rails rising south, then flat track; the waiting carts at H's rest places (place
/// 1 against the gate z 4.115, place 2 0.79 behind a ridden front (its rider's box) or 0.98 behind
/// an empty one, as scene queue-settle found them; on the slope y = 64.0625 + its height), the
/// arrival set off down the slope from the top flat at 1.5 past its start. Waiting: place 1 ridden
/// or empty; place 2 behind a ridden front ridden or empty, behind an empty front ridden. Arrivals
/// ridden and empty at stored 0.2, 0.3, 0.4, 0.45, 0.5, 0.633, 0.8, 1.0, 1.12, 1.5, 2.014; and
/// farmopt's (b) on the sloped approach. Scene h-contact-farmopt: the same pairs and geometries at
/// farmopt's measured H arrival speeds (7b90c3f1's "meter H arrivals"): 0.01, 0.03, 0.06, 0.10,
/// 0.13, 0.82, 0.89, 1.05, 1.08, 1.40. Scene h-contact-sweep (ironspec's threshold sweep): the
/// sloped geometry, place 1 empty, place 2 ridden front + empty 2nd and ridden + ridden (the
/// control), ridden and empty arrivals at 0.2, 0.25, 0.3, 0.35, each in 16 copies whose arrival
/// starts 1/16 block apart (the contact's phase) at its own place in the world (its entity order).
/// The last field: the arrival's start offset back from 1.5 past the top flat's start.
#[allow(clippy::type_complexity)]
fn hcontact_lanes(name: &str) -> Vec<(i32, i32, i32, Vec<(f64, bool)>, (f64, bool), String, f64)> {
    let speeds: &[f64] = match name {
        "h-contact-farmopt" => &[0.01, 0.03, 0.06, 0.10, 0.13, 0.82, 0.89, 1.05, 1.08, 1.40],
        "h-contact-sweep" => &[0.2, 0.25, 0.3, 0.35],
        _ => &[0.2, 0.3, 0.4, 0.45, 0.5, 0.633, 0.8, 1.0, 1.12, 1.5, 2.014],
    };
    let copies = if name == "h-contact-sweep" { 16 } else { 1 };
    let mut v = Vec::new();
    let (p1, p2r, p2e) = (4.1150, 4.9050, 5.0950);
    let mut add = |flat: i32, waits: Vec<(f64, bool)>, arrive_ridden: bool, label: &str| {
        for &sp in speeds {
            for c in 0..copies {
                let k = v.len() as i32;
                v.push((1 + 2 * (k % 26), 12 * (k / 26), flat, waits.clone(), (sp, arrive_ridden), label.to_string(), c as f64 / 16.0));
            }
        }
    };
    if name == "h-contact-sweep" {
        for ar in [true, false] {
            let a = if ar { "R" } else { "E" };
            add(0, vec![(p1, false)], ar, &format!("sloped place 1: E waiting, {a} arriving"));
            add(0, vec![(p1, true), (p2r, false)], ar, &format!("sloped place 2: R front, E waiting, {a} arriving"));
            add(0, vec![(p1, true), (p2r, true)], ar, &format!("sloped place 2: R front, R waiting, {a} arriving"));
        }
        return v;
    }
    for flat in [0, 1, 2] {
        let f = ["sloped", "hybrid", "flat"][flat as usize];
        for ar in [true, false] {
            let a = if ar { "R" } else { "E" };
            add(flat, vec![(p1, true)], ar, &format!("{f} place 1: R waiting, {a} arriving"));
            add(flat, vec![(p1, false)], ar, &format!("{f} place 1: E waiting, {a} arriving"));
            add(flat, vec![(p1, true), (p2r, true)], ar, &format!("{f} place 2: R front, R waiting, {a} arriving"));
            add(flat, vec![(p1, true), (p2r, false)], ar, &format!("{f} place 2: R front, E waiting, {a} arriving"));
            add(flat, vec![(p1, false), (p2e, true)], ar, &format!("{f} place 2: E front, R waiting, {a} arriving"));
        }
    }
    // (farmopt's (b): the 2nd cart pushed back by the front rider's jiggle, its gap to the arrival
    // under the wall when the arrival comes: the 2nd 0.1 and 0.2 behind its rest)
    if name == "h-contact" {
        for d in [0.1, 0.2] {
            add(0, vec![(p1, true), (p2r + d, true)], true, &format!("sloped place 2: R front, R waiting {d} back, R arriving"));
        }
    }
    v
}

/// (tickexact 2026-11-11, the re-path of a napper off its bed: ironspec's targeted proof) scene
/// repath-nap's lanes: (x0, the change: 0 none, 1 two cells set from air to stone in one tick 14 gt
/// before the drop, 2 the same two cells swapped stone -> redstone_block (no collision change), 3
/// the two cells from air 25 gt before the drop, 4 the two cells from air 30 gt into the nap (the
/// sleeper holding no path and no delayed recompute)), 4 copies of each (the flee target is a random
/// draw); lanes 20 apart, so no lane's cells come within a neighbour's path range.
fn repath_lanes() -> Vec<(i32, u8)> {
    (0..20).map(|k| (2 + 20 * k, (k / 4) as u8)).collect()
}

/// The tick scene repath-nap's riders lie down (all lanes alike).
const REPATH_DROP: u32 = 135;

/// (tickexact 2026-11-11, farmopt's H retry on a frozen cart) scene gate-frozen's lanes: how deep the
/// frozen cart's box is in the gate's (z 7.375..7.625), whether a retry opens and shuts the gate
/// during the freeze, and whether the cart is ridden.
const GATE_FROZEN_LANES: [(f64, bool, bool); 8] = [(0.06, false, true), (0.06, true, true), (0.2, false, true), (0.2, true, true), (0.5, false, true), (0.5, true, true), (0.06, false, false), (0.06, true, false)];

/// A scene's scripted block changes: after N ticks, these blocks set like a player's (flags 3:
/// neighbor and shape updates, `onPlace`), on both sides between the same two ticks.
pub fn scene_actions(name: &str) -> Vec<(u32, Vec<([i32; 3], String)>)> {
    let s = |p: [i32; 3], b: &str| (p, b.to_string());
    if name == "carousel-loop" {
        return carousel_loop_actions();
    }
    if let Some(v) = drive_actions(name) {
        return v;
    }
    if name == "plant-supports" {
        return plant_supports().1;
    }
    match name {
        // (tickexact 2026-11-11, the re-path proof) the two cells west of each lane's corridor (x0-1,
        // 64, 33 and 35: near the middle of a rider's flee path, from the line near the bed back to
        // its flee target behind the corridor's west wall): set from air in one tick 14 gt before
        // the drop (change 1) or 25 gt before (change 3), or swapped stone -> redstone_block 14 gt
        // before (change 2)
        "repath-nap" => {
            let lanes = repath_lanes();
            let cells = |x0: i32| [[x0 - 1, 64, 33], [x0 - 1, 64, 35]];
            let at = |c: u8, st: &str| -> Vec<([i32; 3], String)> { lanes.iter().filter(|(_, k)| *k == c).flat_map(|(x0, _)| cells(*x0).map(|p| (p, st.to_string()))).collect() };
            let mut v = vec![(REPATH_DROP - 25, at(3, "minecraft:stone"))];
            let mut t14 = at(1, "minecraft:stone");
            t14.extend(at(2, "minecraft:redstone_block"));
            v.push((REPATH_DROP - 14, t14));
            v.push((REPATH_DROP + 30, at(4, "minecraft:stone")));
            v
        }
        // (tickexact 2026-11-11, ironspec's junction-settle) every lane's junction placed at t1 (like a
        // player: onPlace settles it), the placed-then-powered lanes' floor under it a redstone
        // block at t3 (the first power update after placement)
        "junction-settle" => {
            let lanes = junction_lanes();
            vec![
                (1, lanes.iter().map(|(j, ..)| (*j, "minecraft:rail[shape=north_south,waterlogged=false]".to_string())).collect()),
                (
                    3,
                    lanes
                        .iter()
                        .filter(|(_, _, s, ..)| *s == 1 || *s == 3)
                        .map(|(j, m, s, ..)| {
                            if *s == 3 {
                                ([j[0], j[1] - 1, j[2]], "minecraft:redstone_block".to_string())
                            } else {
                                let d = arm_step(*m);
                                ([j[0] + 2 * d[0], j[1] - 1, j[2] + 2 * d[1]], "minecraft:redstone_block".to_string())
                            }
                        })
                        .collect(),
                ),
            ]
        }
        // (tickexact 2026-11-11, ironspec's curve spacing) the from-rest pairs' powered rails powered
        // at t5 (redstone blocks for the floor under them: lanes x0 49 and 61 of row B)
        "curve-pair" => vec![(1, [49, 61].iter().flat_map(|x| [([*x, 63, 32], "minecraft:redstone_block".to_string()), ([*x, 63, 33], "minecraft:redstone_block".to_string())]).collect())],
        // (tickexact 2026-11-11, ironspec's hatch sight lines) every lane's hatch shut at t300 (a
        // top-half trapdoor closed: the sight lines should be clear) and opened again at t600
        "siding-hatch" => {
            let hatch = |open: bool| format!("minecraft:iron_trapdoor[facing=west,half=top,open={open},powered=false,waterlogged=false]");
            vec![(300, (0..3).map(|l| (siding_hatch_at(l), hatch(false))).collect()), (600, (0..3).map(|l| (siding_hatch_at(l), hatch(true))).collect())]
        }
        // (tickexact 2026-11-11, farmopt's hold H) every lane's H opened at t30, shut its lane's
        // ticks later
        "gate-hold-w" => {
            let gate = |open: bool| format!("minecraft:oak_fence_gate[facing=north,in_wall=false,open={open},powered=false]");
            let lanes = gate_hold_lanes();
            let mut v: Vec<(u32, Vec<([i32; 3], String)>)> = vec![(30, (0..lanes.len()).map(|k| ([1 + 2 * k as i32, 64, 7], gate(true))).collect())];
            for (k, (_, _, w)) in lanes.iter().enumerate() {
                let t = 30 + w;
                let p = ([1 + 2 * k as i32, 64, 7], gate(false));
                match v.iter_mut().find(|(tt, _)| *tt == t) {
                    Some((_, b)) => b.push(p),
                    None => v.push((t, vec![p])),
                }
            }
            v.sort_by_key(|(t, _)| *t);
            v
        }
        // (tickexact 2026-11-11, ironspec's frozen-rider sensors) chunk (1,0) frozen at level 32 from
        // t40 to t240; at t260 the two powered rails powered (a redstone block for the floor under
        // each) to kick their carts on
        "rider-sensors" => vec![
            (40, vec![s([0, 0, 0], "unload:1,0,1,0")]),
            (240, vec![s([0, 0, 0], "load:1,0,1,0")]),
            (260, vec![s([20, 63, 16], "minecraft:redstone_block"), s([28, 63, 16], "minecraft:redstone_block")]),
        ],
        // (tickexact 2026-11-11, G1's VillageBoundRandomStroll question) the bed's ticket taken
        "stroll-village-taken" => vec![(1, vec![s([8, 64, 3], "poi-take")])],
        // (tickexact 2026-11-11, farmopt's H retry on a frozen cart) the lanes' chunk (1,0) frozen
        // (its ticket dropped: level 32 beside the forced chunk 0,0) from the summon; every gate shut
        // at t7; the retry lanes' gates opened at t30 and shut at t42; the chunk back at t80
        "gate-frozen" => {
            let gate = |open: bool| format!("minecraft:oak_fence_gate[facing=north,in_wall=false,open={open},powered=false]");
            let lanes: Vec<(i32, bool)> = GATE_FROZEN_LANES.iter().enumerate().map(|(k, (_, retry, _))| (16 + 2 * k as i32, *retry)).collect();
            vec![
                (0, vec![s([0, 0, 0], "unload:1,0,1,0")]),
                (7, lanes.iter().map(|(x, _)| ([*x, 64, 7], gate(false))).collect()),
                (30, lanes.iter().filter(|(_, r)| *r).map(|(x, _)| ([*x, 64, 7], gate(true))).collect()),
                (42, lanes.iter().filter(|(_, r)| *r).map(|(x, _)| ([*x, 64, 7], gate(false))).collect()),
                (80, vec![s([0, 0, 0], "load:1,0,1,0")]),
            ]
        }
        // (tickexact 2026-11-11) the gate admissions: C1's and C2's gates (z 9, 12) for the first
        // tick only; G0 (z 3) and G_P (z 6) opened at t200 (3 and 2 gt) and at t600 (3 gt each);
        // each input a redstone block north of its gate
        "gate-admit" | "gate-admit-rev" | "gate-admit-s1" | "gate-admit-s2" => {
            let rb = "minecraft:redstone_block";
            let mut v = vec![
                (0, vec![s([20, 65, 8], rb), s([20, 65, 11], rb)]),
                (1, vec![s([20, 65, 8], "minecraft:air"), s([20, 65, 11], "minecraft:air")]),
                (200, vec![s([20, 65, 2], rb), s([20, 65, 5], rb)]),
                (202, vec![s([20, 65, 5], "minecraft:air")]),
                (203, vec![s([20, 65, 2], "minecraft:air")]),
                (600, vec![s([20, 65, 2], rb), s([20, 65, 5], rb)]),
                (603, vec![s([20, 65, 2], "minecraft:air"), s([20, 65, 5], "minecraft:air")]),
            ];
            // (tickexact 2026-11-11, the reload-order rule) the whole part away 1 gt at t300 (after
            // the first admissions), its load in that order; every entity of the scene stands in
            // chunk (1, 0), so only the order inside that chunk changes (the within-chunk part
            // alone: the riders' carts, the zombie's boat, each with its passenger after it)
            let order = match name {
                "gate-admit-rev" => Some("load@reverse"),
                "gate-admit-s1" => Some("load@seed:1"),
                "gate-admit-s2" => Some("load@seed:2"),
                _ => None,
            };
            if let Some(o) = order {
                v.push((300, vec![s([0, 0, 0], "unload")]));
                v.push((301, vec![s([0, 0, 0], o)]));
                v.sort_by_key(|(t, _)| *t);
            }
            v
        }
        // (tickexact 2026-11-11) the re-claim delay: each pen's west wall opened at its time
        "reclaim-backoff" | "reclaim-backoff-day" => {
            let open = |x: i32| vec![s([x - 1, 64, 6], "minecraft:air"), s([x - 1, 65, 6], "minecraft:air")];
            vec![(300, open(6)), (1500, open(16)), (4000, open(26))]
        }
        "reclaim-backoff-1" => vec![(300, vec![s([5, 64, 6], "minecraft:air"), s([5, 65, 6], "minecraft:air")])],
        // (tickexact 2026-11-11, ironspec's ticket accounting) A (e0) and B (e1) with HOME on the bed
        // as data at t1 (the port's start state: its ticket free); A gives HOME up at t5 (a release
        // with the ticket free: nothing to give back) and re-claims the free ticket; B gives HOME up
        // at t200 (the release frees A's ticket: no holder is kept) and re-claims it; A's memory is
        // erased at t400 with nothing released (the ticket stays taken: A stays homeless)
        "ticket-release" => vec![
            (1, vec![s([5, 64, 6], "home:e0"), s([5, 64, 6], "home:e1")]),
            (5, vec![s([0, 0, 0], "unhome:e0")]),
            (200, vec![s([0, 0, 0], "unhome:e1")]),
            (400, vec![s([0, 0, 0], "forget:e0")]),
        ],
        // (tickexact 2026-11-11) the recoverability scenes: HOME and tickets at t1-t3, the freeze
        // 900..2400, the pens opened at t2500
        "recover-single" | "recover-shared" | "recover-occupied" => {
            let bed = [5, 64, 6];
            let open = |x: i32, z: i32| vec![s([x - 1, 64, z], "minecraft:air"), s([x - 1, 65, z], "minecraft:air")];
            // the bed broken and placed again in one go (no tick sees it missing)
            let replace = vec![
                s([5, 64, 5], "minecraft:air"),
                s([5, 64, 6], "minecraft:air"),
                s([5, 64, 6], "minecraft:red_bed[facing=south,occupied=false,part=head]"),
                s([5, 64, 5], "minecraft:red_bed[facing=south,occupied=false,part=foot]"),
            ];
            match name {
                "recover-single" => vec![
                    (1, vec![s(bed, "home:e0"), s(bed, "poi-take")]),
                    (900, vec![s([0, 0, 0], "unload:0,0,0,0")]),
                    (2400, vec![s([0, 0, 0], "load:0,0,0,0")]),
                    (2500, open(8, 6)),
                ],
                "recover-shared" => vec![
                    (1, vec![s(bed, "home:e0"), s(bed, "poi-take")]),
                    (2, replace),
                    (3, vec![s(bed, "home:e1"), s(bed, "poi-take")]),
                    (900, vec![s([0, 0, 0], "unload:0,0,0,0")]),
                    (2400, vec![s([0, 0, 0], "load:0,0,0,0")]),
                    (2500, [open(8, 6), open(8, 9)].concat()),
                ],
                _ => vec![
                    (1, vec![s(bed, "home:e1"), s(bed, "poi-take")]),
                    (2, replace),
                    (3, vec![s(bed, "home:e0"), s(bed, "poi-take")]),
                    (100, open(8, 6)),
                    (900, vec![s([0, 0, 0], "unload:1,0,1,0")]),
                    (2400, vec![s([0, 0, 0], "load:1,0,1,0")]),
                    (2500, open(22, 6)),
                ],
            }
        }
        // (tickexact 2026-11-11) the bed radius: S's HOME and the bed's ticket, the riders' HOME,
        // at t1; S let out at t100; "-reload": the whole part away 1 gt at t300
        "bed-radius" | "bed-radius-zombie" | "bed-radius-zombie-reload" | "bed-radius-reload" => {
            let mut v = vec![
                (1, vec![s([5, 64, 6], "home:e0"), s([5, 64, 6], "poi-take"), s([5, 64, 6], "home:e1.p0"), s([5, 64, 6], "home:e2.p0")]),
                (100, vec![s([7, 64, 6], "minecraft:air"), s([7, 65, 6], "minecraft:air")]),
            ];
            if name.ends_with("-reload") {
                v.push((300, vec![s([0, 0, 0], "unload")]));
                v.push((301, vec![s([0, 0, 0], "load")]));
            }
            v
        }
        // (tickexact 2026-11-11) the rider push: each rider's HOME at t1 (R's and P's beds' tickets
        // taken; N's in an unloaded chunk)
        "rider-push" => vec![(1, vec![s([16, 64, 6], "home:e1.p0"), s([16, 64, 6], "poi-take"), s([16, 64, 11], "home:e3.p0"), s([16, 64, 11], "poi-take"), s([100, 64, 13], "home:e5.p0")])],
        "home-reach-open" => vec![(1, vec![s([10, 64, 11], "home:e0.p0"), s([10, 64, 11], "poi-take"), s([26, 64, 11], "home:e1.p0"), s([26, 64, 11], "poi-take")])],
        // (tickexact 2026-11-11) the freeze across a cant-reach streak: each rider's HOME and its
        // bed's POI at t1, then R1's chunk (0, 0) frozen from t900 for 1500 (or 5000) gt
        "freeze-cantreach" | "freeze-cantreach-5000" | "freeze-cantreach-zombie" | "freeze-cantreach-zombie-5000" => {
            let back = if name.ends_with("-5000") { 5900 } else { 2400 };
            vec![
                (1, vec![s([10, 64, 11], "home:e0.p0"), s([10, 64, 11], "poi-take"), s([26, 64, 11], "home:e1.p0"), s([26, 64, 11], "poi-take")]),
                (900, vec![s([0, 0, 0], "unload:0,0,0,0")]),
                (back, vec![s([0, 0, 0], "load:0,0,0,0")]),
            ]
        }
        // (tickexact 2026-11-10) the boarding cooldown across a reload: the whole part away 1 gt
        // right after the throw (t8)
        "board-cooldown-reload" => vec![(10, vec![s([0, 0, 0], "unload")]), (11, vec![s([0, 0, 0], "load")])],
        // (tickexact 2026-11-10) the natural doors' halves: the first copper door's upper half
        // removed, the second's lower half, the floor under the third, the fourth's upper half
        // swapped for an east-facing door's, the oak door's upper half removed
        "door-halves" => vec![
            (10, vec![s([4, 65, 4], "minecraft:air")]),
            (20, vec![s([8, 64, 4], "minecraft:air")]),
            (30, vec![s([12, 63, 4], "minecraft:air")]),
            (40, vec![s([16, 65, 4], "minecraft:waxed_oxidized_copper_door[facing=east,half=upper,hinge=left,open=false,powered=false]")]),
            (50, vec![s([20, 65, 4], "minecraft:air")]),
        ],
        // (tickexact 2026-11-10) the contact queue: the stone at the slope's foot a rail for 12 gt
        // every 200 gt from t100; the unload variant adds the whole part away 30 gt at t250 (the
        // queue pressed), 5 gt at t702 (mid-release), N = 0 at t900, and the west chunk (the run
        // and the released carts) at 32 from t1000 for 200 gt, the east chunk (the slope's top) at
        // 32 from t1250 for 40 gt
        "contact6" | "contact6-unload" => {
            let mut v: Vec<(u32, Vec<([i32; 3], String)>)> = Vec::new();
            for k in 0..6u32 {
                let t = 100 + 200 * k;
                v.push((t, vec![s([3, 64, 2], "minecraft:rail[shape=east_west,waterlogged=false]")]));
                v.push((t + 12, vec![s([3, 64, 2], "minecraft:stone")]));
            }
            if name == "contact6-unload" {
                let acts: [(u32, &str); 10] =
                    [(250, "unload"), (280, "load"), (702, "unload"), (707, "load"), (900, "unload"), (900, "load"), (1000, "unload:-1,0,-1,0"), (1200, "load:-1,0,-1,0"), (1250, "unload:1,0,1,0"), (1290, "load:1,0,1,0")];
                for (t, a) in acts {
                    match v.iter_mut().find(|(at, _)| *at == t) {
                        Some((_, b)) => b.push(s([0, 0, 0], a)),
                        None => v.push((t, vec![s([0, 0, 0], a)])),
                    }
                }
                v.sort_by_key(|(t, _)| *t);
            }
            v
        }
        // (tickexact 2026-11-11, ironspec's forced-phase mode) the sleeper's reload: HOME at t1,
        // the whole sandbox away t200..t240, the zombie's wall gone at the load; `mobai
        // reload-wake`'s pair (or MOBAI_SENSOR_FORCE=A,B) forces the two wake sensors' start delays
        // right after the load
        "reload-wake" => {
            let mut at240 = vec![s([0, 0, 0], "load")];
            let forced = RELOAD_WAKE_FORCE.lock().map(|g| *g).unwrap_or(None).or_else(|| {
                std::env::var("MOBAI_SENSOR_FORCE").ok().and_then(|f| f.split_once(',').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
            });
            if let Some((a, b)) = forced {
                at240.push(s([8, 64, 9], &format!("sensor:0={a}")));
                at240.push(s([8, 64, 9], &format!("sensor:5={b}")));
            }
            at240.push(s([11, 64, 9], "minecraft:air"));
            at240.push(s([11, 65, 9], "minecraft:air"));
            vec![(1, vec![s([8, 64, 9], "home:e0"), s([8, 64, 9], "poi-take")]), (200, vec![s([0, 0, 0], "unload")]), (240, at240)]
        }
        // (tickexact 2026-11-11) the panes' neighbours changed
        "pane-connect" => vec![
            (10, vec![s([6, 64, 4], "minecraft:air")]),
            (20, vec![s([5, 64, 7], "minecraft:stone")]),
            (30, vec![s([6, 64, 12], "minecraft:air")]),
        ],
        // (tickexact 2026-11-11) level 33: the east chunks' tickets dropped, the three redstone
        // blocks set at t60, the tickets back at t240
        "level33-set" => vec![
            (40, vec![s([0, 0, 0], "unload:0,0,1,0")]),
            (60, vec![s([18, 64, 2], "minecraft:redstone_block"), s([18, 64, 6], "minecraft:redstone_block"), s([2, 64, 10], "minecraft:redstone_block")]),
            (240, vec![s([0, 0, 0], "load:0,0,1,0")]),
        ],
        // (tickexact 2026-11-10) level 32: the top chest filled, the repeater loop started by a
        // one-tick pulse, the east chunk's forced ticket dropped from t40 to t240
        "level32" => vec![
            (1, vec![s([4, 66, 8], "nbt:{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:64}]}")]),
            (5, vec![s([6, 64, 4], "minecraft:redstone_block")]),
            (6, vec![s([6, 64, 4], "minecraft:air")]),
            (40, vec![s([0, 0, 0], "unload:0,0,0,0")]),
            (240, vec![s([0, 0, 0], "load:0,0,0,0")]),
        ],
        // corrupt-poi-entity R1: the claimed bed broken, then placed again (a fresh POI record
        // with its ticket free), then the second villager's pen opened
        "bed-r1" => vec![
            (600, vec![s([31, 64, 21], "minecraft:air"), s([31, 64, 20], "minecraft:air")]),
            (601, vec![s([31, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=foot]"), s([31, 64, 21], "minecraft:red_bed[facing=south,occupied=false,part=head]")]),
            (602, vec![s([40, 64, 38], "minecraft:air"), s([40, 65, 38], "minecraft:air")]),
        ],
        // the carousel's rider: the second villager's pen opened once the carted one holds the bed
        "bed-cart" => vec![(400, vec![s([40, 64, 38], "minecraft:air"), s([40, 65, 38], "minecraft:air")])],
        // (tickexact) the release and the re-take: A's box shut, the R1 double claim, A's box opened
        "bed-retake" => vec![
            (300, vec![s([11, 64, 10], "minecraft:stone"), s([11, 65, 10], "minecraft:stone")]),
            (600, vec![s([31, 64, 21], "minecraft:air"), s([31, 64, 20], "minecraft:air")]),
            (601, vec![s([31, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=foot]"), s([31, 64, 21], "minecraft:red_bed[facing=south,occupied=false,part=head]")]),
            (602, vec![s([40, 64, 38], "minecraft:air"), s([40, 65, 38], "minecraft:air")]),
            (12600, vec![s([11, 64, 10], "minecraft:air"), s([11, 65, 10], "minecraft:air")]),
        ],
        // (tickexact) the nap's panic wake: the window on the penned zombie opened, shut, opened
        "cart-queue" => vec![(300, (2..=9).map(|x| s([x, 63, 24], "minecraft:redstone_block")).collect())],
        "cart-queue-bend" => vec![(300, (5..=8).map(|x| s([x, 63, 30], "minecraft:redstone_block")).collect())],
        "cart-queue-slope" => vec![
            (300, vec![s([41, 65, 24], "minecraft:redstone_block"), s([42, 64, 24], "minecraft:redstone_block"), s([43, 63, 24], "minecraft:redstone_block")]),
            (600, (2..=9).map(|x| s([x, 66, 24], "minecraft:redstone_block")).collect()),
        ],
        "cart-train" => vec![
            (100, vec![s([24, 62, 13], "minecraft:redstone_torch")]),
            (700, vec![s([24, 62, 13], "minecraft:air")]),
            (800, vec![s([24, 62, 30], "minecraft:redstone_torch")]),
        ],
        "bed-panic" => vec![
            (2000, vec![s([35, 65, 21], "minecraft:air")]),
            (2400, vec![s([35, 65, 21], "minecraft:stone")]),
            (4600, vec![s([35, 65, 21], "minecraft:air")]),
        ],
        // (tickexact) the summon tick: the window row on the dorm's zombie opened; the golems' pad
        // taken away under the first golem (summoned at 2100) so it falls out of every sensor, and
        // put back at 2600 for the slot 600 after the summon
        "golem-summon" => vec![
            (2000, (27..=35).map(|x| s([x, 65, 23], "minecraft:air")).collect()),
            (2100, (27..=35).flat_map(|x| (12..=14).map(move |z| s([x, 63, z], "minecraft:air"))).collect()),
            (2600, (27..=35).flat_map(|x| (12..=14).map(move |z| s([x, 63, z], "minecraft:stone"))).collect()),
        ],
        _ => Vec::new(),
    }
}

/// (tickexact 2026-11-11, `mobai reload-wake`) The forced start delays of scene reload-wake's
/// sleeper (NearestLivingEntitySensor, VillagerHostilesSensor), and the villagers' poses each
/// compare tick read back (game, redsim) while it is `Some`.
static RELOAD_WAKE_FORCE: std::sync::Mutex<Option<(i64, i64)>> = std::sync::Mutex::new(None);
static RELOAD_WAKE_POSES: std::sync::Mutex<Option<Vec<(i64, Option<String>, Option<String>)>>> = std::sync::Mutex::new(None);

/// (tickexact 2026-11-11, the ledge report) the game's minecarts each tick while a ledge scene runs:
/// (game time, entity number, position, motion).
type LedgeTrack = Vec<(i64, u32, [f64; 3], [f64; 3])>;
static LEDGE_TRACK: std::sync::Mutex<Option<LedgeTrack>> = std::sync::Mutex::new(None);

/// (tickexact 2026-11-11, ironspec's ledge on R1's drop) `mctas mobai ledge`: each ledge scene (one per
/// stored speed of [`LEDGE_SPEEDS`]) in the game and in redsim, compared tick for tick, and per row
/// (its follower's gap, riders or not) from the game's frames: each cart's take-off (the last tick
/// on the take-off flats), its landing (the first tick back on a rail: the cell, its speed), the
/// carts' least centre spacing after the take-off (a merge: closer than a cart's length, 0.98) and
/// whether each cart ends on the line's rails (a derail: off them).
pub fn ledge(version: &str, ticks: u32, only: &[String]) -> Result<()> {
    let all = scenes();
    let runs: Vec<(String, f64, i32)> = LEDGE_SPEEDS
        .iter()
        .map(|(tag, sp)| (format!("ledge-r1-{tag}"), *sp, 24))
        .chain([("ledge-r1n-182".to_string(), 1.82, 25), ("ledge-r1s-182".to_string(), 1.82, 24), ("takeoff-clamp".to_string(), 0.0, 24)])
        .filter(|(n, _, _)| only.is_empty() || only.iter().any(|o| o == n))
        .collect();
    for (name, sp, line) in runs {
        let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}"))?;
        let rows = ledge_rows_of(&name);
        *LEDGE_TRACK.lock().map_err(|_| anyhow!("lock"))? = Some(Vec::new());
        let c = compare(version, s, 1, ticks, 100, false, false, None);
        let track = LEDGE_TRACK.lock().map_err(|_| anyhow!("lock"))?.take().unwrap_or_default();
        let c = c?;
        println!("{name} (stored {sp} b/t): {} ticks compared, {}", c.ticks, c.first.as_ref().map_or("0 differences".to_string(), |d| format!("FIRST DIFFERENCE at {}: {}", d.t, d.what)));
        // the rows' carts in entity order (a ridden cart's villager takes the next number)
        let mut n = 0u32;
        for (k, (gap, ridden)) in rows.iter().enumerate() {
            let carts: Vec<u32> = (0..if gap.is_some() { 2 } else { 1 }).map(|j| n + j * if *ridden { 2 } else { 1 }).collect();
            n += carts.len() as u32 * if *ridden { 2 } else { 1 };
            let z_line = (2 + 8 * k) as f64 + 0.5 + (line - 24) as f64;
            let path = |e: u32| -> Vec<(i64, [f64; 3], [f64; 3])> { track.iter().filter(|(_, m, _, _)| *m == e).map(|(t, _, p, v)| (*t, *p, *v)).collect() };
            let t0 = track.first().map(|x| x.0).unwrap_or(0);
            let mut parts = Vec::new();
            for (j, e) in carts.iter().enumerate() {
                let p = path(*e);
                // take-off: the last tick at rail height on the flats (x >= 8.0: design 6.0); landing:
                // the first tick after it back at rest on a rail of the drop (no fall: motion y >= 0)
                let take = p.iter().rposition(|(_, q, _)| q[0] >= 8.0 && (q[1] - 73.0625).abs() < 1e-6);
                let land = take.and_then(|i| p[i + 1..].iter().position(|(_, q, v)| q[1] < 73.0 && v[1].abs() < 1e-9 && q[0] < 8.0).map(|d| i + 1 + d));
                let desc = match (take, land) {
                    (Some(i), Some(l)) => {
                        let (tl, ql, vl) = p[l];
                        let cell = [(ql[0] - 2.0).floor() as i32, (ql[1] - 60.0).floor() as i32, line];
                        format!("cart {} off at t+{} landed t+{} at x {:.2} y {:.4} (design cell {},{},{}) at {:.3} b/t", ["A", "B"][j], p[i].0 - t0, tl - t0, ql[0] - 2.0, ql[1] - 60.0, cell[0], cell[1], cell[2], (vl[0] * vl[0] + vl[2] * vl[2]).sqrt())
                    }
                    (Some(i), None) => format!("cart {} off at t+{}, never landed on a rail", ["A", "B"][j], p[i].0 - t0),
                    _ => format!("cart {} never left the flats", ["A", "B"][j]),
                };
                // the end: at the line's stop (the descent's flats, design x 0, y 5), stuck on the
                // take-off's lip (over the gap at the flats' height, resting on their support), stuck
                // in the gap (on the landing's support, below the gap), or elsewhere
                let end = p.last().map(|(_, q, v)| {
                    let (x, y, z) = (q[0] - 2.0, q[1] - 60.0, q[2] - (8 * k) as f64 + 22.0);
                    let moving = (v[0] * v[0] + v[2] * v[2]).sqrt() > 1e-6;
                    let what = if (x - 0.5).abs() < 0.01 && (y - 5.0625).abs() < 0.01 && z >= 27.5 {
                        "ends at the stop".to_string()
                    } else if (5.0..6.3).contains(&x) && (y - 13.0).abs() < 0.01 && !moving {
                        "STUCK on the take-off's lip (resting on its support over the gap)".to_string()
                    } else if (4.5..6.0).contains(&x) && y < 12.0 && y > 10.0 && !moving {
                        "STUCK in the gap (resting on the landing's support)".to_string()
                    } else if (q[2] - z_line).abs() > 0.01 && (q[0] - 2.5).abs() > 0.01 {
                        "DERAILED (off the line)".to_string()
                    } else {
                        format!("ends {}", if moving { "still moving" } else { "at rest" })
                    };
                    format!("{what} at design {x:.2},{y:.2},{z:.2}")
                }).unwrap_or_default();
                parts.push(format!("{desc}; {end}"));
            }
            // the least centre spacing of the pair after the leader's take-off
            let spacing = if carts.len() == 2 {
                let (a, b) = (path(carts[0]), path(carts[1]));
                let off = a.iter().rposition(|(_, q, _)| q[0] >= 8.0).map(|i| a[i].0).unwrap_or(t0);
                let least = a
                    .iter()
                    .filter(|(t, _, _)| *t > off)
                    .filter_map(|(t, qa, _)| b.iter().find(|(tb, _, _)| tb == t).map(|(_, qb, _)| (*t, ((qa[0] - qb[0]).powi(2) + (qa[1] - qb[1]).powi(2) + (qa[2] - qb[2]).powi(2)).sqrt())))
                    .min_by(|x, y| x.1.total_cmp(&y.1));
                least.map(|(t, d)| format!("; least spacing {d:.3} at t+{}{}", t - t0, if d < 0.98 { " (MERGED: closer than a cart's length)" } else { "" })).unwrap_or_default()
            } else {
                String::new()
            };
            let what = match gap {
                None => "a lone cart".to_string(),
                Some(g) if *g == 0.0 => "two touching carts (0.98 apart)".to_string(),
                Some(g) => format!("two carts {g} gt apart"),
            };
            println!("  row {k} ({what}, {}): {}{spacing}", if *ridden { "ridden" } else { "empty" }, parts.join(" | "));
        }
    }
    Ok(())
}

/// `mctas mobai reload-wake` (ironspec's forced-phase mode): scene reload-wake (a sleeper whose
/// whole sandbox is unloaded and loaded again, a zombie shown at the load) once per pair of forced
/// start delays of its NearestLivingEntitySensor and VillagerHostilesSensor and per sandbox seed,
/// game against redsim tick for tick; per run the wake after the load on both sides against the
/// sensors' rule (each fires first at max(1, delay) and every 20 after; the first hostile scan at
/// or after the first living scan wakes it), then whether every run was exact and on the rule.
pub fn reload_wake(version: &str, living: &[i64], hostiles: &[i64], seeds: &[i64]) -> Result<()> {
    let all = scenes();
    let s = all.iter().find(|s| s.name == "reload-wake").ok_or_else(|| anyhow!("no scene reload-wake"))?;
    let rule = |a: i64, b: i64| -> i64 {
        let (l, h) = (a.max(1), b.max(1));
        if h >= l { h } else { h + 20 * ((l - h + 19) / 20) }
    };
    let (mut runs, mut exact, mut on_rule) = (0, 0, 0);
    for &seed in seeds {
        for &a in living {
            for &b in hostiles {
                *RELOAD_WAKE_FORCE.lock().map_err(|_| anyhow!("lock"))? = Some((a, b));
                *RELOAD_WAKE_POSES.lock().map_err(|_| anyhow!("lock"))? = Some(Vec::new());
                let c = compare(version, s, seed, 320, 100, false, false, None);
                let poses = RELOAD_WAKE_POSES.lock().map_err(|_| anyhow!("lock"))?.take().unwrap_or_default();
                *RELOAD_WAKE_FORCE.lock().map_err(|_| anyhow!("lock"))? = None;
                let c = c?;
                let wake = |game: bool| poses.iter().find(|(t, g, r)| *t > 240 && (if game { g } else { r }).as_deref() == Some("standing")).map(|(t, ..)| t - 240);
                let (g, r) = (wake(true), wake(false));
                let want = rule(a, b);
                runs += 1;
                exact += c.first.is_none() as usize;
                on_rule += (g == Some(want) && r == Some(want)) as usize;
                println!(
                    "living {a:>2} hostiles {b:>2} seed {seed}: the game wakes {}, redsim {}, the rule +{want}; {} ticks compared, {}",
                    g.map_or("never".to_string(), |w| format!("+{w}")),
                    r.map_or("never".to_string(), |w| format!("+{w}")),
                    c.ticks,
                    c.first.as_ref().map_or("0 differences".to_string(), |d| format!("FIRST DIFFERENCE at {}: {}", d.t, d.what))
                );
            }
        }
    }
    println!("{runs} runs: {exact} exact, {on_rule} woke on the sensors' rule on both sides");
    Ok(())
}

/// The next scripted action at or after `done` ticks: (its tick, its blocks).
fn next_action(actions: &[(u32, Vec<([i32; 3], String)>)], done: i64) -> Option<&(u32, Vec<([i32; 3], String)>)> {
    actions.iter().find(|(t, _)| *t as i64 >= done)
}

/// A `sensor:K=V` action's sensor index and delay (K below the villager's sensor count, V below
/// that sensor's scan rate).
fn sensor_kv(kv: &str) -> Result<(usize, i64)> {
    let (k, v) = kv.split_once('=').ok_or_else(|| anyhow!("sensor:{kv}: K=V"))?;
    let (k, v): (usize, i64) = (k.parse()?, v.parse()?);
    let rates = redsim::entity::mob::villager::SENSORS;
    anyhow::ensure!(k < rates.len() && (0..rates[k].1).contains(&v), "sensor:{kv}: sensor 0..{} and a delay below its scan rate", rates.len() - 1);
    Ok((k, v))
}

/// redsim's side of an action: each block set with flags 3; a container's fill (`nbt:<snbt>`)
/// merged as the game's data merge does ([`redsim::redstone::container::apply_block_entities`]).
fn apply_redsim(w: &mut World, blocks: &[([i32; 3], String)], ctx: &ActCtx) -> Result<()> {
    for (p, s) in blocks {
        let pos = BlockPos::new(p[0], p[1], p[2]);
        // (tickexact 2026-11-10, ironspec's fault rule) every chunk saved and unloaded / loaded
        // again
        if s == "unload" {
            redsim::reload::unload_all(w);
            continue;
        }
        if s == "load" {
            redsim::reload::load_all(w);
            continue;
        }
        // (tickexact 2026-11-11, the reload-order rule) `load@<order>`: the entities in that order
        if let Some(o) = s.strip_prefix("load@") {
            let order = redsim::reload::Order::parse(o).ok_or_else(|| anyhow!("load@{o}: an order is save, reverse or seed:K"))?;
            redsim::reload::load_all_ordered(w, order);
            continue;
        }
        // (tickexact 2026-11-11, ironspec's forced-phase mode) `sensor:K=V`: the villager nearest
        // the action's cell (within 2) gets its K-th sensor's start delay set to V
        if let Some(kv) = s.strip_prefix("sensor:") {
            let (k, v) = sensor_kv(kv)?;
            let c = redsim::entity::Vec3::new(p[0] as f64 + 0.5, p[1] as f64 + 0.5, p[2] as f64 + 0.5);
            let best = w
                .entities
                .list
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.removed && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager))
                .map(|(i, e)| (i, (e.pos.x - c.x).powi(2) + (e.pos.y - c.y).powi(2) + (e.pos.z - c.z).powi(2)))
                .filter(|(_, d)| *d < 4.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
                .ok_or_else(|| anyhow!("sensor:{kv}: no villager within 2 of {p:?}"))?;
            if let redsim::entity::EntityKind::Mob(m) = &mut w.entities.list[best].kind
                && let Some(b) = m.brain.as_deref_mut()
            {
                b.sensors[k] = v;
            }
            continue;
        }
        // (the partial form: a chunk box's forced tickets)
        if let Some(b) = s.strip_prefix("unload:").and_then(redsim::entity::mob::drive::chunk_box) {
            redsim::reload::unload_some(w, b);
            continue;
        }
        if let Some(b) = s.strip_prefix("load:").and_then(redsim::entity::mob::drive::chunk_box) {
            redsim::reload::load_some(w, b);
            if std::env::var_os("MOBAI_TRACE_LOAD").is_some() {
                eprintln!("redsim {s} tick list: {:?}", w.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect::<Vec<_>>());
            }
            continue;
        }
        // (tickexact 2026-11-10, farmopt's station) a drive's Nether build: redsim's Nether part
        // ([`GameScene::apply`]: the bridge's Nether sandbox's)
        if let Some(st) = s.strip_prefix("nether:") {
            let n = w.nether.as_deref_mut().ok_or_else(|| anyhow!("a nether-set act in a scene without a Nether part"))?;
            let b = Block::parse(st).or_else(|_| redsim::terrain::land::block_of(st)).map_err(|e| anyhow!("action block {st}: {e}"))?;
            n.set_block(pos, b, redsim::world::flags::ALL);
            continue;
        }
        if let Some(snbt) = s.strip_prefix("nether-nbt:") {
            let n = w.nether.as_deref_mut().ok_or_else(|| anyhow!("a nether-nbt act in a scene without a Nether part"))?;
            redsim::redstone::container::apply_block_entities(n, [(pos, snbt)]).map_err(|e| anyhow!(e))?;
            continue;
        }
        if let Some(snbt) = s.strip_prefix("nbt:") {
            redsim::redstone::container::apply_block_entities(w, [(pos, snbt)]).map_err(|e| anyhow!(e))?;
            continue;
        }
        // a bench's entity act: redsim's drive replay (`drive::apply`)
        if let Some(act) = entity_act(*p, s)? {
            // (ironspec's ticket accounting: the HOME record's free tickets around an unhome or
            // forget, redsim's side; the game's from its `home_release`)
            let home_of = |w: &World, act: &redsim::entity::mob::drive::Act| -> Option<BlockPos> {
                let who = match act {
                    redsim::entity::mob::drive::Act::Unhome(who) | redsim::entity::mob::drive::Act::Forget(who) => *who,
                    _ => return None,
                };
                let n = ctx.number(who).ok()?;
                let i = w.entities.list.iter().position(|e| !e.removed && e.id == n)?;
                matches!(w.entities.list[i].kind, redsim::entity::EntityKind::Mob(_)).then(|| redsim::entity::mob::ai::brain::brain(w, i).home).flatten()
            };
            let home = home_of(w, &act);
            let before = home.and_then(|h| redsim::entity::mob::ai::poi::free_tickets(w, h));
            redsim::entity::mob::drive::apply(w, &ctx.lines, ctx.g0, &act);
            if let Some(h) = home {
                let (t, after) = (w.time, redsim::entity::mob::ai::poi::free_tickets(w, h));
                eprintln!("redsim {s} at t {t}: HOME {},{},{}; its free tickets {before:?} -> {after:?}", h.x, h.y, h.z);
            }
            continue;
        }
        let b = Block::parse(s).or_else(|_| redsim::terrain::land::block_of(s)).map_err(|e| anyhow!("action block {s}: {e}"))?;
        w.set_block(pos, b, redsim::world::flags::ALL);
    }
    Ok(())
}

impl Scene {
    /// The scene as canonical text (a recording keeps its hash: a scene changed since it was
    /// recorded fails its replay).
    pub fn canonical(&self) -> String {
        let mut v = json!({
            "name": self.name,
            "chunks": self.chunks,
            "blocks": self.blocks.iter().map(|(p, s)| json!([p, s])).collect::<Vec<_>>(),
            "mobs": self.mobs.iter().map(|(t, p, n)| json!([t, p, n])).collect::<Vec<_>>(),
            "day_time": self.day_time,
            "difficulty": difficulty_name(self.difficulty),
            "mob_griefing": self.mob_griefing,
            "watch": [self.watch.0, self.watch.1],
            "player": self.player,
        });
        if self.player_creative {
            v["player_creative"] = json!(true);
        }
        if self.random_tick_speed != 3 {
            v["random_tick_speed"] = json!(self.random_tick_speed);
        }
        let actions = scene_actions(&self.name);
        if !actions.is_empty() {
            v["actions"] = json!(actions.iter().map(|(t, b)| json!([t, b.iter().map(|(p, s)| json!([p, s])).collect::<Vec<_>>()])).collect::<Vec<_>>());
        }
        let settle = scene_settle(&self.name);
        if settle > 0 {
            v["settle"] = json!(settle);
        }
        if scene_judge_placed(&self.name) {
            v["judge_placed"] = json!(true);
            v["block_entities"] = json!(scene_block_entities(&self.name));
            if scene_bench_start(&self.name) {
                v["bench_start"] = json!(true);
            }
        } else if scene_placement(&self.name) == Placement::Merged {
            v["placed"] = json!("as written");
            v["block_entities"] = json!(scene_block_entities(&self.name));
        } else if scene_placement(&self.name) == Placement::Built {
            v["placed"] = json!("built in order");
            v["spawn"] = json!(scene_spawn(&self.name));
        }
        v.to_string()
    }

    pub fn hash(&self) -> String {
        format!("{:016x}", mctas::recorded::fnv64(self.canonical().as_bytes()))
    }
}

/// (tickexact 2026-11-10) ironfarm's single-lane carousel loop (explore/ironfarm c69c22943,
/// `carousel-single-lane-loop-wip.txt`, frozen here): its blocks and carts as they are, moved into
/// the sandbox (design x + 34, y + 64, z + 10).
const CAROUSEL_LOOP: &str = include_str!("../scenes/carousel-loop-wip.txt");
const CAROUSEL_SHIFT: [i32; 3] = [34, 64, 10];

/// (tickexact 2026-11-11, ironspec's ledge on R1's drop) ironfarm's escR1, generated by main's
/// generator (`mctas mobfarm queue-loop --slots 14 --boost-first 8 --row-gap 3 --detector
/// --zombie-every 8 --stand-stop --tail-esc 5 --esc-adm --adm-ledge --adm-xq 4`, 13,740 bytes),
/// frozen here: its take-off flats (8,13,24) (7,13,24) (6,13,24), the gap at x 5, the landing
/// (4,11,24) then (3,10,24), the slopes (2,9,24) (1,8,24), the curve (0,8,24) and the descent south.
const LEDGE_ESCR1: &str = include_str!("../scenes/ledge-escR1.txt");

/// (tickexact 2026-11-11, ironspec's hatch sight lines) 58f770cc's siding (b) FC on the tight cage
/// by itself (explore/farmopt-station 10d6461ab, `carousel-siding-b-fc-alone.txt`), frozen.
const SIDING_B_FC: &str = include_str!("../scenes/siding-b-fc-alone.txt");

/// A siding block as geometry for a sight-line scene: redstone components gone (no collision box,
/// or a slab's: repeaters and comparators as smooth stone slabs; redstone blocks and droppers as
/// stone), rails and trapdoors unpowered, leaves off any log (distance 7); every collision shape
/// a sight ray meets kept.
fn siding_geometry(st: &str) -> Option<String> {
    let id = st.split('[').next().unwrap_or(st);
    match id {
        "minecraft:redstone_wire" | "minecraft:redstone_torch" | "minecraft:redstone_wall_torch" | "minecraft:tripwire" | "minecraft:tripwire_hook" => None,
        "minecraft:repeater" | "minecraft:comparator" => Some("minecraft:smooth_stone_slab[type=bottom,waterlogged=false]".to_string()),
        "minecraft:redstone_block" | "minecraft:dropper" | "minecraft:observer" | "minecraft:hopper" => Some("minecraft:stone".to_string()),
        "minecraft:powered_rail" | "minecraft:activator_rail" | "minecraft:detector_rail" | "minecraft:iron_trapdoor" => Some(st.replace("powered=true", "powered=false")),
        "minecraft:oak_leaves" => Some(st.replace("distance=1", "distance=7")),
        _ => Some(st.to_string()),
    }
}

/// The hatch's cell in scene siding-hatch's lane `lane` (design (3,7,3)).
fn siding_hatch_at(lane: i32) -> [i32; 3] {
    [3 + 8, 7 + 68, 3 + 5 + 20 * lane]
}

/// (tickexact 2026-11-11) ironfarm's R1 contact scenes (explore/ironfarm 5d930291e, `mobfarm
/// contact-scenes`): the escapement column copied from escR1 (x 0, z 26..34, y -1..8: the slopes
/// (0,7..3,26..30), G0 (0,3,31) shut over P (0,2,31) on its redstone block, G_P (0,2,32) over R,
/// slot 0 (0,0,33)), case g0 (G0 shut) and case p (G0 open).
const R1_CONTACT_G0: &str = include_str!("../scenes/r1-contact-g0.txt");
const R1_CONTACT_P: &str = include_str!("../scenes/r1-contact-p.txt");

/// (tickexact 2026-11-11) A lane at `x`: a closed oak gate at (x,64,3) at the foot of `n` ascending
/// rails rising south ((x,64+s,4+s), each on stone), then flat rails at y 64+n to z 14.
fn slope_lane(b: &mut Vec<([i32; 3], String)>, x: i32, n: i32, stone: &str) {
    slope_lane_at(b, x, 0, n, stone);
}

/// [`slope_lane`] moved `dz` along z.
fn slope_lane_at(b: &mut Vec<([i32; 3], String)>, x: i32, dz: i32, n: i32, stone: &str) {
    b.push(([x, 64, 3 + dz], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string()));
    for s in 0..n {
        for yy in 64..64 + s {
            b.push(([x, yy, 4 + s + dz], stone.to_string()));
        }
        b.push(([x, 64 + s, 4 + s + dz], "minecraft:rail[shape=ascending_south,waterlogged=false]".to_string()));
    }
    for z in (4 + n)..=14 {
        for yy in 64..64 + n {
            b.push(([x, yy, z + dz], stone.to_string()));
        }
        b.push(([x, 64 + n, z + dz], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
    }
}

/// (tickexact 2026-11-11, ironspec's queue-back sweep) A pressed 3-cart queue on `n` slope rails
/// before a closed gate (slope_lane's), its front and middle ridden and its back empty, at rest
/// where scene queue-settle found them (z 4.1150 / 4.9050 / 5.6950, y 64.1775 / 64.9675 / 65.7575:
/// each 0.79 behind the one ahead, its rider's box), and an arrival set off from the flat just
/// behind the slope's top (z 4.5 + n, at least 7.5: 1.8 behind the back cart) at stored 0.02 k (101 lanes, 0 to 2.0, 26 to a row 2 apart,
/// rows 12 apart), ridden or empty.
fn queue_back_scene(name: &str, n: i32, arrive_ridden: bool) -> Scene {
    let stone = "minecraft:stone";
    let mut b = Vec::new();
    floor(&mut b, 0, 64, 0, 64, 63, stone);
    let mut mobs = Vec::new();
    let cart = |x: f64, y: f64, z: f64, v: f64, ridden: bool| {
        let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
        ("minecraft:minecart".to_string(), [x, y, z], Some(format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", -v)))
    };
    for k in 0..101 {
        let (col, row) = (k % 26, k / 26);
        let (x, dz) = (1 + 2 * col as i32, 12 * row as i32);
        slope_lane_at(&mut b, x, dz, n, stone);
        let xf = x as f64 + 0.5;
        mobs.push(cart(xf, 64.1775, 4.1150 + dz as f64, 0.0, true));
        mobs.push(cart(xf, 64.9675, 4.9050 + dz as f64, 0.0, true));
        mobs.push(cart(xf, 65.7575, 5.6950 + dz as f64, 0.0, false));
        mobs.push(cart(xf, (64 + n) as f64, (4.5 + n as f64).max(7.5) + dz as f64, 0.02 * k as f64, arrive_ridden));
    }
    Scene {
        name: name.to_string(),
        chunks: [0, 0, 3, 3],
        blocks: b,
        mobs,
        day_time: 6000,
        difficulty: Difficulty::Normal,
        mob_griefing: true,
        watch: ([0, 62, 0], [63, 72, 63]),
        player: None,
        player_creative: false,
        random_tick_speed: 0,
    }
}

/// (tickexact 2026-11-11, ironspec's pinned-front sweep) R1's column (`file`, its x 0 lane) copied to
/// 101 lanes, 26 to a row 2 apart in x, rows 12 apart in z: in lane k a waiting cart at rest at
/// `wait` (pinned: G0's rest, or P's cart on G_P) and an arrival at `arrive` (mid-slope) set off
/// down the slope (+z) at stored 0.02 k (0 to 2.0); the waiting cart's and the arrival's riders as
/// given.
fn r1_sweep_scene(name: &str, file: &str, wait: [f64; 3], arrive: [f64; 3], wait_ridden: bool, arrive_ridden: bool, raw_p: bool) -> Scene {
    let d = mctas::design::Design::parse(file).expect("ironfarm's contact scene parses");
    let column: Vec<([i32; 3], String)> = d.blocks.iter().filter(|(p, _)| p[0] == 0).map(|(p, s)| (*p, s.clone())).collect();
    let mut b: Vec<([i32; 3], String)> = Vec::new();
    let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
    let cart = |p: [f64; 3], v: f64, ridden: bool| {
        let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
        ("minecraft:minecart".to_string(), p, Some(format!("{{Motion:[0.0d,0.0d,{v:.4}d]{rider}}}")))
    };
    for k in 0..101 {
        let (col, row) = (k % 26, k / 26);
        let (dx, dz) = (1 + 2 * col as i32, 2 + 12 * row as i32 - 26);
        for (p, s) in &column {
            // (P (0,2,31) sits on its redstone block, always on as R1 runs: a scene is placed raw,
            // so its state is written powered)
            let s = if *p == [0, 2, 31] && !raw_p { s.replace("powered=false", "powered=true") } else { s.clone() };
            b.push(([p[0] + dx, p[1] + 64, p[2] + dz], s));
        }
        // (ironspec's raw-placement audit: the column's cut ends, its top rail (0,7,26) and its last
        // (0,0,34), against stone on their high sides as escR1 has them, so no update pops them)
        b.push(([dx, 7 + 64, 25 + dz], "minecraft:stone".to_string()));
        b.push(([dx, 64, 35 + dz], "minecraft:stone".to_string()));
        mobs.push(cart([wait[0] + dx as f64, wait[1] + 64.0, wait[2] + dz as f64], 0.0, wait_ridden));
        mobs.push(cart([arrive[0] + dx as f64, arrive[1] + 64.0, arrive[2] + dz as f64], 0.02 * k as f64, arrive_ridden));
    }
    Scene {
        name: name.to_string(),
        chunks: [0, 0, 3, 2],
        blocks: b,
        mobs,
        day_time: 6000,
        difficulty: Difficulty::Normal,
        mob_griefing: true,
        watch: ([0, 62, 0], [63, 75, 47]),
        player: None,
        player_creative: false,
        random_tick_speed: 0,
    }
}

/// The ledge scenes' stored speeds (b/t): the take-off's measured 1.82..1.96 and a band round it.
pub const LEDGE_SPEEDS: [(&str, f64); 4] = [("150", 1.5), ("182", 1.82), ("196", 1.96), ("220", 2.2)];

/// The ledge scenes' rows (each its own copy of the ledge, 8 apart in z): per row the follower's
/// gap behind the leader in game ticks at the 0.4 cap (None: a lone cart; 0: touching, 0.98 apart)
/// and whether both carts carry a villager.
pub const LEDGE_ROWS: [(Option<f64>, bool); 20] = [
    (None, false),
    (Some(0.0), false),
    (Some(3.0), false),
    (Some(4.0), false),
    (Some(5.0), false),
    (Some(6.0), false),
    (Some(7.0), false),
    (Some(8.0), false),
    (Some(10.0), false),
    (Some(12.0), false),
    (None, true),
    (Some(0.0), true),
    (Some(3.0), true),
    (Some(4.0), true),
    (Some(5.0), true),
    (Some(6.0), true),
    (Some(7.0), true),
    (Some(8.0), true),
    (Some(10.0), true),
    (Some(12.0), true),
];

/// (tickexact 2026-11-11, ironspec: the clean floor printed) the sweep scene's rows: the follower's
/// gap at both edges of the bad band in quarter ticks (3..4 and 6..8 gt), empty, then ridden.
pub fn ledge_sweep_rows() -> Vec<(Option<f64>, bool)> {
    let gaps: Vec<f64> = (12..=16).chain(24..=32).map(|q| q as f64 / 4.0).collect();
    [false, true].iter().flat_map(|r| gaps.iter().map(move |g| (Some(*g), *r))).collect()
}

/// The rows of a ledge scene by name.
pub fn ledge_rows_of(name: &str) -> Vec<(Option<f64>, bool)> {
    if name == "takeoff-clamp" {
        return TAKEOFF_ROWS.iter().map(|(_, r)| (None, *r)).collect();
    }
    if name.starts_with("ledge-r1s-") { ledge_sweep_rows() } else { LEDGE_ROWS.to_vec() }
}

/// (tickexact 2026-11-11, ironspec's take-off clamp) scene takeoff-clamp's rows: a lone cart leaving
/// R1's take-off at each stored speed, ridden (the visible step on a rail is 0.75 x stored for a cart
/// with a passenger: the 0.4 cap from 0.5333) or empty (1.0 x stored: from 0.4); comeOffTrack clamps
/// the stored speed per axis to 0.4 the tick it leaves the rail.
pub const TAKEOFF_ROWS: [(f64, bool); 20] = [
    (1.95, true),
    (1.2, true),
    (0.8, true),
    (0.6, true),
    (0.55, true),
    (0.54, true),
    (0.53, true),
    (0.5, true),
    (0.45, true),
    (0.35, true),
    (0.25, true),
    (1.95, false),
    (1.2, false),
    (0.6, false),
    (0.45, false),
    (0.42, false),
    (0.4, false),
    (0.38, false),
    (0.35, false),
    (0.3, false),
];

/// One ledge scene: the design's cells x -1..9, y 4..13, z 23..29 copied into each row (x + 2,
/// y + 60, z + 8k - 22: row k's take-off line at z 2 + 8k), the corner (9,13,24) and the run north
/// replaced by flats east to x 15 on glass (both carts start on straight flats), the descent's end
/// stopped with stone; per row a leader at x 7.0 (design) moving west at `v` stored, and the row's
/// follower behind it.
fn ledge_scene(tag: &str, v: f64) -> Scene {
    ledge_scene_row(&format!("ledge-r1-{tag}"), v, false, &LEDGE_ROWS)
}

/// [`ledge_scene`] named `name` with `rows`, or with `north` farmopt's (c′) row: the x-run one row
/// north (the take-off flats, the gap, the landing and the slopes at design z 25), the curve (0,8,25)
/// turning straight onto the descent's first slope (0,7,26) (escR1's flat (0,8,25) gone).
fn ledge_scene_row(name: &str, v: f64, north: bool, rows: &[(Option<f64>, bool)]) -> Scene {
    let rows: Vec<(Option<f64>, bool, f64)> = rows.iter().map(|(g, r)| (*g, *r, v)).collect();
    ledge_scene_speeds(name, north, &rows)
}

/// (tickexact 2026-11-11, ironspec's take-off clamp) [`ledge_scene_row`] with each row's own stored
/// speed (`rows`: the follower's gap, ridden, the speed).
fn ledge_scene_speeds(name: &str, north: bool, rows: &[(Option<f64>, bool, f64)]) -> Scene {
    let d = mctas::design::Design::parse(LEDGE_ESCR1).expect("the frozen escR1 parses");
    let mut b: Vec<([i32; 3], String)> = Vec::new();
    // (the rows' extent: row k from z 2 + 8k - 1 to its stop at 2 + 8k + 7)
    let chunks_z = (2 + 8 * rows.len() as i32 + 16) / 16;
    floor(&mut b, 0, 32, 0, 16 * chunks_z, 63, "minecraft:stone");
    let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
    let line = if north { 25 } else { 24 };
    for (k, (gap, ridden, v)) in rows.iter().enumerate() {
        let v = *v;
        let dz = 8 * k as i32 - 22;
        let at = |p: [i32; 3]| [p[0] + 2, p[1] + 60, p[2] + dz];
        for (p, st) in &d.blocks {
            if (-1..=8).contains(&p[0]) && (4..=13).contains(&p[1]) && (24..=27).contains(&p[2]) {
                if north && p[2] == 25 {
                    continue;
                }
                let q = if north && p[2] == 24 { [p[0], p[1], 25] } else { *p };
                b.push((at(q), st.clone()));
            }
        }
        for x in 9..=15 {
            b.push((at([x, 13, line]), "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
            b.push((at([x, 12, line]), "minecraft:glass".to_string()));
        }
        b.push((at([16, 13, line]), "minecraft:stone".to_string()));
        // (the descent's slope (0,6,27) runs onto flats south to z 30, a stone stop at z 31)
        for z in 28..=30 {
            b.push((at([0, 5, z]), "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            b.push((at([0, 4, z]), "minecraft:glass".to_string()));
        }
        b.push((at([0, 5, 31]), "minecraft:stone".to_string()));
        let cart = |x: f64| {
            let nbt = if *ridden {
                format!("{{Motion:[{:.4}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", -v)
            } else {
                format!("{{Motion:[{:.4}d,0.0d,0.0d]}}", -v)
            };
            let p = at([0, 13, line]);
            ("minecraft:minecart".to_string(), [x + 2.0, p[1] as f64, p[2] as f64 + 0.5], Some(nbt))
        };
        mobs.push(cart(7.0));
        if let Some(g) = gap {
            let behind = if *g == 0.0 { 0.98 } else { 0.4 * *g };
            mobs.push(cart(7.0 + behind));
        }
    }
    Scene {
        name: name.to_string(),
        chunks: [0, 0, 1, chunks_z - 1],
        blocks: b,
        mobs,
        day_time: 6000,
        difficulty: Difficulty::Normal,
        mob_griefing: true,
        watch: ([0, 62, 0], [31, 75, 16 * chunks_z - 1]),
        player: None,
        player_creative: false,
        random_tick_speed: 0,
    }
}

/// The carousel loop's design, parsed.
fn carousel_loop_design() -> mctas::design::Design {
    mctas::design::Design::parse(CAROUSEL_LOOP).expect("the frozen carousel loop design parses")
}

/// The loop's slot controls (each braked slope's support: glass a brake, a redstone block go), in
/// sandbox coordinates: the queue's 18 slots in travel order (row A at design z 0 going -x: slot
/// 0 at its east end; then row B at z 2 going +x up to slot 17), and the stand's 3 (row B east of
/// the release run, travel order).
fn carousel_loop_controls() -> (Vec<[i32; 3]>, Vec<[i32; 3]>) {
    let d = carousel_loop_design();
    let mut row_a = Vec::new();
    let mut row_b = Vec::new();
    for (p, st) in &d.blocks {
        if st.starts_with("minecraft:powered_rail[powered=false,shape=ascending") {
            let c = [p[0] + CAROUSEL_SHIFT[0], p[1] - 1 + CAROUSEL_SHIFT[1], p[2] + CAROUSEL_SHIFT[2]];
            if p[2] == 0 { row_a.push(c) } else { row_b.push(c) }
        }
    }
    row_a.sort_by_key(|c| -c[0]);
    row_b.sort_by_key(|c| c[0]);
    let stand = row_b.split_off(9);
    let mut slots = row_a;
    slots.extend(row_b);
    (slots, stand)
}

/// ironfarm's per-slot schedule (`queue-loop-run`'s ideal controls) for two slots from t0 = 100
/// and 200, without its position triggers: the release group (slots 15..17) and the stand powered
/// at t0 (both trains leave), the release group braked at t0 + 20; the advance's three 9-gt pulses
/// on every queue slot at t0 + 40, 60, 80 (the register's one-slot moves, the U-turn's chaining).
fn carousel_loop_actions() -> Vec<(u32, Vec<([i32; 3], String)>)> {
    let (slots, stand) = carousel_loop_controls();
    let set = |cs: &[[i32; 3]], on: bool| cs.iter().map(|c| (*c, if on { "minecraft:redstone_block" } else { "minecraft:glass" }.to_string())).collect::<Vec<_>>();
    let release = &slots[15..18];
    let mut v = Vec::new();
    for t0 in [100u32, 200] {
        v.push((t0, [set(release, true), set(&stand, true)].concat()));
        v.push((t0 + 20, set(release, false)));
        for k in 0..3 {
            v.push((t0 + 40 + 20 * k, set(&slots, true)));
            v.push((t0 + 49 + 20 * k, set(&slots, false)));
        }
    }
    v
}

/// (tickexact 2026-11-10) Loop scenes driven by a bench's drive log: (name, the frozen design, the
/// frozen drive log). Each design is taken as it is (its blocks and entities, in its own
/// coordinates); its bench's drive log (`mctas mobfarm queue-loop-run <file> ... --drive-out`)
/// plays its `act T set` lines at their ticks. What drives each:
/// - carousel-catch: ironfarm's loop, explore/ironfarm 7f6c75643 (the turn slot, the deep tail
///   slots, the detector rail), `--cycles 6 --stand-detect 28 --intake-brake 110,116`: the bench's
///   timers and the stand's brake it timed from the design's real detector rail in its redsim run
///   (the game's detector compared in the watched box); no other redstone.
/// - carousel-catch2: explore/ironfarm ab96cd355 (8 release-run boosters, 7 zombies in boats by
///   the loop), `--cycles 6 --release-for 36 --pulse 12 --stand-detect 27 --intake-brake 116,124
///   --stand-offset 6 --tail0-late 2`: the same kind of scripted controls.
/// - carousel-wired: day-1's fully wired loop (main 74d7e62f0,
///   `designs/farms/iron/parts/carousel-loop/carousel-single-lane-loop-wired.txt`, the same file as
///   explore/day1-sched-wip d9b20f773's: ironfarm's a5f46f390 loop, day-1's release controller and
///   its 21 control lines, 791 redstone blocks; 8 zombies in boats), `--wired=-7,-1,5
///   --tail-check-at 43 --cycles 60`: WIRED, the only bench input the golem pulse (a redstone block
///   at (-7,-1,5) for 10 gt at each slot's start); every control is the design's own redstone.
/// - siding-raw / siding-built: farmopt's bed siding with its give-up (explore/farmopt-station
///   ee139b64b, `carousel-siding-wired.txt`, written settled: an entry detector rail, an arrival
///   latch, day-1's 220 gt comparator timer), placed as written / built in order, its
///   `siding-bench --wired --launch --from 1 --to 1 --nights 1` drive logs: WIRED (the siding's
///   own redstone switches the T, the activator, the hatch and the kick); the bench launches each
///   cart from rest on its launcher and moves it back to its hold after the exit (`tp`) and places
///   the release as a redstone block for 10 gt. The one bed's shared claim is the design's (`port
///   home`, R1's end state: [`scene_home`]): every villager loads HOME = the bed as data, no
///   ticket taken on either side.
/// - carousel-wired-v3 / carousel-wired-v3-built: day-1's published wired loop (explore/day1-counter
///   b808c2125: v2 plus 31 glass covers over the off-pad golem spawn cells), placed as written /
///   built in order, ironfarm's `queue-loop-run --wired=-7,-1,5 --tail-check-at 43 --cycles 60
///   --placed-raw | --placed-in-order` drive logs: WIRED, the golem pulse the only input.
/// - station-raw / station-built: farmopt's linear stand with its Nether part
///   (`carousel-linear-stand.txt`), its `linear-bench --from 1 --to 1 --days 1 --lead 60
///   --placed-raw | --placed-in-order` drive logs (bench-driven: the batches teleported to the
///   stand). The bridge's Nether sandbox (`nether_create`: its own level under the Nether's key,
///   its chunks loaded but not simulated until a crossing's PORTAL ticket) and redsim's Nether
///   part ([`scene_nether`]) take the design's `nether` lines (built in order: the drive's
///   `nether-set` / `nether-nbt` acts); each frame carries the Nether's. Exact to t2470 placed as
///   written (24 golems through the portal, their Nether ticks, those angry at a zombie post
///   chasing it from the Nether: `target::Who::Foreign`) and through the build to t9704 built in
///   order; then a zombie post's MoveThroughVillageGoal finds the village close in the game after
///   the bed's release (the POI manager's stored level 6, mob-AI's) and not in redsim.
/// - unload-v3-raw / unload-siding-raw: ironspec's fault rule (a player leaving and coming back)
///   on day-1's wired loop v3 and farmopt's bed siding, placed as written: their drives (the
///   benches' acts, expects dropped) with `unload` / `load` acts added: mid-cycle (30 and 40 gt
///   away), mid-pulse (3 and 5 gt), N = 0 mid-pulse, and the siding's 240 gt (past its 220 gt
///   timer). Every chunk saved and unloaded (the bridge's `unload`: the level ticks on), loaded
///   again (`load`: the entities back as new objects, numbered in the chunks' order; the drive's
///   `tp eN` follow them, `World::renumbered`); redsim's `reload::unload_all` / `load_all`.
/// - unload-part-v3-raw: the partial form on the wired loop v3 (`unload x0,z0,x1,z1 N`: a chunk
///   box's forced tickets dropped, the edge of the loaded area through the part): the east column
///   (simulation level 32: blocks tick, entities do not) 30 gt mid-cycle, the north chunks (32 and
///   33) 5 gt mid-pulse, N = 0, and 60 gt mid-pulse across the pulse's end with the middle at 33;
///   the bridge re-forces the chunks one by one so their entities rejoin the tick list in the box's
///   order (`Sandbox.loadSome`), as redsim's `reload::load_some`.
const DRIVE_SCENES: &[(&str, &str, &str)] = &[
    ("carousel-catch", include_str!("../scenes/carousel-catch.txt"), include_str!("../scenes/carousel-catch-28.drive")),
    ("carousel-catch2", include_str!("../scenes/carousel-catch2.txt"), include_str!("../scenes/carousel-catch2-27.drive")),
    ("carousel-wired", include_str!("../scenes/carousel-wired.txt"), include_str!("../scenes/carousel-wired.drive")),
    ("carousel-wired-raw", include_str!("../scenes/carousel-wired.txt"), include_str!("../scenes/carousel-wired-raw.drive")),
    ("carousel-wired-v2", include_str!("../scenes/carousel-wired-v2.txt"), include_str!("../scenes/carousel-wired-v2-raw.drive")),
    ("carousel-wired-built", include_str!("../scenes/carousel-wired-v2.txt"), include_str!("../scenes/carousel-wired-v2-built.drive")),
    ("carousel-wired-v3", include_str!("../scenes/carousel-wired-v3.txt"), include_str!("../scenes/carousel-wired-v3-raw.drive")),
    ("carousel-wired-v3-built", include_str!("../scenes/carousel-wired-v3.txt"), include_str!("../scenes/carousel-wired-v3-built.drive")),
    ("siding-raw", include_str!("../scenes/siding-wired.txt"), include_str!("../scenes/siding-wired-raw.drive")),
    ("siding-built", include_str!("../scenes/siding-wired.txt"), include_str!("../scenes/siding-wired-built.drive")),
    ("station-raw", include_str!("../scenes/station-linear-stand.txt"), include_str!("../scenes/station-linear-stand-raw.drive")),
    ("station-built", include_str!("../scenes/station-linear-stand.txt"), include_str!("../scenes/station-linear-stand-built.drive")),
    ("unload-v3-raw", include_str!("../scenes/carousel-wired-v3.txt"), include_str!("../scenes/unload-v3-raw.drive")),
    ("unload-siding-raw", include_str!("../scenes/siding-wired.txt"), include_str!("../scenes/unload-siding-raw.drive")),
    ("unload-part-v3-raw", include_str!("../scenes/carousel-wired-v3.txt"), include_str!("../scenes/unload-part-v3-raw.drive")),
    ("carousel-wired-pocket", include_str!("../scenes/carousel-wired-pocket.txt"), include_str!("../scenes/carousel-wired-pocket.drive")),
    // (tickexact 2026-11-10, 58f770cc's siding unload cases: main 1ae586523's
    // carousel-siding-wired.txt and its three fault drives (`siding-bench --wired --nights 1
    // --release-held --visits 6 --unload ..`, launched arrivals, visit 3): the whole part away 3 gt at
    // the kick (seed 4), 700 gt at the sleep onset +10 (seed 1), and chunks x 0, z -1..0 at level
    // 32 for 40 gt at the sleep onset (seed 1; the sandbox's box forced, the bench's hold chunk
    // z -2 included))
    ("siding-fault-kick3", include_str!("../scenes/siding-fault.txt"), include_str!("../scenes/siding-fault-whole-kick-3-seed4.drive")),
    ("siding-fault-asleep700", include_str!("../scenes/siding-fault.txt"), include_str!("../scenes/siding-fault-whole-asleep-700-seed1.drive")),
    ("siding-fault-nap40", include_str!("../scenes/siding-fault.txt"), include_str!("../scenes/siding-fault-partial-x-hi-32-nap-40-seed1.drive")),
    // (tickexact 2026-11-11, ironfarm's escG6L admissions: its tail-bench `--scene-out` (the
    // design reduced to the bench's tail carts with their riders and the 8 zombie posts) and
    // `--drive-out`, 2 cycles: per advance (+38/+58/+78) the slot controls 9 gt, G_P (0,2,32) set
    // open at +6 for 2 gt, G0 (0,3,31) open 3 gt from G_P's +3, P's redstone block (0,1,31) once)
    ("gate-escg6l", include_str!("../scenes/gate-escg6l.txt"), include_str!("../scenes/gate-escg6l.drive")),
    // (ironfarm's escG6L-v3: the same cells, G0 at G_P +2 for 4 gt at advances 1 and 2, at G_P +10
    // for 4 gt at advance 3 (the binding case); one full cycle from the scene's 4 tail carts)
    ("gate-escg6l-v3", include_str!("../scenes/gate-escg6l-v3.txt"), include_str!("../scenes/gate-escg6l-v3.drive")),
    // (ironfarm's longer escG6L-v3 scene: 6 tail carts with riders as its tail bench had them at
    // cycle 6's start (positions, motions, facings), G_P 2 gt at advance +6, G0 at G_P +4 for 6 gt
    // on advances 1 and 2 and at G_P +10 for 6 gt on advance 3, over 3 cycles)
    ("gate-escg6l-v4", include_str!("../scenes/gate-escg6l-v4.txt"), include_str!("../scenes/gate-escg6l-v4.drive")),
    // (tickexact 2026-11-11, 58f770cc's plate: main's faults/carousel-siding-wired-plate.txt, a
    // stone pressure plate (1,2,1) on the glass ceiling over the booster rail (1,0,1), and its three
    // drives (`siding-bench --wired --launch --nights 1 --drive-out`): a normal night (seed 1), a
    // day (seed 1), the whole part away 3 gt at visit 3's kick (seed 4))
    ("siding-plate-normal", include_str!("../scenes/siding-plate.txt"), include_str!("../scenes/siding-plate-normal-seed1.drive")),
    ("siding-plate-day", include_str!("../scenes/siding-plate.txt"), include_str!("../scenes/siding-plate-day-seed1.drive")),
    ("siding-plate-kick3", include_str!("../scenes/siding-plate.txt"), include_str!("../scenes/siding-plate-whole-kick-3-seed4.drive")),
    // (tickexact 2026-11-11, ironspec's night-time sleep failure with the bed free: 58f770cc's two
    // drives on explore/farmopt-station 16ba69af8's member file carousel-siding-wired.txt, seed 1,
    // the fault on visit 3: the activator a plain rail 2729..3356 (the napper stays aboard and parks
    // with its cart), or the bed set occupied with nobody in it 2759..3355)
    ("siding-sleepfail-nodrop3", include_str!("../scenes/siding-member.txt"), include_str!("../scenes/siding-no-drop-3-seed1.drive")),
    ("siding-sleepfail-bedtaken3", include_str!("../scenes/siding-member.txt"), include_str!("../scenes/siding-bed-taken-3-seed1.drive")),
];

/// (tickexact 2026-11-10, ironspec's chunk alignment) Drive scenes moved in the sandbox, with
/// extra acts: (name, the drive scene it copies, the shift in blocks, the extra drive lines). The
/// carousel's final alignment puts the loop frame's cell 0, 0 at X & 15 = 6, Z & 15 = 6, its chunk
/// lines at x -7|-6, 9|10, -23|-22 and z -7|-6, 9|10, 25|26, 41|42: the lean loop moved by +6, 0,
/// +6, with partial unloads at those lines (`act T unload x0,z0,x1,z1 N` in the sandbox's chunks,
/// sandbox = frame + 6; its box is chunks x -5..2, z -2..3):
/// - t1250 for 30 gt, chunk row z 1 (frame z 10..25): the stand's release run cut at z 9|10 at
///   level 32 mid-release (the slot's golem pulse at t1240), the queue's z 25|26 line too;
/// - t2080 for 60 gt, row z 2 (frame z 26..41): the queue's z 25|26 and 41|42 lines at level 32
///   across its advance pulses;
/// - t3200 for 200 gt, column x -2 (frame x -38..-23): the siding's x -23|-22 line at level 32
///   while slot 30's visitor sleeps in the bed;
/// - t5040 for 100 gt, rows z 0..2 (frame z -6..41): rows 0 and 2 at level 32, row 1 at 33 (the
///   stand and the queue's south end loaded but frozen) for a whole slot.
const MOVED_SCENES: &[(&str, &str, [i32; 3], &str)] = &[
    (
        "carousel-wired-pocket-a6",
        "carousel-wired-pocket",
        [6, 0, 6],
        "act 1250 unload -5,1,2,1 30\nact 2080 unload -5,2,2,2 60\nact 3200 unload -2,-2,-2,3 200\nact 5040 unload -5,0,2,2 100\n",
    ),
    // (tickexact 2026-11-10, day-1's question: the whole part unloaded during a k2 visit, slot 38's
    // (the bench's slot k from 40 + 100 k: its 40 settle ticks first; the napper asleep
    // +203..+234, the k2 cart leaving the siding at +386): at visit +250 for 40 gt and at +280
    // for 15 gt, both back before the next golem pulse)
    ("carousel-wired-pocket-u250", "carousel-wired-pocket", [0, 0, 0], "act 4090 unload all 40\n"),
    ("carousel-wired-pocket-u280", "carousel-wired-pocket", [0, 0, 0], "act 4120 unload all 15\n"),
    // (and away 4,000 gt, the golem pulses while away dropped (`drop T0 T1`: the base's acts
    // strictly inside): at visit +250 (the napper just woke, walking back), +280 (waiting in the
    // siding), +392 (the k2 cart moving from the siding's exit toward its hold, the napper aboard))
    ("carousel-wired-pocket-u250l", "carousel-wired-pocket", [0, 0, 0], "act 4090 unload all 4000\ndrop 4090 8090\n"),
    ("carousel-wired-pocket-u280l", "carousel-wired-pocket", [0, 0, 0], "act 4120 unload all 4000\ndrop 4120 8120\n"),
    ("carousel-wired-pocket-u392l", "carousel-wired-pocket", [0, 0, 0], "act 4232 unload all 4000\ndrop 4232 8232\n"),
    // (tickexact 2026-11-11, the reload-order rule) the u280l unload with the load's entities in
    // another order: the whole saved sequence reversed; shuffled by seed:K (chunks, then each
    // chunk's vehicles-with-passengers)
    ("carousel-wired-pocket-u280l-rev", "carousel-wired-pocket", [0, 0, 0], "act 4120 unload all 4000 order=reverse\ndrop 4120 8120\n"),
    ("carousel-wired-pocket-u280l-s3", "carousel-wired-pocket", [0, 0, 0], "act 4120 unload all 4000 order=seed:3\ndrop 4120 8120\n"),
    // (tickexact 2026-11-11, (a)'s rate in the game: `mobai unload-rate`) 58f770cc's kick drive
    // without its own unload: the scene as it runs, the unload placed at each seed's kick
    ("siding-fault-kick3-base", "siding-fault-kick3", [0, 0, 0], "nounload\n"),
];

/// A moved scene's copy: (the drive scene, the shift, the extra drive lines).
fn moved(name: &str) -> Option<(&'static str, [i32; 3], &'static str)> {
    MOVED_SCENES.iter().find(|(n, ..)| *n == name).map(|(_, b, s, x)| (*b, *s, *x))
}

/// Every drive scene's name: the frozen ones, then the moved copies.
fn drive_scene_names() -> Vec<&'static str> {
    DRIVE_SCENES.iter().map(|(n, _, _)| *n).chain(MOVED_SCENES.iter().map(|(n, ..)| *n)).collect()
}

/// A design moved by `s` (its blocks, block entities, ports and entities).
fn shift_design(mut d: mctas::design::Design, s: [i32; 3]) -> mctas::design::Design {
    let add = |p: [i32; 3]| [p[0] + s[0], p[1] + s[1], p[2] + s[2]];
    d.blocks = d.blocks.into_iter().map(|(p, st)| (add(p), st)).collect();
    d.block_entities = d.block_entities.into_iter().map(|(p, n)| (add(p), n)).collect();
    for v in d.ports.values_mut() {
        *v = add(*v);
    }
    for e in &mut d.entities {
        e.pos = [e.pos[0] + s[0] as f64, e.pos[1] + s[1] as f64, e.pos[2] + s[2] as f64];
    }
    d
}

/// A drive log moved by `s` (its block acts' cells, its teleports' and expected positions), with
/// `extra` act lines merged in by tick.
fn shift_drive(text: &str, s: [i32; 3], extra: &str) -> String {
    let mut head = Vec::new();
    let mut acts: Vec<(u32, String)> = Vec::new();
    let mut expects = Vec::new();
    let moved_ints = |f: &[&str], at: usize| -> String {
        let mut g: Vec<String> = f.iter().map(|x| x.to_string()).collect();
        for k in 0..3 {
            let v: i32 = g[at + k].trim().parse().unwrap_or_else(|_| panic!("a drive line's cell: {f:?}"));
            g[at + k] = (v + s[k]).to_string();
        }
        g.join(" ")
    };
    let moved_floats = |f: &[&str], at: usize| -> String {
        let mut g: Vec<String> = f.iter().map(|x| x.to_string()).collect();
        for k in 0..3 {
            let v: f64 = g[at + k].trim().parse().unwrap_or_else(|_| panic!("a drive line's position: {f:?}"));
            g[at + k] = format!("{}", v + s[k] as f64);
        }
        g.join(" ")
    };
    let nounload = extra.lines().any(|l| l.trim() == "nounload");
    for line in text.lines() {
        if line.starts_with("expect ") {
            let f: Vec<&str> = line.split(' ').collect();
            expects.push(if f.len() == 7 && f[2] == "pos" { moved_floats(&f, 4) } else { line.to_string() });
            continue;
        }
        if !line.starts_with("act ") {
            head.push(line.to_string());
            continue;
        }
        let f: Vec<&str> = line.splitn(7, ' ').collect();
        let t: u32 = f[1].parse().unwrap_or_else(|_| panic!("a drive line's tick: {line:?}"));
        // (`nounload`: the base's own unload acts left out: the scene as it runs without them)
        if nounload && matches!(f.get(2).copied(), Some("unload") | Some("load")) {
            continue;
        }
        let l = match f.get(2).copied() {
            Some("set") | Some("nbt") if f.len() == 7 => moved_ints(&f, 3),
            Some("tp") if f.len() == 7 => moved_floats(&f, 4),
            Some("home") if f.len() == 7 => moved_ints(&f, 4),
            Some("poi-take") if f.len() == 6 => moved_ints(&f, 3),
            Some("unload") | Some("load") | Some("nether-set") | Some("nether-nbt") if s != [0, 0, 0] => panic!("a moved drive scene's base holds {line:?}: chunk boxes and Nether cells do not move"),
            _ => line.to_string(),
        };
        acts.push((t, l));
    }
    for line in extra.lines().filter(|l| l.starts_with("act ")) {
        let t: u32 = line.split(' ').nth(1).and_then(|x| x.parse().ok()).unwrap_or_else(|| panic!("an extra drive line's tick: {line:?}"));
        acts.push((t, line.to_string()));
    }
    // (`drop T0 T1`: the base's acts strictly between T0 and T1 left out: a bench's controls while
    // its player is away)
    for line in extra.lines().filter(|l| l.starts_with("drop ")) {
        let w: Vec<u32> = line.split(' ').skip(1).filter_map(|x| x.parse().ok()).collect();
        let [t0, t1] = w[..] else { panic!("an extra drive line: {line:?} is not `drop T0 T1`") };
        acts.retain(|(t, l)| !(*t > t0 && *t < t1 && !l.contains(" unload") && !l.contains(" load")));
    }
    acts.sort_by_key(|(t, _)| *t);
    let mut out = head;
    out.extend(acts.into_iter().map(|(_, l)| l));
    out.extend(expects);
    out.join("\n") + "\n"
}

/// A drive scene's day time at its spawn in its bench's run: the bench's world is
/// `RunOpts::steady(seed, ..)` of the drive log's `seed` line (its `day0`).
fn drive_day0(name: &str) -> Option<i64> {
    let t = drive_text(name)?;
    // (`seed N`, or `# seed N` as farmopt's benches write it)
    let seed: i64 = t.lines().find_map(|l| l.trim_start_matches('#').trim_start().strip_prefix("seed "))?.trim().parse().ok()?;
    Some(redsim::entity::mob::farm::RunOpts::steady(seed, 0, redsim::entity::mob::farm::WalkMode::Random).day0)
}

/// A drive scene's design (a moved scene's: its base's, moved).
fn drive_design(name: &str) -> Option<mctas::design::Design> {
    if let Some((base, s, _)) = moved(name) {
        return drive_design(base).map(|d| shift_design(d, s));
    }
    let (_, d, _) = DRIVE_SCENES.iter().find(|(n, _, _)| *n == name)?;
    Some(mctas::design::Design::parse(d).unwrap_or_else(|e| panic!("the frozen design of {name}: {e}")))
}

/// (tickexact 2026-11-10, farmopt's siding since main 7f8152452) A drive scene's `port home`: R1's
/// one shared bed. Every villager of the design holds HOME = the bed, loaded as data as
/// circuit::studies::mobfarm writes it ([`with_home`]): no POI ticket taken, on either side (a
/// load takes none; redsim's AI world starts with every ticket free).
fn scene_home(name: &str) -> Option<[i32; 3]> {
    drive_design(name)?.ports.get("home").copied()
}

/// A design entity's data with HOME = `h` for a villager (its own compound, or a villager
/// passenger's: the carts' riders), `Brain:{memories:{"minecraft:home":..}}` as a load reads it.
fn with_home(kind: &str, nbt: Option<String>, h: [i32; 3]) -> Option<String> {
    let brain = format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}", h[0], h[1], h[2]);
    if kind.trim_start_matches("minecraft:") == "villager" {
        return Some(match nbt.as_deref().map(str::trim) {
            Some(n) if n.len() > 2 => format!("{},{brain}}}", n.strip_suffix('}').unwrap_or(n)),
            _ => format!("{{{brain}}}"),
        });
    }
    nbt.map(|n| n.replace("{id:\"minecraft:villager\"}", &format!("{{id:\"minecraft:villager\",{brain}}}")))
}

/// A drive scene's drive log: the frozen one, or `MOBAI_DRIVE=<file>` (another drive log of the
/// same design: the catch's tick window, `--stand-detect` 26, 27, 29...).
fn drive_text(name: &str) -> Option<String> {
    if let Some((base, s, extra)) = moved(name) {
        return drive_text(base).map(|t| shift_drive(&t, s, extra));
    }
    let (_, _, t) = DRIVE_SCENES.iter().find(|(n, _, _)| *n == name)?;
    Some(std::env::var("MOBAI_DRIVE").ok().map(|p| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("MOBAI_DRIVE {p}: {e}"))).unwrap_or_else(|| t.to_string()))
}

/// The drive log's `act T set x y z <state>` lines, grouped by tick (T = ticks after the spawn,
/// applied before the next tick, as the fixture applies its actions). Any other `act` (a bench
/// `tp`) is refused. The design sits in its own coordinates (the bench's: cart physics in floating
/// point is not translation-invariant, so a knife-edge catch exact in one place may land a hair
/// apart in another; the bench's expectations hold only where it ran).
fn drive_actions(name: &str) -> Option<Vec<(u32, Vec<([i32; 3], String)>)>> {
    let text = drive_text(name)?;
    let mut v: Vec<(u32, Vec<([i32; 3], String)>)> = Vec::new();
    let mut later: Vec<(u32, ([i32; 3], String))> = Vec::new();
    let push = |v: &mut Vec<(u32, Vec<([i32; 3], String)>)>, t: u32, c: ([i32; 3], String)| match v.last_mut() {
        Some((lt, b)) if *lt == t => b.push(c),
        _ => v.push((t, vec![c])),
    };
    let bad = |line: &str| -> ! { panic!("the drive log of {name}: {line:?} is not `act T set|nbt x y z <state|snbt>`, `tp eN x y z`, `home eN.pK x y z`, `last-slept eN.pK t`, `poi-take x y z`, `unload`, `load` or `unload all N`") };
    for line in text.lines().filter(|l| l.starts_with("act ")) {
        let f: Vec<&str> = line.splitn(7, ' ').collect();
        let int = |i: usize| f.get(i).and_then(|x| x.trim().parse::<i32>().ok());
        let Some(t) = f.get(1).and_then(|x| x.parse::<u32>().ok()) else { bad(line) };
        // (block acts as states; a container's fill as `nbt:<snbt>` (the game's data merge); a
        // bench's entity acts as `tp:` / `home:` / `last-slept:` / `poi-take` (redsim's
        // `drive::Act`, the bridge's `teleport` / `memory` / `poi_take`): [`apply_redsim`],
        // [`GameScene::apply`])
        let c = match f.get(2).copied() {
            Some("set") | Some("nbt") if f.len() == 7 => {
                let (Some(x), Some(y), Some(z)) = (int(3), int(4), int(5)) else { bad(line) };
                ([x, y, z], if f[2] == "nbt" { format!("nbt:{}", f[6].trim()) } else { f[6].trim().to_string() })
            }
            Some("tp") if f.len() == 7 => ([0, 0, 0], format!("tp:{}:{}:{}:{}", f[3], f[4], f[5], f[6].trim())),
            Some("home") if f.len() == 7 => {
                let (Some(x), Some(y), Some(z)) = (int(4), int(5), int(6)) else { bad(line) };
                ([x, y, z], format!("home:{}", f[3]))
            }
            Some("last-slept") if f.len() == 5 => ([0, 0, 0], format!("last-slept:{}:{}", f[3], f[4].trim())),
            Some("poi-take") if f.len() == 6 => {
                let (Some(x), Some(y), Some(z)) = (int(3), int(4), int(5)) else { bad(line) };
                ([x, y, z], "poi-take".to_string())
            }
            // (farmopt's station) its Nether part's build, in the file's Nether coordinates
            // (anchor 0 0 0): redsim's Nether only ([`apply_redsim`])
            Some("nether-set") | Some("nether-nbt") if f.len() == 7 => {
                let (Some(x), Some(y), Some(z)) = (int(3), int(4), int(5)) else { bad(line) };
                ([x, y, z], format!("{}:{}", if f[2] == "nether-set" { "nether" } else { "nether-nbt" }, f[6].trim()))
            }
            // (tickexact 2026-11-10, ironspec's fault rule) every chunk of the scene saved and
            // unloaded (a player leaving), then loaded again (back): the bridge's `unload` /
            // `load`, redsim's `reload::unload_all` / `load_all`
            Some(a @ ("unload" | "load")) if f.len() == 3 => ([0, 0, 0], a.to_string()),
            // (tickexact 2026-11-11, the reload-order rule) `load order=save|reverse|seed:K`
            Some("load") if f.len() == 4 && f[3].trim().starts_with("order=") => {
                let o = f[3].trim().trim_start_matches("order=");
                if redsim::reload::Order::parse(o).is_none() {
                    bad(line)
                }
                ([0, 0, 0], if o == "save" { "load".to_string() } else { format!("load@{o}") })
            }
            // (the brief's shared act: `unload all <N> [order=..]`, the load N ticks later)
            Some("unload") if (f.len() == 5 || f.len() == 6) && f[3] == "all" => {
                let Some(n) = f[4].trim().parse::<u32>().ok() else { bad(line) };
                let load = match f.get(5).map(|o| o.trim().trim_start_matches("order=")) {
                    None | Some("save") => "load".to_string(),
                    Some(o) if redsim::reload::Order::parse(o).is_some() && f[5].trim().starts_with("order=") => format!("load@{o}"),
                    Some(_) => bad(line),
                };
                later.push((t + n, ([0, 0, 0], load)));
                later.sort_by_key(|(t, _)| *t);
                ([0, 0, 0], "unload".to_string())
            }
            // (its partial form: `unload x0,z0,x1,z1 <N>`, a chunk box's forced tickets dropped,
            // back N ticks later; or the halves `unload x0,z0,x1,z1` / `load x0,z0,x1,z1`)
            Some("unload") if f.len() == 5 => {
                let (Some(b), Some(n)) = (redsim::entity::mob::drive::chunk_box(f[3]), f[4].trim().parse::<u32>().ok()) else { bad(line) };
                let s = format!("{},{},{},{}", b[0], b[1], b[2], b[3]);
                later.push((t + n, ([0, 0, 0], format!("load:{s}"))));
                later.sort_by_key(|(t, _)| *t);
                ([0, 0, 0], format!("unload:{s}"))
            }
            Some(a @ ("unload" | "load")) if f.len() == 4 => {
                let Some(b) = redsim::entity::mob::drive::chunk_box(f[3].trim()) else { bad(line) };
                ([0, 0, 0], format!("{a}:{},{},{},{}", b[0], b[1], b[2], b[3]))
            }
            _ => bad(line),
        };
        // (an act an earlier line put at or before this line's time goes in first, as redsim's
        // drive sorts them)
        while let Some(i) = later.iter().position(|(lt, _)| *lt <= t) {
            let (lt, c) = later.remove(i);
            push(&mut v, lt, c);
        }
        push(&mut v, t, c);
    }
    later.sort_by_key(|(t, _)| *t);
    for (t, c) in later {
        push(&mut v, t, c);
    }
    Some(v)
}

/// (tickexact 2026-11-10) What a drive's entity acts need on both sides: line N's entity number
/// (its vehicle, in summon order), each line's passengers in order (`eN.pK`), and the game time of
/// the drive's origin (`last-slept eN.pK t'`: `t0 + t'`).
#[derive(Clone, Debug, Default)]
pub struct ActCtx {
    pub lines: Vec<u32>,
    pub riders: Vec<Vec<u32>>,
    pub g0: i64,
}

impl ActCtx {
    /// From redsim's world right after the summon (`ids`: the summon's entity numbers in order).
    fn of(w: &World, ids: &[u64], g0: i64) -> ActCtx {
        let mut c = ActCtx { g0, ..Default::default() };
        for &id in ids {
            if let Some(e) = w.entities.list.iter().find(|e| e.id as u64 == id)
                && e.vehicle.is_none()
            {
                c.lines.push(e.id);
                c.riders.push(e.passengers.clone());
            }
        }
        c
    }

    /// The entity number of a drive's `eN` / `eN.pK`.
    fn number(&self, who: redsim::entity::mob::drive::Who) -> Result<u32> {
        use redsim::entity::mob::drive::Who;
        match who {
            Who::Line(n) => self.lines.get(n).copied(),
            Who::Rider(n, k) => self.riders.get(n).and_then(|r| r.get(k)).copied(),
            // (an id: redsim's, the bridge's number for the same entity)
            Who::Id(id) => Some(id),
        }
        .ok_or_else(|| anyhow!("the drive names {who:?}: no such entity in the scene"))
    }
}

/// A drive's `eN` / `eN.pK`.
fn parse_who(s: &str) -> Result<redsim::entity::mob::drive::Who> {
    use redsim::entity::mob::drive::Who;
    let bad = || anyhow!("{s:?}: eN, eN.pK or idN");
    if let Some(n) = s.strip_prefix("id") {
        return Ok(Who::Id(n.parse().map_err(|_| bad())?));
    }
    let s = s.strip_prefix('e').ok_or_else(bad)?;
    Ok(match s.split_once(".p") {
        Some((n, k)) => Who::Rider(n.parse().map_err(|_| bad())?, k.parse().map_err(|_| bad())?),
        None => Who::Line(s.parse().map_err(|_| bad())?),
    })
}

/// An action that is one of a bench's entity acts (`tp:` / `home:` / `last-slept:` / `poi-take`),
/// as redsim's drive act.
fn entity_act(p: [i32; 3], s: &str) -> Result<Option<redsim::entity::mob::drive::Act>> {
    use redsim::entity::mob::drive::Act;
    let f: Vec<&str> = s.split(':').collect();
    let num = |i: usize| -> Result<f64> { f.get(i).and_then(|x| x.parse().ok()).ok_or_else(|| anyhow!("{s:?}: a number")) };
    let pos = BlockPos::new(p[0], p[1], p[2]);
    Ok(Some(match f[0] {
        "tp" => Act::Tp(parse_who(f.get(1).copied().unwrap_or(""))?, redsim::entity::Vec3::new(num(2)?, num(3)?, num(4)?)),
        "home" => Act::Home(parse_who(f.get(1).copied().unwrap_or(""))?, pos),
        "last-slept" => Act::LastSlept(parse_who(f.get(1).copied().unwrap_or(""))?, num(2)? as i64),
        "poi-take" => Act::PoiTake(pos),
        "unhome" => Act::Unhome(parse_who(f.get(1).copied().unwrap_or(""))?),
        "forget" => Act::Forget(parse_who(f.get(1).copied().unwrap_or(""))?),
        _ => return Ok(None),
    }))
}

/// A built-in-order drive log's spawn tick (`# spawn S`, counted from the build's start).
fn drive_spawn(name: &str) -> Option<u32> {
    drive_text(name)?.lines().find_map(|l| l.strip_prefix("# spawn "))?.trim().parse().ok()
}

/// The drive log's `expect T pos eK x y z` lines (line K's entity at tick T, the bench's redsim
/// run): (T, the entity's number in the scene, its position). Each entity line is numbered in
/// summon order with its passengers after it (one per `id:"` in its data). The fixture checks the
/// game's frames against them: the scripted replay reproduces the bench's run (a positive control
/// on the drive log's timing).
fn drive_expects(name: &str) -> Vec<(i64, u32, [f64; 3])> {
    let (Some(d), Some(text)) = (drive_design(name), drive_text(name)) else { return Vec::new() };
    let mut lines = Vec::new();
    let mut n = 0u32;
    for e in &d.entities {
        lines.push(n);
        n += 1 + e.nbt.as_deref().map(|s| s.matches("id:\"").count() as u32).unwrap_or(0);
    }
    text.lines()
        .filter(|l| l.starts_with("expect "))
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() != 7 || f[2] != "pos" {
                return None;
            }
            let p = [f[4].parse::<f64>().ok()?, f[5].parse::<f64>().ok()?, f[6].parse::<f64>().ok()?];
            // (`idN`: an entity by its id, redsim's and the bridge's number)
            if let Some(id) = f[3].strip_prefix("id") {
                return Some((f[1].parse().ok()?, id.parse().ok()?, p));
            }
            let k: usize = f[3].strip_prefix('e')?.parse().ok()?;
            Some((f[1].parse().ok()?, *lines.get(k)?, p))
        })
        .collect()
}

/// A scene's expected positions (tick, entity number, position): a drive scene's drive log.
fn scene_expects(name: &str) -> Vec<(i64, u32, [f64; 3])> {
    drive_expects(name)
}

/// (tickexact 2026-11-10) The ticks a scene's blocks run alone before its mobs are summoned (both
/// sides, uncompared but for the blocks every tick after): the wired loop's bench settles its
/// design for `SETTLE_TICKS` before it spawns the carts (`Run::arrive`), so its controller starts
/// from the state its redstone settles in with no cart on its detector rails. Its actions count
/// from the summon.
fn scene_settle(name: &str) -> u32 {
    if name == "carousel-wired" || name == "gate-escg6l" || name == "gate-escg6l-v3" || name == "gate-escg6l-v4" || scene_bench_start(name) { redsim::entity::mob::farm::SETTLE_TICKS as u32 } else { 0 }
}

/// The ticks before a scene's mobs are summoned: a built-in-order scene's build (its drive log's
/// `# spawn S`, its actions counted from the build's start), else its settle (actions counted from
/// the summon).
fn scene_spawn(name: &str) -> u32 {
    if scene_placement(name) == Placement::Built { drive_spawn(name).unwrap_or(0) } else { scene_settle(name) }
}

/// Whether a scene's actions and expected positions count from its start (a built-in-order
/// scene's), not from its summon.
fn scene_counts_from_start(name: &str) -> bool {
    scene_placement(name) == Placement::Built
}

/// (tickexact 2026-11-10) How a scene's blocks are put in the world.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// Raw states as written (every scene but the wired loop's).
    Raw,
    /// "Placed as written" (the bench's `--placed-raw`, the brief's "works as placed"): raw
    /// states, then the design's block entities' data merged ([`scene_block_entities`]: the
    /// game's `data merge`, which tells a comparator reading a container), nothing else.
    Merged,
    /// The judge's way (the bench's `Run::start`): raw writes, the block entities merged, a shape
    /// update of every wire, then a `self` update of every block, in the design's order
    /// (`World::place_updates`; in the game the clock verifier's `Placement::Judge`).
    Judge,
    /// "Built in order" (the bench's `--placed-in-order`, the brief's second check): the scene
    /// starts with none of its blocks; its drive log places each one (`act T set`, a placement
    /// with the game's updates) and fills each container (`act T nbt`) at its tick, counted from
    /// the build's start, and the mobs come at its `# spawn S`.
    Built,
}

/// A scene's [`Placement`]. The wired loop's file (v1) holds every wire as a cross and a
/// container's stock as an `nbt:` line, as its generator wrote them; carousel-wired builds it the
/// judge's way, carousel-wired-raw places it as written.
fn scene_placement(name: &str) -> Placement {
    if let Some((base, ..)) = moved(name) {
        return scene_placement(base);
    }
    match name {
        "carousel-wired" | "carousel-wired-pocket" | "siding-fault-kick3" | "siding-fault-asleep700" | "siding-fault-nap40" | "gate-escg6l" | "gate-escg6l-v3" | "gate-escg6l-v4" | "siding-plate-normal" | "siding-plate-day" | "siding-plate-kick3" | "siding-sleepfail-nodrop3" | "siding-sleepfail-bedtaken3" => Placement::Judge,
        "carousel-wired-raw" | "carousel-wired-v2" | "carousel-wired-v3" | "unload-v3-raw" | "unload-part-v3-raw" => Placement::Merged,
        "carousel-wired-built" | "carousel-wired-v3-built" | "siding-built" | "station-built" => Placement::Built,
        "siding-raw" | "station-raw" | "unload-siding-raw" => Placement::Merged,
        _ => Placement::Raw,
    }
}

/// Whether a scene is placed the judge's way ([`Placement::Judge`]).
fn scene_judge_placed(name: &str) -> bool {
    scene_placement(name) == Placement::Judge
}

/// (tickexact 2026-11-10, day-1's lean pocket loop) Whether a scene starts as its bench's own run
/// does (`Run::start`, the settle, `Run::arrive`): its daylight detectors held back until the
/// summon (they would read the settle's sky), then placed raw with their updates; its output chest
/// (`port output`) set when it is not one, and emptied.
fn scene_bench_start(name: &str) -> bool {
    name == "carousel-wired-pocket" || name.starts_with("siding-fault-") || name.starts_with("siding-plate-") || name.starts_with("siding-sleepfail-") || moved(name).is_some_and(|(base, ..)| scene_bench_start(base))
}

/// The chest a bench start sets at the output.
const BENCH_CHEST: &str = "minecraft:chest[facing=north,type=single,waterlogged=false]";

/// A scene's blocks as its start places them (a bench start holds its daylight detectors back).
fn scene_placed_blocks(s: &Scene) -> Vec<([i32; 3], String)> {
    if !scene_bench_start(&s.name) {
        return s.blocks.clone();
    }
    s.blocks.iter().filter(|(_, st)| !st.starts_with("minecraft:daylight_detector")).cloned().collect()
}

/// The daylight detectors a bench start places at the summon (none otherwise).
fn scene_held_detectors(s: &Scene) -> Vec<([i32; 3], String)> {
    if !scene_bench_start(&s.name) {
        return Vec::new();
    }
    s.blocks.iter().filter(|(_, st)| st.starts_with("minecraft:daylight_detector")).cloned().collect()
}

/// A bench start's output chest (the design's `port output`).
fn scene_bench_output(s: &Scene) -> Option<[i32; 3]> {
    if !scene_bench_start(&s.name) {
        return None;
    }
    drive_design(&s.name)?.ports.get("output").copied()
}

/// A scene's block-entity data merged after the raw paste (its design's `nbt:` lines; none for a
/// [`Placement::Raw`] scene).
fn scene_block_entities(name: &str) -> Vec<([i32; 3], String)> {
    if matches!(scene_placement(name), Placement::Raw | Placement::Built) {
        return Vec::new();
    }
    drive_design(name).map(|d| d.block_entities.into_iter().collect()).unwrap_or_default()
}

/// (tickexact 2026-11-10) The plant-supports scene: a stone floor at y 63 and three rows (z 4, 8,
/// 12) of natural plants at y 65 on their supports at y 64, x 2..30 every 2; then, from tick 20,
/// every 10 ticks one row's action: its support set to air, or a two-block plant's upper half, or
/// a stone beside the plant (a shape update it survives). The plants cover every plant class of
/// the natural table (leaf litter, grass, ferns, flowers, flower beds, bushes, a mushroom, a
/// carpet, dry vegetation, two-block plants), and the random tables (short grass, ferns, dead
/// bushes) several times over.
fn plant_supports() -> (Vec<([i32; 3], String)>, Vec<(u32, Vec<([i32; 3], String)>)>) {
    // what a row's action sets to air
    #[derive(Clone, Copy)]
    enum Cut {
        Support,
        Upper,
        Side,
    }
    use Cut::*;
    // (dirt, not grass blocks: a void sandbox's grass would spread by its random ticks, whose light
    // test needs block light, which redsim models in terrain worlds only)
    const DIRT: &str = "minecraft:dirt";
    const PODZOL: &str = "minecraft:podzol[snowy=false]";
    const SAND: &str = "minecraft:sand";
    const STONE: &str = "minecraft:stone";
    // (support, plant, its upper half, the action)
    let rows: Vec<(&str, &str, Option<&str>, Cut)> = vec![
        (DIRT, "minecraft:leaf_litter[facing=north,segment_amount=3]", None, Support),
        (PODZOL, "minecraft:leaf_litter[facing=east,segment_amount=1]", None, Support),
        (STONE, "minecraft:leaf_litter[facing=south,segment_amount=4]", None, Support),
        (DIRT, "minecraft:short_grass", None, Support),
        (PODZOL, "minecraft:fern", None, Support),
        (DIRT, "minecraft:tall_grass[half=lower]", Some("minecraft:tall_grass[half=upper]"), Support),
        (DIRT, "minecraft:peony[half=lower]", Some("minecraft:peony[half=upper]"), Support),
        (DIRT, "minecraft:peony[half=lower]", Some("minecraft:peony[half=upper]"), Upper),
        (DIRT, "minecraft:tall_grass[half=lower]", Some("minecraft:tall_grass[half=upper]"), Upper),
        (DIRT, "minecraft:azure_bluet", None, Support),
        (DIRT, "minecraft:wildflowers[facing=north,flower_amount=4]", None, Support),
        (DIRT, "minecraft:bush", None, Support),
        (DIRT, "minecraft:firefly_bush", None, Support),
        // the podzol under the mushroom lifts its light rule: it stays
        (PODZOL, "minecraft:brown_mushroom", None, Side),
        (DIRT, "minecraft:poppy", None, Side),
        (STONE, "minecraft:moss_carpet", None, Support),
        (SAND, "minecraft:dead_bush", None, Support),
        (SAND, "minecraft:short_dry_grass", None, Support),
        (SAND, "minecraft:tall_dry_grass", None, Support),
        (DIRT, "minecraft:large_fern[half=lower]", Some("minecraft:large_fern[half=upper]"), Support),
        (DIRT, "minecraft:pink_petals[facing=north,flower_amount=2]", None, Support),
        (DIRT, "minecraft:poppy", None, Support),
        (DIRT, "minecraft:sunflower[half=lower]", Some("minecraft:sunflower[half=upper]"), Support),
    ];
    let mut all = rows.clone();
    // the random tables again: short grass's 1-in-8 seeds, a dead bush's 0-2 sticks, a large fern's
    // seeds
    for _ in 0..14 {
        all.push((DIRT, "minecraft:short_grass", None, Support));
    }
    for _ in 0..4 {
        all.push((SAND, "minecraft:dead_bush", None, Support));
    }
    for _ in 0..4 {
        all.push((DIRT, "minecraft:large_fern[half=lower]", Some("minecraft:large_fern[half=upper]"), Support));
    }
    let mut blocks = Vec::new();
    floor(&mut blocks, 0, 32, 0, 16, 63, STONE);
    let mut actions = Vec::new();
    for (k, (support, plant, upper, cut)) in all.iter().enumerate() {
        let (x, z) = (2 + 2 * (k as i32 % 15), 4 + 4 * (k as i32 / 15));
        blocks.push(([x, 64, z], support.to_string()));
        blocks.push(([x, 65, z], plant.to_string()));
        if let Some(u) = upper {
            blocks.push(([x, 66, z], u.to_string()));
        }
        let at = match cut {
            Support => [x, 64, z],
            Upper => [x, 66, z],
            Side => {
                blocks.push(([x + 1, 65, z], STONE.to_string()));
                [x + 1, 65, z]
            }
        };
        actions.push((20 + 10 * k as u32, vec![(at, "minecraft:air".to_string())]));
    }
    (blocks, actions)
}

/// A stone floor over `x0..x1`, `z0..z1` at height `y`.
fn floor(out: &mut Vec<([i32; 3], String)>, x0: i32, x1: i32, z0: i32, z1: i32, y: i32, state: &str) {
    for x in x0..x1 {
        for z in z0..z1 {
            out.push(([x, y, z], state.to_string()));
        }
    }
}

/// A wall from `(x0, z0)` to `(x1, z1)` (inclusive, one axis), `h` high above `y`.
fn wall(out: &mut Vec<([i32; 3], String)>, a: (i32, i32), b: (i32, i32), y: i32, h: i32, state: &str) {
    for x in a.0.min(b.0)..=a.0.max(b.0) {
        for z in a.1.min(b.1)..=a.1.max(b.1) {
            for dy in 1..=h {
                out.push(([x, y + dy, z], state.to_string()));
            }
        }
    }
}

/// The connected states of the scene's oak fences (a fence joins the fences beside it), so raw
/// placement leaves them as the game would after any shape update. (A fence gate is not a fence:
/// it keeps its own state.)
fn connect_fences(blocks: &mut [([i32; 3], String)]) {
    let is_fence = |s: &str| s == "minecraft:oak_fence" || s.starts_with("minecraft:oak_fence[");
    let fences: std::collections::HashSet<[i32; 3]> = blocks.iter().filter(|(_, s)| is_fence(s)).map(|(p, _)| *p).collect();
    for (p, s) in blocks.iter_mut() {
        if !is_fence(s) {
            continue;
        }
        let has = |dx: i32, dz: i32| fences.contains(&[p[0] + dx, p[1], p[2] + dz]);
        *s = format!("minecraft:oak_fence[east={},north={},south={},waterlogged=false,west={}]", has(1, 0), has(0, -1), has(0, 1), has(-1, 0));
    }
}

/// The built-in scenes.
pub fn scenes() -> Vec<Scene> {
    let mut v = scenes_raw();
    for s in &mut v {
        // a 2-high stone wall on the sandbox's edge cells keeps every mob in the box (beyond it the
        // game's chunks are loaded but empty, redsim's are not there)
        let (x0, z0, x1, z1) = (s.chunks[0] * 16, s.chunks[1] * 16, s.chunks[2] * 16 + 15, s.chunks[3] * 16 + 15);
        let floor_y = s.blocks.iter().map(|(p, _)| p[1]).min().unwrap_or(63);
        for y in floor_y + 1..=floor_y + 2 {
            for x in x0..=x1 {
                for z in [z0, z1] {
                    s.blocks.push(([x, y, z], "minecraft:stone".into()));
                }
            }
            for z in z0 + 1..z1 {
                for x in [x0, x1] {
                    s.blocks.push(([x, y, z], "minecraft:stone".into()));
                }
            }
        }
        connect_fences(&mut s.blocks);
    }
    v
}

fn scenes_raw() -> Vec<Scene> {
    let stone = "minecraft:stone";
    let mut v = Vec::new();
    // a zombie alone on a floor at night: strolls (LandRandomPos), looks around
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        v.push(Scene {
            name: "zombie-stroll".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:zombie".into(), [24.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // a zombie and a golem walled off from it (the golem stands still: no AI modeled for it yet,
    // so the scene's golem never moves: NoAI)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (16, 4), (16, 40), 63, 3, stone);
        wall(&mut b, (30, 8), (30, 46), 63, 2, stone);
        floor(&mut b, 20, 26, 20, 26, 64, stone);
        v.push(Scene {
            name: "zombie-obstacles".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:zombie".into(), [6.5, 64.0, 6.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // an iron golem and a zombie 12 apart at night: each targets the other (the golem: enemies
    // but creepers, every 3 ticks; the zombie: iron golems, mustSee), the chase, the hits
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        v.push(Scene {
            name: "golem-zombie".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:iron_golem".into(), [8.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombie".into(), [20.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // a zombie pathing to a villager penned in fences beyond two low walls (the walls and the
    // fences do not block the sight between their eyes): the target, the path around the walls,
    // the hits through the fence; the villager panics (its brain's PANIC)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (16, 12), (16, 36), 63, 1, stone);
        wall(&mut b, (26, 20), (26, 44), 63, 1, stone);
        for (x, z) in [(35, 23), (36, 23), (37, 23), (35, 24), (37, 24), (35, 25), (36, 25), (37, 25)] {
            b.push(([x, 64, z], "minecraft:oak_fence".to_string()));
        }
        v.push(Scene {
            name: "zombie-villager".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:zombie".into(), [6.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [36.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
        // (mob-AI 2026-11-10) on Hard, in a small pen: the villager the zombie kills becomes a
        // zombie villager (Zombie.killedEntity: convertVillagerToZombieVillager), which then
        // hunts on its own
        let mut pen = Vec::new();
        floor(&mut pen, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((20, 20), (26, 20)), ((20, 26), (26, 26)), ((20, 20), (20, 26)), ((26, 20), (26, 26))] {
            wall(&mut pen, a, c, 63, 2, stone);
        }
        v.push(Scene {
            name: "zombie-villager-hard".into(),
            chunks: [0, 0, 2, 2],
            blocks: pen,
            mobs: vec![
                ("minecraft:zombie".into(), [21.5, 64.0, 21.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [25.5, 64.0, 25.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // a zombie standing in a 2-deep pool (its eye in water): 600 ticks, then the 300-tick
    // conversion, then the drowned in its place (its handleAttributes draws, its amphibious
    // navigation and its goals: the stroll through the water, its move control's extra sink)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for x in 21..=27 {
            for z in 21..=27 {
                let edge = x == 21 || x == 27 || z == 21 || z == 27;
                for y in 64..=66 {
                    if edge {
                        b.push(([x, y, z], stone.to_string()));
                    } else if y <= 65 {
                        b.push(([x, y, z], "minecraft:water[level=0]".to_string()));
                    }
                }
            }
        }
        v.push(Scene {
            name: "zombie-drowning".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:zombie".into(), [24.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 40.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // a zombie that breaks doors (Hard) going for a villager in a fence pen through its oak door
    // (the fences do not block the sight between their eyes): the path through the door, the
    // horizontal collision, BreakDoorGoal's 240 ticks and its nextInt(20) swings, the door gone
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for x in 34..=38 {
            for z in 22..=26 {
                let edge = x == 34 || x == 38 || z == 22 || z == 26;
                if edge && !(x == 34 && z == 24) {
                    b.push(([x, 64, z], "minecraft:oak_fence".to_string()));
                }
            }
        }
        b.push(([34, 64, 24], "minecraft:oak_door[facing=east,half=lower,hinge=left,open=false,powered=false]".to_string()));
        b.push(([34, 65, 24], "minecraft:oak_door[facing=east,half=upper,hinge=left,open=false,powered=false]".to_string()));
        v.push(Scene {
            name: "zombie-door".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:zombie".into(), [8.5, 64.0, 24.5], Some("{PersistenceRequired:1b,CanBreakDoors:1b}".into())),
                ("minecraft:villager".into(), [36.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // three homeless villagers and three free beds before dusk: AcquirePoi claims the beds (the
    // closest five, one path to all of them, the retry markers), then REST walks them to their
    // beds (SetWalkTargetFromBlockMemory) and they sleep
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        // (the one-bed dorm, ironspec's carousel) one bed for two villagers, an observer facing
        // its head (its output into a lamp): the bed's OCCUPIED flips as one sleeps and wakes
        let mut ob = b.clone();
        ob.push(([31, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        ob.push(([31, 64, 21], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        ob.push(([31, 64, 22], "minecraft:observer[facing=north,powered=false]".to_string()));
        ob.push(([31, 64, 23], "minecraft:redstone_lamp[lit=false]".to_string()));
        v.push(Scene {
            name: "bed-observer".into(),
            chunks: [0, 0, 2, 2],
            blocks: ob,
            mobs: vec![
                ("minecraft:villager".into(), [10.5, 64.0, 10.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [14.5, 64.0, 10.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 11000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
        // corrupt-poi-entity R1: villager A claims the one bed from 23 away; the bed is broken
        // and placed again (scripted at 600 and 601); villager B, penned until 602, takes the
        // fresh record while A keeps its HOME
        let mut rb = b.clone();
        rb.push(([31, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        rb.push(([31, 64, 21], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        for x in 38..=42 {
            for z in 38..=42 {
                if x == 38 || x == 42 || z == 38 || z == 42 {
                    rb.push(([x, 64, z], "minecraft:stone".to_string()));
                    rb.push(([x, 65, z], "minecraft:stone".to_string()));
                }
            }
        }
        v.push(Scene {
            name: "bed-r1".into(),
            chunks: [0, 0, 2, 2],
            blocks: rb.clone(),
            mobs: vec![
                ("minecraft:villager".into(), [10.5, 64.0, 10.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [40.5, 64.0, 40.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
        // the carousel's rider: villager A claims the one bed from a minecart on a rail 23 away and
        // stays in it through the night (REST's walk target unreachable: CANT_REACH, then the
        // release after 1200 ticks), villager B (let out at 400) walks to the bed homeless and
        // takes it when it is free
        let mut cb = rb.clone();
        cb.push(([10, 64, 10], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
        v.push(Scene {
            name: "bed-cart".into(),
            chunks: [0, 0, 2, 2],
            blocks: cb,
            mobs: vec![
                ("minecraft:minecart".into(), [10.5, 64.0, 10.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into())),
                ("minecraft:villager".into(), [40.5, 64.0, 40.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 11000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
        // (tickexact 2026-11-09, ironspec's one-bed carousel) the release and the re-take on a
        // shared bed: villager A claims the one bed from a minecart in a closed box 23 away whose
        // door is open; the door shuts at 300 (the bed out of A's reach), the bed is broken and
        // placed again (600, 601) and B, let out at 602, takes the fresh record (the R1 double
        // claim: both HOMEs on one bed). In REST A's walk target cannot be reached (CANT_REACH from
        // its first REST tick); 1200 gt later SetWalkTargetFromBlockMemory releases its HOME
        // (Villager.releasePoi: the record's one ticket free while B keeps its HOME); A's AcquirePoi
        // fails while shut in; the door opens at 12600 and A takes the ticket back
        let mut kb = rb;
        kb.push(([10, 64, 10], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
        for x in 9..=11 {
            for z in 9..=11 {
                if (x, z) != (10, 10) && (x, z) != (11, 10) {
                    kb.push(([x, 64, z], "minecraft:stone".to_string()));
                    kb.push(([x, 65, z], "minecraft:stone".to_string()));
                }
                kb.push(([x, 66, z], "minecraft:stone".to_string()));
            }
        }
        v.push(Scene {
            name: "bed-retake".into(),
            chunks: [0, 0, 2, 2],
            blocks: kb,
            mobs: vec![
                ("minecraft:minecart".into(), [10.5, 64.0, 10.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into())),
                ("minecraft:villager".into(), [40.5, 64.0, 40.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
        for x in [28, 31, 34] {
            b.push(([x, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
            b.push(([x, 64, 21], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        }
        // (tickexact 2026-11-09, ironspec's one-bed carousel) the R1 state from a design file:
        // three villagers summoned with HOME = the one bed in their data (`Brain:{memories:
        // {"minecraft:home":...}}`, no ticket taken), free on the floor at dusk: at REST each walks
        // to the bed; the first lies down; the others, awake within 16 of their OCCUPIED bed,
        // drop their HOME (ValidateNearbyPoi; not released while a villager sleeps in it)
        let mut sb = b.clone();
        sb.retain(|(p, _)| !(p[1] == 64 && p[2] >= 20 && p[2] <= 21 && (p[0] == 28 || p[0] == 34)));
        let home = |x: f64| ("minecraft:villager".to_string(), [x, 64.0, 10.5], Some("{PersistenceRequired:1b,Brain:{memories:{\"minecraft:home\":{value:{dimension:\"minecraft:overworld\",pos:[I;31,64,21]}}}}}".to_string()));
        v.push(Scene {
            name: "bed-shared".into(),
            chunks: [0, 0, 2, 2],
            blocks: sb,
            mobs: vec![home(10.5), home(12.5), home(14.5)],
            day_time: 11000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
        v.push(Scene {
            name: "villager-beds".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:villager".into(), [10.5, 64.0, 10.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [12.5, 64.0, 10.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [14.5, 64.0, 10.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 11000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-09, ironfarm's release + hold, its geometry as described) a lane along z
    // on glass supports: at each end a 3-rail incline of powered rails descending into the lane
    // with 2 flat powered rails at its foot (stone supports: a redstone torch under one powers the
    // end), a glass stop at the incline's top; 15 flat rails between with a booster (a powered rail
    // on a redstone block) every 4. Three carts with villagers aboard, touching, rest braked on the
    // north incline. At 100 a torch under the north foot powers that end: they roll down (gravity
    // and boost, no kick), run the lane and climb the unpowered south incline (brake: x0.5 a tick,
    // zero under 0.03), the lead into the incline, the followers compressed onto the flat brakes.
    // At 700 the north torch goes; at 800 a torch under the south foot launches them back north
    {
        let mut tb = Vec::new();
        floor(&mut tb, 0, 24, 0, 48, 63, stone);
        floor(&mut tb, 25, 48, 0, 48, 63, stone);
        let powered = |s: &str| format!("minecraft:powered_rail[powered=false,shape={s},waterlogged=false]");
        let rail = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
        let glass = "minecraft:glass".to_string();
        // the north end: the stop, the incline (its low end at z 12 level with the lane, its top at
        // z 10, y 66: an ascending rail joins the lane's level on its low side), the flat foot
        tb.push(([24, 66, 9], glass.clone()));
        tb.push(([24, 67, 9], glass.clone()));
        for (z, y) in [(10, 66), (11, 65), (12, 64)] {
            tb.push(([24, y - 1, z], glass.clone()));
            tb.push(([24, y, z], powered("ascending_north")));
        }
        for z in [13, 14] {
            tb.push(([24, 63, z], stone.to_string()));
            tb.push(([24, 64, z], powered("north_south")));
        }
        // the lane: 15 flat rails, a booster every 4 (z 18, 22, 26)
        for z in 15..=29 {
            let booster = matches!(z, 18 | 22 | 26);
            tb.push(([24, 63, z], if booster { "minecraft:redstone_block".to_string() } else { glass.clone() }));
            tb.push(([24, 64, z], if booster { "minecraft:powered_rail[powered=true,shape=north_south,waterlogged=false]".to_string() } else { rail("north_south") }));
        }
        // the south end, the mirror image: the flat foot (z 30, 31), the incline up to z 34, the stop
        for z in [30, 31] {
            tb.push(([24, 63, z], stone.to_string()));
            tb.push(([24, 64, z], powered("north_south")));
        }
        for (z, y) in [(32, 64), (33, 65), (34, 66)] {
            tb.push(([24, y - 1, z], glass.clone()));
            tb.push(([24, y, z], powered("ascending_south")));
        }
        tb.push(([24, 66, 35], glass.clone()));
        tb.push(([24, 67, 35], glass.clone()));
        // the torches' floors under the two feet (the torch itself is the signal)
        tb.push(([24, 61, 13], stone.to_string()));
        tb.push(([24, 61, 30], stone.to_string()));
        let cart = |y: f64, z: f64| ("minecraft:minecart".to_string(), [24.5, y, z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string()));
        v.push(Scene {
            name: "cart-train".into(),
            chunks: [0, 0, 2, 2],
            blocks: tb,
            mobs: vec![cart(66.0, 10.5), cart(65.0, 11.5), cart(64.0, 12.5)],
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-09, ironspec's one-bed carousel) the nap and the panic wake: a homeless
    // villager claims the one bed and lies down in REST, the bed's stand-up cells walled but the
    // one east of its head (the brief's single free cell), an observer looking at the bed's foot
    // (OCCUPIED's flips reach the foot by its shape update); at 2000 a window opens on a zombie
    // penned 5 east of the head, in its sight within 8 (VillagerHostilesSensor, PANIC, WakeUp,
    // stopSleeping's findStandUpPosition); at 2400 the window shuts: it calms down, walks back and
    // lies down again (SleepInBed: LAST_WOKEN 100 gt before, the bed free); at 4600 it opens again
    {
        let mut pb = Vec::new();
        floor(&mut pb, 0, 48, 0, 48, 63, stone);
        pb.push(([31, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        pb.push(([31, 64, 21], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        // the surround of a south-facing bed's head (findStandUpPosition's ten cells), all walled
        // 2 high but (32, 64, 21); the observer takes (31, 19), looking south at the foot
        for (x, z) in [(30, 19), (32, 19), (30, 20), (32, 20), (30, 21), (30, 22), (31, 22), (32, 22)] {
            pb.push(([x, 64, z], stone.to_string()));
            pb.push(([x, 65, z], stone.to_string()));
        }
        pb.push(([31, 64, 19], "minecraft:observer[facing=south,powered=false]".to_string()));
        pb.push(([31, 65, 19], stone.to_string()));
        // the zombie's pen (36, 64..65, 21): walls 2 high, a roof, the window (35, 65, 21) shut
        for (x, z) in [(35, 21), (37, 21), (36, 20), (36, 22)] {
            pb.push(([x, 64, z], stone.to_string()));
            pb.push(([x, 65, z], stone.to_string()));
        }
        pb.push(([36, 66, 21], stone.to_string()));
        v.push(Scene {
            name: "bed-panic".into(),
            chunks: [0, 0, 2, 2],
            blocks: pb,
            mobs: vec![
                ("minecraft:villager".into(), [36.5, 64.0, 12.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombie".into(), [36.5, 64.0, 21.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 11000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-09, ironspec's carousel timing) the summon tick: three homeless
    // villagers shut in a dorm with three beds sleep at REST (LAST_SLEPT); at 2000 a window opens
    // on a zombie penned against the dorm's south wall (5 from them, in sight): PANIC wakes them,
    // and each panicking villager's VillagerPanicTrigger tries spawnGolemIfNeeded(3) at the game
    // time's multiples of 100; a golem's only floor in reach is one stone pad north of the dorm
    // over the void (it walks off and falls out of every sensor's 16), so GOLEM_DETECTED_RECENTLY
    // (600 gt from golemDetected on every villager in the summoner's 10-box) is the only lock:
    // the slots it holds, and when each villager is free again
    {
        let mut gb = Vec::new();
        // the dorm's floor only (x 26..36, z 17..23), its walls 2 high and a glass roof: no golem
        // fits inside (2.7 tall) and none stands on glass (SpawnUtil's LEGACY_IRON_GOLEM)
        floor(&mut gb, 26, 36, 17, 23, 63, stone);
        for x in 26..=36 {
            for z in 17..=23 {
                if x == 26 || x == 36 || z == 17 || z == 23 {
                    for y in 64..=65 {
                        gb.push(([x, y, z], stone.to_string()));
                    }
                }
                gb.push(([x, 66, z], "minecraft:glass".to_string()));
            }
        }
        for x in [28, 31, 34] {
            gb.push(([x, 64, 19], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
            gb.push(([x, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        }
        // the zombie's pen (30..32, 64..65, 24) against the dorm's wall, the wall's eye row
        // (27..35, 65, 23) its window, shut: open, every sleeper sees the zombie
        for x in 30..=32 {
            gb.push(([x, 63, 24], stone.to_string()));
            gb.push(([x, 64, 25], stone.to_string()));
            gb.push(([x, 65, 25], stone.to_string()));
        }
        for x in [29, 33] {
            gb.push(([x, 64, 24], stone.to_string()));
            gb.push(([x, 65, 24], stone.to_string()));
        }
        for x in 29..=33 {
            for z in 24..=25 {
                // stone keeps the sun off the zombie, glass over it keeps golems off its roof
                gb.push(([x, 66, z], stone.to_string()));
                gb.push(([x, 67, z], "minecraft:glass".to_string()));
            }
        }
        // the golems' only floor: a 9 x 3 stone pad over a 33-deep pit north of the dorm (within
        // the summon's 8 of the beds), taken away right after the first golem stands on it; the
        // pit's floor keeps the fallen golem alive, more than 16 below every villager
        for x in 27..=35 {
            for z in 12..=14 {
                gb.push(([x, 63, z], stone.to_string()));
            }
        }
        floor(&mut gb, 24, 38, 8, 16, 30, stone);
        v.push(Scene {
            name: "golem-summon".into(),
            chunks: [0, 0, 2, 2],
            blocks: gb,
            mobs: vec![
                ("minecraft:villager".into(), [29.5, 64.0, 21.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [31.5, 64.0, 21.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [33.5, 64.0, 21.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombie".into(), [31.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 11000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 0, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // a creeper going for a survival player who stands inside a ring of fences 4 out (it never
    // gets within the fuse's 3 blocks): the target, the path to the ring, the looks
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for x in 32..=40 {
            for z in 20..=28 {
                let edge = x == 32 || x == 40 || z == 20 || z == 28;
                if edge {
                    b.push(([x, 64, z], "minecraft:oak_fence".to_string()));
                }
            }
        }
        v.push(Scene {
            name: "creeper-player".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:creeper".into(), [20.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([36.5, 64.0, 24.5]),
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (mob-AI 2026-11-10) a skeleton with a bow (summoned with it in hand: reassessWeaponGoal's
    // bow goal) and an iron golem 22 apart at night on Hard: RangedBowAttackGoal closes in, strafes,
    // draws 20 ticks and shoots (the arrows' flight, their hits on the golem: its knockback
    // resistance, its target on the skeleton; the misses stuck in the floor), until the golem
    // reaches and kills it
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        v.push(Scene {
            name: "skeleton-golem".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:skeleton".into(), [16.5, 64.0, 24.5], Some("{PersistenceRequired:1b,equipment:{mainhand:{id:\"minecraft:bow\",count:1}}}".into())),
                ("minecraft:iron_golem".into(), [30.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 90.0, 46.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) a zombie with an iron spear and a villager in a walled arena at night:
    // SpearUseGoal (the walk up to 10, the spear lowered for reducedTickDelay(12 + 225), the
    // charge, the backing off and the flight), KineticWeapon's stabs (the look's reach, the
    // speeds, the damage and knockback conditions, stabAttack, the spear damage type)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (4, 4), (44, 4), 64, 3, stone);
        wall(&mut b, (4, 44), (44, 44), 64, 3, stone);
        wall(&mut b, (4, 4), (4, 44), 64, 3, stone);
        wall(&mut b, (44, 4), (44, 44), 64, 3, stone);
        v.push(Scene {
            name: "zombie-spear".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:zombie".into(), [14.5, 64.0, 24.5], Some("{PersistenceRequired:1b,equipment:{mainhand:{id:\"minecraft:iron_spear\",count:1}}}".into())),
                ("minecraft:villager".into(), [30.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) a wolf pack, a sheep and a skeleton with a bow in a walled arena at
    // night, a shallow pool in a corner: the wolves' goals (their prey, the skeleton, the leap and
    // the bite, the stroll), their anger and alerts, a wet wolf's shake; the skeleton's
    // AvoidEntityGoal of wolves and its arrows; the sheep's panic and its loot
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (4, 4), (44, 4), 64, 3, stone);
        wall(&mut b, (4, 44), (44, 44), 64, 3, stone);
        wall(&mut b, (4, 4), (4, 44), 64, 3, stone);
        wall(&mut b, (44, 4), (44, 44), 64, 3, stone);
        for x in 34..=38 {
            for z in 34..=38 {
                b.push(([x, 63, z], "minecraft:water[level=0]".to_string()));
            }
        }
        let p = |id: &str, at: [f64; 3], nbt: &str| (id.to_string(), at, Some(nbt.to_string()));
        v.push(Scene {
            name: "wolf-pack".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                p("minecraft:wolf", [20.5, 64.0, 20.5], "{PersistenceRequired:1b}"),
                p("minecraft:wolf", [22.5, 64.0, 21.5], "{PersistenceRequired:1b}"),
                p("minecraft:sheep", [30.5, 64.0, 26.5], "{PersistenceRequired:1b}"),
                p("minecraft:skeleton", [12.5, 64.0, 34.5], "{PersistenceRequired:1b,equipment:{mainhand:{id:\"minecraft:bow\",count:1}}}"),
            ],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) two wolves and a skeleton 8 apart at night: the skeleton's
    // AvoidEntityGoal of wolves, the wolves' NearestAttackableTarget of skeletons, the fight
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (4, 4), (44, 4), 64, 3, stone);
        wall(&mut b, (4, 44), (44, 44), 64, 3, stone);
        wall(&mut b, (4, 4), (4, 44), 64, 3, stone);
        wall(&mut b, (44, 4), (44, 44), 64, 3, stone);
        let p = |id: &str, at: [f64; 3], nbt: &str| (id.to_string(), at, Some(nbt.to_string()));
        v.push(Scene {
            name: "wolf-skeleton".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                p("minecraft:wolf", [20.5, 64.0, 20.5], "{PersistenceRequired:1b}"),
                p("minecraft:wolf", [21.5, 64.0, 22.5], "{PersistenceRequired:1b}"),
                p("minecraft:skeleton", [26.5, 64.0, 26.5], "{PersistenceRequired:1b,equipment:{mainhand:{id:\"minecraft:bow\",count:1}}}"),
            ],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) a drowned with a trident in a pool at night and an iron golem on the
    // shore: its DrownedTridentAttackGoal (the throw every 40 ticks in sight within 10), the
    // ThrownTrident's flight (0.99 in water), its hit (8, the bounce back), lying in the ground
    // (dealt damage after 4 ticks); the golem's fight
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (4, 4), (44, 4), 64, 3, stone);
        wall(&mut b, (4, 44), (44, 44), 64, 3, stone);
        wall(&mut b, (4, 4), (4, 44), 64, 3, stone);
        wall(&mut b, (44, 4), (44, 44), 64, 3, stone);
        // a pool two deep on stone
        floor(&mut b, 18, 26, 18, 26, 61, stone);
        for x in 18..=26 {
            for z in 18..=26 {
                b.push(([x, 62, z], "minecraft:water[level=0]".to_string()));
                b.push(([x, 63, z], "minecraft:water[level=0]".to_string()));
            }
        }
        let p = |id: &str, at: [f64; 3], nbt: &str| (id.to_string(), at, Some(nbt.to_string()));
        v.push(Scene {
            name: "drowned-trident".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                p("minecraft:drowned", [22.5, 62.0, 22.5], "{PersistenceRequired:1b,equipment:{mainhand:{id:\"minecraft:trident\",count:1}}}"),
                p("minecraft:iron_golem", [31.5, 64.0, 22.5], "{PersistenceRequired:1b}"),
            ],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 58, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) a skeleton, an enderman and an iron golem in a line in a corridor at
    // night: the golem fights the enderman (Enderman.aiStep's updatePersistentAnger: its anger
    // timer's draw each tick it has a target), the skeleton shoots at the golem
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (8, 22), (40, 22), 64, 4, stone);
        wall(&mut b, (8, 26), (40, 26), 64, 4, stone);
        wall(&mut b, (8, 22), (8, 26), 64, 4, stone);
        wall(&mut b, (40, 22), (40, 26), 64, 4, stone);
        let p = |id: &str, at: [f64; 3], nbt: &str| (id.to_string(), at, Some(nbt.to_string()));
        v.push(Scene {
            name: "enderman-golem".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                p("minecraft:skeleton", [14.5, 64.0, 24.5], "{PersistenceRequired:1b,equipment:{mainhand:{id:\"minecraft:bow\",count:1}}}"),
                p("minecraft:enderman", [21.5, 64.0, 24.5], "{PersistenceRequired:1b}"),
                p("minecraft:iron_golem", [30.5, 64.0, 24.5], "{PersistenceRequired:1b}"),
            ],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: false,
            watch: ([0, 58, 0], [47, 72, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) a zombie horse and its baby (Age -24000: ZombieHorse.BABY_DIMENSIONS,
    // the zombie horse's x 0.7) at night: the baby's box and its FollowParentGoal, both strolls
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (4, 4), (44, 4), 64, 3, stone);
        wall(&mut b, (4, 44), (44, 44), 64, 3, stone);
        wall(&mut b, (4, 4), (4, 44), 64, 3, stone);
        wall(&mut b, (44, 4), (44, 44), 64, 3, stone);
        let p = |id: &str, at: [f64; 3], nbt: &str| (id.to_string(), at, Some(nbt.to_string()));
        v.push(Scene {
            name: "zombie-horse-baby".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                p("minecraft:zombie_horse", [20.5, 64.0, 20.5], "{PersistenceRequired:1b}"),
                p("minecraft:zombie_horse", [28.5, 64.0, 26.5], "{PersistenceRequired:1b,Age:-24000}"),
            ],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) an enderman by a patch of dandelions on grass in a stone arena at night:
    // it takes a dandelion (EndermanTakeBlockGoal) and, off the grass, its EndermanLeaveBlockGoal
    // finds the flower cannot survive on stone (VegetationBlock.updateShape: air), so it "places"
    // air and holds nothing
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (4, 4), (44, 4), 64, 4, stone);
        wall(&mut b, (4, 44), (44, 44), 64, 4, stone);
        wall(&mut b, (4, 4), (4, 44), 64, 4, stone);
        wall(&mut b, (44, 4), (44, 44), 64, 4, stone);
        for x in 22..=24 {
            for z in 22..=24 {
                b.push(([x, 63, z], "minecraft:grass_block[snowy=false]".to_string()));
                b.push(([x, 64, z], "minecraft:dandelion".to_string()));
            }
        }
        v.push(Scene {
            name: "enderman-flower".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:enderman".into(), [23.5, 64.0, 26.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 72, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) a cat and a creeper at night: the cat's wild goals (its stroll, looks),
    // the creeper's AvoidEntityGoal of cats (6, 1.0, 1.2)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (4, 4), (44, 4), 64, 3, stone);
        wall(&mut b, (4, 44), (44, 44), 64, 3, stone);
        wall(&mut b, (4, 4), (4, 44), 64, 3, stone);
        wall(&mut b, (44, 4), (44, 44), 64, 3, stone);
        let p = |id: &str, at: [f64; 3], nbt: &str| (id.to_string(), at, Some(nbt.to_string()));
        v.push(Scene {
            name: "cat-creeper".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                p("minecraft:cat", [20.5, 64.0, 20.5], "{PersistenceRequired:1b}"),
                p("minecraft:creeper", [23.5, 64.0, 22.5], "{PersistenceRequired:1b}"),
            ],
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            // a creative player within 32 keeps the cat's noActionTime at 0 (checkDespawn), so
            // its stroll runs, and its LookAtPlayerGoal has someone to look at
            player: Some([30.5, 64.0, 34.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // a skeleton alone at night: strolls, looks around (RestrictSunGoal idle in the dark)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (16, 4), (16, 40), 63, 2, stone);
        v.push(Scene {
            name: "skeleton-night".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:skeleton".into(), [24.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (mob-AI 2026-11-10) fish in a 4-deep pond: three salmon and two cod (FishSwimGoal's random
    // swims on WaterBoundPathNavigation and SwimNodeEvaluator, FishMoveControl, travelInWater,
    // FollowFlockLeaderGoal's schools once its constructor's countdown runs out), and a salmon
    // left on the stone beside it (its flops, its air running out, its death)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for x in 14..=34 {
            for z in 14..=34 {
                let edge = x == 14 || x == 34 || z == 14 || z == 34;
                for y in 64..=68 {
                    if edge {
                        b.push(([x, y, z], stone.to_string()));
                    } else if y <= 67 {
                        b.push(([x, y, z], "minecraft:water[level=0]".to_string()));
                    }
                }
            }
        }
        let fish = |id: &str, p: [f64; 3]| (id.to_string(), p, Some("{PersistenceRequired:1b}".to_string()));
        v.push(Scene {
            name: "salmon-pond".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                fish("minecraft:salmon", [20.5, 65.0, 20.5]),
                fish("minecraft:salmon", [27.5, 66.0, 22.5]),
                fish("minecraft:salmon", [24.5, 64.5, 29.5]),
                fish("minecraft:cod", [18.5, 65.5, 28.5]),
                fish("minecraft:cod", [30.5, 64.5, 30.5]),
                fish("minecraft:salmon", [8.5, 64.0, 24.5]),
            ],
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // a skeleton in the morning sun beside a roofed shelter: it burns, FleeSunGoal finds the shade,
    // RestrictSunGoal keeps its strolls out of the sky
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        // a shelter: a stone roof at y 67 over x 28..36, z 18..30, posts at its corners
        for (x, z) in [(28, 18), (36, 18), (28, 30), (36, 30)] {
            for y in 64..=66 {
                b.push(([x, y, z], "minecraft:stone".to_string()));
            }
        }
        floor(&mut b, 28, 37, 18, 31, 67, stone);
        v.push(Scene {
            name: "skeleton-sun".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:skeleton".into(), [20.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // a spider in a walled yard at night: it strolls into the walls and climbs (Spider.tick's
    // climbing, handleOnClimbable, WallClimberNavigation)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((12, 12), (12, 36)), ((36, 12), (36, 36)), ((12, 12), (36, 12)), ((12, 36), (36, 36))] {
            wall(&mut b, a, c, 63, 3, stone);
        }
        wall(&mut b, (24, 16), (24, 32), 63, 2, stone);
        v.push(Scene {
            name: "spider-walls".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:spider".into(), [18.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            // a creative player above the yard keeps the spider from idling (the despawn check)
            player: Some([24.5, 72.0, 24.5]),
            player_creative: true,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10, ironspec's portal pad) zombified piglins from a nether portal's random
    // ticks: a 2 x 3 portal (x 24, z 21..22, axis z) on a stone yard, a creative player above (a
    // player close enough for spawning: `anyPlayerCloseEnoughForSpawning`), Hard, monsters on
    // (`scene_spawns`: the natural spawner finds nothing in the noon light), random ticks x1000:
    // each piglin spawned on the portal's bottom frame with PortalCooldown 300, then its walk. The
    // game's side only (`mobai timeline portal-piglin`): with spawn_mobs on, redsim's natural
    // spawner needs the block light it models in terrain worlds only, so the fixture cannot compare
    // it yet (the piglins' AI is checked in piglin-golem)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for y in 63..=67 {
            for z in 20..=23 {
                let inside = (64..=66).contains(&y) && (21..=22).contains(&z);
                b.push(([24, y, z], if inside { "minecraft:nether_portal[axis=z]".to_string() } else { "minecraft:obsidian".to_string() }));
            }
        }
        v.push(Scene {
            name: "portal-piglin".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: Vec::new(),
            day_time: 6000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 72.0, 30.5]),
            player_creative: true,
            random_tick_speed: 3000,
        });
    }
    // (tickexact 2026-11-10) zombified piglins and an iron golem on a stone yard at noon (Hard):
    // the golem takes a piglin as its target (an Enemy), hits it; the piglin's HurtByTargetGoal
    // alerts the other (its own class), both fight back (ZombieAttackGoal, 5 damage), angry
    // (+0.05 speed, the anger timer)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        v.push(Scene {
            name: "piglin-golem".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:iron_golem".into(), [18.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombified_piglin".into(), [27.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombified_piglin".into(), [31.5, 64.0, 27.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 6000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // a spider and an iron golem 10 apart at night: each targets the other (the spider only in the
    // dark), the spider leaps (LeapAtTargetGoal) and bites, the golem hits
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        v.push(Scene {
            name: "spider-golem".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:spider".into(), [14.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:iron_golem".into(), [24.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10) the spawn effects a spider group draws on Hard (Spider.finalizeSpawn's
    // SpiderEffectsGroupData: `addEffect(new MobEffectInstance(effect, -1))`), loaded as data (the
    // same infinite effects; strength's ATTACK_DAMAGE +3 restored from `attributes` as a load does)
    // at night: a spider with strength bites the iron golem for 2 + 3; an invisible one is seen at
    // max(16 x 0.07, 2) only (the golem's NearestAttackableTargetGoal, the villager's visible list)
    // until it bites and the golem's HurtByTargetGoal answers; one with regeneration, hurt to 6 and
    // penned in glass, heals 1 whenever its tickCount is a multiple of 50
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        let glass = "minecraft:glass";
        wall(&mut b, (38, 38), (44, 38), 63, 3, glass);
        wall(&mut b, (38, 44), (44, 44), 63, 3, glass);
        wall(&mut b, (38, 38), (38, 44), 63, 3, glass);
        wall(&mut b, (44, 38), (44, 44), 63, 3, glass);
        floor(&mut b, 38, 45, 38, 45, 67, glass);
        let strength = "{PersistenceRequired:1b,active_effects:[{id:\"minecraft:strength\",amplifier:0b,duration:-1}],attributes:[{id:\"minecraft:attack_damage\",base:2.0d,modifiers:[{id:\"minecraft:effect.strength\",amount:3.0d,operation:\"add_value\"}]}]}";
        v.push(Scene {
            name: "spider-effects".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:iron_golem".into(), [20.5, 64.0, 20.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:spider".into(), [10.5, 64.0, 20.5], Some(strength.into())),
                ("minecraft:spider".into(), [20.5, 64.0, 12.5], Some(strength.into())),
                ("minecraft:spider".into(), [14.5, 64.0, 28.5], Some(strength.into())),
                ("minecraft:spider".into(), [30.5, 64.0, 20.5], Some("{PersistenceRequired:1b,active_effects:[{id:\"minecraft:invisibility\",amplifier:0b,duration:-1}]}".into())),
                ("minecraft:villager".into(), [30.5, 64.0, 26.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:spider".into(), [41.5, 64.0, 41.5], Some("{PersistenceRequired:1b,Health:6.0f,active_effects:[{id:\"minecraft:regeneration\",amplifier:0b,duration:-1}]}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-10, b8949c96's village door) natural doors' halves (DoorBlock.updateShape:
    // waxed copper doors, natural rows; oak parses as redsim's modeled door): an upper half removed
    // (a zombie's BreakDoorGoal: the lower half pops its loot), a lower half removed, the floor
    // under a lower half removed, an upper half swapped for another door's (the lower half follows),
    // and the modeled oak door's upper half removed (scene_actions: "door-halves")
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        let door = |name: &str, facing: &str, hinge: &str, half: &str| format!("minecraft:{name}[facing={facing},half={half},hinge={hinge},open=false,powered=false]");
        for (x, name, facing, hinge) in [(4, "waxed_copper_door", "east", "left"), (8, "waxed_oxidized_copper_door", "north", "left"), (12, "waxed_oxidized_copper_door", "south", "right"), (16, "waxed_oxidized_copper_door", "west", "right"), (20, "oak_door", "north", "left")] {
            b.push(([x, 64, 4], door(name, facing, hinge, "lower")));
            b.push(([x, 65, 4], door(name, facing, hinge, "upper")));
        }
        v.push(Scene {
            name: "door-halves".into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs: Vec::new(),
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 68, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec: the gate admissions the tail relies on) the pocket stand's
    // gate geometry (a closed fence gate facing east over a powered rail ascending west on a
    // redstone block, the arrival slope west of it, the departure east: rails ascending east,
    // then flat to a stone); each gate's input a redstone block placed north of it (z - 1):
    // line A (z 3): a queue of four carts with villagers pressed down the slope against G0,
    // panicking at a zombie in a boat; G0 opened 3 gt at t200, again at t600; line B (z 6): one
    // cart with a villager pressed against G_P on the always-on booster under it, opened 2 gt at
    // t200, 3 gt at t600; the band's edge: line C1 (z 9): an empty cart summoned at rest touching
    // the gate's plane, line C2 (z 12): one summoned at rest 0.05 short of it, both gates opened
    // for the first tick only (scene_actions: "gate-admit")
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        for z in [3, 6, 9, 12] {
            // the arrival slope: rails ascending west, one step up every block to x 14 (y 70)
            for (k, x) in (14..=19).rev().enumerate() {
                let y = 65 + k as i32;
                for yy in 64..y {
                    b.push(([x, yy, z], stone.to_string()));
                }
                b.push(([x, y, z], "minecraft:rail[shape=ascending_west,waterlogged=false]".to_string()));
            }
            b.push(([20, 63, z], "minecraft:redstone_block".to_string()));
            b.push(([20, 64, z], "minecraft:powered_rail[powered=true,shape=ascending_west,waterlogged=false]".to_string()));
            b.push(([20, 65, z], "minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]".to_string()));
            b.push(([21, 63, z], "minecraft:redstone_block".to_string()));
            b.push(([21, 64, z], "minecraft:powered_rail[powered=true,shape=ascending_east,waterlogged=false]".to_string()));
            b.push(([22, 64, z], stone.to_string()));
            for x in 22..=29 {
                b.push(([x, 65, z], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
            }
            // (tickexact 2026-11-11, ironspec's raw-placement audit: the departure's rails on stone
            // and the slope's top rail against stone on its high side, so no update pops them)
            for x in 23..=29 {
                b.push(([x, 64, z], stone.to_string()));
            }
            b.push(([13, 70, z], stone.to_string()));
            b.push(([30, 65, z], stone.to_string()));
        }
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        // line A: four ridden carts on the slope, the front one touching G0 (the stand's -0.115)
        for k in 0..4 {
            let x = 19.885 - k as f64;
            mobs.push(("minecraft:minecart".into(), [x, 65.1775 + k as f64, 3.5], Some(rider.into())));
        }
        // line B: P against G_P
        mobs.push(("minecraft:minecart".into(), [19.885, 65.1775, 6.5], Some(rider.into())));
        // C1 touching the gate's plane (x 20.375 - 0.49), C2 0.05 short (the slope's height there)
        mobs.push(("minecraft:minecart".into(), [19.885, 65.1775, 9.5], None));
        mobs.push(("minecraft:minecart".into(), [19.835, 65.2275, 12.5], None));
        mobs.push(("minecraft:oak_boat".into(), [16.5, 64.0, 4.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        v.push(Scene {
            name: "gate-admit".into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 72, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
        // (tickexact 2026-11-11, the reload-order rule) the same with a reload at t300 in each order
        let base = v.last().expect("gate-admit").clone();
        for n in ["gate-admit-rev", "gate-admit-s1", "gate-admit-s2"] {
            let mut c = base.clone();
            c.name = n.into();
            v.push(c);
        }
    }
    // (tickexact 2026-11-11, ironspec: CANT_REACH_WALK_TARGET_SINCE across a freeze) two riders,
    // each a villager in a minecart on a rail, each with HOME on its own bed sealed in glass (the
    // path never reaches: MoveToTargetSink sets the streak, SetWalkTargetFromBlockMemory(HOME, ..,
    // 1200) releases past 1200 gt); R1 (e0) in chunk (0, 0), frozen from t900 (its chunk's forced
    // ticket dropped: level 32, its entities still while game time runs), R2 (e1) in chunk (1, 0),
    // never frozen (the control: its release at 1200 gt of streak); night (REST from day time
    // 13000); "-zombie": a zombie in a boat in R1's sight (4 blocks: R1 panics), out of R2's (13);
    // "-5000": the freeze 5000 gt instead of 1500 (scene_actions)
    for (name, zombie) in [("freeze-cantreach", false), ("freeze-cantreach-5000", false), ("freeze-cantreach-zombie", true), ("freeze-cantreach-zombie-5000", true), ("home-reach-open", false)] {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        // ("home-reach-open", ironspec's e5 puzzle: R1's bed on the open floor, reachable on foot
        // from its cart (AcquirePoi's path, reach 1, ends beside or on top of the bed), R2's sealed
        // as before; nothing frozen)
        let open = name == "home-reach-open";
        for (cart_x, bed_x) in [(5, 10), (21, 26)] {
            b.push(([cart_x, 64, 5], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
            for x in bed_x - 1..=bed_x + 1 {
                for z in 9..=12 {
                    for y in 64..=65 {
                        if !(x == bed_x && (z == 10 || z == 11) && y == 64) && !(open && bed_x == 10) {
                            b.push(([x, y, z], "minecraft:glass".to_string()));
                        }
                    }
                }
            }
            b.push(([bed_x, 64, 10], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
            b.push(([bed_x, 64, 11], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        }
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = vec![("minecraft:minecart".into(), [5.5, 64.0, 5.5], Some(rider.into())), ("minecraft:minecart".into(), [21.5, 64.0, 5.5], Some(rider.into()))];
        if zombie {
            mobs.push(("minecraft:oak_boat".into(), [8.5, 64.0, 2.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        }
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: 13000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 67, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's bed radius: ValidateNearbyPoi(HOME) in REST erases a
    // villager's HOME when the bed (closerToCenterThan 16 of it) is OCCUPIED and it is not the
    // sleeper, keeping the bed's ticket while a villager sleeps there) a bed (head (5,64,6)) and
    // its sleeper S (e0, HOME and the ticket at t1, penned in glass 3 blocks east until t100: it
    // lies down soon after); two riders in minecarts on rails, both with HOME on that bed from
    // t1: R15 (e1) 15.0 from the bed's center along x, R17 (e2) 17.0 along z; night (REST from day
    // time 13000). "-zombie": a zombie in a boat 5 blocks from R15 (out of S's and R17's range: R15
    // panics before the bed is taken); "-reload": the whole part away 1 gt at t300, S asleep (the
    // riders' fresh brains calm in REST until their sensors scan)
    for (name, zombie) in [("bed-radius", false), ("bed-radius-zombie", true), ("bed-radius-zombie-reload", true), ("bed-radius-reload", false)] {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 32, 63, stone);
        b.push(([5, 64, 5], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        b.push(([5, 64, 6], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        for (x, z) in [(7, 6), (9, 6), (8, 5), (8, 7)] {
            for y in 64..=65 {
                b.push(([x, y, z], "minecraft:glass".to_string()));
            }
        }
        b.push(([8, 66, 6], "minecraft:glass".to_string()));
        b.push(([20, 64, 6], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
        b.push(([5, 64, 23], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = vec![
            ("minecraft:villager".into(), [8.5, 64.0, 6.5], Some("{PersistenceRequired:1b}".into())),
            ("minecraft:minecart".into(), [20.5, 64.0, 6.5], Some(rider.into())),
            ("minecraft:minecart".into(), [5.5, 64.0, 23.5], Some(rider.into())),
        ];
        if zombie {
            mobs.push(("minecraft:oak_boat".into(), [24.5, 64.0, 9.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        }
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 1, 1],
            blocks: b,
            mobs,
            day_time: 13000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 67, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's ledge on R1's drop) four scenes, one per stored speed, and
    // farmopt's (c′) row at 1.82
    for (tag, sp) in LEDGE_SPEEDS {
        v.push(ledge_scene(tag, sp));
    }
    v.push(ledge_scene_row("ledge-r1n-182", 1.82, true, &LEDGE_ROWS));
    v.push(ledge_scene_row("ledge-r1s-182", 1.82, false, &ledge_sweep_rows()));
    // (tickexact 2026-11-11, ironspec's take-off clamp) a lone cart at each stored speed, ridden, then empty
    v.push(ledge_scene_speeds("takeoff-clamp", false, &TAKEOFF_ROWS.iter().map(|(s, r)| (None, *r, *s)).collect::<Vec<_>>()));
    // (tickexact 2026-11-11, 58f770cc's HOME finding: a panicking rider's flee target) two lanes in a
    // void (no floor; the sandbox's edge walls kept beyond 16 of every rider, the lanes 24 apart): a
    // villager in a cart on a rail on glass, a zombie in a boat on glass 3 blocks north of it
    // (VillagerPanicTrigger; PANIC's SetWalkTargetAwayFrom(NEAREST_HOSTILE): LandRandomPos.getPosAway
    // (16, 7), each offset at most 16 a side), and one stone 4 below the rider's level to the south:
    // lane A 15 blocks off (in the flee's reach), lane B 17 (out of it); lane C as A with the zombie
    // moved south of the rider, between it and the stone (the flee runs north: the stone not "away")
    {
        let mut b = Vec::new();
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        for (x, z, dz, zombie_dz) in [(20, 24, 15, -3), (44, 24, 17, -3), (20, 60, 15, 3)] {
            b.push(([x, 63, z], "minecraft:glass".to_string()));
            b.push(([x, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            b.push(([x, 63, z + zombie_dz], "minecraft:glass".to_string()));
            b.push(([x, 60, z + dz], "minecraft:stone".to_string()));
            mobs.push(("minecraft:minecart".to_string(), [x as f64 + 0.5, 64.0, z as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string())));
            mobs.push(("minecraft:oak_boat".to_string(), [x as f64 + 0.5, 64.0, (z + zombie_dz) as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".to_string())));
        }
        v.push(Scene {
            name: "flee-stone".into(),
            chunks: [0, 0, 5, 5],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 56, 0], [95, 70, 95]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's ticket accounting) one bed (head (5,64,6)) in a glass-walled
    // room (x 3..9, z 3..10) with A (e0) and B (e1) on foot beside it, day (AcquirePoi(HOME) runs in
    // CORE all day): the acts give HOME up and erase it (see `actions`)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 16, 0, 16, 63, stone);
        b.push(([5, 64, 5], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        b.push(([5, 64, 6], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        for y in 64..=65 {
            for x in 2..=10 {
                b.push(([x, y, 2], "minecraft:glass".to_string()));
                b.push(([x, y, 11], "minecraft:glass".to_string()));
            }
            for z in 3..=10 {
                b.push(([2, y, z], "minecraft:glass".to_string()));
                b.push(([10, y, z], "minecraft:glass".to_string()));
            }
        }
        let mobs: Vec<(String, [f64; 3], Option<String>)> = vec![
            ("minecraft:villager".to_string(), [7.5, 64.0, 6.5], Some("{PersistenceRequired:1b}".to_string())),
            ("minecraft:villager".to_string(), [7.5, 64.0, 8.5], Some("{PersistenceRequired:1b}".to_string())),
        ];
        v.push(Scene {
            name: "ticket-release".into(),
            chunks: [0, 0, 0, 0],
            blocks: b,
            mobs,
            day_time: 1000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [15, 67, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's recoverability) one bed (head (5,64,6)); villagers penned
    // in glass (a 1x1 cell, 2 high, roofed) so the path to the bed never reaches: their streak
    // builds in REST, a freeze of their chunk (900 for 1500 gt) releases HOME on the resume
    // (releasePoi: the ticket back, at most the record's 1), then the pens open (their west wall,
    // t2500) and AcquirePoi may claim the bed again. "recover-single": V (e0) penned 3 blocks east,
    // HOME and the ticket at t1. "recover-shared": V1 (e0) and V2 (e1) sharing the bed the way the
    // build does: V1's HOME and ticket, the bed broken and placed again (a fresh record, its ticket
    // free), V2's HOME and ticket; both penned in the frozen chunk. "recover-occupied": S (e0)
    // asleep in the bed (let out at t100), V (e1) penned 17 blocks east in chunk (1, 0) (beyond
    // ValidateNearbyPoi's 16) with HOME on the bed from before the break (its ticket gone with the
    // old record), S's ticket on the new one; V's chunk frozen; dawn at t4000 (day time 20000)
    for name in ["recover-single", "recover-shared", "recover-occupied"] {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        b.push(([5, 64, 5], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        b.push(([5, 64, 6], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        let pens: Vec<[i32; 2]> = match name {
            "recover-single" => vec![[8, 6]],
            "recover-shared" => vec![[8, 6], [8, 9]],
            _ => vec![[8, 6], [22, 6]],
        };
        for [x, z] in &pens {
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                for y in 64..=65 {
                    b.push(([x + dx, y, z + dz], "minecraft:glass".to_string()));
                }
            }
            b.push(([*x, 66, *z], "minecraft:glass".to_string()));
        }
        let mobs: Vec<(String, [f64; 3], Option<String>)> = pens.iter().map(|[x, z]| ("minecraft:villager".to_string(), [*x as f64 + 0.5, 64.0, *z as f64 + 0.5], Some("{PersistenceRequired:1b}".to_string()))).collect();
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: if name == "recover-occupied" { 20000 } else { 13000 },
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 67, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec: the re-claim delay after a long failure) three villagers,
    // each homeless and penned in glass 3 blocks east of its own bed (the path never reaches: every
    // AcquirePoi attempt fails, its retry interval to that bed growing, JitteredLinearRetry: +40..80
    // gt a failure up to 400), each pen opened at its own time (V0 t300, V1 t1500, V2 t4000); night
    for name in ["reclaim-backoff", "reclaim-backoff-day", "reclaim-backoff-1"] {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        // ("-1": V0 and its bed only, the one-bed control)
        for k in 0..(if name == "reclaim-backoff-1" { 1 } else { 3 }) {
            let (bx, px) = (3 + 10 * k, 6 + 10 * k);
            b.push(([bx, 64, 5], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
            b.push(([bx, 64, 6], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                for y in 64..=65 {
                    b.push(([px + dx, y, 6 + dz], "minecraft:glass".to_string()));
                }
            }
            b.push(([px, 66, 6], "minecraft:glass".to_string()));
            mobs.push(("minecraft:villager".to_string(), [px as f64 + 0.5, 64.0, 6.5], Some("{PersistenceRequired:1b}".to_string())));
        }
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            // ("-day": in the day, IDLE: AcquirePoi(HOME) runs in CORE all day)
            day_time: if name.ends_with("-day") { 1000 } else { 13000 },
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 67, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec / ironfarm's merges: AbstractMinecart.push skips a pair
    // closer than 0.01 horizontally, dist² < 1e-4) four lines of flat rail between two stones
    // (x 3..20), two minecarts each, summoned moving east at 0.2: A (z 2) at the identical spot with
    // villagers riding; B (z 5) 0.005 apart (dist² 2.5e-5) with riders; C (z 8) 0.02 apart (dist²
    // 4e-4: the positive control, pushed apart) with riders; D (z 11) empty at the identical spot;
    // a zombie in a boat in the riders' sight (they panic)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        for z in [2, 5, 8, 11] {
            for x in 3..=20 {
                b.push(([x, 64, z], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
            }
            b.push(([2, 64, z], stone.to_string()));
            b.push(([21, 64, z], stone.to_string()));
        }
        let rider = "{Motion:[0.2d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let rider_rest = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let empty_rest = "{}";
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        // A: moving together into the stop (the merge forms there); B: at rest 0.005 apart; C: at
        // rest 0.02 apart with riders; D: at rest 0.02 apart, empty
        for (z, dx, data) in [(2.5, 0.0, rider), (5.5, 0.005, rider_rest), (8.5, 0.02, rider_rest), (11.5, 0.02, empty_rest)] {
            mobs.push(("minecraft:minecart".into(), [10.5, 64.0, z], Some(data.into())));
            mobs.push(("minecraft:minecart".into(), [10.5 + dx, 64.0, z], Some(data.into())));
        }
        mobs.push(("minecraft:oak_boat".into(), [12.5, 64.0, 14.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        v.push(Scene {
            name: "cart-merge".into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 67, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, the reviewer via ironspec: does a calm rider following a partial path
    // push its neighbour?) three lines, each a slope down east (rails ascending west x 14..19, y
    // 69..64) against two stones at its foot (20, 64..65): an empty cart at the foot and a ridden
    // one pressed behind it up the slope, as a queue presses them; night, no hostile (the riders
    // calm in REST, each walking to its HOME): R (z 3) HOME on a bed on the open floor (its path
    // reaches: the control), P (z 8) HOME on a bed boxed in glass with the roof two over it (the
    // siding's: the cell over the bed not standable, so a partial path), N (z 13) HOME in an
    // unloaded chunk (x 100: no path to it; MoveToTargetSink's random fallback toward it)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        for z in [3, 8, 13] {
            for (k, x) in (14..=19).rev().enumerate() {
                let y = 64 + k as i32;
                for yy in 64..y {
                    b.push(([x, yy, z], stone.to_string()));
                }
                b.push(([x, y, z], "minecraft:rail[shape=ascending_west,waterlogged=false]".to_string()));
            }
            // (tickexact 2026-11-11, ironspec's raw-placement audit: the top rail against stone on
            // its high side, so no update pops it)
            b.push(([13, 69, z], stone.to_string()));
            b.push(([20, 64, z], stone.to_string()));
            b.push(([20, 65, z], stone.to_string()));
        }
        // R's bed on the floor; P's in a glass box, roofed at y 66 (the cell over the bed air)
        b.push(([16, 64, 5], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        b.push(([16, 64, 6], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        for x in 15..=17 {
            for z in 9..=12 {
                for y in 64..=66 {
                    let inside = x == 16 && (z == 10 || z == 11) && y <= 65;
                    if !inside {
                        b.push(([x, y, z], "minecraft:glass".to_string()));
                    }
                }
            }
        }
        b.push(([16, 64, 10], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        b.push(([16, 64, 11], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        for z in [3.5, 8.5, 13.5] {
            mobs.push(("minecraft:minecart".into(), [19.5, 64.5, z], None));
            mobs.push(("minecraft:minecart".into(), [18.5, 65.5, z], Some(rider.into())));
        }
        v.push(Scene {
            name: "rider-push".into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: 13000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 74, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-10, the boarding cooldown across a reload) a ridden minecart over a
    // powered activator rail throws its villager out (`ejectPassengers`: boardingCooldown 60);
    // a second, empty cart on the parallel line behind it comes through the villager's cell
    // within those 60 gt and must not take it; "board-cooldown-reload" unloads and loads the whole
    // part for 1 gt right after the throw: the game does not save the cooldown, the second cart
    // takes the villager (scene_actions)
    for name in ["board-cooldown", "board-cooldown-reload"] {
        let mut b = Vec::new();
        floor(&mut b, 0, 16, -16, 32, 63, stone);
        for z in -12..=24 {
            for x in [3, 4] {
                let rail = if x == 4 && z == 8 { "minecraft:activator_rail[powered=true,shape=north_south,waterlogged=false]" } else { "minecraft:rail[shape=north_south,waterlogged=false]" };
                b.push(([x, 64, z], rail.to_string()));
            }
        }
        b.push(([4, 63, 8], "minecraft:redstone_block".to_string()));
        b.push(([3, 64, 25], stone.to_string()));
        b.push(([4, 64, 25], stone.to_string()));
        v.push(Scene {
            name: name.into(),
            chunks: [0, -1, 0, 1],
            blocks: b,
            mobs: vec![
                ("minecraft:minecart".into(), [4.5, 64.0, 6.5], Some("{Motion:[0.0d,0.0d,0.3d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into())),
                ("minecraft:minecart".into(), [3.5, 64.0, -0.5], Some("{Motion:[0.0d,0.0d,0.5d]}".into())),
            ],
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, -16], [15, 68, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-10, ironspec's contact request) a gravity queue of six minecarts, each
    // with a villager, pressed together down a rail slope against a stone at its foot, the riders
    // panicking at a zombie in a boat in sight (their travel pushes the carts); every 200 gt from
    // t100 the stone turns to a rail for 12 gt and back, letting the front carts roll west onto a
    // flat run (scene_actions: "contact6"; "contact6-unload" adds the whole part's and partial
    // unloads)
    for name in ["contact6", "contact6-unload"] {
        let mut b = Vec::new();
        floor(&mut b, -12, 17, 0, 9, 63, stone);
        b.push(([-11, 64, 2], stone.to_string()));
        for x in -10..=2 {
            b.push(([x, 64, 2], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
        }
        b.push(([3, 64, 2], stone.to_string()));
        b.push(([4, 64, 2], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
        for x in 5..=10 {
            let y = 64 + (x - 5);
            if y > 64 {
                b.push(([x, y - 1, 2], stone.to_string()));
            }
            b.push(([x, y, 2], "minecraft:rail[shape=ascending_east,waterlogged=false]".to_string()));
        }
        // (tickexact 2026-11-11, ironspec's raw-placement audit: the top rail against stone on its
        // high side, so no update pops it)
        b.push(([11, 69, 2], stone.to_string()));
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = vec![("minecraft:minecart".into(), [4.5, 64.0, 2.5], Some(rider.into()))];
        for x in 5..=9 {
            mobs.push(("minecraft:minecart".into(), [x as f64 + 0.5, 64.5 + (x - 5) as f64, 2.5], Some(rider.into())));
        }
        mobs.push(("minecraft:oak_boat".into(), [7.5, 64.0, 7.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        v.push(Scene {
            name: name.into(),
            chunks: [-1, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([-12, 62, 0], [16, 72, 8]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's comb-stand option S2: a wrong-side entry into a curve) 24
    // lines along x, each a straight east-west run to x 9 into a curve cell at x 10 entered from
    // its unconnected west side: lines 0..11 a north_east curve (its exits north, onto a north-south
    // run, and east, onto an east-west run), 12..23 a south_east one; per shape the cart at x 9.5
    // moving +x at 0.05, 0.1, 0.2, 0.4 b/t and a stored 0.8 and 2.0 (the speed off boosters past the
    // 0.4 cap), empty and with a villager riding
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 112, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let speeds = [0.05, 0.1, 0.2, 0.4, 0.8, 2.0];
        for k in 0..24usize {
            let z = 2 + 4 * k as i32;
            let north = k < 12;
            for x in 2..=9 {
                b.push(([x, 64, z], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
            }
            b.push(([1, 64, z], stone.to_string()));
            b.push(([10, 64, z], format!("minecraft:rail[shape={},waterlogged=false]", if north { "north_east" } else { "south_east" })));
            for x in 11..=24 {
                b.push(([x, 64, z], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
            }
            b.push(([25, 64, z], stone.to_string()));
            let dz = if north { -1 } else { 1 };
            b.push(([10, 64, z + dz], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            b.push(([10, 64, z + 2 * dz], stone.to_string()));
            let v = speeds[k % 6];
            let ridden = (k % 12) >= 6;
            let nbt = if ridden { format!("{{Motion:[{v:.2}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}") } else { format!("{{Motion:[{v:.2}d,0.0d,0.0d]}}") };
            mobs.push(("minecraft:minecart".into(), [9.5, 64.0, z as f64 + 0.5], Some(nbt)));
        }
        v.push(Scene {
            name: "curve-wrong-side".into(),
            chunks: [0, 0, 1, 6],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 66, 111]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's code reads bridged) east-west lines (rails x 2..29), night,
    // every empty cart still at 0.1 b/t or more when its inflated box first meets the mob: A (z 2)
    // at 0.15 onto a zombie standing on the track (the pickup); B (z 6) at 0.08 (below 0.1: no
    // pickup); D (z 14) at 0.3 toward a glass pane across the track (x 7) with a zombie pressed
    // behind it, the cart's box 0.05 short of the pane when its inflated box meets the zombie (picked
    // up through the pane: pushAndPickupEntities reads the box only); G (z 28) at 0.3 past a zombie
    // behind a glass pane along the track (z 29: the pane keeps it out of the box); E (z 20) an empty
    // cart and F (z 24) a cart with a villager rider, both at 0.4 on the flat (the natural slowdown,
    // 0.96 against 0.997). (The game also boards a boat, with the zombie in it, into a moving empty
    // cart: redsim's named gap, "a moving minecart picking up a boat", not in this scene.)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 32, 63, stone);
        for z in [2, 6, 10, 14, 20, 24, 28] {
            for x in 2..=29 {
                if !(z == 14 && x == 7) {
                    b.push(([x, 64, z], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
                }
            }
            b.push(([1, 64, z], stone.to_string()));
            b.push(([30, 64, z], stone.to_string()));
        }
        for y in [64, 65] {
            b.push(([7, y, 14], "minecraft:glass_pane[east=false,north=true,south=true,waterlogged=false,west=false]".to_string()));
            b.push(([7, y, 13], stone.to_string()));
            b.push(([7, y, 15], stone.to_string()));
            for x in 7..=14 {
                b.push(([x, y, 29], "minecraft:glass_pane[east=true,north=false,south=false,waterlogged=false,west=true]".to_string()));
            }
            b.push(([6, y, 29], stone.to_string()));
            b.push(([15, y, 29], stone.to_string()));
        }
        let zombie = || Some("{PersistenceRequired:1b}".to_string());
        let mobs: Vec<(String, [f64; 3], Option<String>)> = vec![
            ("minecraft:minecart".into(), [5.5, 64.0, 2.5], Some("{Motion:[0.15d,0.0d,0.0d]}".into())),
            ("minecraft:zombie".into(), [7.0, 64.0, 2.5], zombie()),
            ("minecraft:minecart".into(), [5.5, 64.0, 6.5], Some("{Motion:[0.08d,0.0d,0.0d]}".into())),
            ("minecraft:zombie".into(), [6.9, 64.0, 6.5], zombie()),
            ("minecraft:minecart".into(), [6.0701, 64.0, 14.5], Some("{Motion:[0.3d,0.0d,0.0d]}".into())),
            ("minecraft:zombie".into(), [7.87, 64.0, 14.5], zombie()),
            ("minecraft:minecart".into(), [3.5, 64.0, 20.5], Some("{Motion:[0.4d,0.0d,0.0d]}".into())),
            ("minecraft:minecart".into(), [3.5, 64.0, 24.5], Some("{Motion:[0.4d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into())),
            ("minecraft:minecart".into(), [5.5, 64.0, 28.5], Some("{Motion:[0.3d,0.0d,0.0d]}".into())),
            ("minecraft:zombie".into(), [10.5, 64.0, 29.87], zombie()),
        ];
        v.push(Scene {
            name: "cart-rules".into(),
            chunks: [0, 0, 1, 1],
            blocks: b,
            mobs,
            day_time: 18000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 67, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's / farmopt's slope merge: cc767b78's bench showed a cart
    // arriving behind an empty cart pressed on a closed gate on a slope land on its exact spot) 13
    // lanes 2 apart (x = 1 + 2i), each a closed oak gate at (x,64,3) at the foot of three ascending
    // rails (x,64,4), (x,65,5), (x,66,6) rising south, then flat rails at y 67 to z 12; lanes 0..4 an
    // empty cart pressed on the gate and a ridden follower set off down the slope from (x,66,6.5)
    // at stored 0, 0.04, 0.2, 0.5 and 2.0; lanes 5..9 the same with an empty follower; lanes 10..12 a
    // pressed queue of three carts let go at rest (z 4.5, 5.5, 6.5), the empty one at the front, the
    // middle, the back, the others ridden
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 47, 0, 15, 63, stone);
        let rail = |shape: &str| format!("minecraft:rail[shape={shape},waterlogged=false]");
        for i in 0..23 {
            let x = 1 + 2 * i;
            b.push(([x, 64, 3], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string()));
            b.push(([x, 64, 4], rail("ascending_south")));
            b.push(([x, 64, 5], stone.to_string()));
            b.push(([x, 65, 5], rail("ascending_south")));
            for y in 64..=65 {
                b.push(([x, y, 6], stone.to_string()));
            }
            b.push(([x, 66, 6], rail("ascending_south")));
            for z in 7..=12 {
                for y in 64..=66 {
                    b.push(([x, y, z], stone.to_string()));
                }
                b.push(([x, 67, z], rail("north_south")));
            }
        }
        let cart = |x: i32, y: f64, z: f64, v: f64, ridden: bool| {
            let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
            ("minecraft:minecart".to_string(), [x as f64 + 0.5, y, z], Some(format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", -v)))
        };
        let mut mobs = Vec::new();
        let speeds = [0.0, 0.04, 0.2, 0.5, 2.0];
        for (k, sp) in speeds.iter().enumerate() {
            for (lane, ridden) in [(k, true), (k + 5, false)] {
                let x = 1 + 2 * lane as i32;
                mobs.push(cart(x, 64.0, 4.5, 0.0, false));
                mobs.push(cart(x, 66.0, 6.5, *sp, ridden));
            }
        }
        for (q, empty_at) in [(10, 0), (11, 1), (12, 2)] {
            let x = 1 + 2 * q;
            for (j, (y, z)) in [(64.0, 4.5), (65.0, 5.5), (66.0, 6.5)].into_iter().enumerate() {
                mobs.push(cart(x, y, z, 0.0, j != empty_at));
            }
        }
        // (lanes 13..22: the merge's threshold, followers at stored 0.6, 0.8, 1.0, 1.2 and 1.5,
        // ridden then empty)
        for (k, sp) in [0.6, 0.8, 1.0, 1.2, 1.5].iter().enumerate() {
            for (lane, ridden) in [(13 + k, true), (18 + k, false)] {
                let x = 1 + 2 * lane as i32;
                mobs.push(cart(x, 64.0, 4.5, 0.0, false));
                mobs.push(cart(x, 66.0, 6.5, *sp, ridden));
            }
        }
        v.push(Scene {
            name: "slope-merge".into(),
            chunks: [0, 0, 2, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [47, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's merge bridges (b): cc767b78's redsim run stops a follower dead
    // 0.79 short of a RIDDEN cart pressed on the gate, against its rider's box) slope-merge's lanes
    // with the front cart ridden: 20 lanes, followers at stored 0, 0.04, 0.2, 0.5, 0.6, 0.8, 1.0, 1.2,
    // 1.5 and 2.0, ridden (lanes 0..9) then empty (lanes 10..19)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 47, 0, 15, 63, stone);
        let rail = |shape: &str| format!("minecraft:rail[shape={shape},waterlogged=false]");
        for i in 0..20 {
            let x = 1 + 2 * i;
            b.push(([x, 64, 3], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string()));
            b.push(([x, 64, 4], rail("ascending_south")));
            b.push(([x, 64, 5], stone.to_string()));
            b.push(([x, 65, 5], rail("ascending_south")));
            for y in 64..=65 {
                b.push(([x, y, 6], stone.to_string()));
            }
            b.push(([x, 66, 6], rail("ascending_south")));
            for z in 7..=12 {
                for y in 64..=66 {
                    b.push(([x, y, z], stone.to_string()));
                }
                b.push(([x, 67, z], rail("north_south")));
            }
        }
        let cart = |x: i32, y: f64, z: f64, v: f64, ridden: bool| {
            let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
            ("minecraft:minecart".to_string(), [x as f64 + 0.5, y, z], Some(format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", -v)))
        };
        let mut mobs = Vec::new();
        for (k, sp) in [0.0, 0.04, 0.2, 0.5, 0.6, 0.8, 1.0, 1.2, 1.5, 2.0].iter().enumerate() {
            for (lane, ridden) in [(k, true), (k + 10, false)] {
                let x = 1 + 2 * lane as i32;
                mobs.push(cart(x, 64.0, 4.5, 0.0, true));
                mobs.push(cart(x, 66.0, 6.5, *sp, ridden));
            }
        }
        v.push(Scene {
            name: "slope-merge-ridden".into(),
            chunks: [0, 0, 2, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [47, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's merge bridges (3): the threshold's sensitivity) an empty cart
    // held at a closed gate at (x,64,3) and an empty follower set off from z 7.5 at stored 0.6, 0.7,
    // 0.8, 0.9, 1.0 and 1.2, in five geometries of six lanes each (x = 1 + 2i): flat track (the front
    // at rest touching the gate); one ascending rail before the gate (the flat at y 65); two (y 66);
    // three (y 67, slope-merge's slope, the follower from the flat); and three with the front cart
    // moving, let go from (x,65,5.5) at stored 0.2 toward the gate as the follower sets off
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 63, 0, 15, 63, stone);
        let rail = |shape: &str| format!("minecraft:rail[shape={shape},waterlogged=false]");
        let cart = |x: i32, y: f64, z: f64, v: f64| ("minecraft:minecart".to_string(), [x as f64 + 0.5, y, z], Some(format!("{{Motion:[0.0d,0.0d,{:.4}d]}}", -v)));
        let mut mobs = Vec::new();
        let speeds = [0.6, 0.7, 0.8, 0.9, 1.0, 1.2];
        // (geometry g: the slopes before the gate, and whether the front moves)
        for (g, (slopes, moving)) in [(0, false), (1, false), (2, false), (3, false), (3, true)].into_iter().enumerate() {
            for (k, sp) in speeds.iter().enumerate() {
                let x = 1 + 2 * (g * 6 + k) as i32;
                b.push(([x, 64, 3], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string()));
                for s in 0..slopes {
                    let y = 64 + s;
                    for yy in 64..y {
                        b.push(([x, yy, 4 + s], stone.to_string()));
                    }
                    b.push(([x, y, 4 + s], rail("ascending_south")));
                }
                let top = 64 + slopes;
                for z in (4 + slopes)..=12 {
                    for yy in 64..top {
                        b.push(([x, yy, z], stone.to_string()));
                    }
                    b.push(([x, top, z], rail("north_south")));
                }
                if moving {
                    mobs.push(cart(x, 65.0, 5.5, 0.2));
                } else {
                    mobs.push(cart(x, 64.0, 4.5, 0.0));
                }
                mobs.push(cart(x, top as f64, 7.5, *sp));
            }
        }
        v.push(Scene {
            name: "merge-threshold".into(),
            chunks: [0, 0, 3, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [63, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's follow-up: "never merges" on flat track and one slope rail
    // must cover the farm's speeds, up to 1.96 stored at R1's take-off) merge-threshold's flat and
    // one-rail geometries with followers at stored 1.2, 1.4, 1.6, 1.8, 2.0 and 2.2, empty (lanes
    // 0..11: flat, then one rail) and ridden (lanes 12..23: flat, then one rail); the front empty
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 63, 0, 15, 63, stone);
        let rail = |shape: &str| format!("minecraft:rail[shape={shape},waterlogged=false]");
        let cart = |x: i32, y: f64, z: f64, v: f64, ridden: bool| {
            let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
            ("minecraft:minecart".to_string(), [x as f64 + 0.5, y, z], Some(format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", -v)))
        };
        let mut mobs = Vec::new();
        let speeds = [1.2, 1.4, 1.6, 1.8, 2.0, 2.2];
        for (r, ridden) in [false, true].into_iter().enumerate() {
            for slopes in 0..2 {
                for (k, sp) in speeds.iter().enumerate() {
                    let x = 1 + 2 * (r * 12 + slopes as usize * 6 + k) as i32;
                    b.push(([x, 64, 3], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string()));
                    if slopes == 1 {
                        b.push(([x, 64, 4], rail("ascending_south")));
                    }
                    let top = 64 + slopes;
                    for z in (4 + slopes)..=12 {
                        for yy in 64..top {
                            b.push(([x, yy, z], stone.to_string()));
                        }
                        b.push(([x, top, z], rail("north_south")));
                    }
                    mobs.push(cart(x, 64.0, 4.5, 0.0, false));
                    mobs.push(cart(x, top as f64, 7.5, *sp, ridden));
                }
            }
        }
        v.push(Scene {
            name: "merge-flat".into(),
            chunks: [0, 0, 3, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [63, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's frozen-rider sensors: the router interlock's tripwire at the
    // hump top and the siding's SP plate) a tripwire at rider height (y 65, P + 1) across a track
    // along z (P = 64): hooks at x0 -/+ 3 on stones at x0 -/+ 4, the string x0-2..x0+2 at z 16. In
    // the forced chunks: seated riders moving +z at 0.1, 0.25, 0.4 and 0.5, one at rest under the
    // string, a bare cart at 0.4, and a standing adult under it (held: glass at z 15 and 17, under
    // and over the string beside it: the positive control). In chunk 1, frozen at level 32 from t40
    // to t240 (scene_actions: "rider-sensors"): a seated rider at rest under a string (x0 20) and one
    // under a stone plate at P + 2 on glass (x 28), each on a powered rail against a stone at its
    // -z end, powered at t260 to kick it on +z
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 112, 0, 32, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let string = |b: &mut Vec<([i32; 3], String)>, x0: i32| {
            b.push(([x0 - 4, 65, 16], stone.to_string()));
            b.push(([x0 + 4, 65, 16], stone.to_string()));
            b.push(([x0 - 3, 65, 16], "minecraft:tripwire_hook[attached=true,facing=east,powered=false]".to_string()));
            b.push(([x0 + 3, 65, 16], "minecraft:tripwire_hook[attached=true,facing=west,powered=false]".to_string()));
            for x in x0 - 2..=x0 + 2 {
                b.push(([x, 65, 16], "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]".to_string()));
            }
        };
        let track = |b: &mut Vec<([i32; 3], String)>, x0: i32| {
            b.push(([x0, 64, 0], stone.to_string()));
            b.push(([x0, 64, 31], stone.to_string()));
            for z in 1..=30 {
                b.push(([x0, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
        };
        // the forced chunks' lanes
        for (k, kind) in ["r0.1", "r0.25", "r0.4", "r0.5", "r0", "e0.4"].iter().enumerate() {
            let x0 = 38 + 10 * k as i32;
            string(&mut b, x0);
            track(&mut b, x0);
            let v: f64 = kind[1..].parse().unwrap_or(0.0);
            let z0 = if v == 0.0 { 16.5 } else { 16.5 - (v * 10.0 + 2.5).round() };
            let nbt = if kind.starts_with('r') { rider.replace("{Passengers", &format!("{{Motion:[0.0d,0.0d,{v:.2}d],Passengers")) } else { format!("{{Motion:[0.0d,0.0d,{v:.2}d]}}") };
            mobs.push(("minecraft:minecart".into(), [x0 as f64 + 0.5, 64.0625, z0], Some(nbt)));
        }
        // the positive control: a standing adult under a string, held
        {
            let x0 = 98;
            string(&mut b, x0);
            for z in [15, 17] {
                for y in 64..=65 {
                    b.push(([x0, y, z], "minecraft:glass".to_string()));
                }
            }
            for x in [x0 - 1, x0 + 1] {
                b.push(([x, 64, 16], "minecraft:glass".to_string()));
                b.push(([x, 66, 16], "minecraft:glass".to_string()));
            }
            mobs.push(("minecraft:villager".into(), [x0 as f64 + 0.5, 64.0, 16.5], Some("{PersistenceRequired:1b}".into())));
        }
        // the frozen chunk's lanes: a string (x0 20) and a stone plate (x 28)
        for (x0, plate) in [(20, false), (28, true)] {
            if plate {
                b.push(([x0, 65, 16], "minecraft:glass".to_string()));
                b.push(([x0, 66, 16], "minecraft:stone_pressure_plate[powered=false]".to_string()));
            } else {
                string(&mut b, x0);
            }
            track(&mut b, x0);
            b.push(([x0, 64, 15], stone.to_string()));
            b.push(([x0, 64, 16], "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]".to_string()));
            mobs.push(("minecraft:minecart".into(), [x0 as f64 + 0.5, 64.0625, 16.5], Some(rider.to_string())));
        }
        v.push(Scene {
            name: "rider-sensors".into(),
            chunks: [0, 0, 6, 1],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [111, 68, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's eye-in-block riders: the siding's glass cap over its booster,
    // and a closed gate over a rider) seated riders in carts held on one rail cell (P = 64) with,
    // in the cell over the cart (where the rider's eye sits, P + 1.68): glass (x 4), a closed oak
    // fence gate facing north (its 1.5-high collision box across the eye, x 12), nothing (the
    // control, x 20); each with a zombie in a boat 7 blocks +z in plain view, under a roof (so it
    // does not burn): the sight ray (ClipContext COLLIDER from the eye) starts inside the block's
    // collision shape
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 32, 63, stone);
        floor(&mut b, 0, 32, 0, 32, 72, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        for (x, cap) in [(4, Some("minecraft:glass")), (12, Some("minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]")), (20, None)] {
            b.push(([x, 64, 7], stone.to_string()));
            b.push(([x, 64, 8], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            b.push(([x, 64, 9], stone.to_string()));
            if let Some(c) = cap {
                b.push(([x, 65, 8], c.to_string()));
            }
            mobs.push(("minecraft:minecart".into(), [x as f64 + 0.5, 64.0625, 8.5], Some(rider.to_string())));
            mobs.push(("minecraft:oak_boat".into(), [x as f64 + 0.5, 64.0, 15.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        }
        v.push(Scene {
            name: "rider-eye-block".into(),
            chunks: [0, 0, 1, 1],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 70, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's roll-back question ahead of 58f770cc's geometry: a halted
    // empty cart on an unpowered powered-rail slope must roll back) lanes along z: three rails
    // ascending south ((x,64,4), (x,65,5), (x,66,6), each on stone), flat rails at y 64 from z 1 to 3
    // below them (a stone at z 0) and at y 67 from z 7 to 12 above (a stone at z 13); the slope of
    // unpowered powered rails or of plain rails; a cart at rest on the middle slope rail, empty or
    // ridden, or one rolling up it from the flat below at 0.3
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        // (lane x, powered-rail slope, the cart: "rest-e" / "rest-r" / "up-e")
        for (x, powered, cart) in [(2, true, "rest-e"), (5, true, "rest-r"), (8, true, "up-e"), (11, false, "rest-e"), (14, false, "rest-r"), (17, false, "up-e")] {
            b.push(([x, 64, 0], stone.to_string()));
            for z in 1..=3 {
                b.push(([x, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            for (k, z) in (4..=6).enumerate() {
                let y = 64 + k as i32;
                for yy in 64..y {
                    b.push(([x, yy, z], stone.to_string()));
                }
                b.push(([x, y, z], if powered { "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]" } else { "minecraft:rail[shape=ascending_south,waterlogged=false]" }.to_string()));
            }
            for z in 7..=12 {
                for yy in 64..67 {
                    b.push(([x, yy, z], stone.to_string()));
                }
                b.push(([x, 67, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            for yy in 64..68 {
                b.push(([x, yy, 13], stone.to_string()));
            }
            let (pos, nbt) = match cart {
                "rest-e" => ([x as f64 + 0.5, 65.5625, 5.5], None),
                "rest-r" => ([x as f64 + 0.5, 65.5625, 5.5], Some(rider.to_string())),
                _ => ([x as f64 + 0.5, 64.0625, 2.5], Some("{Motion:[0.0d,0.0d,0.3d]}".to_string())),
            };
            mobs.push(("minecraft:minecart".into(), pos, nbt));
        }
        v.push(Scene {
            name: "rollback-halt".into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, 58f770cc's roll-back after the booster: the exit branch's rise past a
    // booster the ridden signal powers; an empty cart gets no boost, stalls on the plain slope and
    // must roll back west through the booster) lanes along z: flat rails z 1..5 (a stone at z 0), an
    // unpowered powered rail (the booster, flat) at z 6, two plain rails ascending south (z 7, 8),
    // flat rails at y 66 z 9..12 (a stone at z 13); an empty cart from z 2.5 at stored 0.1..1.0
    // (x 2..29, every 3), and the same lanes with a plain rail for the booster (x 32..59)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 64, 0, 16, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        for (k, booster) in (0..20).map(|k| (k, k < 10)) {
            let x = 2 + 3 * k as i32;
            let v = 0.1 * ((k % 10) + 1) as f64;
            b.push(([x, 64, 0], stone.to_string()));
            for z in 1..=5 {
                b.push(([x, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            b.push(([x, 64, 6], if booster { "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]" } else { "minecraft:rail[shape=north_south,waterlogged=false]" }.to_string()));
            b.push(([x, 64, 7], "minecraft:rail[shape=ascending_south,waterlogged=false]".to_string()));
            b.push(([x, 64, 8], stone.to_string()));
            b.push(([x, 65, 8], "minecraft:rail[shape=ascending_south,waterlogged=false]".to_string()));
            for z in 9..=12 {
                b.push(([x, 64, z], stone.to_string()));
                b.push(([x, 65, z], stone.to_string()));
                b.push(([x, 66, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            for yy in 64..67 {
                b.push(([x, yy, 13], stone.to_string()));
            }
            mobs.push(("minecraft:minecart".into(), [x as f64 + 0.5, 64.0625, 2.5], Some(format!("{{Motion:[0.0d,0.0d,{v:.2}d]}}"))));
        }
        v.push(Scene {
            name: "rollback-booster".into(),
            chunks: [0, 0, 3, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [63, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, farmopt's hold H: (a) the pressed train's release through H for W =
    // 1, 3, 5, 7 gt, ridden and empty; (b) H shut 1..10 gt after it opens on one pressed cart, so
    // it shuts with the cart in its box at each tick of the passage) gate-frozen's geometry: a
    // fence gate at (x,64,7) at the foot of three rails rising south ((x,64,8), (x,65,9),
    // (x,66,10)), a flat at y 67 to z 14, the gate's cell over a rail one level down ((x,63,4..7),
    // stone at z 3); the carts pressed down the slope against the shut gate (front at z 8.115, the
    // next 0.79 behind each); lanes x 1, 3, ... (gate_hold_lanes; scene_actions: H opened at t30)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 64, 0, 16, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        for (k, (n, ridden, _)) in gate_hold_lanes().into_iter().enumerate() {
            let x = 1 + 2 * k as i32;
            b.push(([x, 64, 7], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string()));
            for (s, z) in (8..=10).enumerate() {
                let y = 64 + s as i32;
                for yy in 64..y {
                    b.push(([x, yy, z], stone.to_string()));
                }
                b.push(([x, y, z], "minecraft:rail[shape=ascending_south,waterlogged=false]".to_string()));
            }
            for z in 11..=14 {
                for yy in 64..67 {
                    b.push(([x, yy, z], stone.to_string()));
                }
                b.push(([x, 67, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            for z in 4..=7 {
                b.push(([x, 62, z], stone.to_string()));
                b.push(([x, 63, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            for c in 0..n {
                let z = 8.115 + 0.79 * c as f64;
                // (on a slope rail: its height 64 + (z - 8) + 0.0625)
                let y = 64.0625 + (z - 8.0);
                mobs.push(("minecraft:minecart".to_string(), [x as f64 + 0.5, y, z], if ridden { Some(rider.to_string()) } else { None }));
            }
        }
        v.push(Scene {
            name: "gate-hold-w".into(),
            chunks: [0, 0, 3, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [63, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's side push: S3's stand B branch beside A's approach one block
    // apart) pairs of level rails along z (z 1..30 between stones), a cart on each: centres 1 apart
    // in x at the same level (carts passing at 0.1, 0.2, 0.4 in opposite directions, ridden or
    // empty; then one at rest mid-rail, the other passing it at those speeds), and the controls at
    // 0.4 and at rest: centres 2 apart, 1 apart one level up, 1 apart one level down (in a trench).
    // A pair every 4 blocks of x (the first rail at x 1 + 4k); cart A from z 5.5 moving +z, cart B
    // from z 25.5 moving -z (or at rest at z 15.5)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 112, 0, 32, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        // (the second rail's offset: dx, dy; the speed; B: 0 passing, 1 at rest as placed (yaw 0:
        // facing +x, across its rail), 2 at rest turned along its rail (Rotation 90), 3 set rolling
        // at 0.04 from z 13.5 (its yaw follows its motion); ridden)
        let mut pairs: Vec<(i32, i32, f64, u8, bool)> = Vec::new();
        for rest in [0, 1] {
            for ridden in [true, false] {
                for v in [0.1, 0.2, 0.4] {
                    pairs.push((1, 0, v, rest, ridden));
                }
            }
        }
        for (dx, dy) in [(2, 0), (1, 1), (1, -1)] {
            for rest in [0, 1] {
                for ridden in [true, false] {
                    pairs.push((dx, dy, 0.4, rest, ridden));
                }
            }
        }
        for kind in [2, 3] {
            for ridden in [true, false] {
                pairs.push((1, 0, 0.4, kind, ridden));
            }
        }
        let track = |b: &mut Vec<([i32; 3], String)>, x: i32, y: i32| {
            for yy in 64..y {
                for z in 0..=31 {
                    b.push(([x, yy, z], stone.to_string()));
                }
            }
            if y < 64 {
                for z in 0..=31 {
                    b.push(([x, y - 1, z], stone.to_string()));
                    b.push(([x, y, z], "minecraft:air".to_string()));
                }
            }
            b.push(([x, y, 0], stone.to_string()));
            b.push(([x, y, 31], stone.to_string()));
            for z in 1..=30 {
                b.push(([x, y, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
        };
        for (k, (dx, dy, v, rest, ridden)) in pairs.iter().enumerate() {
            let xa = 1 + 4 * k as i32;
            let (xb, yb) = (xa + dx, 64 + dy);
            track(&mut b, xa, 64);
            track(&mut b, xb, yb);
            let nbt = |m: f64, extra: &str| {
                let motion = format!("Motion:[0.0d,0.0d,{m:.2}d]{extra}");
                if *ridden { Some(format!("{{{motion},Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")) } else { Some(format!("{{{motion}}}")) }
            };
            let _ = rider;
            mobs.push(("minecraft:minecart".into(), [xa as f64 + 0.5, 64.0625, 5.5], nbt(*v, "")));
            match rest {
                1 => mobs.push(("minecraft:minecart".into(), [xb as f64 + 0.5, yb as f64 + 0.0625, 15.5], nbt(0.0, ""))),
                2 => mobs.push(("minecraft:minecart".into(), [xb as f64 + 0.5, yb as f64 + 0.0625, 15.5], nbt(0.0, ",Rotation:[90.0f,0.0f]"))),
                3 => mobs.push(("minecraft:minecart".into(), [xb as f64 + 0.5, yb as f64 + 0.0625, 13.5], nbt(0.04, ""))),
                _ => mobs.push(("minecraft:minecart".into(), [xb as f64 + 0.5, yb as f64 + 0.0625, 25.5], nbt(-*v, ""))),
            }
        }
        v.push(Scene {
            name: "side-push".into(),
            chunks: [0, 0, 6, 1],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 60, 0], [111, 68, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's hatch sight lines: the siding (b) FC's wake post, a zombie in
    // a boat in a glass box whose one open cell (3,7,3) holds the hatch, a top-half iron trapdoor
    // facing west: closed should leave the lines clear, open should block them) three copies of the
    // siding's geometry (siding_geometry: its redstone gone) 20 apart in z, design (x, y, z) at
    // (x + 8, y + 68, z + 5 + 20 lane), each with the design's zombie boat at (4.5,7,3.5), the hatch
    // open: lane 0 a villager at the stand-up cell (1.4,0,2.5) remembering the bed (0,0,2) as HOME
    // (it lies down: the sleeper); lane 1 a villager held at the stand-up cell (glass at (1,0,1),
    // (1,1,1) and over the bed cells (0,1,2), (0,1,3), the bed taken out); lane 2 a villager in a cart at rest on the T
    // (1,0,0). Night (REST); the hatches shut at t300, open again at t600 (scene_actions)
    {
        let d = mctas::design::Design::parse(SIDING_B_FC).expect("the frozen siding parses");
        let mut b: Vec<([i32; 3], String)> = Vec::new();
        floor(&mut b, 0, 32, 0, 64, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        for lane in 0..3 {
            let at = |p: [i32; 3]| [p[0] + 8, p[1] + 68, p[2] + 5 + 20 * lane];
            let atf = |p: [f64; 3]| [p[0] + 8.0, p[1] + 68.0, p[2] + 5.0 + 20.0 * lane as f64];
            for (p, st) in &d.blocks {
                if p[1] < -4 {
                    continue;
                }
                // (lane 1: no bed, so its villager claims none and stays awake at the stand-up cell)
                if lane == 1 && st.contains("_bed[") {
                    continue;
                }
                if let Some(s) = siding_geometry(st) {
                    b.push((at([p[0], p[1], p[2]]), s));
                }
            }
            if lane != 2 {
                for p in [[1, 0, 1], [1, 1, 1]] {
                    b.push((at(p), "minecraft:glass".to_string()));
                }
            }
            if lane == 1 {
                for p in [[0, 1, 2], [0, 1, 3]] {
                    b.push((at(p), "minecraft:glass".to_string()));
                }
            }
            mobs.push(("minecraft:oak_boat".into(), atf([4.5, 7.0, 3.5]), Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
            match lane {
                0 => {
                    let bed = at([0, 0, 2]);
                    mobs.push(("minecraft:villager".into(), atf([1.4, 0.0, 2.5]), Some(format!("{{PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}", bed[0], bed[1], bed[2]))));
                }
                1 => mobs.push(("minecraft:villager".into(), atf([1.4, 0.0, 2.5]), Some("{PersistenceRequired:1b}".into()))),
                _ => mobs.push(("minecraft:minecart".into(), atf([1.5, 0.0625, 0.5]), Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into()))),
            }
        }
        v.push(Scene {
            name: "siding-hatch".into(),
            chunks: [0, 0, 1, 3],
            blocks: b,
            mobs,
            day_time: 12500,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 80, 63]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's curve spacing: stand B's stagger-3 failure, cart 2 reaching a
    // curve while cart 1 still turns in it) two carts in sequence at a visible 0.4 a tick (stored
    // 0.4 empty, 0.6 ridden: a rider's 0.75 scale) down a straight run along +z into a 90-degree curve
    // ((x0, 64, z0 + 10), north_east) and on east; row A (z0 0): spaced 1.0, 1.2, 1.4, 1.6 blocks,
    // ridden and empty; row B (z0 24): an S-bend (a second curve (x0 + 1, 64, z0 + 10), south_west,
    // then south) at 1.0 and 1.4, ridden and empty, and a pair touching at the curve's foot, set
    // creeping at 0.02 on powered rails against a stone, powered from t1 (scene_actions); row C (z0
    // 48): the 90-degree curve at 1.8, 2.0, 2.2, 2.4; lanes x0 = 1 + 12k
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 96, 0, 64, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let cart = |x: f64, z: f64, v: f64, ridden: bool| {
            let motion = format!("Motion:[0.0d,0.0d,{v:.2}d]");
            let nbt = if ridden { format!("{{{motion},Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}") } else { format!("{{{motion}}}") };
            ("minecraft:minecart".to_string(), [x, 64.0625, z], Some(nbt))
        };
        let ns = "minecraft:rail[shape=north_south,waterlogged=false]";
        let ew = "minecraft:rail[shape=east_west,waterlogged=false]";
        // (kind: 0 the 90-degree curve, 1 the S-bend, 2 from rest at the foot; spacing; ridden)
        let mut lanes: Vec<(i32, i32, u8, f64, bool)> = Vec::new();
        let mut k = 0;
        for s in [1.0, 1.2, 1.4, 1.6] {
            for ridden in [true, false] {
                lanes.push((1 + 12 * k, 0, 0, s, ridden));
                k += 1;
            }
        }
        k = 0;
        for s in [1.0, 1.4] {
            for ridden in [true, false] {
                lanes.push((1 + 12 * k, 24, 1, s, ridden));
                k += 1;
            }
        }
        for ridden in [true, false] {
            lanes.push((1 + 12 * k, 24, 2, 1.0, ridden));
            k += 1;
        }
        // (row C, z0 48: the 90-degree curve at wider spacings)
        k = 0;
        for s in [1.8, 2.0, 2.2, 2.4] {
            for ridden in [true, false] {
                lanes.push((1 + 12 * k, 48, 0, s, ridden));
                k += 1;
            }
        }
        for (x0, z0, kind, s, ridden) in lanes {
            b.push(([x0, 64, z0], stone.to_string()));
            for z in z0 + 1..=z0 + 9 {
                b.push(([x0, 64, z], ns.to_string()));
            }
            b.push(([x0, 64, z0 + 10], "minecraft:rail[shape=north_east,waterlogged=false]".to_string()));
            if kind == 1 {
                b.push(([x0 + 1, 64, z0 + 10], "minecraft:rail[shape=south_west,waterlogged=false]".to_string()));
                for z in z0 + 11..=z0 + 19 {
                    b.push(([x0 + 1, 64, z], ns.to_string()));
                }
                b.push(([x0 + 1, 64, z0 + 20], stone.to_string()));
            } else {
                for x in x0 + 1..=x0 + 9 {
                    b.push(([x, 64, z0 + 10], ew.to_string()));
                }
                b.push(([x0 + 10, 64, z0 + 10], stone.to_string()));
            }
            let xf = x0 as f64 + 0.5;
            if kind == 2 {
                b.push(([x0, 64, z0 + 7], stone.to_string()));
                for z in [z0 + 8, z0 + 9] {
                    b.push(([x0, 64, z], "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]".to_string()));
                }
                mobs.push(cart(xf, z0 as f64 + 9.5, 0.02, ridden));
                mobs.push(cart(xf, z0 as f64 + 8.5, 0.02, ridden));
            } else {
                let v = if ridden { 0.6 } else { 0.4 };
                mobs.push(cart(xf, z0 as f64 + 5.5, v, ridden));
                mobs.push(cart(xf, z0 as f64 + 5.5 - s, v, ridden));
            }
        }
        v.push(Scene {
            name: "curve-pair".into(),
            chunks: [0, 0, 5, 3],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [95, 68, 63]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, 58f770cc's sleeper walking off its bed in redsim: a napper dropped
    // beside its bed falls asleep, then walks off it 8 gt later in 8 of 168 visits) the siding's drop:
    // a bed (head (x0, 64, 6), foot (x0, 64, 7)) beside a rail line along x0 + 1 running +z, a
    // powered activator rail (on a redstone block) at (x0 + 1, 64, 6) beside the bed's head, the
    // line on south to a stone at z 18; a villager remembering the bed as HOME rides a cart from z
    // 1.5 + k/16 at stored 0.3, night (REST: its walk to HOME set while riding), ejected at the
    // activator beside the bed; lanes x0 = 2 + 5k, k 0..15
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 80, 0, 32, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        for k in 0..16 {
            let x0 = 2 + 5 * k;
            b.push(([x0, 64, 6], "minecraft:red_bed[facing=north,occupied=false,part=head]".to_string()));
            b.push(([x0, 64, 7], "minecraft:red_bed[facing=north,occupied=false,part=foot]".to_string()));
            b.push(([x0 + 1, 64, 0], stone.to_string()));
            for z in 1..=17 {
                if z == 6 {
                    b.push(([x0 + 1, 63, z], "minecraft:redstone_block".to_string()));
                    b.push(([x0 + 1, 64, z], "minecraft:activator_rail[powered=true,shape=north_south,waterlogged=false]".to_string()));
                } else {
                    b.push(([x0 + 1, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
                }
            }
            b.push(([x0 + 1, 64, 18], stone.to_string()));
            let bed = [x0, 64, 6];
            let home = format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}", bed[0], bed[1], bed[2]);
            mobs.push(("minecraft:minecart".into(), [x0 as f64 + 1.5, 64.0625, 1.5 + k as f64 / 16.0], Some(format!("{{Motion:[0.0d,0.0d,0.3d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,{home}}}]}}"))));
        }
        v.push(Scene {
            name: "sleep-drop".into(),
            chunks: [0, 0, 4, 1],
            blocks: b,
            mobs,
            day_time: 13000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [79, 68, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's junction-settle: day-1's W siding, half-turned, routed every
    // cart to the exit; router S as built) three-arm junctions (junction_lanes): the junction cell
    // empty at t0 and placed at t1 (scene_actions), each arm 4 rails and a stone, the
    // placed-onto-power lanes' floor under it a redstone block from t0; router S's lanes with its
    // west arm turning south (a south_east curve 2 west, then south) and a longer south arm (8 rails)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 128, 0, 192, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let ns = "minecraft:rail[shape=north_south,waterlogged=false]";
        let ew = "minecraft:rail[shape=east_west,waterlogged=false]";
        for (j, missing, settle, (entry, sp, ridden, gap), router) in junction_lanes() {
            if settle == 2 {
                b.push(([j[0], 63, j[2]], "minecraft:redstone_block".to_string()));
            }
            if settle == 1 {
                // the repeater under the missing arm's first cell, its output into the stone under
                // the junction (a repeater's facing is its input side), on stone
                let d = arm_step(missing);
                let facing = ["north", "east", "south", "west"][missing as usize];
                b.push(([j[0] + d[0], 62, j[2] + d[1]], stone.to_string()));
                b.push(([j[0] + d[0], 63, j[2] + d[1]], format!("minecraft:repeater[delay=1,facing={facing},locked=false,powered=false]")));
            }
            for a in (0..4u8).filter(|a| *a != missing) {
                let d = arm_step(a);
                let len = if router && a == 2 { 8 } else if router && a == 3 { 1 } else if router { 12 } else { 4 };
                for k in 1..=len {
                    b.push(([j[0] + d[0] * k, 64, j[2] + d[1] * k], if d[0] == 0 { ns } else { ew }.to_string()));
                }
                if router && a == 3 {
                    // the west arm's curve 2 west (south_east), then south 12 and a stone
                    b.push(([j[0] - 2, 64, j[2]], "minecraft:rail[shape=south_east,waterlogged=false]".to_string()));
                    for k in 1..=12 {
                        b.push(([j[0] - 2, 64, j[2] + k], ns.to_string()));
                    }
                    b.push(([j[0] - 2, 64, j[2] + 13], stone.to_string()));
                } else {
                    b.push(([j[0] + d[0] * (len + 1), 64, j[2] + d[1] * (len + 1)], stone.to_string()));
                }
            }
            let d = arm_step(entry);
            let cart = |k: f64, ridden: bool| {
                let motion = format!("Motion:[{:.3}d,0.0d,{:.3}d]", -d[0] as f64 * sp, -d[1] as f64 * sp);
                let nbt = if ridden { format!("{{{motion},Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}") } else { format!("{{{motion}}}") };
                ("minecraft:minecart".to_string(), [j[0] as f64 + 0.5 + d[0] as f64 * k, 64.0625, j[2] as f64 + 0.5 + d[1] as f64 * k], Some(nbt))
            };
            if router {
                mobs.push(cart(3.0, ridden));
                let visible = (if ridden { 0.75 } else { 1.0 } * sp).min(0.4);
                mobs.push(cart(3.0 + visible * gap as f64, ridden));
            } else {
                mobs.push(cart(4.0, ridden));
            }
        }
        v.push(Scene {
            name: "junction-settle".into(),
            chunks: [0, 0, 7, 11],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [127, 68, 191]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, 58f770cc's sleeper walking off its bed in redsim, second bridge: the
    // siding (b) FC's own drop) 16 copies of the siding's geometry (siding_geometry, a 4 x 4 grid 20
    // apart: design (x, y, z) at (x + 8 + 20 cx, y + 68, z + 5 + 20 cz)) with its boosters (1,0,1),
    // (1,0,2) and the activator slope (1,-1,3) powered by redstone blocks under them, a ridden cart on
    // the entry detector rail (0,0,0) moving +x at 0.20 + 0.02 k (the T turns it south down the bed
    // line, the activator drops its rider beside the bed's head (0,0,2), the park's brake (1,-1,5)
    // holds the cart), the rider remembering the bed as HOME; no dorm zombie; night (REST)
    {
        let d = mctas::design::Design::parse(SIDING_B_FC).expect("the frozen siding parses");
        let mut b: Vec<([i32; 3], String)> = Vec::new();
        floor(&mut b, 0, 80, 0, 80, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        for k in 0..16 {
            let (cx, cz) = (k % 4, k / 4);
            let at = |p: [i32; 3]| [p[0] + 8 + 20 * cx, p[1] + 68, p[2] + 5 + 20 * cz];
            for (p, st) in &d.blocks {
                if p[1] < -4 {
                    continue;
                }
                if let Some(s) = siding_geometry(st) {
                    b.push((at([p[0], p[1], p[2]]), s));
                }
            }
            for (p, st) in [
                ([1, -1, 1], "minecraft:redstone_block"),
                ([1, -1, 2], "minecraft:redstone_block"),
                ([1, -2, 3], "minecraft:redstone_block"),
                ([1, 0, 1], "minecraft:powered_rail[powered=true,shape=north_south,waterlogged=false]"),
                ([1, 0, 2], "minecraft:powered_rail[powered=true,shape=north_south,waterlogged=false]"),
                ([1, -1, 3], "minecraft:activator_rail[powered=true,shape=ascending_north,waterlogged=false]"),
            ] {
                b.push((at(p), st.to_string()));
            }
            let bed = at([0, 0, 2]);
            let home = format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}", bed[0], bed[1], bed[2]);
            let start = at([0, 0, 0]);
            let v = 0.20 + 0.02 * k as f64;
            mobs.push(("minecraft:minecart".into(), [start[0] as f64 + 0.5, start[1] as f64 + 0.0625, start[2] as f64 + 0.5], Some(format!("{{Motion:[{v:.2}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,{home}}}]}}"))));
        }
        v.push(Scene {
            name: "sleep-siding".into(),
            chunks: [0, 0, 4, 4],
            blocks: b,
            mobs,
            day_time: 13000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [79, 78, 79]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, farmopt's and ironspec's H rest-place contacts: a ridden arrival into a
    // ridden cart pressed at H's 2nd place, the riderless rows' empty waiting carts, and the flat
    // last approach, and the hybrid) hcontact_lanes: 352 lanes, 26 to a row 2 apart, rows 12 apart;
    // h-contact-farmopt (300 lanes) and h-contact-sweep (384) the same way
    for name in ["h-contact", "h-contact-farmopt", "h-contact-sweep"] {
        let lanes = hcontact_lanes(name);
        let rows = (lanes.len() as i32 + 25) / 26;
        let mut b = Vec::new();
        floor(&mut b, 0, 64, 0, 12 * rows + 8, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let cart = |x: f64, y: f64, z: f64, v: f64, ridden: bool| {
            let rider = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
            ("minecraft:minecart".to_string(), [x, y, z], Some(format!("{{Motion:[0.0d,0.0d,{:.4}d]{rider}}}", -v)))
        };
        for (x, dz, flat, waits, (sp, ar), _, back) in lanes {
            b.push(([x, 64, 3 + dz], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string()));
            for z in 4..4 + flat {
                b.push(([x, 64, z + dz], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            for s in 0..3 {
                for yy in 64..64 + s {
                    b.push(([x, yy, 4 + flat + s + dz], stone.to_string()));
                }
                b.push(([x, 64 + s, 4 + flat + s + dz], "minecraft:rail[shape=ascending_south,waterlogged=false]".to_string()));
            }
            for z in (7 + flat)..=11 {
                for yy in 64..67 {
                    b.push(([x, yy, z + dz], stone.to_string()));
                }
                b.push(([x, 67, z + dz], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            b.push(([x, 67, 12 + dz], stone.to_string()));
            let xf = x as f64 + 0.5;
            // (a cart's y on this lane at z: flat 64.0625 before the slope, then 1 per block up it)
            let y_at = |z: f64| 64.0625 + (z - 4.0 - flat as f64).clamp(0.0, 3.0);
            for (z, r) in waits {
                mobs.push(cart(xf, y_at(z), z + dz as f64, 0.0, r));
            }
            let za = 8.5 + flat as f64 + back;
            mobs.push(cart(xf, y_at(za), za + dz as f64, sp, ar));
        }
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 3, (12 * rows + 7) / 16],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [63, 72, 12 * rows + 7]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's grazing eye: blindness counted from the eye point inside a
    // collision shape, as hasLineOfSight clips from the eye point, not from isInWall's eye box,
    // ±0.24) seated riders in carts at rest on a rail along z (x 4, P = 64; rails z0-1 .. z0+2), each
    // with its eye (P + 1.68) at z0 + e beside a shape at eye height: a closed oak fence gate facing
    // north at (4,65,z0) (its box z0+0.375 .. z0+0.625) with e 0.10 (clear), 0.20, 0.30, 0.36 (the
    // eye box overlaps it, the point outside), 0.40, 0.50 (the point inside); glass at (4,65,z0+1)
    // (its face z0+1: not suffocating) with e 0.70 (clear), 0.80, 0.90, 0.98 (grazing), 1.02, 1.10
    // (inside); a zombie in a boat 7 blocks +x at the eye's z (the sight ray runs along x, outside
    // the shape unless it starts in it); lanes 10 apart in z (another lane's zombie over 12 away);
    // a roof at y 72
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 16, 0, 128, 63, stone);
        floor(&mut b, 0, 16, 0, 128, 72, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let gate = "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]";
        let lanes: Vec<(bool, f64)> = [0.10, 0.20, 0.30, 0.36, 0.40, 0.50].iter().map(|e| (true, *e)).chain([0.70, 0.80, 0.90, 0.98, 1.02, 1.10].iter().map(|e| (false, *e))).collect();
        for (k, (is_gate, e)) in lanes.into_iter().enumerate() {
            let z0 = 4 + 10 * k as i32;
            b.push(([4, 64, z0 - 2], stone.to_string()));
            for z in z0 - 1..=z0 + 2 {
                b.push(([4, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            b.push(([4, 64, z0 + 3], stone.to_string()));
            if is_gate {
                b.push(([4, 65, z0], gate.to_string()));
            } else {
                b.push(([4, 65, z0 + 1], "minecraft:glass".to_string()));
            }
            let z = z0 as f64 + e;
            mobs.push(("minecraft:minecart".into(), [4.5, 64.0625, z], Some(rider.to_string())));
            mobs.push(("minecraft:oak_boat".into(), [11.5, 64.0, z], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        }
        v.push(Scene {
            name: "rider-eye-graze".into(),
            chunks: [0, 0, 0, 7],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [15, 72, 127]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, the re-path of a napper off its bed: ironspec's targeted proof of the
    // mechanism behind 58f770cc's early wakes) repath_lanes, 20 apart in x: a bed (head (x0,64,56),
    // foot (x0,64,57)) beside a rail line along x0+1 from z 2 to 62 (a stone at 63), a powered
    // activator rail beside the bed's head (on a redstone block); a ridden cart from z 2.5 at stored
    // 1.0 (visible 0.4), its rider remembering the bed as HOME; a zombie in a boat 3 blocks east of
    // the line at z 6.5 (the rider panics and takes a flee path), stone walls 3 high on both sides
    // of the line from z 12 to 48 (it calms down: REST, the flee path kept while it rides); night
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 400, 0, 64, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        for (x0, change) in repath_lanes() {
            b.push(([x0, 64, 56], "minecraft:red_bed[facing=north,occupied=false,part=head]".to_string()));
            b.push(([x0, 64, 57], "minecraft:red_bed[facing=north,occupied=false,part=foot]".to_string()));
            b.push(([x0 + 1, 64, 1], stone.to_string()));
            for z in 2..=62 {
                if z == 56 {
                    b.push(([x0 + 1, 63, z], "minecraft:redstone_block".to_string()));
                    b.push(([x0 + 1, 64, z], "minecraft:activator_rail[powered=true,shape=north_south,waterlogged=false]".to_string()));
                } else {
                    b.push(([x0 + 1, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
                }
            }
            b.push(([x0 + 1, 64, 63], stone.to_string()));
            for z in 12..=48 {
                for y in 64..67 {
                    b.push(([x0, y, z], stone.to_string()));
                    b.push(([x0 + 2, y, z], stone.to_string()));
                }
            }
            if change == 2 {
                for z in [33, 35] {
                    b.push(([x0 - 1, 64, z], stone.to_string()));
                }
            }
            let bed = [x0, 64, 56];
            let home = format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}", bed[0], bed[1], bed[2]);
            mobs.push(("minecraft:minecart".into(), [x0 as f64 + 1.5, 64.0625, 2.5], Some(format!("{{Motion:[0.0d,0.0d,1.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,{home}}}]}}"))));
            mobs.push(("minecraft:oak_boat".into(), [x0 as f64 + 4.5, 64.0, 6.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        }
        v.push(Scene {
            name: "repath-nap".into(),
            chunks: [0, 0, 24, 3],
            blocks: b,
            mobs,
            day_time: 13000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [399, 68, 63]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, G1's rows under tooFar: the job site and the bell at 30 and 60
    // Manhattan) four riders in carts held between stones on an open floor (their idle walks reach
    // their targets, so no stamp is left from before), from day time 0: a composter 30 and 60 east
    // as JOB_SITE, a bell 30 and 60 east as MEETING_POINT, each walled in stone (no path reaches it)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 96, 0, 32, 63, stone);
        let mem = |key: &str, p: [i32; 3]| format!("\"minecraft:{key}\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}", p[0], p[1], p[2]);
        let mut mobs = Vec::new();
        for (z, key, poi, state) in [
            (4, "job_site", [38, 64, 4], "minecraft:composter[level=0]"),
            (10, "job_site", [68, 64, 10], "minecraft:composter[level=0]"),
            (16, "meeting_point", [38, 64, 16], "minecraft:bell[attachment=floor,facing=north,powered=false]"),
            (22, "meeting_point", [68, 64, 22], "minecraft:bell[attachment=floor,facing=north,powered=false]"),
        ] {
            b.push(([8, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            b.push(([8, 64, z - 1], stone.to_string()));
            b.push(([8, 64, z + 1], stone.to_string()));
            b.push((poi, state.to_string()));
            for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
                b.push(([poi[0] + d[0], poi[1] + d[1], poi[2] + d[2]], stone.to_string()));
            }
            mobs.push(("minecraft:minecart".to_string(), [8.5, 64.0, z as f64 + 0.5], Some(format!("{{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{{}}}}}}}]}}", mem(key, poi)))));
        }
        v.push(Scene {
            name: "jobsite-near".into(),
            chunks: [0, 0, 5, 1],
            blocks: b,
            mobs,
            day_time: 0,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [95, 66, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's job-site rows: a villager brought in still remembering a job
    // site or a meeting point) three riders in carts held between stones, through a day from day
    // time 0: A remembers a composter 30 blocks east as its JOB_SITE (WORK, 2000..9000: the walk to
    // it, unreachable from the cart, its can't-reach stamp, and the memory forgotten 1200 gt on),
    // B a bell 30 blocks east as its MEETING_POINT (MEET, 9000..11000: tooLong 200), C a composter
    // 122 blocks east (beyond WORK's tooFar 100: DefaultRandomPos toward it)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 144, 0, 32, 63, stone);
        let mem = |key: &str, p: [i32; 3]| format!("\"minecraft:{key}\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}", p[0], p[1], p[2]);
        let mut mobs = Vec::new();
        for (z, key, poi, state) in [(8, "job_site", [38, 64, 8], "minecraft:composter[level=0]"), (20, "meeting_point", [38, 64, 20], "minecraft:bell[attachment=floor,facing=north,powered=false]"), (14, "job_site", [130, 64, 14], "minecraft:composter[level=0]")] {
            b.push(([8, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            b.push(([8, 64, z - 1], stone.to_string()));
            b.push(([8, 64, z + 1], stone.to_string()));
            // (the rider's cart in a glass cell, as a farm's riders sit: no path leaves it)
            for dz in -1..=1 {
                for y in 64..=65 {
                    b.push(([7, y, z + dz], "minecraft:glass".to_string()));
                    b.push(([9, y, z + dz], "minecraft:glass".to_string()));
                }
                for x in 7..=9 {
                    b.push(([x, 66, z + dz], "minecraft:glass".to_string()));
                }
            }
            b.push(([8, 65, z - 1], "minecraft:glass".to_string()));
            b.push(([8, 65, z + 1], "minecraft:glass".to_string()));
            b.push((poi, state.to_string()));
            // (the POI walled in stone, as a farm's riders find theirs: no path reaches it)
            for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
                b.push(([poi[0] + d[0], poi[1] + d[1], poi[2] + d[2]], stone.to_string()));
            }
            mobs.push(("minecraft:minecart".to_string(), [8.5, 64.0, z as f64 + 0.5], Some(format!("{{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Brain:{{memories:{{{}}}}}}}]}}", mem(key, poi)))));
        }
        v.push(Scene {
            name: "jobsite-memory".into(),
            chunks: [0, 0, 8, 1],
            blocks: b,
            mobs,
            day_time: 0,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [143, 66, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's flee distances: the two code distances every zombie post is
    // laid to) a villager riding a cart held between stones on a rail at floor level (its position
    // 0.0625 over the rail), and one standing on a 5-high pillar (no way down: its walks fail), each
    // with a zombie in an oak boat (0.5125 under the boat) on the same level and the same x, at 3D
    // distances 5.80, 5.95, 6.05, 6.20, 7.80, 7.95 and 8.05 along +z; and a rider with its zombie at
    // 7.0 behind a 2-high stone wall halfway (sight needed for panic). Slots 18 apart in x and 24 in
    // z (no zombie within 8 of another slot's villager); a stone roof at y 80 (no sunburn), day.
    // Expected from 26.3: NEAREST_HOSTILE (VillagerHostilesSensor: a zombie within 8, in sight)
    // and PANIC up to 8, a flee walk target only inside 6 (SetWalkTargetAwayFrom's closerThan 6)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 112, 0, 80, 63, stone);
        floor(&mut b, 0, 112, 0, 80, 80, stone);
        let mut mobs = Vec::new();
        let ds = [5.80, 5.95, 6.05, 6.20, 7.80, 7.95, 8.05];
        let mut slots: Vec<(f64, bool, bool)> = Vec::new();
        for d in ds {
            slots.push((d, true, false));
            slots.push((d, false, false));
        }
        slots.push((7.0, true, true));
        for (k, (d, riding, walled)) in slots.into_iter().enumerate() {
            let (x, z) = (4 + 18 * (k % 6) as i32, 4 + 24 * (k / 6) as i32);
            let (vy, zy) = if riding {
                // (the rider's position 0.0625 over the rail cell, the zombie 0.5125 under its boat on
                // the floor: the vertical 0.575)
                b.push(([x, 64, z], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
                b.push(([x - 1, 64, z], stone.to_string()));
                b.push(([x + 1, 64, z], stone.to_string()));
                (64.0625, 63.4875)
            } else {
                for y in 64..=68 {
                    b.push(([x, y, z], stone.to_string()));
                }
                (69.0, 68.4875)
            };
            let h = (d * d - (vy - zy) * (vy - zy)).sqrt();
            let zz = z as f64 + 0.5 + h;
            if !riding {
                // (the zombie's boat on its own pillar top at the villager's level)
                for y in 64..=68 {
                    b.push(([x, y, zz.floor() as i32], stone.to_string()));
                }
            }
            if walled {
                let wz = (z as f64 + 0.5 + h / 2.0).floor() as i32;
                for y in 64..=65 {
                    b.push(([x, y, wz], stone.to_string()));
                }
            }
            if riding {
                mobs.push(("minecraft:minecart".to_string(), [x as f64 + 0.5, 64.0, z as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string())));
            } else {
                mobs.push(("minecraft:villager".to_string(), [x as f64 + 0.5, 69.0, z as f64 + 0.5], Some("{PersistenceRequired:1b}".to_string())));
            }
            let by = if riding { 64.0 } else { 69.0 };
            mobs.push(("minecraft:oak_boat".to_string(), [x as f64 + 0.5, by, zz], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".to_string())));
        }
        v.push(Scene {
            name: "flee-distance".into(),
            chunks: [0, 0, 6, 4],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [111, 81, 79]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's queue-back sweep, its first step: where a pressed 3-cart
    // queue rests) a closed gate at (x,64,3) at the foot of N ascending rails rising south
    // ((x,64+s,4+s)), the flat at y 64+N to z 14; a ridden front, a ridden middle and an empty back
    // let go at rest at z 4.5, 5.5, 6.5; N = 2, 3, 5 (lanes x 1, 3, 5)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 16, 0, 16, 63, stone);
        let mut mobs = Vec::new();
        for (k, n) in [2, 3, 5].into_iter().enumerate() {
            let x = 1 + 2 * k as i32;
            slope_lane(&mut b, x, n, stone);
            for (j, z) in [4.5f64, 5.5, 6.5].into_iter().enumerate() {
                let cell = z.floor() as i32;
                let y = 64 + (cell - 4).clamp(0, n);
                let rider = if j < 2 { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
                mobs.push(("minecraft:minecart".to_string(), [x as f64 + 0.5, y as f64, z], Some(format!("{{Motion:[0.0d,0.0d,0.0d]{rider}}}"))));
            }
        }
        v.push(Scene {
            name: "queue-settle".into(),
            chunks: [0, 0, 0, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [15, 72, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, farmopt's H retry on a frozen cart: cc767b78's bench freezes the
    // released front cart with its box 0.06 into H's, H shuts on it, a retry opens and shuts it
    // again, then the thaw) H's geometry in lanes x 17..31 (chunk 1,0): a fence gate at (x,64,7) at
    // the foot of two ascending rails rising south ((x,64,8), (x,65,9)) and the flat at y 66, the
    // gate's cell over a rail one level down ((x,63,4..7), stone at z 3: the snap turn's drop); in
    // each lane a cart set with its front d into the gate's box (center z 8.115 - d), moving -z at
    // stored 0.2, the gate open; the chunk frozen from the summon (scene_actions: "gate-frozen")
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        let mut mobs = Vec::new();
        for (k, (d, _retry, ridden)) in GATE_FROZEN_LANES.iter().enumerate() {
            // (16..30: no lane on the frozen chunk's far edge, x 31, beside the unloaded chunk 2)
            let x = 16 + 2 * k as i32;
            b.push(([x, 64, 7], "minecraft:oak_fence_gate[facing=north,in_wall=false,open=true,powered=false]".to_string()));
            b.push(([x, 64, 8], "minecraft:rail[shape=ascending_south,waterlogged=false]".to_string()));
            b.push(([x, 64, 9], stone.to_string()));
            b.push(([x, 65, 9], "minecraft:rail[shape=ascending_south,waterlogged=false]".to_string()));
            for z in 10..=14 {
                for y in 64..=65 {
                    b.push(([x, y, z], stone.to_string()));
                }
                b.push(([x, 66, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            for z in 4..=7 {
                b.push(([x, 62, z], stone.to_string()));
                b.push(([x, 63, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            }
            let zc = 8.115 - d;
            let yc = 64.0625 + (zc - 8.0).max(0.0);
            let rider = if *ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
            mobs.push(("minecraft:minecart".to_string(), [x as f64 + 0.5, yc, zc], Some(format!("{{Motion:[0.0d,0.0d,-0.2d]{rider}}}"))));
        }
        v.push(Scene {
            name: "gate-frozen".into(),
            chunks: [0, 0, 1, 0],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [31, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, 58f770cc's plate question: does a stone pressure plate see a seated
    // villager passing under it?) the siding's plate: a stone pressure plate at y 66 on glass, over
    // a flat rail at y 64 (P = 64), at z 15 of each lane (lanes 3 apart in x, rails z 1..30 between
    // stones): seated villagers in carts moving +z at 0.1, 0.25 and 0.4 (from 3.5, 4.5 and 5.5
    // blocks before the plate) and one at rest under it; bare carts at the same speeds; standing
    // under the plate in a glass cell, an adult (its top at P + 1.95), a baby, and an adult on a
    // bottom slab (its top at P + 2.45: the positive control); a seated baby at rest under it (the
    // babies in plate-rider-baby, run in the game alone: redsim refuses a baby villager)
    for (name, kinds) in [("plate-rider", &["r0.1", "r0.25", "r0.4", "r0", "e0.1", "e0.25", "e0.4", "stand", "slab"][..]), ("plate-rider-baby", &["baby", "rbaby0", "stand", "slab"][..])] {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 32, 63, stone);
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        let baby_rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b,Age:-2147483000}]}";
        // (lane, its kind)
        for (k, kind) in kinds.iter().enumerate() {
            let x = 2 + 3 * k as i32;
            b.push(([x, 65, 15], "minecraft:glass".to_string()));
            b.push(([x, 66, 15], "minecraft:stone_pressure_plate[powered=false]".to_string()));
            let xf = x as f64 + 0.5;
            let cart_lane = kind.starts_with('r') || kind.starts_with('e');
            if cart_lane {
                b.push(([x, 64, 0], stone.to_string()));
                b.push(([x, 64, 31], stone.to_string()));
                for z in 1..=30 {
                    b.push(([x, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
                }
                let v: f64 = kind[1..].parse().unwrap_or(0.0);
                let z0 = if v == 0.0 { 15.5 } else { 15.5 - (v * 10.0 + 2.5).round() };
                let nbt = match *kind {
                    "rbaby0" => baby_rider.to_string(),
                    k if k.starts_with('r') => rider.replace("{Passengers", &format!("{{Motion:[0.0d,0.0d,{v:.2}d],Passengers")),
                    _ => format!("{{Motion:[0.0d,0.0d,{v:.2}d]}}"),
                };
                mobs.push(("minecraft:minecart".into(), [xf, 64.0625, z0], Some(nbt)));
            } else {
                // a glass cell around the plate's column (the plate's glass over the mob's head)
                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    for y in 64..=65 {
                        b.push(([x + dx, y, 15 + dz], "minecraft:glass".to_string()));
                    }
                }
                let (y, nbt) = match *kind {
                    "slab" => {
                        b.push(([x, 64, 15], "minecraft:stone_slab[type=bottom,waterlogged=false]".to_string()));
                        (64.5, "{PersistenceRequired:1b}")
                    }
                    "baby" => (64.0, "{PersistenceRequired:1b,Age:-2147483000}"),
                    _ => (64.0, "{PersistenceRequired:1b}"),
                };
                mobs.push(("minecraft:villager".into(), [xf, y, 15.5], Some(nbt.to_string())));
            }
        }
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 2, 1],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [47, 68, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, G1's VillageBoundRandomStroll question: the panic stroll (2/2) picks
    // with LandRandomPos where `isVillage` holds (an occupied village POI's section within 1) or no
    // section within 2 is closer to one, and with DefaultRandomPos.getPosTowards (a half-plane toward
    // the closer section's centre) otherwise; the shared bed is a village POI only while its ticket
    // is taken) a bed at (8,64,2..3) in section (0,4,0), and riders in carts held on one rail cell
    // between stones at section distances 0 (x 4, 12), 1 (x 20, 28) and 2 (x 36, 44), z 8, each
    // panicking at a zombie in a boat 7 blocks away (+z: panic, no flee); open stone floor around;
    // stroll-village with the bed's ticket free, stroll-village-taken with it taken at t1
    // (scene_actions)
    for name in ["stroll-village", "stroll-village-taken"] {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 32, 63, stone);
        // (a roof, so the zombies do not burn: flee-distance's)
        floor(&mut b, 0, 48, 0, 32, 72, stone);
        b.push(([8, 64, 2], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        b.push(([8, 64, 3], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        // (the bed walled in, so no rider's AcquirePoi reaches it: its ticket stays free unless the
        // scene takes it)
        for x in 7..=9 {
            for z in 1..=4 {
                if !(x == 8 && (z == 2 || z == 3)) {
                    for y in 64..=65 {
                        b.push(([x, y, z], stone.to_string()));
                    }
                }
            }
        }
        b.push(([8, 65, 2], stone.to_string()));
        b.push(([8, 65, 3], stone.to_string()));
        let mut mobs: Vec<(String, [f64; 3], Option<String>)> = Vec::new();
        let rider = "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}";
        for x in [4, 12, 20, 28, 36, 44] {
            b.push(([x, 64, 7], stone.to_string()));
            b.push(([x, 64, 8], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
            b.push(([x, 64, 9], stone.to_string()));
            mobs.push(("minecraft:minecart".into(), [x as f64 + 0.5, 64.0625, 8.5], Some(rider.to_string())));
            mobs.push(("minecraft:oak_boat".into(), [x as f64 + 0.5, 64.0, 15.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into())));
        }
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 2, 1],
            blocks: b,
            mobs,
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [47, 70, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, ironspec's queue-back sweep: an arrival hits a queue's back, not its
    // front) a pressed queue of a ridden front, a ridden middle and an empty back on 2, 3 and 5
    // slope rails before a gate, arrivals at stored 0..2.0 in steps of 0.02, ridden or empty
    for n in [2, 3, 5] {
        for (tag, ridden) in [("r", true), ("e", false)] {
            v.push(queue_back_scene(&format!("queue-back-{n}-{tag}"), n, ridden));
        }
    }
    // (tickexact 2026-11-11, ironspec's pinned-front sweep on R1's own column: ironfarm's redsim
    // runs merge from 0.44 stored with the front at rest pinned on G0, against slope-merge's
    // 0.74..0.96 with the front still rolling in) eight scenes of 101 lanes, arrivals at stored
    // 0..2.0 in steps of 0.02: case g0 (the front at rest at G0's rest (0.5,3.1775,30.885), the
    // arrival from mid-slope (0,5,28)) and case p (P's cart at rest on P against G_P
    // (0.5,2.1775,31.885), the arrival from (0,4,29) through G0 open), each with the waiting cart and
    // the arrival ridden or empty (rr, re, er, ee)
    for (case, file, wait, arrive) in [("g0", R1_CONTACT_G0, [0.5, 3.1775, 30.885], [0.5, 5.5625, 28.5]), ("p", R1_CONTACT_P, [0.5, 2.1775, 31.885], [0.5, 4.5625, 29.5])] {
        for (tag, wr, ar) in [("rr", true, true), ("re", true, false), ("er", false, true), ("ee", false, false)] {
            v.push(r1_sweep_scene(&format!("r1-sweep-{case}-{tag}"), file, wait, arrive, wr, ar, false));
        }
    }
    // (tickexact 2026-11-11, ironspec's forced-phase mode: the bridge for the sensors that time a
    // reloaded sleeper's wake) a villager asleep in its bed, a zombie walled in stone 4 blocks east;
    // the whole sandbox unloaded at t200, loaded at t240 and the zombie's west wall taken away at
    // once, so the reloaded villager's first scans that see it wake it; with MOBAI_SENSOR_FORCE=A,B
    // its NearestLivingEntitySensor and VillagerHostilesSensor start delays set to A and B right
    // after the load (scene_actions: "reload-wake")
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 16, 0, 16, 63, stone);
        b.push(([8, 64, 8], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
        b.push(([8, 64, 9], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        for y in 64..=65 {
            for c in [[11, y, 9], [13, y, 9], [12, y, 8], [12, y, 10]] {
                b.push((c, stone.to_string()));
            }
        }
        b.push(([12, 66, 9], stone.to_string()));
        v.push(Scene {
            name: "reload-wake".into(),
            chunks: [0, 0, 0, 0],
            blocks: b,
            mobs: vec![
                ("minecraft:villager".into(), [7.5, 64.0, 9.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombie".into(), [12.5, 64.0, 9.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 12600,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [15, 67, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, d1u-drop-side run 25's refusal: a village window's pane lost its east
    // neighbour and turned to glass_pane[west=true], a state the natural table lacked) panes whose
    // connections change, every starting state one the table had: A between two stones, the east
    // one removed at t10 (west only); B alone, a stone set north of it at t20 (north only); C one of
    // two panes between stones, the other removed at t30 (west only) (scene_actions: "pane-connect")
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 16, 0, 16, 63, stone);
        b.push(([4, 64, 4], stone.to_string()));
        b.push(([5, 64, 4], "minecraft:glass_pane[east=true,north=false,south=false,waterlogged=false,west=true]".to_string()));
        b.push(([6, 64, 4], stone.to_string()));
        b.push(([5, 64, 8], "minecraft:glass_pane[east=false,north=false,south=false,waterlogged=false,west=false]".to_string()));
        b.push(([4, 64, 12], stone.to_string()));
        b.push(([5, 64, 12], "minecraft:glass_pane[east=true,north=false,south=false,waterlogged=false,west=true]".to_string()));
        b.push(([6, 64, 12], "minecraft:glass_pane[east=true,north=false,south=false,waterlogged=false,west=true]".to_string()));
        b.push(([7, 64, 12], stone.to_string()));
        v.push(Scene {
            name: "pane-connect".into(),
            chunks: [0, 0, 0, 0],
            blocks: b,
            mobs: Vec::new(),
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([0, 62, 0], [15, 66, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-11, farmopt: does a block a bench sets inside a level-33 chunk still drive
    // its redstone?) three chunks x -1..1, the east two's forced tickets dropped t40..t240 (chunk
    // (0, 0) at level 32, (1, 0) at level 33); at t60 a redstone block set by the drive: A in the
    // level-33 chunk feeding dust into a fence gate (neighbour updates only), B in the level-33
    // chunk feeding a repeater into a fence gate (a scheduled tick), C the same as B in the
    // level-32 chunk (scene_actions: "level33-set")
    {
        let mut b = Vec::new();
        floor(&mut b, -16, 32, 0, 16, 63, stone);
        let gate = "minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]";
        b.push(([19, 64, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]".to_string()));
        b.push(([20, 64, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]".to_string()));
        b.push(([21, 64, 2], gate.to_string()));
        for (x0, z) in [(19, 6), (3, 10)] {
            b.push(([x0, 64, z], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]".to_string()));
            b.push(([x0 + 1, 64, z], gate.to_string()));
        }
        v.push(Scene {
            name: "level33-set".into(),
            chunks: [-1, 0, 1, 0],
            blocks: b,
            mobs: Vec::new(),
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([-16, 62, 0], [31, 68, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-10, the reviewer's question on a partial unload) level 32: two chunks,
    // the west one forced throughout, the east one's forced ticket dropped from t40 to t240
    // (simulation level 32: block ticking, not entity ticking). In each chunk a randomly ticking
    // farmland (watered: no change; its section's random ticks step randValue); in the east one a
    // repeater loop (started at t5), a hopper between two chests (the top one filled at t1), a
    // minecart with a villager rolled onto a detector rail against a stone, and an empty minecart
    // still rolling down a long track at t40 (scene_actions: "level32")
    {
        let mut b = Vec::new();
        floor(&mut b, -16, 16, 0, 16, 63, stone);
        for x0 in [-8, 4] {
            b.push(([x0, 63, 12], "minecraft:farmland[moisture=7]".to_string()));
            b.push(([x0 + 1, 63, 12], "minecraft:water[level=0]".to_string()));
        }
        // the repeater loop: four repeaters (delay 1) and four dust corners around (8, 5)
        // (tickexact 2026-11-11, ironspec's raw-placement audit: each corner's dust written in the
        // shape its neighbours give it, the two repeaters it joins, not a cross)
        let corner = |e: bool, n: bool, s: bool, w: bool| {
            let f = |c: bool| if c { "side" } else { "none" };
            format!("minecraft:redstone_wire[east={},north={},power=0,south={},west={}]", f(e), f(n), f(s), f(w))
        };
        b.push(([8, 64, 4], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]".to_string()));
        b.push(([9, 64, 4], corner(false, false, true, true)));
        b.push(([9, 64, 5], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]".to_string()));
        b.push(([9, 64, 6], corner(false, true, false, true)));
        b.push(([8, 64, 6], "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]".to_string()));
        b.push(([7, 64, 6], corner(true, true, false, false)));
        b.push(([7, 64, 5], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]".to_string()));
        b.push(([7, 64, 4], corner(true, false, true, false)));
        // the hopper between two chests
        b.push(([4, 64, 8], "minecraft:chest[facing=north,type=single,waterlogged=false]".to_string()));
        b.push(([4, 65, 8], "minecraft:hopper[enabled=true,facing=down]".to_string()));
        b.push(([4, 66, 8], "minecraft:chest[facing=north,type=single,waterlogged=false]".to_string()));
        // the detector rail at the end of a short track (a cart rolls onto it and stops against
        // the stone) and a long east-west track a second cart is still rolling on at t40
        for z in 1..=3 {
            b.push(([12, 64, z], "minecraft:rail[shape=north_south,waterlogged=false]".to_string()));
        }
        b.push(([12, 64, 4], "minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]".to_string()));
        b.push(([12, 64, 5], stone.to_string()));
        for x in 1..=14 {
            b.push(([x, 64, 13], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
        }
        b.push(([0, 64, 13], stone.to_string()));
        b.push(([15, 64, 13], stone.to_string()));
        v.push(Scene {
            name: "level32".into(),
            chunks: [-1, 0, 0, 0],
            blocks: b,
            mobs: vec![
                ("minecraft:minecart".into(), [12.5, 64.0, 1.5], Some("{Motion:[0.0d,0.0d,0.3d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".into())),
                ("minecraft:minecart".into(), [1.5, 64.0, 13.5], Some("{Motion:[0.15d,0.0d,0.0d]}".into())),
            ],
            day_time: 6000,
            difficulty: Difficulty::Normal,
            mob_griefing: true,
            watch: ([-16, 62, 0], [15, 68, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // bats in a closed stone hall at night: a summoned bat flies (BatFlags 0: its target cells,
    // its motion, hanging again under the ceiling 1 in 100); one hangs from the ceiling (BatFlags
    // 1) and wakes when its ceiling block is mined... here a creative player hangs 3 below it
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        floor(&mut b, 8, 40, 8, 40, 72, stone);
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 8, stone);
        }
        v.push(Scene {
            name: "bat-hall".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:bat".into(), [20.5, 66.0, 20.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:bat".into(), [30.5, 71.1, 30.5], Some("{PersistenceRequired:1b,BatFlags:1b}".into())),
                ("minecraft:bat".into(), [14.5, 71.1, 30.5], Some("{PersistenceRequired:1b,BatFlags:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 73, 47]),
            player: Some([30.5, 68.0, 30.5]),
            player_creative: true,
            random_tick_speed: 3,
        });
    }
    // sulfur cubes (size 2) hopping in a stone pen with a pool: CubeMobRandomDirectionGoal's
    // directions, the jumping move control, landing draws, floating in the water
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 3, stone);
        }
        for x in 28..34 {
            for z in 28..34 {
                b.push(([x, 63, z], "minecraft:water[level=0]".to_string()));
            }
        }
        v.push(Scene {
            name: "sulfur-cubes".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:sulfur_cube".into(), [16.5, 64.0, 16.5], Some("{PersistenceRequired:1b,Size:1}".into())),
                ("minecraft:sulfur_cube".into(), [24.5, 64.0, 20.5], Some("{PersistenceRequired:1b,Size:1}".into())),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 72.0, 24.5]),
            player_creative: true,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10, d1u-deep's refusal) an adult sulfur cube beside a dropped rail (no
    // pickup delay): not in `#sulfur_cube_swallowable`, so SulfurCubeSearchForItemsGoal ignores it
    // (the scene runs, no model error); sulfur-cube-dirt: a dirt item instead (swallowable: the
    // goal would chase it, a named model error)
    for (name, item) in [("sulfur-cube-rail", "minecraft:rail"), ("sulfur-cube-dirt", "minecraft:dirt")] {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 3, stone);
        }
        v.push(Scene {
            name: name.into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:sulfur_cube".into(), [16.5, 64.0, 16.5], Some("{PersistenceRequired:1b,Size:1}".into())),
                ("minecraft:item".into(), [20.5, 64.0, 18.5], Some(format!("{{Item:{{id:\"{item}\",count:1}},PickupDelay:0s,Age:-32768s}}"))),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 72.0, 24.5]),
            player_creative: true,
            random_tick_speed: 3,
        });
    }
    // a zombie at night outside a closed stone house whose three villagers claim their beds:
    // MoveThroughVillageGoal (the claimed POIs, LandRandomPos scored by the nearest unvisited
    // one, the path toward it)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((28, 18), (28, 30)), ((38, 18), (38, 30)), ((28, 18), (38, 18)), ((28, 30), (38, 30))] {
            wall(&mut b, a, c, 63, 3, stone);
        }
        floor(&mut b, 28, 39, 18, 31, 67, stone);
        for (k, z) in [21, 24, 27].into_iter().enumerate() {
            let x = 31 + 2 * k as i32;
            b.push(([x, 64, z], "minecraft:red_bed[facing=south,part=foot,occupied=false]".to_string()));
            b.push(([x, 64, z + 1], "minecraft:red_bed[facing=south,part=head,occupied=false]".to_string()));
        }
        v.push(Scene {
            name: "zombie-village".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:villager".into(), [30.5, 64.0, 20.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [33.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [36.5, 64.0, 28.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombie".into(), [10.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 14000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // farm animals in a grass pen with tufts of short grass, a creative player above keeping them
    // from idling: strolls weighted by the grass, the sheep eating grass, the chicken's slow fall
    // and its egg
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for x in 9..39 {
            for z in 9..39 {
                b.push(([x, 63, z], "minecraft:grass_block[snowy=false]".to_string()));
            }
        }
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 2, stone);
        }
        let p = |s: &str| Some(s.to_string());
        v.push(Scene {
            name: "animals".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:pig".into(), [16.5, 64.0, 16.5], p("{PersistenceRequired:1b}")),
                ("minecraft:cow".into(), [30.5, 64.0, 16.5], p("{PersistenceRequired:1b}")),
                ("minecraft:sheep".into(), [16.5, 64.0, 30.5], p("{PersistenceRequired:1b}")),
                ("minecraft:chicken".into(), [30.5, 66.0, 30.5], p("{PersistenceRequired:1b}")),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // animals that panic (PanicGoal): a cow and a sheep on magma blocks (hot floor damage), a pig
    // set burning by a fire and a chicken beside lava, a water pool to run to (lookForWater), on a
    // stone floor inside a low wall
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 2, stone);
        }
        for x in 14..=16 {
            for z in 14..=16 {
                b.push(([x, 63, z], "minecraft:magma_block".to_string()));
            }
        }
        for x in 26..=28 {
            for z in 26..=27 {
                b.push(([x, 63, z], "minecraft:water[level=0]".to_string()));
            }
        }
        b.push(([22, 64, 24], "minecraft:fire[age=0,east=false,north=false,south=false,up=false,west=false]".to_string()));
        b.push(([20, 63, 30], "minecraft:lava[level=0]".to_string()));
        let p = |s: &str| Some(s.to_string());
        v.push(Scene {
            name: "animal-panic".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:cow".into(), [15.5, 64.0, 15.5], p("{PersistenceRequired:1b}")),
                ("minecraft:sheep".into(), [14.5, 64.0, 14.5], p("{PersistenceRequired:1b}")),
                ("minecraft:pig".into(), [22.5, 64.0, 24.5], p("{PersistenceRequired:1b}")),
                ("minecraft:chicken".into(), [21.5, 64.0, 30.5], p("{PersistenceRequired:1b}")),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // babies growing up against blocks (refreshDimensions' fudgePositionAfterSizeChange): a baby
    // cow, sheep, pig and chicken each off-centre in a 1x1 pen (the adult's box overlaps a wall:
    // pushed to the free position closest to its old centre), a baby cow under a ceiling one block
    // up (no room for the adult's height: the second search, ignoring y); Age -60
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        let pens = [12, 16, 20, 24, 28];
        for &x in &pens {
            for (dx, dz) in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
                for y in 64..=66 {
                    b.push(([x + dx, y, 12 + dz], "minecraft:stone".to_string()));
                }
            }
        }
        b.push(([28, 65, 12], "minecraft:stone".to_string()));
        let p = |s: &str| Some(s.to_string());
        v.push(Scene {
            name: "baby-grow-wall".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:cow".into(), [12.3, 64.0, 12.5], p("{PersistenceRequired:1b,Age:-60}")),
                ("minecraft:sheep".into(), [16.72, 64.0, 12.4], p("{PersistenceRequired:1b,Age:-60}")),
                ("minecraft:pig".into(), [20.5, 64.0, 12.26], p("{PersistenceRequired:1b,Age:-60}")),
                ("minecraft:chicken".into(), [24.2, 64.0, 12.21], p("{PersistenceRequired:1b,Age:-60}")),
                ("minecraft:cow".into(), [28.5, 64.0, 12.5], p("{PersistenceRequired:1b,Age:-60}")),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // a chicken jockey's flag (Chicken.isChickenJockey: no eggs): two adult chickens with their
    // egg timers at 30, the second flagged as a jockey (as a baby zombie's roll leaves it)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 2, stone);
        }
        let p = |s: &str| Some(s.to_string());
        v.push(Scene {
            name: "chicken-jockey-flag".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:chicken".into(), [16.5, 64.0, 16.5], p("{PersistenceRequired:1b,EggLayTime:30}")),
                ("minecraft:chicken".into(), [30.5, 64.0, 30.5], p("{PersistenceRequired:1b,EggLayTime:30,IsChickenJockey:1b}")),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // horses in a grass pen (AbstractHorse's goals in insertion order, the grass eating that holds
    // its AI 50 ticks, the random stands (20 ticks), the tail roll, 1 health in 900): a horse, a
    // donkey, a baby horse (follows its parent), a zombie horse and a skeleton horse, summoned
    // with data (no finalizeSpawn: the base attributes)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for x in 9..39 {
            for z in 9..39 {
                b.push(([x, 63, z], "minecraft:grass_block[snowy=false]".to_string()));
            }
        }
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 2, stone);
        }
        let p = |s: &str| Some(s.to_string());
        v.push(Scene {
            name: "horses".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:horse".into(), [16.5, 64.0, 16.5], p("{PersistenceRequired:1b}")),
                ("minecraft:donkey".into(), [30.5, 64.0, 16.5], p("{PersistenceRequired:1b}")),
                ("minecraft:horse".into(), [18.5, 64.0, 18.5], p("{PersistenceRequired:1b,Age:-24000}")),
                ("minecraft:zombie_horse".into(), [16.5, 64.0, 30.5], p("{PersistenceRequired:1b}")),
                ("minecraft:skeleton_horse".into(), [30.5, 64.0, 30.5], p("{PersistenceRequired:1b}")),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // the farm animals' babies by their parents in a grass pen: FollowParentGoal, a baby sheep's
    // EatBlockGoal (25) and its age-up, two babies growing up during the run
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for x in 9..39 {
            for z in 9..39 {
                b.push(([x, 63, z], "minecraft:grass_block[snowy=false]".to_string()));
            }
        }
        for (a, c) in [((8, 8), (8, 39)), ((39, 8), (39, 39)), ((8, 8), (39, 8)), ((8, 39), (39, 39))] {
            wall(&mut b, a, c, 63, 2, stone);
        }
        let p = |s: &str| Some(s.to_string());
        v.push(Scene {
            name: "baby-animals".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:pig".into(), [16.5, 64.0, 16.5], p("{PersistenceRequired:1b}")),
                ("minecraft:pig".into(), [22.5, 64.0, 16.5], p("{PersistenceRequired:1b,Age:-400}")),
                ("minecraft:cow".into(), [30.5, 64.0, 16.5], p("{PersistenceRequired:1b}")),
                ("minecraft:cow".into(), [30.5, 64.0, 22.5], p("{PersistenceRequired:1b,Age:-24000}")),
                ("minecraft:sheep".into(), [16.5, 64.0, 30.5], p("{PersistenceRequired:1b}")),
                ("minecraft:sheep".into(), [16.5, 64.0, 24.5], p("{PersistenceRequired:1b,Age:-24000}")),
                ("minecraft:chicken".into(), [30.5, 64.0, 30.5], p("{PersistenceRequired:1b}")),
                ("minecraft:chicken".into(), [24.5, 64.0, 30.5], p("{PersistenceRequired:1b,Age:-1200}")),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // two squids and a glow squid in a deep stone pool: SquidRandomMovementGoal, the tentacle
    // cycle's impulse and turns, the glow squid's particle draws, a squid stranded on the rim
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 55, stone);
        for (a, c) in [((11, 11), (11, 36)), ((36, 11), (36, 36)), ((11, 11), (36, 11)), ((11, 36), (36, 36))] {
            wall(&mut b, a, c, 56, 8, stone);
        }
        for x in 12..36 {
            for z in 12..36 {
                for y in 56..63 {
                    b.push(([x, y, z], "minecraft:water[level=0]".to_string()));
                }
            }
        }
        let p = |s: &str| Some(s.to_string());
        v.push(Scene {
            name: "squids".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:squid".into(), [18.5, 59.0, 18.5], p("{PersistenceRequired:1b}")),
                ("minecraft:squid".into(), [28.5, 58.0, 26.5], p("{PersistenceRequired:1b}")),
                ("minecraft:glow_squid".into(), [22.5, 57.0, 30.5], p("{PersistenceRequired:1b}")),
                ("minecraft:squid".into(), [8.5, 56.0, 8.5], p("{PersistenceRequired:1b}")),
            ],
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 50, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 3,
        });
    }
    // a witch in a stone pen by daylight, a creative player above: Witch.aiStep's potion rolls,
    // its strolls and looks, its targets' draws (the healable raider's nextBoolean)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        for (a, c) in [((10, 10), (10, 37)), ((37, 10), (37, 37)), ((10, 10), (37, 10)), ((10, 37), (37, 37))] {
            wall(&mut b, a, c, 63, 3, stone);
        }
        v.push(Scene {
            name: "witch".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:witch".into(), [20.5, 64.0, 22.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 6000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 24.5]),
            player_creative: true,
            random_tick_speed: 3,
        });
    }
    // natural mobs' loot: each mob falls 30 blocks onto stone and dies on landing (no panic, no
    // potion): a spider's string, a skeleton's arrows and bones, a creeper's gunpowder, a zombie
    // villager's flesh, a cow's leather and beef, a pig's porkchops, a black sheep's mutton and
    // black wool, a witch's weighted rolls and redstone
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 60, stone);
        let p = |s: &str| Some(s.to_string());
        let fall = |id: &str, x: f64, z: f64, nbt: &str| (id.to_string(), [x, 91.0, z], p(nbt));
        v.push(Scene {
            name: "loot".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                fall("minecraft:spider", 12.5, 12.5, "{PersistenceRequired:1b}"),
                fall("minecraft:skeleton", 18.5, 12.5, "{PersistenceRequired:1b}"),
                fall("minecraft:creeper", 24.5, 12.5, "{PersistenceRequired:1b}"),
                fall("minecraft:zombie_villager", 30.5, 12.5, "{PersistenceRequired:1b}"),
                fall("minecraft:cow", 12.5, 30.5, "{PersistenceRequired:1b}"),
                fall("minecraft:pig", 18.5, 30.5, "{PersistenceRequired:1b}"),
                fall("minecraft:sheep", 24.5, 30.5, "{PersistenceRequired:1b,Color:15b}"),
                fall("minecraft:witch", 30.5, 30.5, "{PersistenceRequired:1b}"),
            ],
            day_time: 18000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 58, 0], [47, 95, 47]),
            player: Some([24.5, 100.0, 4.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // (mob-AI 2026-11-10) loot pickup (Mob.aiStep's looting) among item entities, in the noon
    // sun: zombie A (CanPickUpLoot) takes the 3 rotten flesh at its feet whole and leaves the iron
    // ingot beside it (a plain item never replaces a plain one in hand); B refuses the glow ink
    // sac and takes the 2 bones once their 10-tick pickup delay is over; C (no CanPickUpLoot)
    // leaves its flesh. They burn and die: the loot table first, then the item in hand
    // (dropCustomDeathLoot's guaranteed drop)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        let p = |s: &str| Some(s.to_string());
        let item = |x: f64, z: f64, id: &str, n: u32, delay: u32| ("minecraft:item".to_string(), [x, 64.0, z], p(&format!("{{Item:{{id:\"minecraft:{id}\",count:{n}}},PickupDelay:{delay}s}}")));
        v.push(Scene {
            name: "loot-pickup".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                item(13.0, 12.5, "rotten_flesh", 3, 0),
                item(12.5, 13.3, "iron_ingot", 1, 0),
                ("minecraft:zombie".into(), [12.5, 64.0, 12.5], p("{CanPickUpLoot:1b}")),
                item(24.9, 12.5, "glow_ink_sac", 1, 0),
                item(24.5, 12.9, "bone", 2, 10),
                ("minecraft:zombie".into(), [24.5, 64.0, 12.5], p("{CanPickUpLoot:1b}")),
                item(36.9, 12.5, "rotten_flesh", 1, 0),
                ("minecraft:zombie".into(), [36.5, 64.0, 12.5], None),
            ],
            day_time: 6000,
            difficulty: Difficulty::Hard,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: Some([24.5, 74.0, 40.5]),
            player_creative: true,
            random_tick_speed: 0,
        });
    }
    // (tickexact 2026-11-10, the parent's single-lane carousel: 21 carts with villagers back to
    // back, released 3 at a time, rejoining the tail; the release mechanism is ironfarm's, this is
    // the queue's own physics) a lane along x at z 24 on the floor: a stone kick at x 1, boosters
    // x 2..9 (powered rails whose redstone blocks come at 300), plain rails x 10..24, then the queue
    // on 21 unpowered powered rails x 25..45 (the brakes) against a glass stop at x 46. The 21 carts
    // sit one a block (x 25.5..45.5) and settle (pushOtherMinecart between neighbours, the brakes'
    // halving and stop under 0.03); at 300 the 3 carts at x 2.5..4.5 launch off the kick, coast and
    // run into the queue's tail at speed
    {
        let mut qb = Vec::new();
        floor(&mut qb, 0, 48, 0, 48, 63, stone);
        let powered = |s: &str| format!("minecraft:powered_rail[powered=false,shape={s},waterlogged=false]");
        let rail = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
        qb.push(([1, 64, 24], stone.to_string()));
        qb.push(([46, 64, 24], "minecraft:glass".to_string()));
        for x in 2..=9 {
            qb.push(([x, 64, 24], powered("east_west")));
        }
        for x in 10..=24 {
            qb.push(([x, 64, 24], rail("east_west")));
        }
        for x in 25..=45 {
            qb.push(([x, 64, 24], powered("east_west")));
        }
        let cart = |x: f64| ("minecraft:minecart".to_string(), [x, 64.0, 24.5], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string()));
        let mut carts: Vec<_> = (25..=45).map(|x| cart(x as f64 + 0.5)).collect();
        carts.extend((2..=4).map(|x| cart(x as f64 + 0.5)));
        v.push(Scene {
            name: "cart-queue".into(),
            chunks: [0, 0, 2, 2],
            blocks: qb,
            mobs: carts,
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10, the single-lane carousel with ironfarm's answers: a slope release, the
    // queue spaced 1.25 so neighbours never touch, the returning carts at speed into the tail, a
    // bend) a causeway along x at z 24 (stone y 64..66, rails at y 67): a stone kick at x 1,
    // boosters x 2..9 (redstone blocks replace the causeway under them at 600), plain rails x 10..17,
    // the queue's brakes x 18..39 (unpowered powered rails), a plain rail at x 40 (no power along
    // the line), then the release: 3 braked powered-rail slopes descending east (x 41..43, y 66 to
    // 64), their supports turned to redstone blocks at 300; a run-out east and a bend north (x 45
    // turns to z 23..18) to a glass stop. 21 carts with villagers: 3 on the slopes, 18 on the flat
    // brakes 1.25 apart (the tail cart on the first brake); 3 more at x 2.5..5.0 launched at 600
    {
        let mut qb = Vec::new();
        floor(&mut qb, 0, 48, 0, 48, 63, stone);
        let powered = |s: &str| format!("minecraft:powered_rail[powered=false,shape={s},waterlogged=false]");
        let rail = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
        // the causeway (the slopes' supports below)
        for x in 1..=40 {
            for y in 64..=66 {
                qb.push(([x, y, 24], stone.to_string()));
            }
        }
        qb.push(([1, 67, 24], stone.to_string()));
        for x in 2..=9 {
            qb.push(([x, 67, 24], powered("east_west")));
        }
        for x in 10..=17 {
            qb.push(([x, 67, 24], rail("east_west")));
        }
        for x in 18..=39 {
            qb.push(([x, 67, 24], powered("east_west")));
        }
        qb.push(([40, 67, 24], rail("east_west")));
        for (x, y) in [(41, 66), (42, 65), (43, 64)] {
            qb.push(([x, y - 1, 24], stone.to_string()));
            qb.push(([x, y, 24], powered("ascending_west")));
        }
        qb.push(([44, 64, 24], rail("east_west")));
        qb.push(([45, 64, 24], rail("north_west")));
        for z in 18..=23 {
            qb.push(([45, 64, z], rail("north_south")));
        }
        qb.push(([45, 64, 17], "minecraft:glass".to_string()));
        let cart = |x: f64, y: f64, z: f64| ("minecraft:minecart".to_string(), [x, y, z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string()));
        let mut carts: Vec<_> = [(41.5, 66.5), (42.5, 65.5), (43.5, 64.5)].into_iter().map(|(x, y)| cart(x, y, 24.5)).collect();
        carts.extend((0..18).map(|k| cart(39.375 - 1.25 * k as f64, 67.0, 24.5)));
        carts.extend((0..3).map(|k| cart(2.5 + 1.25 * k as f64, 67.0, 24.5)));
        v.push(Scene {
            name: "cart-queue-slope".into(),
            chunks: [0, 0, 2, 2],
            blocks: qb,
            mobs: carts,
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10, the single-lane carousel's bends, the reviewer's ask) a U of track on
    // the floor: lane A along x at z 30 (a stone kick at x 4, boosters x 5..8 whose redstone blocks
    // come at 300, plain rails x 9..15, brakes x 16..30), a curve at (31, 30) turning north, lane B
    // up x 31 (brakes z 21..29), a curve at (31, 20) turning west, lane C back along z 20 (brakes
    // x 11..30) to a glass stop at x 10. 12 carts with villagers rest 1.25 apart on lane A's brakes,
    // the front one by the first curve; at 300 three carts launch off the kick into the tail and
    // the queue is pushed around the bend (the curves are plain rails: no brake on them)
    {
        let mut ub = Vec::new();
        floor(&mut ub, 0, 48, 0, 48, 63, stone);
        let powered = |s: &str| format!("minecraft:powered_rail[powered=false,shape={s},waterlogged=false]");
        let rail = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
        ub.push(([4, 64, 30], stone.to_string()));
        for x in 5..=8 {
            ub.push(([x, 64, 30], powered("east_west")));
        }
        for x in 9..=15 {
            ub.push(([x, 64, 30], rail("east_west")));
        }
        for x in 16..=30 {
            ub.push(([x, 64, 30], powered("east_west")));
        }
        ub.push(([31, 64, 30], rail("north_west")));
        for z in 21..=29 {
            ub.push(([31, 64, z], powered("north_south")));
        }
        ub.push(([31, 64, 20], rail("south_west")));
        for x in 11..=30 {
            ub.push(([x, 64, 20], powered("east_west")));
        }
        ub.push(([10, 64, 20], "minecraft:glass".to_string()));
        let cart = |x: f64, z: f64| ("minecraft:minecart".to_string(), [x, 64.0, z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}".to_string()));
        let mut carts: Vec<_> = (0..12).map(|k| cart(30.375 - 1.25 * k as f64, 30.5)).collect();
        carts.extend((0..3).map(|k| cart(5.5 + 1.25 * k as f64, 30.5)));
        v.push(Scene {
            name: "cart-queue-bend".into(),
            chunks: [0, 0, 2, 2],
            blocks: ub,
            mobs: carts,
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10) ironfarm's single-lane carousel loop as it is (its WIP design, frozen:
    // the sawtooth register of 18 slots on two rows joined by a boosted U-turn, the release run, the
    // 3-slot stand, the departure and return runs; 21 carts with villagers), driven by its slot
    // schedule ([`carousel_loop_actions`]): the release train, the register's one-slot moves on
    // 9-gt pulses, the U-turn's chaining at 20-gt pulses
    {
        let d = carousel_loop_design();
        let sh = CAROUSEL_SHIFT;
        let blocks: Vec<([i32; 3], String)> = d.blocks.iter().map(|(p, st)| ([p[0] + sh[0], p[1] + sh[1], p[2] + sh[2]], st.clone())).collect();
        let mobs: Vec<(String, [f64; 3], Option<String>)> = d.entities.iter().map(|e| (e.kind.clone(), [e.pos[0] + sh[0] as f64, e.pos[1] + sh[1] as f64, e.pos[2] + sh[2] as f64], e.nbt.clone())).collect();
        v.push(Scene {
            name: "carousel-loop".into(),
            chunks: [0, 0, 4, 1],
            blocks,
            mobs,
            day_time: 1000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [79, 70, 31]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10, mob-AI's forest t2504: a leaf litter whose support an enderman took)
    // natural plants on their supports ([`plant_supports`]), each support (or a two-block plant's
    // upper half) set to air in turn: the plant's canSurvive, its updateShape to air, the
    // destroyBlock drop (its loot table with its random sequence, popResource's level draws, the
    // item entity)
    {
        let (pb, _) = plant_supports();
        v.push(Scene {
            name: "plant-supports".into(),
            chunks: [0, 0, 1, 0],
            blocks: pb,
            mobs: Vec::new(),
            day_time: 6000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [31, 70, 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    // (tickexact 2026-11-10, ironfarm's stand catch) its loop designs as they are, each driven by
    // its bench's drive log ([`DRIVE_SCENES`], [`drive_actions`]): cart physics, the riders (and
    // carousel-catch2's zombies in boats) and the detector rail under scripted controls (no other
    // redstone in the designs). carousel-catch2 runs on Hard at day time 200, as the bench's world.
    for name in drive_scene_names() {
        let name = &name;
        let d = drive_design(name).expect("a drive scene's design");
        let blocks: Vec<([i32; 3], String)> = d.blocks.iter().map(|(p, st)| (*p, st.clone())).collect();
        let home = scene_home(name);
        let mobs: Vec<(String, [f64; 3], Option<String>)> = d
            .entities
            .iter()
            .map(|e| (e.kind.clone(), e.pos, match home {
                Some(h) => with_home(&e.kind, e.nbt.clone(), h),
                None => e.nbt.clone(),
            }))
            .collect();
        let max_x = blocks.iter().map(|(p, _)| p[0]).chain(mobs.iter().map(|m| m.1[0] as i32)).max().unwrap_or(0);
        let max_z = blocks.iter().map(|(p, _)| p[2]).chain(mobs.iter().map(|m| m.1[2] as i32)).max().unwrap_or(0);
        let min_x = blocks.iter().map(|(p, _)| p[0]).chain(mobs.iter().map(|m| m.1[0].floor() as i32)).min().unwrap_or(0);
        let min_z = blocks.iter().map(|(p, _)| p[2]).chain(mobs.iter().map(|m| m.1[2].floor() as i32)).min().unwrap_or(0);
        // (2 cells of margin to the sandbox's edge walls)
        let c = [(min_x - 2).div_euclid(16), (min_z - 2).div_euclid(16), (max_x + 2).div_euclid(16), (max_z + 2).div_euclid(16)];
        let first = *name == "carousel-catch";
        // (the wired loop starts at its bench's day time: `RunOpts::steady` of the drive log's
        // seed, less the settle before the summon)
        let day_time = if first {
            1000
        } else if name.starts_with("carousel-wired") || name.starts_with("siding") || name.starts_with("station") || name.starts_with("unload-") {
            // (the bench's world: day0 at the end of its settle, which the raw paste skips)
            drive_day0(name).unwrap_or(200) - redsim::entity::mob::farm::SETTLE_TICKS
        } else {
            200
        };
        // (the carousel catches watch y -6..6 as recorded; the wired loop its own height, every
        // redstone component of its controls)
        let (wy0, wy1) = if name.starts_with("carousel-catch") {
            (-6, 6)
        } else {
            (blocks.iter().map(|(p, _)| p[1]).min().unwrap_or(0) - 1, blocks.iter().map(|(p, _)| p[1]).max().unwrap_or(0) + 2)
        };
        v.push(Scene {
            name: name.to_string(),
            chunks: c,
            // (a built-in-order scene starts with none of its blocks: its drive log places them)
            blocks: if scene_placement(name) == Placement::Built { Vec::new() } else { blocks },
            mobs,
            day_time,
            difficulty: if first { Difficulty::Easy } else { Difficulty::Hard },
            mob_griefing: true,
            watch: ([c[0] * 16, wy0, c[1] * 16], [c[2] * 16 + 15, wy1, c[3] * 16 + 15]),
            player: None,
            player_creative: false,
            random_tick_speed: 3,
        });
    }
    v
}

/// The game's side of a scene.
pub struct GameScene {
    w: Worker,
    sb: u32,
    lo: [i32; 3],
    hi: [i32; 3],
    /// The block-ticking chunks in the bridge's order (a scene with a player; else empty).
    pub ticking: Vec<[i32; 2]>,
    /// The spawning chunks before the shuffle (a scene with a player).
    pub spawning: Vec<[i32; 2]>,
    /// (tickexact 2026-11-10) A scene's Nether sandbox ([`scene_nether_part`]): its id and the box
    /// both sides watch.
    nether: Option<(u32, [i32; 3], [i32; 3])>,
    /// (tickexact 2026-11-10) Entities a `load` brought back under new numbers: old -> current
    /// (the bridge's answer; a drive's `eN` follows it).
    renumbered: std::collections::HashMap<u32, u32>,
    /// (tickexact 2026-11-11) The game's last whole-part load sequence (`n:type ...`, the bridge's
    /// `load` reply), compared with redsim's `World::last_load`.
    last_load: String,
    /// (mob-AI 2026-11-11) The POI type holders' identity hashes in this JVM (the bridge's
    /// `identity_hashes`), for redsim's `AiWorld::poi_hashes`.
    pub poi_hashes: std::collections::HashMap<String, i32>,
}

/// The bridge's `identity_hashes`: each POI type's registry holder's identity hash in this JVM.
fn poi_identity_hashes(w: &mut Worker) -> Result<std::collections::HashMap<String, i32>> {
    let v = w.call(json!({"op": "identity_hashes"}))?;
    Ok(v["poi_types"].as_object().map(|m| m.iter().filter_map(|(k, h)| h.as_i64().map(|h| (k.clone(), h as i32))).collect()).unwrap_or_default())
}

/// A recording's POI holder hashes (its header's `poi_types`; none in a recording made before
/// they were kept).
fn header_poi_hashes(header: &serde_json::Value) -> Option<std::collections::HashMap<String, i32>> {
    header["poi_types"].as_object().map(|m| m.iter().filter_map(|(k, h)| h.as_i64().map(|h| (k.clone(), h as i32))).collect())
}

impl GameScene {
    pub fn start(version: &str, s: &Scene, seed: i64, trace_randoms: bool) -> Result<GameScene> {
        let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 2048, ..Default::default() })?;
        let sb = w.create(&SandboxOptions { chunks: s.chunks, seed })?;
        w.call(json!({
            "op": "world", "sandbox": sb,
            "advance_time": true, "day_time": s.day_time,
            "advance_weather": false, "spawn_mobs": scene_spawns(&s.name), "spawn_monsters": scene_spawns(&s.name),
            "difficulty": difficulty_name(s.difficulty),
            "rules": {"mob_griefing": if s.mob_griefing { "true" } else { "false" }, "random_tick_speed": s.random_tick_speed.to_string()},
            "trace_randoms": trace_randoms,
        }))?;
        if let Some(p) = s.player {
            let mode = if s.player_creative { "creative" } else { "survival" };
            w.call(json!({"op": "world", "sandbox": sb, "player": {"pos": p, "mode": mode, "simulation": 6, "view": 6}}))?;
        }
        if !s.blocks.is_empty() {
            w.set_blocks(sb, &scene_placed_blocks(s), PlaceMode::Raw)?;
        }
        // (tickexact 2026-11-10) the block entities' data (a merged or judge-placed scene), then
        // for a judge-placed one the shape update of every wire and the `self` update of every
        // block ([`Placement`])
        for (p, nbt) in scene_block_entities(&s.name) {
            w.merge_block_entity(sb, p, &nbt)?;
        }
        if scene_judge_placed(&s.name) {
            let placed = scene_placed_blocks(s);
            let wires: Vec<[i32; 3]> = placed.iter().filter(|(_, st)| st.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
            w.update(sb, &wires, "shape")?;
            let all: Vec<[i32; 3]> = placed.iter().map(|(p, _)| *p).collect();
            w.update(sb, &all, "self")?;
        }
        // (a bench start: the output chest set when it is not one, and emptied)
        if let Some(out) = scene_bench_output(s) {
            if !s.blocks.iter().any(|(q, st)| *q == out && st.contains("chest")) {
                w.set_blocks(sb, &[(out, BENCH_CHEST.to_string())], PlaceMode::Raw)?;
            }
            w.merge_block_entity(sb, out, "{Items:[]}")?;
        }
        // (tickexact 2026-11-10, farmopt's station) a design's Nether part: the Nether sandbox
        // (kept loaded, not simulated until a crossing), pasted as redsim's (a merged scene: its
        // cells raw, its data merged, its output chest emptied; a built one: by its drive's acts)
        let nether = match scene_nether_part(&s.name)? {
            Some(part) => {
                // (the chunks kept loaded: the part's and 4 around them, every chunk a crossing's
                // PORTAL ticket (radius 3) and its ticking neighbours need already full, so the
                // ticket's promotion waits on no chunk generation: a fixed number of level ticks)
                let ch = part.chunks;
                let v = w.call(json!({"op": "nether_create", "sandbox": sb, "chunks": [ch[0] - 4, ch[1] - 4, ch[2] + 4, ch[3] + 4], "seed": seed ^ NETHER_SALT}))?;
                let nsb = v["nether"].as_u64().ok_or_else(|| anyhow!("nether_create: no id"))? as u32;
                if scene_placement(&s.name) != Placement::Built {
                    w.set_blocks(nsb, &part.blocks, PlaceMode::Raw)?;
                    for (p, nbt) in part.spec.block_entities_at() {
                        w.merge_block_entity(nsb, [p.x, p.y, p.z], &nbt)?;
                    }
                    if let Some(out) = part.spec.output_at()
                        && part.blocks.iter().any(|(q, st)| *q == [out.x, out.y, out.z] && st.contains("chest"))
                    {
                        w.merge_block_entity(nsb, [out.x, out.y, out.z], "{Items:[]}")?;
                    }
                }
                Some((nsb, part.lo, part.hi))
            }
            None => None,
        };
        // the block-ticking and spawning chunks right before the first tick (a player's ring
        // widens them; the spawning list is shuffled every tick, spawning or not)
        let (ticking, spawning) = if s.player.is_some() {
            let v = w.call(json!({"op": "chunk_lists", "sandbox": sb}))?;
            let list = |k: &str| -> Vec<[i32; 2]> { v[k].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default() };
            (list("ticking_chunks"), list("spawning_chunks"))
        } else {
            (Vec::new(), Vec::new())
        };
        // (mob-AI 2026-11-11) this JVM's POI type holders' identity hashes: AcquirePoi's retry
        // markers are made in a HashSet<Pair<Holder<PoiType>, BlockPos>>'s order (tickexact's
        // reclaim-backoff: three free beds, the markers' delays dealt in the game's order)
        let poi_hashes = poi_identity_hashes(&mut w)?;
        Ok(GameScene { w, sb, lo: s.watch.0, hi: s.watch.1, ticking, spawning, nether, renumbered: Default::default(), last_load: String::new(), poi_hashes })
    }

    /// The bridge's `unload` / `load` of every chunk of the sandbox (a load's renumbering kept),
    /// or of a chunk box's (`unload:x0,z0,x1,z1`: its forced tickets).
    fn unload_load(&mut self, act: &str) -> Result<()> {
        // (`load@<order>`: the whole-part load with its entities in that order)
        let (act, order) = match act.split_once('@') {
            Some((a, o)) => (a, Some(o)),
            None => (act, None),
        };
        let (op, chunks) = match act.split_once(':') {
            Some((op, b)) => (op, redsim::entity::mob::drive::chunk_box(b)),
            None => (act, None),
        };
        let mut req = json!({"op": op, "sandbox": self.sb});
        if let Some(b) = chunks {
            req["chunks"] = json!(b);
        }
        if let Some(o) = order {
            req["order"] = json!(o);
        }
        let v = self.w.call(req)?;
        if let Some(seq) = v["sequence"].as_str() {
            self.last_load = seq.to_string();
        }
        if std::env::var_os("MOBAI_TRACE_LOAD").is_some() {
            eprintln!("bridge {op}: {}", v["order"].as_str().unwrap_or(""));
            if v["tick_list"].is_array() {
                eprintln!("bridge {act} tick list: {}", v["tick_list"]);
            }
        }
        if let Some(a) = v["renumbered"].as_array() {
            let pairs: Vec<(u32, u32)> = a.iter().filter_map(|p| Some((p[0].as_u64()? as u32, p[1].as_u64()? as u32))).collect();
            // (a number renumbered before follows to its newest)
            for (o, n) in &pairs {
                for v in self.renumbered.values_mut() {
                    if *v == *o {
                        *v = *n;
                    }
                }
            }
            for (o, n) in pairs {
                self.renumbered.insert(o, n);
            }
        }
        Ok(())
    }

    /// A drive entity's number now (after the loads that renumbered it).
    fn now(&self, n: u32) -> u32 {
        self.renumbered.get(&n).copied().unwrap_or(n)
    }

    /// Summon the scene's mobs (seeded); their numbers.
    pub fn summon(&mut self, s: &Scene) -> Result<Vec<u64>> {
        // (a bench start: the daylight detectors held back, placed raw with their updates)
        let held = scene_held_detectors(s);
        if !held.is_empty() {
            self.w.set_blocks(self.sb, &held, PlaceMode::Raw)?;
            let at: Vec<[i32; 3]> = held.iter().map(|(p, _)| *p).collect();
            self.w.update(self.sb, &at, "self")?;
        }
        let specs: Vec<serde_json::Value> = s.mobs.iter().map(|(ty, p, nbt)| json!({"type": ty, "pos": p, "nbt": nbt})).collect();
        let v = self.w.call(json!({"op": "summon", "sandbox": self.sb, "mobs": true, "entities": specs}))?;
        Ok(v["net_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default())
    }

    pub fn trace(&mut self, ticks: u32, draws: bool) -> Result<Vec<redsim::exact::Frame>> {
        self.trace_raw(ticks, draws)?.iter().map(mobfarm::exact::parse_frame).collect()
    }

    /// The bridge's frames of the next `ticks` ticks as it sends them (what a recording keeps).
    pub fn trace_raw(&mut self, ticks: u32, draws: bool) -> Result<Vec<serde_json::Value>> {
        let mut req = json!({"op": "trace", "sandbox": self.sb, "ticks": ticks, "draws": draws, "box": {"from": self.lo, "to": self.hi}});
        if let Some((_, lo, hi)) = self.nether {
            req["nether_box"] = json!({"from": lo, "to": hi});
        }
        let v = self.w.call(req)?;
        Ok(v["frames"].as_array().ok_or_else(|| anyhow!("trace: no frames"))?.clone())
    }

    /// A scripted action on the game's side: the blocks set like a player's (flags 3).
    pub fn apply(&mut self, blocks: &[([i32; 3], String)], ctx: &ActCtx) -> Result<()> {
        // in order: runs of block sets (placed with updates), each container fill merged, each
        // entity act through its bridge op
        let mut run: Vec<([i32; 3], &str)> = Vec::new();
        for (p, s) in blocks {
            // (tickexact 2026-11-10, ironspec's fault rule) every chunk of the sandbox saved and
            // unloaded / loaded again, between two ticks
            if s == "unload" || s == "load" || s.starts_with("unload:") || s.starts_with("load:") || s.starts_with("load@") {
                if !run.is_empty() {
                    self.w.set_blocks(self.sb, &run, PlaceMode::Update)?;
                    run.clear();
                }
                self.unload_load(s)?;
                continue;
            }
            // (tickexact 2026-11-11, ironspec's forced-phase mode) a villager's sensor start delay
            if let Some(kv) = s.strip_prefix("sensor:") {
                if !run.is_empty() {
                    self.w.set_blocks(self.sb, &run, PlaceMode::Update)?;
                    run.clear();
                }
                let (k, v) = sensor_kv(kv)?;
                self.w.call(json!({"op": "sensor_delay", "sandbox": self.sb, "pos": p, "sensor": k, "value": v}))?;
                continue;
            }
            // (a Nether build act: the Nether sandbox's, placed like a player's)
            if s.starts_with("nether:") || s.starts_with("nether-nbt:") {
                if !run.is_empty() {
                    self.w.set_blocks(self.sb, &run, PlaceMode::Update)?;
                    run.clear();
                }
                let (nsb, _, _) = self.nether.ok_or_else(|| anyhow!("a Nether act in a scene without a Nether sandbox"))?;
                if let Some(st) = s.strip_prefix("nether:") {
                    self.w.set_blocks(nsb, &[(*p, st)], PlaceMode::Update)?;
                } else if let Some(snbt) = s.strip_prefix("nether-nbt:") {
                    self.w.merge_block_entity(nsb, *p, snbt)?;
                }
                continue;
            }
            let special = s.starts_with("nbt:") || entity_act(*p, s)?.is_some();
            if special && !run.is_empty() {
                self.w.set_blocks(self.sb, &run, PlaceMode::Update)?;
                run.clear();
            }
            if let Some(snbt) = s.strip_prefix("nbt:") {
                self.w.merge_block_entity(self.sb, *p, snbt)?;
            } else if let Some(act) = entity_act(*p, s)? {
                self.apply_act(&act, ctx)?;
            } else {
                run.push((*p, s.as_str()));
            }
        }
        if !run.is_empty() {
            self.w.set_blocks(self.sb, &run, PlaceMode::Update)?;
        }
        Ok(())
    }

    /// A bench's entity act in the game (`teleport`, `memory`, `poi_take`), its entities by number.
    fn apply_act(&mut self, a: &redsim::entity::mob::drive::Act, ctx: &ActCtx) -> Result<()> {
        use redsim::entity::mob::drive::Act;
        let sb = self.sb;
        match a {
            Act::Tp(who, v) => self.w.call(json!({"op": "teleport", "sandbox": sb, "entity": self.now(ctx.number(*who)?), "pos": [v.x, v.y, v.z]}))?,
            Act::Home(who, p) => self.w.call(json!({"op": "memory", "sandbox": sb, "entity": self.now(ctx.number(*who)?), "memories": {"home": [p.x, p.y, p.z]}}))?,
            Act::LastSlept(who, t) => self.w.call(json!({"op": "memory", "sandbox": sb, "entity": self.now(ctx.number(*who)?), "memories": {"last_slept": ctx.g0 + t}}))?,
            Act::PoiTake(p) => self.w.call(json!({"op": "poi_take", "sandbox": sb, "pos": [p.x, p.y, p.z]}))?,
            Act::Unhome(who) | Act::Forget(who) => {
                let release = matches!(a, Act::Unhome(_));
                let r = self.w.call(json!({"op": "home_release", "sandbox": sb, "entity": self.now(ctx.number(*who)?), "release": release}))?;
                eprintln!("game {} e{}: the HOME record's free tickets {} -> {}", if release { "unhome" } else { "forget" }, ctx.number(*who)?, r["free_before"], r["free_after"]);
                r
            }
            Act::Unload => {
                self.unload_load("unload")?;
                json!({})
            }
            Act::Load => {
                self.unload_load("load")?;
                json!({})
            }
            Act::LoadOrdered(o) => {
                self.unload_load(&format!("load@{}", o.name()))?;
                json!({})
            }
            Act::UnloadSome(b) => {
                self.unload_load(&format!("unload:{},{},{},{}", b[0], b[1], b[2], b[3]))?;
                json!({})
            }
            Act::LoadSome(b) => {
                self.unload_load(&format!("load:{},{},{},{}", b[0], b[1], b[2], b[3]))?;
                json!({})
            }
            Act::Set(..) => bail!("a block act through apply_act"),
        };
        Ok(())
    }

    /// The sandbox's game time now (`status`).
    pub fn time_now(&mut self) -> Result<i64> {
        let v = self.w.call(json!({"op": "status", "sandbox": self.sb}))?;
        v["time"].as_i64().ok_or_else(|| anyhow!("status: no time"))
    }

    /// The bridge's `mobstate` (debugging): each mob's controls and path, one line each.
    pub fn mob_state(&mut self) -> Result<Vec<String>> {
        let v = self.w.call(json!({"op": "mobstate", "sandbox": self.sb}))?;
        Ok(v["mobs"].as_array().map(|a| a.iter().map(|m| m.to_string()).collect()).unwrap_or_default())
    }
}

/// redsim's side of `mobstate`: each AI mob's controls and path.
fn redsim_mob_state(w: &World) -> Vec<String> {
    let mut out = Vec::new();
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        if let EntityKind::Mob(m) = &e.kind
            && let Some(a) = m.ai.as_deref()
        {
            let path = a.nav.path.as_ref().map(|p| format!("{}:{}", p.next_node_index, p.nodes.iter().map(|n| format!(" {},{},{}", n.x, n.y, n.z)).collect::<String>()));
            out.push(format!(
                "{} x {:?} y {:?} z {:?} yaw {:?} move {:?} {:?} {:?} {:?} {:?} look {:?} {:?} {:?} {} path {:?}",
                m.ty.def().id, e.pos.x, e.pos.y, e.pos.z, e.y_rot, a.move_ctl.wanted_x, a.move_ctl.wanted_y, a.move_ctl.wanted_z, a.move_ctl.operation, a.move_ctl.speed_modifier, a.look.wanted_x, a.look.wanted_y, a.look.wanted_z, a.look.look_at_cooldown, path
            ));
        }
    }
    out
}

/// redsim's side of a scene: the world over the chunk box (y -64..319), the blocks raw, the mob
/// layer and its AI on with the bridge's seeding, the level random mirrored, random ticks at 3.
pub fn redsim_world(s: &Scene, seed: i64) -> Result<World> {
    let lo = BlockPos::new(s.chunks[0] * 16, -64, s.chunks[1] * 16);
    let size = [(s.chunks[2] - s.chunks[0] + 1) * 16, 384, (s.chunks[3] - s.chunks[1] + 1) * 16];
    let mut w = World::new(lo, size);
    w.mob.seed(seed, s.day_time);
    w.mob.ai.on = true;
    w.mob.ai.difficulty = s.difficulty;
    w.mob.ai.mob_griefing = s.mob_griefing;
    if let Some(p) = s.player {
        let mode = if s.player_creative { redsim::entity::mob::ai::GameMode::Creative } else { redsim::entity::mob::ai::GameMode::Survival };
        w.mob.ai.players.push(redsim::entity::mob::ai::Player { pos: redsim::entity::Vec3::new(p[0], p[1], p[2]), mode, invulnerable: false });
    }
    let mut list = Vec::new();
    for (p, st) in &scene_placed_blocks(s) {
        // a block redsim models by kind, else a natural state from the table (a nether portal)
        list.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(st).or_else(|_| redsim::terrain::land::block_of(st)).map_err(|e| anyhow!("{st}: {e}"))?));
    }
    w.place_raw(&list);
    // (tickexact 2026-11-11, ironspec's raw-placement audit) a scene placed raw (as built here, not
    // a merged or judge-placed design) whose written states one update pass would change sets up a
    // configuration no farm has, in the game and in redsim alike: refused
    if scene_placement(&s.name) == Placement::Raw {
        let c = scene_raw_conflicts(s)?;
        if !c.is_empty() {
            let cells: Vec<String> = c.iter().take(4).map(|(p, a, b)| format!("{},{},{}: {a} -> {b}", p.x, p.y, p.z)).collect();
            bail!("scene {}: {} cells whose written state one update pass would change (mctas mobai scene-lint): {}", s.name, c.len(), cells.join("; "));
        }
    }
    // (a merged or judge-placed scene: its block entities' data; a judge-placed one then the
    // judge's updates)
    let bes: Vec<(BlockPos, String)> = scene_block_entities(&s.name).into_iter().map(|(p, nbt)| (BlockPos::new(p[0], p[1], p[2]), nbt)).collect();
    redsim::redstone::container::apply_block_entities(&mut w, bes.iter().map(|(p, nbt)| (*p, nbt.as_str()))).map_err(|e| anyhow!(e))?;
    if scene_judge_placed(&s.name) {
        w.place_updates(&list);
    }
    // (a bench start: the output chest set when it is not one, and emptied, as `Run::start`)
    if let Some(out) = scene_bench_output(s) {
        let at = BlockPos::new(out[0], out[1], out[2]);
        if !matches!(w.get(at), Block::Chest { .. }) {
            w.set_block(at, Block::parse(BENCH_CHEST).map_err(|e| anyhow!(e))?, redsim::world::flags::RAW);
        }
        redsim::redstone::container::apply_block_entities(&mut w, [(at, "{Items:[]}")]).map_err(|e| anyhow!(e))?;
    }
    w.mirror_level_random(seed, s.chunks);
    w.random_ticks = Some(redsim::crops::RandomTicks::new(s.random_tick_speed, seed));
    // the sandbox's SPAWN_MOBS / SPAWN_MONSTERS rules ([`scene_spawns`]: a nether portal's random
    // tick draws only with monsters spawning)
    w.spawn_monsters = Some(scene_spawns(&s.name));
    if let Some(n) = scene_nether(&s.name, seed, s.random_tick_speed)? {
        w.nether = Some(Box::new(n));
    }
    Ok(w)
}

/// (tickexact 2026-11-10, farmopt's station) The salt of a scene's Nether level random (and its
/// randValue and loot sequences): the scene's seed ^ "nether", on both sides.
const NETHER_SALT: i64 = 0x6e65_7468_6572;

/// A drive scene's Nether part as the farm's Run::start places it (`NetherSpec`: the design's
/// `nether` lines at the anchor's Nether image): its cells and states, its block-entity data, its
/// output chest, the chunks the bridge keeps loaded for it and the box both sides watch.
struct SceneNether {
    spec: redsim::entity::mob::farm::NetherSpec,
    blocks: Vec<([i32; 3], String)>,
    chunks: [i32; 4],
    lo: [i32; 3],
    hi: [i32; 3],
}

fn scene_nether_part(name: &str) -> Result<Option<SceneNether>> {
    let Some(d) = drive_design(name) else { return Ok(None) };
    if d.nether_blocks.is_empty() {
        return Ok(None);
    }
    let spec = circuit::mobfarm::spec_of(&circuit::Circuit::from_design(d)?).map_err(|e| anyhow!(e))?;
    let Some(ns) = spec.nether else { return Ok(None) };
    let placed = ns.placed();
    let blocks: Vec<([i32; 3], String)> = placed.iter().map(|(p, b)| ([p.x, p.y, p.z], b.to_string())).collect();
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for (p, _) in &blocks {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    // (a margin of a cell around the part; its chunks and the ring a crossing's PORTAL ticket
    // simulates are the game's own business)
    let (lo, hi) = ([lo[0] - 1, lo[1] - 1, lo[2] - 1], [hi[0] + 1, hi[1] + 1, hi[2] + 1]);
    let chunks = [lo[0].div_euclid(16), lo[2].div_euclid(16), hi[0].div_euclid(16), hi[2].div_euclid(16)];
    Ok(Some(SceneNether { spec: ns, blocks, chunks, lo, hi }))
}

/// (tickexact 2026-11-10, farmopt's station) A drive scene's Nether part in redsim, as the farm's
/// `Run::start` builds it (`World::nether`: its own world, the design's `nether` lines at the
/// anchor's Nether image, its `nether nbt:` data merged, its output chest emptied; a built-in-order
/// scene's part starts empty, its drive's `nether-set` acts build it), its randoms the bridge's
/// Nether sandbox's (the level random, randValue and loot sequences from seed ^ [`NETHER_SALT`];
/// its entities numbered and seeded as the overworld's, by the scene's seed).
fn scene_nether(name: &str, seed: i64, random_tick_speed: u32) -> Result<Option<World>> {
    let Some(part) = scene_nether_part(name)? else { return Ok(None) };
    let ns = &part.spec;
    let bench_seed = drive_text(name)
        .and_then(|t| t.lines().find_map(|l| l.trim_start_matches('#').trim_start().strip_prefix("seed ").and_then(|s| s.trim().parse::<i64>().ok())))
        .unwrap_or(1);
    let o = redsim::entity::mob::farm::RunOpts { ai: true, raw: true, ..redsim::entity::mob::farm::RunOpts::steady(bench_seed, 24000, redsim::entity::mob::farm::WalkMode::Random) };
    let mut n = ns.world(&o).map_err(|e| anyhow!(e))?;
    let nseed = seed ^ NETHER_SALT;
    n.mirror_level_random(nseed, part.chunks);
    n.exact_seed = Some(seed);
    n.random_ticks = Some(redsim::crops::RandomTicks::new(random_tick_speed, nseed));
    if scene_placement(name) != Placement::Built {
        n.place_raw(&ns.placed());
        let bes = ns.block_entities_at();
        redsim::redstone::container::apply_block_entities(&mut n, bes.iter().map(|(p, s)| (*p, s.as_str()))).map_err(|e| anyhow!(e))?;
        if let Some(out) = ns.output_at()
            && matches!(n.get(out), Block::Chest { .. })
        {
            redsim::redstone::container::apply_block_entities(&mut n, [(out, "{Items:[]}")]).map_err(|e| anyhow!(e))?;
        }
    }
    Ok(Some(n))
}

/// Summon the scene's mobs in redsim (no follow-range bonus: a summon has none).
pub fn redsim_summon(w: &mut World, s: &Scene) -> Result<Vec<u64>> {
    // (a bench start: the daylight detectors held back, placed raw with their updates, as
    // `Run::arrive` does)
    let held: Vec<(BlockPos, Block)> = scene_held_detectors(s).iter().map(|(p, st)| Ok((BlockPos::new(p[0], p[1], p[2]), Block::parse(st).map_err(|e| anyhow!("{st}: {e}"))?))).collect::<Result<_>>()?;
    if !held.is_empty() {
        w.place_raw(&held);
        w.place_updates(&held);
    }
    let mut ids = Vec::new();
    for (ty, p, nbt) in &s.mobs {
        let id = redsim::entity::spawn(w, ty, *p, nbt.as_deref()).map_err(|e| anyhow!(e))?;
        ids.push(id as u64);
        // the summon's passengers, in tree order (the bridge lists them after their vehicle)
        let mut stack = vec![id];
        while let Some(v) = stack.pop() {
            let riders = redsim::entity::index_of(w, v).map(|k| w.entities.list[k].passengers.clone()).unwrap_or_default();
            for r in riders.iter().rev() {
                stack.push(*r);
            }
            if v != id {
                ids.push(v as u64);
            }
        }
    }
    for e in w.entities.list.iter_mut() {
        if let EntityKind::Mob(m) = &mut e.kind {
            m.follow_range = m.ty.def().follow_range;
        }
    }
    Ok(ids)
}

/// Where the game's frames come from: the live bridge, or a recording of it (`mobai record`).
pub enum Source {
    Live(Box<GameScene>),
    Recorded { frames: std::collections::VecDeque<serde_json::Value>, game_ids: Vec<u64>, ticking: Vec<[i32; 2]>, spawning: Vec<[i32; 2]>, poi_hashes: Option<std::collections::HashMap<String, i32>> },
}

impl Source {
    /// (mob-AI 2026-11-11) The game's POI holder identity hashes (a recording's when it kept them).
    fn poi_hashes(&self) -> Option<std::collections::HashMap<String, i32>> {
        match self {
            Source::Live(g) => Some(g.poi_hashes.clone()),
            Source::Recorded { poi_hashes, .. } => poi_hashes.clone(),
        }
    }
    /// The game's last whole-part load sequence (a live game's; a recording does not keep it).
    fn last_load(&self) -> Option<&str> {
        match self {
            Source::Live(g) => Some(g.last_load.as_str()),
            Source::Recorded { .. } => None,
        }
    }

    /// (tickexact 2026-11-11) The game's POI record at `p`: its free tickets (-1: none there; a
    /// recording does not keep them: None).
    fn poi_free(&mut self, p: [i32; 3]) -> Result<Option<i64>> {
        match self {
            Source::Live(g) => {
                let sb = g.sb;
                Ok(g.w.call(json!({"op": "poi_free", "sandbox": sb, "pos": p}))?["free"].as_i64())
            }
            Source::Recorded { .. } => Ok(None),
        }
    }

    fn ticking(&self) -> Vec<[i32; 2]> {
        match self {
            Source::Live(g) => g.ticking.clone(),
            Source::Recorded { ticking, .. } => ticking.clone(),
        }
    }

    fn spawning(&self) -> Vec<[i32; 2]> {
        match self {
            Source::Live(g) => g.spawning.clone(),
            Source::Recorded { spawning, .. } => spawning.clone(),
        }
    }

    fn summon(&mut self, s: &Scene) -> Result<Vec<u64>> {
        match self {
            Source::Live(g) => g.summon(s),
            Source::Recorded { game_ids, .. } => Ok(game_ids.clone()),
        }
    }

    fn frames(&mut self, n: u32, draws: bool) -> Result<Vec<redsim::exact::Frame>> {
        match self {
            Source::Live(g) => g.trace(n, draws),
            Source::Recorded { frames, .. } => (0..n).filter_map(|_| frames.pop_front()).map(|v| mobfarm::exact::parse_frame(&v)).collect(),
        }
    }

    fn mob_state(&mut self) -> Result<Vec<String>> {
        match self {
            Source::Live(g) => g.mob_state(),
            Source::Recorded { .. } => Ok(Vec::new()),
        }
    }

    /// The game time before the next tick: the bridge's `status`, or a recording's next frame's
    /// time less one.
    fn time_now(&mut self) -> Result<i64> {
        match self {
            Source::Live(g) => g.time_now(),
            Source::Recorded { frames, .. } => Ok(frames.front().and_then(|f| f["t"].as_i64()).map(|t| t - 1).unwrap_or(0)),
        }
    }

    /// A scripted action on the game's side (a recording has it in its frames already).
    fn apply(&mut self, b: &[([i32; 3], String)], ctx: &ActCtx) -> Result<()> {
        match self {
            Source::Live(g) => g.apply(b, ctx),
            Source::Recorded { .. } => Ok(()),
        }
    }
}

/// Run a scene on both sides for `ticks` ticks; the compare. `trace`: at that game time, the game's
/// draws of the tick (kept in the compare's `probe`) and redsim's draws of the level random (or of
/// the entity's own random) printed by call site.
#[allow(clippy::too_many_arguments)]
pub fn compare(version: &str, s: &Scene, seed: i64, ticks: u32, batch: u32, full: bool, probe: bool, trace: Option<(i64, Option<u32>)>) -> Result<Compare> {
    let game = GameScene::start(version, s, seed, probe || trace.is_some())?;
    compare_source(&mut Source::Live(Box::new(game)), s, seed, ticks, batch, full, probe, trace)
}

/// [`compare`] against any source of the game's frames.
#[allow(clippy::too_many_arguments)]
pub fn compare_source(game: &mut Source, s: &Scene, seed: i64, ticks: u32, batch: u32, full: bool, probe: bool, trace: Option<(i64, Option<u32>)>) -> Result<Compare> {
    let mut ours = redsim_world(s, seed)?;
    // (tickexact probe: MOBAI_SHOW_PUSHES: the mob-on-cart pushes logged from the first tick)
    if std::env::var("MOBAI_SHOW_PUSHES").is_ok() {
        redsim::entity::minecart::log_cart_pushes(&mut ours);
    }
    if let Some(h) = game.poi_hashes() {
        ours.mob.ai.poi_hashes = Some(h);
    }
    let ticking = game.ticking();
    if !ticking.is_empty() {
        ours.chunk_order = ticking;
    }
    if let Some(p) = s.player {
        // spawn_mobs off: natural spawning only shuffles its chunks (and checks despawning); a
        // spawning scene has monsters on (`scene_spawns`)
        use redsim::entity::mob::natural::Natural;
        let pos = redsim::entity::Vec3::new(p[0], p[1], p[2]);
        let on = scene_spawns(&s.name);
        ours.mob.natural = Some(Natural { spawning: on, spawn_enemies: on, void_biome: Some("minecraft:plains"), spawn_order: game.spawning(), ..Natural::new(vec![pos], s.difficulty) });
    }
    let mut first = true;
    // (tickexact 2026-11-10) the ticks before the summon: a settle (the blocks alone) or a
    // built-in-order scene's build (its placements and fills at their ticks, compared every tick)
    let spawn = scene_spawn(&s.name);
    let from_start = scene_counts_from_start(&s.name);
    let actions = scene_actions(&s.name);
    let (wlo, whi) = (BlockPos::new(s.watch.0[0], s.watch.0[1], s.watch.0[2]), BlockPos::new(s.watch.1[0], s.watch.1[1], s.watch.1[2]));
    let mut cmp = Compare { probe_at: trace.map(|t| t.0), ..Default::default() };
    // the game time before the first tick (a built scene's drive counts from it)
    let g_start = game.time_now()?;
    let no_entities = ActCtx { g0: g_start, ..Default::default() };
    if from_start && let Some((_, b)) = actions.iter().find(|(t, _)| *t == 0) {
        game.apply(b, &no_entities)?;
        apply_redsim(&mut ours, b, &no_entities)?;
    }
    // (the watch's baseline after the actions at 0, as the game's first traced tick has it)
    let mut early_watch = from_start.then(|| Watch::new(&ours, wlo, whi));
    // (a scene's Nether: its own watch over its part's box, its frame beside each frame)
    let nether_box = scene_nether_part(&s.name)?.map(|p| (BlockPos::new(p.lo[0], p.lo[1], p.lo[2]), BlockPos::new(p.hi[0], p.hi[1], p.hi[2])));
    let mut nwatch: Option<Watch> = match (nether_box, ours.nether.as_deref()) {
        (Some((lo, hi)), Some(n)) => Some(Watch::new(n, lo, hi)),
        _ => None,
    };
    let mut pre: i64 = 0;
    while pre < spawn as i64 {
        let mut n = (spawn as i64 - pre).min(batch as i64) as u32;
        if from_start && let Some((t, _)) = next_action(&actions, pre + 1) {
            n = n.min((*t as i64 - pre).max(1) as u32);
        }
        let frames = game.frames(n, false)?;
        if frames.is_empty() {
            bail!("the game's frames end before the scene's summon");
        }
        for f in &frames {
            if first {
                ours.time = f.t - 1;
                if let Some(d) = f.day {
                    ours.mob.day_offset = d - f.t;
                }
                // (a scene's Nether: the same clocks)
                let (t, day) = (ours.time, ours.mob.day_offset);
                if let Some(n) = ours.nether.as_deref_mut() {
                    n.time = t;
                    n.mob.day_offset = day;
                }
                first = false;
            }
            ours.tick();
            if let Some(wt) = early_watch.as_mut() {
                let mut o = wt.frame(&ours);
                if let (Some(nw), Some(n)) = (nwatch.as_mut(), ours.nether.as_deref()) {
                    o.nether = Some(Box::new(nw.frame(n)));
                }
                cmp.tick(f, &o);
            }
        }
        pre += frames.len() as i64;
        // (the acts at the summon's tick come after it)
        if from_start
            && pre < spawn as i64
            && let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == pre)
        {
            game.apply(b, &no_entities)?;
            apply_redsim(&mut ours, b, &no_entities)?;
        }
        if cmp.first.is_some() && !full {
            break;
        }
    }
    // (a probe: MOBAI_SETTLED=1: after a scene's settle, each of its blocks redsim holds in
    // another state than the scene placed)
    if scene_settle(&s.name) > 0 && std::env::var_os("MOBAI_SETTLED").is_some() {
        for (p, st) in &s.blocks {
            let now = ours.get(BlockPos::new(p[0], p[1], p[2])).to_string();
            if now != *st {
                eprintln!("settled {} {} {}: placed {st}, now {now}", p[0], p[1], p[2]);
            }
        }
    }
    // (a settled or built scene's watch starts before its summon: the game's first frame after it
    // lists the mobs as entered, its baseline the last frame before)
    let settled_watch = early_watch.or_else(|| (spawn > 0).then(|| Watch::new(&ours, wlo, whi)));
    // the game time a drive log's ticks count from: the summon's after a settle, else the start
    let t_summon = if spawn > 0 && !from_start { ours.time } else { 0 };
    let g_summon = game.time_now()?;
    let game_ids = game.summon(s)?;
    let our_ids = redsim_summon(&mut ours, s)?;
    if game_ids != our_ids {
        bail!("the game's mobs entered as {game_ids:?}, redsim's as {our_ids:?}");
    }
    let ctx = ActCtx::of(&ours, &our_ids, if from_start { g_start } else { g_summon });
    // the acts at the summon's tick (a drive's t0: the bench's memories, its POI claim), before the
    // game's first traced tick of a scene with nothing before its summon (then the watch's
    // baseline comes after them, as the game's)
    let at_summon = if from_start { spawn as i64 } else { 0 };
    if let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == at_summon) {
        game.apply(b, &ctx)?;
        apply_redsim(&mut ours, b, &ctx)?;
    }
    let mut watch = settled_watch.unwrap_or_else(|| Watch::new(&ours, wlo, whi));
    let show = std::env::var("MOBAI_SHOW").is_ok();
    let show_until: Option<i64> = std::env::var("MOBAI_SHOW_UNTIL").ok().and_then(|v| v.parse().ok());
    let mut left = if cmp.first.is_some() && !full { 0 } else { ticks };
    let state_range: Option<(i64, i64)> = std::env::var("MOBAI_STATE").ok().and_then(|v| {
        let (a, b) = v.split_once(',')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    });
    // (tickexact 2026-11-11, a probe) MOBAI_SHOW_POI="x,y,z[;x,y,z..]@T,T,..": at those ticks from
    // the summon, each cell's POI record's free tickets, the game's and redsim's
    let poi_probe: (Vec<[i32; 3]>, Vec<i64>) = std::env::var("MOBAI_SHOW_POI")
        .ok()
        .and_then(|v| {
            let (cells, times) = v.split_once('@')?;
            let cells = cells.split(';').filter_map(|c| {
                let f: Vec<i32> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (f.len() == 3).then(|| [f[0], f[1], f[2]])
            });
            let mut times: Vec<i64> = times.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            times.sort();
            Some((cells.collect(), times))
        })
        .unwrap_or_default();
    let mut done: i64 = if from_start { pre } else { 0 };
    // (tickexact 2026-11-10) the scene's expected positions (a drive log's): checked on the game's
    // frames, summed up at the end
    let expects = scene_expects(&s.name);
    let (mut exp_ok, mut exp_bad) = (0usize, Vec::<String>::new());
    while left > 0 {
        // one tick per traced step inside the state window (the controls between ticks)
        let in_window = state_range.is_some_and(|(a, b)| done + 1 >= a && done < b);
        let mut n = match state_range {
            _ if in_window => 1,
            Some((a, _)) if done + 1 < a => (left.min(batch) as i64).min(a - 1 - done).max(1) as u32,
            _ => left.min(batch),
        };
        // a batch stops at the next scripted action
        if let Some((t, _)) = next_action(&actions, done + 1) {
            n = n.min((*t as i64 - done).max(1) as u32);
        }
        // (and at the next POI probe: MOBAI_SHOW_POI)
        if let Some(pt) = poi_probe.1.iter().find(|pt| **pt > done) {
            n = n.min((*pt - done).max(1) as u32);
        }
        let frames = game.frames(n, probe || trace.is_some())?;
        if frames.is_empty() {
            // a recording that ends here
            break;
        }
        done += n as i64;
        let game_state = if in_window { game.mob_state()? } else { Vec::new() };
        for f in frames {
            if first {
                // the sandbox's clock: redsim's game time before the first traced tick
                ours.time = f.t - 1;
                if let Some(d) = f.day {
                    ours.mob.day_offset = d - f.t;
                }
                // (a scene's Nether: the same clocks)
                let (t, day) = (ours.time, ours.mob.day_offset);
                if let Some(n) = ours.nether.as_deref_mut() {
                    n.time = t;
                    n.mob.day_offset = day;
                }
                first = false;
            }
            let tracing = trace.filter(|(at, _)| *at == ours.time + 1);
            if let Some((at, ent)) = tracing {
                let addr = match ent {
                    None => ours.level_random.as_ref().map(|r| r as *const _ as usize).unwrap_or(0),
                    Some(id) => ours
                        .entities
                        .list
                        .iter()
                        .chain(ours.nether.as_deref().map(|n| n.entities.list.iter()).into_iter().flatten())
                        .find(|e| e.id == id)
                        .and_then(|e| e.random.as_ref())
                        .map(|r| r as *const _ as usize)
                        .unwrap_or(0),
                };
                eprintln!("redsim's draws at game time {at}:");
                redsim::rng::TRACE.store(addr, std::sync::atomic::Ordering::Relaxed);
            }
            ours.tick();
            redsim::rng::TRACE.store(0, std::sync::atomic::Ordering::Relaxed);
            let mut o = watch.frame(&ours);
            if let (Some(nw), Some(n)) = (nwatch.as_mut(), ours.nether.as_deref()) {
                o.nether = Some(Box::new(nw.frame(n)));
            }
            if show && o.t % 100 == 0 {
                for e in &o.entities {
                    eprintln!("t {} #{} {} pos {:?} goals {:?} path {:?} target {:?} no_action {:?}", o.t, e.n, e.ty, e.pos, e.goals, e.path, e.target, e.no_action);
                }
            }
            // (tickexact probe: MOBAI_SHOW_PUSHES="T0:T1": every push a mob's pushEntities gave a
            // minecart in redsim at each tick of T0..=T1, counted at the push (ironspec's rule))
            if let Ok(v) = std::env::var("MOBAI_SHOW_PUSHES") {
                let parts: Vec<i64> = v.split(':').filter_map(|x| x.parse().ok()).collect();
                redsim::entity::minecart::log_cart_pushes(&mut ours);
                let now = redsim::entity::minecart::take_cart_pushes(&mut ours);
                if parts.len() == 2 && o.t >= parts[0] && o.t <= parts[1] {
                    for p in now {
                        eprintln!("t {} push #{} pushed cart #{} by {:.4} {:.4} ({:.3} apart)", o.t, p.mob, p.cart, p.push[0], p.push[1], p.dist);
                    }
                }
            }
            // (tickexact probe: MOBAI_SHOW_TOUCH="T0:T1": at each tick of T0..=T1, each of redsim's
            // riders whose box touches another minecart's box (`redsim::entity::touching_cart`, the
            // walker rule's contact) or is within 0.2 of one, with its walk target)
            if let Ok(v) = std::env::var("MOBAI_SHOW_TOUCH") {
                let parts: Vec<i64> = v.split(':').filter_map(|x| x.parse().ok()).collect();
                if parts.len() == 2 && o.t >= parts[0] && o.t <= parts[1] {
                    for e in ours.entities.list.iter().filter(|e| !e.removed && e.vehicle.is_some()) {
                        let touch = redsim::entity::touching_cart(&ours, e.id, 0.0);
                        let near = redsim::entity::touching_cart(&ours, e.id, 0.2);
                        if touch || near {
                            let walk = match &e.kind {
                                redsim::entity::EntityKind::Mob(m) => m.brain.as_ref().and_then(|b| b.ex.as_ref()).map(|x| format!("walk_target {:?} path {}", x.mem.walk_target.as_ref().map(|w| format!("{:?}", w.target)), match &x.mem.path { Some(p) if p.can_reach() => "reach", Some(_) => "partial", None => "none" })).unwrap_or_default(),
                                _ => String::new(),
                            };
                            eprintln!("t {} touch #{} at {:.3} {:.3} {:.3} riding {:?}: {} {walk}", o.t, e.id, e.pos.x, e.pos.y, e.pos.z, e.vehicle, if touch { "TOUCHING a cart" } else { "within 0.2 of a cart" });
                        }
                    }
                }
            }
            // (tickexact probe: MOBAI_SHOW_WAKE: at every tick, each villager's pose in the game's
            // frame and in redsim's (a reloaded sleeper's wake tick on both sides))
            if let Ok(mut g) = RELOAD_WAKE_POSES.lock()
                && let Some(v) = g.as_mut()
            {
                let pose = |es: &[redsim::exact::EntityFrame]| es.iter().find(|e| e.ty.ends_with("villager")).and_then(|e| e.pose.clone());
                v.push((f.t, pose(&f.entities), pose(&o.entities)));
            }
            // (tickexact, the ledge report) every minecart of the game's frame, each tick
            if let Ok(mut g) = LEDGE_TRACK.lock()
                && let Some(v) = g.as_mut()
            {
                for e in f.entities.iter().filter(|e| e.ty.ends_with("minecart")) {
                    v.push((f.t, e.n, e.pos, e.motion));
                }
            }
            if std::env::var_os("MOBAI_SHOW_WAKE").is_some() {
                let g: Vec<String> = f.entities.iter().filter(|e| e.ty.ends_with("villager")).map(|e| format!("#{} {}", e.n, e.pose.as_deref().unwrap_or("-"))).collect();
                let r: Vec<String> = o.entities.iter().filter(|e| e.ty.ends_with("villager")).map(|e| format!("#{} {}", e.n, e.pose.as_deref().unwrap_or("-"))).collect();
                eprintln!("t {} wake game {} | redsim {}", f.t, g.join(" "), r.join(" "));
            }
            // (tickexact probe: MOBAI_SHOW_RIDERS="T": at tick T, each of the game's villagers: its
            // position and what it rides)
            if let Ok(v) = std::env::var("MOBAI_SHOW_RIDERS")
                && v.split(',').filter_map(|x| x.parse::<i64>().ok()).any(|x| x == f.t)
            {
                for e in f.entities.iter().filter(|e| e.ty.ends_with("villager")) {
                    let mem = |k: &str| e.memories.as_ref().and_then(|m| m.get(k).cloned());
                    eprintln!("t {} #{} villager at {:.2} {:.2} {:.2} riding {:?} activities {:?} home {:?} walk_target {:?} path {:?} cant_reach {:?}", f.t, e.n, e.pos[0], e.pos[1], e.pos[2], e.vehicle, e.activities, mem("home"), mem("walk_target"), mem("path").or_else(|| e.path.clone()), mem("cant_reach_walk_target_since"));
                }
            }
            // (tickexact probe: MOBAI_SHOW_BLOCKS="T0:T1": the game's block and container changes
            // at each tick of T0..=T1)
            if let Ok(v) = std::env::var("MOBAI_SHOW_BLOCKS") {
                let parts: Vec<i64> = v.split(':').filter_map(|x| x.parse().ok()).collect();
                if parts.len() == 2 && f.t >= parts[0] && f.t <= parts[1] {
                    for (p, s) in &f.blocks {
                        eprintln!("t {} block {} {} {} {s}", f.t, p[0], p[1], p[2]);
                    }
                    for (p, s) in &f.containers {
                        eprintln!("t {} container {} {} {} [{s}]", f.t, p[0], p[1], p[2]);
                    }
                }
            }
            // (tickexact probe: MOBAI_SHOW_IDS="n,n:T0:T1": those entities' position and motion on
            // both sides at each tick of T0..=T1)
            if let Ok(v) = std::env::var("MOBAI_SHOW_IDS") {
                let parts: Vec<&str> = v.split(':').collect();
                let ids: Vec<u32> = parts.first().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
                let (a, b) = (parts.get(1).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0), parts.get(2).and_then(|x| x.parse::<i64>().ok()).unwrap_or(i64::MAX));
                if f.t >= a && f.t <= b {
                    for n in ids {
                        // (the Nether's frame too: a crossed entity's copy)
                        let find = |fr: &redsim::exact::Frame| fr.entities.iter().find(|e| e.n == n).cloned().or_else(|| fr.nether.as_ref().and_then(|nf| nf.entities.iter().find(|e| e.n == n).cloned()));
                        let (g, e) = (find(&f), find(&o));
                        eprintln!("t {} #{n} game {:?} {:?} | redsim {:?} {:?}", f.t, g.as_ref().map(|e| e.pos), g.as_ref().map(|e| e.motion), e.as_ref().map(|e| e.pos), e.as_ref().map(|e| e.motion));
                    }
                }
            }
            if let Some(n) = show_until
                && o.t <= n
            {
                for (g, e) in f.entities.iter().zip(o.entities.iter()) {
                    eprintln!("t {} #{} game: yaw {} head {:?} body {:?} pitch {} goals {:?} path {:?} pos {:?} | redsim: yaw {} head {:?} body {:?} pitch {} goals {:?} path {:?} pos {:?}", o.t, e.n, g.yaw, g.head, g.body, g.pitch, g.goals, g.path, g.pos, e.yaw, e.head, e.body, e.pitch, e.goals, e.path, e.pos);
                    if std::env::var_os("MOBAI_SHOW_MEM").is_some() {
                        eprintln!("    game activities {:?} behaviors {:?} memories {:?}", g.activities, g.behaviors, g.memories);
                        eprintln!("    redsim activities {:?} behaviors {:?} memories {:?}", e.activities, e.behaviors, e.memories);
                    }
                }
            }
            for (_, n, p) in expects.iter().filter(|(t, _, _)| *t == f.t - t_summon) {
                // (an entity a reload brought back: its new number, redsim's renumbering, the
                // bridge's as the frames agree)
                let n = &ours.renumbered.get(n).copied().unwrap_or(*n);
                match f.entities.iter().find(|e| e.n == *n) {
                    Some(e) if (0..3).all(|k| (e.pos[k] - p[k]).abs() < 0.0006) => exp_ok += 1,
                    Some(e) => exp_bad.push(format!("t {} #{n}: the game at {:?}, the drive log expects {:?}", f.t, e.pos, p)),
                    None => exp_bad.push(format!("t {} #{n}: not in the game's frame", f.t)),
                }
            }
            cmp.tick(&f, &o);
            if in_window {
                for l in &game_state {
                    eprintln!("state t {} game   {l}", o.t);
                }
                for l in redsim_mob_state(&ours) {
                    eprintln!("state t {} redsim {l}", o.t);
                }
            }
        }
        // a scripted action between the two ticks, on both sides (a recording has the game's in
        // its frames)
        if poi_probe.1.contains(&done) {
            for p in &poi_probe.0 {
                let g = game.poi_free(*p)?;
                let r = redsim::entity::mob::ai::poi::free_tickets(&mut ours, BlockPos::new(p[0], p[1], p[2])).map_or(-1, |n| n as i64);
                let differ = if g.is_some_and(|g| g != r) { " DIFFER" } else { "" };
                eprintln!("poi t {done} {},{},{} free tickets: game {} redsim {r}{differ}", p[0], p[1], p[2], g.map_or("?".to_string(), |g| g.to_string()));
            }
        }
        if let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == done) {
            game.apply(b, &ctx)?;
            apply_redsim(&mut ours, b, &ctx)?;
            // (tickexact 2026-11-11, the reload-order rule) each whole-part load's sequence, the
            // game's and redsim's (new number and type, in the order they came back; the game's
            // types as redsim tells them apart)
            if let Some((_, a)) = b.iter().find(|(_, a)| a == "load" || a.starts_with("load@")) {
                let order = a.strip_prefix("load@").unwrap_or("save");
                let norm = |s: &str| s.split(' ').map(|t| match t.split_once(':') {
                    Some((n, ty)) => format!("{n}:{}", redsim::exact::normalize_type(ty)),
                    None => t.to_string(),
                }).collect::<Vec<_>>().join(" ");
                match game.last_load().map(norm) {
                    Some(g) if g == norm(&ours.last_load) => eprintln!("load order={order} at drive t {done}: {g}"),
                    Some(g) => eprintln!("load order={order} at drive t {done}: the sequences DIFFER:\n  game   {g}\n  redsim {}", norm(&ours.last_load)),
                    None => eprintln!("load order={order} at drive t {done}: redsim {}", norm(&ours.last_load)),
                }
            }
        }
        left -= n;
        if cmp.first.is_some() && !full {
            break;
        }
    }
    if !expects.is_empty() {
        eprintln!("the drive log's expected positions: {exp_ok} of {} matched in the game (within 0.0006){}", expects.len(), if exp_bad.is_empty() { String::new() } else { format!("; first misses: {}", exp_bad.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) });
    }
    if !ours.errors.is_empty() {
        eprintln!("redsim model errors: {:?}", ours.errors);
    }
    Ok(cmp)
}

/// `mctas mobai fixture <name>`.
#[allow(clippy::too_many_arguments)]
pub fn run(version: &str, name: &str, seed: i64, ticks: u32, full: bool, probe: bool, trace_at: Option<i64>, entity: Option<u32>) -> Result<()> {
    let all = scenes();
    let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}; scenes: {}", all.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")))?;
    let t0 = std::time::Instant::now();
    let c = compare(version, s, seed, ticks, 100, full, probe, trace_at.map(|t| (t, entity)))?;
    if let Some(t) = trace_at {
        println!("the game's draws at game time {t}:");
        for l in &c.probe {
            println!("  {l}");
        }
    }
    print!("{}", c.report());
    eprintln!("{} in {:.1} s", s.name, t0.elapsed().as_secs_f64());
    Ok(())
}

/// `mctas mobai record`: each scene run in the game alone, its frames kept (gzipped JSON lines
/// after a header: the scene's hash, the seed, the mobs' numbers, the chunk lists, how).
pub fn record(version: &str, names: &[String], seed: i64, ticks: u32, dir: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let all = scenes();
    let pick: Vec<&Scene> = if names.is_empty() {
        all.iter().collect()
    } else {
        names.iter().map(|n| all.iter().find(|s| &s.name == n).ok_or_else(|| anyhow!("no scene {n}"))).collect::<Result<_>>()?
    };
    let git = |args: &[&str]| -> String { std::process::Command::new("git").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default() };
    let now = std::process::Command::new("date").args(["-u", "+%Y-%m-%dT%H:%MZ"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let recorded = format!("{now} on {}", git(&["rev-parse", "--short", "HEAD"]));
    for s in pick {
        let t0 = std::time::Instant::now();
        let mut game = GameScene::start(version, s, seed, false)?;
        let mut frames = Vec::new();
        // the ticks before the summon (a settle, or a built scene's build with its placements and
        // fills at their ticks) are recorded ahead of it
        let actions = scene_actions(&s.name);
        let from_start = scene_counts_from_start(&s.name);
        let spawn = scene_spawn(&s.name) as i64;
        let g_start = game.time_now()?;
        let no_entities = ActCtx { g0: g_start, ..Default::default() };
        if from_start && let Some((_, b)) = actions.iter().find(|(t, _)| *t == 0) {
            game.apply(b, &no_entities)?;
        }
        let mut pre: i64 = 0;
        while pre < spawn {
            let mut n = (spawn - pre).min(100) as u32;
            if from_start && let Some((t, _)) = next_action(&actions, pre + 1) {
                n = n.min((*t as i64 - pre).max(1) as u32);
            }
            frames.extend(game.trace_raw(n, false)?);
            pre += n as i64;
            if from_start
                && pre < spawn
                && let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == pre)
            {
                game.apply(b, &no_entities)?;
            }
        }
        let g_summon = game.time_now()?;
        let game_ids = game.summon(s)?;
        // the drive's entities by line: from redsim's summon of the same scene (its numbers are the
        // game's: the compare checks they agree)
        let ctx = {
            let mut scratch = redsim_world(s, seed)?;
            let ids = redsim_summon(&mut scratch, s)?;
            ActCtx::of(&scratch, &ids, if from_start { g_start } else { g_summon })
        };
        let at_summon = if from_start { spawn } else { 0 };
        if let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == at_summon) {
            game.apply(b, &ctx)?;
        }
        let mut left = ticks;
        let mut done: i64 = if from_start { pre } else { 0 };
        while left > 0 {
            // a batch stops at the next scripted action, applied between its two ticks
            let mut n = left.min(100);
            if let Some((t, _)) = next_action(&actions, done + 1) {
                n = n.min((*t as i64 - done).max(1) as u32);
            }
            frames.extend(game.trace_raw(n, false)?);
            left -= n;
            done += n as i64;
            if let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == done) {
                game.apply(b, &ctx)?;
            }
        }
        let header = json!({
            "kind": "mobai-fixture",
            "scene": s.name,
            "scene_hash": s.hash(),
            "seed": seed,
            "ticks": ticks,
            "version": version,
            "recorded": recorded,
            "how": format!("mctas mobai record {} --seed {seed} --ticks {ticks} --dir {}", s.name, dir.display()),
            "game_ids": game_ids,
            "ticking": game.ticking,
            "spawning": game.spawning,
            "poi_types": game.poi_hashes,
        });
        let mut text = header.to_string();
        text.push('\n');
        for f in &frames {
            text.push_str(&f.to_string());
            text.push('\n');
        }
        let path = dir.join(format!("{}.rec", s.name));
        mctas::recorded::write_gz(&path, &text)?;
        eprintln!("recorded {}: {} frames in {:.1} s ({})", s.name, frames.len(), t0.elapsed().as_secs_f64(), path.display());
    }
    eprintln!("then: mctas recorded write {} --fresh", dir.display());
    Ok(())
}

/// One game run of [`unload_rate`]: the event's tick and what the rider did.
struct UnloadRun {
    /// The event's game tick (the first frame after `after` with the cell's new state).
    event: Option<i64>,
    /// The rider's pose and vehicle at the event (its frame).
    at_event: Option<(Option<String>, Option<String>)>,
    /// The rider riding its own cart at event + check (None: the run did not reach it).
    riding: Option<bool>,
    /// The rider's position then.
    pos: Option<[f64; 3]>,
    /// The game time the run reached.
    reached: i64,
    /// The summon's game time.
    g_summon: i64,
    /// The entities a reload brought back under new numbers (0: no reload happened).
    renumbered: usize,
}

/// The scene run in the game alone with `actions` (as [`record`] runs it), watched for the event
/// and the rider; `until`: the last game time to trace (the event + check when known).
#[allow(clippy::too_many_arguments)]
fn unload_run(version: &str, s: &Scene, seed: i64, actions: &[(u32, Vec<([i32; 3], String)>)], cell: [i32; 3], state: &str, after: i64, cart_line: usize, check: i64, until: i64, known_event: Option<i64>) -> Result<UnloadRun> {
    let mut game = GameScene::start(version, s, seed, false)?;
    let from_start = scene_counts_from_start(&s.name);
    anyhow::ensure!(!from_start, "unload-rate: a built-in-order scene is not supported");
    let spawn = scene_spawn(&s.name) as i64;
    let mut pre: i64 = 0;
    while pre < spawn {
        let n = (spawn - pre).min(100) as u32;
        game.trace_raw(n, false)?;
        pre += n as i64;
    }
    let g_summon = game.time_now()?;
    let _ = game.summon(s)?;
    let ctx = {
        let mut scratch = redsim_world(s, seed)?;
        let ids = redsim_summon(&mut scratch, s)?;
        ActCtx::of(&scratch, &ids, g_summon)
    };
    let cart = *ctx.lines.get(cart_line).ok_or_else(|| anyhow!("unload-rate: no line e{cart_line}"))?;
    let rider = *ctx.riders.get(cart_line).and_then(|r| r.first()).ok_or_else(|| anyhow!("unload-rate: e{cart_line} carries no rider"))?;
    if let Some((_, b)) = actions.iter().find(|(t, _)| *t == 0) {
        game.apply(b, &ctx)?;
    }
    let mut out = UnloadRun { event: known_event, at_event: None, riding: None, pos: None, reached: g_summon, g_summon, renumbered: 0 };
    let mut done: i64 = 0;
    while g_summon + done < until.max(after + 1) {
        let mut n = (until - g_summon - done).clamp(1, 100) as u32;
        if let Some((t, _)) = next_action(actions, done + 1) {
            n = n.min((*t as i64 - done).max(1) as u32);
        }
        for v in game.trace_raw(n, false)? {
            let f = mobfarm::exact::parse_frame(&v)?;
            out.reached = f.t;
            if out.event.is_none() && f.t > after && f.blocks.iter().any(|(p, st)| *p == cell && st.contains(state)) {
                out.event = Some(f.t);
            }
            let (cart_now, rider_now) = (game.now(cart), game.now(rider));
            if out.event == Some(f.t) {
                out.at_event = f.entities.iter().find(|e| e.n == rider_now).map(|e| (e.pose.clone(), e.vehicle.clone()));
            }
            if let Some(g) = out.event
                && f.t == g + check
            {
                let e = f.entities.iter().find(|e| e.n == rider_now);
                out.riding = Some(e.and_then(|e| e.vehicle.clone()).is_some_and(|v| v == format!("#{cart_now}")));
                out.pos = e.map(|e| e.pos);
            }
        }
        done += n as i64;
        if let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == done) {
            game.apply(b, &ctx)?;
        }
        if let Some(g) = out.event
            && g_summon + done >= g + check
        {
            break;
        }
    }
    out.renumbered = game.renumbered.len();
    Ok(out)
}

/// The Wilson score interval (95%) of k successes in n.
fn wilson(k: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let (z, nf, p) = (1.959964f64, n as f64, k as f64 / n as f64);
    let d = 1.0 + z * z / nf;
    let c = (p + z * z / (2.0 * nf)) / d;
    let h = z * ((p * (1.0 - p) / nf + z * z / (4.0 * nf * nf)).sqrt()) / d;
    ((c - h).max(0.0), (c + h).min(1.0))
}

/// `mctas mobai unload-rate` (see the command's help).
#[allow(clippy::too_many_arguments)]
pub fn unload_rate(version: &str, name: &str, seeds: std::ops::RangeInclusive<i64>, cell: [i32; 3], state: &str, after: i64, cart_line: usize, len: u32, check: i64) -> Result<()> {
    let all = scenes();
    let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}"))?;
    let base = scene_actions(&s.name);
    anyhow::ensure!(!base.iter().any(|(_, b)| b.iter().any(|(_, a)| a == "unload" || a == "load" || a.starts_with("unload:") || a.starts_with("load:") || a.starts_with("load@"))), "unload-rate: the scene's drive already unloads (use one without)");
    let horizon = after + 4000;
    let (mut asleep, mut fail_a, mut fail_b) = (0usize, 0usize, 0usize);
    for seed in seeds {
        let a = unload_run(version, s, seed, &base, cell, state, after, cart_line, check, horizon, None)?;
        let Some(g) = a.event else {
            println!("seed {seed}: no event after game t {after} (reached {})", a.reached);
            continue;
        };
        let t = (g - a.g_summon) as u32;
        let mut acts = base.clone();
        let mut add = |at: u32, what: &str| match acts.iter_mut().find(|(x, _)| *x == at) {
            Some((_, v)) => v.push(([0, 0, 0], what.to_string())),
            None => acts.push((at, vec![([0, 0, 0], what.to_string())])),
        };
        add(t, "unload");
        add(t + len, "load");
        acts.sort_by_key(|(x, _)| *x);
        let b = unload_run(version, s, seed, &acts, cell, state, after, cart_line, check, g + check, Some(g))?;
        let (pose, veh) = a.at_event.clone().unwrap_or((None, None));
        let is_asleep = pose.as_deref() == Some("sleeping");
        let fmt = |r: &UnloadRun| match (r.riding, r.pos) {
            (Some(true), _) => "aboard".to_string(),
            (Some(false), Some(p)) => format!("NOT aboard (at {:.2} {:.2} {:.2})", p[0], p[1], p[2]),
            (Some(false), None) => "NOT aboard (gone)".to_string(),
            (None, _) => format!("not reached (game t {})", r.reached),
        };
        println!("seed {seed}: event at game t {g} (drive t {t}); the rider then pose {pose:?} vehicle {veh:?}; at +{check}: as it is {}, with the unload {} ({} entities back under new numbers)", fmt(&a), fmt(&b), b.renumbered);
        anyhow::ensure!(b.renumbered > 0, "unload-rate: seed {seed}'s second run did not reload (nothing renumbered)");
        if is_asleep && a.riding.is_some() && b.riding.is_some() {
            asleep += 1;
            fail_a += (a.riding == Some(false)) as usize;
            fail_b += (b.riding == Some(false)) as usize;
        }
    }
    let (la, ha) = wilson(fail_a, asleep);
    let (lb, hb) = wilson(fail_b, asleep);
    println!("over the {asleep} seeds whose rider was asleep at the event: left behind as it is {fail_a}/{asleep} (95% {:.0}%..{:.0}%), with the unload {fail_b}/{asleep} (95% {:.0}%..{:.0}%)", la * 100.0, ha * 100.0, lb * 100.0, hb * 100.0);
    Ok(())
}

/// `mctas mobai sensor-sweep`: a recorded scene replayed on redsim to its action at `reveal`
/// (the window that shows a sleeper a hostile), then once for each of the 400 phase pairs of the
/// villager's NearestLivingEntitySensor and VillagerHostilesSensor (the ticks until each fires
/// next, 1..20), the action applied and the ticks until it wakes (PANIC's WakeUp) counted: the
/// distribution of the panic onset over the sensors' phases (the bridge's recording fixes one
/// pair per villager: `--check` replays it first, the delay the game showed).
pub fn sensor_sweep(name: &str, villager: u32, reveal: u32, dir: &std::path::Path) -> Result<()> {
    let all = scenes();
    let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}"))?;
    let path = dir.join(format!("{name}.rec"));
    let text = mctas::recorded::read_text(&path)?;
    let mut lines = text.lines();
    let header: serde_json::Value = serde_json::from_str(lines.next().unwrap_or("")).map_err(|e| anyhow!("{}: header: {e}", path.display()))?;
    anyhow::ensure!(header["scene_hash"].as_str() == Some(s.hash().as_str()), "{}: the scene changed since it was recorded", path.display());
    let first: serde_json::Value = serde_json::from_str(lines.next().unwrap_or("{}"))?;
    let list = |k: &str| -> Vec<[i32; 2]> { header[k].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default() };
    let seed = header["seed"].as_i64().unwrap_or(1);
    let mut ours = redsim_world(s, seed)?;
    if let Some(h) = header_poi_hashes(&header) {
        ours.mob.ai.poi_hashes = Some(h);
    }
    let ticking = list("ticking");
    if !ticking.is_empty() {
        ours.chunk_order = ticking;
    }
    if let Some(p) = s.player {
        use redsim::entity::mob::natural::Natural;
        let pos = redsim::entity::Vec3::new(p[0], p[1], p[2]);
        let on = scene_spawns(&s.name);
        ours.mob.natural = Some(Natural { spawning: on, spawn_enemies: on, void_biome: Some("minecraft:plains"), spawn_order: list("spawning"), ..Natural::new(vec![pos], s.difficulty) });
    }
    redsim_summon(&mut ours, s)?;
    let t0 = first["t"].as_i64().unwrap_or(1);
    ours.time = t0 - 1;
    if let Some(d) = first["day"].as_i64() {
        ours.mob.day_offset = d - t0;
    }
    let actions = scene_actions(name);
    for done in 1..=reveal as i64 {
        ours.tick();
        if done < reveal as i64
            && let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == done)
        {
            apply_redsim(&mut ours, b, &ActCtx::default())?;
        }
    }
    let (_, shown) = actions.iter().find(|(t, _)| *t == reveal).ok_or_else(|| anyhow!("no action of {name} at {reveal}"))?;
    let idx = ours.entities.list.iter().position(|e| e.id == villager).ok_or_else(|| anyhow!("no entity #{villager}"))?;
    let sleeping = |w: &World| matches!(&w.entities.list[idx].kind, redsim::entity::EntityKind::Mob(m) if m.sleeping_pos.is_some());
    anyhow::ensure!(sleeping(&ours), "#{villager} is not asleep at {reveal}");
    let phases = |w: &World| -> (i64, i64) {
        match &w.entities.list[idx].kind {
            redsim::entity::EntityKind::Mob(m) => m.brain.as_deref().map(|b| (b.sensors[0], b.sensors[5])).unwrap_or((0, 0)),
            _ => (0, 0),
        }
    };
    let wake = |mut w: World| -> Result<Option<u32>> {
        apply_redsim(&mut w, shown, &ActCtx::default())?;
        for d in 1..=80u32 {
            w.tick();
            if !sleeping(&w) {
                return Ok(Some(d));
            }
        }
        Ok(None)
    };
    let (la, ha) = phases(&ours);
    println!("#{villager} at {reveal}: the recorded phases (ticks to the next scan) living {la}, hostiles {ha}: wakes after {:?} gt", wake(ours.clone())?);
    let mut hist = std::collections::BTreeMap::new();
    let mut never = 0;
    for a in 1..=20i64 {
        for b in 1..=20i64 {
            let mut w = ours.clone();
            if let redsim::entity::EntityKind::Mob(m) = &mut w.entities.list[idx].kind
                && let Some(br) = m.brain.as_deref_mut()
            {
                br.sensors[0] = a;
                br.sensors[5] = b;
            }
            match wake(w)? {
                Some(d) => *hist.entry(d).or_insert(0u32) += 1,
                None => never += 1,
            }
        }
    }
    let n: u32 = hist.values().sum();
    let mean = hist.iter().map(|(d, c)| *d as f64 * *c as f64).sum::<f64>() / n.max(1) as f64;
    println!("400 phase pairs: woke in {n} (never within 80: {never}); min {:?}, max {:?}, mean {mean:.2} gt", hist.keys().next(), hist.keys().last());
    for (d, c) in &hist {
        println!("  {d:>2} gt: {c:>3} {}", "#".repeat(*c as usize));
    }
    Ok(())
}

/// `mctas mobai timeline`: a scene run in the game alone, each tick where a mob's pose, vehicle,
/// activities, running behaviors or memories (its HOME, LAST_SLEPT, LAST_WOKEN, walk target and
/// CANT_REACH_WALK_TARGET_SINCE among them) or a watched block change, printed as it happens (the
/// numbers a design is built on: when a villager lies down, wakes, loses its HOME).
pub fn timeline(version: &str, name: &str, seed: i64, ticks: u32, also: &[String]) -> Result<()> {
    let all = scenes();
    let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}"))?;
    let mut game = GameScene::start(version, s, seed, false)?;
    game.summon(s)?;
    let actions = scene_actions(&s.name);
    let mut last: std::collections::BTreeMap<(u64, &'static str), String> = Default::default();
    let mut mem_last: std::collections::BTreeMap<String, String> = Default::default();
    let mut walks: std::collections::BTreeMap<u64, Vec<(i64, i64, i64)>> = Default::default();
    let mut stamps: std::collections::BTreeMap<u64, usize> = Default::default();
    let mut left = ticks;
    let mut done: i64 = 0;
    while left > 0 {
        let mut n = left.min(100);
        if let Some((t, _)) = next_action(&actions, done + 1) {
            n = n.min((*t as i64 - done).max(1) as u32);
        }
        for f in game.trace_raw(n, false)? {
            let t = f["t"].as_i64().unwrap_or(0);
            // (a change is [x, y, z, state], or [pos, state])
            for x in f["blocks"].as_array().into_iter().flatten() {
                let (p, b) = match x.as_array().map(|a| a.len()) {
                    Some(4) => (format!("[{},{},{}]", x[0], x[1], x[2]), x[3].as_str().unwrap_or("?").to_string()),
                    _ => (x.get(0).map_or("?".to_string(), |p| p.to_string()), x.get(1).and_then(|b| b.as_str()).unwrap_or("?").to_string()),
                };
                println!("t {t} block {p}: {b}");
            }
            for e in f["entities"].as_array().into_iter().flatten() {
                let n = e["n"].as_u64().unwrap_or(0);
                let ty = e["ty"].as_str().or(e["type"].as_str()).unwrap_or("?").trim_start_matches("minecraft:");
                let at = e["pos"].as_array().map(|p| p.iter().map(|c| format!("{:.2}", c.as_f64().unwrap_or(0.0))).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                for (k, v) in [("pose", &e["pose"]), ("vehicle", &e["vehicle"]), ("activities", &e["activities"])] {
                    let now = if v.is_null() { String::new() } else { v.to_string() };
                    let was = last.insert((n, k), now.clone());
                    if was.as_ref() != Some(&now) && !(was.is_none() && now.is_empty()) {
                        println!("t {t} #{n} {ty} at {at} {k}: {now}");
                    }
                }
                // the memories a bed's story is told in (the sensors' lists, paths and gaze left out,
                // unless named by `also`)
                for (m, v) in e["memories"].as_object().into_iter().flatten() {
                    if !also.iter().any(|a| a == m) && matches!(m.as_str(), "mobs" | "visible_mobs" | "path" | "walk_target" | "look_target" | "nearest_players" | "nearest_visible_player" | "interactable_doors" | "doors_to_close" | "secondary_job_site" | "nearest_bed") {
                        continue;
                    }
                    let now = v.to_string();
                    let key = format!("{n} {m}");
                    if mem_last.insert(key, now.clone()).as_ref() != Some(&now) {
                        println!("t {t} #{n} {ty} at {at} memory {m}: {now}");
                        // (with `also` walk_target: the pick's offset from the mob's block, for the
                        // summary at the end)
                        if m == "walk_target" {
                            let c: Vec<i64> = now.trim_matches('"').strip_prefix("walk block ").map(|r| r.split(' ').take(3).filter_map(|s| s.parse().ok()).collect()).unwrap_or_default();
                            let p: Vec<f64> = e["pos"].as_array().map(|a| a.iter().map(|c| c.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default();
                            if c.len() == 3 && p.len() == 3 {
                                walks.entry(n).or_default().push((c[0] - p[0].floor() as i64, c[1] - p[1].floor() as i64, c[2] - p[2].floor() as i64));
                            }
                        }
                        if m == "cant_reach_walk_target_since" {
                            *stamps.entry(n).or_default() += 1;
                        }
                    }
                }
                let keys: Vec<String> = e["memories"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                let gone: Vec<String> = mem_last.keys().filter(|k| k.split_once(' ').is_some_and(|(id, m)| id == n.to_string() && !keys.iter().any(|x| x == m))).cloned().collect();
                for k in gone {
                    mem_last.remove(&k);
                    println!("t {t} #{n} {ty} at {at} memory {} erased", k.split_once(' ').map(|(_, m)| m).unwrap_or(""));
                }
            }
        }
        left -= n;
        done += n as i64;
        if let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == done) {
            println!("(scene action after {done} ticks: {b:?})");
            game.apply(b, &ActCtx::default())?;
        }
    }
    // (with `also` walk_target: each mob's picks by their offset from its block (a pick repeated at
    // once counts once), and its can't-reach stamps)
    for (n, w) in &walks {
        let hist = |f: fn(&(i64, i64, i64)) -> i64| {
            let mut h: std::collections::BTreeMap<i64, usize> = Default::default();
            for x in w {
                *h.entry(f(x)).or_default() += 1;
            }
            h.iter().map(|(k, c)| format!("{k:+}:{c}")).collect::<Vec<_>>().join(" ")
        };
        let neg = w.iter().filter(|x| x.0 < 0).count();
        let pos = w.iter().filter(|x| x.0 > 0).count();
        println!("walk picks #{n}: {} picks, dx<0 {neg}, dx=0 {}, dx>0 {pos}; dx {}; dz {}; stamps {}", w.len(), w.len() - neg - pos, hist(|x| x.0), hist(|x| x.2), stamps.get(n).copied().unwrap_or(0));
    }
    Ok(())
}

/// `mctas mobai replay`: every recording replayed on redsim against its frames (no game). A scene
/// changed since it was recorded fails (its hash), as does any difference.
/// (tickexact 2026-11-11, ironspec's slope merge) `mctas mobai rec-carts <recording>`: a recorded
/// scene's minecarts lane by lane (a lane: the carts sharing a block x, and with `band` > 0 a band
/// of `band` blocks of z), each `every` ticks: their positions along z, speeds, which are ridden,
/// and the gaps between neighbours (centres under 0.01 apart: MERGED); then per lane the closest
/// pair over the run and the first contact, with the arriving (faster) cart's stored speed the tick
/// before. `summary`: one line a lane, nothing per tick.
pub fn rec_carts(path: &std::path::Path, every: u32, band: i32, summary: bool, xz: bool) -> Result<()> {
    let text = mctas::recorded::read_text(path)?;
    let mut lines = text.lines();
    let _header = lines.next();
    // per lane x: per tick, (n, pos, motion, ridden)
    type Cart = (u64, [f64; 3], [f64; 3], bool);
    let mut lanes: std::collections::BTreeMap<(i64, i64), Vec<(usize, Vec<Cart>)>> = std::collections::BTreeMap::new();
    // (each cart stays in the lane it starts in, its x and z band at its first frame: a cart that
    // bounces across a band's edge is not counted as a lane of its own)
    let mut lane_of: std::collections::HashMap<u64, (i64, i64)> = std::collections::HashMap::new();
    for (t, l) in lines.enumerate() {
        let f: serde_json::Value = serde_json::from_str(l).map_err(|e| anyhow!("frame {t}: {e}"))?;
        let ents = f["entities"].as_array().cloned().unwrap_or_default();
        let v3 = |v: &serde_json::Value| [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0), v[2].as_f64().unwrap_or(0.0)];
        let ridden: std::collections::HashSet<u64> = ents.iter().filter_map(|e| e["vehicle"].as_str().and_then(|s| s.strip_prefix('#')).and_then(|s| s.parse().ok())).collect();
        let mut by_lane: std::collections::BTreeMap<(i64, i64), Vec<Cart>> = std::collections::BTreeMap::new();
        for e in ents.iter().filter(|e| e["type"] == "minecart") {
            let n = e["n"].as_u64().unwrap_or(0);
            let p = v3(&e["pos"]);
            let zb = if band > 0 { (p[2].floor() as i64).div_euclid(band as i64) } else { 0 };
            let key = *lane_of.entry(n).or_insert((p[0].floor() as i64, zb));
            by_lane.entry(key).or_default().push((n, p, v3(&e["motion"]), ridden.contains(&n)));
        }
        for (x, mut cs) in by_lane {
            cs.sort_by(|a, b| a.1[2].total_cmp(&b.1[2]));
            lanes.entry(x).or_default().push((t, cs));
        }
    }
    let gap = |a: &Cart, b: &Cart| ((a.1[0] - b.1[0]).powi(2) + (a.1[1] - b.1[1]).powi(2) + (a.1[2] - b.1[2]).powi(2)).sqrt();
    let cart = |c: &Cart| {
        if xz {
            format!("#{}{} x {:.4} z {:.4} y {:.4} vx {:+.4} vz {:+.4}", c.0, if c.3 { "R" } else { "E" }, c.1[0], c.1[2], c.1[1], c.2[0], c.2[2])
        } else {
            format!("#{}{} z {:.4} y {:.4} vz {:+.4}", c.0, if c.3 { "R" } else { "E" }, c.1[2], c.1[1], c.2[2])
        }
    };
    // (with `summary`: per lane the faster cart's starting speed, the contact speed, the closest
    // centres, merged; printed at the end in starting-speed order as runs of merged and not)
    let mut sweep: Vec<(f64, Option<f64>, f64, bool)> = Vec::new();
    for ((x, zb), ticks) in &lanes {
        if !summary {
            println!("lane x {x}{}:", if band > 0 { format!(" z band {zb}") } else { String::new() });
        }
        let mut closest: Option<(f64, usize, u64, u64)> = None;
        let mut was_merged = false;
        let mut merged_at: Option<usize> = None;
        let mut merged_pair: Option<(Cart, Cart)> = None;
        // (the first contact: neighbours whose boxes, inflated 0.2 sideways as pushAndPickupEntities
        // reads them, overlap: horizontal centres under 1.18 and heights under 0.7 apart; the
        // arriving cart's (the faster one's) stored speed the tick before; a pair already touching
        // at the lane's first frame (a pressed queue) is not a contact)
        let touching = |a: &Cart, b: &Cart| ((a.1[0] - b.1[0]).powi(2) + (a.1[2] - b.1[2]).powi(2)).sqrt() < 1.18 && (a.1[1] - b.1[1]).abs() < 0.7;
        let start_pairs: std::collections::HashSet<(u64, u64)> = ticks.first().map(|(_, cs)| cs.windows(2).filter(|w| touching(&w[0], &w[1])).map(|w| (w[0].0.min(w[1].0), w[0].0.max(w[1].0))).collect()).unwrap_or_default();
        let mut contact: Option<(usize, f64, f64)> = None;
        let mut prev: Option<&Vec<Cart>> = None;
        for (t, cs) in ticks {
            if contact.is_none() {
                for w in cs.windows(2) {
                    let h = ((w[0].1[0] - w[1].1[0]).powi(2) + (w[0].1[2] - w[1].1[2]).powi(2)).sqrt();
                    if touching(&w[0], &w[1]) && !start_pairs.contains(&(w[0].0.min(w[1].0), w[0].0.max(w[1].0))) {
                        let speed = |n: u64| prev.and_then(|p| p.iter().find(|c| c.0 == n)).map_or(f64::NAN, |c| c.2[0].hypot(c.2[2]));
                        contact = Some((*t, speed(w[0].0).max(speed(w[1].0)), h));
                        break;
                    }
                }
            }
            prev = Some(cs);
            let gaps: Vec<f64> = cs.windows(2).map(|w| gap(&w[0], &w[1])).collect();
            for (k, g) in gaps.iter().enumerate() {
                if closest.is_none_or(|c| *g < c.0) {
                    closest = Some((*g, *t, cs[k].0, cs[k + 1].0));
                }
            }
            // (every `every` ticks, the ticks a merge begins or ends, and the last)
            let merged = gaps.iter().any(|g| *g < 0.01);
            if merged && merged_at.is_none() {
                merged_at = Some(*t);
                if let Some(k) = gaps.iter().position(|g| *g < 0.01) {
                    merged_pair = Some((cs[k], cs[k + 1]));
                }
            }
            if !summary && ((every > 0 && *t as u32 % every == 0) || merged != was_merged || *t + 1 == ticks.len()) {
                let parts: Vec<String> = cs.iter().map(cart).collect();
                let gs: Vec<String> = gaps.iter().map(|g| format!("{g:.4}")).collect();
                println!("  t{t}: {} | gaps {}{}", parts.join(", "), gs.join(" "), if merged { "  MERGED" } else { "" });
            }
            was_merged = merged;
        }
        if summary {
            // (one line: the lane, its carts' riders in z order and their speeds at the first frame,
            // the first contact, the closest centres, whether merged and whether still at the end)
            let first = &ticks[0].1;
            let carts: Vec<String> = first.iter().map(|c| format!("{}{:.2}", if c.3 { "R" } else { "E" }, c.2[0].hypot(c.2[2]))).collect();
            let end_merged = ticks.last().is_some_and(|(_, cs)| cs.windows(2).any(|w| gap(&w[0], &w[1]) < 0.01));
            let c = contact.map_or("no contact".to_string(), |(t, s, h)| format!("contact t{t} at {s:.4} (centres {h:.3})"));
            let g = closest.map_or(f64::NAN, |c| c.0);
            let pair = merged_pair.map_or(String::new(), |(a, b)| format!(" #{}{} with #{}{}", a.0, if a.3 { "R" } else { "E" }, b.0, if b.3 { "R" } else { "E" }));
            println!("x {x} z band {zb}: {} | {c} | closest {g:.4}{}", carts.join(" "), match merged_at {
                Some(t) => format!(" | MERGED t{t}{pair}{}", if end_merged { " to the end" } else { ", parted" }),
                None => String::new(),
            });
            let start = first.iter().map(|c| c.2[0].hypot(c.2[2])).fold(0.0f64, f64::max);
            sweep.push((start, contact.map(|c| c.1), g, merged_at.is_some()));
        } else if let Some((g, t, a, b)) = closest {
            let first = contact.map_or(String::new(), |(t, s, h)| format!("; first contact t{t} (horizontal centres {h:.4}), the arriving cart's stored speed the tick before {s:.4}"));
            println!("  closest pair: #{a} #{b} {g:.4} apart at t{t}{}{first}", if g < 0.01 { " (MERGED)" } else { "" });
        }
    }
    if summary && !sweep.is_empty() {
        sweep.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut k = 0;
        println!("by starting speed (runs): ");
        while k < sweep.len() {
            let m = sweep[k].3;
            let mut j = k;
            while j + 1 < sweep.len() && sweep[j + 1].3 == m {
                j += 1;
            }
            let run = &sweep[k..=j];
            let cs: Vec<f64> = run.iter().filter_map(|r| r.1).collect();
            let (clo, chi) = (cs.iter().copied().fold(f64::INFINITY, f64::min), cs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
            let near = run.iter().map(|r| r.2).fold(f64::INFINITY, f64::min);
            println!(
                "  start {:.2}..{:.2} ({} lanes): {}; contact speed {}{}",
                run[0].0,
                run[run.len() - 1].0,
                run.len(),
                if m { "MERGED" } else { "not merged" },
                if cs.is_empty() { "none (no contact)".to_string() } else { format!("{clo:.4}..{chi:.4}") },
                if m { String::new() } else { format!(", closest centres {near:.4}") }
            );
            k = j + 1;
        }
    }
    Ok(())
}

/// (tickexact 2026-11-11, 58f770cc's plate question) A recorded scene's pressure plates tick by tick:
/// each plate's powered state (the scene's, then the recording's block changes) and every mob whose
/// box overlaps the plate's touch box (`BasePressurePlateBlock.TOUCH_AABB`: x and z 1/16..15/16, y
/// 0..4/16 of its cell) in x and z, with how far its top reaches into the cell (an adult villager
/// 1.95 tall, a baby (age < 0) 0.975); the least such reach while a mob is in the column, and the
/// ticks the plate turns on and off.
pub fn rec_plate(path: &std::path::Path, scene: &str) -> Result<()> {
    let s = scenes().into_iter().find(|s| s.name == scene).ok_or_else(|| anyhow!("no scene {scene}"))?;
    // (a tripwire's string too: its box `TripWireBlock.SHAPE_ATTACHED`, y 1/16..2.5/16 of its cell,
    // the whole cell across; any entity counts, a cart too)
    let mut plates: Vec<([i32; 3], bool)> = scene_placed_blocks(&s).iter().filter(|(_, st)| st.contains("pressure_plate") || st.starts_with("minecraft:tripwire[")).map(|(p, st)| (*p, st.contains("powered=true"))).collect();
    plates.sort();
    let wire: Vec<bool> = {
        let placed = scene_placed_blocks(&s);
        plates.iter().map(|(p, _)| placed.iter().any(|(q, st)| q == p && st.starts_with("minecraft:tripwire["))).collect()
    };
    let text = mctas::recorded::read_text(path)?;
    let mut lines = text.lines();
    let _header = lines.next();
    // per plate: (the least reach while in the column, its tick), the on/off ticks, the first tick
    // a mob's top is in the cell
    let mut least: Vec<Option<(f64, usize, u64)>> = vec![None; plates.len()];
    let mut first_in: Vec<Option<usize>> = vec![None; plates.len()];
    let mut edges: Vec<Vec<(usize, bool)>> = vec![Vec::new(); plates.len()];
    for (t, l) in lines.enumerate() {
        let f: serde_json::Value = serde_json::from_str(l).map_err(|e| anyhow!("frame {t}: {e}"))?;
        for c in f["blocks"].as_array().into_iter().flatten() {
            let p = [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32, c[2].as_i64().unwrap_or(0) as i32];
            if let Some(k) = plates.iter().position(|(q, _)| *q == p) {
                let on = c[3].as_str().unwrap_or("").contains("powered=true");
                if on != plates[k].1 {
                    plates[k].1 = on;
                    edges[k].push((t, on));
                }
            }
        }
        for e in f["entities"].as_array().into_iter().flatten().filter(|e| e["type"] != "boat") {
            let p = [e["pos"][0].as_f64().unwrap_or(0.0), e["pos"][1].as_f64().unwrap_or(0.0), e["pos"][2].as_f64().unwrap_or(0.0)];
            let baby = e["age"].as_i64().is_some_and(|a| a < 0);
            let cart = e["type"] == "minecart";
            let (h, hw) = if cart { (0.7, 0.49) } else if baby { (0.975, 0.15) } else { (1.95, 0.3) };
            for (k, (q, _)) in plates.iter().enumerate() {
                // (a plate's touch box is inset 1/16 and counts mobs only; a string's box is the
                // whole cell across and counts any entity)
                if cart && !wire[k] {
                    continue;
                }
                let (inset, width) = if wire[k] { (0.0, 1.0) } else { (0.0625, 0.875) };
                let (x0, z0) = (q[0] as f64 + inset, q[2] as f64 + inset);
                if p[0] + hw > x0 && p[0] - hw < x0 + width && p[2] + hw > z0 && p[2] - hw < z0 + width {
                    // (a plate: how far the top reaches into the cell; a string: the vertical
                    // overlap with its box, y + 1/16 .. y + 2.5/16)
                    let reach = if wire[k] { (p[1] + h).min(q[1] as f64 + 0.15625) - p[1].max(q[1] as f64 + 0.0625) } else { p[1] + h - q[1] as f64 };
                    let n = e["n"].as_u64().unwrap_or(0);
                    if least[k].is_none_or(|(r, _, _)| reach < r) {
                        least[k] = Some((reach, t, n));
                    }
                    if reach > 0.0 && first_in[k].is_none() {
                        first_in[k] = Some(t);
                    }
                }
            }
        }
    }
    for (k, (q, _)) in plates.iter().enumerate() {
        let l = least[k].map_or("no mob in its column".to_string(), |(r, t, n)| format!("least reach into the cell {r:+.4} (#{n} at t{t})"));
        let fi = first_in[k].map_or(String::new(), |t| format!(", a top first in the cell at t{t}"));
        let ed: Vec<String> = edges[k].iter().map(|(t, on)| format!("{} t{t}", if *on { "on" } else { "off" })).collect();
        println!("plate {},{},{}: {l}{fi}; {}", q[0], q[1], q[2], if ed.is_empty() { "never powered".to_string() } else { ed.join(", ") });
    }
    Ok(())
}

/// (tickexact 2026-11-11, ironspec's side push) A recorded scene's carts in pairs of lanes (`pairs`:
/// each the block x of two lanes, one cart each): every tick their boxes, inflated 0.2 sideways as
/// pushAndPickupEntities reads them, overlap the other's, with both carts' positions and velocities
/// and each velocity's departure from free running (its own the tick before times the rail's
/// friction, 0.997 ridden, 0.96 empty: a push shows as a departure); then each pair's largest
/// departure.
pub fn rec_side(path: &std::path::Path, pairs: &[(i64, i64)]) -> Result<()> {
    let text = mctas::recorded::read_text(path)?;
    let mut lines = text.lines();
    let _header = lines.next();
    type C = ([f64; 3], [f64; 3], bool);
    let mut frames: Vec<std::collections::HashMap<i64, C>> = Vec::new();
    for (t, l) in lines.enumerate() {
        let f: serde_json::Value = serde_json::from_str(l).map_err(|e| anyhow!("frame {t}: {e}"))?;
        let ents = f["entities"].as_array().cloned().unwrap_or_default();
        let v3 = |v: &serde_json::Value| [v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0), v[2].as_f64().unwrap_or(0.0)];
        let ridden: std::collections::HashSet<u64> = ents.iter().filter_map(|e| e["vehicle"].as_str().and_then(|s| s.strip_prefix('#')).and_then(|s| s.parse().ok())).collect();
        let mut m = std::collections::HashMap::new();
        for e in ents.iter().filter(|e| e["type"] == "minecart") {
            let p = v3(&e["pos"]);
            m.insert(p[0].floor() as i64, (p, v3(&e["motion"]), ridden.contains(&e["n"].as_u64().unwrap_or(0))));
        }
        frames.push(m);
    }
    for &(xa, xb) in pairs {
        println!("pair x {xa} / {xb}:");
        let mut worst = (0.0f64, 0usize);
        for t in 1..frames.len() {
            let (Some(a), Some(b), Some(pa), Some(pb)) = (frames[t].get(&xa), frames[t].get(&xb), frames[t - 1].get(&xa), frames[t - 1].get(&xb)) else { continue };
            let dev = |c: &C, p: &C| {
                let f = if c.2 { 0.997 } else { 0.96 };
                ((c.1[0] - p.1[0] * f).powi(2) + (c.1[2] - p.1[2] * f).powi(2)).sqrt()
            };
            let (da, db) = (dev(a, pa), dev(b, pb));
            let near = (a.0[0] - b.0[0]).abs() < 1.18 && (a.0[2] - b.0[2]).abs() < 1.18 && (a.0[1] - b.0[1]).abs() < 0.7;
            if near {
                println!("  t{t}: A z {:.4} vz {:+.4} (dev {:.4}) | B z {:.4} y {:.3} vz {:+.4} (dev {:.4})", a.0[2], a.1[2], da, b.0[2], b.0[1], b.1[2], db);
                if da.max(db) > worst.0 {
                    worst = (da.max(db), t);
                }
            }
        }
        println!("  largest departure while overlapping: {:.4} at t{}", worst.0, worst.1);
    }
    Ok(())
}

/// (tickexact 2026-11-11, ironspec's hatch sight lines) `mctas mobai hatch-sweep <rec>`: redsim's line
/// of sight (bridged by scene siding-hatch, 0 differences) from the napper's eye over its whole range
/// to the dorm zombie's eye, the hatch closed and open, with the sensor's 8-block reach between their
/// feet: the stand-up cell (feet x 1.3..1.5, z 2.3..2.7, y 0; eye 1.62), the sleeper (in the bed,
/// eye 0.2 over its 0.6875) and a rider on the T (seated eye 1.68 over the rail). The zombie's place
/// from the recording's first frame (lane 0).
pub fn hatch_sweep(path: &std::path::Path) -> Result<()> {
    let s = scenes().into_iter().find(|s| s.name == "siding-hatch").ok_or_else(|| anyhow!("no scene siding-hatch"))?;
    let text = mctas::recorded::read_text(path)?;
    let f: serde_json::Value = serde_json::from_str(text.lines().nth(1).ok_or_else(|| anyhow!("no frame"))?)?;
    let z = f["entities"].as_array().into_iter().flatten().find(|e| e["type"] == "zombie" && e["pos"][2].as_f64().is_some_and(|z| z < 20.0)).ok_or_else(|| anyhow!("no zombie in lane 0"))?;
    let zp = [z["pos"][0].as_f64().unwrap_or(0.0), z["pos"][1].as_f64().unwrap_or(0.0), z["pos"][2].as_f64().unwrap_or(0.0)];
    let zeye = redsim::entity::Vec3::new(zp[0], zp[1] + 1.74, zp[2]);
    println!("the zombie at {:.3},{:.3},{:.3} (design {:.3},{:.3},{:.3}), its eye {:.3} up", zp[0], zp[1], zp[2], zp[0] - 8.0, zp[1] - 68.0, zp[2] - 5.0, 1.74);
    let mut w = redsim_world(&s, 1)?;
    let at = |p: [f64; 3]| [p[0] + 8.0, p[1] + 68.0, p[2] + 5.0];
    for open in [false, true] {
        let st = format!("minecraft:iron_trapdoor[facing=west,half=top,open={open},powered=false,waterlogged=false]");
        let h = siding_hatch_at(0);
        w.set_block(BlockPos::new(h[0], h[1], h[2]), Block::parse(&st).map_err(|e| anyhow!(e))?, redsim::world::flags::RAW);
        println!("hatch {}:", if open { "open" } else { "closed" });
        let mut probe = |what: &str, feet: [f64; 3], eye: f64| {
            let p = at(feet);
            let from = redsim::entity::Vec3::new(p[0], p[1] + eye, p[2]);
            let sees = redsim::entity::mob::los::has_line_of_sight(&w, from, zeye);
            let dist = ((p[0] - zp[0]).powi(2) + (p[1] - zp[1]).powi(2) + (p[2] - zp[2]).powi(2)).sqrt();
            format!("{what} {}{}", if sees { "SEES" } else { "blocked" }, if dist <= 8.0 { String::new() } else { format!(" (feet {dist:.2} apart: past the sensor's 8)") })
        };
        let mut row = Vec::new();
        for x in [1.3, 1.35, 1.4, 1.45, 1.5] {
            for zz in [2.3, 2.4, 2.5, 2.6, 2.7] {
                row.push(probe(&format!("({x:.2},{zz:.1})"), [x, 0.0, zz], 1.62));
            }
            println!("  stand-up cell: {}", row.join("; "));
            row.clear();
        }
        println!("  sleeper: {}", probe("(0.50,0.6875,2.50)", [0.5, 0.6875, 2.5], 0.2));
        let t: Vec<String> = [[1.5, 0.0625, 0.5], [1.25, 0.0625, 0.75], [1.5, 0.0625, 1.0], [1.0, 0.0625, 0.5], [0.5, 0.0625, 0.5]].iter().map(|p| probe(&format!("({:.2},{:.2})", p[0], p[2]), *p, 1.62)).collect();
        println!("  T rider: {}", t.join("; "));
    }
    Ok(())
}

/// (tickexact 2026-11-11, ironspec's junction-settle) `mctas mobai rec-junction <rec>`: per lane of scene
/// junction-settle its junction's settled shape (its last state in the recording) and each cart's
/// route: the arm it ends on (N, E, S, W from the junction, or beyond the west arm's curve: "W→S")
/// and its distance out, with its yaw.
pub fn rec_junction(path: &std::path::Path) -> Result<()> {
    let text = mctas::recorded::read_text(path)?;
    let mut lines = text.lines();
    let _header = lines.next();
    let frames: Vec<serde_json::Value> = lines.map(|l| serde_json::from_str(l).map_err(|e| anyhow!("{e}"))).collect::<Result<_>>()?;
    let lanes = junction_lanes();
    let arm = ["N", "E", "S", "W"];
    let settle = ["unpowered", "placed, then powered by a repeater", "placed onto power", "placed, then a redstone block put under it"];
    let carts = |f: &serde_json::Value| -> Vec<(u64, [f64; 3], f64)> {
        f["entities"].as_array().into_iter().flatten().filter(|e| e["type"] == "minecart").map(|e| (e["n"].as_u64().unwrap_or(0), [e["pos"][0].as_f64().unwrap_or(0.0), e["pos"][1].as_f64().unwrap_or(0.0), e["pos"][2].as_f64().unwrap_or(0.0)], e["yaw"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0))).collect()
    };
    let (first, last) = (carts(&frames[0]), carts(frames.last().ok_or_else(|| anyhow!("no frames"))?));
    for (k, (j, missing, s, (entry, sp, ridden, gap), router)) in lanes.iter().enumerate() {
        // the junction's last state
        let mut shape = "(never placed)".to_string();
        for f in &frames {
            for c in f["blocks"].as_array().into_iter().flatten() {
                if c[0].as_i64() == Some(j[0] as i64) && c[1].as_i64() == Some(j[1] as i64) && c[2].as_i64() == Some(j[2] as i64) {
                    shape = c[3].as_str().unwrap_or("?").trim_start_matches("minecraft:").to_string();
                }
            }
        }
        let inside = |p: &[f64; 3]| (p[0] - j[0] as f64 - 0.5).abs() <= 5.6 && p[2] - j[2] as f64 - 0.5 >= -5.6 && p[2] - j[2] as f64 - 0.5 <= 9.6;
        let mine: Vec<u64> = first.iter().filter(|(_, p, _)| inside(p)).map(|(n, ..)| *n).collect();
        let mut ends = Vec::new();
        let place = |p: &[f64; 3]| -> (String, f64) {
            let (dx, dz) = (p[0] - j[0] as f64 - 0.5, p[2] - j[2] as f64 - 0.5);
            if *router && dx < -1.5 && dz > 0.5 {
                ("W→S".to_string(), dz)
            } else if dx.abs() > dz.abs() {
                ((if dx > 0.0 { "E" } else { "W" }).to_string(), dx.abs())
            } else {
                ((if dz > 0.0 { "S" } else { "N" }).to_string(), dz.abs())
            }
        };
        for n in &mine {
            // (the route: the first arm the cart reaches 1.5 out on that is not its entry arm; a
            // leader coming back off a stop later does not count)
            let entry_arm = arm[*entry as usize];
            let route = frames.iter().enumerate().find_map(|(t, f)| {
                carts(f).into_iter().find(|(m, ..)| m == n).and_then(|(_, p, yaw)| {
                    let (a, d) = place(&p);
                    (a != entry_arm && d >= 1.5).then(|| format!("{a} by t{t} (yaw {yaw:.0})"))
                })
            });
            let Some((_, p, _)) = last.iter().find(|(m, ..)| m == n) else { continue };
            let (a, d) = place(p);
            ends.push(format!("#{n} {}; ends {a} {d:.2}", route.map_or("never left its arm".to_string(), |r| format!("out {r}"))));
        }
        println!(
            "lane {k}{}: missing {}, {}, from {} at {sp}{}{}; junction {shape}; {}",
            if *router { " (router S)" } else { "" },
            arm[*missing as usize],
            settle[*s as usize],
            arm[*entry as usize],
            if *ridden { " ridden" } else { " empty" },
            if *gap > 0 { format!(", a follower {gap} gt behind") } else { String::new() },
            ends.join("; ")
        );
    }
    Ok(())
}

/// (tickexact 2026-11-11, farmopt's and ironspec's H rest-place contacts) `mctas mobai rec-hcontact
/// <rec>`: per lane of scene h-contact the arrival's closest centre distance to the cart ahead of it
/// (and the tick), the closest of any two of the lane's carts, a merge (centres within 0.05), how
/// far back up the slope each waiting cart went (and whether one reached the top flat: T's cell), and
/// where the arrival came to rest.
pub fn rec_hcontact(path: &std::path::Path) -> Result<()> {
    let text = mctas::recorded::read_text(path)?;
    let mut lines = text.lines();
    let _header = lines.next();
    let frames: Vec<serde_json::Value> = lines.map(|l| serde_json::from_str(l).map_err(|e| anyhow!("{e}"))).collect::<Result<_>>()?;
    let carts = |f: &serde_json::Value| -> Vec<(u64, [f64; 3])> {
        f["entities"].as_array().into_iter().flatten().filter(|e| e["type"] == "minecart").map(|e| (e["n"].as_u64().unwrap_or(0), [e["pos"][0].as_f64().unwrap_or(0.0), e["pos"][1].as_f64().unwrap_or(0.0), e["pos"][2].as_f64().unwrap_or(0.0)])).collect()
    };
    let first = carts(&frames[0]);
    let d3 = |a: [f64; 3], b: [f64; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    // (per lane kind: arrival merges, other pair merges, T's cell, closest without a merge, front offset)
    #[allow(clippy::type_complexity)]
    let mut summary: std::collections::BTreeMap<String, (Vec<f64>, Vec<f64>, Vec<f64>, f64, f64)> = std::collections::BTreeMap::new();
    // (per lane kind and speed: the lanes run, those with a merge or a cart into T's cell)
    let mut lanes_at: std::collections::BTreeMap<(String, String), (u32, u32)> = std::collections::BTreeMap::new();
    let scene = path.file_stem().and_then(|s| s.to_str()).unwrap_or("h-contact").trim_end_matches(".rec").to_string();
    for (x, dz, flat, waits, (sp, _), label, _) in hcontact_lanes(&scene) {
        let mut mine: Vec<(u64, f64)> = first.iter().filter(|(_, p)| p[0] >= x as f64 && p[0] < x as f64 + 1.0 && p[2] >= dz as f64 + 3.0 && p[2] < dz as f64 + 13.0).map(|(n, p)| (*n, p[2])).collect();
        mine.sort_by(|a, b| a.1.total_cmp(&b.1));
        if mine.len() != waits.len() + 1 {
            println!("{label} at {sp}: {} carts found, expected {}", mine.len(), waits.len() + 1);
            continue;
        }
        let ids: Vec<u64> = mine.iter().map(|(n, _)| *n).collect();
        let arr = *ids.last().unwrap_or(&0);
        let ahead = ids[ids.len() - 2];
        let (mut close, mut close_t) = (f64::MAX, 0usize);
        let (mut pair, mut pair_t, mut pair_ab) = (f64::MAX, 0usize, (0u64, 0u64));
        let mut back: Vec<f64> = vec![f64::MIN; waits.len()];
        let mut last = Vec::new();
        for (t, f) in frames.iter().enumerate() {
            let c = carts(f);
            let at: Vec<Option<[f64; 3]>> = ids.iter().map(|n| c.iter().find(|(m, _)| m == n).map(|(_, p)| *p)).collect();
            if let (Some(a), Some(b)) = (at[ids.len() - 1], at[ids.len() - 2]) {
                let d = d3(a, b);
                if d < close {
                    (close, close_t) = (d, t);
                }
            }
            for i in 0..ids.len() {
                for j in i + 1..ids.len() {
                    if let (Some(a), Some(b)) = (at[i], at[j]) {
                        let d = d3(a, b);
                        if d < pair {
                            (pair, pair_t, pair_ab) = (d, t, (ids[i], ids[j]));
                        }
                    }
                }
            }
            for (k, p) in at.iter().take(waits.len()).enumerate() {
                if let Some(p) = p {
                    back[k] = back[k].max(p[2] - dz as f64);
                }
            }
            last = at;
        }
        let top = (7 + flat) as f64;
        let backs: Vec<String> = back.iter().zip(&waits).map(|(m, (z0, _))| format!("{:+.3}{}", m - z0, if *m >= top { " (into T's cell)" } else { "" })).collect();
        let into_t = back.iter().any(|m| *m >= top);
        let rest = match (last.last().copied().flatten(), last[last.len() - 2]) {
            (Some(a), Some(b)) => format!("{:.3}", d3(a, b)),
            _ => "?".to_string(),
        };
        // (the front cart's distance from its place against the gate at the end)
        let front_off = last[0].map(|p| p[2] - dz as f64 - 4.115).unwrap_or(f64::NAN);
        println!(
            "{label} at {sp}: arrival #{arr} closest {close:.3} to #{ahead} at t{close_t}{}; any pair {pair:.3} (#{} #{} t{pair_t}){}; waiting carts back {}; arrival rests {rest} behind; front ends {front_off:+.3} from the gate",
            if close < 0.05 { " MERGED" } else { "" },
            pair_ab.0,
            pair_ab.1,
            if pair < 0.05 && close >= 0.05 { " PAIR MERGED" } else { "" },
            backs.join(", ")
        );
        let s = summary.entry(label.clone()).or_insert_with(|| (Vec::new(), Vec::new(), Vec::new(), f64::MAX, 0.0f64));
        if close < 0.05 {
            s.0.push(sp);
        } else if pair < 0.05 {
            s.1.push(sp);
        }
        if into_t {
            s.2.push(sp);
        }
        if close >= 0.05 {
            s.3 = s.3.min(close);
        }
        s.4 = s.4.max(front_off.abs());
        let n = lanes_at.entry((label.clone(), format!("{sp}"))).or_insert((0u32, 0u32));
        n.0 += 1;
        if close < 0.05 || pair < 0.05 || into_t {
            n.1 += 1;
        }
    }
    println!("\nsummary (per lane kind: the arrival speeds that merge with the cart ahead, that merge two other carts, that put a cart into T's cell; the closest approach without a merge; the front's largest end offset from the gate; then per speed the lanes with a merge or a cart into T's cell, of the lanes run):");
    for (label, (m, pm, t, c, f)) in &summary {
        let fmt = |v: &Vec<f64>| {
            if v.is_empty() {
                return "none".to_string();
            }
            let mut u: Vec<String> = Vec::new();
            for x in v {
                let s = format!("{x}");
                if !u.contains(&s) {
                    u.push(s);
                }
            }
            u.join(" ")
        };
        let per: Vec<String> = lanes_at.iter().filter(|((l, _), _)| l == label).map(|((_, s), (k, bad))| format!("{s}: {bad}/{k}")).collect();
        let clean = lanes_at.iter().filter(|((l, _), (_, bad))| l == label && *bad == 0).map(|((_, s), _)| s.parse::<f64>().unwrap_or(0.0)).fold(f64::NAN, f64::max);
        println!(
            "  {label}: merges at {}; other pair merges at {}; into T's cell at {}; closest without a merge {}; front off the gate up to {f:.3}; per speed {}; highest clean speed {}",
            fmt(m),
            fmt(pm),
            fmt(t),
            if *c == f64::MAX { "-".to_string() } else { format!("{c:.3}") },
            per.join(", "),
            if clean.is_nan() { "none".to_string() } else { format!("{clean}") }
        );
    }
    Ok(())
}

/// (tickexact 2026-11-11, ironspec's raw-placement audit) The cells of scene `s` whose state one
/// update pass would change: the scene is placed raw (no neighbour updates: a powered rail on its
/// redstone block stays unpowered), so a block whose written state its neighbours contradict sets
/// up a configuration the farm never has, in the game and in redsim alike, and 0 differences cannot
/// see it. Each placed cell and its six neighbours, the raw world against a copy given the judge's
/// updates (`place_updates`: each placed block's neighbour updates, in placement order).
pub fn scene_raw_conflicts(s: &Scene) -> Result<Vec<(BlockPos, String, String)>> {
    let lo = BlockPos::new(s.chunks[0] * 16, -64, s.chunks[1] * 16);
    let size = [(s.chunks[2] - s.chunks[0] + 1) * 16, 384, (s.chunks[3] - s.chunks[1] + 1) * 16];
    let mut w = World::new(lo, size);
    let mut list = Vec::new();
    for (p, st) in &scene_placed_blocks(s) {
        list.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(st).or_else(|_| redsim::terrain::land::block_of(st)).map_err(|e| anyhow!("{st}: {e}"))?));
    }
    w.place_raw(&list);
    let mut u = w.clone();
    u.place_updates(&list);
    let mut cells: Vec<BlockPos> = Vec::new();
    for (p, _) in &list {
        for d in [(0, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
            let q = BlockPos::new(p.x + d.0, p.y + d.1, p.z + d.2);
            if !cells.contains(&q) {
                cells.push(q);
            }
        }
    }
    Ok(cells.into_iter().filter(|q| w.get(*q) != u.get(*q)).map(|q| (q, w.get(q).to_string(), u.get(q).to_string())).collect())
}

/// `mctas mobai flee-control [scene]` (ironspec Nov 11 11:13): `mobfarm flee-check`'s 6-8 metric on
/// a mob AI scene, default flee-distance (tickexact's lanes: a rider or a standing villager and a
/// zombie in a boat at 5.80, 5.95, 6.05, 6.20, 7.80, 7.95, 8.05, and a rider at 7.0 behind a wall;
/// recorded in the game: flee targets at 5.80 and 5.95, panic with none at 6.05..7.95, no panic at
/// 8.05 or behind the wall). Each villager's distance as flee-check measures it (the zombie seated
/// in its boat, the rider seated in its cart, the standing villager on its pillar) against the
/// scene's, and its class (flee: closer than 6; panic: within 8 and seen; none) against the game's.
/// FAIL on any distance off by more than 1e-6 or any class that differs.
pub fn flee_control(name: &str) -> Result<()> {
    let sc = scenes().into_iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}"))?;
    anyhow::ensure!(name == "flee-distance", "the control's expected distances and classes are flee-distance's");
    let mut blocks = Vec::new();
    for (p, s) in &sc.blocks {
        blocks.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(s).map_err(|e| anyhow!("{s}: {e}"))?));
    }
    let (mut lo, mut hi) = (BlockPos::new(i32::MAX, i32::MAX, i32::MAX), BlockPos::new(i32::MIN, i32::MIN, i32::MIN));
    for (p, _) in &blocks {
        lo = BlockPos::new(lo.x.min(p.x - 2), lo.y.min(p.y - 2), lo.z.min(p.z - 2));
        hi = BlockPos::new(hi.x.max(p.x + 2), hi.y.max(p.y + 10), hi.z.max(p.z + 2));
    }
    let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: sc.mobs.clone(), output: BlockPos::new(lo.x, hi.y, lo.z), lo, hi, block_entities: Vec::new(), rock: false, nether: None, land: false };
    // (the standing villagers' cells, tried on foot)
    let foot: Vec<BlockPos> = sc.mobs.iter().filter(|(t, _, _)| t == "minecraft:villager").map(|(_, p, _)| BlockPos::new(p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32)).collect();
    let places = mobfarm::fleecheck::check(&spec, None, &foot, &[]).map_err(|e| anyhow!(e))?;
    // (the lanes in the scene's order: each distance ridden then standing, then the walled rider)
    let ds = [5.80, 5.95, 6.05, 6.20, 7.80, 7.95, 8.05];
    let mut lanes: Vec<(f64, bool, bool)> = ds.iter().flat_map(|d| [(*d, true, false), (*d, false, false)]).collect();
    lanes.push((7.0, true, true));
    let villagers: Vec<&(String, [f64; 3], Option<String>)> = sc.mobs.iter().filter(|(t, _, _)| t == "minecraft:minecart" || t == "minecraft:villager").collect();
    anyhow::ensure!(villagers.len() == lanes.len(), "{} villagers for {} lanes", villagers.len(), lanes.len());
    let mut bad = 0;
    for ((d, riding, walled), (_, p, _)) in lanes.iter().zip(&villagers) {
        let cell = BlockPos::new(p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
        let Some(q) = places.iter().find(|q| q.cell == cell && q.on_foot == !riding) else {
            println!("  {d:.2} {}: no flee-check place at {},{},{}", if *riding { "rider" } else { "standing" }, cell.x, cell.y, cell.z);
            bad += 1;
            continue;
        };
        // (flee-check's least over a rider's three points is the scene's cart at the cell's centre:
        // the other two points lie off the line to the zombie)
        let got = q.near.map_or(f64::NAN, |n| n.0);
        let game = if *d < 6.0 { "flee" } else if *d <= 8.0 && !walled { "panic" } else { "none" };
        let class = if q.flee_from > 0 { "flee" } else if q.seen > 0 { "panic" } else { "none" };
        let ok = (got - d).abs() <= 1e-6 && class == game;
        bad += !ok as usize;
        println!(
            "  {d:.2} {}{}: flee-check {got:.6} ({}), the game's {game}{}",
            if *riding { "rider" } else { "standing" },
            if *walled { " behind the wall" } else { "" },
            class,
            if ok { "" } else { "  DIFFERS" }
        );
    }
    println!("flee-control on {name}: {} of {} lanes as recorded{}", lanes.len() - bad, lanes.len(), if bad == 0 { " (PASS)" } else { "" });
    // (the reachability controls: flee-check's paths are the villager's own navigation, 48 long,
    // the search stopping after 767 nodes; ironspec Nov 11 11:48)
    let rc = mobfarm::fleecheck::reach_control().map_err(|e| anyhow!(e))?;
    for (what, reached, nodes, ok) in &rc {
        println!("  reach {what}: {} ({nodes} nodes){}", if *reached { "reachable" } else { "unreachable" }, if *ok { "" } else { "  DIFFERS" });
    }
    let rbad = rc.iter().filter(|r| !r.3).count();
    println!("reach-control: {} of {} as expected{}", rc.len() - rbad, rc.len(), if rbad == 0 { " (PASS)" } else { "" });
    // (the eye check's positive control: a stone at a seated rider's eye must fail it)
    let (n, hits) = mobfarm::fleecheck::eye_check(&spec, true).map_err(|e| anyhow!(e))?;
    let (_, eye_fails) = mobfarm::fleecheck::eye_lines(n, &hits, 0);
    println!("eye-control (a stone at the first rail cell's rider eye): the suffocation gate {}", if eye_fails { "FAILS, as it must (PASS)" } else { "passes  DIFFERS" });
    // (the blindness pair, ironspec Nov 11 14:08: the eye point inside a shape is blind; an eye
    // box only grazing a shape, the point outside, is sighted)
    let bc = mobfarm::fleecheck::blind_control().map_err(|e| anyhow!(e))?;
    for (what, blind, ok) in &bc {
        println!("  blind {what}: {}{}", if *blind { "blind" } else { "sighted" }, if *ok { "" } else { "  DIFFERS" });
    }
    let bbad = bc.iter().filter(|b| !b.2).count();
    println!("blind-control: {} of {} as expected{}", bc.len() - bbad, bc.len(), if bbad == 0 { " (PASS)" } else { "" });
    // (the pickup gate's positive control: a boated zombie 0.1 beside a cart's box must fail it)
    let (_, pk) = mobfarm::fleecheck::pickup_check(&spec, true);
    println!("pickup-control (a boated zombie 0.1 beside the first rail cell's cart): the pickup gate {}", if pk { "FAILS, as it must (PASS)" } else { "passes  DIFFERS" });
    let bad = bad + rbad + (!eye_fails) as usize + bbad + (!pk) as usize;
    if bad > 0 {
        bail!("flee-control: {bad} lanes differ");
    }
    Ok(())
}

/// `mctas mobai scene-lint [scenes...]`: [`scene_raw_conflicts`] of each scene (default: every
/// scene placed raw); FAIL when any scene has one. First its positive control: case p of r1-sweep
/// with P written unpowered on its redstone block (the configuration 3be9617b3 measured) must fail.
pub fn scene_lint(names: &[String]) -> Result<()> {
    let control = r1_sweep_scene("r1-sweep-p-ee-raw-p", R1_CONTACT_P, [0.5, 2.1775, 31.885], [0.5, 4.5625, 29.5], false, false, true);
    let c = scene_raw_conflicts(&control)?;
    let p_hits = c.iter().filter(|(_, a, _)| a.contains("powered_rail")).count();
    if p_hits == 0 {
        bail!("positive control: the raw-placed p scene passes the lint");
    }
    println!("positive control: the raw-placed p scene fails, {p_hits} unpowered P rails (e.g. {})", c.iter().find(|(_, a, _)| a.contains("powered_rail")).map(|(p, a, b)| format!("{},{},{}: {a} -> {b}", p.x, p.y, p.z)).unwrap_or_default());
    let all = scenes();
    let pick: Vec<&Scene> = if names.is_empty() { all.iter().filter(|s| scene_placement(&s.name) == Placement::Raw).collect() } else { all.iter().filter(|s| names.iter().any(|n| *n == s.name)).collect() };
    let mut bad = 0;
    for s in pick {
        let c = scene_raw_conflicts(s)?;
        if c.is_empty() {
            println!("ok {}", s.name);
        } else {
            bad += 1;
            println!("FAIL {}: {} cells one update pass changes", s.name, c.len());
            for (p, a, b) in c.iter().take(6) {
                println!("  {},{},{}: {a} -> {b}", p.x, p.y, p.z);
            }
        }
    }
    if bad > 0 {
        bail!("{bad} scenes with raw states their neighbours contradict");
    }
    Ok(())
}

pub fn replay(paths: &[std::path::PathBuf]) -> Result<()> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            files.extend(mctas::recorded::recordings(p)?.into_iter().filter(|f| f.extension().is_some_and(|e| e == "rec")));
        } else {
            files.push(p.clone());
        }
    }
    let all = scenes();
    let mut failed = Vec::new();
    for f in &files {
        let t0 = std::time::Instant::now();
        let text = mctas::recorded::read_text(f)?;
        let mut lines = text.lines();
        let header: serde_json::Value = serde_json::from_str(lines.next().unwrap_or("")).map_err(|e| anyhow!("{}: header: {e}", f.display()))?;
        if header["kind"] != "mobai-fixture" {
            continue;
        }
        let name = header["scene"].as_str().unwrap_or("");
        let Some(s) = all.iter().find(|s| s.name == name) else {
            failed.push(format!("{}: no scene {name} any more", f.display()));
            continue;
        };
        if header["scene_hash"].as_str() != Some(s.hash().as_str()) {
            failed.push(format!("{}: STALE: the scene {name} changed since it was recorded (record it again: `{}`)", f.display(), header["how"].as_str().unwrap_or("mctas mobai record")));
            continue;
        }
        let list = |k: &str| -> Vec<[i32; 2]> { header[k].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default() };
        let frames: std::collections::VecDeque<serde_json::Value> = lines.map(serde_json::from_str).collect::<std::result::Result<_, _>>().map_err(|e| anyhow!("{}: a frame: {e}", f.display()))?;
        let ticks = frames.len() as u32;
        let mut src = Source::Recorded {
            frames,
            game_ids: header["game_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default(),
            ticking: list("ticking"),
            spawning: list("spawning"),
            poi_hashes: header_poi_hashes(&header),
        };
        let seed = header["seed"].as_i64().unwrap_or(1);
        let c = compare_source(&mut src, s, seed, ticks, 100, false, false, None)?;
        match &c.first {
            None => println!("PASS {name}: {ticks} ticks, 0 differences ({:.1} s)", t0.elapsed().as_secs_f64()),
            Some(_) => {
                print!("{}", c.report());
                failed.push(format!("{}: differs from the game", f.display()));
            }
        }
    }
    for l in &failed {
        println!("FAIL {l}");
    }
    let n = files.len();
    anyhow::ensure!(failed.is_empty(), "{} of {n} mob AI recordings failed", failed.len());
    println!("REPLAY: {n}/{n} mob AI recordings exact");
    Ok(())
}
