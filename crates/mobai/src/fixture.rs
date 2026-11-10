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
    match name {
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

/// The next scripted action at or after `done` ticks: (its tick, its blocks).
fn next_action(actions: &[(u32, Vec<([i32; 3], String)>)], done: i64) -> Option<&(u32, Vec<([i32; 3], String)>)> {
    actions.iter().find(|(t, _)| *t as i64 >= done)
}

/// redsim's side of an action: each block set with flags 3.
fn apply_redsim(w: &mut World, blocks: &[([i32; 3], String)]) -> Result<()> {
    for (p, s) in blocks {
        let b = Block::parse(s).or_else(|_| redsim::terrain::land::block_of(s)).map_err(|e| anyhow!("action block {s}: {e}"))?;
        w.set_block(BlockPos::new(p[0], p[1], p[2]), b, redsim::world::flags::ALL);
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
/// frozen drive log). Each design is ironfarm's single-lane carousel loop taken as it is (its
/// blocks and entities, in its own coordinates); its bench's drive log (`mctas mobfarm
/// queue-loop-run <file> ... --drive-out`) plays every control change at its tick: the bench's
/// timers, and the stand's brake the bench timed from the design's real detector rail in its
/// redsim run (the game's detector compared in the watched box). No other redstone drives them.
/// - carousel-catch: explore/ironfarm 7f6c75643 (the turn slot, the deep tail slots, the detector
///   rail), `--cycles 6 --stand-detect 28 --intake-brake 110,116`.
/// - carousel-catch2: explore/ironfarm ab96cd355 (8 release-run boosters, 7 zombies in boats by
///   the loop), `--cycles 6 --release-for 36 --pulse 12 --stand-detect 27 --intake-brake 116,124
///   --stand-offset 6 --tail0-late 2`.
const DRIVE_SCENES: &[(&str, &str, &str)] = &[
    ("carousel-catch", include_str!("../scenes/carousel-catch.txt"), include_str!("../scenes/carousel-catch-28.drive")),
    ("carousel-catch2", include_str!("../scenes/carousel-catch2.txt"), include_str!("../scenes/carousel-catch2-27.drive")),
];

/// A drive scene's design.
fn drive_design(name: &str) -> Option<mctas::design::Design> {
    let (_, d, _) = DRIVE_SCENES.iter().find(|(n, _, _)| *n == name)?;
    Some(mctas::design::Design::parse(d).unwrap_or_else(|e| panic!("the frozen design of {name}: {e}")))
}

/// A drive scene's drive log: the frozen one, or `MOBAI_DRIVE=<file>` (another drive log of the
/// same design: the catch's tick window, `--stand-detect` 26, 27, 29...).
fn drive_text(name: &str) -> Option<String> {
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
    for line in text.lines().filter(|l| l.starts_with("act ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let parsed = (f.len() == 7 && f[2] == "set").then(|| Some((f[1].parse::<u32>().ok()?, [f[3].parse::<i32>().ok()?, f[4].parse::<i32>().ok()?, f[5].parse::<i32>().ok()?]))).flatten();
        let Some((t, p)) = parsed else { panic!("the drive log of {name}: {line:?} is not `act T set x y z <state>`") };
        let c = (p, f[6].to_string());
        match v.last_mut() {
            Some((lt, b)) if *lt == t => b.push(c),
            _ => v.push((t, vec![c])),
        }
    }
    Some(v)
}

/// The drive log's `expect T pos eK x y z` lines (the design's minecart K at tick T, the bench's
/// redsim run): (T, the cart's entity number in the scene, its position). Each entity line is
/// numbered in summon order with its passengers after it (one per `id:"` in its data). The fixture
/// checks the game's frames against them: the scripted replay reproduces the bench's run (a
/// positive control on the drive log's timing).
fn drive_expects(name: &str) -> Vec<(i64, u32, [f64; 3])> {
    let (Some(d), Some(text)) = (drive_design(name), drive_text(name)) else { return Vec::new() };
    let mut carts = Vec::new();
    let mut n = 0u32;
    for e in &d.entities {
        if e.kind == "minecraft:minecart" {
            carts.push(n);
        }
        n += 1 + e.nbt.as_deref().map(|s| s.matches("id:\"").count() as u32).unwrap_or(0);
    }
    text.lines()
        .filter(|l| l.starts_with("expect "))
        .filter_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() != 7 || f[2] != "pos" {
                return None;
            }
            let k: usize = f[3].strip_prefix('e')?.parse().ok()?;
            let p = [f[4].parse::<f64>().ok()?, f[5].parse::<f64>().ok()?, f[6].parse::<f64>().ok()?];
            Some((f[1].parse().ok()?, *carts.get(k)?, p))
        })
        .collect()
}

/// A scene's expected positions (tick, entity number, position): a drive scene's drive log.
fn scene_expects(name: &str) -> Vec<(i64, u32, [f64; 3])> {
    drive_expects(name)
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
/// placement leaves them as the game would after any shape update.
fn connect_fences(blocks: &mut [([i32; 3], String)]) {
    let fences: std::collections::HashSet<[i32; 3]> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:oak_fence")).map(|(p, _)| *p).collect();
    for (p, s) in blocks.iter_mut() {
        if !s.starts_with("minecraft:oak_fence") {
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
    // (tickexact 2026-11-10, ironfarm's stand catch) its loop designs as they are, each driven by
    // its bench's drive log ([`DRIVE_SCENES`], [`drive_actions`]): cart physics, the riders (and
    // carousel-catch2's zombies in boats) and the detector rail under scripted controls (no other
    // redstone in the designs). carousel-catch2 runs on Hard at day time 200, as the bench's world.
    for (name, _, _) in DRIVE_SCENES {
        let d = drive_design(name).expect("a drive scene's design");
        let blocks: Vec<([i32; 3], String)> = d.blocks.iter().map(|(p, st)| (*p, st.clone())).collect();
        let mobs: Vec<(String, [f64; 3], Option<String>)> = d.entities.iter().map(|e| (e.kind.clone(), e.pos, e.nbt.clone())).collect();
        let max_x = blocks.iter().map(|(p, _)| p[0]).chain(mobs.iter().map(|m| m.1[0] as i32)).max().unwrap_or(0);
        let max_z = blocks.iter().map(|(p, _)| p[2]).chain(mobs.iter().map(|m| m.1[2] as i32)).max().unwrap_or(0);
        let min_x = blocks.iter().map(|(p, _)| p[0]).chain(mobs.iter().map(|m| m.1[0].floor() as i32)).min().unwrap_or(0);
        let min_z = blocks.iter().map(|(p, _)| p[2]).chain(mobs.iter().map(|m| m.1[2].floor() as i32)).min().unwrap_or(0);
        // (2 cells of margin to the sandbox's edge walls)
        let c = [(min_x - 2).div_euclid(16), (min_z - 2).div_euclid(16), (max_x + 2).div_euclid(16), (max_z + 2).div_euclid(16)];
        let first = *name == "carousel-catch";
        v.push(Scene {
            name: name.to_string(),
            chunks: c,
            blocks,
            mobs,
            day_time: if first { 1000 } else { 200 },
            difficulty: if first { Difficulty::Easy } else { Difficulty::Hard },
            mob_griefing: true,
            watch: ([c[0] * 16, -6, c[1] * 16], [c[2] * 16 + 15, 6, c[3] * 16 + 15]),
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
            w.set_blocks(sb, &s.blocks, PlaceMode::Raw)?;
        }
        // the block-ticking and spawning chunks right before the first tick (a player's ring
        // widens them; the spawning list is shuffled every tick, spawning or not)
        let (ticking, spawning) = if s.player.is_some() {
            let v = w.call(json!({"op": "chunk_lists", "sandbox": sb}))?;
            let list = |k: &str| -> Vec<[i32; 2]> { v[k].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default() };
            (list("ticking_chunks"), list("spawning_chunks"))
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(GameScene { w, sb, lo: s.watch.0, hi: s.watch.1, ticking, spawning })
    }

    /// Summon the scene's mobs (seeded); their numbers.
    pub fn summon(&mut self, s: &Scene) -> Result<Vec<u64>> {
        let specs: Vec<serde_json::Value> = s.mobs.iter().map(|(ty, p, nbt)| json!({"type": ty, "pos": p, "nbt": nbt})).collect();
        let v = self.w.call(json!({"op": "summon", "sandbox": self.sb, "mobs": true, "entities": specs}))?;
        Ok(v["net_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default())
    }

    pub fn trace(&mut self, ticks: u32, draws: bool) -> Result<Vec<redsim::exact::Frame>> {
        self.trace_raw(ticks, draws)?.iter().map(mobfarm::exact::parse_frame).collect()
    }

    /// The bridge's frames of the next `ticks` ticks as it sends them (what a recording keeps).
    pub fn trace_raw(&mut self, ticks: u32, draws: bool) -> Result<Vec<serde_json::Value>> {
        let v = self.w.call(json!({"op": "trace", "sandbox": self.sb, "ticks": ticks, "draws": draws, "box": {"from": self.lo, "to": self.hi}}))?;
        Ok(v["frames"].as_array().ok_or_else(|| anyhow!("trace: no frames"))?.clone())
    }

    /// A scripted action on the game's side: the blocks set like a player's (flags 3).
    pub fn apply(&mut self, blocks: &[([i32; 3], String)]) -> Result<()> {
        let list: Vec<([i32; 3], &str)> = blocks.iter().map(|(p, s)| (*p, s.as_str())).collect();
        self.w.set_blocks(self.sb, &list, PlaceMode::Update)?;
        Ok(())
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
    for (p, st) in &s.blocks {
        // a block redsim models by kind, else a natural state from the table (a nether portal)
        list.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(st).or_else(|_| redsim::terrain::land::block_of(st)).map_err(|e| anyhow!("{st}: {e}"))?));
    }
    w.place_raw(&list);
    w.mirror_level_random(seed, s.chunks);
    w.random_ticks = Some(redsim::crops::RandomTicks::new(s.random_tick_speed, seed));
    Ok(w)
}

/// Summon the scene's mobs in redsim (no follow-range bonus: a summon has none).
pub fn redsim_summon(w: &mut World, s: &Scene) -> Result<Vec<u64>> {
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
    Recorded { frames: std::collections::VecDeque<serde_json::Value>, game_ids: Vec<u64>, ticking: Vec<[i32; 2]>, spawning: Vec<[i32; 2]> },
}

impl Source {
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
    let game_ids = game.summon(s)?;
    let our_ids = redsim_summon(&mut ours, s)?;
    if game_ids != our_ids {
        bail!("the game's mobs entered as {game_ids:?}, redsim's as {our_ids:?}");
    }
    let mut watch = Watch::new(&ours, BlockPos::new(s.watch.0[0], s.watch.0[1], s.watch.0[2]), BlockPos::new(s.watch.1[0], s.watch.1[1], s.watch.1[2]));
    let mut cmp = Compare { probe_at: trace.map(|t| t.0), ..Default::default() };
    let show = std::env::var("MOBAI_SHOW").is_ok();
    let show_until: Option<i64> = std::env::var("MOBAI_SHOW_UNTIL").ok().and_then(|v| v.parse().ok());
    let mut left = ticks;
    let mut first = true;
    let state_range: Option<(i64, i64)> = std::env::var("MOBAI_STATE").ok().and_then(|v| {
        let (a, b) = v.split_once(',')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    });
    let mut done: i64 = 0;
    let actions = scene_actions(&s.name);
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
                first = false;
            }
            let tracing = trace.filter(|(at, _)| *at == ours.time + 1);
            if let Some((at, ent)) = tracing {
                let addr = match ent {
                    None => ours.level_random.as_ref().map(|r| r as *const _ as usize).unwrap_or(0),
                    Some(id) => ours.entities.list.iter().find(|e| e.id == id).and_then(|e| e.random.as_ref()).map(|r| r as *const _ as usize).unwrap_or(0),
                };
                eprintln!("redsim's draws at game time {at}:");
                redsim::rng::TRACE.store(addr, std::sync::atomic::Ordering::Relaxed);
            }
            ours.tick();
            redsim::rng::TRACE.store(0, std::sync::atomic::Ordering::Relaxed);
            let o = watch.frame(&ours);
            if show && o.t % 100 == 0 {
                for e in &o.entities {
                    eprintln!("t {} #{} {} pos {:?} goals {:?} path {:?} target {:?} no_action {:?}", o.t, e.n, e.ty, e.pos, e.goals, e.path, e.target, e.no_action);
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
                        let (g, e) = (f.entities.iter().find(|e| e.n == n), o.entities.iter().find(|e| e.n == n));
                        eprintln!("t {} #{n} game {:?} {:?} | redsim {:?} {:?}", f.t, g.map(|e| e.pos), g.map(|e| e.motion), e.map(|e| e.pos), e.map(|e| e.motion));
                    }
                }
            }
            if let Some(n) = show_until
                && o.t <= n
            {
                for (g, e) in f.entities.iter().zip(o.entities.iter()) {
                    eprintln!("t {} #{} game: yaw {} head {:?} body {:?} pitch {} goals {:?} path {:?} pos {:?} | redsim: yaw {} head {:?} body {:?} pitch {} goals {:?} path {:?} pos {:?}", o.t, e.n, g.yaw, g.head, g.body, g.pitch, g.goals, g.path, g.pos, e.yaw, e.head, e.body, e.pitch, e.goals, e.path, e.pos);
                    if std::env::var_os("MOBAI_SHOW_MEM").is_some() {
                        eprintln!("    game behaviors {:?} memories {:?}", g.behaviors, g.memories);
                        eprintln!("    redsim behaviors {:?} memories {:?}", e.behaviors, e.memories);
                    }
                }
            }
            for (_, n, p) in expects.iter().filter(|(t, _, _)| *t == f.t) {
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
        if let Some((_, b)) = actions.iter().find(|(t, _)| *t as i64 == done) {
            if let Source::Live(g) = game {
                g.apply(b)?;
            }
            apply_redsim(&mut ours, b)?;
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
        let game_ids = game.summon(s)?;
        let mut frames = Vec::new();
        let mut left = ticks;
        let actions = scene_actions(&s.name);
        let mut done: i64 = 0;
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
                game.apply(b)?;
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
            apply_redsim(&mut ours, b)?;
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
        apply_redsim(&mut w, shown)?;
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
pub fn timeline(version: &str, name: &str, seed: i64, ticks: u32) -> Result<()> {
    let all = scenes();
    let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}"))?;
    let mut game = GameScene::start(version, s, seed, false)?;
    game.summon(s)?;
    let actions = scene_actions(&s.name);
    let mut last: std::collections::BTreeMap<(u64, &'static str), String> = Default::default();
    let mut mem_last: std::collections::BTreeMap<String, String> = Default::default();
    let mut left = ticks;
    let mut done: i64 = 0;
    while left > 0 {
        let mut n = left.min(100);
        if let Some((t, _)) = next_action(&actions, done + 1) {
            n = n.min((*t as i64 - done).max(1) as u32);
        }
        for f in game.trace_raw(n, false)? {
            let t = f["t"].as_i64().unwrap_or(0);
            for (p, b) in f["blocks"].as_array().into_iter().flatten().filter_map(|x| Some((x.get(0)?, x.get(1)?.as_str()?))) {
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
                // the memories a bed's story is told in (the sensors' lists, paths and gaze left out)
                for (m, v) in e["memories"].as_object().into_iter().flatten() {
                    if matches!(m.as_str(), "mobs" | "visible_mobs" | "path" | "walk_target" | "look_target" | "nearest_players" | "nearest_visible_player" | "interactable_doors" | "doors_to_close" | "secondary_job_site" | "nearest_bed") {
                        continue;
                    }
                    let now = v.to_string();
                    let key = format!("{n} {m}");
                    if mem_last.insert(key, now.clone()).as_ref() != Some(&now) {
                        println!("t {t} #{n} {ty} at {at} memory {m}: {now}");
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
            game.apply(b)?;
        }
    }
    Ok(())
}

/// `mctas mobai replay`: every recording replayed on redsim against its frames (no game). A scene
/// changed since it was recorded fails (its hash), as does any difference.
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
