//! `mctas door redsim-fuzz`: differential fuzzing of the redsim model against the real game.
//!
//! Random small circuits (a box of random components from a palette, levers as inputs) are run
//! in the bridge sandbox (Minecraft 26.3's own code) and in `redsim`, the same way the judge
//! places designs (raw writes, a shape update of every wire, a `self` update of every block),
//! then ticked with lever flips in between. EVERY block state of the box is compared after
//! placement and after every tick and every flip. A mismatching circuit is shrunk (one block or
//! one step at a time, as long as some mismatch remains) and saved as a case file; `door
//! redsim-case` replays one, `--fixture` writes it as a redsim regression test (the bridge's
//! trace as the expectation, see `crates/redsim/tests/`).

use anyhow::{Context, Result, bail};
use mctas::sim::{PlaceMode, Pool, Pos, SandboxOptions, Worker};
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Mutex;

/// Exact real-game traces of entity / fluid clocks replayed in redsim (track clk-misc).
pub mod trace_compare;
pub mod trace_fixtures;
pub mod loot_gen;
pub mod recipes_gen;
pub mod archive_check;

/// splitmix64.
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as u64) as i32
    }
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 * 1.0 < p
    }
    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
}

/// One step after placement.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Tick n times (a frame after each).
    Run(u32),
    /// Right-click a lever/button between ticks (a frame after it).
    Use(Pos),
    /// Place a state (air = remove) between ticks like a player: `setBlock(.., 3)` (a frame
    /// after it).
    Set(Pos, String),
}

/// A circuit and its inputs.
#[derive(Clone, Debug)]
pub struct Case {
    pub blocks: Vec<(Pos, String)>,
    pub steps: Vec<Step>,
    /// Compared box (inclusive).
    pub lo: Pos,
    pub hi: Pos,
    /// Place the blocks one by one with updates (`setBlock(.., 3)`, onPlace runs) instead of the
    /// judge's way.
    pub update_place: bool,
    /// Container contents set right after the blocks (`data merge block`): (pos, slot, item,
    /// count).
    pub items: Vec<(Pos, usize, String, u8)>,
}

impl Case {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "box {} {} {} {} {} {}", self.lo[0], self.lo[1], self.lo[2], self.hi[0], self.hi[1], self.hi[2]);
        if self.update_place {
            let _ = writeln!(s, "place update");
        }
        for (p, b) in &self.blocks {
            let _ = writeln!(s, "block {} {} {} {}", p[0], p[1], p[2], b);
        }
        for (p, slot, item, count) in &self.items {
            let _ = writeln!(s, "item {} {} {} {slot} {item} {count}", p[0], p[1], p[2]);
        }
        for st in &self.steps {
            match st {
                Step::Run(n) => {
                    let _ = writeln!(s, "run {n}");
                }
                Step::Use(p) => {
                    let _ = writeln!(s, "use {} {} {}", p[0], p[1], p[2]);
                }
                Step::Set(p, b) => {
                    let _ = writeln!(s, "set {} {} {} {b}", p[0], p[1], p[2]);
                }
            }
        }
        s
    }

    pub fn parse(text: &str) -> Result<Case> {
        let mut c = Case { blocks: Vec::new(), steps: Vec::new(), lo: [0; 3], hi: [0; 3], update_place: false, items: Vec::new() };
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let t: Vec<&str> = line.split_whitespace().collect();
            let int = |i: usize| -> Result<i32> { t.get(i).context("missing field")?.parse::<i32>().with_context(|| format!("bad number in {line:?}")) };
            match t[0] {
                "box" => {
                    c.lo = [int(1)?, int(2)?, int(3)?];
                    c.hi = [int(4)?, int(5)?, int(6)?];
                }
                "place" => c.update_place = t.get(1) == Some(&"update"),
                "item" => c.items.push(([int(1)?, int(2)?, int(3)?], int(4)? as usize, t.get(5).context("missing item")?.to_string(), int(6)? as u8)),
                "block" => c.blocks.push(([int(1)?, int(2)?, int(3)?], t.get(4).context("missing state")?.to_string())),
                // a box of one state (mobinfra: lava's floors), written back as its blocks
                "fill" => {
                    let (a, b) = ([int(1)?, int(2)?, int(3)?], [int(4)?, int(5)?, int(6)?]);
                    let st = t.get(7).context("missing state")?.to_string();
                    for y in a[1].min(b[1])..=a[1].max(b[1]) {
                        for z in a[2].min(b[2])..=a[2].max(b[2]) {
                            for x in a[0].min(b[0])..=a[0].max(b[0]) {
                                c.blocks.push(([x, y, z], st.clone()));
                            }
                        }
                    }
                }
                "run" => c.steps.push(Step::Run(int(1)? as u32)),
                "use" => c.steps.push(Step::Use([int(1)?, int(2)?, int(3)?])),
                "set" => c.steps.push(Step::Set([int(1)?, int(2)?, int(3)?], t.get(4).context("missing state")?.to_string())),
                // fixture expectations are read by the redsim test, not here
                "frame" | "cell" => {}
                _ => bail!("bad case line {line:?}"),
            }
        }
        Ok(c)
    }

    pub fn cells(&self) -> Vec<Pos> {
        let mut v = Vec::new();
        for y in self.lo[1]..=self.hi[1] {
            for z in self.lo[2]..=self.hi[2] {
                for x in self.lo[0]..=self.hi[0] {
                    v.push([x, y, z]);
                }
            }
        }
        v
    }
}

/// One frame: the state of every cell of the box (x fastest, then z, then y), canonical strings.
pub type Frame = Vec<String>;

/// The bridge's state strings, canonicalized through redsim's parser when it knows the block
/// (so formatting differences never count), as-is otherwise.
fn canon(cache: &mut HashMap<String, String>, s: &str) -> String {
    if let Some(c) = cache.get(s) {
        return c.clone();
    }
    let c = match Block::parse(s) {
        Ok(b) => b.to_string(),
        Err(_) => format!("?{s}"),
    };
    cache.insert(s.to_string(), c.clone());
    c
}

fn region_frame(r: &mctas::sim::Region, cache: &mut HashMap<String, String>) -> Frame {
    r.data.iter().map(|&i| canon(cache, &r.palette[i as usize])).collect()
}

