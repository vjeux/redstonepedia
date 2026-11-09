//! Mobs (ironfarm, 2026-11-08; brief `designs/briefs/farm-iron.md`): villagers, iron golems and
//! zombies in redsim's entity list, ticked in its order like every other entity, with the rules
//! read in the 26.3 source:
//!
//! * `types.rs`: the type registry (sizes, attributes, immunities, loot);
//! * `living.rs`: `LivingEntity` (base tick, AI step, travel, damage, death, drops, pushing);
//! * `villager.rs`: the villager's `Brain` (memories, sensors, activities, the schedule), its
//!   panic and sleep behaviors, `spawnGolemIfNeeded`, sleeping and waking;
//! * `spawn.rs`: `SpawnUtil.trySpawnMob` (`LEGACY_IRON_GOLEM`) and `IronGolem.checkSpawnObstruction`;
//! * `los.rs`: line of sight (`ClipContext.Block.COLLIDER`);
//! * `blocks.rs`: the block properties these rules read (beds, lava, spawn checks);
//! * `walk.rs`: the walking input the judge gives a mob whose AI is not modeled (its robustness
//!   runs), never a guess of the game's goals.
//!
//! Exactness policy: the same formulas in the same order as the game for everything that sets a
//! rate or a pass/fail. Where the game draws from a random redsim cannot mirror (an entity's own
//! unseeded random; the level random that unmodeled behaviors also draw from), the draw comes from
//! [`MobWorld::rng`], seeded by the caller, with the same distribution.

pub mod absence;
// ---- mob AI (the mob-AI lane, `ai/`): path finding, navigation, controls, goals ----
pub mod ai;
pub mod blocks;
pub mod custom;
pub mod enchant;
pub mod exit;
pub mod farm;
pub mod hostile;
pub mod living;
pub mod los;
pub mod natural;
pub mod prices;
pub mod refuse;
pub mod site;
pub mod spawn;
pub mod sun;
pub mod types;
pub mod villager;
pub mod view;
pub mod walk;

#[cfg(test)]
mod tests;

/// The items the mob farms add to the item registry (`redstone::container`, collected by
/// build.rs): an iron golem's poppy.
pub const MODULE_ITEMS: &[(&str, u8)] = &[("poppy", 64)];

use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::rng::LegacyRandom;
use crate::world::World;
use types::MobType;

/// The damage sources the modeled rules deal (`DamageTypes`), with their tags.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Damage {
    Lava,
    OnFire,
    InWall,
    Cramming,
    Fall,
    Drown,
    HotFloor,
    /// mobinfra: a lightning bolt (`lightning_bolt`: not fire, armor counts).
    Lightning,
    /// mobinfra: standing in a fire a bolt set (`in_fire`: fire, armor counts).
    InFire,
    /// mob AI: a mob's melee hit (`mob_attack`: armor counts, knockback from the attacker, who is
    /// `MobWorld.ai.attacker` during the call).
    MobAttack,
}

impl Damage {
    /// Tag `is_fire`.
    pub fn is_fire(self) -> bool {
        matches!(self, Damage::Lava | Damage::OnFire | Damage::HotFloor | Damage::InFire)
    }

    /// Tag `bypasses_armor`.
    pub fn bypasses_armor(self) -> bool {
        matches!(self, Damage::OnFire | Damage::InWall | Damage::Cramming | Damage::Fall | Damage::Drown)
    }

    pub fn name(self) -> &'static str {
        match self {
            Damage::Lava => "lava",
            Damage::OnFire => "on_fire",
            Damage::InWall => "in_wall",
            Damage::Cramming => "cramming",
            Damage::Fall => "fall",
            Damage::Drown => "drown",
            Damage::HotFloor => "hot_floor",
            Damage::Lightning => "lightning_bolt",
            Damage::InFire => "in_fire",
            Damage::MobAttack => "mob_attack",
        }
    }

    /// The damage type's `message_id` (`DamageSource.getMsgId()`, what the bridge's frames print).
    pub fn msg_id(self) -> &'static str {
        match self {
            Damage::Lava => "lava",
            Damage::OnFire => "onFire",
            Damage::InWall => "inWall",
            Damage::Cramming => "cramming",
            Damage::Fall => "fall",
            Damage::Drown => "drown",
            Damage::HotFloor => "hotFloor",
            Damage::Lightning => "lightningBolt",
            Damage::InFire => "inFire",
            Damage::MobAttack => "mob",
        }
    }
}

