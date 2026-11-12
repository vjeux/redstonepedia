//! Rails (clk-misc: the fluids + entities port's M4), read in the 26.3 source: `BaseRailBlock`,
//! `RailState` (for every rail kind: plain rails curve, the others are straight), `RailBlock`
//! (plain rails), `DetectorRailBlock`, and `AbstractMinecart.EXITS`. Powered and activator rails
//! keep their redstone model in `redstone::blocks::rail`; this module's `RailState` sees them as
//! rails too.

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Kind, RailShape};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::World;

/// `BaseRailBlock.isRail(state)`: powered, activator, plain and detector rails.
pub fn is_rail(b: &Block) -> bool {
    matches!(b, Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. })
}

/// The rail's shape property.
pub fn shape_of(b: &Block) -> RailShape {
    match *b {
        Block::Rail { shape, .. } | Block::PlainRail { shape } | Block::DetectorRail { shape, .. } => shape,
        _ => RailShape::NorthSouth,
    }
}

/// `BaseRailBlock.isStraight()`: every rail but the plain one.
fn is_straight(b: &Block) -> bool {
    !matches!(b, Block::PlainRail { .. })
}

fn with_shape(b: Block, shape: RailShape) -> Block {
    match b {
        Block::Rail { activator, powered, .. } => Block::Rail { activator, shape, powered },
        Block::PlainRail { .. } => Block::PlainRail { shape },
        Block::DetectorRail { powered, .. } => Block::DetectorRail { shape, powered },
        x => x,
    }
}

/// `AbstractMinecart.EXITS`: the two exits of a shape (unit steps, a slope's low end one down).
pub fn exits(s: RailShape) -> ([i32; 3], [i32; 3]) {
    match s {
        RailShape::NorthSouth => ([0, 0, -1], [0, 0, 1]),
        RailShape::EastWest => ([-1, 0, 0], [1, 0, 0]),
        RailShape::AscendingEast => ([-1, -1, 0], [1, 0, 0]),
        RailShape::AscendingWest => ([-1, 0, 0], [1, -1, 0]),
        RailShape::AscendingNorth => ([0, 0, -1], [0, -1, 1]),
        RailShape::AscendingSouth => ([0, -1, -1], [0, 0, 1]),
        RailShape::SouthEast => ([0, 0, 1], [1, 0, 0]),
        RailShape::SouthWest => ([0, 0, 1], [-1, 0, 0]),
        RailShape::NorthWest => ([0, 0, -1], [-1, 0, 0]),
        RailShape::NorthEast => ([0, 0, -1], [1, 0, 0]),
    }
}

/// `RailState.updateConnections`.
fn conns_for(pos: BlockPos, shape: RailShape) -> Vec<BlockPos> {
    let (n, s, w, e) = (pos.rel(Dir::North), pos.rel(Dir::South), pos.rel(Dir::West), pos.rel(Dir::East));
    match shape {
        RailShape::NorthSouth => vec![n, s],
        RailShape::EastWest => vec![w, e],
        RailShape::AscendingEast => vec![w, e.above()],
        RailShape::AscendingWest => vec![w.above(), e],
        RailShape::AscendingNorth => vec![n.above(), s],
        RailShape::AscendingSouth => vec![n, s.above()],
        RailShape::SouthEast => vec![e, s],
        RailShape::SouthWest => vec![w, s],
        RailShape::NorthWest => vec![w, n],
        RailShape::NorthEast => vec![e, n],
    }
}

/// `RailState`.
pub struct RailState {
    pos: BlockPos,
    state: Block,
    straight: bool,
    conns: Vec<BlockPos>,
}

fn has_rail(w: &World, p: BlockPos) -> bool {
    is_rail(&w.get(p)) || is_rail(&w.get(p.above())) || is_rail(&w.get(p.below()))
}

fn get_rail(w: &World, p: BlockPos) -> Option<RailState> {
    for q in [p, p.above(), p.below()] {
        let s = w.get(q);
        if is_rail(&s) {
            return Some(RailState::new(q, s));
        }
    }
    None
}

impl RailState {
    pub fn new(pos: BlockPos, state: Block) -> RailState {
        RailState { pos, state, straight: is_straight(&state), conns: conns_for(pos, shape_of(&state)) }
    }

