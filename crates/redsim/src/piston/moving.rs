//! `PistonMovingBlockEntity`, and the block-entity bookkeeping of `LevelChunk` / `Level` for
//! it: the block-entity map (pos -> entity), the ticker wrappers (`tickersInLevel`: a new entity
//! at a position whose wrapper is still bound REBINDS it, keeping its slot), and the ticker list
//! (`blockEntityTickers` + `pendingBlockEntityTickers`) whose order is the tick order of the
//! moving blocks.

use crate::behavior as bh;
use crate::block::Block;
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// One moving block. Progress is kept in halves: 0, 1 (= 0.5), 2 (= 1.0).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MovingBe {
    /// `movedState`.
    pub moved: Block,
    /// `direction`: the facing of the piston that moves it.
    pub direction: Dir,
    pub extending: bool,
    /// `isSourcePiston`: the piston's own head (extending) or base (retracting).
    pub source: bool,
    pub progress: u8,
    /// `progressO` (what `getProgress(0)` returns; the bridge reports it as `progress`).
    pub progress_o: u8,
    pub last_ticked: i64,
}

impl MovingBe {
    pub fn new(moved: Block, direction: Dir, extending: bool, source: bool) -> MovingBe {
        MovingBe { moved, direction, extending, source, progress: 0, progress_o: 0, last_ticked: 0 }
    }
    /// `getPushDirection` / `getMovementDirection`.
    pub fn push_direction(&self) -> Dir {
        if self.extending { self.direction } else { self.direction.opposite() }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct State {
    /// `LevelChunk.blockEntities`: live entities with their ids.
    bes: Vec<(BlockPos, u32, MovingBe)>,
    /// Wrapper id -> the entity id it is bound to (None = `NULL_TICKER`, removed).
    wrappers: Vec<Option<u32>>,
    /// `tickersInLevel`: pos -> wrapper id.
    wrapper_at: Vec<(BlockPos, u32)>,
    /// `Level.blockEntityTickers` (wrapper ids in tick order).
    tickers: Vec<u32>,
    /// `Level.pendingBlockEntityTickers`.
    pending: Vec<u32>,
    /// `Level.tickingBlockEntities`.
    ticking: bool,
    next_id: u32,
    /// Ideal inputs (mechanism search, `door fullmech`): tracked pistons by id, their base cell
    /// (it follows the piston when another piston moves it) and whether their input is on.
    pub tracked: Vec<(BlockPos, bool)>,
    /// Where each input's source sits while it is on: the cell of the piston when the input was
    /// switched on (a redstone block right behind it, out of the plane, as `door ideal` plays
    /// it). It powers a piston in that cell, and by quasi-connectivity the piston right below
    /// (whose cell above it is), never anything else; it does not follow a carried piston.
    pub ideal_src: Vec<Option<BlockPos>>,
}

impl State {
    /// The live moving blocks in ticker order (pending tickers are appended to the list at the
    /// next block-entity phase, so the concatenation is the order they will tick in), their last
    /// tick relative to `now`, and the ideal-input bookkeeping, for
    /// [`crate::world::World::state_key`]. Wrapper and entity ids only name entities: left out.
    pub(crate) fn key_into<H: std::hash::Hasher>(&self, now: i64, h: &mut H) {
        use std::hash::Hash;
        for wid in self.tickers.iter().chain(self.pending.iter()) {
            if let Some(id) = self.wrappers[*wid as usize].filter(|id| self.is_live(*id)) {
                let (p, _, be) = self.bes.iter().find(|(_, i, _)| *i == id).unwrap();
                (p, be.moved, be.direction, be.extending, be.source, be.progress, be.progress_o, now - be.last_ticked).hash(h);
            }
        }
        0xfeu8.hash(h);
        // Entities without a live ticker (none in practice) and the wrapper positions still bound.
        let mut rest: Vec<_> = self.bes.iter().map(|(p, _, be)| (*p, be.moved, be.direction, be.extending, be.source, be.progress, be.progress_o)).collect();
        rest.sort_by_key(|r| r.0);
        rest.hash(h);
        let mut bound: Vec<BlockPos> = self.wrapper_at.iter().filter(|(_, w)| self.wrappers[*w as usize].is_some()).map(|(p, _)| *p).collect();
        bound.sort();
        bound.hash(h);
        self.tracked.hash(h);
        self.ideal_src.hash(h);
    }

    pub fn be(&self, p: BlockPos) -> Option<&MovingBe> {
        self.bes.iter().find(|(q, _, _)| *q == p).map(|(_, _, b)| b)
    }
    fn be_by_id_mut(&mut self, id: u32) -> Option<&mut MovingBe> {
        self.bes.iter_mut().find(|(_, i, _)| *i == id).map(|(_, _, b)| b)
    }
    fn is_live(&self, id: u32) -> bool {
        self.bes.iter().any(|(_, i, _)| *i == id)
    }
    /// Every live moving block with its position.
    pub fn all(&self) -> impl Iterator<Item = (BlockPos, &MovingBe)> {
        self.bes.iter().map(|(p, _, b)| (*p, b))
    }
    /// Tickers that are not removed (`Sandbox.movingPistonCount`).
    pub fn live_count(&self) -> usize {
        self.tickers
            .iter()
            .chain(self.pending.iter())
            .filter(|w| self.wrappers[**w as usize].is_some_and(|id| self.is_live(id)))
            .count()
    }
    pub fn ideal_powered(&self, p: BlockPos) -> bool {
        let above = p.above();
        self.ideal_src.iter().flatten().any(|s| *s == p || *s == above)
    }

    /// Pistons moved by a piston action keep their identity (and input): `moved` lists the old
    /// cells of moved pistons, `dir` the motion.
    pub fn follow(&mut self, moved: &[BlockPos], dir: crate::pos::Dir) {
        if self.tracked.is_empty() || moved.is_empty() {
            return;
        }
        for t in self.tracked.iter_mut() {
            if moved.contains(&t.0) {
                t.0 = t.0.rel(dir);
            }
        }
    }

    /// `LevelChunk.removeBlockEntity`: the entity at `p` is dropped (setRemoved) and its wrapper
    /// rebound to the null ticker.
    pub fn remove(&mut self, p: BlockPos) {
        self.bes.retain(|(q, _, _)| *q != p);
        if let Some(i) = self.wrapper_at.iter().position(|(q, _)| *q == p) {
            let (_, w) = self.wrapper_at.remove(i);
            self.wrappers[w as usize] = None;
        }
    }

    /// (tickexact) A chunk reload (`crate::reload`): the moving blocks in reloaded cells leave with
    /// their chunk (`clearAllBlockEntities`: their wrappers rebound to the null ticker) and come
    /// back as new block entities in `order` (`registerAllBlockEntitiesAfterLevelLoad`), each
    /// loaded as saved: `saveAdditional` writes `progressO` as "progress" and `loadAdditional`
    /// sets both from it (the half step of the last tick is lost); `lastTicked` is not saved (0).
    pub fn reload(&mut self, reloaded: &dyn Fn(BlockPos) -> bool, order: &[BlockPos]) {
        let mut saved: Vec<(BlockPos, MovingBe)> = Vec::new();
        self.bes.retain(|(p, _, be)| {
            if reloaded(*p) {
                saved.push((*p, *be));
                false
            } else {
                true
            }
        });
        let wrappers = &mut self.wrappers;
        self.wrapper_at.retain(|(p, w)| {
            if reloaded(*p) {
                wrappers[*w as usize] = None;
                false
            } else {
                true
            }
        });
        for p in order {
            if let Some(&(_, be)) = saved.iter().find(|(q, _)| q == p) {
                self.add(*p, MovingBe { progress: be.progress_o, progress_o: be.progress_o, last_ticked: 0, ..be });
            }
        }
    }

    /// `LevelChunk.addAndRegisterBlockEntity` (the cell already holds a moving piston).
    fn add(&mut self, p: BlockPos, be: MovingBe) {
        let id = self.next_id;
        self.next_id += 1;
        // blockEntities.put: a previous entry at p is setRemoved
        self.bes.retain(|(q, _, _)| *q != p);
        self.bes.push((p, id, be));
        // updateBlockEntityTicker: rebind an existing wrapper, or append a new one
        if let Some(&(_, w)) = self.wrapper_at.iter().find(|(q, _)| *q == p) {
            self.wrappers[w as usize] = Some(id);
        } else {
            let w = self.wrappers.len() as u32;
            self.wrappers.push(Some(id));
            self.wrapper_at.push((p, w));
            if self.ticking { self.pending.push(w) } else { self.tickers.push(w) }
        }
    }
}

/// `Level.setBlockEntity(MovingPistonBlock.newMovingBlockEntity(..))`.
pub fn set_block_entity(w: &mut World, p: BlockPos, be: MovingBe) {
    if !matches!(w.get(p), Block::MovingPiston { .. }) {
        // LevelChunk.setBlockEntity: "state does not allow it": ignored (logged by the game)
        w.errors.push(format!("moving block entity refused at {p}: {}", w.get(p)));
        return;
    }
    w.piston.add(p, be);
}

/// `PistonMovingBlockEntity.finalTick`.
pub fn final_tick(w: &mut World, p: BlockPos) {
    let Some(be) = w.piston.be(p).copied() else { return };
    if be.progress_o >= 2 {
        return;
    }
    w.piston.remove(p);
    if matches!(w.get(p), Block::MovingPiston { .. }) {
        let new_state = if be.source { Block::Air } else { bh::update_from_neighbour_shapes(w, be.moved, p) };
        w.set_block(p, new_state, 3);
        w.neighbor_changed(p, new_state.kind());
    }
}

/// `Level.tickBlockEntities` for the moving pistons.
pub fn tick_all(w: &mut World) {
    w.piston.ticking = true;
    let pending = std::mem::take(&mut w.piston.pending);
    w.piston.tickers.extend(pending);
    let mut i = 0;
    while i < w.piston.tickers.len() {
        let wid = w.piston.tickers[i];
        let bound = w.piston.wrappers[wid as usize].filter(|id| w.piston.is_live(*id));
        match bound {
            None => {
                w.piston.tickers.remove(i);
            }
            Some(id) => {
                tick_one(w, id);
                i += 1;
            }
        }
    }
    w.piston.ticking = false;
}

/// `PistonMovingBlockEntity.tick` (the entities it moves: `entity::piston_push`).
fn tick_one(w: &mut World, id: u32) {
    let Some(&(p, _, _)) = w.piston.bes.iter().find(|(_, i, _)| *i == id) else { return };
    // BoundTickingBlockEntity: the type must be valid for the cell's current state
    if !matches!(w.get(p), Block::MovingPiston { .. }) {
        return;
    }
    let time = w.time;
    let be = w.piston.be_by_id_mut(id).unwrap();
    be.last_ticked = time;
    be.progress_o = be.progress;
    if be.progress_o >= 2 {
        let be = *be;
        w.piston.remove(p);
        if matches!(w.get(p), Block::MovingPiston { .. }) {
            let new_state = bh::update_from_neighbour_shapes(w, be.moved, p);
            if new_state.is_air() {
                w.set_block(p, be.moved, 340);
                w.update_or_destroy(be.moved, new_state, p, 3, 512);
            } else {
                // a waterlogged block loses its water when a piston moves it
                let new_state = crate::fluid::set_waterlogged(new_state, false).unwrap_or(new_state);
                w.set_block(p, new_state, 67);
                w.neighbor_changed(p, new_state.kind());
            }
        }
    } else {
        // entities in the way move first (`moveCollidedEntities`, `moveStuckEntities`: clk-misc)
        let be = *be;
        crate::entity::piston_push::tick(w, p, be);
        if let Some(be) = w.piston.be_by_id_mut(id) {
            be.progress = (be.progress + 1).min(2);
        }
    }
}
