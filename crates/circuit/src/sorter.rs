//! Study 13, item sorters (designs/circuits/tracks/sorter/WR.md, docs/circuits.md "Item
//! sorters"): kind `sorter <item>`. A slice takes items from a line, sends every item of its kind
//! into its chest and lets every other item go on along the line.
//!
//! Judged in the game by its own runner, like the shulker kinds ([`crate::shulker`]), on what a
//! player does with a sorter (vjeux 2026-10-21: "REAL USE IS THE BAR"): a ROW of slices side by
//! side, fed by an input chest, a hopper line or a water stream, mixed items and full stacks at
//! the line's full speed, non-stackable items going through to the end, and a slice's chest that
//! fills up. The judge puts a chest on each `items`, `pass` and `output` port (air cells, not
//! counted), keeps the source topped up from a stream of stacks, counts and empties `pass` and the
//! outputs at each visit, then waits for rest and takes an inventory of the watched box.
//!
//! The runs ([`Run`]):
//! * `base` (16 builds, the metrics): one slice fed from an input chest on `items`: the design's
//!   kind mixed with 64-stack, 16-stack and non-stackable items, full and partial stacks, runs of
//!   the kind back to back. Every item of the kind must reach `output`, every other item `pass`,
//!   nothing else; the design's containers must end as they started (the filter recipe intact), no
//!   item entity, then rest.
//! * `tiled` (required, 4 rotations): a row of [`COPIES`] slices at the `tile:` step chained along
//!   the line, each sorting its own kind (the judge rewrites the recipe's item), fed by a HOPPER
//!   LINE: the judge's line hopper one step before the first slice (where the slice before it
//!   would have its first hopper), fed from a chest above it; every later slice is fed by the
//!   slice before it.
//! * `stream` (required, 4 rotations): the same row fed by a WATER STREAM ending on that line
//!   hopper: every stack of the stream arrives as an item entity at rest above it, a few waiting at
//!   a time; the hopper takes whole stacks and passes them on one item at a time.
//! * `jam` (required, 4 rotations): the hopper-fed row with the middle copy's chest full from the
//!   start ([`JAM_FILLER`] in every slot); its kind keeps coming ([`JAM_STACKS`] stacks). Once
//!   the line is quiet the judge empties that chest, waits for the line to settle and supplies a
//!   tail. Required: nothing ever reaches a wrong chest, no other copy's kind goes to the end of
//!   the line, every copy ends with its recipe, everything is delivered, and after the chest is
//!   emptied the tail's items of its kind reach it (the slice sorts again). Badge
//!   `overflow-proof`: while that chest is full the line keeps moving (every other item supplied
//!   reaches its place before the judge empties it: the full slice's surplus goes on to the end).
//! * `direct` (badge `stream-direct`): the row with the water stream ending right on the first
//!   slice (the stream's item entities dropped in the `items` cell, onto the design's own first
//!   hopper, which takes whole stacks).

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, SEG, Stack, items_snbt, stacks_of_be};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// Kinds the stream carries besides the sorted ones, by max stack; a run leaves out any kind the
/// design itself holds (its recipe and fillers). All in redsim's item registry, so the redsim
/// screen ([`sim_run`]) runs the same scenarios as the game.
pub const JUNK64: [&str; 5] = ["minecraft:cobblestone", "minecraft:stone", "minecraft:iron_block", "minecraft:slime_block", "minecraft:heavy_weighted_pressure_plate"];
pub const JUNK16: [&str; 3] = ["minecraft:snowball", "minecraft:ender_pearl", "minecraft:bucket"];
pub const JUNK1: [&str; 3] = ["minecraft:iron_shovel", "minecraft:wooden_shovel", "minecraft:diamond_sword"];
/// The kinds the tiled copies sort (copy 1, 2, ...), 64-stack like most recipes.
pub const COPY_KINDS64: [&str; 4] = ["minecraft:redstone", "minecraft:slime_ball", "minecraft:iron_nugget", "minecraft:iron_bars"];
/// The same for a 16-stack recipe.
pub const COPY_KINDS16: [&str; 3] = ["minecraft:ender_pearl", "minecraft:bucket", "minecraft:snowball"];
/// Copies in the row runs.
pub const COPIES: usize = 3;
/// The `row` run (storage rows, `storage: row`): the slices in the row, and the kinds copies 1..
/// sort (64-stack recipes; a 16-stack recipe's row has 1 + [`COPY_KINDS16`] slices).
pub const ROW: usize = 8;
pub const ROW_KINDS64: [&str; 7] = ["minecraft:redstone", "minecraft:slime_ball", "minecraft:iron_nugget", "minecraft:iron_bars", "minecraft:gold_ingot", "minecraft:copper_ingot", "minecraft:raw_iron"];
/// The `jam` run: the stacks of its kind the jammed copy gets.
pub const JAM_STACKS: u32 = 8;
/// The `jam` run: what fills the jammed chest, one per slot (non-stackable: every slot full).
pub const JAM_FILLER: &str = "minecraft:music_disc_11";
/// The `jam` run: the gt with nothing delivered and nothing taken from the judge's supply before
/// the judge empties the jammed chest (and, after it, before the tail).
pub const QUIET: u32 = 200;
/// The water-stream runs: the stream's stacks kept waiting as item entities at its end.
pub const WAITING: usize = 2;
/// Why a design without `tile:` fails.
pub const NO_TILE: &str = "a sorter is a row of slices side by side: it needs a `tile: dx dy dz` header (the step from one slice to the next along the line)";

/// `minecraft:` + the id.
pub fn full_id(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") }
}

/// An item's max stack (redsim's registry; 64 for the kinds it does not know).
pub fn max_stack(id: &str) -> u32 {
    redsim::redstone::container::max_stack_of(&full_id(id)) as u32
}

/// How the judge's items come in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feed {
    /// An input chest on `items` (the judge's chest): the design's first hopper pulls from it.
    Chest,
    /// A hopper line: the judge's line hopper one step before the first slice (where the slice
    /// before it would have its first hopper: the row's `feed` cell), pointing into the hopper
    /// under `items`, pushes one item every 8 gt; it pulls them from the judge's chest above it.
    /// (A hopper ABOVE the first line hopper would feed it twice as fast: that hopper pushes down
    /// while the line hopper pulls from it.)
    Hopper,
    /// A water stream ending on the line hopper before the first slice (the `feed` cell): every
    /// stack of the stream an item entity at rest above it, [`WAITING`] at a time; that hopper takes
    /// whole stacks and passes them on one item at a time.
    Stream,
    /// A water stream ending right on the first slice: the item entities in the `items` cell, onto
    /// the design's own first hopper.
    Direct,
}

/// The judge's runs ([`verify_in_pool`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Base,
    Tiled,
    Stream,
    Jam,
    Direct,
    Intrusion,
    /// Storage rows (`storage: row`): [`ROW`] slices fed from an input chest, ending in the
    /// overflow chest (`pass`).
    Row,
}

/// The `intrusion` run's stray items (the real-use audit's, [`crate::intrusion::DROPS`]): one per
/// moment, each dropped on every open hopper top of the row.
pub const STRAYS: [(&str, u32); 3] = [("minecraft:cobblestone", 1), ("minecraft:iron_ingot", 64), ("minecraft:iron_shovel", 1)];

impl Run {
    pub const ALL: [Run; 7] = [Run::Base, Run::Tiled, Run::Stream, Run::Jam, Run::Intrusion, Run::Direct, Run::Row];
    /// The runs a sorter must pass besides the base run.
    pub const REQUIRED: [Run; 4] = [Run::Tiled, Run::Stream, Run::Jam, Run::Intrusion];

    /// The runs a design must pass besides the base run: a storage row (`storage: row`) adds
    /// the long chest-fed row (and its `jam` run must keep the line moving: [`overflow_required`]).
    pub fn required(c: &Circuit) -> Vec<Run> {
        let mut v = Run::REQUIRED.to_vec();
        if storage(c) {
            v.push(Run::Row);
        }
        v
    }

    pub fn name(self) -> &'static str {
        match self {
            Run::Base => "base",
            Run::Tiled => "tiled",
            Run::Stream => "stream",
            Run::Jam => "jam",
            Run::Direct => "direct",
            Run::Intrusion => "intrusion",
            Run::Row => "row",
        }
    }

    /// `base`, `tiled`, `stream`, `jam`, `direct`, `intrusion`, `row` (and the old badge names
    /// `tileable`, `overflow`).
    pub fn parse(s: &str) -> Option<Run> {
        Some(match s {
            "base" => Run::Base,
            "tiled" | "tileable" => Run::Tiled,
            "stream" | "water" => Run::Stream,
            "jam" | "overflow" => Run::Jam,
            "direct" => Run::Direct,
            "intrusion" | "strays" => Run::Intrusion,
            "row" | "storage" => Run::Row,
            _ => return None,
        })
    }

    /// What the run is, for a verdict.
    pub fn title(self) -> &'static str {
        match self {
            Run::Base => "one slice fed from an input chest",
            Run::Tiled => "slices side by side fed by a hopper line",
            Run::Stream => "slices side by side fed by a water stream (ending on the line's hopper before the first slice)",
            Run::Jam => "slices side by side with the middle slice's chest full",
            Run::Direct => "a water stream ending right on the first slice",
            Run::Intrusion => "stray items dropped on every open hopper top of the row",
            Run::Row => "a storage row: 8 slices fed from an input chest, ending in an overflow chest",
        }
    }
}

/// The `storage` metric of a storage row.
pub const STORAGE_METRIC: &str = "row of 8 slices from an input chest, overflow chest, overflow never blocks";

/// The `storage` metric: a bulk slice (an `output.b` port: a double chest) says so first.
pub fn storage_metric(c: &Circuit) -> String {
    if c.outputs.iter().any(|o| o.name.split('.').any(|s| s == "b")) { format!("bulk (a double chest a slice, 54 stacks), {STORAGE_METRIC}") } else { STORAGE_METRIC.to_string() }
}

/// A storage row (`storage: row` in the design): the sorter judge plus the long chest-fed row,
/// and the `jam` run's line must keep moving while a chest is full (a storage system's overflow
/// never blocks its line).
pub fn storage(c: &Circuit) -> bool {
    c.clock.design.meta.get("storage").is_some_and(|v| v.trim() == "row")
}

/// Why a storage row fails the `jam` run's overflow check (`v`: the `overflow-proof` metric).
pub fn overflow_required(v: &str) -> String {
    format!("a storage row's line must keep moving while a chest is full (the overflow chest takes the surplus): overflow-proof {v}")
}

