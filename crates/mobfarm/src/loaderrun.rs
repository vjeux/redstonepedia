//! (loaderlay, Nov 13 2026; farmopt 21:44: "run them in redsim with arrivals and golem arrivals printed
//! per pad") `mctas mobfarm loader-layout ... --redsim TICKS`: the layout's portal skeleton run in redsim
//! (`World::nether`, `entity::portal`: crossings both ways, the closest-portal search, the arrival
//! placement, the Nether's PORTAL ticket): the loader's cells whole, each copy's portal cells with their
//! frame obsidian (the copies' other cells enter no portal search and no item's flight), the loader's
//! block entities. A portal block is a terrain block, as the farm judge parses it
//! (`circuit::studies::mobfarm::farm_block`). The loop's items are what the loader's droppers hold (a
//! clock's own items sit in its hoppers). A side whose clock is an observer pair starts as the build
//! starts it: its observer farther from the portal is taken out and put back (O1 sees it), and the cell
//! is read back; a side with no observer pair needs no start. Launches are read back on each side: an
//! item entity no crossing made. At `--golem-at` an iron golem is put in each pad at three places of its
//! bottom row (the second cell from each end and the middle: its 1.4-wide box inside the portal). Every
//! crossing is printed by the portal it came out of. A world open (`liveness::restart`, save order, 2
//! idle ticks) at `--world-open-at` (default half the run) puts the gap bar on a world-open row
//! (ironspec 01:50b): its evidence printed (chunks unloaded and loaded again, the scheduled ticks out
//! and back), the reload model keeping the Nether part in memory (unmodelled there, said in the row),
//! and how the row differs from the reference timing row (ironspec 02:33 (2)).
//!
//! The bars (each FAILS by name; G1 06:37 (1)-(3)):
//! - every item arrival lands at the loader's own portal on each side, and each side's worst gap
//!   between arrivals stays within the PORTAL ticket's 300 gt (`TicketType.PORTAL`), the world open
//!   included;
//! - both sides launch;
//! - the loop's items are all there at every tick (less any item redsim itself refused to spawn while
//!   `Run::start` settled the Nether part: a named model limit, counted by name);
//! - golems were placed (none placed FAILS), and every golem crosses to its own copy's twin;
//! - the world open happened inside the run;
//! - no crossing's copy goes missing;
//! - no world error other than the two model limits NOTES names for loader-clock-x8 (redsim's light
//!   model lacking the extended sticky piston and the piston head; a spit while `Run::start` settles
//!   the Nether part). A failed bar always FAILS; a run whose bars all pass with only those named limits
//!   met is NOT MODELED (it judges nothing); any other world error FAILS by name.
//!
//! The bars' failing controls (`Control`, `--redsim-control`): one planted change each, read back, a
//! no-op refused by name.

use std::collections::{BTreeMap, BTreeSet};

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, NetherSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

use crate::loaderlayout::{built, portals, Laid, Portal, C};

/// The run's options.
#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub ticks: i64,
    pub seed: i64,
    pub golem_at: i64,
    /// (ironspec 01:50b, the timing rule: every timing bar on the worst-jitter + world-open row) the
    /// tick of the world open the gap bar spans: `liveness::restart` with `Restart::load` (save order,
    /// 2 idle ticks: the larger of a world open's 1 or 2).
    pub world_open: i64,
    /// A failing control of the bars (G1 06:37 (3)).
    pub control: Option<Control>,
}

/// A failing control of the runner's bars (G1 Nov 14 06:37 (3)): one planted change on the run, read
/// back, a no-op refused by name; each drives one bar to FAIL through [`run`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Both observers of the overworld clock taken out (raw: no update) a quarter into the run, read back
    /// as air: the overworld side stops launching, so the arrivals stop on both sides (the gap bars).
    StallOverworldClock,
    /// Both observers of the Nether clock taken out (raw) before the run, read back: the Nether side never
    /// launches (the launches bar).
    StopNetherClock,
    /// One loop item in flight killed (`entity::kill`) from a quarter into the run, read back: the items
    /// bar.
    StealItem,
    /// A lit portal planted (raw, framed, the loader portal's axis) on the return's exit blocks one plane
    /// off the loader's overworld portal, read back: the returns whose exit block is there arrive in it
    /// (the arrivals bar's "elsewhere").
    DivertReturn,
}

