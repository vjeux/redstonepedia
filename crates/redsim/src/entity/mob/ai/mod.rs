//! Mob AI (the mob-AI lane, 2026-11-08; brief `designs/briefs/mob-ai.md`): what makes mobs walk,
//! read in the 26.3 source and modeled tick exact: path finding and navigation (`path/`), the
//! controls (`control.rs`), the goal selectors and goals (`goal/`), targeting (`target.rs`).
//! Off unless the world turns it on ([`super::MobWorld::ai`]), so the mob layer's other runs are
//! unchanged.
//!
//! [`server_ai_step`] is `Mob.serverAiStep` after its `noActionTime++` (which `living.rs` does):
//! the sight cache, the target and goal selectors (every tick or every other tick by
//! `(tickCount + id) % 2`), the navigation, the mob's own step (a villager's brain), then the move,
//! look and jump controls. [`after_ai_step`] is the rest of `LivingEntity.tick` after `aiStep`
//! (`tickHeadTurn` = `BodyRotationControl.clientTick`, the rotation range checks) and `Mob.tick`'s
//! `updateControlFlags` every 5 ticks.

pub mod attack;
pub mod brain;
pub mod control;
pub mod goal;
pub mod path;
pub mod poi;
pub mod target;

use super::types::MobType;
use super::{mob, mob_mut};
use crate::entity::{Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;
use control::{BodyRotationControl, LookControl, MoveControl, Operation};
use goal::GoalSelector;
use path::node::Arena;
use path::{Malus, NavKind, NavMob, Navigation};

/// `Sensing`: the line-of-sight answers of this tick (`seen`, `unseen`), cleared at each AI step.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Sensing {
    pub seen: Vec<u32>,
    pub unseen: Vec<u32>,
}

/// A goal mob's AI state (the `Mob` fields of the AI: navigation, controls, selectors, sensing,
/// target, head and body yaw).
#[derive(Clone, PartialEq, Debug)]
pub struct MobAi {
    pub nav: Navigation,
    pub move_ctl: MoveControl,
    pub look: LookControl,
    /// `JumpControl.jump`.
    pub jump: bool,
    pub body: BodyRotationControl,
    pub goals: GoalSelector,
    pub targets: GoalSelector,
    pub sensing: Sensing,
    /// `Mob.target` (an entity id; `getTarget` = `asValidTarget`).
    pub target: Option<u32>,
    pub y_head_rot: f32,
    pub y_body_rot: f32,
    pub y_head_rot_o: f32,
    pub y_body_rot_o: f32,
    /// `Mob.ambientSoundTime`.
    pub ambient_sound_time: i32,
    /// `Mob.isAggressive()` (`DATA_MOB_FLAGS` bit 4).
    pub aggressive: bool,
    /// `getPathfindingMalus` overrides.
    pub malus: Malus,
    /// The search buffers (reused between searches).
    pub arena: Arena,
    /// `getLastHurtByMob` (id) and its timestamp (`lastHurtByMobTimestamp` = tickCount).
    pub last_hurt_by_mob: Option<u32>,
    pub last_hurt_by_mob_timestamp: i32,
    /// `Zombie.canBreakDoors`.
    pub can_break_doors: bool,
    /// `lastHurtMob` (the last entity this one hit).
    pub last_hurt_mob: Option<u32>,
    /// The attacker of the last damage (`lastDamageSource.getEntity()`).
    pub last_damage_entity: Option<u32>,
    /// `NeutralMob`: the persistent anger target and its end time (-1: none).
    pub anger_target: Option<u32>,
    pub anger_end: i64,
    /// The creeper's fuse: `swell`, `DATA_SWELL_DIR` (-1 at creation) and SwellGoal's target.
    pub swell: i32,
    pub swell_dir: i32,
    pub swell_target: Option<u32>,
}

