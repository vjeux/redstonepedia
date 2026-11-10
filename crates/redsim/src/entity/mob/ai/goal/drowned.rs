//! The drowned's goals (`Drowned.addBehaviourGoals`, after `Zombie.registerGoals`' turtle eggs and
//! looks): going to water by day, the trident (none modeled: refused when it would start), the
//! attack (`DrownedAttackGoal`: the zombie's, only at a target it may hunt), the beach and the swim
//! up at night (refused when they would start: named in the run's legacy list), the plain stroll;
//! its targets. Its navigation is amphibious (`AmphibiousPathNavigation`).

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::melee::{MeleeAttack, MeleeKind};
use super::stroll::RandomStroll;
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::zombie::TurtleEgg;
use super::{Goal, JUMP, LOOK, MOVE, reduced_tick_delay};
use crate::entity::Vec3;
use crate::entity::mob::ai::{MobAi, ai, with_nav};
use crate::entity::mob::mob;
use crate::pos::BlockPos;
use crate::world::World;

/// The level's sea level: a terrain world's (the overworld's 63, as the bridge's terrain sandboxes
/// report it), else the flat sandbox's (-63) (tickexact 2026-11-10: the swim up and the beach read
/// -63 in terrain worlds).
pub fn sea_level(w: &World) -> i32 {
    if w.land.is_some() { 63 } else { -63 }
}

#[derive(Clone, PartialEq, Debug)]
pub enum DrownedGoal {
    /// `DrownedGoToWaterGoal`: the water cell it heads for.
    GoToWater { wanted: Vec3 },
    /// `DrownedGoToBeachGoal` (`MoveToBlockGoal(8, 2)`): `nextStartTick`, `tryTicks`,
    /// `maxStayTicks`, `blockPos`, `reachedTarget`.
    GoToBeach { next_start_tick: i32, try_ticks: i32, max_stay_ticks: i32, block_pos: BlockPos, reached: bool },
    /// `DrownedSwimUpGoal`: `stuck`.
    SwimUp { stuck: bool },
    /// `DrownedTridentAttackGoal` (`RangedAttackGoal`).
    TridentAttack,
}

impl DrownedGoal {
    pub fn flags(&self) -> u8 {
        match self {
            DrownedGoal::GoToWater { .. } => MOVE,
            DrownedGoal::GoToBeach { .. } => MOVE | JUMP,
            DrownedGoal::SwimUp { .. } => 0,
            DrownedGoal::TridentAttack => MOVE | LOOK,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            DrownedGoal::GoToWater { .. } => "DrownedGoToWaterGoal",
            DrownedGoal::GoToBeach { .. } => "DrownedGoToBeachGoal",
            DrownedGoal::SwimUp { .. } => "DrownedSwimUpGoal",
            DrownedGoal::TridentAttack => "DrownedTridentAttackGoal",
        }
    }

    /// `requiresUpdateEveryTick()`: `MoveToBlockGoal`'s and `RangedAttackGoal`'s.
    pub fn every_tick(&self) -> bool {
        matches!(self, DrownedGoal::GoToBeach { .. } | DrownedGoal::TridentAttack)
    }
}

pub fn is_drowned_id(id: &str) -> bool {
    matches!(id, "drowned" | "baby_drowned")
}

/// `Zombie.registerGoals()` (turtle eggs, the looks) then `Drowned.addBehaviourGoals()`.
pub fn register(ai: &mut MobAi) {
    ai.goals.add(4, Goal::ZombieAttackTurtleEgg(TurtleEgg::default()));
    ai.goals.add(8, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    ai.goals.add(8, Goal::RandomLookAround(RandomLookAround::default()));
    ai.goals.add(1, Goal::Drowned(DrownedGoal::GoToWater { wanted: Vec3::ZERO }));
    ai.goals.add(2, Goal::Drowned(DrownedGoal::TridentAttack));
    ai.goals.add(2, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Drowned, 1.0, false)));
    ai.goals.add(5, Goal::Drowned(DrownedGoal::GoToBeach { next_start_tick: 0, try_ticks: 0, max_stay_ticks: 0, block_pos: BlockPos::new(0, 0, 0), reached: false }));
    ai.goals.add(6, Goal::Drowned(DrownedGoal::SwimUp { stuck: false }));
    ai.goals.add(7, Goal::RandomStroll(RandomStroll::default_interval(1.0)));
    ai.targets.add(1, Goal::HurtByTarget(HurtByTarget::new(true)));
    ai.targets.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::DrownedPlayer, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Villager, 10, false, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::IronGolem, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Axolotl, 10, true, false)));
    ai.targets.add(5, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::BabyTurtle, 10, true, false)));
}

