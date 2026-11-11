//! The enderman (`Enderman`): its goals (`FloatGoal`, `EndermanFreezeWhenLookedAt` (a player
//! target only), `MeleeAttackGoal`, `WaterAvoidingRandomStrollGoal` with probability 0 (its
//! `nextFloat` still drawn; every cell worth 0: `getWalkTargetValue`), `LookAtPlayerGoal` 8,
//! `RandomLookAroundGoal`, `EndermanLeaveBlockGoal`, `EndermanTakeBlockGoal`) and targets
//! (`EndermanLookForPlayerGoal`: a creative player never angers it, no draws; `HurtByTargetGoal`;
//! endermites; `ResetUniversalAngerTargetGoal`), its `customServerAiStep` (in daylight, seeing the
//! sky in light above 0.5, the random teleport), water and rain's drown damage at the end of its
//! `aiStep`, and `hurtServer`'s teleport (nine times in ten, a damage no living entity caused).
//! A teleport is `LivingEntity.randomTeleport`: down from the drawn cell to a block in
//! `#entities_can_teleport_to`, not one of `#enderman_does_not_teleport_to`, the box free of
//! blocks, liquids and those blocks, not above water; then `snapTo` and the navigation stopped.
//! Taking and leaving blocks follow the goals' own draws and tests (`#enderman_holdable`, the
//! OUTLINE clip, `canPlaceBlock`); a carried block keeps the enderman (requiresCustomPersistence).

use super::{Goal, JUMP, MOVE};
use crate::block::Block;
use crate::entity::Vec3;
use crate::entity::mob::{mob, mob_mut};
use crate::pos::BlockPos;
use crate::world::World;

/// The enderman's own state (`DATA_CARRY_STATE`, `targetChangeTime`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct EndermanState {
    pub carried: Option<Block>,
    pub target_change_time: i32,
}

#[derive(Clone, PartialEq, Debug)]
pub enum EndermanGoal {
    Freeze,
    LeaveBlock,
    TakeBlock,
    LookForPlayer,
}

impl EndermanGoal {
    pub fn flags(&self) -> u8 {
        match self {
            EndermanGoal::Freeze => JUMP | MOVE,
            EndermanGoal::LookForPlayer => super::TARGET,
            _ => 0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            EndermanGoal::Freeze => "EndermanFreezeWhenLookedAt",
            EndermanGoal::LeaveBlock => "EndermanLeaveBlockGoal",
            EndermanGoal::TakeBlock => "EndermanTakeBlockGoal",
            EndermanGoal::LookForPlayer => "EndermanLookForPlayerGoal",
        }
    }
}

pub fn is_enderman(w: &World, i: usize) -> bool {
    mob(w, i).ty.def().id == "enderman"
}

/// `Enderman.registerGoals`.
pub fn register(a: &mut crate::entity::mob::ai::MobAi) {
    use super::look::{LookAtPlayer, LookType, RandomLookAround};
    use super::melee::{MeleeAttack, MeleeKind};
    use super::stroll::RandomStroll;
    use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
    a.goals.add(0, Goal::Float);
    a.goals.add(1, Goal::Enderman(EndermanGoal::Freeze));
    a.goals.add(2, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Melee, 1.0, false)));
    a.goals.add(7, Goal::RandomStroll(RandomStroll { water_avoiding: Some(0.0), ..RandomStroll::water_avoiding(1.0) }));
    a.goals.add(8, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    a.goals.add(8, Goal::RandomLookAround(RandomLookAround::default()));
    a.goals.add(10, Goal::Enderman(EndermanGoal::LeaveBlock));
    a.goals.add(11, Goal::Enderman(EndermanGoal::TakeBlock));
    a.targets.add(1, Goal::Enderman(EndermanGoal::LookForPlayer));
    a.targets.add(2, Goal::HurtByTarget(HurtByTarget::new(false)));
    a.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Endermite, 10, true, false)));
    a.targets.add(4, Goal::ResetUniversalAnger);
    a.enderman = Some(EndermanState::default());
}

fn state(w: &World, i: usize) -> &EndermanState {
    crate::entity::mob::ai::ai(w, i).enderman.as_ref().expect("an enderman")
}

