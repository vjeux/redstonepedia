//! The exact compare (track tickexact, 2026-11-08; docs/mechanics.md "The bridge's seeds"): a
//! world's state after each tick in the shape of the bridge's traced step (`trace` op, bridge
//! `Exact.Watch`/`entityFrame`), and the first difference between the game's frames and
//! redsim's.
//!
//! The game runs in the bridge with every random seeded from the sandbox seed, so a run repeats;
//! redsim draws the same values from the same streams ([`crate::rng`]'s registry). A frame holds
//! what the compare checks every tick: the level random and `randValue`, the day time, the
//! weather, every entity (position, motion, rotations, health, fire, air, pose, vehicle, its own
//! random's state, a mob's brain memories with their expiry, its goals and path), the entities
//! that entered and left, and the block changes in the watched box. A field one side does not
//! report is `None`; a field the game reports and redsim does not model yet is a GAP, listed by
//! [`Compare::gaps`] (never a silent pass: exactness needs that list empty).

use crate::block::Block;
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// One entity after a tick (`Exact.entityFrame`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityFrame {
    pub n: u32,
    pub ty: String,
    pub pos: [f64; 3],
    pub motion: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub fire: Option<i32>,
    pub air: Option<i32>,
    pub pose: Option<String>,
    pub vehicle: Option<String>,
    /// The entity's own random's internal state (`LegacyRandomSource.seed`).
    pub random: Option<i64>,
    pub item: Option<String>,
    pub age: Option<i32>,
    pub head: Option<f32>,
    pub body: Option<f32>,
    pub health: Option<f32>,
    pub hurt_time: Option<i32>,
    pub death_time: Option<i32>,
    pub no_action: Option<i32>,
    /// Brain memories with a value, canonical (`Exact.memoryValue`; `" ttl N"` when it expires).
    pub memories: Option<BTreeMap<String, String>>,
    /// The memories redsim models for this entity (absent from `memories` = no value); `None` on
    /// the game's side (it reports them all).
    pub modeled_memories: Option<&'static [&'static str]>,
    /// Every memory counts as modeled (the bridge against itself).
    pub all_memories: bool,
    pub activities: Option<String>,
    pub behaviors: Option<String>,
    pub goals: Option<String>,
    pub path: Option<String>,
    pub target: Option<String>,
}

/// One tick (`Exact.Watch.frame`).
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub t: i64,
    pub day: Option<i64>,
    pub rng: Option<i64>,
    pub rv: Option<i32>,
    pub weather: Option<String>,
    pub entities: Vec<EntityFrame>,
    pub entered: Vec<u32>,
    pub left: Vec<u32>,
    /// Block changes in the watched box since the previous frame: (cell, redsim's state text).
    pub blocks: Vec<([i32; 3], String)>,
    /// The game's draw log of the tick: (stream, call site, count), in draw order.
    pub draws: Vec<(String, String, u32)>,
    /// Each draw-log row's raw `next(bits)` outputs (a probed tick).
    pub draw_values: Vec<Vec<i32>>,
    /// A probed tick: the spawning chunks in the order `tickChunks` ticked them (after its shuffle).
    pub spawning_shuffled: Vec<[i32; 2]>,
    /// Container changes in the watched box since the previous frame: (cell, its items as
    /// [`container_text`] prints them; empty when the container went).
    pub containers: Vec<([i32; 3], String)>,
    /// The game's AFK player's position (the bridge's frames; redsim's is `World::afk`).
    pub player: Option<[f64; 3]>,
    /// (tickexact 2026-11-10, farmopt's station) The Nether's frame of the same tick (the
    /// bridge's Nether sandbox; redsim's `World::nether`), when one is traced.
    pub nether: Option<Box<Frame>>,
}

/// A container's items, slot by slot (`slot:item:count`, empty slots left out) and, for a
/// crafter, its disabled slots: the compare's text on both sides.
pub fn container_text(c: &crate::redstone::container::ContainerBe) -> String {
    let mut s: Vec<String> = c.slots.iter().enumerate().filter_map(|(i, st)| st.map(|st| format!("{i}:{}:{}", crate::redstone::container::item_name(st.item), st.count))).collect();
    if c.crafter && c.disabled != 0 {
        s.push(format!("disabled:{}", c.disabled));
    }
    s.join(" ")
}

