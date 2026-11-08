//! The archive and the catalog of the contraption studies. `mctas circuit archive` judges a design
//! in the real game and keeps it in `designs/circuits/<study>/archive/` (the design with its judge
//! line, a row of `verdicts.tsv`, a caption in `captions.tsv`); `mctas circuit catalog` writes, per
//! study, `CATALOG.md` (by the study's axis, smallest first; a design with no fewer blocks than
//! another with the same axis value and no better on any variation, whatever its block kinds, is a
//! variant, listed on `variants.md`), the index
//! `designs/circuits/CATALOG.md`, and the data the website renders: `designs/circuits/catalog.tsv`.

use crate::check::Outcome;
use crate::verify::{self, Options};
use crate::{Circuit, Kind};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::Pool;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const VERDICTS: &str = "verdicts.tsv";
pub const CAPTIONS: &str = "captions.tsv";
const HEADER: &str = "archived\tfile\tkind\taxis\tmetrics\tblocks\tdims\tvolume\tfamily\trobustness\tkinds\ttype_key\thash\tsummary";

/// The studies older than the registry, in order, with their titles (frozen: a new study registers
/// itself, crate::registry; [`studies`] is the whole list; the composter and fuel factory studies
/// predate it on explore/sorter).
pub const STUDIES: [(&str, &str); 26] = [
    ("pulse", "Pulse circuits"),
    ("tff", "T flip-flops"),
    ("delay", "Delay lines"),
    ("gates", "Logic gates"),
    ("memory", "Memory"),
    ("pext", "Piston extenders"),
    ("fly", "Flying machines"),
    ("shulker", "Shulker loaders and unloaders"),
    ("tnt", "TNT dupers"),
    ("bud", "BUD switches"),
    ("pulsemult", "Pulse multipliers, dividers and monostables"),
    ("adder", "Adders"),
    ("compute", "Computational redstone"),
    ("sorter", "Item sorters"),
    ("lock", "Combination locks"),
    ("display", "Seven-segment display decoders"),
    ("random", "Randomizers"),
    ("smelter", "Auto-smelters"),
    ("farm", "Crop farms"),
    ("rail", "Minecart stations"),
    ("crafter", "Auto-crafters"),
    ("brew", "Auto-brewers"),
    ("compost", "Auto-composters"),
    ("factory", "Fuel factories"),
    ("elevator-items", "Item elevators"),
    ("autodoor", "Automatic doors"),
];

/// Every study, in order, with its title: [`STUDIES`], then the registered studies not among them
/// (crate::registry, in its order; a registered study's title wins).
pub fn studies() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = STUDIES.to_vec();
    for s in crate::registry::all_studies() {
        match v.iter_mut().find(|(id, _)| *id == s.id) {
            Some(e) => e.1 = s.title,
            None => v.push((s.id, s.title)),
        }
    }
    v
}

/// One archived circuit (a row of `verdicts.tsv`).
#[derive(Clone, Debug)]
pub struct Row {
    pub archived: String,
    pub file: String,
    pub kind: String,
    /// The study's axis value (`L = 4 gt`, `xor`, `N = 3`...).
    pub axis: String,
    pub metrics: String,
    pub blocks: usize,
    pub dims: String,
    pub volume: i64,
    /// The kinds of the blocks whose state changes during the run.
    pub family: String,
    pub robustness: String,
    /// The block kinds it is built with (`+`-joined), for the dominance fold.
    pub kinds: String,
    pub type_key: String,
    pub hash: String,
    pub summary: String,
}

impl Row {
    fn to_tsv(&self) -> String {
        [
            self.archived.clone(),
            self.file.clone(),
            self.kind.clone(),
            self.axis.clone(),
            self.metrics.clone(),
            self.blocks.to_string(),
            self.dims.clone(),
            self.volume.to_string(),
            self.family.clone(),
            self.robustness.clone(),
            self.kinds.clone(),
            self.type_key.clone(),
            self.hash.clone(),
            self.summary.clone(),
        ]
        .map(|s| s.replace(['\t', '\n'], " "))
        .join("\t")
    }

    fn parse(line: &str) -> Option<Row> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 14 {
            return None;
        }
        Some(Row {
            archived: f[0].into(),
            file: f[1].into(),
            kind: f[2].into(),
            axis: f[3].into(),
            metrics: f[4].into(),
            blocks: f[5].parse().ok()?,
            dims: f[6].into(),
            volume: f[7].parse().unwrap_or(0),
            family: f[8].into(),
            robustness: f[9].into(),
            kinds: f[10].into(),
            type_key: f[11].into(),
            hash: f[12].into(),
            summary: f[13].into(),
        })
    }

    fn kind_set(&self) -> BTreeSet<String> {
        self.kinds.split('+').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
    }
}

/// A study's archive directory.
pub fn archive_dir(root: &Path, study: &str) -> PathBuf {
    root.join(study).join("archive")
}

/// The rows of an archive, one per file (a repeated file keeps its newest row).
pub fn read_rows(dir: &Path) -> Result<Vec<Row>> {
    let p = dir.join(VERDICTS);
    if !p.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    // one row per file: a union merge can keep two versions of an updated row; the one with the
    // most columns, then the newest `archived` stamp, then the last (clock::tables)
    let t = clock::tables::Table { path: p.clone(), key: vec![1], stamp: Some(0), ..Default::default() };
    let (one, _, _) = clock::tables::dedupe(&t, &text, None);
    Ok(one.lines().skip(1).filter_map(Row::parse).collect())
}

fn write_rows(dir: &Path, rows: &[Row]) -> Result<()> {
    let mut all = format!("{HEADER}\n");
    for r in rows {
        all.push_str(&r.to_tsv());
        all.push('\n');
    }
    std::fs::write(dir.join(VERDICTS), all)?;
    Ok(())
}

pub fn read_captions(dir: &Path) -> BTreeMap<String, String> {
    // one caption per file (clock::tables: the most columns, then the last)
    let p = dir.join(CAPTIONS);
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let t = clock::tables::Table { path: p, key: vec![0], ..Default::default() };
    let (one, _, _) = clock::tables::dedupe(&t, &text, None);
    one.lines()
        .skip(1)
        .filter_map(|l| l.split_once('\t'))
        .map(|(f, c)| (f.to_string(), c.to_string()))
        .collect()
}

/// Whether design `b` keeps its own card over `a` (same kind and axis value): `b` does better than
/// `a` on a variation a builder would choose on (coordinator 2026-10-20). The dominance fold folds
/// a design under any other with no more blocks that it does not beat this way, whatever its
/// block palette (vjeux 2026-10-20: palette alone is never a distinction), and a design that is
/// not robust under a robust one ([`clock::catalog::fold_hosts_robust`]).
/// Shulker kinds: the flow (`items/s`, higher), the badges (one `a` lacks) and `hoppers awake`
/// (lag, fewer). BUD switches: a denser `tileable` pitch (or tileable where the host is not),
/// `latency`, `re-arm`, `footprint` (lower) and no `jams`; a `fragile` one never wins. TNT dupers:
/// precise, launched, dropped, stationary. Every other kind ([`beats_generic`]): a lower latency /
/// delay / min-pulse / min-period / re-arm / extend / retract..., a higher speed, a denser
/// `tileable` pitch (or tileable at all), a different value of a metric with no better direction
/// (an edge, a class, an output shape, a pulse length), no glitches where `a` has some, flat where
/// `a` is not. Never a distinction: the materials (slime-free...), a lower box that is not flat.
pub fn beats_on_variation(b: &Row, a: &Row) -> bool {
    let get = |r: &Row, k: &str| r.metrics.split(", ").find_map(|m| m.strip_prefix(k).map(|v| v.trim().to_string()));
    let num = |r: &Row, k: &str| get(r, k).and_then(|v| v.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()));
    // a BUD (same detected classes): faster, re-arms sooner, a smaller footprint, or does not jam
    // where the host does (study 10)
    if b.kind == "bud" && a.kind == "bud" {
        if get(b, "fragile ").is_some() {
            return false;
        }
        let less = |k: &str| matches!((num(b, k), num(a, k)), (Some(x), Some(y)) if x < y);
        // tiles denser (or at all, where the host does not)
        let tile = |r: &Row| num(r, "tileable every ");
        if tile(b).is_some_and(|tb| tile(a).is_none_or(|ta| tb < ta)) {
            return true;
        }
        return less("latency ") || less("re-arm ") || less("footprint ") || (get(b, "jams ").is_none() && get(a, "jams ").is_some());
    }
    // TNT dupers (study 9): a PRECISE or SURVIVABLE duper (every TNT at one point at fuse 1; intact
    // under its own explosions) keeps its own card over a smaller one that is not (slime-free is a
    // material restriction, no distinction: coordinator 2026-10-20)
    if b.kind.starts_with("tnt-duper") && a.kind.starts_with("tnt-duper") {
        let has = |r: &Row, k: &str| r.summary.contains(k);
        return ["(precise", "launched at", "dropped:", "stationary:"].iter().any(|k| has(b, k) && !has(a, k));
    }
    if !(b.kind.starts_with("shulker-") && a.kind.starts_with("shulker-")) {
        return beats_generic(b, a);
    }
    if let (Some(fb), Some(fa)) = (num(b, "items/s "), num(a, "items/s ")) {
        if fb > fa * 1.01 {
            return true;
        }
    }
    let badges = |r: &Row| -> BTreeSet<String> { get(r, "badges ").map(|s| s.split('+').filter(|x| *x != "none" && !x.starts_with("tileable(")).map(String::from).collect()).unwrap_or_default() };
    if !badges(b).is_subset(&badges(a)) {
        return true;
    }
    // `tileable(N)`: copies every N blocks; a denser pitch (or tileable at all) is better
    let pitch = |r: &Row| get(r, "badges ").and_then(|s| s.split('+').find_map(|x| x.strip_prefix("tileable(").and_then(|v| v.trim_end_matches(')').parse::<u32>().ok())));
    if pitch(b).is_some_and(|pb| pitch(a).is_none_or(|pa| pb < pa)) {
        return true;
    }
    matches!((num(b, "hoppers awake "), num(a, "hoppers awake ")), (Some(hb), Some(ha)) if hb < ha)
}

/// What two designs of one kind and value must share to be equivalent layouts: their metrics and
/// flat or not (their block kinds may differ: palette alone is no distinction).
pub fn variation_signature(r: &Row) -> String {
    let flat = r.dims.split('x').nth(1).and_then(|h| h.parse::<i32>().ok()).is_some_and(|h| h <= 2);
    format!("{}|flat {flat}", r.metrics)
}

/// Whether a design passed in every build (else it folds under a robust one of its value).
pub fn is_robust(r: &Row) -> bool {
    r.robustness == "robust"
}

/// A crop harvester's blocks per cane in tenths (its `per cane 6.5 blocks` metric), for the fold.
pub fn per_cane_tenths(r: &Row) -> Option<usize> {
    let i = r.metrics.find("per cane ")?;
    let v: f64 = r.metrics[i + 9..].split_whitespace().next()?.parse().ok()?;
    Some((v * 10.0).round() as usize)
}

/// A crop harvester's rank for the fold: its EFFICIENCY (`items per hour per block`, vjeux
/// 2026-10-21), highest first, as a size (smaller ranks first); None before the metric existed.
pub fn farm_rank(r: &Row) -> Option<usize> {
    let key = "items per hour per block ";
    let i = r.metrics.find(key)?;
    let v: f64 = r.metrics[i + key.len()..].split_whitespace().next()?.trim_end_matches(',').parse().ok()?;
    (v > 0.0).then(|| (1_000_000.0 / v) as usize)
}

/// A design's input class for the fold: `own levers` / `own buttons` when its inputs are its own
/// devices, `signal` otherwise (inputs from the rest of a build: the judge's repeater line, R7),
/// empty for designs without inputs or that do not load.
pub fn input_class(path: &Path) -> String {
    let Ok(c) = Circuit::load(path) else { return String::new() };
    if c.inputs.is_empty() {
        return String::new();
    }
    let mut v: Vec<&str> = c
        .inputs
        .iter()
        .map(|i| match i.how {
            crate::InputHow::Lever => "own levers",
            crate::InputHow::Button => "own buttons",
            _ => "signal",
        })
        .collect();
    v.sort();
    v.dedup();
    v.join("+")
}