/// (mob-AI 2026-11-10) `Enderman.dropCustomDeathLoot` after Mob's: the carried block's drops with
/// a silk-touch diamond axe (`ENDERMAN_LOOT_DROP`: silk touch I, no draw), which for every
/// #enderman_holdable block is the block itself, one; `spawnAtLocation` at its feet.
/// cross-4-underground plains build 2 t8953: an enderman drowned holding grass.
pub fn drop_carried(w: &mut World, i: usize) {
    if !crate::entity::mob::ai::has_ai(w, i) {
        return;
    }
    let Some(carried) = crate::entity::mob::ai::ai(w, i).enderman.as_ref().and_then(|s| s.carried) else { return };
    let name = carried.to_string();
    let base = name.split('[').next().unwrap_or("").to_string();
    let base = if base.contains(':') { base } else { format!("minecraft:{base}") };
    let pos = w.entities.list[i].pos;
    let by = w.entities.list[i].id;
    crate::entity::mob::living::spawn_item_at(w, pos, by, crate::entity::mob::ai::loot::intern(&base), 1);
}

fn state_mut(w: &mut World, i: usize) -> &mut EndermanState {
    crate::entity::mob::ai::ai_mut(w, i).enderman.as_mut().expect("an enderman")
}

fn rnd(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("a mob's own random")
}

/// An enderman's block action the model does not cover: the fallback rule for ambient natural mobs
/// (2026-11-08): the action is skipped and named in the run's report (the legacy list), never a
/// stop; the farm's own mobs are never endermen.
fn refuse(w: &mut World, why: String) {
    let _ = why;
    w.mob.ai.legacy.insert("minecraft:enderman (some block placements)");
}