/// What the mob layer records, in order (the judge's evidence).
#[derive(Clone, PartialEq, Debug)]
pub enum MobEvent {
    /// A villager ran `spawnGolemIfNeeded` with enough willing villagers: `golem` is the new
    /// golem's id, or `None` when none of the 10 tries found a spot.
    GolemTry { t: i64, villager: u32, willing: u32, golem: Option<u32>, at: Option<[i32; 3]> },
    /// A villager's golem sensor saw a golem (`golemDetected`).
    GolemSeen { t: i64, villager: u32 },
    Hurt { t: i64, id: u32, ty: MobType, src: Damage, amount: f32 },
    Death { t: i64, id: u32, ty: MobType, src: Damage, pos: [f64; 3] },
    Drop { t: i64, from: u32, item: u32, what: &'static str, count: u32 },
    Removed { t: i64, id: u32 },
    Slept { t: i64, id: u32 },
    Woke { t: i64, id: u32 },
    /// A villager gave up a POI memory (`Villager.releasePoi`: its HOME, job site or meeting point
    /// unreachable too long: SetWalkTargetFromBlockMemory).
    Released { t: i64, id: u32 },
    /// A villager's active activity changed (`Brain.setActiveActivity`).
    Activity { t: i64, id: u32, activity: villager::Activity },
}

/// The block effects `checkInsideBlocks` collected for a mob this tick (`InsideBlockEffectApplier`).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Inside {
    pub lava: bool,
    pub water: bool,
    /// mobinfra: a fire a bolt set (`BaseFireBlock.entityInside`: FIRE_IGNITE, then 1 `inFire`).
    pub fire: bool,
}

/// A mob's state (the `LivingEntity` / `Mob` fields the rules read and write).
#[derive(Clone, PartialEq, Debug)]
pub struct MobState {
    pub ty: MobType,
    pub health: f32,
    /// `dead` (set by `die`); the body is removed 20 gt later (`death_time`).
    pub dead: bool,
    pub death_time: i32,
    pub hurt_time: i32,
    /// `damageCooldownTime` and `lastHurt`.
    pub damage_cooldown: i32,
    pub last_hurt: f32,
    /// `remainingFireTicks`: `-getFireImmuneTicks()`, 0 for a mob in 26.3, at creation, at load
    /// (`Entity.load` reads `Fire` with default 0) and after a fire.
    pub fire_ticks: i32,
    /// `airSupply` (300).
    pub air: i32,
    /// `Entity.moveDist` and `nextStep` (the step and swim sounds' clock; the mob-AI lane).
    pub move_dist: f32,
    pub next_step: f32,
    /// `getSleepingPos()`: sleeping (pose SLEEPING) when set.
    pub sleeping_pos: Option<BlockPos>,
    /// `FOLLOW_RANGE` with `Mob.finalizeSpawn`'s random bonus.
    pub follow_range: f64,
    /// `xxa`, `zza`, `jumping`, `speed` (the walking input).
    pub xxa: f32,
    pub zza: f32,
    pub jumping: bool,
    pub speed: f32,
    pub no_jump_delay: i32,
    pub no_action_time: i32,
    /// `persistenceRequired` (a name tag, or picked-up loot).
    pub persistence_required: bool,
    /// `LivingEntity.onClimbable()` at its last tick (scaffolding refused; a spider's climbing).
    pub on_climbable: bool,
    /// The spider's climbing flag (`Spider.setClimbing(horizontalCollision)` after its tick).
    pub climbing: bool,
    /// The bat's resting flag (set at construction) and its flight target.
    pub bat_resting: bool,
    pub bat_target: Option<BlockPos>,
    /// The lava tracker's height (`getFluidHeight(LAVA)`; `isInLava` when positive).
    pub lava_height: f64,
    /// This tick's collected block effects.
    pub inside: Inside,
    /// The last damage: (game time, source).
    pub last_damage: Option<(i64, Damage)>,
    /// A villager's brain.
    pub brain: Option<Box<villager::Brain>>,
    /// The walking input the judge gives a mob whose AI is not modeled.
    pub walk: walk::Walk,
    /// ---- mob AI (the mob-AI lane): the goal mob's AI state, when the world's AI is on ----
    pub ai: Option<Box<ai::MobAi>>,
}

