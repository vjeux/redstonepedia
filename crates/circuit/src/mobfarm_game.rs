//! The mob farms' real-game gate (mobinfra, 2026-11-08). vjeux, 04:19, on the iron-farm pages'
//! caveats ("tested on our simulation of the game ... checked in the real game", "clear weather
//! is not judged", "stops in rain"): "Why all of these caveats? Just do the real work!?". Every
//! other study archives only what passes in the real game; this is the mob farms' gate: each build
//! (4 rotations x 2 positions) runs in the UNMODIFIED 26.3 server (`mctas::vanilla`) for game
//! days with the weather on, on Hard, and passes when it works.
//!
//! **Why the official server and not the bridge** (read in the 26.3 source):
//! * lightning needs a player: `ServerLevel.tickThunder` runs only for the chunks within 128
//!   blocks of a non-spectator player (`ServerChunkCache.tickSpawningChunk`,
//!   `ChunkMap.collectSpawningChunks`); the bridge has no player. So does fire: lava's random-tick
//!   fire and `FireBlock`'s spread run only within `fire_spread_radius_around_player` (128) of one
//!   (`ServerLevel.canSpreadFireAround`). The gate's bot is the farm's AFK player: creative, 60
//!   blocks under the farm's chest with a stone over its head (within 128 of every chunk of the
//!   farm, out of every mob's sight, never a bolt's target, never within 10 of a trap horse on the
//!   farm);
//! * the day clock is the server's (`ServerClockManager.tick` in `MinecraftServer.tickChildren`),
//!   which the bridge replaces (its sandboxes' day time moves by `time add`);
//! * the weather is the server's too (`MinecraftServer.weatherData`, advanced by every level that
//!   can have weather): the bridge's sandboxes would share one.
//!
//! Frozen and stepped (`tick step`) at `tick rate 10000` (set once the bot is in: a login has 600
//! server ticks), a farm runs at the speed of its ticks: about 15 s a build-day.
//!
//! **One build's run**: a void superflat in plains (it rains there), Hard, `spawn_mobs` on (trap
//! horses, zombie reinforcements) and natural monsters off (the terrain lane judges spawning),
//! the weather and the day clock running. The weather first runs alone for a random 150k..400k gt
//! (from a fresh world's first draw toward the cycle's long-run mix, the card's). The farm is the
//! redsim judge's own build after its water settled (`farm::Run`), pasted with `setblock ...
//! strict`; then the day time of the judge's steady state, the daylight detectors, the villagers
//! (each with a `LAST_SLEPT` at the end of the last rest; they claim their beds themselves) and
//! the zombie in its vehicle. It runs `days` game days of natural weather, then a forced day of
//! rain and a forced day of thunder (`weather rain|thunder 24000`). Every 20 gt the gate reads
//! every golem, villager, witch, zombie villager, zombie, vehicle, trap horse and item (a target
//! selector lists only living entities: a golem gone from the list inside the box died), and every
//! 1000 gt the farm's flammable blocks and its lava.
//!
//! **A build passes** when golems spawn; no golem leaves the farm's box, vanishes outside it or
//! lives over 1200 gt; the villagers stay as many, unhurt (no health lost) and unconverted (no
//! witch, no zombie villager); the zombie stays unhurt in its vehicle; no trap horse lands on the
//! farm; no flammable block burns and no lava moves; every drop reaches the chest: no item ever
//! outside the box, and at the end the iron in the farm's containers and on its floor is redsim's
//! delivered iron per golem (the loot's 4 on average, less what the farm's own lava or fire burns:
//! `RunReport::iron_burnt` over the reference runs) for every golem that died (one-sided, 3
//! standard deviations of the per-golem spread, at least the loot's uniform 3..5: a few lost
//! ingots hide in the loot's own spread, a leak does not).
//!
//! **The rates**: each window's golems per hour, the builds' mean, must fall within redsim's
//! interval for that many builds under the same schedule: the judge's own runs (`farm::run` from
//! the cycle's stationary state, `Plan::Windows` with the same forced days, 64 runs), their mean
//! plus or minus z standard errors of the game's mean and of redsim's (Bonferroni over the k
//! windows checked: the natural days at a family-wise 95%, each forced day at a family-wise 99%,
//! z of 0.05/k and 0.01/k two-sided: 2.394 and 2.935 for the three windows, about 2.3% of good
//! farms refused by chance).

use crate::Circuit;
use anyhow::{Context, Result, anyhow, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use redsim::entity::mob::farm::{self, FarmSpec, RunOpts, WalkMode};
use redsim::weather::Plan;
use std::collections::BTreeMap;

/// The gate's settings.
#[derive(Clone, Debug)]
pub struct GateOpts {
    pub version: String,
    /// Game days of natural weather (the rate's).
    pub days: u32,
    /// A forced game day of rain, then one of thunder, after them.
    pub rain_day: bool,
    pub thunder_day: bool,
    /// The weather's warm-up (gt): a random length in this range.
    pub warmup: (u32, u32),
    pub seed: i64,
    /// Builds: (turns, offset); empty: the judge's 8.
    pub builds: Vec<(u8, [i32; 3])>,
    /// Servers at once.
    pub parallel: usize,
    /// Keep each server's world folder.
    pub keep: bool,
    /// Print the samples (every 1000 gt).
    pub trace: bool,
    /// mobinfra: a terrain sample (`village-edge`...): the farm runs at its Site in the sample's
    /// kept world (`world`, default `$MCTAS_HOME/terrain/worlds/<the sample's world>`), `parallel`
    /// runs of the one placement instead of the 8 builds.
    pub terrain: Option<String>,
    pub world: Option<std::path::PathBuf>,
    /// Natural monsters on (`spawn_monsters` in a terrain run; the default): off for an experiment
    /// that separates the farm from the natural mobs around it.
    pub monsters: bool,
    /// A terrain sample's number of runs (0: `parallel`), `parallel` at once: enough runs for a
    /// narrow interval without as many servers at once.
    pub runs: usize,
}

impl GateOpts {
    pub fn new(version: &str) -> GateOpts {
        GateOpts { version: version.to_string(), days: 3, rain_day: true, thunder_day: true, warmup: (150_000, 400_000), seed: 1, builds: Vec::new(), parallel: 8, keep: false, trace: false, terrain: None, world: None, monsters: true, runs: 0 }
    }
}

/// A stretch of one build's run.
#[derive(Clone, Debug, Default)]
pub struct Window {
    pub name: String,
    pub ticks: u32,
    pub golems: u32,
    /// Samples (20 gt) raining and thundering (`weather_check`).
    pub rain_samples: u32,
    pub thunder_samples: u32,
    pub samples: u32,
    /// mobinfra (vjeux 2026-11-08: "It's all about tradeoffs"): lightning's breaks seen in the
    /// window, checked against the card's rates (`GateReport::tradeoffs`): witch-making bolts
    /// (samples with new witches; `witch_villagers` the villagers they converted), vehicles lost,
    /// skeleton trap horses on the farm.
    pub witches: u32,
    pub witch_villagers: u32,
    pub vehicles: u32,
    pub traps: u32,
}

impl Window {
    pub fn golems_per_hour(&self) -> f64 {
        self.golems as f64 * 72000.0 / self.ticks.max(1) as f64
    }
}

/// One build in the game.
#[derive(Clone, Debug, Default)]
pub struct BuildRun {
    pub turns: u8,
    pub offset: [i32; 3],
    /// The run's seed (redsim's steady state, the weather's draw): a failing run reproduces with
    /// it.
    pub seed: i64,
    pub windows: Vec<Window>,
    pub golems: u32,
    pub deaths: u32,
    /// Iron in the chest, in the farm's other containers, loose in the world; poppies in the chest.
    pub iron_chest: u32,
    pub iron_containers: u32,
    pub iron_loose: u32,
    /// The design's own iron in its containers as placed (its `nbt:` lines: a sorter's lock and
    /// filter stacks), not the farm's output.
    pub iron_start: u32,
    /// (terrain) The farm's drops (iron, poppies) seen inside the farm's box, by UUID: one later
    /// outside is a leak; one never inside is the world's (`items_foreign`, traced once).
    pub items_inside: std::collections::BTreeSet<String>,
    pub items_foreign: std::collections::BTreeSet<String>,
    pub poppies: u32,
    /// Bolts seen at a sample (a bolt lives 2..30 gt: most are missed).
    pub bolts_seen: Vec<[i32; 3]>,
    /// Cells of the build whose block changed: a watched one (flammable or lava) when it changed
    /// (every 1000 gt), every cell at the end.
    pub changed: Vec<String>,
    /// The farm's flammable blocks a fire burned (or lava then flowed over), seen at the 1000 gt
    /// checks: a measured event, checked against the card's burn rate over the builds
    /// ([`GateReport::tradeoffs`]), not a failure of the build.
    pub burned: u32,
    pub failures: Vec<String>,
    pub seconds: f64,
    /// The window the build first broke in (a witch, a lost vehicle: tradeoffs the card measures);
    /// what follows a break is its consequence, not checked.
    pub broken: Option<usize>,
    /// Witches, vehicles and trap horses on the farm at the last sample.
    pub witches_now: u32,
    pub vehicles_now: u32,
    pub traps_now: u32,
}

/// What block is at `p` now, among the ones a farm turns into when it breaks.
fn now_at(s: &mut VanillaServer, p: [i32; 3]) -> Result<String> {
    for id in ["air", "lava", "fire", "water"] {
        if s.cmd(&format!("execute if block {} minecraft:{id}", xyz(p)))?.contains("passed") {
            return Ok(id.to_string());
        }
    }
    Ok("another block".to_string())
}

impl BuildRun {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    fn fail(&mut self, msg: String) {
        // each failure once, at its first game time
        let key = |m: &str| if m.starts_with("gt ") { m.split_once(": ").map(|x| x.1.to_string()).unwrap_or_else(|| m.to_string()) } else { m.to_string() };
        let k = key(&msg);
        if self.failures.len() < 8 && !self.failures.iter().any(|f| key(f) == k) {
            self.failures.push(msg);
        }
    }

    /// The farm's iron: what the chest, its containers and the floor hold, less what the design
    /// placed in its containers.
    pub fn iron(&self) -> u32 {
        (self.iron_chest + self.iron_containers + self.iron_loose).saturating_sub(self.iron_start)
    }
}

/// `x y z` of an integer position.
fn xyz(p: [i32; 3]) -> String {
    format!("{} {} {}", p[0], p[1], p[2])
}

/// The numbers of an SNBT list value after `key:[` (`Pos:[1.5d,2.0d,3.5d]`, `UUID:[I;1,2,3,4]`).
fn list_after(s: &str, key: &str) -> Option<Vec<f64>> {
    let i = s.find(&format!("{key}:["))? + key.len() + 2;
    let j = s[i..].find(']')? + i;
    Some(
        s[i..j]
            .trim_start_matches("I;")
            .split(',')
            .filter_map(|t| t.trim().trim_end_matches(['d', 'f', 'b', 's', 'L']).parse::<f64>().ok())
            .collect(),
    )
}

/// A number after `key:` (`Health:20.0f`, `DeathTime:0s`).
fn num_after(s: &str, key: &str) -> Option<f64> {
    let mut from = 0;
    while let Some(k) = s[from..].find(&format!("{key}:")) {
        let i = from + k;
        // a whole key (`Health:`, not `MaxHealth:`)
        let whole = i == 0 || !s.as_bytes()[i - 1].is_ascii_alphanumeric();
        let rest = &s[i + key.len() + 1..];
        let t: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'E').collect();
        if whole && let Ok(v) = t.parse::<f64>() {
            return Some(v);
        }
        from = i + key.len() + 1;
    }
    None
}

