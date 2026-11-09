//! Shulker boxes (shulker study, clk-rt): a dispenser places one (`ShulkerBoxDispenseBehavior` +
//! `BlockItem.place`), a piston destroys one into an item entity carrying its contents
//! (`ShulkerBoxBlock.getDrops`, `Block.popResource` as the bridge seeds it: `rng::Drop`). The box
//! itself is a 27-slot container block entity (`container.rs`), read by comparators and filled
//! or emptied by hoppers like any container; a hopper or dropper never puts a box into a box
//! (`ShulkerBoxBlockEntity.canPlaceItemThroughFace`).

use crate::block::{Block, DYES};
use crate::pos::{BlockPos, Dir};
use crate::redstone::container::{self, CRef, Stack};
use crate::world::{World, flags};

/// The block a shulker box item places (`colour` from its name: `shulker_box` or
/// `<dye>_shulker_box`).
pub fn box_block_of(item: &str, facing: Dir) -> Option<Block> {
    let n = item.strip_prefix("minecraft:").unwrap_or(item);
    if n == "shulker_box" {
        return Some(Block::ShulkerBox { colour: 0, facing });
    }
    let dye = n.strip_suffix("_shulker_box")?;
    let i = DYES.iter().position(|d| *d == dye)?;
    Some(Block::ShulkerBox { colour: 1 + i as u8, facing })
}

/// The item a shulker box block drops.
pub fn box_item_of(s: &Block) -> Option<String> {
    match *s {
        Block::ShulkerBox { colour: 0, .. } => Some("minecraft:shulker_box".to_string()),
        Block::ShulkerBox { colour, .. } => Some(format!("minecraft:{}_shulker_box", DYES[colour as usize - 1])),
        _ => None,
    }
}

/// `ShulkerBoxDispenseBehavior.execute` for the dispenser at `pos` facing `facing`, its `slot`
/// holding `st` (a shulker box item): `BlockItem.place` with a `DirectionalPlaceContext` on the
/// cell in front (clicked face: the dispenser's facing over air, else UP; the cell must be
/// replaceable): `setBlock` (flags 11, an empty block entity), the item's contents into the block
/// entity and `setChanged`, the BLOCK_PLACE game event, the item consumed. Then (success or not)
/// `DispenserBlock.dispenseFrom`'s `setItem(slot, what is left)`.
pub fn dispense_box(w: &mut World, pos: BlockPos, slot: usize, st: Stack, facing: Dir) {
    let me = CRef::Block(pos);
    let target = pos.rel(facing);
    let name = container::item_name(st.item);
    let clicked = if w.get(target.rel(Dir::Down)).is_air() { facing } else { Dir::Up };
    let placed = match box_block_of(name, clicked) {
        Some(b) if crate::behavior::can_be_replaced(&w.get(target)) => Some(b),
        _ => None,
    };
    let Some(b) = placed else {
        container::set_item(w, me, slot, Some(st));
        return;
    };
    w.set_block(target, b, flags::NEIGHBORS | flags::CLIENTS | flags::IMMEDIATE);
    // updateBlockEntityComponents: applyComponentsFromItemStack (the contents), then setChanged
    let slots = container::payload_slots(w, st.payload);
    if let Some(c) = w.containers.map.get_mut(&target) {
        c.slots = slots;
    }
    container::set_changed(w, target);
    w.game_event("block_place", target);
    let rest = (st.count > 1).then(|| st.with_count(st.count - 1));
    container::set_item(w, me, slot, rest);
}

/// A shulker box block's contents as the payload of the item it drops (its loot table's
/// `copy_components`: the block entity still there); `None` for any other block.
pub fn box_payload(w: &mut World, p: BlockPos, s: &Block) -> Option<u16> {
    if !matches!(s, Block::ShulkerBox { .. }) {
        return None;
    }
    let slots = w.containers.map.get(&p).map(|c| c.slots.clone()).unwrap_or_default();
    let contents: Vec<(usize, Stack)> = slots.iter().enumerate().filter_map(|(i, s)| s.map(|s| (i, s))).collect();
    Some(container::new_payload(w, &contents))
}

/// `Block.dropResources` of a shulker box a piston destroys (its block entity still there): with
/// a drop seed, the item entity the bridge spawns (`rng::Drop::seeded`, the world's n-th drop):
/// the box item with its contents, pickup delay 10 (`setDefaultPickUpDelay`), appended to the
/// entities. Without one, nothing (the caller records the drop). (Superseded by
/// `World::drop_resources`, which pops every block's loot this way.)
pub fn drop_box(w: &mut World, p: BlockPos, s: &Block) -> bool {
    let (Some(seed), Some(item)) = (w.drop_seed, box_item_of(s)) else { return false };
    let slots = w.containers.map.get(&p).map(|c| c.slots.clone()).unwrap_or_default();
    let contents: Vec<(usize, Stack)> = slots.iter().enumerate().filter_map(|(i, s)| s.map(|s| (i, s))).collect();
    let payload = container::new_payload(w, &contents);
    let d = crate::rng::Drop::seeded(seed, w.drop_count, p);
    w.drop_count += 1;
    let state = crate::entity::item::ItemState { item, count: 1, pickup_delay: 10, age: 0, payload, health: 5, ..Default::default() };
    let id = w.entities.add(crate::entity::EntityKind::Item(state), crate::entity::Vec3::new(d.pos[0], d.pos[1], d.pos[2]), d.y_rot);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.delta_movement = crate::entity::Vec3::new(d.motion[0], d.motion[1], d.motion[2]);
    }
    true
}