impl MobState {
    /// A fresh mob of type `ty` (as `EntityType.create` + `finalizeSpawn` leave it; `bonus` is
    /// the follow-range multiplier's `triangle(0, 0.11485)` draw).
    pub fn new(ty: MobType, bonus: f64) -> MobState {
        let d = ty.def();
        MobState {
            ty,
            health: d.max_health,
            dead: false,
            death_time: 0,
            hurt_time: 0,
            damage_cooldown: 0,
            last_hurt: 0.0,
            // `remainingFireTicks = -getFireImmuneTicks()`: 0 for a mob in 26.3
            fire_ticks: 0,
            air: 300,
            move_dist: 0.0,
            next_step: 1.0,
            sleeping_pos: None,
            follow_range: d.follow_range * (1.0 + bonus),
            xxa: 0.0,
            zza: 0.0,
            jumping: false,
            speed: d.speed as f32,
            no_jump_delay: 0,
            no_action_time: 0,
            persistence_required: false,
            on_climbable: false,
            climbing: false,
            bat_resting: d.id == "bat",
            bat_target: None,
            lava_height: 0.0,
            inside: Inside::default(),
            last_damage: None,
            brain: None,
            walk: walk::Walk::default(),
            ai: None,
        }
    }

    /// `getDimensions(pose)`: (width, height).
    pub fn dims(&self) -> (f32, f32) {
        if self.sleeping_pos.is_some() {
            (types::SLEEPING.0, types::SLEEPING.1)
        } else {
            let d = self.ty.def();
            (d.width, d.height)
        }
    }

    /// `getEyeHeight()` in its pose.
    pub fn eye(&self) -> f32 {
        if self.sleeping_pos.is_some() { types::SLEEPING.2 } else { self.ty.def().eye }
    }
}

/// The mob layer's level state.
#[derive(Clone, Debug)]
pub struct MobWorld {
    /// The seeded stand-in for the randoms the game draws that redsim cannot mirror (see the module
    /// doc). `None`: the mob layer is off (a mob in the world is a model error).
    pub rng: Option<LegacyRandom>,
    /// Day time = `day_offset + game time` (`ServerLevel.tickTime` advances both by one a tick
    /// while `advance_time` holds).
    pub day_offset: i64,
    /// Game rule `maxEntityCramming`.
    pub cramming: i32,
    /// The record (cleared by whoever reads it).
    pub events: Vec<MobEvent>,
    /// A golem spawn attempt's columns and outcome for the judge's spawn-spot statistics:
    /// (villager block, the cell each try ended at (`None`: no spot in the column), the spot it
    /// spawned at).
    pub spawn_tries: Vec<(BlockPos, Vec<Option<BlockPos>>, Option<BlockPos>)>,
    /// The walking input every new iron golem gets (the judge's robustness runs: [`walk::Walk`]).
    pub golem_walk: walk::Walk,
    /// Villagers spawn golems (`false`: the judge's golem exit flood, which adds its own golem).
    pub golem_spawning: bool,
    /// Natural spawning and despawning (the terrain lane: [`natural::Natural`]); `None`: off.
    pub natural: Option<natural::Natural>,
    /// (tickexact) The overworld's custom spawners ([`custom::Custom`]); `None`: the level runs none.
    pub custom: Option<custom::Custom>,
    /// ---- mob AI (the mob-AI lane): on/off, difficulty, players, game rules ----
    pub ai: ai::AiWorld,
}

