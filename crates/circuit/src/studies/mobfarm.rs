//! Mob farms (the ironfarm track, 2026-11-08; brief `designs/briefs/farm-iron.md`, the work in
//! `designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`): kind `mob-farm <item>`, a farm whose
//! product drops from mobs. The first product is `iron_ingot`: iron golems that villagers summon
//! when a zombie scares them, killed and collected into the judge's chest at `port output`.
//!
//! Judged on redsim's mob layer (`redsim::entity::mob`; vjeux 2026-11-07: "Please implement
//! everything you need on redsim. We can't use the game code, it's way too slow"): `circuit sim`
//! is that judge; `circuit verify` (so `circuit archive`) also runs every build in the real game
//! (mobinfra 2026-11-08, `crate::mobfarm_game`: the 26.3 server, days of weather, a forced day of
//! rain and of thunder, the must-haves and the rate against redsim's), and a farm the game fails
//! does not pass.
//!
//! * Before the runs, on the base build: the refusals (`mob::refuse`), the golem exit flood
//!   (`mob::exit`: from every position a golem can stand on, still and 8 inputs must kill it
//!   within 1200 gt), the 9 worst-policy runs on one seed, and the restart probe for a design
//!   that claims it (`restart: yes`).
//! * Each build (4 rotations x 2 positions) runs [`JUDGE_SEEDS`] seeds of [`JUDGE_DAYS`] game
//!   days from the steady state (a day time in the day, `LAST_SLEPT` at the end of the last
//!   REST), each golem walking under a random policy; every must-have is checked as it runs
//!   (`mob::farm`).
//! * The card's rate is the mean over every build's runs, with its 95% interval. Builds agree
//!   when their own means are within 20% (`Agree::Rate`).
//!
//! Metrics: iron per hour per block (the board's rank), iron per hour, golems per hour, seconds
//! per golem, the share of the ceiling (one golem per 600 gt per villager cluster), the worst
//! policy's rate, the misses, the exit flood, the bill (iron, quartz, villagers, beds) and the
//! payback.

use crate::check::Outcome;
use crate::registry::{Agree, KindDef, RunAt, Site, StudyDef, Twin};
use crate::stimulus::Schedule;
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Result, anyhow, bail};
use mctas::sim::Pool;
use redsim::entity::mob::absence::Away;
use redsim::entity::mob::exit::{self, ExitReport};
use redsim::entity::mob::farm::{self, AwayPlan, AwayReport, FarmSpec, RunOpts, RunReport, WalkMode};
use redsim::entity::mob::walk::Walk;
use redsim::{Block, BlockPos};

/// The study's registration (crate::registry): `circuit: mob-farm iron_ingot [tier]`.
pub static STUDY: StudyDef = StudyDef {
    id: "mobfarm",
    title: "Mob farms",
    site_title: "Mob farms",
    site_intro: "Iron golems spawn when three villagers that slept in the last game day are scared by a zombie: at most one golem every 30 seconds for each group of villagers, on a solid block within 8 blocks of them. These farms keep the villagers and a zombie in place, let the villagers sleep each night (a panicking villager never sleeps), kill each golem and carry its 3 to 5 iron ingots into a chest. Three farms, one per stage of the game: a day-1 farm built from what you have on your first day, a middle-ground farm for an evening's build, and the fastest farm, made of modules you copy side by side.",
    site_uses: "Iron for hoppers, rails and beacons without mining: AFK near the farm and empty its chest.",
    site_blurb: "Iron golem farms: villagers, a zombie and beds, rated in iron ingots per hour.",
    kinds: &[KindDef {
        word: "mob-farm",
        parse: parse_kind,
        inputs: &[],
        check: check_kind,
        verify: verify_in_pool,
        twin: Twin::Exact(sim_verify),
        agree: Agree::Rate { key: "iron per hour", tol: 0.2 },
        ticks: |_| JUDGE_DAYS * 24000,
        axis: |args, m| match tier(args) {
            Some((_, words)) => words.to_string(),
            None => {
                let n = m("modules");
                if n == "1" { "1 module".to_string() } else { format!("{n} modules") }
            }
        },
        runs: &["base", "still"],
        run: run_kind,
        site: Site {
            title: |k| {
                let w: Vec<&str> = k.split_whitespace().collect();
                match (w.get(1).copied(), w.get(2).copied()) {
                    (Some("iron_ingot") | None, Some("day-1")) => "Day-1 iron farm (iron armour on your first day)".to_string(),
                    (Some("iron_ingot") | None, Some("middle")) => "Middle-ground iron farm (the most iron for an evening's build)".to_string(),
                    (Some("iron_ingot") | None, Some("fastest")) => "Fastest iron farm (the most iron, module after module)".to_string(),
                    (Some("iron_ingot") | None, _) => "Iron farm".to_string(),
                    (Some(item), _) => format!("Mob farm: {}", item.replace('_', " ")),
                }
            },
            summary: |_| "villagers scared by a zombie summon iron golems; the farm kills them and collects their iron in a chest".into(),
            // (written for a player deciding whether to build it, vjeux 2026-11-08: what the farm
            // does, costs and needs, and what to watch out for; not how it was tested)
            metric: |m| match m {
                "iron per hour per block" => Some("iron ingots an hour for each block you place"),
                "iron per hour" => Some("iron ingots an hour"),
                "iron range" => Some("the range its rate falls in, 95% sure"),
                "golems per hour" => Some("iron golems an hour"),
                "seconds per golem" => Some("the time between two golems"),
                "share of ceiling" => Some("how close it comes to the game's limit: one golem every 30 seconds for each group of villagers"),
                "modules" => Some("groups of villagers that each summon golems on their own"),
                "iron per hour per module" => Some("iron ingots an hour for each group of villagers"),
                "kill" => Some("how long a golem lives before the farm kills it, at the longest"),
                "worst policy" => Some("its rate if every golem happened to walk the worst way"),
                "restarts after a missed night" => Some("whether it starts again by itself after a night its villagers could not sleep (rain at dusk, a storm, you away)"),
                "restarts after days away" => Some("whether it starts again by itself when you come back after days away"),
                "misses" => Some("summons that found no room for a golem, and golems the villagers saw (each one delays the next)"),
                "bill" => Some("what it costs in survival: iron (hoppers, the bucket and every tool you need), quartz, villagers, beds"),
                "payback" => Some("how long the farm takes to make back the iron it cost"),
                "tools" => Some("the tools you need and the iron each one costs: a bucket for lava and water, shears for leaves, an iron pickaxe for redstone dust"),
                "lava sources" => Some("lava source blocks to bring: one bucket trip each"),
                "materials" => Some("the stage of the game its materials need: day 1, the Nether (quartz, slime, magma...) or the End"),
                "chunk columns" => Some("how many 16 x 16 chunk columns it covers"),
                "storage" => Some("how long its chests and hoppers hold the iron and poppies before they fill"),
                "hoppers per 1000 iron/h" => Some("hoppers for each 1000 iron an hour: each hopper costs the server a little work every tick"),
                "build time" => Some("about how long the build takes: placing and digging its blocks, fetching the lava, moving the villagers in"),
                "time to an iron kit" => Some("about how long from starting the build to a full set of iron armour (24 iron), its own iron counted"),
                "real game" => Some("it works in the real game too, in rain and in thunder"),
                "lightning" => Some("what thunderstorms cost: the iron a strike burns, and how often a strike turns a villager into a witch, breaks the zombie's boat or starts a fire"),
                "thunder loss" => Some("iron lost for each hour of thunder"),
                "rain" => Some("iron an hour on a clear day, a rainy day and a stormy day"),
                "sunburn" => Some("whether the zombie can burn in daylight"),
                "fire" => Some("whether the lava can set the farm on fire"),
                _ => None,
            },
        },
    }],
};

/// The brief's tiers (designs/briefs/farm-iron.md revision 3), each a kind of its own so each
/// gets its own site page (vjeux via the decisions inbox 2026-11-08, after the crop farms' one
/// kind per crop): the word on the `circuit:` line, and in words.
pub const TIERS: [(&str, &str); 3] = [("day-1", "day 1"), ("middle", "middle ground"), ("fastest", "fastest")];

/// The tier a kind's words name (none: a farm in no tier, like the brief's obvious design).
pub fn tier(args: &[String]) -> Option<(&'static str, &'static str)> {
    args.get(1).and_then(|t| TIERS.iter().copied().find(|(w, _)| w == t))
}

/// Seeds each build runs.
pub const JUDGE_SEEDS: u32 = 8;
/// Game days per run.
pub const JUDGE_DAYS: u32 = 5;
/// Game days of the restart probe.
pub const RESTART_DAYS: u32 = 2;

/// `mob-farm <item> [tier]`: only `iron_ingot` so far; the tier one of [`TIERS`] (none: a farm
/// in no tier, published nowhere but the catalog).
fn parse_kind(w: &[&str]) -> Result<Vec<String>> {
    let item = match w.first().copied() {
        Some("iron_ingot") | Some("minecraft:iron_ingot") => "iron_ingot".to_string(),
        Some(other) => bail!("mob-farm: no farm for {other:?} yet (iron_ingot)"),
        None => bail!("mob-farm: give the product (`circuit: mob-farm iron_ingot`)"),
    };
    match w[1..] {
        [] => Ok(vec![item]),
        [t] if TIERS.iter().any(|(k, _)| *k == t) => Ok(vec![item, t.to_string()]),
        [t] => bail!("mob-farm: no tier {t:?} ({})", TIERS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")),
        _ => bail!("mob-farm: at most a product and a tier (`circuit: mob-farm iron_ingot middle`)"),
    }
}

/// The ports: `port output` (the farm's chest: a single chest in the design, or an air cell where
/// the judge puts one).
fn check_kind(c: &Circuit, _args: &[String]) -> Result<()> {
    let d = &c.clock.design;
    let Some(out) = d.ports.get("output") else { bail!("mob-farm: needs `port output: x y z` (the farm's chest, or an air cell for the judge's)") };
    match d.blocks.get(out) {
        None => Ok(()),
        Some(s) if s.trim_start_matches("minecraft:").starts_with("chest[") && s.contains("type=single") => Ok(()),
        Some(s) => bail!("mob-farm: port output {out:?} holds {s}: it must be the farm's single chest or an air cell (the judge puts its chest there)"),
    }
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// The terrain site of a design in the standard sample `sample` (brief revision 8: day 1 is judged
/// in terrain), from its meta: `ground: <y>` (the design y lying at the ground layer; default -1,
/// a void design's floor under its villagers), `afk: x y z` (the AFK player's feet; default on top
/// of the build's middle) and `terrain: cells` (a design made for terrain: only its listed cells
/// are written, natural rock for the rest; otherwise its box is cleared first).
pub fn site_of(c: &Circuit, spec: &FarmSpec, sample: &str) -> std::result::Result<redsim::entity::mob::site::Site, String> {
    let d = &c.clock.design;
    let s = redsim::terrain::samples::sample(sample).ok_or_else(|| format!("no terrain sample {sample}"))?;
    let (ground, afk, clear_box) = redsim::entity::mob::site::params(spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str))?;
    redsim::entity::mob::site::Site::new(s, spec, ground, afk, clear_box)
}

/// The terrain sample a design is judged in (`redsim::entity::mob::site::sample_of`, the rule the
/// 3D view and the game gate share: `sample: <name>`, else village-edge for a design made for
/// terrain, `terrain: cells`); `None`: the void.
pub fn terrain_sample(c: &Circuit) -> std::result::Result<Option<&'static redsim::terrain::samples::Sample>, String> {
    let d = &c.clock.design;
    redsim::entity::mob::site::sample_of(d.meta.get("terrain").map(String::as_str), d.meta.get("sample").map(String::as_str))
}

