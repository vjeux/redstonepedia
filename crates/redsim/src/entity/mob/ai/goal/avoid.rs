//! `AvoidEntityGoal` (26.3): the skeleton's of wolves (6, walk 1.0, sprint 1.2), the creeper's of
//! ocelots and cats (none are modeled: never found), a fish's of players (8, 1.6, 1.4; a creative
//! player is no combat target: never found). The nearest one within its reach (the box inflated by
//! it, 3 up and down; the combat conditions with its range and line of sight), a spot away from it
//! (`DefaultRandomPos.getPosAway(16, 7)`) no nearer to it than the mob is, a path there (reach 0);
//! walked at the walk speed, sprinted within 7 of what it avoids.

use super::super::path::Path;
use super::super::target::{self, Conditions, Who};
use super::super::with_own_nav;
use crate::entity::Vec3;
use crate::world::World;

/// The class an `AvoidEntityGoal` avoids.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AvoidClass {
    Wolf,
    /// `Ocelot`, `Cat`: not modeled, never found.
    Ocelot,
    Cat,
    /// `Armadillo` (not scared; the spider's): not modeled, never found.
    Armadillo,
    /// `Player` with `NO_SPECTATORS` (a fish's).
    Player,
    /// (mob-AI 2026-11-11) `Zombie` (a wandering trader's: 8, 0.5, 0.5).
    Zombie,
}

/// An `AvoidEntityGoal`.
#[derive(Clone, PartialEq, Debug)]
pub struct AvoidEntity {
    pub class: AvoidClass,
    pub max_dist: f32,
    pub walk: f64,
    pub sprint: f64,
    /// `toAvoid` (an entity id, a player's from `PLAYER_ID_BASE`).
    pub to_avoid: Option<u32>,
    pub path: Option<Path>,
}

impl AvoidEntity {
    pub fn new(class: AvoidClass, max_dist: f32, walk: f64, sprint: f64) -> AvoidEntity {
        AvoidEntity { class, max_dist, walk, sprint, to_avoid: None, path: None }
    }
}

pub fn can_use(w: &mut World, i: usize, g: &mut AvoidEntity) -> bool {
    let d = g.max_dist as f64;
    let area = w.entities.list[i].bb.inflate(d, 3.0, d);
    let list: Vec<Who> = match g.class {
        AvoidClass::Wolf => target::mobs_in(w, area, |ty| ty.game_id() == "minecraft:wolf"),
        // (mob-AI 2026-11-11) `Zombie` and its subclasses (a wandering trader's)
        AvoidClass::Zombie => target::mobs_in(w, area, |ty| matches!(ty.game_id(), "minecraft:zombie" | "minecraft:husk" | "minecraft:drowned" | "minecraft:zombie_villager" | "minecraft:zombified_piglin")),
        // (any cat: a tame one's own predicate is not modeled, none are tame)
        AvoidClass::Cat => target::mobs_in(w, area, |ty| ty.game_id() == "minecraft:cat"),
        AvoidClass::Ocelot | AvoidClass::Armadillo => Vec::new(),
        AvoidClass::Player => target::players(w)
            .into_iter()
            .filter(|p| {
                let pp = target::pos_of(w, *p);
                crate::entity::make_bounding_box(pp, target::PLAYER_WIDTH, target::PLAYER_HEIGHT).intersects(area)
            })
            .collect(),
    };
    let cond = Conditions::combat().range(d);
    let at = w.entities.list[i].pos;
    let Some(t) = target::nearest(w, i, &list, &cond, at) else {
        g.to_avoid = None;
        return false;
    };
    if std::env::var("MCTAS_TRACE_AVOID").is_ok() {
        eprintln!("avoid t{}: #{} avoids {:?}", w.time, w.entities.list[i].id, g.class);
    }
    g.to_avoid = Some(target::id_of(w, t));
    let tp = target::pos_of(w, t);
    let Some(pos) = super::stroll::default_pos_away(w, i, 16, 7, tp) else { return false };
    if tp.distance_to_sqr(pos) < tp.distance_to_sqr(w.entities.list[i].pos) {
        return false;
    }
    // createPath(x, y, z, 0): the cell containing the spot, on `pathNav` (the mob's own navigation:
    // the constructor's getNavigation(), before any ride; a spider jockey's skeleton keeps using
    // its own, whose tick then steers the spider's move control; forest build 3 t20081)
    let cell = crate::pos::BlockPos::new(pos.x.floor() as i32, pos.y.floor() as i32, pos.z.floor() as i32);
    g.path = with_own_nav(w, i, |w, nm, nav, arena| nav.create_path_pos(w, nm, cell, 0, arena));
    g.path.is_some()
}

pub fn can_continue(w: &mut World, i: usize, _g: &mut AvoidEntity) -> bool {
    !super::super::ai(w, i).nav.is_done()
}

pub fn start(w: &mut World, i: usize, g: &mut AvoidEntity) {
    let (path, speed) = (g.path.clone(), g.walk);
    with_own_nav(w, i, |w, nm, nav, _| nav.move_to_path(w, nm, path, speed));
}

pub fn stop(_w: &mut World, _i: usize, g: &mut AvoidEntity) {
    g.to_avoid = None;
}

/// `tick()`: sprinting within 7 of what it avoids (on getNavigation()'s: a rider's vehicle's).
pub fn tick(w: &mut World, i: usize, g: &mut AvoidEntity) {
    let Some(t) = g.to_avoid.and_then(|id| target::who(w, id)) else { return };
    let near = w.entities.list[i].pos.distance_to_sqr(target::pos_of(w, t)) < 49.0;
    let speed = if near { g.sprint } else { g.walk };
    super::super::nav_of_mut(w, i).speed_modifier = speed;
}

#[allow(dead_code)]
fn _v(_: Vec3) {}