/// One run's inputs.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub run: Run,
    /// The kind each copy sorts, copy 0 first (one entry: a single slice).
    pub sorts: Vec<String>,
    /// What the source supplies, in order.
    pub stream: Vec<Stack>,
    pub feed: Feed,
    /// The `jam` run: this copy's chest starts full; once the line is quiet the judge empties it,
    /// waits for the line to settle and supplies `tail`.
    pub jam: Option<usize>,
    pub tail: Vec<Stack>,
    /// The `intrusion` run: (gt, stack) dropped on every open hopper top at the first visit from
    /// that gt on.
    pub strays: Vec<(u32, Stack)>,
    /// Ticks after the start before giving up.
    pub cap: u32,
}

fn pick<'a>(from: &[&'a str], avoid: &[String], n: usize) -> Vec<&'a str> {
    from.iter().filter(|k| !avoid.iter().any(|a| a == *k)).take(n).copied().collect()
}

impl Scenario {
    fn with_cap(run: Run, sorts: Vec<String>, stream: Vec<Stack>, feed: Feed, jam: Option<usize>, tail: Vec<Stack>) -> Scenario {
        let n: u64 = stream.iter().chain(tail.iter()).map(|s| s.count as u64).sum();
        // one hopper's pace (8 gt an item) x 1.5, + a jam's drain and settling, + 3000
        let cap = (n * 12) as u32 + if jam.is_some() { 8000 } else { 0 } + 3000;
        Scenario { run, sorts, stream, feed, jam, tail, strays: Vec::new(), cap }
    }

    /// The base run for a design sorting `kind`, avoiding the kinds the design holds.
    pub fn base(kind: &str, avoid: &[String]) -> Result<Scenario> {
        let k = full_id(kind);
        let mut av = avoid.to_vec();
        av.push(k.clone());
        let j = pick(&JUNK64, &av, 3);
        let s = pick(&JUNK16, &av, 2);
        let u = pick(&JUNK1, &av, 1);
        if j.len() < 3 || s.len() < 2 || u.is_empty() {
            bail!("sorter: the design holds too many of the judge's stream kinds ({av:?})");
        }
        let st = |id: &str, n: u32| Stack::new(id, n.min(max_stack(id)));
        let stream = vec![
            st(&k, 64), st(j[0], 64), st(&k, 20), st(s[0], 16), st(j[1], 37), st(&k, 64), st(&k, 64), st(u[0], 1), st(s[1], 5), st(&k, 33), st(j[2], 64), st(u[0], 1), st(&k, 1), st(j[0], 10), st(&k, 64),
        ];
        Ok(Scenario::with_cap(Run::Base, vec![k], stream, Feed::Chest, None, Vec::new()))
    }

    /// A run of the whole judge for a design sorting `kind`.
    pub fn for_run(run: Run, kind: &str, avoid: &[String]) -> Result<Scenario> {
        if run == Run::Base {
            return Scenario::base(kind, avoid);
        }
        if run == Run::Row {
            return Scenario::row(kind, avoid);
        }
        let k0 = full_id(kind);
        let mut av = avoid.to_vec();
        av.push(k0.clone());
        let pool: &[&str] = if max_stack(&k0) == 16 { &COPY_KINDS16 } else { &COPY_KINDS64 };
        let more = pick(pool, &av, COPIES - 1);
        if more.len() < COPIES - 1 {
            bail!("sorter: no copy kinds left ({av:?})");
        }
        let mut sorts = vec![k0.clone()];
        sorts.extend(more.iter().map(|s| s.to_string()));
        av.extend(sorts.iter().cloned());
        let j = pick(&JUNK64, &av, 3);
        let s = pick(&JUNK16, &av, 2);
        let u = pick(&JUNK1, &av, 2);
        if j.len() < 3 || u.is_empty() {
            bail!("sorter: the design holds too many of the judge's stream kinds ({av:?})");
        }
        let u1 = u.get(1).copied().unwrap_or(u[0]);
        // a 16-stack recipe's copies use up the 16-stack kinds: 64-stack junk stands in
        let s: Vec<&str> = (0..2).map(|i| s.get(i).copied().unwrap_or(j[i])).collect();
        let st = |id: &str, n: u32| Stack::new(id, n.min(max_stack(id)));
        let (a, b, c) = (sorts[0].as_str(), sorts[1].as_str(), sorts[2].as_str());
        let feed = match run {
            Run::Stream => Feed::Stream,
            Run::Direct => Feed::Direct,
            _ => Feed::Hopper,
        };
        if run == Run::Jam {
            let mut stream = Vec::new();
            for i in 0..JAM_STACKS {
                stream.push(st(b, 64));
                if i == 3 {
                    stream.extend([st(j[0], 64), st(a, 20), st(u[0], 1)]);
                }
            }
            stream.extend([st(c, 30), st(s[0], 16), st(j[1], 37), st(u1, 1), st(a, 5)]);
            let tail = vec![st(b, 20), st(j[2], 10), st(c, 5), st(u[0], 1), st(a, 3)];
            return Ok(Scenario::with_cap(run, sorts, stream, feed, Some(1), tail));
        }
        // non-stackables back to back (two in a row, as a pile of tools would come)
        let stream = vec![
            st(a, 64), st(j[0], 64), st(b, 64), st(c, 20), st(s[0], 16), st(b, 30), st(a, 10), st(u[0], 1), st(u1, 1), st(c, 64), st(j[1], 37), st(a, 64), st(b, 1), st(s[1], 5), st(c, 33), st(j[2], 64), st(u[0], 1), st(a, 1), st(b, 64),
        ];
        let mut sc = Scenario::with_cap(run, sorts, stream, feed, None, Vec::new());
        if run == Run::Intrusion {
            // a quarter, half and three quarters of the stream's time at the line's pace (8 gt an
            // item): the real-use audit's moments
            let span = sc.total() as u32 * 8;
            sc.strays = STRAYS.iter().enumerate().map(|(k, (id, n))| (span * (k as u32 + 1) / 4, Stack::new(id, *n))).collect();
        }
        Ok(sc)
    }

    /// Items supplied of a kind (stream and tail).
    pub fn supplied(&self, id: &str) -> u64 {
        self.stream.iter().chain(self.tail.iter()).filter(|s| s.id == id).map(|s| s.count as u64).sum()
    }

    /// The `row` run (storage rows): [`ROW`] slices, each its own kind, fed from an input chest
    /// on copy 0: a full stack of every kind, junk between them (64-stack, 16-stack and
    /// non-stackable), then a partial stack of every kind in the reverse order; everything else
    /// ends in the overflow chest (`pass`).
    pub fn row(kind: &str, avoid: &[String]) -> Result<Scenario> {
        let k0 = full_id(kind);
        let mut av = avoid.to_vec();
        av.push(k0.clone());
        let (pool, n): (&[&str], usize) = if max_stack(&k0) == 16 { (&COPY_KINDS16, 1 + COPY_KINDS16.len()) } else { (&ROW_KINDS64, ROW) };
        let more = pick(pool, &av, n - 1);
        if more.len() < n - 1 {
            bail!("sorter: no row kinds left ({av:?})");
        }
        let mut sorts = vec![k0.clone()];
        sorts.extend(more.iter().map(|s| s.to_string()));
        av.extend(sorts.iter().cloned());
        let j = pick(&JUNK64, &av, 3);
        let s = pick(&JUNK16, &av, 2);
        let u = pick(&JUNK1, &av, 2);
        if j.len() < 3 || u.is_empty() {
            bail!("sorter: the design holds too many of the judge's stream kinds ({av:?})");
        }
        let u1 = u.get(1).copied().unwrap_or(u[0]);
        let s: Vec<&str> = (0..2).map(|i| s.get(i).copied().unwrap_or(j[i])).collect();
        let st = |id: &str, n: u32| Stack::new(id, n.min(max_stack(id)));
        let mut stream = Vec::new();
        for (i, k) in sorts.iter().enumerate() {
            stream.push(st(k, 64));
            match i {
                1 => stream.push(st(j[0], 64)),
                3 => stream.extend([st(s[0], 16), st(u[0], 1), st(u1, 1)]),
                5 => stream.push(st(j[1], 37)),
                _ => {}
            }
        }
        for k in sorts.iter().rev() {
            stream.push(st(k, 10));
        }
        stream.extend([st(j[2], 64), st(u[0], 1), st(s[1], 5)]);
        Ok(Scenario::with_cap(Run::Row, sorts, stream, Feed::Chest, None, Vec::new()))
    }

    pub fn total(&self) -> u64 {
        self.stream.iter().chain(self.tail.iter()).map(|s| s.count as u64).sum()
    }

    /// The kinds supplied (and dropped as strays), sorted kinds first.
    pub fn kinds(&self) -> Vec<String> {
        let mut v = self.sorts.clone();
        for s in self.stream.iter().chain(self.tail.iter()).chain(self.strays.iter().map(|s| &s.1)) {
            if !v.contains(&s.id) {
                v.push(s.id.clone());
            }
        }
        v
    }

    /// The census's quick screen: a short stream (the kind, 64-stack, 16-stack and non-stackable
    /// junk, 14 items).
    pub fn quick(kind: &str, avoid: &[String]) -> Result<Scenario> {
        let full = Scenario::base(kind, avoid)?;
        let k = full.sorts[0].clone();
        let junk: Vec<String> = full.kinds().into_iter().filter(|x| *x != k).collect();
        let by = |n: u32| junk.iter().find(|j| max_stack(j) == n).cloned().unwrap_or_default();
        let (j, s, u) = (by(64), by(16), by(1));
        let st = |id: &str, n: u32| Stack::new(id, n.min(max_stack(id)));
        let stream = vec![st(&k, 3), st(&j, 3), st(&k, 1), st(&s, 2), st(&k, 2), st(&u, 1), st(&j, 2)];
        Ok(Scenario::with_cap(Run::Base, vec![k], stream, Feed::Chest, None, Vec::new()))
    }
}