/// The entity data answers of `execute as <sel> run data get entity @s [path]`, one per entity,
/// in compact SNBT (26.3 prints `key: value, ...`: the spaces are dropped).
fn entity_answers(out: &str) -> Vec<String> {
    out.split("has the following entity data: ").skip(1).map(compact).collect()
}

/// SNBT without the spaces 26.3's printer puts after `:`, `,` and `;`.
fn compact(s: &str) -> String {
    s.replace(": ", ":").replace(", ", ",").replace("; ", ";")
}

/// `count` of an `Items` entry or an item entity's `Item` for `id`.
fn count_item(nbt: &str, id: &str) -> u32 {
    let nbt = compact(nbt);
    let key = format!("id:\"{id}\"");
    let mut total = 0;
    let mut rest = nbt.as_str();
    while let Some(k) = rest.find(&key) {
        let start = rest[..k].rfind('{').unwrap_or(0);
        let end = rest[k..].find('}').map(|e| k + e).unwrap_or(rest.len());
        let entry = &rest[start..end];
        total += num_after(entry, "count").map(|v| v as u32).unwrap_or(1);
        rest = &rest[end..];
    }
    total
}

/// The iron in an item list's NBT as ingots: ingots and 9 per iron block (a farm's crafter).
fn count_iron(nbt: &str) -> u32 {
    count_item(nbt, "minecraft:iron_ingot") + 9 * count_item(nbt, "minecraft:iron_block")
}

/// The game's `data get block` reply (`back`) holds every field of the design's SNBT (`want`).
fn check_block_entity(back: &str, want: &str) -> Result<()> {
    use redsim::redstone::container::parse_snbt;
    let data = back.split_once("block data: ").map(|(_, d)| d).ok_or_else(|| anyhow!("no block data in `{back}`"))?;
    let got = parse_snbt(data.trim()).map_err(|e| anyhow!("the game's data `{data}`: {e}"))?;
    let want_v = parse_snbt(want).map_err(|e| anyhow!("the design's `{want}`: {e}"))?;
    if snbt_holds(&got, &want_v) {
        Ok(())
    } else {
        bail!("the game holds {data}")
    }
}

/// Does `got` hold every field of `want`: a compound its keys (more allowed), a list the same
/// length with each element of `want` held by a distinct one of `got`, numbers by value whatever
/// their suffix (a boolean as 1 or 0), ids with or without `minecraft:`.
fn snbt_holds(got: &redsim::redstone::container::Snbt, want: &redsim::redstone::container::Snbt) -> bool {
    use redsim::redstone::container::Snbt;
    let text = |s: &str| s.trim_start_matches("minecraft:").to_string();
    let flag = |w: &str| match w {
        "true" => Some(1),
        "false" => Some(0),
        _ => None,
    };
    match (got, want) {
        (Snbt::Compound(g), Snbt::Compound(w)) => w.iter().all(|(k, wv)| g.iter().any(|(gk, gv)| gk == k && snbt_holds(gv, wv))),
        (Snbt::List(g), Snbt::List(w)) => {
            if g.len() != w.len() {
                return false;
            }
            let mut used = vec![false; g.len()];
            w.iter().all(|wv| match (0..g.len()).find(|&j| !used[j] && snbt_holds(&g[j], wv)) {
                Some(j) => {
                    used[j] = true;
                    true
                }
                None => false,
            })
        }
        (Snbt::Int(a), Snbt::Int(b)) => a == b,
        (Snbt::Int(a), Snbt::Word(b)) | (Snbt::Word(b), Snbt::Int(a)) => flag(b) == Some(*a),
        (Snbt::Str(a) | Snbt::Word(a), Snbt::Str(b) | Snbt::Word(b)) => text(a) == text(b),
        _ => false,
    }
}
/// A golem as the gate tracks it.
#[derive(Clone, Debug)]
struct Golem {
    born: u32,
    /// Where it was first seen (its spawn cell, give or take a sample's 20 gt of walking).
    first: [f64; 3],
    last: [f64; 3],
    dead: bool,
    seen: bool,
    /// Already reported (each golem's failure once).
    reported: bool,
    /// Not the farm's (in a terrain sample: a golem standing outside the farm at the start, the
    /// village's): never counted, never checked.
    foreign: bool,
}

/// The judge's builds: (turns, offset).
pub fn judge_builds() -> Vec<(u8, [i32; 3])> {
    let mut v = Vec::new();
    for t in 0..4u8 {
        for off in clock::offsets(2) {
            v.push((t, off));
        }
    }
    v
}

/// The riders' seats as redsim seats them: every passenger's position after the arrival and one
/// tick (positionRider), with its entity id.
fn rider_seats(spec: &FarmSpec, o: &RunOpts) -> Result<Vec<(String, [f64; 3])>> {
    let mut run = farm::Run::new(spec, o).map_err(|e| anyhow!(e))?;
    run.tick();
    Ok(run
        .world
        .entities
        .list
        .iter()
        .filter(|e| e.vehicle.is_some() && !e.removed)
        .filter_map(|e| match &e.kind {
            redsim::entity::EntityKind::Mob(m) => Some((m.ty.def().id.to_string(), [e.pos.x, e.pos.y, e.pos.z])),
            _ => None,
        })
        .collect())
}

/// A vehicle's `Passengers:[{id:"..."},...]` (riders with no data of their own) and the rest of its
/// data (a turned build's `Rotation`...); `None` without such a list (summoned as written).
fn split_passengers(nbt: &str) -> Option<(Vec<String>, String)> {
    let start = nbt.find("Passengers:[")?;
    let list_start = start + "Passengers:[".len();
    let len = nbt[list_start..].find(']')?;
    let inner = &nbt[list_start..list_start + len];
    let mut ids = Vec::new();
    for part in inner.split("},") {
        let part = part.trim().trim_start_matches('{').trim_end_matches('}');
        let id = part.strip_prefix("id:\"")?.strip_suffix('"')?;
        // a rider with data of its own: summoned as written
        if id.contains(['"', ',', '{', '[', ':']) && !id.starts_with("minecraft:") {
            return None;
        }
        ids.push(id.to_string());
    }
    let rest = format!("{}{}", &nbt[..start], &nbt[list_start + len + 1..]).replace("{,", "{").replace(",}", "}").replace(",,", ",");
    (!ids.is_empty()).then_some((ids, rest))
}

/// Entity data `nbt` (a compound, or `{}`) with `tag` added to its `Tags` (made when missing).
fn with_tag(nbt: &str, tag: &str) -> String {
    let body = nbt.trim().trim_start_matches('{').trim_end_matches('}').trim();
    if let Some(i) = body.find("Tags:[") {
        let at = i + "Tags:[".len();
        let sep = if body[at..].starts_with(']') { "" } else { "," };
        return format!("{{{}\"{tag}\"{sep}{}}}", &body[..at], &body[at..]);
    }
    if body.is_empty() { format!("{{Tags:[\"{tag}\"]}}") } else { format!("{{Tags:[\"{tag}\"],{body}}}") }
}

/// The farm's blocks as the redsim judge builds them (`farm::Run::start` and its water's settle;
/// the daylight detectors held back), as `setblock` states, and the detectors.
/// The farm as the redsim judge builds it: its blocks after its water settled (`farm::Run::start`
/// and its settle; the daylight detectors held back) as `setblock` states, the detectors, and the
/// cells watched as it runs (flammable blocks and lava: a fire or a flow breaks the farm).
struct Built {
    blocks: Vec<([i32; 3], String)>,
    detectors: Vec<([i32; 3], String)>,
    watch: Vec<([i32; 3], String)>,
    /// The comparators' outputs as redsim settled them (`ComparatorBlockEntity.OutputSignal`: a
    /// pasted comparator's starts at 0 whatever its state).
    comparators: Vec<([i32; 3], u8)>,
}

/// The comparators among `blocks` with their settled output in `w`.
fn comparator_outputs(w: &redsim::World, blocks: &[([i32; 3], String)]) -> Vec<([i32; 3], u8)> {
    blocks.iter().filter(|(_, st)| st.starts_with("minecraft:comparator")).map(|(p, _)| (*p, w.comparator_out.get(&redsim::BlockPos::new(p[0], p[1], p[2])).copied().unwrap_or(0))).collect()
}

fn built_blocks(spec: &FarmSpec, seed: i64) -> Result<Built> {
    let o = RunOpts::steady(seed, 0, WalkMode::Random);
    let mut run = farm::Run::start(spec, &o).map_err(|e| anyhow!(e))?;
    while run.settle_tick() {}
    let w = &run.world;
    let mut blocks = Vec::new();
    let mut watch = Vec::new();
    for y in spec.lo.y..=spec.hi.y {
        for z in spec.lo.z..=spec.hi.z {
            for x in spec.lo.x..=spec.hi.x {
                let b = w.get(redsim::BlockPos::new(x, y, z));
                if !b.is_air() {
                    let st = b.to_string();
                    if redsim::weather::lightning::flammable(&b).is_some() || matches!(b, redsim::Block::Lava { .. }) {
                        watch.push(([x, y, z], st.split('[').next().unwrap_or(&st).to_string()));
                    }
                    blocks.push(([x, y, z], st));
                }
            }
        }
    }
    let detectors = spec.blocks.iter().filter(|(_, b)| matches!(b, redsim::Block::DaylightDetector { .. })).map(|(p, b)| ([p.x, p.y, p.z], b.to_string())).collect();
    let comparators = comparator_outputs(w, &blocks);
    Ok(Built { blocks, detectors, watch, comparators })
}

/// The containers of the farm that can hold its drops (hoppers, chests, barrels...), not the
/// output chest.
fn containers(blocks: &[([i32; 3], String)], output: [i32; 3]) -> Vec<[i32; 3]> {
    blocks.iter().filter(|(p, s)| *p != output && (s.contains("hopper") || s.contains(":chest") || s.contains("barrel") || s.contains("dropper") || s.contains("dispenser"))).map(|(p, _)| *p).collect()
}