/// redsim's side of the watch: the box's blocks and the entities present at the last frame.
pub struct Watch {
    lo: BlockPos,
    hi: BlockPos,
    blocks: Vec<Block>,
    present: BTreeSet<u32>,
    containers: BTreeMap<[i32; 3], String>,
}

/// The memory names redsim's villager brain models (`villager::Brain`), in the game's names
/// (`hurt_by` holds a damage source in the game; redsim keeps its time only: a gap).
pub const VILLAGER_MEMORIES: &[&str] = &["golem_detected_recently", "home", "last_slept", "last_woken", "nearest_hostile", "mobs", "visible_mobs"];

impl Watch {
    /// Watch `lo..=hi` (inclusive; empty when `lo > hi`) from the world as it is now.
    pub fn new(w: &World, lo: BlockPos, hi: BlockPos) -> Watch {
        let mut blocks = Vec::new();
        if lo.x <= hi.x && lo.y <= hi.y && lo.z <= hi.z {
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    for x in lo.x..=hi.x {
                        blocks.push(w.get(BlockPos::new(x, y, z)));
                    }
                }
            }
        }
        let present = w.entities.list.iter().filter(|e| !e.removed).map(|e| e.id).collect();
        let mut watch = Watch { lo, hi, blocks, present, containers: BTreeMap::new() };
        watch.containers = watch.container_texts(w);
        watch
    }

    /// The text of every container in the box.
    fn container_texts(&self, w: &World) -> BTreeMap<[i32; 3], String> {
        let (lo, hi) = (self.lo, self.hi);
        w.containers
            .map
            .iter()
            .filter(|(p, _)| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y && p.z >= lo.z && p.z <= hi.z)
            .map(|(p, c)| ([p.x, p.y, p.z], container_text(c)))
            .collect()
    }

    /// The frame of the tick that just ran.
    pub fn frame(&mut self, w: &World) -> Frame {
        let mut f = Frame { t: w.time, day: Some(w.day_time()), ..Default::default() };
        f.rng = w.level_random.as_ref().map(|r| r.state());
        f.rv = w.random_ticks.as_ref().map(|r| r.value);
        f.weather = weather_text(w);
        f.player = w.afk.map(|a| [a.x, a.y, a.z]);
        let mut now = BTreeSet::new();
        for e in w.entities.list.iter().filter(|e| !e.removed) {
            now.insert(e.id);
            if !self.present.contains(&e.id) {
                f.entered.push(e.id);
            }
            f.entities.push(entity_frame(w, e));
        }
        // the lightning bolts (entities in the game, a list of their own here: `weather::Weather`)
        for b in w.weather.bolts.iter().filter(|b| !b.removed) {
            now.insert(b.id);
            if !self.present.contains(&b.id) {
                f.entered.push(b.id);
            }
            f.entities.push(EntityFrame { n: b.id, ty: "lightning_bolt".into(), pos: [b.pos.x, b.pos.y, b.pos.z], random: Some(b.random.state()), ..Default::default() });
        }
        // the falling blocks (entities in the game, `gravity::State`'s list here: a natural sand
        // or gravel an update dropped, a design's)
        for fb in w.gravity.entities.iter().filter(|e| !e.removed) {
            now.insert(fb.id);
            if !self.present.contains(&fb.id) {
                f.entered.push(fb.id);
            }
            f.entities.push(EntityFrame { n: fb.id, ty: "falling_block".into(), pos: [fb.x, fb.y, fb.z], motion: [fb.vx, fb.vy, fb.vz], fire: Some(0), ..Default::default() });
        }
        f.entities.sort_by_key(|e| e.n);
        f.entered.sort();
        f.left = self.present.difference(&now).copied().collect();
        self.present = now;
        // (tickexact 2026-11-10) the part unloaded: its blocks and containers not read (the
        // bridge's watch reads nothing while its chunks are away: a read would load them); what
        // changed before or while it was away shows at the reload, as there
        if w.away.is_some() {
            return f;
        }
        let mut i = 0;
        if !self.blocks.is_empty() {
            for y in self.lo.y..=self.hi.y {
                for z in self.lo.z..=self.hi.z {
                    for x in self.lo.x..=self.hi.x {
                        let b = w.get(BlockPos::new(x, y, z));
                        if b != self.blocks[i] {
                            f.blocks.push(([x, y, z], b.to_string()));
                            self.blocks[i] = b;
                        }
                        i += 1;
                    }
                }
            }
        }
        let now = self.container_texts(w);
        for (p, s) in &now {
            if self.containers.get(p) != Some(s) {
                f.containers.push((*p, s.clone()));
            }
        }
        for p in self.containers.keys().filter(|p| !now.contains_key(*p)) {
            f.containers.push((*p, String::new()));
        }
        f.containers.sort();
        self.containers = now;
        f
    }
}