impl Control {
    pub const NAMES: [&'static str; 4] = ["stall-overworld-clock", "stop-nether-clock", "steal-item", "divert-return"];

    pub fn parse(s: &str) -> Option<Control> {
        match s.trim() {
            "stall-overworld-clock" => Some(Control::StallOverworldClock),
            "stop-nether-clock" => Some(Control::StopNetherClock),
            "steal-item" => Some(Control::StealItem),
            "divert-return" => Some(Control::DivertReturn),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Control::StallOverworldClock => "stall-overworld-clock",
            Control::StopNetherClock => "stop-nether-clock",
            Control::StealItem => "steal-item",
            Control::DivertReturn => "divert-return",
        }
    }
}

fn bp(c: C) -> BlockPos {
    BlockPos::new(c[0], c[1], c[2])
}

/// The portal (of `ps`, one dimension) an arrival stands in: the one whose cells hold the entity's
/// block, else the first whose cell box grown 1 across its plane holds it (an arrival is placed in its
/// portal's cells, centred on the plane, and may be nudged out of a colliding cell).
fn portal_of(ps: &[&Portal], pos: [f64; 3]) -> Option<String> {
    let b = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
    ps.iter()
        .find(|p| p.cells.contains(&b))
        .or_else(|| {
            ps.iter().find(|p| {
                let (lo, hi) = p.lo_hi();
                let g = if p.axis == 'x' { [0, 1, 1] } else { [1, 1, 0] };
                (0..3).all(|k| b[k] >= lo[k] - g[k] && b[k] <= hi[k] + g[k])
            })
        })
        .map(|p| p.name.clone())
}

/// The observers of `cells` that face another observer facing back: (the pair's two cells).
fn observer_pairs(cells: &BTreeMap<C, String>) -> Vec<(C, C)> {
    let step = |s: &str| -> Option<C> {
        let f = s.split_once("facing=")?.1.split([',', ']']).next()?;
        match f {
            "north" => Some([0, 0, -1]),
            "south" => Some([0, 0, 1]),
            "west" => Some([-1, 0, 0]),
            "east" => Some([1, 0, 0]),
            "up" => Some([0, 1, 0]),
            "down" => Some([0, -1, 0]),
            _ => None,
        }
    };
    let mut out = Vec::new();
    for (p, s) in cells.iter().filter(|(_, s)| s.starts_with("minecraft:observer")) {
        let Some(d) = step(s) else { continue };
        let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        if let Some(t) = cells.get(&q).filter(|t| t.starts_with("minecraft:observer"))
            && step(t) == Some([-d[0], -d[1], -d[2]])
            && p < &q
        {
            out.push((*p, q));
        }
    }
    out
}

/// The loop's items in a world: the kinds the loader's droppers held at the start, in containers and as
/// item entities.
fn loop_items_in(w: &redsim::world::World, kinds: &BTreeSet<String>) -> u32 {
    use redsim::redstone::container as rc;
    let ids: Vec<u8> = kinds.iter().filter_map(|k| rc::item_id(k)).collect();
    let inv: u32 = w.containers.map.values().map(|c| c.slots.iter().flatten().filter(|s| ids.contains(&s.item)).map(|s| s.count as u32).sum::<u32>()).sum();
    let ents: u32 = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .map(|e| match &e.kind {
            EntityKind::Item(s) if kinds.contains(s.item.trim_start_matches("minecraft:")) => s.count as u32,
            _ => 0,
        })
        .sum();
    inv + ents
}

/// The items an SNBT container line holds: (item id without the namespace, count). Each item's count
/// is read inside its own compound (the innermost braces around its id, depth-counted both ways, at
/// that compound's top level), never from the next item's (G1 06:37 N6); no count is 1.
fn snbt_items(s: &str) -> Vec<(String, u32)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(off) = s[from..].find("id:\"") {
        let i = from + off;
        let after = &s[i + 4..];
        let Some(end) = after.find('"') else { break };
        let id = after[..end].trim_start_matches("minecraft:").to_string();
        let (mut depth, mut open) = (0i32, 0usize);
        for k in (0..i).rev() {
            match b[k] {
                b'}' => depth += 1,
                b'{' if depth == 0 => {
                    open = k + 1;
                    break;
                }
                b'{' => depth -= 1,
                _ => {}
            }
        }
        let (mut depth, mut close) = (0i32, b.len());
        for (k, ch) in b.iter().enumerate().skip(i) {
            match ch {
                b'{' => depth += 1,
                b'}' if depth == 0 => {
                    close = k;
                    break;
                }
                b'}' => depth -= 1,
                _ => {}
            }
        }
        let body = &s[open..close];
        let mut count = 1;
        let mut depth = 0i32;
        for (k, ch) in body.bytes().enumerate() {
            match ch {
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                _ if depth == 0 && body[k..].starts_with("count:") && (k == 0 || matches!(body.as_bytes()[k - 1], b',' | b'{')) => {
                    count = body[k + 6..].split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse().ok()).unwrap_or(1);
                    break;
                }
                _ => {}
            }
        }
        out.push((id, count));
        from = i + 4 + end;
    }
    out
}

/// The loop's items: what the loader's droppers hold in its `nbt:` / `nether nbt:` lines (a clock's own
/// items, in its hoppers, are not the loop's): (kinds, total).
fn loop_items(l: &Laid) -> (BTreeSet<String>, u32) {
    let mut kinds = BTreeSet::new();
    let mut n = 0;
    for (cells, bes) in [(&l.blocks, &l.block_entities), (&l.nether, &l.nether_block_entities)] {
        for (c, s) in bes {
            if cells.get(c).is_some_and(|b| b.starts_with("minecraft:dropper")) {
                for (id, k) in snbt_items(s) {
                    kinds.insert(id);
                    n += k;
                }
            }
        }
    }
    (kinds, n)
}

/// The skeleton's spec.
fn spec_of(parts: &[Laid]) -> Result<FarmSpec, String> {
    let ps = portals(parts, &[])?;
    let (ow, _) = built(parts, false, &[])?;
    let (nw, _) = built(parts, true, &[])?;
    let li = parts.iter().position(|p| p.name == "loader").ok_or("no loader part")?;
    let mut o: BTreeMap<C, String> = parts[li].blocks.clone();
    let mut n: BTreeMap<C, String> = parts[li].nether.clone();
    for p in ps.iter().filter(|p| p.part != li) {
        let m = if p.nether { &nw } else { &ow };
        let dst = if p.nether { &mut n } else { &mut o };
        for c in p.cells.iter().copied().chain(p.frame(m)) {
            if let Some((s, _)) = m.get(&c) {
                dst.insert(c, s.clone());
            }
        }
    }
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for c in o.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k] - 4);
            hi[k] = hi[k].max(c[k] + 4);
        }
    }
    // (the farm judge's rule, circuit::studies::mobfarm::farm_block: a portal block is a terrain block)
    let block = |s: &str| -> Result<Block, String> { if s.starts_with("minecraft:nether_portal[") { redsim::terrain::land::block_of(s) } else { Block::parse(s) } };
    let parse = |m: &BTreeMap<C, String>| -> Result<Vec<(BlockPos, Block)>, String> { m.iter().map(|(c, s)| Ok((bp(*c), block(s)?))).collect() };
    let spec = FarmSpec {
        blocks: parse(&o)?,
        entities: Vec::new(),
        // (the runner's output chest: a corner of the box, away from every cell)
        output: BlockPos::new(lo[0], lo[1], lo[2]),
        lo: bp(lo),
        hi: bp(hi),
        block_entities: parts[li].block_entities.iter().map(|(c, s)| (bp(*c), s.clone())).collect(),
        rock: false,
        nether: Some(NetherSpec { anchor: BlockPos::new(0, 0, 0), blocks: parse(&n)?, output: None, block_entities: parts[li].nether_block_entities.iter().map(|(c, s)| (bp(*c), s.clone())).collect() }),
        land: false,
    };
    Ok(spec)
}

