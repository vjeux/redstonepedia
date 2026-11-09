//! Falling blocks (agent pistmodel): `FallingBlock` (sand, red sand, gravel, concrete powder)
//! and `FallingBlockEntity`, read in the 26.3 source.
//!
//! * `onPlace` and `updateShape` schedule a tick 2 gt later (pistons land blocks with `onPlace`,
//!   a neighbor change gives a shape update); the tick turns the block into an entity if the cell
//!   below is free (air, or replaceable).
//! * The entity (ticked in the entity phase, insertion order): gravity 0.04, `move` with
//!   collisions against the block collision shapes of its column (full blocks, piston bases and
//!   heads, moving pistons with their progress offset), then drag 0.98F. On the ground it lands:
//!   a moving piston in its cell makes it bounce and wait; a replaceable cell where it would not
//!   keep falling gets the block (`setBlockAndUpdate`); anything else breaks it into an item.
//! * Moving blocks push falling blocks like any entity (`entity::piston_push`, clk-misc): a
//!   block moved off its cell's center, or given sideways motion by a moving slime block, then
//!   moves with the general `Entity.move` ([`move_general`]: every block shape around its 0.98F
//!   box, sideways hits stop that axis, soul sand and honey slow it); the others keep the column
//!   model below, unchanged.
//! * Not modeled: fluids; `applyEffectsFromBlocks`
//!   (pressure plates, tripwires and bubble columns do not see falling blocks, and plate or
//!   tripwire counts leave them out).
//! * The entity list: falling blocks take their ids from the counter every entity shares and tick
//!   with the other entities in id order ([`crate::entity::tick_phase`]).

use crate::behavior as bh;
use crate::block::{Block, Simple};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

pub const MIN_Y: i32 = -64;
pub const MAX_Y: i32 = 319;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FallingBlock {
    pub block: Block,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vy: f64,
    pub time: i32,
    pub removed: bool,
    /// Its entity id, from the counter every entity shares ([`crate::entity::Entities::next_id`]):
    /// the entity phase ticks falling blocks and the other entities together in id order.
    pub id: u32,
    // ---- moved by pistons (entity/piston_push.rs, clk-misc) ----
    /// Horizontal motion: only a moving slime block gives any (its bounce).
    pub vx: f64,
    pub vz: f64,
    /// `pistonDeltas` / `pistonDeltasGameTime` (`Entity.limitPistonMovement`).
    pub piston_deltas: [f64; 3],
    pub piston_deltas_time: i64,
}

impl FallingBlock {
    /// A falling block at rest horizontally (what `fall` makes; a summoned one gives its own
    /// position, fall speed and age).
    pub fn new(block: Block, x: f64, y: f64, z: f64, vy: f64, time: i32, id: u32) -> FallingBlock {
        FallingBlock { block, x, y, z, vy, time, removed: false, id, vx: 0.0, vz: 0.0, piston_deltas: [0.0; 3], piston_deltas_time: 0 }
    }
    /// `getBoundingBox()` (`EntityType.FALLING_BLOCK` is 0.98F x 0.98F).
    pub fn bb(&self) -> crate::entity::Aabb {
        crate::entity::make_bounding_box(crate::entity::Vec3::new(self.x, self.y, self.z), 0.98, 0.98)
    }
    /// Off its cell's center or moving sideways: only after a piston moved it. Such a block
    /// moves with the general `Entity.move` ([`move_general`]); every other one keeps the
    /// column model.
    pub fn off_column(&self) -> bool {
        self.vx != 0.0 || self.vz != 0.0 || self.x - self.x.floor() != 0.5 || self.z - self.z.floor() != 0.5
    }
}

/// `FallingBlockEntity.fall`: the entity is made (taking the next entity id), the cell becomes
/// air (flags 3), then the entity joins the entity list.
pub fn fall(w: &mut World, block: Block, p: BlockPos) {
    let id = w.entities.next_id;
    w.entities.next_id += 1;
    let e = FallingBlock::new(block, p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5, 0.0, 0, id);
    w.set_block_and_update(p, Block::Air);
    w.gravity.entities.push(e);
}

/// Falling-block entities, ticked in insertion order in the entity phase.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct State {
    pub entities: Vec<FallingBlock>,
}

