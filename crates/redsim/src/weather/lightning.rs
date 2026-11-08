//! Lightning (mobinfra, 2026-11-08), read in the 26.3 source: `ServerLevel.tickThunder`,
//! `findLightningTargetAround`, `findLightningRod`, `LightningBolt`, `Entity.thunderHit` and its
//! overrides (`Villager`: a witch; `Creeper`: charged; `ArmorStand`, `BlockAttachedEntity`:
//! nothing), `VehicleEntity.hurtServer` (a boat or minecart: 50 damage > 40, destroyed),
//! `ItemEntity.hurtServer` (health 5: destroyed), `LightningBolt.spawnFire` and `FireBlock` for
//! the fires it sets, `LightningRodBlock.onLightningStrike`.
//!
//! * **The roll** (`ServerChunkCache.tickSpawningChunk` -> `tickThunder`, before natural spawning
//!   and the chunk's random ticks): for every chunk with a non-spectator player within 128 blocks
//!   (`ChunkMap.collectSpawningChunks`; the judge assumes the AFK player of a farm, so every chunk
//!   that can matter), while raining and thundering, `random.nextInt(100000) == 0`: a random
//!   column of the chunk (`getBlockRandomPos`), its `MOTION_BLOCKING` top as the center.
//! * **The target**: the closest lightning rod within 128 of the center (`PoiManager.findClosest`:
//!   squared block distance) whose rod is its column's `WORLD_SURFACE` top, the bolt one above it;
//!   else a random living entity, alive, that sees the sky at its block, whose box meets the
//!   column's box (the center up to the top of the world) inflated by 3; else the center. Only if
//!   it rains at that block does a bolt fall (an entity under glass is picked, but stays dry).
//! * **Traps** (Hard): with `spawn_mobs`, `nextDouble() < effectiveDifficulty * 0.01` and no rod
//!   below, a skeleton trap horse spawns there and the bolt is visual (it hurts nothing).
//! * **The bolt** (`LightningBolt.tick`): life 2, 1..3 flashes (its own random). Its first tick
//!   sets a fire at its block and at 4 random cells of the cube around it (Normal and Hard) and
//!   powers the rod under it. Every tick while its life is not negative it hits every alive entity
//!   whose box meets its own (x +-3, y -3..+9, z +-3) - through any roof. Then a flash re-arms it
//!   (life 1, a fire at its block) once its life is below `-nextInt(10)`, until none is left.
//! * **Fires**: blocks ([`super::fire`]: `FireBlock` exactly, spread, burn-out, rain), set where
//!   the bolt's block and its 4 random cells are air and a fire survives
//!   (`LightningBolt.spawnFire`), only with a player within 128 (`canSpreadFireAround`).
//!
//! The level random and every entity's own random are the weather's seeded stand-in
//! ([`super::Weather::rng`]), with the game's distributions.

use super::{WeatherEvent, can_see_sky, motion_blocking_height, world_surface_height};
use crate::block::{Block, Simple};
use crate::entity::{Aabb, EntityKind, Vec3};
use crate::pos::{BlockPos, Dir};
use crate::rng::LegacyRandom;
use crate::world::World;

/// The top of the overworld (`getMaxY()`: 319).
pub const WORLD_MAX_Y: i32 = 319;

/// `Difficulty`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub fn id(self) -> i32 {
        match self {
            Difficulty::Peaceful => 0,
            Difficulty::Easy => 1,
            Difficulty::Normal => 2,
            Difficulty::Hard => 3,
        }
    }
}

/// The game settings lightning reads (the judge's: Hard, `spawn_mobs` on, an AFK player near the
/// farm, a mature world).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub difficulty: Difficulty,
    /// `GameRules.SPAWN_MOBS` (trap horses).
    pub spawn_mobs: bool,
    /// A player within 128 blocks of every chunk of the world: the thunder rolls
    /// (`collectSpawningChunks`) and fires (`fire_spread_radius_around_player`, 128).
    pub player_near: bool,
    /// The world is mature (`DifficultyInstance`: world age over 1,512,000 gt and the chunk
    /// inhabited over 3,600,000 gt, an AFK farm's): the trap chance's worst case but the moon.
    pub mature: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { difficulty: Difficulty::Hard, spawn_mobs: true, player_near: true, mature: true }
    }
}

/// `DimensionType.MOON_BRIGHTNESS_PER_PHASE`.
const MOON_BRIGHTNESS: [f32; 8] = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75];

/// `DifficultyInstance.calculateDifficulty` in a mature world (or a new one): the base times 0.75,
/// plus the world age's 0.25, plus the chunk's inhabited time (1 on Hard, else 0.75) and the moon
/// (`moonBrightness * 0.25`, at most the age part), halved on Easy.
pub fn effective_difficulty(s: &Settings, day_time: i64) -> f32 {
    if s.difficulty == Difficulty::Peaceful {
        return 0.0;
    }
    let hard = s.difficulty == Difficulty::Hard;
    let mut f = 0.75f32;
    let age = if s.mature { 0.25f32 } else { 0.0 };
    f += age;
    let mut h = 0.0f32;
    if s.mature {
        h += if hard { 1.0 } else { 0.75 };
    }
    let moon = MOON_BRIGHTNESS[(day_time.div_euclid(24000)).rem_euclid(8) as usize];
    h += (moon * 0.25f32).clamp(0.0, age);
    if s.difficulty == Difficulty::Easy {
        h *= 0.5;
    }
    f += h;
    s.difficulty.id() as f32 * f
}

/// What a bolt aimed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A lightning rod (the bolt above it).
    Rod([i32; 3]),
    /// A living entity that saw the sky (its id).
    Entity(u32),
    /// The column's motion-blocking top.
    Column,
}