// ---------------------------------------------------------------------- the judge's sandbox

/// The judge's margin around a shulker design's blocks and ports: its watch box, the sandbox's
/// box (`circuit::run::watch_box(c, 3)`).
pub const JUDGE_MARGIN: i32 = 3;

/// The sandbox seed of a build (crates/circuit/src/shulker.rs `build_seed`, which calls this):
/// from where it stands (its watch box `from..to`), and the stress run's number (`salt`, 0 for
/// the base scenario). The bridge reseeds the sandbox's level random to it (create and reset) and
/// seeds each piston-broken drop's own random from it; redsim mirrors both
/// (`World::mirror_level_random`, `World::drop_seed`).
pub fn build_seed(from: [i32; 3], to: [i32; 3], salt: i64) -> i64 {
    (from[0] as i64).wrapping_mul(73_856_093) ^ (from[1] as i64).wrapping_mul(19_349_663) ^ (from[2] as i64).wrapping_mul(83_492_791) ^ (to[0] as i64 - to[2] as i64).wrapping_mul(2_654_435_761) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15u64 as i64)
}

/// The sandbox's forced chunk range `[minX, minZ, maxX, maxZ]` for a watch box (the bridge forces
/// the chunks of the box: `SandboxOptions::chunks`).
pub fn chunk_range(from: [i32; 3], to: [i32; 3]) -> [i32; 4] {
    [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)]
}

/// Run a world like the judge's sandbox for the base scenario of a design whose watch box is
/// `from..to`: the level random mirrored and the drops seeded from [`build_seed`] (salt 0).
pub fn judge_sandbox(w: &mut World, from: [i32; 3], to: [i32; 3]) {
    let seed = build_seed(from, to, 0);
    w.drop_seed = Some(seed);
    w.mirror_level_random(seed, chunk_range(from, to));
}

// ---------------------------------------------------------------------- the judge's chests

/// The settle between the judge's chests and their first fill (`circuit::stimulus::SETTLE`).
pub const SETTLE: u32 = 40;

/// The shulker judge's chests on a design's ports, driven as crates/circuit/src/shulker.rs
/// `run_in` drives them in the game (stepping 1 gt): wasm-safe, for the in-browser viewer (an
/// endless run: the sources and the box chests kept full) and the replay of real-game traces
/// (`mctas circuit shulker compare`: the judge's own supply). The order: the design placed,
/// [`Chests::place`], [`SETTLE`] ticks, [`Chests::start`] (and the start lever), then for every
/// tick: `World::tick`, the frame, [`Chests::visit`].
#[derive(Clone, Debug)]
pub struct Chests {
    pub loader: bool,
    /// The scenario item (a redsim item id) for the endless run.
    pub item: u8,
    /// `items` chests: a loader's source, an unloader's sink.
    pub sources: Vec<BlockPos>,
    /// `boxes` chests: empty boxes for a loader, full ones for an unloader.
    pub supplies: Vec<BlockPos>,
    /// `output` chests (emptied at every visit).
    pub outputs: Vec<BlockPos>,
    /// `reject` chests (a filter's; emptied at every visit).
    pub rejects: Vec<BlockPos>,
    /// The judge's run: what a loader's sources get, stack by stack (`None`: endless).
    queue: Option<Vec<Stack>>,
    /// The judge's run: the box supply, 27 per chest in order (`None`: endless).
    supply: Option<Vec<Stack>>,
    /// An auto-brewer's supply chests, each filled once at the start with its own stacks (in
    /// `supplies` order; `None`: the supply above).
    fills: Option<Vec<Vec<Stack>>>,
    cursor: usize,
    /// The endless unloader's full box: its payload (made at the start).
    full_box: u16,
}

/// The `minecraft:shulker_box` item.
fn box_item() -> u8 {
    container::item_id("shulker_box").expect("shulker_box is a redsim item")
}

/// A chest's stacks in slot order (the judge's `read_chest`).
pub fn chest_stacks(w: &World, p: BlockPos) -> Vec<Stack> {
    w.containers.map.get(&p).map(|c| c.slots.iter().flatten().copied().collect()).unwrap_or_default()
}

/// The judge's `merge_block_entity(Items: stacks)`: the stacks in slots 0.., the rest empty,
/// `setChanged` when anything changed.
pub fn set_chest(w: &mut World, p: BlockPos, stacks: &[Stack]) {
    let Some(c) = w.containers.map.get_mut(&p) else { return };
    let mut slots = vec![None; c.slots.len()];
    for (i, s) in stacks.iter().enumerate().take(slots.len()) {
        slots[i] = Some(*s);
    }
    if slots != c.slots {
        c.slots = slots;
        container::set_changed(w, p);
    }
}