/// (mob-AI) `FallingBlockEntity`'s break into its item (`spawnAtLocation(level, block)`: an
/// `ItemEntity` at its position) where the mob AI keeps item entities (a natural sand or gravel
/// landing on a spike); the redstone judges keep only `World::drops`.
fn drop_item(w: &mut World, e: &FallingBlock) {
    if !w.mob.ai.on {
        return;
    }
    let item: &'static str = match e.block.to_string().as_str() {
        "minecraft:sand" => "minecraft:sand",
        "minecraft:red_sand" => "minecraft:red_sand",
        "minecraft:gravel" => "minecraft:gravel",
        "minecraft:white_concrete_powder" => "minecraft:white_concrete_powder",
        _ => return,
    };
    crate::entity::mob::living::spawn_item_at(w, crate::entity::Vec3::new(e.x, e.y, e.z), e.id, item, 1);
}

impl State {
    /// The live falling blocks for [`crate::world::World::state_key`] (floats by their bits).
    pub(crate) fn key_into<H: std::hash::Hasher>(&self, h: &mut H) {
        use std::hash::Hash;
        for e in self.entities.iter().filter(|e| !e.removed) {
            (e.block, e.x.to_bits(), e.y.to_bits(), e.z.to_bits(), e.vy.to_bits(), e.time).hash(h);
            // horizontal motion only after a moving slime block (keys of everything else unchanged)
            if e.vx != 0.0 || e.vz != 0.0 {
                (e.vx.to_bits(), e.vz.to_bits()).hash(h);
            }
        }
        0xfeu8.hash(h);
    }
}

pub fn is_idle(w: &World) -> bool {
    w.gravity.entities.iter().all(|e| e.removed)
}

/// `FallingBlock.isFree`: air, fire, liquid or replaceable.
pub fn is_free(b: Block) -> bool {
    b.is_air() || bh::can_be_replaced(&b)
}

/// `FallingBlock.onPlace`.
pub fn on_place(w: &mut World, state: Block, p: BlockPos, _old: Block, _moved: bool) {
    w.schedule_tick(p, state.kind(), 2, crate::ticks::Priority::Normal);
}

/// `FallingBlock.updateShape`.
pub fn update_shape(w: &mut World, state: Block, p: BlockPos, _dir: Dir, _np: BlockPos, _ns: Block) -> Block {
    w.schedule_tick(p, state.kind(), 2, crate::ticks::Priority::Normal);
    state
}

/// `FallingBlock.tick`: `FallingBlockEntity.fall` when the cell below is free.
pub fn tick(w: &mut World, state: Block, p: BlockPos) {
    if is_free(w.get(p.below())) && p.y >= MIN_Y {
        fall(w, state, p);
    }
}

const PX: f64 = 1.0 / 16.0;

/// Vertical extents of a block's collision boxes (empty collision context) that overlap the
/// column of a falling block centered in the cell (x, z in 0.01..0.99).
fn context_free_boxes(b: &Block) -> Vec<(f64, f64)> {
    bh::collision_boxes(b)
        .into_iter()
        .filter(|c| (c[1] as f64) > 0.16 && (c[0] as f64) < 15.84 && (c[5] as f64) > 0.16 && (c[4] as f64) < 15.84)
        .map(|c| (c[2] as f64 * PX, c[3] as f64 * PX))
        .collect()
}

/// The vertical extents (relative to the cell) of the parts of a block's collision shape that
/// overlap the column of a falling block centered in it (x, z in 0.01..0.99), for an entity whose
/// bottom is at `e_bottom` (scaffolding depends on it).
fn shape_boxes(w: &World, b: Block, p: BlockPos, e_bottom: f64) -> Vec<(f64, f64)> {
    match b {
        Block::Air => vec![],
        Block::Piston { extended: false, .. } => vec![(0.0, 1.0)],
        Block::Piston { facing, extended: true, .. } => match facing {
            Dir::Up => vec![(0.0, 12.0 * PX)],
            Dir::Down => vec![(4.0 * PX, 1.0)],
            _ => vec![(0.0, 1.0)],
        },
        Block::PistonHead { facing, short, .. } => head_boxes(facing, short),
        Block::MovingPiston { .. } => moving_boxes(w, p),
        Block::Scaffolding { .. } => crate::redstone::blocks::scaffolding::entity_boxes(&b, p.y, e_bottom)
            .into_iter()
            .map(|c| (c[2] as f64 * PX, c[3] as f64 * PX))
            .collect(),
        // every other block: its collision shape (honey 15 px, soul sand 14 px, slabs,
        // trapdoors, fence gates 24 px, ...)
        _ => context_free_boxes(&b),
    }
}