/// A bolt (`LightningBolt`): an entity in the game, here a list of its own ticked in the entity
/// order (its id comes from the counter every entity shares).
#[derive(Clone, Debug)]
pub struct Bolt {
    pub id: u32,
    /// `position()`: the bottom center of its block.
    pub pos: Vec3,
    pub life: i32,
    pub flashes: i32,
    /// `seed` (the client draws its zigzag from it).
    pub seed: i64,
    pub visual_only: bool,
    pub removed: bool,
    /// Ticks lived (`tickCount`; the viewer).
    pub tick_count: i32,
    /// Its own random (`Entity.random`).
    pub random: LegacyRandom,
}

impl Bolt {
    /// `blockPosition()`.
    pub fn block(&self) -> BlockPos {
        BlockPos::new(self.pos.x.floor() as i32, self.pos.y.floor() as i32, self.pos.z.floor() as i32)
    }

    /// The box it hits entities in (`LightningBolt.tick`).
    pub fn hit_box(&self) -> Aabb {
        hit_box_at(self.pos)
    }
}

/// The hit box of a bolt at `pos`: x and z +-3, y -3..+9 (`getY() + 6.0 + 3.0`).
pub fn hit_box_at(p: Vec3) -> Aabb {
    Aabb::new(p.x - 3.0, p.y - 3.0, p.z - 3.0, p.x + 3.0, p.y + 6.0 + 3.0, p.z + 3.0)
}

/// `FireBlock`'s ignite and burn odds of the modeled blocks (`setFlammable`; waterlogged: 0);
/// `None`: not flammable.
pub fn flammable(b: &Block) -> Option<(i32, i32)> {
    Some(match b {
        Block::Leaves { .. } => (30, 60),
        Block::FenceGate { .. } => (5, 20),
        Block::Fence { kind, waterlogged: false, .. } if crate::fence::wooden(*kind) => (5, 20),
        Block::Log { .. } => (5, 5),
        Block::Tnt { .. } => (15, 100),
        Block::Target { .. } => (15, 20),
        Block::Scaffolding { .. } => (60, 60),
        Block::Bamboo { .. } => (60, 60),
        Block::Composter { .. } => (5, 20),
        // (chiseled bookshelves are not in 26.3's table; big dripleaves and cactus flowers are)
        Block::BigDripleaf { waterlogged: false, .. } => (60, 100),
        Block::CactusFlower => (60, 100),
        Block::Simple(s) => match s {
            Simple::OakPlanks | Simple::SprucePlanks => (5, 20),
            Simple::Bookshelf => (30, 20),
            Simple::CoalBlock => (5, 5),
            Simple::DriedKelpBlock => (30, 60),
            Simple::WhiteWool
            | Simple::OrangeWool
            | Simple::BlackWool
            | Simple::MagentaWool
            | Simple::LightBlueWool
            | Simple::YellowWool
            | Simple::LimeWool
            | Simple::PinkWool
            | Simple::GrayWool
            | Simple::LightGrayWool
            | Simple::CyanWool
            | Simple::PurpleWool
            | Simple::BlueWool
            | Simple::BrownWool
            | Simple::GreenWool
            | Simple::RedWool => (30, 60),
            _ => return None,
        },
        _ => return None,
    })
}

/// `FireBlock.isValidFireLocation`: a flammable block on one of the six sides.
fn flammable_neighbour(w: &World, p: BlockPos) -> Option<(BlockPos, Block)> {
    for d in [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East] {
        let q = p.rel(d);
        let b = w.get(q);
        if flammable(&b).is_some() {
            return Some((q, b));
        }
    }
    None
}

/// The lightning rods of the world whose rod tops its column (`findLightningRod`'s filter).
fn rods(w: &World) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for (i, b) in w.cells.iter().enumerate() {
        if matches!(b, Block::LightningRod { .. }) {
            let i = i as i32;
            let (sx, sz) = (w.size[0], w.size[2]);
            let x = w.min.x + i % sx;
            let z = w.min.z + (i / sx) % sz;
            let y = w.min.y + i / (sx * sz);
            if y == world_surface_height(w, x, z) - 1 {
                v.push(BlockPos::new(x, y, z));
            }
        }
    }
    v
}

/// `findLightningRod(center)`: the closest such rod within 128 (squared block distance), one
/// above it. Ties: every tied rod (the game takes the first of its section order).
pub fn rod_targets(w: &World, center: BlockPos) -> Vec<BlockPos> {
    closest_rods(&rods(w), center)
}

/// [`rod_targets`] over the rods [`rods`] found.
fn closest_rods(rods: &[BlockPos], center: BlockPos) -> Vec<BlockPos> {
    let d2 = |p: &BlockPos| {
        let (dx, dy, dz) = ((p.x - center.x) as f64, (p.y - center.y) as f64, (p.z - center.z) as f64);
        dx * dx + dy * dy + dz * dz
    };
    let near: Vec<BlockPos> = rods.iter().copied().filter(|p| d2(p) <= 128.0 * 128.0).collect();
    let Some(best) = near.iter().map(d2).min_by(f64::total_cmp) else { return Vec::new() };
    near.into_iter().filter(|p| d2(p) == best).map(|p| p.offset(0, 1, 0)).collect()
}

/// `findLightningTargetAround`'s entity candidates for a column top: the living entities, alive,
/// that see the sky at their block, whose box meets the column box inflated by 3 (indices).
pub fn entity_candidates(w: &World, center: BlockPos) -> Vec<usize> {
    let search = Aabb::new(center.x as f64, center.y as f64, center.z as f64, (center.x + 1) as f64, (WORLD_MAX_Y + 2) as f64, (center.z + 1) as f64).inflate(3.0, 3.0, 3.0);
    let mut out = Vec::new();
    for (i, e) in w.entities.list.iter().enumerate() {
        let EntityKind::Mob(m) = &e.kind else { continue };
        if e.removed || m.health <= 0.0 || !e.bb.intersects(search) {
            continue;
        }
        if can_see_sky(w, e.block_position()).unwrap_or(false) {
            out.push(i);
        }
    }
    out
}