impl MobAi {
    /// A mob's AI as its constructor leaves it: the navigation (`createNavigation`, its budget from
    /// the FOLLOW_RANGE base value), the controls, `registerGoals`. `y_rot` is its yaw (the head
    /// and body start at it).
    pub fn new(ty: MobType, y_rot: f32) -> MobAi {
        let d = ty.def();
        let kind = if d.id == "spider" || d.id == "cave_spider" { NavKind::WallClimber } else { NavKind::Ground };
        let malus = if ty == MobType::Villager { Malus::villager() } else { Malus::default() };
        let mut nav = Navigation::new(kind, d.follow_range);
        if ty == MobType::Villager {
            // AbstractVillager/Villager's constructor: doors, floating, a 48-block path budget
            // (setRequiredPathLength: floor(max(FOLLOW_RANGE, 48) x 16) nodes)
            nav.flags.can_open_doors = true;
            nav.flags.can_float = true;
            nav.required_path_length = 48.0;
            nav.max_visited_nodes = mth::floor(((d.follow_range as f32).max(48.0) * 16.0f32) as f64);
        }
        let mut ai = MobAi {
            nav,
            move_ctl: MoveControl::default(),
            look: LookControl::default(),
            jump: false,
            body: BodyRotationControl::default(),
            goals: GoalSelector::default(),
            targets: GoalSelector::default(),
            sensing: Sensing::default(),
            target: None,
            y_head_rot: y_rot,
            y_body_rot: y_rot,
            y_head_rot_o: y_rot,
            y_body_rot_o: y_rot,
            ambient_sound_time: 0,
            aggressive: false,
            malus,
            arena: Arena::default(),
            last_hurt_by_mob: None,
            last_hurt_by_mob_timestamp: 0,
            can_break_doors: false,
            last_hurt_mob: None,
            last_damage_entity: None,
            anger_target: None,
            anger_end: -1,
            swell: 0,
            swell_dir: -1,
            swell_target: None,
        };
        goal::register_goals(&mut ai, ty);
        ai
    }
}

/// The AI of entity `i` (it must have one).
pub fn ai(w: &World, i: usize) -> &MobAi {
    mob(w, i).ai.as_deref().expect("a mob with AI")
}

pub fn ai_mut(w: &mut World, i: usize) -> &mut MobAi {
    mob_mut(w, i).ai.as_deref_mut().expect("a mob with AI")
}

/// Whether entity `i` is a mob with AI.
pub fn has_ai(w: &World, i: usize) -> bool {
    matches!(&w.entities.list[i].kind, crate::entity::EntityKind::Mob(m) if m.ai.is_some())
}

/// `getAttributeValue(MOVEMENT_SPEED)`.
pub fn movement_speed(w: &World, i: usize) -> f64 {
    mob(w, i).ty.def().speed
}

/// `getAttributeValue(FOLLOW_RANGE)`.
pub fn follow_range(w: &World, i: usize) -> f64 {
    mob(w, i).follow_range
}

/// `Mob.getMaxFallDistance()` (the creeper's own rule: its health minus 1 with a target).
pub fn max_fall_distance(w: &World, i: usize) -> i32 {
    let m = mob(w, i);
    let a = ai(w, i);
    if target::get_target(w, i).is_none() {
        return 3;
    }
    if m.ty.def().id == "creeper" {
        return mth::floor((m.health - 1.0 + 3.0f32) as f64);
    }
    let max = m.ty.def().max_health;
    let _ = a;
    let mut sacrifice = (m.health - max * 0.33f32) as i32;
    sacrifice -= (3 - difficulty_id(w)) * 4;
    if sacrifice < 0 {
        sacrifice = 0;
    }
    mth::floor((sacrifice as f32 + 3.0f32) as f64)
}

/// `Difficulty.getId()` (peaceful 0 .. hard 3) of the world (the natural spawner's setting, else
/// the AI world's).
pub fn difficulty_id(w: &World) -> i32 {
    use super::natural::Difficulty;
    match w.mob.natural.as_ref().map(|n| n.difficulty).unwrap_or(w.mob.ai.difficulty) {
        Difficulty::Peaceful => 0,
        Difficulty::Easy => 1,
        Difficulty::Normal => 2,
        Difficulty::Hard => 3,
    }
}

/// A player the AI sees (the AFK player of a farm, the harness's fake player): its feet, its
/// game mode. Players are not in the entity list; their ids are [`PLAYER_ID_BASE`] + index.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Player {
    pub pos: Vec3,
    pub mode: GameMode,
    pub invulnerable: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    Survival,
    Creative,
    Spectator,
}

/// The ids players take in targets and sight caches.
pub const PLAYER_ID_BASE: u32 = 0xF000_0000;

