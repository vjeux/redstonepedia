//! Study 18, crop farms (designs/circuits/farm/WR.md, docs/circuits.md "Crop farms"): kind
//! `harvester <crop>` (`sugar_cane` for now). A harvester breaks every grown block of its crop and
//! sends the items into a chest, by itself, forever.
//!
//! Judged in the game only (redsim has no random ticks): the judge plants the crop on every `port
//! crop:` (an air cell on the design's soil; the crop is not counted), puts a chest on every `port
//! output:` (an air cell, not counted), raises `random_tick_speed` to [`RTS`] (100 times vanilla's
//! 3, so the [`RUN`] ticks of the run hold about a hundred growths per cane) and records every
//! crop column after every tick: each block that grows above the bottom one, each harvest (blocks
//! gone), how long a grown block waited and how tall the canes got. Then growth stops, the run
//! rests until nothing moves, and the judge counts the items.
//!
//! Pass: no crop is lost (the bottom block stays), no cane stands at full height longer than
//! [`STUCK`] ticks (a missed harvest), every harvested block's item is in the judge's chests at
//! the end (no item on the ground, none lost to despawning, none left in the design's
//! containers), nothing else is left loose, and the build comes to rest. Metrics: blocks per
//! cane, items per hour per cane at vanilla speed (the measured harvest, scaled by 3 / RTS), the
//! harvest height (2: harvested as soon as it grows; 3: when full) and the longest wait from a
//! block growing to its harvest. A `tile:` design is also judged as copies side by side.

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, stacks_of_be};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// `random_tick_speed` during the judged run (vanilla: 3), for farms of up to [`RTS_PLANTS`] plants.
pub const RTS: u32 = 300;
/// A bigger farm runs at RTS x RTS_PLANTS / plants (at least [`RTS_MIN`]), so its items arrive about
/// as fast as an 8-plant farm's at 300 (a hopper line's 2.5 items/s carries a 32-stem row then): a
/// full-size farm at 300 makes items 100 times faster than in the game, more than any collection
/// line carries, so items would pile up over full hoppers in a way no player's farm does (clk-rt and
/// w86, 2026-10-21; designs/briefs/farm-melon-pumpkin.md). The farm's rates come from the vanilla
/// twin run anyway; these runs judge robustness.
pub const RTS_PLANTS: u32 = 8;
pub const RTS_MIN: u32 = 30;

/// The judge's random_tick_speed for a design (by its plant count).
pub fn rts_for(c: &Circuit) -> u32 {
    let plants = c.stores.iter().filter(|s| s.role == "crop").count().max(1) as u32;
    if plants <= RTS_PLANTS { RTS } else { (RTS * RTS_PLANTS / plants).max(RTS_MIN) }
}

/// The vanilla-speed twin run of a farm's base build (the source of its rates), run once per verify.
pub type VanillaRun = std::result::Result<std::result::Result<RunLog, String>, String>;

/// [`VanillaRun`] of `c`: the redsim twin at random_tick_speed 3 for [`VANILLA_RUN`] gt.
pub fn vanilla_run(c: &Circuit, margin: i32) -> VanillaRun {
    sim_run(c, VANILLA_RTS, VANILLA_RUN, margin)
}

/// The robustness runs' random_tick_speed for `c` (a build of `base`, or its copies side by side):
/// [`rts_for`], and no faster than the farm's own collection carries. That is at most 2.5 items/s per
/// output (one hopper's rate) over the farm's vanilla items per second from its twin run; a farm
/// whose collection is sized for the game's speed would otherwise back up and drop items only at
/// the judge's speed (w86's melon/pumpkin brief, 2026-10-22: one collector for a 64-stem lattice).
pub fn game_rts(c: &Circuit, base: &Circuit, vanilla: &VanillaRun) -> u32 {
    let r = rts_for(c);
    let Ok(Ok(log)) = vanilla else { return r };
    if !judge(base, log).pass {
        return r;
    }
    let per_s = per_hour_vanilla(log, &crop_of(base)) * log.canes as f64 / 3600.0;
    if per_s <= 0.0 {
        return r;
    }
    let ports = base.outputs.len().max(1) as f64;
    let cap = (VANILLA_RTS as f64 * 2.5 * ports / per_s).floor() as u32;
    r.min(cap.max(VANILLA_RTS))
}

/// A robustness run's length at `rts`: [`RUN`], longer (up to 4 x) when [`game_rts`] slowed the run
/// below [`rts_for`], so the plants still grow about as often per run.
pub fn game_ticks(c: &Circuit, rts: u32) -> u32 {
    RUN * (rts_for(c) / rts.max(1)).clamp(1, 4)
}
/// Vanilla's `random_tick_speed`, for the per-hour metric.
pub const VANILLA_RTS: u32 = 3;
/// The vanilla-speed twin run that measures a farm's rates (`verify`): 720000 gt, 10 game hours.
pub const VANILLA_RUN: u32 = 720_000;
/// The judged run: 20 minutes of game time.
pub const RUN: u32 = 24000;
/// Ticks per recorded batch (the intruders fall on batch boundaries; the 3D view replays them:
/// `mcview::circuit::FARM_BATCH`).
pub const BATCH: u32 = 500;
/// `mctas circuit farm-run --trace`: 20-tick batches, and every crop item seen outside the crop
/// columns and the cells above the design's hoppers is noted (where items get lost).
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A cane may stand at full height (3) at most this long before it is harvested.
pub const STUCK: u32 = 40;
/// The judge empties its chests every EMPTY gt during the run (counting what it takes).
pub const EMPTY: u32 = 500;
/// At most REGROW gt (at rts 300, scaled by 300 / rts) of growth after the run while a crop item
/// lies on a crop: its next growth frees it ([`finish`]).
pub const REGROW: u32 = 1600;
/// Game ticks per hour.
const HOUR: f64 = 72000.0;
/// ITEM INTRUSION ([`crate::intrusion`], like every kind): at a quarter, half and three quarters
/// of the run the judge drops each of [`crate::intrusion::DROPS`] (one cobblestone, 64 iron ingots,
/// an iron shovel) on every open hopper intake of the design and above every crop column (items
/// falling onto the farm): they may end anywhere, but the crop must still all reach the chests.
pub const INTRUDERS: [(&str, u32); 3] = [("minecraft:cobblestone", 1), ("minecraft:iron_ingot", 64), ("minecraft:iron_shovel", 1)];

/// The crops a harvester may name, with their full block ids and the state the judge plants.
pub const CROPS: [(&str, &str); 6] = [
    ("sugar_cane", "minecraft:sugar_cane[age=0]"),
    ("bamboo", "minecraft:bamboo[age=0,leaves=none,stage=0]"),
    ("kelp", "minecraft:kelp[age=0]"),
    ("pumpkin", "minecraft:pumpkin_stem[age=0]"),
    ("melon", "minecraft:melon_stem[age=0]"),
    ("cactus", "minecraft:cactus[age=0]"),
];

/// A stem crop (pumpkin, melon): the judge plants the stem; its fruit grows on one of the four
/// cells beside it (the game picks a side at random; it grows there only on air with dirt or
/// farmland below). The column's height is 1 for the stem, 2 while a fruit is attached to it.
pub fn is_stem_crop(crop: &str) -> bool {
    crop == "minecraft:pumpkin" || crop == "minecraft:melon"
}

/// A crop whose harvests the judge also counts by their drops, tick by tick: stems (a fruit can be
/// broken within the tick it grew, by a piston another stem's observer fires; always counted by
/// drops) and bamboo (a block can be broken within the tick it grew, by a piston another stalk's
/// observer fires: bamboo grows with no memory, so farms cut every stalk together). For bamboo the
/// drops count only when they exceed the height count: a design whose containers take every drop
/// in the tick it appears (bamboo-hc-12b's carts) keeps the height count. Each drop is one stack.
pub fn counts_drops(crop: &str) -> bool {
    is_stem_crop(crop) || crop == "minecraft:bamboo"
}

/// The item a harvest gives: the crop's own item, melon slices for a melon (3 to 7 a fruit, in one
/// stack).
pub fn crop_item(crop: &str) -> &str {
    if crop == "minecraft:melon" { "minecraft:melon_slice" } else { crop }
}

/// A crop name (`sugar_cane` or `minecraft:sugar_cane`) as its full id.
pub fn crop_id(name: &str) -> Result<String> {
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    match CROPS.iter().find(|(c, _)| *c == short) {
        Some((c, _)) => Ok(format!("minecraft:{c}")),
        None => bail!("circuit: harvester {name:?}: the crops judged so far are {}", CROPS.iter().map(|c| c.0).collect::<Vec<_>>().join(", ")),
    }
}

fn planted_state(crop: &str) -> &'static str {
    let short = crop.strip_prefix("minecraft:").unwrap_or(crop);
    CROPS.iter().find(|(c, _)| *c == short).map(|c| c.1).unwrap_or("minecraft:sugar_cane[age=0]")
}

/// The tallest a crop grows by itself (sugar cane: 3; bamboo: 16, though a stalk may stop earlier:
/// its top block then reads `stage=1`).
pub fn full_height(crop: &str) -> u32 {
    if crop.ends_with("bamboo") {
        16
    } else if crop.ends_with("kelp") {
        26
    } else if is_stem_crop(crop) {
        2
    } else {
        3
    }
}

/// Whether growths are counted by age resets (sugar cane: only the top block ages, and a growth
/// restarts it at 0, so a block destroyed in the tick it grew still counts) or by height (bamboo:
/// no age counter; its harvesters fire only on the growth itself).
fn grows_by_age(crop: &str) -> bool {
    // kelp: a harvested stalk's new top takes a random age, so ages cannot count growths; stems:
    // a fruit is counted when it attaches
    !crop.ends_with("bamboo") && !crop.ends_with("kelp") && !is_stem_crop(crop)
}

/// Whether the column has stopped growing by itself: at full height, (bamboo) its top block reads
/// `stage=1`, or (kelp) its top is at age 25 or has no water (still or flowing) above it.
fn stopped(frame: &mctas::sim::Region, p: Pos, h: u32, crop: &str) -> bool {
    let top = frame.get([p[0], p[1] + h as i32 - 1, p[2]]);
    h >= full_height(crop)
        || (h >= 2 && crop.ends_with("bamboo") && top.is_some_and(|s| s.contains("stage=1")))
        || (h >= 1 && crop.ends_with("kelp") && (top.is_some_and(|s| s.contains("age=25")) || frame.get([p[0], p[1] + h as i32, p[2]]).is_some_and(|s| !s.starts_with("minecraft:water"))))
}

/// A water source block (what kelp grows into).
pub fn is_water_source(s: &str) -> bool {
    s == "minecraft:water" || s == "minecraft:water[level=0]"
}

/// Whether a block state is part of the crop's column (kelp: its top `kelp` and the `kelp_plant`
/// below it).
fn is_crop_block(s: &str, crop: &str) -> bool {
    let is = |id: &str| s == id || s.strip_prefix(id).is_some_and(|r| r.starts_with('['));
    is(crop) || (crop.ends_with("kelp") && is("minecraft:kelp_plant"))
}

/// The burn time (gt) of a crop item that is a furnace fuel, from 26.3's
/// `ContextIntProviders.bootstrap` (bamboo 50 gt: a quarter of an item smelted); None for the rest.
fn fuel_burn(item: &str) -> Option<u32> {
    match item {
        "minecraft:bamboo" => Some(50),
        _ => None,
    }
}

/// One run's record.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    pub canes: usize,
    pub ticks: u32,
    pub rts: u32,
    /// Blocks that grew above the bottom ones.
    pub grown: u64,
    /// Blocks harvested (gone from above the bottom ones).
    pub harvested: u64,
    /// Items of the crop in the judge's chests at the end.
    pub delivered: u64,
    /// Items of the crop that the design's own containers hold at the end, minus at the start.
    pub held: i64,
    /// The design's containers, every kind: at the start and at the end.
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    /// Entities alive at the end that the design did not declare (the intruders aside).
    pub loose: Vec<String>,
    /// The intruders: loose at the end, and in the judge's chests; the cells they were dropped on.
    pub intruders_loose: u64,
    pub intruders_delivered: u64,
    pub drop_cells: usize,
    /// A crop whose bottom block broke (tick, position).
    pub lost: Option<String>,
    /// A cane left at full height longer than [`STUCK`].
    pub stuck: Option<String>,
    /// (a note) the gt of growth resumed at the end while a crop item lay on a crop.
    pub regrown: u32,
    /// (a note) crop items seen outside the build's box (uuid, where and when).
    pub left: Vec<(String, String)>,
    /// (a note) a harvester whose items settled but whose blocks keep moving: a clock.
    pub clocked: Option<String>,
    /// (a note) the first crop that waited longer than [`STUCK`] at full height during the run.
    pub waited: Option<String>,
    /// The longest a grown block waited for its harvest (ticks), and the tallest a cane got.
    pub wait_max: u32,
    pub height_max: u32,
    /// The longest a cane stood at full height.
    pub full_max: u32,
    pub restless: String,
    /// Crop items seen loose at two batch ends in a row (where items get stranded).
    pub stranded: Vec<String>,
    /// (melon) the drop stacks seen, and their items when first seen: what the harvests gave.
    pub drops_seen: u64,
    pub drop_items: u64,
    /// The YIELD rule: the growths the plants give under free growth in the run (the crop's own
    /// odds per random tick at its conditions: [`free_growth`]), and their spread.
    pub expected: f64,
    pub spread: f64,
    /// Growth-phase ticks with a moving piston or a piston head in a plant's next cell (the cell
    /// above its top): the random ticks landing then are lost. A transient loss of the judge's 100x
    /// random-tick speed (the head stays a few gt per harvest, which is 100 times rarer next to the
    /// growth at vanilla speed), so the rates add those growths back ([`speedup_items`]).
    pub blocked: u64,
    /// (`--trace`) the tick-by-tick path of the first crop items seen off their way.
    pub trails: Vec<String>,
    /// Entities seen leaving the build's box (the first per batch).
    pub escaped: Vec<String>,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