/// `ServerLevel.tickThunder` for every chunk of the world (inflated by 3 blocks: a bolt in a
/// neighbouring chunk reaches entities that far), in chunk order. The chunk order the game
/// shuffles, and the level random it draws from, are the stand-in's.
pub fn tick_thunder(w: &mut World) {
    if !w.weather.on || !w.weather.lightning.player_near || !(w.weather.is_raining() && w.weather.is_thundering()) {
        return;
    }
    let (x0, x1) = ((w.min.x - 3).div_euclid(16), (w.min.x + w.size[0] + 2).div_euclid(16));
    let (z0, z1) = ((w.min.z - 3).div_euclid(16), (w.min.z + w.size[2] + 2).div_euclid(16));
    for cz in z0..=z1 {
        for cx in x0..=x1 {
            if w.weather.rng.next_int(100000) != 0 {
                continue;
            }
            let (dx, dz) = (w.weather.rng.next_int(16), w.weather.rng.next_int(16));
            strike(w, cx * 16 + dx, cz * 16 + dz);
        }
    }
}

/// One thunder roll that came up in column (x, z): the target, the rain check, the trap, the bolt.
pub fn strike(w: &mut World, x: i32, z: i32) {
    let center = BlockPos::new(x, motion_blocking_height(w, x, z), z);
    let (pos, target) = {
        let rods = rod_targets(w, center);
        if let Some(r) = rods.first() {
            (*r, Target::Rod([r.x, r.y - 1, r.z]))
        } else {
            let cands = entity_candidates(w, center);
            if cands.is_empty() {
                (center, Target::Column)
            } else {
                let k = cands[w.weather.rng.next_int(cands.len() as i32) as usize];
                let e = &w.entities.list[k];
                (e.block_position(), Target::Entity(e.id))
            }
        }
    };
    if !w.is_raining_at(pos) {
        return;
    }
    let s = w.weather.lightning;
    let below_rod = matches!(w.get(pos.offset(0, -1, 0)), Block::LightningRod { .. });
    let trap = s.spawn_mobs && w.weather.rng.next_double() < (effective_difficulty(&s, w.day_time()) * 0.01f32) as f64 && !below_rod;
    let t = w.time;
    if trap {
        w.weather.events.push(WeatherEvent::TrapHorse { t, at: [pos.x, pos.y, pos.z] });
    }
    let seed = w.weather.rng.next_long();
    let mut random = LegacyRandom::new(seed);
    let bolt_seed = random.next_long();
    let flashes = random.next_int(3) + 1;
    let id = w.entities.next_id;
    w.entities.next_id += 1;
    let bolt = Bolt { id, pos: Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5), life: 2, flashes, seed: bolt_seed, visual_only: trap, removed: false, tick_count: 0, random };
    w.weather.events.push(WeatherEvent::Strike { t, bolt: id, at: [pos.x, pos.y, pos.z], target, visual: trap });
    w.weather.bolts.push(bolt);
}

/// `summon minecraft:lightning_bolt` at the bottom center of `pos` (no aim, no trap: the bolt
/// checks' scenarios); its own random from the stand-in. Its id.
pub fn summon_bolt(w: &mut World, pos: BlockPos) -> u32 {
    let seed = w.weather.rng.next_long();
    let mut random = LegacyRandom::new(seed);
    let bolt_seed = random.next_long();
    let flashes = random.next_int(3) + 1;
    let id = w.entities.next_id;
    w.entities.next_id += 1;
    let t = w.time;
    w.weather.bolts.push(Bolt { id, pos: Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5), life: 2, flashes, seed: bolt_seed, visual_only: false, removed: false, tick_count: 0, random });
    w.weather.events.push(WeatherEvent::Strike { t, bolt: id, at: [pos.x, pos.y, pos.z], target: Target::Column, visual: false });
    id
}

/// `LightningBolt.tick` of bolt `k`.
pub fn tick_bolt(w: &mut World, k: usize) {
    w.weather.bolts[k].tick_count += 1;
    if w.weather.bolts[k].life == 2 {
        if matches!(w.weather.lightning.difficulty, Difficulty::Normal | Difficulty::Hard) {
            spawn_fire(w, k, 4);
        }
        power_rod(w, k);
    }
    let b = &mut w.weather.bolts[k];
    b.life -= 1;
    if b.life < 0 {
        if b.flashes == 0 {
            b.removed = true;
        } else if b.life < -b.random.next_int(10) {
            b.flashes -= 1;
            b.life = 1;
            b.seed = b.random.next_long();
            spawn_fire(w, k, 0);
        }
    }
    let b = &w.weather.bolts[k];
    if b.life >= 0 && !b.visual_only {
        let (id, hit) = (b.id, b.hit_box());
        let victims: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed && alive(e) && e.bb.intersects(hit)).map(|e| e.id).collect();
        for v in victims {
            thunder_hit(w, id, v);
        }
    }
}

/// `Entity.isAlive()`: not removed (a living entity: health above 0 too).
fn alive(e: &crate::entity::Entity) -> bool {
    match &e.kind {
        EntityKind::Mob(m) => m.health > 0.0,
        _ => true,
    }
}

