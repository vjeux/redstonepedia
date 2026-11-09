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

fn difficulty_name(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Peaceful => "peaceful",
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
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
        v.to_string()
    }

    pub fn hash(&self) -> String {
        format!("{:016x}", mctas::recorded::fnv64(self.canonical().as_bytes()))
    }
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
        for x in [28, 31, 34] {
            b.push(([x, 64, 20], "minecraft:red_bed[facing=south,occupied=false,part=foot]".to_string()));
            b.push(([x, 64, 21], "minecraft:red_bed[facing=south,occupied=false,part=head]".to_string()));
        }
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
            "advance_weather": false, "spawn_mobs": false, "spawn_monsters": false,
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
        list.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(st).map_err(|e| anyhow!("{st}: {e}"))?));
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
        // spawn_mobs off: natural spawning only shuffles its chunks (and checks despawning)
        use redsim::entity::mob::natural::Natural;
        let pos = redsim::entity::Vec3::new(p[0], p[1], p[2]);
        ours.mob.natural = Some(Natural { spawning: false, spawn_enemies: false, void_biome: Some("minecraft:plains"), spawn_order: game.spawning(), ..Natural::new(vec![pos], s.difficulty) });
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
    while left > 0 {
        // one tick per traced step inside the state window (the controls between ticks)
        let in_window = state_range.is_some_and(|(a, b)| done + 1 >= a && done < b);
        let n = match state_range {
            _ if in_window => 1,
            Some((a, _)) if done + 1 < a => (left.min(batch) as i64).min(a - 1 - done).max(1) as u32,
            _ => left.min(batch),
        };
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
        left -= n;
        if cmp.first.is_some() && !full {
            break;
        }
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
        while left > 0 {
            let n = left.min(100);
            frames.extend(game.trace_raw(n, false)?);
            left -= n;
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