/// Run a case in the real game.
pub fn run_bridge(w: &mut Worker, sb: u32, c: &Case) -> Result<Vec<Frame>> {
    let mut cache = HashMap::new();
    w.clear(sb)?;
    if !c.blocks.is_empty() {
        if c.update_place {
            // One block per request: the bridge's `set` reuses ONE MutableBlockPos for all its
            // blocks, and a block event queued by an onPlace (a piston) keeps that object, so
            // the next block's position overwrites the event's (it then runs at the wrong cell).
            for b in &c.blocks {
                w.set_blocks(sb, std::slice::from_ref(b), PlaceMode::Update)?;
            }
        } else {
            w.set_blocks(sb, &c.blocks, PlaceMode::Raw)?;
        }
    }
    for (p, items) in group_items(c) {
        let list: Vec<String> = items.iter().map(|(slot, item, count)| format!("{{Slot:{slot}b,id:\"minecraft:{item}\",count:{count}}}")).collect();
        w.exec(sb, &format!("data merge block {} {} {} {{Items:[{}]}}", p[0], p[1], p[2], list.join(",")))?;
    }
    if !c.update_place {
        let wires: Vec<Pos> = c.blocks.iter().filter(|(_, s)| s.contains("redstone_wire")).map(|(p, _)| *p).collect();
        if !wires.is_empty() {
            w.update(sb, &wires, "shape")?;
        }
        let all: Vec<Pos> = c.blocks.iter().map(|(p, _)| *p).collect();
        if !all.is_empty() {
            w.update(sb, &all, "self")?;
        }
    }
    let mut frames = vec![region_frame(&w.get_region(sb, c.lo, c.hi)?, &mut cache)];
    for st in &c.steps {
        match st {
            Step::Run(n) => {
                let r = w.step_record(sb, *n, false, c.lo, c.hi)?;
                if r.frames.len() != *n as usize {
                    bail!("bridge recorded {} frames for {n} ticks", r.frames.len());
                }
                for f in &r.frames {
                    frames.push(region_frame(f, &mut cache));
                }
            }
            Step::Use(p) => {
                w.use_block(sb, *p)?;
                frames.push(region_frame(&w.get_region(sb, c.lo, c.hi)?, &mut cache));
            }
            Step::Set(p, s) => {
                w.set_blocks(sb, &[(*p, s.clone())], PlaceMode::Update)?;
                frames.push(region_frame(&w.get_region(sb, c.lo, c.hi)?, &mut cache));
            }
        }
    }
    Ok(frames)
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A case's container contents by container, in first-appearance order.
fn group_items(c: &Case) -> Vec<(Pos, Vec<(usize, String, u8)>)> {
    let mut out: Vec<(Pos, Vec<(usize, String, u8)>)> = Vec::new();
    for (p, slot, item, count) in &c.items {
        match out.iter_mut().find(|(q, _)| q == p) {
            Some((_, v)) => v.push((*slot, item.clone(), *count)),
            None => out.push((*p, vec![(*slot, item.clone(), *count)])),
        }
    }
    out
}

/// The same contents in redsim (`data merge block`: contents replaced, then setChanged).
fn sim_items(w: &mut World, c: &Case) -> Result<()> {
    use redsim::redstone::container::{Stack, item_id, set_contents};
    for (p, items) in group_items(c) {
        let mut v = Vec::new();
        for (slot, item, count) in items {
            let id = item_id(&item).with_context(|| format!("unknown item {item}"))?;
            v.push((slot, Stack::new(id, count)));
        }
        set_contents(w, bp(p), &v);
    }
    Ok(())
}

fn sim_frame(w: &World, cells: &[Pos]) -> Frame {
    cells.iter().map(|p| w.get(bp(*p)).to_string()).collect()
}

/// Run a case in redsim; also returns the model's own error notes.
pub fn run_sim(c: &Case) -> Result<(Vec<Frame>, Vec<String>)> {
    run_sim_noted(c).map(|(f, n, _)| (f, n))
}

/// [`run_sim`] plus the first frame at which the model had a note (frames before it are in
/// scope even when a note comes later: a primed TNT's explosion 80 gt on).
pub fn run_sim_noted(c: &Case) -> Result<(Vec<Frame>, Vec<String>, Option<usize>)> {
    let margin = 6;
    let min = BlockPos::new(c.lo[0] - margin, c.lo[1] - margin, c.lo[2] - margin);
    let size = [c.hi[0] - c.lo[0] + 1 + 2 * margin, c.hi[1] - c.lo[1] + 1 + 2 * margin, c.hi[2] - c.lo[2] + 1 + 2 * margin];
    let mut w = World::new(min, size);
    let mut blocks = Vec::new();
    for (p, s) in &c.blocks {
        blocks.push((bp(*p), Block::parse(s).map_err(|e| anyhow::anyhow!(e))?));
    }
    if c.update_place {
        for (p, b) in &blocks {
            w.set_block(*p, *b, redsim::world::flags::ALL);
        }
        sim_items(&mut w, c)?;
    } else {
        w.place_raw(&blocks);
        sim_items(&mut w, c)?;
        w.place_updates(&blocks);
    }
    let cells = c.cells();
    let mut frames = vec![sim_frame(&w, &cells)];
    let mut note_frame: Option<usize> = if w.errors.is_empty() { None } else { Some(0) };
    for st in &c.steps {
        match st {
            Step::Run(n) => {
                for _ in 0..*n {
                    w.tick();
                    frames.push(sim_frame(&w, &cells));
                    if note_frame.is_none() && !w.errors.is_empty() {
                        note_frame = Some(frames.len() - 1);
                    }
                }
            }
            Step::Use(p) => {
                w.use_block(bp(*p));
                frames.push(sim_frame(&w, &cells));
            }
            Step::Set(p, s) => {
                let b = Block::parse(s).map_err(|e| anyhow::anyhow!(e))?;
                w.set_block(bp(*p), b, redsim::world::flags::ALL);
                frames.push(sim_frame(&w, &cells));
            }
        }
        if note_frame.is_none() && !w.errors.is_empty() {
            note_frame = Some(frames.len() - 1);
        }
    }
    let mut notes = w.errors.clone();
    // item entities (drops of popped blocks) can fall into hoppers below them: out of scope
    let hoppers: Vec<BlockPos> = w.containers.map.iter().filter(|(_, c)| c.hopper).map(|(p, _)| *p).collect();
    let near = |d: &BlockPos| hoppers.iter().any(|h| (d.x - h.x).abs() <= 1 && (d.z - h.z).abs() <= 1 && d.y >= h.y && d.y - h.y <= 4);
    if let Some((p, b)) = w.drops.iter().find(|(p, _)| near(p)) {
        notes.push(format!("drops near hoppers (item entities, not modeled): {p} {b}"));
        // (when the drop fell is not tracked: the whole case is out of scope)
        note_frame = Some(0);
    }
    Ok((frames, notes, note_frame))
}

/// Where the two traces first differ: (frame index, [(cell, redsim, game)]).
pub fn first_diff(c: &Case, sim: &[Frame], game: &[Frame]) -> Option<(usize, Vec<(Pos, String, String)>)> {
    let cells = c.cells();
    for (i, (a, b)) in sim.iter().zip(game.iter()).enumerate() {
        if a != b {
            let d = cells.iter().zip(a.iter().zip(b.iter())).filter(|(_, (x, y))| x != y).map(|(p, (x, y))| (*p, x.clone(), y.clone())).collect();
            return Some((i, d));
        }
    }
    if sim.len() != game.len() {
        return Some((sim.len().min(game.len()), Vec::new()));
    }
    None
}

/// What frame `i` is: "placed", "tick k of run j", "after use x y z".
pub fn frame_label(c: &Case, i: usize) -> String {
    if i == 0 {
        return "after placement".into();
    }
    let mut k = 1;
    let mut t = 0;
    for st in &c.steps {
        match st {
            Step::Run(n) => {
                for j in 0..*n {
                    t += 1;
                    if k == i {
                        return format!("tick {t} (run step tick {})", j + 1);
                    }
                    k += 1;
                }
            }
            Step::Use(p) => {
                if k == i {
                    return format!("right after use {} {} {} (after tick {t})", p[0], p[1], p[2]);
                }
                k += 1;
            }
            Step::Set(p, s) => {
                if k == i {
                    return format!("right after set {} {} {} {s} (after tick {t})", p[0], p[1], p[2]);
                }
                k += 1;
            }
        }
    }
    format!("frame {i}")
}

// ---------------------------------------------------------------------- generator

fn hdir(r: &mut Rng) -> &'static str {
    *r.pick(&["north", "east", "south", "west"])
}
fn anydir(r: &mut Rng) -> &'static str {
    *r.pick(&["north", "east", "south", "west", "up", "down"])
}
fn tf(r: &mut Rng, p: f64) -> &'static str {
    if r.chance(p) { "true" } else { "false" }
}