/// `thunderHit` of entity `id` by bolt `bolt`.
fn thunder_hit(w: &mut World, bolt: u32, id: u32) {
    let Some(i) = crate::entity::index_of(w, id) else { return };
    if w.entities.list[i].removed {
        return;
    }
    let t = w.time;
    let what = crate::entity::mob::kind_name(&w.entities.list[i].kind);
    w.weather.events.push(WeatherEvent::Hit { t, bolt, entity: id, what });
    match &w.entities.list[i].kind {
        EntityKind::Mob(_) => mob_thunder_hit(w, i),
        EntityKind::Boat(_) | EntityKind::Minecart(_) => {
            // Entity.thunderHit: the fire ticks (a vehicle does not burn), then hurtServer(5):
            // VehicleEntity adds 5 x 10 = 50 damage, over 40: destroy (kill, its item dropped)
            lead_unmodeled(w, i, "lightning destroyed");
            w.entities.list[i].removed = true;
            w.weather.events.push(WeatherEvent::VehicleDestroyed { t, entity: id });
            let riders = w.entities.list[i].passengers.clone();
            for r in riders {
                w.dismounted.push(r);
            }
        }
        EntityKind::Item(s) => {
            // ItemEntity.hurtServer: health 5 - 5
            let what = if s.item.ends_with("iron_ingot") { "iron ingot" } else if s.item.ends_with("poppy") { "poppy" } else { "item" };
            w.entities.list[i].removed = true;
            w.weather.events.push(WeatherEvent::ItemDestroyed { t, entity: id, what });
        }
        // ArmorStand.thunderHit does nothing; a primed TNT is not hurt by it (no health)
        EntityKind::ArmorStand(_) | EntityKind::PrimedTnt(_) => {}
    }
}