impl Default for MobWorld {
    fn default() -> MobWorld {
        MobWorld { rng: None, day_offset: 0, cramming: 24, events: Vec::new(), spawn_tries: Vec::new(), golem_walk: walk::Walk::Still, golem_spawning: true, natural: None, custom: None, ai: ai::AiWorld::default() }
    }
}

impl MobWorld {
    /// The seeded random (a world with mobs must set one: [`MobWorld::seed`]).
    pub fn rng_mut(&mut self) -> &mut LegacyRandom {
        self.rng.get_or_insert_with(|| LegacyRandom::new(0x6d6f_6266_6172_6d))
    }

    /// Turn the mob layer on with this seed and day time at game time 0.
    pub fn seed(&mut self, seed: i64, day_time_at_zero: i64) {
        self.rng = Some(LegacyRandom::new(seed));
        self.day_offset = day_time_at_zero;
    }
}

impl World {
    /// The level's day time (`getDayTime`).
    pub fn day_time(&self) -> i64 {
        self.mob.day_offset + self.time
    }
}

/// The mob state of entity `i` (panics if it is not a mob).
pub fn mob(w: &World, i: usize) -> &MobState {
    match &w.entities.list[i].kind {
        EntityKind::Mob(m) => m,
        k => panic!("entity {i} is not a mob: {}", kind_name(k)),
    }
}

/// The mob state of entity `i`, mutable.
pub fn mob_mut(w: &mut World, i: usize) -> &mut MobState {
    match &mut w.entities.list[i].kind {
        EntityKind::Mob(m) => m,
        _ => panic!("entity {i} is not a mob"),
    }
}

/// A short name of an entity kind (for messages).
pub fn kind_name(k: &EntityKind) -> &'static str {
    match k {
        EntityKind::Boat(_) => "boat",
        EntityKind::Minecart(_) => "minecart",
        EntityKind::Item(_) => "item",
        EntityKind::ArmorStand(_) => "armor stand",
        EntityKind::PrimedTnt(_) => "primed TNT",
        EntityKind::Mob(m) => m.ty.def().id,
    }
}