/// Palette families and weights (`--palette core|all|<name>:w,...`).
pub fn palette(spec: &str) -> Result<Vec<(String, u32)>> {
    let core: &[(&str, u32)] = &[
        ("stone", 12),
        ("glass", 3),
        ("simple", 3),
        ("dust", 16),
        ("repeater", 7),
        ("comparator", 5),
        ("torch", 4),
        ("walltorch", 4),
        ("observer", 5),
        ("rblock", 2),
        ("lamp", 2),
        ("bulb", 2),
        ("note", 2),
        ("target", 2),
        ("lever", 1),
        ("button", 1),
    ];
    let extra: &[(&str, u32)] = &[
        ("trapdoor", 2),
        ("gate", 1),
        ("skull", 1),
        ("rail", 2),
        ("dispenser", 2),
        ("crafter", 1),
        // hoppers move items (block entities + item entities: out of scope); `hopper:1` adds them
        ("hopper", 0),
        ("slab", 2),
        ("leaves", 1),
        ("log", 1),
        ("scaffold", 2),
        ("scaffold7", 0),
        ("analog", 2),
        ("door", 1),
        // plain blocks of every equivalence class and pistons: `--palette core,plain:10,piston:4`
        ("plain", 0),
        ("piston", 0),
        // droppers/hoppers with items: `--palette core,container:12`
        ("container", 0),
        // fluids (redsim M1): `--palette core,water:12,soulsand:2,magma:2`
        ("water", 0),
        ("soulsand", 0),
        ("magma", 0),
        // plain and detector rails (redsim M4)
        ("plainrail", 0),
        ("detrail", 0),
        // TNT and dead coral fans (track tnt): `--palette core,piston:4,tnt:3,coral:3`
        ("tnt", 0),
        ("coral", 0),
    ];
    let mut out: Vec<(String, u32)> = Vec::new();
    for tok in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match tok {
            "core" => out.extend(core.iter().map(|(n, w)| (n.to_string(), *w))),
            "extra" => out.extend(extra.iter().map(|(n, w)| (n.to_string(), *w))),
            "all" => {
                out.extend(core.iter().map(|(n, w)| (n.to_string(), *w)));
                out.extend(extra.iter().map(|(n, w)| (n.to_string(), *w)));
            }
            t => {
                let (n, w) = match t.split_once(':') {
                    Some((n, w)) => (n, w.parse::<u32>().context("bad weight")?),
                    None => (t, 1),
                };
                if !core.iter().chain(extra.iter()).any(|(m, _)| *m == n) {
                    bail!("unknown palette family {n:?}");
                }
                out.retain(|(m, _)| m != n);
                out.push((n.to_string(), w));
            }
        }
    }
    out.retain(|(_, w)| *w > 0);
    if out.is_empty() {
        bail!("empty palette");
    }
    Ok(out)
}