/// The AI's level state (`MobWorld.ai`).
#[derive(Clone, Debug)]
pub struct AiWorld {
    /// Mobs added while this holds get their AI (`Mob` constructor: `registerGoals`).
    pub on: bool,
    pub difficulty: super::natural::Difficulty,
    pub players: Vec<Player>,
    /// `GameRules.MOB_GRIEFING`.
    pub mob_griefing: bool,
    /// The attacker of the hit `living::hurt` is dealing (set by [`attack::do_hurt_target`]).
    pub attacker: Option<u32>,
    /// The POI index (`poi::index`; `None`: to rebuild).
    pub pois: Option<Vec<(crate::pos::BlockPos, poi::PoiKind)>>,
    /// The light sources of a design world (`goal::stroll::light_magic`; `None`: to rebuild).
    pub lights: Option<Vec<(crate::pos::BlockPos, i32)>>,
    /// The sky light of a design world's cells asked so far (cleared when a block changes).
    pub sky_cache: std::collections::HashMap<crate::pos::BlockPos, i32>,
}

impl Default for AiWorld {
    fn default() -> AiWorld {
        AiWorld { on: false, difficulty: super::natural::Difficulty::Normal, players: Vec::new(), mob_griefing: true, attacker: None, pois: None, lights: None, sky_cache: std::collections::HashMap::new() }
    }
}

/// What the navigation reads of mob `i` now.
pub fn nav_mob(w: &World, i: usize) -> NavMob {
    let e = &w.entities.list[i];
    let m = mob(w, i);
    let (width, height) = m.dims();
    NavMob {
        pos: e.pos,
        bb: e.bb,
        width,
        height,
        max_up_step: m.ty.def().step,
        on_ground: e.on_ground,
        in_liquid: e.fluid.in_water() || m.lava_height > 0.0,
        in_floatable: e.fluid.in_water(),
        is_passenger: e.vehicle.is_some(),
        speed: m.speed,
        malus: ai(w, i).malus,
        max_fall: max_fall_distance(w, i),
        follow_range: m.follow_range,
        game_time: w.time,
    }
}

/// Run `f` on mob `i`'s navigation with its search buffers (taken out of the mob for the call).
pub fn with_nav<R>(w: &mut World, i: usize, f: impl FnOnce(&World, &NavMob, &mut Navigation, &mut Arena) -> R) -> R {
    let nm = nav_mob(w, i);
    let a = ai_mut(w, i);
    let mut nav = std::mem::replace(&mut a.nav, Navigation::new(NavKind::Ground, 0.0));
    let mut arena = std::mem::take(&mut a.arena);
    let r = f(w, &nm, &mut nav, &mut arena);
    let a = ai_mut(w, i);
    a.nav = nav;
    a.arena = arena;
    r
}

/// `Mob.serverAiStep` after `noActionTime++`.
pub fn server_ai_step(w: &mut World, i: usize) {
    {
        let a = ai_mut(w, i);
        a.sensing.seen.clear();
        a.sensing.unseen.clear();
    }
    let e = &w.entities.list[i];
    let id_based = e.tick_count.wrapping_add(e.network_id);
    let every_other = id_based % 2 != 0 && e.tick_count > 1;
    goal::tick_selector(w, i, true, every_other);
    goal::tick_selector(w, i, false, every_other);
    // navigation.tick()
    let wanted = with_nav(w, i, |w, nm, nav, arena| nav.tick(w, nm, arena));
    if let Some((x, y, z, s)) = wanted {
        ai_mut(w, i).move_ctl.set_wanted_position(x, y, z, s);
    }
    custom_server_ai_step(w, i);
    if w.entities.list[i].removed {
        return;
    }
    move_control_tick(w, i);
    look_control_tick(w, i);
    jump_control_tick(w, i);
}

/// `customServerAiStep`: the villager's brain (and its raid draw); nothing for the goal mobs
/// modeled so far.
fn custom_server_ai_step(w: &mut World, i: usize) {
    if mob(w, i).ty == MobType::Villager {
        super::villager::brain_tick(w, i);
    }
}