/// `Drowned.okTarget(target)`: by night any, by day only one in water.
pub fn ok_target(w: &World, target_in_water: bool) -> bool {
    !super::zombie::is_bright_outside(w) || target_in_water
}

/// `Drowned.wantsToSwim()`: searching for land (its swim up) or a target in water.
pub fn wants_to_swim(w: &World, i: usize) -> bool {
    if ai(w, i).searching_for_land {
        return true;
    }
    super::super::target::get_target(w, i).and_then(|id| crate::entity::index_of(w, id)).is_some_and(|ti| w.entities.list[ti].was_touching_water)
}

/// `Entity.isUnderWater()`: the eye in water last tick and touching water.
pub fn is_under_water(w: &World, i: usize) -> bool {
    let e = &w.entities.list[i];
    e.was_eye_in_water && e.was_touching_water
}

/// `Drowned.updateSwimming` (`baseTick`, after the fluid interaction): swimming while under water
/// and wanting to swim.
pub fn update_swimming(w: &mut World, i: usize) {
    let s = is_under_water(w, i) && wants_to_swim(w, i);
    crate::entity::mob::ai::ai_mut(w, i).swimming = s;
}

/// `MoveToBlockGoal.findNearestBlock` (range 8, vertical 2, from `verticalSearchStart` 0: y 0,
/// 1, -1, 2, -2; rings outward, each ring's x and z 0, 1, -1, 2, -2...): the first cell (one
/// below the offset) `DrownedGoToBeachGoal.isValidTarget` takes.
fn find_beach(w: &World, i: usize) -> Option<BlockPos> {
    let mob_pos = w.entities.list[i].block_position();
    let next = |v: i32| if v > 0 { -v } else { 1 - v };
    let mut y = 0;
    while y <= 2 {
        for r in 0..8 {
            let mut x = 0;
            while x <= r {
                let mut z = if x < r && x > -r { r } else { 0 };
                while z <= r {
                    let p = BlockPos::new(mob_pos.x + x, mob_pos.y + y - 1, mob_pos.z + z);
                    if beach_target(w, p) {
                        return Some(p);
                    }
                    z = next(z);
                }
                x = next(x);
            }
        }
        y = next(y);
    }
    None
}

/// `DrownedGoToBeachGoal.isValidTarget`: two empty cells above and the cell's collision top full.
fn beach_target(w: &World, p: BlockPos) -> bool {
    w.get(p.above()).is_air() && w.get(p.above().above()).is_air() && crate::entity::mob::blocks::entity_can_stand_on(w, p)
}

/// `Drowned.closeToNextPos()`: the path's target within 2 of the drowned (its corner).
fn close_to_next_pos(w: &World, i: usize) -> bool {
    let Some(t) = crate::entity::mob::ai::nav_of(w, i).path.as_ref().map(|p| p.target) else { return false };
    let e = &w.entities.list[i];
    let (dx, dy, dz) = (t.x as f64 - e.pos.x, t.y as f64 - e.pos.y, t.z as f64 - e.pos.z);
    dx * dx + dy * dy + dz * dz < 4.0
}

fn legacy(w: &mut World, what: &'static str) {
    w.mob.ai.legacy.insert(what);
}