/// Run a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, rts: u32, ticks: u32, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let (from, to, seed) = judge_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    let r = run_in(w, sb, c, rts, ticks, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

/// Which drop (its number in the sandbox and its cell: `rng::Drop::seeded`, the bridge's
/// `DropSeeder`) an item seen after its first tick at `pos` is, in a box of a sandbox seeded
/// `seed` (the twin's calibration probe).
pub fn which_drop(seed: i64, from: Pos, to: Pos, pos: [f64; 3]) -> Vec<String> {
    let mut found = Vec::new();
    for n in 0..5000u64 {
        for cx in from[0]..=to[0] {
            for cy in from[1]..=to[1] {
                for cz in from[2]..=to[2] {
                    let d = redsim::rng::Drop::seeded(seed, n, redsim::BlockPos::new(cx, cy, cz));
                    if (d.pos[0] + d.motion[0] - pos[0]).abs() < 1e-5 && (d.pos[2] + d.motion[2] - pos[2]).abs() < 1e-5 {
                        found.push(format!("drop {n} of cell [{cx}, {cy}, {cz}]"));
                    }
                }
            }
        }
    }
    found
}

/// The drop numbers whose drift is `motion` seen after the first tick (x and z, the drag 0.98F
/// applied): the drift is a function of the number alone.
pub fn which_drift(seed: i64, motion: [f64; 3]) -> Vec<u64> {
    let drag = 0.98f32 as f64;
    (0..100_000u64).filter(|n| {
        let d = redsim::rng::Drop::seeded(seed, *n, redsim::BlockPos::new(0, 0, 0));
        (d.motion[0] * drag - motion[0]).abs() < 1e-6 && (d.motion[2] * drag - motion[2]).abs() < 1e-6
    }).collect()
}

/// A build's box (the watched box, raised for the crop to grow) and its sandbox seed.
pub fn judge_box(c: &Circuit, margin: i32) -> (Pos, Pos, i64) {
    let (mut from, mut to) = crate::run::watch_box(c, margin);
    // room for the crop to grow
    to[1] = to[1].max(c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos[1] + 3).max().unwrap_or(to[1]));
    from[1] = from[1].min(to[1] - 2);
    (from, to, shulker::build_seed(from, to, 0))
}

