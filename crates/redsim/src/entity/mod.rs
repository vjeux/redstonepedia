//! Entities (redsim M2; docs: designs/clocks/tracks/clockinfra/REDSIM-FLUIDS-ENTITIES.md):
//! boats, minecarts and item entities in ONE list, in insertion order (`entityTickList`), ticked
//! in the entity phase of [`World::tick`] (after block events, before block entities).
//!
//! Ownership: clockinfra owns this module's core (`mod.rs`, `vec3.rs`, `mth.rs`, `collide.rs`,
//! `fluid_push.rs`: `Entity.move`, collisions, `checkInsideBlocks`, fluid pushing); clk-misc owns
//! the kinds (`boat.rs`, `minecart.rs`, `item.rs`: `AbstractBoat`, `AbstractMinecart`, ...) and
//! the blocks that react to entities. The engine calls keep the game's names and order.
//!
//! STATUS: API skeleton. The list, its tick hook and the queries are live; `move_`,
//! `check_inside_blocks` and `update_fluid_height_and_do_fluid_pushing` are stubs that record a
//! model error (a run using them is not trustworthy) until M2 fills them. Falling blocks still
//! live in `gravity.rs` (they move into this list as a kind in M2, bit-identical).

pub mod armor_stand;
pub mod boat;
pub mod collide;
pub mod fluid_push;
pub mod frame;
pub mod item;
pub mod jmath;
pub mod fdlibm;
pub mod kinds;
pub mod leash;
pub mod minecart;
pub mod mth;
pub mod piston_push;
pub mod portal;
pub mod primed_tnt;
// ---- mob farms (ironfarm): villagers, iron golems, zombies (`mob/`) ----
pub mod mob;
pub mod push;
pub mod vec3;

use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;
pub use vec3::{Aabb, Vec3};

/// `MoverType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoverType {
    Self_,
    Player,
    Piston,
    ShulkerBox,
    Shulker,
}

/// What an entity is; the kind's own state is its payload (owned by the kind's module).
#[derive(Clone, PartialEq, Debug)]
pub enum EntityKind {
    Boat(boat::BoatState),
    Minecart(minecart::MinecartState),
    Item(item::ItemState),
    ArmorStand(armor_stand::ArmorStandState),
    /// Track tnt: a primed TNT (`primed_tnt.rs`), only with `World::tnt_kick`.
    PrimedTnt(primed_tnt::PrimedTntState),
    /// ironfarm: a mob (`mob/`: villager, iron golem, zombie).
    Mob(Box<mob::MobState>),
}

impl EntityKind {
    /// `EntityType.getDimensions`: (width, height).
    pub fn dims(&self) -> (f32, f32) {
        match self {
            EntityKind::Boat(_) => (boat::WIDTH, boat::HEIGHT),
            EntityKind::Minecart(_) => (minecart::WIDTH, minecart::HEIGHT),
            EntityKind::Item(_) => (item::WIDTH, item::HEIGHT),
            EntityKind::ArmorStand(_) => (armor_stand::WIDTH, armor_stand::HEIGHT),
            EntityKind::PrimedTnt(_) => (primed_tnt::WIDTH, primed_tnt::HEIGHT),
            EntityKind::Mob(m) => m.dims(),
        }
    }

    /// `EntityDimensions.eyeHeight` (`eyeHeight(..)` in `EntityTypes`, else height * 0.85F).
    pub fn eye_height(&self) -> f32 {
        match self {
            EntityKind::Boat(_) => 0.5625,
            EntityKind::Minecart(_) => minecart::HEIGHT * 0.85,
            EntityKind::Item(_) => 0.2125,
            EntityKind::ArmorStand(_) => 1.7775,
            EntityKind::PrimedTnt(_) => primed_tnt::EYE_HEIGHT,
            EntityKind::Mob(m) => m.eye(),
        }
    }
}

/// The `Entity` fields the engine reads and writes (the game's names).
#[derive(Clone, PartialEq, Debug)]
pub struct Entity {
    /// Insertion order (the tick order); never reused.
    pub id: u32,
    pub kind: EntityKind,
    /// `position()`.
    pub pos: Vec3,
    /// `getDeltaMovement()`.
    pub delta_movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub y_rot_o: f32,
    pub x_rot_o: f32,
    /// `xo, yo, zo` (the position at the start of the tick).
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    /// `xOld, yOld, zOld`.
    pub x_old: f64,
    pub y_old: f64,
    pub z_old: f64,
    /// `getBoundingBox()`.
    pub bb: Aabb,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub vertical_collision_below: bool,
    pub minor_horizontal_collision: bool,
    pub removed: bool,
    pub was_touching_water: bool,
    pub was_eye_in_water: bool,
    /// `fluidHeight` per fluid tag: [water, lava].
    pub fluid_height: [f64; 2],
    pub first_tick: bool,
    pub fall_distance: f64,
    pub tick_count: i32,
    // ---- movement (collide.rs, clk-misc) ----
    /// `movementThisTick` (recorded by `move`, replayed by `applyEffectsFromBlocks`).
    pub movement_this_tick: Vec<collide::Movement>,
    /// `finalMovementsThisTick`.
    pub final_movements: Vec<collide::Movement>,
    /// `mainSupportingBlockPos`.
    pub main_support: Option<BlockPos>,
    /// `onGroundNoBlocks`.
    pub on_ground_no_blocks: bool,
    /// `stuckSpeedMultiplier` (cobwebs and the like: never set by a modeled block).
    pub stuck_speed: Vec3,
    /// `portalCooldown` and the `PortalProcessor` (`entity::portal`).
    pub portal_cooldown: i32,
    pub portal: Option<portal::PortalProc>,
    pub no_physics: bool,
    /// `lastKnownPosition` / `lastKnownSpeed` (`computeSpeed`).
    pub last_known_position: Option<Vec3>,
    pub last_known_speed: Vec3,
    pub invulnerable_time: i32,
    /// The game's network id (`getId()`), which some logic reads (`ItemEntity.tick`'s
    /// `(tickCount + getId()) % 4`): the insertion id unless a trace gives the game's.
    pub network_id: i32,
    /// `EntityFluidInteraction` (trackers, current): `fluid_push.rs`.
    pub fluid: fluid_push::FluidInteraction,
    // ---- piston pushing (piston_push.rs, clk-misc) ----
    /// `pistonDeltas` (how far pistons moved it per axis this game tick, each within +-0.51)
    /// and `pistonDeltasGameTime`.
    pub piston_deltas: [f64; 3],
    pub piston_deltas_time: i64,
    // ---- explosions (explosion.rs, clk-misc) ----
    /// `VehicleEntity.getDamage()` (boats, minecarts): +10 per point of damage, destroyed past
    /// 40, -1 per tick.
    pub vehicle_damage: f32,
    // ---- the entity's own random (clk-misc, 2026-10-21: items pushed into blocks) ----
    /// `Entity.random`, when the bridge seeds it: a summoned entity's is reseeded to
    /// `0x6d63746173 + its summon index` (`Sandbox.summon`), a popResource drop's to the drop's own
    /// seed, past its six placing draws (`rng::Drop`). `None`: unknown (a draw from it is a model
    /// error).
    pub random: Option<crate::rng::LegacyRandom>,
    /// The bridge's summon index of a summoned (design) entity: its UUID's number, the count of
    /// design entities alive when it came, past any number one of them still holds.
    pub summon_index: Option<u32>,
    /// The entity section it is in (`SectionPos` of its `blockPosition()`: x, y, z >> 4) and when
    /// it entered it (`Entities::section_counter`): `EntitySectionStorage` lists an entity at the
    /// end of its section when it enters, and level queries visit sections in key order
    /// ([`World::get_entities`]).
    pub section: [i32; 3],
    pub section_seq: u64,
    // ---- passengers (clk-misc 2026-10-21, the rail study: `/ride`) ----
    /// `vehicle`: the id of the entity it rides.
    pub vehicle: Option<u32>,
    /// `passengers`: the ids of the entities riding it, in order.
    pub passengers: Vec<u32>,
    /// Resolved server-side `LeashData` (ordinary boat-to-entity attachments).
    pub leash: Option<leash::LeashData>,
    /// `boardingCooldown` (ironfarm, the carousel): 60 when it leaves a vehicle, down 1 each
    /// `baseTick`; a rider boards by itself (`startRiding` without force) only at 0.
    pub boarding_cooldown: i32,
}

