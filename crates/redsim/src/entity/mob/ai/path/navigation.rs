//! `PathNavigation` and `GroundPathNavigation` (26.3), with `WallClimberNavigation`'s overrides:
//! creating paths (the region radius, the node budget, the surface search), following them
//! (waypoints, corner cutting, stuck detection, the waypoint timeout), recomputing them when a
//! block on the way changes.

use super::finder;
use super::node::Arena;
use super::path::Path;
use super::path_type::{Malus, PathType};
use super::walk::{EvalFlags, PathMob, WalkEval};
use crate::entity::{Aabb, Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;

/// What a navigation reads of its mob each call (the `Mob` getters), filled by the caller.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct NavMob {
    pub pos: Vec3,
    pub bb: Aabb,
    pub width: f32,
    pub height: f32,
    pub max_up_step: f32,
    pub on_ground: bool,
    /// `isInLiquid()`: in water or lava.
    pub in_liquid: bool,
    /// `isInFloatableFluid()`.
    pub in_floatable: bool,
    pub is_passenger: bool,
    /// `LivingEntity.getSpeed()`.
    pub speed: f32,
    pub malus: Malus,
    pub max_fall: i32,
    /// `getAttributeValue(FOLLOW_RANGE)`.
    pub follow_range: f64,
    pub game_time: i64,
}

impl NavMob {
    fn path_mob(&self) -> PathMob {
        PathMob { pos: self.pos, bb: self.bb, width: self.width, height: self.height, max_up_step: self.max_up_step, on_ground: self.on_ground, in_floatable: self.in_floatable, malus: self.malus, max_fall: self.max_fall }
    }

    pub fn block_position(&self) -> BlockPos {
        BlockPos::new(mth::floor(self.pos.x), mth::floor(self.pos.y), mth::floor(self.pos.z))
    }
}

/// Which navigation class a mob has (`createNavigation`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavKind {
    Ground,
    WallClimber,
    /// `AmphibiousPathNavigation` (a drowned's): `AmphibiousNodeEvaluator(false)`, always free to
    /// update its path, its mob position at half its height, no surface search.
    Amphibious,
    /// `WaterBoundPathNavigation` (a fish's): `SwimNodeEvaluator(false)`, its path updated only in
    /// a liquid, its mob position at half its height, the target's own y, a waypoint within 0.5
    /// up or down, no surface search, a destination is any cell that does not render solid.
    WaterBound,
}

impl NavKind {
    /// A navigation whose mob position is `getY(0.5)` and whose ground is the target's own y.
    fn mid_height(self) -> bool {
        matches!(self, NavKind::Amphibious | NavKind::WaterBound)
    }
}