/// The twin's calibration probe in the game (`farm-run --compare --probe`): [`run_in`]'s build
/// and run, one tick per step, every tick's state as [`sim_digest`] writes it (from the run's
/// start), up to `ticks + 2 * REST`.
pub fn game_probe(w: &mut Worker, c: &Circuit, rts: u32, ticks: u32, margin: i32) -> Result<Vec<String>> {
    let Some(Kind::Harvester(crop)) = &c.kind else { bail!("{}: not a harvester", c.name()) };
    let (from, to, seed) = judge_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    clock::verify::place(w, sb, &c.clock)?;
    let crops: Vec<Pos> = c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos).collect();
    let chests: Vec<(Pos, String)> = c.outputs.iter().map(|o| (o.pos, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    w.exec(sb, "gamerule random_tick_speed 0")?;
    let planted: Vec<(Pos, String)> = crops.iter().map(|p| (*p, planted_state(crop).to_string())).collect();
    w.set_blocks(sb, &planted, PlaceMode::Update)?;
    w.step(sb, c.clock.free_from() + SETTLE)?;
    let full = full_height(crop);
    let digest = |w: &mut Worker| -> Result<String> {
        let r = w.get_region(sb, from, to)?;
        let blocks: Vec<String> = r.iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(p, s)| format!("{p:?} {s}")).collect();
        let mut ents: Vec<String> = w.entities(sb)?.iter().map(|e| format!("{} {:.6} {:.6} {:.6} m {:.6} {:.6} {:.6}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.motion[0], e.motion[1], e.motion[2])).collect();
        ents.sort();
        let mut held: Vec<(Pos, Option<i32>, Vec<(usize, String, u32)>)> = Vec::new();
        for be in w.block_entities(sb, from, to)? {
            // the hopper's TransferCooldown, and the slots as redsim's parser reads them
            let cd = be.nbt.split("TransferCooldown:").nth(1).and_then(|s| s.split(|c: char| !(c.is_ascii_digit() || c == '-')).next()).and_then(|n| n.parse::<i32>().ok());
            let slots: Vec<(usize, String, u32)> = match redsim::redstone::container::entity_items(&be.nbt) {
                Ok(v) => v.into_iter().map(|(i, s)| (i, norm_id(redsim::redstone::container::item_name(s.item)), s.count as u32)).collect(),
                Err(_) => stacks_of_be(&be.nbt).into_iter().enumerate().map(|(i, s)| (i, s.id, s.count)).collect(),
            };
            held.push((be.pos, cd, slots));
        }
        let (rng, rv) = w.random_states(sb)?;
        Ok(format!("{} | {} | {} | rng {rng} rv {rv}", blocks.join("; "), ents.join("; "), probe_containers(held)))
    };
    let mut out = vec![digest(w)?];
    w.exec(sb, &format!("gamerule random_tick_speed {rts}"))?;
    let intakes = crate::intrusion::intakes(c);
    let drop_at: Vec<u32> = (1..=3).map(|k| (ticks * k / 4 / BATCH) * BATCH).collect();
    let drop_cells: Vec<[f64; 3]> = drop_cells_of(&intakes, &crops, full);
    for t in 0..ticks + 2 * REST {
        if t == ticks {
            w.exec(sb, "gamerule random_tick_speed 0")?;
        }
        if let Some(k) = drop_at.iter().position(|d| *d == t && t > 0) {
            w.summon(sb, &intruder_specs(k, &drop_cells))?;
        }
        w.step(sb, 1)?;
        out.push(digest(w)?);
        // (run_in's) the judge empties its chests every EMPTY gt, after that tick
        if (t + 1) % EMPTY == 0 {
            for (p, _) in &chests {
                let took = w.block_entities(sb, *p, *p)?.iter().any(|be| stacks_of_be(&be.nbt).iter().any(|s| s.id == crop_item(crop) || INTRUDERS.iter().any(|(id, _)| *id == s.id)));
                if took {
                    w.exec(sb, &format!("data merge block {} {} {} {{Items:[]}}", p[0], p[1], p[2]))?;
                }
            }
        }
    }
    let _ = w.drop_sandbox(sb);
    Ok(out)
}

/// The kinds and counts the design's own containers hold (judge chests left out).
fn held(w: &mut Worker, sb: u32, from: Pos, to: Pos, judge: &[Pos]) -> Result<BTreeMap<String, u64>> {
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

/// The crop column's height: crop blocks from `p` up.
fn height(frame: &mctas::sim::Region, p: Pos, crop: &str) -> u32 {
    if is_stem_crop(crop) {
        // 1 for the stem, 2 while a fruit is attached (an attached stem faces its fruit)
        let name = crop.trim_start_matches("minecraft:");
        return match frame.get(p) {
            Some(s) if s.starts_with(&format!("minecraft:{name}_stem")) => 1,
            Some(s) if s.starts_with(&format!("minecraft:attached_{name}_stem")) => 2,
            _ => 0,
        };
    }
    let mut h = 0;
    while frame.get([p[0], p[1] + h as i32, p[2]]).is_some_and(|s| is_crop_block(s, crop)) {
        h += 1;
    }
    h
}

/// Per crop column, what the judge follows tick by tick.
struct Column {
    pos: Pos,
    h: u32,
    /// Each standing block's age, bottom first (a growth resets the grower's age to 0).
    ages: Vec<u8>,
    /// The height before the run.
    h0: u32,
    /// The tick each block above the bottom one grew (bottom first).
    grew: Vec<u32>,
    full_since: Option<u32>,
}

/// The ages of the column's standing crop blocks, bottom first.
fn ages_of(frame: &mctas::sim::Region, p: Pos, h: u32) -> Vec<u8> {
    (0..h).map(|i| frame.get([p[0], p[1] + i as i32, p[2]]).and_then(|s| s.split_once("age=")).and_then(|(_, r)| r.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|a| a.parse::<u8>().ok()).unwrap_or(0)).collect()
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, rts: u32, ticks: u32, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    let Some(Kind::Harvester(crop)) = &c.kind else { bail!("{}: not a harvester", c.name()) };
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let crops: Vec<Pos> = c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos).collect();
    let judge: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let chests: Vec<(Pos, String)> = judge.iter().map(|p| (*p, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    w.exec(sb, "gamerule random_tick_speed 0")?;
    let planted: Vec<(Pos, String)> = crops.iter().map(|p| (*p, planted_state(crop).to_string())).collect();
    w.set_blocks(sb, &planted, PlaceMode::Update)?;
    w.step(sb, c.clock.free_from() + SETTLE)?;
    // the record box: every crop column, full height
    let full = full_height(crop);
    let rf = [crops.iter().map(|p| p[0]).min().unwrap(), crops.iter().map(|p| p[1]).min().unwrap(), crops.iter().map(|p| p[2]).min().unwrap()];
    let rt = [crops.iter().map(|p| p[0]).max().unwrap(), crops.iter().map(|p| p[1]).max().unwrap() + full as i32, crops.iter().map(|p| p[2]).max().unwrap()];
    let start = w.get_region(sb, rf, rt)?;
    if let Some(p) = crops.iter().find(|p| height(&start, **p, crop) == 0) {
        let why = if crop.ends_with("kelp") {
            "kelp needs a water source at the crop cell, water above it and a sturdy block or kelp below it"
        } else if crop.ends_with("bamboo") {
            "bamboo needs dirt, mud, sand or gravel below it"
        } else if is_stem_crop(crop) {
            "a pumpkin or melon stem needs farmland below it"
        } else if crop.ends_with("cactus") {
            "cactus needs sand below it and nothing solid beside it"
        } else {
            "sugar cane needs sand, dirt or grass below it with water beside that block"
        };
        return Ok(Err(format!("the crop planted at {p:?} does not survive ({why})")));
    }
    let mut cols: Vec<Column> = crops.iter().map(|p| {
        let h = height(&start, *p, crop);
        Column { pos: *p, h, ages: ages_of(&start, *p, h), h0: h, grew: Vec::new(), full_since: None }
    }).collect();
    let mut log = RunLog { canes: crops.len(), ticks, rts, dims: shulker::design_dims(c), initial: held(w, sb, from, to, &judge)?, ..Default::default() };
    // the YIELD rule's expectation: each plant's free growth at its own conditions
    for p in &crops {
        let around = w.get_region(sb, [p[0] - 2, p[1] - 1, p[2] - 2], [p[0] + 2, p[1], p[2] + 2])?;
        let (e, d) = free_growth(crop, ticks, rts, &around, *p);
        log.expected += e;
        log.spread = (log.spread * log.spread + d * d).sqrt();
    }
    for col in &cols {
        if col.h > 1 {
            log.notes.push(format!("the crop at {:?} stands {} high before the run", col.pos, col.h));
        }
    }
    w.exec(sb, &format!("gamerule random_tick_speed {rts}"))?;
    let mut t = 0u32;
    let mut seen_items: Vec<String> = Vec::new();
    // each crop item's last position inside the build (to say where one that leaves came from)
    let mut last_pos: BTreeMap<String, Vec<([f64; 3], u32)>> = BTreeMap::new();
    // an entity leaving the build's box (an item falling off it: lost) is noted
    let watch = mctas::sim::EntityWatch { watch: (from, to), walk: ([from[0], from[1] - 64, from[2]], [from[0], from[1] - 64, from[2]]) };
    // the run, then the rest (growth stopped) while the last harvests come in
    let rest_ticks = 2 * REST;
    let trace = TRACE.load(std::sync::atomic::Ordering::Relaxed);
    // a melon breaks into 3 to 7 slices (one stack), and a stem's fruit may be broken within the tick
    // it grew (another stem's observer firing the shared piston): every crop item stack is seen once,
    // tick by tick, to know what the harvests gave (the loss check is exact when every drop was seen)
    let count_drops = crop_item(crop) != crop || counts_drops(crop);
    let mut ever: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // (trace) one tick per batch, a wider record box: every crop item's path, tick by tick
    let batch = if trace || count_drops { 1 } else { BATCH };
    let (qf, qt) = if trace { ([rf[0] - 2, rf[1] - 2, rf[2] - 2], [rt[0] + 2, rt[1] + 1, rt[2] + 2]) } else { (rf, rt) };
    let mut trails: BTreeMap<String, Vec<TrailPoint>> = BTreeMap::new();
    let mut reported: Vec<String> = Vec::new();
    let mut column_since: BTreeMap<String, u32> = BTreeMap::new();
    // item intrusion: one of the drops at a quarter, half and three quarters of the run, on every
    // open hopper intake (crate::intrusion) and above every crop column (items falling onto the farm)
    let intakes = crate::intrusion::intakes(c);
    let drop_at: Vec<u32> = (1..=3).map(|k| (ticks * k / 4 / batch) * batch).collect();
    let drop_cells: Vec<[f64; 3]> = drop_cells_of(&intakes, &crops, full);
    log.drop_cells = drop_cells.len();
    // (trace) the cells an item may be in on its way: the crop columns (a stem's: with the four
    // fruit cells beside it) and above the hoppers
    let hoppers: Vec<Pos> = c.clock.design.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:hopper")).map(|(p, _)| *p).collect();
    let reach = if is_stem_crop(crop) { 1.2 } else { 0.2 };
    let on_way = |p: [f64; 3]| {
        crops.iter().any(|q| p[0] >= q[0] as f64 - reach && p[0] <= q[0] as f64 + 1.0 + reach && p[2] >= q[2] as f64 - reach && p[2] <= q[2] as f64 + 1.0 + reach && p[1] >= q[1] as f64 - 0.2)
            || hoppers.iter().any(|h| p[0] >= h[0] as f64 - 0.2 && p[0] <= h[0] as f64 + 1.2 && p[2] >= h[2] as f64 - 0.2 && p[2] <= h[2] as f64 + 1.2 && p[1] >= h[1] as f64 + 0.5 && p[1] <= h[1] as f64 + 3.2)
    };
    while t < ticks + rest_ticks {
        if t == ticks {
            w.exec(sb, "gamerule random_tick_speed 0")?;
        }
        if let Some(k) = drop_at.iter().position(|d| *d == t && t > 0) {
            // the bridge's summon (clk-misc 2026-10-21, w86 agreed): each item's own random is
            // seeded (`moveTowardsClosestSpace` draws from it when a piston pushes the item into a
            // block), so a run repeats for its seed and the redsim twin follows it ([`sim_run`]);
            // vanilla's summon command left it unseeded
            w.summon(sb, &intruder_specs(k, &drop_cells))?;
        }
        let n = batch.min(if t < ticks { ticks - t } else { ticks + rest_ticks - t });
        let r = w.step_record_watch(sb, n, false, qf, qt, &watch)?;
        // an escape above the build's box is an intruder at its drop point (crop columns' tops)
        let above_box = |e: &str| e.split(" at ").nth(1).and_then(|p| p.split_whitespace().nth(1)).and_then(|y| y.parse::<f64>().ok()).is_some_and(|y| y > (to[1] + 1) as f64);
        if let Some(e) = &r.entity_escape
            && log.escaped.len() < 4
            && !above_box(e)
        {
            log.escaped.push(format!("batch from tick {t}: {e}"));
        }
        for frame in &r.frames {
            t += 1;
            follow(&mut cols, frame, crop, t, &mut log);
            // the speed-up's transient loss: a piston in a plant's next cell during growth
            if t <= ticks && !is_stem_crop(crop) {
                for col in &cols {
                    let above = [col.pos[0], col.pos[1] + col.h as i32, col.pos[2]];
                    if frame.get(above).is_some_and(|s| s.starts_with("minecraft:moving_piston") || s.starts_with("minecraft:piston_head")) {
                        log.blocked += 1;
                    }
                }
            }
        }
        if log.lost.is_some() {
            column_notes(w, sb, &cols, t, &mut log)?;
            break;
        }
        if t == ticks && log.grown == 0 {
            column_notes(w, sb, &cols, t, &mut log)?;
        }
        // the judge empties its chests every EMPTY gt (counting what it takes), as a player empties a
        // farm's chests: a run at 100x speed fills a chest 100 times faster than the farm would
        if t % EMPTY == 0 {
            for p in &judge {
                let mut any = false;
                for be in w.block_entities(sb, *p, *p)? {
                    for st in stacks_of_be(&be.nbt) {
                        if st.id == crop_item(crop) {
                            log.delivered += st.count as u64;
                            any = true;
                        } else if INTRUDERS.iter().any(|(id, _)| *id == st.id) {
                            log.intruders_delivered += st.count as u64;
                            any = true;
                        }
                    }
                }
                if any {
                    w.exec(sb, &format!("data merge block {} {} {} {{Items:[]}}", p[0], p[1], p[2]))?;
                }
            }
        }
        // stranded items: a crop item still loose one batch later (a note: where items get lost)
        let mut now_items: Vec<String> = Vec::new();
        for e in w.entities(sb)? {
            if e.design || shulker::stack_of_entity(&e.state).is_none_or(|s| s.id != crop_item(crop)) {
                continue;
            }
            // a crop item outside the build's box, below or beside it (it falls out of the world:
            // lost with no trace; one above the box is a tall crop's item still in the air)
            let out = (0..3).any(|i| e.pos[i] < from[i] as f64 - 0.5 || (i != 1 && e.pos[i] > (to[i] + 1) as f64 + 0.5));
            if out && !log.left.iter().any(|(u, _)| *u == e.uuid) && log.left.len() < 6 {
                let before = last_pos.get(&e.uuid).map(|v| v.iter().step_by(4).map(|(p, t0)| format!("t{t0} {p:.2?}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                log.left.push((e.uuid.clone(), format!("tick {t}: a crop item left the build at {:.2?} (motion {:.2?}); before: {before}", e.pos, e.motion)));
            }
            if !out {
                let v = last_pos.entry(e.uuid.clone()).or_default();
                v.push((e.pos, t));
                if v.len() > 80 {
                    v.remove(0);
                }
            }
            // an item about to despawn (Age counts to 6000): where it has been lying
            let age = e.state.split("Age: ").nth(1).and_then(|r| r.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|a| a.parse::<u32>().ok()).unwrap_or(0);
            if age >= 5800 && !log.left.iter().any(|(u, _)| *u == e.uuid) && log.left.len() < 6 {
                log.left.push((e.uuid.clone(), format!("tick {t}: a crop item about to despawn (age {age}) at {:.2?} (motion {:.2?})", e.pos, e.motion)));
            }
            // (melon) every drop stack's count when first seen: what the harvest gave
            if count_drops && !ever.contains(&e.uuid) {
                ever.insert(e.uuid.clone());
                log.drops_seen += 1;
                log.drop_items += shulker::stack_of_entity(&e.state).map(|s| s.count as u64).unwrap_or(0);
            }
            if !trace && seen_items.contains(&e.uuid) && log.stranded.len() < 6 {
                let under = [e.pos[0].floor() as i32, (e.pos[1] - 0.01).floor() as i32, e.pos[2].floor() as i32];
                let what = w.get_region(sb, under, under).ok().and_then(|r| r.get(under).map(|s| clock::short(s).to_string())).unwrap_or_default();
                log.stranded.push(format!("tick {t}: an item at {:.2?} on {what} at {under:?} (motion {:.2?})", e.pos, e.motion));
            }
            if trace {
                let frame = r.frames.last();
                let cell = |p: [f64; 3]| -> String {
                    let q = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
                    frame.and_then(|f| f.get(q)).map(|s| clock::short(s).to_string()).unwrap_or_else(|| "?".into())
                };
                let pt = TrailPoint { t0: t, t1: t, pos: e.pos, motion: e.motion, inside: cell(e.pos), on: cell([e.pos[0], e.pos[1] - 0.01, e.pos[2]]), column: column_letters(frame, &crops, e.pos, full) };
                let tr = trails.entry(e.uuid.clone()).or_default();
                match tr.last_mut() {
                    Some(last) if last.pos == pt.pos && last.inside == pt.inside && last.on == pt.on && last.column == pt.column => last.t1 = t,
                    _ => {
                        tr.push(pt);
                        if tr.len() > TRAIL {
                            tr.remove(0);
                        }
                    }
                }
                // an item resting on a crop (above its soil) for 5 ticks: only the next growth clears it
                let in_column = crops.iter().any(|q| e.pos[0] >= q[0] as f64 && e.pos[0] < q[0] as f64 + 1.0 && e.pos[2] >= q[2] as f64 && e.pos[2] < q[2] as f64 + 1.0 && e.pos[1] >= q[1] as f64 + 0.99);
                let since = *column_since.entry(e.uuid.clone()).or_insert(t);
                if !in_column {
                    column_since.insert(e.uuid.clone(), t);
                }
                // an item resting on a crop for 5 ticks, or lingering above it for 40 (afloat)
                let on_crop = in_column && (tr.last().is_some_and(|p| p.t1 >= p.t0 + 4 && p.motion.iter().all(|m| m.abs() < 1e-9)) || t >= since + 40);
                if (!on_way(e.pos) || on_crop) && !reported.contains(&e.uuid) && reported.len() < 4 {
                    reported.push(e.uuid.clone());
                    let what = if on_crop { "resting on a crop" } else { "off its way" };
                    log.trails.push(format!("a crop item {what} at tick {t}; its last {} moves (column: the nearest crop, from under its soil up):", tr.len()));
                    log.trails.extend(tr.iter().map(TrailPoint::line));
                }
            }
            now_items.push(e.uuid.clone());
        }
        if trace {
            // a crop item gone while inside a crop column (killed there: a cactus) is reported
            for (uuid, tr) in trails.iter() {
                if now_items.contains(uuid) || reported.contains(uuid) || reported.len() >= 4 {
                    continue;
                }
                let Some(last) = tr.last() else { continue };
                let inside = crops.iter().any(|q| last.pos[0] >= q[0] as f64 - 0.125 && last.pos[0] < q[0] as f64 + 1.125 && last.pos[2] >= q[2] as f64 - 0.125 && last.pos[2] < q[2] as f64 + 1.125 && last.pos[1] < q[1] as f64 + full as f64);
                if inside {
                    reported.push(uuid.clone());
                    log.trails.push(format!("a crop item gone inside a crop column at tick {t}; its last {} moves:", tr.len()));
                    log.trails.extend(tr.iter().map(TrailPoint::line));
                }
            }
            trails.retain(|k, _| now_items.contains(k));
            column_since.retain(|k, _| now_items.contains(k));
        }
        seen_items = now_items;
    }
    w.exec(sb, "gamerule random_tick_speed 0")?;
    end_stuck(&cols, t, &mut log);
    finish(w, sb, c, crop, from, to, &judge, &mut log)?;
    Ok(Ok(log))
}

/// At most this many intakes and this many crop columns get ITEM INTRUSION's drops (evenly spread):
/// a full-size farm would otherwise get thousands of stray items at once, which no player drops
/// (clk-rt, 2026-10-21).
pub const DROP_SAMPLE: usize = 8;

/// The cells ITEM INTRUSION drops its items on: up to [`DROP_SAMPLE`] intakes (on them) and crop
/// columns (above their full height), spread evenly over each list.
fn drop_cells_of(intakes: &[Pos], crops: &[Pos], full: u32) -> Vec<[f64; 3]> {
    let pick = |v: &[Pos]| -> Vec<Pos> { if v.len() <= DROP_SAMPLE { v.to_vec() } else { (0..DROP_SAMPLE).map(|i| v[i * v.len() / DROP_SAMPLE]).collect() } };
    pick(intakes).iter().map(|p| [p[0] as f64 + 0.5, p[1] as f64 + 0.25, p[2] as f64 + 0.5]).chain(pick(crops).iter().map(|p| [p[0] as f64 + 0.5, (p[1] + full as i32 + 1) as f64, p[2] as f64 + 0.5])).collect()
}

/// A missed harvest: a crop still stopped at full height (a stem still attached) at the end of the
/// run and its rest, longer than [`STUCK`] (nothing will ever harvest it).
fn end_stuck(cols: &[Column], t: u32, log: &mut RunLog) {
    for col in cols {
        if let Some(since) = col.full_since
            && t.saturating_sub(since) > STUCK
            && log.stuck.is_none()
        {
            log.stuck = Some(format!("the crop at {:?} still stands {} high at the end of the run, unharvested since tick {since}", col.pos, col.h));
        }
    }
}

/// (`--trace`) points kept per crop item path (a resting item's ticks fold into one point).
const TRAIL: usize = 160;

/// (`--trace`) one point of a crop item's path: from tick t0 to t1 at `pos` with `motion`, the
/// block its center is in and the block under it.
struct TrailPoint {
    t0: u32,
    t1: u32,
    pos: [f64; 3],
    motion: [f64; 3],
    inside: String,
    on: String,
    /// The nearest crop's column from its soil up, one letter a cell.
    column: String,
}

/// (`--trace`) the column of the crop nearest to `p`, from its soil up to `full` + 1 blocks: `b`
/// the crop, `m` a moving piston, `h` a piston head, `.` air, `#` anything else.
fn column_letters(frame: Option<&mctas::sim::Region>, crops: &[Pos], p: [f64; 3], full: u32) -> String {
    let d = |q: &Pos| (q[0] as f64 + 0.5 - p[0]).powi(2) + (q[2] as f64 + 0.5 - p[2]).powi(2);
    let Some(q) = crops.iter().min_by(|a, b| d(a).total_cmp(&d(b))) else { return String::new() };
    let top = (full + 1).min(5) as i32;
    (-1..=top)
        .map(|k| match frame.and_then(|f| f.get([q[0], q[1] + k, q[2]])) {
            None => '?',
            Some(s) if s.contains("moving_piston") => 'm',
            Some(s) if s.contains("piston_head") => 'h',
            Some(s) if mctas::blockstate::is_air(s) => '.',
            Some(s) if s.contains("bamboo") || s.contains("sugar_cane") || s.contains("kelp") || s.contains("_stem") || s.contains("cactus") => 'b',
            Some(s) if s == "minecraft:pumpkin" || s == "minecraft:melon" => 'f',
            Some(_) => '#',
        })
        .collect()
}

impl TrailPoint {
    fn line(&self) -> String {
        let ticks = if self.t1 > self.t0 { format!("{}-{}", self.t0, self.t1) } else { self.t0.to_string() };
        let [x, y, z] = self.pos;
        let [u, v, s] = self.motion;
        format!("  tick {ticks}: at [{x:.3}, {y:.3}, {z:.3}] motion [{u:.3}, {v:.3}, {s:.3}] in {} on {}, column {}", self.inside, self.on, self.column)
    }
}

/// Why a crop broke or never grew: each column and its neighbours as they are now (a note).
fn column_notes(w: &mut Worker, sb: u32, cols: &[Column], t: u32, log: &mut RunLog) -> Result<()> {
    for col in cols.iter().take(2) {
        let p = col.pos;
        let now = w.get_region(sb, [p[0] - 1, p[1] - 1, p[2] - 1], [p[0] + 1, p[1] + 2, p[2] + 1])?;
        let mut cells: Vec<String> = Vec::new();
        for (q, s) in now.iter() {
            if !mctas::blockstate::is_air(s) {
                cells.push(format!("{:?} {}", [q[0] - p[0], q[1] - p[1], q[2] - p[2]], clock::short(s)));
            }
        }
        log.notes.push(format!("after tick {t}: around the crop at {p:?} (relative): {}", cells.join(", ")));
    }
    Ok(())
}

/// One recorded tick: growths, harvests, waits, full-height time.
fn follow(cols: &mut [Column], frame: &mctas::sim::Region, crop: &str, t: u32, log: &mut RunLog) {
    for col in cols.iter_mut() {
        let h = height(frame, col.pos, crop);
        let ages = ages_of(frame, col.pos, h);
        if h != col.h && log.notes.len() < 6 {
            let s: Vec<String> = (0..4).map(|i| frame.get([col.pos[0], col.pos[1] + i - 1, col.pos[2]]).map(|s| clock::short(s).to_string()).unwrap_or_else(|| "-".into())).collect();
            log.notes.push(format!("tick {t}: the crop at {:?} {} -> {} ({})", col.pos, col.h, h, s.join(" / ")));
        }
        if h == 0 && log.lost.is_none() {
            log.lost = Some(format!("tick {t}: the crop at {:?} broke (its bottom block)", col.pos));
        }
        // a growth: a standing block's age dropped (the grower restarts at 0), seen even when the
        // new block is destroyed within the same tick (sugar cane); bamboo: the height grew
        if grows_by_age(crop) {
            for j in 0..(h.min(col.h) as usize) {
                if ages[j] < col.ages[j] {
                    log.grown += 1;
                }
            }
        } else if h > col.h && col.h > 0 {
            log.grown += (h - col.h) as u64;
        }
        if h > col.h {
            for _ in col.h..h {
                col.grew.push(t);
            }
            log.height_max = log.height_max.max(h);
        } else if h < col.h {
            let gone = col.h - h.max(1);
            for _ in 0..gone {
                if let Some(g) = col.grew.pop() {
                    log.wait_max = log.wait_max.max(t - g);
                }
            }
            // the blocks below the harvested ones keep their growth ticks
            col.grew.truncate(h.saturating_sub(1) as usize);
        }
        if stopped(frame, col.pos, h, crop) {
            let since = *col.full_since.get_or_insert(t);
            log.full_max = log.full_max.max(t - since);
            // (a missed harvest is a crop still stopped at the END of the run, `end_stuck`: a wait
            // during the run, e.g. a stem waiting for its neighbour's next harvest or a clocked
            // harvester's period, costs rate, which the farm's efficiency already counts;
            // coordinator 2026-10-21)
            if t - since > STUCK && log.waited.is_none() {
                log.waited = Some(format!("the crop at {:?} waited over {STUCK} gt {h} high from tick {since}", col.pos));
            }
        } else {
            col.full_since = None;
        }
        col.h = h;
        col.ages = ages;
    }
    // harvested: every growth minus what still stands above the start
    let standing: i64 = cols.iter().map(|c| c.h as i64 - c.h0 as i64).sum();
    log.harvested = (log.grown as i64 - standing).max(0) as u64;
}

/// The YIELD rule's floor (coordinator 2026-10-21, vjeux: "these are the slowest farms I've ever
/// seen"): a harvester must deliver at least 90% of its plants' free growth, less 2 spreads of
/// chance (the count of a run is random: a free plant's own count varies by about one spread).
pub const YIELD_SHARE: f64 = 0.9;
pub const YIELD_SPREADS: f64 = 2.0;

/// The yield floor of a run: harvests under it fail.
pub fn yield_floor(log: &RunLog) -> f64 {
    (YIELD_SHARE * log.expected - YIELD_SPREADS * log.spread).max(0.0)
}

/// The growths a FREE plant gives (nothing in its way, harvested the moment it grows) during
/// `ticks` gt at random_tick_speed `rts`: the mean count of a control plant in the same sandbox
/// under the same random ticks, and its spread (26.3 source). A block gets a random tick with odds
/// rts / 4096 each gt. Sugar cane and cactus: a growth every 16 random ticks of the top block (age
/// 0..15; a cane ages only with air above it). Bamboo: 1 in 3 of the top's random ticks. Kelp:
/// 0.14 of them. A pumpkin or melon stem alone on its farmland with its own fruit cell: a roll of 1
/// in 13 per random tick (CropBlock.getGrowthSpeed 2: dry farmland under it, no farmland or stem
/// around), 7 rolls to grow up, then 1 roll in 4 picks the fruit cell's side. A stem whose speed
/// a neighbouring stem halves, or that shares its fruit cell, yields less than this.
pub fn free_growth(crop: &str, ticks: u32, rts: u32, _around: &mctas::sim::Region, _p: Pos) -> (f64, f64) {
    let n = ticks as f64 * rts as f64 / 4096.0;
    if crop.ends_with("sugar_cane") || crop.ends_with("cactus") {
        return (n / 16.0, n.sqrt() / 16.0);
    }
    if crop.ends_with("bamboo") {
        let e = n / 3.0;
        return (e, e.sqrt());
    }
    if crop.ends_with("kelp") {
        let e = n * 0.14;
        return (e, e.sqrt());
    }
    if is_stem_crop(crop) {
        let roll = 1.0 / 13.0;
        let e = (n - 7.0 / roll).max(0.0) * roll / 4.0;
        return (e, e.sqrt());
    }
    (0.0, 0.0)
}

/// The end drain (coordinator 2026-10-21): after growth stops, the collection runs until nothing
/// changes, up to DRAIN x [`REST`] gt (the run's length), so items backed up in hoppers, carts and
/// the chest path at the judge's 100x random-tick speed (a rate no vanilla farm reaches) still
/// arrive; an item on the ground or lost still fails.
pub const DRAIN: u32 = RUN / REST;

/// Rest until nothing moves (up to [`DRAIN`] x [`REST`]), then count what is where. A harvester
/// rests when its ITEMS have settled (clk-rt, 2026-10-21): no crop or intruder item moving and the
/// crop and intruder counts in every container unchanged over one REST window. Blocks may keep
/// moving: a clocked harvester's clock is part of the farm (noted, `log.clocked`).
#[allow(clippy::too_many_arguments)]
fn finish(w: &mut Worker, sb: u32, c: &Circuit, crop: &str, from: Pos, to: Pos, judge: &[Pos], log: &mut RunLog) -> Result<()> {
    // a crop item resting on a living crop (on a bamboo stump) is not lost: the crop's next growth
    // pushes it out of the new block. So growth resumes, at the run's speed, while any crop item
    // lies on a crop, up to REGROW x 300 / rts gt (w86's fuel-farm brief, 2026-10-22: the obvious
    // bamboo farm leaves an item on a stump at the end in about half its builds)
    let cap = REGROW * RTS / log.rts.max(1);
    let mut resumed = 0u32;
    loop {
        let mut on_crop = false;
        for e in w.entities(sb)? {
            if e.design || shulker::stack_of_entity(&e.state).is_none_or(|s| s.id != crop_item(crop)) {
                continue;
            }
            let under = [e.pos[0].floor() as i32, (e.pos[1] - 0.01).floor() as i32, e.pos[2].floor() as i32];
            if w.get_region(sb, under, under)?.get(under).is_some_and(|s| is_crop_block(s, crop)) {
                on_crop = true;
                break;
            }
        }
        if !on_crop || resumed >= cap {
            break;
        }
        if resumed == 0 {
            w.exec(sb, &format!("gamerule random_tick_speed {}", log.rts))?;
        }
        w.step(sb, BATCH)?;
        resumed += BATCH;
    }
    if resumed > 0 {
        w.exec(sb, "gamerule random_tick_speed 0")?;
        log.regrown = resumed;
    }
    let mut a = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
    let mut b = a.clone();
    let mut status = w.status(sb)?;
    let farm_items = |bes: &[mctas::sim::BlockEntityInfo]| -> Vec<(Pos, u64)> {
        // only the block entities holding farm items (a moving piston's block entity moves with
        // its machine: a clocked farm's would never compare equal)
        bes.iter()
            .map(|be| (be.pos, stacks_of_be(&be.nbt).iter().filter(|s| s.id == crop_item(crop) || INTRUDERS.iter().any(|(id, _)| *id == s.id)).map(|s| s.count as u64).sum::<u64>()))
            .filter(|(_, n)| *n > 0)
            .collect()
    };
    let mut settled = false;
    let mut last_mover: Option<String> = None;
    for _ in 0..DRAIN {
        w.step(sb, REST)?;
        b = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
        status = w.status(sb)?;
        if a == b && status.block_ticks + status.block_events + status.moving_pistons == 0 {
            break;
        }
        let inside = |p: [f64; 3]| (0..3).all(|i| p[i] >= from[i] as f64 && p[i] <= (to[i] + 1) as f64);
        let mover = w.entities(sb)?.into_iter().find(|e| !e.still && inside(e.pos) && shulker::stack_of_entity(&e.state).is_some_and(|s| s.id == crop_item(crop) || INTRUDERS.iter().any(|(id, _)| *id == s.id)));
        let moving = mover.is_some();
        last_mover = mover.map(|e| format!("{} at {:.2?} (motion {:.2?})", shulker::stack_of_entity(&e.state).map(|s| clock::short(&s.id).to_string()).unwrap_or_default(), e.pos, e.motion));
        if !moving && farm_items(&a.1) == farm_items(&b.1) {
            settled = true;
            break;
        }
        a = b.clone();
    }
    if settled {
        let p = b.0.iter().find(|(p, s)| a.0.get(*p) != Some(s)).map(|(p, s)| format!("{} at {p:?}", clock::short(s))).unwrap_or_else(|| "scheduled ticks or a container's own items".into());
        log.clocked = Some(format!("the farm's items have settled but its blocks keep moving ({p}): it does not rest"));
    } else if a.0 != b.0 {
        let p = b.0.iter().find(|(p, s)| a.0.get(*p) != Some(s)).map(|(p, s)| format!("{} at {p:?}", clock::short(s))).unwrap_or_default();
        log.restless = format!("blocks still change {} gt after growth stopped ({p})", DRAIN * REST);
    } else if a.1 != b.1 {
        let p = b.1.iter().find(|x| !a.1.contains(x)).map(|x| format!("{} at {:?}", clock::short(&x.kind), x.pos)).unwrap_or_default();
        log.restless = format!("container contents still change {} gt after growth stopped ({p})", DRAIN * REST);
    } else if status.block_ticks + status.block_events + status.moving_pistons > 0 {
        log.restless = format!("{} scheduled ticks, {} block events, {} moving pistons pending {} gt after growth stopped{}", status.block_ticks, status.block_events, status.moving_pistons, DRAIN * REST, last_mover.as_ref().map(|m| format!(", and an item still moves: {m}")).unwrap_or_default());
    }
    for be in &b.1 {
        if judge.contains(&be.pos) {
            log.delivered += stacks_of_be(&be.nbt).iter().filter(|s| s.id == crop_item(crop)).map(|s| s.count as u64).sum::<u64>();
        }
    }
    log.final_held = held(w, sb, from, to, judge)?;
    let had = log.initial.get(crop_item(crop)).copied().unwrap_or(0) as i64;
    log.held = log.final_held.get(crop_item(crop)).copied().unwrap_or(0) as i64 - had;
    for e in w.entities(sb)? {
        // the intruders (design entities since they are summoned by the bridge) may end anywhere
        if let Some(s) = shulker::stack_of_entity(&e.state)
            && INTRUDERS.iter().any(|(id, _)| *id == s.id)
        {
            log.intruders_loose += s.count as u64;
            continue;
        }
        if e.design {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            Some(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos)),
            None => log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos)),
        }
    }
    for be in &b.1 {
        if judge.contains(&be.pos) {
            log.intruders_delivered += stacks_of_be(&be.nbt).iter().filter(|s| INTRUDERS.iter().any(|(id, _)| *id == s.id)).map(|s| s.count as u64).sum::<u64>();
        }
    }
    let _ = c;
    Ok(())
}

/// Items per hour per cane at vanilla speed.
pub fn per_hour(log: &RunLog) -> f64 {
    if log.canes == 0 || log.ticks == 0 {
        return 0.0;
    }
    (log.delivered as f64 + log.held.max(0) as f64) / log.canes as f64 / (log.ticks as f64 / HOUR) * VANILLA_RTS as f64 / log.rts.max(1) as f64
}

/// The growths the judge's speed-up cost the run: the random ticks that landed while a piston
/// stood in a plant's next cell ([`RunLog::blocked`], each tick a random tick with odds rts / 4096),
/// times the crop's growth per random tick (sugar cane and cactus 1/16, bamboo 1/3, kelp 0.14).
pub fn speedup_items(log: &RunLog, crop: &str) -> f64 {
    // at vanilla speed the loss is the farm's own
    if log.rts <= VANILLA_RTS {
        return 0.0;
    }
    let per_tick = if crop.ends_with("sugar_cane") || crop.ends_with("cactus") {
        1.0 / 16.0
    } else if crop.ends_with("bamboo") {
        1.0 / 3.0
    } else if crop.ends_with("kelp") {
        0.14
    } else {
        0.0
    };
    log.blocked as f64 * log.rts as f64 / 4096.0 * per_tick
}

/// Items per hour per plant at vanilla speed, the speed-up's transient loss added back (the rate
/// the farm metrics use; coordinator 2026-10-21: a loss that exists only at the judge's 100x speed
/// is not the farm's).
pub fn per_hour_vanilla(log: &RunLog, crop: &str) -> f64 {
    if log.canes == 0 || log.ticks == 0 {
        return 0.0;
    }
    (log.delivered as f64 + log.held.max(0) as f64 + speedup_items(log, crop)) / log.canes as f64 / (log.ticks as f64 / HOUR) * VANILLA_RTS as f64 / log.rts.max(1) as f64
}

/// The crop a harvester design names (`minecraft:sugar_cane`...), empty for another kind.
fn crop_of(c: &Circuit) -> String {
    match &c.kind {
        Some(Kind::Harvester(crop)) => crop.clone(),
        _ => String::new(),
    }
}

/// Judge a run: the outcome, its metrics and a table.
pub fn judge(c: &Circuit, log: &RunLog) -> Outcome {
    let blocks = c.blocks();
    let crop = crop_of(c);
    let what = if is_stem_crop(&crop) { "fruits" } else { "blocks" };
    let gave = if log.drops_seen > 0 { format!(" ({} drops seen, {} items)", log.drops_seen, log.drop_items) } else { String::new() };
    let mut table = vec![
        format!("  {} canes, {} ticks at random_tick_speed {}: {} {what} grew, {} harvested{gave}, {} items in the judge's chests", log.canes, log.ticks, log.rts, log.grown, log.harvested, log.delivered),
        format!("  tallest cane {}, longest wait from growing to harvest {} gt, longest at full height {} gt", log.height_max, log.wait_max, log.full_max),
        format!("  intruders ({} dropped on {} cells): {} in the judge's chests, {} loose", log.drop_cells as u64 * INTRUDERS.iter().map(|x| x.1 as u64).sum::<u64>(), log.drop_cells, log.intruders_delivered, log.intruders_loose),
    ];
    for n in &log.notes {
        table.push(format!("  note: {n}"));
    }
    for n in &log.stranded {
        table.push(format!("  stranded: {n}"));
    }
    for n in &log.trails {
        table.push(format!("  trail: {n}"));
    }
    if let Some(n) = &log.waited {
        table.push(format!("  note: {n} (a wait costs rate only; a missed harvest is a crop still standing at the end)"));
    }
    if log.regrown > 0 {
        table.push(format!("  regrown: growth resumed {} gt after the run while a crop item lay on a crop (its next growth frees it)", log.regrown));
    }
    for (_, n) in &log.left {
        table.push(format!("  left: {n}"));
    }
    table.push(match &log.clocked {
        Some(n) => format!("  rests: no ({n})"),
        None => "  rests: yes".to_string(),
    });
    for n in &log.escaped {
        table.push(format!("  escaped: {n}"));
    }
    let mut why: Vec<String> = Vec::new();
    if let Some(l) = &log.lost {
        why.push(l.clone());
    }
    if let Some(s) = &log.stuck {
        why.push(format!("a missed harvest: {s}"));
    }
    if log.harvested == 0 {
        why.push("nothing was harvested".into());
    }
    // what the harvests gave: a block's item each, or (stem crops) the drop stacks seen tick by tick:
    // one per fruit, so more drops than fruits seen grow means fruits broken within the tick they
    // grew, and fewer means drops taken in the tick they appeared (not countable)
    let gave = if is_stem_crop(&crop) || (counts_drops(&crop) && log.drops_seen > log.harvested) {
        if log.drops_seen < log.harvested {
            why.push(format!("{} {what} were harvested but {} drops were seen: a drop taken in the tick it appeared (a container reaching the crop's cell) cannot be counted, so the loss check would not be exact", log.harvested, log.drops_seen));
        } else if log.drops_seen > log.harvested {
            table.push(format!("  note: {} {what} were broken within the tick they grew (their drops are counted)", log.drops_seen - log.harvested));
        }
        // one item per drop (bamboo, pumpkin): count the drops, since a drop that absorbs an
        // earlier one in the tick it appears is first seen holding both (an item counted twice)
        if crop_item(&crop) == crop { log.drops_seen } else { log.drop_items }
    } else {
        log.harvested
    };
    // the collection's backlog counts as collected (coordinator 2026-10-21: at the judge's 100x
    // speed items arrive 100 times faster than in a vanilla farm, so hopper lines and carts back up
    // in a way no player's farm does): the crop items in the design's own containers at the end
    let missing = gave as i64 - log.delivered as i64 - log.held;
    // the yield (informational since the coordinator's 2026-10-21 19:05 ruling: the farm's
    // EFFICIENCY metric, items per hour per block, ranks slow designs low): the harvests against
    // the plants' free growth
    let floor = yield_floor(log);
    // (stems: no single reference, since a stem's rate follows the farmland around it and its
    // free sides, which the design chooses)
    if log.expected > 0.0 && !is_stem_crop(&crop) {
        table.push(format!("  yield: {} harvested of about {:.0} from free growth ({:.0}%)", log.harvested, log.expected, 100.0 * log.harvested as f64 / log.expected));
    }
    let _ = floor;
    if missing > 0 {
        why.push(format!(
            "{} harvested items are neither in the judge's chests nor in the design's containers ({} loose){}",
            missing,
            log.loose.len(),
            log.left.first().map(|(_, n)| format!("; left: {}", n.split("; before:").next().unwrap_or(n))).unwrap_or_default()
        ));
    } else if missing < 0 {
        // more items collected than harvests seen: a block broken within the tick it grew (the
        // judge sees the column at the same height before and after); nothing is lost
        table.push(format!("  note: {} more items collected than harvests seen (blocks broken within the tick they grew)", -missing));
    }
    if log.held > 0 {
        table.push(format!("  backlog: {} items still in the design's hoppers and carts at the end (the judge's speed-up outruns the collection; counted as collected)", log.held));
    }
    let other: Vec<&String> = log.loose.iter().collect();
    if !other.is_empty() {
        why.push(format!("left loose: {}", other.iter().take(4).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
    }
    if !log.restless.is_empty() {
        why.push(log.restless.clone());
    }
    let per_cane = blocks as f64 / log.canes.max(1) as f64;
    let per_cane_text = if (blocks * 10) % log.canes.max(1) == 0 { format!("{per_cane:.1}") } else { format!("{per_cane:.2}") };
    // EFFICIENCY (vjeux 2026-10-21: "the efficiency of the farm is number of blocks placed compared
    // to number of items gathered per hour"): the items per hour at vanilla speed of all its plants
    // per block of the design (every block: soil, water, carts; a tileable design: one slice); the
    // speed-up's transient loss added back (per_hour_vanilla)
    let rate = per_hour_vanilla(log, &crop);
    if log.blocked > 0 {
        table.push(format!(
            "  speed-up: a piston stood {} gt in a plant's next cell during growth, about {:.1} growths lost at rts {} (negligible at vanilla speed, added back: {:.2} items per hour per plant measured, {rate:.2} counted)",
            log.blocked,
            speedup_items(log, &crop),
            log.rts,
            per_hour(log)
        ));
    }
    let efficiency = rate * log.canes as f64 / blocks.max(1) as f64;
    let mut metrics = vec![
        ("items per hour per block".to_string(), format!("{efficiency:.3}")),
        ("items per hour".to_string(), format!("{:.1}", rate * log.canes as f64)),
        ("per cane".to_string(), format!("{per_cane_text} blocks")),
        ("items per hour per cane".to_string(), format!("{rate:.2}")),
        ("harvest height".to_string(), log.height_max.to_string()),
        ("wait".to_string(), format!("{} gt", log.wait_max)),
    ];
    // FUEL (w86's fuel-farm brief, approved 2026-10-22): a crop whose item burns in a furnace also
    // shows what its fuel smelts at vanilla speed (an item smelted per 200 gt of burn time)
    if let Some(burn) = fuel_burn(crop_item(&crop)) {
        let f = burn as f64 / 200.0;
        metrics.push(("items smelted per hour per block".to_string(), format!("{:.3}", efficiency * f)));
        metrics.push(("items smelted per hour".to_string(), format!("{:.1}", rate * log.canes as f64 * f)));
    }
    // a stem's fruit grows beside it: no harvest height
    if is_stem_crop(&crop_of(c)) {
        metrics.retain(|m| m.0 != "harvest height");
    }
    if why.is_empty() {
        Outcome { pass: true, why: String::new(), metrics, table }
    } else {
        Outcome { pass: false, why: why.join("; "), metrics, table }
    }
}

/// The judge for the harvester kind (`mctas circuit verify`): the run at N positions x 4
/// rotations, then (a `tile:` header) the copies side by side at 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    if !matches!(kind, Kind::Harvester(_)) {
        bail!("{}: not a harvester", kind.name());
    }
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    // the vanilla-speed twin run first: the rates, and how fast the robustness runs may go
    let vanilla = vanilla_run(c, o.margin);
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let rts = game_rts(&rc, c, &vanilla);
        let r = game_run(w, &rc, rts, game_ticks(&rc, rts), o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&rc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(0);
    schedule.ticks = RUN;
    let rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind.clone()), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    let tiled = if c.tile.is_some() { Some(tiled_run(pool, c, o, &vanilla)) } else { None };
    Ok(aggregate(c, rep, tiled, &vanilla))
}

/// A passing harvester verdict's card (the game judge and its redsim twin alike): the builds'
/// mean rate, worst wait and tallest harvest on the base build, its bill, and the tiled run's
/// line (a failing tiled run fails the verdict).
fn aggregate(c: &Circuit, mut rep: VerifyReport, tiled: Option<std::result::Result<String, String>>, vanilla: &VanillaRun) -> VerifyReport {
    // the builds' mean rate and worst wait
    let metric = |k: &BuildOk, name: &str| k.outcome.metrics.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok());
    let oks: Vec<&BuildOk> = rep.builds.iter().filter_map(|b| b.result.as_ref().ok()).filter(|k| k.outcome.pass).collect();
    let rates: Vec<f64> = oks.iter().filter_map(|k| metric(k, "items per hour per cane")).collect();
    let mean = if rates.is_empty() { 0.0 } else { rates.iter().sum::<f64>() / rates.len() as f64 };
    // EFFICIENCY at vanilla speed (coordinator 2026-10-21): the rates come from one long run of the
    // redsim twin at random_tick_speed 3 (VANILLA_RUN gt, about 10 game hours), free of every
    // speed-up artifact; the game builds above judge robustness. A design the twin does not model,
    // or whose twin run fails, keeps the game builds' rates (noted).
    let (mean, rate_note) = match vanilla {
        Ok(Ok(log)) => {
            let out = judge(c, log);
            if out.pass {
                (per_hour_vanilla(log, &crop_of(c)), format!("rates: the redsim twin at random_tick_speed 3 for {} gt ({} harvested, {} collected; robustness runs at random_tick_speed {})", VANILLA_RUN, log.harvested, log.delivered + log.held.max(0) as u64, game_rts(c, c, vanilla)))
            } else {
                (mean, format!("rates: from the game runs at random_tick_speed {} (the vanilla-speed twin run fails: {})", rts_for(c), out.why))
            }
        }
        Ok(Err(why)) | Err(why) => (mean, format!("rates: from the game runs at random_tick_speed {} (the twin does not model it: {why})", rts_for(c))),
    };
    let worst = oks.iter().filter_map(|k| metric(k, "wait")).fold(0.0f64, f64::max) as u32;
    let tallest = oks.iter().filter_map(|k| metric(k, "harvest height")).fold(0.0f64, f64::max) as u32;
    let mut line = String::new();
    if let Some(t) = tiled {
        line = match t {
            Ok(per) => format!("tileable: yes ({per})"),
            Err(why) => {
                rep.pass = false;
                rep.verdict = format!("FAIL: tiled: {why}");
                format!("tileable: no ({why})")
            }
        };
    }
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        // the rate and the wait follow the random growth: the builds' mean rate, the worst wait
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "items per hour per cane") {
            m.1 = format!("{mean:.2}");
        }
        // the efficiency and the farm's rate from the builds' mean rate
        let plants = c.stores.iter().filter(|s| s.role == "crop").count().max(1);
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "items per hour per block") {
            m.1 = format!("{:.3}", mean * plants as f64 / c.blocks().max(1) as f64);
        }
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "items per hour") {
            m.1 = format!("{:.1}", mean * plants as f64);
        }
        // (fuel) the items its fuel smelts, from the same mean rate
        if let Some(burn) = fuel_burn(crop_item(&crop_of(c))) {
            let f = burn as f64 / 200.0;
            if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "items smelted per hour per block") {
                m.1 = format!("{:.3}", mean * plants as f64 / c.blocks().max(1) as f64 * f);
            }
            if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "items smelted per hour") {
                m.1 = format!("{:.1}", mean * plants as f64 * f);
            }
        }
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "wait") {
            m.1 = format!("{worst} gt");
        }
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "harvest height") {
            m.1 = tallest.to_string();
        }
        // the random_tick_speed its robustness runs used ([`game_rts`]: `rts_for`, no faster than
        // its collection carries, from the vanilla twin run): the site's viewer plays a big farm at
        // it (`data-farm-rts`), read from here instead of a twin run of its own at every deploy
        // (coordinator 2026-10-23); `archive --fill-game-rts` writes it into older verdicts
        k.outcome.metrics.push(("game rts".to_string(), game_rts(c, c, vanilla).to_string()));
        k.outcome.metrics.push(("bill".to_string(), shulker::bill(c)));
        k.outcome.table.push(format!("  {rate_note}"));
        if !line.is_empty() {
            k.outcome.table.push(format!("  {line}"));
        }
    }
    rep
}