/// The entity section of a position (`SectionPos.asLong(BlockPos.containing(pos))`'s x, y, z).
pub fn section_of(pos: Vec3) -> [i32; 3] {
    [mth::floor(pos.x) >> 4, mth::floor(pos.y) >> 4, mth::floor(pos.z) >> 4]
}

/// A section's place in a level query's visit order (`EntitySectionStorage
/// .forEachAccessibleNonEmptySection`: x ascending, then within an x the section keys ascending:
/// z then y as unsigned 22- and 20-bit fields).
pub fn section_order(s: [i32; 3]) -> (i32, u32, u32) {
    (s[0], (s[2] as u32) & 0x3F_FFFF, (s[1] as u32) & 0xF_FFFF)
}

/// `EntityDimensions.makeBoundingBox(x, y, z)`: float half width and height, widened.
pub fn make_bounding_box(pos: Vec3, width: f32, height: f32) -> Aabb {
    let w = (width / 2.0) as f64;
    let h = height as f64;
    Aabb::new(pos.x - w, pos.y, pos.z - w, pos.x + w, pos.y + h, pos.z + w)
}

impl Entity {
    /// A new entity at `pos` (as `EntityType.create` + `moveTo` would leave it).
    pub fn new(id: u32, kind: EntityKind, pos: Vec3, y_rot: f32) -> Entity {
        let (w, h) = kind.dims();
        Entity {
            id,
            kind,
            pos,
            delta_movement: Vec3::ZERO,
            y_rot,
            x_rot: 0.0,
            y_rot_o: y_rot,
            x_rot_o: 0.0,
            xo: pos.x,
            yo: pos.y,
            zo: pos.z,
            x_old: pos.x,
            y_old: pos.y,
            z_old: pos.z,
            bb: make_bounding_box(pos, w, h),
            on_ground: false,
            horizontal_collision: false,
            vertical_collision: false,
            vertical_collision_below: false,
            minor_horizontal_collision: false,
            removed: false,
            was_touching_water: false,
            was_eye_in_water: false,
            fluid_height: [0.0; 2],
            first_tick: true,
            fall_distance: 0.0,
            tick_count: 0,
            movement_this_tick: Vec::new(),
            final_movements: Vec::new(),
            main_support: None,
            on_ground_no_blocks: false,
            stuck_speed: Vec3::ZERO,
            portal_cooldown: 0,
            portal: None,
            no_physics: false,
            last_known_position: None,
            last_known_speed: Vec3::ZERO,
            invulnerable_time: 0,
            network_id: id as i32,
            fluid: Default::default(),
            piston_deltas: [0.0; 3],
            piston_deltas_time: 0,
            vehicle_damage: 0.0,
            random: None,
            summon_index: None,
            section: section_of(pos),
            section_seq: 0,
            vehicle: None,
            passengers: Vec::new(),
            leash: None,
            boarding_cooldown: 0,
        }
    }

    /// `setPos`: the position and the bounding box made from it.
    pub fn set_pos(&mut self, pos: Vec3) {
        self.pos = pos;
        let (w, h) = self.kind.dims();
        self.bb = make_bounding_box(pos, w, h);
    }

    /// `blockPosition()`.
    pub fn block_position(&self) -> BlockPos {
        BlockPos::new(mth::floor(self.pos.x), mth::floor(self.pos.y), mth::floor(self.pos.z))
    }
}

/// The level's entities, in insertion order.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Entities {
    pub list: Vec<Entity>,
    pub next_id: u32,
    /// `PistonMovingBlockEntity.NOCLIP`: the direction of the piston move under way (set only
    /// inside `piston_push::move_entity_by_piston`, None between calls).
    pub noclip: Option<crate::pos::Dir>,
    /// Section entries so far (`Entity::section_seq`).
    pub section_counter: u64,
}

/// The top-level keys of an SNBT compound (`{Motion:[..],Items:[{..}]}` -> Motion, Items).
fn snbt_keys(nbt: &str) -> Vec<String> {
    let s = nbt.trim();
    let inner = s.strip_prefix('{').and_then(|x| x.strip_suffix('}')).unwrap_or(s);
    let mut keys = Vec::new();
    let (mut depth, mut quote, mut at_key) = (0i32, false, true);
    let mut cur = String::new();
    for ch in inner.chars() {
        if quote {
            if ch == '"' {
                quote = false;
            }
            continue;
        }
        match ch {
            '"' => quote = true,
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                at_key = true;
                cur.clear();
                continue;
            }
            ':' if depth == 0 && at_key => {
                keys.push(cur.trim().to_string());
                at_key = false;
                continue;
            }
            _ => {}
        }
        if at_key && depth == 0 {
            cur.push(ch);
        }
    }
    keys
}