/// Every path search's count and the time spent in them (benchmarks read them).
pub static SEARCHES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static SEARCH_NANOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A probe: `MCTAS_PATH_DUMP="x,y,z"` prints every path found to that target (its start, its
/// nodes with their types, whether it reaches).
fn path_dump_target() -> Option<BlockPos> {
    static P: std::sync::OnceLock<Option<BlockPos>> = std::sync::OnceLock::new();
    *P.get_or_init(|| {
        let v = std::env::var("MCTAS_PATH_DUMP").ok()?;
        let c: Vec<i32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        (c.len() == 3).then(|| BlockPos::new(c[0], c[1], c[2]))
    })
}

/// A `PathNavigation`'s state.
#[derive(Clone, PartialEq, Debug)]
pub struct Navigation {
    pub kind: NavKind,
    pub path: Option<Path>,
    pub speed_modifier: f64,
    pub tick: i32,
    pub last_stuck_check: i32,
    pub last_stuck_check_pos: Vec3,
    pub timeout_cached_node: BlockPos,
    pub timeout_timer: i64,
    pub last_timeout_check: i64,
    pub timeout_limit: f64,
    pub max_distance_to_waypoint: f32,
    pub has_delayed_recomputation: bool,
    pub time_last_recompute: i64,
    pub flags: EvalFlags,
    pub target_pos: Option<BlockPos>,
    pub reach_range: i32,
    pub max_visited_nodes_multiplier: f32,
    /// The path finder's `maxVisitedNodes`.
    pub max_visited_nodes: i32,
    pub is_stuck: bool,
    pub required_path_length: f32,
    pub avoid_sun: bool,
    pub can_path_to_targets_below_surface: bool,
    /// `WallClimberNavigation.pathToPosition`.
    pub path_to_position: Option<BlockPos>,
    /// Searches run (for the benchmarks).
    pub searches: u64,
}

impl Navigation {
    /// The constructor: the path finder's budget from the FOLLOW_RANGE base value x 16.
    pub fn new(kind: NavKind, follow_range_base: f64) -> Navigation {
        Navigation {
            kind,
            path: None,
            speed_modifier: 0.0,
            tick: 0,
            last_stuck_check: 0,
            last_stuck_check_pos: Vec3::ZERO,
            timeout_cached_node: BlockPos::new(0, 0, 0),
            timeout_timer: 0,
            last_timeout_check: 0,
            timeout_limit: 0.0,
            max_distance_to_waypoint: 0.5,
            has_delayed_recomputation: false,
            time_last_recompute: 0,
            flags: EvalFlags::default(),
            target_pos: None,
            reach_range: 0,
            max_visited_nodes_multiplier: 1.0,
            max_visited_nodes: mth::floor(follow_range_base * 16.0),
            is_stuck: false,
            required_path_length: 16.0,
            avoid_sun: false,
            can_path_to_targets_below_surface: false,
            path_to_position: None,
            searches: 0,
        }
    }

    /// `getMaxPathLength()`.
    fn max_path_length(&self, m: &NavMob) -> f32 {
        (m.follow_range as f32).max(self.required_path_length)
    }

    /// `setRequiredPathLength` (then `updatePathfinderMaxVisitedNodes`).
    pub fn set_required_path_length(&mut self, len: f32, m: &NavMob) {
        self.required_path_length = len;
        self.max_visited_nodes = mth::floor((self.max_path_length(m) * 16.0f32) as f64);
    }

    /// `GroundPathNavigation.canUpdatePath` (`AmphibiousPathNavigation`: always;
    /// `WaterBoundPathNavigation`: in a liquid).
    fn can_update_path(&self, m: &NavMob) -> bool {
        match self.kind {
            NavKind::Amphibious => true,
            NavKind::WaterBound => m.in_liquid,
            _ => m.on_ground || m.in_liquid || m.is_passenger,
        }
    }

    /// `GroundPathNavigation.getSurfaceY`.
    fn surface_y(&self, w: &World, m: &NavMob) -> i32 {
        if m.in_floatable && self.flags.can_float {
            let mut surface = mth::floor(m.pos.y);
            let (x, z) = (mth::floor(m.pos.x), mth::floor(m.pos.z));
            let mut steps = 0;
            while crate::fluid::fluid_state(w, BlockPos::new(x, surface, z)).fluid.is_water() {
                surface += 1;
                steps += 1;
                if steps > 16 {
                    return mth::floor(m.pos.y);
                }
            }
            surface
        } else {
            mth::floor(m.pos.y + 0.5)
        }
    }

    /// `getTempMobPos()` (`AmphibiousPathNavigation`, `WaterBoundPathNavigation`: `getY(0.5)`).
    fn temp_mob_pos(&self, w: &World, m: &NavMob) -> Vec3 {
        if self.kind.mid_height() {
            return Vec3::new(m.pos.x, m.pos.y + m.height as f64 * 0.5, m.pos.z);
        }
        Vec3::new(m.pos.x, self.surface_y(w, m) as f64, m.pos.z)
    }

    /// `GroundPathNavigation.findSurfacePosition` (the target's column in its chunk; the level's
    /// own height: tickexact 2026-11-10, a Nether golem's path to an overworld zombie below the
    /// Nether's floor ends at its ceiling, 256).
    fn find_surface_position(&self, w: &World, mut pos: BlockPos) -> BlockPos {
        let (min_y, max_y) = (w.level_min_y(), w.level_max_y());
        if w.get(pos).is_air() {
            let mut c = pos.below();
            while c.y >= min_y && w.get(c).is_air() {
                c = c.below();
            }
            if c.y >= min_y {
                return c.above();
            }
            c = BlockPos::new(pos.x, pos.y + 1, pos.z);
            while c.y <= max_y && w.get(c).is_air() {
                c = c.above();
            }
            pos = c;
        }
        if !crate::behavior::is_solid(&w.get(pos)) {
            return pos;
        }
        let mut c = pos.above();
        while c.y <= max_y && crate::behavior::is_solid(&w.get(c)) {
            c = c.above();
        }
        c
    }

    /// The target's chunk is loaded (`getChunkNow` != null): it meets the world's box.
    fn chunk_loaded(w: &World, p: BlockPos) -> bool {
        let (cx, cz) = (p.x >> 4, p.z >> 4);
        // (mob-AI 2026-11-10) `getChunkNow`: the game's chunks at full status, when the bridge
        // reported them (a terrain world's sandbox loads void superflat chunks past the sample:
        // a wolf at the box's edge paths into one; forest build 1 t13035)
        if let Some(c) = w.mob.custom.as_ref().filter(|c| !c.full_chunks.is_empty()) {
            return c.full_chunks.contains(&[cx, cz]);
        }
        let (x0, z0) = (w.min.x >> 4, w.min.z >> 4);
        let (x1, z1) = ((w.min.x + w.size[0] - 1) >> 4, (w.min.z + w.size[2] - 1) >> 4);
        cx >= x0 && cx <= x1 && cz >= z0 && cz <= z1
    }

    /// `createPath(BlockPos, reach)` (GroundPathNavigation: the surface first; WallClimber
    /// remembers the position).
    pub fn create_path_pos(&mut self, w: &World, m: &NavMob, pos: BlockPos, reach: i32, arena: &mut Arena) -> Option<Path> {
        let requested = pos;
        if self.kind == NavKind::WallClimber {
            self.path_to_position = Some(pos);
        }
        // PathNavigation.createPath(BlockPos, reach) (a water-bound navigation's): no chunk test,
        // no surface search
        if self.kind == NavKind::WaterBound {
            let max = self.max_path_length(m);
            return self.create_path_set(w, m, &[pos], 8, false, reach, max, arena);
        }
        if !Self::chunk_loaded(w, pos) {
            return None;
        }
        let pos = if self.can_path_to_targets_below_surface || self.kind == NavKind::Amphibious { pos } else { self.find_surface_position(w, pos) };
        if path_dump_target() == Some(pos) {
            let col: Vec<String> = (-2..=2).map(|d| format!("{}:{}", pos.y + d, w.get(BlockPos::new(pos.x, pos.y + d, pos.z)))).collect();
            eprintln!("path dump t{}: target {:?} surface-searched to {:?}; column {}", m.game_time, requested, pos, col.join(" "));
        }
        let max = self.max_path_length(m);
        self.create_path_set(w, m, &[pos], 8, false, reach, max, arena)
    }

    /// `createPath(Set<BlockPos>, reach)` (no surface search).
    pub fn create_path_positions(&mut self, w: &World, m: &NavMob, positions: &[BlockPos], reach: i32, arena: &mut Arena) -> Option<Path> {
        let max = self.max_path_length(m);
        self.create_path_set(w, m, positions, 8, false, reach, max, arena)
    }

    /// `createPath(BlockPos, reach, maxPathLength)`.
    pub fn create_path_max(&mut self, w: &World, m: &NavMob, pos: BlockPos, reach: i32, max_path_length: i32, arena: &mut Arena) -> Option<Path> {
        self.create_path_set(w, m, &[pos], 8, false, reach, max_path_length as f32, arena)
    }

    /// `createPath(Set, radiusOffset, above, reach, maxPathLength)`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_path_set(&mut self, w: &World, m: &NavMob, targets: &[BlockPos], _radius_offset: i32, _above: bool, reach: i32, max_path_length: f32, arena: &mut Arena) -> Option<Path> {
        if targets.is_empty() || m.pos.y < w.level_min_y() as f64 || !self.can_update_path(m) {
            return None;
        }
        if let (Some(p), Some(tp)) = (&self.path, self.target_pos)
            && !p.is_done()
            && targets.contains(&tp)
        {
            return Some(p.clone());
        }
        let max_visited = (self.max_visited_nodes as f32 * self.max_visited_nodes_multiplier) as i32;
        // (the search's time for `mobai bench`; the browser's wasm has no clock: `Instant::now`
        // panics there, and the viewer's mob farms run this)
        #[cfg(not(target_arch = "wasm32"))]
        let t0 = std::time::Instant::now();
        let path = {
            let mut flags = self.flags;
            flags.amphibious = self.kind == NavKind::Amphibious;
            flags.swim = self.kind == NavKind::WaterBound;
            let mut eval = WalkEval::new(w, m.path_mob(), flags, arena);
            let p = finder::find_path(&mut eval, targets, max_path_length, reach, max_visited);
            // MCTAS_PATH_DUMP: the search's nodes within 4 of the target too
            if let Some(want) = path_dump_target()
                && targets.contains(&want)
            {
                for n in eval.arena.nodes.iter() {
                    let (dx, dy, dz) = ((n.x - want.x) as f32, (n.y - want.y) as f32, (n.z - want.z) as f32);
                    if (dx * dx + dy * dy + dz * dz).sqrt() <= 4.0 {
                        let from = if n.came_from == super::node::NONE { "-".to_string() } else { let c = eval.arena.n(n.came_from); format!("{},{},{}", c.x, c.y, c.z) };
                        eprintln!("path node t{}: {},{},{} {:?} malus {} g {} walked {} closed {} open {} from {}", m.game_time, n.x, n.y, n.z, n.ty, n.cost_malus, n.g, n.walked_distance, n.closed, n.heap_idx >= 0, from);
                    }
                }
            }
            p
        };
        #[cfg(not(target_arch = "wasm32"))]
        SEARCH_NANOS.fetch_add(t0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        SEARCHES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.searches += 1;
        if let (Some(want), Some(p)) = (path_dump_target(), &path)
            && want == p.target
        {
            let nodes: Vec<String> = p.nodes.iter().map(|n| format!("{},{},{}:{:?}", n.x, n.y, n.z, n.ty)).collect();
            eprintln!("path dump t{} from {:?} reach {} max {} (follow {} required {}): {} nodes [{}] reached {} (searched {} of {} nodes)", m.game_time, m.pos, reach, max_path_length, m.follow_range, self.required_path_length, nodes.len(), nodes.join(" "), p.can_reach(), finder::LAST_COUNT.load(std::sync::atomic::Ordering::Relaxed), finder::LAST_BUDGET.load(std::sync::atomic::Ordering::Relaxed));
        }
        if let Some(p) = &path {
            self.target_pos = Some(p.target);
            self.reach_range = reach;
            self.reset_stuck_timeout();
        }
        path
    }

    /// `moveTo(path, speedModifier)`.
    pub fn move_to_path(&mut self, w: &World, m: &NavMob, new_path: Option<Path>, speed: f64) -> bool {
        let Some(np) = new_path else {
            self.path = None;
            return false;
        };
        if !np.same_as(self.path.as_ref()) {
            self.path = Some(np);
        }
        if self.is_done() {
            return false;
        }
        self.trim_path(w, m);
        if self.path.as_ref().is_none_or(|p| p.node_count() == 0) {
            return false;
        }
        self.speed_modifier = speed;
        let mp = self.temp_mob_pos(w, m);
        self.last_stuck_check = self.tick;
        self.last_stuck_check_pos = mp;
        true
    }

    /// `moveTo(x, y, z, speedModifier)` (reach 1).
    pub fn move_to_xyz(&mut self, w: &World, m: &NavMob, x: f64, y: f64, z: f64, reach: i32, speed: f64, arena: &mut Arena) -> bool {
        let p = BlockPos::new(mth::floor(x), mth::floor(y), mth::floor(z));
        let path = self.create_path_pos(w, m, p, reach, arena);
        self.move_to_path(w, m, path, speed)
    }

    /// `moveTo(entity, reach, speedModifier)` (the three-argument form: `createPath(entity,
    /// reach)` then `moveTo(path)`; a wall climber's `createPath` remembers the position).
    pub fn move_to_entity(&mut self, w: &World, m: &NavMob, target_block: BlockPos, reach: i32, speed: f64, arena: &mut Arena) -> bool {
        let np = self.create_path_pos(w, m, target_block, reach, arena);
        np.is_some() && self.move_to_path(w, m, np, speed)
    }

    /// `moveTo(entity, speedModifier)` (reach 1; `WallClimberNavigation`: reach 0, and the
    /// target's position as a direct goal when there is no path).
    pub fn move_to_entity_default(&mut self, w: &World, m: &NavMob, target_block: BlockPos, speed: f64, arena: &mut Arena) -> bool {
        if self.kind == NavKind::WallClimber {
            let np = self.create_path_pos(w, m, target_block, 0, arena);
            if np.is_some() {
                return self.move_to_path(w, m, np, speed);
            }
            self.path_to_position = Some(target_block);
            self.speed_modifier = speed;
            return true;
        }
        self.move_to_entity(w, m, target_block, 1, speed, arena)
    }

    pub fn is_done(&self) -> bool {
        self.path.as_ref().is_none_or(|p| p.is_done())
    }

    pub fn is_in_progress(&self) -> bool {
        !self.is_done()
    }

    pub fn stop(&mut self) {
        self.path = None;
    }

    fn reset_stuck_timeout(&mut self) {
        self.timeout_cached_node = BlockPos::new(0, 0, 0);
        self.timeout_timer = 0;
        self.timeout_limit = 0.0;
        self.is_stuck = false;
    }

    /// `trimPath()`: cauldrons lift their nodes; avoid-sun cuts the path at the first sky-lit node.
    fn trim_path(&mut self, w: &World, m: &NavMob) {
        let Some(p) = self.path.as_mut() else { return };
        for i in 0..p.node_count() {
            let n = p.nodes[i];
            if is_cauldron(&w.get(n.pos())) {
                p.nodes[i] = n.moved(n.x, n.y + 1, n.z);
                if i + 1 < p.node_count() {
                    let next = p.nodes[i + 1];
                    if n.y >= next.y {
                        p.nodes[i + 1] = n.moved(next.x, n.y + 1, next.z);
                    }
                }
            }
        }
        if self.avoid_sun {
            let here = BlockPos::new(mth::floor(m.pos.x), mth::floor(m.pos.y + 0.5), mth::floor(m.pos.z));
            if w.can_see_sky(here).unwrap_or(false) {
                return;
            }
            let p = self.path.as_mut().expect("checked");
            for i in 0..p.node_count() {
                if w.can_see_sky(p.nodes[i].pos()).unwrap_or(false) {
                    p.truncate_nodes(i);
                    return;
                }
            }
        }
    }

    /// `recomputePath()`.
    pub fn recompute_path(&mut self, w: &World, m: &NavMob, arena: &mut Arena) {
        if m.game_time - self.time_last_recompute <= 20 || !self.can_update_path(m) {
            self.has_delayed_recomputation = true;
        } else if let Some(tp) = self.target_pos {
            self.path = None;
            let reach = self.reach_range;
            self.path = self.create_path_pos(w, m, tp, reach, arena);
            self.time_last_recompute = m.game_time;
            self.has_delayed_recomputation = false;
        }
    }

    /// `shouldRecomputePath(pos)`: a changed block near the rest of the path.
    pub fn should_recompute_path(&self, m: &NavMob, pos: BlockPos) -> bool {
        if self.has_delayed_recomputation {
            return false;
        }
        match &self.path {
            Some(p) if !p.is_done() && p.node_count() != 0 => {
                let t = p.end_node().expect("non-empty");
                let mid = Vec3::new((t.x as f64 + m.pos.x) / 2.0, (t.y as f64 + m.pos.y) / 2.0, (t.z as f64 + m.pos.z) / 2.0);
                let d = (p.node_count() - p.next_node_index) as f64;
                let c = Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
                c.distance_to_sqr(mid) < d * d
            }
            _ => false,
        }
    }

    /// `canCutCorner(type)`.
    fn can_cut_corner(t: PathType) -> bool {
        t != PathType::FireInNeighbor && t != PathType::DamagingInNeighbor && t != PathType::WalkableDoor
    }

    /// `tick()`: returns the wanted position for the move control (`setWantedPosition`), if any.
    pub fn tick(&mut self, w: &World, m: &NavMob, arena: &mut Arena) -> Option<(f64, f64, f64, f64)> {
        if self.kind == NavKind::WallClimber && self.is_done() {
            if let Some(pp) = self.path_to_position {
                let c = Vec3::new(pp.x as f64 + 0.5, pp.y as f64 + 0.5, pp.z as f64 + 0.5);
                let wd = m.width as f64;
                let near = c.distance_to_sqr(m.pos) < wd * wd;
                let above = m.pos.y > pp.y as f64 && {
                    let q = BlockPos::new(pp.x, mth::floor(m.pos.y), pp.z);
                    let qc = Vec3::new(q.x as f64 + 0.5, q.y as f64 + 0.5, q.z as f64 + 0.5);
                    qc.distance_to_sqr(m.pos) < wd * wd
                };
                if !near && !above {
                    return Some((pp.x as f64, pp.y as f64, pp.z as f64, self.speed_modifier));
                }
                self.path_to_position = None;
            }
            return None;
        }
        self.tick += 1;
        if self.has_delayed_recomputation {
            self.recompute_path(w, m, arena);
        }
        if self.is_done() {
            return None;
        }
        if self.can_update_path(m) {
            self.follow_the_path(w, m);
        } else if self.path.as_ref().is_some_and(|p| !p.is_done()) {
            let mp = self.temp_mob_pos(w, m);
            let p = self.path.as_mut().expect("checked");
            let np = p.next_entity_pos(m.width);
            if mp.y > np.y && !m.on_ground && mth::floor(mp.x) == mth::floor(np.x) && mth::floor(mp.z) == mth::floor(np.z) {
                p.advance();
            }
        }
        if self.is_done() {
            return None;
        }
        let target = self.path.as_ref().expect("not done").next_entity_pos(m.width);
        Some((target.x, self.ground_y(w, target), target.z, self.speed_modifier))
    }

    /// `getGroundY(target)` (`AmphibiousPathNavigation`, `WaterBoundPathNavigation`: the target's
    /// y).
    fn ground_y(&self, w: &World, target: Vec3) -> f64 {
        if self.kind.mid_height() {
            return target.y;
        }
        let bp = BlockPos::new(mth::floor(target.x), mth::floor(target.y), mth::floor(target.z));
        if w.get(bp.below()).is_air() { target.y } else { super::walk::Level { w }.floor_level(bp) }
    }

    /// `followThePath()`.
    fn follow_the_path(&mut self, w: &World, m: &NavMob) {
        let mp = self.temp_mob_pos(w, m);
        self.max_distance_to_waypoint = if m.width > 0.75 { m.width / 2.0 } else { 0.75 - m.width / 2.0 };
        let p = self.path.as_ref().expect("following");
        let cur = p.next_node_pos();
        let xd = (m.pos.x - (cur.x as f64 + 0.5)).abs();
        let yd = (m.pos.y - cur.y as f64).abs();
        let zd = (m.pos.z - (cur.z as f64 + 0.5)).abs();
        let mdw = self.max_distance_to_waypoint as f64;
        // getMaxVerticalDistanceToWaypoint: 1 (WaterBoundPathNavigation: 0.5)
        let max_vertical = if self.kind == NavKind::WaterBound { 0.5 } else { 1.0 };
        let close = xd < mdw && zd < mdw && yd < max_vertical;
        if close || (Self::can_cut_corner(p.next_node().ty) && self.should_target_next_node_in_direction(w, m, mp)) {
            self.path.as_mut().expect("following").advance();
        }
        self.do_stuck_detection(m, mp);
    }

    /// `shouldTargetNextNodeInDirection(mobPosition)` (`canMoveDirectly` is false on the ground;
    /// `AmphibiousPathNavigation`'s in a liquid: `isClearForMovementBetween`, a collider clip to
    /// the next entity position at half the mob's height).
    fn should_target_next_node_in_direction(&self, w: &World, m: &NavMob, mp: Vec3) -> bool {
        let p = self.path.as_ref().expect("following");
        if p.next_node_index + 1 >= p.node_count() {
            return false;
        }
        let cn = p.next_node_pos();
        let current = Vec3::new(cn.x as f64 + 0.5, cn.y as f64, cn.z as f64 + 0.5);
        if !(mp.distance_to_sqr(current) < 2.0 * 2.0) {
            return false;
        }
        // canMoveDirectly: an amphibious navigation's in a liquid, a water-bound one's always
        // (`isClearForMovementBetween(mob, from, to, false)`)
        if (self.kind == NavKind::Amphibious && m.in_liquid) || self.kind == NavKind::WaterBound {
            let stop = p.next_entity_pos(m.width);
            let to = Vec3::new(stop.x, stop.y + m.height as f64 * 0.5, stop.z);
            if crate::explosion::clip_misses(w, mp, to) {
                return true;
            }
        }
        let nn = p.nodes[p.next_node_index + 1].pos();
        let next = Vec3::new(nn.x as f64 + 0.5, nn.y as f64, nn.z as f64 + 0.5);
        let to_current = current.subtract_v(mp);
        let to_next = next.subtract_v(mp);
        let cur_sqr = to_current.length_sqr();
        let next_sqr = to_next.length_sqr();
        let closer = next_sqr < cur_sqr;
        let within = cur_sqr < 0.5;
        if !closer && !within {
            return false;
        }
        let md = to_current.normalize();
        let pd = to_next.normalize();
        pd.dot(md) < 0.0
    }

    /// `doStuckDetection(mobPos)`.
    fn do_stuck_detection(&mut self, m: &NavMob, mp: Vec3) {
        if self.tick - self.last_stuck_check > 100 {
            let eff = if m.speed >= 1.0 { m.speed } else { m.speed * m.speed };
            let threshold = eff * 100.0f32 * 0.25f32;
            if mp.distance_to_sqr(self.last_stuck_check_pos) < (threshold * threshold) as f64 {
                self.is_stuck = true;
                self.stop();
            } else {
                self.is_stuck = false;
            }
            self.last_stuck_check = self.tick;
            self.last_stuck_check_pos = mp;
        }
        if let Some(p) = &self.path
            && !p.is_done()
        {
            let pos = p.next_node_pos();
            let time = m.game_time;
            if pos == self.timeout_cached_node {
                self.timeout_timer += time - self.last_timeout_check;
            } else {
                self.timeout_cached_node = pos;
                let c = Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5);
                let dist = mp.distance_to_sqr(c).sqrt();
                self.timeout_limit = if m.speed > 0.0 { dist / m.speed as f64 * 20.0 } else { 0.0 };
            }
            if self.timeout_limit > 0.0 && self.timeout_timer as f64 > self.timeout_limit * 3.0 {
                self.reset_stuck_timeout();
                self.stop();
            }
            self.last_timeout_check = time;
        }
    }
}

/// `BlockTags.CAULDRONS` (none of redsim's own blocks; terrain states by class).
fn is_cauldron(b: &crate::block::Block) -> bool {
    match b {
        crate::block::Block::Terrain(id) => crate::terrain::blocks::props(*id).classes.iter().any(|c| c == "AbstractCauldronBlock"),
        _ => false,
    }
}