/// One random state of a family (None: the family places nothing here).
fn random_state(r: &mut Rng, fam: &str) -> Vec<String> {
    let st = |s: String| vec![format!("minecraft:{s}")];
    match fam {
        "stone" => st("stone".into()),
        "glass" => st("glass".into()),
        "simple" => st(r
            .pick(&["white_wool", "oak_planks", "gold_block", "soul_sand", "slime_block", "honey_block", "waxed_copper_grate[waterlogged=false]", "glowstone", "obsidian", "smooth_stone", "packed_ice", "clay", "sea_lantern", "tinted_glass", "iron_block", "emerald_block"])
            .to_string()),
        // any plain block of the class universe (redsim::classes)
        "plain" => {
            let u = redsim::classes::universe();
            vec![r.pick(&u).to_string()]
        }
        "piston" => vec![format!("minecraft:{}[extended=false,facing={}]", if r.chance(0.5) { "sticky_piston" } else { "piston" }, anydir(r))],
        "dust" => {
            let side = |r: &mut Rng| *r.pick(&["none", "side", "side", "up"]);
            let (n, e, s, w) = match r.below(4) {
                0 => ("side", "side", "side", "side"),
                1 => ("none", "none", "none", "none"),
                _ => (side(r), side(r), side(r), side(r)),
            };
            let p = if r.chance(0.15) { r.range(0, 15) } else { 0 };
            st(format!("redstone_wire[east={e},north={n},power={p},south={s},west={w}]"))
        }
        "repeater" => {
            let d = r.range(1, 4);
            st(format!("repeater[delay={d},facing={},locked={},powered={}]", hdir(r), tf(r, 0.1), tf(r, 0.15)))
        }
        "comparator" => st(format!("comparator[facing={},mode={},powered={}]", hdir(r), if r.chance(0.5) { "subtract" } else { "compare" }, tf(r, 0.15))),
        "torch" => st(format!("redstone_torch[lit={}]", tf(r, 0.8))),
        "walltorch" => st(format!("redstone_wall_torch[facing={},lit={}]", hdir(r), tf(r, 0.8))),
        "observer" => st(format!("observer[facing={},powered={}]", anydir(r), tf(r, 0.1))),
        "rblock" => st("redstone_block".into()),
        "lamp" => st(format!("redstone_lamp[lit={}]", tf(r, 0.2))),
        "bulb" => {
            let n = *r.pick(&["waxed_copper_bulb", "waxed_exposed_copper_bulb", "waxed_weathered_copper_bulb", "waxed_oxidized_copper_bulb"]);
            st(format!("{n}[lit={},powered={}]", tf(r, 0.3), tf(r, 0.2)))
        }
        "note" => st(format!("note_block[instrument=harp,note={},powered={}]", r.range(0, 24), tf(r, 0.15))),
        "target" => st(format!("target[power={}]", if r.chance(0.1) { r.range(1, 15) } else { 0 })),
        "lever" | "button" => {
            let face = *r.pick(&["floor", "wall", "wall", "ceiling"]);
            let n = if fam == "lever" { "lever" } else if r.chance(0.5) { "stone_button" } else { "oak_button" };
            st(format!("{n}[face={face},facing={},powered={}]", hdir(r), tf(r, 0.2)))
        }
        "trapdoor" => st(format!(
            "{}[facing={},half={},open={},powered={},waterlogged=false]",
            if r.chance(0.5) { "iron_trapdoor" } else { "oak_trapdoor" },
            hdir(r),
            if r.chance(0.5) { "top" } else { "bottom" },
            tf(r, 0.2),
            tf(r, 0.15)
        )),
        "gate" => st(format!("oak_fence_gate[facing={},in_wall=false,open={},powered={}]", hdir(r), tf(r, 0.2), tf(r, 0.15))),
        "skull" => {
            if r.chance(0.5) {
                st(format!("skeleton_skull[powered={},rotation={}]", tf(r, 0.2), r.range(0, 15)))
            } else {
                st(format!("skeleton_wall_skull[facing={},powered={}]", hdir(r), tf(r, 0.2)))
            }
        }
        "rail" => {
            let shape = *r.pick(&["north_south", "east_west", "north_south", "east_west", "ascending_east", "ascending_west", "ascending_north", "ascending_south"]);
            st(format!("{}[powered={},shape={shape},waterlogged=false]", if r.chance(0.7) { "powered_rail" } else { "activator_rail" }, tf(r, 0.2)))
        }
        // plain rails (curves too) and detector rails (redsim M4: rails.rs): `--palette
        // stone:6,plainrail:6,detrail:3,rail:3,...`
        "plainrail" => {
            let shape = *r.pick(&[
                "north_south",
                "east_west",
                "north_south",
                "east_west",
                "ascending_east",
                "ascending_west",
                "ascending_north",
                "ascending_south",
                "south_east",
                "south_west",
                "north_west",
                "north_east",
            ]);
            st(format!("rail[shape={shape},waterlogged=false]"))
        }
        "detrail" => {
            let shape = *r.pick(&["north_south", "east_west", "ascending_east", "ascending_west", "ascending_north", "ascending_south"]);
            st(format!("detector_rail[powered={},shape={shape},waterlogged=false]", tf(r, 0.15)))
        }
        "dispenser" => st(format!("{}[facing={},triggered={}]", if r.chance(0.5) { "dispenser" } else { "dropper" }, anydir(r), tf(r, 0.15))),
        // item transfer: droppers and hoppers (contents from `container_items`)
        "container" => {
            if r.chance(0.6) {
                st(format!("dropper[facing={},triggered=false]", anydir(r)))
            } else {
                st(format!("hopper[enabled=true,facing={}]", r.pick(&["down", "north", "east", "south", "west"])))
            }
        }
        "crafter" => st(format!("crafter[crafting=false,orientation={},triggered={}]", r.pick(&redsim::block::ORIENTATIONS), tf(r, 0.15))),
        "hopper" => st(format!("hopper[enabled={},facing={}]", tf(r, 0.8), r.pick(&["down", "north", "east", "south", "west"]))),
        "slab" => st(format!("smooth_stone_slab[type={},waterlogged=false]", r.pick(&["bottom", "top", "top", "double"]))),
        // water (redsim M1): mostly sources; flowing and falling levels too (they recompute at
        // their first fluid tick)
        "water" => st(format!("water[level={}]", if r.chance(0.6) { 0 } else { r.range(1, 15) })),
        "soulsand" => st("soul_sand".into()),
        "magma" => st("magma_block".into()),
        // TNT dupers (track tnt): TNT (primes when powered; an explosion is a model note) and dead
        // coral fans, floor or wall, sometimes waterlogged
        "tnt" => st("tnt[unstable=false]".into()),
        "coral" => {
            let k = *r.pick(&redsim::tnt::DEAD_CORALS);
            let wl = tf(r, 0.1);
            if r.chance(0.5) { st(format!("dead_{k}_coral_fan[waterlogged={wl}]")) } else { st(format!("dead_{k}_coral_wall_fan[facing={},waterlogged={wl}]", hdir(r))) }
        }
        "leaves" => st(format!("oak_leaves[distance={},persistent=true,waterlogged=false]", r.range(1, 7))),
        // scaffolding: distance 7 (only from placement) makes it fall as an entity at its first
        // tick (`scaffold7:1` adds those)
        "scaffold" => st(format!("scaffolding[bottom={},distance={},waterlogged=false]", tf(r, 0.5), r.range(0, 6))),
        "scaffold7" => st(format!("scaffolding[bottom={},distance=7,waterlogged=false]", tf(r, 0.5))),
        "log" => st(format!("oak_log[axis={}]", r.pick(&["x", "y", "z"]))),
        "analog" => match r.below(4) {
            0 => st(format!("cake[bites={}]", r.range(0, 6))),
            1 => st(format!("end_portal_frame[eye={},facing={}]", tf(r, 0.5), hdir(r))),
            2 => st(format!("composter[level={}]", r.range(0, 8))),
            _ => st(format!("respawn_anchor[charges={}]", r.range(0, 4))),
        },
        "door" => {
            let iron = if r.chance(0.5) { "iron_door" } else { "oak_door" };
            let (f, h, o, p) = (hdir(r), if r.chance(0.5) { "left" } else { "right" }, tf(r, 0.2), tf(r, 0.15));
            vec![
                format!("minecraft:{iron}[facing={f},half=lower,hinge={h},open={o},powered={p}]"),
                format!("minecraft:{iron}[facing={f},half=upper,hinge={h},open={o},powered={p}]"),
            ]
        }
        _ => Vec::new(),
    }
}

/// [`random_case`] in a stone basin: the compared box's floor and four side walls are stone
/// (levers kept), so water stays inside it (redsim's fluids, M1).
pub fn basin_case(r: &mut Rng, pal: &[(String, u32)], size: [i32; 3], density: f64) -> Case {
    let mut c = random_case(r, pal, size, density);
    let (lo, hi) = (c.lo, c.hi);
    let wall = |p: &Pos| p[1] < hi[1] && (p[1] == lo[1] || p[0] == lo[0] || p[0] == hi[0] || p[2] == lo[2] || p[2] == hi[2]);
    let levers: Vec<Pos> = c.blocks.iter().filter(|(_, s)| s.contains("lever")).map(|(p, _)| *p).collect();
    c.blocks.retain(|(p, s)| !wall(p) || s.contains("lever"));
    for y in lo[1]..hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let p = [x, y, z];
                if wall(&p) && !levers.contains(&p) {
                    c.blocks.push((p, "minecraft:stone".into()));
                }
            }
        }
    }
    c
}