/// The site the judge's base build and the 3D view put a design (as written) at: the cheapest of
/// the nearest valid spots of its sample; `None`: a void design.
pub fn terrain_site(c: &Circuit) -> Option<std::result::Result<redsim::entity::mob::site::Site, String>> {
    let sample = match terrain_sample(c) {
        Ok(Some(s)) => s,
        Ok(None) => return None,
        Err(e) => return Some(Err(e)),
    };
    Some(spec_of(c).and_then(|spec| site_of(c, &spec, sample.name)))
}

/// The judge's terrain sites of a design as written: priced at the nearest valid spots of its
/// sample, the site at each, cheapest first (build k at site k; build 0, the card's and the 3D
/// view's, at the cheapest). Each is leaked (`RunOpts.site` is `'static`); every one holds the
/// sample's one shared world.
pub fn terrain_sites(c: &Circuit, spec: &FarmSpec, sample: &'static redsim::terrain::samples::Sample) -> std::result::Result<Vec<&'static redsim::entity::mob::site::Site>, String> {
    use redsim::entity::mob::site as ts;
    let d = &c.clock.design;
    let (ground, afk, clear_box) = ts::params(spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str))?;
    let shared = ts::sample_world_shared(sample)?;
    let fp = ts::footprint(spec, ground)?;
    let priced = {
        let w = shared.lock().unwrap_or_else(|e| e.into_inner());
        let spots = ts::spots_for(&w, sample, fp);
        if spots.is_empty() {
            return Err(format!("no spot for a {}x{} footprint {} deep in {}", fp.size[0], fp.size[1], fp.below, sample.name));
        }
        ts::price_all(&w, spec, fp, ground, clear_box, &spots)
    };
    Ok((0..priced.len()).map(|k| &*Box::leak(Box::new(ts::Site::at_priced(shared.clone(), sample, priced.clone(), k, afk, clear_box)))).collect())
}

/// The farm at its site as the worst-case checks read it: built, then its settle run
/// (`Run::start` and every settle tick: the farm's water flows, its light settles), the blocks the
/// game's farm holds when the player arrives (a static placement misses the cells its water flows
/// into: `mctas mobfarm spawn-rules` found C1 listing bats in two of D1U's cells the game fills
/// with flowing water).
pub fn settled_world(site: &'static redsim::entity::mob::site::Site, moved: &FarmSpec) -> redsim::World {
    let o = redsim::entity::mob::farm::RunOpts { site: Some(site), ..redsim::entity::mob::farm::RunOpts::steady(1, 0, WalkMode::Random) };
    match redsim::entity::mob::farm::Run::start(moved, &o) {
        Ok(mut run) => {
            while run.settle_tick() {}
            let mut w = run.world;
            w.settle_light();
            w
        }
        Err(_) => {
            let mut w = site.world_for(moved);
            w.place_raw(&moved.blocks);
            w.settle_light();
            w
        }
    }
}

/// The card's `exposures` line (the page-copy lane's one-line form): the farm at its site, then
/// each channel of the terrain track's worst-case checks for natural mobs (`hostile::check`) with
/// each mob's cell count: `in village-edge (-352,244): C2 entry zombie 155 cells, spider 139; ...`.
/// "none" when no natural mob can reach the farm in any way.
pub fn exposures_line(site: &'static redsim::entity::mob::site::Site, moved: &FarmSpec) -> String {
    exposures_of(site, moved).0
}

/// The channels of `ex` that can break the farm (brief farm-iron.md revision 9, iteration 2: the
/// card's `hostile break days` names them, no invented rate): a hostile that can spawn inside or
/// get in (C1, C2), hit a farm villager or the zombie (C5), a creeper's blast (C8), an enderman
/// placing a block in it (C9). A sight (C3), a stolen drop (C6), the village's mobs near it (C4)
/// and phantoms over the player (C10) cost iron, not the farm.
pub fn breaking_channels(ex: &[redsim::entity::mob::hostile::Exposure]) -> Vec<String> {
    const HOSTILE: [&str; 13] = ["zombie", "skeleton", "spider", "creeper", "pillager", "witch", "slime", "enderman", "vindicator", "husk", "drowned", "stray", "phantom"];
    let mut out: Vec<String> = Vec::new();
    for e in ex {
        let c = e.channel.split_whitespace().next().unwrap_or("");
        let hostile = HOSTILE.iter().any(|h| e.mob.contains(h)) && !e.mob.contains("village");
        let breaks = match c {
            "C1" | "C2" => hostile,
            "C5" => hostile && (e.what.contains("villager") || e.what.contains("the zombie")),
            "C8" | "C9" => true,
            _ => false,
        };
        if breaks && !out.iter().any(|o| o == e.channel) {
            out.push(e.channel.to_string());
        }
    }
    out
}

/// [`exposures_line`] and the breaking channels ([`breaking_channels`]).
pub fn exposures_of(site: &'static redsim::entity::mob::site::Site, moved: &FarmSpec) -> (String, Vec<String>) {
    let w = settled_world(site, moved);
    let summoners = {
        let mut with = w.clone();
        for (k, p, n) in &moved.entities {
            let _ = redsim::entity::spawn(&mut with, k, *p, n.as_deref());
        }
        redsim::entity::mob::site::villager_cells(&with)
    };
    let farm = hostile_farm(&w, moved, site.afk, &summoners);
    let ex = redsim::entity::mob::hostile::check(&w, &farm, true);
    let at = format!("in {} ({},{})", site.sample, site.offset[0], site.offset[2]);
    if ex.is_empty() {
        return (format!("{at}: none"), Vec::new());
    }
    let mut channels: Vec<(String, Vec<String>)> = Vec::new();
    for e in &ex {
        let mob = e.mob.trim_start_matches("a ").trim_start_matches("the ");
        let item = format!("{mob} {}", e.cells.len());
        match channels.iter_mut().find(|(c, _)| c == e.channel) {
            Some((_, v)) => v.push(item),
            None => channels.push((e.channel.to_string(), vec![format!("{item} cells")])),
        }
    }
    (format!("{at}: {}", channels.iter().map(|(c, v)| format!("{c} {}", v.join(", "))).collect::<Vec<_>>().join("; ")), breaking_channels(&ex))
}

