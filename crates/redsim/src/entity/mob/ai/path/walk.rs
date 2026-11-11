//! `WalkNodeEvaluator` (26.3): the nodes a walking mob can step to, read line by line
//! (`getStart`, `getNeighbors`, `findAcceptedNode`, `tryJumpOn`, the water and ground scans,
//! `getPathTypeOfMob`, `getPathTypeWithinMobBB`, `getPathTypeStatic`, `checkNeighbourBlocks`,
//! `getFloorLevel`, `canReachWithoutCollision`). The level's blocks come from redsim's world; the
//! per-search caches (`pathTypesByPosCacheByMob`, `collisionCache`) are plain caches: the world does
//! not change during a search.

use super::node::{Arena, IntMap, NONE, NodeId};
use super::path_type::{self, Malus, PathType};
use crate::block::Block;
use crate::entity::{Aabb, Vec3, mth};
use crate::pos::{BlockPos, Dir};
use crate::world::World;
use std::collections::HashMap;

/// What a search reads of the mob (copied from it when the search starts: `Mob` getters).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PathMob {
    pub pos: Vec3,
    pub bb: Aabb,
    /// `getBbWidth()`, `getBbHeight()` (the pose's dimensions, floats).
    pub width: f32,
    pub height: f32,
    /// `maxUpStep()`.
    pub max_up_step: f32,
    pub on_ground: bool,
    /// `isInFloatableFluid()` (a water height above 0).
    pub in_floatable: bool,
    pub malus: Malus,
    /// `getMaxFallDistance()`.
    pub max_fall: i32,
}

impl PathMob {
    pub fn block_position(&self) -> BlockPos {
        BlockPos::new(mth::floor(self.pos.x), mth::floor(self.pos.y), mth::floor(self.pos.z))
    }
}

/// `NodeEvaluator`'s switches (`setCanPassDoors`, `setCanOpenDoors`, `setCanFloat`,
/// `setCanWalkOverFences`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EvalFlags {
    pub can_pass_doors: bool,
    pub can_open_doors: bool,
    pub can_float: bool,
    pub can_walk_over_fences: bool,
    /// `AmphibiousNodeEvaluator` (a drowned's `AmphibiousPathNavigation`; `prefersShallowSwimming`
    /// false): water nodes up and down, water borders, the walk costing 6.
    pub amphibious: bool,
    /// `SwimNodeEvaluator(allowBreaching false)` (a fish's `WaterBoundPathNavigation`): water
    /// cells only, the 6 directions then the horizontal diagonals ([`WalkEval::swim_neighbors`]).
    pub swim: bool,
}

impl Default for EvalFlags {
    fn default() -> EvalFlags {
        EvalFlags { can_pass_doors: true, can_open_doors: false, can_float: false, can_walk_over_fences: false, amphibious: false, swim: false }
    }
}

/// `Level.getMinY()` of the game's overworld (the sandbox levels use its dimension type).
pub const MIN_Y: i32 = -64;

/// The block facts a search reads through the level: a block state, its collision shape's top,
/// `getPathTypeFromState`.
pub struct Level<'w> {
    pub w: &'w World,
}

impl<'w> Level<'w> {
    #[inline]
    pub fn block(&self, p: BlockPos) -> Block {
        self.w.get(p)
    }

    /// `PathfindingContext.getPathTypeFromState` (the level's cache is transparent here).
    #[inline]
    pub fn type_from_state(&self, x: i32, y: i32, z: i32) -> PathType {
        path_type::from_state(&self.w.get(BlockPos::new(x, y, z)))
    }

    /// A block's collision shape top in its own cell (`VoxelShape.max(Y)`, empty context), `None`
    /// for an empty shape.
    pub fn shape_top(&self, p: BlockPos) -> Option<f64> {
        let s = self.w.get(p);
        if s.is_air() {
            return None;
        }
        local_shape_top(self.w, &s, p)
    }