#[allow(dead_code)]
fn refuse_hard(w: &mut World, why: String) {
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

pub fn can_use(w: &mut World, i: usize, g: &mut EndermanGoal) -> bool {
    match g {
        // a player target only: a creative player is never one
        EndermanGoal::Freeze | EndermanGoal::LookForPlayer => false,
        EndermanGoal::LeaveBlock => state(w, i).carried.is_some() && w.mob.ai.mob_griefing && rnd(w, i).next_int(super::reduced_tick_delay(2000)) == 0,
        EndermanGoal::TakeBlock => state(w, i).carried.is_none() && w.mob.ai.mob_griefing && rnd(w, i).next_int(super::reduced_tick_delay(20)) == 0,
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut EndermanGoal) {
    match g {
        EndermanGoal::TakeBlock => take_block(w, i),
        EndermanGoal::LeaveBlock => leave_block(w, i),
        _ => {}
    }
}

/// The natural-state row of a cell's state (a design block by its game string).
fn props_at(w: &World, p: BlockPos) -> Option<&'static crate::terrain::blocks::Props> {
    use crate::terrain::blocks;
    match w.get(p) {
        Block::Terrain(id) => Some(blocks::props(id)),
        Block::Air => blocks::id_of("minecraft:air").map(blocks::props),
        b => blocks::id_of(&b.to_string()).map(blocks::props),
    }
}

/// `EndermanTakeBlockGoal.tick()`.
fn take_block(w: &mut World, i: usize) {
    let e = &w.entities.list[i];
    let (x, y, z) = (e.pos.x, e.pos.y, e.pos.z);
    let (bx, bz) = (e.block_position().x, e.block_position().z);
    let r = rnd(w, i);
    let xt = crate::entity::mth::floor(x - 2.0 + r.next_double() * 4.0);
    let yt = crate::entity::mth::floor(y + r.next_double() * 3.0);
    let zt = crate::entity::mth::floor(z - 2.0 + r.next_double() * 4.0);
    let pos = BlockPos::new(xt, yt, zt);
    let from = Vec3::new(bx as f64 + 0.5, yt as f64 + 0.5, bz as f64 + 0.5);
    let to = Vec3::new(xt as f64 + 0.5, yt as f64 + 0.5, zt as f64 + 0.5);
    // the clip (OUTLINE, no fluids): a miss reports the end cell
    let reachable = crate::entity::mob::los::first_hit_outline(w, from, to).unwrap_or(pos) == pos;
    let holdable = props_at(w, pos).is_some_and(|p| p.has_tag("minecraft:enderman_holdable"));
    if holdable && reachable {
        let carried = match w.get(pos) {
            Block::Terrain(id) => {
                let name = crate::terrain::blocks::props(id).block_id().to_string();
                crate::terrain::land::block_of(&name).ok()
            }
            b => Block::parse(b.to_string().split('[').next().unwrap_or("")).ok(),
        };
        let Some(carried) = carried else {
            refuse(w, format!("an enderman taking {} (its default state) is not modeled", w.get(pos)));
            return;
        };
        // removeBlock(pos, false): the cell's fluid's legacy block, else air (flags 3)
        if !crate::fluid::fluid_state(w, pos).is_empty() {
            refuse(w, "an enderman taking a block out of a fluid is not modeled".into());
            return;
        }
        w.set_block(pos, Block::Air, crate::world::flags::ALL);
        state_mut(w, i).carried = Some(carried);
    }
}

/// `EndermanLeaveBlockGoal.tick()`.
fn leave_block(w: &mut World, i: usize) {
    let e = &w.entities.list[i];
    let (x, y, z) = (e.pos.x, e.pos.y, e.pos.z);
    let r = rnd(w, i);
    let xt = crate::entity::mth::floor(x - 1.0 + r.next_double() * 2.0);
    let yt = crate::entity::mth::floor(y + r.next_double() * 2.0);
    let zt = crate::entity::mth::floor(z - 1.0 + r.next_double() * 2.0);
    let pos = BlockPos::new(xt, yt, zt);
    let below = pos.below();
    let Some(carried) = state(w, i).carried else { return };
    let target_air = w.get(pos).is_air();
    let below_ok = !w.get(below).is_air() && props_at(w, below).is_some_and(|p| p.block_id() != "minecraft:bedrock" && p.full_collision);
    if !(target_air && below_ok) {
        return;
    }
    // Block.updateFromNeighbourShapes and canSurvive: a block without its own canSurvive survives
    // anywhere (a snowy one takes its snowy flag from the cell above); a small flower needs
    // #supports_vegetation under it (VegetationBlock), a mushroom #overrides_mushroom_light_
    // requirement or raw light below 13 over a solid-render block (MushroomBlock). One that
    // cannot survive there turns to air in its shape update (VegetationBlock.updateShape), and
    // air survives: the enderman "places" air on the air and holds nothing (its block is gone)
    let name = carried.to_string();
    let base = name.split('[').next().unwrap_or("").to_string();
    let Some(cp) = props_at_name(&name) else {
        refuse(w, format!("an enderman placing {base} (not in the natural-state table) is not modeled"));
        return;
    };
    let below_props = props_at(w, below);
    let survives = if !cp.reactive.iter().any(|m| m == "canSurvive") {
        true
    } else if cp.has_tag("minecraft:small_flowers") && base != "minecraft:wither_rose" {
        below_props.is_some_and(|b| b.has_tag("minecraft:supports_vegetation"))
    } else if base == "minecraft:brown_mushroom" || base == "minecraft:red_mushroom" {
        below_props.is_some_and(|b| b.has_tag("minecraft:overrides_mushroom_light_requirement"))
            || (crate::entity::mob::natural::brightness(w, pos, 0) < 13 && below_props.is_some_and(|b| b.solid_render))
    } else {
        refuse(w, format!("an enderman placing {base} (its canSurvive and shape update) is not modeled"));
        return;
    };
    let placed = if !survives {
        Block::Air
    } else if name.contains("snowy=") {
        let above = w.get(pos.above()).to_string();
        let snowy = above.starts_with("minecraft:snow[") || above == "minecraft:snow_block" || above.starts_with("minecraft:snow ");
        let s = name.replace("snowy=true", "snowy=false").replace("snowy=false", if snowy { "snowy=true" } else { "snowy=false" });
        crate::terrain::land::block_of(&s).unwrap_or(carried)
    } else {
        carried
    };
    // no entity but this one in the cell (`getEntities(enderman, unitCube)`)
    let me = w.entities.list[i].id;
    let cube = crate::entity::Aabb::of_block(pos);
    if !w.get_entities(cube, |e| e.id != me && !e.removed).is_empty() {
        return;
    }
    w.set_block(pos, placed, crate::world::flags::ALL);
    state_mut(w, i).carried = None;
}

fn props_at_name(name: &str) -> Option<&'static crate::terrain::blocks::Props> {
    crate::terrain::blocks::id_of(name).map(crate::terrain::blocks::props)
}