/// `MoveControl.tick()`.
pub fn move_control_tick(w: &mut World, i: usize) {
    let op = ai(w, i).move_ctl.operation;
    match op {
        Operation::Strafe => {
            let speed = movement_speed(w, i) as f32;
            let mc = ai(w, i).move_ctl;
            let speed_modified = mc.speed_modifier as f32 * speed;
            let mut xa = mc.strafe_forwards;
            let mut za = mc.strafe_right;
            let mut dist = mth::sqrt(xa * xa + za * za);
            if dist < 1.0 {
                dist = 1.0;
            }
            dist = speed_modified / dist;
            xa *= dist;
            za *= dist;
            let y_rot = w.entities.list[i].y_rot;
            let sin = mth::sin((y_rot * (std::f32::consts::PI / 180.0f32)) as f64);
            let cos = mth::cos((y_rot * (std::f32::consts::PI / 180.0f32)) as f64);
            let dx = xa * cos - za * sin;
            let dz = za * cos + xa * sin;
            if !is_walkable(w, i, dx, dz) {
                let a = ai_mut(w, i);
                a.move_ctl.strafe_forwards = 1.0;
                a.move_ctl.strafe_right = 0.0;
            }
            let (f, r) = (ai(w, i).move_ctl.strafe_forwards, ai(w, i).move_ctl.strafe_right);
            let m = mob_mut(w, i);
            m.speed = speed_modified;
            m.zza = f;
            m.xxa = r;
            ai_mut(w, i).move_ctl.operation = Operation::Wait;
        }
        Operation::MoveTo => {
            ai_mut(w, i).move_ctl.operation = Operation::Wait;
            let mc = ai(w, i).move_ctl;
            let e = &w.entities.list[i];
            let xd = mc.wanted_x - e.pos.x;
            let zd = mc.wanted_z - e.pos.z;
            let yd = mc.wanted_y - e.pos.y;
            let dd = xd * xd + yd * yd + zd * zd;
            if dd < 2.5000003e-7f32 as f64 {
                mob_mut(w, i).zza = 0.0;
                return;
            }
            let y_rot_d = control::atan2_degrees(zd, xd) - 90.0f32;
            let y_rot = control::rotlerp(w.entities.list[i].y_rot, y_rot_d, 90.0);
            w.entities.list[i].y_rot = y_rot;
            let speed = (mc.speed_modifier * movement_speed(w, i)) as f32;
            {
                let m = mob_mut(w, i);
                m.speed = speed;
                m.zza = speed;
            }
            let e = &w.entities.list[i];
            let pos = e.block_position();
            let s = w.get(pos);
            let top = path::walk::local_shape_top(w, &s, pos);
            let step = mob(w, i).ty.def().step as f64;
            let width = mob(w, i).dims().0;
            let jump_up = yd > step && xd * xd + zd * zd < 1.0f32.max(width) as f64;
            let in_block = top.is_some_and(|t| e.pos.y < t + pos.y as f64) && !is_door_or_fence(&s);
            if jump_up || in_block {
                let a = ai_mut(w, i);
                a.jump = true;
                a.move_ctl.operation = Operation::Jumping;
            }
        }
        Operation::Jumping => {
            // Mob.setSpeed: the speed and the forward input
            let speed = (ai(w, i).move_ctl.speed_modifier * movement_speed(w, i)) as f32;
            {
                let m = mob_mut(w, i);
                m.speed = speed;
                m.zza = speed;
            }
            let e = &w.entities.list[i];
            let in_liquid = e.fluid.in_water() || mob(w, i).lava_height > 0.0;
            if e.on_ground || in_liquid {
                ai_mut(w, i).move_ctl.operation = Operation::Wait;
            }
        }
        Operation::Wait => {
            mob_mut(w, i).zza = 0.0;
        }
    }
}

/// `BlockTags.DOORS` or `BlockTags.FENCES` (the jump test skips them).
fn is_door_or_fence(s: &crate::block::Block) -> bool {
    use crate::block::Block;
    match s {
        Block::Door { .. } | Block::Fence { .. } => true,
        Block::Terrain(id) => {
            let p = crate::terrain::blocks::props(*id);
            p.has_tag("minecraft:doors") || p.has_tag("minecraft:fences") || p.classes.iter().any(|c| c == "DoorBlock" || c == "FenceBlock")
        }
        _ => false,
    }
}

/// `MoveControl.isWalkable(dx, dz)`: the static path type at the strafe target is WALKABLE.
fn is_walkable(w: &World, i: usize, dx: f32, dz: f32) -> bool {
    let e = &w.entities.list[i];
    let p = BlockPos::new(mth::floor(e.pos.x + dx as f64), mth::floor(e.pos.y), mth::floor(e.pos.z + dz as f64));
    let arena = &mut Arena::default();
    let eval = path::WalkEval::new(w, nav_mob(w, i).to_path_mob(), ai(w, i).nav.flags, arena);
    eval.path_type_static(p.x, p.y, p.z) == path::PathType::Walkable
}