    /// `getConnections`.
    pub fn connections(&self) -> &[BlockPos] {
        &self.conns
    }

    fn has_connection(&self, p: BlockPos) -> bool {
        self.conns.iter().any(|c| c.x == p.x && c.z == p.z)
    }

    fn remove_soft_connections(&mut self, w: &World) {
        let mut i = 0;
        while i < self.conns.len() {
            match get_rail(w, self.conns[i]) {
                Some(r) if r.has_connection(self.pos) => {
                    self.conns[i] = r.pos;
                    i += 1;
                }
                _ => {
                    self.conns.remove(i);
                }
            }
        }
    }

    /// `countPotentialConnections` (`Direction.Plane.HORIZONTAL`).
    pub fn count_potential_connections(&self, w: &World) -> i32 {
        Dir::HORIZONTAL.iter().filter(|d| has_rail(w, self.pos.rel(**d))).count() as i32
    }

    fn can_connect_to(&self, p: BlockPos) -> bool {
        self.has_connection(p) || self.conns.len() != 2
    }

    fn ascend(&self, w: &World, shape: Option<RailShape>) -> Option<RailShape> {
        let (n, s, wst, e) = (self.pos.rel(Dir::North), self.pos.rel(Dir::South), self.pos.rel(Dir::West), self.pos.rel(Dir::East));
        let mut shape = shape;
        if shape == Some(RailShape::NorthSouth) {
            if is_rail(&w.get(n.above())) {
                shape = Some(RailShape::AscendingNorth);
            }
            if is_rail(&w.get(s.above())) {
                shape = Some(RailShape::AscendingSouth);
            }
        }
        if shape == Some(RailShape::EastWest) {
            if is_rail(&w.get(e.above())) {
                shape = Some(RailShape::AscendingEast);
            }
            if is_rail(&w.get(wst.above())) {
                shape = Some(RailShape::AscendingWest);
            }
        }
        shape
    }

    /// `connectTo`.
    fn connect_to(&mut self, w: &mut World, p: BlockPos) {
        self.conns.push(p);
        let (n, s, wst, e) = (self.pos.rel(Dir::North), self.pos.rel(Dir::South), self.pos.rel(Dir::West), self.pos.rel(Dir::East));
        let (hn, hs, hw, he) = (self.has_connection(n), self.has_connection(s), self.has_connection(wst), self.has_connection(e));
        let mut shape = None;
        if hn || hs {
            shape = Some(RailShape::NorthSouth);
        }
        if hw || he {
            shape = Some(RailShape::EastWest);
        }
        if !self.straight {
            if hs && he && !hn && !hw {
                shape = Some(RailShape::SouthEast);
            }
            if hs && hw && !hn && !he {
                shape = Some(RailShape::SouthWest);
            }
            if hn && hw && !hs && !he {
                shape = Some(RailShape::NorthWest);
            }
            if hn && he && !hs && !hw {
                shape = Some(RailShape::NorthEast);
            }
        }
        let shape = self.ascend(w, shape).unwrap_or(RailShape::NorthSouth);
        self.state = with_shape(self.state, shape);
        w.set_block_and_update(self.pos, self.state);
    }

    fn has_neighbor_rail(&self, w: &World, p: BlockPos) -> bool {
        match get_rail(w, p) {
            Some(mut n) => {
                n.remove_soft_connections(w);
                n.can_connect_to(self.pos)
            }
            None => false,
        }
    }

