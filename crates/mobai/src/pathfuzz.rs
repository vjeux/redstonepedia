//! The path probe (brief `designs/briefs/mob-ai.md`, Validation): the game's
//! `PathNavigation.createPath` against redsim's on random block worlds, node by node. No
//! randomness is involved on either side, so every difference is a model error.

use anyhow::{Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, WorkerConfig};
use redsim::block::Block;
use redsim::entity::Vec3;
use redsim::entity::mob::ai::path::node::Arena;
use redsim::entity::mob::ai::path::{EvalFlags, Malus, NavKind, NavMob, Navigation};
use redsim::entity::mob::types::MobType;
use redsim::pos::BlockPos;
use redsim::rng::LegacyRandom;
use redsim::world::World;
use serde_json::{Value, json};

/// A mob kind the probe runs, with what its constructor sets on its navigation.
#[derive(Clone, Copy, Debug)]
pub struct ProbeMob {
    pub id: &'static str,
    pub kind: NavKind,
    pub flags: EvalFlags,
    pub required: Option<f32>,
    pub malus: fn() -> Malus,
}

/// The kinds (`registerGoals` and constructors: villagers open doors, float and need 48 blocks;
/// creepers and spiders float (`FloatGoal`); spiders climb (`WallClimberNavigation`)).
pub fn probe_mobs() -> Vec<ProbeMob> {
    let base = EvalFlags::default();
    vec![
        ProbeMob { id: "zombie", kind: NavKind::Ground, flags: base, required: None, malus: Malus::default },
        ProbeMob { id: "villager", kind: NavKind::Ground, flags: EvalFlags { can_open_doors: true, can_float: true, ..base }, required: Some(48.0), malus: Malus::villager },
        ProbeMob { id: "iron_golem", kind: NavKind::Ground, flags: base, required: None, malus: Malus::default },
        ProbeMob { id: "creeper", kind: NavKind::Ground, flags: EvalFlags { can_float: true, ..base }, required: None, malus: Malus::default },
        ProbeMob { id: "spider", kind: NavKind::WallClimber, flags: EvalFlags { can_float: true, ..base }, required: None, malus: Malus::default },
        ProbeMob { id: "skeleton", kind: NavKind::Ground, flags: base, required: None, malus: Malus::default },
    ]
}

/// One probe query's answer: the nodes (x, y, z, type), reached, target; `None` = no path.
#[derive(Clone, PartialEq, Debug)]
pub struct Answer {
    pub nodes: Option<Vec<(i32, i32, i32, String)>>,
    pub reached: bool,
    pub target: Option<[i32; 3]>,
}

impl Answer {
    fn short(&self) -> String {
        match &self.nodes {
            None => "no path".into(),
            Some(n) => {
                let list: Vec<String> = n.iter().map(|(x, y, z, t)| format!("{x},{y},{z}:{t}")).collect();
                format!("reached {} target {:?} [{}]", self.reached, self.target, list.join(" "))
            }
        }
    }
}

/// redsim's answer to the same query.
pub fn redsim_path(w: &World, m: &ProbeMob, pos: Vec3, on_ground: bool, target: BlockPos, reach: i32) -> Answer {
    let ty = MobType::from_id(m.id).expect("probe mob types are modeled");
    let d = ty.def();
    let bb = redsim::entity::make_bounding_box(pos, d.width, d.height);
    let in_water = water_height_positive(w, ty, pos);
    let nm = NavMob {
        pos,
        bb,
        width: d.width,
        height: d.height,
        max_up_step: d.step,
        on_ground,
        in_liquid: in_water,
        in_floatable: in_water,
        is_passenger: false,
        speed: 0.0,
        malus: (m.malus)(),
        max_fall: 3,
        follow_range: d.follow_range,
        game_time: 0,
    };
    let mut nav = Navigation::new(m.kind, d.follow_range);
    nav.flags = m.flags;
    if let Some(r) = m.required {
        nav.set_required_path_length(r, &nm);
    }
    let mut arena = Arena::default();
    match nav.create_path_pos(w, &nm, target, reach, &mut arena) {
        None => Answer { nodes: None, reached: false, target: None },
        Some(p) => Answer { nodes: Some(p.nodes.iter().map(|n| (n.x, n.y, n.z, n.ty.name().to_string())).collect()), reached: p.reached, target: Some([p.target.x, p.target.y, p.target.z]) },
    }
}