pub fn can_use(w: &mut World, i: usize, g: &mut DrownedGoal) -> bool {
    match g {
        // canUse: by day, out of water, a water cell among 10 tries around (nextInt(20) - 10,
        // 2 - nextInt(8), nextInt(20) - 10)
        DrownedGoal::GoToWater { wanted } => {
            if !super::zombie::is_bright_outside(w) || w.entities.list[i].was_touching_water {
                return false;
            }
            let p = w.entities.list[i].block_position();
            for _ in 0..10 {
                let r = w.entities.list[i].random.as_mut().expect("random");
                let (dx, dy, dz) = (r.next_int(20) - 10, 2 - r.next_int(8), r.next_int(20) - 10);
                let q = BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
                if is_water_block(w, q) {
                    *wanted = Vec3::new(q.x as f64 + 0.5, q.y as f64, q.z as f64 + 0.5);
                    return true;
                }
            }
            false
        }
        // MoveToBlockGoal.canUse (its countdown, then nextStartTick's draw and the search), then by
        // night, in water, at most 3 under the sea level
        DrownedGoal::GoToBeach { next_start_tick, block_pos, .. } => {
            if *next_start_tick > 0 {
                *next_start_tick -= 1;
                return false;
            }
            let n = w.entities.list[i].random.as_mut().expect("random").next_int(200);
            *next_start_tick = reduced_tick_delay(200 + n);
            let Some(p) = find_beach(w, i) else { return false };
            *block_pos = p;
            !super::zombie::is_bright_outside(w) && w.entities.list[i].was_touching_water && w.entities.list[i].pos.y >= (sea_level(w) - 3) as f64
        }
        // by night, in water, more than 2 under the sea level
        DrownedGoal::SwimUp { .. } => !super::zombie::is_bright_outside(w) && w.entities.list[i].was_touching_water && w.entities.list[i].pos.y < (sea_level(w) - 2) as f64,
        // RangedAttackGoal.canUse (a live target) and a trident in its hand: none here
        DrownedGoal::TridentAttack => {
            if ai(w, i).trident && super::super::target::get_target(w, i).is_some() {
                legacy(w, "a drowned's trident (DrownedTridentAttackGoal: it attacks in melee instead)");
            }
            false
        }
    }
}

pub fn can_continue(w: &mut World, i: usize, g: &mut DrownedGoal) -> bool {
    match g {
        // canContinueToUse: the navigation still going
        DrownedGoal::GoToWater { .. } => !crate::entity::mob::ai::nav_of(w, i).is_done(),
        // MoveToBlockGoal.canContinueToUse: within its stay and give-up ticks, the target valid
        DrownedGoal::GoToBeach { try_ticks, max_stay_ticks, block_pos, .. } => *try_ticks >= -*max_stay_ticks && *try_ticks <= 1200 && beach_target(w, *block_pos),
        // canUse and not stuck
        DrownedGoal::SwimUp { stuck } => !*stuck && !super::zombie::is_bright_outside(w) && w.entities.list[i].was_touching_water && w.entities.list[i].pos.y < (sea_level(w) - 2) as f64,
        _ => false,
    }
}

pub fn start(w: &mut World, i: usize, g: &mut DrownedGoal) {
    match g {
        DrownedGoal::GoToWater { wanted } => {
            let p = *wanted;
            with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, 1.0, arena));
        }
        // setSearchingForLand(false), then MoveToBlockGoal.start: moveMobToBlock (the cell's top
        // centre), tryTicks 0, maxStayTicks nextInt(nextInt(1200) + 1200) + 1200
        DrownedGoal::GoToBeach { try_ticks, max_stay_ticks, block_pos, .. } => {
            crate::entity::mob::ai::ai_mut(w, i).searching_for_land = false;
            let p = *block_pos;
            with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x as f64 + 0.5, (p.y + 1) as f64, p.z as f64 + 0.5, 1, 1.0, arena));
            *try_ticks = 0;
            let r = w.entities.list[i].random.as_mut().expect("random");
            let inner = r.next_int(1200) + 1200;
            *max_stay_ticks = r.next_int(inner) + 1200;
        }
        DrownedGoal::SwimUp { stuck } => {
            crate::entity::mob::ai::ai_mut(w, i).searching_for_land = true;
            *stuck = false;
        }
        DrownedGoal::TridentAttack => {}
    }
}

/// `tick()`.
pub fn tick(w: &mut World, i: usize, g: &mut DrownedGoal) {
    match g {
        // MoveToBlockGoal.tick: away from the cell above the target (closerToCenterThan 1), one
        // more try tick and every 40th a new path there; at it, one less
        DrownedGoal::GoToBeach { try_ticks, block_pos, reached, .. } => {
            let t = block_pos.above();
            let pos = w.entities.list[i].pos;
            let (dx, dy, dz) = (t.x as f64 + 0.5 - pos.x, t.y as f64 + 0.5 - pos.y, t.z as f64 + 0.5 - pos.z);
            if dx * dx + dy * dy + dz * dz >= 1.0 {
                *reached = false;
                *try_ticks += 1;
                if *try_ticks % 40 == 0 {
                    with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, t.x as f64 + 0.5, t.y as f64, t.z as f64 + 0.5, 1, 1.0, arena));
                }
            } else {
                *reached = true;
                *try_ticks -= 1;
            }
        }
        // under the sea level - 1, its navigation done or near its path's end: a position 4 x 8
        // towards the sea level's surface above it (90 degrees), or stuck
        DrownedGoal::SwimUp { stuck } => {
            let sea = sea_level(w);
            let pos = w.entities.list[i].pos;
            if pos.y < (sea - 1) as f64 && (crate::entity::mob::ai::nav_of(w, i).is_done() || close_to_next_pos(w, i)) {
                let towards = Vec3::new(pos.x, (sea - 1) as f64, pos.z);
                match super::stroll::default_pos_towards(w, i, 4, 8, towards, std::f32::consts::FRAC_PI_2 as f64) {
                    None => *stuck = true,
                    Some(p) => {
                        with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, 1.0, arena));
                    }
                }
            }
        }
        _ => {}
    }
}