/// `summon <type> x y z [nbt]` as the judge summons a design's entities (`entity:` lines): the
/// kind from the type id (payload defaults; the kinds' owners refine them from the NBT), the
/// position, and `Rotation:[yaw, pitch]` from the NBT when given. Returns the entity's id. NBT
/// redsim does not model (container contents...) is refused.
pub fn spawn(w: &mut World, type_id: &str, pos: [f64; 3], nbt: Option<&str>) -> Result<u32, String> {
    let id = type_id.strip_prefix("minecraft:").unwrap_or(type_id);
    // ironfarm: mobs, and a vehicle carrying one (`Passengers:[{id:"minecraft:zombie"}]`)
    if let Some(r) = mob::spawn_hook(w, id, pos, nbt) {
        return r;
    }
    if id == "falling_block" {
        return spawn_falling_block(w, pos, nbt);
    }
    // (clk-item) item frames: comparator inputs, not moving entities (`frame`)
    if id == "item_frame" {
        if let Some(n) = nbt
            && let Some(k) = snbt_keys(n).into_iter().find(|k| !["Facing", "Item", "ItemRotation"].contains(&k.as_str()))
        {
            return Err(format!("entity {type_id} with {k} in its data ({n}) is not modeled by redsim"));
        }
        frame::spawn(w, pos, nbt)?;
        return Ok(u32::MAX);
    }
    // (clk-misc) container entities: their contents (Items) and size
    let container_size = match id {
        "hopper_minecart" => Some(5),
        _ if id.ends_with("_chest_boat") || id == "bamboo_chest_raft" => Some(27),
        _ => None,
    };
    if let Some(n) = nbt {
        let allowed: &[&str] = if id == "item" {
            &["Motion", "Rotation", "Item", "PickupDelay", "Age"]
        } else if container_size.is_some() {
            &["Motion", "Rotation", "Items"]
        } else {
            &["Motion", "Rotation"]
        };
        if let Some(k) = snbt_keys(n).into_iter().find(|k| !allowed.contains(&k.as_str())) {
            return Err(format!("entity {type_id} with {k} in its data ({n}) is not modeled by redsim"));
        }
    }
    let kind = match id {
        "oak_boat" | "spruce_boat" | "birch_boat" | "jungle_boat" | "acacia_boat" | "dark_oak_boat" | "mangrove_boat" | "cherry_boat" | "pale_oak_boat" | "bamboo_raft" => EntityKind::Boat(boat::BoatState::default()),
        "oak_chest_boat" | "spruce_chest_boat" | "birch_chest_boat" | "jungle_chest_boat" | "acacia_chest_boat" | "dark_oak_chest_boat" | "mangrove_chest_boat" | "cherry_chest_boat" | "pale_oak_chest_boat" | "bamboo_chest_raft" => {
            EntityKind::Boat(boat::BoatState { chest: true, ..Default::default() })
        }
        "minecart" => EntityKind::Minecart(minecart::MinecartState::default()),
        "hopper_minecart" => EntityKind::Minecart(minecart::MinecartState { hopper: true, enabled: true, ..Default::default() }),
        "armor_stand" => EntityKind::ArmorStand(armor_stand::ArmorStandState::default()),
        "item" => EntityKind::Item(item::ItemState::from_nbt(nbt)),
        _ => return Err(format!("entity {type_id} is not modeled by redsim")),
    };
    let items = match (container_size, nbt) {
        (Some(_), Some(n)) => crate::redstone::container::entity_items(n).map_err(|e| format!("entity {type_id}: {e}"))?,
        _ => Vec::new(),
    };
    let (mut yaw, mut pitch) = (0.0f32, 0.0f32);
    if let Some(n) = nbt
        && let Some(i) = n.find("Rotation:[")
    {
        let rest = &n[i + "Rotation:[".len()..];
        let inner = rest.split(']').next().unwrap_or("");
        let v: Vec<f32> = inner.split(',').filter_map(|s| s.trim().trim_end_matches(['f', 'F', 'd', 'D']).parse().ok()).collect();
        if let Some(y) = v.first() {
            yaw = *y;
        }
        if let Some(p) = v.get(1) {
            pitch = *p;
        }
    }
    // (`Entity.load` ends with `setRot`: the loaded yaw's remainder by 360, Java's, the pitch's too)
    let id = w.entities.add(kind, Vec3::new(pos[0], pos[1], pos[2]), yaw % 360.0);
    // the bridge's summon numbering (`Sandbox.summon`): after the design entities alive (item
    // frames, which never leave, among them), past any number a live one still holds; its own
    // random reseeded to 0x6d63746173 + that number
    let live: Vec<u32> = w.entities.list.iter().filter(|e| !e.removed).filter_map(|e| e.summon_index).collect();
    let mut index = (live.len() + w.frames.len()) as u32;
    while live.contains(&index) {
        index += 1;
    }
    if let Some(e) = w.entities.list.last_mut() {
        e.summon_index = Some(index);
        e.random = Some(crate::rng::LegacyRandom::new(0x6d_6374_6173_i64 + index as i64));
        e.x_rot = pitch;
        e.x_rot_o = pitch;
        // Motion:[x, y, z] (doubles)
        if let Some(n) = nbt
            && let Some(i) = n.find("Motion:[")
        {
            let inner = n[i + "Motion:[".len()..].split(']').next().unwrap_or("");
            let v: Vec<f64> = inner.split(',').filter_map(|s| s.trim().trim_end_matches(['d', 'D', 'f', 'F']).parse().ok()).collect();
            if v.len() == 3 {
                e.delta_movement = Vec3::new(v[0], v[1], v[2]);
            }
        }
    }
    if let Some(size) = container_size {
        let mut c = crate::redstone::container::ContainerBe::entity(size);
        for (slot, st) in items {
            if slot < size {
                c.slots[slot] = Some(st);
            }
        }
        w.containers.entities.insert(id, c);
    }
    Ok(id)
}