/// The entity core's spawn hook: a mob (`spawn`), or a boat or minecart carrying mobs (`summon
/// <vehicle> x y z {..,Passengers:[{id:"minecraft:zombie"}]}`: the vehicle through the core's own
/// spawn, then each passenger mounted). `None`: not for the mob layer.
pub fn spawn_hook(w: &mut World, type_id: &str, pos: [f64; 3], nbt: Option<&str>) -> Option<Result<u32, String>> {
    if let Some(r) = spawn(w, type_id, pos, nbt) {
        return Some(r);
    }
    let n = nbt?;
    let start = n.find("Passengers:[")?;
    let list_start = start + "Passengers:[".len();
    let mut depth = 1i32;
    let mut end = None;
    for (k, ch) in n[list_start..].char_indices() {
        match ch {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(list_start + k);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else { return Some(Err(format!("{type_id}: unclosed Passengers list"))) };
    if type_id.contains("raft") {
        return Some(Err(format!("{type_id}: a raft's seat (0.5) is not modeled; use a wooden boat or a minecart")));
    }
    let list = &n[list_start..end];
    // the vehicle's own data without the Passengers entry
    let mut rest = format!("{}{}", &n[..start], &n[end + 1..]);
    rest = rest.replace("{,", "{").replace(",}", "}").replace(",,", ",");
    let rest = if rest.trim() == "{}" { None } else { Some(rest) };
    let vehicle = match crate::entity::spawn(w, type_id, pos, rest.as_deref()) {
        Ok(v) => v,
        Err(e) => return Some(Err(e)),
    };
    for item in split_compounds(list) {
        let Some(i) = item.find("id:") else { return Some(Err(format!("{type_id}: a passenger without an id: {item}"))) };
        let pid: String = item[i + 3..].trim_start().trim_start_matches('"').chars().take_while(|c| *c != '"' && *c != ',' && *c != '}').collect();
        // the passenger's own data without its id, through the same contract as a summoned mob
        // (`spawn`: PersistenceRequired and Rotation only; NoAI and anything else refused)
        let id_end = i + 3 + item[i + 3..].find(pid.as_str()).map(|k| k + pid.len()).unwrap_or(0);
        let id_end = if item[id_end..].starts_with('"') { id_end + 1 } else { id_end };
        let mut rest = format!("{}{}", &item[..i], &item[id_end..]);
        rest = rest.replace("{,", "{").replace(",}", "}").replace(",,", ",");
        let rest = if rest.trim() == "{}" { None } else { Some(rest) };
        if MobType::from_id(&pid).is_none() {
            return Some(Err(format!("{type_id}: passenger {pid} is not modeled")));
        }
        let rider = match spawn(w, &pid, pos, rest.as_deref()) {
            Some(Ok(r)) => r,
            Some(Err(e)) => return Some(Err(format!("{type_id}'s passenger: {e}"))),
            None => return Some(Err(format!("{type_id}: passenger {pid} is not modeled"))),
        };
        if let Err(e) = mount(w, rider, vehicle) {
            return Some(Err(e));
        }
    }
    Some(Ok(vehicle))
}

/// The top-level `{...}` items of an SNBT list's inside.
fn split_compounds(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut cur) = (0i32, String::new());
    for ch in list.chars() {
        match ch {
            '{' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            '}' | ']' => {
                depth -= 1;
                cur.push(ch);
                if depth == 0 {
                    out.push(std::mem::take(&mut cur).trim().to_string());
                }
            }
            ',' if depth == 0 => {}
            _ => {
                if depth > 0 {
                    cur.push(ch);
                }
            }
        }
    }
    out
}

/// `summon <mob> x y z [nbt]` (the entity core's spawn hook): `Some` for a modeled mob type, else
/// `None` (the caller goes on). NBT is refused except `PersistenceRequired` and `Rotation` (the
/// yaw; a design turned by the judge carries one). The mob's own random and its follow-range bonus
/// come from [`MobWorld::rng`]: the judge models a mob the player brought in (its
/// `finalizeSpawn` ran where it spawned), not a `/summon` with data.
pub fn spawn(w: &mut World, type_id: &str, pos: [f64; 3], nbt: Option<&str>) -> Option<Result<u32, String>> {
    let mut ty = MobType::from_id(type_id)?;
    let mut persistent = false;
    let mut can_break_doors = false;
    let mut bat_flags = 0;
    let mut cube_size: Option<i32> = None;
    let mut yaw = None;
    if let Some(n) = nbt {
        let mut t = n.trim().to_string();
        if let Some(s) = t.find("Rotation:[") {
            let Some(e) = t[s..].find(']').map(|e| s + e) else { return Some(Err(format!("entity {type_id}: unclosed Rotation in {n}"))) };
            let first = t[s + "Rotation:[".len()..e].split(',').next().unwrap_or("").trim().trim_end_matches(['f', 'F', 'd', 'D']).to_string();
            match first.parse::<f32>() {
                Ok(v) => yaw = Some(v),
                Err(_) => return Some(Err(format!("entity {type_id}: bad Rotation in {n}"))),
            }
            t = format!("{}{}", &t[..s], &t[e + 1..]);
        }
        let inner = t.trim().strip_prefix('{').and_then(|s| s.strip_suffix('}')).unwrap_or(&t).trim().to_string();
        for kv in inner.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match kv.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("PersistenceRequired", v)) => persistent = matches!(v, "1b" | "true" | "1"),
                // ---- mob AI (the mob-AI lane): a zombie that breaks doors ----
                Some(("CanBreakDoors", v)) if w.mob.ai.on && matches!(ty.def().id, "zombie" | "baby_zombie") => can_break_doors = matches!(v, "1b" | "true" | "1"),
                // a bat's flags (resting: bit 1)
                Some(("BatFlags", v)) if ty.def().id == "bat" => bat_flags = v.trim_end_matches(['b', 'B']).parse::<i32>().unwrap_or(0),
                // a sulfur cube's size (`Size` = size - 1)
                Some(("Size", v)) if ty.def().game_id == "minecraft:sulfur_cube" => cube_size = v.trim_end_matches(['b', 'B']).parse::<i32>().ok().map(|s| s + 1),
                _ => return Some(Err(format!("entity {type_id} with {kv} in its data is not modeled by redsim"))),
            }
        }
    }
    // a loaded sulfur cube of size 2 (an adult: no Age); size 1 loaded is an adult of the baby's
    // size, not modeled
    if ty.def().game_id == "minecraft:sulfur_cube" {
        match cube_size.unwrap_or(1) {
            2 => ty = MobType::from_id("sulfur_cube_2").expect("a row"),
            s => return Some(Err(format!("a summoned sulfur cube of size {s} (an adult of that size) is not modeled"))),
        }
    }
    let id = add(w, ty, Vec3::new(pos[0], pos[1], pos[2]), persistent);
    // a loaded bat rests only if its BatFlags say so (`readAdditionalSaveData`, default 0: a
    // summoned bat flies)
    if ty.def().id == "bat"
        && let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let EntityKind::Mob(m) = &mut e.kind
    {
        m.bat_resting = bat_flags & 1 != 0;
    }
    // a summoned mob faces its `Rotation`, else 0 (`Entity.load` reads it with the default
    // Vec2.ZERO): the constructor's random yaw is drawn and thrown away
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.y_rot = yaw.unwrap_or(0.0);
        // ---- mob AI (the mob-AI lane): Entity.load: the head and the body take the loaded yaw ----
        if let EntityKind::Mob(m) = &mut e.kind
            && let Some(a) = m.ai.as_deref_mut()
        {
            a.y_head_rot = e.y_rot;
            a.y_body_rot = e.y_rot;
        }
    }
    if can_break_doors
        && let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let EntityKind::Mob(m) = &mut e.kind
        && let Some(a) = m.ai.as_deref_mut()
    {
        ai::goal::zombie::set_can_break_doors(a, true);
    }
    Some(Ok(id))
}

