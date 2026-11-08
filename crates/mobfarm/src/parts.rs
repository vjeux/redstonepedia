//! Parts from the repo's catalogs (vjeux 2026-11-08 10:48: "We built an entire clock system to
//! find the smallest one for each tick. Why are we not using it."): a farm's timing need is an
//! archived design in `designs/circuits/catalog.tsv` that meets it ([`timers`]: every one that
//! fits, fewest blocks first), turned and moved into the farm by its ports ([`place`]) and wired to
//! the farm by dust ([`route`]). The farm optimizer's grammar calls these.

use crate::layouts::Builder;
use anyhow::{Context, Result, anyhow};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

/// The catalog of every archived circuit design.
pub const CATALOG: &str = "designs/circuits/catalog.tsv";

/// A repo path: from the working directory (mctas runs at the repo's root), else from this
/// crate's (the tests run in it).
fn repo_path(rel: &str) -> std::path::PathBuf {
    let here = Path::new(rel);
    if here.exists() { here.to_path_buf() } else { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel) }
}

/// One row of the catalog.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub study: String,
    pub kind: String,
    pub axis: String,
    pub name: String,
    /// The design file, under `designs/circuits/`.
    pub path: String,
    pub metrics: String,
    pub blocks: u32,
    pub dims: String,
    /// The block ids it is made of (`comparator+redstone_wire`).
    pub family: String,
    pub robustness: String,
}

impl Row {
    /// A metric's first number: `L 24 gt` -> 24, `min-pulse 2 gt (...)` -> 2.
    pub fn metric(&self, key: &str) -> Option<f64> {
        self.metrics.split(", ").find_map(|m| m.strip_prefix(key).and_then(|v| v.trim().split_whitespace().next()).and_then(|v| v.parse::<f64>().ok()))
    }

    /// The box's volume (`5x3x3` -> 45).
    pub fn volume(&self) -> u32 {
        self.dims.split('x').filter_map(|d| d.parse::<u32>().ok()).product()
    }

    /// The block ids it is made of.
    pub fn blocks_of(&self) -> Vec<&str> {
        self.family.split('+').filter(|s| !s.is_empty()).collect()
    }
}

/// The catalog's rows.
pub fn catalog() -> Result<Vec<Row>> {
    let text = std::fs::read_to_string(repo_path(CATALOG)).with_context(|| format!("reading {CATALOG}"))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let cols = ["study", "kind", "axis", "name", "path", "metrics", "blocks", "dims", "family", "robustness"].map(col);
    let get = |f: &[&str], c: Option<usize>| c.and_then(|c| f.get(c)).map(|s| s.to_string()).unwrap_or_default();
    Ok(lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Row {
                study: get(&f, cols[0]),
                kind: get(&f, cols[1]),
                axis: get(&f, cols[2]),
                name: get(&f, cols[3]),
                path: get(&f, cols[4]),
                metrics: get(&f, cols[5]),
                blocks: get(&f, cols[6]).parse().unwrap_or(u32::MAX),
                dims: get(&f, cols[7]),
                family: get(&f, cols[8]),
                robustness: get(&f, cols[9]),
            }
        })
        .collect())
}

/// The stage of the game a farm is built in (designs/briefs/farm-iron.md: its materials define
/// it). Day 1: no Nether item, no slime; redstone (dust, repeaters, torches: an iron pickaxe and
/// redstone ore) only where `redstone` allows it. The Nether: quartz (observers, comparators,
/// daylight detectors), blaze rods (copper bulbs), glowstone. The End: anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Day1 { redstone: bool },
    Nether,
    End,
}

/// The earliest stage a block id can be had in: (needs redstone dust, 0 = day 1 / 1 = the Nether
/// or slime / 2 = the End).
pub fn block_stage(id: &str) -> (bool, u8) {
    let redstone = ["redstone", "repeater", "piston", "observer", "comparator", "dropper", "dispenser", "note_block", "powered_rail", "detector_rail", "activator_rail", "crafter", "daylight_detector", "target", "calibrated_sculk_sensor", "lamp"].iter().any(|k| id.contains(k));
    let tier = if ["observer", "comparator", "daylight_detector", "copper_bulb", "redstone_lamp", "soul_sand", "brewing_stand", "slime", "sticky_piston", "honey", "sculk", "magma", "quartz", "glowstone", "blaze"].iter().any(|k| id.contains(k)) {
        1
    } else if ["shulker", "end_rod", "elytra", "chorus", "purpur", "dragon"].iter().any(|k| id.contains(k)) {
        2
    } else {
        0
    };
    (redstone, tier)
}