impl Chests {
    /// The viewer's run: a loader's sources and every box chest kept full (stacks of `item`;
    /// an unloader's boxes full of it).
    pub fn endless(loader: bool, item: u8, sources: Vec<BlockPos>, supplies: Vec<BlockPos>, outputs: Vec<BlockPos>, rejects: Vec<BlockPos>) -> Chests {
        Chests { loader, item, sources, supplies, outputs, rejects, queue: None, supply: None, fills: None, cursor: 0, full_box: 0 }
    }

    /// The judge's run: its box supply and (a loader) its source stacks, in order.
    pub fn judged(loader: bool, sources: Vec<BlockPos>, supplies: Vec<BlockPos>, outputs: Vec<BlockPos>, rejects: Vec<BlockPos>, supply: Vec<Stack>, queue: Vec<Stack>) -> Chests {
        let item = queue.first().map(|s| s.item).unwrap_or(0);
        Chests { loader, item, sources, supplies, outputs, rejects, queue: Some(queue), supply: Some(supply), fills: None, cursor: 0, full_box: 0 }
    }

    /// An auto-brewer judge's run (crates/circuit/src/brewer.rs `drive`): every supply chest filled
    /// once at the start with its own stacks (each at most 27), the outputs emptied at every visit.
    pub fn filled(supplies: Vec<(BlockPos, Vec<Stack>)>, outputs: Vec<BlockPos>) -> Chests {
        let (cells, fills): (Vec<BlockPos>, Vec<Vec<Stack>>) = supplies.into_iter().unzip();
        Chests { loader: false, item: 0, sources: Vec::new(), supplies: cells, outputs, rejects: Vec::new(), queue: Some(Vec::new()), supply: Some(Vec::new()), fills: Some(fills), cursor: 0, full_box: 0 }
    }

    /// Every chest, placed raw (the judge's `PlaceMode::Raw`, after the design).
    pub fn place(&self, w: &mut World) {
        let chest = Block::Chest { facing: Dir::North, kind: 0, waterlogged: false };
        let cells: Vec<(BlockPos, Block)> = self.sources.iter().chain(&self.supplies).chain(&self.rejects).chain(&self.outputs).map(|p| (*p, chest)).collect();
        w.place_raw(&cells);
    }

    /// After the settle: the box supply, then the sources (`run_in`'s `topup(0)`).
    pub fn start(&mut self, w: &mut World) {
        if let Some(fills) = self.fills.clone() {
            for (p, stacks) in self.supplies.clone().iter().zip(&fills) {
                set_chest(w, *p, stacks);
            }
            return;
        }
        match self.supply.clone() {
            Some(supply) => {
                for (k, chunk) in supply.chunks(27).enumerate() {
                    if let Some(p) = self.supplies.get(k) {
                        set_chest(w, *p, chunk);
                    }
                }
            }
            None => {
                if !self.loader {
                    let n = container::max_stack_of(container::item_name(self.item));
                    let contents: Vec<(usize, Stack)> = (0..27).map(|i| (i, Stack::new(self.item, n))).collect();
                    self.full_box = container::new_payload(w, &contents);
                }
                self.boxes_up(w);
            }
        }
        self.top_up(w);
    }

    /// The judge's visit after a tick: the outputs, an unloader's sinks and the rejects emptied,
    /// the sources topped up (an endless run's box chests too).
    pub fn visit(&mut self, w: &mut World) {
        let sinks: &[BlockPos] = if self.loader { &[] } else { &self.sources };
        let empty: Vec<BlockPos> = self.outputs.iter().chain(sinks).chain(&self.rejects).copied().collect();
        for p in empty {
            if !chest_stacks(w, p).is_empty() {
                set_chest(w, p, &[]);
            }
        }
        if self.supply.is_none() {
            self.boxes_up(w);
        }
        self.top_up(w);
    }

    /// A loader's sources up to 27 stacks (the judge's: from its queue; endless: full stacks).
    fn top_up(&mut self, w: &mut World) {
        if !self.loader {
            return;
        }
        let max = container::max_stack_of(container::item_name(self.item));
        for p in self.sources.clone() {
            if self.queue.as_ref().is_some_and(|q| self.cursor >= q.len()) {
                continue;
            }
            let mut have = chest_stacks(w, p);
            let before = have.len();
            while have.len() < 27 {
                match &self.queue {
                    Some(q) if self.cursor < q.len() => {
                        have.push(q[self.cursor]);
                        self.cursor += 1;
                    }
                    Some(_) => break,
                    None => have.push(Stack::new(self.item, max)),
                }
            }
            if have.len() != before {
                set_chest(w, p, &have);
            }
        }
    }

    /// An endless run's box chests up to 27 boxes (empty for a loader, full for an unloader).
    fn boxes_up(&mut self, w: &mut World) {
        let b = Stack { item: box_item(), count: 1, payload: if self.loader { 0 } else { self.full_box } };
        for p in self.supplies.clone() {
            let mut have = chest_stacks(w, p);
            if have.len() < 27 {
                have.resize(27, b);
                set_chest(w, p, &have);
            }
        }
    }
}
