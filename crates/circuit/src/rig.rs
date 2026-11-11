//! A rail run's world (clk-misc 2026-10-21, the rail study's redsim twin): the operations the
//! station and cart loader / unloader runners make ([`crate::station`], [`crate::cartio`]), on the
//! game's sandbox ([`GameRig`], which can record the run as an exact trace for `redsim-fuzz
//! trace-compare`: frames like `clock misc trace-export --exact`, the judge's track in the header's
//! `raw` and its entity actions in `script`) or on redsim ([`SimRig`], the twin).
//!
//! Entities are named by handles: the game's UUIDs, redsim's entity ids. A recorded script names
//! them by the game's network id, which redsim's ids match (`trace-compare` checks them).

use crate::Pos;
use crate::shulker::{self, Stack};
use anyhow::{Context, Result, anyhow, bail};
use mctas::design::EntitySpec;
use mctas::sim::Worker;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

/// An entity as a runner sees it.
#[derive(Clone, Debug)]
pub struct RigEntity {
    pub handle: String,
    pub kind: String,
    pub pos: [f64; 3],
    /// A container entity's stacks, an item entity's stack.
    pub stacks: Vec<Stack>,
}

/// The world a rail run drives.
pub trait Rig {
    /// `n` game ticks.
    fn step(&mut self, n: u32) -> Result<()>;
    /// `summon` (one batch, numbered together like the bridge's `summon` op): the new entities'
    /// handles.
    fn summon(&mut self, specs: &[EntitySpec]) -> Result<Vec<String>>;
    /// `ride <rider> mount <vehicle>`.
    fn ride(&mut self, rider: &str, vehicle: &str) -> Result<()>;
    /// Attach a lead, or remove it without dropping an item when holder is None.
    fn leash(&mut self, entity: &str, holder: Option<&str>) -> Result<()>;
    /// `kill <entity>`.
    fn kill(&mut self, h: &str) -> Result<()>;
    /// `data merge entity <entity> {Items:[]}`: a container entity emptied.
    fn empty(&mut self, h: &str) -> Result<()>;
    /// `data merge block x y z {Items:[...]}`: a container's contents replaced (`items` is the
    /// SNBT list).
    fn fill(&mut self, pos: Pos, items: &str) -> Result<()>;
    /// A player's right-click on the block there (a button press).
    fn press(&mut self, pos: Pos) -> Result<()>;
    fn entities(&mut self) -> Result<Vec<RigEntity>>;
    /// Every block container in the box: its stacks.
    fn containers(&mut self, from: Pos, to: Pos) -> Result<Vec<(Pos, Vec<Stack>)>>;
    /// The block state there (`STATION_TRACE`).
    fn cell(&mut self, p: Pos) -> Result<String>;
}

// ------------------------------------------------------------------------------------- the game

/// An exact trace of a run, frame by frame (the state after t ticks), with its script.
pub struct Recorder {
    from: Pos,
    to: Pos,
    frames: Vec<String>,
    prev: HashMap<Pos, String>,
    ids: HashMap<String, usize>,
    script: Vec<Value>,
    net: HashMap<String, i64>,
}

impl Recorder {
    pub fn new(from: Pos, to: Pos) -> Recorder {
        Recorder { from, to, frames: Vec::new(), prev: HashMap::new(), ids: HashMap::new(), script: Vec::new(), net: HashMap::new() }
    }

    /// The tick an action applied now belongs to (`trace-export`'s order: the inputs due at t, one
    /// tick, frame t).
    fn t(&self) -> usize {
        self.frames.len()
    }