    /// `WalkNodeEvaluator.getFloorLevel(level, pos)`.
    pub fn floor_level(&self, pos: BlockPos) -> f64 {
        let target = pos.below();
        target.y as f64 + self.shape_top(target).unwrap_or(0.0)
    }

    /// Water at `p` (`FluidTags.WATER`, = `ENTITY_FLOATABLE`).
    pub fn is_water(&self, p: BlockPos) -> bool {
        crate::fluid::fluid_state(self.w, p).fluid.is_water()
    }

    /// `BlockState.isPathfindable(PathComputationType.WATER)` of the block at `p`: a terrain state's
    /// from the game's table; redsim's own blocks: `BlockBehaviour`'s default (its fluid is water),
    /// a door or fence gate never, a trapdoor when waterlogged.
    pub fn pathfindable_water(&self, p: BlockPos) -> bool {
        match self.w.get(p) {
            Block::Terrain(id) => crate::terrain::blocks::props(id).path[1],
            Block::Door { .. } | Block::FenceGate { .. } => false,
            _ => self.is_water(p),
        }
    }

    /// `!noCollision(mob, box)` through a `PathNavigationRegion` (no entity collisions).
    pub fn has_collisions(&self, bb: Aabb) -> bool {
        crate::entity::collide::collides_with_blocks(self.w, bb)
    }
}

/// The top of a block's collision shape, in its own cell coordinates.
pub fn local_shape_top(w: &World, s: &Block, p: BlockPos) -> Option<f64> {
    match s {
        Block::Terrain(id) => crate::terrain::blocks::props(*id).boxes.iter().map(|b| b[4]).reduce(f64::max),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } | Block::Bamboo { .. } => {
            crate::entity::collide::boxes_at(w, s, p).iter().map(|b| b.max_y - p.y as f64).reduce(f64::max)
        }
        _ => crate::behavior::collision_boxes(s).iter().map(|c| c[3] as f64 / 16.0).reduce(f64::max),
    }
}

/// `WalkNodeEvaluator` for one search.
pub struct WalkEval<'a, 'w> {
    pub level: Level<'w>,
    pub mob: PathMob,
    pub flags: EvalFlags,
    pub arena: &'a mut Arena,
    entity_width: i32,
    entity_height: i32,
    entity_depth: i32,
    type_cache: IntMap<i64, PathType>,
    collision_cache: HashMap<[u64; 6], bool>,
    reusable_neighbors: [NodeId; 4],
}

/// `BlockPos.asLong` (26 / 12 / 26 bits).
#[inline]
pub fn as_long(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FF_FFFF) << 38) | ((z as i64 & 0x3FF_FFFF) << 12) | (y as i64 & 0xFFF)
}

/// `Direction.get2DDataValue` of a horizontal direction (S 0, W 1, N 2, E 3).
fn data2d(d: Dir) -> usize {
    match d {
        Dir::South => 0,
        Dir::West => 1,
        Dir::North => 2,
        Dir::East => 3,
        _ => unreachable!(),
    }
}

fn partial_collision(t: PathType) -> bool {
    matches!(t, PathType::Fence | PathType::DoorWoodClosed | PathType::DoorIronClosed)
}

impl<'a, 'w> WalkEval<'a, 'w> {
    /// `prepare(level, mob)`.
    pub fn new(w: &'w World, mob: PathMob, flags: EvalFlags, arena: &'a mut Arena) -> WalkEval<'a, 'w> {
        arena.clear();
        let entity_width = mth::floor((mob.width + 1.0f32) as f64);
        let entity_height = mth::floor((mob.height + 1.0f32) as f64);
        WalkEval {
            level: Level { w },
            mob,
            flags,
            arena,
            entity_width,
            entity_height,
            entity_depth: entity_width,
            type_cache: IntMap::default(),
            collision_cache: HashMap::new(),
            reusable_neighbors: [NONE; 4],
        }
    }