/// A random case: a box of random components, 1-2 input levers on solid blocks, flips.
pub fn random_case(r: &mut Rng, pal: &[(String, u32)], size: [i32; 3], density: f64) -> Case {
    let total: u32 = pal.iter().map(|(_, w)| *w).sum();
    let ox = r.range(-14, 15 - size[0]);
    let oz = r.range(-14, 15 - size[2]);
    let oy = r.range(0, 3);
    let lo = [ox, oy, oz];
    let hi = [ox + size[0] - 1, oy + size[1] - 1, oz + size[2] - 1];
    let mut grid: HashMap<Pos, String> = HashMap::new();
    let mut order: Vec<Pos> = Vec::new();
    // with containers in the palette: a stone floor under the box (nothing pops onto hoppers)
    if pal.iter().any(|(n, w)| n == "container" && *w > 0) {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let p = [x, lo[1] - 1, z];
                grid.insert(p, "minecraft:stone".into());
                order.push(p);
            }
        }
    }
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let p = [x, y, z];
                if grid.contains_key(&p) || !r.chance(density) {
                    continue;
                }
                let mut k = r.below(total as u64) as u32;
                let mut fam = pal[0].0.as_str();
                for (n, w) in pal {
                    if k < *w {
                        fam = n;
                        break;
                    }
                    k -= w;
                }
                let states = random_state(r, fam);
                if states.len() == 2 {
                    let up = [x, y + 1, z];
                    if y + 1 > hi[1] || grid.contains_key(&up) {
                        continue;
                    }
                    grid.insert(p, states[0].clone());
                    grid.insert(up, states[1].clone());
                    order.push(p);
                    order.push(up);
                } else if let Some(s) = states.into_iter().next() {
                    grid.insert(p, s);
                    order.push(p);
                }
            }
        }
    }
    // Input levers: on a stone block's face, in an empty cell of the box.
    let mut inputs = Vec::new();
    let n_inputs = r.range(1, 2);
    for _ in 0..40 {
        if inputs.len() >= n_inputs as usize {
            break;
        }
        let p = [r.range(lo[0], hi[0]), r.range(lo[1], hi[1]), r.range(lo[2], hi[2])];
        if grid.contains_key(&p) {
            continue;
        }
        let faces: [(&str, &str, Pos); 6] = [
            ("floor", "north", [0, -1, 0]),
            ("ceiling", "north", [0, 1, 0]),
            ("wall", "south", [0, 0, -1]),
            ("wall", "north", [0, 0, 1]),
            ("wall", "east", [-1, 0, 0]),
            ("wall", "west", [1, 0, 0]),
        ];
        let (face, facing, off) = *r.pick(&faces);
        let sup = [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        let ok = match grid.get(&sup) {
            Some(s) => s == "minecraft:stone",
            None => {
                // put a stone there if the cell is free and inside the box
                if (0..3).all(|i| sup[i] >= lo[i] && sup[i] <= hi[i]) {
                    grid.insert(sup, "minecraft:stone".into());
                    order.push(sup);
                    true
                } else {
                    false
                }
            }
        };
        if !ok {
            continue;
        }
        let facing = if face == "floor" || face == "ceiling" { hdir(r) } else { facing };
        grid.insert(p, format!("minecraft:lever[face={face},facing={facing},powered=false]"));
        order.push(p);
        inputs.push(p);
    }
    // most components that need support get a stone under them (or behind a wall torch), so
    // fewer pop at placement (popped items would also feed hoppers)
    for p in order.clone() {
        let s = grid[&p].clone();
        if !r.chance(0.85) {
            continue;
        }
        let sup: Option<Pos> = if ["redstone_wire", "repeater", "comparator", "redstone_torch[", "_rail[", "_door[", "half=lower"].iter().any(|n| s.contains(n)) && !s.contains("half=upper") {
            Some([p[0], p[1] - 1, p[2]])
        } else if s.contains("redstone_wall_torch") {
            let f = ["north", "south", "west", "east"].iter().find(|f| s.contains(&format!("facing={f}"))).copied().unwrap_or("north");
            let d: Pos = match f {
                "north" => [0, 0, 1],
                "south" => [0, 0, -1],
                "west" => [1, 0, 0],
                _ => [-1, 0, 0],
            };
            Some([p[0] + d[0], p[1] + d[1], p[2] + d[2]])
        } else {
            None
        };
        if let Some(q) = sup
            && (0..3).all(|i| q[i] >= lo[i] - if i == 1 { 1 } else { 0 } && q[i] <= hi[i])
            && !grid.contains_key(&q)
        {
            grid.insert(q, "minecraft:stone".into());
            order.push(q);
        }
    }
    // containers mostly face a neighboring container (item transfer instead of spitting items)
    let dirs: [(&str, Pos); 6] = [("north", [0, 0, -1]), ("south", [0, 0, 1]), ("west", [-1, 0, 0]), ("east", [1, 0, 0]), ("up", [0, 1, 0]), ("down", [0, -1, 0])];
    for p in &order {
        let s = grid[p].clone();
        let is_cont = |s: &str| s.contains("minecraft:dropper") || s.contains("minecraft:hopper");
        if !is_cont(&s) || !r.chance(0.85) {
            continue;
        }
        let hopper = s.contains("minecraft:hopper");
        let targets: Vec<&str> = dirs
            .iter()
            .filter(|(n, d)| !(hopper && *n == "up") && grid.get(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]]).is_some_and(|t| is_cont(t)))
            .map(|(n, _)| *n)
            .collect();
        if targets.is_empty() {
            continue;
        }
        let f = *r.pick(&targets);
        let ns = if hopper { format!("minecraft:hopper[enabled=true,facing={f}]") } else { format!("minecraft:dropper[facing={f},triggered=false]") };
        grid.insert(*p, ns);
    }
    let blocks: Vec<(Pos, String)> = order.iter().map(|p| (*p, grid[p].clone())).collect();
    let total2 = total;
    let random_single = |r: &mut Rng| -> String {
        loop {
            let mut k = r.below(total2 as u64) as u32;
            let mut fam = pal[0].0.as_str();
            for (n, w) in pal {
                if k < *w {
                    fam = n;
                    break;
                }
                k -= w;
            }
            let v = random_state(r, fam);
            if v.len() == 1 {
                return v[0].clone();
            }
        }
    };
    let edits = r.chance(0.4);
    let usable: Vec<Pos> = order
        .iter()
        .filter(|p| {
            let s = &grid[*p];
            ["note_block", "repeater", "comparator", "redstone_wire", "oak_trapdoor", "oak_door"].iter().any(|n| s.contains(n))
        })
        .copied()
        .collect();
    let mut steps = vec![Step::Run(r.range(10, 30) as u32)];
    let flips = r.range(2, 5);
    for _ in 0..flips {
        if !inputs.is_empty() {
            steps.push(Step::Use(*r.pick(&inputs)));
        }
        if !usable.is_empty() && r.chance(0.3) {
            steps.push(Step::Use(*r.pick(&usable)));
        }
        if edits {
            for _ in 0..r.range(0, 2) {
                let p = [r.range(lo[0], hi[0]), r.range(lo[1], hi[1]), r.range(lo[2], hi[2])];
                if inputs.contains(&p) {
                    continue;
                }
                let s = if r.chance(0.5) { "minecraft:air".to_string() } else { random_single(r) };
                steps.push(Step::Set(p, s));
            }
        }
        steps.push(Step::Run(r.range(1, 24) as u32));
    }
    steps.push(Step::Run(16));
    let update_place = r.chance(0.35);
    Case { blocks, steps, lo: [lo[0] - 1, lo[1] - 1, lo[2] - 1], hi: [hi[0] + 1, hi[1] + 1, hi[2] + 1], update_place, items: container_items(r, &order, &grid) }
}