/// The weather as the bridge prints it (`Weather::bridge_text`: the timers, the flags and both
/// levels; mobinfra).
fn weather_text(w: &World) -> Option<String> {
    Some(w.weather.bridge_text())
}

/// A boat's or raft's type as the game names it, reduced to what redsim tells apart.
pub fn normalize_type(ty: &str) -> String {
    let t = ty.trim_start_matches("minecraft:");
    if t.ends_with("_chest_boat") || t.ends_with("_chest_raft") {
        "chest_boat".into()
    } else if t.ends_with("_boat") || t.ends_with("_raft") {
        "boat".into()
    } else {
        t.into()
    }
}

/// One redsim entity's frame.
pub fn entity_frame(w: &World, e: &crate::entity::Entity) -> EntityFrame {
    let v = |p: Vec3| [p.x, p.y, p.z];
    let mut f = EntityFrame {
        n: e.id,
        ty: match &e.kind {
            // the game's entity type (a size or a baby is the same type there: sulfur_cube_2 and
            // baby_zombie are redsim's rows of minecraft:sulfur_cube and minecraft:zombie)
            EntityKind::Mob(m) => m.ty.game_id().trim_start_matches("minecraft:").to_string(),
            EntityKind::Item(_) => "item".into(),
            EntityKind::Boat(b) => if b.chest { "chest_boat".into() } else { "boat".into() },
            EntityKind::Minecart(m) => if m.hopper { "hopper_minecart".into() } else { "minecart".into() },
            EntityKind::ArmorStand(_) => "armor_stand".into(),
            EntityKind::PrimedTnt(_) => "tnt".into(),
            EntityKind::Arrow(a) => if a.trident { "trident".into() } else if a.spit { "llama_spit".into() } else { "arrow".into() },
        },
        pos: v(e.pos),
        motion: v(e.delta_movement),
        yaw: e.y_rot,
        pitch: e.x_rot,
        on_ground: e.on_ground,
        vehicle: Some(e.vehicle.map(|id| format!("#{id}")).unwrap_or_else(|| "none".into())),
        random: e.random.as_ref().map(|r| r.state()),
        ..Default::default()
    };
    match &e.kind {
        EntityKind::Item(it) => {
            f.item = Some(format!("{} {}", it.item.trim_start_matches("minecraft:"), it.count));
            f.age = Some(it.age);
            // its fire (a drop in lava burns: tickexact 2026-11-08)
            f.fire = Some(it.fire);
        }
        // (mob AI) an arrow's fire
        EntityKind::Arrow(a) => {
            f.fire = Some(crate::entity::arrow::fire(a));
        }
        EntityKind::Mob(m) => {
            f.fire = Some(m.fire_ticks);
            f.air = Some(m.air);
            f.pose = Some(if m.sleeping_pos.is_some() && !m.pose_standing { "sleeping" } else if m.dead { "dying" } else { "standing" }.into());
            f.health = Some(m.health);
            f.hurt_time = Some(m.hurt_time);
            f.death_time = Some(m.death_time);
            f.no_action = Some(m.no_action_time);
            if let Some(b) = m.brain.as_deref() {
                if b.ex.is_some()
                    && let Some(i) = crate::entity::index_of(w, e.id)
                {
                    // ---- the exact brain (the mob-AI lane): every memory, activities, behaviours ----
                    f.memories = Some(crate::entity::mob::ai::brain::frame_memories(w, i));
                    f.modeled_memories = Some(crate::entity::mob::ai::brain::MODELED);
                    f.activities = Some(crate::entity::mob::ai::brain::frame_activities(w, i));
                    f.behaviors = Some(crate::entity::mob::ai::brain::frame_behaviors(w, i));
                } else {
                    f.memories = Some(villager_memories(b));
                    f.modeled_memories = Some(VILLAGER_MEMORIES);
                }
            }
            // ---- mob AI (the mob-AI lane): head and body yaw, running goals, path, target ----
            if let Some(a) = m.ai.as_deref()
                && let Some(i) = crate::entity::index_of(w, e.id)
            {
                f.head = Some(a.y_head_rot);
                f.body = Some(a.y_body_rot);
                f.goals = Some(crate::entity::mob::ai::frame_goals(a));
                // getNavigation(): a rider steering a mob vehicle reports the vehicle's path
                let owner = crate::entity::mob::ai::nav_owner(w, i);
                f.path = Some(crate::entity::mob::ai::frame_path(crate::entity::mob::ai::ai(w, owner)));
                f.target = Some(crate::entity::mob::ai::frame_target(w, i));
            }
        }
        _ => {}
    }
    f
}