    /// The current state as the next frame.
    pub fn frame(&mut self, w: &mut Worker, sb: u32) -> Result<()> {
        let t = self.frames.len();
        let region = w.get_region(sb, self.from, self.to)?;
        let mut blocks: Vec<Value> = Vec::new();
        for (p, s) in region.iter() {
            let air = mctas::blockstate::is_air(s);
            let changed = match self.prev.get(&p) {
                Some(x) => x != s,
                None => !air,
            };
            if changed {
                blocks.push(json!([p[0], p[1], p[2], s]));
                self.prev.insert(p, s.to_string());
            }
        }
        let mut ents: Vec<Value> = Vec::new();
        for e in w.entities(sb)? {
            let n = self.ids.len();
            let id = *self.ids.entry(e.uuid.clone()).or_insert(n);
            self.net.insert(e.uuid.clone(), e.net_id);
            ents.push(json!({"id": id, "net_id": e.net_id, "type": e.kind, "pos": e.pos, "motion": e.motion, "yaw": e.yaw, "pitch": e.pitch,
                "leash_holder": e.leash_holder, "angular_momentum": e.angular_momentum}));
        }
        let (rng, rv) = w.random_states(sb)?;
        self.frames.push(json!({"t": t, "blocks": blocks, "entities": ents, "rng": rng, "rv": rv}).to_string());
        Ok(())
    }

    /// The trace file: `header` (the design, box, sandbox...) with `ticks` and `script` added.
    pub fn write(&self, path: &Path, mut header: Value) -> Result<()> {
        header["ticks"] = json!(self.frames.len().saturating_sub(1));
        header["script"] = json!(self.script);
        let mut out = std::io::BufWriter::new(std::fs::File::create(path).with_context(|| format!("create {}", path.display()))?);
        writeln!(out, "{header}")?;
        for f in &self.frames {
            writeln!(out, "{f}")?;
        }
        out.flush()?;
        Ok(())
    }
}

/// The game's sandbox, optionally recorded.
pub struct GameRig<'a> {
    pub w: &'a mut Worker,
    pub sb: u32,
    pub rec: Option<Recorder>,
}

impl GameRig<'_> {
    fn net_of(&mut self, h: &str) -> Result<i64> {
        if let Some(r) = &self.rec
            && let Some(n) = r.net.get(h)
        {
            return Ok(*n);
        }
        let e = self.w.entities(self.sb)?.into_iter().find(|e| e.uuid == h).ok_or_else(|| anyhow!("no entity {h}"))?;
        if let Some(r) = &mut self.rec {
            r.net.insert(h.to_string(), e.net_id);
        }
        Ok(e.net_id)
    }

    fn script(&mut self, v: Value) {
        if let Some(r) = &mut self.rec {
            let t = r.t();
            let mut a = vec![json!(t)];
            a.extend(v.as_array().cloned().unwrap_or_default());
            r.script.push(Value::Array(a));
        }
    }
}