/// `summon falling_block x y z {BlockState:"<block>" or {id:"<block>", properties:{...}}, Time:n,
/// Motion:[0d, y, 0d]}` (26.3's `BlockState.CODEC`: a block name or `{id, properties}`; the old
/// `{Name, Properties}` fails to decode in the game, which then falls back to sand, so it is
/// refused): a [`crate::gravity::FallingBlock`] at that exact position, taking the next entity id.
/// Other keys (DropItem, HurtEntities...) and horizontal motion are refused.
fn spawn_falling_block(w: &mut World, pos: [f64; 3], nbt: Option<&str>) -> Result<u32, String> {
    let n = nbt.ok_or("entity falling_block needs its BlockState")?;
    if let Some(k) = snbt_keys(n).into_iter().find(|k| !["BlockState", "Time", "Motion"].contains(&k.as_str())) {
        return Err(format!("entity falling_block with {k} in its data ({n}) is not modeled by redsim"));
    }
    let bs = n.find("BlockState:").map(|i| n[i + "BlockState:".len()..].trim_start()).ok_or("falling_block: no BlockState")?;
    let quoted = |s: &str| -> Option<String> {
        let s = s.trim_start();
        let s = s.strip_prefix('"').or_else(|| s.strip_prefix('\''))?;
        Some(s[..s.find(['"', '\''])?].to_string())
    };
    let text = if bs.starts_with('{') {
        // the compound: up to its matching brace
        let mut depth = 0;
        let mut end = bs.len();
        for (k, ch) in bs.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let bs = &bs[..end];
        if bs.contains("Name:") {
            return Err(format!("falling_block: BlockState {bs} is the old format; 26.3 reads \"<block>\" or {{id:\"<block>\",properties:{{...}}}} (the old one decodes to sand)"));
        }
        let name = bs.find("id:").and_then(|i| quoted(&bs[i + 3..])).ok_or("falling_block: BlockState without id")?;
        let mut props = Vec::new();
        if let Some(i) = bs.find("properties:{") {
            let inner = bs[i + "properties:{".len()..].split('}').next().unwrap_or("");
            for kv in inner.split(',').filter(|s| !s.trim().is_empty()) {
                let (k, v) = kv.split_once(':').ok_or("falling_block: bad properties")?;
                props.push(format!("{}={}", k.trim().trim_matches(['"', '\'']), quoted(v).unwrap_or_else(|| v.trim().to_string())));
            }
        }
        if props.is_empty() { name } else { format!("{name}[{}]", props.join(",")) }
    } else {
        quoted(bs).ok_or("falling_block: BlockState is neither a block name nor {id, properties}")?
    };
    let block = crate::block::Block::parse(&text).map_err(|e| format!("falling_block: {text}: {e}"))?;
    let time = n
        .find("Time:")
        .map(|i| n[i + 5..].chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect::<String>())
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let mut vy = 0.0;
    if let Some(i) = n.find("Motion:[") {
        let inner = n[i + "Motion:[".len()..].split(']').next().unwrap_or("");
        let v: Vec<f64> = inner.split(',').filter_map(|s| s.trim().trim_end_matches(['d', 'D', 'f', 'F']).parse().ok()).collect();
        if v.len() != 3 || v[0] != 0.0 || v[2] != 0.0 {
            return Err(format!("falling_block: horizontal motion is not modeled ({inner})"));
        }
        vy = v[1];
    }
    let id = w.entities.next_id;
    w.entities.next_id += 1;
    w.gravity.entities.push(crate::gravity::FallingBlock::new(block, pos[0], pos[1], pos[2], vy, time, id));
    Ok(id)
}

impl Entities {
    /// `addFreshEntity`: appended (ticked after every older entity). Returns its id.
    pub fn add(&mut self, kind: EntityKind, pos: Vec3, y_rot: f32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let mut e = Entity::new(id, kind, pos, y_rot);
        e.section_seq = self.section_counter;
        self.section_counter += 1;
        self.list.push(e);
        id
    }

    /// `Entity.setPos` of entity `i`: its position and box, and when its block position moves
    /// it to another section (`PersistentEntitySectionManager.Callback.onMove`), its entry
    /// there, at the end.
    pub fn set_pos(&mut self, i: usize, pos: Vec3) {
        let e = &mut self.list[i];
        e.set_pos(pos);
        let s = section_of(pos);
        if s != e.section {
            e.section = s;
            e.section_seq = self.section_counter;
            self.section_counter += 1;
        }
    }