/// A villager brain's memories as the bridge prints them (`Exact.memoryValue`).
fn villager_memories(b: &crate::entity::mob::villager::Brain) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let ids = |l: &[(u32, crate::entity::mob::types::MobType)]| format!("[{}]", l.iter().map(|(id, _)| format!("#{id}")).collect::<Vec<_>>().join(" "));
    if let Some(h) = b.home {
        m.insert("home".into(), format!("{} {} {}", h.x, h.y, h.z));
    }
    if let Some(t) = b.last_slept {
        m.insert("last_slept".into(), t.to_string());
    }
    if let Some(t) = b.last_woken {
        m.insert("last_woken".into(), t.to_string());
    }
    if let Some(ttl) = b.golem_detected {
        m.insert("golem_detected_recently".into(), format!("true ttl {ttl}"));
    }
    if let Some(id) = b.nearest_hostile {
        m.insert("nearest_hostile".into(), format!("#{id}"));
    }
    if !b.nearest_living.is_empty() {
        m.insert("mobs".into(), ids(&b.nearest_living));
    }
    if !b.visible.entities.is_empty() {
        m.insert("visible_mobs".into(), ids(&b.visible.entities));
    }
    m
}

/// What a compare found: the first difference (with its tick), the first difference of each
/// category ("level random", "villager pos", "block"...: each owner's first, even behind another's),
/// and the gaps (fields the game reports that redsim does not model yet, by entity type).
#[derive(Clone, Debug, Default)]
pub struct Compare {
    pub ticks: u64,
    pub first: Option<Difference>,
    /// The compare's tick count when the first difference was found.
    pub first_tick: u64,
    pub firsts: BTreeMap<String, Difference>,
    pub gaps: BTreeMap<String, BTreeSet<String>>,
    /// mob AI (the mob-AI lane): keep the game's draws of this tick (`--probe-at`).
    pub probe_at: Option<i64>,
    pub probe: Vec<String>,
    /// Only these kinds of difference count (`mobfarm compare --only`; empty: every one):
    /// `world` (game and day time, the AFK player, the level random, randValue, the weather),
    /// `blocks` (blocks and containers), `entities` (everything of the entities), or a category's
    /// own name ("level random", "villager pos", ...). The others are still printed as gaps of
    /// their kind, never as the first difference.
    pub only: Vec<String>,
}

/// The kind of a difference's category, for [`Compare::only`].
pub fn kind_of_category(category: &str) -> &'static str {
    match category.strip_prefix("Nether ").unwrap_or(category) {
        "game time" | "AFK player" | "level random" | "randValue" | "day time" | "weather" => "world",
        "block" | "container" => "blocks",
        _ => "entities",
    }
}