/// The copies side by side ([`crate::tiled`]) at 4 rotations: Ok(the copies' per-hour line) or
/// Err(why) at the first failure.
fn tiled_run(pool: &Pool, c: &Circuit, o: &Options, vanilla: &VanillaRun) -> std::result::Result<String, String> {
    let tc = crate::tiled(c).map_err(|e| format!("{e:#}"))?;
    let jobs: Vec<u8> = vec![0, 1, 2, 3];
    let results = pool.map(jobs.clone(), |w, _, t| {
        let rc = tc.rotated(w, t)?;
        let rts = game_rts(&rc, c, vanilla);
        Ok((game_run(w, &rc, rts, game_ticks(&rc, rts), o.margin)?, rc))
    });
    let mut line = String::new();
    for (t, r) in jobs.into_iter().zip(results) {
        match r {
            Ok((Ok(log), rc)) => {
                let out = judge(&rc, &log);
                if !out.pass {
                    let extra: Vec<String> = log.stranded.iter().chain(log.escaped.iter()).take(4).cloned().collect();
                    return Err(format!("rotation {t}: {}{}", out.why, if extra.is_empty() { String::new() } else { format!(" (stranded: {})", extra.join("; ")) }));
                }
                if t == 0 {
                    line = format!("{} copies, {:.2} items per hour per cane", log.canes, per_hour(&log));
                }
            }
            Ok((Err(why), _)) => return Err(format!("rotation {t}: {why}")),
            Err(e) => return Err(format!("rotation {t}: {e:#}")),
        }
    }
    Ok(line)
}