    /// `Entity.setPosRaw`: the position (and its section) without the bounding box, which keeps
    /// its old place until the next move (a resting bat's snap to its ceiling).
    pub fn set_pos_raw(&mut self, i: usize, pos: Vec3) {
        let e = &mut self.list[i];
        e.pos = pos;
        let s = section_of(pos);
        if s != e.section {
            e.section = s;
            e.section_seq = self.section_counter;
            self.section_counter += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.iter().all(|e| e.removed)
    }
}

/// The entity phase: every live entity's `commonTick` then `tick` (`ServerLevel.tickNonPassenger`),
/// in insertion order; removed ones leave the list.
pub fn tick_all(w: &mut World) {
    if w.entities.list.is_empty() {
        return;
    }
    let n = w.entities.list.len();
    for i in 0..n {
        tick_one(w, i);
    }
    w.entities.list.retain(|e| !e.removed);
}

/// One entity's `tickNonPassenger` (the caller drops the removed ones afterwards); none outside
/// the entity-ticking chunks (`World::entity_ticks_at`). A passenger whose vehicle is alive and
/// carries it is skipped (it ticks after its vehicle: [`tick_passenger`]); one whose vehicle is
/// gone stops riding first (`ServerLevel.tick`).
pub fn tick_one(w: &mut World, i: usize) {
    if w.entities.list[i].removed {
        return;
    }
    // an entity outside the chunks whose sections tick is not in `ServerLevel.entityTickList` (its
    // section's visibility is TRACKED: `PersistentEntitySectionManager` stops ticking it as it
    // moves there), so neither its checkDespawn nor its tick runs; the ticking sections are the
    // bridge's `canSpawnEntitiesInChunk` list when known (the natural spawner's), else the
    // entity-ticking box
    let p = w.entities.list[i].pos;
    let in_tick_list = match w.mob.natural.as_ref().filter(|n| !n.entity_ticking.is_empty()) {
        Some(n) => {
            let (cx, cz) = ((p.x.floor() as i64).div_euclid(16) as i32, (p.z.floor() as i64).div_euclid(16) as i32);
            n.entity_ticking.iter().any(|c| c[0] == cx && c[1] == cz)
        }
        None => w.entity_ticks_at(p.x, p.z),
    };
    if !in_tick_list {
        let id = w.entities.list[i].id;
        w.note_frozen(id);
        return;
    }
    // `ServerLevel.tick`: `checkDespawn` before the passenger test (terrain mode's natural mobs)
    let natural_kind = matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if matches!(m.ty, mob::types::MobType::Natural(_)));
    if w.mob.natural.is_some() && natural_kind {
        mob::natural::check_despawn(w, i);
        if w.entities.list[i].removed {
            return;
        }
    } else if w.mob.ai.on && matches!(w.entities.list[i].kind, EntityKind::Mob(_)) {
        // ---- mob AI (the mob-AI lane): checkDespawn for the other mobs (villagers, golems,
        // zombies, ...) against every player the world has ----
        mob::ai::check_despawn(w, i);
    }
    // `inEntityTickingRange`: only there does it tick
    if !w.entity_ticks_at(p.x, p.z) {
        let id = w.entities.list[i].id;
        w.note_frozen(id);
        return;
    }
    if let Some(v) = w.entities.list[i].vehicle {
        let me = w.entities.list[i].id;
        let carried = w.entities.list.iter().any(|e| e.id == v && !e.removed && e.passengers.contains(&me));
        if carried {
            return;
        }
        stop_riding(w, i);
    }
    common_tick(w, i);
    match w.entities.list[i].kind {
        EntityKind::Boat(_) => boat::tick(w, i),
        EntityKind::Minecart(_) => minecart::tick(w, i),
        EntityKind::Item(_) => item::tick(w, i),
        EntityKind::ArmorStand(_) => armor_stand::tick(w, i),
        EntityKind::PrimedTnt(_) => primed_tnt::tick(w, i),
        EntityKind::Mob(_) => mob::tick(w, i),
    }
    let me = w.entities.list[i].id;
    let riders = w.entities.list[i].passengers.clone();
    for r in riders {
        tick_passenger(w, me, r);
    }
}

/// The index of the live or removed entity with this id.
pub fn index_of(w: &World, id: u32) -> Option<usize> {
    w.entities.list.iter().position(|e| e.id == id)
}

/// `ServerLevel.tickPassenger(vehicle, entity)`: a passenger that left (removed, or riding something
/// else) stops riding; otherwise its `commonTick` and `rideTick`, then its own passengers.
fn tick_passenger(w: &mut World, vehicle: u32, rider: u32) {
    let Some(j) = index_of(w, rider) else { return };
    if w.entities.list[j].removed || w.entities.list[j].vehicle != Some(vehicle) {
        stop_riding(w, j);
        return;
    }
    common_tick(w, j);
    ride_tick(w, j);
    let riders = w.entities.list[j].passengers.clone();
    for r in riders {
        tick_passenger(w, rider, r);
    }
}

/// `Entity.rideTick` (`LivingEntity.rideTick`: then `resetFallDistance`): the motion zeroed, the
/// entity's own tick (an armor stand's `travel` runs with no input: it moves by nothing, and
/// gravity leaves it -0.0784 a tick), then its vehicle puts it in its seat (`positionRider`).
fn ride_tick(w: &mut World, j: usize) {
    // ironfarm: a riding mob's rideTick, seat and fall reset (`mob::ride_tick`)
    if matches!(w.entities.list[j].kind, EntityKind::Mob(_)) {
        mob::ride_tick(w, j);
        return;
    }
    w.entities.list[j].delta_movement = Vec3::ZERO;
    match w.entities.list[j].kind {
        EntityKind::ArmorStand(_) => armor_stand::tick(w, j),
        ref k => {
            w.errors.push(format!("a riding {k:?} (rideTick) is not modeled"));
            return;
        }
    }
    if let Some(v) = w.entities.list[j].vehicle
        && let Some(vi) = index_of(w, v)
    {
        position_rider(w, vi, j);
    }
    if matches!(w.entities.list[j].kind, EntityKind::ArmorStand(_)) {
        w.entities.list[j].fall_distance = 0.0;
    }
}

/// `Entity.positionRider(passenger)`: the passenger's position is the vehicle's plus its
/// passenger attachment point (a minecart's: 0.1875 up, the same at every yaw), minus the
/// passenger's vehicle attachment (at its feet for every modeled rider), `setPos`.
pub fn position_rider(w: &mut World, vi: usize, j: usize) {
    let v = &w.entities.list[vi];
    let rider = w.entities.list[j].id;
    if !v.passengers.contains(&rider) {
        return;
    }
    let up = match v.kind {
        EntityKind::Minecart(_) => 0.1875f32 as f64,
        ref k => {
            w.errors.push(format!("a passenger of {k:?} (its attachment points) is not modeled"));
            return;
        }
    };
    let p = v.pos.add(0.0, up, 0.0);
    w.entities.set_pos(j, p);
}

/// `Entity.stopRiding` of entity `i` (`removeVehicle`; a living one then `dismountVehicle`): it
/// leaves its vehicle's passengers (60 gt boarding cooldown, not modeled: no rider boards by itself
/// here) and, alive, a living rider is moved off: when the vehicle was removed, at its own x and z
/// and the higher of the two y (`findFreePosition` is not modeled: a model error if the rider is
/// still alive at the next tick, [`World::dismounted`]); a vehicle still there (an activator rail's
/// eject) is not modeled. ENTITY_DISMOUNT at the old vehicle's position (a living, unremoved rider).
/// `LivingEntity.dismountVehicle` from a removed vehicle: at its own x and z and the higher of the
/// two y, a small rider (at most 4 x 4) moved to `findFreePosition` around its centre there (the
/// free part of its box-sized centre region: block boxes inflated by half its size taken away,
/// the point of what is left closest to the centre), then `dismountTo` (`teleportTo`: `snapTo`).
fn dismount_from_removed(w: &mut World, i: usize, vpos: Option<Vec3>) {
    let e = &w.entities.list[i];
    let (wd, ht) = match &e.kind {
        EntityKind::Mob(m) => {
            let (a, b) = m.dims();
            (a as f64, b as f64)
        }
        _ => return,
    };
    let y = e.pos.y.max(vpos.map_or(e.pos.y, |p| p.y));
    let mut target = Vec3::new(e.pos.x, y, e.pos.z);
    if wd <= 4.0 && ht <= 4.0 {
        let half = ht / 2.0;
        let center = target.add(0.0, half, 0.0);
        let allowed = Aabb::new(center.x - wd / 2.0, center.y - ht / 2.0, center.z - wd / 2.0, center.x + wd / 2.0, center.y + ht / 2.0, center.z + wd / 2.0);
        if let Some(p) = find_free_position(w, allowed, center, wd, ht, wd) {
            target = p.add(0.0, -half, 0.0);
        }
    }
    w.entities.set_pos(i, target);
    let e = &mut w.entities.list[i];
    (e.xo, e.yo, e.zo) = (target.x, target.y, target.z);
    (e.x_old, e.y_old, e.z_old) = (target.x, target.y, target.z);
}

