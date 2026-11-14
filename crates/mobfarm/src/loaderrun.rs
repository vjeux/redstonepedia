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
//! item entity no crossing made. At the first tick at or after `--golem-at` an iron golem is put in each
//! pad at three places of its bottom row (the second cell from each end and the middle: its 1.4-wide box
//! inside the portal). Every crossing is printed by the portal it came out of. A world open
//! (`liveness::restart`, save order, 2 idle ticks: it advances game time 2 past the tick count) at
//! `--world-open-at` (default half the run) puts the gap bar on a world-open row (ironspec 01:50b): its
//! evidence printed (chunks unloaded and loaded again, the scheduled ticks out and back), the reload
//! model keeping the Nether part in memory (unmodelled there, said in the row), and how the row differs
//! from the reference timing row (ironspec 02:33 (2)).
//!
//! The bars (each FAILS by name; G1 06:37 (1)-(3), 08:37 (1)-(3)):
//! - every item arrival lands at the loader's own portal on each side, and each side's worst gap
//!   between arrivals stays within the PORTAL ticket's 300 gt (`TicketType.PORTAL`), the world open
//!   included;
//! - both sides launch;
//! - the loop's items are all there at every tick, less the items read back lost while `Run::start`
//!   settled the Nether part when redsim refused that settle's spits (a named model limit);
//! - golems were placed (none placed FAILS), and every golem crosses to its own copy's twin;
//! - the world open happened inside the run;
//! - no crossing's copy goes missing;
//! - no world error other than the two model limits NOTES names for loader-clock-x8 (redsim's light
//!   model lacking the extended sticky piston and the piston head; a spit refused while `Run::start`
//!   settles the Nether part). A failed bar always FAILS; a run whose bars all pass with only those
//!   named limits met is NOT MODELED (it judges nothing); any other world error FAILS by name.
//!
//! Every bar has a failing control through [`run`] (`Control`, `--redsim-control`): one planted change,
//! read back, fired at the first tick at or after its time (a world open's idle ticks skip game times),
//! and REFUSED (`Verdict::Refused`) when it planted nothing or its own bar didn't fail.

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
    /// A failing control of the bars (G1 06:37 (3), 08:37 (2)).
    pub control: Option<Control>,
}

/// A failing control of the runner's bars (G1 Nov 14 06:37 (3), 08:37 (1) / (2); ironspec 08:07a (5)):
/// one planted change on the run, read back, a no-op refused by name; each drives its own bar
/// ([`Control::target`]) to FAIL through [`run`], else the run is REFUSED.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Both observers of the overworld clock taken out (raw: no update) at the first tick at or after a
    /// quarter of the run, read back as air: the overworld side stops launching, so the arrivals stop
    /// on both sides.
    StallOverworldClock,
    /// Both observers of the Nether clock taken out (raw) before the run, read back: the Nether side never
    /// launches.
    StopNetherClock,
    /// One loop item in flight killed (`entity::kill`) from a quarter into the run, read back.
    StealItem,
    /// A lit portal planted (raw, framed, the loader portal's axis) on the return's exit blocks one plane
    /// off the loader's overworld portal, read back: the returns whose exit block is there arrive in it.
    DivertReturn,
    /// (ironspec 08:07a (5), a crash's torn save) One loop item DOUBLED from a quarter into the run: one
    /// more of the loop's kind put into the overworld side's firing dropper, read back: the items bar
    /// FAILS on the count, and the other bars print what a doubled item does to the loading.
    DoubleItem,
    /// The first copy's Nether twin put out (its portal cells set to air, raw) before the run, read
    /// back: that copy's golems cross to another portal.
    PutOutTwin,
    /// The copy of the first loop item crossing from a quarter into the run taken out of its world's
    /// entity list (as redsim drops a removed entity), read back: a crossing whose copy is in neither
    /// world.
    DropCopy,
    /// The overworld's drop seed unset from a quarter into the run, read back: its next dropper spit is a
    /// world error NOTES does not name.
    UnseedDrops,
}