/// `PistonHeadBlock` shapes: the 4 px plate at the front, the 4x4 arm behind it (4 px longer
/// than the cell unless short).
fn head_boxes(facing: Dir, short: bool) -> Vec<(f64, f64)> {
    let arm = if short { 16.0 } else { 20.0 } * PX;
    match facing {
        Dir::Up => vec![(12.0 * PX, 1.0), (12.0 * PX - (arm - 4.0 * PX), 12.0 * PX)],
        Dir::Down => vec![(0.0, 4.0 * PX), (4.0 * PX, arm)],
        // sideways: the plate spans the full height; the arm (6..10 px) is inside it vertically
        _ => vec![(0.0, 1.0)],
    }
}

/// `PistonMovingBlockEntity.getCollisionShape` (no-clip direction never set here).
fn moving_boxes(w: &World, p: BlockPos) -> Vec<(f64, f64)> {
    let Some(be) = w.piston.be(p) else { return vec![] };
    let prog = be.progress as f64 / 2.0;
    let mut out = Vec::new();
    // a retracting source piston keeps its extended base shape
    if !be.extending && be.source {
        if let Block::Piston { facing, .. } = be.moved {
            out.extend(match facing {
                Dir::Up => vec![(0.0, 12.0 * PX)],
                Dir::Down => vec![(4.0 * PX, 1.0)],
                _ => vec![(0.0, 1.0)],
            });
        }
    }
    let moved = if be.source {
        let short = be.extending != (1.0 - prog < 0.25);
        Block::PistonHead { sticky: false, facing: be.direction, short }
    } else {
        be.moved
    };
    let ext = if be.extending { prog - 1.0 } else { 1.0 - prog };
    let dy = be.direction.step()[1] as f64 * ext;
    // the moved state's collision shape with the empty context (no entity)
    let boxes = match moved {
        Block::PistonHead { facing, short, .. } => head_boxes(facing, short),
        Block::Piston { extended: false, .. } => vec![(0.0, 1.0)],
        Block::Piston { facing, extended: true, .. } => match facing {
            Dir::Up => vec![(0.0, 12.0 * PX)],
            Dir::Down => vec![(4.0 * PX, 1.0)],
            _ => vec![(0.0, 1.0)],
        },
        Block::MovingPiston { .. } => vec![],
        m => context_free_boxes(&m),
    };
    out.extend(boxes.into_iter().map(|(a, b)| (a + dy, b + dy)));
    out
}

/// `Entity.collide` for a pure vertical movement `dy` (the `VoxelShape.collide` tolerances).
/// `BlockCollisions` also visits the neighboring columns for blocks with large collision shapes:
/// a moving piston's shape (offset by its progress) can reach into the entity's column.
fn collide_y(w: &World, e: &FallingBlock, dy: f64) -> f64 {
    if dy.abs() < 1.0e-7 {
        return 0.0;
    }
    let bx = e.x.floor() as i32;
    let bz = e.z.floor() as i32;
    let (ex0, ex1, ez0, ez1) = (e.x - 0.49, e.x + 0.49, e.z - 0.49, e.z + 0.49);
    let (lo, hi) = if dy < 0.0 { (e.y + dy, e.y + 0.98) } else { (e.y, e.y + 0.98 + dy) };
    let mut allowed = dy;
    for by in (lo.floor() as i32 - 1)..=(hi.floor() as i32 + 1) {
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
            let p = BlockPos::new(bx + dx, by, bz + dz);
            let b = w.get(p);
            let boxes: Vec<(f64, f64)> = if let Block::MovingPiston { .. } = b {
                crate::piston::shape::moving(w, p)
                    .into_iter()
                    .filter(|c| {
                        let c = c.0;
                        let (x0, x1) = (p.x as f64 + c[0] as f64 * PX, p.x as f64 + c[1] as f64 * PX);
                        let (z0, z1) = (p.z as f64 + c[4] as f64 * PX, p.z as f64 + c[5] as f64 * PX);
                        x0 < ex1 && x1 > ex0 && z0 < ez1 && z1 > ez0
                    })
                    .map(|c| (c.0[2] as f64 * PX, c.0[3] as f64 * PX))
                    .collect()
            } else if dx == 0 && dz == 0 {
                shape_boxes(w, b, p, e.y)
            } else {
                continue;
            };
            for (a, t) in boxes {
                let (bot, top) = (by as f64 + a, by as f64 + t);
                // BlockCollisions: only shapes touching the swept box
                if top <= lo || bot >= hi {
                    continue;
                }
                if dy < 0.0 {
                    if e.y + 1.0e-7 >= top {
                        let d = top - e.y;
                        if d <= 1.0e-7 {
                            allowed = allowed.max(d);
                        }
                    }
                } else if e.y + 0.98 - 1.0e-7 <= bot {
                    let d = bot - (e.y + 0.98);
                    if d >= -1.0e-7 {
                        allowed = allowed.min(d);
                    }
                }
                if allowed.abs() < 1.0e-7 {
                    return 0.0;
                }
            }
        }
    }
    allowed
}