impl Rig for GameRig<'_> {
    fn step(&mut self, n: u32) -> Result<()> {
        match &mut self.rec {
            None => self.w.step(self.sb, n).map(|_| ()),
            Some(r) => {
                for _ in 0..n {
                    self.w.step(self.sb, 1)?;
                    r.frame(self.w, self.sb)?;
                }
                Ok(())
            }
        }
    }
    fn summon(&mut self, specs: &[EntitySpec]) -> Result<Vec<String>> {
        let hs = self.w.summon(self.sb, specs)?;
        if hs.len() != specs.len() {
            bail!("summon: {} entities asked, {} uuids", specs.len(), hs.len());
        }
        if self.rec.is_some() {
            let list: Vec<Value> = specs.iter().map(|s| json!([s.kind, s.pos, s.nbt])).collect();
            // (the bridge reuses a killed entity's UUID for the next summon: look the new ones up)
            let fresh = self.w.entities(self.sb)?;
            for h in &hs {
                let n = fresh.iter().find(|e| &e.uuid == h).map(|e| e.net_id).ok_or_else(|| anyhow!("summoned {h} is not there"))?;
                if let Some(r) = &mut self.rec {
                    r.net.insert(h.clone(), n);
                }
            }
            self.script(json!(["summon", list]));
        }
        Ok(hs)
    }
    fn ride(&mut self, rider: &str, vehicle: &str) -> Result<()> {
        self.w.exec(self.sb, &format!("ride {rider} mount {vehicle}"))?;
        if self.rec.is_some() {
            let (a, b) = (self.net_of(rider)?, self.net_of(vehicle)?);
            self.script(json!(["ride", a, b]));
        }
        Ok(())
    }
    fn leash(&mut self, entity: &str, holder: Option<&str>) -> Result<()> {
        self.w.call(json!({"op": "leash", "sandbox": self.sb, "entity": entity, "holder": holder}))?;
        if self.rec.is_some() {
            let e = self.net_of(entity)?;
            let h = holder.map(|h| self.net_of(h)).transpose()?;
            self.script(json!(["leash", e, h]));
        }
        Ok(())
    }
    fn kill(&mut self, h: &str) -> Result<()> {
        let n = if self.rec.is_some() { Some(self.net_of(h)?) } else { None };
        self.w.exec(self.sb, &format!("kill {h}"))?;
        if let Some(n) = n {
            self.script(json!(["kill", n]));
            if let Some(r) = &mut self.rec {
                r.net.remove(h);
            }
        }
        Ok(())
    }
    fn empty(&mut self, h: &str) -> Result<()> {
        self.w.exec(self.sb, &format!("data merge entity {h} {{Items:[]}}"))?;
        if self.rec.is_some() {
            let n = self.net_of(h)?;
            self.script(json!(["empty", n]));
        }
        Ok(())
    }
    fn fill(&mut self, pos: Pos, items: &str) -> Result<()> {
        self.w.exec(self.sb, &format!("data merge block {} {} {} {{Items:{items}}}", pos[0], pos[1], pos[2]))?;
        self.script(json!(["fill", pos, items]));
        Ok(())
    }
    fn press(&mut self, pos: Pos) -> Result<()> {
        clock::verify::apply(self.w, self.sb, &clock::Action::Use { pos })?;
        self.script(json!(["use", pos]));
        Ok(())
    }
    fn entities(&mut self) -> Result<Vec<RigEntity>> {
        Ok(self
            .w
            .entities(self.sb)?
            .into_iter()
            .map(|e| {
                let stacks = if e.kind.trim_start_matches("minecraft:") == "item" { shulker::stack_of_entity(&e.state).into_iter().collect() } else { shulker::stacks_of_entity_items(&e.state) };
                RigEntity { handle: e.uuid, kind: e.kind, pos: e.pos, stacks }
            })
            .collect())
    }
    fn containers(&mut self, from: Pos, to: Pos) -> Result<Vec<(Pos, Vec<Stack>)>> {
        Ok(self.w.block_entities(self.sb, from, to)?.into_iter().map(|be| (be.pos, shulker::stacks_of_be(&be.nbt))).collect())
    }
    fn cell(&mut self, p: Pos) -> Result<String> {
        let r = self.w.get_region(self.sb, p, p)?;
        Ok(r.get(p).map(|s| s.to_string()).unwrap_or_default())
    }
}

// ------------------------------------------------------------------------------------- redsim

/// A circuit built in redsim as the rail judges build it in the game (`clock::verify::place`: raw
/// writes, block-entity data, the wires' shape updates and every block's `self` update, or a
/// player's placement block by block; the design's entities), the sandbox's level random mirrored
/// (`shulker::build_seed` of the box, its chunks) with the bridge's seeded drops, then `raw` written
/// raw (the judge's track and chest). Outer Err: what redsim cannot model; inner: a block a player
/// could not place (the game judge's refusal, word for word).
pub fn sim_world(c: &crate::Circuit, from: Pos, to: Pos, raw: &[(Pos, String)]) -> std::result::Result<std::result::Result<redsim::World, String>, String> {
    sim_world_of(&c.clock, from, to, raw)
}