/// Add a mob of type `ty` at `pos` (`addFreshEntity`): its yaw, follow-range bonus and own random
/// drawn from [`MobWorld::rng`]; a villager gets its brain. Returns its id.
pub fn add(w: &mut World, ty: MobType, pos: Vec3, persistent: bool) -> u32 {
    let (y_rot, bonus, seed) = {
        let r = w.mob.rng_mut();
        let y_rot = crate::entity::mth::wrap_degrees(r.next_float() * 360.0);
        let bonus = r.triangle(0.0, 0.11485000000000001);
        (y_rot, bonus, r.next_long())
    };
    let mut st = MobState::new(ty, bonus);
    st.persistence_required = persistent;
    // ---- mob AI (the mob-AI lane): `registerGoals`; `LivingEntity.speed` starts at 0 ----
    if w.mob.ai.on {
        st.ai = Some(Box::new(ai::MobAi::new(ty, y_rot)));
        st.speed = 0.0;
    }
    let mut own = LegacyRandom::new(seed);
    // ---- the bridge's seeding in an exact world (`rng::entity_seed`, the mob-AI lane's): the
    // constructor's draws redone: the UUID, the LivingEntity yaw (`nextFloat() * (float)(PI * 2)`,
    // the head with it, the body at 0: a summon's data then sets all three, a natural spawn's snap
    // only the yaw) ----
    if let Some(s) = w.exact_seed {
        own = crate::rng::entity_random(s, w.entities.next_id);
        let ctor_yaw = own.next_float() * (std::f64::consts::PI * 2.0) as f32;
        if let Some(a) = st.ai.as_deref_mut() {
            a.y_head_rot = ctor_yaw;
            a.y_body_rot = 0.0;
            // a cube mob's move control takes its direction from the constructor's yaw
            if let Some(c) = a.cube.as_mut() {
                c.yrot = 180.0f32 * ctor_yaw / std::f32::consts::PI;
            }
        }
    }
    // ---- mob AI (the mob-AI lane): a mob whose goals are not modeled is refused, never run
    // without them (villagers have their brain, bats their own flight) ----
    if let Some(a) = st.ai.as_deref()
        && ty != MobType::Villager
        && ty.def().id != "bat"
        && a.goals.goals.is_empty()
        && a.targets.goals.is_empty()
    {
        let why = format!("the AI of {} (its goals) is not modeled", ty.game_id());
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
    if ty == MobType::Villager {
        st.brain = Some(Box::new(villager::Brain::new(&mut own, w.time, w.day_time())));
        // ---- mob AI (the mob-AI lane): the exact brain, the live brain's sensors drawn again (its
        // shuffling lists keyed by the bridge's entity seed in an exact world, else by the stand-in
        // own random's seed: the judge's runs) ----
        if w.mob.ai.on
            && let Some(b) = st.brain.as_deref_mut()
        {
            let es = match w.exact_seed {
                Some(s) => crate::rng::entity_seed(s, w.entities.next_id),
                None => seed,
            };
            ai::brain::attach(b, &mut own, es, w.time, w.day_time());
        }
    }
    let id = w.entities.add(EntityKind::Mob(Box::new(st)), pos, y_rot);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.random = Some(own);
    }
    id
}