/// Metrics where less is better (a speed, a reach of input lengths or clocks, a footprint).
const LOWER_BETTER: [&str; 12] = ["latency", "delay", "min-pulse", "min-gap", "min-period", "re-arm", "repeat", "extend", "retract", "setup", "hold", "stop"];
/// Metrics where more is better.
const HIGHER_BETTER: [&str; 1] = ["speed"];
/// Metrics present only when something goes wrong.
const WORSE_IF_PRESENT: [&str; 3] = ["glitches", "jams", "fragile"];

/// [`beats_on_variation`] for the kinds without their own rule: every metric of `b` against `a`'s.
pub fn beats_generic(b: &Row, a: &Row) -> bool {
    let parse = |r: &Row| -> BTreeMap<String, String> {
        r.metrics
            .split(", ")
            .filter_map(|m| {
                let m = m.trim();
                let (k, v) = m.split_once(' ').unwrap_or((m, ""));
                (!k.is_empty()).then(|| (k.to_string(), v.trim().to_string()))
            })
            .collect()
    };
    let (mb, ma) = (parse(b), parse(a));
    // `over 30 gt` (a period the judge could not bound) reads as just above 30: a lower-is-better
    // metric's "over N" is worse than any value <= N, so it is no distinct property (it kept
    // tff-crafter7, which loses every number to tff-bulb, on its own card: critique)
    let first = |v: &str| {
        let (over, v) = match v.strip_prefix("over ") {
            Some(rest) => (0.5, rest),
            None => (0.0, v),
        };
        v.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()).map(|x| x + over)
    };
    for (k, vb) in &mb {
        if WORSE_IF_PRESENT.contains(&k.as_str()) {
            continue;
        }
        let Some(va) = ma.get(k) else { return true };
        if vb == va {
            continue;
        }
        // `tileable every N (dx dy dz)`: a denser pitch is better
        if k == "tileable" {
            let pitch = |v: &str| v.split_whitespace().nth(1).and_then(|x| x.parse::<i32>().ok());
            match (pitch(vb), pitch(va)) {
                (Some(x), Some(y)) if x >= y => continue,
                _ => return true,
            }
        }
        match (first(vb), first(va)) {
            (Some(x), Some(y)) if LOWER_BETTER.contains(&k.as_str()) => {
                if x < y {
                    return true;
                }
            }
            (Some(x), Some(y)) if HIGHER_BETTER.contains(&k.as_str()) => {
                if x > y {
                    return true;
                }
            }
            // a class, an edge, an output shape, a pulse length or a range: another behaviour
            _ => return true,
        }
    }
    if WORSE_IF_PRESENT.iter().any(|k| ma.contains_key(*k) && !mb.contains_key(*k)) {
        return true;
    }
    let height = |r: &Row| r.dims.split('x').nth(1).and_then(|h| h.parse::<i32>().ok());
    let flat = |r: &Row| height(r).is_some_and(|h| h <= 2);
    flat(b) && !flat(a)
}

/// After a union merge of an archive (both sides' rows kept): one verdict row per file (the
/// newest, as [`read_rows`] reads them; `rows` without the `stale` ones) and one caption per file
/// (the last; none for a stale file), written back when anything changes.
fn tidy(dir: &Path, rows: &[Row], stale: &[String]) -> Result<()> {
    let text = std::fs::read_to_string(dir.join(VERDICTS)).unwrap_or_default();
    if text.lines().skip(1).filter(|l| !l.trim().is_empty()).count() != rows.len() {
        write_rows(dir, rows)?;
    }
    let caps = std::fs::read_to_string(dir.join(CAPTIONS)).unwrap_or_default();
    let mut order: Vec<String> = Vec::new();
    let mut last: BTreeMap<String, String> = BTreeMap::new();
    for l in caps.lines().skip(1) {
        if let Some((f, c)) = l.split_once('\t') {
            if !last.contains_key(f) {
                order.push(f.to_string());
            }
            last.insert(f.to_string(), c.to_string());
        }
    }
    let n = order.len();
    order.retain(|f| !stale.contains(f));
    if order.len() != n || n != caps.lines().skip(1).filter(|l| l.contains('\t')).count() {
        let mut out = String::from("file\tcaption\n");
        for f in &order {
            out.push_str(&format!("{f}\t{}\n", last[f]));
        }
        std::fs::write(dir.join(CAPTIONS), out)?;
    }
    Ok(())
}

/// Take a design out of its study's archive (`<study>/<file>`, e.g. a design another one now
/// dominates): the file, its verdict row and its caption.
pub fn unarchive(root: &Path, spec: &str) -> Result<String> {
    let (study, file) = spec.split_once('/').ok_or_else(|| anyhow::anyhow!("--remove {spec}: want <study>/<file>"))?;
    let dir = archive_dir(root, study);
    let rows = read_rows(&dir)?;
    if !rows.iter().any(|r| r.file == file) {
        anyhow::bail!("--remove {spec}: no such row in {}", dir.join(VERDICTS).display());
    }
    write_rows(&dir, &rows.into_iter().filter(|r| r.file != file).collect::<Vec<_>>())?;
    let caps: String = std::fs::read_to_string(dir.join(CAPTIONS)).unwrap_or_else(|_| "file\tcaption\n".into()).lines().filter(|l| l.split('\t').next() != Some(file)).map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join(CAPTIONS), caps)?;
    let _ = std::fs::remove_file(dir.join(file));
    Ok(format!("REMOVED {}", dir.join(file).display()))
}

/// The design the type key is computed on: every swap (sticky pistons -> pistons, inert solids ->
/// stone: `clock::ident`) whose run on redsim under the kind's stimulus is identical modulo the swap
/// (every cell's full state at every frame); the circuit itself when redsim does not model it.
pub fn normalized(c: &Circuit) -> Circuit {
    let Some(kind) = c.kind.clone() else {
        return c.clone();
    };
    // a station's carts (riders) are not modeled by redsim: the design itself
    if matches!(kind, Kind::Station(_) | Kind::CartUnloader | Kind::CartLoader | Kind::AutoDoor) {
        return c.clone();
    }
    let gap = verify::sizing(c, 3).unwrap_or(crate::check::Sizing::default().gap);
    let s = crate::check::stimulus(c, &kind, &crate::check::Sizing { gap });
    let Ok(mut base) = crate::run::sim_trace(c, &s, 3) else {
        return c.clone();
    };
    let mut cur = c.clone();
    let swaps: [&dyn Fn(&str) -> String; 2] = [&clock::ident::unstick, &clock::ident::unsolid];
    for f in swaps {
        let mut d = cur.clock.design.clone();
        let mut changed = false;
        for st in d.blocks.values_mut() {
            let n = f(st);
            if n != *st {
                *st = n;
                changed = true;
            }
        }
        let Some(c2) = changed.then(|| Circuit::from_design(d).ok()).flatten() else {
            continue;
        };
        let Ok(t2) = crate::run::sim_trace(&c2, &s, 3) else {
            continue;
        };
        let same = base.frames() == t2.frames()
            && base.cells.iter().zip(&t2.cells).all(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|((p, x), (q, y))| p == q && f(&base.names[*x as usize]) == f(&t2.names[*y as usize])));
        if same {
            cur = c2;
            base = t2;
        }
    }
    cur
}

/// The structure's TUNABLE key: the design with every repeater's delay and every comparator's mode
/// masked, with its input and output cells marked, up to translation, rotation and mirror image.
/// Designs that differ only by those settings are one tunable mechanism (vjeux 2026-10-19: "we
/// don't need 20 variations of the same design"): the catalog shows them as one group card.
pub fn tunable_key(c: &Circuit) -> String {
    let mut d = c.clock.design.clone();
    for st in d.blocks.values_mut() {
        if st.contains("repeater[") {
            *st = mask_prop(st, "delay", "1");
        }
        if st.contains("comparator[") {
            *st = mask_prop(st, "mode", "compare");
        }
    }
    for i in &c.inputs {
        d.blocks.insert(i.pos, "circuit:input".to_string());
        d.block_entities.remove(&i.pos);
    }
    for p in &c.outputs {
        d.entities.push(mctas::design::EntitySpec { kind: "circuit:output".into(), pos: [p.pos[0] as f64 + 0.5, p.pos[1] as f64, p.pos[2] as f64 + 0.5], nbt: None });
    }
    for t in &c.targets {
        d.blocks.insert(t.pos, "circuit:target".to_string());
    }
    // a TNT duper judged with explosions off (`explode: off`, analysis only) is its own type
    if no_explode(c) {
        d.entities.push(mctas::design::EntitySpec { kind: "circuit:noexplode".into(), pos: [0.5, 0.0, 0.5], nbt: None });
    }
    clock::canon::design_type_key(&d)
}

/// A TNT duper with explosions turned off (`explode: off`: analysis only).
fn no_explode(c: &Circuit) -> bool {
    matches!(c.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered)) && !c.explodes()
}

/// A block state with one property set to a value (`delay=3` -> `delay=1`).
fn mask_prop(state: &str, key: &str, value: &str) -> String {
    let Some(i) = state.find(&format!("{key}=")) else {
        return state.to_string();
    };
    let start = i + key.len() + 1;
    let end = state[start..].find([',', ']']).map(|j| start + j).unwrap_or(state.len());
    format!("{}{value}{}", &state[..start], &state[end..])
}

/// The structure's type key: the [`normalized`] design up to translation, the 4 rotations and the
/// mirror image (`clock::canon::design_type_key`), with its input cells marked (whatever drives
/// them: a lever and a redstone-block input are the same circuit) and its output and payload cells
/// marked.
pub fn type_key(c: &Circuit) -> String {
    let n = normalized(c);
    let c = &n;
    let mut d = c.clock.design.clone();
    for i in &c.inputs {
        // the input DEVICE is part of the type (a lever, a stone or a wooden button, a judge-placed
        // block: different timings, coordinator 2026-10-19); its facing is not
        let id = |s: &str| -> String {
            let s = s.split('[').next().unwrap_or(s);
            s.strip_prefix("minecraft:").unwrap_or(s).to_string()
        };
        let dev = match &i.how {
            crate::InputHow::Block { on, .. } => id(on),
            // the same type as the plain input it was (type keys stay stable across R7)
            crate::InputHow::Driven { .. } => id(crate::INPUT_ON),
            crate::InputHow::Lever => "lever".to_string(),
            crate::InputHow::Button => c.clock.design.blocks.get(&i.pos).map(|s| id(s)).unwrap_or_else(|| "button".into()),
            crate::InputHow::Strength { .. } => "strength".to_string(),
            crate::InputHow::Frame { .. } => "frame".to_string(),
        };
        d.blocks.insert(i.pos, format!("circuit:input_{dev}"));
        d.block_entities.remove(&i.pos);
    }
    for (tag, probes) in [("circuit:output", &c.outputs), ("circuit:payload", &c.payloads)] {
        for p in probes {
            d.entities.push(mctas::design::EntitySpec { kind: tag.into(), pos: [p.pos[0] as f64 + 0.5, p.pos[1] as f64, p.pos[2] as f64 + 0.5], nbt: None });
        }
    }
    // a TNT duper judged with explosions off (analysis only) is its own type
    if no_explode(c) {
        d.entities.push(mctas::design::EntitySpec { kind: "circuit:noexplode".into(), pos: [0.5, 0.0, 0.5], nbt: None });
    }
    // a BUD's target cells (where the judge acts) are part of its structure
    for t in &c.targets {
        d.blocks.insert(t.pos, "circuit:target".to_string());
    }
    clock::canon::design_type_key(&d)
}

/// A circuit's value on its study's axis.
pub fn axis(kind: &Kind, out: &Outcome, family: &str) -> String {
    let a = axis_base(kind, out, family);
    // a sculk design that ambient game events disturb is its own class (crate::vibration)
    if out.metrics.iter().any(|(k, v)| k == "vibration-proof" && v.starts_with("no")) { format!("{a}, vibration-sensitive") } else { a }
}