/// A difference of a compare.
#[derive(Clone, Debug)]
pub struct Difference {
    pub t: i64,
    /// What differs: "level random", "entity #3 villager pos", "block 1 2 3", ...
    pub what: String,
    /// Its category (what, without entity numbers and cells).
    pub category: String,
    pub game: String,
    pub redsim: String,
    /// Context lines (the level random's draw distance, the game's draw log...).
    pub context: Vec<String>,
}

/// How many `randValue` steps (`value * 3 + 1013904223`) lead from `a` to `b`, within `max`.
pub fn rand_value_steps(a: i32, b: i32, max: u32) -> Option<u32> {
    let mut v = a;
    for k in 0..=max {
        if v == b {
            return Some(k);
        }
        v = v.wrapping_mul(3).wrapping_add(1_013_904_223);
    }
    None
}

impl Compare {
    fn gap(&mut self, ty: &str, field: &str) {
        self.gaps.entry(ty.to_string()).or_default().insert(field.to_string());
    }

    /// Compare one tick's frames: every difference of the tick is found; the first of the
    /// compare and the first of each category are kept. (tickexact 2026-11-10) Their Nether
    /// frames too, when both carry one: its differences named "Nether ...".
    pub fn tick(&mut self, game: &Frame, ours: &Frame) {
        self.ticks += 1;
        self.frame_pair(game, ours, "");
        if let (Some(g), Some(o)) = (&game.nether, &ours.nether) {
            self.frame_pair(g, o, "Nether ");
        }
    }

