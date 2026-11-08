//! The zombie's goals (`Zombie.registerGoals` / `addBehaviourGoals`): turtle eggs
//! (`ZombieAttackTurtleEggGoal` = `RemoveBlockGoal` / `MoveToBlockGoal`), the spear, the attack,
//! the village (`MoveThroughVillageGoal`), the stroll, the looks; its targets; `BreakDoorGoal`
//! (`DoorInteractGoal`) once it can break doors.

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::melee::{MeleeAttack, MeleeKind};
use super::stroll::RandomStroll;
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::{Goal, reduced_tick_delay};
use crate::block::Block;
use crate::entity::mob::ai::{MobAi, ai, difficulty_id};
use crate::entity::mob::mob;
use crate::entity::{Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;

/// `Zombie.registerGoals()` then `addBehaviourGoals()`.
pub fn register(ai: &mut MobAi) {
    ai.goals.add(4, Goal::ZombieAttackTurtleEgg(TurtleEgg::default()));
    ai.goals.add(8, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    ai.goals.add(8, Goal::RandomLookAround(RandomLookAround::default()));
    ai.goals.add(2, Goal::SpearUse);
    ai.goals.add(3, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Zombie, 1.0, false)));
    ai.goals.add(6, Goal::MoveThroughVillage(MoveThroughVillage::new(1.0, true, 4)));
    ai.goals.add(7, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
    ai.targets.add(1, Goal::HurtByTarget(HurtByTarget::new(true)));
    ai.targets.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Player, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Villager, 10, false, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::IronGolem, 10, true, false)));
    ai.targets.add(5, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::BabyTurtle, 10, true, false)));
}

/// `Zombie.setCanBreakDoors(true)`: the navigation opens doors (its path types), and
/// `BreakDoorGoal` joins at priority 1 (last in the list).
pub fn set_can_break_doors(ai: &mut MobAi, v: bool) {
    if ai.can_break_doors == v {
        return;
    }
    ai.can_break_doors = v;
    ai.nav.flags.can_open_doors = v;
    if v {
        ai.goals.add(1, Goal::BreakDoor(BreakDoor::default()));
    } else {
        ai.goals.goals.retain(|g| !matches!(g.goal, Goal::BreakDoor(_)));
        ai.goals.locked = [None; 4];
    }
}

// ----------------------------------------------------------------------- turtle eggs

/// `ZombieAttackTurtleEggGoal` (`RemoveBlockGoal` of turtle eggs, search range 24, vertical 3).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TurtleEgg {
    pub next_start_tick: i32,
}

/// `RemoveBlockGoal.canUse()`: no turtle egg exists in redsim's worlds, so the search fails and
/// waits `reducedTickDelay(200 + nextInt(200))`.
pub fn turtle_can_use(w: &mut World, i: usize, g: &mut TurtleEgg) -> bool {
    if !w.mob.ai.mob_griefing {
        return false;
    }
    if g.next_start_tick > 0 {
        g.next_start_tick -= 1;
        return false;
    }
    if turtle_egg_near(w, i) {
        let why = "a zombie near a turtle egg (ZombieAttackTurtleEggGoal) is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return false;
    }
    let n = w.entities.list[i].random.as_mut().expect("random").next_int(200);
    g.next_start_tick = reduced_tick_delay(200 + n);
    false
}

/// A turtle egg in the search box (24 around, y - 4 .. y + 2): only terrain states can be one.
fn turtle_egg_near(w: &World, i: usize) -> bool {
    let p = w.entities.list[i].block_position();
    for y in p.y - 4..=p.y + 2 {
        for z in p.z - 24..=p.z + 24 {
            for x in p.x - 24..=p.x + 24 {
                if let Block::Terrain(id) = w.get(BlockPos::new(x, y, z))
                    && crate::terrain::blocks::props(id).block_id() == "minecraft:turtle_egg"
                {
                    return true;
                }
            }
        }
    }
    false
}

pub fn turtle_can_continue(_w: &mut World, _i: usize, _g: &mut TurtleEgg) -> bool {
    false
}
pub fn turtle_start(_w: &mut World, _i: usize, _g: &mut TurtleEgg) {}
pub fn turtle_stop(_w: &mut World, _i: usize, _g: &mut TurtleEgg) {}
pub fn turtle_tick(_w: &mut World, _i: usize, _g: &mut TurtleEgg) {}