/// One run in the game.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// Per sink port (`pass`, `output`, `output.t1`...): items of each kind that reached it.
    pub reached: BTreeMap<String, BTreeMap<String, u64>>,
    /// (gt, items delivered so far, every sink) at every change: the flow.
    pub flow: Vec<(u32, u64)>,
    pub supplied: u64,
    /// Items handed to the judge's supply so far, per kind.
    pub supplied_kinds: BTreeMap<String, u64>,
    pub end: u32,
    pub timed_out: bool,
    /// The `jam` run: the gt the judge emptied the jammed chest (0: never), the gt it supplied the
    /// tail (0: never), and what had reached every sink then.
    pub emptied_at: u32,
    pub tail_at: u32,
    pub before_tail: BTreeMap<String, BTreeMap<String, u64>>,
    /// The `jam` run: `None` while undecided; `Some(true)` when the line kept moving while the
    /// chest was full (badge `overflow-proof`), else why not in `jam_why`.
    pub jam_free: Option<bool>,
    pub jam_why: String,
    /// The `intrusion` run: stray items dropped so far, per kind, and those still lying on the
    /// open hopper tops at the end (never taken in: not lost, not delivered).
    pub dropped: BTreeMap<String, u64>,
    pub lying: BTreeMap<String, u64>,
    pub restless: String,
    /// Design hoppers unlocked at rest (lag).
    pub awake: u32,
    /// The design's containers, per kind: before the judge fills anything, and at the end.
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    /// Entities alive at the end that the design did not declare (and every item entity).
    pub loose: Vec<String>,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

impl RunLog {
    fn delivered(&self) -> u64 {
        self.reached.values().flat_map(|m| m.values()).sum()
    }

    fn delivered_of(&self, kind: &str) -> u64 {
        self.reached.values().map(|m| m.get(kind).copied().unwrap_or(0)).sum()
    }
}

/// The kinds and counts the design's own containers hold (judge containers left out).
pub(crate) fn held(w: &mut Worker, sb: u32, from: Pos, to: Pos, judge: &[Pos]) -> Result<BTreeMap<String, u64>> {
    let mut m = BTreeMap::new();
    for be in w.block_entities(sb, from, to)? {
        if judge.contains(&be.pos) {
            continue;
        }
        for s in stacks_of_be(&be.nbt) {
            *m.entry(s.id.clone()).or_insert(0) += s.count as u64;
        }
    }
    Ok(m)
}

/// The kinds the design's containers hold as written in its file (the run's stream avoids them).
pub fn design_kinds(c: &Circuit) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for nbt in c.clock.design.block_entities.values() {
        for s in stacks_of_be(nbt) {
            if !v.contains(&s.id) {
                v.push(s.id.clone());
            }
        }
    }
    v
}

/// Run a scenario on a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, sc: &Scenario, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let (from, to) = crate::run::watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, sc, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

/// The sink chests: `pass` and every output, with the copy each output belongs to.
fn sinks(c: &Circuit) -> Vec<(String, Pos)> {
    let mut v: Vec<(String, Pos)> = c.stores.iter().filter(|s| s.role == "pass").map(|s| ("pass".to_string(), s.pos)).collect();
    for o in &c.outputs {
        v.push((if o.name == "out" { "output".to_string() } else { format!("output.{}", o.name) }, o.pos));
    }
    v
}

/// The copy an output port belongs to (`output` 0, `output.t<k>` k).
pub fn copy_of(port: &str) -> Option<usize> {
    if port == "pass" {
        return None;
    }
    Some(shulker::group_of(port) as usize)
}

/// Bulk slices: an output port `output.b` (copies `output.b.t<k>`) is the other half of a DOUBLE
/// chest with `output` (`output.t<k>`): the two cells side by side, the `output` half `type=right`
/// and the `.b` half `type=left`, facing so (`ChestBlock.getConnectedDirection`: a right half's
/// partner is counter-clockwise of its facing). The judge's chest state of every such cell.
fn double_chests(sinks: &[(String, Pos)]) -> std::result::Result<BTreeMap<Pos, String>, String> {
    let mut m = BTreeMap::new();
    for (n, b) in sinks {
        let segs: Vec<&str> = n.split('.').collect();
        if !segs.contains(&"b") {
            continue;
        }
        let partner: String = segs.iter().filter(|s| **s != "b").copied().collect::<Vec<_>>().join(".");
        let Some((_, a)) = sinks.iter().find(|(m2, _)| *m2 == partner) else { return Err(format!("{n}: no `{partner}` half for its double chest")) };
        // d: from the right half (output) to the left half (.b); facing = d turned clockwise
        let facing = match (b[0] - a[0], b[1] - a[1], b[2] - a[2]) {
            (1, 0, 0) => "south",
            (0, 0, 1) => "west",
            (-1, 0, 0) => "north",
            (0, 0, -1) => "east",
            d => return Err(format!("{n} at {b:?} is not beside {partner} at {a:?} ({d:?}): a double chest's halves stand side by side")),
        };
        m.insert(*a, format!("minecraft:chest[facing={facing},type=right,waterlogged=false]"));
        m.insert(*b, format!("minecraft:chest[facing={facing},type=left,waterlogged=false]"));
    }
    Ok(m)
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    run_in_rec(w, sb, c, sc, from, to, false).map(|r| r.map(|(log, _)| log))
}

/// One frame of a recorded run: every non-air cell of the watched box with its full state (block
/// state + the compared block-entity part: container contents, a comparator's output), sorted.
pub type Frame = Vec<(Pos, String)>;

/// [`run_in`], with every gt's frame from the placement on when `record` (the exact comparison
/// with redsim: [`compare`]).
fn run_in_rec(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos, record: bool) -> Result<std::result::Result<(RunLog, Vec<Frame>), String>> {
    let p = match ports(c, sc.feed) {
        Ok(p) => p,
        Err(why) => return Ok(Err(why)),
    };
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    w.set_blocks(sb, &p.blocks, PlaceMode::Raw)?;
    let mut b = GameBench { w, sb, from, to, base: None, frames: record.then(Vec::new) };
    b.record()?;
    b.step(c.clock.free_from() + SETTLE)?;
    b.base = Some(b.w.get_region(sb, from, to)?);
    let mut log = RunLog { dims: shulker::design_dims(c), initial: held(b.w, sb, from, to, &p.judge)?, ..Default::default() };
    drive(&mut b, sc, &p, &mut log)?;
    let frames = b.frames.take().unwrap_or_default();
    finish(w, sb, c, from, to, &p, &mut log)?;
    Ok(Ok((log, frames)))
}

/// Where the judge's things go in a run (also the auto-crafters' runs, [`crate::crafter`]).
#[derive(Clone)]
pub(crate) struct Ports {
    /// The chests the stream is written into (an input chest on `items`; a hopper line's chest
    /// above it).
    pub(crate) sources: Vec<Pos>,
    /// The judge's input hopper (hopper line, water stream): on `items`.
    pub(crate) hopper: Option<Pos>,
    /// The cell the stream's item entities are dropped in (water streams).
    pub(crate) drop: Option<Pos>,
    pub(crate) sinks: Vec<(String, Pos)>,
    /// Every judge container (left out of the design's own).
    pub(crate) judge: Vec<Pos>,
    /// The judge's blocks (chests, the input hopper).
    pub(crate) blocks: Vec<(Pos, String)>,
    /// The `intrusion` run: the open hopper tops of the row where the strays fall
    /// ([`crate::intrusion::intakes`], the judge's own blocks left out).
    pub(crate) strays: Vec<Pos>,
}

/// The judge's ports for a feed; Err when one lands on a design block.
fn ports(c: &Circuit, feed: Feed) -> std::result::Result<Ports, String> {
    let items: Vec<Pos> = c.stores.iter().filter(|s| s.role == "items").map(|s| s.pos).collect();
    let line: Option<Pos> = c.stores.iter().find(|s| s.role == "feed").map(|s| s.pos);
    let up = |p: Pos| [p[0], p[1] + 1, p[2]];
    let need_line = || line.ok_or_else(|| "the hopper-line and water-stream feeds need the row's `feed` cell (a tiled sorter's)".to_string());
    let (sources, hopper, drop) = match feed {
        Feed::Chest => (items.clone(), None, None),
        Feed::Hopper => {
            let l = need_line()?;
            (vec![up(l)], Some(l), None)
        }
        Feed::Stream => {
            let l = need_line()?;
            (Vec::new(), Some(l), Some(up(l)))
        }
        Feed::Direct => (Vec::new(), None, items.first().copied()),
    };
    let sinks = sinks(c);
    let judge: Vec<Pos> = sources.iter().copied().chain(hopper).chain(sinks.iter().map(|s| s.1)).collect();
    let pairs = double_chests(&sinks)?;
    let mut blocks: Vec<(Pos, String)> = sources.iter().chain(sinks.iter().map(|s| &s.1)).map(|p| (*p, pairs.get(p).cloned().unwrap_or_else(|| CHEST.to_string()))).collect();
    if let Some(h) = hopper {
        // pointing into the hopper under `items` (one step along the line)
        let it = items.first().copied().ok_or_else(|| "no `items` port".to_string())?;
        let facing = match (it[0] - h[0], it[2] - h[2]) {
            (1, 0) => "east",
            (-1, 0) => "west",
            (0, 1) => "south",
            (0, -1) => "north",
            d => return Err(format!("the judge's line hopper at {h:?} is not next to the cell under `items` ({d:?})")),
        };
        blocks.push((h, format!("minecraft:hopper[enabled=true,facing={facing}]")));
    }
    for (q, what) in blocks.iter().map(|(q, s)| (*q, s.as_str())).chain(drop.map(|d| (d, "stream's end"))) {
        if let Some(s) = c.clock.design.blocks.get(&q) {
            return Err(format!("the judge's {} at {q:?} lands on the design's {}", clock::short(what), clock::short(s)));
        }
    }
    // the open hopper tops (the judge's own blocks close the ones they stand on)
    let taken: Vec<Pos> = blocks.iter().map(|b| b.0).chain(drop).collect();
    let strays: Vec<Pos> = crate::intrusion::intakes(c).into_iter().filter(|p| !taken.contains(p)).collect();
    Ok(Ports { sources, hopper, drop, sinks, judge, blocks, strays })
}

/// The box an item lying on a hopper's top occupies: the cell above the hopper and its bowl.
pub(crate) fn top_box(d: Pos) -> ([f64; 3], [f64; 3]) {
    ([d[0] as f64, d[1] as f64 - 1.0 + 11.0 / 16.0, d[2] as f64], [d[0] as f64 + 1.0, d[1] as f64 + 1.0, d[2] as f64 + 1.0])
}

