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
    if name == "plant-supports" {
        return plant_supports().1;
    }
    match name {
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

/// The next scripted action at or after `done` ticks: (its tick, its blocks).
fn next_action(actions: &[(u32, Vec<([i32; 3], String)>)], done: i64) -> Option<&(u32, Vec<([i32; 3], String)>)> {
    actions.iter().find(|(t, _)| *t as i64 >= done)
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
            redsim::entity::mob::drive::apply(w, &ctx.lines, ctx.g0, &act);
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
    // (tickexact 2026-11-11, 58f770cc's plate: main's faults/carousel-siding-wired-plate.txt, a
    // stone pressure plate (1,2,1) on the glass ceiling over the booster rail (1,0,1), and its three
    // drives (`siding-bench --wired --launch --nights 1 --drive-out`): a normal night (seed 1), a
    // day (seed 1), the whole part away 3 gt at visit 3's kick (seed 4))
    ("siding-plate-normal", include_str!("../scenes/siding-plate.txt"), include_str!("../scenes/siding-plate-normal-seed1.drive")),
    ("siding-plate-day", include_str!("../scenes/siding-plate.txt"), include_str!("../scenes/siding-plate-day-seed1.drive")),
    ("siding-plate-kick3", include_str!("../scenes/siding-plate.txt"), include_str!("../scenes/siding-plate-whole-kick-3-seed4.drive")),
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
    if name == "carousel-wired" || name == "gate-escg6l" || name == "gate-escg6l-v3" || scene_bench_start(name) { redsim::entity::mob::farm::SETTLE_TICKS as u32 } else { 0 }
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
        "carousel-wired" | "carousel-wired-pocket" | "siding-fault-kick3" | "siding-fault-asleep700" | "siding-fault-nap40" | "gate-escg6l" | "gate-escg6l-v3" | "siding-plate-normal" | "siding-plate-day" | "siding-plate-kick3" => Placement::Judge,
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
    name == "carousel-wired-pocket" || name.starts_with("siding-fault-") || name.starts_with("siding-plate-") || moved(name).is_some_and(|(base, ..)| scene_bench_start(base))
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
    for (name, zombie) in [("freeze-cantreach", false), ("freeze-cantreach-5000", false), ("freeze-cantreach-zombie", true), ("freeze-cantreach-zombie-5000", true)] {
        let mut b = Vec::new();
        floor(&mut b, 0, 32, 0, 16, 63, stone);
        for (cart_x, bed_x) in [(5, 10), (21, 26)] {
            b.push(([cart_x, 64, 5], "minecraft:rail[shape=east_west,waterlogged=false]".to_string()));
            for x in bed_x - 1..=bed_x + 1 {
                for z in 9..=12 {
                    for y in 64..=65 {
                        if !(x == bed_x && (z == 10 || z == 11) && y == 64) {
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
        let dust = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
        b.push(([8, 64, 4], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]".to_string()));
        b.push(([9, 64, 4], dust.to_string()));
        b.push(([9, 64, 5], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]".to_string()));
        b.push(([9, 64, 6], dust.to_string()));
        b.push(([8, 64, 6], "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]".to_string()));
        b.push(([7, 64, 6], dust.to_string()));
        b.push(([7, 64, 5], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]".to_string()));
        b.push(([7, 64, 4], dust.to_string()));
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
        Ok(GameScene { w, sb, lo: s.watch.0, hi: s.watch.1, ticking, spawning, nether, renumbered: Default::default(), last_load: String::new() })
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
    Recorded { frames: std::collections::VecDeque<serde_json::Value>, game_ids: Vec<u64>, ticking: Vec<[i32; 2]>, spawning: Vec<[i32; 2]> },
}

impl Source {
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
            // (tickexact probe: MOBAI_SHOW_RIDERS="T": at tick T, each of the game's villagers: its
            // position and what it rides)
            if let Ok(v) = std::env::var("MOBAI_SHOW_RIDERS")
                && v.split(',').filter_map(|x| x.parse::<i64>().ok()).any(|x| x == f.t)
            {
                for e in f.entities.iter().filter(|e| e.ty.ends_with("villager")) {
                    eprintln!("t {} #{} villager at {:.2} {:.2} {:.2} riding {:?} activities {:?} home {:?}", f.t, e.n, e.pos[0], e.pos[1], e.pos[2], e.vehicle, e.activities, e.memories.as_ref().and_then(|m| m.get("home").cloned()));
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
            game.apply(b, &ActCtx::default())?;
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