/// The whole judge in a terrain sample (brief `farm-iron.md` revision 8: day 1 is judged in real
/// terrain): the design as written at each of the nearest valid spots of its sample (build k at
/// the k-th cheapest; build 0, the card's and the 3D view's, at the cheapest), every run in the
/// sample's world with natural spawning on Hard and the weather's natural cycle
/// (`farm::judge_opts_in`); the base checks (refusals, exit flood, worst policy, restart and
/// absence probes, rain and thunder windows) at the cheapest spot. The card adds the spot, the time
/// to an iron kit at the cheapest spot and over every spot, and the exposures to natural mobs.
fn terrain_judge(c: &Circuit, jo: &JudgeOpts, sample: &'static redsim::terrain::samples::Sample) -> std::result::Result<VerifyReport, String> {
    let kind = kind_of(c).map_err(|e| e.to_string())?;
    let spec = spec_of(c)?;
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let sites = terrain_sites(c, &spec, sample)?;
    let jobs: Vec<(&'static redsim::entity::mob::site::Site, FarmSpec, JudgeOpts)> = sites.iter().map(|s| (*s, s.spec(&spec), JudgeOpts { site: Some(*s), ..*jo })).collect();
    // day 1's measured T at each spot (brief revision 9, iterations 2-3: from dawn of day 1, the
    // build minutes at the stage's prices mining the build's own iron, until the chest holds 24)
    let b = bill(c);
    let kit = redsim::entity::mob::site::KIT_IRON as u32;
    let stage = price_stage(c);
    let build_s: Vec<Option<f64>> = jobs.iter().map(|(s, _, _)| site_bill(c, Some(*s), stage).ok().map(|bill| bill.seconds)).collect();
    let (base, runs, exposures, tkit) = std::thread::scope(|s| {
        let ex = s.spawn(|| exposures_of(jobs[0].0, &jobs[0].1));
        let base = s.spawn(|| base_checks(c, &jobs[0].1, &jobs[0].2));
        let hs: Vec<_> = jobs.iter().enumerate().map(|(k, (_, spec, o))| s.spawn(move || build_runs(spec, o, k as u32, WalkMode::Random))).collect();
        let hand = c.clock.design.meta.get("restart").map(|r| r.trim()) != Some("yes");
        let ts: Vec<_> = jobs.iter().zip(&build_s).map(|((site, spec, _), bs)| s.spawn(move || bs.map_or_else(Vec::new, |bs| t_kit_runs(spec, site, bs, kit, 8, hand)))).collect();
        let runs: Vec<std::result::Result<Vec<RunReport>, String>> = hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a build panicked".to_string()))).collect();
        let tkit: Vec<Vec<Option<f64>>> = ts.into_iter().map(|h| h.join().unwrap_or_default()).collect();
        (base.join().unwrap_or_else(|_| Err("the base checks panicked".to_string())), runs, ex.join().unwrap_or_else(|_| ("the exposure checks panicked".to_string(), vec!["the exposure checks panicked".to_string()])), tkit)
    });
    let pooled: Vec<RunReport> = runs.iter().filter_map(|r| r.as_ref().ok()).flatten().cloned().collect();
    let mut builds = Vec::new();
    for (k, ((site, spec_k, _), r)) in jobs.iter().zip(runs).enumerate() {
        let result = match (k, r, &base) {
            (_, Err(e), _) => Err(e),
            (0, Ok(_), Err(e)) => Err(e.clone()),
            (0, Ok(runs), Ok(b)) => Ok(BuildOk { outcome: outcome(c, spec_k, &runs, &pooled, Some(b), Some(site)), dims: dims(c), family: family.clone() }),
            (_, Ok(runs), _) => Ok(BuildOk { outcome: outcome(c, spec_k, &runs, &[], None, Some(site)), dims: dims(c), family: family.clone() }),
        };
        let o0 = sites[0].offset;
        builds.push(Build { turns: 0, offset: [site.offset[0] - o0[0], site.offset[1] - o0[1], site.offset[2] - o0[2]], result });
    }
    // the measured T replaces the formula's on each build's card ("time to an iron kit", the stage
    // cards' "t kit minutes"); a run that never fills the chest within 3 game days is reported
    let mean_t = |ts: &[Option<f64>]| -> Option<f64> { if ts.iter().all(|t| t.is_some()) && !ts.is_empty() { Some(ts.iter().flatten().sum::<f64>() / ts.len() as f64) } else { None } };
    for (b, ts) in builds.iter_mut().zip(&tkit) {
        let Ok(k) = b.result.as_mut() else { continue };
        let text = match mean_t(ts) {
            Some(t) => format!("{t:.0} min"),
            None => format!("more than 3 game days in {} of {} runs", ts.iter().filter(|t| t.is_none()).count(), ts.len()),
        };
        for (name, v) in k.outcome.metrics.iter_mut() {
            if name == "time to an iron kit" {
                *v = text.clone();
            } else if name == "t kit minutes" {
                *v = mean_t(ts).map_or("none".to_string(), |t| format!("{t:.1}"));
            }
        }
    }
    // the base build's card: where it stands, day 1's rank over every spot, the exposures
    let ts: Vec<f64> = tkit.iter().filter_map(|t| mean_t(t)).collect();
    let n = builds.len();
    if let Some(b0) = builds.first_mut()
        && let Ok(k) = b0.result.as_mut()
        && k.outcome.pass
    {
        let s0 = sites[0];
        let t0: Vec<f64> = tkit.first().map(|t| t.iter().flatten().copied().collect()).unwrap_or_default();
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(name, _)| name == "time to an iron kit")
            && ts.len() == n
            && !t0.is_empty()
        {
            m.1 = format!(
                "{} at the cheapest spot (from dawn of day 1 until the chest holds {kit} iron; runs {:.0}-{:.0} min; {:.0}-{:.0} min over the {n} nearest spots)",
                m.1,
                t0.iter().cloned().fold(f64::INFINITY, f64::min),
                t0.iter().cloned().fold(0.0, f64::max),
                ts.iter().cloned().fold(f64::INFINITY, f64::min),
                ts.iter().cloned().fold(0.0, f64::max)
            );
        }
        k.outcome.metrics.push(("spot".to_string(), format!("{} at {} {} {} (the cheapest of the {n} nearest valid spots)", s0.sample, s0.offset[0], s0.offset[1], s0.offset[2])));
        let (exposures, breaks) = exposures;
        k.outcome.metrics.push(("exposures".to_string(), exposures));
        // the stage cards' day-1 lines (brief revision 9, iteration 2): the same build's minutes on
        // flat plains (its cheapest of the 12 nearest spots there), the sample village's villagers
        // the farm leaves, the channels that can break it
        let flat = site_of(c, &spec, "plains").and_then(|p| site_bill(c, Some(&p), stage)).map(|bill| bill.seconds / 60.0);
        k.outcome.metrics.push(("build minutes flat".to_string(), flat.map_or_else(|e| format!("none ({e})"), |m| format!("{m:.1}"))));
        let village = redsim::entity::mob::site::sample_villagers(sample).unwrap_or(0);
        k.outcome.metrics.push(("villagers left".to_string(), format!("{}", village.saturating_sub(b.villagers as usize))));
        k.outcome.metrics.push(("hostile break days".to_string(), if breaks.is_empty() { "never".to_string() } else { format!("exposed: {}", breaks.join("; ")) }));
    }
    let mut schedule = Schedule::new(0);
    schedule.ticks = jo.days * 24000;
    Ok(crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() }))
}

/// The stage whose prices a design's build pays (redsim `prices::stage_ix`): its kind's tier
/// (`mob-farm iron_ingot middle`; the fastest farm pays the late stage's), day 1 for an untiered
/// design.
pub fn price_stage(c: &Circuit) -> usize {
    match c.clock.design.meta.get("circuit").map(|k| k.split_whitespace().nth(2).unwrap_or("").to_string()).as_deref() {
        Some("day-1") | Some("day-1-dust") => 0,
        Some("middle") => 1,
        Some("fastest") | Some("late") => 2,
        // an untiered design (iteration 3): day-1 prices when day 1 can get every item, else middle
        _ => {
            let blocks: Vec<String> = c.clock.design.block_list().into_iter().map(|(_, s)| s).collect();
            let blocks: Vec<&str> = blocks.iter().map(String::as_str).collect();
            if redsim::entity::mob::prices::bill(&blocks, &[], 0, &[], 0).is_ok() { 0 } else { 1 }
        }
    }
}

/// A build's bill at the stage's prices (brief farm-iron.md revision 9, iteration 3: redsim
/// `prices::bill`, designs/farms/prices.tsv): every block the design places; the natural blocks dug
/// (in a site: the spot's, the seals' dug first and the way down to the chest's); the site's own
/// blocks (its cobblestone seals, its torch or glass caps) and the stair's (a torch every 8 steps, a
/// fence gate); its entities (each villager moved, the zombie and its boat); each lava source; the
/// tools once (a bucket, the shears for leaves).
pub fn site_bill(c: &Circuit, site: Option<&redsim::entity::mob::site::Site>, stage: usize) -> std::result::Result<redsim::entity::mob::prices::Bill, String> {
    let blocks: Vec<String> = c.clock.design.block_list().into_iter().map(|(_, s)| s).collect();
    let blocks: Vec<&str> = blocks.iter().map(String::as_str).collect();
    let ents: Vec<(String, String)> = c.clock.design.entities.iter().map(|e| (e.kind.clone(), e.nbt.clone().unwrap_or_default())).collect();
    let ents: Vec<(&str, &str)> = ents.iter().map(|(k, n)| (k.as_str(), n.as_str())).collect();
    let (dug, extra): (u32, Vec<String>) = match site {
        Some(s) => {
            let mut extra: Vec<String> = s.site_blocks().into_iter().filter(|(_, b)| !b.is_air()).map(|(_, b)| b.to_string()).collect();
            let (adug, aplaced) = access_of(c, s);
            if aplaced > 0 {
                extra.extend((1..aplaced).map(|_| "minecraft:torch".to_string()));
                extra.push("minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]".to_string());
            }
            // (the fire clearance's cells are dug, a replaced one placed again as cobblestone)
            (s.dug + s.seals.iter().filter(|(_, d)| *d).count() as u32 + s.clears.len() as u32 + adug, extra)
        }
        None => (0, Vec::new()),
    };
    let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
    let mut bill = redsim::entity::mob::prices::bill(&blocks, &ents, dug, &extra, stage)?;
    // the items its containers start with (a sorter's filter iron and its named filler items: each
    // at its price, a named one also an anvil's rename, the anvil once)
    let p = redsim::entity::mob::prices::Prices::get()?;
    let mut renamed = 0u32;
    let mut add = |bill: &mut redsim::entity::mob::prices::Bill, item: &str, n: u32, each: f64| {
        let e = bill.items.entry(item.to_string()).or_insert((0, 0.0));
        e.0 += n;
        e.1 += each * n as f64;
        bill.seconds += each * n as f64;
    };
    for snbt in c.clock.design.block_entities.values() {
        for (item, n, named) in held_items(snbt) {
            add(&mut bill, &item, n, p.price(&item, stage)?);
            renamed += if named { n } else { 0 };
        }
    }
    if renamed > 0 {
        add(&mut bill, "rename", renamed, p.price("rename", stage)?);
        add(&mut bill, "tool:anvil", 1, p.price("tool:anvil", stage)?);
    }
    Ok(bill)
}