/// `LookControl.tick()`.
pub fn look_control_tick(w: &mut World, i: usize) {
    // resetXRotOnTick
    w.entities.list[i].x_rot = 0.0;
    let lc = ai(w, i).look;
    if lc.look_at_cooldown > 0 {
        ai_mut(w, i).look.look_at_cooldown -= 1;
        let e = &w.entities.list[i];
        let eye_y = e.pos.y + mob(w, i).eye() as f64;
        let xd = lc.wanted_x - e.pos.x;
        let zd = lc.wanted_z - e.pos.z;
        let yd = lc.wanted_y - eye_y;
        if zd.abs() > 1.0e-5f32 as f64 || xd.abs() > 1.0e-5f32 as f64 {
            let y_rot_d = control::atan2_degrees(zd, xd) - 90.0f32;
            let a = ai_mut(w, i);
            a.y_head_rot = control::rotate_towards(a.y_head_rot, y_rot_d, lc.y_max_rot_speed);
        }
        let sd = (xd * xd + zd * zd).sqrt();
        if yd.abs() > 1.0e-5f32 as f64 || sd.abs() > 1.0e-5f32 as f64 {
            let x_rot_d = -control::atan2_degrees(yd, sd);
            let cur = w.entities.list[i].x_rot;
            set_x_rot(w, i, control::rotate_towards(cur, x_rot_d, lc.x_max_rot_angle));
        }
    } else {
        let a = ai_mut(w, i);
        a.y_head_rot = control::rotate_towards(a.y_head_rot, a.y_body_rot, 10.0);
    }
    // clampHeadRotationToBody
    if !ai(w, i).nav.is_done() {
        let max = max_head_y_rot(w, i) as f32;
        let a = ai_mut(w, i);
        a.y_head_rot = control::rotate_if_necessary(a.y_head_rot, a.y_body_rot, max);
    }
}

/// `Entity.setXRot`: `Math.clamp(xRot % 360, -90, 90)`.
pub fn set_x_rot(w: &mut World, i: usize, x: f32) {
    if x.is_finite() {
        w.entities.list[i].x_rot = (x % 360.0).clamp(-90.0, 90.0);
    }
}

/// `Mob.getMaxHeadYRot()` (75).
pub fn max_head_y_rot(_w: &World, _i: usize) -> i32 {
    75
}

/// `Mob.getMaxHeadXRot()` (40).
pub fn max_head_x_rot(_w: &World, _i: usize) -> i32 {
    40
}

/// `Mob.getHeadRotSpeed()` (10).
pub fn head_rot_speed(_w: &World, _i: usize) -> i32 {
    10
}

/// `JumpControl.tick()`.
pub fn jump_control_tick(w: &mut World, i: usize) {
    let j = std::mem::replace(&mut ai_mut(w, i).jump, false);
    mob_mut(w, i).jumping = j;
}

/// `BodyRotationControl.clientTick()` (`Mob.tickHeadTurn`).
pub fn body_rotation_tick(w: &mut World, i: usize) {
    let e = &w.entities.list[i];
    let xd = e.pos.x - e.xo;
    let zd = e.pos.z - e.zo;
    let moving = xd * xd + zd * zd > 2.5000003e-7f32 as f64;
    let y_rot = e.y_rot;
    let max = max_head_y_rot(w, i) as f32;
    let carrying_mob = e.passengers.first().and_then(|p| crate::entity::index_of(w, *p)).is_some_and(|j| matches!(w.entities.list[j].kind, crate::entity::EntityKind::Mob(_)));
    let a = ai_mut(w, i);
    if moving {
        a.y_body_rot = y_rot;
        a.y_head_rot = control::rotate_if_necessary(a.y_head_rot, a.y_body_rot, max);
        a.body.last_stable_y_head_rot = a.y_head_rot;
        a.body.head_stable_time = 0;
    } else if !carrying_mob {
        if (a.y_head_rot - a.body.last_stable_y_head_rot).abs() > 15.0 {
            a.body.head_stable_time = 0;
            a.body.last_stable_y_head_rot = a.y_head_rot;
            a.y_body_rot = control::rotate_if_necessary(a.y_body_rot, a.y_head_rot, max);
        } else {
            a.body.head_stable_time += 1;
            if a.body.head_stable_time > 10 {
                let since = a.body.head_stable_time - 10;
                let frac = mth::clampf(since as f32 / 10.0, 0.0, 1.0);
                let remaining = max * (1.0 - frac);
                a.y_body_rot = control::rotate_if_necessary(a.y_body_rot, a.y_head_rot, remaining);
            }
        }
    }
}