/// Whether every block of `row` can be had at `stage`.
pub fn allowed(row: &Row, stage: Stage) -> bool {
    row.blocks_of().iter().all(|id| {
        let (redstone, tier) = block_stage(id);
        match stage {
            Stage::Day1 { redstone: r } => tier == 0 && (r || !redstone),
            Stage::Nether => tier <= 1,
            Stage::End => true,
        }
    })
}

/// A timing part for a pulse: the part, the repeater that first stretches the input to the
/// part's `min-pulse` (its delay in redstone ticks; none: the input is long enough), the output
/// pulse's length in gt for the input given, and the blocks with the stretcher (a repeater and the
/// stone under it).
#[derive(Clone, Debug)]
pub struct Timer<'a> {
    pub row: &'a Row,
    pub stretch: Option<u8>,
    pub length: u32,
    pub blocks: u32,
}

/// Every robust part that turns an input pulse of `input` gt into an output pulse of `l_min`
/// gt or more (`l_max` at most) with `stage`'s materials: pulse extenders (the output lasts `L`)
/// and off-delays (the input's length plus `L`), each with a repeater first when the input is
/// shorter than its `min-pulse` (the delay the stretch needs, at most 4 redstone ticks); none
/// that hears vibrations (villagers and golems make them). Fewest blocks first, then the smallest
/// box, then the shortest pulse.
pub fn timers(rows: &[Row], l_min: u32, l_max: u32, input: u32, stage: Stage) -> Vec<Timer<'_>> {
    let mut out: Vec<Timer> = rows
        .iter()
        .filter(|r| matches!(r.kind.as_str(), "pulse-extender" | "off-delay") && r.robustness == "robust" && !r.axis.contains("vibration") && !r.family.contains("sculk") && allowed(r, stage))
        .filter_map(|r| {
            let (l, min) = (r.metric("L")? as u32, r.metric("min-pulse")? as u32);
            let (stretch, pulse) = if input >= min {
                (None, input)
            } else {
                let d = min.div_ceil(2) as u8;
                if d > 4 || (matches!(stage, Stage::Day1 { redstone: false })) {
                    return None;
                }
                (Some(d), 2 * d as u32)
            };
            let length = if r.kind == "off-delay" { pulse + l } else { l };
            (length >= l_min && length <= l_max).then_some(Timer { row: r, stretch, length, blocks: r.blocks + if stretch.is_some() { 2 } else { 0 } })
        })
        .collect();
    out.sort_by(|a, b| (a.blocks, a.row.volume(), a.length).cmp(&(b.blocks, b.row.volume(), b.length)));
    out
}

/// A part placed in a farm: its input and output cells in the farm's coordinates and every cell
/// it fills.
#[derive(Clone, Debug)]
pub struct Placed {
    pub name: String,
    pub input: [i32; 3],
    pub output: [i32; 3],
    pub cells: BTreeSet<[i32; 3]>,
}

/// Place `row`'s design turned `turns` quarter turns clockwise (seen from above, the judge's
/// turn) with its input cell at `input_at`: its blocks and block entities go into `b`. Its input
/// cell holds what the judge's driven input holds (a stone: the caller powers it with a repeater
/// pointing into it, as the judge does); its output cell is the judge's consumer's (a block that
/// reads the output goes there).
pub fn place(b: &mut Builder, row: &Row, turns: u8, input_at: [i32; 3]) -> Result<Placed> {
    let path = repo_path(&format!("designs/circuits/{}", row.path));
    let c = circuit::Circuit::load(&path)?.turned_pure(turns)?;
    let d = &c.clock.design;
    let input = *d.ports.get("input").ok_or_else(|| anyhow!("{}: no input port", row.name))?;
    let output = *d.ports.get("output").ok_or_else(|| anyhow!("{}: no output port", row.name))?;
    let at = |p: [i32; 3]| [p[0] - input[0] + input_at[0], p[1] - input[1] + input_at[1], p[2] - input[2] + input_at[2]];
    let mut cells = BTreeSet::new();
    for (p, s) in &d.blocks {
        let q = at(*p);
        b.set(q[0], q[1], q[2], s);
        cells.insert(q);
    }
    for (p, s) in &d.block_entities {
        b.d.block_entities.insert(at(*p), s.clone());
    }
    if !d.blocks.contains_key(&input) {
        b.set(input_at[0], input_at[1], input_at[2], "stone");
        cells.insert(input_at);
    }
    Ok(Placed { name: row.name.clone(), input: input_at, output: at(output), cells })
}