/// `Entity.fudgePositionAfterSizeChange(previous)` of a mob grown from (`ow`, `oh`): the free
/// position closest to the old centre among the centres within the growth (+1e-6) of it, else for
/// a box grown both ways the same ignoring y with the old height; `setPos` there. True when moved.
pub fn fudge_position_after_size_change(w: &mut World, i: usize, ow: f32, oh: f32) -> bool {
    let (nw, nh) = match &w.entities.list[i].kind {
        EntityKind::Mob(m) => m.dims(),
        _ => return false,
    };
    let pos = w.entities.list[i].pos;
    let old_center = pos.add(0.0, oh as f64 / 2.0, 0.0);
    let wd = (nw - ow).max(0.0) as f64 + 1.0e-6;
    let hd = (nh - oh).max(0.0) as f64 + 1.0e-6;
    let of_size = |c: Vec3, x: f64, y: f64, z: f64| Aabb::new(c.x - x / 2.0, c.y - y / 2.0, c.z - z / 2.0, c.x + x / 2.0, c.y + y / 2.0, c.z + z / 2.0);
    if let Some(p) = find_free_position(w, of_size(old_center, wd, hd, wd), old_center, nw as f64, nh as f64, nw as f64) {
        w.entities.set_pos(i, p.add(0.0, -(nh as f64) / 2.0, 0.0));
        return true;
    }
    if nw > ow && nh > oh
        && let Some(p) = find_free_position(w, of_size(old_center, wd, 1.0e-6, wd), old_center, nw as f64, oh as f64, nw as f64)
    {
        w.entities.set_pos(i, p.add(0.0, -(oh as f64) / 2.0 + 1.0e-6, 0.0));
        return true;
    }
    false
}

/// `CollisionGetter.findFreePosition(source, allowedCenters (one box), preferredCenter, sx, sy,
/// sz)`: the block collisions over the box inflated by the size, each box inflated by half the
/// size and taken from the allowed box (`Shapes.join(.., ONLY_FIRST)`), then `closestPointTo`.
pub fn find_free_position(w: &World, allowed: Aabb, p: Vec3, sx: f64, sy: f64, sz: f64) -> Option<Vec3> {
    let search = allowed.inflate(sx, sy, sz);
    let blocks: Vec<Aabb> = collide::block_collisions(w, search).into_iter().flat_map(|(_, b)| b).map(|b| b.inflate(sx / 2.0, sy / 2.0, sz / 2.0)).collect();
    // the allowed box cut along every inflated box's faces inside it: a cell is free when no box
    // holds its centre
    let axis = |lo: f64, hi: f64, f: &dyn Fn(&Aabb) -> (f64, f64)| -> Vec<f64> {
        let mut v = vec![lo, hi];
        for b in &blocks {
            let (a, c) = f(b);
            for x in [a, c] {
                if x > lo && x < hi {
                    v.push(x);
                }
            }
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup();
        v
    };
    let xs = axis(allowed.min_x, allowed.max_x, &|b| (b.min_x, b.max_x));
    let ys = axis(allowed.min_y, allowed.max_y, &|b| (b.min_y, b.max_y));
    let zs = axis(allowed.min_z, allowed.max_z, &|b| (b.min_z, b.max_z));
    let mut best: Option<(f64, Vec3)> = None;
    for xi in 0..xs.len() - 1 {
        for yi in 0..ys.len() - 1 {
            for zi in 0..zs.len() - 1 {
                let c = Vec3::new((xs[xi] + xs[xi + 1]) / 2.0, (ys[yi] + ys[yi + 1]) / 2.0, (zs[zi] + zs[zi + 1]) / 2.0);
                if blocks.iter().any(|b| b.min_x < c.x && c.x < b.max_x && b.min_y < c.y && c.y < b.max_y && b.min_z < c.z && c.z < b.max_z) {
                    continue;
                }
                let q = Vec3::new(p.x.clamp(xs[xi], xs[xi + 1]), p.y.clamp(ys[yi], ys[yi + 1]), p.z.clamp(zs[zi], zs[zi + 1]));
                let d = p.distance_to_sqr(q);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, q));
                }
            }
        }
    }
    best.map(|(_, q)| q)
}

/// `Entity.setRemoved`'s `getPassengers().forEach(Entity::stopRiding)`: each passenger of a
/// removed entity dismounts now.
pub fn eject_passengers(w: &mut World, i: usize) {
    let riders = w.entities.list[i].passengers.clone();
    for r in riders {
        if let Some(j) = index_of(w, r) {
            stop_riding(w, j);
        }
    }
}