/// An entity on a lead, or holding one, that a bolt removes: the lead drops as an item entity with
/// an unseeded kick in the game (boat-leads' `leash.rs` contract), not modeled: a model error.
fn lead_unmodeled(w: &mut World, i: usize, what: &str) {
    let id = w.entities.list[i].id;
    let leashed = w.entities.list[i].leash.is_some();
    let holds = w.entities.list.iter().any(|e| !e.removed && e.leash.as_ref().is_some_and(|l| l.holder == id));
    if leashed || holds {
        let why = format!("{what} entity {id}, which {} a lead: the lead's item drop is not modeled", if leashed { "is on" } else { "holds" });
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
}

/// `Entity.thunderHit` of a mob: its fire ticks + 1 (8 s of fire if that makes 0: one that was
/// not burning, at -1), then 5 lightning damage; `Villager.thunderHit`: it turns into a witch
/// unless the game is Peaceful (`convertTo(WITCH)`: the villager is discarded, its bed and job
/// released; the witch is not simulated: the judge fails the run at the conversion).
fn mob_thunder_hit(w: &mut World, i: usize) {
    use crate::entity::mob::{self, Damage, types::MobType};
    let t = w.time;
    if mob::mob(w, i).ty == MobType::Villager && w.weather.lightning.difficulty != Difficulty::Peaceful {
        let id = w.entities.list[i].id;
        let (pos, yaw) = (w.entities.list[i].pos, w.entities.list[i].y_rot);
        lead_unmodeled(w, i, "lightning converted");
        w.entities.list[i].removed = true;
        // the witch takes its place, persistent (`setPersistenceRequired`)
        let witch = mob::add(w, MobType::Witch, pos, true);
        if let Some(k) = crate::entity::index_of(w, witch) {
            w.entities.list[k].y_rot = yaw;
        }
        w.weather.events.push(WeatherEvent::Converted { t, entity: id });
        return;
    }
    let m = mob::mob_mut(w, i);
    m.fire_ticks += 1;
    if m.fire_ticks == 0 && m.fire_ticks < 160 {
        m.fire_ticks = 160;
    }
    mob::living::hurt(w, i, Damage::Lightning, 5.0);
}

/// `LightningBolt.powerLightningRod`: the block under the bolt (`getStrikePosition`), if a rod
/// (a visual bolt powers it too).
fn power_rod(w: &mut World, k: usize) {
    let p = w.weather.bolts[k].pos;
    let strike = BlockPos::new(p.x.floor() as i32, (p.y - 1.0e-6).floor() as i32, p.z.floor() as i32);
    if let Block::LightningRod { .. } = w.get(strike) {
        rod_strike(w, strike);
    }
}

/// `LightningRodBlock.updateNeighbours`: the neighbours of the block the rod hangs on.
fn rod_update_neighbours(w: &mut World, s: &Block, p: BlockPos) {
    if let Block::LightningRod { facing, .. } = *s {
        w.update_neighbors_at(p.rel(facing.opposite()), s.kind());
    }
}

/// `LightningRodBlock.onLightningStrike`: powered (`setBlockAndUpdate`), the neighbours of its
/// support, off again in 8 gt.
pub fn rod_strike(w: &mut World, p: BlockPos) {
    let Block::LightningRod { facing, waxed, .. } = w.get(p) else { return };
    let on = Block::LightningRod { facing, powered: true, waxed };
    w.set_block_and_update(p, on);
    rod_update_neighbours(w, &on, p);
    w.schedule_tick(p, on.kind(), 8, crate::ticks::Priority::Normal);
}

/// `LightningRodBlock.tick`: off (`setBlockAndUpdate`), the neighbours of its support.
pub fn rod_tick(w: &mut World, s: Block, p: BlockPos) {
    let Block::LightningRod { facing, waxed, .. } = s else { return };
    w.set_block_and_update(p, Block::LightningRod { facing, powered: false, waxed });
    rod_update_neighbours(w, &s, p);
}

/// `LightningRodBlock.onPlace`: a powered rod placed fresh schedules its off tick.
pub fn rod_on_place(w: &mut World, s: Block, p: BlockPos, old: Block) {
    if let Block::LightningRod { powered: true, .. } = s
        && s.kind() != old.kind()
        && !w.has_scheduled_tick(p, s.kind())
    {
        w.schedule_tick(p, s.kind(), 8, crate::ticks::Priority::Normal);
    }
}

/// `LightningRodBlock.affectNeighborsAfterRemoval`: a powered rod updates its support's neighbours.
pub fn rod_after_removal(w: &mut World, s: Block, p: BlockPos) {
    if let Block::LightningRod { powered: true, .. } = s {
        rod_update_neighbours(w, &s, p);
    }
}

/// The rod's signals (`ownSignal`: 15 while powered, every side; `getDirectSignal`: 15 toward
/// `facing`, into the block it hangs on).
pub fn rod_signal(s: &Block, d: Option<Dir>) -> i32 {
    match (*s, d) {
        (Block::LightningRod { powered: true, .. }, None) => 15,
        (Block::LightningRod { powered: true, facing, .. }, Some(d)) if facing == d => 15,
        _ => 0,
    }
}

/// The rod's collision boxes (`RodBlock.SHAPES`: `cube(4, 4, 16)` turned to its axis).
pub fn rod_boxes(facing: Dir) -> Vec<crate::behavior::Box16> {
    match facing {
        Dir::Up | Dir::Down => vec![[6, 10, 0, 16, 6, 10]],
        Dir::North | Dir::South => vec![[6, 10, 6, 10, 0, 16]],
        Dir::West | Dir::East => vec![[0, 16, 6, 10, 6, 10]],
    }
}

/// `LightningBolt.spawnFire(additional)`: a player near the bolt's block
/// (`canSpreadFireAround`), a fire at that block, then at `additional` random cells of the cube
/// around it (the three offsets drawn in x, y, z order whether or not a fire is set), each where
/// the cell is air and the fire survives ([`super::fire::place_fire`]).
fn spawn_fire(w: &mut World, k: usize, additional: i32) {
    let at = w.weather.bolts[k].block();
    if w.weather.bolts[k].visual_only || !super::fire::can_spread_fire_around(w, at) {
        return;
    }
    super::fire::place_fire(w, at);
    for _ in 0..additional {
        let r = &mut w.weather.bolts[k].random;
        let (dx, dy, dz) = (r.next_int(3) - 1, r.next_int(3) - 1, r.next_int(3) - 1);
        super::fire::place_fire(w, at.offset(dx, dy, dz));
    }
}

/// Rain reaches block `p` (`isRainingAt` while it rains: no motion-blocking block above, sky
/// light 15, plains).
fn rain_reaches(w: &World, p: BlockPos) -> bool {
    motion_blocking_height(w, p.x, p.z) <= p.y && can_see_sky(w, p).unwrap_or(false)
}

/// [`static_check`] of a farm as the judge builds it (`farm::Run::new`, no weather), with the cells
/// its golems and drops were in over `runs` (the judge's runs of that build).
pub fn check_farm(spec: &crate::entity::mob::farm::FarmSpec, runs: &[crate::entity::mob::farm::RunReport]) -> Vec<String> {
    use crate::entity::mob::farm::{Run, RunOpts, WalkMode};
    let run = match Run::new(spec, &RunOpts::steady(0, 0, WalkMode::Random)) {
        Ok(r) => r,
        Err(e) => return vec![format!("lightning: the farm does not build: {e}")],
    };
    let mut golems: Vec<BlockPos> = runs.iter().flat_map(|r| r.golem_cells.iter()).map(|(c, _)| BlockPos::new(c[0], c[1], c[2])).collect();
    golems.sort_by_key(|p| (p.x, p.y, p.z));
    golems.dedup();
    let mut drops: Vec<BlockPos> = runs.iter().flat_map(|r| r.drop_cells.iter()).map(|(c, _)| BlockPos::new(c[0], c[1], c[2])).collect();
    drops.sort_by_key(|p| (p.x, p.y, p.z));
    drops.dedup();
    static_check(&run.world, &golems, &drops, spec.inner())
}

/// The judge's exhaustive lightning check of a built farm (the runs see a few random bolts a game
/// day of thunder; this sees every one that can fall). Every column within 3 blocks of the farm's
/// box, its strike aimed as `findLightningTargetAround` aims it: at the closest rod topping its
/// column within 128 (then nothing else), else at any living entity that can be there and sees
/// the sky (the villagers, the zombie, and every cell a golem was seen in, `golem_cells`), or at
/// its motion-blocking top when none is there. A bolt that falls (rain at its block) fails the
/// farm when its box meets a villager's cell, the zombie or its vehicle, or a cell a drop was seen
/// in (`drop_cells`), when a fire it sets (its block and the cube around it) touches a flammable
/// block or such a cell, or when a trap horse it brings (Hard, `spawn_mobs`, not above a rod)
/// stands on the farm (`inner`). Each failure once, with the bolt that does it.
pub fn static_check(w: &World, golem_cells: &[BlockPos], drop_cells: &[BlockPos], inner: Aabb) -> Vec<String> {
    use crate::entity::mob::types::MobType;
    let mut out: Vec<String> = Vec::new();
    let mut protect: Vec<(Aabb, String)> = Vec::new();
    let mut statics: Vec<(BlockPos, Aabb, String)> = Vec::new();
    for e in &w.entities.list {
        let what = match &e.kind {
            EntityKind::Mob(m) if m.ty == MobType::Villager => {
                // anywhere in its cell, standing on its bed or sleeping
                let c = e.block_position();
                protect.push((Aabb::new(c.x as f64, e.pos.y.min(c.y as f64), c.z as f64, (c.x + 1) as f64, e.pos.y + 1.95 + 0.5625, (c.z + 1) as f64), format!("the villager at {c}")));
                "a villager"
            }
            EntityKind::Mob(m) if m.ty == MobType::Zombie => {
                protect.push((e.bb, "the zombie".to_string()));
                "the zombie"
            }
            EntityKind::Boat(_) | EntityKind::Minecart(_) => {
                protect.push((e.bb, "the zombie's vehicle".to_string()));
                continue;
            }
            _ => continue,
        };
        if can_see_sky(w, e.block_position()).unwrap_or(false) {
            statics.push((e.block_position(), e.bb, what.to_string()));
        }
    }
    for c in drop_cells {
        protect.push((Aabb::new(c.x as f64, c.y as f64, c.z as f64, (c.x + 1) as f64, (c.y + 1) as f64, (c.z + 1) as f64), format!("the drops passing at {c}")));
    }
    let golems: Vec<(BlockPos, Aabb)> = golem_cells
        .iter()
        .filter(|c| can_see_sky(w, **c).unwrap_or(false))
        .map(|c| (*c, Aabb::new(c.x as f64 + 0.5 - 0.7, c.y as f64, c.z as f64 + 0.5 - 0.7, c.x as f64 + 0.5 + 0.7, c.y as f64 + 2.7, c.z as f64 + 0.5 + 0.7)))
        .collect();
    let mut bolts: Vec<(BlockPos, String, bool)> = Vec::new();
    let mut add = |p: BlockPos, why: String, rod: bool| {
        if !bolts.iter().any(|(q, _, _)| *q == p) {
            bolts.push((p, why, rod));
        }
    };
    let (x0, x1) = (w.min.x - 3, w.min.x + w.size[0] + 2);
    let (z0, z1) = (w.min.z - 3, w.min.z + w.size[2] + 2);
    for z in z0..=z1 {
        for x in x0..=x1 {
            let center = BlockPos::new(x, motion_blocking_height(w, x, z), z);
            let rods = rod_targets(w, center);
            if !rods.is_empty() {
                for r in rods {
                    add(r, format!("a strike near {x} {z} goes to the rod under {r}"), true);
                }
                continue;
            }
            let search = Aabb::new(center.x as f64, center.y as f64, center.z as f64, (center.x + 1) as f64, (WORLD_MAX_Y + 2) as f64, (center.z + 1) as f64).inflate(3.0, 3.0, 3.0);
            let mut always = false;
            for (p, bb, what) in &statics {
                if bb.intersects(search) {
                    add(*p, format!("a strike near {x} {z} aims at {what} (it sees the sky)"), false);
                    always = true;
                }
            }
            for (p, bb) in &golems {
                if bb.intersects(search) {
                    add(*p, format!("a strike near {x} {z} aims at a golem at {p}"), false);
                }
            }
            if !always {
                add(center, format!("a strike at the top of column {x} {z}"), false);
            }
        }
    }
    let mut fail = |m: String| {
        if out.len() < 8 && !out.contains(&m) {
            out.push(m);
        }
    };
    for (b, why, rod) in &bolts {
        if !rain_reaches(w, *b) {
            continue;
        }
        let bolt = Vec3::new(b.x as f64 + 0.5, b.y as f64, b.z as f64 + 0.5);
        let hit = hit_box_at(bolt);
        for (bb, what) in &protect {
            if hit.intersects(*bb) {
                fail(format!("lightning: {why}: the bolt at {b} hits {what}"));
            }
        }
        // the first tick's fires: its block and the cube around it
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let p = b.offset(dx, dy, dz);
                    if !w.get(p).is_air() {
                        continue;
                    }
                    let fire = super::fire::state_at(w, p);
                    if !super::fire::can_survive(w, &fire, p) {
                        continue;
                    }
                    if let (Some((q, f)), Block::Fire { soul: false, .. }) = (flammable_neighbour(w, p), fire) {
                        fail(format!("lightning: {why}: the bolt at {b} can set a fire at {p} next to {f} at {q}, which burns"));
                    }
                    // a fire's cell burns whoever is inside it (the full cell)
                    let cell = Aabb::of_block(p);
                    for (bb, what) in &protect {
                        if cell.intersects(*bb) {
                            fail(format!("lightning: {why}: the bolt at {b} can set a fire at {p} under {what}"));
                        }
                    }
                }
            }
        }
        let on_farm = inner.inflate(1.0, 1.0, 1.0);
        let (hx, hy, hz) = (b.x as f64, b.y as f64, b.z as f64);
        if !rod && (on_farm.min_x..on_farm.max_x).contains(&hx) && (on_farm.min_y..on_farm.max_y).contains(&hy) && (on_farm.min_z..on_farm.max_z).contains(&hz) {
            fail(format!("lightning: {why}: a skeleton trap horse can spawn on the farm at {b} (its walking is not modeled; a lightning rod topping its column within 128 blocks takes every strike and prevents traps)"));
        }
    }
    out
}