/// A container circuit with nothing that can pop or fall (`--gen containers`): a stone floor,
/// droppers and hoppers (mostly facing each other) with one item type, comparators standing on
/// the floor or on containers, floor levers as inputs, glass and stone.
pub fn container_case(r: &mut Rng, size: [i32; 3]) -> Case {
    let ox = r.range(-14, 15 - size[0]);
    let oz = r.range(-14, 15 - size[2]);
    let oy = r.range(1, 3);
    let lo = [ox, oy, oz];
    let hi = [ox + size[0] - 1, oy + size[1] - 1, oz + size[2] - 1];
    let mut grid: HashMap<Pos, String> = HashMap::new();
    let mut order: Vec<Pos> = Vec::new();
    for z in lo[2]..=hi[2] {
        for x in lo[0]..=hi[0] {
            let p = [x, lo[1] - 1, z];
            grid.insert(p, "minecraft:stone".into());
            order.push(p);
        }
    }
    let mut inputs = Vec::new();
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let p = [x, y, z];
                let below = grid.get(&[x, y - 1, z]).cloned().unwrap_or_default();
                let solid_below = below.contains("stone") || below.contains("dropper") || below.contains("hopper");
                let k = r.below(100);
                let s = if k < 45 {
                    if r.chance(0.55) {
                        format!("minecraft:dropper[facing={},triggered=false]", anydir(r))
                    } else {
                        format!("minecraft:hopper[enabled=true,facing={}]", r.pick(&["down", "north", "east", "south", "west"]))
                    }
                } else if k < 62 && solid_below {
                    format!("minecraft:comparator[facing={},mode={},powered=false]", hdir(r), if r.chance(0.5) { "subtract" } else { "compare" })
                } else if k < 70 && below.contains("stone") {
                    inputs.push(p);
                    format!("minecraft:lever[face=floor,facing={},powered=false]", hdir(r))
                } else if k < 76 {
                    "minecraft:stone".into()
                } else if k < 80 {
                    "minecraft:glass".into()
                } else {
                    continue;
                };
                grid.insert(p, s);
                order.push(p);
            }
        }
    }
    // containers face a neighboring container
    let dirs: [(&str, Pos); 6] = [("north", [0, 0, -1]), ("south", [0, 0, 1]), ("west", [-1, 0, 0]), ("east", [1, 0, 0]), ("up", [0, 1, 0]), ("down", [0, -1, 0])];
    for p in &order {
        let s = grid[p].clone();
        let is_cont = |s: &str| s.contains("minecraft:dropper") || s.contains("minecraft:hopper");
        if !is_cont(&s) || !r.chance(0.9) {
            continue;
        }
        let hopper = s.contains("minecraft:hopper");
        let targets: Vec<&str> = dirs
            .iter()
            .filter(|(n, d)| !(hopper && *n == "up") && grid.get(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]]).is_some_and(|t| is_cont(t)))
            .map(|(n, _)| *n)
            .collect();
        if let Some(f) = (!targets.is_empty()).then(|| *r.pick(&targets)) {
            let ns = if hopper { format!("minecraft:hopper[enabled=true,facing={f}]") } else { format!("minecraft:dropper[facing={f},triggered=false]") };
            grid.insert(*p, ns);
        }
    }
    let blocks: Vec<(Pos, String)> = order.iter().map(|p| (*p, grid[p].clone())).collect();
    let mut steps = vec![Step::Run(r.range(10, 30) as u32)];
    for _ in 0..r.range(2, 6) {
        if !inputs.is_empty() {
            steps.push(Step::Use(*r.pick(&inputs)));
        }
        steps.push(Step::Run(r.range(1, 30) as u32));
    }
    steps.push(Step::Run(24));
    let update_place = r.chance(0.3);
    let items = container_items(r, &order, &grid);
    Case { blocks, steps, lo: [lo[0] - 1, lo[1] - 2, lo[2] - 1], hi: [hi[0] + 1, hi[1] + 1, hi[2] + 1], update_place, items }
}

/// Random contents for the containers of a case: droppers get one stack (a dropper with
/// several picks a random slot), hoppers up to two.
fn container_items(r: &mut Rng, order: &[Pos], grid: &HashMap<Pos, String>) -> Vec<(Pos, usize, String, u8)> {
    let mut out = Vec::new();
    // one item type per case: a dropper holding two kinds would pick a random slot
    let (item, max) = *r.pick(&redsim::redstone::container::ITEMS);
    for p in order {
        let s = &grid[p];
        let dropper = s.contains("minecraft:dropper");
        let hopper = s.contains("minecraft:hopper");
        if !(dropper || hopper) || r.chance(0.3) {
            continue;
        }
        // a dropper that does not face a container would spit its item into the world
        if dropper {
            let d: Pos = if s.contains("facing=north") {
                [0, 0, -1]
            } else if s.contains("facing=south") {
                [0, 0, 1]
            } else if s.contains("facing=west") {
                [-1, 0, 0]
            } else if s.contains("facing=east") {
                [1, 0, 0]
            } else if s.contains("facing=up") {
                [0, 1, 0]
            } else {
                [0, -1, 0]
            };
            let t = grid.get(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
            if !t.is_some_and(|t| t.contains("minecraft:dropper") || t.contains("minecraft:hopper")) {
                continue;
            }
        }
        let stacks = if hopper { r.range(1, 2) } else { 1 };
        let mut used = Vec::new();
        for _ in 0..stacks {
            let slot = if dropper { 0 } else { r.below(5) as usize };
            if used.contains(&slot) {
                continue;
            }
            used.push(slot);
            let count = if r.chance(0.6) { 1 } else { r.range(1, (max as i32).min(4)) as u8 };
            out.push((*p, slot, item.to_string(), count));
        }
    }
    out
}

// ---------------------------------------------------------------------- driver

pub struct Outcome {
    pub diff: Option<(usize, Vec<(Pos, String, String)>)>,
    pub sim_errors: Vec<String>,
    /// The first frame with a model note (None: no note).
    pub note_frame: Option<usize>,
}

impl Outcome {
    /// A difference the notes do not excuse: no note, or one that came after the difference.
    pub fn real_diff(&self) -> bool {
        match (&self.diff, self.note_frame) {
            (Some(_), None) => true,
            (Some((f, _)), Some(n)) => *f < n,
            _ => false,
        }
    }
}

pub fn check(w: &mut Worker, sb: u32, c: &Case) -> Result<Outcome> {
    let game = run_bridge(w, sb, c)?;
    let (sim, sim_errors, note_frame) = run_sim_noted(c)?;
    Ok(Outcome { diff: first_diff(c, &sim, &game), sim_errors, note_frame })
}

/// Greedy shrink: drop blocks, then steps, while the case still mismatches (without model
/// notes, when the original had none: a smaller case must not wander out of scope).
pub fn shrink(w: &mut Worker, sb: u32, mut c: Case) -> Result<Case> {
    let clean = check(w, sb, &c)?.sim_errors.is_empty();
    let still = |w: &mut Worker, t: &Case| -> Result<bool> {
        let o = check(w, sb, t)?;
        Ok(o.diff.is_some() && (!clean || o.sim_errors.is_empty()))
    };
    loop {
        let mut better = false;
        let mut i = c.blocks.len();
        while i > 0 {
            i -= 1;
            let mut t = c.clone();
            let (p, _) = t.blocks.remove(i);
            t.steps.retain(|s| *s != Step::Use(p));
            t.items.retain(|(q, ..)| *q != p);
            if still(w, &t)? {
                c = t;
                better = true;
            }
        }
        let mut j = c.steps.len();
        while j > 0 {
            j -= 1;
            let mut t = c.clone();
            match t.steps[j] {
                Step::Run(n) if n > 1 => t.steps[j] = Step::Run(n / 2),
                _ => {
                    t.steps.remove(j);
                }
            }
            if still(w, &t)? {
                c = t;
                better = true;
            }
        }
        if !better {
            break;
        }
    }
    // tighten the box to the remaining blocks + 1
    if !c.blocks.is_empty() {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for (p, _) in &c.blocks {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k] - 1);
                hi[k] = hi[k].max(p[k] + 1);
            }
        }
        let mut t = c.clone();
        t.lo = lo;
        t.hi = hi;
        if check(w, sb, &t)?.diff.is_some() {
            c = t;
        }
    }
    Ok(c)
}