/// [`sim_world`] for any design (crate::cartsys: a whole cart line, no circuit kind).
pub fn sim_world_of(clk: &clock::Clock, from: Pos, to: Pos, raw: &[(Pos, String)]) -> std::result::Result<std::result::Result<redsim::World, String>, String> {
    use redsim::{Block, World};
    struct C<'a> {
        clock: &'a clock::Clock,
    }
    let c = C { clock: clk };
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let seed = shulker::build_seed(from, to, 0);
    let mut w = World::new(bp(from), [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1]);
    w.mirror_level_random(seed, chunks);
    w.drop_seed = Some(seed);
    let parse = |s: &str| Block::parse(s).map_err(|e| format!("not modeled by redsim: {e}"));
    let last = c.clock.withheld();
    let mut blocks: Vec<(redsim::BlockPos, Block)> = Vec::new();
    for (q, s) in c.clock.design.block_list() {
        if Some(q) != last {
            blocks.push((bp(q), parse(&s)?));
        }
    }
    blocks.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    let be: Vec<(redsim::BlockPos, &str)> = c.clock.design.block_entities.iter().filter(|(q, _)| Some(**q) != last).map(|(q, s)| (bp(*q), s.as_str())).collect();
    match c.clock.placement {
        clock::Placement::Judge => {
            w.place_raw(&blocks);
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| format!("nbt: {e}"))?;
            w.place_updates(&blocks);
        }
        clock::Placement::Player => {
            for (q, s) in &blocks {
                w.set_block(*q, *s, redsim::world::flags::ALL);
            }
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| format!("nbt: {e}"))?;
        }
    }
    for e in &c.clock.design.entities {
        redsim::entity::spawn(&mut w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| format!("not modeled by redsim: {why}"))?;
    }
    let popped = c.clock.design.blocks.keys().filter(|q| Some(**q) != last).find(|q| w.get(bp(**q)).is_air()).copied();
    let floating = blocks.iter().find(|(q, s)| !w.get(*q).is_air() && !redsim::behavior::can_survive(&w, s, *q)).map(|(q, _)| [q.x, q.y, q.z]);
    if let Some(q) = popped.or(floating) {
        let st = c.clock.design.blocks.get(&q).cloned().unwrap_or_default();
        return Ok(Err(format!("{} at {q:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let mut cells = Vec::new();
    for (q, s) in raw {
        cells.push((bp(*q), parse(s)?));
    }
    w.place_raw(&cells);
    if let Some(e) = w.errors.first() {
        return Err(format!("redsim: {e}"));
    }
    Ok(Ok(w))
}

/// A redsim world, the game's sandbox mirrored (its level random from the seed and chunk box, the
/// bridge's seeded drops).
pub struct SimRig {
    pub w: redsim::World,
}

fn bp(p: Pos) -> redsim::BlockPos {
    redsim::BlockPos::new(p[0], p[1], p[2])
}

fn sim_id(h: &str) -> Result<u32> {
    h.parse::<u32>().with_context(|| format!("a redsim entity handle: {h}"))
}

impl SimRig {
    fn check(&self) -> Result<()> {
        if let Some(e) = self.w.errors.first() {
            bail!("redsim: {e}");
        }
        Ok(())
    }
}

impl Rig for SimRig {
    fn step(&mut self, n: u32) -> Result<()> {
        for _ in 0..n {
            self.w.tick();
        }
        self.check()
    }
    fn summon(&mut self, specs: &[EntitySpec]) -> Result<Vec<String>> {
        let mut v = Vec::new();
        for spec in specs {
            let id = redsim::entity::spawn(&mut self.w, &spec.kind, spec.pos, spec.nbt.as_deref()).map_err(|e| anyhow!("redsim: {e}"))?;
            v.push(id.to_string());
        }
        Ok(v)
    }
    fn ride(&mut self, rider: &str, vehicle: &str) -> Result<()> {
        redsim::entity::ride(&mut self.w, sim_id(rider)?, sim_id(vehicle)?).map_err(|e| anyhow!("redsim: {e}"))
    }
    fn leash(&mut self, entity: &str, holder: Option<&str>) -> Result<()> {
        let e = sim_id(entity)?;
        match holder {
            Some(h) => redsim::entity::leash::attach(&mut self.w, e, sim_id(h)?),
            None => redsim::entity::leash::remove(&mut self.w, e),
        }.map_err(|e| anyhow!("redsim: {e}"))
    }
    fn kill(&mut self, h: &str) -> Result<()> {
        redsim::entity::kill(&mut self.w, sim_id(h)?).map_err(|e| anyhow!("redsim: {e}"))
    }
    fn empty(&mut self, h: &str) -> Result<()> {
        let id = sim_id(h)?;
        if let Some(c) = self.w.containers.entities.get_mut(&id) {
            for s in c.slots.iter_mut() {
                *s = None;
            }
        }
        Ok(())
    }
    fn fill(&mut self, pos: Pos, items: &str) -> Result<()> {
        let v = redsim::redstone::container::entity_items(&format!("{{Items:{items}}}")).map_err(|e| anyhow!("redsim: {e}"))?;
        redsim::redstone::container::set_contents(&mut self.w, bp(pos), &v);
        self.check()
    }
    fn press(&mut self, pos: Pos) -> Result<()> {
        if !self.w.use_block(bp(pos)) {
            bail!("redsim: a right-click on {} is not modeled", self.w.get(bp(pos)));
        }
        self.check()
    }
    fn entities(&mut self) -> Result<Vec<RigEntity>> {
        use redsim::entity::EntityKind;
        use redsim::redstone::container as rc;
        let w = &self.w;
        let stack = |s: &rc::Stack| Stack { id: format!("minecraft:{}", rc::item_name(s.item)), count: s.count as u32, contents: Vec::new(), name: None };
        Ok(w.entities
            .list
            .iter()
            .filter(|e| !e.removed)
            .map(|e| {
                let (kind, stacks) = match &e.kind {
                    EntityKind::Minecart(m) if m.hopper => ("minecraft:hopper_minecart".to_string(), w.containers.entities.get(&e.id).map(|c| c.slots.iter().flatten().map(stack).collect()).unwrap_or_default()),
                    EntityKind::Minecart(_) => ("minecraft:minecart".to_string(), Vec::new()),
                    EntityKind::Item(s) => ("minecraft:item".to_string(), vec![Stack { id: if s.item.contains(':') { s.item.clone() } else { format!("minecraft:{}", s.item) }, count: s.count as u32, contents: Vec::new(), name: None }]),
                    EntityKind::ArmorStand(_) => ("minecraft:armor_stand".to_string(), Vec::new()),
                    EntityKind::Boat(b) => (if b.chest { "minecraft:oak_chest_boat" } else { "minecraft:oak_boat" }.to_string(), Vec::new()),
                    EntityKind::PrimedTnt(_) => ("minecraft:tnt".to_string(), Vec::new()),
                    EntityKind::Mob(m) => (m.ty.game_id().to_string(), Vec::new()),
                    EntityKind::Arrow(_) => ("minecraft:arrow".to_string(), Vec::new()),
                };
                RigEntity { handle: e.id.to_string(), kind, pos: [e.pos.x, e.pos.y, e.pos.z], stacks }
            })
            .collect())
    }
    fn containers(&mut self, from: Pos, to: Pos) -> Result<Vec<(Pos, Vec<Stack>)>> {
        use redsim::redstone::container as rc;
        let mut v: Vec<(Pos, Vec<Stack>)> = self
            .w
            .containers
            .map
            .iter()
            .filter(|(q, _)| (0..3).all(|i| [q.x, q.y, q.z][i] >= from[i] && [q.x, q.y, q.z][i] <= to[i]))
            .map(|(q, c)| ([q.x, q.y, q.z], c.slots.iter().flatten().map(|s| Stack { id: format!("minecraft:{}", rc::item_name(s.item)), count: s.count as u32, contents: Vec::new(), name: None }).collect()))
            .collect();
        v.sort_by_key(|(q, _)| *q);
        Ok(v)
    }
    fn cell(&mut self, p: Pos) -> Result<String> {
        Ok(self.w.get(bp(p)).to_string())
    }
}