/// Cover every golem spawn spot of the farm in `b` that `keep` does not name (a part's top, a
/// support: stray spots a golem could spawn on and walk off) with glass in its feet cell
/// (LEGACY_IRON_GOLEM excludes glass: nothing new spawns on it). Returns the glass placed.
pub fn cap_spawns(b: &mut Builder, keep: &dyn Fn([i32; 3]) -> bool) -> Result<u32> {
    use redsim::entity::mob::farm::{RunOpts, WalkMode, build};
    let mut placed = 0;
    for _ in 0..3 {
        let c = circuit::Circuit::from_design(b.d.clone())?;
        let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
        let w = build(&spec, &RunOpts::steady(1, 0, WalkMode::Random)).map_err(|e| anyhow!(e))?;
        let stray: Vec<[i32; 3]> = redsim::entity::mob::exit::spawn_spots(&w).into_iter().map(|p| [p.x, p.y, p.z]).filter(|p| !keep(*p)).collect();
        if stray.is_empty() {
            break;
        }
        for p in stray {
            b.set(p[0], p[1], p[2], "glass");
            placed += 1;
        }
    }
    Ok(placed)
}

/// A block that takes or gives redstone power (a dust beside it would feed it or be fed).
pub fn active(state: &str) -> bool {
    ["repeater", "comparator", "observer", "redstone", "torch", "piston", "bulb", "lever", "button", "trapdoor", "hopper", "lamp", "door", "rail", "plate", "sculk", "target", "dropper", "dispenser", "crafter", "note_block", "bell", "tnt"].iter().any(|k| state.contains(k))
}

/// The six cells next to `p`.
fn near(p: [i32; 3]) -> [[i32; 3]; 6] {
    [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]].map(|d| [p[0] + d[0], p[1] + d[1], p[2] + d[2]])
}

/// Dust from `start` (the cell a component's output points into: the first dust) to `goal` (the
/// last dust), on `start`'s plane, never on a cell `forbid` names, each dust on a stone (put
/// under it), the signal still at least `tail + 1` at the goal (dust after it). No dust beside a
/// cell of `keep_clear` or an active block ([`active`]), but the cells `allow` names (the source
/// and the sink). A repeater replaces a straight dust where the signal would end. Returns the
/// path (dust and repeaters), or None when no path exists.
pub fn route(b: &mut Builder, start: [i32; 3], goal: [i32; 3], forbid: &dyn Fn([i32; 3]) -> bool, keep_clear: &BTreeSet<[i32; 3]>, allow: &[[i32; 3]], tail: u32) -> Option<Vec<[i32; 3]>> {
    let y = start[1];
    let usable = |b: &Builder, p: [i32; 3]| {
        if forbid(p) || b.d.blocks.contains_key(&p) {
            return false;
        }
        let under = [p[0], p[1] - 1, p[2]];
        if let Some(s) = b.d.blocks.get(&under)
            && (active(s) || keep_clear.contains(&under))
        {
            return false;
        }
        near(p).iter().all(|q| allow.contains(q) || (!keep_clear.contains(q) && !b.d.blocks.get(q).is_some_and(|s| active(s))))
    };
    if !usable(b, start) || !usable(b, goal) {
        return None;
    }
    let mut prev: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
    let mut queue = VecDeque::from([start]);
    prev.insert(start, start);
    while let Some(p) = queue.pop_front() {
        if p == goal {
            break;
        }
        for q in near(p) {
            if q[1] != y || prev.contains_key(&q) || !usable(b, q) {
                continue;
            }
            prev.insert(q, p);
            queue.push_back(q);
        }
    }
    prev.get(&goal)?;
    let mut path = vec![goal];
    while *path.last().unwrap() != start {
        path.push(prev[path.last().unwrap()]);
    }
    path.reverse();
    let dir = |a: [i32; 3], c: [i32; 3]| match (c[0] - a[0], c[2] - a[2]) {
        (1, _) => "east",
        (-1, _) => "west",
        (_, 1) => "south",
        _ => "north",
    };
    let straight = |k: usize| k > 0 && k + 1 < path.len() && dir(path[k - 1], path[k]) == dir(path[k], path[k + 1]);
    // repeaters: walking from the start at 15, one on the last straight cell before the signal
    // would reach 0 (or fall under tail + 1 at the goal)
    let mut repeaters: BTreeSet<usize> = BTreeSet::new();
    let mut last_rep = 0usize;
    let mut k = 0usize;
    while k < path.len() {
        let left = path.len() - 1 - k;
        let strength = 15i32 - (k - last_rep) as i32;
        let need = if left == 0 { tail as i32 + 1 } else { 1 };
        if strength < need {
            // back up to the last straight cell after the previous repeater
            let mut j = k;
            while j > last_rep && !straight(j) {
                j -= 1;
            }
            if j == last_rep {
                return None;
            }
            repeaters.insert(j);
            last_rep = j;
            k = j + 1;
            continue;
        }
        k += 1;
    }
    for (k, p) in path.iter().enumerate() {
        b.set(p[0], p[1] - 1, p[2], "stone");
        if repeaters.contains(&k) {
            // `facing` names its input side: the cell before it
            b.set(p[0], p[1], p[2], &format!("repeater[delay=1,facing={},locked=false,powered=false]", dir(path[k], path[k - 1])));
            continue;
        }
        let mut sides: BTreeMap<&str, &str> = ["east", "north", "south", "west"].iter().map(|d| (*d, "none")).collect();
        if k > 0 {
            sides.insert(dir(*p, path[k - 1]), "side");
        }
        if let Some(q) = path.get(k + 1) {
            sides.insert(dir(*p, *q), "side");
        }
        b.set(p[0], p[1], p[2], &format!("redstone_wire[east={},north={},power=0,south={},west={}]", sides["east"], sides["north"], sides["south"], sides["west"]));
    }
    Some(path)
}