/// What a run needs from the world it runs in: the game's sandbox ([`GameBench`]) or a redsim
/// world ([`SimBench`]) (shared with the smelter study).
pub(crate) trait Bench {
    fn step(&mut self, n: u32) -> Result<()>;
    /// A judge container's stacks.
    fn read(&mut self, p: Pos) -> Result<Vec<Stack>>;
    /// Replace a judge chest's contents (slot order).
    fn write(&mut self, p: Pos, items: &[Stack]) -> Result<()>;
    /// A stack as an item entity at rest at a point (a water stream's end, a stray).
    fn drop_item(&mut self, at: [f64; 3], s: &Stack) -> Result<()>;
    /// The item entities inside a box: (id, count) each.
    fn items_in(&mut self, lo: [f64; 3], hi: [f64; 3]) -> Result<Vec<(String, u64)>>;
    /// After the steps up to `t` (the game: the changed blocks for the table, early on).
    fn note(&mut self, _t: u32, _notes: &mut Vec<String>) -> Result<()> {
        Ok(())
    }
    /// The brewer study: the brew time left of the brewing stands at these cells (0: idle).
    fn brew_times(&mut self, at: &[Pos]) -> Result<Vec<i32>>;
    /// The composter study: the levels of the composters at these cells.
    fn levels(&mut self, at: &[Pos]) -> Result<Vec<i32>>;
}

/// The game's sandbox as a run's world (shared with the smelter study, [`crate::smelter`]).
pub(crate) struct GameBench<'a> {
    pub(crate) w: &'a mut Worker,
    pub(crate) sb: u32,
    pub(crate) from: Pos,
    pub(crate) to: Pos,
    pub(crate) base: Option<mctas::sim::Region>,
    /// Every gt's frame (recorded runs).
    pub(crate) frames: Option<Vec<Frame>>,
}

impl GameBench<'_> {
    /// The current frame, when recording.
    pub(crate) fn record(&mut self) -> Result<()> {
        if let Some(frames) = &mut self.frames {
            let f = self.w.get_region(self.sb, self.from, self.to)?;
            let ex: std::collections::HashMap<Pos, String> = self.w.block_entities(self.sb, self.from, self.to)?.into_iter().filter_map(|b| clock::wave::be_extra_from_nbt(&b.kind, &b.nbt).map(|e| (b.pos, e))).collect();
            let mut v: Frame = f.iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(p, s)| (p, clock::wave::with_extra(s, ex.get(&p).map(|e| e.as_str())))).collect();
            v.sort();
            frames.push(v);
        }
        Ok(())
    }
}

impl Bench for GameBench<'_> {
    fn levels(&mut self, at: &[Pos]) -> Result<Vec<i32>> {
        if at.is_empty() {
            return Ok(Vec::new());
        }
        let r = self.w.get_region(self.sb, self.from, self.to)?;
        Ok(at.iter().map(|p| r.get(*p).map(|s| crate::composter::level_of(s)).unwrap_or(0)).collect())
    }
    fn brew_times(&mut self, at: &[Pos]) -> Result<Vec<i32>> {
        if at.is_empty() {
            return Ok(Vec::new());
        }
        let bes = self.w.block_entities(self.sb, self.from, self.to)?;
        Ok(at.iter().map(|p| bes.iter().find(|b| b.pos == *p).map(|b| int_field(&b.nbt, "BrewTime:")).unwrap_or(0)).collect())
    }
    fn step(&mut self, n: u32) -> Result<()> {
        if self.frames.is_none() {
            return self.w.step(self.sb, n).map(|_| ());
        }
        for _ in 0..n {
            self.w.step(self.sb, 1)?;
            self.record()?;
        }
        Ok(())
    }
    fn read(&mut self, p: Pos) -> Result<Vec<Stack>> {
        shulker::read_chest(self.w, self.sb, p)
    }
    fn write(&mut self, p: Pos, items: &[Stack]) -> Result<()> {
        self.w.merge_block_entity(self.sb, p, &items_snbt(items)).map(|_| ())
    }
    fn drop_item(&mut self, at: [f64; 3], s: &Stack) -> Result<()> {
        let nbt = format!("{{Item:{{id:\"{}\",count:{}}},PickupDelay:0s,Motion:[0.0d,0.0d,0.0d]}}", s.id, s.count);
        self.w.summon(self.sb, &[mctas::design::EntitySpec { kind: "minecraft:item".into(), pos: at, nbt: Some(nbt) }]).map(|_| ())
    }
    fn items_in(&mut self, lo: [f64; 3], hi: [f64; 3]) -> Result<Vec<(String, u64)>> {
        let mut v = Vec::new();
        for e in self.w.entities(self.sb)? {
            if e.kind == "minecraft:item" && (0..3).all(|i| e.pos[i] >= lo[i] && e.pos[i] < hi[i]) {
                let s = shulker::stack_of_entity(&e.state);
                v.push((s.as_ref().map(|s| s.id.clone()).unwrap_or_default(), s.map(|s| s.count as u64).unwrap_or(1)));
            }
        }
        Ok(v)
    }
    fn note(&mut self, t: u32, notes: &mut Vec<String>) -> Result<()> {
        if (t == SEG || t == 10 * SEG)
            && let Some(base) = &self.base
        {
            notes.extend(shulker::snapshot(self.w, self.sb, self.from, self.to, base, &format!("t={t}"))?);
        }
        Ok(())
    }
}

/// A redsim world as a run's world (shared with the smelter study).
pub(crate) struct SimBench<'a> {
    pub(crate) w: &'a mut redsim::World,
    pub(crate) from: Pos,
    pub(crate) to: Pos,
    /// Every gt's frame (recorded runs).
    pub(crate) frames: Option<Vec<Frame>>,
    /// The furnaces whose fuel slot is watched (the smelter's `--coal-log`).
    pub(crate) coal: Option<CoalWatch>,
    /// Item entities whose id contains this are printed every gt (`lift-run --watch`).
    pub(crate) watch: Option<String>,
}

/// The gt at which each watched furnace's fuel slot gained an item (`smelter-run --coal-log`).
#[derive(Default)]
pub(crate) struct CoalWatch {
    pub(crate) at: Vec<Pos>,
    pub(crate) last: Vec<u8>,
    pub(crate) got: Vec<Vec<u32>>,
    pub(crate) t: u32,
    pub(crate) locked: Vec<Pos>,
    /// The hopper two above each furnace (the intake of the row designs): the gt of each item it
    /// took, and the gt each of its windows opened (the first intake unlocked).
    pub(crate) intake: Vec<Vec<u32>>,
    pub(crate) intake_last: Vec<u32>,
    pub(crate) opened: Vec<u32>,
    pub(crate) was_open: bool,
    pub(crate) trail: Vec<String>,
    pub(crate) trail_windows: u32,
}

impl CoalWatch {
    pub(crate) fn new(at: Vec<Pos>) -> Self {
        let n = at.len();
        CoalWatch { last: vec![0; n], got: vec![Vec::new(); n], intake: vec![Vec::new(); n], intake_last: vec![0; n], at, trail_windows: 30, ..Default::default() }
    }

    /// Per furnace, the windows (1-based) in which its intake took an item, and the windows it
    /// missed.
    pub(crate) fn intake_report(&self) -> Vec<String> {
        let win = |t: u32| self.opened.iter().filter(|o| **o <= t).count();
        let mut out = vec![format!("intake log: {} windows, opened at gt {}", self.opened.len(), self.opened.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" "))];
        for (f, got) in self.at.iter().zip(&self.intake) {
            let ws: Vec<usize> = got.iter().map(|t| win(*t)).collect();
            let missed: Vec<String> = (1..=self.opened.len()).filter(|w| !ws.contains(w)).map(|w| w.to_string()).collect();
            out.push(format!("intake log: furnace {f:?} took {} items; missed windows {}", got.len(), missed.join(" ")));
        }
        out
    }
    fn sample(&mut self, w: &redsim::World) {
        self.t += 1;
        // the locked hoppers, each time they change during the first 3000 gt (the clocks'
        // windows reaching the intakes)
        let mut lk: Vec<Pos> = w.containers.map.keys().filter(|q| matches!(w.get(**q), redsim::Block::Hopper { enabled: false, .. })).map(|q| [q.x, q.y, q.z]).collect();
        lk.sort();
        if lk != self.locked && self.t < 3000 {
            eprintln!("coal log: t={} locked {lk:?}", self.t);
        }
        self.locked = lk;
        if self.t % 2000 == 0 {
            let mut held: Vec<(Pos, u32)> = Vec::new();
            for (q, x) in w.containers.map.iter() {
                let n: u32 = x.slots.iter().flatten().filter(|s| redsim::redstone::container::item_name(s.item) == "coal").map(|s| s.count as u32).sum();
                if n > 0 {
                    held.push(([q.x, q.y, q.z], n));
                }
            }
            held.sort();
            eprintln!("coal log: t={} coal at {}", self.t, held.iter().map(|(q, n)| format!("{q:?} {n}")).collect::<Vec<_>>().join(", "));
        }
        for (i, p) in self.at.iter().enumerate() {
            let n = w.containers.map.get(&bp(*p)).and_then(|x| x.slots[1]).map(|s| s.count).unwrap_or(0);
            for _ in self.last[i]..n {
                self.got[i].push(self.t);
            }
            self.last[i] = n;
            let q = bp([p[0], p[1] + 2, p[2]]);
            let m: u32 = w.containers.map.get(&q).map(|x| x.slots.iter().flatten().filter(|s| redsim::redstone::container::item_name(s.item) != "coal").map(|s| s.count as u32).sum()).unwrap_or(0);
            if m > self.intake_last[i] {
                self.intake[i].push(self.t);
            }
            self.intake_last[i] = m;
        }
        if let Some(p) = self.at.first() {
            let open = matches!(w.get(bp([p[0], p[1] + 2, p[2]])), redsim::Block::Hopper { enabled: true, .. });
            if open && !self.was_open {
                self.opened.push(self.t);
                self.trail = vec![String::new(); self.at.len()];
            }
            // during a window and 8 gt either side: per furnace, the items in the hoppers 3, 2 and 1
            // above it (ring, intake, top hopper) each gt; printed for the furnaces that missed it
            let items = |q: Pos| -> u32 { w.containers.map.get(&bp(q)).map(|x| x.slots.iter().flatten().filter(|s| redsim::redstone::container::item_name(s.item) != "coal").map(|s| s.count as u32).sum()).unwrap_or(0) };
            if let Some(&o) = self.opened.last()
                && self.t >= o
                && self.t < o + 24
            {
                for (i, f) in self.at.iter().enumerate() {
                    let s = format!("{}{}{}{} ", if open { "o" } else { "-" }, items([f[0], f[1] + 3, f[2]]), items([f[0], f[1] + 2, f[2]]), items([f[0], f[1] + 1, f[2]]));
                    self.trail[i].push_str(&s);
                }
                if self.t == o + 23 && self.trail_windows > 0 {
                    self.trail_windows -= 1;
                    let n = self.opened.len();
                    for (i, f) in self.at.iter().enumerate() {
                        if !self.intake[i].iter().any(|t| *t >= o && *t < o + 24) {
                            eprintln!("intake log: window {n} (gt {o}): furnace {f:?} missed; ring/intake/top each gt: {}", self.trail[i]);
                        }
                    }
                }
            }
            self.was_open = open;
        }
    }
}