/// (G1 06:37 (1)) The two redsim model limits NOTES names for loader-clock-x8, the only world errors a
/// run may meet and still be NOT MODELED rather than FAIL: redsim's light model lacking the extended
/// sticky piston and the piston head; and a dropper's spit refused while `Run::start` settled the Nether
/// part, before any caller can seed that world's drop random (`settle_spits`: the spits recorded right
/// after the start, each matched once).
fn model_limit(e: &str, settle_spits: &mut Vec<String>) -> Option<&'static str> {
    if e.contains("the light behaviour of minecraft:sticky_piston[extended=true") || e.contains("the light behaviour of minecraft:piston_head[") {
        return Some("redsim's light model lacks the extended sticky piston and the piston head");
    }
    let bare = e.strip_prefix("the Nether: ").unwrap_or(e);
    if let Some(k) = settle_spits.iter().position(|s| s == bare) {
        settle_spits.remove(k);
        return Some("a dropper's spit refused while Run::start settled the Nether part, before its drop random could be seeded");
    }
    None
}

/// The world of side `wi` (0 the overworld, 1 the Nether).
fn side(run: &mut Run, wi: usize) -> Result<&mut redsim::world::World, String> {
    if wi == 0 { Ok(&mut run.world) } else { run.world.nether.as_deref_mut().ok_or_else(|| "no Nether world".to_string()) }
}