pub struct FuzzOptions {
    pub n: usize,
    pub seed: u64,
    pub size: [i32; 3],
    pub density: f64,
    pub palette: Vec<(String, u32)>,
    pub out: std::path::PathBuf,
    pub shrink: bool,
    pub max_report: usize,
    /// Case generator: `random` (palette circuits) or `containers` ([`container_case`]).
    pub generator: String,
}

/// Run `n` random cases; returns (cases, mismatches).
pub fn fuzz(pool: &Pool, o: &FuzzOptions, log: &dyn Fn(&str)) -> Result<(usize, usize)> {
    std::fs::create_dir_all(&o.out)?;
    let sandboxes: Vec<Mutex<Option<u32>>> = (0..pool.len()).map(|_| Mutex::new(None)).collect();
    let sb_for = |w: &mut Worker, wi: usize| -> Result<u32> {
        let mut s = sandboxes[wi].lock().unwrap();
        if let Some(sb) = *s {
            return Ok(sb);
        }
        let sb = w.create(&SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
        *s = Some(sb);
        Ok(sb)
    };
    let mut rng = Rng::new(o.seed);
    let cases: Vec<(usize, Case)> = (0..o.n)
        .map(|i| {
            (
                i,
                match o.generator.as_str() {
                    "containers" => container_case(&mut rng, o.size),
                    // water (redsim M1): a random circuit in a stone basin (floor and four
                    // walls), so no water leaves the compared box
                    "basin" => basin_case(&mut rng, &o.palette, o.size, o.density),
                    _ => random_case(&mut rng, &o.palette, o.size, o.density),
                },
            )
        })
        .collect();
    if let Some((_, c)) = cases.first() {
        std::fs::write(o.out.join(format!("sample-{}.txt", o.seed)), c.to_text())?;
    }
    let t0 = std::time::Instant::now();
    let results = pool.map(cases, |w, wi, (i, c)| {
        let sb = sb_for(w, wi)?;
        let out = check(w, sb, &c)?;
        Ok((i, c, out))
    });
    let mut bad: Vec<(usize, Case, usize, Vec<(Pos, String, String)>)> = Vec::new();
    let mut errors = 0;
    let mut sim_err_cases = 0;
    let mut noted_bad = 0;
    let mut note_kinds: HashMap<String, usize> = HashMap::new();
    for r in results {
        match r {
            Ok((i, c, out)) => {
                if !out.sim_errors.is_empty() {
                    sim_err_cases += 1;
                    for e in &out.sim_errors {
                        let k: String = e.split(|ch: char| ch == '(' || ch == ':').next().unwrap_or("").trim().to_string();
                        *note_kinds.entry(k).or_insert(0usize) += 1;
                    }
                }
                if out.diff.is_some() {
                    // a case the model flagged as out of scope (random dropper slot, item
                    // entities, an explosion...) is not a model error from the note's frame on
                    if out.real_diff() {
                        let (f, d) = out.diff.unwrap();
                        bad.push((i, c, f, d));
                    } else {
                        noted_bad += 1;
                    }
                }
            }
            Err(e) => {
                if errors < 3 {
                    log(&format!("error: {e:#}"));
                }
                errors += 1;
            }
        }
    }
    log(&format!(
        "{} cases in {:.1}s: {} mismatches ({:.3}%), {errors} errors, {sim_err_cases} with model notes ({noted_bad} of them differ: out of scope)",
        o.n,
        t0.elapsed().as_secs_f64(),
        bad.len(),
        100.0 * bad.len() as f64 / o.n.max(1) as f64
    ));
    let mut nk: Vec<_> = note_kinds.into_iter().collect();
    nk.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, n) in nk.iter().take(6) {
        log(&format!("  note x{n}: {k}"));
    }
    // group by the first mismatching game state's block name
    let mut by: HashMap<String, usize> = HashMap::new();
    for (_, _, _, d) in &bad {
        let k = d.first().map(|(_, _, g)| g.split('[').next().unwrap_or("").to_string()).unwrap_or_default();
        *by.entry(k).or_default() += 1;
    }
    let mut byv: Vec<_> = by.into_iter().collect();
    byv.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, n) in &byv {
        log(&format!("  {n:6} first mismatch at a {k}"));
    }
    if o.shrink && !bad.is_empty() {
        let mut w = pool.worker(0);
        let sb = sb_for(&mut w, 0)?;
        for (i, c, f, d) in bad.iter().take(o.max_report) {
            let small = shrink(&mut w, sb, c.clone())?;
            let out = check(&mut w, sb, &small)?;
            let path = o.out.join(format!("case-{}-{i}.txt", o.seed));
            let mut text = format!("# redsim-fuzz seed {} case {i}: first mismatch {} ({} cells)\n", o.seed, frame_label(c, *f), d.len());
            if let Some((f2, d2)) = &out.diff {
                let _ = writeln!(text, "# shrunk: first mismatch {}", frame_label(&small, *f2));
                for (p, s, g) in d2.iter().take(6) {
                    let _ = writeln!(text, "#   {} {} {}: redsim {s} / game {g}", p[0], p[1], p[2]);
                }
            }
            text.push_str(&small.to_text());
            std::fs::write(&path, &text)?;
            log(&format!("  shrunk case {i}: {} blocks -> {}", c.blocks.len(), path.display()));
        }
    }
    Ok((o.n, bad.len()))
}