pub(crate) fn bp(p: Pos) -> redsim::BlockPos {
    redsim::BlockPos::new(p[0], p[1], p[2])
}

impl SimBench<'_> {
    /// The current frame, when recording (the same text as [`GameBench::record`]'s).
    pub(crate) fn record(&mut self) {
        if let Some(frames) = &mut self.frames {
            let mut v: Frame = Vec::new();
            for x in self.from[0]..=self.to[0] {
                for y in self.from[1]..=self.to[1] {
                    for z in self.from[2]..=self.to[2] {
                        let q = bp([x, y, z]);
                        let b = self.w.get(q);
                        let s = b.to_string();
                        if !mctas::blockstate::is_air(&s) {
                            v.push(([x, y, z], clock::wave::with_extra(&s, clock::wave::be_extra_redsim(self.w, q, &b).as_deref())));
                        }
                    }
                }
            }
            v.sort();
            frames.push(v);
        }
    }
}

/// An int field of a block entity's SNBT (`BrewTime:` ...), 0 without it.
pub(crate) fn int_field(nbt: &str, key: &str) -> i32 {
    let Some(i) = nbt.find(key) else { return 0 };
    let digits: String = nbt[i + key.len()..].chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    digits.parse().unwrap_or(0)
}

impl Bench for SimBench<'_> {
    fn levels(&mut self, at: &[Pos]) -> Result<Vec<i32>> {
        Ok(at.iter().map(|p| if let redsim::Block::Composter { level } = self.w.get(bp(*p)) { level as i32 } else { 0 }).collect())
    }
    fn brew_times(&mut self, at: &[Pos]) -> Result<Vec<i32>> {
        Ok(at.iter().map(|p| self.w.containers.map.get(&bp(*p)).map(|c| c.brew_time).unwrap_or(0)).collect())
    }
    fn step(&mut self, n: u32) -> Result<()> {
        for _ in 0..n {
            self.w.tick();
            self.record();
            if let Some(c) = &mut self.coal {
                c.sample(self.w);
            }
            if let Some(want) = &self.watch {
                for e in self.w.entities.list.iter().filter(|e| !e.removed) {
                    if let redsim::entity::EntityKind::Item(s) = &e.kind
                        && s.item.contains(want.as_str())
                    {
                        eprintln!("watch: t={} {} x{} id {} at [{:.3}, {:.3}, {:.3}] v [{:.3}, {:.3}, {:.3}]", self.w.time, s.item, s.count, e.id, e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z);
                    }
                }
            }
        }
        if let Some(e) = self.w.errors.first() {
            bail!("redsim: {e}");
        }
        Ok(())
    }
    fn read(&mut self, p: Pos) -> Result<Vec<Stack>> {
        use redsim::redstone::container as rc;
        let c = self.w.containers.map.get(&bp(p)).ok_or_else(|| anyhow!("no container at {p:?}"))?;
        Ok(c.slots.iter().flatten().map(|s| Stack { id: format!("minecraft:{}", rc::item_name(s.item)), count: s.count as u32, contents: Vec::new(), name: (s.payload != 0).then(|| format!("payload {}", s.payload)) }).collect())
    }
    fn write(&mut self, p: Pos, items: &[Stack]) -> Result<()> {
        use redsim::redstone::container as rc;
        let mut v = Vec::new();
        for (i, s) in items.iter().enumerate() {
            let item = rc::item_id(&s.id).ok_or_else(|| anyhow!("redsim has no item {}", s.id))?;
            if s.name.is_some() || !s.contents.is_empty() {
                bail!("the redsim screen feeds plain stacks only ({} has components)", s.id);
            }
            v.push((i, rc::Stack { item, count: s.count as u8, payload: 0 }));
        }
        rc::set_contents(self.w, bp(p), &v);
        Ok(())
    }
    fn drop_item(&mut self, at: [f64; 3], s: &Stack) -> Result<()> {
        use redsim::entity::{EntityKind, Vec3, item::ItemState};
        if redsim::redstone::container::item_id(&s.id).is_none() {
            bail!("redsim has no item {}", s.id);
        }
        let state = ItemState { item: s.id.clone(), count: s.count.min(255) as u8, pickup_delay: 0, age: 0, payload: 0, health: 5 };
        self.w.entities.add(EntityKind::Item(state), Vec3::new(at[0], at[1], at[2]), 0.0);
        Ok(())
    }
    fn items_in(&mut self, lo: [f64; 3], hi: [f64; 3]) -> Result<Vec<(String, u64)>> {
        let mut v = Vec::new();
        for e in &self.w.entities.list {
            if let redsim::entity::EntityKind::Item(s) = &e.kind
                && !e.removed
                && s.count > 0
            {
                let p = [e.pos.x, e.pos.y, e.pos.z];
                if (0..3).all(|i| p[i] >= lo[i] && p[i] < hi[i]) {
                    v.push((s.item.clone(), s.count as u64));
                }
            }
        }
        Ok(v)
    }
}

/// The box a dropped item waits in: the drop cell and the hopper bowl under it.
pub(crate) fn drop_box(d: Pos) -> ([f64; 3], [f64; 3]) {
    ([d[0] as f64, d[1] as f64 - 1.0, d[2] as f64], [d[0] as f64 + 1.0, d[1] as f64 + 1.0, d[2] as f64 + 1.0])
}

/// The `intrusion` run: drop `s` on every open hopper top (the real-use audit's spot: the cell's
/// center, at rest).
pub(crate) fn drop_strays<B: Bench>(b: &mut B, p: &Ports, s: &Stack, log: &mut RunLog) -> Result<()> {
    for q in &p.strays {
        b.drop_item([q[0] as f64 + 0.5, q[1] as f64 + 0.5, q[2] as f64 + 0.5], s)?;
        *log.dropped.entry(s.id.clone()).or_insert(0) += s.count as u64;
    }
    Ok(())
}

/// The strays still lying on the open hopper tops, per kind.
pub(crate) fn strays_lying<B: Bench>(b: &mut B, p: &Ports) -> Result<BTreeMap<String, u64>> {
    let mut m = BTreeMap::new();
    for q in &p.strays {
        let (lo, hi) = top_box(*q);
        for (id, n) in b.items_in(lo, hi)? {
            *m.entry(id).or_insert(0) += n;
        }
    }
    Ok(m)
}

/// Hand the next stacks of `stream` to the judge's supply: the source chests topped up (27 slots
/// each), or (water streams) item entities dropped until [`WAITING`] wait at the stream's end.
pub(crate) fn supply<B: Bench>(b: &mut B, feed: Feed, p: &Ports, stream: &[Stack], cursor: &mut usize, log: &mut RunLog) -> Result<()> {
    let take = |s: &Stack, log: &mut RunLog| {
        log.supplied += s.count as u64;
        *log.supplied_kinds.entry(s.id.clone()).or_insert(0) += s.count as u64;
    };
    match (feed, p.drop) {
        (Feed::Stream | Feed::Direct, Some(d)) => {
            let (lo, hi) = drop_box(d);
            let mut n = b.items_in(lo, hi)?.len();
            while n < WAITING && *cursor < stream.len() {
                let s = &stream[*cursor];
                b.drop_item([d[0] as f64 + 0.5, d[1] as f64 + 0.25, d[2] as f64 + 0.5], s)?;
                take(s, log);
                *cursor += 1;
                n += 1;
            }
        }
        _ => {
            for q in &p.sources {
                if *cursor >= stream.len() {
                    break;
                }
                let mut have = b.read(*q)?;
                let before = have.len();
                while have.len() < 27 && *cursor < stream.len() {
                    take(&stream[*cursor], log);
                    have.push(stream[*cursor].clone());
                    *cursor += 1;
                }
                if have.len() != before {
                    b.write(*q, &have)?;
                }
            }
        }
    }
    Ok(())
}

/// The items still in the judge's supply: its chests, its input hopper, the stream's waiting
/// entities.
pub(crate) fn supply_left<B: Bench>(b: &mut B, p: &Ports) -> Result<u64> {
    let mut n = 0u64;
    for q in p.sources.iter().chain(p.hopper.iter()) {
        n += b.read(*q)?.iter().map(|s| s.count as u64).sum::<u64>();
    }
    if let Some(d) = p.drop {
        let (lo, hi) = drop_box(d);
        n += b.items_in(lo, hi)?.iter().map(|x| x.1).sum::<u64>();
    }
    Ok(n)
}