/// `isInWater()` as the fluid trackers see the mob (redsim's `EntityFluidInteraction.update` on a
/// throwaway entity at the position).
fn water_height_positive(w: &World, ty: MobType, pos: Vec3) -> bool {
    let st = redsim::entity::mob::MobState::new(ty, 0.0);
    let mut e = redsim::entity::Entity::new(0, redsim::entity::EntityKind::Mob(Box::new(st)), pos, 0.0);
    redsim::entity::fluid_push::update(w, &mut e, true);
    e.fluid.in_water()
}

fn parse_answer(v: &Value) -> Result<Answer> {
    if v["nodes"].is_null() {
        return Ok(Answer { nodes: None, reached: false, target: None });
    }
    let mut nodes = Vec::new();
    for n in v["nodes"].as_array().ok_or_else(|| anyhow!("path: no nodes in {v}"))? {
        let a = n.as_array().ok_or_else(|| anyhow!("path: bad node {n}"))?;
        nodes.push((a[0].as_i64().unwrap_or(0) as i32, a[1].as_i64().unwrap_or(0) as i32, a[2].as_i64().unwrap_or(0) as i32, a[3].as_str().unwrap_or("?").to_string()));
    }
    let t = &v["target"];
    Ok(Answer { nodes: Some(nodes), reached: v["reached"].as_bool().unwrap_or(false), target: Some([t[0].as_i64().unwrap_or(0) as i32, t[1].as_i64().unwrap_or(0) as i32, t[2].as_i64().unwrap_or(0) as i32]) })
}

/// The fuzzer's block palette: what a farm or a village puts in a mob's way.
fn palette() -> Vec<&'static str> {
    vec![
        "minecraft:stone",
        "minecraft:glass",
        "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]",
        "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]",
        "minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]",
        "minecraft:smooth_stone_slab[type=bottom,waterlogged=false]",
        "minecraft:smooth_stone_slab[type=top,waterlogged=false]",
        "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]",
        "minecraft:oak_trapdoor[facing=east,half=top,open=true,powered=false,waterlogged=false]",
        "minecraft:iron_trapdoor[facing=south,half=top,open=false,powered=false,waterlogged=false]",
        "minecraft:water[level=0]",
        "minecraft:lava[level=0]",
        "minecraft:magma_block",
        "minecraft:honey_block",
        "minecraft:soul_sand",
        "minecraft:cactus[age=0]",
        "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]",
        "minecraft:rail[shape=north_south,waterlogged=false]",
        "minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]",
        "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]",
        "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]",
        "minecraft:scaffolding[bottom=false,distance=0,waterlogged=false]",
        "minecraft:hopper[enabled=true,facing=down]",
        "minecraft:powder_snow",
    ]
}

/// A random scene in the box x, z in [-8, 8), y in [0, 6): a stone floor at y = 0 with steps and
/// pits, scattered palette blocks, a door or two. Returns the blocks (redsim's and the game's
/// strings).
fn scene(r: &mut LegacyRandom, density: f64) -> Result<Vec<(BlockPos, Block)>> {
    let pal = palette();
    let mut out: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |out: &mut Vec<(BlockPos, Block)>, p: BlockPos, s: &str| -> Result<()> {
        let b = Block::parse(s).map_err(|e| anyhow!("{s}: {e}"))?;
        out.retain(|(q, _)| *q != p);
        out.push((p, b));
        Ok(())
    };
    for z in -8..8 {
        for x in -8..8 {
            let h = match r.next_int(10) {
                0 => -1,
                1 | 2 => 1,
                3 => 2,
                _ => 0,
            };
            for y in -2..=h {
                put(&mut out, BlockPos::new(x, y, z), "minecraft:stone")?;
            }
            if r.next_double() < density {
                let s = pal[r.next_int(pal.len() as i32) as usize];
                put(&mut out, BlockPos::new(x, h + 1, z), s)?;
            }
        }
    }
    // doors (both halves)
    for _ in 0..r.next_int(3) {
        let (x, z) = (r.next_int(16) - 8, r.next_int(16) - 8);
        let top = out.iter().filter(|(p, b)| p.x == x && p.z == z && !b.is_air()).map(|(p, _)| p.y).max().unwrap_or(0);
        let facing = ["north", "south", "east", "west"][r.next_int(4) as usize];
        let open = r.next_int(2) == 0;
        let iron = r.next_int(4) == 0;
        let name = if iron { "iron_door" } else { "oak_door" };
        for (half, dy) in [("lower", 1), ("upper", 2)] {
            put(&mut out, BlockPos::new(x, top + dy, z), &format!("minecraft:{name}[facing={facing},half={half},hinge=left,open={open},powered=false]"))?;
        }
    }
    Ok(out)
}