fn axis_base(kind: &Kind, out: &Outcome, family: &str) -> String {
    let m = |k: &str| out.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
    match kind {
        Kind::Elevator(n) => format!("height {n}"),
        Kind::ElevatorFloors(n, s) => format!("{n} floors, {s} apart"),
        Kind::Station(n) => if *n == 2 { "two-way line" } else { "one-way line" }.to_string(),
        Kind::CartUnloader | Kind::CartLoader => if m("line") == "through" { "through line" } else { "terminus" }.to_string(),
        Kind::AutoDoor => "2x2".to_string(),
        Kind::RisingEdge | Kind::FallingEdge | Kind::DualEdge | Kind::PulseLimiter | Kind::PulseExtender => format!("L = {}", m("L")),
        Kind::Delay => format!("N = {}", m("N")),
        Kind::Gate(_) => m("function"),
        Kind::Tff => family.to_string(),
        Kind::RsLatch | Kind::DLatch | Kind::TffReset | Kind::Dff | Kind::DffEnable => kind.name(),
        Kind::Counter(n) => {
            // site QA 2026-10-21: the counter's pick gives one output pulse per N input pulses;
            // square waves (dividers) and other duties are their own classes
            let c = m("counts");
            if c.is_empty() || c.starts_with("one pulse per N") { format!("counter {n}") } else { format!("counter {n}, {c}") }
        }
        Kind::Shift(n) => format!("shift {n}"),
        Kind::PistonExtender(n) => format!("N = {n}"),
        Kind::FlyingMachine => {
            let s = m("speed");
            let a = if s.is_empty() { "two-way".to_string() } else { s };
            // real use (fly 2026-10-21): a machine a player starts with its own lever is its own
            // class, a kick-started one needs a device the judge adds (the picks choose on it)
            if m("start").contains("lever") { format!("{a}, lever start") } else { a }
        }
        Kind::Bud => format!("detects {}{}", m("detects"), if m("reset").is_empty() { "" } else { ", latched (manual reset)" }),
        // the item class: the gallery's rows; speed and size are the columns
        Kind::ShulkerLoader(s) => format!("load {s}-stack"),
        Kind::ShulkerUnloader(s) => format!("unload {s}-stack"),
        // the variation, then `, switchable` for an on/off lever (circuit::tnt::check_lever) or input
        Kind::TntDuper => {
            let base = if out.why.contains("in mid-air") && out.why.contains(" blocks below the machine") {
                format!("{} precise drop", m("rate"))
            } else if out.why.contains("in mid-air") {
                format!("{} precise air burst", m("rate"))
            } else if out.why.contains("precise:") {
                format!("{} precise", m("rate"))
            } else if out.why.contains("dropped:") {
                format!("{} dropped", m("rate"))
            } else if out.why.contains("launched at") {
                format!("{} launched", m("rate"))
            } else if out.why.contains("slime-free") {
                format!("{} slime-free", m("rate"))
            } else {
                m("rate")
            };
            if m("runs").is_empty() { base } else { format!("{base}, switchable") }
        }
        Kind::TntDuperTriggered => format!("{} TNT per pulse", m("per pulse")),
        Kind::Burst(n) => format!("N = {n}"),
        Kind::Divider(n) => format!("N = {n} {}", m("output").split_whitespace().next().unwrap_or("")),
        Kind::Monostable(_) => format!("L = {} {}", m("L"), m("retrigger")),
        // site QA 2026-10-21 (a pressure plate keeping a light on): the full stay-on time from a
        // 20 gt input; a design that needs a longer input (its min-pulse) is its own class
        Kind::OffDelay => {
            let mp: u32 = m("min-pulse").split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            format!("off L = {}{}", m("L"), if mp > 20 { ", needs a long input" } else { "" })
        }
        Kind::OnDelay => format!("on T = {}", m("T")),
        Kind::PulseTrain => format!("P = {}", m("P")),
        // the adder study (w86): what it adds, by its carry ports; a bit slice is a ripple adder of
        // any width (judged at 1..4 and 8 bits)
        Kind::Compute(spec) if spec.op == crate::compute::Op::Adder => {
            if !m("slice").is_empty() {
                "4-bit adder slice".to_string()
            } else if spec.n == 1 && m("carry") == "in and out" {
                "full adder".to_string()
            } else if spec.n == 1 && m("carry") == "out" {
                "half adder".to_string()
            } else {
                format!("{}-bit adder{}", spec.n, if m("carry").is_empty() { String::new() } else { format!(", carry {}", m("carry")) })
            }
        }
        Kind::Compute(..) => kind.name(),
        // the item class the recipe sorts (64- or 16-stack); size and badges are the columns
        Kind::Sorter(item) if m("storage").starts_with("bulk") => format!("bulk storage row, {}-stack items", crate::sorter::max_stack(item)),
        Kind::Sorter(item) if !m("storage").is_empty() => format!("storage row, {}-stack items", crate::sorter::max_stack(item)),
        Kind::Sorter(item) => format!("sorts {}-stack items", crate::sorter::max_stack(item)),
        Kind::Lock(n) => format!("{n} dials"),
        Kind::SeqLock(n, l) => format!("{n} buttons, code {l}"),
        Kind::Randomizer(n) => format!("1 of {n}"),
        // the furnace count; a tileable slice is its own class (it builds any count)
        Kind::Smelter(n) => {
            if m("tileable").is_empty() {
                format!("{n} furnace{}", if *n == 1 { "" } else { "s" })
            } else {
                format!("tileable, {n} furnace{} a slice", if *n == 1 { "" } else { "s" })
            }
        }
        // the crop it harvests (blocks per cane and items per hour are the columns)
        Kind::Harvester(crop) => crop.strip_prefix("minecraft:").unwrap_or(crop).replace('_', " "),
        Kind::Brewer(n) => {
            if m("tileable").is_empty() {
                format!("{n} brewing stand{}", if *n == 1 { "" } else { "s" })
            } else {
                format!("tileable, {n} brewing stand{} a slice", if *n == 1 { "" } else { "s" })
            }
        }
        Kind::Composter(n) => {
            if m("tileable").is_empty() {
                format!("{n} composter{}", if *n == 1 { "" } else { "s" })
            } else {
                format!("tileable, {n} composter{} a slice", if *n == 1 { "" } else { "s" })
            }
        }
        Kind::KelpFactory(n) => format!("{n} kelp plant{}", if *n == 1 { "" } else { "s" }),
        Kind::Crafter(r) => crate::crafter::recipe(r).map(|x| format!("{} recipe", x.class)).unwrap_or_default(),
        // the height (blocks and items/s are the columns); a registered study's kind: its own axis
        Kind::Study(s) => (s.def().axis)(&s.args, &m),
    }
}

/// Result of [`archive`].
pub struct Archived {
    pub file: PathBuf,
    pub summary: String,
    pub new: bool,
}

/// `mctas circuit archive`: the judge, then the design kept in its study's archive. Refused: a
/// failing design, or one whose structure (type key) is already archived as the same kind.
pub fn archive(pool: &Pool, path: &Path, o: &Options, root: &Path, caption: Option<&str>) -> Result<Archived> {
    let c = Circuit::load(path)?;
    let kind = c.kind.clone().ok_or_else(|| anyhow!("{}: no `circuit: <kind>` line", path.display()))?;
    if no_explode(&c) {
        bail!("not archived: {} has `explode: off` (analysis only): a TNT duper is archived only when it survives its own explosions", path.display());
    }
    let dir = archive_dir(root, kind.study());
    let text = std::fs::read_to_string(path)?;
    // the same blocks and ports under another kind (a claim) are another entry; so is a TNT duper
    // judged with explosions on (the SURVIVABLE variation)
    let hash = format!("{}-{}{}", clock::catalog::content_hash(&c.clock), kind.name().replace(' ', "_"), if no_explode(&c) { "-noexplode" } else { "" });
    let rows = read_rows(&dir)?;
    if let Some(r) = rows.iter().find(|r| r.hash == hash) {
        return Ok(Archived { file: dir.join(&r.file), summary: r.summary.clone(), new: false });
    }
    let tk = type_key(&c);
    if let Some(r) = rows.iter().find(|r| r.type_key == tk && r.kind == kind.name()).filter(|_| !c.clock.design.meta.contains_key("extend family")) {
        return Ok(Archived { file: dir.join(&r.file), summary: format!("same type as {} (a rotation, mirror image or move of it, same kind): not archived", r.file), new: false });
    }
    // vjeux 2026-10-21: obsidian only where needed: an obsidian block the kind's redsim judge
    // accepts as stone (same kind, axis and metrics) is refused (`circuit material` writes the fix)
    if c.clock.design.blocks.values().any(|s| s == "minecraft:obsidian") {
        let m = crate::material::obsidian_pass(&c.clock.design, &verify::Options { version: o.version.clone(), ..Default::default() });
        if !m.replaced.is_empty() {
            bail!(
                "not archived: obsidian at {} can be stone (the same kind, axis and metrics on its redsim judge): `mctas circuit material {} --out DIR` writes it (vjeux 2026-10-21: obsidian only where needed)",
                m.replaced.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(", "),
                path.display()
            );
        }
    }
    // a design stores what a player's placement gives (the coordinator, 2026-10-21)
    clock::catalog::refuse_unplaced_shapes(&c.clock, path)?;
    let rep = verify::verify_in_pool(pool, &c, o)?;
    if !rep.pass {
        bail!("not archived: {}", rep.summary(&c));
    }
    let base = rep.base().ok_or_else(|| anyhow!("no passing build"))?.clone();
    // an extendable family: n = low, low+1, low+2, the middle and the high end judged in the game
    // first (axis value and blocks must follow the formulas), recorded in extend.tsv
    let family = family_of(&c)?;
    let instances = match &family {
        Some(f) => check_family(pool, path, f, &kind, o).map_err(|e| anyhow!("not archived: {e:#}"))?,
        None => Vec::new(),
    };
    std::fs::create_dir_all(&dir)?;
    let stem: String = c.name().chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' }).collect();
    let stem = if stem.is_empty() { "circuit".to_string() } else { stem };
    let mut file = format!("{stem}.txt");
    // a name taken by ANOTHER structure: name + '-' + a short hash of the type, then -2, -3...
    if dir.join(&file).exists() {
        let h = tk.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        file = format!("{stem}-{:06x}.txt", h & 0xffffff);
    }
    let base_file = file.trim_end_matches(".txt").to_string();
    let mut n = 2;
    while dir.join(&file).exists() {
        file = format!("{base_file}-{n}.txt");
        n += 1;
    }
    let summary = rep.summary(&c);
    let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CIRCUIT: ")).map(|l| format!("{l}\n")).collect();
    let now = clock::catalog::now_utc();
    std::fs::write(dir.join(&file), format!("# archived {now} from {} by mctas circuit archive\n# {summary}\n{body}", path.display()))?;
    let [w, h, d] = base.dims;
    let row = Row {
        archived: now,
        file: file.clone(),
        kind: kind.name(),
        axis: axis(&kind, &base.outcome, &base.family),
        metrics: base.outcome.metrics_text(),
        blocks: c.blocks(),
        dims: format!("{w}x{h}x{d}"),
        volume: (w * h * d) as i64,
        family: base.family.clone(),
        robustness: if rep.note.is_empty() { rep.verdict.clone() } else { format!("{} ({})", rep.verdict, rep.note) },
        kinds: c.kinds().into_iter().collect::<Vec<_>>().join("+"),
        type_key: tk,
        hash,
        summary: summary.clone(),
    };
    let mut now_rows = read_rows(&dir)?;
    now_rows.retain(|r| r.file != row.file);
    now_rows.push(row);
    write_rows(&dir, &now_rows)?;
    let cap = caption.map(|s| s.to_string()).unwrap_or_else(|| clock::catalog::default_caption(&body));
    if !cap.is_empty() {
        let cp = dir.join(CAPTIONS);
        let old = if cp.exists() { std::fs::read_to_string(&cp)? } else { "file\tcaption\n".to_string() };
        let mut caps: String = old.lines().filter(|l| l.split('\t').next() != Some(file.as_str())).map(|l| format!("{l}\n")).collect();
        caps.push_str(&format!("{file}\t{}\n", cap.replace(['\t', '\n'], " ")));
        std::fs::write(&cp, caps)?;
    }
    if !instances.is_empty() {
        let ep = dir.join(EXTEND_TSV);
        let fam = &instances[0].family;
        let old = if ep.exists() { std::fs::read_to_string(&ep)? } else { "family\tn\taxis\tblocks\ttype_key\n".to_string() };
        let mut all: String = old.lines().filter(|l| l.split('\t').next() != Some(fam.as_str())).map(|l| format!("{l}\n")).collect();
        for i in &instances {
            all.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", i.family, i.n, i.axis, i.blocks, i.type_key));
        }
        std::fs::write(&ep, all)?;
    }
    let summary = match &family {
        Some(f) => format!("{summary}; extend family {}: {} = {}, n = {}..{} (judged: n = {})", f.id, axis_label(&axis(&kind, &base.outcome, &base.family)).0, f.period.text(), f.n_lo, f.n_hi, instances.iter().map(|i| i.n.to_string()).collect::<Vec<_>>().join(", ")),
        None => summary,
    };
    Ok(Archived { file: dir.join(file), summary, new: true })
}