/// The run's loop, on any bench: supply the stream, count and empty the sinks every [`SEG`] gt.
/// The `jam` run: the jammed chest first; once the line is quiet the judge decides the badge,
/// empties that chest, waits for the line to settle and supplies the tail. Stops when every
/// supplied item is accounted for, or at the cap.
fn drive<B: Bench>(b: &mut B, sc: &Scenario, p: &Ports, log: &mut RunLog) -> Result<()> {
    // the jammed copy's chests (a bulk slice's double chest: both halves)
    let jammed: Vec<(String, Pos)> = sc.jam.map(|k| p.sinks.iter().filter(|(n, _)| copy_of(n) == Some(k)).cloned().collect()).unwrap_or_default();
    for (_, q) in &jammed {
        let filler: Vec<Stack> = (0..27).map(|_| Stack::new(JAM_FILLER, 1)).collect();
        b.write(*q, &filler)?;
    }
    // 0: a plain run; the jam run: 1 the chest full, 2 emptied (the rest of the stream), 3 the tail
    let mut phase = if jammed.is_empty() { 0 } else { 1 };
    let (mut cursor, mut tail_cursor, mut quiet, mut t) = (0usize, 0usize, 0u32, 0u32);
    let mut strays = 0usize;
    supply(b, sc.feed, p, &sc.stream, &mut cursor, log)?;
    let mut last_left = u64::MAX;
    loop {
        b.step(SEG)?;
        t += SEG;
        b.note(t, &mut log.notes)?;
        // the intrusion run: the strays due by now, on every open hopper top
        while strays < sc.strays.len() && t >= sc.strays[strays].0 {
            drop_strays(b, p, &sc.strays[strays].1, log)?;
            log.notes.push(format!("t={t}: the judge drops {} x{} on each of {} open hopper top(s) {:?}", clock::short(&sc.strays[strays].1.id), sc.strays[strays].1.count, p.strays.len(), p.strays));
            strays += 1;
        }
        let before = log.delivered();
        for (name, q) in &p.sinks {
            if phase == 1 && jammed.iter().any(|j| j.1 == *q) {
                continue;
            }
            let have = b.read(*q)?;
            if have.is_empty() {
                continue;
            }
            let m = log.reached.entry(name.clone()).or_default();
            for s in &have {
                *m.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(*q, &[])?;
        }
        let now = log.delivered();
        if now != before {
            log.flow.push((t, now));
        }
        if phase == 3 {
            supply(b, sc.feed, p, &sc.tail, &mut tail_cursor, log)?;
        } else {
            supply(b, sc.feed, p, &sc.stream, &mut cursor, log)?;
        }
        // quiet: nothing delivered and nothing taken from the judge's supply
        let left = if phase == 0 { 0 } else { supply_left(b, p)? };
        quiet = if now == before && left == last_left { quiet + SEG } else { 0 };
        last_left = left;
        if phase == 1
            && quiet >= QUIET
            && let Some(k) = sc.jam
            && !jammed.is_empty()
        {
            // badge overflow-proof: every other item supplied so far has reached its place
            let kind = &sc.sorts[k];
            let mut held: Vec<String> = Vec::new();
            for (id, n) in &log.supplied_kinds {
                let got = log.delivered_of(id);
                if id != kind && got < *n {
                    held.push(format!("{} of {n} {}", n - got, clock::short(id)));
                }
            }
            if cursor < sc.stream.len() {
                held.push(format!("{} stacks never taken in", sc.stream.len() - cursor));
            }
            log.jam_free = Some(held.is_empty());
            if !held.is_empty() {
                log.jam_why = format!("the line stops while copy {k}'s chest is full: {} held back", held.join(", "));
            }
            for (name, q) in &jammed {
                let have = b.read(*q)?;
                let got: u32 = have.iter().filter(|s| s.id != JAM_FILLER).map(|s| s.count).sum();
                log.notes.push(format!("t={t}: the judge empties {name}, full since the start ({got} items besides its {})", clock::short(JAM_FILLER)));
                let m = log.reached.entry(name.clone()).or_default();
                for s in have.iter().filter(|s| s.id != JAM_FILLER) {
                    *m.entry(s.id.clone()).or_insert(0) += s.count as u64;
                }
                b.write(*q, &[])?;
            }
            log.emptied_at = t;
            phase = 2;
            quiet = 0;
        } else if phase == 2 && quiet >= QUIET && cursor >= sc.stream.len() && left == 0 {
            log.before_tail = log.reached.clone();
            log.tail_at = t;
            log.notes.push(format!("t={t}: the line has settled; the judge supplies the tail"));
            phase = 3;
            quiet = 0;
            supply(b, sc.feed, p, &sc.tail, &mut tail_cursor, log)?;
        }
        let all_supplied = cursor >= sc.stream.len() && (jammed.is_empty() || (phase == 3 && tail_cursor >= sc.tail.len())) && strays >= sc.strays.len();
        // the strays: delivered, or lying on a hopper top that cannot take them, once the line
        // has been quiet for a while
        let dropped: u64 = log.dropped.values().sum();
        let lying: u64 = if dropped > 0 && log.delivered() < sc.total() + dropped { strays_lying(b, p)?.values().sum() } else { 0 };
        let done = all_supplied && (log.delivered() + lying >= sc.total() + dropped) && (lying == 0 || quiet >= QUIET);
        if done || t >= sc.cap {
            log.timed_out = !done;
            if !done && !jammed.is_empty() {
                log.notes.push(format!("stopped in phase {phase} ({})", ["", "the chest full", "after the judge emptied it", "the tail"][phase]));
            }
            break;
        }
    }
    log.lying = strays_lying(b, p)?;
    log.end = t;
    Ok(())
}

/// The kinds and counts a redsim world's design containers hold (judge containers left out).
pub(crate) fn sim_held(w: &redsim::World, judge: &[Pos]) -> BTreeMap<String, u64> {
    use redsim::redstone::container as rc;
    let mut m = BTreeMap::new();
    for (p, c) in &w.containers.map {
        if judge.contains(&[p.x, p.y, p.z]) {
            continue;
        }
        for s in c.slots.iter().flatten() {
            *m.entry(format!("minecraft:{}", rc::item_name(s.item))).or_insert(0) += s.count as u64;
        }
    }
    m
}

/// The same run on redsim (milliseconds): `mctas circuit sim` and the searches' screen. Err: the
/// design cannot stand or redsim does not model it.
pub fn sim_run(c: &Circuit, sc: &Scenario, margin: i32) -> std::result::Result<RunLog, String> {
    sim_run_rec(c, sc, margin, false).map(|(log, _)| log)
}

/// [`sim_run`], with every gt's frame from the placement on when `record` ([`compare`]).
fn sim_run_rec(c: &Circuit, sc: &Scenario, margin: i32, record: bool) -> std::result::Result<(RunLog, Vec<Frame>), String> {
    let (from, to) = crate::run::watch_box(c, margin);
    if let Some((p, st)) = clock::sim::unsupported(&c.clock, from, to)?.first() {
        return Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(st)));
    }
    let p = ports(c, sc.feed)?;
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    for (q, s) in &p.blocks {
        w.set_block(bp(*q), redsim::Block::parse(s)?, redsim::world::flags::ALL);
    }
    let mut b = SimBench { w: &mut w, from, to, frames: record.then(Vec::new), coal: None, watch: None };
    b.record();
    b.step(c.clock.free_from() + SETTLE).map_err(|e| format!("{e:#}"))?;
    let mut log = RunLog { dims: shulker::design_dims(c), initial: sim_held(b.w, &p.judge), ..Default::default() };
    drive(&mut b, sc, &p, &mut log).map_err(|e| format!("{e:#}"))?;
    let frames = b.frames.take().unwrap_or_default();
    // rest: the whole state repeats over a window of REST gt, nothing scheduled
    let mut key = w.state_key();
    let mut still = false;
    for _ in 0..5 {
        for _ in 0..REST {
            w.tick();
        }
        let k = w.state_key();
        if k == key && w.idle() {
            still = true;
            break;
        }
        key = k;
    }
    if !still {
        log.restless = format!("the state still changes {} gt after the last item", 5 * REST);
    }
    log.awake = c.clock.design.blocks.keys().filter(|p| matches!(w.get(bp(**p)), redsim::Block::Hopper { enabled: true, .. })).count() as u32;
    log.final_held = sim_held(&w, &p.judge);
    log.loose = w.entities.list.iter().filter(|e| !e.removed && !on_top(&p.strays, [e.pos.x, e.pos.y, e.pos.z])).map(|e| format!("{:?}", e.kind)).collect();
    if let Some((p, b)) = w.drops.first() {
        log.loose.push(format!("{b} at {p} broke with a drop"));
    }
    Ok((log, frames))
}

/// The exact gate of a sorter (the coordinator 2026-10-21, every published pick checked against
/// the game): one of the judge's runs (its chests, the settle, the stream supplied and the sinks
/// emptied every SEG gt) in the game and on redsim, every cell of the watched box compared every
/// gt from the placement to the last item (full states: block state, container contents,
/// comparator outputs). `Ok(Ok(frames))` when identical, `Ok(Err(first divergence))` otherwise.
pub fn compare(pool: &Pool, c: &Circuit, margin: i32, run: Run) -> Result<std::result::Result<usize, String>> {
    compare_with(Some(pool), c, margin, run, std::path::Path::new(""), &crate::recorded::Mode::Live)
}

/// [`compare`] live, recording the game's frames, or replaying a recording (no game, no pool):
/// `mctas test`'s recorded sorter runs ([`crate::recorded`]).
pub fn compare_with(pool: Option<&Pool>, c: &Circuit, margin: i32, run: Run, design: &std::path::Path, mode: &crate::recorded::Mode) -> Result<std::result::Result<usize, String>> {
    let (sc, rc) = scenario_for(c, run)?;
    let sim = sim_run_rec(&rc, &sc, margin, true).map(|(_, f)| f);
    let game = || -> Result<std::result::Result<Vec<Frame>, String>> {
        let pool = pool.ok_or_else(|| anyhow!("the game run needs a pool"))?;
        let r = pool.map(vec![0u8], |w, _, _| {
            let (from, to) = crate::run::watch_box(&rc, margin);
            let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
            let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
            let r = run_in_rec(w, sb, &rc, &sc, from, to, true);
            let _ = w.drop_sandbox(sb);
            r
        });
        Ok(r.into_iter().next().ok_or_else(|| anyhow!("no run"))??.map(|(_, f)| f))
    };
    crate::recorded::gate("sorter-run", design, &run.name(), margin, sim, game, mode)
}

/// Whether a point lies on one of the open hopper tops (a stray never taken in).
pub(crate) fn on_top(strays: &[Pos], at: [f64; 3]) -> bool {
    strays.iter().any(|q| {
        let (lo, hi) = top_box(*q);
        (0..3).all(|i| at[i] >= lo[i] && at[i] < hi[i])
    })
}

/// After the run: rest (up to 5 windows of [`REST`] gt in which nothing may change), the design's
/// containers, the entities left (strays still lying on an open hopper top are counted apart).
pub(crate) fn finish(w: &mut Worker, sb: u32, c: &Circuit, from: Pos, to: Pos, p: &Ports, log: &mut RunLog) -> Result<()> {
    let judge = &p.judge;
    let mut a = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
    let mut b = a.clone();
    let mut status = w.status(sb)?;
    for _ in 0..5 {
        w.step(sb, REST)?;
        b = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
        status = w.status(sb)?;
        if a == b && status.block_ticks + status.block_events + status.moving_pistons == 0 {
            break;
        }
        a = b.clone();
    }
    if a.0 != b.0 {
        let p = b.0.iter().find(|(p, s)| a.0.get(*p) != Some(s)).map(|(p, s)| format!("{} at {p:?}", clock::short(s))).unwrap_or_default();
        log.restless = format!("blocks still change {} gt after the last item ({p})", 5 * REST);
    } else if a.1 != b.1 {
        let p = b.1.iter().find(|x| !a.1.contains(x)).map(|x| format!("{} at {:?}", clock::short(&x.kind), x.pos)).unwrap_or_default();
        log.restless = format!("container contents still change {} gt after the last item ({p})", 5 * REST);
    } else if status.block_ticks + status.block_events + status.moving_pistons > 0 {
        log.restless = format!("{} scheduled ticks, {} block events, {} moving pistons pending {} gt after the last item", status.block_ticks, status.block_events, status.moving_pistons, 5 * REST);
    }
    log.awake = b.0.iter().filter(|(p, s)| c.clock.design.blocks.contains_key(p) && s.starts_with("minecraft:hopper[") && s.contains("enabled=true")).count() as u32;
    log.final_held = held(w, sb, from, to, judge)?;
    for e in w.entities(sb)? {
        // the stream's item entities are summoned (tagged as the design's): an item left over is
        // loose all the same; a stray lying on an open hopper top is not
        if (e.design && e.kind != "minecraft:item") || on_top(&p.strays, e.pos) {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            Some(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos)),
            None => log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos)),
        }
    }
    Ok(())
}