/// `mctas mobai pathfuzz`.
pub fn pathfuzz(version: &str, cases: u32, seed: i64, density: f64, verbose: bool) -> Result<()> {
    let pool = Pool::spawn(1, &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let mut wk = pool.worker(0);
    let sb = wk.create(&SandboxOptions { chunks: [-1, -1, 0, 0], seed: 1 })?;
    let mobs = probe_mobs();
    let mut r = LegacyRandom::new(seed);
    let (mut same, mut diff, mut none_both) = (0u32, 0u32, 0u32);
    for case in 0..cases {
        let blocks = scene(&mut r, density)?;
        wk.clear(sb)?;
        let strings: Vec<(redsim::pos::BlockPos, String)> = blocks.iter().map(|(p, b)| (*p, b.to_string())).collect();
        let bp: Vec<([i32; 3], String)> = strings.iter().map(|(p, s)| ([p.x, p.y, p.z], s.clone())).collect();
        wk.set_blocks(sb, &bp, PlaceMode::Raw)?;
        let mut w = World::new(BlockPos::new(-16, -8, -16), [32, 24, 32]);
        w.place_raw(&blocks);
        for _ in 0..8 {
            let m = mobs[r.next_int(mobs.len() as i32) as usize];
            // a start: the top of a random column (standing on it), and a random target cell
            let (sx, sz) = (r.next_int(14) - 7, r.next_int(14) - 7);
            let top = blocks.iter().filter(|(p, b)| p.x == sx && p.z == sz && redsim::behavior::collides(b)).map(|(p, _)| p.y).max().unwrap_or(-2);
            let floor = top as f64 + 1.0;
            let start = Vec3::new(sx as f64 + 0.25 + r.next_double() * 0.5, floor, sz as f64 + 0.25 + r.next_double() * 0.5);
            let target = BlockPos::new(r.next_int(16) - 8, r.next_int(5), r.next_int(16) - 8);
            let reach = r.next_int(2);
            let on_ground = r.next_int(8) != 0;
            let req = json!({"op": "path", "sandbox": sb, "mob": format!("minecraft:{}", m.id), "pos": [start.x, start.y, start.z], "target": [target.x, target.y, target.z], "reach": reach, "on_ground": on_ground});
            let v = wk.call(req)?;
            let game = parse_answer(&v)?;
            let ours = redsim_path(&w, &m, start, on_ground, target, reach);
            if game == ours {
                same += 1;
                if game.nodes.is_none() {
                    none_both += 1;
                }
                if verbose {
                    println!("case {case} {} from {:.3},{:.3},{:.3} to {target} reach {reach}: same ({})", m.id, start.x, start.y, start.z, game.short());
                }
            } else {
                diff += 1;
                println!("DIFF case {case} seed {seed} {} from {:?} on_ground {on_ground} to {target} reach {reach}", m.id, start);
                println!("  game:   {}", game.short());
                println!("  redsim: {}", ours.short());
                if diff >= 5 {
                    bail!("pathfuzz: {diff} differences (stopped)");
                }
            }
        }
    }
    println!("pathfuzz: {same} same ({none_both} with no path on both sides), {diff} different, over {} queries", same + diff);
    if diff > 0 {
        bail!("pathfuzz: {diff} differences");
    }
    Ok(())
}