/// `stop()`.
pub fn stop(w: &mut World, i: usize, g: &mut DrownedGoal) {
    if let DrownedGoal::SwimUp { .. } = g {
        crate::entity::mob::ai::ai_mut(w, i).searching_for_land = false;
    }
}

/// `BlockState.is(Blocks.WATER)`: the water block (a source or flowing), not a waterlogged one.
fn is_water_block(w: &World, p: BlockPos) -> bool {
    match w.get(p) {
        crate::block::Block::Water { .. } => true,
        crate::block::Block::Terrain(id) => crate::terrain::blocks::props(id).block_id() == "minecraft:water",
        _ => false,
    }
}

/// `DrownedMoveControl.tick()`: wanting to swim in water, it rises 0.002 towards a higher target
/// or while it searches for land, and steers in water itself (speed eased by 0.125 to the move's,
/// the push along the wanted direction: x and z 0.005, y 0.1 of the unit height; true: handled,
/// `MoveControl.tick` does not run); else off the ground it sinks 0.008 more first (false: the
/// plain `MoveControl.tick` follows).
pub fn move_control_before(w: &mut World, i: usize) -> bool {
    if wants_to_swim(w, i) && w.entities.list[i].was_touching_water {
        let my_y = w.entities.list[i].pos.y;
        let target_above = super::super::target::get_target(w, i).and_then(|id| crate::entity::index_of(w, id)).is_some_and(|ti| w.entities.list[ti].pos.y > my_y);
        if target_above || ai(w, i).searching_for_land {
            let e = &mut w.entities.list[i];
            e.delta_movement = e.delta_movement.add(0.0, 0.002, 0.0);
        }
        let mc = ai(w, i).move_ctl;
        if mc.operation != crate::entity::mob::ai::control::Operation::MoveTo || crate::entity::mob::ai::nav_of(w, i).is_done() {
            // Mob.setSpeed: the speed and the forward input
            let m = crate::entity::mob::mob_mut(w, i);
            m.speed = 0.0;
            m.zza = 0.0;
            return true;
        }
        let p = w.entities.list[i].pos;
        let (xd, mut yd, zd) = (mc.wanted_x - p.x, mc.wanted_y - p.y, mc.wanted_z - p.z);
        let dd = (xd * xd + yd * yd + zd * zd).sqrt();
        yd /= dd;
        let y_rot_d = crate::entity::mob::ai::control::atan2_degrees(zd, xd) - 90.0f32;
        let y_rot = crate::entity::mob::ai::control::rotlerp(w.entities.list[i].y_rot, y_rot_d, 90.0);
        w.entities.list[i].y_rot = y_rot;
        crate::entity::mob::ai::ai_mut(w, i).y_body_rot = y_rot;
        let target_speed = (mc.speed_modifier * crate::entity::mob::ai::movement_speed(w, i)) as f32;
        let cur = crate::entity::mob::mob(w, i).speed;
        let new_speed = cur + 0.125f32 * (target_speed - cur);
        {
            let m = crate::entity::mob::mob_mut(w, i);
            m.speed = new_speed;
            m.zza = new_speed;
        }
        let s = new_speed as f64;
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(s * xd * 0.005, s * yd * 0.1, s * zd * 0.005);
        return true;
    }
    if !w.entities.list[i].on_ground {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(0.0, -0.008, 0.0);
    }
    false
}

/// Whether mob `i` is a drowned (its AI's own parts).
pub fn is_drowned(w: &World, i: usize) -> bool {
    is_drowned_id(mob(w, i).ty.def().id)
}