/// Judge a run: the outcome, its metrics and a table (per sink, what reached it).
pub fn judge(sc: &Scenario, log: &RunLog) -> Outcome {
    let mut table: Vec<String> = Vec::new();
    for (port, m) in &log.reached {
        let list: Vec<String> = m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect();
        table.push(format!("{port}: {}", list.join(", ")));
    }
    table.push(format!("supplied {} items, delivered {}, end t={}", log.supplied, log.delivered(), log.end));
    if !log.dropped.is_empty() {
        let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
        table.push(format!("strays dropped: {}; still lying on a hopper top: {}", l(&log.dropped), if log.lying.is_empty() { "none".to_string() } else { l(&log.lying) }));
    }
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    // every output: its copy's kind alone; `pass`: everything else (a jammed copy's kind too; a
    // stray of a copy's kind dropped on the line may pass by it, as from a stream ending there)
    for (port, m) in &log.reached {
        match copy_of(port) {
            Some(k) => {
                let want = sc.sorts.get(k).cloned().unwrap_or_default();
                if let Some((wrong, n)) = m.iter().find(|(id, _)| **id != want) {
                    return fail(format!("{} x{n} reached copy {k}'s chest (it sorts {})", clock::short(wrong), clock::short(&want)), table);
                }
            }
            None => {
                for (k, kind) in sc.sorts.iter().enumerate() {
                    if let Some(n) = m.get(kind)
                        && sc.jam != Some(k)
                        && *n > log.dropped.get(kind).copied().unwrap_or(0)
                    {
                        return fail(format!("{} x{n} went past copy {k} to the end of the line (copy {k} sorts it)", clock::short(kind)), table);
                    }
                }
            }
        }
    }
    // the jam run: once its chest was emptied the slice sorts again (the tail's items of its kind
    // reach its chest)
    if let Some(k) = sc.jam
        && log.tail_at > 0
    {
        let kind = &sc.sorts[k];
        let at = |m: &BTreeMap<String, BTreeMap<String, u64>>| m.get("pass").and_then(|m| m.get(kind)).copied().unwrap_or(0);
        let n = at(&log.reached) - at(&log.before_tail);
        if n > 0 {
            return fail(format!("{} x{n} of the tail went on to the end of the line after copy {k}'s chest was emptied (the slice does not sort again)", clock::short(kind)), table);
        }
    }
    if log.timed_out {
        return fail(format!("{} of {} items delivered after {} gt (the cap)", log.delivered(), sc.total() + log.dropped.values().sum::<u64>(), log.end), table);
    }
    for kind in sc.kinds() {
        let got = log.delivered_of(&kind) + log.lying.get(&kind).copied().unwrap_or(0);
        let want = sc.supplied(&kind) + log.dropped.get(&kind).copied().unwrap_or(0);
        if got != want {
            return fail(format!("{got} of {want} {} accounted for (lost or duplicated)", clock::short(&kind)), table);
        }
    }
    if log.final_held != log.initial {
        let diff: Vec<String> = log.initial.keys().chain(log.final_held.keys()).collect::<std::collections::BTreeSet<_>>().into_iter().filter(|k| log.initial.get(*k) != log.final_held.get(*k)).map(|k| format!("{} {} -> {}", clock::short(k), log.initial.get(k).copied().unwrap_or(0), log.final_held.get(k).copied().unwrap_or(0))).collect();
        return fail(format!("the design's containers changed: {} (a recipe must stay as it was)", diff.join(", ")), table);
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...)", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    if !log.restless.is_empty() {
        return fail(log.restless.clone(), table);
    }
    let total = sc.total();
    let at = |q: u64| log.flow.iter().find(|(_, m)| *m >= q).copied();
    let rate = match (at(total.div_ceil(10)), at(total * 9 / 10)) {
        (Some((ta, qa)), Some((tb, qb))) if tb > ta => (qb - qa) as f64 * 20.0 / (tb - ta) as f64,
        _ => 0.0,
    };
    let first = log.flow.first().map(|x| x.0).unwrap_or(0);
    let mut metrics = vec![("items/s".to_string(), format!("{rate:.2}")), ("first item".to_string(), format!("by {first} gt")), ("hoppers awake".to_string(), log.awake.to_string())];
    if sc.jam.is_some() {
        metrics.push(("overflow-proof".to_string(), if log.jam_free == Some(true) { "yes".to_string() } else { format!("no ({})", log.jam_why) }));
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The tiled design of a sorter: `copies` copies at the `tile:` step chained along the line;
/// copy k's recipe holds `sorts[k]` instead of the design's kind. Ports: copy 0's `items`, the
/// last copy's `pass`, every copy's output (`output`, `output.t<k>`).
pub fn tiled(c: &Circuit, sorts: &[String]) -> Result<Circuit> {
    let t = c.tile.ok_or_else(|| anyhow!("{}: no `tile:` header", c.name()))?;
    let base = c.clock.design.clone();
    let n = sorts.len() as i32;
    let k0 = format!("\"{}\"", sorts[0]);
    let mut d = base.clone();
    d.meta.remove("tile");
    d.meta.insert("tiled".into(), n.to_string());
    d.ports.retain(|name, _| name == "items" || name == "output" || name.starts_with("output."));
    for k in 1..n {
        let off = [t.step[0] * k, t.step[1] * k, t.step[2] * k];
        let sh = |p: &Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        for (p, s) in &base.blocks {
            let q = sh(p);
            if let Some(o) = d.blocks.get(&q) {
                bail!("tile: copy {k} puts {} on {} at {q:?} (the copies overlap)", clock::short(s), clock::short(o));
            }
            d.blocks.insert(q, s.clone());
        }
        let kk = format!("\"{}\"", sorts[k as usize]);
        for (p, s) in &base.block_entities {
            d.block_entities.insert(sh(p), s.replace(&k0, &kk));
        }
        for e in &base.entities {
            let mut e2 = e.clone();
            for i in 0..3 {
                e2.pos[i] += off[i] as f64;
            }
            d.entities.push(e2);
        }
        if let Some(p) = base.ports.get("output") {
            d.ports.insert(format!("output.t{k}"), sh(p));
        }
        // a bulk slice's other chest halves (`output.b`)
        for (name, p) in base.ports.iter().filter(|(n, _)| n.starts_with("output.")) {
            d.ports.insert(format!("{name}.t{k}"), sh(p));
        }
        if k == n - 1
            && let Some(p) = base.ports.get("pass")
        {
            d.ports.insert("pass".into(), sh(p));
        }
    }
    for (name, p) in &d.ports {
        // (the input chest may stand on copy 0's cover: a full block on its first line hopper,
        // there to keep strays off the copies after it, as an auto-composter's)
        if let Some(s) = d.blocks.get(p)
            && !(name == "items" && crate::smelter::is_cover(s))
        {
            bail!("tile: port {name} at {p:?} lands on a copy's block");
        }
    }
    // the hopper-line and water-stream feeds: the judge's line hopper where the slice before copy 0
    // would have its last line hopper: the design's hopper that pushes into copy 1's first hopper
    // (the one under `items`), one step back; it must stand next to copy 0's first hopper
    if let Some(it) = base.ports.get("items") {
        if t.step[1] != 0 {
            bail!("tile: a sorter's slices stand side by side (a horizontal step), not {:?}", t.step);
        }
        let first = [it[0], it[1] - 1, it[2]];
        let next = [first[0] + t.step[0], first[1], first[2] + t.step[2]];
        let dir = |s: &str| -> Option<Pos> {
            let f = s.split("facing=").nth(1)?.split([',', ']']).next()?;
            Some(match f {
                "east" => [1, 0, 0],
                "west" => [-1, 0, 0],
                "south" => [0, 0, 1],
                "north" => [0, 0, -1],
                _ => return None,
            })
        };
        let last = base.blocks.iter().find(|(p, s)| s.starts_with("minecraft:hopper[") && dir(s).is_some_and(|v| [p[0] + v[0], p[1] + v[1], p[2] + v[2]] == next)).map(|(p, _)| *p);
        let f = match last {
            Some(p) => [p[0] - t.step[0], p[1], p[2] - t.step[2]],
            None => [first[0] - t.step[0].signum(), first[1], first[2] - t.step[2].signum()],
        };
        if (f[0] - first[0]).abs() + (f[2] - first[2]).abs() != 1 || f[1] != first[1] {
            bail!("tile: the line does not run from slice to slice along the step (the hopper feeding copy 1's first hopper, one step back, is {f:?}, not next to copy 0's first hopper {first:?})");
        }
        if let Some(s) = d.blocks.get(&f) {
            bail!("tile: the judge's line hopper before copy 0 at {f:?} lands on the design's {}", clock::short(s));
        }
        d.ports.insert("feed".into(), f);
    }
    Circuit::from_design(d).context("tiled sorter")
}

/// The width of a slice: the `tile:` step's length (1: slices 1 block wide, side by side).
pub fn slice_width(c: &Circuit) -> Option<i32> {
    c.tile.map(|t| t.step.iter().map(|v| v.abs()).sum())
}

/// A slice's box (its blocks; the judge's chests left out): the archive's size.
pub fn dims(c: &Circuit) -> [i32; 3] {
    shulker::design_dims(c)
}

/// The judge for the sorter kind (`mctas circuit verify`): the base run at N positions x 4
/// rotations; then the row's required runs (`tiled`, `stream`, `jam`) and the badge run
/// (`direct`), each at 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let Kind::Sorter(item) = &kind else { bail!("{}: not a sorter", kind.name()) };
    let avoid = design_kinds(c);
    let sc = Scenario::base(item, &avoid)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, &sc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&sc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(c.inputs.len());
    schedule.ticks = sc.cap;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind.clone()), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    let Some(width) = slice_width(c) else {
        rep.pass = false;
        rep.verdict = NO_TILE.to_string();
        return Ok(rep);
    };
    let mut won: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for run in Run::required(c) {
        match run_rotations(pool, c, run, o) {
            Ok(outs) => {
                lines.push(format!("{}: passes (4 rotations)", run.title()));
                if run == Run::Jam {
                    match outs.iter().find_map(|k| k.metrics.iter().find(|(n, v)| n == "overflow-proof" && v != "yes").map(|(_, v)| v.clone())) {
                        None => {
                            won.push("overflow-proof".to_string());
                            lines.push("badge overflow-proof: yes".to_string());
                        }
                        Some(v) if storage(c) => {
                            rep.pass = false;
                            rep.verdict = format!("{}: {}", run.title(), overflow_required(&v));
                            return Ok(rep);
                        }
                        Some(v) => lines.push(format!("badge overflow-proof: {v}")),
                    }
                }
            }
            Err(why) => {
                rep.pass = false;
                rep.verdict = format!("{}: {why}", run.title());
                return Ok(rep);
            }
        }
    }
    match run_rotations(pool, c, Run::Direct, o) {
        Ok(_) => {
            won.push("stream-direct".to_string());
            lines.push("badge stream-direct: yes".to_string());
        }
        Err(why) => lines.push(format!("badge stream-direct: no ({why})")),
    }
    let bill = shulker::bill(c);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        k.outcome.metrics.push(("slice width".to_string(), width.to_string()));
        if storage(c) {
            k.outcome.metrics.push(("storage".to_string(), storage_metric(c)));
        }
        k.outcome.metrics.push(("badges".to_string(), if won.is_empty() { "none".to_string() } else { won.join("+") }));
        k.outcome.metrics.push(("bill".to_string(), bill));
        k.outcome.table.extend(lines);
    }
    Ok(rep)
}