/// The judge's consumer cell of a placed part, read by a repeater: the repeater in the output
/// cell (air in the design) faces the part's one block beside it (its input), so the signal
/// leaves the other way. Returns the cell its output points into, or None when the output cell
/// is a block or has no single part block beside it on its plane.
pub fn read_output(b: &mut Builder, p: &Placed) -> Option<[i32; 3]> {
    let o = p.output;
    if b.d.blocks.contains_key(&o) {
        return None;
    }
    let sides = [("east", [1, 0, 0]), ("west", [-1, 0, 0]), ("south", [0, 0, 1]), ("north", [0, 0, -1])];
    let beside: Vec<(&str, [i32; 3])> = sides.iter().filter(|(_, d)| p.cells.contains(&[o[0] + d[0], o[1], o[2] + d[2]])).copied().collect();
    let [(facing, d)] = beside.as_slice() else { return None };
    b.set(o[0], o[1] - 1, o[2], "stone");
    b.set(o[0], o[1], o[2], &format!("repeater[delay=1,facing={facing},locked=false,powered=false]"));
    Some([o[0] - d[0], o[1], o[2] - d[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M2's hide: 20 gt or more from the bed observer's 2 gt pulse (VillagerHostilesSensor runs
    /// every 20 gt). The Nether's smallest is the comparator ring L24 behind a stretching repeater
    /// (4 + 24 gt); day 1 without redstone has none.
    #[test]
    fn the_hide_is_a_catalog_part() {
        let rows = catalog().unwrap();
        let nether = timers(&rows, 20, 400, 2, Stage::Nether);
        let first = nether.first().expect("a Nether part");
        assert_eq!((first.row.name.as_str(), first.stretch, first.length, first.blocks), ("off-delay-ring-L24", Some(2), 28, 15), "{first:?}");
        assert!(nether.windows(2).all(|w| w[0].blocks <= w[1].blocks));
        assert!(timers(&rows, 20, 400, 2, Stage::Day1 { redstone: false }).is_empty());
        assert!(timers(&rows, 20, 400, 2, Stage::Day1 { redstone: true }).iter().all(|t| allowed(t.row, Stage::Day1 { redstone: true })));
        let ring = rows.iter().find(|r| r.name == "off-delay-ring-L24").unwrap();
        assert!(!allowed(ring, Stage::Day1 { redstone: true }) && allowed(ring, Stage::Nether));
    }
}