/// A design aid (`mctas circuit farm-run`): one run (`--tiled`: the copies) turned and moved like
/// one of `circuit verify`'s builds, its table printed.
#[allow(clippy::too_many_arguments)]
pub fn run_one(pool: &Pool, c: &Circuit, tiled: bool, turns: u8, offset: Pos, rts: u32, ticks: u32, margin: i32) -> Result<Outcome> {
    let base = if tiled { crate::tiled(c)? } else { c.clone() };
    let r = pool.map(vec![0u8], |w, _, _| {
        let rc = base.rotated(w, turns)?.translated(offset)?;
        Ok((game_run(w, &rc, rts, ticks, margin)?, rc))
    });
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        (Ok(log), rc) => Ok(judge(&rc, &log)),
        (Err(why), _) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// The intruders of drop round `k` (`INTRUDERS[k]` on every drop cell), as the bridge summons them.
fn intruder_specs(k: usize, cells: &[[f64; 3]]) -> Vec<mctas::design::EntitySpec> {
    let (id, n) = INTRUDERS[k];
    cells.iter().map(|q| mctas::design::EntitySpec { kind: "minecraft:item".into(), pos: *q, nbt: Some(format!("{{Item:{{id:\"{id}\",count:{n}}}}}")) }).collect()
}

/// SNBT in a canonical text: compound keys sorted, numbers by value (`90f` = `90.0f`).
fn snbt_canon(s: &str) -> String {
    use redsim::redstone::container::{Snbt, parse_snbt};
    fn walk(v: &Snbt) -> String {
        match v {
            Snbt::Compound(kv) => {
                let mut parts: Vec<String> = kv.iter().map(|(k, v)| format!("{k}:{}", walk(v))).collect();
                parts.sort();
                format!("{{{}}}", parts.join(","))
            }
            Snbt::List(l) => format!("[{}]", l.iter().map(walk).collect::<Vec<_>>().join(",")),
            Snbt::Str(s) => format!("{s:?}"),
            Snbt::Int(i) => i.to_string(),
            Snbt::Word(w) => match w.trim_end_matches(['f', 'F', 'd', 'D']).parse::<f64>() {
                Ok(x) => format!("{x:?}"),
                Err(_) => w.clone(),
            },
        }
    }
    match parse_snbt(s) {
        Ok(v) => walk(&v),
        Err(_) => s.to_string(),
    }
}

/// How a design turned without the game (`Circuit::turned_pure`) differs from the game's turn
/// (`Circuit::rotated`): the first difference in blocks, ports, block entities, entities (kind,
/// position, data by value) or header, None when they agree.
pub fn turn_difference(game: &Circuit, pure: &Circuit) -> Option<String> {
    let (g, p) = (&game.clock.design, &pure.clock.design);
    if let Some((pos, s)) = g.blocks.iter().find(|(pos, s)| p.blocks.get(*pos) != Some(*s)) {
        return Some(format!("block {pos:?}: game {s}, pure {}", p.blocks.get(pos).map(String::as_str).unwrap_or("(none)")));
    }
    if g.blocks.len() != p.blocks.len() {
        return Some(format!("{} blocks in the game's turn, {} in the pure one", g.blocks.len(), p.blocks.len()));
    }
    if g.ports != p.ports {
        return Some(format!("ports: game {:?}, pure {:?}", g.ports, p.ports));
    }
    if g.block_entities != p.block_entities {
        return Some("block entities differ".into());
    }
    if g.entities.len() != p.entities.len() {
        return Some(format!("{} entities in the game's turn, {} in the pure one", g.entities.len(), p.entities.len()));
    }
    for (a, b) in g.entities.iter().zip(&p.entities) {
        let (na, nb) = (a.nbt.as_deref().map(snbt_canon), b.nbt.as_deref().map(snbt_canon));
        if a.kind != b.kind || a.pos != b.pos || na != nb {
            return Some(format!("entity: game {} {:?} {:?}, pure {} {:?} {:?}", a.kind, a.pos, na, b.kind, b.pos, nb));
        }
    }
    if g.meta != p.meta {
        return Some(format!("header: game {:?}, pure {:?}", g.meta, p.meta));
    }
    None
}

// ------------------------------------------------------------------------ the redsim twin

/// The farm judge's run on redsim (clk-misc, 2026-10-21; coordinator: "a redsim twin for the farm
/// judge so w86's searches can run in redsim"): [`game_run`] step for step on redsim's world
/// (the same box, seed and chunks; the judge's placement, the chests placed raw, the crop planted
/// with updates, the settle, `random_tick_speed` set as the judge sets it, the intruders summoned
/// as the bridge summons them, the batches, the rest), the game's random ticks mirrored draw for
/// draw (`redsim::crops`), so a build's twin run IS the game run of that build and seed
/// (`redsim-fuzz trace-compare` on `trace-export --farm` traces). The same [`RunLog`], judged
/// by [`judge`]. Outer Err: what redsim cannot build or run (UNMODELED); inner Err: the judge's
/// refusal of the build (as [`game_run`]'s).
pub fn sim_run(c: &Circuit, rts: u32, ticks: u32, margin: i32) -> std::result::Result<std::result::Result<RunLog, String>, String> {
    sim_run_probe(c, rts, ticks, margin, None)
}

/// A world's state as one line (a probe's tick): the box's blocks and every entity (kind,
/// position, motion), sorted. (Public for the 3D view's `web check`, which holds the viewer's
/// world to the twin's every tick.)
pub fn sim_digest(w: &redsim::World, from: Pos, to: Pos) -> String {
    let blocks: Vec<String> = sim_region(w, from, to).iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(p, s)| format!("{p:?} {s}")).collect();
    let mut ents: Vec<String> = w.entities.list.iter().filter(|e| !e.removed).map(|e| format!("{} {:.6} {:.6} {:.6} m {:.6} {:.6} {:.6}", sim_entity_name(e), e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z)).collect();
    ents.sort();
    let held: Vec<(Pos, Option<i32>, Vec<(usize, String, u32)>)> = w
        .containers
        .map
        .iter()
        .filter(|(p, _)| (0..3).all(|i| [p.x, p.y, p.z][i] >= from[i] && [p.x, p.y, p.z][i] <= to[i]))
        .map(|(p, c)| {
            let slots = c.slots.iter().enumerate().filter_map(|(i, s)| s.map(|s| (i, norm_id(redsim::redstone::container::item_name(s.item)), s.count as u32))).collect();
            ([p.x, p.y, p.z], if c.hopper { Some(c.cooldown) } else { None }, slots)
        })
        .collect();
    let rng = format!("rng {} rv {}", w.level_random.as_ref().map(|r| r.state()).unwrap_or(0), w.random_ticks.as_ref().map(|r| r.value).unwrap_or(0));
    format!("{} | {} | {} | {rng}", blocks.join("; "), ents.join("; "), probe_containers(held))
}