/// One row run at 4 rotations (the first position): the outcomes, or Err(why) at the first
/// failure.
fn run_rotations(pool: &Pool, c: &Circuit, run: Run, o: &Options) -> std::result::Result<Vec<Outcome>, String> {
    let (sc, tc) = scenario_for(c, run).map_err(|e| format!("{e:#}"))?;
    let jobs: Vec<u8> = vec![0, 1, 2, 3];
    let results = pool.map(jobs.clone(), |w, _, t| {
        let rc = tc.rotated(w, t)?;
        game_run(w, &rc, &sc, o.margin)
    });
    let mut outs = Vec::new();
    for (t, r) in jobs.into_iter().zip(results) {
        match r {
            Ok(Ok(log)) => {
                let out = judge(&sc, &log);
                if !out.pass {
                    return Err(format!("rotation {t}: {}", out.why));
                }
                outs.push(out);
            }
            Ok(Err(why)) => return Err(format!("rotation {t}: {why}")),
            Err(e) => return Err(format!("rotation {t}: {e:#}")),
        }
    }
    Ok(outs)
}

/// A design aid (`mctas circuit sorter-run`): one run at rotation 0 in the game, its table.
pub fn run_one(pool: &Pool, c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, run)?;
    let r = pool.map(vec![0u8], |w, _, _| game_run(w, &rc, &sc, margin));
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        Ok(log) => Ok(judge(&sc, &log)),
        Err(why) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// [`run_one`] on redsim (`mctas circuit sorter-run --sim`).
pub fn sim_one(c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, run)?;
    Ok(match sim_run(&rc, &sc, margin) {
        Ok(log) => judge(&sc, &log),
        Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
    })
}

/// The scenario and the circuit (the row for every run but `base`) of one run.
pub fn scenario_for(c: &Circuit, run: Run) -> Result<(Scenario, Circuit)> {
    let Some(Kind::Sorter(item)) = &c.kind else { bail!("{}: not a sorter", c.name()) };
    let avoid = design_kinds(c);
    let sc = Scenario::for_run(run, item, &avoid)?;
    if run == Run::Base {
        return Ok((sc, c.clone()));
    }
    if c.tile.is_none() {
        bail!("{NO_TILE}");
    }
    let tc = tiled(c, &sc.sorts)?;
    Ok((sc, tc))
}

/// The whole judge on redsim, at rotation 0 (the searches' screen and `sorter-run --sim --run
/// all`): the base run's outcome, failing with the first required run that fails; the metrics add
/// the slice width and the badges (`overflow-proof`, `stream-direct`).
pub fn sim_verdict(c: &Circuit) -> Outcome {
    sim_verdict_with(c, true)
}

/// Items that reached the right place in a run (a search's gradient): every output's own kind,
/// every other kind at `pass` (a jammed copy's kind too).
fn correct(sc: &Scenario, log: &RunLog) -> u64 {
    let mut n = 0;
    for (port, m) in &log.reached {
        for (id, k) in m {
            let right = match copy_of(port) {
                Some(c) => sc.sorts.get(c) == Some(id),
                None => sc.sorts.iter().position(|s| s == id).is_none_or(|c| sc.jam == Some(c)),
            };
            if right {
                n += k;
            }
        }
    }
    n
}

/// The searches' score of a sorter on redsim (`circuit compact`): the quick screen, then every
/// required run in turn (`base`, `tiled`, `stream`, `jam`, `intrusion`); the score counts the runs
/// passed (x 1000) plus the items the first failing run put in the right place (a gradient for
/// broken designs). The outcome is [`sim_verdict_with`]'s (no badge-only run).
pub fn sim_score(c: &Circuit) -> (i64, Outcome) {
    let Some(Kind::Sorter(item)) = &c.kind else { return (0, Outcome::fail("not a sorter".to_string(), Vec::new())) };
    let avoid = design_kinds(c);
    let quick = match Scenario::quick(item, &avoid) {
        Ok(q) => q,
        Err(e) => return (0, Outcome::fail(format!("{e:#}"), Vec::new())),
    };
    let one = |sc: &Scenario, rc: &Circuit| match sim_run(rc, sc, 3) {
        Ok(log) => (judge(sc, &log), correct(sc, &log).min(999) as i64),
        Err(why) => (Outcome::fail(why, Vec::new()), 0),
    };
    let (out, k) = one(&quick, c);
    if !out.pass {
        return (k, out);
    }
    let mut passed = 1i64;
    let mut base: Option<Outcome> = None;
    let mut won: Vec<&str> = Vec::new();
    let mut runs = vec![Run::Base];
    runs.extend(Run::required(c));
    for r in runs {
        let (sc, rc) = match scenario_for(c, r) {
            Ok(x) => x,
            Err(e) => return (passed * 1000, Outcome::fail(format!("{e:#}"), Vec::new())),
        };
        let (out, k) = one(&sc, &rc);
        if !out.pass {
            let why = if r == Run::Base { out.why } else { format!("{}: {}", r.title(), out.why) };
            return (passed * 1000 + k, Outcome::fail(why, out.table));
        }
        passed += 1;
        if r == Run::Jam {
            match out.metrics.iter().find(|(n, _)| n == "overflow-proof").map(|(_, v)| v.clone()) {
                Some(v) if v == "yes" => won.push("overflow-proof"),
                Some(v) if storage(c) => return (passed * 1000, Outcome::fail(format!("{}: {}", r.title(), overflow_required(&v)), out.table)),
                _ => {}
            }
        }
        if r == Run::Base {
            base = Some(out);
        }
    }
    let mut base = base.unwrap_or_else(|| Outcome::fail("no base run", Vec::new()));
    base.metrics.push(("slice width".to_string(), slice_width(c).unwrap_or(0).to_string()));
    if storage(c) {
        base.metrics.push(("storage".to_string(), storage_metric(c)));
    }
    base.metrics.push(("badges".to_string(), if won.is_empty() { "none".to_string() } else { won.join("+") }));
    (passed * 1000, base)
}

/// [`sim_verdict`]; `badges: false` skips the badge-only run (`direct`).
pub fn sim_verdict_with(c: &Circuit, badges: bool) -> Outcome {
    let run = |r: Run| match scenario_for(c, r) {
        Ok((sc, rc)) => match sim_run(&rc, &sc, 3) {
            Ok(log) => judge(&sc, &log),
            Err(why) => Outcome::fail(why, Vec::new()),
        },
        Err(e) => Outcome::fail(format!("{e:#}"), Vec::new()),
    };
    let mut base = run(Run::Base);
    if !base.pass {
        return base;
    }
    let Some(width) = slice_width(c) else { return Outcome::fail(NO_TILE.to_string(), base.table) };
    let mut won: Vec<&str> = Vec::new();
    for r in Run::required(c) {
        let out = run(r);
        if !out.pass {
            return Outcome::fail(format!("{}: {}", r.title(), out.why), out.table);
        }
        if r == Run::Jam {
            match out.metrics.iter().find(|(n, _)| n == "overflow-proof").map(|(_, v)| v.clone()) {
                Some(v) if v == "yes" => won.push("overflow-proof"),
                Some(v) if storage(c) => return Outcome::fail(format!("{}: {}", r.title(), overflow_required(&v)), out.table),
                _ => {}
            }
        }
    }
    if badges && run(Run::Direct).pass {
        won.push("stream-direct");
    }
    base.metrics.push(("slice width".to_string(), width.to_string()));
    if storage(c) {
        base.metrics.push(("storage".to_string(), storage_metric(c)));
    }
    base.metrics.push(("badges".to_string(), if won.is_empty() { "none".to_string() } else { won.join("+") }));
    base
}

/// `mctas circuit sorter-census` (a local search on redsim): designs `remove` or fewer of the base
/// design's blocks (hoppers and comparators stay) lighter, with up to `remove - 1` palette blocks
/// added in the air cells of `cells` (ports left out), so every candidate is smaller than the
/// base. A candidate passes the quick screen, then the whole redsim judge with the base's badges
/// (overflow-proof kept). Returns the passing designs (block count, the design), smallest first.
pub fn census(base: &Circuit, remove: usize, cells: (Pos, Pos), palette: &[String], threads: usize) -> Result<Vec<(usize, mctas::design::Design)>> {
    let Some(Kind::Sorter(item)) = &base.kind else { bail!("{}: not a sorter", base.name()) };
    let avoid = design_kinds(base);
    let quick = Scenario::quick(item, &avoid)?;
    let b0 = sim_verdict_with(base, false);
    if !b0.pass {
        bail!("{}: the base fails the redsim judge ({}), nothing to shrink", base.name(), b0.why);
    }
    let proof = b0.metrics.iter().any(|(n, v)| n == "badges" && v.contains("overflow-proof"));
    let keep = vec!["minecraft:hopper".to_string(), "minecraft:comparator".to_string()];
    let pass = |cand: &Circuit| {
        sim_run(cand, &quick, 3).is_ok_and(|log| judge(&quick, &log).pass) && {
            let v = sim_verdict_with(cand, false);
            v.pass && (!proof || v.metrics.iter().any(|(n, v)| n == "badges" && v.contains("overflow-proof")))
        }
    };
    Ok(crate::local::census(base, remove, cells, palette, &keep, threads, &pass))
}