/// The sort key of an axis value: its first number (if any), then the text.
fn axis_key(a: &str) -> (i64, String) {
    let digits: String = a.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(i64::MAX), a.to_string())
}

/// The first integer of an axis value (`L = 8 gt` -> 8).
fn axis_number(a: &str) -> Option<i64> {
    let digits: String = a.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// An axis value's label and unit (`L = 8 gt` -> ("L", "gt"); `N = 3` -> ("N", ""); a flying
/// machine's `10 gt/block` -> ("speed", "gt/block")).
fn axis_label(a: &str) -> (String, String) {
    match a.split_once(" = ") {
        Some((l, v)) => (l.to_string(), v.split_whitespace().nth(1).unwrap_or("").to_string()),
        None if a.ends_with(" gt/block") => ("speed".to_string(), "gt/block".to_string()),
        // a counter's axis is its kind, `counter N`
        None if a.starts_with("counter ") => ("N".to_string(), String::new()),
        // an item elevator's, its height (`5 blocks up`)
        None if a.ends_with(" blocks up") => ("height".to_string(), "blocks up".to_string()),
        None => (String::new(), String::new()),
    }
}

pub const EXTEND_TSV: &str = "extend.tsv";

/// A circuit's extendable family (the clock `extend` headers: docs/clocks.md#extendable-clocks):
/// `extend axis:` is the formula of the study's axis value in n (L, N, the reach...; `extend
/// period:` is read the same), `extend blocks:` the block count's.
pub fn family_of(c: &Circuit) -> Result<Option<clock::extend::Family>> {
    let mut d = c.clock.design.clone();
    if !d.meta.contains_key("extend family") {
        return Ok(None);
    }
    if let Some(a) = d.meta.get("extend axis").cloned() {
        d.meta.entry("extend period".to_string()).or_insert(a);
    }
    clock::extend::Family::of(&clock::Clock::from_design(d)?)
}

/// One judged member of a family (a row of `extend.tsv`).
#[derive(Clone, Debug)]
pub struct Instance {
    pub family: String,
    pub n: i64,
    pub axis: String,
    pub blocks: usize,
    pub type_key: String,
}

fn read_instances(dir: &Path) -> Vec<Instance> {
    std::fs::read_to_string(dir.join(EXTEND_TSV))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 5).then(|| Instance { family: f[0].into(), n: f[1].parse().unwrap_or(0), axis: f[2].into(), blocks: f[3].parse().unwrap_or(0), type_key: f[4].into() })
        })
        .collect()
}

/// Judge the members `ns` of a family, built with its generator, in the game: each must pass with
/// the formulas' axis value and block count.
fn check_family(pool: &Pool, path: &Path, f: &clock::extend::Family, kind: &Kind, o: &Options) -> Result<Vec<Instance>> {
    check_members(pool, path, f, kind, o, &f.judged_ns())
}

fn check_members(pool: &Pool, path: &Path, f: &clock::extend::Family, kind: &Kind, o: &Options, ns: &[i64]) -> Result<Vec<Instance>> {
    let dir = mctas::paths::run_dir().join(format!("circuit-extend-{}", std::process::id()));
    let mut out = Vec::new();
    for &n in ns {
        let p = f.build(path, n, &dir).with_context(|| format!("extend family {}: building n = {n}", f.id))?;
        let c = Circuit::load(&p)?;
        let rep = verify::verify_in_pool(pool, &c, o)?;
        let Some(b) = rep.base().filter(|_| rep.pass) else {
            bail!("extend family {}: n = {n}: {}", f.id, rep.summary(&c));
        };
        // the member's own kind: its argument is the axis of `piston-extender N` / `counter N`
        let ax = axis(c.kind.as_ref().unwrap_or(kind), &b.outcome, &b.family);
        let (want_a, want_b) = (f.period.eval(n), f.blocks.eval(n));
        if axis_number(&ax) != Some(want_a) || c.blocks() as i64 != want_b {
            bail!("extend family {}: n = {n}: {ax}, {} blocks; the formulas say {} = {want_a}, {want_b} blocks", f.id, c.blocks(), axis_label(&ax).0);
        }
        out.push(Instance { family: f.id.clone(), n, axis: ax, blocks: c.blocks(), type_key: type_key(&c) });
    }
    Ok(out)
}

pub const ALIASES: &str = "aliases.tsv";

/// The first frame where an output's level differs between a run with the judge's old block
/// placement (`a`) and one with real input devices (`b`, R7).
fn first_difference(a: &std::result::Result<crate::trace::Trace, String>, b: &std::result::Result<crate::trace::Trace, String>) -> Option<String> {
    let (Ok(a), Ok(b)) = (a, b) else { return None };
    (0..a.frames().min(b.frames())).find(|f| a.levels.iter().zip(&b.levels).any(|(x, y)| x.get(*f) != y.get(*f))).map(|f| {
        let lv = |t: &crate::trace::Trace| t.levels.iter().map(|l| l.get(f).copied().unwrap_or(0).to_string()).collect::<Vec<_>>().join("/");
        format!("first difference at frame {f}: output {} with the block placement, {} with the repeater", lv(a), lv(b))
    })
}

/// R7's note for a design the current judge refuses (`mctas circuit archive --rejudge`): when it
/// passes with the judge's old block placement on redsim, `needs the judge's block placement
/// (R7): <first difference>`, so a removed design explains itself.
pub fn r7_note(path: &Path, o: &Options) -> Option<String> {
    crate::set_input_device_block(true);
    let old = Circuit::load(path);
    crate::set_input_device_block(false);
    let new = Circuit::load(path).ok()?;
    let old = old.ok()?;
    if !old.inputs.iter().any(|i| matches!(&i.how, crate::InputHow::Block { on, .. } if on == crate::INPUT_ON)) {
        return None;
    }
    let so = crate::verify::sim(&old, o).ok()?;
    if !so.outcome.as_ref().is_some_and(|x| x.pass) {
        return None;
    }
    let sn = crate::verify::sim(&new, o).ok()?;
    if sn.outcome.as_ref().is_some_and(|x| x.pass) {
        return None;
    }
    Some(format!("needs the judge's block placement (R7: passes with a redstone block written into its input cell, not with a real input){}", first_difference(&so.trace, &sn.trace).map(|f| format!("; {f}")).unwrap_or_default()))
}

/// `mctas circuit real-use` (the real-use audit, 2026-10-21): every archived design of the
/// studies judged on redsim without and with the audit's stimulus parts (real presses, a lever held
/// down, a lever flipped back too soon: `check::set_real_use`), in parallel. One line per design
/// whose verdict or metrics change: `NOW-FAILS` (with the audit's reason), `CHANGED` (other
/// metrics: a new metric such as an extender's `flips`), then a count per study.
pub fn real_use_compare(root: &Path, studies: &[String], o: &Options, threads: usize) -> Result<Vec<String>> {
    let mut jobs: Vec<(String, PathBuf)> = Vec::new();
    for (study, _) in self::studies().iter().filter(|(s, _)| studies.is_empty() || studies.iter().any(|w| w == s)) {
        let dir = archive_dir(root, study);
        for row in read_rows(&dir)? {
            jobs.push((study.to_string(), dir.join(&row.file)));
        }
    }
    // (the switch is global: every design without the audit, then every design with it)
    let run = |on: bool| -> Vec<Option<crate::check::Outcome>> {
        crate::check::set_real_use(on);
        let next = std::sync::atomic::AtomicUsize::new(0);
        let out: std::sync::Mutex<Vec<Option<crate::check::Outcome>>> = std::sync::Mutex::new(vec![None; jobs.len()]);
        std::thread::scope(|sc| {
            for _ in 0..threads.max(1) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= jobs.len() {
                        break;
                    }
                    let r = Circuit::load(&jobs[i].1).ok().and_then(|c| crate::verify::sim(&c, o).ok()).and_then(|s| s.outcome);
                    out.lock().unwrap()[i] = r;
                });
            }
        });
        out.into_inner().unwrap()
    };
    let before = run(false);
    let after = run(true);
    crate::check::set_real_use(true);
    let metrics = |m: &[(String, String)]| m.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
    let mut out = Vec::new();
    let mut counts: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    for (i, (study, path)) in jobs.iter().enumerate() {
        let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let e = counts.entry(study.clone()).or_default();
        match (&before[i], &after[i]) {
            (Some(b), Some(a)) if b.pass && !a.pass => {
                e.0 += 1;
                out.push(format!("NOW-FAILS {study}/{file}: {}", a.why));
            }
            (Some(b), Some(a)) if b.pass && a.pass && b.metrics != a.metrics => {
                e.1 += 1;
                out.push(format!("CHANGED {study}/{file}: {} -> {}", metrics(&b.metrics), metrics(&a.metrics)));
            }
            (Some(b), Some(_)) if b.pass => e.2 += 1,
            _ => e.3 += 1,
        }
    }
    for (study, (f, c, s, k)) in counts {
        out.push(format!("{study}: {f} now fail, {c} changed, {s} unchanged, {k} without a redsim verdict"));
    }
    Ok(out)
}

/// `mctas circuit r7`: every archived design with a plain input, run in redsim twice: with the
/// judge's old input placement (a redstone block written into the input cell) and with real input
/// devices (R7: the judge's stone powered by its repeater). One line per design whose verdict or
/// metrics change: `PLACEMENT-ONLY` (passes only by the judge's block placement, with the first
/// frame where an output differs between the two runs), `REFUSED` (no way in for the judge's
/// repeater), `CHANGED` (passes both ways, other metrics); then a count per study.
pub fn r7_compare(root: &Path, studies: &[String], o: &Options) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let load = |path: &Path, block: bool| -> Result<Circuit> {
        crate::set_input_device_block(block);
        let c = Circuit::load(path);
        crate::set_input_device_block(false);
        c
    };
    for (study, _) in self::studies().iter().filter(|(s, _)| studies.is_empty() || studies.iter().any(|w| w == s)) {
        let dir = archive_dir(root, study);
        let (mut same, mut skipped, mut changed, mut only, mut refused) = (0, 0, 0, 0, 0);
        for row in read_rows(&dir)? {
            let path = dir.join(&row.file);
            let Ok(old) = load(&path, true) else { continue };
            if !old.inputs.iter().any(|i| matches!(&i.how, crate::InputHow::Block { on, off } if on == crate::INPUT_ON && mctas::blockstate::is_air(off))) {
                skipped += 1;
                continue;
            }
            let new = match load(&path, false) {
                Ok(c) if c.input_refused.is_none() => c,
                Ok(c) => {
                    refused += 1;
                    out.push(format!("REFUSED {study}/{}: {}", row.file, c.input_refused.unwrap_or_default()));
                    continue;
                }
                Err(e) => {
                    refused += 1;
                    out.push(format!("REFUSED {study}/{}: {:#}", row.file, e));
                    continue;
                }
            };
            let (Ok(so), Ok(sn)) = (crate::verify::sim(&old, o), crate::verify::sim(&new, o)) else {
                skipped += 1;
                continue;
            };
            let (Some(oo), Some(on)) = (&so.outcome, &sn.outcome) else {
                skipped += 1;
                continue;
            };
            let metrics = |m: &[(String, String)]| m.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
            if oo.pass && !on.pass {
                // a design whose input feeds a diode's side needs a signal source there (R7
                // model B): the same design with dust in its input cells
                let dusted = (|| -> Option<String> {
                    let mut d = mctas::design::Design::load(&path).ok()?;
                    for i in &new.inputs {
                        if !d.blocks.contains_key(&i.pos) {
                            d.blocks.insert(i.pos, "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]".into());
                        }
                    }
                    let c = Circuit::from_design(d).ok()?;
                    let r = crate::verify::sim(&c, o).ok()?;
                    r.outcome.filter(|x| x.pass).map(|x| metrics(&x.metrics))
                })();
                if let Some(m) = dusted {
                    changed += 1;
                    out.push(format!("NEEDS-DUST {study}/{}: passes with dust in its input cell (+1 block per input; a diode side reads only a signal source): {m}", row.file));
                    continue;
                }
                only += 1;
                out.push(format!("PLACEMENT-ONLY {study}/{}: passes only by the judge's block placement: {}{}", row.file, on.why, first_difference(&so.trace, &sn.trace).map(|f| format!("; {f}")).unwrap_or_default()));
            } else if oo.pass && on.pass && oo.metrics != on.metrics {
                changed += 1;
                out.push(format!("CHANGED {study}/{}: {} -> {}", row.file, metrics(&oo.metrics), metrics(&on.metrics)));
            } else {
                same += 1;
            }
        }
        out.push(format!("{study}: {only} pass only by the block placement, {refused} refused, {changed} changed, {same} unchanged, {skipped} without a plain input or a redsim verdict"));
    }
    Ok(out)
}