/// Start the build's server: plains void superflat, Hard, frozen at `tick rate 10000`, the bot
/// off the farm under a stone block, the weather and time running, natural monsters off.
fn start_server(o: &GateOpts, spec: &FarmSpec) -> Result<(VanillaServer, [i32; 3])> {
    let v = VanillaOptions {
        version: o.version.clone(),
        heap_mb: 2048,
        keep_dir: o.keep,
        view_distance: 6,
        simulation_distance: 6,
        difficulty: "hard".into(),
        biome: "minecraft:plains".into(),
        ..Default::default()
    };
    let mut s = VanillaServer::start(&v)?;
    // the fast rate only once the bot is in (a login has 600 server ticks: 60 ms at 10000)
    s.cmd_ok("tick rate 10000")?;
    // the AFK player 60 blocks under the farm's chest, a stone over its head: within 128 blocks of
    // every chunk of the farm (the thunder rolls), entity ticking there, out of every mob's sight,
    // never a bolt's target, and a trap horse on the farm (60 blocks up) never within its 10
    let bot = [spec.output.x, spec.lo.y - 60, spec.output.z];
    s.cmd_ok(&format!("forceload add {} {} {} {}", spec.lo.x - 8, spec.lo.z - 8, spec.hi.x + 8, spec.hi.z + 8))?;
    s.cmd_ok(&format!("setblock {} {} {} minecraft:stone strict", bot[0], bot[1] + 3, bot[2]))?;
    s.teleport_bot(bot[0] as f64 + 0.5, bot[1] as f64, bot[2] as f64 + 0.5)?;
    for c in ["gamerule spawn_mobs true", "gamerule spawn_monsters false", "gamerule spawn_phantoms false", "gamerule spawn_patrols false", "gamerule spawn_wandering_traders false", "gamerule advance_time true", "gamerule advance_weather true"] {
        s.cmd_ok(c)?;
    }
    Ok((s, bot))
}

/// Step `n` ticks in steps of at most 1000 (`tick step` waits at most 60 s).
fn step(s: &mut VanillaServer, n: u32) -> Result<()> {
    let mut left = n;
    while left > 0 {
        let k = left.min(1000);
        s.step(k)?;
        left -= k;
    }
    Ok(())
}

/// The game run of a terrain sample (`GateOpts::terrain`): the farm at its Site (the judge's spot,
/// offset and fills) in a copy of the sample's kept world, the one the committed sample was cut
/// from (devvm69725's, push #5), its saved weather the cycle's stationary state (no warm-up ticks
/// that would age the village), the AFK bot at the design's `afk:` (creative counts for spawning),
/// Hard, natural monsters on (phantoms, patrols, traders off, as the terrain judge), simulation
/// distance 10 (every chunk within 128 of the bot ticks, as redsim's natural spawning), only the
/// farm's box forced (forced chunks spawn too). Run `k` of `o.parallel`.
pub fn run_terrain(c: &Circuit, name: &str, k: u8, o: &GateOpts) -> Result<BuildRun> {
    let t0 = std::time::Instant::now();
    let spec = crate::mobfarm::spec_of(c).map_err(|e| anyhow!(e))?;
    let sample = redsim::terrain::samples::sample(name).ok_or_else(|| anyhow!("no terrain sample {name}"))?;
    let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(crate::mobfarm::site_of(c, &spec, name).map_err(|e| anyhow!(e))?));
    let spec = site.spec(&spec);
    let seed = o.seed.wrapping_mul(1000) + 700 + k as i64;
    let mut ro = RunOpts::steady(seed, 0, WalkMode::Random);
    ro.site = Some(site);
    let built = terrain_built(&spec, site, &ro)?;
    let seats = rider_seats(&spec, &ro)?;
    let output = [spec.output.x, spec.output.y, spec.output.z];
    let boxes = containers(&built.blocks, output);
    let world = o.world.clone().unwrap_or_else(|| mctas::paths::home().join("terrain").join("worlds").join(sample.world));
    if !world.join("level.dat").is_file() {
        bail!("the kept world of {name} ({}) is not at {}: copy the one the sample was cut from (`--world`)", sample.world, world.display());
    }
    let w = redsim::weather::Weather::new(Plan::Cycle, seed).data;
    let saved = mctas::vanilla::SavedWeather { clear_weather_time: w.clear_weather_time, rain_time: w.rain_time, thunder_time: w.thunder_time, raining: w.raining, thundering: w.thundering };
    let v = VanillaOptions {
        version: o.version.clone(),
        heap_mb: 4096,
        keep_dir: o.keep,
        view_distance: 10,
        simulation_distance: 10,
        difficulty: "hard".into(),
        world: Some(world),
        default_world: true,
        saved_weather: Some(saved),
        ..Default::default()
    };
    let mut s = VanillaServer::start(&v)?;
    s.cmd_ok("tick rate 10000")?;
    s.cmd_ok(&format!("forceload add {} {} {} {}", spec.lo.x, spec.lo.z, spec.hi.x, spec.hi.z))?;
    s.teleport_bot(site.afk.x, site.afk.y, site.afk.z)?;
    let monsters = if o.monsters { "gamerule spawn_monsters true" } else { "gamerule spawn_monsters false" };
    for c in ["gamerule spawn_mobs true", monsters, "gamerule spawn_phantoms false", "gamerule spawn_patrols false", "gamerule spawn_wandering_traders false", "gamerule advance_time true", "gamerule advance_weather true"] {
        s.cmd_ok(c)?;
    }
    if o.trace {
        eprintln!("terrain run {k}: {name} offset {:?}, {} cells changed, the AFK bot at {:.1} {:.1} {:.1}, weather {saved:?}", site.offset, built.blocks.len(), site.afk.x, site.afk.y, site.afk.z);
    }
    let run = BuildRun { turns: k, offset: site.offset, seed, ..Default::default() };
    let p = Prepared { entities: spec.entities.clone(), scope: Scope::terrain(&spec), label: format!("terrain run {k}"), day0: ro.day0, spec, built, seats, output, boxes };
    run_farm(s, p, run, o, t0)
}

/// The farm the terrain judge builds at its Site (`farm::Run::start` with the site, its water
/// settled) as the cells it changed from the sample's world: the farm's box and 3 around it (the
/// site's seals) and the fills (C7's glass), air where it dug; the detectors held back; the farm's
/// flammable blocks and lava watched.
fn terrain_built(spec: &FarmSpec, site: &redsim::entity::mob::site::Site, o: &RunOpts) -> Result<Built> {
    let mut run = farm::Run::start(spec, o).map_err(|e| anyhow!(e))?;
    while run.settle_tick() {}
    let before = site.world.read().map_err(|_| anyhow!("the site's world lock is poisoned"))?;
    let w = &run.world;
    // every cell the build changed: the farm's box and 3 around it (the site's cobblestone seals
    // of natural openings next to the farm, its digging at the edges), and the fills (C7's glass,
    // wherever the golem search reached)
    let mut cells: Vec<redsim::BlockPos> = Vec::new();
    let (lo, hi) = (spec.lo.offset(-3, -3, -3), spec.hi.offset(3, 3, 3));
    let inside = |p: &redsim::BlockPos| (lo.x..=hi.x).contains(&p.x) && (lo.y..=hi.y).contains(&p.y) && (lo.z..=hi.z).contains(&p.z);
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                cells.push(redsim::BlockPos::new(x, y, z));
            }
        }
    }
    cells.extend(site.fills.iter().copied().filter(|p| !inside(p)));
    cells.extend(site.seals.iter().map(|(p, _)| *p).filter(|p| !inside(p)));
    cells.extend(site.clears.iter().map(|(p, _)| *p).filter(|p| !inside(p)));
    let detectors: Vec<([i32; 3], String)> = spec.blocks.iter().filter(|(_, b)| matches!(b, redsim::Block::DaylightDetector { .. })).map(|(p, b)| ([p.x, p.y, p.z], b.to_string())).collect();
    let mut blocks = Vec::new();
    let mut watch = Vec::new();
    for p in cells {
        let (was, b) = (before.get(p), w.get(p));
        // the same state (a natural stone where the design's stone stands: `Block::Terrain` and
        // `Block::Simple` are one state in the game) is no change
        if was == b || was.to_string() == b.to_string() || detectors.iter().any(|(d, _)| *d == [p.x, p.y, p.z]) {
            continue;
        }
        let st = b.to_string();
        if redsim::weather::lightning::flammable(&b).is_some() || matches!(b, redsim::Block::Lava { .. }) {
            watch.push(([p.x, p.y, p.z], st.split('[').next().unwrap_or(&st).to_string()));
        }
        blocks.push(([p.x, p.y, p.z], st));
    }
    let comparators = comparator_outputs(w, &blocks);
    Ok(Built { blocks, detectors, watch, comparators })
}

/// The game run of one build: `c` turned and moved.
pub fn run_build(c: &Circuit, turns: u8, offset: [i32; 3], o: &GateOpts) -> Result<BuildRun> {
    let t0 = std::time::Instant::now();
    let spec = crate::mobfarm::spec_of(c).map_err(|e| anyhow!(e))?;
    let seed = o.seed.wrapping_mul(1000) + turns as i64 * 10 + (offset[0] & 7) as i64;
    let Built { blocks, detectors, watch, comparators } = built_blocks(&spec, seed)?;
    let seats = rider_seats(&spec, &RunOpts::steady(seed, 0, WalkMode::Random))?;
    if o.trace {
        eprintln!("build {turns} {offset:?}: redsim's seats {seats:?}");
    }
    let output = [spec.output.x, spec.output.y, spec.output.z];
    let boxes = containers(&blocks, output);
    let (mut s, _bot) = start_server(o, &spec)?;
    let mut run = BuildRun { turns, offset, seed, ..Default::default() };
    // the weather toward its long-run mix, the farm not built yet
    let mut r = redsim::rng::LegacyRandom::new(seed ^ 0x6761_7465);
    let warm = o.warmup.0 + r.next_int((o.warmup.1 - o.warmup.0).max(1) as i32) as u32;
    step(&mut s, warm)?;
    let entities = c.clock.design.entities.iter().map(|e| (e.kind.clone(), e.pos, e.nbt.clone())).collect();
    let day0 = RunOpts::steady(seed, 0, WalkMode::Random).day0;
    let p = Prepared { spec, built: Built { blocks, detectors, watch, comparators }, seats, entities, day0, scope: Scope::void(), label: format!("build {turns} {offset:?}"), output, boxes };
    run_farm(s, p, run, o, t0)
}

/// What a game run needs once its server is up (world coordinates): the farm as the judge built
/// it, the entities to summon, the riders' seats, the steady state's day time, which entities the
/// samples read.
struct Prepared {
    spec: FarmSpec,
    built: Built,
    seats: Vec<(String, [f64; 3])>,
    entities: Vec<(String, [f64; 3], Option<String>)>,
    day0: i64,
    scope: Scope,
    label: String,
    output: [i32; 3],
    boxes: Vec<[i32; 3]>,
}