/// The rest of `LivingEntity.tick` after `aiStep` for a mob with AI (`tickHeadTurn`, the rotation
/// range checks) and `Mob.tick`'s `updateControlFlags` every 5 ticks.
pub fn after_ai_step(w: &mut World, i: usize) {
    // IronGolem.aiStep's tail (after Mob.aiStep): attackAnimationTick/offerFlowerTick (client
    // looks) and the persistent anger update
    if mob(w, i).ty == MobType::IronGolem {
        goal::golem::update_anger(w, i);
    }
    body_rotation_tick(w, i);
    {
        let e = &mut w.entities.list[i];
        while e.y_rot - e.y_rot_o < -180.0 {
            e.y_rot_o -= 360.0;
        }
        while e.y_rot - e.y_rot_o >= 180.0 {
            e.y_rot_o += 360.0;
        }
    }
    {
        let a = ai_mut(w, i);
        while a.y_body_rot - a.y_body_rot_o < -180.0 {
            a.y_body_rot_o -= 360.0;
        }
        while a.y_body_rot - a.y_body_rot_o >= 180.0 {
            a.y_body_rot_o += 360.0;
        }
    }
    {
        let e = &mut w.entities.list[i];
        while e.x_rot - e.x_rot_o < -180.0 {
            e.x_rot_o -= 360.0;
        }
        while e.x_rot - e.x_rot_o >= 180.0 {
            e.x_rot_o += 360.0;
        }
    }
    {
        let a = ai_mut(w, i);
        while a.y_head_rot - a.y_head_rot_o < -180.0 {
            a.y_head_rot_o -= 360.0;
        }
        while a.y_head_rot - a.y_head_rot_o >= 180.0 {
            a.y_head_rot_o += 360.0;
        }
    }
    if mob(w, i).sleeping_pos.is_some() {
        w.entities.list[i].x_rot = 0.0;
    }
    if w.entities.list[i].tick_count % 5 == 0 {
        update_control_flags(w, i);
    }
}

/// `Mob.updateControlFlags()`: no mob controls this one; JUMP is off in a boat.
fn update_control_flags(w: &mut World, i: usize) {
    let in_boat = w.entities.list[i].vehicle.and_then(|v| crate::entity::index_of(w, v)).is_some_and(|vi| matches!(w.entities.list[vi].kind, crate::entity::EntityKind::Boat(_)));
    let a = ai_mut(w, i);
    a.goals.set_control_flag(goal::Flag::Move, true);
    a.goals.set_control_flag(goal::Flag::Jump, !in_boat);
    a.goals.set_control_flag(goal::Flag::Look, true);
}

/// `Entity.baseTick`'s `canSpawnSprintParticle()` (before the fluid update): the iron golem's
/// override draws `nextInt(5)` from its own random whenever it moves sideways; on a hit,
/// `spawnSprintParticle` draws two `nextDouble`s (the particle's x and z) unless the block 0.2
/// below (`getOnPosLegacy`) renders invisible (air, liquids, bubble columns, moving pistons...).
pub fn before_base_tick(w: &mut World, i: usize) {
    if mob(w, i).ty == MobType::IronGolem {
        let d = w.entities.list[i].delta_movement;
        if d.x * d.x + d.z * d.z > 2.5000003e-7f32 as f64 && w.entities.list[i].random.as_mut().expect("random").next_int(5) == 0 {
            let p = crate::entity::collide::get_on_pos(w, &w.entities.list[i], 0.2);
            let invisible = matches!(w.get(p), crate::block::Block::Air | crate::block::Block::Water { .. } | crate::block::Block::Lava { .. } | crate::block::Block::BubbleColumn { .. } | crate::block::Block::MovingPiston { .. });
            if !invisible {
                let r = w.entities.list[i].random.as_mut().expect("random");
                r.next_double();
                r.next_double();
            }
        }
    }
}