/// `mctas circuit captions <tsv>`: replace archived designs' captions, one
/// `<study>/<file><TAB><caption>` per line (`#` lines and blank lines skipped). Each must name an
/// archived design. Returns one line per caption set.
pub fn set_captions(root: &Path, tsv: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for line in tsv.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let (spec, cap) = line.split_once('\t').ok_or_else(|| anyhow!("captions: no tab in `{line}`"))?;
        let (study, file) = spec.split_once('/').ok_or_else(|| anyhow!("captions: `{spec}` is not <study>/<file>"))?;
        let dir = archive_dir(root, study);
        if !dir.join(file).exists() {
            bail!("captions: {} is not archived", dir.join(file).display());
        }
        let cp = dir.join(CAPTIONS);
        let old = if cp.exists() { std::fs::read_to_string(&cp)? } else { "file\tcaption\n".to_string() };
        let cap = cap.trim().replace(['\t', '\n'], " ");
        // `+ <sentence>`: insert the sentence after the plain part of the existing caption (before
        // its " Details:"), or at its end
        let insert = cap.strip_prefix("+ ").map(str::to_string);
        let mut found = false;
        let mut caps = String::new();
        let mut shown = cap.clone();
        for l in old.lines() {
            if l.split('\t').next() == Some(file) {
                let new = match &insert {
                    Some(s) => {
                        let cur = l.split_once('\t').map(|x| x.1).unwrap_or("");
                        if cur.contains(s.as_str()) {
                            cur.to_string()
                        } else if let Some(i) = cur.find(" Details:") {
                            format!("{} {s}{}", &cur[..i], &cur[i..])
                        } else {
                            format!("{cur} {s}")
                        }
                    }
                    None => cap.clone(),
                };
                caps.push_str(&format!("{file}\t{new}\n"));
                shown = new;
                found = true;
            } else {
                caps.push_str(&format!("{l}\n"));
            }
        }
        if !found {
            if insert.is_some() {
                bail!("captions: {file} has no caption to insert into");
            }
            caps.push_str(&format!("{file}\t{cap}\n"));
        }
        std::fs::write(&cp, caps)?;
        let cap = shown;
        out.push(format!("{study}/{file}: {}", cap.split(". ").next().unwrap_or(&cap)));
    }
    Ok(out)
}

/// `mctas circuit archive --fill-families`: judge in the game every member of each archived
/// extendable family that extend.tsv does not list yet (every n of its range up to
/// `clock::extend::EVERY_MAX` members: no unjudged step between judged ones), in the given studies
/// (all when empty), and add the passing ones to extend.tsv. A member that fails is reported and
/// not added (its family keeps its card; fix the generator or the range).
pub fn fill_families(pool: &Pool, root: &Path, o: &Options, studies: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (study, _) in self::studies().iter().filter(|(s, _)| studies.is_empty() || studies.iter().any(|w| w == s)) {
        let dir = archive_dir(root, study);
        let rows = read_rows(&dir)?;
        let mut instances = read_instances(&dir);
        let mut added = 0;
        for row in &rows {
            let path = dir.join(&row.file);
            let Ok(c) = Circuit::load(&path) else { continue };
            let (Some(kind), Ok(Some(f))) = (c.kind.clone(), family_of(&c)) else { continue };
            let have: Vec<i64> = instances.iter().filter(|i| i.family == f.id).map(|i| i.n).collect();
            let missing: Vec<i64> = f.judged_ns().into_iter().filter(|n| !have.contains(n)).collect();
            if missing.is_empty() {
                continue;
            }
            match check_members(pool, &path, &f, &kind, o, &missing) {
                Ok(new) => {
                    out.push(format!("{study} {}: judged n = {} ({})", f.id, missing.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "), new.iter().map(|i| i.axis.clone()).collect::<Vec<_>>().join(", ")));
                    added += new.len();
                    instances.extend(new);
                }
                Err(e) => out.push(format!("{study} {}: FAILED {e:#}", f.id)),
            }
        }
        if added > 0 {
            instances.sort_by(|a, b| (a.family.as_str(), a.n).cmp(&(b.family.as_str(), b.n)));
            let mut all = "family\tn\taxis\tblocks\ttype_key\n".to_string();
            for i in &instances {
                all.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", i.family, i.n, i.axis, i.blocks, i.type_key));
            }
            std::fs::write(dir.join(EXTEND_TSV), all)?;
        }
    }
    Ok(out)
}

/// `mctas circuit archive --rejudge`: judge every archived design again (the current judge, in
/// the game; family bases with their sampled members) in the given studies (all when empty). A
/// passing design keeps its row with the new metrics, box, family, robustness and summary (its
/// `# CIRCUIT:` line rewritten); a failing one leaves the archive (file, verdict row, caption,
/// its family's extend.tsv rows, aliases it hosts) and is copied back next to the track file it
/// was archived from, under `update-less/` when it fails for an update-less output (else
/// `rejudged/`), with a first line saying why.
pub fn rejudge(pool: &Pool, root: &Path, o: &Options, studies: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (study, _) in self::studies().iter().filter(|(s, _)| studies.is_empty() || studies.iter().any(|w| w == s)) {
        let dir = archive_dir(root, study);
        let rows = read_rows(&dir)?;
        if rows.is_empty() {
            continue;
        }
        let mut keep: Vec<Row> = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        let mut instances = read_instances(&dir);
        for row in rows {
            let path = dir.join(&row.file);
            if !path.exists() {
                out.push(format!("MISSING {}: no file, row dropped", path.display()));
                removed.push(row.file.clone());
                continue;
            }
            let c = Circuit::load(&path)?;
            let Some(kind) = c.kind.clone() else {
                keep.push(row);
                continue;
            };
            let rep = verify::verify_in_pool(pool, &c, o)?;
            let fam = family_of(&c)?;
            let fam_check = match (&fam, rep.pass) {
                (Some(f), true) => Some(check_family(pool, &path, f, &kind, o)),
                _ => None,
            };
            let failure = if !rep.pass {
                Some(rep.summary(&c))
            } else if let Some(Err(e)) = &fam_check {
                let e = format!("{e:#}");
                // a generator that could not run is the tool's fault, not the design's: keep it
                if e.contains("tool error") {
                    out.push(format!("ERROR (kept) {}: {e}", path.display()));
                    None
                } else {
                    Some(e)
                }
            } else {
                None
            };
            let text = std::fs::read_to_string(&path)?;
            if let Some(why) = failure {
                // R7: a design that passed only by the judge's block placement says so
                let why = match r7_note(&path, o) {
                    Some(n) => format!("{n}: {why}"),
                    None => why,
                };
                // back to the track it came from, with why
                let from = text.lines().find_map(|l| l.strip_prefix("# archived ").and_then(|r| r.split(" from ").nth(1)).map(|r| r.trim_end_matches(" by mctas circuit archive").to_string()));
                let sub = if why.contains("update-less output") { "update-less" } else { "rejudged" };
                let back = match from.as_deref().map(Path::new).and_then(|p| p.parent()) {
                    Some(d) if d.starts_with(root.join(study).join("tracks")) => d.join(sub),
                    _ => root.join(study).join("tracks").join(sub),
                };
                std::fs::create_dir_all(&back)?;
                let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CIRCUIT: ")).map(|l| format!("{l}\n")).collect();
                let copy = back.join(&row.file);
                std::fs::write(&copy, format!("# {sub} ({}, mctas circuit archive --rejudge): {why}\n{body}", clock::catalog::now_utc()))?;
                std::fs::remove_file(&path)?;
                if let Some(f) = &fam {
                    instances.retain(|i| i.family != f.id);
                }
                out.push(format!("REMOVED {}: {why} (copy: {})", path.display(), copy.display()));
                removed.push(row.file.clone());
                continue;
            }
            let base = rep.base().ok_or_else(|| anyhow!("no passing build"))?.clone();
            let summary = rep.summary(&c);
            let [w, h, d] = base.dims;
            let new = Row {
                axis: axis(&kind, &base.outcome, &base.family),
                metrics: base.outcome.metrics_text(),
                blocks: c.blocks(),
                dims: format!("{w}x{h}x{d}"),
                volume: (w * h * d) as i64,
                family: base.family.clone(),
                robustness: if rep.note.is_empty() { rep.verdict.clone() } else { format!("{} ({})", rep.verdict, rep.note) },
                summary: summary.clone(),
                ..row.clone()
            };
            let changed = new.metrics != row.metrics || new.robustness != row.robustness || new.dims != row.dims;
            let header_lines: Vec<String> = text.lines().map(|l| if l.starts_with("# CIRCUIT: ") || l.starts_with("# FAIL: ") { format!("# {summary}") } else { l.to_string() }).collect();
            std::fs::write(&path, header_lines.join("\n") + "\n")?;
            if let (Some(f), Some(Ok(inst))) = (&fam, fam_check) {
                instances.retain(|i| i.family != f.id);
                instances.extend(inst);
            }
            out.push(if changed { format!("UPDATED {}: {} (was: {}, {})", path.display(), summary, row.metrics, row.robustness) } else { format!("KEPT {}: {summary}", path.display()) });
            keep.push(new);
        }
        write_rows(&dir, &keep)?;
        if !removed.is_empty() {
            let cp = dir.join(CAPTIONS);
            if cp.exists() {
                let caps: String = std::fs::read_to_string(&cp)?.lines().filter(|l| !removed.iter().any(|f| l.split('\t').next() == Some(f.as_str()))).map(|l| format!("{l}\n")).collect();
                std::fs::write(&cp, caps)?;
            }
            let ap = dir.join(ALIASES);
            if ap.exists() {
                let al: String = std::fs::read_to_string(&ap)?.lines().filter(|l| !removed.iter().any(|f| l.split('\t').next() == Some(f.as_str()))).map(|l| format!("{l}\n")).collect();
                std::fs::write(&ap, al)?;
            }
        }
        let ep = dir.join(EXTEND_TSV);
        if ep.exists() || !instances.is_empty() {
            let mut all = "family\tn\taxis\tblocks\ttype_key\n".to_string();
            for i in &instances {
                all.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", i.family, i.n, i.axis, i.blocks, i.type_key));
            }
            std::fs::write(&ep, all)?;
        }
    }
    Ok(out)
}

/// `mctas circuit archive --fill-game-rts`: write the `game rts` metric (the random_tick_speed the
/// farm judge's robustness runs use, [`crate::farm::game_rts`], from the vanilla twin run alone:
/// no game) into every archived harvester's verdict row, summary and `# CIRCUIT:` line in the given
/// studies (all when empty), before its bill, where the judge now records it. A verdict that holds
/// another value gets the new one; every other metric is left as it is (coordinator 2026-10-23:
/// the site ran a twin run of its own for each big farm at every deploy to get this number).
pub fn fill_game_rts(root: &Path, studies: &[String], margin: i32) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (study, _) in self::studies().iter().filter(|(s, _)| studies.is_empty() || studies.iter().any(|w| w == s)) {
        let dir = archive_dir(root, study);
        let mut rows = read_rows(&dir)?;
        let todo: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| r.kind.starts_with("harvester")).map(|(i, _)| i).collect();
        if todo.is_empty() {
            continue;
        }
        // the twin runs side by side (CPU only, no game), 4 at a time
        let next = std::sync::atomic::AtomicUsize::new(0);
        let done: std::sync::Mutex<Vec<(usize, std::result::Result<u32, String>)>> = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| {
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let Some(&i) = todo.get(k) else { break };
                        let r = Circuit::load(&dir.join(&rows[i].file)).map_err(|e| format!("{e:#}")).map(|c| crate::farm::game_rts(&c, &c, &crate::farm::vanilla_run(&c, margin)));
                        done.lock().unwrap_or_else(|e| e.into_inner()).push((i, r));
                    }
                });
            }
        });
        let mut done = done.into_inner().unwrap_or_else(|e| e.into_inner());
        done.sort_by_key(|(i, _)| *i);
        for (i, r) in done {
            let row = &mut rows[i];
            let rts = match r {
                Ok(rts) => rts,
                Err(why) => {
                    out.push(format!("ERROR {}/{}: {why}", study, row.file));
                    continue;
                }
            };
            let (Some(metrics), Some(summary)) = (with_game_rts(&row.metrics, rts), with_game_rts(&row.summary, rts)) else {
                out.push(format!("ERROR {}/{}: no bill to put `game rts {rts}` before", study, row.file));
                continue;
            };
            let path = dir.join(&row.file);
            let text = std::fs::read_to_string(&path)?;
            let lines: Vec<String> = text.lines().map(|l| if l.starts_with("# CIRCUIT: ") { with_game_rts(l, rts).unwrap_or_else(|| l.to_string()) } else { l.to_string() }).collect();
            let new_text = lines.join("\n") + "\n";
            let changed = metrics != row.metrics || summary != row.summary || new_text != text;
            if new_text != text {
                std::fs::write(&path, new_text)?;
            }
            row.metrics = metrics;
            row.summary = summary;
            out.push(format!("{} {}/{}: game rts {rts}", if changed { "FILLED" } else { "KEPT" }, study, row.file));
        }
        write_rows(&dir, &rows)?;
    }
    Ok(out)
}