    fn frame_pair(&mut self, game: &Frame, ours: &Frame, prefix: &str) {
        let t = game.t;
        if self.probe_at == Some(t) && prefix.is_empty() {
            for (stream, site, n) in &game.draws {
                self.probe.push(format!("game draw: {stream} {site} x{n}"));
            }
        }
        let mut ds: Vec<Difference> = Vec::new();
        let mut push = |what: String, category: &str, g: String, o: String, context: Vec<String>| {
            ds.push(Difference { t, what: format!("{prefix}{what}"), category: format!("{prefix}{category}"), game: g, redsim: o, context });
        };
        if game.t != ours.t {
            push("game time".into(), "game time", game.t.to_string(), ours.t.to_string(), Vec::new());
        }
        // the AFK player (the bridge's real player; redsim's `World::afk`): natural spawning's
        // distances, fire spread and the despawn rules all read it
        if let (Some(g), Some(o)) = (game.player, ours.player)
            && g != o
        {
            push("AFK player".into(), "AFK player", format!("{g:?}"), format!("{o:?}"), Vec::new());
        }
        if let Some((g, o)) = opt_cmp("level random", &game.rng, &ours.rng, self, "world") {
            let ctx = match (game.rng, ours.rng) {
                (Some(a), Some(b)) => crate::rng::state_diff(a, b).into_iter().collect(),
                _ => Vec::new(),
            };
            push("level random".into(), "level random", g, o, ctx);
        }
        if let Some((g, o)) = opt_cmp("randValue", &game.rv, &ours.rv, self, "world") {
            let mut ctx = Vec::new();
            if let (Some(a), Some(b)) = (game.rv, ours.rv) {
                match (rand_value_steps(b, a, 100_000), rand_value_steps(a, b, 100_000)) {
                    (Some(k), _) => ctx.push(format!("redsim stepped randValue {k} fewer times (getBlockRandomPos: 3 per randomly ticking section per chunk tick, 1 per ice-and-snow hit)")),
                    (_, Some(k)) => ctx.push(format!("redsim stepped randValue {k} more times")),
                    _ => ctx.push("not within 100000 randValue steps".into()),
                }
            }
            push("randValue".into(), "randValue", g, o, ctx);
        }
        if let Some((g, o)) = opt_cmp("day time", &game.day, &ours.day, self, "world") {
            push("day time".into(), "day time", g, o, Vec::new());
        }
        if let Some((g, o)) = opt_cmp("weather", &game.weather, &ours.weather, self, "world") {
            push("weather".into(), "weather", g, o, Vec::new());
        }
        if game.entered != ours.entered || game.left != ours.left {
            push("entities entered/left".into(), "entered/left", format!("entered {:?} left {:?}", game.entered, game.left), format!("entered {:?} left {:?}", ours.entered, ours.left), Vec::new());
        }
        let ours_by: BTreeMap<u32, &EntityFrame> = ours.entities.iter().map(|e| (e.n, e)).collect();
        for g in &game.entities {
            let Some(o) = ours_by.get(&g.n) else {
                push(format!("entity #{} {}", g.n, g.ty), &format!("{} presence", g.ty), "present".into(), "absent".into(), Vec::new());
                continue;
            };
            for (field, gv, ov) in self.entity(g, o) {
                let mut ctx = Vec::new();
                if field == "random"
                    && let (Some(a), Some(b)) = (g.random, o.random)
                    && let Some(s) = crate::rng::state_diff(a, b)
                {
                    ctx.push(s.replacen("level random", "its own random", 1));
                }
                push(format!("entity #{} {} {field}", g.n, g.ty), &format!("{} {field}", normalize_type(&g.ty)), gv, ov, ctx);
            }
        }
        for o in ours.entities.iter().filter(|o| !game.entities.iter().any(|g| g.n == o.n)) {
            push(format!("entity #{} {}", o.n, o.ty), &format!("{} presence", o.ty), "absent".into(), "present".into(), Vec::new());
        }
        let g: BTreeMap<[i32; 3], &String> = game.blocks.iter().map(|(p, s)| (*p, s)).collect();
        let o: BTreeMap<[i32; 3], &String> = ours.blocks.iter().map(|(p, s)| (*p, s)).collect();
        // both sides' states read the same way: the game's frame text goes through Block::parse
        // (mobfarm::exact::parse_frame), so redsim's does too (a natural-state table row such as
        // grass_block[snowy=false] is redsim's GrassBlock there: its bare name)
        let canon = |s: &str| crate::block::Block::parse(s).map(|b| b.to_string()).unwrap_or_else(|_| s.to_string());
        for p in g.keys().chain(o.keys()).collect::<BTreeSet<_>>() {
            let (a, b) = (g.get(p).map(|s| s.as_str()).unwrap_or("(unchanged)"), o.get(p).map(|s| s.as_str()).unwrap_or("(unchanged)"));
            if a != b && canon(a) != canon(b) {
                push(format!("block {} {} {}", p[0], p[1], p[2]), "block", a.to_string(), b.to_string(), Vec::new());
            }
        }
        let g: BTreeMap<[i32; 3], &String> = game.containers.iter().map(|(p, s)| (*p, s)).collect();
        let o: BTreeMap<[i32; 3], &String> = ours.containers.iter().map(|(p, s)| (*p, s)).collect();
        for p in g.keys().chain(o.keys()).collect::<BTreeSet<_>>() {
            let (a, b) = (g.get(p).map(|s| s.as_str()).unwrap_or("(unchanged)"), o.get(p).map(|s| s.as_str()).unwrap_or("(unchanged)"));
            if a != b {
                push(format!("container {} {} {}", p[0], p[1], p[2]), "container", format!("[{a}]"), format!("[{b}]"), Vec::new());
            }
        }
        for mut x in ds {
            if !self.only.is_empty() && !self.only.iter().any(|k| k == kind_of_category(&x.category) || *k == x.category) {
                continue;
            }
            if self.first.is_none() {
                for (stream, site, n) in game.draws.iter().take(80) {
                    x.context.push(format!("game draw: {stream} {site} x{n}"));
                }
                self.first = Some(x.clone());
                self.first_tick = self.ticks;
            }
            self.firsts.entry(x.category.clone()).or_insert(x);
        }
    }