/// `IronGolem.doPush(entity)` before the push itself: an enemy (not a creeper) it pushes becomes
/// its target one time in 20 (its own random).
pub fn before_do_push(w: &mut World, i: usize, j: usize) {
    if mob(w, i).ty != MobType::IronGolem {
        return;
    }
    let ty = mob(w, j).ty;
    if !goal::targets::is_enemy(ty) {
        return;
    }
    if w.entities.list[i].random.as_mut().expect("random").next_int(20) == 0 {
        let id = w.entities.list[j].id;
        target::set_target(w, i, Some(id));
    }
}

/// The rest of `LivingEntity.baseTick` (the attacker memory, the old head and body yaw) and
/// `Mob.baseTick`'s ambient sound roll.
pub fn after_base_tick(w: &mut World, i: usize) {
    attack::forget_attacker(w, i);
    {
        let a = ai_mut(w, i);
        a.y_head_rot_o = a.y_head_rot;
        a.y_body_rot_o = a.y_body_rot;
    }
    ambient_sound_tick(w, i);
}

/// `Mob.checkDespawn()` where natural spawning (natural.rs, which owns it there) is off and the
/// mob AI has players: the nearest non-spectator player within 32 resets `noActionTime`; a mob
/// that is not persistent and would despawn (beyond 128, or the 1-in-800 roll past 600 idle
/// ticks beyond 32: that roll is drawn) is refused.
pub fn check_despawn(w: &mut World, i: usize) {
    let m = mob(w, i);
    if m.dead {
        return;
    }
    if difficulty_id(w) == 0 && m.ty.def().monster {
        refuse_once(w, "a monster in a peaceful world (discarded) is not modeled");
        return;
    }
    let persistent = m.persistence_required;
    let no_action = m.no_action_time;
    let removes_far = !matches!(m.ty, MobType::Villager | MobType::IronGolem);
    let p = w.entities.list[i].pos;
    let near = w.mob.ai.players.iter().filter(|q| q.mode != GameMode::Spectator).map(|q| q.pos.distance_to_sqr(p)).fold(None, |a: Option<f64>, d| Some(a.map_or(d, |a| a.min(d))));
    let Some(d2) = near else { return };
    if !persistent && d2 > 128.0 * 128.0 && removes_far {
        refuse_once(w, "a mob despawning (beyond 128 of every player) is not modeled");
        return;
    }
    let roll = !persistent && no_action > 600 && w.entities.list[i].random.as_mut().expect("random").next_int(800) == 0;
    if roll && d2 > 32.0 * 32.0 && removes_far {
        refuse_once(w, "a mob despawning (the 1-in-800 roll) is not modeled");
    } else if d2 < 32.0 * 32.0 {
        mob_mut(w, i).no_action_time = 0;
    }
}

fn refuse_once(w: &mut World, why: &str) {
    if !w.errors.iter().any(|e| e == why) {
        w.errors.push(why.to_string());
    }
}

/// `Monster.updateNoActionTime()` (first in `Monster.aiStep`): bright light at the eye counts
/// double.
pub fn update_no_action_time(w: &mut World, i: usize) {
    if !mob(w, i).ty.def().monster {
        return;
    }
    let e = &w.entities.list[i];
    let p = BlockPos::new(mth::floor(e.pos.x), mth::floor(e.pos.y + mob(w, i).eye() as f64), mth::floor(e.pos.z));
    let br = goal::stroll::light_magic(w, p);
    if br > 0.5 {
        mob_mut(w, i).no_action_time += 2;
    }
}

/// `ServerLevel.sendBlockUpdated` (a block set with flag 2): when the collision shape changed,
/// every navigating mob whose path runs near asks for a new one (`shouldRecomputePath`,
/// `recomputePath`; a search per mob, independent of the order).
pub fn send_block_updated(w: &mut World, pos: BlockPos, old: crate::block::Block, new: crate::block::Block) {
    let a = crate::entity::collide::boxes_at(w, &old, pos);
    let b = crate::entity::collide::boxes_at(w, &new, pos);
    if a == b {
        return;
    }
    let mut todo = Vec::new();
    for i in 0..w.entities.list.len() {
        if w.entities.list[i].removed || !has_ai(w, i) {
            continue;
        }
        let nm = nav_mob(w, i);
        if ai(w, i).nav.should_recompute_path(&nm, pos) {
            todo.push(i);
        }
    }
    for i in todo {
        with_nav(w, i, |w, nm, nav, arena| nav.recompute_path(w, nm, arena));
    }
}