/// `SpearUseGoal.canUse()`: a target and a spear in the main hand (no mob holds items here).
pub fn spear_can_use(_w: &mut World, _i: usize) -> bool {
    false
}

// ----------------------------------------------------------------------- the village

/// `MoveThroughVillageGoal`.
#[derive(Clone, PartialEq, Debug)]
pub struct MoveThroughVillage {
    pub speed_modifier: f64,
    pub only_at_night: bool,
    pub distance_to_poi: i32,
    pub visited: Vec<BlockPos>,
    pub poi_pos: BlockPos,
    pub path: Option<crate::entity::mob::ai::path::Path>,
}

impl MoveThroughVillage {
    pub fn new(speed: f64, only_at_night: bool, distance: i32) -> MoveThroughVillage {
        MoveThroughVillage { speed_modifier: speed, only_at_night, distance_to_poi: distance, visited: Vec::new(), poi_pos: BlockPos::new(0, 0, 0), path: None }
    }
}

/// `Level.isBrightOutside()`: the sky darkened by less than 4.
pub fn is_bright_outside(w: &World) -> bool {
    crate::daylight::sky_darken(w.day_time()) < 4
}

/// `canUse()`.
pub fn village_can_use(w: &mut World, i: usize, g: &mut MoveThroughVillage) -> bool {
    if g.visited.len() > 15 {
        g.visited.remove(0);
    }
    if g.only_at_night && is_bright_outside(w) {
        return false;
    }
    let pos = w.entities.list[i].block_position();
    if !crate::entity::mob::ai::poi::is_close_to_village(w, pos, 6) {
        return false;
    }
    let why = "a zombie at night near a village (MoveThroughVillageGoal with claimed POIs) is not modeled yet".to_string();
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
    false
}

pub fn village_can_continue(w: &mut World, i: usize, g: &mut MoveThroughVillage) -> bool {
    if ai(w, i).nav.is_done() {
        return false;
    }
    let width = mob(w, i).dims().0 as f64;
    let c = Vec3::new(g.poi_pos.x as f64 + 0.5, g.poi_pos.y as f64 + 0.5, g.poi_pos.z as f64 + 0.5);
    let d = width + g.distance_to_poi as f64;
    !(c.distance_to_sqr(w.entities.list[i].pos) < d * d)
}

pub fn village_start(w: &mut World, i: usize, g: &mut MoveThroughVillage) {
    let (p, s) = (g.path.clone(), g.speed_modifier);
    crate::entity::mob::ai::with_nav(w, i, |w, nm, nav, _| nav.move_to_path(w, nm, p, s));
}

pub fn village_stop(w: &mut World, i: usize, g: &mut MoveThroughVillage) {
    let c = Vec3::new(g.poi_pos.x as f64 + 0.5, g.poi_pos.y as f64 + 0.5, g.poi_pos.z as f64 + 0.5);
    let d = g.distance_to_poi as f64;
    if ai(w, i).nav.is_done() || c.distance_to_sqr(w.entities.list[i].pos) < d * d {
        g.visited.push(g.poi_pos);
    }
}

// ----------------------------------------------------------------------- doors

/// `BreakDoorGoal` (with `DoorInteractGoal`'s fields): Hard only, 240 ticks.
#[derive(Clone, PartialEq, Debug)]
pub struct BreakDoor {
    pub door_pos: BlockPos,
    pub has_door: bool,
    pub passed: bool,
    pub door_open_dir_x: f32,
    pub door_open_dir_z: f32,
    pub break_time: i32,
    pub last_break_progress: i32,
}

impl Default for BreakDoor {
    fn default() -> BreakDoor {
        BreakDoor { door_pos: BlockPos::new(0, 0, 0), has_door: false, passed: false, door_open_dir_x: 0.0, door_open_dir_z: 0.0, break_time: 0, last_break_progress: -1 }
    }
}

/// `getDoorBreakTime()`.
const DOOR_BREAK_TIME: i32 = 240;

/// `DoorBlock.isWoodenDoor(level, pos)`: a door a hand can open (not iron).
pub fn is_wooden_door(w: &World, p: BlockPos) -> bool {
    match w.get(p) {
        Block::Door { iron, .. } => !iron,
        Block::Terrain(id) => {
            let pr = crate::terrain::blocks::props(id);
            pr.classes.iter().any(|c| c == "DoorBlock") && pr.block_id() != "minecraft:iron_door"
        }
        _ => false,
    }
}