/// The probe's container part (`--probe`): every block container with items or a hopper cooldown,
/// by position: a hopper's `TransferCooldown`, then every slot (index, item, count), so the first
/// tick a hopper's contents or its timing differ shows.
fn probe_containers(held: Vec<(Pos, Option<i32>, Vec<(usize, String, u32)>)>) -> String {
    let mut v: Vec<String> = held
        .into_iter()
        .filter(|(_, cd, s)| !s.is_empty() || cd.is_some_and(|c| c > 0))
        .map(|(p, cd, slots)| {
            let cd = cd.filter(|c| *c > 0).map(|c| format!(" cd {c}")).unwrap_or_default();
            format!("{p:?}{cd} {}", slots.iter().map(|(i, id, n)| format!("{i}:{} x{n}", clock::short(id))).collect::<Vec<_>>().join(" "))
        })
        .collect();
    v.sort();
    v.join("; ")
}

/// [`sim_run`], every tick's [`sim_digest`] pushed to `probe` (from the run's start: after the
/// placement, the planting and the settle).
pub fn sim_run_probe(c: &Circuit, rts: u32, ticks: u32, margin: i32, mut probe: Option<&mut Vec<String>>) -> std::result::Result<std::result::Result<RunLog, String>, String> {
    use redsim::world::flags;
    use redsim::{Block, BlockPos, World};
    let Some(Kind::Harvester(crop)) = &c.kind else { return Err(format!("{}: not a harvester", c.name())) };
    let crop = crop.clone();
    let bp = |p: Pos| BlockPos::new(p[0], p[1], p[2]);
    let (from, to, seed) = judge_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let mut w = World::new(bp(from), [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1]);
    w.mirror_level_random(seed, chunks);
    w.drop_seed = Some(seed);
    w.random_ticks = Some(redsim::crops::RandomTicks::new(3, seed));
    let parse = |s: &str| Block::parse(s).map_err(|e| format!("not modeled by redsim: {e}"));
    // the judge's placement (clock::verify::place: raw writes, the block entities' data, the
    // wires' shapes and a self update of every block), then the design's entities
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for (p, s) in c.clock.design.block_list() {
        blocks.push((bp(p), parse(&s)?));
    }
    w.place_raw(&blocks);
    let be: Vec<(BlockPos, &str)> = c.clock.design.block_entities.iter().map(|(p, s)| (bp(*p), s.as_str())).collect();
    redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| format!("nbt: {e}"))?;
    w.place_updates(&blocks);
    for e in &c.clock.design.entities {
        redsim::entity::spawn(&mut w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| format!("not modeled by redsim: {why}"))?;
    }
    let popped = c.clock.design.blocks.keys().find(|p| w.get(bp(**p)).is_air());
    let floating = blocks.iter().find(|(p, s)| !w.get(*p).is_air() && !redsim::behavior::can_survive(&w, s, *p)).map(|(p, _)| [p.x, p.y, p.z]);
    if let Some(p) = popped.copied().or(floating) {
        let st = c.clock.design.blocks.get(&p).cloned().unwrap_or_default();
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let crops: Vec<Pos> = c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos).collect();
    let judge_cells: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let chest = parse(CHEST)?;
    let chests: Vec<(BlockPos, Block)> = judge_cells.iter().map(|p| (bp(*p), chest)).collect();
    w.place_raw(&chests);
    let set_rts = |w: &mut World, n: u32| {
        if let Some(rt) = w.random_ticks.as_mut() {
            rt.speed = n;
        }
    };
    set_rts(&mut w, 0);
    let planted = parse(planted_state(&crop))?;
    for p in &crops {
        w.set_block(bp(*p), planted, flags::ALL);
    }
    for _ in 0..c.clock.free_from() + SETTLE {
        w.tick();
    }
    let full = full_height(&crop);
    let rf = [crops.iter().map(|p| p[0]).min().unwrap_or(0), crops.iter().map(|p| p[1]).min().unwrap_or(0), crops.iter().map(|p| p[2]).min().unwrap_or(0)];
    let rt = [crops.iter().map(|p| p[0]).max().unwrap_or(0), crops.iter().map(|p| p[1]).max().unwrap_or(0) + full as i32, crops.iter().map(|p| p[2]).max().unwrap_or(0)];
    let start = sim_region(&w, rf, rt);
    if let Some(p) = crops.iter().find(|p| height(&start, **p, &crop) == 0) {
        return Ok(Err(format!("the crop planted at {p:?} does not survive (sugar cane needs sand, dirt or grass below it with water beside that block)")));
    }
    let mut cols: Vec<Column> = crops
        .iter()
        .map(|p| {
            let h = height(&start, *p, &crop);
            Column { pos: *p, h, ages: ages_of(&start, *p, h), h0: h, grew: Vec::new(), full_since: None }
        })
        .collect();
    let mut log = RunLog { canes: crops.len(), ticks, rts, dims: shulker::design_dims(c), initial: sim_held(&w, from, to, &judge_cells), ..Default::default() };
    // the YIELD rule's expectation, as in the game run
    for p in &crops {
        let around = sim_region(&w, [p[0] - 2, p[1] - 1, p[2] - 2], [p[0] + 2, p[1], p[2] + 2]);
        let (e, d) = free_growth(&crop, ticks, rts, &around, *p);
        log.expected += e;
        log.spread = (log.spread * log.spread + d * d).sqrt();
    }
    for col in &cols {
        if col.h > 1 {
            log.notes.push(format!("the crop at {:?} stands {} high before the run", col.pos, col.h));
        }
    }
    set_rts(&mut w, rts);
    let rest_ticks = 2 * REST;
    let intakes = crate::intrusion::intakes(c);
    // (melon, the stems, bamboo: `counts_drops`) the game run steps one tick per batch and sees
    // every drop stack once
    let count_drops = crop_item(&crop) != crop || counts_drops(&crop);
    let mut ever: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    let batch = if count_drops { 1 } else { BATCH };
    let drop_at: Vec<u32> = (1..=3).map(|k| (ticks * k / 4 / batch) * batch).collect();
    let drop_cells: Vec<[f64; 3]> = drop_cells_of(&intakes, &crops, full);
    log.drop_cells = drop_cells.len();
    // the bridge's entity watch: the first entity (in the level's order) whose box leaves the
    // build's box, its tick within the batch
    let inside = |b: &redsim::entity::Aabb| b.min_x >= from[0] as f64 && b.min_y >= from[1] as f64 && b.min_z >= from[2] as f64 && b.max_x <= (to[0] + 1) as f64 && b.max_y <= (to[1] + 1) as f64 && b.max_z <= (to[2] + 1) as f64;
    let mut t = 0u32;
    let mut seen_items: Vec<u32> = Vec::new();
    // (run_in's) where each crop item was at the end of each batch, inside the box (the `left:`
    // notes' history)
    let mut last_pos: BTreeMap<u32, Vec<([f64; 3], u32)>> = BTreeMap::new();
    if let Some(p) = probe.as_deref_mut() {
        p.push(sim_digest(&w, from, to));
    }
    while t < ticks + rest_ticks {
        if t == ticks {
            set_rts(&mut w, 0);
        }
        if let Some(k) = drop_at.iter().position(|d| *d == t && t > 0) {
            for s in intruder_specs(k, &drop_cells) {
                redsim::entity::spawn(&mut w, &s.kind, s.pos, s.nbt.as_deref()).map_err(|why| format!("an intruder: {why}"))?;
            }
        }
        let n = batch.min(if t < ticks { ticks - t } else { ticks + rest_ticks - t });
        let mut escape: Option<String> = None;
        for k in 0..n {
            w.tick();
            if let Some(e) = w.errors.first() {
                return Err(format!("tick {}: {e}", t + 1));
            }
            t += 1;
            if let Some(p) = probe.as_deref_mut() {
                p.push(sim_digest(&w, from, to));
            }
            if escape.is_none()
                && let Some(e) = w.entities.list.iter().find(|e| !e.removed && !inside(&e.bb))
            {
                escape = Some(format!("tick {}: {} at {:.4} {:.4} {:.4}", k + 1, sim_entity_name(e), e.pos.x, e.pos.y, e.pos.z));
            }
            let region = sim_region(&w, rf, rt);
            follow(&mut cols, &region, &crop, t, &mut log);
            // (the game run's) the speed-up's transient loss: a piston in a plant's next cell
            // during growth
            if t <= ticks && !is_stem_crop(&crop) {
                for col in &cols {
                    let above = [col.pos[0], col.pos[1] + col.h as i32, col.pos[2]];
                    if region.get(above).is_some_and(|s| s.starts_with("minecraft:moving_piston") || s.starts_with("minecraft:piston_head")) {
                        log.blocked += 1;
                    }
                }
            }
        }
        // (the game's: an escape above the build's box is an intruder at its drop point, not logged)
        let above_box = |e: &str| e.split(" at ").nth(1).and_then(|p| p.split_whitespace().nth(1)).and_then(|y| y.parse::<f64>().ok()).is_some_and(|y| y > (to[1] + 1) as f64);
        if let Some(e) = escape
            && log.escaped.len() < 4
            && !above_box(&e)
        {
            log.escaped.push(format!("batch from tick {}: {e}", t - n));
        }
        if log.lost.is_some() {
            sim_column_notes(&w, &cols, t, &mut log);
            break;
        }
        if t == ticks && log.grown == 0 {
            sim_column_notes(&w, &cols, t, &mut log);
        }
        // (the game run's) the judge empties its chests every EMPTY gt, counting what it takes
        // (`data merge block .. {Items:[]}` when it took something: the chest's setChanged)
        if t % EMPTY == 0 {
            for p in &judge_cells {
                let (crops, intruders) = sim_chest_counts(&w, std::slice::from_ref(p), &crop);
                log.delivered += crops;
                log.intruders_delivered += intruders;
                if crops + intruders > 0 {
                    redsim::redstone::container::set_contents(&mut w, bp(*p), &[]);
                }
            }
            if let Some(e) = w.errors.first() {
                return Err(format!("tick {t}: emptying the judge's chests: {e}"));
            }
        }
        // stranded items: a crop item still loose one batch later (a note)
        let mut now_items: Vec<u32> = Vec::new();
        for e in w.entities.list.iter().filter(|e| !e.removed && e.summon_index.is_none()) {
            let redsim::entity::EntityKind::Item(s) = &e.kind else { continue };
            if norm_id(&s.item) != crop_item(&crop) {
                continue;
            }
            // (run_in's `left:` notes) a crop item outside the build's box, below or beside it
            // (one above the box is a tall crop's item still in the air), with where it was at
            // the batches before; then an item about to despawn (its age counts to 6000)
            let pos = [e.pos.x, e.pos.y, e.pos.z];
            let motion = [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z];
            let key = e.id.to_string();
            let out = (0..3).any(|i| pos[i] < from[i] as f64 - 0.5 || (i != 1 && pos[i] > (to[i] + 1) as f64 + 0.5));
            if out && !log.left.iter().any(|(u, _)| *u == key) && log.left.len() < 6 {
                let before = last_pos.get(&e.id).map(|v| v.iter().step_by(4).map(|(p, t0)| format!("t{t0} {p:.2?}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                log.left.push((key.clone(), format!("tick {t}: a crop item left the build at {pos:.2?} (motion {motion:.2?}); before: {before}")));
            }
            if !out {
                let v = last_pos.entry(e.id).or_default();
                v.push((pos, t));
                if v.len() > 80 {
                    v.remove(0);
                }
            }
            if s.age >= 5800 && !log.left.iter().any(|(u, _)| *u == key) && log.left.len() < 6 {
                log.left.push((key.clone(), format!("tick {t}: a crop item about to despawn (age {}) at {pos:.2?} (motion {motion:.2?})", s.age)));
            }
            // (melon) every drop stack's count when first seen: what the harvest gave
            if count_drops && ever.insert(e.id) {
                log.drops_seen += 1;
                log.drop_items += s.count as u64;
            }
            if seen_items.contains(&e.id) && log.stranded.len() < 6 {
                let under = BlockPos::new(e.pos.x.floor() as i32, (e.pos.y - 0.01).floor() as i32, e.pos.z.floor() as i32);
                // (the game reads void_air outside the world's height, y -64..319)
                let what = if under.y < -64 || under.y > 319 { "void_air".to_string() } else { clock::short(&w.get(under).to_string()).to_string() };
                log.stranded.push(format!("tick {t}: an item at {:.2?} on {what} at {:?} (motion {:.2?})", [e.pos.x, e.pos.y, e.pos.z], [under.x, under.y, under.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z]));
            }
            now_items.push(e.id);
        }
        seen_items = now_items;
    }
    set_rts(&mut w, 0);
    end_stuck(&cols, t, &mut log);
    sim_finish(&mut w, &crop, from, to, &judge_cells, &mut log)?;
    Ok(Ok(log))
}

/// A stack's item id with the `minecraft:` prefix.
fn norm_id(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") }
}

/// The type of a redsim entity, as the game names it.
fn sim_entity_name(e: &redsim::entity::Entity) -> &'static str {
    use redsim::entity::EntityKind;
    match &e.kind {
        EntityKind::Boat(b) => {
            if b.chest {
                "minecraft:oak_chest_boat"
            } else {
                "minecraft:oak_boat"
            }
        }
        EntityKind::Minecart(m) => {
            if m.hopper {
                "minecraft:hopper_minecart"
            } else {
                "minecraft:minecart"
            }
        }
        EntityKind::Item(_) => "minecraft:item",
        EntityKind::ArmorStand(_) => "minecraft:armor_stand",
        EntityKind::PrimedTnt(_) => "minecraft:tnt",
        EntityKind::Mob(m) => m.ty.game_id(),
    }
}

/// Redsim's cells `from..=to` as the game's `Region` (the states as the game writes them).
fn sim_region(w: &redsim::World, from: Pos, to: Pos) -> mctas::sim::Region {
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let mut palette: Vec<String> = Vec::new();
    let mut data: Vec<u16> = Vec::with_capacity((size[0] * size[1] * size[2]) as usize);
    for y in from[1]..=to[1] {
        for z in from[2]..=to[2] {
            for x in from[0]..=to[0] {
                let s = w.get(redsim::BlockPos::new(x, y, z)).to_string();
                let i = match palette.iter().position(|p| *p == s) {
                    Some(i) => i,
                    None => {
                        palette.push(s);
                        palette.len() - 1
                    }
                };
                data.push(i as u16);
            }
        }
    }
    mctas::sim::Region { from, size, palette, flags: Vec::new(), data }
}

/// The kinds and counts the design's own block containers hold (judge chests left out).
fn sim_held(w: &redsim::World, from: Pos, to: Pos, judge: &[Pos]) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    for (p, c) in &w.containers.map {
        let q = [p.x, p.y, p.z];
        if judge.contains(&q) || (0..3).any(|i| q[i] < from[i] || q[i] > to[i]) {
            continue;
        }
        for s in c.slots.iter().flatten() {
            *m.entry(norm_id(redsim::redstone::container::item_name(s.item))).or_insert(0) += s.count as u64;
        }
    }
    m
}