impl Control {
    pub const NAMES: [&'static str; 8] = ["stall-overworld-clock", "stop-nether-clock", "steal-item", "divert-return", "double-item", "put-out-twin", "drop-copy", "unseed-drops"];

    pub fn parse(s: &str) -> Option<Control> {
        match s.trim() {
            "stall-overworld-clock" => Some(Control::StallOverworldClock),
            "stop-nether-clock" => Some(Control::StopNetherClock),
            "steal-item" => Some(Control::StealItem),
            "divert-return" => Some(Control::DivertReturn),
            "double-item" => Some(Control::DoubleItem),
            "put-out-twin" => Some(Control::PutOutTwin),
            "drop-copy" => Some(Control::DropCopy),
            "unseed-drops" => Some(Control::UnseedDrops),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Control::StallOverworldClock => "stall-overworld-clock",
            Control::StopNetherClock => "stop-nether-clock",
            Control::StealItem => "steal-item",
            Control::DivertReturn => "divert-return",
            Control::DoubleItem => "double-item",
            Control::PutOutTwin => "put-out-twin",
            Control::DropCopy => "drop-copy",
            Control::UnseedDrops => "unseed-drops",
        }
    }

    /// The bar the control must fail.
    pub fn target(self) -> &'static str {
        match self {
            Control::StallOverworldClock => "the arrivals' worst gap",
            Control::StopNetherClock => "the launches",
            Control::StealItem | Control::DoubleItem => "the items",
            Control::DivertReturn => "the arrivals at the loader's own overworld portal",
            Control::PutOutTwin => "every golem at its copy's twin",
            Control::DropCopy => "no crossing's copy missing",
            Control::UnseedDrops => "no other world error",
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

/// The loop's items in both worlds.
fn loop_items_both(run: &Run, kinds: &BTreeSet<String>) -> u32 {
    loop_items_in(&run.world, kinds) + run.world.nether.as_deref().map_or(0, |n| loop_items_in(n, kinds))
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

/// The loop's items laid on each side: [overworld, Nether] (the droppers' `nbt:` / `nether nbt:` lines).
fn loop_items_sides(l: &Laid) -> [u32; 2] {
    let mut out = [0u32; 2];
    for (k, (cells, bes)) in [(&l.blocks, &l.block_entities), (&l.nether, &l.nether_block_entities)].into_iter().enumerate() {
        for (c, s) in bes {
            if cells.get(c).is_some_and(|b| b.starts_with("minecraft:dropper")) {
                out[k] += snbt_items(s).iter().map(|(_, n)| n).sum::<u32>();
            }
        }
    }
    out
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
    // (G1 08:37 N: anchored to redsim's whole message, daylight.rs: "the light behaviour of {block}
    // (its light dampening and occluding faces) is not modeled")
    const TAIL: &str = "] (its light dampening and occluding faces) is not modeled";
    let state = |name: &str| e.strip_prefix(&format!("the light behaviour of minecraft:{name}[")).and_then(|r| r.strip_suffix(TAIL)).filter(|s| !s.contains(']'));
    if state("sticky_piston").is_some_and(|s| s.split(',').any(|p| p == "extended=true")) || state("piston_head").is_some() {
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

/// A control refused (G1 08:37 (1)): the run's text so far with the refusal line, `Verdict::Refused`.
fn refusal(mut s: String, seed: i64, why: &str) -> Result<(String, Verdict), String> {
    s.push_str(&format!("  REFUSED: {why}\n  redsim verdict (seed {seed}): REFUSED\n"));
    Ok((s, Verdict::Refused))
}

/// The dropper position a refused-spit message names (redsim's `World::spawn_dispensed`: "dropper at
/// (x,y,z) spits an item into the world: ..."), or None when the message doesn't read so.
fn spit_position(msg: &str) -> Option<C> {
    let t = msg.strip_prefix("dropper at (")?.split_once(") spits an item into the world")?.0;
    let v: Vec<i32> = t.split(',').map(|n| n.trim().parse().ok()).collect::<Option<Vec<i32>>>()?;
    (v.len() == 3).then(|| [v[0], v[1], v[2]])
}

/// (ironspec 10:35a (4) (3)) The settle's refused spits that came back as world errors (every recorded
/// settle spit not left in `unmatched`), each read for its dropper: (the items they explain, one per
/// spit at a dropper of the loader's Nether part; every matched spit the runner can't credit, by name:
/// a message it can't read, or a position that is no loader dropper). An uncredited spit explains no
/// item, so its loss surfaces as unexplained in the items bar, beside its own FAIL.
fn settle_spit_items(settle_spits: &[String], unmatched: &[String], nether: &BTreeMap<C, String>) -> (u32, Vec<String>) {
    let mut remaining = unmatched.to_vec();
    let (mut items, mut unread) = (0u32, Vec::new());
    for sp in settle_spits {
        if let Some(k) = remaining.iter().position(|r| r == sp) {
            remaining.remove(k);
            continue;
        }
        match spit_position(sp) {
            Some(c) if nether.get(&c).is_some_and(|b| b.starts_with("minecraft:dropper")) => items += 1,
            Some(c) => unread.push(format!("a refused spit at ({},{},{}), no dropper of the loader's Nether part", c[0], c[1], c[2])),
            None => unread.push(format!("a refused spit whose message the runner can't read (no dropper position in it): {sp:?}")),
        }
    }
    (items, unread)
}

/// The items bar (G1 08:37 N / 09:47 (3), ironspec 09:47a (3)): the loop's items kept at every tick, less
/// only what the settle's refused dropper spits explain, each spit matched to its own shortfall: one item
/// at its own side (a dropper spits one item, redstone/container.rs 1319), read back side by side
/// right after `Run::start` (`laid` and `at_start`: [overworld, Nether]; the settle's spits are the
/// Nether part's). Any shortfall no spit explains, on either side, and any surplus, FAILS by name:
/// (pass, line).
fn items_bar(laid: [u32; 2], at_start: [u32; 2], spit_items: u32, items_end: u32, min_items: u32) -> (bool, String) {
    let items0 = laid[0] + laid[1];
    let lost = [laid[0].saturating_sub(at_start[0]), laid[1].saturating_sub(at_start[1])];
    let surplus = at_start[0].saturating_sub(laid[0]) + at_start[1].saturating_sub(laid[1]);
    let excused = lost[1].min(spit_items);
    let unexplained = lost[0] + lost[1] - excused;
    let want = items0 - excused;
    let kept = unexplained == 0 && surplus == 0 && items_end == want && min_items == want;
    let mut why = String::new();
    if excused > 0 {
        why.push_str(&format!(
            ", {excused} lost to {excused} refused dropper spit(s) while Run::start settled the Nether part (one item each, matched on the Nether side: {} laid there, {} right after the start; a named model limit, below)",
            laid[1], at_start[1]
        ));
    }
    if unexplained > 0 {
        why.push_str(&format!(
            ", {unexplained} more missing right after the start than the refused spits explain (overworld {} laid, {} after; Nether {} laid, {} after; not excused)",
            laid[0], at_start[0], laid[1], at_start[1]
        ));
    }
    if surplus > 0 {
        why.push_str(&format!(", {surplus} more right after the start than laid (not excused)"));
    }
    (kept, format!("  the items: {items0} laid{why}, {items_end} at the end, at least {min_items} at every tick: {}\n", if kept { "PASS" } else { "FAIL" }))
}

/// One seed's run; (text, verdict).
pub fn run(parts: &[Laid], o: &Opts) -> Result<(String, Verdict), String> {
    // (G1 08:37 (3), 09:47 (2): whatever stops the run before its first tick is a refusal, exit 4,
    // never the FAIL code)
    let mut s = String::new();
    macro_rules! pre {
        ($e:expr) => {
            match $e {
                Ok(v) => v,
                Err(e) => return refusal(s, o.seed, &format!("before the run's first tick: {e}")),
            }
        };
    }
    let ps = pre!(portals(parts, &[]));
    let li = pre!(parts.iter().position(|p| p.name == "loader").ok_or("no loader part"));
    let lo_ow = pre!(ps.iter().find(|p| p.part == li && !p.nether).ok_or("no loader overworld portal"));
    let lo_n = pre!(ps.iter().find(|p| p.part == li && p.nether).ok_or("no loader Nether portal"));
    let n_ps: Vec<&Portal> = ps.iter().filter(|p| p.nether).collect();
    let spec = pre!(spec_of(parts));
    let (kinds, items0) = loop_items(&parts[li]);
    if items0 == 0 {
        return refusal(s, o.seed, "the loader's droppers hold no item (the loop's items are what its droppers hold): nothing to follow");
    }
    s.push_str(&format!(
        "redsim run (seed {}, {} gt; bench, tool unreviewed): the portal skeleton ({} overworld cells: the loader's and each copy's portal cells with their frame obsidian; {} Nether cells); the loop's items (its droppers'): {items0} ({}){}\n",
        o.seed,
        o.ticks,
        spec.blocks.len(),
        spec.nether.as_ref().map_or(0, |n| n.blocks.len()),
        kinds.iter().cloned().collect::<Vec<_>>().join(", "),
        o.control.map_or(String::new(), |c| format!("; CONTROL {}, planted to fail {} (not evidence for the layout)", c.name(), c.target()))
    ));
    let ro = RunOpts::steady(o.seed, (o.ticks + 200) as u32, WalkMode::Random);
    let mut run = pre!(Run::start(&spec, &ro));
    // (the spits redsim refused while Run::start settled the Nether part: the second named model limit;
    // the loop's items read back right after the start: what those spits lost, G1 08:37 N)
    let settle_spits: Vec<String> = run.world.nether.as_deref().map(|n| n.errors.iter().filter(|e| e.contains("spits an item into the world")).cloned().collect()).unwrap_or_default();
    let at_start = [loop_items_in(&run.world, &kinds), run.world.nether.as_deref().map_or(0, |n| loop_items_in(n, &kinds))];
    // (each world's level random and drop seed, set before the overworld's settle: a clock that starts as
    // it is built (a hopper clock) fires its droppers while the layout settles, and a dropper's spit draws
    // from them)
    for wi in 0..2 {
        let w = pre!(side(&mut run, wi));
        let k = if wi == 0 { 0 } else { 0x4e45 };
        w.level_random = Some(redsim::rng::LegacyRandom::new(o.seed ^ k));
        w.drop_seed = Some(o.seed ^ k);
    }
    while run.settle_tick() {}
    // (the clocks' start, as the build starts an observer pair: each side's observer farther from its
    // portal taken out and put back, read back; a side with no observer pair (a hopper clock) needs none)
    let air = pre!(Block::parse("minecraft:air"));
    let raw = redsim::world::flags::RAW;
    let mut stall_pair: Option<(C, C)> = None;
    for (wi, cells, portal) in [(0usize, &parts[li].blocks, lo_ow), (1usize, &parts[li].nether, lo_n)] {
        let side_name = if wi == 0 { "overworld" } else { "Nether" };
        let targeted = matches!((o.control, wi), (Some(Control::StopNetherClock), 1) | (Some(Control::StallOverworldClock), 0));
        let pairs = observer_pairs(cells);
        let (a, b) = match pairs.as_slice() {
            [] if targeted => return refusal(s, o.seed, &format!("control {}: the {side_name} clock has no observer pair to take out: control planted nothing", o.control.map_or("", |c| c.name()))),
            [] => {
                s.push_str(&format!("  the {side_name} clock: no observer pair (nothing taken out: a clock that starts as it is built)\n"));
                continue;
            }
            [(a, b)] => (*a, *b),
            _ => return refusal(s, o.seed, &format!("before the run's first tick: the loader's {side_name} side has {} observer pairs (its clock: at most one)", pairs.len())),
        };
        if o.control == Some(Control::StopNetherClock) && wi == 1 {
            let w = pre!(side(&mut run, 1));
            for c in [a, b] {
                w.set_block(bp(c), air.clone(), raw);
            }
            let stopped = [a, b].iter().all(|c| w.get(bp(*c)).is_air());
            s.push_str(&format!(
                "  CONTROL stop-nether-clock: the Nether clock's observers ({},{},{}) and ({},{},{}) taken out (raw, no update); read back: {}\n",
                a[0],
                a[1],
                a[2],
                b[0],
                b[1],
                b[2],
                if stopped { "air, air" } else { "NOT air" }
            ));
            if !stopped {
                return refusal(s, o.seed, "control stop-nether-clock: the observers did not read back as air");
            }
            continue;
        }
        if o.control == Some(Control::StallOverworldClock) && wi == 0 {
            stall_pair = Some((a, b));
        }
        let plane = |c: &C| if portal.axis == 'x' { (c[2] - portal.cells[0][2]).abs() } else { (c[0] - portal.cells[0][0]).abs() };
        let o2 = if plane(&a) >= plane(&b) { a } else { b };
        let w = pre!(side(&mut run, wi));
        let st = w.get(bp(o2)).clone();
        w.set_block(bp(o2), air.clone(), redsim::world::flags::ALL);
        w.set_block(bp(o2), st.clone(), redsim::world::flags::ALL);
        let back = w.get(bp(o2)).clone();
        s.push_str(&format!("  the {side_name} clock's start: O2 ({},{},{}) taken out and put back; read back: {back}{}\n", o2[0], o2[1], o2[2], if back == st { "" } else { " (CHANGED)" }));
        if back != st {
            return refusal(s, o.seed, &format!("before the run's first tick: the {side_name} clock's O2 did not read back"));
        }
    }
    // (the divert control's planted portal: on the return's exit blocks (one move, the code's crossing)
    // that lie one plane off the loader's overworld portal, 3 high and at least 2 wide, framed)
    let mut plant: Option<Portal> = None;
    if o.control == Some(Control::DivertReturn) {
        let fire = pre!(crate::loaderlayout::firing_droppers(&parts[li].nether, &lo_n.cells));
        let [(p, d)] = fire[..] else { return refusal(s, o.seed, &format!("control divert-return: {} droppers fire into the loader's Nether portal (one expected)", fire.len())) };
        let (l1, h1) = crate::loaderlayout::crossing_box(p, d, 1);
        let exits = crate::loaderlayout::exits_of_box(l1, h1, 8.0);
        let axis = lo_ow.axis;
        let across = |c: &C| if axis == 'x' { c[2] } else { c[0] };
        let along = |c: &C| if axis == 'x' { c[0] } else { c[2] };
        let plane = across(&lo_ow.cells[0]);
        let pick = |side_off: i32| -> Vec<C> { exits.iter().filter(|c| across(c) == plane + side_off).copied().collect() };
        let front = if pick(-1).is_empty() { pick(1) } else { pick(-1) };
        let Some(f0) = front.first() else { return refusal(s, o.seed, "control divert-return: no return exit block lies one plane off the loader's portal: a no-op") };
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
        if let Some(c) = cells.iter().chain(frame.iter()).find(|c| !run.world.get(bp(**c)).is_air()) {
            let held = run.world.get(bp(*c)).to_string();
            return refusal(s, o.seed, &format!("control divert-return: the planted portal's cell ({},{},{}) holds {held}", c[0], c[1], c[2]));
        }
        let portal_block = pre!(redsim::terrain::land::block_of(&format!("minecraft:nether_portal[axis={axis}]")));
        let obsidian = pre!(Block::parse("minecraft:obsidian"));
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
            if back { "every cell" } else { "CHANGED" }
        ));
        if !back {
            return refusal(s, o.seed, "control divert-return: the planted portal did not read back");
        }
        plant = Some(Portal { name: "planted.divert".into(), part: usize::MAX, nether: false, cells, axis });
    }
    // (the put-out-twin control: the first copy's Nether twin put out before the run)
    if o.control == Some(Control::PutOutTwin) {
        let Some(twin) = ps.iter().find(|p| p.nether && p.part != li) else { return refusal(s, o.seed, "control put-out-twin: no copy twin: control planted nothing") };
        let w = pre!(side(&mut run, 1));
        for c in &twin.cells {
            w.set_block(bp(*c), air.clone(), raw);
        }
        let out = twin.cells.iter().all(|c| w.get(bp(*c)).is_air());
        s.push_str(&format!("  CONTROL put-out-twin: {}'s {} portal cells set to air (raw); read back: {}\n", twin.name, twin.cells.len(), if out { "air" } else { "NOT air" }));
        if !out {
            return refusal(s, o.seed, "control put-out-twin: the twin's cells did not read back as air");
        }
    }
    // (G1 10:35, ironspec 10:35a (4): the run starts here, so no refusal may follow; `pre!` is shadowed by
    // a compile error from this line on, so a use in the tick loop or after it fails to build)
    #[allow(unused_macros)]
    macro_rules! pre {
        ($e:expr) => {
            compile_error!("pre! after the run's start: a refusal there would hide a run that happened; fail a bar or return the error instead")
        };
    }
    let ow_ps: Vec<&Portal> = plant.iter().chain(ps.iter().filter(|p| !p.nether)).collect();
    let start = run.world.time;
    let mut seen = run.world.crossings.len();
    let mut golems: BTreeMap<u32, (String, [f64; 3])> = BTreeMap::new();
    let mut golems_at: Option<i64> = None;
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
    // (each timed control fires once, at the first tick at or after its time: G1 08:37 (1))
    let mut fired = false;
    let quarter = o.ticks / 4;
    for _ in 0..o.ticks {
        let mut rel = run.world.time - start;
        if rel >= o.world_open && opened.is_none() {
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
            // (G1 09:47 N: the open's idle ticks moved game time on; what follows in this tick is logged at it)
            rel = run.world.time - start;
        }
        if rel >= o.golem_at && golems_at.is_none() {
            golems_at = Some(rel);
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
        if rel >= quarter && !fired {
            match o.control {
                Some(Control::StallOverworldClock) => {
                    let Some((a, b)) = stall_pair else { return refusal(s, o.seed, "control stall-overworld-clock: no overworld observer pair: control planted nothing") };
                    for c in [a, b] {
                        run.world.set_block(bp(c), air.clone(), raw);
                    }
                    let stopped = [a, b].iter().all(|c| run.world.get(bp(*c)).is_air());
                    s.push_str(&format!(
                        "  CONTROL stall-overworld-clock: at t {rel} the overworld clock's observers ({},{},{}) and ({},{},{}) taken out (raw, no update); read back: {}\n",
                        a[0],
                        a[1],
                        a[2],
                        b[0],
                        b[1],
                        b[2],
                        if stopped { "air, air" } else { "NOT air" }
                    ));
                    if !stopped {
                        return refusal(s, o.seed, "control stall-overworld-clock: the observers did not read back as air");
                    }
                    fired = true;
                }
                Some(Control::StealItem) => {
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
                        let before = loop_items_both(&run, &kinds);
                        if let Err(e) = side(&mut run, wi).and_then(|w| redsim::entity::kill(w, id)) {
                            return refusal(s, o.seed, &format!("control steal-item: the item could not be killed ({e}): control planted nothing"));
                        }
                        let after = loop_items_both(&run, &kinds);
                        s.push_str(&format!(
                            "  CONTROL steal-item: at t {rel} a loop item in flight on the {} side (entity {id}, {k}) killed; read back: the loop's items {before} -> {after}\n",
                            if wi == 0 { "overworld" } else { "Nether" }
                        ));
                        if after + k != before {
                            return refusal(s, o.seed, "control steal-item: the loop's items did not read back one item less");
                        }
                        fired = true;
                    }
                }
                Some(Control::DoubleItem) => {
                    // (G1 09:47 (1): every way this control can't plant is a refusal, never the FAIL code)
                    let fire = match crate::loaderlayout::firing_droppers(&parts[li].blocks, &lo_ow.cells) {
                        Ok(f) => f,
                        Err(e) => return refusal(s, o.seed, &format!("control double-item: {e}: control planted nothing")),
                    };
                    let [(p, _)] = fire[..] else { return refusal(s, o.seed, &format!("control double-item: {} droppers fire into the loader's overworld portal (one expected): control planted nothing", fire.len())) };
                    let Some(kind) = kinds.iter().next().cloned() else { return refusal(s, o.seed, "control double-item: no loop item kind: control planted nothing") };
                    let Some(id) = redsim::redstone::container::item_id(&kind) else { return refusal(s, o.seed, &format!("control double-item: no item id for {kind}: control planted nothing")) };
                    let before = loop_items_both(&run, &kinds);
                    let Some(c) = run.world.containers.map.get_mut(&bp(p)) else { return refusal(s, o.seed, "control double-item: the firing dropper has no inventory: control planted nothing") };
                    if let Some(st) = c.slots.iter_mut().flatten().find(|st| st.item == id) {
                        st.count += 1;
                    } else if let Some(slot) = c.slots.iter_mut().find(|st| st.is_none()) {
                        *slot = Some(redsim::redstone::container::Stack::new(id, 1));
                    } else {
                        return refusal(s, o.seed, "control double-item: the firing dropper is full: control planted nothing");
                    }
                    let after = loop_items_both(&run, &kinds);
                    s.push_str(&format!("  CONTROL double-item: at t {rel} one {kind} more put into the overworld side's firing dropper ({},{},{}); read back: the loop's items {before} -> {after}\n", p[0], p[1], p[2]));
                    if after != before + 1 {
                        return refusal(s, o.seed, "control double-item: the loop's items did not read back one item more");
                    }
                    fired = true;
                }
                Some(Control::UnseedDrops) => {
                    run.world.drop_seed = None;
                    s.push_str(&format!("  CONTROL unseed-drops: at t {rel} the overworld's drop seed unset; read back: {}\n", if run.world.drop_seed.is_none() { "unset" } else { "STILL SET" }));
                    if run.world.drop_seed.is_some() {
                        return refusal(s, o.seed, "control unseed-drops: the drop seed did not read back unset");
                    }
                    fired = true;
                }
                _ => {}
            }
        }
        run.tick();
        let mut cr: Vec<(i64, u32, u32)> = run.world.crossings[seen..].to_vec();
        seen = run.world.crossings.len();
        // (the drop-copy control: the first loop item crossing's copy taken out of its world's entity list)
        if o.control == Some(Control::DropCopy) && rel >= quarter && !fired {
            let item_copy = |w: &redsim::world::World, id: u32| redsim::entity::index_of(w, id).is_some_and(|i| matches!(&w.entities.list[i].kind, EntityKind::Item(st) if kinds.contains(st.item.trim_start_matches("minecraft:"))));
            if let Some(&(_, _, copy)) = cr.iter().find(|(_, _, c)| item_copy(&run.world, *c) || run.world.nether.as_deref().is_some_and(|n| item_copy(n, *c))) {
                for wi in 0..2 {
                    side(&mut run, wi)?.entities.list.retain(|e| e.id != copy);
                }
                let gone = redsim::entity::index_of(&run.world, copy).is_none() && run.world.nether.as_deref().is_some_and(|n| redsim::entity::index_of(n, copy).is_none());
                s.push_str(&format!("  CONTROL drop-copy: at t {rel} the copy {copy} of a loop item's crossing taken out of its world's entity list; read back: {}\n", if gone { "in neither world" } else { "STILL THERE" }));
                if !gone {
                    return refusal(s, o.seed, "control drop-copy: the copy did not read back gone");
                }
                fired = true;
            }
        }
        let n = run.world.nether.as_deref().ok_or("no Nether world")?;
        if run.world.away.is_none() && n.away.is_none() {
            min_items = min_items.min(loop_items_in(&run.world, &kinds) + loop_items_in(n, &kinds));
        }
        for (_, from, copy) in cr.drain(..) {
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
    // (a timed control that never fired planted nothing: refused by name)
    if matches!(o.control, Some(Control::StallOverworldClock | Control::StealItem | Control::DoubleItem | Control::DropCopy | Control::UnseedDrops)) && !fired {
        let c = o.control.map_or("", |c| c.name());
        return refusal(s, o.seed, &format!("control {c}: nothing to plant from t {quarter} on: control planted nothing"));
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
    // (the items the settle's refused spits lost: G1 09:47 (3), each matched spit at a loader dropper
    // explains one item (a dropper spits one: redstone/container.rs 1319, `spawn_dispensed(.., 1, ..)`),
    // read back against the shortfall right after the start; any more missing FAILS by name; a matched
    // spit whose message the runner can't read, or at no loader dropper, FAILS by name and explains no
    // item, so its loss surfaces as unexplained too: ironspec 10:35a (4) (3))
    let (spit_items, unread) = settle_spit_items(&settle_spits, &spits, &parts[li].nether);
    let mut ok = true;
    if !unread.is_empty() {
        ok = false;
        s.push_str(&format!("  the settle's refused spits the runner can't credit: {}: FAIL\n", unread.join("; ")));
    }
    // (the world-open row: the open must have happened inside the run)
    match opened {
        Some(true) => {}
        Some(false) => ok = false,
        None => {
            ok = false;
            s.push_str(&format!("  the world open at t {}: never reached in {} gt: FAIL (the gap bar needs its world-open row)\n", o.world_open, o.ticks));
        }
    }
    let mut worst_gaps = [0i64; 2];
    let mut elsewhere_n = [0usize; 2];
    for (dim, own, label) in [(1usize, &lo_n.name, "into the Nether"), (0usize, &lo_ow.name, "into the overworld")] {
        let a = &arrivals[dim];
        let total: usize = a.values().sum();
        let elsewhere: usize = a.iter().filter(|(k, _)| *k != own).map(|(_, v)| v).sum();
        let mut gaps: Vec<i64> = times[dim].windows(2).map(|w| w[1] - w[0]).collect();
        if let Some(l) = times[dim].last() {
            gaps.push(end - l);
        }
        let worst = gaps.iter().copied().max().unwrap_or(o.ticks);
        worst_gaps[dim] = worst;
        elsewhere_n[dim] = elsewhere;
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
    let (kept, items_line) = items_bar(loop_items_sides(&parts[li]), at_start, spit_items, items_end, min_items);
    ok &= kept;
    s.push_str(&items_line);
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
        golems_at.map_or_else(|| format!("{} (never reached)", o.golem_at), |t| t.to_string()),
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
    // (G1 08:37 (1): a control whose own bar didn't fail is refused)
    if let Some(c) = o.control {
        let failed = match c {
            Control::StallOverworldClock => worst_gaps.iter().any(|g| *g > 300),
            Control::StopNetherClock => !launched,
            Control::StealItem | Control::DoubleItem => !kept,
            Control::DivertReturn => arrivals[0].get("planted.divert").copied().unwrap_or(0) > 0 && elsewhere_n[0] > 0,
            Control::PutOutTwin => gfail > 0,
            Control::DropCopy => !missing.is_empty(),
            Control::UnseedDrops => !unnamed.is_empty(),
        };
        if !failed {
            return refusal(s, o.seed, &format!("control {}: its own bar ({}) did not fail: control planted nothing (ironspec 08:37a, the standing rule: not a pass)", c.name(), c.target()));
        }
    }
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
            Verdict::Refused => "REFUSED".to_string(),
        }
    ));
    Ok((s, v))
}

/// A run's verdict: its bars met, a bar failed, every bar met with only named model limits (a run that
/// judges nothing: ironspec 02:45 (3), G1 06:37 (1)), or a control refused (it planted nothing or its
/// own bar didn't fail: G1 08:37 (1)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    NotModeled,
    Refused,
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
        assert!(model_limit("the light behaviour of minecraft:piston_head[facing=west,short=false,type=sticky] (its light dampening and occluding faces) is not modeled", &mut none).is_some());
        assert!(model_limit("the light behaviour of minecraft:sticky_piston[extended=false,facing=east] (its light dampening and occluding faces) is not modeled", &mut none).is_none());
        // (G1 08:37 N: anchored to the whole message, not a fragment inside another)
        assert!(model_limit("x: the light behaviour of minecraft:sticky_piston[extended=true,facing=east] (its light dampening and occluding faces) is not modeled", &mut none).is_none());
        assert!(model_limit("the light behaviour of minecraft:sticky_piston[extended=true,facing=east] (something else)", &mut none).is_none());
        assert!(model_limit("block \"nether_portal\" is not modeled by redsim", &mut none).is_none());
    }

    /// A loader (in tree) at `at` with rev 3 (in tree) at `copies`.
    fn layout_of(loader: &str, at: C, copies: &[C]) -> Vec<Laid> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |p: &str| mctas::design::Design::parse(&std::fs::read_to_string(root.join(p)).unwrap()).unwrap();
        let m = read("designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-record7f13b.txt");
        let l = read(&format!("designs/farms/iron/carousel-assembly/wip/loaderlay/{loader}"));
        let mut parts: Vec<Laid> = copies.iter().enumerate().map(|(k, o)| crate::loaderlayout::lay(&format!("copy{}", k + 1), "m", &m, *o, 64).unwrap()).collect();
        parts.push(crate::loaderlayout::lay("loader", "l", &l, at, 0).unwrap());
        parts
    }

    /// The loader of record at (25,80,19) with rev 3 at `copies`.
    fn layout_x8(copies: &[C]) -> Vec<Laid> {
        layout_of("loader-x8.txt", [25, 80, 19], copies)
    }

    fn opts(control: Option<Control>) -> Opts {
        Opts { ticks: 1200, seed: 1, golem_at: 100, world_open: 600, control }
    }

    /// The bar's line in a run's text: the line that starts with `key` (a header naming a bar never
    /// matches).
    fn line<'a>(t: &'a str, key: &str) -> &'a str {
        t.lines().find(|l| l.trim_start().starts_with(key)).unwrap_or_else(|| panic!("no line starting {key:?} in\n{t}"))
    }