/// One mob's tick (the entity core's dispatch hook).
pub fn tick(w: &mut World, i: usize) {
    living::tick(w, i);
}

/// (tickexact) In an exact world, a mob that just entered the level (`addFreshEntity`) gets the
/// bridge's entering random: its number's seed ([`World::entering_random`]) with its constructor's
/// draws replayed (the UUID's two longs, `LivingEntity`'s yaw). A mob with brain sensors replays
/// their `nextInt(scanRate)` too (none of the natural types redsim spawns has them). Nothing
/// without an exact world (the judge's stand-in stays).
pub fn seed_entered(w: &mut World, id: u32) {
    if w.exact_seed.is_none() {
        return;
    }
    let mut own = w.entering_random(id);
    crate::rng::replay_uuid(&mut own);
    own.next_float();
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.random = Some(own);
    }
}

/// `Entity.applyMovementEmissionAndPlaySound` for a mob that is no passenger (the mob-AI lane):
/// `moveDist` grows by the horizontal move x 0.6 (climbing is refused elsewhere); past `nextStep`
/// over a block that is not air, a mob on the ground steps (its step sounds draw nothing) and one
/// in water swims: `playSwimSound`'s pitch draws two floats from its own random.
pub fn movement_emission(w: &mut World, i: usize, movement: Vec3) {
    let moved = (movement.horizontal_distance() * 0.6f32 as f64) as f32;
    let m = mob_mut(w, i);
    m.move_dist += moved;
    if !(m.move_dist > m.next_step) {
        return;
    }
    let e = &w.entities.list[i];
    let supporting = crate::entity::collide::get_on_pos(w, e, 1.0e-5);
    if w.get(supporting).is_air() {
        return;
    }
    let (on_ground, in_water) = (e.on_ground, e.was_touching_water);
    if on_ground {
        let m = mob_mut(w, i);
        m.next_step = (m.move_dist as i32 + 1) as f32;
    } else if in_water {
        let m = mob_mut(w, i);
        m.next_step = (m.move_dist as i32 + 1) as f32;
        // a bat's movement emits events only (`Bat.getMovementEmission`: no swim sound)
        if mob(w, i).ty.def().id != "bat"
            && let Some(r) = w.entities.list[i].random.as_mut()
        {
            r.next_float();
            r.next_float();
        }
    }
}

/// `maxUpStep()` (`Attributes.STEP_HEIGHT`).
pub fn max_up_step(m: &MobState) -> f32 {
    m.ty.def().step
}

/// `getGravity()` (living: `Attributes.GRAVITY` 0.08).
pub fn gravity() -> f64 {
    0.08
}