/// `Enderman.customServerAiStep` (after the navigation's tick): in daylight 600 ticks after its
/// last target change, light above 0.5 under the open sky, `nextFloat() * 30 < (br - 0.4) * 2`
/// drops its target and teleports.
pub fn custom_server_ai_step(w: &mut World, i: usize) {
    let tick_count = w.entities.list[i].tick_count;
    if std::env::var("MCTAS_TRACE_ENDERMAN").is_ok_and(|v| v.parse::<u32>().ok() == Some(w.entities.list[i].id)) {
        let day = w.day_time();
        eprintln!("enderman t{} sky: day {day} rain {} thunder {} level {} darken {} target {:?} tct {} tick {tick_count}", w.time, w.weather.rain(1.0), w.weather.thunder(1.0), crate::daylight::sky_light_level(day), w.sky_darken(), crate::entity::mob::ai::ai(w, i).target, state(w, i).target_change_time);
    }
    if !super::zombie::is_bright_outside(w) || tick_count < state(w, i).target_change_time + 600 {
        return;
    }
    let e = &w.entities.list[i];
    let eye = crate::entity::mob::mob(w, i).eye() as f64;
    let cell = BlockPos::new(crate::entity::mth::floor(e.pos.x), crate::entity::mth::floor(e.pos.y + eye), crate::entity::mth::floor(e.pos.z));
    let feet = e.block_position();
    let br = super::stroll::light_magic(w, cell);
    let eid = w.entities.list[i].id;
    if std::env::var("MCTAS_TRACE_ENDERMAN").is_ok_and(|v| v.parse::<u32>().ok() == Some(eid)) {
        eprintln!("enderman t{} #{eid}: eye cell {cell} br {br} feet {feet} sky light {:?} block light {:?} tick {tick_count} target change {}", w.time, w.sky_light(feet), w.block_light(cell), state(w, i).target_change_time);
    }
    if br > 0.5 && w.can_see_sky(feet).unwrap_or(false) && rnd(w, i).next_float() * 30.0 < (br - 0.4) * 2.0 {
        set_target(w, i, None);
        teleport(w, i);
    }
}

/// `Enderman.setTarget` (target::set_target keeps its target change time).
pub fn set_target(w: &mut World, i: usize, t: Option<u32>) {
    super::super::target::set_target(w, i, t);
}

/// The movement speed's attack boost (`SPEED_MODIFIER_ATTACKING`: +0.15 while it has a target).
pub fn speed_bonus(w: &World, i: usize) -> f64 {
    if super::super::target::get_target(w, i).is_some() { 0.15f32 as f64 } else { 0.0 }
}

/// `Enderman.repeatedlyTryToTeleport()` (a projectile's hit): not a passenger, up to 64
/// teleports until one lands.
pub fn repeatedly_try_to_teleport(w: &mut World, i: usize) {
    if w.entities.list[i].vehicle.is_some() {
        return;
    }
    for _ in 0..64 {
        if teleport(w, i) {
            return;
        }
    }
}

/// `Enderman.teleport()`: a cell within 32 (x, z) and 32 (y) drawn from its own random.
pub fn teleport(w: &mut World, i: usize) -> bool {
    if mob(w, i).health <= 0.0 || mob(w, i).dead {
        return false;
    }
    let p = w.entities.list[i].pos;
    let r = rnd(w, i);
    let xx = p.x + (r.next_double() - 0.5) * 64.0;
    let yy = p.y + (r.next_int(64) - 32) as f64;
    let zz = p.z + (r.next_double() - 0.5) * 64.0;
    teleport_to(w, i, xx, yy, zz)
}