    /// Every field of an entity that differs: (field, game, redsim).
    fn entity(&mut self, g: &EntityFrame, o: &EntityFrame) -> Vec<(String, String, String)> {
        let ty = g.ty.clone();
        let mut out = Vec::new();
        if normalize_type(&g.ty) != o.ty {
            out.push(("type".into(), g.ty.clone(), o.ty.clone()));
            return out;
        }
        if g.pos != o.pos {
            out.push(("pos".into(), format!("{:?}", g.pos), format!("{:?}", o.pos)));
        }
        if g.motion != o.motion {
            out.push(("motion".into(), format!("{:?}", g.motion), format!("{:?}", o.motion)));
        }
        if g.yaw.to_bits() != o.yaw.to_bits() {
            out.push(("yaw".into(), g.yaw.to_string(), o.yaw.to_string()));
        }
        if g.pitch.to_bits() != o.pitch.to_bits() {
            out.push(("pitch".into(), g.pitch.to_string(), o.pitch.to_string()));
        }
        if g.on_ground != o.on_ground {
            out.push(("on_ground".into(), g.on_ground.to_string(), o.on_ground.to_string()));
        }
        macro_rules! field {
            ($name:literal, $f:ident) => {
                if let Some((a, b)) = opt_cmp($name, &g.$f, &o.$f, self, &ty) {
                    out.push(($name.into(), a, b));
                }
            };
        }
        field!("random", random);
        field!("health", health);
        field!("fire", fire);
        field!("air", air);
        field!("pose", pose);
        field!("vehicle", vehicle);
        field!("item", item);
        field!("age", age);
        field!("hurt_time", hurt_time);
        field!("death_time", death_time);
        field!("no_action", no_action);
        field!("head", head);
        field!("body", body);
        if let Some(gm) = &g.memories {
            match (&o.memories, o.modeled_memories) {
                (Some(om), modeled) if modeled.is_some() || o.all_memories => {
                    for (k, gv) in gm {
                        if !o.all_memories && !modeled.is_some_and(|m| m.contains(&k.as_str())) {
                            self.gap(&ty, &format!("memory {k}"));
                            continue;
                        }
                        let ov = om.get(k).map(|s| s.as_str()).unwrap_or("(none)");
                        if ov != gv {
                            out.push((format!("memory {k}"), gv.clone(), ov.to_string()));
                        }
                    }
                    for (k, ov) in om {
                        if !gm.contains_key(k) {
                            out.push((format!("memory {k}"), "(none)".into(), ov.clone()));
                        }
                    }
                }
                _ => self.gap(&ty, "memories"),
            }
        }
        field!("activities", activities);
        field!("behaviors", behaviors);
        field!("goals", goals);
        field!("path", path);
        field!("target", target);
        out
    }

    /// The report: the first difference with its context (or the ticks compared), the first of
    /// each other category, then the gaps.
    pub fn report(&self) -> String {
        let mut s = String::new();
        match &self.first {
            None => s.push_str(&format!("{} ticks compared: 0 differences\n", self.ticks)),
            Some(d) => {
                s.push_str(&format!("FIRST DIFFERENCE at game time {} (tick {} of {} compared): {}\n  game:   {}\n  redsim: {}\n", d.t, self.first_tick, self.ticks, d.what, d.game, d.redsim));
                for c in &d.context {
                    s.push_str(&format!("  {c}\n"));
                }
                let mut rest: Vec<&Difference> = self.firsts.values().filter(|x| x.category != d.category).collect();
                rest.sort_by_key(|x| x.t);
                if !rest.is_empty() {
                    s.push_str("the first difference of each other category:\n");
                }
                for x in rest {
                    s.push_str(&format!("  t {} {}: game {} | redsim {}\n", x.t, x.what, x.game, x.redsim));
                    for c in &x.context {
                        s.push_str(&format!("      {c}\n"));
                    }
                }
            }
        }
        if !self.gaps.is_empty() {
            s.push_str("not compared (the game reports them, redsim does not model them yet):\n");
            for (ty, fields) in &self.gaps {
                s.push_str(&format!("  {ty}: {}\n", fields.iter().cloned().collect::<Vec<_>>().join(", ")));
            }
        }
        s
    }
}

/// Compare an optional field: the game's `None` = not reported (no check); redsim's `None` with
/// the game's `Some` = a gap; both `Some` and unequal = a difference (game, redsim).
fn opt_cmp<T: PartialEq + std::fmt::Debug>(name: &str, g: &Option<T>, o: &Option<T>, c: &mut Compare, ty: &str) -> Option<(String, String)> {
    match (g, o) {
        (Some(a), Some(b)) if a != b => Some((format!("{a:?}"), format!("{b:?}"))),
        (Some(_), None) => {
            c.gap(ty, name);
            None
        }
        _ => None,
    }
}