/// One seed's run; (text, verdict).
pub fn run(parts: &[Laid], o: &Opts) -> Result<(String, Verdict), String> {
    let ps = portals(parts, &[])?;
    let li = parts.iter().position(|p| p.name == "loader").ok_or("no loader part")?;
    let (lo_ow, lo_n) = (ps.iter().find(|p| p.part == li && !p.nether).ok_or("no loader overworld portal")?, ps.iter().find(|p| p.part == li && p.nether).ok_or("no loader Nether portal")?);
    let n_ps: Vec<&Portal> = ps.iter().filter(|p| p.nether).collect();
    let spec = spec_of(parts)?;
    let (kinds, items0) = loop_items(&parts[li]);
    if items0 == 0 {
        return Err("the loader's droppers hold no item (the loop's items are what its droppers hold): nothing to follow, refused".into());
    }
    let mut s = format!(
        "redsim run (seed {}, {} gt; bench, tool unreviewed): the portal skeleton ({} overworld cells: the loader's and each copy's portal cells with their frame obsidian; {} Nether cells); the loop's items (its droppers'): {items0} ({}){}\n",
        o.seed,
        o.ticks,
        spec.blocks.len(),
        spec.nether.as_ref().map_or(0, |n| n.blocks.len()),
        kinds.iter().cloned().collect::<Vec<_>>().join(", "),
        o.control.map_or(String::new(), |c| format!("; CONTROL {} (a failing control: not evidence for the layout)", c.name()))
    );
    let ro = RunOpts::steady(o.seed, (o.ticks + 200) as u32, WalkMode::Random);
    let mut run = Run::start(&spec, &ro)?;
    // (the spits redsim refused while Run::start settled the Nether part: the second named model limit)
    let settle_spits: Vec<String> = run.world.nether.as_deref().map(|n| n.errors.iter().filter(|e| e.contains("spits an item into the world")).cloned().collect()).unwrap_or_default();
    // (each world's level random and drop seed, set before the overworld's settle: a clock that starts as
    // it is built (a hopper clock) fires its droppers while the layout settles, and a dropper's spit draws
    // from them)
    for wi in 0..2 {
        let w = side(&mut run, wi)?;
        let k = if wi == 0 { 0 } else { 0x4e45 };
        w.level_random = Some(redsim::rng::LegacyRandom::new(o.seed ^ k));
        w.drop_seed = Some(o.seed ^ k);
    }
    while run.settle_tick() {}
    // (the clocks' start, as the build starts an observer pair: each side's observer farther from its
    // portal taken out and put back, read back; a side with no observer pair (a hopper clock) needs none)
    let air = Block::parse("minecraft:air")?;
    let raw = redsim::world::flags::RAW;
    let mut stall_pair: Option<(C, C)> = None;
    for (wi, cells, portal) in [(0usize, &parts[li].blocks, lo_ow), (1usize, &parts[li].nether, lo_n)] {
        let side_name = if wi == 0 { "overworld" } else { "Nether" };
        let targeted = matches!((o.control, wi), (Some(Control::StopNetherClock), 1) | (Some(Control::StallOverworldClock), 0));
        let pairs = observer_pairs(cells);
        let (a, b) = match pairs.as_slice() {
            [] if targeted => return Err(format!("control {}: the {side_name} clock has no observer pair to take out: a no-op, refused", o.control.map_or("", |c| c.name()))),
            [] => {
                s.push_str(&format!("  the {side_name} clock: no observer pair (nothing taken out: a clock that starts as it is built)\n"));
                continue;
            }
            [(a, b)] => (*a, *b),
            _ => return Err(format!("the loader's {side_name} side: {} observer pairs (its clock: at most one)", pairs.len())),
        };
        if o.control == Some(Control::StopNetherClock) && wi == 1 {
            let w = side(&mut run, 1)?;
            for c in [a, b] {
                w.set_block(bp(c), air.clone(), raw);
            }
            let stopped = [a, b].iter().all(|c| w.get(bp(*c)).is_air());
            s.push_str(&format!(
                "  CONTROL stop-nether-clock: the Nether clock's observers ({},{},{}) and ({},{},{}) taken out (raw, no update); read back: {}\n",
                a[0], a[1], a[2], b[0], b[1], b[2],
                if stopped { "air, air" } else { "NOT air: refused" }
            ));
            if !stopped {
                return Err("control stop-nether-clock: the observers did not read back as air".into());
            }
            continue;
        }
        if o.control == Some(Control::StallOverworldClock) && wi == 0 {
            stall_pair = Some((a, b));
        }
        let plane = |c: &C| if portal.axis == 'x' { (c[2] - portal.cells[0][2]).abs() } else { (c[0] - portal.cells[0][0]).abs() };
        let o2 = if plane(&a) >= plane(&b) { a } else { b };
        let w = side(&mut run, wi)?;
        let st = w.get(bp(o2)).clone();
        w.set_block(bp(o2), air.clone(), redsim::world::flags::ALL);
        w.set_block(bp(o2), st.clone(), redsim::world::flags::ALL);
        let back = w.get(bp(o2)).clone();
        s.push_str(&format!("  the {side_name} clock's start: O2 ({},{},{}) taken out and put back; read back: {back}{}\n", o2[0], o2[1], o2[2], if back == st { "" } else { " (CHANGED: refused)" }));
        if back != st {
            return Err("the clock's O2 did not read back".into());
        }
    }
    // (the divert control's planted portal: on the return's exit blocks (one move, the code's crossing)
    // that lie one plane off the loader's overworld portal, 3 high and at least 2 wide, framed)
    let mut plant: Option<Portal> = None;
    if o.control == Some(Control::DivertReturn) {
        let fire = crate::loaderlayout::firing_droppers(&parts[li].nether, &lo_n.cells)?;
        let [(p, d)] = fire[..] else { return Err(format!("control divert-return: {} droppers fire into the loader's Nether portal (one expected)", fire.len())) };
        let (l1, h1) = crate::loaderlayout::crossing_box(p, d, 1);
        let exits = crate::loaderlayout::exits_of_box(l1, h1, 8.0);
        let axis = lo_ow.axis;
        let across = |c: &C| if axis == 'x' { c[2] } else { c[0] };
        let along = |c: &C| if axis == 'x' { c[0] } else { c[2] };
        let plane = across(&lo_ow.cells[0]);
        let pick = |side_off: i32| -> Vec<C> { exits.iter().filter(|c| across(c) == plane + side_off).copied().collect() };
        let front = if pick(-1).is_empty() { pick(1) } else { pick(-1) };
        let Some(f0) = front.first() else { return Err("control divert-return: no return exit block lies one plane off the loader's portal: a no-op, refused".into()) };
        let pl = across(f0);
        let a0 = front.iter().map(along).min().unwrap_or(0);
        let a1 = front.iter().map(along).max().unwrap_or(0).max(a0 + 1);
        let y0 = front.iter().map(|c| c[1]).min().unwrap_or(0);
        let mk = |a: i32, y: i32| if axis == 'x' { [a, y, pl] } else { [pl, y, a] };
        let cells: Vec<C> = (a0..=a1).flat_map(|a| (y0..y0 + 3).map(move |y| mk(a, y))).collect();
        let mut frame: Vec<C> = Vec::new();
        for y in y0..y0 + 3 {
            frame.push(mk(a0 - 1, y));
            frame.push(mk(a1 + 1, y));
        }
        for a in a0..=a1 {
            frame.push(mk(a, y0 - 1));
            frame.push(mk(a, y0 + 3));
        }
        for c in cells.iter().chain(frame.iter()) {
            if !run.world.get(bp(*c)).is_air() {
                return Err(format!("control divert-return: the planted portal's cell ({},{},{}) holds {}: refused", c[0], c[1], c[2], run.world.get(bp(*c))));
            }
        }
        let portal_block = redsim::terrain::land::block_of(&format!("minecraft:nether_portal[axis={axis}]"))?;
        let obsidian = Block::parse("minecraft:obsidian")?;
        for c in &frame {
            run.world.set_block(bp(*c), obsidian.clone(), raw);
        }
        for c in &cells {
            run.world.set_block(bp(*c), portal_block.clone(), raw);
        }
        let back = cells.iter().all(|c| run.world.get(bp(*c)) == portal_block) && frame.iter().all(|c| run.world.get(bp(*c)) == obsidian);
        s.push_str(&format!(
            "  CONTROL divert-return: a lit portal planted (raw, axis {axis}) at {} cells {}..{} along, y {}..{}, the plane {} (the return's exit blocks there: {}), with {} frame obsidian; read back: {}\n",
            cells.len(),
            a0,
            a1,
            y0,
            y0 + 2,
            pl,
            front.len(),
            frame.len(),
            if back { "every cell" } else { "CHANGED: refused" }
        ));
        if !back {
            return Err("control divert-return: the planted portal did not read back".into());
        }
        plant = Some(Portal { name: "planted.divert".into(), part: usize::MAX, nether: false, cells, axis });
    }
    let ow_ps: Vec<&Portal> = plant.iter().chain(ps.iter().filter(|p| !p.nether)).collect();
    let start = run.world.time;
    let mut seen = run.world.crossings.len();
    let mut golems: BTreeMap<u32, (String, [f64; 3])> = BTreeMap::new();
    let mut arrivals: [BTreeMap<String, usize>; 2] = [BTreeMap::new(), BTreeMap::new()];
    let mut times: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    let mut golem_out: Vec<(String, [f64; 3], String, i64)> = Vec::new();
    let mut first_launch: [Option<i64>; 2] = [None, None];
    let mut launches = [0usize; 2];
    let mut copies: BTreeSet<u32> = BTreeSet::new();
    let mut known: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    let mut min_items = u32::MAX;
    let mut opened: Option<bool> = None;
    let mut missing: Vec<String> = Vec::new();
    let mut stolen = false;
    for _ in 0..o.ticks {
        let rel = run.world.time - start;
        if rel == o.world_open && opened.is_none() {
            // (ironspec 01:50b: the gap bar spans a world open; the overworld's entities come back under
            // new ids, so its items then are not launches)
            let r = crate::liveness::restart(&mut run.world, crate::liveness::Restart::load(redsim::reload::Order::Save))?;
            let happened = r.chunks >= 1 && r.ticks_back == r.ticks_out;
            opened = Some(happened);
            s.push_str(&format!(
                "  the world open at t {rel} (save order, {} idle ticks: the larger): {} chunks unloaded and loaded again, scheduled ticks {} out / {} back, {} entities held, game time t {} after it; the Nether part {}: {}\n",
                crate::liveness::WORLD_OPEN_IDLE,
                r.chunks,
                r.ticks_out,
                r.ticks_back,
                r.held,
                r.at - start,
                if r.nether { "kept in memory (the reload model saves only the overworld: unmodelled there)" } else { "absent" },
                if happened { "it happened" } else { "FAIL: it did not happen" }
            ));
            for e in run.world.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Item(_))) {
                known[0].insert(e.id);
            }
        }
        if rel == o.golem_at {
            for p in ps.iter().filter(|p| p.part != li && !p.nether) {
                let (lo, hi) = p.lo_hi();
                // (the second cell from each end and the middle: a 1.4-wide golem's box inside the portal)
                let along = |a: i32, b: i32| [(a + 1).min(b), (a + b) / 2, (b - 1).max(a)];
                let xs = if p.axis == 'x' { along(lo[0], hi[0]) } else { [lo[0]; 3] };
                let zs = if p.axis == 'x' { [lo[2]; 3] } else { along(lo[2], hi[2]) };
                for k in 0..3 {
                    let at = [xs[k] as f64 + 0.5, lo[1] as f64, zs[k] as f64 + 0.5];
                    let id = redsim::entity::spawn(&mut run.world, "minecraft:iron_golem", at, Some("{PersistenceRequired:1b}"))?;
                    golems.insert(id, (p.name.clone(), at));
                }
            }
        }
        if o.control == Some(Control::StallOverworldClock) && rel == o.ticks / 4 {
            let (a, b) = stall_pair.ok_or("control stall-overworld-clock: no overworld observer pair")?;
            for c in [a, b] {
                run.world.set_block(bp(c), air.clone(), raw);
            }
            let stopped = [a, b].iter().all(|c| run.world.get(bp(*c)).is_air());
            s.push_str(&format!(
                "  CONTROL stall-overworld-clock: at t {rel} the overworld clock's observers ({},{},{}) and ({},{},{}) taken out (raw, no update); read back: {}\n",
                a[0], a[1], a[2], b[0], b[1], b[2],
                if stopped { "air, air" } else { "NOT air: refused" }
            ));
            if !stopped {
                return Err("control stall-overworld-clock: the observers did not read back as air".into());
            }
        }
        if o.control == Some(Control::StealItem) && rel >= o.ticks / 4 && !stolen {
            let mut pick: Option<(usize, u32, u32)> = None;
            for wi in 0..2 {
                let w = side(&mut run, wi)?;
                if let Some(e) = w.entities.list.iter().find(|e| !e.removed && matches!(&e.kind, EntityKind::Item(st) if kinds.contains(st.item.trim_start_matches("minecraft:")))) {
                    let k = match &e.kind {
                        EntityKind::Item(st) => st.count as u32,
                        _ => 0,
                    };
                    pick = Some((wi, e.id, k));
                    break;
                }
            }
            if let Some((wi, id, k)) = pick {
                let count = |run: &Run| -> u32 { loop_items_in(&run.world, &kinds) + run.world.nether.as_deref().map_or(0, |n| loop_items_in(n, &kinds)) };
                let before = count(&run);
                redsim::entity::kill(side(&mut run, wi)?, id)?;
                let after = count(&run);
                stolen = true;
                s.push_str(&format!(
                    "  CONTROL steal-item: at t {rel} a loop item in flight on the {} side (entity {id}, {k}) killed; read back: the loop's items {before} -> {after}{}\n",
                    if wi == 0 { "overworld" } else { "Nether" },
                    if after + k == before { "" } else { " (UNCHANGED: refused)" }
                ));
                if after + k != before {
                    return Err("control steal-item: the loop's items did not read back one item less".into());
                }
            }
        }
        run.tick();
        let n = run.world.nether.as_deref().ok_or("no Nether world")?;
        if run.world.away.is_none() && n.away.is_none() {
            min_items = min_items.min(loop_items_in(&run.world, &kinds) + loop_items_in(n, &kinds));
        }
        let cr: Vec<(i64, u32, u32)> = run.world.crossings[seen..].to_vec();
        seen = run.world.crossings.len();
        for (_, from, copy) in cr {
            copies.insert(copy);
            let (dim, pos, kind) = if let Some(i) = redsim::entity::index_of(n, copy) {
                let e = &n.entities.list[i];
                (1usize, [e.pos.x, e.pos.y, e.pos.z], matches!(e.kind, EntityKind::Item(_)))
            } else if let Some(i) = redsim::entity::index_of(&run.world, copy) {
                let e = &run.world.entities.list[i];
                (0usize, [e.pos.x, e.pos.y, e.pos.z], matches!(e.kind, EntityKind::Item(_)))
            } else {
                // (G1 06:37 N10: a crossing whose copy is in neither world is lost: it FAILS)
                missing.push(format!("t {rel}: {from} -> {copy}"));
                continue;
            };
            let at = portal_of(if dim == 1 { &n_ps } else { &ow_ps }, pos).unwrap_or_else(|| format!("no portal ({:.2},{:.2},{:.2})", pos[0], pos[1], pos[2]));
            if let Some((pad, p0)) = golems.get(&from) {
                golem_out.push((pad.clone(), *p0, at, rel));
            } else if kind {
                *arrivals[dim].entry(at).or_default() += 1;
                times[dim].push(rel);
            } else {
                s.push_str(&format!("  t {rel}: a non-item crossing {from} -> {copy} into {at}\n"));
            }
        }
        // (a launch: a new item entity no crossing made, on each side)
        for (wi, w) in [(0usize, &run.world), (1usize, n)] {
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Item(_))) {
                if known[wi].insert(e.id) && !copies.contains(&e.id) {
                    launches[wi] += 1;
                    first_launch[wi].get_or_insert(rel);
                }
            }
        }
    }
    // (a control that changed nothing refuses by name)
    if o.control == Some(Control::StealItem) && !stolen {
        return Err("control steal-item: no loop item was ever in flight to take: a no-op, refused".into());
    }
    if o.control == Some(Control::DivertReturn) && arrivals[0].get("planted.divert").copied().unwrap_or(0) == 0 {
        return Err("control divert-return: no return arrived in the planted portal: a no-op, refused".into());
    }
    let n = run.world.nether.as_deref().ok_or("no Nether world")?;
    // (the run's last tick, game time: the world open's idle ticks advance it past the tick count)
    let end = run.world.time - start - 1;
    let items_end = loop_items_in(&run.world, &kinds) + loop_items_in(n, &kinds);
    // (the world errors: the two named model limits, and every other one, which FAILS by name)
    let mut spits = settle_spits.clone();
    let mut named: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut unnamed: Vec<String> = Vec::new();
    for (w, e) in run.world.errors.iter().map(|e| ("overworld", e)).chain(n.errors.iter().map(|e| ("Nether", e))) {
        match model_limit(e, &mut spits) {
            Some(why) => *named.entry(why).or_default() += 1,
            None => unnamed.push(format!("{w}: {e}")),
        }
    }
    // (the settle's refused spits each lost the one item their dropper had already taken)
    let refused = (settle_spits.len() - spits.len()) as u32;
    let mut ok = true;
    // (the world-open row: the open must have happened inside the run)
    match opened {
        Some(true) => {}
        Some(false) => ok = false,
        None => {
            ok = false;
            s.push_str(&format!("  the world open at t {}: never reached in {} gt: FAIL (the gap bar needs its world-open row)\n", o.world_open, o.ticks));
        }
    }
    for (dim, own, label) in [(1usize, &lo_n.name, "into the Nether"), (0usize, &lo_ow.name, "into the overworld")] {
        let a = &arrivals[dim];
        let total: usize = a.values().sum();
        let elsewhere: usize = a.iter().filter(|(k, _)| *k != own).map(|(_, v)| v).sum();
        let mut gaps: Vec<i64> = times[dim].windows(2).map(|w| w[1] - w[0]).collect();
        if let Some(l) = times[dim].last() {
            gaps.push(end - l);
        }
        let worst = gaps.iter().copied().max().unwrap_or(o.ticks);
        let first = times[dim].first().copied();
        let pass = total > 0 && elsewhere == 0 && worst <= 300;
        ok &= pass;
        s.push_str(&format!(
            "  item arrivals {label}: {total} ({}), {elsewhere} elsewhere, the first at t {}, the worst gap after it {worst} gt, the world open included (the PORTAL ticket, TicketType.PORTAL: 300 gt, {} gt left): {}\n",
            a.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "),
            first.map_or("-".to_string(), |f| f.to_string()),
            300 - worst,
            if pass { "PASS" } else { "FAIL" }
        ));
    }
    // (ironspec 02:33 (2): a row that differs from the reference timing row says how)
    s.push_str(&format!(
        "  the gap bar's row against the reference timing row (ironfarm's flags: each clocked event +-2 gt in the tightening direction, a k + j = 3 world open at every phase, the worst over phases): it differs: ONE world open at t {} (save order, 2 idle ticks), no +-2 gt shift of the loader's clocked events; the game's 01:20 restart rows (tick-exact) are the loader's world-open evidence\n",
        o.world_open
    ));
    let launched = first_launch[0].is_some() && first_launch[1].is_some();
    ok &= launched;
    s.push_str(&format!(
        "  launches (a new item entity no crossing made; read back: the clocks run): overworld side {} (the first at t {}), Nether side {} (the first at t {}; its clock ticks once an arrival tickets it): {}\n",
        launches[0],
        first_launch[0].map_or("never".into(), |t| t.to_string()),
        launches[1],
        first_launch[1].map_or("never".into(), |t| t.to_string()),
        if launched { "PASS" } else { "FAIL" }
    ));
    let want = items0 - refused.min(items0);
    let kept = items_end == want && min_items == want;
    ok &= kept;
    s.push_str(&format!(
        "  the items: {items0} laid{}, {items_end} at the end, at least {min_items} at every tick: {}\n",
        if refused > 0 { format!(", {refused} refused by redsim while Run::start settled the Nether part (a named model limit, below)") } else { String::new() },
        if kept { "PASS" } else { "FAIL" }
    ));
    let mut per_pad: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut gfail = 0;
    let crossed: BTreeSet<&String> = golem_out.iter().map(|g| &g.0).collect();
    for (pad, p0, at, t) in &golem_out {
        let want = ps.iter().find(|p| p.name == *pad).map(|p| ps.iter().find(|q| q.part == p.part && q.nether).map(|q| q.name.clone()).unwrap_or_default()).unwrap_or_default();
        if *at != want {
            gfail += 1;
        }
        per_pad.entry(pad.clone()).or_default().push(format!("({:.1},{:.1},{:.1}) -> {at} at t {t}{}", p0[0], p0[1], p0[2], if *at == want { "" } else { " FAIL" }));
    }
    let placed = golems.len();
    // (G1 06:37 (2): no golem placed judges nothing: it FAILS)
    let gpass = placed > 0 && golem_out.len() == placed && gfail == 0;
    ok &= gpass;
    s.push_str(&format!(
        "  golems: {placed} placed at t {}{} ({} pads), {} crossed, {gfail} at another portal than their copy's twin: {}\n",
        o.golem_at,
        if placed == 0 { format!(" (none: --golem-at {} is outside the run's 0..{} or no pad)", o.golem_at, o.ticks) } else { String::new() },
        crossed.len(),
        golem_out.len(),
        if gpass { "PASS" } else { "FAIL" }
    ));
    for (pad, v) in &per_pad {
        s.push_str(&format!("    {pad}: {}\n", v.join("; ")));
    }
    if !missing.is_empty() {
        ok = false;
    }
    s.push_str(&format!("  crossings whose copy is in neither world: {}\n", if missing.is_empty() { "none".to_string() } else { format!("{} ({}): FAIL", missing.len(), missing.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }));
    let tickets: Vec<String> = n.portal_tickets.iter().map(|(c, t)| format!("({},{}) renewed t {}", c[0], c[1], t - start)).collect();
    s.push_str(&format!("  the Nether's PORTAL tickets at the end: {}\n", if tickets.is_empty() { "none".into() } else { tickets.join(", ") }));
    s.push_str(&format!(
        "  redsim's named model limits met (NOTES' loader-clock-x8 row; not judged): {}\n",
        if named.is_empty() { "none".to_string() } else { named.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join("; ") }
    ));
    if !unnamed.is_empty() {
        ok = false;
    }
    s.push_str(&format!("  other world errors: {}\n", if unnamed.is_empty() { "none".to_string() } else { format!("{}: FAIL", unnamed.join("; ")) }));
    // (G1 06:37 (1): a failed bar always FAILS; only a run whose bars all pass with only the named model
    // limits met is NOT MODELED)
    let v = if !ok {
        Verdict::Fail
    } else if !named.is_empty() {
        Verdict::NotModeled
    } else {
        Verdict::Pass
    };
    s.push_str(&format!(
        "  redsim verdict (seed {}): {}\n",
        o.seed,
        match v {
            Verdict::Pass => "PASS".to_string(),
            Verdict::Fail => "FAIL".to_string(),
            Verdict::NotModeled => format!("NOT MODELED (every bar passed; {} named model limits met: the run judges nothing)", named.values().sum::<usize>()),
        }
    ));
    Ok((s, v))
}