/// `DoorInteractGoal.isOpen()`.
fn is_open(w: &World, g: &mut BreakDoor) -> bool {
    if !g.has_door {
        return false;
    }
    match w.get(g.door_pos) {
        Block::Door { open, .. } => open,
        Block::Terrain(id) if crate::terrain::blocks::props(id).classes.iter().any(|c| c == "DoorBlock") => crate::terrain::blocks::props(id).name.contains("open=true"),
        _ => {
            g.has_door = false;
            false
        }
    }
}

/// `DoorInteractGoal.canUse()`.
fn door_interact_can_use(w: &World, i: usize, g: &mut BreakDoor) -> bool {
    let e = &w.entities.list[i];
    if !e.horizontal_collision {
        return false;
    }
    let Some(path) = ai(w, i).nav.path.as_ref().filter(|p| !p.is_done()) else { return false };
    let n = (path.next_node_index + 2).min(path.node_count());
    for k in 0..n {
        let node = path.nodes[k];
        g.door_pos = BlockPos::new(node.x, node.y + 1, node.z);
        let (dx, dz) = (e.pos.x - g.door_pos.x as f64, e.pos.z - g.door_pos.z as f64);
        if !(dx * dx + dz * dz > 2.25) {
            g.has_door = is_wooden_door(w, g.door_pos);
            if g.has_door {
                return true;
            }
        }
    }
    g.door_pos = e.block_position().above();
    g.has_door = is_wooden_door(w, g.door_pos);
    g.has_door
}

pub fn door_can_use(w: &mut World, i: usize, g: &mut BreakDoor) -> bool {
    if !door_interact_can_use(w, i, g) {
        return false;
    }
    w.mob.ai.mob_griefing && difficulty_id(w) == 3 && !is_open(w, g)
}

pub fn door_start(w: &mut World, i: usize, g: &mut BreakDoor) {
    g.passed = false;
    let e = &w.entities.list[i];
    g.door_open_dir_x = (g.door_pos.x as f64 + 0.5 - e.pos.x) as f32;
    g.door_open_dir_z = (g.door_pos.z as f64 + 0.5 - e.pos.z) as f32;
    g.break_time = 0;
}

pub fn door_can_continue(w: &mut World, i: usize, g: &mut BreakDoor) -> bool {
    let c = Vec3::new(g.door_pos.x as f64 + 0.5, g.door_pos.y as f64 + 0.5, g.door_pos.z as f64 + 0.5);
    g.break_time <= DOOR_BREAK_TIME && !is_open(w, g) && c.distance_to_sqr(w.entities.list[i].pos) < 4.0 && difficulty_id(w) == 3
}

pub fn door_stop(_w: &mut World, _i: usize, _g: &mut BreakDoor) {}

/// `BreakDoorGoal.tick()` (and `DoorInteractGoal.tick`'s passed test).
pub fn door_tick(w: &mut World, i: usize, g: &mut BreakDoor) {
    let e = &w.entities.list[i];
    let nx = (g.door_pos.x as f64 + 0.5 - e.pos.x) as f32;
    let nz = (g.door_pos.z as f64 + 0.5 - e.pos.z) as f32;
    if g.door_open_dir_x * nx + g.door_open_dir_z * nz < 0.0 {
        g.passed = true;
    }
    let _ = w.entities.list[i].random.as_mut().expect("random").next_int(20);
    g.break_time += 1;
    let progress = (g.break_time as f32 / DOOR_BREAK_TIME as f32 * 10.0) as i32;
    if progress != g.last_break_progress {
        g.last_break_progress = progress;
    }
    if g.break_time == DOOR_BREAK_TIME && difficulty_id(w) == 3 {
        // Level.removeBlock(doorPos, false): the fluid's legacy block (air), flags 3
        let p = g.door_pos;
        let fluid = crate::fluid::fluid_state(w, p);
        let to = if fluid.is_empty() { Block::Air } else { fluid.create_legacy_block() };
        w.set_block(p, to, crate::world::flags::ALL);
    }
    let _ = mth::floor(0.0);
}