pub fn stop_riding(w: &mut World, i: usize) {
    let Some(v) = w.entities.list[i].vehicle.take() else { return };
    let me = w.entities.list[i].id;
    let vi = index_of(w, v);
    let mut vpos = None;
    let mut vehicle_removed = true;
    if let Some(vi) = vi {
        w.entities.list[vi].passengers.retain(|p| *p != me);
        vpos = Some(w.entities.list[vi].pos);
        vehicle_removed = w.entities.list[vi].removed;
    }
    // `removePassenger`: `passenger.boardingCooldown = 60`
    w.entities.list[i].boarding_cooldown = 60;
    let removed = w.entities.list[i].removed;
    // ENTITY_DISMOUNT (a removed rider was killed or discarded: `shouldDestroy`)
    if let Some(vp) = vpos {
        w.game_event_at("entity_dismount", [vp.x, vp.y, vp.z], None, false);
    }
    // (ironfarm, the carousel) `LivingEntity.dismountVehicle`: a living rider of a minecart that
    // stays (an activator rail's eject) is teleported to `getDismountLocationForPassenger`
    if !removed
        && !vehicle_removed
        && let EntityKind::Mob(m) = &w.entities.list[i].kind
        && let Some(vi) = vi
        && matches!(w.entities.list[vi].kind, EntityKind::Minecart(_))
    {
        let d = m.ty.def();
        let to = minecart::dismount_location(w, vi, d.width as f64, d.height as f64);
        w.entities.set_pos(i, to);
        let e = &mut w.entities.list[i];
        // (`teleportTo`: `teleportSetPosition`, the old position too, the motion zero)
        (e.xo, e.yo, e.zo) = (to.x, to.y, to.z);
        e.delta_movement = Vec3::ZERO;
        return;
    }
    if !removed {
        if matches!(w.entities.list[i].kind, EntityKind::Mob(_)) {
            if vehicle_removed {
                dismount_from_removed(w, i, vpos);
            } else {
                w.errors.push("a mob dismounting from a vehicle that stays (getDismountLocationForPassenger) is not modeled".into());
            }
        } else if matches!(w.entities.list[i].kind, EntityKind::ArmorStand(_)) {
            if vehicle_removed {
                let e = &w.entities.list[i];
                let y = e.pos.y.max(vpos.map_or(e.pos.y, |p| p.y));
                let p = Vec3::new(e.pos.x, y, e.pos.z);
                w.entities.set_pos(i, p);
                w.dismounted.push(me);
            } else {
                w.errors.push("a rider dismounting from a vehicle that stays (getDismountLocationForPassenger) is not modeled".into());
            }
        }
    }
}

/// `/ride <passenger> mount <vehicle>` (`RideCommand.mount`, then `Entity.startRiding(vehicle,
/// force, sendEventAndTriggers)`): the passenger joins the vehicle's passengers and an
/// ENTITY_MOUNT game event goes off at the vehicle's position. It takes its seat at the vehicle's
/// next tick ([`position_rider`] after its `rideTick`).
pub fn ride(w: &mut World, passenger: u32, vehicle: u32) -> Result<(), String> {
    let (Some(pi), Some(vi)) = (index_of(w, passenger), index_of(w, vehicle)) else { return Err(format!("ride: no entity {passenger} or {vehicle}")) };
    if w.entities.list[pi].removed || w.entities.list[vi].removed {
        return Err("ride: a removed entity".into());
    }
    if w.entities.list[pi].vehicle.is_some() {
        return Err("ride: the passenger already rides".into());
    }
    if passenger == vehicle || w.entities.list[pi].passengers.contains(&vehicle) {
        return Err("ride: a mounting loop".into());
    }
    if !matches!(w.entities.list[vi].kind, EntityKind::Minecart(_)) || !matches!(w.entities.list[pi].kind, EntityKind::ArmorStand(_)) {
        return Err(format!("ride: {:?} riding {:?} is not modeled (an armor stand in a minecart is)", w.entities.list[pi].kind, w.entities.list[vi].kind));
    }
    w.entities.list[pi].vehicle = Some(vehicle);
    w.entities.list[vi].passengers.push(passenger);
    let vp = w.entities.list[vi].pos;
    w.game_event_at("entity_mount", [vp.x, vp.y, vp.z], None, false);
    Ok(())
}

/// `/kill <entity>` (`Entity.kill`: removed as KILLED, its passengers stop riding, ENTITY_DIE at
/// its position; an armor stand the same through `hurtServer` with genericKill). A container
/// minecart drops its contents first (`AbstractMinecartContainer.remove`: `Containers.dropContents`
/// draws the drop spot's three `nextDouble` from the level random for EVERY slot, empty ones too);
/// a stack left in it would drop as items: not modeled, so it must be empty.
pub fn kill(w: &mut World, id: u32) -> Result<(), String> {
    let Some(i) = index_of(w, id) else { return Err(format!("kill: no entity {id}")) };
    if w.entities.list[i].removed {
        return Ok(());
    }
    if w.entities.list[i].leash.is_some() {
        return Err("kill: a leashed entity drops an unseeded lead ItemEntity (not modeled)".into());
    }
    if let Some(c) = w.containers.entities.get(&id) {
        if c.slots.iter().any(|s| s.is_some()) {
            return Err("kill: a container minecart that holds items would drop them (not modeled): empty it first".into());
        }
        let n = c.slots.len();
        if let Some(r) = w.level_random.as_mut() {
            for _ in 0..n * 3 {
                r.next_double();
            }
        }
    }
    w.entities.list[i].removed = true;
    stop_riding(w, i);
    let riders = w.entities.list[i].passengers.clone();
    for r in riders {
        if let Some(j) = index_of(w, r) {
            stop_riding(w, j);
        }
    }
    let p = w.entities.list[i].pos;
    w.game_event_at("entity_die", [p.x, p.y, p.z], None, false);
    Ok(())
}

/// The entity phase (`ServerLevel.tick` over `entityTickList`): every entity in the order it
/// joined, falling blocks ([`crate::gravity`]) and the rest together (they share one id counter, so
/// id order is spawn order); an entity spawned during the phase ticks from the next tick on.
pub fn tick_phase(w: &mut World) {
    // mobinfra: lightning bolts are entities too (`weather::lightning`), in the same id order
    if !w.weather.bolts.is_empty() {
        tick_phase_with_bolts(w);
        return;
    }
    if w.entities.list.is_empty() {
        crate::gravity::tick_entities(w);
        return;
    }
    if w.gravity.entities.is_empty() {
        tick_all(w);
        return;
    }
    let (ng, ne) = (w.gravity.entities.len(), w.entities.list.len());
    let (mut i, mut j) = (0, 0);
    while i < ng || j < ne {
        if j >= ne || (i < ng && w.gravity.entities[i].id < w.entities.list[j].id) {
            crate::gravity::tick_one(w, i);
            i += 1;
        } else {
            tick_one(w, j);
            j += 1;
        }
    }
    w.gravity.entities.retain(|e| !e.removed);
    w.entities.list.retain(|e| !e.removed);
}