    /// The shape `place` settles this rail to with `has_signal` (`default` where both axes join),
    /// nothing placed: the computation `place` runs, read alone by the junction check (a three-arm
    /// rail's two power states).
    pub fn shape_for(&self, w: &World, has_signal: bool, default: RailShape) -> RailShape {
        let (n, s, wst, e) = (self.pos.rel(Dir::North), self.pos.rel(Dir::South), self.pos.rel(Dir::West), self.pos.rel(Dir::East));
        let hn = self.has_neighbor_rail(w, n);
        let hs = self.has_neighbor_rail(w, s);
        let hw = self.has_neighbor_rail(w, wst);
        let he = self.has_neighbor_rail(w, e);
        let ns = hn || hs;
        let ew = hw || he;
        let mut shape = None;
        if ns && !ew {
            shape = Some(RailShape::NorthSouth);
        }
        if ew && !ns {
            shape = Some(RailShape::EastWest);
        }
        let (se, sw, ne, nw) = (hs && he, hs && hw, hn && he, hn && hw);
        if !self.straight {
            if se && !hn && !hw {
                shape = Some(RailShape::SouthEast);
            }
            if sw && !hn && !he {
                shape = Some(RailShape::SouthWest);
            }
            if nw && !hs && !he {
                shape = Some(RailShape::NorthWest);
            }
            if ne && !hs && !hw {
                shape = Some(RailShape::NorthEast);
            }
        }
        if shape.is_none() {
            if ns && ew {
                shape = Some(default);
            } else if ns {
                shape = Some(RailShape::NorthSouth);
            } else if ew {
                shape = Some(RailShape::EastWest);
            }
            if !self.straight {
                if has_signal {
                    if se {
                        shape = Some(RailShape::SouthEast);
                    }
                    if sw {
                        shape = Some(RailShape::SouthWest);
                    }
                    if ne {
                        shape = Some(RailShape::NorthEast);
                    }
                    if nw {
                        shape = Some(RailShape::NorthWest);
                    }
                } else {
                    if nw {
                        shape = Some(RailShape::NorthWest);
                    }
                    if ne {
                        shape = Some(RailShape::NorthEast);
                    }
                    if sw {
                        shape = Some(RailShape::SouthWest);
                    }
                    if se {
                        shape = Some(RailShape::SouthEast);
                    }
                }
            }
        }
        self.ascend(w, shape).unwrap_or(default)
    }

    /// `place(hasSignal, first, defaultShape)`: the new state.
    pub fn place(mut self, w: &mut World, has_signal: bool, first: bool, default: RailShape) -> Block {
        let shape = self.shape_for(w, has_signal, default);
        self.conns = conns_for(self.pos, shape);
        self.state = with_shape(self.state, shape);
        if first || w.get(self.pos) != self.state {
            w.set_block_and_update(self.pos, self.state);
            for i in 0..self.conns.len() {
                if let Some(mut nb) = get_rail(w, self.conns[i]) {
                    nb.remove_soft_connections(w);
                    if nb.can_connect_to(self.pos) {
                        nb.connect_to(w, self.pos);
                    }
                }
            }
        }
        self.state
    }
}

/// A plain rail joined to three or four rails, as the game settles it (ironspec Nov 11 12:50, the
/// junction check): the sides it can join, its stored shape, the shapes `updateDir` gives it with
/// no signal and with one, and whether it has a signal now. The settle order is fixed (south,
/// then east, preferred unpowered; north, then west, powered), so a part turned or mirrored keeps
/// its stored shapes but not this choice: day-1's half-turned siding T (arms E, W, N) settled
/// powered into the exit side where its design (arms W, E, S) settled into the bed line.
pub struct Junction {
    pub pos: BlockPos,
    pub arms: Vec<Dir>,
    pub stored: RailShape,
    pub unpowered: RailShape,
    pub powered: RailShape,
    pub signal: bool,
}

/// The junction at `p` ([`Junction`]), if `p` holds a plain rail with three or more rail sides.
pub fn junction_at(w: &World, p: BlockPos) -> Option<Junction> {
    let s = w.get(p);
    if !matches!(s, Block::PlainRail { .. }) {
        return None;
    }
    let stored = shape_of(&s);
    let r = RailState::new(p, s);
    let arms: Vec<Dir> = [Dir::North, Dir::South, Dir::West, Dir::East].into_iter().filter(|d| r.has_neighbor_rail(w, p.rel(*d))).collect();
    if arms.len() < 3 {
        return None;
    }
    Some(Junction { pos: p, arms, stored, unpowered: r.shape_for(w, false, stored), powered: r.shape_for(w, true, stored), signal: w.has_neighbor_signal(p) })
}

// ---------------------------------------------------------------------------- BaseRailBlock

fn rigid(w: &World, p: BlockPos) -> bool {
    bh::sturdy(w, &w.get(p), p, Dir::Up, Support::Rigid)
}