/// The crop items and the intruders in the judge's chests.
fn sim_chest_counts(w: &redsim::World, judge: &[Pos], crop: &str) -> (u64, u64) {
    let (mut crops, mut intruders) = (0u64, 0u64);
    for p in judge {
        let Some(c) = w.containers.map.get(&redsim::BlockPos::new(p[0], p[1], p[2])) else { continue };
        for s in c.slots.iter().flatten() {
            let id = norm_id(redsim::redstone::container::item_name(s.item));
            if id == crop_item(crop) {
                crops += s.count as u64;
            } else if INTRUDERS.iter().any(|(i, _)| *i == id) {
                intruders += s.count as u64;
            }
        }
    }
    (crops, intruders)
}

/// [`column_notes`] on redsim.
fn sim_column_notes(w: &redsim::World, cols: &[Column], t: u32, log: &mut RunLog) {
    for col in cols.iter().take(2) {
        let p = col.pos;
        let now = sim_region(w, [p[0] - 1, p[1] - 1, p[2] - 1], [p[0] + 1, p[1] + 2, p[2] + 1]);
        let cells: Vec<String> = now.iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(q, s)| format!("{:?} {}", [q[0] - p[0], q[1] - p[1], q[2] - p[2]], clock::short(s))).collect();
        log.notes.push(format!("after tick {t}: around the crop at {p:?} (relative): {}", cells.join(", ")));
    }
}