/// `text` (a metrics list, a summary or a `# CIRCUIT:` line) with its `game rts` set to `rts`: the
/// value replaced, else the metric put before the bill; None without either.
fn with_game_rts(text: &str, rts: u32) -> Option<String> {
    const KEY: &str = "game rts ";
    if let Some(i) = text.find(KEY) {
        let digits = text[i + KEY.len()..].chars().take_while(|c| c.is_ascii_digit()).count();
        return Some(format!("{}{KEY}{rts}{}", &text[..i], &text[i + KEY.len() + digits..]));
    }
    if let Some(i) = text.find(", bill ") {
        return Some(format!("{}, {KEY}{rts}{}", &text[..i], &text[i..]));
    }
    text.starts_with("bill ").then(|| format!("{KEY}{rts}, {text}"))
}
fn read_aliases(dir: &Path) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in std::fs::read_to_string(dir.join(ALIASES)).unwrap_or_default().lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 3 {
            m.entry(f[0].to_string()).or_default().push(f[2].to_string());
        }
    }
    m
}

/// `mctas circuit archive --dedupe`: in every study, keep ONE archived design per (kind, type) (the
/// earliest archived), remove the others (design, verdict row, caption) and record them in
/// `aliases.tsv` (kept file, removed file, its name and what it was built with), which the catalog
/// lists as "also found as". The type keys are recomputed from the files (the swaps included).
pub fn dedupe(root: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for (study, _) in studies() {
        let dir = archive_dir(root, study);
        let mut rows = read_rows(&dir)?;
        if rows.is_empty() {
            continue;
        }
        // a row whose design file is gone (removed on one side of a merge, brought back by a
        // union of the TSVs): out, with its caption
        let stale: Vec<String> = rows.iter().filter(|r| !dir.join(&r.file).exists()).map(|r| r.file.clone()).collect();
        for f in &stale {
            lines.push(format!("REMOVED {study}/{f}: a verdict row without its design file (stale)"));
        }
        rows.retain(|r| !stale.contains(&r.file));
        tidy(&dir, &rows, &stale)?;
        rows.sort_by(|a, b| (&a.archived, &a.file).cmp(&(&b.archived, &b.file)));
        let mut kept: BTreeMap<(String, String), String> = BTreeMap::new();
        let mut gone: Vec<(String, String, String)> = Vec::new();
        for r in &rows {
            let c = Circuit::load(&dir.join(&r.file));
            // an extendable family's base is its own entry: never removed, never removes another
            if c.as_ref().is_ok_and(|c| c.clock.design.meta.contains_key("extend family")) {
                continue;
            }
            let tk = c.as_ref().map(type_key).unwrap_or_else(|_| r.type_key.clone());
            match kept.get(&(r.kind.clone(), tk.clone())) {
                Some(k) if *k != r.file => {
                    let with = match (Circuit::load(&dir.join(k)), &c) {
                        (Ok(a), Ok(b)) => {
                            let (ka, kb) = (a.kinds(), b.kinds());
                            let extra: Vec<&String> = kb.difference(&ka).collect();
                            let missing: Vec<&String> = ka.difference(&kb).collect();
                            let mut parts = Vec::new();
                            if !extra.is_empty() {
                                parts.push(format!("with {}", extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
                            }
                            if !missing.is_empty() {
                                parts.push(format!("without {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
                            }
                            if parts.is_empty() { String::new() } else { format!(" (built {})", parts.join(", ")) }
                        }
                        _ => String::new(),
                    };
                    gone.push((k.clone(), r.file.clone(), format!("{}{with}", r.file.trim_end_matches(".txt"))));
                }
                Some(_) => {}
                None => {
                    kept.insert((r.kind.clone(), tk), r.file.clone());
                }
            }
        }
        if gone.is_empty() {
            continue;
        }
        let removed: BTreeSet<String> = gone.iter().map(|g| g.1.clone()).collect();
        rows.retain(|r| !removed.contains(&r.file));
        write_rows(&dir, &rows)?;
        let caps: String = std::fs::read_to_string(dir.join(CAPTIONS)).unwrap_or_else(|_| "file\tcaption\n".into()).lines().filter(|l| !l.split('\t').next().is_some_and(|f| removed.contains(f))).map(|l| format!("{l}\n")).collect();
        std::fs::write(dir.join(CAPTIONS), caps)?;
        let ap = dir.join(ALIASES);
        let mut al = if ap.exists() { std::fs::read_to_string(&ap)? } else { "kept\tremoved\tname\n".to_string() };
        for (k, f, name) in &gone {
            al.push_str(&format!("{k}\t{f}\t{name}\n"));
            let _ = std::fs::remove_file(dir.join(f));
            lines.push(format!("REMOVED {study}/{f} ({name}): same type as {k} (kept, archived first)"));
        }
        std::fs::write(&ap, al)?;
    }
    if lines.is_empty() {
        lines.push("no duplicate types".into());
    }
    Ok(lines)
}

/// The "Extendable families" section of a study page (one card per family; families of one
/// `extend group`, or that differ by one block kind only, share a card, as for clocks) and the
/// families' rows of `families.tsv` (the website's data).
fn family_cards(fams: &[(Row, clock::extend::Family, BTreeSet<String>)], instances: &[Instance], ftsv: &mut String, study: &str) -> String {
    use clock::extend::Family;
    if fams.is_empty() {
        return String::new();
    }
    let items: Vec<clock::catalog::CardItem> = fams
        .iter()
        .enumerate()
        .map(|(i, (r, f, k))| clock::catalog::CardItem { formula: f.period.text(), kinds: k.clone(), size: (f.blocks.eval(f.n_lo), r.volume, i), group: f.group.clone() })
        .collect();
    let mut cards = clock::catalog::card_groups(&items);
    cards.sort_by_key(|c| {
        let f = &fams[c.members[0]].1;
        (f.period.eval(f.n_lo), f.id.clone())
    });
    // the judged members by their axis value (`N = 2, 5, 8, 11, 14`), not by step n; a family
    // whose value does not change with n (a longer flying-machine train: same speed) by n, and a
    // run of every n as a range
    let judged = |f: &Family| -> String {
        let mut v: Vec<(i64, String)> = instances.iter().filter(|i| i.family == f.id).map(|i| (i.n, i.axis.clone())).collect();
        v.sort();
        let label = v.first().map(|(_, a)| axis_label(a).0).unwrap_or_default();
        let nums: Vec<String> = v.iter().map(|(_, a)| a.split_once(" = ").map(|x| x.1.to_string()).unwrap_or_else(|| a.clone())).collect();
        if nums.is_empty() {
            return String::new();
        }
        let ns: Vec<i64> = v.iter().map(|(n, _)| *n).collect();
        let every = ns.windows(2).all(|w| w[1] == w[0] + 1);
        if nums.len() > 1 && nums.iter().all(|x| *x == nums[0]) {
            let n = if every && ns.len() > 2 { format!("{}..{}", ns[0], ns[ns.len() - 1]) } else { ns.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ") };
            return format!("n = {n} (every one {label} = {})", nums[0]);
        }
        format!("{label} = {}", nums.join(", "))
    };
    let judged_n = |f: &Family| -> String { instances.iter().filter(|i| i.family == f.id).map(|i| i.n.to_string()).collect::<Vec<_>>().join(", ") };
    let mut md = String::from(
        "\n## Extendable families\n\nEach card: the value's formula in n, the size, how to extend it and the members judged in the game, by value (`mctas circuit archive` judges every member of a range of up to 16; `--fill-families` judges the ones an older archive skipped); the members are not repeated below.\n",
    );
    for card in &cards {
        let (r, f, _) = &fams[card.members[0]];
        let (label, unit) = axis_label(&r.axis);
        let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        let val = |f: &Family, n: i64| format!("{label} = {}{u}", f.period.eval(n));
        if card.members.len() == 1 {
            // its human title (`extend group title:`), else its id; its note (why a builder picks it)
            let title = f.group_title.clone().unwrap_or_else(|| f.id.clone());
            md.push_str(&format!("\n### {label} = {}{u}\n\n**{}** ([base design](archive/{})) · {} blocks · {}\n\n", f.period.text(), title, r.file, f.blocks.text(), r.kind));
            if let Some(n) = &f.group_note {
                md.push_str(&format!("{n}\n\n"));
            }
            let ns: Vec<i64> = [f.n_lo, f.n_lo + 1].into_iter().filter(|n| *n <= f.n_hi).collect();
            md.push_str(&ns.iter().map(|n| format!("n = {n}: {}, {} blocks", val(f, *n), f.blocks.eval(*n))).collect::<Vec<_>>().join(" · "));
            let j = judged(f);
            md.push_str(&format!(
                "\n\nExtend: {}. n = {}..{} ({} .. {}){}.\n",
                f.rule.trim_end_matches('.'),
                f.n_lo,
                f.n_hi,
                val(f, f.n_lo),
                val(f, f.n_hi),
                if j.is_empty() { String::new() } else { format!("; judged in the game: {j}") }
            ));
        } else {
            // a group: the general formula when given, else the formulas
            let fs: Vec<&Family> = card.members.iter().map(|&i| &fams[i].1).collect();
            let mut texts: Vec<String> = Vec::new();
            for g in &fs {
                if !texts.contains(&g.period.text()) {
                    texts.push(g.period.text());
                }
            }
            let heading = fs.iter().find_map(|g| g.group_period.clone()).unwrap_or_else(|| texts.join(" · "));
            // an explicit group (`extend group:`) takes its title and note from the first member
            // carrying them; a card of families grouped by their block kinds keeps a member's title
            // only when they all share it (else each row shows its own) and shows each distinct note
            let title = if card.group.is_some() {
                fs.iter().find_map(|g| g.group_title.clone()).or_else(|| card.group.clone()).unwrap_or_default()
            } else {
                match &fs[0].group_title {
                    Some(t) if fs.iter().all(|g| g.group_title.as_ref() == Some(t)) => t.clone(),
                    _ => format!("{} versions of one layout (one block swapped)", fs.len()),
                }
            };
            let mut notes: Vec<String> = Vec::new();
            for g in &fs {
                if let Some(n) = &g.group_note
                    && !notes.contains(n)
                    && (card.group.is_none() || notes.is_empty())
                {
                    notes.push(n.clone());
                }
            }
            let note = if notes.is_empty() {
                String::new()
            } else if card.group.is_some() {
                format!(" · {}", notes.join(" "))
            } else {
                format!("\n\n{}", notes.join(" "))
            };
            md.push_str(&format!("\n### {label} = {heading}{u}\n\n**{title}** · {} families{note}\n\n| family | {label} | blocks | n ({label}) | judged in the game |\n|---|---|---|---|---|\n", fs.len()));
            for &i in &card.members {
                let (mr, mf, _) = &fams[i];
                let name = mf.group_title.clone().filter(|_| card.group.is_none()).unwrap_or_else(|| mf.id.clone());
                md.push_str(&format!("| [{}](archive/{}) | {} | {} | {}..{} ({} .. {}) | {} |\n", name, mr.file, mf.period.text(), mf.blocks.text(), mf.n_lo, mf.n_hi, mf.period.eval(mf.n_lo), mf.period.eval(mf.n_hi), judged(mf)));
            }
        }
    }
    for (r, f, _) in fams {
        ftsv.push_str(
            &[study.to_string(), f.id.clone(), f.group.clone().unwrap_or_default(), r.kind.clone(), f.period.text(), f.blocks.text(), f.n_lo.to_string(), f.n_hi.to_string(), format!("{study}/archive/{}", r.file), f.rule.clone(), judged_n(f), judged(f)]
                .map(|s| s.replace(['\t', '\n'], " "))
                .join("\t"),
        );
        ftsv.push('\n');
    }
    md
}

/// A tunable group's name from a member's file: the stem without its trailing value
/// (`pulse-extender-repeater-L8.txt` -> `pulse-extender-repeater`).
fn group_label(file: &str) -> String {
    let stem = file.trim_end_matches(".txt");
    if let Some((head, tail)) = stem.rsplit_once('-') {
        let t = tail.trim_start_matches(['L', 'N', 'P', 'n']);
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
            return head.to_string();
        }
    }
    stem.to_string()
}

/// The axis values of a group as a short range: (`4..38`, ` (every 2)`), (`4, 6, 8`, ``).
fn range_text(vals: &[i64]) -> (String, String) {
    let mut v: Vec<i64> = vals.to_vec();
    v.sort();
    v.dedup();
    fn gcd(a: i64, b: i64) -> i64 {
        if b == 0 { a.abs() } else { gcd(b, a % b) }
    }
    match v.len() {
        0 => (String::new(), String::new()),
        1 => (v[0].to_string(), String::new()),
        n if n <= 3 => (v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "), String::new()),
        n => {
            let g = v.windows(2).fold(0, |g, w| gcd(g, w[1] - w[0]));
            let full = ((v[n - 1] - v[0]) / g.max(1) + 1) as usize;
            let note = if full == n { format!(" (every {g})") } else { format!(" (step {g}: {n} of {full} values)") };
            (format!("{}..{}", v[0], v[n - 1]), note)
        }
    }
}

/// The number of family cards on a study page (families of one group share a card).
fn clock_card_count(fams: &[(Row, clock::extend::Family, BTreeSet<String>)]) -> usize {
    if fams.is_empty() {
        return 0;
    }
    let items: Vec<clock::catalog::CardItem> = fams
        .iter()
        .enumerate()
        .map(|(i, (r, f, k))| clock::catalog::CardItem { formula: f.period.text(), kinds: k.clone(), size: (f.blocks.eval(f.n_lo), r.volume, i), group: f.group.clone() })
        .collect();
    clock::catalog::card_groups(&items).len()
}

/// The "Tunable groups" section of a study page: one card per group (the value range as the
/// heading, the two smallest members, a compact table value -> blocks -> design).
fn tunable_cards(cards: &[(String, String, Vec<&Row>)], circuits: &BTreeMap<String, Circuit>, fams: &[(Row, clock::extend::Family, BTreeSet<String>)]) -> String {
    if cards.is_empty() {
        return String::new();
    }
    let mut md = String::from(
        "\n## Tunable groups\n\nEach card: ONE mechanism whose value is set by its repeater delays, its comparator modes or the length of its repeater line, or that comes in variants named by a `group:` header (the same layout with only those settings or parts changed); its members are not repeated below.\n",
    );
    for (name, kind, members) in cards {
        let (label, unit) = axis_label(&members[0].axis);
        let label = if label.is_empty() { "value".to_string() } else { label };
        let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        let vals: Vec<i64> = members.iter().filter_map(|r| axis_number(&r.axis)).collect();
        let meta = |k: &str| members.iter().find_map(|r| circuits.get(&r.file).and_then(|c| c.clock.design.meta.get(k).cloned()));
        let title = meta("group title").unwrap_or_else(|| name.clone());
        let note = meta("group note").unwrap_or_else(|| {
            if members.iter().any(|r| circuits.get(&r.file).is_some_and(|c| c.clock.design.meta.contains_key("group"))) {
                "set by the repeater delays and the length of the repeater line".to_string()
            } else {
                "the same layout, set by its repeater delays / comparator modes".to_string()
            }
        });
        let mut small: Vec<&&Row> = members.iter().collect();
        small.sort_by(|a, b| (a.blocks, axis_number(&a.axis), a.volume, &a.file).cmp(&(b.blocks, axis_number(&b.axis), b.volume, &b.file)));
        let smallest: Vec<String> = small.iter().take(2).map(|r| format!("[{}](archive/{}) ({}, {} blocks)", r.file.trim_end_matches(".txt"), r.file, r.axis, r.blocks)).collect();
        let (core, rnote) = range_text(&vals);
        // a non-numeric axis (a BUD's detected classes): the members' values themselves
        let heading = if vals.is_empty() {
            let mut v: Vec<&str> = members.iter().map(|r| r.axis.as_str()).collect();
            v.sort();
            v.dedup();
            v.join(" / ")
        } else {
            format!("{label} = {core}{u}{rnote}")
        };
        md.push_str(&format!("\n### {heading}\n\n**{title}** · {kind} · {} designs · {note}. Smallest: {}.\n", members.len(), smallest.join(", ")));
        let fam: Vec<String> = fams
            .iter()
            .filter(|(fr, _, _)| circuits.get(&fr.file).and_then(|c| c.clock.design.meta.get("group")).is_some_and(|g| g == name))
            .map(|(fr, f, _)| format!("[{}](archive/{}): {label} = {}{u}, {} blocks", f.id, fr.file, f.period.text(), f.blocks.text()))
            .collect();
        if !fam.is_empty() {
            md.push_str(&format!("\nAlso as an extendable family: {}.\n", fam.join("; ")));
        }
        md.push_str(&format!("\n| {label} | blocks | design | metrics |\n|---|---|---|---|\n"));
        for r in members {
            md.push_str(&format!("| {} | {} | [{}](archive/{}) | {} |\n", axis_number(&r.axis).map(|v| format!("{v}{u}")).unwrap_or_else(|| r.axis.clone()), r.blocks, r.file.trim_end_matches(".txt"), r.file, r.metrics));
        }
    }
    md
}

/// `mctas circuit catalog`: every study's CATALOG.md and variants.md, the index, catalog.tsv and
/// families.tsv, from the archives' rows (no game). Returns one line per study.
pub fn catalog(root: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    let mut index = String::from(
        "# Contraption studies\n\nEvery design judged in the real game (`mctas circuit verify`: 4 positions x 4 rotations, [docs/circuits.md](../../docs/circuits.md)), by study; the data the website renders is [catalog.tsv](catalog.tsv). Generated by `mctas circuit catalog`.\n\n| study | designs | shown | variants |\n|---|---|---|---|\n",
    );
    let mut tsv = String::from("study\tkind\taxis\tname\tpath\tmetrics\tblocks\tdims\tfamily\trobustness\tvariant_of\tcaption\tgroup\n");
    let mut ftsv = String::from("study\tfamily\tgroup\tkind\taxis_formula\tblocks_formula\tn_lo\tn_hi\tbase\trule\tjudged_n\tjudged_values\n");
    for (study, title) in studies() {
        let dir = archive_dir(root, study);
        let rows = read_rows(&dir)?;
        let stale: Vec<&str> = rows.iter().filter(|r| !dir.join(&r.file).exists()).map(|r| r.file.as_str()).collect();
        if !stale.is_empty() {
            bail!("{study}: verdict rows without their design file: {} (a removal undone by a merge of the TSVs? `mctas circuit archive --dedupe` drops them)", stale.join(", "));
        }
        let caps = read_captions(&dir);
        let aliases = read_aliases(&dir);
        // the input class of each design (critique 2026-10-20): inputs that are the design's own
        // levers or buttons are a different class from signal inputs (the judge's repeater line,
        // R7), and the two never fold into each other or count as equivalent layouts
        let class: BTreeMap<String, String> = rows.iter().map(|r| (r.file.clone(), input_class(&dir.join(&r.file)))).collect();
        let cls = |r: &Row| class.get(&r.file).cloned().unwrap_or_default();
        // extendable families (bases with `extend` headers) and their judged members: one card
        // each (cards of one group share one), the members leave the value tables
        let mut fams: Vec<(Row, clock::extend::Family, BTreeSet<String>)> = Vec::new();
        for r in &rows {
            if let Ok(c) = Circuit::load(&dir.join(&r.file))
                && let Ok(Some(f)) = family_of(&c)
            {
                fams.push((r.clone(), f, c.kinds()));
            }
        }
        let instances = read_instances(&dir);
        let member_keys: BTreeSet<&String> = instances.iter().filter(|i| fams.iter().any(|(_, f, _)| f.id == i.family)).map(|i| &i.type_key).collect();
        let in_family = |r: &Row| fams.iter().any(|(fr, _, _)| fr.file == r.file) || member_keys.contains(&r.type_key);
        // TUNABLE GROUPS (vjeux 2026-10-19): designs that are one mechanism with other repeater
        // delays, comparator modes or a longer U are ONE card: a `group: <name>` header (stamped by
        // the generators), else the same tunable key (delays and modes masked) and kind
        let circuits: BTreeMap<String, Circuit> = rows.iter().filter_map(|r| Circuit::load(&dir.join(&r.file)).ok().map(|c| (r.file.clone(), c))).collect();
        let mut tgroups: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| !in_family(r)) {
            let Some(c) = circuits.get(&r.file) else { continue };
            let (kind_key, id) = match c.clock.design.meta.get("group") {
                // an explicit group spans the kind's argument too (`counter N`: one card for
                // every N of one mechanism)
                Some(g) => (r.kind.split_whitespace().next().unwrap_or(&r.kind).to_string(), format!("h:{g}")),
                None => (r.kind.clone(), format!("t:{}", tunable_key(c))),
            };
            tgroups.entry((kind_key, id)).or_default().push(r);
        }
        tgroups.retain(|_, v| v.len() >= 2);
        let mut group_of: BTreeMap<String, String> = BTreeMap::new();
        let mut group_cards: Vec<(String, String, Vec<&Row>)> = Vec::new();
        for ((kind, id), members) in &mut tgroups {
            members.sort_by(|a, b| (axis_key(&a.axis), a.blocks, a.volume, &a.file).cmp(&(axis_key(&b.axis), b.blocks, b.volume, &b.file)));
            let name = match id.strip_prefix("h:") {
                Some(g) => g.to_string(),
                None => group_label(&members.iter().min_by_key(|r| (r.blocks, r.volume, r.file.clone())).map(|r| r.file.clone()).unwrap_or_default()),
            };
            for m in members.iter() {
                group_of.insert(m.file.clone(), name.clone());
            }
            group_cards.push((name, kind.clone(), members.clone()));
        }
        group_cards.sort_by_key(|(n, k, m)| (axis_key(&m[0].axis), k.clone(), n.clone()));
        let in_group = |r: &Row| group_of.contains_key(&r.file);
        // the dominance fold within one kind and axis value (vjeux 2026-10-20: palette alone is
        // never a distinction): a design folds under one with fewer blocks (or as many and better
        // on a variation) that it does not beat on any variation metric or property
        // ([`beats_on_variation`]), whatever their block kinds
        let mut groups: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| !in_family(r) && !in_group(r)) {
            // study 2's axis is the BLOCK FAMILY, which is a palette label, not a capability: with
            // it in the key every mechanism becomes its own card and nothing ever folds, so a
            // visitor reads nine T flip-flops that tff-bulb beats on every metric (critique
            // 2026-10-20). Fold those by kind alone; the variation metrics still protect anything
            // that wins something real.
            let axis = if r.kind == "tff" { String::new() } else { r.axis.clone() };
            groups.entry((r.kind.clone(), axis)).or_default().push(r);
        }
        let mut host: BTreeMap<String, String> = BTreeMap::new();
        for g in groups.values() {
            // a crop harvester (study 18) is ranked by blocks PER CANE, not its total blocks: a
            // 2-cane slice of 13 blocks (6.5 per cane) is not a variant of a 1-cane one of 8
            let size = |r: &Row| if r.kind.starts_with("harvester") { farm_rank(r).or_else(|| per_cane_tenths(r).map(|t| t * 100_000)).unwrap_or(r.blocks * 1_000_000) } else { r.blocks };
            let items: Vec<(usize, i64, String)> = g.iter().map(|r| (size(r), r.volume, r.file.clone())).collect();
            let robust: Vec<bool> = g.iter().map(|r| is_robust(r)).collect();
            for (i, h) in clock::catalog::fold_hosts_robust(&items, &robust, |i, j| cls(g[i]) != cls(g[j]) || beats_on_variation(g[i], g[j])).into_iter().enumerate() {
                if let Some(h) = h {
                    host.insert(g[i].file.clone(), g[h].file.clone());
                }
            }
        }
        // EQUIVALENT LAYOUTS: the same kind, value, block count and variation signature (metrics,
        // flat, slime-free), whatever the block kinds: one shown, the others listed as "also N
        // other layouts" (catalog.tsv variant_of)
        let mut layout: BTreeMap<String, String> = BTreeMap::new();
        let mut layouts_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
        {
            let mut eq: BTreeMap<(String, String, usize, String), Vec<&Row>> = BTreeMap::new();
            for r in rows.iter().filter(|r| !in_family(r) && !in_group(r) && !host.contains_key(&r.file)) {
                eq.entry((r.kind.clone(), r.axis.clone(), r.blocks, format!("{}|{}", variation_signature(r), cls(r)))).or_default().push(r);
            }
            for v in eq.values_mut() {
                if v.len() < 2 {
                    continue;
                }
                v.sort_by(|a, b| (a.volume, &a.archived, &a.file).cmp(&(b.volume, &b.archived, &b.file)));
                for o in &v[1..] {
                    layout.insert(o.file.clone(), v[0].file.clone());
                    layouts_of.entry(v[0].file.clone()).or_default().push(o.file.clone());
                }
            }
        }
        let shown: Vec<&Row> = rows.iter().filter(|r| !host.contains_key(&r.file) && !in_family(r) && !in_group(r) && !layout.contains_key(&r.file)).collect();
        let cards_md = family_cards(&fams, &instances, &mut ftsv, study);
        let groups_md = tunable_cards(&group_cards, &circuits, &fams);
        let sdir = root.join(study);
        if rows.is_empty() {
            index.push_str(&format!("| {title} | 0 | 0 | 0 |\n"));
            continue;
        }
        std::fs::create_dir_all(&sdir)?;
        // by axis value (numbers ascending), then the smallest first
        let mut by_axis: BTreeMap<(i64, String), Vec<&Row>> = BTreeMap::new();
        for r in &shown {
            by_axis.entry(axis_key(&r.axis)).or_default().push(r);
        }
        let judged = if study == "mobfarm" { "judged on redsim's mob layer (each caption gives its game check in the real server)" } else { "judged in the real game" };
        let mut md = format!(
            "# {title}\n\n{} designs {judged}: {} shown, {} variants (no fewer blocks and no better on a variation a builder would choose on than a design with the same value, whatever their block kinds, or not robust where it is: [variants](variants.md)), {} other layouts (same kind, value, size, metrics and flatness as a shown design){}{}. [All studies](../CATALOG.md) · [docs/circuits.md](../../../docs/circuits.md). Generated by `mctas circuit catalog`.\n",
            rows.len(),
            shown.len(),
            host.len(),
            layout.len(),
            if fams.is_empty() { String::new() } else { format!(", {} extendable famil{} (their members are on the cards)", fams.len(), if fams.len() == 1 { "y" } else { "ies" }) },
            if group_cards.is_empty() { String::new() } else { format!(", {} tunable group{} ({} designs on their cards)", group_cards.len(), if group_cards.len() == 1 { "" } else { "s" }, group_of.len()) }
        );
        md.push_str(&cards_md);
        md.push_str(&groups_md);
        for ((_, ax), rs) in &mut by_axis {
            rs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
            md.push_str(&format!("\n## {ax}\n\n| design | kind | metrics | blocks | box | robustness | how it works |\n|---|---|---|---|---|---|---|\n"));
            for r in rs.iter() {
                let also = aliases.get(&r.file).map(|v| format!(" (also found as {})", v.join(", "))).unwrap_or_default();
                let others = layouts_of
                    .get(&r.file)
                    .map(|v| format!(" (also {} other layout{}: {})", v.len(), if v.len() == 1 { "" } else { "s" }, v.iter().map(|f| format!("[{}](archive/{f})", f.trim_end_matches(".txt"))).collect::<Vec<_>>().join(", ")))
                    .unwrap_or_default();
                md.push_str(&format!(
                    "| [{}](archive/{}){also}{others} | {} | {} | {} | {} | {} | {} |\n",
                    r.file.trim_end_matches(".txt"),
                    r.file,
                    r.kind,
                    r.metrics,
                    r.blocks,
                    r.dims,
                    r.robustness,
                    caps.get(&r.file).map(|s| s.replace('|', "/")).unwrap_or_default()
                ));
            }
        }
        std::fs::write(sdir.join("CATALOG.md"), md)?;
        let mut vmd = format!("# {title}: variants\n\n[{title}](CATALOG.md)\n\nA variant has the same kind and value as a smaller design (its host) and uses every block kind the host does, plus others.\n");
        let mut hosts: BTreeMap<&String, Vec<&Row>> = BTreeMap::new();
        for r in &rows {
            if let Some(h) = host.get(&r.file) {
                hosts.entry(h).or_default().push(r);
            }
        }
        for (h, vs) in &hosts {
            let hr = rows.iter().find(|r| &&r.file == h).unwrap();
            vmd.push_str(&format!("\n**[{}](archive/{})** ({}, {}; {} blocks: {}):\n\n| variant | blocks | adds |\n|---|---|---|\n", hr.file.trim_end_matches(".txt"), hr.file, hr.kind, hr.axis, hr.blocks, hr.kinds.replace('+', ", ")));
            let hk = hr.kind_set();
            for v in vs {
                let adds: Vec<String> = v.kind_set().difference(&hk).cloned().collect();
                vmd.push_str(&format!("| [{}](archive/{}) | {} | {} |\n", v.file.trim_end_matches(".txt"), v.file, v.blocks, if adds.is_empty() { "-".to_string() } else { adds.join(", ") }));
            }
        }
        std::fs::write(sdir.join("variants.md"), vmd)?;
        for r in &rows {
            tsv.push_str(
                &[
                    study.to_string(),
                    r.kind.clone(),
                    r.axis.clone(),
                    r.file.trim_end_matches(".txt").to_string(),
                    format!("{study}/archive/{}", r.file),
                    r.metrics.clone(),
                    r.blocks.to_string(),
                    r.dims.clone(),
                    r.family.clone(),
                    r.robustness.clone(),
                    host.get(&r.file).or_else(|| layout.get(&r.file)).cloned().unwrap_or_default(),
                    caps.get(&r.file).cloned().unwrap_or_default(),
                    group_of.get(&r.file).cloned().unwrap_or_default(),
                ]
                .map(|s| s.replace(['\t', '\n'], " "))
                .join("\t"),
            );
            tsv.push('\n');
        }
        let cards = shown.len() + group_cards.len() + clock_card_count(&fams);
        // the card count without tunable groups and layouts (the fold over every non-family row)
        let before = {
            let mut g: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
            for r in rows.iter().filter(|r| !in_family(r)) {
                g.entry((r.kind.clone(), r.axis.clone())).or_default().push(r);
            }
            let mut folded = 0usize;
            for v in g.values() {
                let items: Vec<(usize, i64, String)> = v.iter().map(|r| (r.blocks, r.volume, r.file.clone())).collect();
                let robust: Vec<bool> = v.iter().map(|r| is_robust(r)).collect();
                folded += clock::catalog::fold_hosts_robust(&items, &robust, |i, j| cls(v[i]) != cls(v[j]) || beats_on_variation(v[i], v[j])).into_iter().filter(|h| h.is_some()).count();
            }
            rows.iter().filter(|r| !in_family(r)).count() - folded + clock_card_count(&fams)
        };
        index.push_str(&format!("| [{title}]({study}/CATALOG.md) | {} | {} | {} |\n", rows.len(), shown.len(), host.len()));
        lines.push(format!(
            "{study}: {} designs, {} shown, {} variants, {} other layouts, {} families, {} tunable groups ({} designs), {} values; cards {before} -> {cards}",
            rows.len(),
            shown.len(),
            host.len(),
            layout.len(),
            fams.len(),
            group_cards.len(),
            group_of.len(),
            by_axis.len()
        ));
    }
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("CATALOG.md"), index)?;
    std::fs::write(root.join("catalog.tsv"), tsv)?;
    std::fs::write(root.join("families.tsv"), ftsv)?;
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(file: &str, metrics: &str, blocks: usize, dims: &str, kinds: &str) -> Row {
        Row::parse(&format!("2026-10-20T00:00Z\t{file}\tpiston-extender 5\tN = 5\t{metrics}\t{blocks}\t{dims}\t0\t-\trobust\t{kinds}\t-\t-\t-")).unwrap()
    }

    /// vjeux 2026-10-20: pext-n5-sf-sweep136 (136 blocks, 230/228 gt) is no better than
    /// pext-n5-sf105 (105 blocks, 149/147 gt) on anything: it folds although it has no plain piston
    /// (palette alone is never a distinction); a faster larger design, a flat one, or one with
    /// another class keeps its card.
    #[test]
    fn variation_not_palette() {
        let small = row("sf105.txt", "N 5, extend 149 gt, retract 147 gt", 105, "9x5x4", "observer+piston+sticky_piston+stone");
        let sweep = row("sweep136.txt", "N 5, extend 230 gt, retract 228 gt", 136, "9x5x6", "observer+sticky_piston+stone");
        assert!(!beats_on_variation(&sweep, &small));
        assert!(beats_on_variation(&small, &sweep));
        let faster = row("fast.txt", "N 5, extend 140 gt, retract 150 gt", 140, "9x5x4", "observer+sticky_piston+stone");
        assert!(beats_on_variation(&faster, &small));
        let flat = row("flat.txt", "N 5, extend 149 gt, retract 147 gt", 120, "9x2x4", "observer+sticky_piston+stone");
        assert!(beats_on_variation(&flat, &small));
        let classed = row("class.txt", "N 5, extend 149 gt, retract 147 gt, edge falling", 120, "9x5x4", "observer+sticky_piston+stone");
        assert!(beats_on_variation(&classed, &small));
        let items: Vec<(usize, i64, String)> = [&small, &sweep, &faster].iter().map(|r| (r.blocks, r.volume, r.file.clone())).collect();
        let rows = [&small, &sweep, &faster];
        assert_eq!(clock::catalog::fold_hosts_by(&items, |i, j| beats_on_variation(rows[i], rows[j])), vec![None, Some(0), None]);
    }
}
