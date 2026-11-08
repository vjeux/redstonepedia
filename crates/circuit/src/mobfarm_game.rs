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
//! outside the box, and at the end the iron in the farm's containers and on its floor is the 4
//! ingots a golem drops on average for every golem that died (one-sided, 3 standard deviations of
//! the loot's uniform 3..5: a few lost ingots hide in the loot's own spread, a leak does not).
//!
//! **The rates**: each window's golems per hour, the builds' mean, must fall within redsim's
//! interval for that many builds under the same schedule: the judge's own runs (`farm::run` from
//! the cycle's stationary state, `Plan::Windows` with the same forced days, 64 runs), their mean
//! plus or minus z standard errors of the game's mean and of redsim's (z = 1.96 for the natural
//! days, 2.576 for each forced day: three tests, about 3% of good farms refused by chance).

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
}

impl GateOpts {
    pub fn new(version: &str) -> GateOpts {
        GateOpts { version: version.to_string(), days: 3, rain_day: true, thunder_day: true, warmup: (150_000, 400_000), seed: 1, builds: Vec::new(), parallel: 8, keep: false, trace: false }
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
    pub windows: Vec<Window>,
    pub golems: u32,
    pub deaths: u32,
    /// Iron in the chest, in the farm's other containers, loose in the world; poppies in the chest.
    pub iron_chest: u32,
    pub iron_containers: u32,
    pub iron_loose: u32,
    pub poppies: u32,
    /// Bolts seen at a sample (a bolt lives 2..30 gt: most are missed).
    pub bolts_seen: Vec<[i32; 3]>,
    /// Cells of the build whose block changed: a watched one (flammable or lava) when it changed
    /// (every 1000 gt), every cell at the end.
    pub changed: Vec<String>,
    pub failures: Vec<String>,
    pub seconds: f64,
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

    pub fn iron(&self) -> u32 {
        self.iron_chest + self.iron_containers + self.iron_loose
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

/// A golem as the gate tracks it.
#[derive(Clone, Debug)]
struct Golem {
    born: u32,
    last: [f64; 3],
    dead: bool,
    seen: bool,
    /// Already reported (each golem's failure once).
    reported: bool,
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

/// The farm's blocks as the redsim judge builds them (`farm::Run::start` and its water's settle;
/// the daylight detectors held back), as `setblock` states, and the detectors.
/// The farm as the redsim judge builds it: its blocks after its water settled (`farm::Run::start`
/// and its settle; the daylight detectors held back) as `setblock` states, the detectors, and the
/// cells watched as it runs (flammable blocks and lava: a fire or a flow breaks the farm).
struct Built {
    blocks: Vec<([i32; 3], String)>,
    detectors: Vec<([i32; 3], String)>,
    watch: Vec<([i32; 3], String)>,
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
    Ok(Built { blocks, detectors, watch })
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

/// The game run of one build: `c` turned and moved.
pub fn run_build(c: &Circuit, turns: u8, offset: [i32; 3], o: &GateOpts) -> Result<BuildRun> {
    let t0 = std::time::Instant::now();
    let spec = crate::mobfarm::spec_of(c).map_err(|e| anyhow!(e))?;
    let seed = o.seed.wrapping_mul(1000) + turns as i64 * 10 + (offset[0] & 7) as i64;
    let Built { blocks, detectors, mut watch } = built_blocks(&spec, seed)?;
    let output = [spec.output.x, spec.output.y, spec.output.z];
    let boxes = containers(&blocks, output);
    let (mut s, _bot) = start_server(o, &spec)?;
    let mut run = BuildRun { turns, offset, ..Default::default() };
    // the weather toward its long-run mix, the farm not built yet
    let mut r = redsim::rng::LegacyRandom::new(seed ^ 0x6761_7465);
    let warm = o.warmup.0 + r.next_int((o.warmup.1 - o.warmup.0).max(1) as i32) as u32;
    step(&mut s, warm)?;
    for (p, st) in &blocks {
        let out = s.cmd(&format!("setblock {} {st} strict", xyz(*p)))?;
        if !out.contains("Changed the block") {
            bail!("setblock {p:?} {st}: {out}");
        }
    }
    // the judge's steady state: a day time in the day, LAST_SLEPT at the end of the last rest
    let day0 = RunOpts::steady(seed, 0, WalkMode::Random).day0;
    s.cmd_ok(&format!("time set {day0}"))?;
    for (p, st) in &detectors {
        s.cmd_ok(&format!("setblock {} {st}", xyz(*p)))?;
    }
    let now = s.game_time()?;
    let slept = now - (day0 - 10);
    let d = &c.clock.design;
    let mut villager_spots: Vec<[f64; 3]> = Vec::new();
    for e in &d.entities {
        let k = e.kind.trim_start_matches("minecraft:");
        let [x, y, z] = e.pos;
        let cmd = match (k, e.nbt.as_deref()) {
            ("villager", _) => {
                villager_spots.push(e.pos);
                format!("summon minecraft:villager {x} {y} {z} {{PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:last_slept\":{{value:{slept}L}}}}}}}}")
            }
            (_, Some(n)) => format!("summon minecraft:{k} {x} {y} {z} {n}"),
            (_, None) => format!("summon minecraft:{k} {x} {y} {z}"),
        };
        s.cmd_ok(&cmd).with_context(|| format!("summon {k}"))?;
    }
    let zombies0 = s.count_entities("@e[type=minecraft:zombie]")?;
    let vehicles0 = s.count_entities("@e[type=#minecraft:boat]")? + s.count_entities("@e[type=minecraft:minecart]")?;
    let villagers0 = villager_spots.len() as u32;
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
            sample(&mut s, &mut run, &mut golems, t, k, inner, &spec, villagers0, zombies0, vehicles0, &mut windows)?;
            // the flammable blocks and the lava: burned away or flowed
            if t % 1000 == 0 {
                let mut still = Vec::new();
                for (p, id) in watch.drain(..) {
                    if s.cmd(&format!("execute if block {} {id}", xyz(p)))?.contains("passed") {
                        still.push((p, id));
                    } else {
                        let now = now_at(&mut s, p)?;
                        let what = format!("gt {t}: the {} at {} is {now} (burned or flowed)", id.trim_start_matches("minecraft:"), xyz(p));
                        run.changed.push(what.clone());
                        run.fail(what);
                    }
                }
                watch = still;
            }
            if o.trace && t % 1000 == 0 {
                eprintln!("build {turns} {offset:?} gt {t}: {} golems, {} dead, failures {:?}", run.golems, run.deaths, run.failures);
                if t % 4000 == 0 {
                    let mem = s.cmd_long("execute as @e[type=minecraft:villager] run data get entity @s Brain.memories")?;
                    eprintln!("  villager memories: {}", mem.chars().take(1500).collect::<String>());
                    let z = s.cmd_long("execute as @e[type=minecraft:zombie] run data get entity @s Health")?;
                    eprintln!("  zombie: {z}");
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
    // the drops: the chest, the farm's containers, the items on the floor
    run.iron_chest = count_item(&s.cmd_long(&format!("data get block {} Items", xyz(output)))?, "minecraft:iron_ingot");
    run.poppies = count_item(&s.cmd_long(&format!("data get block {} Items", xyz(output)))?, "minecraft:poppy");
    for p in &boxes {
        run.iron_containers += count_item(&s.cmd_long(&format!("data get block {} Items", xyz(*p)))?, "minecraft:iron_ingot");
    }
    run.iron_loose = count_item(&s.cmd_long("execute as @e[type=minecraft:item] run data get entity @s Item")?, "minecraft:iron_ingot");
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
    if run.deaths > 0 {
        let n = run.deaths as f64;
        let floor = 4.0 * n - 3.0 * (2.0 * n / 3.0).sqrt();
        if (run.iron() as f64) < floor {
            run.fail(format!("{} iron ingots for {} dead golems ({:.2} each; the loot gives 4, and at least {floor:.0} within 3 standard deviations): drops were lost", run.iron(), run.deaths, run.iron() as f64 / n));
        }
    }
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
    _window: usize,
    inner: redsim::entity::Aabb,
    spec: &FarmSpec,
    villagers0: u32,
    zombies0: u32,
    vehicles0: u32,
    windows: &mut [Window],
) -> Result<()> {
    let w = windows.last_mut().unwrap();
    let rain = s.cmd("execute if predicate {type:\"minecraft:weather_check\",raining:true}")?;
    if rain.contains("passed") {
        w.rain_samples += 1;
    }
    if s.cmd("execute if predicate {type:\"minecraft:weather_check\",thundering:true}")?.contains("passed") {
        w.thunder_samples += 1;
    }
    // golems
    for g in golems.values_mut() {
        g.seen = false;
    }
    for a in entity_answers(&s.cmd_long("execute as @e[type=minecraft:iron_golem] run data get entity @s")?) {
        let Some(uuid) = list_after(&a, "UUID").map(|v| format!("{v:?}")) else { continue };
        let pos = list_after(&a, "Pos").unwrap_or_default();
        if pos.len() != 3 {
            continue;
        }
        let p = [pos[0], pos[1], pos[2]];
        let health = num_after(&a, "Health").unwrap_or(0.0);
        let death = num_after(&a, "DeathTime").unwrap_or(0.0);
        let g = golems.entry(uuid).or_insert_with(|| {
            run.golems += 1;
            windows.last_mut().unwrap().golems += 1;
            Golem { born: t, last: p, dead: false, seen: true, reported: false }
        });
        g.seen = true;
        g.last = p;
        if !g.dead && (death > 0.0 || health <= 0.0) {
            g.dead = true;
            run.deaths += 1;
        }
        if !g.dead && !g.reported {
            if p[0] < inner.min_x || p[0] > inner.max_x || p[1] < inner.min_y || p[1] > inner.max_y || p[2] < inner.min_z || p[2] > inner.max_z {
                g.reported = true;
                run.fail(format!("gt {t}: a golem left the farm at {:.1} {:.1} {:.1}", p[0], p[1], p[2]));
            } else if t - g.born > farm::MAX_GOLEM_LIFE as u32 + 20 {
                g.reported = true;
                run.fail(format!("gt {t}: a golem spawned by gt {} is still alive at {:.1} {:.1} {:.1} (not killed within {} gt)", g.born, p[0], p[1], p[2], farm::MAX_GOLEM_LIFE));
            }
        }
    }
    // a golem no longer listed died (a target selector lists only living entities: a dead golem's
    // 20 death ticks are invisible to `@e`), or left the world; one seen last outside the box was
    // already reported
    let gone: Vec<String> = golems.iter().filter(|(_, g)| !g.seen && !g.dead).map(|(k, _)| k.clone()).collect();
    for k in gone {
        let g = &golems[&k];
        let p = g.last;
        if p[0] < inner.min_x || p[0] > inner.max_x || p[1] < inner.min_y || p[1] > inner.max_y || p[2] < inner.min_z || p[2] > inner.max_z {
            run.fail(format!("gt {t}: a golem vanished outside the farm at {:.1} {:.1} {:.1} (it left the world)", p[0], p[1], p[2]));
        } else {
            run.deaths += 1;
        }
        golems.remove(&k);
    }
    golems.retain(|_, g| g.seen);
    // the villagers: as many, unhurt, unconverted
    let vs = entity_answers(&s.cmd_long("execute as @e[type=minecraft:villager] run data get entity @s Health")?);
    if vs.len() as u32 != villagers0 {
        run.fail(format!("gt {t}: {} villagers of {villagers0}", vs.len()));
    }
    if vs.iter().any(|a| a.trim().trim_end_matches('f').parse::<f64>().unwrap_or(20.0) < 20.0) {
        run.fail(format!("gt {t}: a villager was hurt"));
    }
    for (ty, what) in [("witch", "a witch (a villager struck by lightning)"), ("zombie_villager", "a zombie villager")] {
        if s.count_entities(&format!("@e[type=minecraft:{ty}]"))? > 0 {
            run.fail(format!("gt {t}: {what} in the world"));
        }
    }
    // a trap horse on the farm (one that fell where a bolt hit the void is harmless)
    for a in entity_answers(&s.cmd_long("execute as @e[type=minecraft:skeleton_horse] run data get entity @s Pos")?) {
        if let Some(p) = list_after(&format!("Pos:{}", a.trim()), "Pos")
            && p.len() == 3
            && p[0] >= spec.lo.x as f64 - 2.0
            && p[0] <= spec.hi.x as f64 + 3.0
            && p[1] >= spec.lo.y as f64 - 2.0
            && p[1] <= spec.hi.y as f64 + 3.0
            && p[2] >= spec.lo.z as f64 - 2.0
            && p[2] <= spec.hi.z as f64 + 3.0
        {
            run.fail(format!("gt {t}: a skeleton trap horse on the farm at {:.1} {:.1} {:.1}", p[0], p[1], p[2]));
        }
    }
    // the zombie: unhurt, in its vehicle
    let zs = entity_answers(&s.cmd_long("execute as @e[type=minecraft:zombie] run data get entity @s Health")?);
    if zs.len() as u32 != zombies0 {
        run.fail(format!("gt {t}: {} zombies of {zombies0}", zs.len()));
    }
    if zs.iter().any(|a| a.trim().trim_end_matches('f').parse::<f64>().unwrap_or(20.0) < 20.0) {
        run.fail(format!("gt {t}: the zombie was hurt"));
    }
    let vehicles = s.count_entities("@e[type=#minecraft:boat]")? + s.count_entities("@e[type=minecraft:minecart]")?;
    if vehicles != vehicles0 {
        run.fail(format!("gt {t}: {vehicles} vehicles of {vehicles0} (one was destroyed)"));
    }
    // drops outside the farm's box
    let (lo, hi) = (spec.lo, spec.hi);
    let all = s.count_entities("@e[type=minecraft:item]")?;
    let inside = s.count_entities(&format!("@e[type=minecraft:item,x={},y={},z={},dx={},dy={},dz={}]", lo.x, lo.y, lo.z, hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1))?;
    if all > inside {
        // where, and what (one query only when something is out)
        let mut out = Vec::new();
        for a in entity_answers(&s.cmd_long("execute as @e[type=minecraft:item] run data get entity @s")?) {
            let p = list_after(&a, "Pos").unwrap_or_default();
            if p.len() == 3 && (p[0] < lo.x as f64 || p[0] > hi.x as f64 + 1.0 || p[1] < lo.y as f64 || p[1] > hi.y as f64 + 1.0 || p[2] < lo.z as f64 || p[2] > hi.z as f64 + 1.0) {
                let id = a.split("id: \"").nth(1).or_else(|| a.split("id:\"").nth(1)).and_then(|r| r.split('"').next()).unwrap_or("?").trim_start_matches("minecraft:").to_string();
                out.push(format!("{id} at {:.1} {:.1} {:.1}", p[0], p[1], p[2]));
            }
        }
        run.fail(format!("gt {t}: {} drops outside the farm ({})", all - inside, out.join("; ")));
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
    let g = gate(c, &GateOpts::new(version))?;
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
        format!("{} {:.1} golems/h (redsim {:.1}, {}% {:.1}-{:.1})", self.name, self.game, self.sim, if self.z > 2.0 { 99 } else { 95 }, self.interval.0.max(0.0), self.interval.1)
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
}

impl GateReport {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }

    /// One line for the card.
    pub fn line(&self) -> String {
        let iron: u32 = self.builds.iter().map(|b| b.iron()).sum();
        let deaths: u32 = self.builds.iter().map(|b| b.deaths).sum();
        format!(
            "{} builds in the 26.3 server on Hard: {}; {:.2} iron per dead golem{}",
            self.builds.len(),
            self.windows.iter().map(|w| w.text()).collect::<Vec<_>>().join(", "),
            iron as f64 / deaths.max(1) as f64,
            if self.passed() { "; every build works in rain and thunder" } else { "" }
        )
    }
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
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
    let builds = if o.builds.is_empty() { judge_builds() } else { o.builds.clone() };
    let jobs: Vec<(u8, [i32; 3], Circuit)> = builds.iter().map(|(t, off)| -> Result<_> { Ok((*t, *off, c.turned_pure(*t)?.translated(*off)?)) }).collect::<Result<_>>()?;
    let mut results: Vec<Option<Result<BuildRun>>> = (0..jobs.len()).map(|_| None).collect();
    for chunk in (0..jobs.len()).collect::<Vec<_>>().chunks(o.parallel.max(1)) {
        let jobs = &jobs;
        let out: Vec<(usize, Result<BuildRun>)> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk.iter().map(|&k| (k, sc.spawn(move || run_build(&jobs[k].2, jobs[k].0, jobs[k].1, o)))).collect();
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
                    rep.failures.push(format!("build {} (rotation {}, offset {:?}): {f}", k, b.turns, b.offset));
                }
                rep.builds.push(b);
            }
            Err(e) => rep.failures.push(format!("build {k}: the game run failed: {e:#}")),
        }
    }
    // the rates: every window against redsim's runs of the same schedule (the judge's own runs,
    // farm::run, from the cycle's stationary state, with the same forced windows)
    let spec = crate::mobfarm::spec_of(c).map_err(|e| anyhow!(e))?;
    let day = 24000i64;
    let natural = o.days as i64 * day;
    let plan = match (o.rain_day, o.thunder_day) {
        (true, true) => Plan::Windows { at: natural, rain: day as i32, thunder: day as i32 },
        (true, false) => Plan::Force { at: natural, cmd: redsim::weather::Command::Rain(day as i32) },
        (false, true) => Plan::Force { at: natural, cmd: redsim::weather::Command::Thunder(day as i32) },
        (false, false) => Plan::Cycle,
    };
    let mut spans: Vec<(String, i64, i64, f64)> = vec![(format!("{} days of natural weather", o.days), 0, natural, 1.96)];
    if o.rain_day {
        spans.push(("a forced day of rain".into(), natural, natural + day, 2.576));
    }
    if o.thunder_day {
        let a = natural + if o.rain_day { day } else { 0 };
        spans.push(("a forced day of thunder".into(), a, a + day, 2.576));
    }
    let total = spans.last().map(|s| s.2).unwrap_or(natural);
    let sims: Vec<RunOpts> = (0..64).map(|k| RunOpts::steady(0x6761_7465 + k, total as u32, WalkMode::Random).with_weather(plan)).collect();
    let sim_runs: Vec<farm::RunReport> = crate::mobfarm::run_all(&spec, &sims).into_iter().filter_map(|r| r.ok()).collect();
    rep.sim_runs = sim_runs.len();
    let arrive = farm::SETTLE_TICKS;
    for (k, (name, a, b, z)) in spans.iter().enumerate() {
        let per_hour = |n: usize| n as f64 * 72000.0 / (b - a) as f64;
        let sim: Vec<f64> = sim_runs.iter().map(|r| per_hour(r.spawn_times.iter().filter(|t| (arrive + a..arrive + b).contains(*t)).count())).collect();
        let game: Vec<f64> = rep.builds.iter().filter_map(|bu| bu.windows.get(k)).map(|w| w.golems_per_hour()).collect();
        let (m, s) = (mean(&sim), sd(&sim));
        let half = z * (s * s / game.len().max(1) as f64 + s * s / sim.len().max(1) as f64).sqrt();
        let w = WindowCheck { name: name.clone(), game: mean(&game), sim: m, interval: (m - half, m + half), z: *z };
        if !game.is_empty() && !w.passed() {
            rep.failures.push(format!("{}: the game's {:.1} golems/h is outside redsim's {}% interval {:.1}-{:.1} for {} builds", name, w.game, if *z > 2.0 { 99 } else { 95 }, w.interval.0, w.interval.1, game.len()));
        }
        rep.windows.push(w);
    }
    Ok(rep)
}