/// [`finish`] on redsim: the rest and end drain (up to [`DRAIN`] x REST ticks until nothing
/// changes and nothing is pending), then the counts.
fn sim_finish(w: &mut redsim::World, crop: &str, from: Pos, to: Pos, judge: &[Pos], log: &mut RunLog) -> std::result::Result<(), String> {
    // [`finish`]'s regrowth: growth resumes while a crop item lies on a crop, up to REGROW x 300 / rts
    let cap = REGROW * RTS / log.rts.max(1);
    let mut resumed = 0u32;
    loop {
        let on_crop = w.entities.list.iter().filter(|e| !e.removed && e.summon_index.is_none()).any(|e| {
            let redsim::entity::EntityKind::Item(s) = &e.kind else { return false };
            if norm_id(&s.item) != crop_item(crop) {
                return false;
            }
            let under = redsim::BlockPos::new(e.pos.x.floor() as i32, (e.pos.y - 0.01).floor() as i32, e.pos.z.floor() as i32);
            is_crop_block(&w.get(under).to_string(), crop)
        });
        if !on_crop || resumed >= cap {
            break;
        }
        if let Some(rt) = w.random_ticks.as_mut() {
            rt.speed = log.rts;
        }
        for _ in 0..BATCH {
            w.tick();
        }
        resumed += BATCH;
    }
    if resumed > 0 {
        if let Some(rt) = w.random_ticks.as_mut() {
            rt.speed = 0;
        }
        log.regrown = resumed;
    }
    // the block entities in the region's order (y, z, x), each with its block's name and contents
    // (a hopper's cooldown too: the game compares every block entity's data)
    let containers = |w: &redsim::World| -> Vec<(Pos, String, String)> {
        let mut v: Vec<(Pos, String, String)> = w
            .containers
            .map
            .iter()
            .filter(|(p, _)| (0..3).all(|i| [p.x, p.y, p.z][i] >= from[i] && [p.x, p.y, p.z][i] <= to[i]))
            .map(|(p, c)| {
                let name = w.get(*p).to_string();
                let kind = name.split('[').next().unwrap_or_default().to_string();
                ([p.x, p.y, p.z], kind, format!("{:?} {}", c.slots, if c.hopper { c.cooldown } else { 0 }))
            })
            .collect();
        v.sort_by_key(|(p, _, _)| (p[1], p[2], p[0]));
        v
    };
    // (finish's `farm_items`) every block entity of the region in its order (y, z, x) with the
    // crop and intruder items it holds (0 for a moving block, a comparator...)
    let crop_id = crop_item(crop).to_string();
    let farm_items = |w: &redsim::World| -> Vec<(Pos, u64)> {
        let mut v = Vec::new();
        for y in from[1]..=to[1] {
            for z in from[2]..=to[2] {
                for x in from[0]..=to[0] {
                    let p = redsim::BlockPos::new(x, y, z);
                    if !redsim::behavior::has_block_entity(&w.get(p)) {
                        continue;
                    }
                    let n = w.containers.map.get(&p).map(|c| c.slots.iter().flatten().filter(|s| {
                        let id = norm_id(redsim::redstone::container::item_name(s.item));
                        id == crop_id || INTRUDERS.iter().any(|(i, _)| *i == id)
                    }).map(|s| s.count as u64).sum::<u64>()).unwrap_or(0);
                    // (finish: only those holding farm items; a moving piston's moves with its machine)
                    if n > 0 {
                        v.push(([x, y, z], n));
                    }
                }
            }
        }
        v
    };
    // the bridge's status: scheduled block ticks, block events, moving pistons (no fluid ticks)
    let pending = |w: &redsim::World| (w.ticks.count(), w.events.len(), w.piston.live_count());
    let mut a = (sim_region(w, from, to), containers(w));
    let mut b = a.clone();
    let mut a_items = farm_items(w);
    let mut status = pending(w);
    let mut settled = false;
    for _ in 0..DRAIN {
        for _ in 0..REST - 1 {
            w.tick();
        }
        // the bridge's `still`: where each summoned (design) entity stood before the last tick
        let before: std::collections::HashMap<u32, redsim::entity::Vec3> = w.entities.list.iter().filter(|e| !e.removed && e.summon_index.is_some()).map(|e| (e.id, e.pos)).collect();
        w.tick();
        if let Some(e) = w.errors.first() {
            return Err(format!("the rest: {e}"));
        }
        b = (sim_region(w, from, to), containers(w));
        let b_items = farm_items(w);
        status = pending(w);
        if a == b && status.0 + status.1 + status.2 == 0 {
            break;
        }
        // (finish's) the harvester rests when its items have settled: no crop or intruder item
        // moving inside the judge's box (an escaped intruder falling forever outside does not
        // count; an item the design did not summon is never still to the bridge) and the same
        // crop and intruder counts in every block entity over the window; blocks may keep moving
        let inside = |p: redsim::entity::Vec3| (0..3).all(|i| [p.x, p.y, p.z][i] >= from[i] as f64 && [p.x, p.y, p.z][i] <= (to[i] + 1) as f64);
        let moving = w.entities.list.iter().filter(|e| !e.removed && inside(e.pos)).any(|e| {
            let redsim::entity::EntityKind::Item(s) = &e.kind else { return false };
            let id = norm_id(&s.item);
            if id != crop_id && !INTRUDERS.iter().any(|(i, _)| *i == id) {
                return false;
            }
            let still = before.get(&e.id).is_some_and(|p| *p == e.pos) && e.delta_movement.x == 0.0 && e.delta_movement.z == 0.0 && (e.delta_movement.y == 0.0 || e.on_ground);
            !still
        });
        if !moving && a_items == b_items {
            settled = true;
            break;
        }
        a = b.clone();
        a_items = b_items;
    }
    if settled {
        let p = b.0.iter().find(|(p, s)| a.0.get(*p) != Some(*s)).map(|(p, s)| format!("{} at {p:?}", clock::short(s))).unwrap_or_else(|| "scheduled ticks or a container's own items".into());
        log.clocked = Some(format!("the farm's items have settled but its blocks keep moving ({p}): it does not rest"));
    } else if let Some(p) = b.0.iter().find(|(p, s)| a.0.get(*p) != Some(*s)).map(|(p, s)| format!("{} at {p:?}", clock::short(s))) {
        log.restless = format!("blocks still change {} gt after growth stopped ({p})", DRAIN * REST);
    } else if a.1 != b.1 {
        let p = b.1.iter().find(|x| !a.1.contains(x)).map(|x| format!("{} at {:?}", clock::short(&x.1), x.0)).unwrap_or_default();
        log.restless = format!("container contents still change {} gt after growth stopped ({p})", DRAIN * REST);
    } else if status.0 + status.1 + status.2 > 0 {
        log.restless = format!("{} scheduled ticks, {} block events, {} moving pistons pending {} gt after growth stopped", status.0, status.1, status.2, DRAIN * REST);
    }
    let (delivered, intruders) = sim_chest_counts(w, judge, crop);
    // (the game's) what the run's emptyings took, plus what the chests hold now
    log.delivered += delivered;
    log.intruders_delivered += intruders;
    log.final_held = sim_held(w, from, to, judge);
    let had = log.initial.get(crop_item(crop)).copied().unwrap_or(0) as i64;
    log.held = log.final_held.get(crop_item(crop)).copied().unwrap_or(0) as i64 - had;
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        if let redsim::entity::EntityKind::Item(s) = &e.kind {
            let id = norm_id(&s.item);
            if INTRUDERS.iter().any(|(i, _)| *i == id) {
                log.intruders_loose += s.count as u64;
                continue;
            }
            if e.summon_index.is_none() {
                log.loose.push(format!("{} x{} at {:.1?}", clock::short(&id), s.count, [e.pos.x, e.pos.y, e.pos.z]));
            }
            continue;
        }
        if e.summon_index.is_none() {
            log.loose.push(format!("{} at {:.1?}", clock::short(sim_entity_name(e)), [e.pos.x, e.pos.y, e.pos.z]));
        }
    }
    Ok(())
}

/// The whole harvester judge on redsim (`circuit sim`: the searches' screen, coordinator
/// 2026-10-21 "so searches match the full judge"): [`verify_in_pool`]'s builds (N positions x 4
/// rotations, turned without the game: `Circuit::turned_pure`), each a [`sim_run`] (the game run
/// of that build and seed), the same verdict ([`crate::verify::verdict`] and [`aggregate`]), and
/// the copies side by side at 4 rotations for a `tile:` design. Err: what redsim cannot model.
pub fn sim_verify(c: &Circuit, o: &Options) -> std::result::Result<VerifyReport, String> {
    let kind = c.kind.clone().ok_or_else(|| "no kind".to_string())?;
    if !matches!(kind, Kind::Harvester(_)) {
        return Err(format!("{}: not a harvester", kind.name()));
    }
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let mut jobs: Vec<(u8, Pos, Circuit)> = Vec::new();
    for t in &turns {
        let tc = c.turned_pure(*t).map_err(|e| format!("rotation {t}: {e:#}"))?;
        for off in &offsets {
            jobs.push((*t, *off, tc.translated(*off).map_err(|e| format!("{e:#}"))?));
        }
    }
    // the builds side by side (a 64-stem farm's run takes a minute), read back in build order; the
    // vanilla-speed run first (the rates, and how fast the robustness runs may go)
    let vanilla = vanilla_run(c, o.margin);
    let runs = par_map(&jobs, o.sim_threads(), |(_, _, rc)| {
        let rts = game_rts(rc, c, &vanilla);
        sim_run(rc, rts, game_ticks(rc, rts), o.margin)
    });
    let mut builds = Vec::new();
    for ((t, off, rc), r) in jobs.iter().zip(runs) {
        let r = r.map_err(|why| format!("rotation {t} at {off:?}: {why}"))?;
        builds.push(Build { turns: *t, offset: *off, result: r.map(|log| BuildOk { outcome: judge(rc, &log), dims: log.dims, family: family.clone() }) });
    }
    let mut schedule = Schedule::new(0);
    schedule.ticks = RUN;
    let rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    let tiled = if c.tile.is_some() { Some(sim_tiled_run(c, o, &vanilla)?) } else { None };
    Ok(aggregate(c, rep, tiled, &vanilla))
}

/// `f` over `jobs` on up to `threads` threads (each its own redsim world), the results in job order.
fn par_map<J: Sync, T: Send>(jobs: &[J], threads: usize, f: impl Fn(&J) -> T + Sync) -> Vec<T> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Vec<std::sync::Mutex<Option<T>>> = jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for _ in 0..threads.clamp(1, jobs.len().max(1)) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= jobs.len() {
                        break;
                    }
                    let r = f(&jobs[i]);
                    *out[i].lock().unwrap() = Some(r);
                }
            });
        }
    });
    out.into_iter().map(|m| m.into_inner().unwrap().expect("every job ran")).collect()
}

/// [`tiled_run`] on redsim. Outer Err: unmodeled.
fn sim_tiled_run(c: &Circuit, o: &Options, vanilla: &VanillaRun) -> std::result::Result<std::result::Result<String, String>, String> {
    let tc = match crate::tiled(c) {
        Ok(tc) => tc,
        Err(e) => return Ok(Err(format!("{e:#}"))),
    };
    let mut line = String::new();
    let mut turned = Vec::new();
    for t in 0..4u8 {
        turned.push(tc.turned_pure(t).map_err(|e| format!("tiled, rotation {t}: {e:#}"))?);
    }
    // the 4 rotations side by side, read back in order
    let runs = par_map(&turned, o.sim_threads(), |rc| {
        let rts = game_rts(rc, c, vanilla);
        sim_run(rc, rts, game_ticks(rc, rts), o.margin)
    });
    for (t, (rc, r)) in turned.iter().zip(runs).enumerate() {
        match r.map_err(|why| format!("tiled, rotation {t}: {why}"))? {
            Ok(log) => {
                let out = judge(rc, &log);
                if !out.pass {
                    let extra: Vec<String> = log.stranded.iter().chain(log.escaped.iter()).take(4).cloned().collect();
                    return Ok(Err(format!("rotation {t}: {}{}", out.why, if extra.is_empty() { String::new() } else { format!(" (stranded: {})", extra.join("; ")) })));
                }
                if t == 0 {
                    line = format!("{} copies, {:.2} items per hour per cane", log.canes, per_hour(&log));
                }
            }
            Err(why) => return Ok(Err(format!("rotation {t}: {why}"))),
        }
    }
    Ok(Ok(line))
}