/// `Enderman.teleport(x, y, z)` (not a passenger) through `LivingEntity.randomTeleport`.
fn teleport_to(w: &mut World, i: usize, xx: f64, yy: f64, zz: f64) -> bool {
    if w.entities.list[i].vehicle.is_some() {
        return false;
    }
    let trace = std::env::var("MCTAS_TRACE_ENDERMAN").is_ok_and(|v| v.parse::<u32>().ok() == Some(w.entities.list[i].id));
    let mut p = BlockPos::new(crate::entity::mth::floor(xx), crate::entity::mth::floor(yy), crate::entity::mth::floor(zz));
    if trace {
        let full = w.mob.custom.as_ref().is_some_and(|c| c.full_chunks.contains(&[p.x >> 4, p.z >> 4]));
        eprintln!("enderman t{} teleport to ({xx}, {yy}, {zz}): start {p} contains {} (box {} size {:?}; the game's chunk loaded {full})", w.time, w.contains(p), w.min, w.size);
    }
    // (mob-AI 2026-11-11, tickexact's reading) past the modeled box the column is read through
    // `World::get` like inside: the terrain world keeps what natural updates wrote past the box
    // (`World::outer`: a flow's stone in the sample's air ring), else air, as the game's import
    // left it; a void column finds nothing on both sides (d1u-deep plains b5 t63833: the game
    // landed at y 8 a block past the box's edge)
    let mut y = yy;
    let min_y = w.min.y;
    while p.y > min_y {
        p = p.below();
        let tags = props_at(w, p);
        if trace && tags.is_none() {
            eprintln!("enderman teleport: {p} {} has no natural row", w.get(p));
        }
        if tags.is_some_and(|t| t.has_tag("minecraft:entities_can_teleport_to")) {
            if trace {
                eprintln!("enderman teleport: lands on {p} {}", w.get(p));
            }
            // checkPositionAndTeleport
            if tags.is_some_and(|t| t.has_tag("minecraft:enderman_does_not_teleport_to")) {
                return false;
            }
            let (wd, ht) = {
                let d = mob(w, i).ty.def();
                (d.width as f64, d.height as f64)
            };
            let bb = crate::entity::Aabb::new(xx - wd / 2.0, y, zz - wd / 2.0, xx + wd / 2.0, y + ht, zz + wd / 2.0);
            if !crate::entity::collide::no_collision(w, i, bb) || crate::entity::mob::blocks::box_has_liquid(w, bb) {
                return false;
            }
            // findBlocksIn(box): any of the box's cells in the tag
            let (x0, y0, z0) = (crate::entity::mth::floor(bb.min_x), crate::entity::mth::floor(bb.min_y), crate::entity::mth::floor(bb.min_z));
            let (x1, y1, z1) = (crate::entity::mth::floor(bb.max_x), crate::entity::mth::floor(bb.max_y), crate::entity::mth::floor(bb.max_z));
            for cy in y0..=y1 {
                for cz in z0..=z1 {
                    for cx in x0..=x1 {
                        if props_at(w, BlockPos::new(cx, cy, cz)).is_some_and(|t| t.has_tag("minecraft:enderman_does_not_teleport_to")) {
                            return false;
                        }
                    }
                }
            }
            // canRandomlyTeleportTo: not above water
            let under = BlockPos::new(crate::entity::mth::floor(xx), crate::entity::mth::floor(y), crate::entity::mth::floor(zz)).below();
            if crate::fluid::fluid_state(w, under).fluid.is_water() {
                return false;
            }
            // teleportTo: snapTo (the old positions too), the navigation stopped
            let pos = Vec3::new(xx, y, zz);
            w.entities.set_pos(i, pos);
            let e = &mut w.entities.list[i];
            (e.xo, e.yo, e.zo) = (xx, y, zz);
            (e.x_old, e.y_old, e.z_old) = (xx, y, zz);
            crate::entity::mob::ai::nav_of_mut(w, i).stop();
            return true;
        }
        y -= 1.0;
    }
    false
}

/// `Enderman.hurtServer`'s tail: a damage no living entity caused teleports it nine times in ten
/// (the roll drawn whether it lived or not).
pub fn after_hurt(w: &mut World, i: usize, src: crate::entity::mob::Damage) {
    // a living attacker's (a hit, a spear's stab, an arrow's or a trident's owner) teleports it
    // not (`source.getEntity() instanceof LivingEntity`)
    if src.by_attacker() && w.mob.ai.attacker.is_some() {
        return;
    }
    if rnd(w, i).next_int(10) != 0 {
        teleport(w, i);
    }
}

/// The end of `LivingEntity.aiStep`: water-sensitive in water or rain, drowned for 1.
pub fn water_tick(w: &mut World, i: usize) {
    if crate::entity::mob::sun::in_water_or_rain(w, i) {
        crate::entity::mob::living::hurt(w, i, crate::entity::mob::Damage::Drown, 1.0);
    }
    let _ = mob_mut;
}