/// Replay one case file on both sides and print every differing frame (up to `max`).
pub fn replay(w: &mut Worker, path: &Path, max: usize) -> Result<bool> {
    let c = Case::parse(&std::fs::read_to_string(path)?)?;
    let sb = w.create(&SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
    let game = run_bridge(w, sb, &c)?;
    let (sim, errs) = run_sim(&c)?;
    for e in &errs {
        println!("model note: {e}");
    }
    let cells = c.cells();
    let mut shown = 0;
    for (i, (a, b)) in sim.iter().zip(game.iter()).enumerate() {
        if a != b {
            println!("frame {i} ({}):", frame_label(&c, i));
            for (p, (x, y)) in cells.iter().zip(a.iter().zip(b.iter())) {
                if x != y {
                    println!("  {} {} {}: redsim {x}\n            game   {y}", p[0], p[1], p[2]);
                }
            }
            shown += 1;
            if shown >= max {
                break;
            }
        }
    }
    if shown == 0 {
        let changes = game.windows(2).filter(|f| f[0] != f[1]).count();
        let cells_set = game.first().map(|f| f.iter().filter(|s| *s != "minecraft:air").count()).unwrap_or(0);
        println!("identical ({} frames, {changes} frames with changes, {cells_set} non-air cells after placement)", sim.len());
    }
    Ok(shown == 0)
}

/// The fixture text of a case: the case plus the game's trace (frame 0 in full, then the cells
/// that changed per frame), for `crates/redsim/tests/fixtures.rs`.
pub fn fixture(w: &mut Worker, c: &Case, note: &str) -> Result<String> {
    let sb = w.create(&SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
    let game = run_bridge(w, sb, c)?;
    let cells = c.cells();
    let mut s = String::new();
    for l in note.lines() {
        let _ = writeln!(s, "# {l}");
    }
    s.push_str(&c.to_text());
    let air = "minecraft:air".to_string();
    let mut prev: Vec<String> = vec![air; cells.len()];
    for (i, f) in game.iter().enumerate() {
        let _ = writeln!(s, "frame {i}");
        for (k, st) in f.iter().enumerate() {
            if *st != prev[k] {
                let p = cells[k];
                let _ = writeln!(s, "cell {} {} {} {st}", p[0], p[1], p[2]);
            }
        }
        prev = f.clone();
    }
    Ok(s)
}

// ---------------------------------------------------------------------- equivalence classes

/// A state's class representative (as a string), or the state itself when it is not a plain
/// block of the class universe.
pub fn class_canon(s: &str) -> String {
    match Block::parse(s) {
        Ok(b) => match redsim::classes::representative(&b) {
            Some(r) => r.to_string(),
            None => b.to_string(),
        },
        Err(_) => s.to_string(),
    }
}

/// The same case with every plain block replaced by a random member of its class.
pub fn class_variant(r: &mut Rng, c: &Case, classes: &[redsim::classes::Class]) -> Case {
    let mut v = c.clone();
    for (_, s) in v.blocks.iter_mut() {
        if let Ok(b) = Block::parse(s)
            && let Some(cl) = classes.iter().find(|cl| cl.members.contains(&b))
        {
            *s = r.pick(&cl.members).to_string();
        }
    }
    v
}

/// Do a case and its variant differ in the game modulo classes?
pub fn class_differs(w: &mut Worker, sb: u32, a: &Case, b: &Case) -> Result<bool> {
    let fa = run_bridge(w, sb, a)?;
    let fb = run_bridge(w, sb, b)?;
    let canon = |fs: &[Frame]| -> Vec<Frame> { fs.iter().map(|f| f.iter().map(|s| class_canon(s)).collect()).collect() };
    Ok(canon(&fa) != canon(&fb))
}

/// Shrink a class violation: revert swaps one by one, then drop blocks (from both), while the
/// pair still differs; returns the minimal pair.
pub fn class_shrink(w: &mut Worker, sb: u32, mut a: Case, mut b: Case) -> Result<(Case, Case)> {
    loop {
        let mut better = false;
        for i in 0..a.blocks.len() {
            if a.blocks[i].1 != b.blocks[i].1 {
                let mut t = b.clone();
                t.blocks[i].1 = a.blocks[i].1.clone();
                if class_differs(w, sb, &a, &t)? {
                    b = t;
                    better = true;
                }
            }
        }
        let mut i = a.blocks.len();
        while i > 0 {
            i -= 1;
            let (mut ta, mut tb) = (a.clone(), b.clone());
            let (p, _) = ta.blocks.remove(i);
            tb.blocks.remove(i);
            ta.steps.retain(|s| *s != Step::Use(p));
            tb.steps.retain(|s| *s != Step::Use(p));
            if class_differs(w, sb, &ta, &tb)? {
                a = ta;
                b = tb;
                better = true;
            }
        }
        if !better {
            return Ok((a, b));
        }
    }
}

/// Game-only proof of the classes: run a case and its class variant IN THE GAME and compare
/// every frame after mapping every state to its class representative. Returns the number of
/// differing pairs (and saves them).
pub fn classfuzz(pool: &Pool, n: usize, seed: u64, pal: &[(String, u32)], size: [i32; 3], density: f64, out: &Path, log: &dyn Fn(&str)) -> Result<usize> {
    std::fs::create_dir_all(out)?;
    let classes = redsim::classes::classes();
    let mut rng = Rng::new(seed);
    let pairs: Vec<(usize, Case, Case)> = (0..n)
        .map(|i| {
            let mut c = random_case(&mut rng, pal, size, density);
            // no mid-run edits: replacing a member by another of its class is itself a block
            // change an observer sees; the classes are about what a design is built from
            c.steps.retain(|s| !matches!(s, Step::Set(..)));
            let v = class_variant(&mut rng, &c, &classes);
            (i, c, v)
        })
        .collect();
    let sbs: Vec<Mutex<Option<u32>>> = (0..pool.len()).map(|_| Mutex::new(None)).collect();
    let t0 = std::time::Instant::now();
    let res = pool.map(pairs, |w, wi, (i, c, v)| {
        let mut s = sbs[wi].lock().unwrap();
        let sb = match *s {
            Some(sb) => sb,
            None => {
                let sb = w.create(&SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
                *s = Some(sb);
                sb
            }
        };
        let a = run_bridge(w, sb, &c)?;
        let b = run_bridge(w, sb, &v)?;
        let canon = |fs: &[Frame]| -> Vec<Frame> { fs.iter().map(|f| f.iter().map(|s| class_canon(s)).collect()).collect() };
        let same = canon(&a) == canon(&b);
        // and redsim on the variant, as a plain differential
        let (sim, notes) = run_sim(&v)?;
        let model_ok = notes.is_empty() && first_diff(&v, &sim, &b).is_none();
        Ok((i, c, v, same, model_ok))
    });
    let (mut bad, mut model_bad, mut errors) = (0, 0, 0);
    for r in res {
        match r {
            Ok((i, c, v, same, model_ok)) => {
                if !same {
                    bad += 1;
                    if bad <= 6 {
                        std::fs::write(out.join(format!("class-{seed}-{i}-a.txt")), c.to_text())?;
                        std::fs::write(out.join(format!("class-{seed}-{i}-b.txt")), v.to_text())?;
                    }
                }
                if !model_ok {
                    model_bad += 1;
                    if model_bad <= 12 {
                        std::fs::write(out.join(format!("model-{seed}-{i}.txt")), v.to_text())?;
                    }
                }
            }
            Err(e) => {
                if errors < 3 {
                    log(&format!("error: {e:#}"));
                }
                errors += 1;
            }
        }
    }
    log(&format!("{n} class swaps in {:.1}s: {bad} traces differ modulo classes, {model_bad} redsim mismatches on the variants, {errors} errors", t0.elapsed().as_secs_f64()));
    Ok(bad)
}