/// The items a block entity's SNBT holds (`{id:"minecraft:x", count:n}` entries, `Count:nb` too),
/// each with whether it carries a name (an anvil's rename).
fn held_items(snbt: &str) -> Vec<(String, u32, bool)> {
    let mut out = Vec::new();
    let mut rest = snbt;
    while let Some(k) = rest.find("id:\"minecraft:") {
        let start = rest[..k].rfind('{').unwrap_or(0);
        let name: String = rest[k + 14..].chars().take_while(|c| *c != '"').collect();
        // the entry: to the brace closing its own (its components nest braces)
        let (mut depth, mut end) = (0i32, rest.len());
        for (i, ch) in rest[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let entry = &rest[start..end];
        let count = ["count:", "Count:"].iter().find_map(|key| entry.find(key).map(|i| entry[i + key.len()..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>())).and_then(|n| n.parse::<u32>().ok()).unwrap_or(1);
        out.push((name, count, entry.contains("custom_name") || entry.contains("Name:")));
        rest = &rest[end.max(k + 1)..];
    }
    out
}

/// A dug-in farm's way down to its chest at a site (redsim `site::access_stair`: the ground layer,
/// the design's `ground:`, to the chest's layer): (dug, placed); none for a void design.
fn access_of(c: &Circuit, s: &redsim::entity::mob::site::Site) -> (u32, u32) {
    if s.clear_box {
        return (0, 0);
    }
    let ground = c.clock.design.meta.get("ground").and_then(|g| g.trim().parse::<i32>().ok()).unwrap_or(-1);
    let chest = c.clock.design.ports.get("output").map_or(ground, |p| p[1]);
    redsim::entity::mob::site::access_stair(ground - chest)
}

/// A build's effort (redsim `site::Effort`): in a terrain site the blocks placed (with its seals
/// and covers) and dug at that spot, plus a dug-in farm's way down to its chest
/// (`site::access_stair`: the ground layer, the design's `ground:`, to the chest's layer); in the
/// void every block placed. With the access stair's (dug, placed).
pub fn site_effort(c: &Circuit, site: Option<&redsim::entity::mob::site::Site>, b: &Bill) -> (redsim::entity::mob::site::Effort, (u32, u32)) {
    use redsim::entity::mob::site::Effort;
    let effort = match site {
        Some(s) => s.effort(b.lava, b.villagers),
        None => Effort { placed: c.clock.design.blocks.values().filter(|s| !matches!(s.split('[').next().unwrap_or("").trim_start_matches("minecraft:"), "air" | "cave_air" | "void_air")).count() as u32, dug: 0, lava: b.lava, villagers: b.villagers },
    };
    let access = match site {
        Some(s) => access_of(c, s),
        None => (0, 0),
    };
    (Effort { dug: effort.dug + access.0, placed: effort.placed + access.1, ..effort }, access)
}

/// Day 1's measured T at one site (brief farm-iron.md revision 9, iteration 2): the player starts
/// at dawn of day 1 and builds for `build_s` seconds; the farm stands complete at day time
/// 20 x `build_s` (mod 24000), its villagers arriving with the `LAST_SLEPT` of their last night in
/// the village (the start of the last REST) and the zombie parked; the judge's terrain run from
/// there (the weather's cycle, natural spawning on Hard) until the chest holds `iron`, for each of
/// `seeds` seeds (at most 3 game days): T in minutes, `None` for a run that did not get there. A
/// farm that cannot restart by itself (`hand_hide`: its design does not claim `restart: yes`) gets
/// the first-night hide by hand its card prints (redsim `farm::run_t`).
pub fn t_kit_runs(spec: &FarmSpec, site: &'static redsim::entity::mob::site::Site, build_s: f64, iron: u32, seeds: u32, hand_hide: bool) -> Vec<Option<f64>> {
    let day0 = ((build_s * 20.0).round() as i64).rem_euclid(24000);
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..seeds)
            .map(|k| {
                s.spawn(move || {
                    let o = RunOpts { day0, slept: true, slept_rest: true, ..farm::judge_opts_in(0x7d1 + k as i64, 3 * 24000, WalkMode::Random, Some(site)) };
                    match farm::run_t(spec, &o, iron, hand_hide) {
                        Ok((_, Some(t))) => Some(build_s / 60.0 + t as f64 / 1200.0),
                        _ => None,
                    }
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap_or(None)).collect()
    })
}

/// The farm the terrain track's worst-case checks read (`hostile::Farm`), from a spec already
/// built into `w` (its site's seals and caps in): its blocks and beds, its villagers'
/// standing and sleeping boxes and eyes, its zombie and vehicle, the golem spawn cells (where a
/// golem can spawn, `checkSpawnObstruction` passing, for the searches of `summoners`: every farm
/// villager, `site::villager_cells`), and the interior: the cells those occupy, the golem
/// cells and the two above them, and every open cell of the design (its dug air, beds, trapdoors,
/// water...: the farm's own space, where a natural spawn is C1 and which no flood starts from).
pub fn hostile_farm(w: &redsim::World, spec: &FarmSpec, afk: redsim::entity::Vec3, summoners: &[BlockPos]) -> redsim::entity::mob::hostile::Farm {
    use redsim::entity::{Aabb, Vec3};
    let mut farm = redsim::entity::mob::hostile::Farm { afk, summoners: summoners.to_vec(), ..Default::default() };
    for (q, _) in &spec.blocks {
        farm.blocks.push(*q);
        if let Block::Bed { head: true, .. } = w.get(*q) {
            farm.beds.push(*q);
        }
    }
    for (kind, p, nbt) in &spec.entities {
        let (x, y, z) = (p[0], p[1], p[2]);
        let kind = kind.trim_start_matches("minecraft:");
        if kind == "villager" {
            farm.villagers.push(Aabb::new(x - 0.3, y, z - 0.3, x + 0.3, y + 1.95, z + 0.3));
            farm.villagers.push(Aabb::new(x - 0.1, y, z - 0.1, x + 0.1, y + 0.2, z + 0.1));
            farm.villager_eyes.push(Vec3::new(x, y + 1.62, z));
            farm.villager_eyes.push(Vec3::new(x, y + 0.2, z));
        } else if kind.ends_with("_boat") || kind.ends_with("_raft") || kind.contains("minecart") {
            farm.zombie.push(Aabb::new(x - 0.6875, y, z - 0.6875, x + 0.6875, y + 0.5625, z + 0.6875));
            if nbt.as_deref().is_some_and(|n| n.contains("zombie")) {
                farm.zombie.push(Aabb::new(x - 0.3, y + 0.2, z - 0.3, x + 0.3, y + 2.15, z + 0.3));
            }
        } else if kind == "zombie" {
            farm.zombie.push(Aabb::new(x - 0.3, y, z - 0.3, x + 0.3, y + 1.95, z + 0.3));
        }
    }
    for s in summoners {
        for (c, ok) in redsim::entity::mob::spawn::golem_spawn_cells_from(w, *s) {
            if ok.is_ok() && !farm.golem_cells.contains(&c) {
                farm.golem_cells.push(c);
            }
        }
    }
    let mut interior: Vec<BlockPos> = Vec::new();
    for b in farm.villagers.iter().chain(farm.zombie.iter()) {
        for y in b.min_y.floor() as i32..=(b.max_y - 1.0e-6).floor() as i32 {
            for z in b.min_z.floor() as i32..=(b.max_z - 1.0e-6).floor() as i32 {
                for x in b.min_x.floor() as i32..=(b.max_x - 1.0e-6).floor() as i32 {
                    interior.push(BlockPos::new(x, y, z));
                }
            }
        }
    }
    for c in &farm.golem_cells {
        for dy in 0..3 {
            interior.push(c.offset(0, dy, 0));
        }
    }
    for (q, b) in &spec.blocks {
        if !redsim::behavior::full_cube(b) {
            interior.push(*q);
        }
    }
    interior.sort_by_key(|p| (p.x, p.y, p.z));
    interior.dedup();
    farm.interior = interior;
    farm
}

/// The redsim farm of a (turned, moved) design: its blocks, entities, output chest and box.
pub fn spec_of(c: &Circuit) -> std::result::Result<FarmSpec, String> {
    let d = &c.clock.design;
    let out = *d.ports.get("output").ok_or("no `port output`")?;
    let (from, to) = crate::run::watch_box(c, 4);
    let mut blocks = Vec::new();
    for (q, s) in d.block_list() {
        blocks.push((bp(q), Block::parse(&s).map_err(|e| format!("not modeled by redsim: {e}"))?));
    }
    blocks.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    let entities = d.entities.iter().map(|e| (e.kind.clone(), e.pos, e.nbt.clone())).collect();
    let block_entities = d.block_entities.iter().map(|(p, s)| (bp(*p), s.clone())).collect();
    Ok(FarmSpec { blocks, entities, output: bp(out), lo: bp(from), hi: bp(to), block_entities })
}

/// What the judge runs.
#[derive(Clone, Copy, Debug)]
pub struct JudgeOpts {
    pub seeds: u32,
    pub days: u32,
    /// The first seed (build k adds k x 1000).
    pub seed0: i64,
    /// Threads for the exit flood.
    pub threads: usize,
    /// The build's terrain site (the spec already moved there): every run, refusal and the exit
    /// flood in the sample's world with natural spawning on Hard; `None`: the void.
    pub site: Option<&'static redsim::entity::mob::site::Site>,
}

impl Default for JudgeOpts {
    fn default() -> JudgeOpts {
        JudgeOpts { seeds: JUDGE_SEEDS, days: JUDGE_DAYS, seed0: farm::JUDGE_SEED0, threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32), site: None }
    }
}

/// A judge run's options: the judge's own for `seed`, in the build's site (natural spawning on
/// Hard and the weather's cycle there: `farm::judge_opts_in`).
fn ropts(o: &JudgeOpts, seed: i64, ticks: u32, walk: WalkMode) -> RunOpts {
    farm::judge_opts_in(seed, ticks, walk, o.site)
}

/// Run several farm runs at once (one thread each).
pub fn run_all(spec: &FarmSpec, jobs: &[RunOpts]) -> Vec<std::result::Result<RunReport, String>> {
    std::thread::scope(|s| {
        let hs: Vec<_> = jobs.iter().map(|j| s.spawn(move || farm::run(spec, j))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a run panicked".to_string()))).collect()
    })
}

/// Build `build`'s runs: the refusals first, then [`JudgeOpts::seeds`] steady-state runs.
pub fn build_runs(spec: &FarmSpec, o: &JudgeOpts, build: u32, walk: WalkMode) -> std::result::Result<Vec<RunReport>, String> {
    let refused = redsim::entity::mob::refuse::static_checks_in(spec, o.site);
    if !refused.is_empty() {
        return Err(format!("refused: {}", refused.join("; ")));
    }
    let jobs: Vec<RunOpts> = (0..o.seeds).map(|k| ropts(o, o.seed0 + build as i64 * 1000 + k as i64, o.days * 24000, walk)).collect();
    run_all(spec, &jobs).into_iter().collect()
}

/// The 9 worst-policy runs: still and the 8 directions, every golem of a run under that one
/// policy, all on the same seed.
pub fn worst_policy(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<Vec<(Walk, RunReport)>, String> {
    let policies = Walk::golem_policies();
    let jobs: Vec<RunOpts> = policies.iter().map(|p| ropts(o, o.seed0 + 777, o.days * 24000, WalkMode::Fixed(*p))).collect();
    let runs: Vec<RunReport> = run_all(spec, &jobs).into_iter().collect::<std::result::Result<_, _>>()?;
    Ok(policies.into_iter().zip(runs).collect())
}

/// The restart probe: a run whose villagers have no `LAST_SLEPT` must spawn a golem within
/// [`RESTART_DAYS`] days.
pub fn restart_probe(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<RunReport, String> {
    let mut ro = ropts(o, o.seed0 + 999, RESTART_DAYS * 24000, WalkMode::Random);
    ro.slept = false;
    farm::run(spec, &ro)
}

/// Whether the design claims to restart after a missed night (`restart: yes`).
pub fn claims_restart(c: &Circuit) -> bool {
    c.clock.design.meta.get("restart").is_some_and(|v| v.trim() == "yes")
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

/// One table line per run.
fn run_line(label: &str, r: &RunReport) -> String {
    let fail = if r.passed() { String::new() } else { format!(": FAIL {}", r.failures.iter().chain(r.errors.iter()).cloned().collect::<Vec<_>>().join("; ")) };
    format!(
        "  {label}: {:.0} iron/h, {:.1} golems/h, {} golems in {:.1} h, kill {:.0} gt, {} tries without a spot, {} sightings, {} sleeps, longest without sleep {} gt{fail}",
        r.iron_per_hour(),
        r.golems_per_hour(),
        r.golems,
        r.hours(),
        mean(&r.lifetimes.iter().map(|v| *v as f64).collect::<Vec<_>>()),
        r.losses.no_spot,
        r.losses.seen,
        r.sleeps,
        r.max_sleep_gap,
    )
}

/// What only the base build checks: the exit flood, the worst policy, the restart probe.
#[derive(Clone, Debug, Default)]
pub struct BaseChecks {
    pub exit: ExitReport,
    pub worst: Vec<(Walk, RunReport)>,
    pub restart: Option<RunReport>,
    /// The player away for days, each way and back by day and at night (farms that claim to
    /// restart): [`absence_probe`].
    pub away: Vec<(Away, i64, RunReport, AwayReport)>,
    /// mobinfra: a natural day, a forced day of rain, a forced day of thunder: [`windows_probe`].
    pub windows: Vec<RunReport>,
}

impl BaseChecks {
    /// Why the base checks fail (empty: they pass).
    pub fn failures(&self) -> Vec<String> {
        let mut v: Vec<String> = self.exit.failures.iter().map(|f| format!("golem exit: {f}")).collect();
        for (p, r) in &self.worst {
            if let Some(f) = r.failures.iter().chain(r.errors.iter()).next() {
                v.push(format!("every golem walking {}: {f}", p.name()));
            }
        }
        if let Some(r) = &self.restart
            && r.golems == 0
        {
            v.push(format!("claims to restart after a missed night, but no golem spawned in {RESTART_DAYS} days without a recent sleep"));
        }
        for (how, back, r, a) in &self.away {
            let when = away_when(*how, *back);
            if let Some(f) = r.failures.iter().chain(r.errors.iter()).next() {
                v.push(format!("{when}: {f}"));
            } else if a.first_golem.is_none() {
                v.push(format!("claims to restart, but {when} no golem spawned in the {AWAY_AFTER_DAYS} days after the return"));
            }
        }
        v
    }
}

/// Days the absence probe stays away, and runs after the return.
pub const AWAY_DAYS: i64 = 3;
pub const AWAY_AFTER_DAYS: i64 = 2;

fn away_when(how: Away, back: i64) -> String {
    format!("after {AWAY_DAYS} days away ({}, back {})", how.name(), if back == 6000 { "by day" } else { "at night" })
}

/// The absence probe (brief, Stages: Middle): a day of the steady state, then [`AWAY_DAYS`] days away
/// with the chunks unloaded or loaded but not ticking, back at noon or at midnight, then
/// [`AWAY_AFTER_DAYS`] days: the farm must spawn again by itself.
pub fn absence_probe(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<Vec<(Away, i64, RunReport, AwayReport)>, String> {
    let cases: Vec<(Away, i64)> = [Away::Unloaded, Away::NotTicking].into_iter().flat_map(|h| [(h, 6000), (h, 18000)]).collect();
    let ro = ropts(o, o.seed0 + 555, 24000, WalkMode::Random);
    let out: Vec<std::result::Result<(RunReport, AwayReport), String>> = std::thread::scope(|s| {
        let hs: Vec<_> = cases.iter().map(|&(how, back_at)| s.spawn(move || farm::run_away(spec, &ro, AwayPlan { before: 24000, days: AWAY_DAYS, back_at, how, after: AWAY_AFTER_DAYS * 24000 }))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("absence run panicked".into()))).collect()
    });
    cases.into_iter().zip(out).map(|((how, back), r)| r.map(|(rep, a)| (how, back, rep, a))).collect()
}

/// The base checks of a farm.
pub fn base_checks(c: &Circuit, spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<BaseChecks, String> {
    let refused = redsim::entity::mob::refuse::static_checks_in(spec, o.site);
    if !refused.is_empty() {
        return Err(format!("refused: {}", refused.join("; ")));
    }
    let exit = exit::golem_exit_in(spec, o.site, o.seed0, o.threads)?;
    // the worst walking policy: only without the mob AI (with it the golems walk by their goals:
    // farm::judge_ai, the mob-AI lane)
    let worst = if exit.failures.is_empty() && !redsim::entity::mob::farm::judge_ai() { worst_policy(spec, o)? } else { Vec::new() };
    let restart = if claims_restart(c) { Some(restart_probe(spec, o)?) } else { None };
    let away = if claims_restart(c) { absence_probe(spec, o)? } else { Vec::new() };
    let windows = windows_probe(spec, o)?;
    Ok(BaseChecks { exit, worst, restart, away, windows })
}

/// Runs of the rain and thunder probe.
pub const WINDOW_RUNS: u32 = 16;

/// The rain and thunder probe (mobinfra; the card's `rain` line): [`WINDOW_RUNS`] runs of a
/// natural day, then a forced day of rain, then a forced day of thunder (`weather::Plan::Windows`,
/// from the cycle's stationary state, as the game gate runs them).
pub fn windows_probe(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<Vec<RunReport>, String> {
    let plan = redsim::weather::Plan::Windows { at: 24000, rain: 24000, thunder: 24000 };
    let jobs: Vec<RunOpts> = (0..WINDOW_RUNS).map(|k| ropts(o, o.seed0 + 4242 + k as i64, 72000, WalkMode::Random).with_weather(plan)).collect();
    run_all(spec, &jobs).into_iter().collect()
}

/// "about every N game days" (or "never") for a rate a game day.
fn every(per_day: f64) -> String {
    if per_day <= 0.0 { "never".to_string() } else { format!("about every {} game days", days(1.0 / per_day)) }
}

fn days(d: f64) -> String {
    if d >= 9.5 { format!("{d:.0}") } else { format!("{d:.1}") }
}

/// A small expected count with two significant digits (0.0041, 0.27, 3.6, 12).
fn small(x: f64) -> String {
    if x >= 9.5 {
        format!("{x:.0}")
    } else if x >= 0.095 || x == 0.0 {
        format!("{x:.1}")
    } else {
        let digits = (-(x.log10().floor()) as usize) + 1;
        format!("{x:.digits$}")
    }
}

/// The card's `lightning` line (one sentence, `;` between its parts: the site splits a card's
/// metrics at ", ").
pub fn lightning_line(m: &redsim::weather::lightning::Measure) -> String {
    let s = redsim::weather::storms();
    let per_day = s.share * 24000.0;
    let mut v = vec![
        format!("about {} iron lost per thunderstorm", small(m.iron * s.mean_len)),
        // a player loses villagers, the gate counts strikes (the parent 2026-11-08): both
        match m.witch_max {
            0 => "no villager ever turned into a witch".to_string(),
            1 => format!("a villager turned into a witch {}", every(m.witch * per_day)),
            k => format!("villagers turned into witches {} (one strike can take up to {k})", every(m.witch * per_day)),
        },
        format!("the zombie's boat broken {}", every(m.vehicle * per_day)),
        format!("a fire beside a flammable block {}", every(m.fire * per_day)),
        format!("a skeleton trap horse on the farm {}", every(m.trap * per_day)),
    ];
    if m.breaks > 0.0 {
        let first = [(m.witch, "a villager made a witch"), (m.vehicle, "the zombie's boat broken"), (m.fire, "a fire")].into_iter().max_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1).unwrap_or("");
        v.push(format!("the farm breaks in about {} game days ({first} first)", days(1.0 / (m.breaks * per_day))));
    } else {
        v.push("lightning never breaks the farm".to_string());
    }
    v.push(format!("thunderstorms {:.1}% of the time ({:.0} gt each on average)", 100.0 * s.share, s.mean_len));
    v.join("; ")
}

/// Iron lost for each hour of thunder: the bolts' and, when one breaks the farm, its rate `iron`
/// (an hour) until the end of that game day (half a day on average: 1/6 h).
pub fn thunder_loss(m: &redsim::weather::lightning::Measure, iron: f64) -> f64 {
    72000.0 * (m.iron + m.breaks * iron / 6.0)
}

/// The card's `fire` line: whether lava's random tick can ever burn one of the farm's blocks.
fn fire_line(spec: &FarmSpec, runs: &[RunReport], site: Option<&'static redsim::entity::mob::site::Site>) -> Result<String, String> {
    // the runs' own burns (the run goes on: the lava flows into what burned)
    let burned: Vec<&(i64, String, [i32; 3])> = runs.iter().filter_map(|r| r.weather.first_burn.as_ref()).collect();
    if let Some((_, what, at)) = burned.first() {
        let name = what.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
        let minutes = burned.iter().map(|b| b.0 as f64 / 1200.0).sum::<f64>() / burned.len() as f64;
        return Ok(format!("burns its {name} (first at {} {} {}) in about {minutes:.0} min in {} of {} runs; the lava then flows in", at[0], at[1], at[2], burned.len(), runs.len()));
    }
    let run = farm::Run::new(spec, &RunOpts { site, ..RunOpts::steady(0, 0, WalkMode::Random) })?;
    Ok(match redsim::weather::fire::lava_can_burn(&run.world, spec.lo, spec.hi) {
        None => "never".to_string(),
        Some((lava, at, q, b)) => {
            let name = b.to_string();
            let name = name.split('[').next().unwrap_or("").trim_start_matches("minecraft:").to_string();
            let hours: f64 = runs.iter().map(|r| r.hours()).sum();
            format!("lava at {} {} {} can light a fire at {} {} {} beside the {name} at {} {} {} (not within the runs' {hours:.0} h)", lava.x, lava.y, lava.z, at.x, at.y, at.z, q.x, q.y, q.z)
        }
    })
}

/// A build's outcome from its runs (`pooled`: every build's runs, for the base build's card;
/// `base`: the base checks, on the base build; `site`: the build's terrain site, `None` in the
/// void).
pub fn outcome(c: &Circuit, spec: &FarmSpec, runs: &[RunReport], pooled: &[RunReport], base: Option<&BaseChecks>, site: Option<&'static redsim::entity::mob::site::Site>) -> Outcome {
    let mut table: Vec<String> = runs.iter().enumerate().map(|(k, r)| run_line(&format!("run {k}"), r)).collect();
    if let Some(b) = base {
        table.push(format!("  golem exit: {} spawn spots, {} positions, {} drift runs, every golem dead within {} gt", b.exit.spawn_spots, b.exit.starts, b.exit.runs, b.exit.worst_life));
        for s in &b.exit.walk_stalls {
            table.push(format!("  golem exit (reported, a constant walk): {s}"));
        }
        table.push(format!("  golems that targeted the farm's zombie, per run: {}", runs.iter().map(|r| r.golems_at_zombie.to_string()).collect::<Vec<_>>().join(" ")));
        for (p, r) in &b.worst {
            table.push(run_line(&format!("every golem {}", p.name()), r));
        }
        if let Some(r) = &b.restart {
            table.push(run_line("restart probe (no recent sleep)", r));
        }
        for (how, back, r, a) in &b.away {
            let first = a.first_golem.map(|t| format!("first golem {t} gt after the return")).unwrap_or_else(|| "no golem after the return".into());
            table.push(format!("{}; {first}, {} golems", run_line(&away_when(*how, *back), r), a.golems));
        }
    }
    let mut why: Vec<String> = runs.iter().filter(|r| !r.passed()).flat_map(|r| r.failures.iter().chain(r.errors.iter()).cloned()).take(3).collect();
    if let Some(b) = base {
        why.extend(b.failures());
    }
    if !why.is_empty() {
        return Outcome { pass: false, why: why.join("; "), metrics: Vec::new(), table };
    }
    let all = if pooled.is_empty() { runs } else { pooled };
    let rates: Vec<f64> = all.iter().map(|r| r.iron_per_hour()).collect();
    let own = mean(&runs.iter().map(|r| r.iron_per_hour()).collect::<Vec<_>>());
    let iron = if base.is_some() { mean(&rates) } else { own };
    let half = 1.96 * sd(&rates) / (rates.len() as f64).sqrt();
    let golems = mean(&all.iter().map(|r| r.golems_per_hour()).collect::<Vec<_>>());
    let modules = villager_clusters(spec);
    let ceiling = 120.0 * modules as f64;
    let mut b = bill(c);
    // the lava plan the build's price chose (iteration 6, G1): its extra day-1 buckets are iron in
    // hand (3 each) and its trips are the card's
    let plan = site_bill(c, site, price_stage(c)).ok();
    let extra_buckets = plan.as_ref().and_then(|p| p.items.get("extra_bucket")).map_or(0, |e| e.0);
    let lava_trips = plan.as_ref().and_then(|p| p.items.get("lava_trip")).map_or(b.lava, |e| e.0);
    if extra_buckets > 0 {
        b.iron += 3.0 * extra_buckets as f64;
        b.tools.push(("extra buckets", 3 * extra_buckets));
    }
    let payback = if iron > 0.0 { b.iron / iron * 60.0 } else { f64::INFINITY };
    let no_spot: u32 = all.iter().map(|r| r.losses.no_spot).sum();
    let seen: u32 = all.iter().map(|r| r.losses.seen).sum();
    let tries = no_spot + all.iter().map(|r| r.golems).sum::<u32>();
    let lifetimes: Vec<f64> = all.iter().flat_map(|r| r.lifetimes.iter().map(|v| *v as f64)).collect();
    let mut metrics = vec![
        ("iron per hour per block".to_string(), format!("{:.2}", iron / c.blocks().max(1) as f64)),
        ("iron per hour".to_string(), format!("{iron:.0}")),
        ("iron range".to_string(), format!("95% {:.0}-{:.0} ({} runs)", iron - half, iron + half, rates.len())),
        ("golems per hour".to_string(), format!("{golems:.1}")),
        ("seconds per golem".to_string(), format!("{:.1}", if golems > 0.0 { 3600.0 / golems } else { 0.0 })),
        ("share of ceiling".to_string(), format!("{:.0}%", 100.0 * golems / ceiling)),
        ("modules".to_string(), modules.to_string()),
        ("iron per hour per module".to_string(), format!("{:.0}", iron / modules as f64)),
        ("kill".to_string(), format!("{:.0} gt", mean(&lifetimes))),
        // the kill-time tail with its sample's bound (iteration 13: a golem alive past the limit
        // in any run fails it; none in G golems bounds the share at 3 / G at 95%)
        ("kill tail".to_string(), {
            let g: u32 = all.iter().map(|r| r.golems).sum();
            let longest = lifetimes.iter().cloned().fold(0.0, f64::max);
            format!("the longest {longest:.0} gt: no golem outlived {} gt in {g} golems (95% bound {:.2}% a golem)", redsim::entity::mob::farm::MAX_GOLEM_LIFE, 300.0 / g.max(1) as f64)
        }),
        ("golems at the zombie".to_string(), {
            let g: u32 = all.iter().map(|r| r.golems).sum();
            format!("{} of {g} golems targeted the farm's zombie", all.iter().map(|r| r.golems_at_zombie).sum::<u32>())
        }),
    ];
    if let Some(bc) = base {
        if let Some((p, r)) = bc.worst.iter().min_by(|a, b| a.1.iron_per_hour().total_cmp(&b.1.iron_per_hour())) {
            metrics.push(("worst policy".to_string(), format!("{:.0} iron/h ({})", r.iron_per_hour(), p.name())));
        } else {
            // (with the mob AI the golems walk by their goals; the exit flood's constant walking
            // inputs are reported here, not refused: brief rev 9 iteration 13)
            metrics.push(("worst policy".to_string(), match bc.exit.walk_stalls.len() {
                0 => "every constant walk reaches the kill".to_string(),
                n => format!("{n} constant walks keep a golem alive past {} gt (reported; walking is judged by the mob AI and the game)", redsim::entity::mob::farm::MAX_GOLEM_LIFE),
            }));
        }
        // lips: where a golem can stand dry with its box in a current (iteration 13's G1 note:
        // reported beside the kill tail and the golems at the zombie, never refused)
        metrics.push(("lips".to_string(), match bc.exit.lips {
            0 => "none (no golem stands dry beside a current)".to_string(),
            n => format!("{n} cells where a golem stands dry beside a current (reported: the kill tail and the golems at the zombie show whether they hold)"),
        }));
        metrics.push(("restarts after a missed night".to_string(), if bc.restart.is_some() { "yes".to_string() } else { "no (hide the zombie by hand the first night)".to_string() }));
        let firsts: Vec<f64> = bc.away.iter().filter_map(|(_, _, _, a)| a.first_golem).map(|t| t as f64 / 72000.0).collect();
        let away = if bc.away.is_empty() || firsts.len() < bc.away.len() {
            "no".to_string()
        } else {
            format!("yes (unloaded or not ticking, back by day or at night: the first golem {:.1}-{:.1} h after the return)", firsts.iter().cloned().fold(f64::INFINITY, f64::min), firsts.iter().cloned().fold(0.0, f64::max))
        };
        metrics.push(("restarts after days away".to_string(), away));
        // mobinfra: the weather's tradeoffs, measured (vjeux 2026-11-08 08:26: "You can't expect
        // day 1 farm to pass everything. It's all about tradeoffs")
        let (golems_all, iron_all): (u32, u32) = all.iter().fold((0, 0), |(g, i), r| (g + r.golems, i + r.iron_dropped));
        let per_golem = if golems_all > 0 { iron_all as f64 / golems_all as f64 } else { 4.0 };
        if !bc.windows.is_empty() {
            let rate = |a: i64| {
                let a = farm::SETTLE_TICKS + a;
                mean(&bc.windows.iter().map(|r| r.spawn_times.iter().filter(|t| (a..a + 24000).contains(*t)).count() as f64 * 3.0 * per_golem).collect::<Vec<_>>())
            };
            metrics.push(("rain".to_string(), format!("clear day {:.0} iron/h; rain day {:.0} iron/h; thunder day {:.0} iron/h", rate(0), rate(24000), rate(48000))));
        }
        match redsim::weather::lightning::measure_farm_in(spec, runs, site) {
            Ok((m, sky)) => {
                metrics.push(("lightning".to_string(), lightning_line(&m)));
                metrics.push(("thunder loss".to_string(), format!("{:.1}", thunder_loss(&m, iron))));
                metrics.push(("sunburn".to_string(), if sky { "the zombie sees the sky: it burns on its first clear day".to_string() } else { "never (the zombie never sees the sky)".to_string() }));
            }
            // a tradeoff the model cannot measure: no card (never a line silently missing)
            Err(e) => return Outcome { pass: false, why: e, metrics, table },
        }
        match fire_line(spec, all, site) {
            Ok(f) => metrics.push(("fire".to_string(), f)),
            Err(e) => return Outcome { pass: false, why: format!("the card's fire line: {e}"), metrics, table },
        }
    }
    metrics.push(("misses".to_string(), format!("{no_spot} of {tries} tries found no spot, {seen} golem sightings")));
    metrics.push(("bill".to_string(), b.text()));
    metrics.push(("payback".to_string(), format!("{payback:.0} min")));
    // the build effort and day 1's rank (brief revision 8, Stages: Day 1; redsim
    // entity/mob/site.rs `Effort`, `day1_minutes`): 2 s a block placed (gathering included, the
    // glass covers too), 1 s a natural block dug (none in the void), 120 s a lava trip, 180 s a
    // villager moved; then the minutes to own a 24-iron kit at the card's rate, the iron the
    // thunderstorms are expected to cost meanwhile included
    use redsim::entity::mob::site::day1_minutes;
    let (effort, access) = site_effort(c, site, &b);
    if site.is_none() {
        // the build at the stage's prices (iteration 3 and 6: redsim prices::bill), its largest
        // items named; then the kit: the build's own iron is inside its minutes, so the chest owes
        // only the kit's 24 iron at the card's rate
        let stage = price_stage(c);
        let priced = site_bill(c, None, stage);
        let head = match &priced {
            Ok(bill) => format!("{:.0} min at {} prices", bill.seconds / 60.0, ["day-1", "middle", "late"][stage]),
            Err(e) => format!("none ({e})"),
        };
        // (the site's cards read `N placed`, `N dug`, `N lava trips` and `N villagers moved` from
        // the parentheses: crates/site/src/cards.rs)
        metrics.push(("build time".to_string(), format!("{head} ({} placed, 0 dug, {lava_trips} lava trips, {} villagers moved)", effort.placed, b.villagers)));
        if iron > 0.0
            && let Ok(bill) = &priced
        {
            metrics.push(("time to an iron kit".to_string(), format!("{:.0} min (the build, then 24 iron at the card's rate)", bill.seconds / 60.0 + redsim::entity::mob::site::KIT_IRON / iron * 60.0)));
        }
    } else {
        let seals = site.map_or(0, |s| s.seals.len());
        let (torches, glass) = site.map_or((0, 0), |s| s.cover_counts());
        let extras: Vec<String> = [(torches, "cobblestone slabs capping stray golem spots"), (glass, "glass caps in water"), (seals, "cobblestone seals")].into_iter().filter(|(n, _)| *n > 0).map(|(n, w)| format!("{n} {w}")).collect();
        let way = if access.0 > 0 { format!(" (the stair down to the chest: {} dug, {} placed)", access.0, access.1) } else { String::new() };
        // the build minutes at the stage's prices (iteration 3): placing, digging and getting
        // every item; an item the table does not price fails the line
        let priced = site_bill(c, site, price_stage(c));
        let head = match &priced {
            Ok(bill) => format!("{:.0} min at day-1 prices", bill.seconds / 60.0),
            Err(e) => format!("none ({e})"),
        };
        let most = priced.as_ref().map(|bill| format!("; the most: {}", bill.line())).unwrap_or_default();
        metrics.push(("build time".to_string(), format!("{head} ({} placed{}, {} dug{way}, {lava_trips} lava trips, {} villagers moved{most})", effort.placed, if extras.is_empty() { String::new() } else { format!(" with {}", extras.join(" and ")) }, effort.dug, b.villagers)));
        if iron > 0.0 {
            let loss_per_hour = metrics.iter().find(|(k, _)| k == "thunder loss").and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(0.0) * redsim::weather::storms().share;
            let t0 = day1_minutes(&effort, b.iron, iron, 0.0);
            let t = day1_minutes(&effort, b.iron, iron, loss_per_hour * t0 / 60.0);
            metrics.push(("time to an iron kit".to_string(), format!("{t:.0} min")));
        }
    }
    // the numeric lines the stage cards bind to (brief revision 9, Stages: Common): the build
    // minutes at the stage's prices (iteration 3: the build's own iron mined in them, so the kit
    // owes the chest only its 24 iron)
    let priced = site_bill(c, site, price_stage(c));
    match &priced {
        Ok(bill) => {
            metrics.push(("build minutes".to_string(), format!("{:.1}", bill.seconds / 60.0)));
            if iron > 0.0 {
                metrics.push(("t kit minutes".to_string(), format!("{:.1}", bill.seconds / 60.0 + redsim::entity::mob::site::KIT_IRON / iron * 60.0)));
            }
        }
        Err(e) => metrics.push(("build minutes".to_string(), format!("none ({e})"))),
    }
    // a player who sleeps through every night (brief revision 9): only the golems before day
    // time 12642 come, and each game day lasts 12642 ticks of play
    let (by_day, golems_n): (u32, u32) = all.iter().fold((0, 0), |(d, g), r| (d + r.golems_by_day, g + r.golems));
    metrics.push(("iron per hour sleeping".to_string(), format!("{:.0}", if golems_n > 0 { iron * by_day as f64 / golems_n as f64 * 24000.0 / farm::SLEEPER_DAY as f64 } else { 0.0 })));
    let (lo, hi) = c.bounds();
    let volume = ((hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1)).max(1) as f64;
    metrics.push(("volume per module".to_string(), format!("{:.0}", volume / modules as f64)));
    let (sum, samples, peak) = all.iter().fold((0u64, 0u32, 0u32), |(s, n, p), r| (s + r.entities_sum, n + r.entity_samples, p.max(r.entities_peak)));
    if samples > 0 {
        metrics.push(("entities alive".to_string(), format!("{:.1}", sum as f64 / samples as f64)));
        metrics.push(("entities peak".to_string(), peak.to_string()));
    }
    // burnt iron (brief revision 9, iteration 11: a measured tradeoff, not a refusal): the iron
    // lost per dead golem, its share of the loot, how often a golem's iron burns
    let (burnt, drops_burnt, dropped, deaths) = all.iter().fold((0u32, 0u32, 0u32, 0u32), |(b, n, d, k), r| (b + r.iron_burnt, n + r.drops_burnt, d + r.iron_dropped, k + r.deaths));
    if burnt > 0 && deaths > 0 {
        metrics.push(("iron burnt per golem".to_string(), format!("{:.2} ({:.1}% of the loot; 1 golem in {:.0})", burnt as f64 / deaths as f64, 100.0 * burnt as f64 / dropped.max(1) as f64, deaths as f64 / drops_burnt.max(1) as f64)));
    }
    // the stage lines (brief, Stages: What every card reports)
    metrics.push(("tools".to_string(), b.tools_text()));
    metrics.push(("lava sources".to_string(), b.lava.to_string()));
    metrics.push(("materials".to_string(), b.stage_text()));
    metrics.push(("chunk columns".to_string(), chunk_columns(c).to_string()));
    let poppies = mean(&all.iter().map(|r| r.poppies_dropped as f64 / r.hours()).collect::<Vec<_>>());
    let (slots, hoppers) = storage_slots(c);
    let hours = if iron + poppies > 0.0 { (slots as f64 - 2.0).max(0.0) * 64.0 / (iron + poppies) } else { 0.0 };
    metrics.push(("storage".to_string(), format!("{hours:.1} h ({slots} slots; {poppies:.0} poppies/h)")));
    metrics.push(("hoppers per 1000 iron/h".to_string(), format!("{:.1}", if iron > 0.0 { hoppers as f64 * 1000.0 / iron } else { 0.0 })));
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The fewest chunk columns the design's footprint covers, over every placement on the chunk grid.
pub fn chunk_columns(c: &Circuit) -> u32 {
    let (lo, hi) = c.bounds();
    let cover = |a: i32, b: i32| (0..16).map(|o| ((b + o).div_euclid(16) - (a + o).div_euclid(16) + 1) as u32).min().unwrap_or(1);
    cover(lo[0], hi[0]) * cover(lo[2], hi[2])
}

/// The slots the farm can store its drops in (every chest, barrel and hopper of the design, and
/// the judge's chest on an air output), and its hoppers.
pub fn storage_slots(c: &Circuit) -> (u32, u32) {
    let d = &c.clock.design;
    let (mut slots, mut hoppers) = (0u32, 0u32);
    for s in d.blocks.values() {
        match s.split('[').next().unwrap_or("").trim_start_matches("minecraft:") {
            "chest" | "trapped_chest" | "barrel" => slots += 27,
            "hopper" => {
                slots += 5;
                hoppers += 1;
            }
            _ => {}
        }
    }
    if d.ports.get("output").is_some_and(|o| !d.blocks.contains_key(o)) {
        slots += 27;
    }
    (slots, hoppers)
}

/// The villager clusters of a farm (villagers within each other's 10-block search box, joined).
pub fn villager_clusters(spec: &FarmSpec) -> usize {
    let pos: Vec<[f64; 3]> = spec.entities.iter().filter(|(k, _, _)| k.ends_with("villager")).map(|(_, p, _)| *p).collect();
    let n = pos.len();
    let mut group: Vec<usize> = (0..n).collect();
    for a in 0..n {
        for b in 0..n {
            let near = (0..3).all(|k| (pos[a][k] - pos[b][k]).abs() < 10.0 + if k == 1 { 1.95 } else { 0.6 });
            if near {
                let (ga, gb) = (group[a], group[b]);
                for g in group.iter_mut() {
                    if *g == gb {
                        *g = ga;
                    }
                }
            }
        }
    }
    group.sort();
    group.dedup();
    group.len().max(1)
}

/// What a build costs in survival.
#[derive(Clone, Debug, Default)]
pub struct Bill {
    /// The iron, the tools the build needs counted (`tools`).
    pub iron: f64,
    pub quartz: u32,
    pub villagers: u32,
    pub beds: u32,
    pub zombies: u32,
    /// The tools the build needs and their iron (brief, Stages: the bucket for lava or water, the
    /// shears for leaves, an iron pickaxe for redstone dust).
    pub tools: Vec<(&'static str, u32)>,
    /// Lava sources (each one bucket trip).
    pub lava: u32,
    /// The materials that need the Nether (quartz, slime, magma, glowstone, obsidian...) and the
    /// End (shulker boxes, end rods...), with their counts.
    pub nether: std::collections::BTreeMap<&'static str, u32>,
    pub end: std::collections::BTreeMap<&'static str, u32>,
}

impl Bill {
    pub fn text(&self) -> String {
        let s = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        format!("{:.0} iron, {} quartz, {}, {}, {}", self.iron.ceil(), self.quartz, s(self.villagers, "villager", "villagers"), s(self.beds, "bed", "beds"), s(self.zombies, "zombie", "zombies"))
    }

    /// The tools line: `bucket 3 + shears 2` (none: `none`; ` + `, since a card's metrics are
    /// separated by commas).
    pub fn tools_text(&self) -> String {
        if self.tools.is_empty() {
            return "none".into();
        }
        self.tools.iter().map(|(t, n)| format!("{t} {n}")).collect::<Vec<_>>().join(" + ")
    }

    /// The latest stage its materials need, with the blocks that need it (`day 1`, `Nether:
    /// quartz in 2 blocks + slime in 4 blocks`).
    pub fn stage_text(&self) -> String {
        let list = |m: &std::collections::BTreeMap<&str, u32>| m.iter().map(|(k, n)| format!("{k} in {n} block{}", if *n == 1 { "" } else { "s" })).collect::<Vec<_>>().join(" + ");
        if !self.end.is_empty() {
            format!("End: {}", list(&self.end))
        } else if !self.nether.is_empty() {
            format!("Nether: {}", list(&self.nether))
        } else {
            "day 1".into()
        }
    }
}

/// Blocks whose recipe needs redstone dust (an iron pickaxe: `redstone_ore` is `needs_iron_tool`).
const DUST: &[&str] = &["redstone_wire", "redstone_torch", "redstone_wall_torch", "repeater", "comparator", "observer", "piston", "sticky_piston", "dispenser", "dropper", "note_block", "redstone_lamp", "target", "powered_rail", "detector_rail", "activator_rail", "crafter", "redstone_block"];

/// The materials of a block that need the Nether or the End (brief, Stages: the middle stage has
/// the Nether, the late one the End).
fn stage_materials(id: &str) -> (Option<&'static str>, Option<&'static str>) {
    match id {
        "comparator" | "observer" | "daylight_detector" | "quartz_block" | "smooth_quartz" | "quartz_slab" => (Some("quartz"), None),
        "slime_block" | "sticky_piston" => (Some("slime"), None),
        "magma_block" => (Some("magma"), None),
        "glowstone" | "redstone_lamp" => (Some("glowstone"), None),
        "obsidian" | "crying_obsidian" => (Some("obsidian"), None),
        "soul_sand" | "soul_soil" => (Some("soul sand"), None),
        "netherrack" | "nether_bricks" => (Some("netherrack"), None),
        id if id.ends_with("shulker_box") => (None, Some("shulker box")),
        "end_rod" => (None, Some("end rod")),
        "purpur_block" | "end_stone" | "end_stone_bricks" => (None, Some("end stone")),
        _ => (None, None),
    }
}

/// The survival cost of a design's blocks and entities: iron by recipe (hopper 5, piston 1,
/// iron trapdoor 4, minecart 5, rails 6 per 16, tripwire hooks 1 per 2) and the tools the build
/// needs (a bucket 3 for the lava and water, shears 2 for leaves, an iron pickaxe 3 for redstone
/// dust), quartz (daylight detector 3, observer 1, comparator 1), villagers, beds, zombies, lava
/// sources, and the Nether and End materials (brief, Stages).
pub fn bill(c: &Circuit) -> Bill {
    let d = &c.clock.design;
    let mut b = Bill::default();
    let (mut bucket, mut leaves, mut dust) = (false, false, false);
    for s in d.blocks.values() {
        let id = s.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
        match id {
            "hopper" => b.iron += 5.0,
            "piston" | "sticky_piston" => b.iron += 1.0,
            "iron_trapdoor" => b.iron += 4.0,
            "iron_block" => b.iron += 9.0,
            "rail" => b.iron += 6.0 / 16.0,
            "detector_rail" | "activator_rail" => b.iron += 1.0,
            "tripwire_hook" => b.iron += 0.5,
            "heavy_weighted_pressure_plate" => b.iron += 2.0,
            "iron_door" if s.contains("half=lower") => b.iron += 2.0,
            "crafter" => b.iron += 5.0,
            "daylight_detector" => b.quartz += 3,
            "observer" | "comparator" => b.quartz += 1,
            "lava" => {
                bucket = true;
                if s.contains("level=0") {
                    b.lava += 1;
                }
            }
            "water" => bucket = true,
            id if id.ends_with("_bed") && s.contains("part=head") => b.beds += 1,
            _ => {}
        }
        leaves |= id.ends_with("_leaves");
        dust |= DUST.contains(&id);
        let (nether, end) = stage_materials(id);
        if let Some(m) = nether {
            *b.nether.entry(m).or_default() += 1;
        }
        if let Some(m) = end {
            *b.end.entry(m).or_default() += 1;
        }
    }
    for (need, tool, iron) in [(bucket, "bucket", 3), (leaves, "shears", 2), (dust, "iron pickaxe", 3)] {
        if need {
            b.iron += iron as f64;
            b.tools.push((tool, iron));
        }
    }
    for e in &d.entities {
        match e.kind.trim_start_matches("minecraft:") {
            "villager" => b.villagers += 1,
            "zombie" => b.zombies += 1,
            "minecart" => b.iron += 5.0,
            "hopper_minecart" => b.iron += 10.0,
            _ => {}
        }
        if let Some(n) = &e.nbt
            && n.contains("minecraft:zombie")
        {
            b.zombies += 1;
        }
    }
    b
}

fn kind_of(c: &Circuit) -> Result<Kind> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    if !matches!(&kind, Kind::Study(_)) {
        bail!("{}: not a mob farm", kind.name());
    }
    Ok(kind)
}

/// The design's box (blocks only).
fn dims(c: &Circuit) -> [i32; 3] {
    let (lo, hi) = c.bounds();
    [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]
}

/// The whole judge on redsim: every build (rotations x positions), each with its seeds.
pub fn sim_verify(c: &Circuit, o: &Options) -> std::result::Result<VerifyReport, String> {
    judge(c, o, &JudgeOpts::default())
}

/// [`sim_verify`] with the judge's run options.
pub fn judge(c: &Circuit, o: &Options, jo: &JudgeOpts) -> std::result::Result<VerifyReport, String> {
    if let Some(sample) = terrain_sample(c)? {
        return terrain_judge(c, jo, sample);
    }
    let kind = kind_of(c).map_err(|e| e.to_string())?;
    let offsets = clock::offsets(o.positions.clamp(1, 2));
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let mut jobs = Vec::new();
    for t in &turns {
        let tc = c.turned_pure(*t).map_err(|e| format!("rotation {t}: {e:#}"))?;
        for off in &offsets {
            let rc = tc.translated(*off).map_err(|e| format!("{e:#}"))?;
            let spec = spec_of(&rc)?;
            jobs.push((*t, *off, rc, spec));
        }
    }
    let base = base_checks(&jobs[0].2, &jobs[0].3, jo);
    let runs: Vec<std::result::Result<Vec<RunReport>, String>> = std::thread::scope(|s| {
        let hs: Vec<_> = jobs.iter().enumerate().map(|(k, (_, _, _, spec))| s.spawn(move || build_runs(spec, jo, k as u32, WalkMode::Random))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a build panicked".to_string()))).collect()
    });
    let pooled: Vec<RunReport> = runs.iter().filter_map(|r| r.as_ref().ok()).flatten().cloned().collect();
    let mut builds = Vec::new();
    // the base checks' card lines (rain, lightning, restarts, the exit flood's reports) go on the
    // base build's card, or on the first build that passes when the base build failed (a
    // position-dependent farm still shows its weather: the site's cards read them)
    let mut base_used = false;
    for (k, ((t, off, rc, spec), r)) in jobs.iter().zip(runs).enumerate() {
        // mobinfra (vjeux 2026-11-08 08:26, "It's all about tradeoffs"): lightning is measured on
        // the card, never a refusal (`redsim::weather::lightning::check_farm` lists the bolts
        // that can fall on a build; their rates per thunderstorm become the card's lines)
        let result = match (k, r, &base) {
            (_, Err(e), _) => Err(e),
            (0, Ok(_), Err(e)) => Err(e.clone()),
            (_, Ok(runs), Ok(b)) if !base_used && (k == 0 || runs.iter().all(|r| r.failures.is_empty() && r.errors.is_empty())) => {
                base_used = true;
                Ok(BuildOk { outcome: outcome(rc, spec, &runs, &pooled, Some(b), None), dims: dims(rc), family: family.clone() })
            }
            (_, Ok(runs), _) => Ok(BuildOk { outcome: outcome(rc, spec, &runs, &[], None, None), dims: dims(rc), family: family.clone() }),
        };
        builds.push(Build { turns: *t, offset: *off, result });
    }
    let mut schedule = Schedule::new(0);
    schedule.ticks = jo.days * 24000;
    Ok(crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() }))
}

/// `circuit verify` (and so `circuit archive`): the redsim judge (the card), confirmed in the real
/// 26.3 server by the mob farms' game gate (mobinfra, `crate::mobfarm_game`: each build for game
/// days with the weather on, on Hard).
pub fn verify_in_pool(_pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let rep = sim_verify(c, o).map_err(|e| anyhow!(e))?;
    crate::mobfarm_game::confirm(c, &o.version, rep)
}

/// `mctas circuit run`: one build's runs (`base`: golems walk at random; `still`: they all stand
/// still), with the base checks.
fn run_kind(_pool: Option<&Pool>, c: &Circuit, at: &RunAt) -> Result<Outcome> {
    let walk = match at.run {
        "base" => WalkMode::Random,
        "still" => WalkMode::Fixed(Walk::Still),
        r => bail!("mob-farm: no run {r:?} (base, still)"),
    };
    let rc = c.turned_pure(at.turns)?.translated(at.offset)?;
    let spec = spec_of(&rc).map_err(|e| anyhow!(e))?;
    let jo = JudgeOpts::default();
    let base = base_checks(&rc, &spec, &jo).map_err(|e| anyhow!(e))?;
    let runs = build_runs(&spec, &jo, 0, walk).map_err(|e| anyhow!(e))?;
    Ok(outcome(&rc, &spec, &runs, &[], Some(&base), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tier is the kind's last word (one site page per tier): day-1, middle, fastest, or none.
    #[test]
    fn the_tier_is_part_of_the_kind() {
        assert_eq!(parse_kind(&["iron_ingot", "middle"]).unwrap(), vec!["iron_ingot".to_string(), "middle".to_string()]);
        assert_eq!(parse_kind(&["minecraft:iron_ingot"]).unwrap(), vec!["iron_ingot".to_string()]);
        assert!(parse_kind(&["iron_ingot", "slowest"]).is_err());
        assert!(parse_kind(&["iron_ingot", "middle", "extra"]).is_err());
        let args = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let m = |k: &str| if k == "modules" { "4".to_string() } else { String::new() };
        let axis = STUDY.kinds[0].axis;
        assert_eq!(axis(&args(&["iron_ingot", "day-1"]), &m), "day 1");
        assert_eq!(axis(&args(&["iron_ingot", "fastest"]), &m), "fastest");
        assert_eq!(axis(&args(&["iron_ingot"]), &m), "4 modules");
        let title = STUDY.kinds[0].site.title;
        assert!(title("mob-farm iron_ingot fastest").starts_with("Fastest iron farm"));
        assert_eq!(title("mob-farm iron_ingot"), "Iron farm");
    }
}