/// `setOldPosAndRot`'s head and body part (`commonTick`): the old head and body yaw.
pub fn common_tick(w: &mut World, i: usize) {
    let a = ai_mut(w, i);
    a.y_head_rot_o = a.y_head_rot;
    a.y_body_rot_o = a.y_body_rot;
}

/// `Mob.baseTick`'s ambient sound roll (after `LivingEntity.baseTick`): `nextInt(1000) <
/// ambientSoundTime++`; when it fires, the time resets to `-getAmbientSoundInterval()` and the
/// sound's pitch (`getVoicePitch`: two `nextFloat`) is drawn when the mob has an ambient sound.
pub fn ambient_sound_tick(w: &mut World, i: usize) {
    if w.entities.list[i].removed || mob(w, i).health <= 0.0 {
        return;
    }
    let t = ai(w, i).ambient_sound_time;
    let roll = match w.entities.list[i].random.as_mut() {
        Some(r) => r.next_int(1000),
        None => return,
    };
    ai_mut(w, i).ambient_sound_time = t + 1;
    if roll < t {
        ai_mut(w, i).ambient_sound_time = -80;
        if has_ambient_sound(w, i)
            && let Some(r) = w.entities.list[i].random.as_mut()
        {
            r.next_float();
            r.next_float();
        }
    }
}

/// `getAmbientSound() != null`: zombies, creepers? (no: `Creeper` has none), skeletons, spiders;
/// a villager unless asleep; an iron golem has none.
fn has_ambient_sound(w: &World, i: usize) -> bool {
    let m = mob(w, i);
    match m.ty {
        MobType::IronGolem => false,
        MobType::Villager => m.sleeping_pos.is_none(),
        MobType::Zombie | MobType::Witch => true,
        MobType::Natural(_) => !matches!(m.ty.def().id, "creeper"),
    }
}

impl NavMob {
    pub fn to_path_mob(&self) -> path::PathMob {
        path::PathMob { pos: self.pos, bb: self.bb, width: self.width, height: self.height, max_up_step: self.max_up_step, on_ground: self.on_ground, in_floatable: self.in_floatable, malus: self.malus, max_fall: self.max_fall }
    }
}

/// The eye position of entity `j` (`getEyeY`).
pub fn eye_pos(w: &World, j: usize) -> Vec3 {
    let e = &w.entities.list[j];
    Vec3::new(e.pos.x, e.pos.y + e.kind.eye_height() as f64, e.pos.z)
}

// ------------------------------------------------------------------ the exact compare's fields

/// The running goals as the bridge prints them (`Exact.entityFrame`): `"<prio>:<Class>"` and
/// `"t<prio>:<Class>"` for the target selector, sorted as Java sorts strings.
pub fn frame_goals(a: &MobAi) -> String {
    let mut v: Vec<String> = a.goals.goals.iter().filter(|g| g.running).map(|g| format!("{}:{}", g.priority, g.goal.name())).collect();
    v.extend(a.targets.goals.iter().filter(|g| g.running).map(|g| format!("t{}:{}", g.priority, g.goal.name())));
    v.sort();
    v.join(" ")
}

/// The path as the bridge prints it: `"path x y z next/count reach|partial"`, or `"none"`.
pub fn frame_path(a: &MobAi) -> String {
    match &a.nav.path {
        None => "none".into(),
        Some(p) => format!("path {} {} {} {}/{} {}", p.target.x, p.target.y, p.target.z, p.next_node_index, p.node_count(), if p.reached { "reach" } else { "partial" }),
    }
}

/// The target as the bridge prints it (`getTarget()`, a valid one): `"#n"` or `"none"`.
pub fn frame_target(w: &World, i: usize) -> String {
    match target::get_target(w, i) {
        Some(id) => frame_ref(id),
        None => "none".into(),
    }
}

/// An entity's reference in the exact frames (the bridge's `Exact.ref`: its number in the
/// sandbox; the AFK player's id is the sandbox's numbering + 2^20 - 4).
pub fn frame_ref(id: u32) -> String {
    if id >= PLAYER_ID_BASE {
        format!("#{}", (1i64 << 20) - 4 - (id - PLAYER_ID_BASE) as i64)
    } else {
        format!("#{id}")
    }
}