/// The entity phase for falling blocks alone (`FallingBlockEntity.tick`); with other entities,
/// [`crate::entity::tick_phase`] interleaves them by id.
pub fn tick_entities(w: &mut World) {
    let n = w.gravity.entities.len();
    for i in 0..n {
        tick_one(w, i);
    }
    w.gravity.entities.retain(|e| !e.removed);
}

/// One falling block's `FallingBlockEntity.tick` (the caller drops the removed ones afterwards);
/// none outside the entity-ticking chunks (`World::entity_ticks_at`).
pub fn tick_one(w: &mut World, i: usize) {
    {
        let mut e = w.gravity.entities[i];
        if e.removed {
            return;
        }
        if !w.entity_ticks_at(e.x, e.z) {
            w.note_frozen(e.id);
            return;
        }
        if e.block.is_air() {
            e.removed = true;
            w.gravity.entities[i] = e;
            return;
        }
        e.time += 1;
        e.vy += -0.04;
        // Entity.move(SELF): a block a piston moved off its column takes the general move
        let on_ground = if e.off_column() {
            let delta = crate::entity::Vec3::new(e.vx, e.vy, e.vz);
            move_general(w, &mut e, delta)
        } else {
            let dy = e.vy;
            let my = collide_y(w, &e, dy);
            if my * my > 1.0e-7 || dy * dy - my * my < 1.0e-7 {
                e.y += my;
            }
            let vertical_collision = dy != my;
            if vertical_collision && dy.abs() > 0.0 {
                // restituteMovementAfterCollisions (26.3): the effect block 0.2 below, in the
                // block's own column
                let effect = BlockPos::new(e.x.floor() as i32, (e.y - 0.2).floor() as i32, e.z.floor() as i32);
                vertical_restitution(w, &mut e, my, dy < 0.0, effect);
            }
            vertical_collision && dy < 0.0
        };
        let p = BlockPos::new(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32);
        if !on_ground {
            if (e.time > 100 && (p.y <= MIN_Y || p.y > MAX_Y)) || e.time > 600 {
                w.drops.push((p, e.block));
                e.removed = true;
            }
        } else {
            let current = w.get(p);
            e.vy *= -0.5;
            e.vx *= 0.7;
            e.vz *= 0.7;
            if !matches!(current, Block::MovingPiston { .. }) {
                let may_replace = is_free(current);
                let would_continue = is_free(w.get(p.below()));
                // canSurvive = blockState.canSurvive(level, pos) && !wouldContinueFalling
                let can_survive = bh::can_survive(w, &e.block, p) && !would_continue;
                e.removed = true;
                w.gravity.entities[i] = e;
                if may_replace && can_survive {
                    if !w.set_block_and_update(p, e.block) {
                        w.drops.push((p, e.block));
                        drop_item(w, &e);
                    }
                } else {
                    w.drops.push((p, e.block));
                    drop_item(w, &e);
                }
                return;
            }
        }
        e.vy *= 0.98f32 as f64;
        e.vx *= 0.98f32 as f64;
        e.vz *= 0.98f32 as f64;
        w.gravity.entities[i] = e;
    }
}