/// Which entities the samples read. In the void every entity is the farm's (`void`); in a terrain
/// sample only the farm's own (tagged `mf` at their summon: villagers, the zombie, the vehicle)
/// and what is near the farm's box (witches, zombie villagers, items within 8 blocks), and only
/// the golems first seen inside the farm (the village's golem and others are not the farm's).
#[derive(Clone, Debug)]
struct Scope {
    own: String,
    near: String,
    all_golems: bool,
}

impl Scope {
    fn void() -> Scope {
        Scope { own: String::new(), near: String::new(), all_golems: true }
    }

    fn terrain(spec: &FarmSpec) -> Scope {
        let (lo, hi) = (spec.lo.offset(-8, -8, -8), spec.hi.offset(8, 8, 8));
        Scope { own: ",tag=mf".into(), near: format!(",x={},y={},z={},dx={},dy={},dz={}", lo.x, lo.y, lo.z, hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1), all_golems: false }
    }
}

/// A game run from a started server on: the farm pasted, the steady state's day time, the
/// detectors, the entities (each tagged `mf`), then the natural days and the forced ones sampled
/// every 20 gt, and the iron at the end.
fn run_farm(mut s: VanillaServer, p: Prepared, mut run: BuildRun, o: &GateOpts, t0: std::time::Instant) -> Result<BuildRun> {
    let Prepared { spec, built: Built { blocks, detectors, mut watch, comparators }, seats, entities, day0, scope, label, output, boxes } = p;
    for (p, st) in &blocks {
        let out = s.cmd(&format!("setblock {} {st} strict", xyz(*p)))?;
        if !out.contains("Changed the block") {
            bail!("setblock {p:?} {st}: {out}");
        }
    }
    // the design's block entities (its `nbt:` lines: a crafter's disabled slots, a sorter's lock
    // and filter items), merged as the judge merges them (`farm::Run::start`) and read back: the
    // game must hold every field the design gives; then the comparators' outputs as redsim
    // settled them (a pasted comparator's block entity starts at 0 whatever its state)
    for (q, snbt) in &spec.block_entities {
        let at = [q.x, q.y, q.z];
        let out = s.cmd(&format!("data merge block {} {snbt}", xyz(at)))?;
        if !out.contains("Modified block data") && !out.contains("Nothing changed") {
            bail!("data merge block {at:?} {snbt}: {out}");
        }
        let back = s.cmd_long(&format!("data get block {}", xyz(at)))?;
        check_block_entity(&back, snbt).with_context(|| format!("the block entity at {at:?} (`nbt: {snbt}`) after the paste"))?;
    }
    for (at, n) in &comparators {
        let out = s.cmd(&format!("data merge block {} {{OutputSignal:{n}}}", xyz(*at)))?;
        if !out.contains("Modified block data") && !out.contains("Nothing changed") {
            bail!("the comparator at {at:?}: output {n}: {out}");
        }
    }
    // the iron the design placed in its containers (not the farm's output)
    for p in &boxes {
        run.iron_start += count_iron(&s.cmd_long(&format!("data get block {} Items", xyz(*p)))?);
    }
    // the judge's steady state: a day time in the day, LAST_SLEPT at the end of the last rest
    s.cmd_ok(&format!("time set {day0}"))?;
    for (p, st) in &detectors {
        s.cmd_ok(&format!("setblock {} {st}", xyz(*p)))?;
    }
    let now = s.game_time()?;
    let slept = now - (day0 - 10);
    let mut villager_spots: Vec<[f64; 3]> = Vec::new();
    for (n, (kind, pos, nbt)) in entities.iter().enumerate() {
        let k = kind.trim_start_matches("minecraft:");
        let [x, y, z] = *pos;
        // a vehicle with its riders: the vehicle alone, then each rider summoned at the seat redsim
        // seats it on and mounted (`ride`), so its first tick is in its seat: summoned in one
        // command, a passenger starts at the vehicle's position and can spend that tick inside a
        // block before positionRider seats it (a hit the farm never takes)
        if let Some((riders, rest)) = nbt.as_deref().and_then(split_passengers) {
            s.cmd_ok(&format!("summon minecraft:{k} {x} {y} {z} {}", with_tag(&with_tag(&rest, &format!("mf_vehicle_{n}")), "mf"))).with_context(|| format!("summon {k}"))?;
            for (j, rider) in riders.iter().enumerate() {
                let short = rider.trim_start_matches("minecraft:");
                let seat = seats
                    .iter()
                    .filter(|(id, _)| id.trim_start_matches("minecraft:") == short)
                    .nth(j)
                    .map(|(_, p)| *p)
                    .ok_or_else(|| anyhow!("redsim seats no {rider} on the {k} at {x} {y} {z}"))?;
                s.cmd_ok(&format!("summon {rider} {} {} {} {{Tags:[\"mf_rider_{n}_{j}\",\"mf\"]}}", seat[0], seat[1], seat[2])).with_context(|| format!("summon {rider}"))?;
                let out = s.cmd(&format!("ride @e[tag=mf_rider_{n}_{j},limit=1] mount @e[tag=mf_vehicle_{n},limit=1]"))?;
                if !out.contains("started riding") {
                    bail!("ride {rider} on its {k}: {out}");
                }
            }
            continue;
        }
        let cmd = match (k, nbt.as_deref()) {
            ("villager", _) => {
                villager_spots.push(*pos);
                format!("summon minecraft:villager {x} {y} {z} {{PersistenceRequired:1b,Tags:[\"mf\"],Brain:{{memories:{{\"minecraft:last_slept\":{{value:{slept}L}}}}}}}}")
            }
            (_, Some(n)) => format!("summon minecraft:{k} {x} {y} {z} {}", with_tag(n, "mf")),
            (_, None) => format!("summon minecraft:{k} {x} {y} {z} {{Tags:[\"mf\"]}}"),
        };
        s.cmd_ok(&cmd).with_context(|| format!("summon {k}"))?;
    }
    let own = &scope.own;
    let zombies0 = s.count_entities(&format!("@e[type=minecraft:zombie{own}]"))?;
    let vehicles0 = s.count_entities(&format!("@e[type=#minecraft:boat{own}]"))? + s.count_entities(&format!("@e[type=minecraft:minecart{own}]"))?;
    let villagers0 = villager_spots.len() as u32;
    run.vehicles_now = vehicles0;
    let inner = spec.inner();
    let mut golems: BTreeMap<String, Golem> = BTreeMap::new();
    let mut t: u32 = 0;
    let mut windows = vec![Window { name: format!("{} days of natural weather", o.days), ..Default::default() }];
    let mut plan: Vec<(u32, &str)> = vec![(o.days * 24000, "")];
    if o.rain_day {
        plan.push((24000, "weather rain 24000"));
    }
    if o.thunder_day {
        plan.push((24000, "weather thunder 24000"));
    }
    // (--trace: the zombie and its vehicle tick by tick over the first 20 gt)
    if o.trace {
        let z = s.cmd_long(&format!("execute as @e[type=minecraft:zombie{own}] run data get entity @s Pos"))?;
        eprintln!("{label} gt 0: zombie {}", z.chars().take(160).collect::<String>());
        for _ in 0..20 {
            step(&mut s, 1)?;
            t += 1;
            let z = s.cmd_long(&format!("execute as @e[type=minecraft:zombie{own}] run data get entity @s"))?;
            let b = s.cmd_long(&format!("execute as @e[type=#minecraft:boat{own}] run data get entity @s Pos"))?;
            let pick = |key: &str| z.split(key).nth(1).map(|r| r.chars().take(40).collect::<String>()).unwrap_or_default();
            eprintln!("{label} gt {t}: zombie Health{} | HurtTime{} | Pos{} | fall_distance{} | Fire{} | boat {}", pick("Health"), pick("HurtTime"), pick("Pos"), pick("fall_distance"), pick("Fire:"), b.chars().take(140).collect::<String>());
        }
    }
    for (k, (len, cmd)) in plan.iter().enumerate() {
        if !cmd.is_empty() {
            s.cmd_ok(cmd)?;
            windows.push(Window { name: cmd.replace("weather ", "forced ").replace(" 24000", " day"), ..Default::default() });
        }
        let end = t + len;
        while t < end {
            step(&mut s, 20)?;
            t += 20;
            let w = windows.last_mut().unwrap();
            w.ticks += 20;
            w.samples += 1;
            sample(&mut s, &mut run, &mut golems, t, k, inner, &spec, villagers0, zombies0, &scope, &mut windows, &villager_spots, o.trace.then_some(label.as_str()))?;
            // the flammable blocks and the lava: burned away or flowed. A burned block is a
            // measured event (the parent, 2026-11-08: the card measures burns, so the gate checks
            // the count against the card's rate over every build: `burns` in the tradeoffs); the
            // farm's own lava moving is not one the card measures: it fails the build
            if t % 1000 == 0 {
                let mut still = Vec::new();
                for (p, id) in watch.drain(..) {
                    if s.cmd(&format!("execute if block {} {id}", xyz(p)))?.contains("passed") {
                        still.push((p, id));
                    } else {
                        let now = now_at(&mut s, p)?;
                        let what = format!("gt {t}: the {} at {} is {now} (burned or flowed)", id.trim_start_matches("minecraft:"), xyz(p));
                        run.changed.push(what.clone());
                        if id == "minecraft:lava" {
                            run.fail(what);
                        } else {
                            run.burned += 1;
                        }
                    }
                }
                watch = still;
            }
            // the AFK bot still on the server (a bot dropped under load would let the chunks
            // around the farm unload: the golems' spawn spots there fail; a harness failure)
            if t % 1000 == 0 && !s.cmd("list")?.contains("mctas_bot") {
                run.fail(format!("gt {t}: the AFK bot is no longer on the server (a harness failure, not the farm's: the chunks around the farm unload)"));
            }
            if o.trace && t % 1000 == 0 {
                eprintln!("{label} gt {t}: {} golems, {} dead, failures {:?}", run.golems, run.deaths, run.failures);
                // the mobs near the farm by type (`data get entity` leaves the type out, so one
                // count per type: a natural mob wandering in, a stray golem)
                let mut by: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
                for ty in ["iron_golem", "zombie", "zombie_villager", "skeleton", "creeper", "spider", "witch", "enderman", "skeleton_horse", "drowned", "slime"] {
                    let n = s.count_entities(&format!("@e[type=minecraft:{ty}{}]", scope.near))?;
                    if n > 0 {
                        by.insert(ty, n);
                    }
                }
                eprintln!("  {label} near the farm: {by:?}");
                if t % 4000 == 0 {
                    let mem = s.cmd_long(&format!("execute as @e[type=minecraft:villager{own}] run data get entity @s Brain.memories"))?;
                    eprintln!("  {label} villager memories: {}", mem.chars().take(1500).collect::<String>());
                    let z = s.cmd_long(&format!("execute as @e[type=minecraft:zombie{own}] run data get entity @s Health"))?;
                    eprintln!("  {label} zombie: {z}");
                }
                // the thunder day in detail (day-1 2026-11-08: D1U's thunder days run slow in the
                // game): every golem within 16 of each farm villager (GolemSensor: no sight
                // needed) and where it stands, and every other mob in the farm's box inflated by
                // (16, 4, 16) (a golem's NearestAttackableTarget reach)
                let thunder_from = o.days * 24000 + if o.rain_day { 24000 } else { 0 };
                if o.thunder_day && t >= thunder_from {
                    let pos_of = |a: &str| list_after(&format!("Pos:{}", a.trim()), "Pos").filter(|p| p.len() == 3);
                    let in_farm = |p: &[f64]| p[0] >= spec.lo.x as f64 && p[0] < spec.hi.x as f64 + 1.0 && p[1] >= spec.lo.y as f64 && p[1] < spec.hi.y as f64 + 1.0 && p[2] >= spec.lo.z as f64 && p[2] < spec.hi.z as f64 + 1.0;
                    let vs = entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:villager{own}] run data get entity @s Pos"))?);
                    for (j, v) in vs.iter().filter_map(|a| pos_of(a)).enumerate() {
                        let gs = entity_answers(&s.cmd_long(&format!("execute positioned {} {} {} as @e[type=minecraft:iron_golem,distance=..16] run data get entity @s Pos", v[0], v[1], v[2]))?);
                        let list: Vec<String> = gs.iter().filter_map(|a| pos_of(a)).map(|p| format!("{:.1} {:.1} {:.1} ({})", p[0], p[1], p[2], if in_farm(&p) { "in the farm" } else { "outside the farm" })).collect();
                        eprintln!("  {label} thunder gt {t}: villager {j} at {:.1} {:.1} {:.1}: {} golems within 16: {}", v[0], v[1], v[2], list.len(), list.join("; "));
                    }
                    let (lo, hi) = (spec.lo.offset(-16, -4, -16), spec.hi.offset(16, 4, 16));
                    let area = format!("x={},y={},z={},dx={},dy={},dz={}", lo.x, lo.y, lo.z, hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1);
                    // `data get entity` leaves the type out (saveWithoutId): one query per type
                    let mut list: Vec<String> = Vec::new();
                    for ty in ["zombie", "zombie_villager", "husk", "drowned", "skeleton", "stray", "bogged", "creeper", "spider", "cave_spider", "enderman", "witch", "slime", "skeleton_horse", "zombie_horse", "pillager", "vindicator", "phantom"] {
                        for a in entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:{ty},{area}] run data get entity @s Pos"))?) {
                            if let Some(p) = pos_of(&a) {
                                list.push(format!("{ty} at {:.1} {:.1} {:.1}{}", p[0], p[1], p[2], if in_farm(&p) { " (in the farm)" } else { "" }));
                            }
                        }
                    }
                    eprintln!("  {label} thunder gt {t}: {} mobs in the box +(16, 4, 16): {}", list.len(), list.join("; "));
                }
            }
            if run.failures.len() >= 4 {
                break;
            }
        }
        if run.failures.len() >= 4 {
            break;
        }
    }
    // the drops: the chest, the farm's containers, the items on the floor (iron as ingots: a block
    // a crafter made counts 9, as redsim's farm counts it)
    let chest = s.cmd_long(&format!("data get block {} Items", xyz(output)))?;
    run.iron_chest = count_iron(&chest);
    run.poppies = count_item(&chest, "minecraft:poppy");
    for p in &boxes {
        run.iron_containers += count_iron(&s.cmd_long(&format!("data get block {} Items", xyz(*p)))?);
    }
    run.iron_loose = count_iron(&s.cmd_long(&format!("execute as @e[type=minecraft:item{}] run data get entity @s Item", scope.near))?);
    // what changed in the build: every cell's block type against the paste
    for (p, st) in &blocks {
        let id = st.split('[').next().unwrap_or(st);
        if !s.cmd(&format!("execute if block {} {id}", xyz(*p)))?.contains("passed") {
            let now = now_at(&mut s, *p)?;
            run.changed.push(format!("the {} at {} is {now} at the end", id.trim_start_matches("minecraft:"), xyz(*p)));
        }
    }
    run.windows = windows;
    if run.golems == 0 {
        run.fail("no golem spawned".into());
    }
    // (the iron each dead golem delivered is checked in `gate`, against redsim's delivered iron
    // per golem from the reference runs: burnt iron is a measured loss, brief rev 9 iteration 11)
    run.seconds = t0.elapsed().as_secs_f64();
    drop(s);
    Ok(run)
}