    fn malus(&self, t: PathType) -> f32 {
        // AmphibiousNodeEvaluator.prepare: WATER 0, WALKABLE 6, WATER_BORDER 4 for the search
        if self.flags.amphibious {
            match t {
                PathType::Water => return 0.0,
                PathType::Walkable => return 6.0,
                PathType::WaterBorder => return 4.0,
                _ => {}
            }
        }
        self.mob.malus.get(t)
    }

    // ------------------------------------------------------------------- path types

    /// `getPathType(context, x, y, z)`: `getPathTypeStatic`; amphibious, water beside a blocked
    /// cell (any of the 6 directions) is a water border.
    fn path_type(&self, x: i32, y: i32, z: i32) -> PathType {
        if self.flags.amphibious && self.level.type_from_state(x, y, z) == PathType::Water {
            for (dx, dy, dz) in [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)] {
                if self.level.type_from_state(x + dx, y + dy, z + dz) == PathType::Blocked {
                    return PathType::WaterBorder;
                }
            }
            return PathType::Water;
        }
        self.path_type_static(x, y, z)
    }

    /// `getPathTypeStatic(context, pos)`.
    pub fn path_type_static(&self, x: i32, y: i32, z: i32) -> PathType {
        let t = self.level.type_from_state(x, y, z);
        if t == PathType::Open && y >= MIN_Y + 1 {
            match self.level.type_from_state(x, y - 1, z) {
                PathType::Open | PathType::Water | PathType::Lava | PathType::Walkable => PathType::Open,
                PathType::Fire => PathType::Fire,
                PathType::Damaging => PathType::Damaging,
                PathType::StickyHoney => PathType::StickyHoney,
                PathType::PowderSnow => PathType::OnTopOfPowderSnow,
                PathType::DamageCautious => PathType::DamageCautious,
                PathType::Trapdoor => PathType::OnTopOfTrapdoor,
                _ => self.check_neighbour_blocks(x, y, z, PathType::Walkable),
            }
        } else {
            t
        }
    }

    /// `checkNeighbourBlocks(context, x, y, z, type)`.
    fn check_neighbour_blocks(&self, x: i32, y: i32, z: i32, t: PathType) -> PathType {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if dx != 0 || dz != 0 {
                        match self.level.type_from_state(x + dx, y + dy, z + dz) {
                            PathType::Damaging => return PathType::DamagingInNeighbor,
                            PathType::Fire | PathType::Lava => return PathType::FireInNeighbor,
                            PathType::Water => return PathType::WaterBorder,
                            PathType::DamageCautious => return PathType::DamageCautious,
                            _ => {}
                        }
                    }
                }
            }
        }
        t
    }

    /// `getPathTypeWithinMobBB`: the set of types (a bitset in enum order).
    fn path_types_within_mob_bb(&self, x: i32, y: i32, z: i32) -> u32 {
        let mut set = 0u32;
        let mp = self.mob.block_position();
        for dx in 0..self.entity_width {
            for dy in 0..self.entity_height {
                for dz in 0..self.entity_depth {
                    let mut t = self.path_type(x + dx, y + dy, z + dz);
                    let can_pass = self.flags.can_pass_doors;
                    if t == PathType::DoorWoodClosed && self.flags.can_open_doors && can_pass {
                        t = PathType::WalkableDoor;
                    }
                    if t == PathType::DoorOpen && !can_pass {
                        t = PathType::Blocked;
                    }
                    if t == PathType::Rail && self.path_type(mp.x, mp.y, mp.z) != PathType::Rail && self.path_type(mp.x, mp.y - 1, mp.z) != PathType::Rail {
                        t = PathType::UnpassableRail;
                    }
                    set |= 1 << t as u32;
                }
            }
        }
        set
    }

    /// `getPathTypeOfMob(context, x, y, z, mob)`.
    pub fn path_type_of_mob(&self, x: i32, y: i32, z: i32) -> PathType {
        let set = self.path_types_within_mob_bb(x, y, z);
        if set.count_ones() == 1 {
            return PathType::ALL[set.trailing_zeros() as usize];
        }
        if set & (1 << PathType::Fence as u32) != 0 {
            return PathType::Fence;
        }
        if set & (1 << PathType::UnpassableRail as u32) != 0 {
            return PathType::UnpassableRail;
        }
        let mut highest_type = PathType::Blocked;
        let mut highest = self.malus(highest_type);
        for t in PathType::ALL {
            if set & (1 << t as u32) == 0 {
                continue;
            }
            let m = self.malus(t);
            if m < 0.0 {
                return t;
            }
            if m >= highest {
                highest = m;
                highest_type = t;
            }
        }
        let current = self.path_type(x, y, z);
        if self.entity_width > 1 {
            let cheaper = self.malus(current) < highest;
            if cheaper && self.malus(PathType::BigMobsCloseToDanger) < highest { PathType::BigMobsCloseToDanger } else { highest_type }
        } else if current == PathType::Open && highest_type != PathType::Open && highest == 0.0 {
            PathType::Open
        } else {
            highest_type
        }
    }

    /// `getCachedPathType(x, y, z)`.
    pub fn cached_type(&mut self, x: i32, y: i32, z: i32) -> PathType {
        let k = as_long(x, y, z);
        if let Some(t) = self.type_cache.get(&k) {
            return *t;
        }
        let t = self.path_type_of_mob(x, y, z);
        self.type_cache.insert(k, t);
        t
    }

    // ------------------------------------------------------------------- the floor and boxes

    /// `getFloorLevel(pos)` (the instance one: half a block up in water for a floating mob).
    fn floor_level(&self, pos: BlockPos) -> f64 {
        if (self.flags.can_float || self.flags.amphibious) && self.level.is_water(pos) { pos.y as f64 + 0.5 } else { self.level.floor_level(pos) }
    }

    /// `hasCollisions(box)` (cached per box).
    fn has_collisions(&mut self, bb: Aabb) -> bool {
        let k = [bb.min_x.to_bits(), bb.min_y.to_bits(), bb.min_z.to_bits(), bb.max_x.to_bits(), bb.max_y.to_bits(), bb.max_z.to_bits()];
        if let Some(v) = self.collision_cache.get(&k) {
            return *v;
        }
        let v = self.level.has_collisions(bb);
        self.collision_cache.insert(k, v);
        v
    }

    /// `canReachWithoutCollision(posTo)`: the mob's box swept from where it is to the node.
    fn can_reach_without_collision(&mut self, id: NodeId) -> bool {
        let n = *self.arena.n(id);
        let mut bb = self.mob.bb;
        let mut delta = Vec3::new(n.x as f64 - self.mob.pos.x + bb.x_size() / 2.0, n.y as f64 - self.mob.pos.y + bb.y_size() / 2.0, n.z as f64 - self.mob.pos.z + bb.z_size() / 2.0);
        let size = (bb.x_size() + bb.y_size() + bb.z_size()) / 3.0;
        let steps = (delta.length() / size).ceil() as i32;
        delta = delta.scale((1.0f32 / steps as f32) as f64);
        for _ in 1..=steps {
            bb = bb.move_v(delta);
            if self.has_collisions(bb) {
                return false;
            }
        }
        true
    }

    // ------------------------------------------------------------------- nodes

    fn node_and_update_cost_to_max(&mut self, x: i32, y: i32, z: i32, t: PathType, cost: f32) -> NodeId {
        let id = self.arena.get(x, y, z);
        let n = self.arena.n_mut(id);
        n.ty = t;
        n.cost_malus = n.cost_malus.max(cost);
        id
    }

    fn blocked_node(&mut self, x: i32, y: i32, z: i32) -> NodeId {
        let id = self.arena.get(x, y, z);
        let n = self.arena.n_mut(id);
        n.ty = PathType::Blocked;
        n.cost_malus = -1.0;
        id
    }

    fn closed_node(&mut self, x: i32, y: i32, z: i32, t: PathType) -> NodeId {
        let id = self.arena.get(x, y, z);
        let n = self.arena.n_mut(id);
        n.closed = true;
        n.ty = t;
        n.cost_malus = t.default_malus();
        id
    }

    fn start_node(&mut self, p: BlockPos) -> NodeId {
        let id = self.arena.get(p.x, p.y, p.z);
        let t = self.cached_type(p.x, p.y, p.z);
        let m = self.malus(t);
        let n = self.arena.n_mut(id);
        n.ty = t;
        n.cost_malus = m;
        id
    }

    fn can_start_at(&mut self, p: BlockPos) -> bool {
        let t = self.cached_type(p.x, p.y, p.z);
        t != PathType::Open && self.malus(t) >= 0.0
    }

    /// `getStart()`.
    pub fn start(&mut self) -> Option<NodeId> {
        // SwimNodeEvaluator.getStart: the plain node (`getNode`: its type and malus untouched) at
        // the box's min corner, half a block up
        if self.flags.swim {
            let bb = self.mob.bb;
            return Some(self.arena.get(mth::floor(bb.min_x), mth::floor(bb.min_y + 0.5), mth::floor(bb.min_z)));
        }
        // AmphibiousNodeEvaluator.getStart: in water, the node at the box's min corner, half a
        // block up
        if self.flags.amphibious && self.mob.in_floatable {
            let bb = self.mob.bb;
            return Some(self.start_node(BlockPos::new(mth::floor(bb.min_x), mth::floor(bb.min_y + 0.5), mth::floor(bb.min_z))));
        }
        let (mx, my, mz) = (self.mob.pos.x, self.mob.pos.y, self.mob.pos.z);
        let (bx, bz) = (mth::floor(mx), mth::floor(mz));
        let mut start_y = mth::floor(my);
        // canStandOnFluid is false for every modeled mob
        if self.flags.can_float && self.mob.in_floatable {
            let mut y = start_y;
            while self.level.is_water(BlockPos::new(bx, y, bz)) {
                y += 1;
            }
            start_y = y - 1;
        } else if self.mob.on_ground {
            start_y = mth::floor(my + 0.5);
        } else {
            let mut ry = mth::floor(my + 1.0);
            while ry > MIN_Y {
                start_y = ry;
                ry -= 1;
                let below = self.level.block(BlockPos::new(bx, ry, bz));
                if !below.is_air() && !path_type::pathfindable_land(&below) {
                    break;
                }
            }
        }
        let sp = self.mob.block_position();
        if !self.can_start_at(BlockPos::new(sp.x, start_y, sp.z)) {
            let bb = self.mob.bb;
            for (x, z) in [(bb.min_x, bb.min_z), (bb.min_x, bb.max_z), (bb.max_x, bb.min_z), (bb.max_x, bb.max_z)] {
                let p = BlockPos::new(mth::floor(x), start_y, mth::floor(z));
                if self.can_start_at(p) {
                    return Some(self.start_node(p));
                }
            }
        }
        Some(self.start_node(BlockPos::new(sp.x, start_y, sp.z)))
    }

    /// `getNeighbors(neighbors, pos)`: the accepted nodes around `pos`, in the game's order.
    pub fn neighbors(&mut self, out: &mut Vec<NodeId>, pos: NodeId) {
        if self.flags.swim {
            return self.swim_neighbors(out, pos);
        }
        out.clear();
        let p = *self.arena.n(pos);
        let mut jump_size = 0;
        let above = self.cached_type(p.x, p.y + 1, p.z);
        let current = self.cached_type(p.x, p.y, p.z);
        if self.malus(above) >= 0.0 && current != PathType::StickyHoney {
            jump_size = mth::floor(1.0f32.max(self.mob.max_up_step) as f64);
        }
        let pos_height = self.floor_level(BlockPos::new(p.x, p.y, p.z));
        for d in Dir::HORIZONTAL {
            let [sx, _, sz] = d.step();
            let node = self.find_accepted_node(p.x + sx, p.y, p.z + sz, jump_size, pos_height, d, current);
            self.reusable_neighbors[data2d(d)] = node.unwrap_or(NONE);
            if let Some(n) = node
                && self.is_neighbor_valid(n, pos)
            {
                out.push(n);
            }
        }
        for d in Dir::HORIZONTAL {
            let second = d.clockwise();
            let (a, b) = (self.reusable_neighbors[data2d(d)], self.reusable_neighbors[data2d(second)]);
            if self.is_diagonal_valid3(pos, a, b) {
                let [sx, _, sz] = d.step();
                let [tx, _, tz] = second.step();
                let diag = self.find_accepted_node(p.x + sx + tx, p.y, p.z + sz + tz, jump_size, pos_height, d, current);
                if let Some(dn) = diag
                    && self.is_diagonal_valid(dn)
                {
                    out.push(dn);
                }
            }
        }
        // AmphibiousNodeEvaluator.getNeighbors: then the water nodes above and below
        if self.flags.amphibious {
            let up = self.find_accepted_node(p.x, p.y + 1, p.z, 0.max(jump_size - 1), pos_height, Dir::Up, current);
            let down = self.find_accepted_node(p.x, p.y - 1, p.z, jump_size, pos_height, Dir::Down, current);
            if let Some(u) = up
                && self.is_neighbor_valid(u, pos)
                && self.arena.n(u).ty == PathType::Water
            {
                out.push(u);
            }
            if let Some(d) = down
                && self.is_neighbor_valid(d, pos)
                && self.arena.n(d).ty == PathType::Water
                && current != PathType::Trapdoor
            {
                out.push(d);
            }
        }
    }

    fn is_neighbor_valid(&self, n: NodeId, current: NodeId) -> bool {
        let a = self.arena.n(n);
        !a.closed && (a.cost_malus >= 0.0 || self.arena.n(current).cost_malus < 0.0)
    }

    /// `isDiagonalValid(pos, ew, ns)`.
    fn is_diagonal_valid3(&self, pos: NodeId, ew: NodeId, ns: NodeId) -> bool {
        if ns == NONE || ew == NONE {
            return false;
        }
        let (p, ew, ns) = (self.arena.n(pos), self.arena.n(ew), self.arena.n(ns));
        if ns.y > p.y || ew.y > p.y {
            return false;
        }
        if ew.ty == PathType::WalkableDoor || ns.ty == PathType::WalkableDoor {
            return false;
        }
        if self.mob.width > 1.0 && (ew.cost_malus > 0.0 || ns.cost_malus > 0.0) {
            return false;
        }
        let between_posts = ns.ty == PathType::Fence && ew.ty == PathType::Fence && (self.mob.width as f64) < 0.5;
        (ns.y < p.y || ns.cost_malus >= 0.0 || between_posts) && (ew.y < p.y || ew.cost_malus >= 0.0 || between_posts)
    }

    /// `isDiagonalValid(diagonal)`.
    fn is_diagonal_valid(&self, d: NodeId) -> bool {
        let n = self.arena.n(d);
        if n.closed {
            return false;
        }
        if n.ty == PathType::WalkableDoor { false } else { n.cost_malus >= 0.0 }
    }

    fn mob_jump_height(&self) -> f64 {
        1.125f64.max(self.mob.max_up_step as f64)
    }

    /// `findAcceptedNode(x, y, z, jumpSize, nodeHeight, travelDirection, blockPathTypeCurrent)`.
    #[allow(clippy::too_many_arguments)]
    fn find_accepted_node(&mut self, x: i32, y: i32, z: i32, jump_size: i32, node_height: f64, dir: Dir, current: PathType) -> Option<NodeId> {
        let mut best: Option<NodeId> = None;
        let max_y_target = self.floor_level(BlockPos::new(x, y, z));
        if max_y_target - node_height > self.mob_jump_height() {
            return None;
        }
        let t = self.cached_type(x, y, z);
        let cost = self.malus(t);
        if cost >= 0.0 {
            best = Some(self.node_and_update_cost_to_max(x, y, z, t, cost));
        }
        if partial_collision(current)
            && let Some(b) = best
            && self.arena.n(b).cost_malus >= 0.0
            && !self.can_reach_without_collision(b)
        {
            best = None;
        }
        if t != PathType::Walkable && !(self.flags.amphibious && t == PathType::Water) {
            let best_bad = best.is_none_or(|b| self.arena.n(b).cost_malus < 0.0);
            if best_bad && jump_size > 0 && (t != PathType::Fence || self.flags.can_walk_over_fences) && t != PathType::UnpassableRail && t != PathType::Trapdoor && t != PathType::PowderSnow {
                best = self.try_jump_on(x, y, z, jump_size, node_height, dir, current);
            } else if !self.flags.amphibious && t == PathType::Water && !self.flags.can_float {
                best = self.try_find_first_non_water_below(x, y, z, best);
            } else if t == PathType::Open {
                best = Some(self.try_find_first_ground_node_below(x, y, z));
            } else if partial_collision(t) && best.is_none() {
                best = Some(self.closed_node(x, y, z, t));
            }
        }
        best
    }

    /// `tryJumpOn`.
    #[allow(clippy::too_many_arguments)]
    fn try_jump_on(&mut self, x: i32, y: i32, z: i32, jump_size: i32, node_height: f64, dir: Dir, current: PathType) -> Option<NodeId> {
        let above = self.find_accepted_node(x, y + 1, z, jump_size - 1, node_height, dir, current)?;
        if self.mob.width >= 1.0 {
            return Some(above);
        }
        let a = *self.arena.n(above);
        if a.ty != PathType::Open && a.ty != PathType::Walkable {
            return Some(above);
        }
        let [sx, _, sz] = dir.step();
        let cx = (x - sx) as f64 + 0.5;
        let cz = (z - sz) as f64 + 0.5;
        let half = self.mob.width as f64 / 2.0;
        let floor_here = self.floor_level(BlockPos::new(mth::floor(cx), y + 1, mth::floor(cz)));
        let floor_above = self.floor_level(BlockPos::new(a.x, a.y, a.z));
        let grow = Aabb::new(cx - half, floor_here + 0.001, cz - half, cx + half, self.mob.height as f64 + floor_above - 0.002, cz + half);
        if self.has_collisions(grow) { None } else { Some(above) }
    }

    /// `tryFindFirstNonWaterBelow`.
    fn try_find_first_non_water_below(&mut self, x: i32, mut y: i32, z: i32, mut best: Option<NodeId>) -> Option<NodeId> {
        y -= 1;
        while y > MIN_Y {
            let t = self.cached_type(x, y, z);
            if t != PathType::Water {
                return best;
            }
            let m = self.malus(t);
            best = Some(self.node_and_update_cost_to_max(x, y, z, t, m));
            y -= 1;
        }
        best
    }

    /// `tryFindFirstGroundNodeBelow`.
    fn try_find_first_ground_node_below(&mut self, x: i32, y: i32, z: i32) -> NodeId {
        let mut cy = y - 1;
        while cy >= MIN_Y {
            if y - cy > self.mob.max_fall {
                return self.blocked_node(x, cy, z);
            }
            let t = self.cached_type(x, cy, z);
            let cost = self.malus(t);
            if t != PathType::Open {
                if cost >= 0.0 {
                    return self.node_and_update_cost_to_max(x, cy, z, t, cost);
                }
                return self.blocked_node(x, cy, z);
            }
            cy -= 1;
        }
        self.blocked_node(x, y, z)
    }

    /// `getTarget(x, y, z)` = `getTargetNodeAt`: the node at the floored position is created in the
    /// map (its own fields untouched), the target is a new node there.
    pub fn target(&mut self, x: f64, y: f64, z: f64) -> super::node::Target {
        // AmphibiousNodeEvaluator.getTarget: half a block up
        let y = if self.flags.amphibious { y + 0.5 } else { y };
        let (bx, by, bz) = (mth::floor(x), mth::floor(y), mth::floor(z));
        self.arena.get(bx, by, bz);
        super::node::Target::new(bx, by, bz)
    }

    // ------------------------------------------------------------------- SwimNodeEvaluator

    /// `SwimNodeEvaluator.getNeighbors`: `findAcceptedNode` in `Direction.values()` order (D, U, N,
    /// S, W, E), each kept when it is not closed; then each horizontal direction (N, E, S, W) and
    /// its clockwise one, both with a malus of 0 or more, give their diagonal at the same height.
    fn swim_neighbors(&mut self, out: &mut Vec<NodeId>, pos: NodeId) {
        out.clear();
        let p = *self.arena.n(pos);
        let mut by_dir = [NONE; 6];
        for (k, d) in Dir::ALL.iter().enumerate() {
            let [sx, sy, sz] = d.step();
            let node = self.swim_find_accepted_node(p.x + sx, p.y + sy, p.z + sz);
            by_dir[k] = node.unwrap_or(NONE);
            if let Some(n) = node
                && !self.arena.n(n).closed
            {
                out.push(n);
            }
        }
        let has_malus = |a: &Arena, id: NodeId| id != NONE && a.n(id).cost_malus >= 0.0;
        let index = |d: Dir| Dir::ALL.iter().position(|&e| e == d).expect("a direction");
        for d in Dir::HORIZONTAL {
            let second = d.clockwise();
            if has_malus(self.arena, by_dir[index(d)]) && has_malus(self.arena, by_dir[index(second)]) {
                let [sx, _, sz] = d.step();
                let [tx, _, tz] = second.step();
                if let Some(n) = self.swim_find_accepted_node(p.x + sx + tx, p.y, p.z + sz + tz)
                    && !self.arena.n(n).closed
                {
                    out.push(n);
                }
            }
        }
    }

    /// `SwimNodeEvaluator.findAcceptedNode` (no breaching): a WATER cell whose malus is 0 or more
    /// is a node (its malus the larger of its own and that one), 8 more where the cell holds no
    /// fluid.
    fn swim_find_accepted_node(&mut self, x: i32, y: i32, z: i32) -> Option<NodeId> {
        let t = self.swim_cached_type(x, y, z);
        if t != PathType::Water {
            return None;
        }
        let cost = self.malus(t);
        if cost < 0.0 {
            return None;
        }
        let id = self.node_and_update_cost_to_max(x, y, z, t, cost);
        if crate::fluid::fluid_state(self.level.w, BlockPos::new(x, y, z)).is_empty() {
            self.arena.n_mut(id).cost_malus += 8.0;
        }
        Some(id)
    }

    /// `SwimNodeEvaluator.getCachedBlockType` (its own cache by position).
    fn swim_cached_type(&mut self, x: i32, y: i32, z: i32) -> PathType {
        let k = as_long(x, y, z);
        if let Some(t) = self.type_cache.get(&k) {
            return *t;
        }
        let t = self.swim_path_type_of_mob(x, y, z);
        self.type_cache.insert(k, t);
        t
    }

    /// `SwimNodeEvaluator.getPathTypeOfMob`: over the mob's cells, air above a block water could
    /// path through is BREACH, a cell without water BLOCKED; then the last cell's
    /// `isPathfindable(WATER)` decides WATER or BLOCKED.
    pub fn swim_path_type_of_mob(&self, x: i32, y: i32, z: i32) -> PathType {
        let mut last = BlockPos::new(x, y, z);
        for xx in x..x + self.entity_width {
            for yy in y..y + self.entity_height {
                for zz in z..z + self.entity_depth {
                    let p = BlockPos::new(xx, yy, zz);
                    last = p;
                    let fluid = crate::fluid::fluid_state(self.level.w, p);
                    if fluid.is_empty() && self.level.pathfindable_water(p.below()) && self.level.block(p).is_air() {
                        return PathType::Breach;
                    }
                    if !fluid.fluid.is_water() {
                        return PathType::Blocked;
                    }
                }
            }
        }
        if self.level.pathfindable_water(last) { PathType::Water } else { PathType::Blocked }
    }
}