/// `getAirDrag()`: `computeModifiedFriction(0.98F, 1)` (not an omnidirectional air mover).
pub fn air_drag() -> f64 {
    (1.0f32 - (1.0f32 - 0.98f32) * 1.0f32).clamp(0.0, 1.0) as f64
}

/// The y of a mob's seat on a vehicle of kind `v` above the vehicle's position: the vehicle's
/// passenger attachment (a minecart's 0.1875; a wooden boat's `rideHeight` = height / 3 = 0.1875;
/// rafts, whose seat is height x 0.8888889 = 0.5, are refused when mounting: `BoatState` does not
/// tell them apart) minus the rider's vehicle attachment (`ridingOffset`: a zombie sits 0.7 lower).
pub fn rider_offset(v: &EntityKind, rider: &MobState) -> Option<f64> {
    let seat = match v {
        EntityKind::Minecart(_) => 0.1875f32,
        EntityKind::Boat(_) => 0.5625f32 / 3.0f32,
        _ => return None,
    };
    Some(seat as f64 + rider.ty.def().riding_offset as f64)
}

/// `Entity.startRiding(vehicle)` as `summon <vehicle> .. {Passengers:[{id:<mob>}]}` sets it up:
/// the mob joins the vehicle's passengers and takes its seat at the vehicle's next tick.
pub fn mount(w: &mut World, rider: u32, vehicle: u32) -> Result<(), String> {
    let (Some(pi), Some(vi)) = (crate::entity::index_of(w, rider), crate::entity::index_of(w, vehicle)) else { return Err(format!("mount: no entity {rider} or {vehicle}")) };
    if !matches!(w.entities.list[pi].kind, EntityKind::Mob(_)) || rider_offset(&w.entities.list[vi].kind, mob(w, pi)).is_none() {
        return Err("mount: a mob in a boat or minecart only".into());
    }
    if !w.entities.list[vi].passengers.is_empty() || w.entities.list[pi].vehicle.is_some() {
        return Err("mount: one passenger per vehicle".into());
    }
    w.entities.list[pi].vehicle = Some(vehicle);
    w.entities.list[vi].passengers.push(rider);
    let off = rider_offset(&w.entities.list[vi].kind, mob(w, pi)).unwrap_or(0.0);
    let vp = w.entities.list[vi].pos;
    w.entities.set_pos(pi, vp.add(0.0, off, 0.0));
    Ok(())
}

/// `rideTick` of a riding mob: its motion zeroed, its own tick, then its seat
/// (`positionRider`), its fall distance reset (`LivingEntity.rideTick`).
pub fn ride_tick(w: &mut World, j: usize) {
    w.entities.list[j].delta_movement = Vec3::ZERO;
    living::tick(w, j);
    if let Some(v) = w.entities.list[j].vehicle
        && let Some(vi) = crate::entity::index_of(w, v)
        && let Some(off) = rider_offset(&w.entities.list[vi].kind, mob(w, j))
    {
        let vp = w.entities.list[vi].pos;
        w.entities.set_pos(j, vp.add(0.0, off, 0.0));
        // `AbstractBoat.positionRider` (the mob-AI lane): the rider turns with the boat
        // (deltaRotation), then `clampRotation`: a mob's body and yaw take the boat's yaw and its
        // head comes within `getMaxHeadYRot` of the body (`clampHeadRotationToBody`)
        if let EntityKind::Boat(b) = &w.entities.list[vi].kind {
            let dr = b.delta_rotation;
            let boat_yaw = w.entities.list[vi].y_rot;
            let limit = ai::max_head_y_rot(w, j) as f32;
            w.entities.list[j].y_rot = boat_yaw;
            if let Some(a) = mob_mut(w, j).ai.as_deref_mut() {
                let head = a.y_head_rot + dr;
                a.y_body_rot = boat_yaw;
                let delta = crate::entity::mth::wrap_degrees(a.y_body_rot - head);
                let target = delta.clamp(-limit, limit);
                a.y_head_rot = head + delta - target;
            }
        }
    }
    w.entities.list[j].fall_distance = 0.0;
}