/// One sample: every golem, the villagers, the zombie, the vehicles, witches, zombie villagers,
/// trap horses, items outside the box, the weather.
#[allow(clippy::too_many_arguments)]
fn sample(
    s: &mut VanillaServer,
    run: &mut BuildRun,
    golems: &mut BTreeMap<String, Golem>,
    t: u32,
    window: usize,
    inner: redsim::entity::Aabb,
    spec: &FarmSpec,
    villagers0: u32,
    zombies0: u32,
    scope: &Scope,
    windows: &mut [Window],
    villagers: &[[f64; 3]],
    trace: Option<&str>,
) -> Result<()> {
    let (own, near) = (&scope.own, &scope.near);
    let w = windows.last_mut().unwrap();
    let rain = s.cmd("execute if predicate {type:\"minecraft:weather_check\",raining:true}")?;
    if rain.contains("passed") {
        w.rain_samples += 1;
    }
    if s.cmd("execute if predicate {type:\"minecraft:weather_check\",thundering:true}")?.contains("passed") {
        w.thunder_samples += 1;
    }
    // lightning's breaks (tradeoffs the card measures, vjeux 2026-11-08: counted here and checked
    // against the card's rates in `gate`; what follows a break is its consequence, unchecked):
    // new witches are villagers a bolt struck (one bolt converts every villager in its box: a
    // sample's new witches are one witch-making bolt, the card's unit), a lost vehicle one a bolt
    // broke; only the farm's own (a converted villager keeps its tags, `ConversionType`: a natural
    // witch near a terrain farm is not the farm's)
    let witches = s.count_entities(&format!("@e[type=minecraft:witch{own}]"))?;
    if witches > run.witches_now {
        let w = windows.last_mut().unwrap();
        w.witches += 1;
        w.witch_villagers += witches - run.witches_now;
        run.witches_now = witches;
        run.broken.get_or_insert(window);
    }
    let vehicles = s.count_entities(&format!("@e[type=#minecraft:boat{own}]"))? + s.count_entities(&format!("@e[type=minecraft:minecart{own}]"))?;
    if vehicles < run.vehicles_now {
        windows.last_mut().unwrap().vehicles += run.vehicles_now - vehicles;
        run.vehicles_now = vehicles;
        run.broken.get_or_insert(window);
    }
    let mut traps = 0;
    // a trap horse a bolt left (`SkeletonTrap`: the horsemen a triggered trap spawns are not
    // traps; in a terrain sample they ride to the AFK player from wherever the bolt fell)
    for a in entity_answers(&s.cmd_long("execute as @e[type=minecraft:skeleton_horse,nbt={SkeletonTrap:1b}] run data get entity @s Pos")?) {
        if let Some(p) = list_after(&format!("Pos:{}", a.trim()), "Pos")
            && p.len() == 3
            && p[0] >= spec.lo.x as f64 - 2.0
            && p[0] <= spec.hi.x as f64 + 3.0
            && p[1] >= spec.lo.y as f64 - 2.0
            && p[1] <= spec.hi.y as f64 + 3.0
            && p[2] >= spec.lo.z as f64 - 2.0
            && p[2] <= spec.hi.z as f64 + 3.0
        {
            traps += 1;
        }
    }
    if traps > run.traps_now {
        windows.last_mut().unwrap().traps += traps - run.traps_now;
    }
    run.traps_now = traps;
    let checking = run.broken.is_none();
    // golems
    for g in golems.values_mut() {
        g.seen = false;
    }
    // in a terrain sample, every golem already standing outside the farm at the first sample is
    // the world's (the village's): marked foreign once, wherever it walks later; and so is one
    // first seen in the ring around the farm's near box (box +8) out to box +24 (+16 up and down):
    // it walked in from the village, while a golem the farm spawns appears within its villagers'
    // search, inside the near box, without ever being in the ring
    if !scope.all_golems {
        let (wlo, whi) = (spec.lo.offset(-24, -16, -24), spec.hi.offset(24, 16, 24));
        let (nlo, nhi) = (spec.lo.offset(-8, -8, -8), spec.hi.offset(8, 8, 8));
        let wide = if t <= 20 { String::new() } else { format!(",x={},y={},z={},dx={},dy={},dz={}", wlo.x, wlo.y, wlo.z, whi.x - wlo.x + 1, whi.y - wlo.y + 1, whi.z - wlo.z + 1) };
        for a in entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:iron_golem{wide}] run data get entity @s"))?) {
            let (Some(uuid), Some(pos)) = (list_after(&a, "UUID").map(|v| format!("{v:?}")), list_after(&a, "Pos").filter(|p| p.len() == 3)) else { continue };
            let p = [pos[0], pos[1], pos[2]];
            let outside = p[0] < inner.min_x || p[0] > inner.max_x || p[1] < inner.min_y || p[1] > inner.max_y || p[2] < inner.min_z || p[2] > inner.max_z;
            let in_near = p[0] >= nlo.x as f64 && p[0] < nhi.x as f64 + 1.0 && p[1] >= nlo.y as f64 && p[1] < nhi.y as f64 + 1.0 && p[2] >= nlo.z as f64 && p[2] < nhi.z as f64 + 1.0;
            if outside && (t <= 20 || !in_near) {
                golems.entry(uuid).or_insert(Golem { born: t, first: p, last: p, dead: false, seen: true, reported: true, foreign: true });
            }
        }
    }
    for a in entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:iron_golem{near}] run data get entity @s"))?) {
        let Some(uuid) = list_after(&a, "UUID").map(|v| format!("{v:?}")) else { continue };
        let pos = list_after(&a, "Pos").unwrap_or_default();
        if pos.len() != 3 {
            continue;
        }
        let p = [pos[0], pos[1], pos[2]];
        if golems.get(&uuid).is_some_and(|g| g.foreign) {
            continue;
        }
        // in a terrain sample a golem is the farm's when first seen inside it (the village's golem
        // and wanderers are not); one first seen outside the farm but within a farm villager's
        // golem search (`SpawnUtil.trySpawnMob`: 8 across, 6 up or down), with no village villager
        // within 24 (a village villager's golem spawns 8 from it and walks: D1U's village-edge
        // site saw four in 200 gt at the surface above the farm, the village's) is a golem the
        // farm spawned at a spot it leaves open: the run fails on it, naming the cell and the tick
        // (an escape is not a rate drop)
        let outside = p[0] < inner.min_x || p[0] > inner.max_x || p[1] < inner.min_y || p[1] > inner.max_y || p[2] < inner.min_z || p[2] > inner.max_z;
        if !scope.all_golems && outside && !golems.contains_key(&uuid) {
            let searched = villagers.iter().any(|v| (p[0] - v[0]).abs() <= 9.0 && (p[1] - v[1]).abs() <= 7.0 && (p[2] - v[2]).abs() <= 9.0);
            let village = searched && s.count_entities(&format!("@e[type=minecraft:villager,tag=!mf,x={:.1},y={:.1},z={:.1},dx=48,dy=24,dz=48]", p[0] - 24.0, p[1] - 12.0, p[2] - 24.0))? > 0;
            if searched && !village {
                run.fail(format!("gt {t}: a golem appeared outside the farm at {:.1} {:.1} {:.1}, within a farm villager's golem search (a spawn spot the farm leaves open)", p[0], p[1], p[2]));
            }
            golems.insert(uuid, Golem { born: t, first: p, last: p, dead: false, seen: true, reported: true, foreign: true });
            continue;
        }
        let health = num_after(&a, "Health").unwrap_or(0.0);
        let death = num_after(&a, "DeathTime").unwrap_or(0.0);
        let new = !golems.contains_key(&uuid);
        let g = golems.entry(uuid).or_insert_with(|| {
            run.golems += 1;
            windows.last_mut().unwrap().golems += 1;
            Golem { born: t, first: p, last: p, dead: false, seen: true, reported: false, foreign: false }
        });
        g.seen = true;
        g.last = p;
        // --trace: every farm golem's spawn (first seen) and, while outside the farm, its path
        if let Some(label) = trace {
            if new {
                eprintln!("  {label} gt {t}: golem spawned (first seen) at {:.1} {:.1} {:.1}", p[0], p[1], p[2]);
            } else if outside && !g.dead {
                eprintln!("  {label} gt {t}: golem from gt {} (first seen at {:.1} {:.1} {:.1}) outside the farm at {:.1} {:.1} {:.1}", g.born, g.first[0], g.first[1], g.first[2], p[0], p[1], p[2]);
            }
        }
        if !g.dead && (death > 0.0 || health <= 0.0) {
            g.dead = true;
            run.deaths += 1;
        }
        if !g.dead && !g.reported && checking {
            if p[0] < inner.min_x || p[0] > inner.max_x || p[1] < inner.min_y || p[1] > inner.max_y || p[2] < inner.min_z || p[2] > inner.max_z {
                g.reported = true;
                run.fail(format!("gt {t}: a golem left the farm at {:.1} {:.1} {:.1} (first seen at {:.1} {:.1} {:.1} at gt {})", p[0], p[1], p[2], g.first[0], g.first[1], g.first[2], g.born));
            } else if t - g.born > farm::MAX_GOLEM_LIFE as u32 + 20 {
                g.reported = true;
                run.fail(format!("gt {t}: a golem spawned by gt {} is still alive at {:.1} {:.1} {:.1} (not killed within {} gt)", g.born, p[0], p[1], p[2], farm::MAX_GOLEM_LIFE));
            }
        }
    }
    // a golem no longer listed died (a target selector lists only living entities: a dead golem's
    // 20 death ticks are invisible to `@e`), or left the world; one seen last outside the box was
    // already reported
    let gone: Vec<String> = golems.iter().filter(|(_, g)| !g.seen && !g.dead && !g.foreign).map(|(k, _)| k.clone()).collect();
    for k in gone {
        let g = &golems[&k];
        let p = g.last;
        if p[0] < inner.min_x || p[0] > inner.max_x || p[1] < inner.min_y || p[1] > inner.max_y || p[2] < inner.min_z || p[2] > inner.max_z {
            if checking {
                run.fail(format!("gt {t}: a golem vanished outside the farm at {:.1} {:.1} {:.1} (it left the world)", p[0], p[1], p[2]));
            }
        } else {
            run.deaths += 1;
        }
        golems.remove(&k);
    }
    golems.retain(|_, g| g.seen);
    // the villagers: as many (but the witches), unhurt; the zombie: unhurt in its vehicle; no
    // drop outside the box (until the build breaks)
    if !checking {
        return Ok(());
    }
    let vs = entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:villager{own}] run data get entity @s Health"))?);
    if vs.len() as u32 != villagers0 {
        run.fail(format!("gt {t}: {} villagers of {villagers0}", vs.len()));
    }
    if vs.iter().any(|a| a.trim().trim_end_matches('f').parse::<f64>().unwrap_or(20.0) < 20.0) {
        run.fail(format!("gt {t}: a villager was hurt"));
    }
    // a farm villager a zombie converted (it keeps its tags: in a terrain sample the natural
    // zombie villagers, 5% of the night's zombies, are not the farm's)
    if s.count_entities(&format!("@e[type=minecraft:zombie_villager{own}]"))? > 0 {
        run.fail(format!("gt {t}: a zombie villager in the world"));
    }
    let zs = entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:zombie{own}] run data get entity @s Health"))?);
    if zs.len() as u32 != zombies0 {
        run.fail(format!("gt {t}: {} zombies of {zombies0}", zs.len()));
    }
    if let Some(h) = zs.iter().map(|a| a.trim().trim_end_matches('f').parse::<f64>().unwrap_or(20.0)).find(|h| *h < 20.0) {
        let why = s.cmd_long(&format!("execute as @e[type=minecraft:zombie{own}] run data get entity @s"))?;
        let hurt = why.split("HurtTime").nth(1).map(|r| r.chars().take(8).collect::<String>()).unwrap_or_default();
        let pos = list_after(&why.replace(' ', ""), "Pos").unwrap_or_default();
        run.fail(format!("gt {t}: the zombie was hurt (health {h:.1}; HurtTime{hurt}; at {pos:?})"));
    }
    // drops outside the farm's box
    let (lo, hi) = (spec.lo, spec.hi);
    let all = s.count_entities(&format!("@e[type=minecraft:item{near}]"))?;
    let inside = s.count_entities(&format!("@e[type=minecraft:item,x={},y={},z={},dx={},dy={},dz={}]", lo.x, lo.y, lo.z, hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1))?;
    let item_id = |a: &str| a.split("id: \"").nth(1).or_else(|| a.split("id:\"").nth(1)).and_then(|r| r.split('"').next()).unwrap_or("?").trim_start_matches("minecraft:").to_string();
    let farm_item = |id: &str| matches!(id, "iron_ingot" | "iron_block" | "poppy");
    // in a terrain sample a drop is the farm's when first seen inside the farm (a village golem
    // killed by zombies at night over the farm drops its iron outside: not the farm's; a farm
    // golem dying outside is an escape, failed on its own)
    if !scope.all_golems && inside > 0 {
        for a in entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:item,x={},y={},z={},dx={},dy={},dz={}] run data get entity @s", lo.x, lo.y, lo.z, hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1))?) {
            if farm_item(&item_id(&a))
                && let Some(uuid) = list_after(&a, "UUID").map(|v| format!("{v:?}"))
            {
                run.items_inside.insert(uuid);
            }
        }
    }
    if all > inside {
        // where, and what (one query only when something is out); only the farm's own drops (a
        // golem's iron and poppies) count: a trap horse a bolt left in the void falls and drops
        // its bones far below, which is not a leak of the farm
        let mut out = Vec::new();
        for a in entity_answers(&s.cmd_long(&format!("execute as @e[type=minecraft:item{near}] run data get entity @s"))?) {
            let p = list_after(&a, "Pos").unwrap_or_default();
            if p.len() == 3 && (p[0] < lo.x as f64 || p[0] > hi.x as f64 + 1.0 || p[1] < lo.y as f64 || p[1] > hi.y as f64 + 1.0 || p[2] < lo.z as f64 || p[2] > hi.z as f64 + 1.0) {
                let id = item_id(&a);
                if !farm_item(&id) {
                    continue;
                }
                let uuid = list_after(&a, "UUID").map(|v| format!("{v:?}")).unwrap_or_default();
                if scope.all_golems || run.items_inside.contains(&uuid) {
                    out.push(format!("{id} at {:.1} {:.1} {:.1}", p[0], p[1], p[2]));
                } else if let Some(label) = trace
                    && run.items_foreign.insert(uuid)
                {
                    eprintln!("  {label} gt {t}: an {id} first seen outside the farm at {:.1} {:.1} {:.1} (not the farm's: never inside it)", p[0], p[1], p[2]);
                }
            }
        }
        if !out.is_empty() {
            run.fail(format!("gt {t}: {} of the farm's drops outside the farm ({})", out.len(), out.join("; ")));
        }
    }
    // bolts (seen only if one lives at the sample)
    for a in entity_answers(&s.cmd_long("execute as @e[type=minecraft:lightning_bolt] run data get entity @s Pos")?) {
        if let Some(p) = list_after(&format!("Pos:{}", a.trim()), "Pos")
            && p.len() == 3
        {
            run.bolts_seen.push([p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
        }
    }
    Ok(())
}

/// `circuit verify` and `circuit archive` of a mob farm (crate::mobfarm's `verify`): the redsim
/// judge's report (`rep`, the card), confirmed in the real game: a design the gate fails does not
/// pass (every build failed with the gate's reasons); a pass adds the card's `real game` line.
pub fn confirm(c: &Circuit, version: &str, mut rep: crate::verify::VerifyReport) -> Result<crate::verify::VerifyReport> {
    if !rep.pass {
        return Ok(rep);
    }
    // a design made for terrain (`terrain: cells`, `sample:`) runs in its sample's kept world (the
    // gate's terrain mode; the kept world must be on this box: the gate says where it looks)
    let mut go = GateOpts::new(version);
    if let Some(sample) = crate::mobfarm::terrain_sample(c).map_err(|e| anyhow!(e))? {
        go.terrain = Some(sample.name.to_string());
    }
    let g = gate(c, &go)?;
    if g.passed() {
        for b in rep.builds.iter_mut() {
            if let Ok(k) = b.result.as_mut()
                && k.outcome.pass
            {
                k.outcome.metrics.push(("real game".to_string(), g.line()));
                k.outcome.table.extend(g.builds.iter().map(|b| format!("  game build rotation {} offset {:?}: {} golems, {:.2} iron per dead golem, {}", b.turns, b.offset, b.golems, b.iron() as f64 / b.deaths.max(1) as f64, b.windows.iter().map(|w| format!("{} {:.1}/h", w.name, w.golems_per_hour())).collect::<Vec<_>>().join(", "))));
            }
        }
        return Ok(rep);
    }
    let why = format!("the real game: {}", g.failures.iter().take(3).cloned().collect::<Vec<_>>().join("; "));
    for b in rep.builds.iter_mut() {
        b.result = Err(why.clone());
    }
    Ok(crate::verify::verdict(rep))
}

/// One stretch of the runs, the game's builds against redsim's runs of the same schedule.
#[derive(Clone, Debug, Default)]
pub struct WindowCheck {
    pub name: String,
    /// The builds' mean golems per hour, redsim's, and redsim's interval for the builds' mean
    /// (`z` standard errors of the game's mean and of redsim's).
    pub game: f64,
    pub sim: f64,
    pub interval: (f64, f64),
    pub z: f64,
}

impl WindowCheck {
    pub fn passed(&self) -> bool {
        self.game >= self.interval.0 - 1e-9 && self.game <= self.interval.1 + 1e-9
    }

    fn text(&self) -> String {
        // (no ", " inside: the site splits a card's metrics there); the level is the family's
        // (natural days 95%, forced days 99%: each window's interval is Bonferroni-widened)
        format!("{} {:.1} golems/h (redsim {:.1}; {}% {:.1}-{:.1})", self.name, self.game, self.sim, if self.name.contains("natural") { 95 } else { 99 }, self.interval.0.max(0.0), self.interval.1)
    }
}

/// The gate's verdict over every build.
#[derive(Clone, Debug, Default)]
pub struct GateReport {
    pub builds: Vec<BuildRun>,
    /// Natural weather (95%), then the forced day of rain and of thunder (99% each).
    pub windows: Vec<WindowCheck>,
    pub sim_runs: usize,
    pub failures: Vec<String>,
    /// Lightning's breaks the builds saw against the card's rates.
    pub tradeoffs: Vec<Tradeoff>,
    /// Redsim's delivered iron per dead golem over the reference runs (dropped less burnt) and the
    /// burnt part.
    pub sim_iron: f64,
    pub sim_burnt: f64,
}

impl GateReport {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    /// One line for the card.
    pub fn line(&self) -> String {
        let iron: u32 = self.builds.iter().map(|b| b.iron()).sum();
        let deaths: u32 = self.builds.iter().map(|b| b.deaths).sum();
        let lightning: Vec<&Tradeoff> = self.tradeoffs.iter().filter(|t| t.what != "farm blocks burned").collect();
        let mut tradeoffs = match lightning.first() {
            Some(t) => format!("; lightning over its {:.0} gt of thunder: {}", t.thunder, lightning.iter().map(|t| format!("{} {} (the card {:.2})", t.seen, t.what, t.expected)).collect::<Vec<_>>().join("; ")),
            None => String::new(),
        };
        // (no ", " inside: the site splits a card's metrics there)
        let burnt = if self.sim_burnt > 0.0 { format!(" (redsim {:.2} with {:.2} burnt)", self.sim_iron, self.sim_burnt) } else { String::new() };
        if let Some(t) = self.tradeoffs.iter().find(|t| t.what == "farm blocks burned") {
            tradeoffs.push_str(&format!("; fire: {} farm blocks burned (the card {:.2})", t.seen, t.expected));
        }
        format!(
            "{} builds in the 26.3 server on Hard: {}; {:.2} iron per dead golem{burnt}{tradeoffs}{}",
            self.builds.len(),
            self.windows.iter().map(|w| w.text()).collect::<Vec<_>>().join("; "),
            iron as f64 / deaths.max(1) as f64,
            if self.passed() { "; every build works in rain and thunder as its card says" } else { "" }
        )
    }
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}

/// Redsim's delivered iron per dead golem over runs given as (iron dropped, iron burnt, dead
/// golems), and the per-golem variance pooled from them (each run's delivered mean has variance
/// v / n: v ~ mean of n (x - e)^2), at least the loot's own (3..5 uniform: 2/3); no death: the
/// loot's 4 and 2/3.
fn delivered_per_golem(runs: &[(u32, u32, u32)]) -> (f64, f64) {
    let (dropped, burnt, deaths) = runs.iter().fold((0u64, 0u64, 0u64), |(d, b, n), r| (d + r.0 as u64, b + r.1 as u64, n + r.2 as u64));
    if deaths == 0 {
        return (4.0, 2.0 / 3.0);
    }
    let e = dropped.saturating_sub(burnt) as f64 / deaths as f64;
    let per: Vec<(f64, f64)> = runs.iter().filter(|r| r.2 > 0).map(|r| (r.2 as f64, r.0.saturating_sub(r.1) as f64 / r.2 as f64)).collect();
    let pooled = per.iter().map(|(n, x)| n * (x - e) * (x - e)).sum::<f64>() / per.len().max(1) as f64;
    (e, pooled.max(2.0 / 3.0))
}

fn sd(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// The gate: every build in its own server (`parallel` at once), then the rate against redsim's.
pub fn gate(c: &Circuit, o: &GateOpts) -> Result<GateReport> {
    let builds = match &o.terrain {
        // a terrain sample: one placement (its Site), `runs` runs of it (`parallel` at once; 0:
        // `parallel` runs), or the runs picked
        Some(_) if !o.builds.is_empty() => o.builds.clone(),
        Some(_) => (0..(if o.runs > 0 { o.runs } else { o.parallel.max(1) }).min(256)).map(|k| (k as u8, [0, 0, 0])).collect(),
        None if o.builds.is_empty() => judge_builds(),
        None => o.builds.clone(),
    };
    let jobs: Vec<(u8, [i32; 3], Circuit)> = match &o.terrain {
        Some(_) => builds.iter().map(|(k, off)| (*k, *off, c.clone())).collect(),
        None => builds.iter().map(|(t, off)| -> Result<_> { Ok((*t, *off, c.turned_pure(*t)?.translated(*off)?)) }).collect::<Result<_>>()?,
    };
    let mut results: Vec<Option<Result<BuildRun>>> = (0..jobs.len()).map(|_| None).collect();
    for chunk in (0..jobs.len()).collect::<Vec<_>>().chunks(o.parallel.max(1)) {
        let jobs = &jobs;
        let out: Vec<(usize, Result<BuildRun>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|&k| {
                    (
                        k,
                        sc.spawn(move || match &o.terrain {
                            Some(name) => run_terrain(&jobs[k].2, name, jobs[k].0, o),
                            None => run_build(&jobs[k].2, jobs[k].0, jobs[k].1, o),
                        }),
                    )
                })
                .collect();
            hs.into_iter().map(|(k, h)| (k, h.join().unwrap_or_else(|_| Err(anyhow!("a build panicked"))))).collect()
        });
        for (k, r) in out {
            results[k] = Some(r);
        }
    }
    let mut rep = GateReport::default();
    for (k, r) in results.into_iter().enumerate() {
        match r.unwrap_or_else(|| Err(anyhow!("not run"))) {
            Ok(b) => {
                for f in &b.failures {
                    rep.failures.push(format!("build {} (rotation {}, offset {:?}, seed {}): {f}", k, b.turns, b.offset, b.seed));
                }
                rep.builds.push(b);
            }
            Err(e) => rep.failures.push(format!("build {k}: the game run failed: {e:#}")),
        }
    }
    // the rates: every window against redsim's runs of the same schedule (the judge's own runs,
    // farm::run, from the cycle's stationary state, with the same forced windows; in a terrain
    // sample at its Site)
    let spec = crate::mobfarm::spec_of(c).map_err(|e| anyhow!(e))?;
    let site: Option<&'static redsim::entity::mob::site::Site> = match &o.terrain {
        Some(name) => Some(Box::leak(Box::new(crate::mobfarm::site_of(c, &spec, name).map_err(|e| anyhow!(e))?))),
        None => None,
    };
    let spec = match site {
        Some(s) => s.spec(&spec),
        None => spec,
    };
    let day = 24000i64;
    let natural = o.days as i64 * day;
    let plan = match (o.rain_day, o.thunder_day) {
        (true, true) => Plan::Windows { at: natural, rain: day as i32, thunder: day as i32 },
        (true, false) => Plan::Force { at: natural, cmd: redsim::weather::Command::Rain(day as i32) },
        (false, true) => Plan::Force { at: natural, cmd: redsim::weather::Command::Thunder(day as i32) },
        (false, false) => Plan::Cycle,
    };
    // Bonferroni over the windows the gate checks (ironfarm 2026-11-08: three windows at 95/99/99%
    // refused about 7% of good farms by chance; a big farm's tight intervals made it bite): the
    // natural days at a family-wise 95%, each forced day at a family-wise 99%, split over the k
    // windows (two-sided z of 0.05/k and 0.01/k)
    let k = 1 + o.rain_day as usize + o.thunder_day as usize;
    let (z_natural, z_forced) = [(1.960, 2.576), (2.241, 2.807), (2.394, 2.935)][k - 1];
    let mut spans: Vec<(String, i64, i64, f64)> = vec![(format!("{} days of natural weather", o.days), 0, natural, z_natural)];
    if o.rain_day {
        spans.push(("a forced day of rain".into(), natural, natural + day, z_forced));
    }
    if o.thunder_day {
        let a = natural + if o.rain_day { day } else { 0 };
        spans.push(("a forced day of thunder".into(), a, a + day, z_forced));
    }
    let total = spans.last().map(|s| s.2).unwrap_or(natural);
    let sims: Vec<RunOpts> = (0..64)
        .map(|k| {
            let mut r = RunOpts::steady(0x6761_7465 + k, total as u32, WalkMode::Random).with_weather(plan);
            r.site = site;
            r
        })
        .collect();
    let mut sim_runs: Vec<farm::RunReport> = Vec::new();
    for (k, r) in crate::mobfarm::run_all(&spec, &sims).into_iter().enumerate() {
        match r {
            Ok(r) => sim_runs.push(r),
            // never a run silently dropped from the interval
            Err(e) => rep.failures.push(format!("redsim's reference run {k}: {e}")),
        }
    }
    rep.sim_runs = sim_runs.len();
    // the iron each build delivered (chest, containers, floor, less the design's own) against
    // redsim's delivered iron per golem over the same schedule (each run's iron dropped less the
    // iron its drops lost to lava or fire, `RunReport::iron_burnt`, per dead golem: brief rev 9
    // iteration 11, burnt iron is a measured loss, not a refusal), at least that many per dead
    // golem minus 3 standard deviations (the per-golem spread pooled from the runs, at least the
    // loot's own 3..5 uniform): a leak shows, the farm's own burns do not
    let (dropped, burnt, deaths) = sim_runs.iter().fold((0u64, 0u64, 0u64), |(d, b, n), r| (d + r.iron_dropped as u64, b + r.iron_burnt as u64, n + r.deaths as u64));
    let (expect, spread) = delivered_per_golem(&sim_runs.iter().map(|r| (r.iron_dropped, r.iron_burnt, r.deaths)).collect::<Vec<_>>());
    rep.sim_iron = expect;
    rep.sim_burnt = if deaths > 0 { burnt as f64 / deaths as f64 } else { 0.0 };
    for (k, b) in rep.builds.iter_mut().enumerate() {
        if b.deaths > 0 && b.broken.is_none() {
            let n = b.deaths as f64;
            let floor = expect * n - 3.0 * (spread * n).sqrt();
            if (b.iron() as f64) < floor {
                let why = format!(
                    "{} iron ingots for {} dead golems ({:.2} each; redsim delivers {expect:.2} per golem ({:.2} dropped less {:.2} burnt), at least {floor:.0} within 3 standard deviations): drops were lost",
                    b.iron(),
                    b.deaths,
                    b.iron() as f64 / n,
                    if deaths > 0 { dropped as f64 / deaths as f64 } else { 4.0 },
                    rep.sim_burnt
                );
                rep.failures.push(format!("build {} (rotation {}, offset {:?}, seed {}): {why}", k, b.turns, b.offset, b.seed));
                b.failures.push(why);
            }
        }
    }
    let arrive = farm::SETTLE_TICKS;
    for (k, (name, a, b, z)) in spans.iter().enumerate() {
        let per_hour = |n: usize| n as f64 * 72000.0 / (b - a) as f64;
        let sim: Vec<f64> = sim_runs.iter().map(|r| per_hour(r.spawn_times.iter().filter(|t| (arrive + a..arrive + b).contains(*t)).count())).collect();
        let game: Vec<f64> = rep.builds.iter().filter(|bu| bu.broken.is_none_or(|b| b > k)).filter_map(|bu| bu.windows.get(k)).map(|w| w.golems_per_hour()).collect();
        let (m, s) = (mean(&sim), sd(&sim));
        let half = z * (s * s / game.len().max(1) as f64 + s * s / sim.len().max(1) as f64).sqrt();
        let w = WindowCheck { name: name.clone(), game: mean(&game), sim: m, interval: (m - half, m + half), z: *z };
        if !game.is_empty() && !w.passed() {
            rep.failures.push(format!("{}: the game's {:.1} golems/h is outside redsim's {}% interval {:.1}-{:.1} for {} builds", name, w.game, if *z > 2.0 { 99 } else { 95 }, w.interval.0, w.interval.1, game.len()));
        }
        rep.windows.push(w);
    }
    // the card's tradeoffs the game can show (vjeux 2026-11-08: "It's all about tradeoffs"):
    // lightning's breaks over the thunder the game's builds saw, against the card's rates
    // (`lightning::measure_farm` over redsim's runs of the same schedule), 99% Poisson; rates the
    // model cannot measure fail the gate (never a check silently skipped)
    match redsim::weather::lightning::measure_farm(&spec, &sim_runs) {
        Ok((m, _)) => {
            let thunder: f64 = rep.builds.iter().flat_map(|b| b.windows.iter()).map(|w| w.thunder_samples as f64 * 20.0).sum();
            let seen = |f: fn(&Window) -> u32| rep.builds.iter().flat_map(|b| b.windows.iter()).map(f).sum::<u32>();
            let villagers = seen(|w| w.witch_villagers);
            let witches = if villagers > 0 { format!("witch-making bolts ({villagers} villagers made witches)") } else { "witch-making bolts".to_string() };
            let checks = [(witches, m.witch, seen(|w| w.witches)), ("boats broken".to_string(), m.vehicle, seen(|w| w.vehicles)), ("skeleton trap horses on the farm".to_string(), m.trap, seen(|w| w.traps))];
            for (what, rate, n) in checks {
                let lambda = rate * thunder;
                let (lo, hi) = poisson_band(lambda, 0.005);
                if n < lo || n > hi {
                    rep.failures.push(format!("lightning: {n} {what} over the game's {thunder:.0} gt of thunder; the card expects {lambda:.2} (99% {lo}-{hi})"));
                }
                rep.tradeoffs.push(Tradeoff { what, seen: n, expected: lambda, thunder });
            }
        }
        Err(e) => rep.failures.push(format!("the card's lightning rates: {e}")),
    }
    // the farm's blocks a fire burned (the parent, 2026-11-08): a measured event, the builds'
    // count against redsim's runs of the same schedule (`WeatherStats::burns`, the card's `fire`
    // line), 99% Poisson over the game ticks the builds ran; a burn the card says never happens
    // still fails (its expected count is 0)
    {
        let sim_ticks: f64 = sim_runs.iter().map(|r| r.hours() * 72000.0).sum();
        let sim_burns: f64 = sim_runs.iter().map(|r| r.weather.burns as f64).sum();
        let game_ticks: f64 = rep.builds.iter().map(|b| b.windows.iter().map(|w| w.ticks as f64).sum::<f64>()).sum();
        let seen: u32 = rep.builds.iter().map(|b| b.burned).sum();
        let lambda = if sim_ticks > 0.0 { sim_burns / sim_ticks * game_ticks } else { 0.0 };
        let (lo, hi) = poisson_band(lambda, 0.005);
        if seen < lo || seen > hi {
            rep.failures.push(format!("fire: {seen} of the farm's blocks burned over the builds' {game_ticks:.0} gt; the card expects {lambda:.2} (99% {lo}-{hi})"));
        }
        rep.tradeoffs.push(Tradeoff { what: "farm blocks burned".to_string(), seen, expected: lambda, thunder: 0.0 });
    }
    Ok(rep)
}