/// `ServerLevel.tickChunk`: `random.nextInt(100000) == 0` a chunk a tick while it rains and
/// thunders, at a uniform column of the chunk (`getBlockRandomPos`).
pub const CHUNK_STRIKE: f64 = 1.0 / 100000.0;

/// What lightning costs a built farm, a tick of thunder (mobinfra; vjeux 2026-11-08 08:26: "It's
/// all about tradeoffs": measured on the card, never a refusal). Exact sums over every strike
/// column within 3 blocks of the farm's box ([`CHUNK_STRIKE`] / 256 each), its strike aimed as
/// `findLightningTargetAround` aims it, over the runs' time-weighted occupancy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measure {
    /// Bolts that fall (rain at their block).
    pub bolts: f64,
    /// Expected iron destroyed (each drop in a bolt's box: `ItemEntity.thunderHit` burns it).
    pub iron: f64,
    /// The chance a bolt makes a villager a witch (its box meets a villager's cell).
    pub witch: f64,
    /// The chance a bolt breaks the zombie's vehicle (its box meets the vehicle or the zombie in it).
    pub vehicle: f64,
    /// The chance a bolt's fires (`spawnFire`: its block, then 4 of the 27 around it) light one
    /// beside a flammable block or in a villager's or the zombie's cell.
    pub fire: f64,
    /// The chance a bolt brings a skeleton trap horse onto the farm (Hard, `spawn_mobs`, no rod
    /// under it; the effective difficulty's mean over the moon's phases).
    pub trap: f64,
    /// The chance the farm breaks (a witch, the vehicle, or such a fire).
    pub breaks: f64,
}