/// A run's verdict: its bars met, a bar failed, or every bar met with only named model limits (a run
/// that judges nothing: ironspec 02:45 (3), G1 06:37 (1)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    NotModeled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_lines_give_their_items() {
        assert_eq!(snbt_items(r#"{Items:[{Slot:0b,id:"minecraft:leather",count:1}]}"#), vec![("leather".to_string(), 1)]);
        assert_eq!(
            snbt_items(r#"{Items:[{Slot:0b,id:"minecraft:cobblestone",count:2},{Slot:1b,id:"minecraft:stone"}]}"#),
            vec![("cobblestone".to_string(), 2), ("stone".to_string(), 1)]
        );
        // (G1 06:37 N6: an item without a count never takes the next item's)
        assert_eq!(
            snbt_items(r#"{Items:[{Slot:0b,id:"minecraft:stone"},{Slot:1b,count:5,id:"minecraft:dirt"}]}"#),
            vec![("stone".to_string(), 1), ("dirt".to_string(), 5)]
        );
        assert!(snbt_items("{}").is_empty());
    }

    #[test]
    fn the_loop_items_are_the_droppers_not_the_clock_hoppers() {
        let mut l = Laid { name: "loader".into(), ..Default::default() };
        l.blocks.insert([0, 0, 0], "minecraft:dropper[facing=north,triggered=false]".into());
        l.blocks.insert([1, 0, 0], "minecraft:hopper[enabled=true,facing=east]".into());
        l.block_entities.insert([0, 0, 0], r#"{Items:[{Slot:0b,id:"minecraft:cobblestone",count:2}]}"#.into());
        l.block_entities.insert([1, 0, 0], r#"{Items:[{Slot:0b,id:"minecraft:stone",count:13}]}"#.into());
        l.nether.insert([3, 0, 0], "minecraft:dropper[facing=south,triggered=false]".into());
        l.nether_block_entities.insert([3, 0, 0], r#"{Items:[{Slot:0b,id:"minecraft:cobblestone",count:1}]}"#.into());
        let (kinds, n) = loop_items(&l);
        assert_eq!((kinds.into_iter().collect::<Vec<_>>(), n), (vec!["cobblestone".to_string()], 3));
    }

    #[test]
    fn observer_pairs_and_portal_membership() {
        let mut cells: BTreeMap<C, String> = BTreeMap::new();
        cells.insert([1, 13, 2], "minecraft:observer[facing=south,powered=false]".into());
        cells.insert([1, 13, 3], "minecraft:observer[facing=north,powered=false]".into());
        cells.insert([5, 13, 2], "minecraft:observer[facing=south,powered=false]".into());
        assert_eq!(observer_pairs(&cells), vec![([1, 13, 2], [1, 13, 3])]);
        let p = Portal { name: "loader.overworld".into(), part: 0, nether: false, cells: (0..5).flat_map(|x| (90..95).map(move |y| [x, y, 19])).collect(), axis: 'x' };
        assert_eq!(portal_of(&[&p], [3.5, 94.4, 19.3]), Some("loader.overworld".into()));
        assert_eq!(portal_of(&[&p], [3.5, 94.4, 20.9]), Some("loader.overworld".into()));
        assert_eq!(portal_of(&[&p], [5.5, 94.4, 19.5]), None);
        // (an exact cell of a second portal wins over the first's grown box)
        let q = Portal { name: "planted".into(), part: 1, nether: false, cells: vec![[3, 94, 18], [4, 94, 18]], axis: 'x' };
        assert_eq!(portal_of(&[&p, &q], [3.5, 94.4, 18.5]), Some("planted".into()));
        assert_eq!(portal_of(&[&q, &p], [3.5, 94.4, 19.4]), Some("loader.overworld".into()));
    }

    #[test]
    fn only_the_two_named_model_limits_are_not_judged() {
        let mut spits = vec!["dropper at (3,90,1) spits an item into the world: x".to_string()];
        assert!(model_limit("the Nether: dropper at (3,90,1) spits an item into the world: x", &mut spits).is_some() && spits.is_empty());
        // (a second spit, after the settle, is not the named one)
        assert!(model_limit("the Nether: dropper at (3,90,1) spits an item into the world: x", &mut spits).is_none());
        let mut none = Vec::new();
        assert!(model_limit("the light behaviour of minecraft:sticky_piston[extended=true,facing=east] (its light dampening and occluding faces) is not modeled", &mut none).is_some());
        assert!(model_limit("the light behaviour of minecraft:piston_head[facing=west,short=false,type=sticky] (x) is not modeled", &mut none).is_some());
        assert!(model_limit("the light behaviour of minecraft:sticky_piston[extended=false,facing=east] (x) is not modeled", &mut none).is_none());
        assert!(model_limit("block \"nether_portal\" is not modeled by redsim", &mut none).is_none());
    }

    /// The loader of record (in tree) at (25,80,19) with rev 3 (in tree) at `copies`.
    fn layout_x8(copies: &[C]) -> Vec<Laid> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |p: &str| mctas::design::Design::parse(&std::fs::read_to_string(root.join(p)).unwrap()).unwrap();
        let m = read("designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-record7f13b.txt");
        let l = read("designs/farms/iron/carousel-assembly/wip/loaderlay/loader-x8.txt");
        let mut parts: Vec<Laid> = copies.iter().enumerate().map(|(k, o)| crate::loaderlayout::lay(&format!("copy{}", k + 1), "m", &m, *o, 64).unwrap()).collect();
        parts.push(crate::loaderlayout::lay("loader", "l", &l, [25, 80, 19], 0).unwrap());
        parts
    }

    fn opts(control: Option<Control>) -> Opts {
        Opts { ticks: 1200, seed: 1, golem_at: 100, world_open: 600, control }
    }

    /// The bar's line in a run's text.
    fn line<'a>(t: &'a str, key: &str) -> &'a str {
        t.lines().find(|l| l.contains(key)).unwrap_or_else(|| panic!("no {key:?} line in\n{t}"))
    }

    #[test]
    fn the_runner_passes_the_loader_of_record_and_each_control_fails_its_bar() {
        // (G1 06:37 (3): the base passes; each control, one planted change, fails its own bar)
        let parts = layout_x8(&[[6, 80, 6]]);
        let (t, v) = run(&parts, &opts(None)).unwrap();
        assert_eq!(v, Verdict::Pass, "{t}");
        // (the gap bars: the overworld clock stalled a quarter in)
        let (t, v) = run(&parts, &opts(Some(Control::StallOverworldClock))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "item arrivals into the Nether").ends_with("FAIL") && line(&t, "CONTROL stall-overworld-clock:").contains("read back: air, air"), "{t}");
        assert!(line(&t, "the items:").ends_with("PASS") && line(&t, "golems:").ends_with("PASS"), "{t}");
        // (the launches bar: the Nether clock never runs)
        let (t, v) = run(&parts, &opts(Some(Control::StopNetherClock))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "launches (").ends_with("FAIL") && line(&t, "launches (").contains("Nether side 0 (the first at t never"), "{t}");
        // (the items bar: one item in flight killed)
        let (t, v) = run(&parts, &opts(Some(Control::StealItem))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "the items:").ends_with("FAIL") && line(&t, "CONTROL steal-item:").contains("read back"), "{t}");
        assert!(line(&t, "launches (").ends_with("PASS") && line(&t, "golems:").ends_with("PASS"), "{t}");
        // (the arrivals bar's elsewhere: a portal planted on the return's exit blocks)
        let (t, v) = run(&parts, &opts(Some(Control::DivertReturn))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "item arrivals into the overworld").contains("planted.divert") && line(&t, "item arrivals into the overworld").ends_with("FAIL"), "{t}");
        // (the golem bar: none placed, --golem-at past the run (game time reaches the tick count plus
        // the world open's idle ticks))
        let (t, v) = run(&parts, &Opts { golem_at: 5000, ..opts(None) }).unwrap();
        assert!(v == Verdict::Fail && line(&t, "golems:").contains("golems: 0 placed") && line(&t, "golems:").ends_with("FAIL"), "{t}");
        // (the world-open row: never reached)
        let (t, v) = run(&parts, &Opts { world_open: 5000, ..opts(None) }).unwrap();
        assert!(v == Verdict::Fail && line(&t, "the world open at t 5000").contains("never reached"), "{t}");
    }
}