/// [`tick_phase`] with lightning bolts (mobinfra): the falling blocks, the entities and the bolts
/// in id order (the order they joined the level), each list as the phase began (an entity added
/// during the phase ticks from the next one, like the game's entity tick list).
fn tick_phase_with_bolts(w: &mut World) {
    let (ng, ne, nb) = (w.gravity.entities.len(), w.entities.list.len(), w.weather.bolts.len());
    let (mut i, mut j, mut k) = (0, 0, 0);
    loop {
        let g = (i < ng).then(|| w.gravity.entities[i].id);
        let e = (j < ne).then(|| w.entities.list[j].id);
        let b = (k < nb).then(|| w.weather.bolts[k].id);
        let Some(next) = [g, e, b].into_iter().flatten().min() else { break };
        if g == Some(next) {
            crate::gravity::tick_one(w, i);
            i += 1;
        } else if e == Some(next) {
            tick_one(w, j);
            j += 1;
        } else {
            if !w.weather.bolts[k].removed {
                crate::weather::lightning::tick_bolt(w, k);
            }
            k += 1;
        }
    }
    w.gravity.entities.retain(|e| !e.removed);
    w.entities.list.retain(|e| !e.removed);
    w.weather.bolts.retain(|b| !b.removed);
}

/// `Entity.commonTick`: `invulnerableTime`, `setOldPosAndRot`, `tickCount`.
pub fn common_tick(w: &mut World, i: usize) {
    let e = &mut w.entities.list[i];
    if e.invulnerable_time > 0 {
        e.invulnerable_time -= 1;
    }
    e.xo = e.pos.x;
    e.x_old = e.pos.x;
    e.yo = e.pos.y;
    e.y_old = e.pos.y;
    e.zo = e.pos.z;
    e.z_old = e.pos.z;
    e.y_rot_o = e.y_rot;
    e.x_rot_o = e.x_rot;
    e.tick_count += 1;
}

/// `Entity.computeSpeed`: `lastKnownSpeed` from the position since the last call.
pub fn compute_speed(w: &mut World, i: usize) {
    let e = &mut w.entities.list[i];
    let last = *e.last_known_position.get_or_insert(e.pos);
    e.last_known_speed = e.pos.subtract_v(last);
    e.last_known_position = Some(e.pos);
}

/// `Entity.baseTick` (no passengers, portals, fire or lava in the modeled worlds):
/// `computeSpeed`, `wasEyeInWater`, `updateFluidInteraction`, `checkBelowWorld`, `firstTick`.
pub fn base_tick(w: &mut World, i: usize) {
    compute_speed(w, i);
    // `if (this.boardingCooldown > 0) this.boardingCooldown--`
    if w.entities.list[i].boarding_cooldown > 0 {
        w.entities.list[i].boarding_cooldown -= 1;
    }
    {
        let e = &mut w.entities.list[i];
        // the previous tick's trackers
        e.was_eye_in_water = e.fluid.eye_in_water();
    }
    fluid_push::update_fluid_interaction(w, i);
    let e = &mut w.entities.list[i];
    // `checkBelowWorld`: below the world's bottom by 64
    if e.pos.y < (crate::gravity::MIN_Y - 64) as f64 {
        e.removed = true;
    }
    e.first_tick = false;
    leash::tick(w, i);
}

/// `Entity.move(type, delta)` of entity `i` (index in the list): `collide::move_`.
pub fn move_(w: &mut World, i: usize, mover: MoverType, delta: Vec3) {
    collide::move_(w, i, mover, delta);
}

/// `Entity.applyEffectsFromBlocks()` of entity `i`: `collide::apply_effects_from_blocks`.
pub fn apply_effects_from_blocks(w: &mut World, i: usize) {
    collide::apply_effects_from_blocks(w, i);
}

/// `CollisionGetter.noCollision(entity, box)`.
pub fn no_collision(w: &World, i: usize, bb: Aabb) -> bool {
    collide::no_collision(w, i, bb)
}

/// `Entity.updateFluidInteraction` (the old name `updateFluidHeightAndDoFluidPushing`): see
/// [`fluid_push::update_fluid_interaction`]; the fluid and scale are the game's (water, 0.014).
pub fn update_fluid_height_and_do_fluid_pushing(w: &mut World, i: usize, _fluid: crate::fluid::Fluid, _flow_scale: f64) -> bool {
    fluid_push::update_fluid_interaction(w, i)
}

impl World {
    /// The block collision boxes that intersect `bb` (`BlockCollisions` with the empty collision
    /// context: every cell from floor(min - 1e-7) - 1 to floor(max + 1e-7) + 1, each block's
    /// `behavior::collision_boxes` moved to its cell). Moving pistons and context-dependent shapes
    /// (scaffolding) come in M2.
    pub fn collision_shapes(&self, bb: Aabb) -> Vec<Aabb> {
        let lo = |v: f64| mth::floor(v - 1.0e-7) - 1;
        let hi = |v: f64| mth::floor(v + 1.0e-7) + 1;
        let mut out = Vec::new();
        for y in lo(bb.min_y)..=hi(bb.max_y) {
            for z in lo(bb.min_z)..=hi(bb.max_z) {
                for x in lo(bb.min_x)..=hi(bb.max_x) {
                    let p = BlockPos::new(x, y, z);
                    let b = self.get(p);
                    if matches!(b, Block::Air) {
                        continue;
                    }
                    for c in crate::behavior::collision_boxes(&b) {
                        let f = |v: i32| v as f64 / 16.0;
                        let a = Aabb::new(x as f64 + f(c[0]), y as f64 + f(c[2]), z as f64 + f(c[4]), x as f64 + f(c[1]), y as f64 + f(c[3]), z as f64 + f(c[5]));
                        if a.intersects(bb) {
                            out.push(a);
                        }
                    }
                }
            }
        }
        out
    }

    /// `getEntities(except none, bb, filter)`: the live entities whose box intersects `bb`, in
    /// list order (indices into `entities.list`).
    pub fn get_entities(&self, bb: Aabb, filter: impl Fn(&Entity) -> bool) -> Vec<usize> {
        // `Level.getEntities(type, bb, predicate)`: the sections in key order, each section's
        // entities in the order they entered it
        let mut v: Vec<usize> = self.entities.list.iter().enumerate().filter(|(_, e)| !e.removed && e.bb.intersects(bb) && filter(e)).map(|(i, _)| i).collect();
        v.sort_by_key(|&i| {
            let e = &self.entities.list[i];
            (section_order(e.section), e.section_seq)
        });
        v
    }
}