/// The mean trap chance of a bolt off a rod: `effectiveDifficulty * 0.01` over the 8 moon phases.
pub fn trap_chance(s: &Settings) -> f64 {
    if !s.spawn_mobs {
        return 0.0;
    }
    (0..8).map(|d| (effective_difficulty(s, d * 24000) * 0.01f32) as f64).sum::<f64>() / 8.0
}

/// A fire cell's danger: air, a fire can stay there, and a flammable block beside it (not soul
/// fire: it burns nothing) or a protected box in it (a fire's cell burns whoever is inside).
fn fire_danger(w: &World, p: BlockPos, protect: &[Aabb]) -> bool {
    if !w.get(p).is_air() {
        return false;
    }
    let fire = super::fire::state_at(w, p);
    if !super::fire::can_survive(w, &fire, p) {
        return false;
    }
    let soul = matches!(fire, Block::Fire { soul: true, .. });
    (!soul && flammable_neighbour(w, p).is_some()) || protect.iter().any(|bb| Aabb::of_block(p).intersects(*bb))
}

/// The chance a bolt at `b` lights a dangerous fire (Normal and Hard: `spawnFire(4)` on its first
/// tick: its block, then 4 cells drawn uniformly from the 27 around it).
fn fire_chance(w: &World, b: BlockPos, protect: &[Aabb]) -> f64 {
    if fire_danger(w, b, protect) {
        return 1.0;
    }
    let mut d = 0;
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                if fire_danger(w, b.offset(dx, dy, dz), protect) {
                    d += 1;
                }
            }
        }
    }
    1.0 - (1.0 - d as f64 / 27.0).powi(4)
}