/// `canSurvive`: a rigid block below.
pub fn can_survive(w: &World, p: BlockPos) -> bool {
    rigid(w, p.below())
}

/// (farmopt, Nov 12 2026, ironspec's ruling after onechunk found ironfarm's top slopes unsupported)
/// Whether the rail at `p` would pop on its next neighbour update: `BaseRailBlock.shouldBeRemoved`
/// (no rigid top below, or an ascending rail with no rigid block on its high side). False for a
/// block that is no rail. `mctas design would-pop` lists them in design files.
pub fn would_pop(w: &World, p: BlockPos) -> bool {
    let s = w.get(p);
    is_rail(&s) && should_be_removed(w, p, shape_of(&s))
}

/// `shouldBeRemoved`.
fn should_be_removed(w: &World, p: BlockPos, shape: RailShape) -> bool {
    if !rigid(w, p.below()) {
        return true;
    }
    match shape {
        RailShape::AscendingEast => !rigid(w, p.rel(Dir::East)),
        RailShape::AscendingWest => !rigid(w, p.rel(Dir::West)),
        RailShape::AscendingNorth => !rigid(w, p.rel(Dir::North)),
        RailShape::AscendingSouth => !rigid(w, p.rel(Dir::South)),
        _ => false,
    }
}

/// `block.defaultBlockState().isSignalSource()` of a neighbor update's source.
fn kind_is_signal_source(k: Kind) -> bool {
    matches!(
        k,
        Kind::Wire
            | Kind::RedstoneBlock
            | Kind::Torch
            | Kind::WallTorch
            | Kind::Lever
            | Kind::Button(_)
            | Kind::Repeater
            | Kind::Comparator
            | Kind::Observer
            | Kind::Target
            | Kind::Sculk(_)
            | Kind::Plate(_)
            | Kind::TripwireHook
            | Kind::DetectorRail
    )
}

/// `BaseRailBlock.updateDir(level, pos, state, first)`.
fn update_dir(w: &mut World, p: BlockPos, s: Block, first: bool) -> Block {
    let signal = w.has_neighbor_signal(p);
    RailState::new(p, s).place(w, signal, first, shape_of(&s))
}

/// `BaseRailBlock.updateState(state, level, pos, movedByPiston)` (from `onPlace`).
fn update_state_placed(w: &mut World, s: Block, p: BlockPos, moved: bool) -> Block {
    let s = update_dir(w, p, s, true);
    if is_straight(&s) {
        w.neighbor_changed_full(s, p, s.kind(), moved);
    }
    s
}

/// `BaseRailBlock.neighborChanged` (plain and detector rails).
pub fn neighbor_changed(w: &mut World, s: Block, p: BlockPos, src: Kind, moved: bool) {
    if w.get(p).kind() != s.kind() {
        return;
    }
    if should_be_removed(w, p, shape_of(&s)) {
        // dropResources: its loot's stacks pop (level-random draws, seeded items)
        w.drop_resources(p, s);
        w.drops.push((p, s));
        w.remove_block(p, moved);
        return;
    }
    // `updateState(state, level, pos, block)`: plain rails reshape at a T junction when a signal
    // source changes; detector rails do nothing
    if let Block::PlainRail { .. } = s
        && kind_is_signal_source(src)
        && RailState::new(p, s).count_potential_connections(w) == 3
    {
        update_dir(w, p, s, false);
    }
}

/// `onPlace` (plain rails: `BaseRailBlock`; detector rails: then `checkPressed`).
pub fn on_place(w: &mut World, s: Block, p: BlockPos, old: Block, moved: bool) {
    if old.kind() == s.kind() {
        return;
    }
    let updated = update_state_placed(w, s, p, moved);
    if let Block::DetectorRail { .. } = updated {
        detector::check_pressed(w, p, updated);
    }
}

/// `affectNeighborsAfterRemoval`.
pub fn affect_neighbors_after_removal(w: &mut World, s: Block, p: BlockPos, moved: bool) {
    if moved {
        return;
    }
    if shape_of(&s).is_ascending() {
        w.update_neighbors_at(p.above(), s.kind());
    }
    if is_straight(&s) {
        w.update_neighbors_at(p, s.kind());
        w.update_neighbors_at(p.below(), s.kind());
    }
}