    #[test]
    fn the_runner_passes_the_loader_of_record_and_each_control_fails_its_bar() {
        // (G1 06:37 (3), 08:37 (2): the base passes; each control, one planted change, fails its own bar)
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
        // (ironspec 08:07a (5): one item doubled: the items bar fails on the count; the loading holds)
        let (t, v) = run(&parts, &opts(Some(Control::DoubleItem))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "the items:").contains("4 laid, 5 at the end") && line(&t, "the items:").ends_with("FAIL") && line(&t, "CONTROL double-item:").contains("4 -> 5"), "{t}");
        assert!(line(&t, "item arrivals into the Nether").ends_with("PASS") && line(&t, "item arrivals into the overworld").ends_with("PASS") && line(&t, "launches (").ends_with("PASS"), "{t}");
        // (G1 08:37 (2): the golem bar's twin: copy1's twin put out, its golems cross elsewhere)
        let (t, v) = run(&parts, &opts(Some(Control::PutOutTwin))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "golems:").ends_with("FAIL") && !line(&t, "golems:").contains(" 0 at another portal") && line(&t, "CONTROL put-out-twin:").contains("read back: air"), "{t}");
        // (the missing-copy bar: a crossing's copy taken out of its world)
        let (t, v) = run(&parts, &opts(Some(Control::DropCopy))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "crossings whose copy is in neither world").ends_with("FAIL") && line(&t, "CONTROL drop-copy:").contains("in neither world"), "{t}");
        // (the other-world-errors bar: a dropper spit with the drop seed unset, after the settle)
        let (t, v) = run(&parts, &opts(Some(Control::UnseedDrops))).unwrap();
        assert!(v == Verdict::Fail && line(&t, "other world errors:").contains("spits an item into the world") && line(&t, "other world errors:").ends_with("FAIL"), "{t}");
        // (the golem bar: none placed, --golem-at past the run (game time reaches the tick count plus
        // the world open's idle ticks))
        let (t, v) = run(&parts, &Opts { golem_at: 5000, ..opts(None) }).unwrap();
        assert!(v == Verdict::Fail && line(&t, "golems:").contains("golems: 0 placed") && line(&t, "golems:").ends_with("FAIL"), "{t}");
        // (the world-open row: never reached)
        let (t, v) = run(&parts, &Opts { world_open: 5000, ..opts(None) }).unwrap();
        assert!(v == Verdict::Fail && line(&t, "the world open at t 5000").contains("never reached"), "{t}");
    }

    #[test]
    fn a_skipped_tick_never_drops_a_control_or_the_golems_and_a_control_that_fails_nothing_is_refused() {
        // (G1 08:37 (1), 09:47 N: a world open at t 298 advances game time 2 within its tick, so t 299
        // and t 300 (a quarter of 1200) are never ticks; the golems due at t 299 and the stall due at t
        // 300 fire in that same tick, logged at the game time they land at, t 300)
        let parts = layout_x8(&[[6, 80, 6]]);
        let (t, v) = run(&parts, &Opts { world_open: 298, golem_at: 299, ..opts(Some(Control::StallOverworldClock)) }).unwrap();
        assert!(v == Verdict::Fail && line(&t, "CONTROL stall-overworld-clock:").contains("at t 300") && line(&t, "golems:").contains("6 placed at t 300") && line(&t, "the world open at t 298").contains("game time t 300 after it"), "{t}");
        // (a stall too late for its gap to pass 300 gt: its own bar doesn't fail, so it is refused)
        let (t, v) = run(&parts, &Opts { ticks: 360, world_open: 180, golem_at: 30, ..opts(Some(Control::StallOverworldClock)) }).unwrap();
        assert!(v == Verdict::Refused && line(&t, "REFUSED:").contains("its own bar (the arrivals' worst gap) did not fail"), "{t}");
        // (G1 09:47 (2): a run that can't start is refused, never the FAIL code: a loader whose droppers
        // hold no item)
        let mut empty = layout_x8(&[[6, 80, 6]]);
        let l = empty.len() - 1;
        empty[l].block_entities.clear();
        empty[l].nether_block_entities.clear();
        let (t, v) = run(&empty, &opts(None)).unwrap();
        assert!(v == Verdict::Refused && line(&t, "REFUSED:").contains("droppers hold no item"), "{t}");
    }

    #[test]
    fn an_unreadable_settle_spit_fails_by_name_and_explains_no_item() {
        // (ironspec 10:35a (4) (3): the planted case: a refused spit whose message the runner can't read
        // is named, credits nothing, and its lost item surfaces as an unexplained loss)
        let mut nether: BTreeMap<C, String> = BTreeMap::new();
        nether.insert([3, 90, 1], "minecraft:dropper[facing=south,triggered=false]".into());
        let good = "dropper at (3,90,1) spits an item into the world: its motion draws from the level random".to_string();
        assert_eq!(settle_spit_items(&[good.clone()], &[], &nether), (1, Vec::<String>::new()));
        let garbled = "dropper at (3;90;1) spits an item into the world: its motion draws from the level random".to_string();
        let (items, unread) = settle_spit_items(&[garbled], &[], &nether);
        assert!(items == 0 && unread.len() == 1 && unread[0].contains("can't read") && unread[0].contains("(3;90;1)"), "{unread:?}");
        let (ok, l) = items_bar([3, 3], [3, 2], items, 5, 5);
        assert!(!ok && l.contains("1 more missing right after the start than the refused spits explain") && !l.contains("lost to"), "{l}");
        // (a readable spit at no loader dropper credits nothing either)
        let (items, unread) = settle_spit_items(&["dropper at (9,90,1) spits an item into the world: x".to_string()], &[], &nether);
        assert!(items == 0 && unread[0].contains("no dropper of the loader's Nether part"), "{unread:?}");
        // (a recorded spit whose error never came back is not counted at all)
        assert_eq!(settle_spit_items(&[good.clone()], &[good], &nether), (0, Vec::<String>::new()));
    }

    #[test]
    fn the_items_bar_excuses_only_what_the_refused_spits_explain() {
        // (G1 09:47 (3), ironspec 09:47a (3): each refused Nether dropper spit matched to one item of the
        // Nether side's shortfall; any loss beyond, on either side, or any surplus, FAILS by name)
        let (ok, l) = items_bar([3, 3], [3, 2], 1, 5, 5);
        assert!(ok && l.contains("1 lost to 1 refused dropper spit(s)") && l.contains("3 laid there, 2 right after the start") && l.ends_with("PASS\n"), "{l}");
        // (one more item gone on the Nether side than its one spit explains)
        let (ok, l) = items_bar([3, 3], [3, 1], 1, 4, 4);
        assert!(!ok && l.contains("1 lost to 1 refused dropper spit(s)") && l.contains("1 more missing right after the start than the refused spits explain") && l.ends_with("FAIL\n"), "{l}");
        // (a Nether spit never explains an overworld loss)
        let (ok, l) = items_bar([3, 3], [2, 3], 1, 5, 5);
        assert!(!ok && !l.contains("lost to") && l.contains("1 more missing"), "{l}");
        // (no spit recorded: a shortfall is never excused)
        let (ok, l) = items_bar([3, 3], [3, 2], 0, 5, 5);
        assert!(!ok && l.contains("1 more missing") && !l.contains("lost to"), "{l}");
        let (ok, l) = items_bar([3, 3], [3, 4], 0, 7, 7);
        assert!(!ok && l.contains("1 more right after the start than laid"), "{l}");
        let (ok, l) = items_bar([2, 2], [2, 2], 0, 4, 4);
        assert!(ok && l == "  the items: 4 laid, 4 at the end, at least 4 at every tick: PASS\n", "{l}");
    }

    #[test]
    fn the_clock_loader_is_not_modeled_and_a_control_on_it_still_fails() {
        // (G1 08:37 N: NOT MODELED through run(): loader-clock-x8 at the four copies, its named limits
        // only; a failing control on it fails, never NOT MODELED; a control on its absent observer pair
        // is refused)
        let parts = layout_of("loader-clock-x8.txt", [10, 85, -14], &[[6, 80, 6], [38, 80, 6], [6, 80, 38], [38, 80, 38]]);
        let o = Opts { ticks: 1200, seed: 1, golem_at: 100, world_open: 600, control: None };
        let (t, v) = run(&parts, &o).unwrap();
        assert!(v == Verdict::NotModeled && line(&t, "the items:").contains("1 lost to 1 refused dropper spit(s)") && line(&t, "the items:").ends_with("PASS") && line(&t, "other world errors:").ends_with("none"), "{t}");
        let (t, v) = run(&parts, &Opts { control: Some(Control::StealItem), ..o }).unwrap();
        assert!(v == Verdict::Fail && line(&t, "the items:").ends_with("FAIL"), "{t}");
        let (t, v) = run(&parts, &Opts { control: Some(Control::StopNetherClock), ..o }).unwrap();
        assert!(v == Verdict::Refused && line(&t, "REFUSED:").contains("has no observer pair"), "{t}");
    }
}