/// [`Measure`] of a built farm: `golems` the blocks a living golem is in with the chance it is
/// there at a tick, `iron` the blocks a drop is in with the iron there at a tick on average.
pub fn measure(w: &World, golems: &[(BlockPos, f64)], iron: &[(BlockPos, f64)], inner: Aabb) -> Measure {
    use crate::entity::mob::types::MobType;
    let q = CHUNK_STRIKE / 256.0;
    let mut villagers: Vec<Aabb> = Vec::new();
    let mut vehicle: Vec<Aabb> = Vec::new();
    let mut zombie: Vec<Aabb> = Vec::new();
    let mut statics: Vec<(BlockPos, Aabb)> = Vec::new();
    for e in &w.entities.list {
        match &e.kind {
            EntityKind::Mob(m) if m.ty == MobType::Villager => {
                let c = e.block_position();
                villagers.push(Aabb::new(c.x as f64, e.pos.y.min(c.y as f64), c.z as f64, (c.x + 1) as f64, e.pos.y + 1.95 + 0.5625, (c.z + 1) as f64));
                if can_see_sky(w, c).unwrap_or(false) {
                    statics.push((c, e.bb));
                }
            }
            EntityKind::Mob(m) if m.ty == MobType::Zombie => {
                zombie.push(e.bb);
                vehicle.push(e.bb);
                if can_see_sky(w, e.block_position()).unwrap_or(false) {
                    statics.push((e.block_position(), e.bb));
                }
            }
            EntityKind::Boat(_) | EntityKind::Minecart(_) => vehicle.push(e.bb),
            _ => {}
        }
    }
    let protect: Vec<Aabb> = villagers.iter().chain(zombie.iter()).copied().collect();
    let golem_boxes: Vec<(BlockPos, Aabb, f64)> = golems
        .iter()
        .filter(|(c, _)| can_see_sky(w, *c).unwrap_or(false))
        .map(|(c, p)| (*c, Aabb::new(c.x as f64 + 0.5 - 0.7, c.y as f64, c.z as f64 + 0.5 - 0.7, c.x as f64 + 0.5 + 0.7, c.y as f64 + 2.7, c.z as f64 + 0.5 + 0.7), p.min(1.0)))
        .collect();
    let trap = trap_chance(&Settings::default());
    let on_farm = inner.inflate(1.0, 1.0, 1.0);
    let mut m = Measure::default();
    let (x0, x1) = (w.min.x - 3, w.min.x + w.size[0] + 2);
    let (z0, z1) = (w.min.z - 3, w.min.z + w.size[2] + 2);
    // a rod takes the strikes of every column within 128 of it whose closest rod it is, in the
    // AFK player's spawning chunks (`anyPlayerCloseEnoughForSpawning`: the chunk's center within
    // 128 of the player, here the farm's middle); outside redsim's box a column's top is the
    // farm's floor
    let mut rod_far: Vec<(BlockPos, f64)> = Vec::new();
    let all_rods = rods(w);
    if !all_rods.is_empty() {
        let (px, pz) = ((x0 + x1) as f64 / 2.0, (z0 + z1) as f64 / 2.0);
        let floor = w.min.y;
        for z in (pz as i32 - 128)..=(pz as i32 + 128) {
            for x in (px as i32 - 128)..=(px as i32 + 128) {
                if (x0..=x1).contains(&x) && (z0..=z1).contains(&z) {
                    continue;
                }
                let (cx, cz) = ((x.div_euclid(16) * 16 + 8) as f64, (z.div_euclid(16) * 16 + 8) as f64);
                if (cx - px).powi(2) + (cz - pz).powi(2) >= 128.0 * 128.0 {
                    continue;
                }
                let tied = closest_rods(&all_rods, BlockPos::new(x, floor, z));
                let n = tied.len() as f64;
                for r in tied {
                    match rod_far.iter_mut().find(|(p, _)| *p == r) {
                        Some((_, c)) => *c += 1.0 / n,
                        None => rod_far.push((r, 1.0 / n)),
                    }
                }
            }
        }
    }
    let mut columns: Vec<(Vec<(BlockPos, f64, bool)>, f64)> = rod_far.into_iter().map(|(r, n)| (vec![(r, 1.0, true)], n)).collect();
    for z in z0..=z1 {
        for x in x0..=x1 {
            let center = BlockPos::new(x, motion_blocking_height(w, x, z), z);
            // the bolts this column's strike makes, each with its chance, and whether a rod is under it
            let mut bolts: Vec<(BlockPos, f64, bool)> = Vec::new();
            let rods = closest_rods(&all_rods, center);
            if !rods.is_empty() {
                let n = rods.len() as f64;
                bolts.extend(rods.into_iter().map(|r| (r, 1.0 / n, true)));
            } else {
                let search = Aabb::new(center.x as f64, center.y as f64, center.z as f64, (center.x + 1) as f64, (WORLD_MAX_Y + 2) as f64, (center.z + 1) as f64).inflate(3.0, 3.0, 3.0);
                let ns = statics.iter().filter(|(_, bb)| bb.intersects(search)).count() as f64;
                let gs: Vec<&(BlockPos, Aabb, f64)> = golem_boxes.iter().filter(|g| g.1.intersects(search)).collect();
                let eg: f64 = gs.iter().map(|g| g.2).sum();
                // the static targets (the villagers and the zombie that see the sky) are always
                // there; a golem in its block with its chance; one of them uniformly
                if ns > 0.0 {
                    let den = ns + eg;
                    for (c, _) in statics.iter().filter(|(_, bb)| bb.intersects(search)) {
                        bolts.push((*c, 1.0 / den, false));
                    }
                    bolts.extend(gs.iter().map(|g| (g.0, g.2 / den, false)));
                } else {
                    let none: f64 = gs.iter().map(|g| 1.0 - g.2).product();
                    bolts.push((center, none, false));
                    if eg > 0.0 {
                        bolts.extend(gs.iter().map(|g| (g.0, (1.0 - none) * g.2 / eg, false)));
                    }
                }
            }
            columns.push((bolts, 1.0));
        }
    }
    for (bolts, columns) in columns {
        for (b, p, rod) in bolts {
            if p <= 0.0 || !rain_reaches(w, b) {
                continue;
            }
            let wgt = q * columns * p;
            let hit = hit_box_at(Vec3::new(b.x as f64 + 0.5, b.y as f64, b.z as f64 + 0.5));
            m.bolts += wgt;
            let witch = villagers.iter().any(|v| hit.intersects(*v));
            let broke = vehicle.iter().any(|v| hit.intersects(*v));
            m.witch += if witch { wgt } else { 0.0 };
            m.vehicle += if broke { wgt } else { 0.0 };
            m.iron += wgt * iron.iter().filter(|(c, _)| hit.intersects(Aabb::of_block(*c))).map(|(_, n)| n).sum::<f64>();
            let fire = fire_chance(w, b, &protect);
            m.fire += wgt * fire;
            m.breaks += wgt * if witch || broke { 1.0 } else { fire };
            let (hx, hy, hz) = (b.x as f64, b.y as f64, b.z as f64);
            if !rod && (on_farm.min_x..on_farm.max_x).contains(&hx) && (on_farm.min_y..on_farm.max_y).contains(&hy) && (on_farm.min_z..on_farm.max_z).contains(&hz) {
                m.trap += wgt * trap;
            }
        }
    }
    m
}

/// [`measure`] of a farm as the judge builds it, over the occupancy of `runs` (the judge's runs of
/// that build); with whether its zombie sees the sky (it burns by day: `Zombie.isSunBurnTick`).
pub fn measure_farm(spec: &crate::entity::mob::farm::FarmSpec, runs: &[crate::entity::mob::farm::RunReport]) -> Result<(Measure, bool), String> {
    use crate::entity::mob::farm::{Run, RunOpts, WalkMode};
    use crate::entity::mob::types::MobType;
    let run = Run::new(spec, &RunOpts::steady(0, 0, WalkMode::Random))?;
    let ticks: f64 = runs.iter().map(|r| r.cell_ticks as f64).sum::<f64>().max(1.0);
    let mut golems: std::collections::BTreeMap<[i32; 3], f64> = Default::default();
    let mut iron: std::collections::BTreeMap<[i32; 3], f64> = Default::default();
    for r in runs {
        for (c, n) in &r.golem_cells {
            *golems.entry(*c).or_insert(0.0) += *n as f64 / ticks;
        }
        for (c, n) in &r.drop_cells {
            *iron.entry(*c).or_insert(0.0) += *n as f64 / ticks;
        }
    }
    let at = |c: &[i32; 3]| BlockPos::new(c[0], c[1], c[2]);
    let golems: Vec<(BlockPos, f64)> = golems.iter().map(|(c, p)| (at(c), *p)).collect();
    let iron: Vec<(BlockPos, f64)> = iron.iter().filter(|(_, n)| **n > 0.0).map(|(c, n)| (at(c), *n)).collect();
    let w = &run.world;
    let sky = w.entities.list.iter().any(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Zombie) && can_see_sky(w, e.block_position()).unwrap_or(false));
    Ok((measure(w, &golems, &iron, spec.inner()), sky))
}