/// A tradeoff of the card the gate saw in the game: how many, and how many the card expects over
/// the same thunder.
#[derive(Clone, Debug, Default)]
pub struct Tradeoff {
    pub what: String,
    pub seen: u32,
    pub expected: f64,
    pub thunder: f64,
}

/// The counts a Poisson(`lambda`) variable stays within but for `alpha` on each side: the
/// largest `lo` with P(X < lo) <= alpha and the smallest `hi` with P(X > hi) <= alpha.
pub fn poisson_band(lambda: f64, alpha: f64) -> (u32, u32) {
    if lambda <= 0.0 {
        return (0, 0);
    }
    let (mut pmf, mut cdf) = ((-lambda).exp(), 0.0f64);
    let (mut lo, mut hi) = (0u32, 0u32);
    let mut k = 0u32;
    loop {
        // cdf = P(X < k) before adding k's mass
        if cdf <= alpha {
            lo = k;
        }
        cdf += pmf;
        if 1.0 - cdf <= alpha {
            hi = k;
            break;
        }
        k += 1;
        pmf *= lambda / k as f64;
        if k > 100_000 {
            hi = k;
            break;
        }
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The iron of a chest as ingots: a crafted block counts 9 (ironfarm's row farm).
    #[test]
    fn iron_blocks_count_nine() {
        let items = r#"[{count: 5, Slot: 0b, id: "minecraft:iron_ingot"}, {count: 2, Slot: 1b, id: "minecraft:iron_block"}, {count: 3, Slot: 2b, id: "minecraft:poppy"}]"#;
        assert_eq!(count_iron(items), 5 + 18);
        assert_eq!(count_item(items, "minecraft:poppy"), 3);
    }

    /// Redsim's delivered iron per golem: the loot's 4 less the burnt iron; the spread at least the
    /// loot's own, wider when burns make some golems deliver nothing.
    #[test]
    fn delivered_iron_less_the_burnt() {
        // no burn: 4 per golem, the loot's variance
        let (e, v) = delivered_per_golem(&[(400, 0, 100), (404, 0, 101)]);
        assert!((e - 4.0).abs() < 1e-9 && (v - 2.0 / 3.0).abs() < 1e-9, "{e} {v}");
        // 1 golem in 25 burnt (its 4 ingots): 3.84 per golem
        let (e, _) = delivered_per_golem(&[(400, 16, 100)]);
        assert!((e - 3.84).abs() < 1e-9, "{e}");
        // runs that differ a lot: a wider spread than the loot's
        let (_, v) = delivered_per_golem(&[(400, 0, 100), (400, 80, 100)]);
        assert!(v > 2.0 / 3.0, "{v}");
        // no golem died anywhere
        assert_eq!(delivered_per_golem(&[(0, 0, 0)]), (4.0, 2.0 / 3.0));
    }

    /// The game's read-back of a merged block entity holds the design's fields (its own format:
    /// spaces, typed arrays, ids with their namespace, more fields), and a lost field is caught.
    #[test]
    fn block_entities_read_back() {
        let back = "34, -27, 0 has the following block data: {crafting_ticks_remaining: 0, disabled_slots: [I; 0, 1, 2, 3, 4, 5, 6, 7, 8], triggered: 0, Items: []}";
        check_block_entity(back, "{disabled_slots:[I;0,1,2,3,4,5,6,7,8]}").unwrap();
        assert!(check_block_entity(back, "{disabled_slots:[I;0,1,2]}").is_err());
        let hopper = r#"34, -25, 1 has the following block data: {TransferCooldown: 0, Items: [{count: 64, Slot: 0b, id: "minecraft:iron_ingot"}, {components: {"minecraft:custom_name": "iron lock"}, count: 1, Slot: 1b, id: "minecraft:paper"}]}"#;
        check_block_entity(hopper, r#"{Items:[{Slot:1b,id:"minecraft:paper",count:1,components:{"minecraft:custom_name":"iron lock"}},{Slot:0b,id:"iron_ingot",count:64}]}"#).unwrap();
        assert!(check_block_entity("34, -25, 1 has the following block data: {Items: []}", r#"{Items:[{Slot:0b,id:"iron_ingot",count:64}]}"#).is_err());
    }

    /// A vehicle's riders split from its data, a turned build's Rotation kept.
    #[test]
    fn passengers_split_from_the_vehicle() {
        assert_eq!(split_passengers("{Passengers:[{id:\"minecraft:zombie\"}]}"), Some((vec!["minecraft:zombie".to_string()], "{}".to_string())));
        assert_eq!(
            split_passengers("{Passengers:[{id:\"minecraft:zombie\"}],Rotation:[90.0f,0.0f]}"),
            Some((vec!["minecraft:zombie".to_string()], "{Rotation:[90.0f,0.0f]}".to_string()))
        );
        assert_eq!(split_passengers("{Rotation:[90.0f,0.0f]}"), None);
        // a rider with data of its own: summoned as written
        assert_eq!(split_passengers("{Passengers:[{id:\"minecraft:zombie\",IsBaby:1b}]}"), None);
        assert_eq!(with_tag("{}", "t"), "{Tags:[\"t\"]}");
        assert_eq!(with_tag("{Rotation:[90.0f,0.0f]}", "t"), "{Tags:[\"t\"],Rotation:[90.0f,0.0f]}");
        assert_eq!(with_tag(&with_tag("{Rotation:[90.0f,0.0f]}", "a"), "mf"), "{Tags:[\"mf\",\"a\"],Rotation:[90.0f,0.0f]}");
    }
}