// ---------------------------------------------------------------------------- detector rails

pub mod detector {
    use super::*;

    fn powered(s: &Block) -> bool {
        matches!(*s, Block::DetectorRail { powered: true, .. })
    }

    /// `getSearchBB`: `(x + 0.2, y, z + 0.2) .. (x + 1 - 0.2, y + 1 - 0.2, z + 1 - 0.2)`.
    pub fn search_bb(p: BlockPos) -> crate::entity::Aabb {
        crate::entity::Aabb::new(p.x as f64 + 0.2, p.y as f64, p.z as f64 + 0.2, (p.x + 1) as f64 - 0.2, (p.y + 1) as f64 - 0.2, (p.z + 1) as f64 - 0.2)
    }

    /// Minecarts in the search box (`getEntitiesOfClass(AbstractMinecart.class, ...)`).
    fn minecarts(w: &World, p: BlockPos) -> usize {
        w.get_entities(search_bb(p), |e| matches!(e.kind, crate::entity::EntityKind::Minecart(_))).len()
    }

    /// `checkPressed(level, pos, state)` (`state` as passed, not re-read).
    pub fn check_pressed(w: &mut World, p: BlockPos, s: Block) {
        if !(can_survive(w, p) && w.get(p).kind() == Kind::DetectorRail) {
            return;
        }
        let Block::DetectorRail { shape, .. } = s else { return };
        let was = powered(&s);
        let should = minecarts(w, p) > 0;
        if should && !was {
            let ns = Block::DetectorRail { shape, powered: true };
            w.set_block_and_update(p, ns);
            update_power_to_connected(w, p, ns);
            w.update_neighbors_at(p, Kind::DetectorRail);
            w.update_neighbors_at(p.below(), Kind::DetectorRail);
        }
        if !should && was {
            let ns = Block::DetectorRail { shape, powered: false };
            w.set_block_and_update(p, ns);
            update_power_to_connected(w, p, ns);
            w.update_neighbors_at(p, Kind::DetectorRail);
            w.update_neighbors_at(p.below(), Kind::DetectorRail);
        }
        if should {
            w.schedule_tick(p, Kind::DetectorRail, 20, Priority::Normal);
        }
        w.update_neighbour_for_output_signal(p, Kind::DetectorRail);
    }

    /// `updatePowerToConnected`: a neighbor update of the blocks at the rail's two connections
    /// (each with itself as the source).
    fn update_power_to_connected(w: &mut World, p: BlockPos, s: Block) {
        for c in RailState::new(p, s).connections().to_vec() {
            let cs = w.get(c);
            w.neighbor_changed_full(cs, c, cs.kind(), false);
        }
    }

    /// `entityInside` (a minecart or anything else: `checkPressed` counts minecarts).
    pub fn entity_inside(w: &mut World, s: Block, p: BlockPos) {
        if !powered(&s) {
            check_pressed(w, p, s);
        }
    }

    /// `tick`.
    pub fn tick(w: &mut World, s: Block, p: BlockPos) {
        if powered(&s) {
            check_pressed(w, p, s);
        }
    }

    /// `ownSignal`.
    pub fn signal(s: &Block) -> i32 {
        if powered(s) { 15 } else { 0 }
    }

    /// `getDirectSignal`: the block below.
    pub fn direct_signal(s: &Block, d: Dir) -> i32 {
        if powered(s) && d == Dir::Up { 15 } else { 0 }
    }

    /// `getAnalogOutputSignal` (clk-misc 2026-10-21, the cart loaders): pressed, the fill level of
    /// the first container minecart in the search box (`getEntitiesOfClass` order; no command block
    /// minecarts are modeled), as a container block's (`getRedstoneSignalFromContainer`); else 0.
    pub fn analog_output(w: &World, s: &Block, p: BlockPos) -> i32 {
        if !powered(s) {
            return 0;
        }
        let carts = w.get_entities(search_bb(p), |e| matches!(e.kind, crate::entity::EntityKind::Minecart(m) if m.hopper));
        match carts.first() {
            Some(&i) => {
                let id = w.entities.list[i].id;
                w.containers.entities.get(&id).map(crate::redstone::container::signal_of).unwrap_or(0)
            }
            None => 0,
        }
    }
}