/// The vertical part of `Entity.restituteMovementAfterCollisions` (26.3, as `collide::restitute`
/// for the other entities): a landing bounces only on slime (1.0F x 0.8F: not a living entity),
/// never on honey (`SUPPRESSES_BOUNCE`), and only when falling faster than gravity (0.04), with
/// the gravity and air-drag compensation of the share of the tick spent moving
/// (`movement_y / vy`); any other vertical hit leaves `-vy * 0.0` (a signed zero). A bounce emits
/// `GameEvent.BOUNCE`: a model error when sculk sensors could hear it.
fn vertical_restitution(w: &mut World, e: &mut FallingBlock, movement_y: f64, below: bool, effect_pos: BlockPos) {
    let cur = e.vy;
    let mut restitution = 0.0f64;
    if below {
        let effect = w.get(effect_pos);
        let block = if matches!(effect, Block::Simple(Simple::SlimeBlock)) { 1.0f32 } else { 0.0f32 } * 0.8f32;
        let suppressed = matches!(effect, Block::Simple(Simple::HoneyBlock));
        restitution = if !(-cur <= 0.04) && !suppressed { restitution.max(block as f64) } else { 0.0 };
    }
    let (gravity_comp, drag) = if restitution > 0.0 {
        let portion = movement_y / cur;
        (portion * 0.04, crate::entity::mth::lerp(portion, 1.0, 0.98f32 as f64))
    } else {
        (0.0, 1.0)
    };
    e.vy = (gravity_comp - cur) * drag * restitution;
    if restitution > 0.0 && !w.sculk.is_empty() {
        w.errors.push("falling block bounce game event near sculk sensors is not modeled".into());
    }
}

/// `Entity.move` of a falling block off its column (moved sideways by a piston, or with
/// sideways motion from a moving slime block), for its own tick and for piston moves (after
/// `limitPistonMovement`): every block shape around its box (`collide::collide_box`; moving
/// blocks under the piston move's `NOCLIP` lose their shape), `restituteMovementAfterCollisions`
/// (a sideways hit stops that axis, a vertical one the 26.3 bounce, [`vertical_restitution`]) and
/// `getBlockSpeedFactor` (soul sand, honey). Returns whether it hit the ground.
pub fn move_general(w: &mut World, e: &mut FallingBlock, delta: crate::entity::Vec3) -> bool {
    use crate::entity::mth;
    let mv = crate::entity::collide::collide_box(w, e.bb(), delta);
    let len = mv.length_sqr();
    if len > 1.0e-7 || delta.length_sqr() - len < 1.0e-7 {
        e.x += mv.x;
        e.y += mv.y;
        e.z += mv.z;
    }
    let x_col = !mth::equal(delta.x, mv.x);
    let z_col = !mth::equal(delta.z, mv.z);
    let vertical = delta.y != mv.y;
    if (delta.y.abs() > 0.0 && vertical) || x_col || z_col {
        if x_col {
            e.vx = -e.vx * 0.0;
        }
        if z_col {
            e.vz = -e.vz * 0.0;
        }
        if vertical {
            // getOnPosLegacy: 0.2 below, under the supporting block found for the landing
            let mut effect = BlockPos::new(e.x.floor() as i32, (e.y - 0.2).floor() as i32, e.z.floor() as i32);
            if delta.y < 0.0 {
                let bb = e.bb();
                let test = crate::entity::Aabb::new(bb.min_x, bb.min_y - 1.0e-6, bb.min_z, bb.max_x, bb.min_y, bb.max_z);
                let pos = crate::entity::Vec3::new(e.x, e.y, e.z);
                let support = crate::entity::collide::find_supporting_block(w, pos, test).or_else(|| crate::entity::collide::find_supporting_block(w, pos, test.move_by(-mv.x, 0.0, -mv.z)));
                if let Some(s) = support {
                    effect = BlockPos::new(s.x, effect.y, s.z);
                }
            }
            vertical_restitution(w, e, mv.y, delta.y < 0.0, effect);
        }
    }
    let sf = |b: &Block| if matches!(b, Block::Simple(Simple::SoulSand) | Block::Simple(Simple::HoneyBlock)) { 0.4f32 } else { 1.0f32 };
    let (bx, bz) = (e.x.floor() as i32, e.z.floor() as i32);
    let here = w.get(BlockPos::new(bx, e.y.floor() as i32, bz));
    let mut f = sf(&here);
    if f == 1.0 && !matches!(here, Block::Water { .. } | Block::BubbleColumn { .. }) {
        f = sf(&w.get(BlockPos::new(bx, (e.y - 0.500001f32 as f64).floor() as i32, bz)));
    }
    e.vx *= f as f64;
    e.vz *= f as f64;
    vertical && delta.y < 0.0
}
