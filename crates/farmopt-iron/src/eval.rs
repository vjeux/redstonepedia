//! The iron farm's staged evaluation (designs/briefs/farm-optimizer.md, "The evaluation ladder"
//! and "The bound"): static (the design built in the void or in the day-1 site: refusals, the
//! scared villagers' exact spawn cells, the mark-success bound, the sure sightings, counts and
//! build time), the proven rate bound, quick runs (4 x 1 game day), the full judge
//! (`circuit::mobfarm`'s pieces, so the runs feed the lightning measure for the net rate).

use crate::cand::Cand;
use crate::parts::{Kill, Place};
use circuit::mobfarm::{self, JudgeOpts};
use farmopt::{Evaluation, Metrics, Value};
use redsim::entity::mob::farm::{self as rfarm, FarmSpec, RunOpts, RunReport, WalkMode};
use redsim::entity::mob::site::{self, Site};
use redsim::{BlockPos, World};
use std::collections::BTreeMap;

/// Per-cluster ceiling: one golem per 600 gt, 4 iron each (uniform(3, 5)).
pub const CEILING: f64 = 480.0;
/// The smallest follow range (16 x (1 - 0.11485)) and the golem / villager half-widths' sum.
pub const R_MIN: f64 = 14.16;
/// The day-1 terrain sample.
pub const SAMPLE: &str = "village-edge";
/// The iron of a full iron kit (5 + 8 + 7 + 4).
pub const KIT_IRON: f64 = 24.0;

/// What the evaluation of one candidate shares between its levels.
pub struct Ctx {
    pub calm_parts: Vec<crate::calm::Part>,
    /// The stage's prices column (`price::stage_ix`).
    pub stage: usize,
}

fn ex(v: f64) -> Value {
    Value::exact(v)
}

/// The circuit and the judge's spec of a candidate.
pub fn spec_of(c: &Cand, ctx: &Ctx) -> Result<(circuit::Circuit, FarmSpec, crate::design::Built), String> {
    let built = crate::design::build(c, &ctx.calm_parts).map_err(|e| format!("{e:#}"))?;
    let d = mctas::design::Design::parse(&built.text).map_err(|e| format!("{e:#}"))?;
    let circ = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let spec = mobfarm::spec_of(&circ)?;
    Ok((circ, spec, built))
}

/// The day-1 site of a build in the terrain: priced at the 12 nearest valid spots of the sample,
/// the cheapest kept (the day-1 lane's `site` API; every site shares the sample's one world).
/// `clear_box`: a void build on the surface (its box cleared to the sky), else dug cell by cell.
pub fn site_of(spec: &FarmSpec, ground: i32, clear_box: bool) -> Result<Site, String> {
    let sample = redsim::terrain::samples::sample(SAMPLE).ok_or("no terrain sample")?;
    let shared = site::sample_world_shared(sample)?;
    let priced = {
        let world = shared.read().unwrap_or_else(|e| e.into_inner());
        let fp = site::footprint(spec, ground)?;
        let spots = spots_cached(&world, sample, fp);
        if spots.is_empty() {
            return Err(format!("no spot for a {}x{} footprint {} deep in {SAMPLE}", fp.size[0], fp.size[1], fp.below));
        }
        site::price_all(&world, spec, fp, ground, clear_box, &spots)
    };
    let k = priced.len();
    let afk = [2.5, ground as f64 + 1.0, 4.5];
    Ok(Site::at_priced(shared, sample, priced, k, afk, clear_box))
}

fn spots_cached(w: &World, sample: &'static redsim::terrain::samples::Sample, fp: site::Footprint) -> Vec<([i32; 2], i32)> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<BTreeMap<([i32; 2], i32), Vec<([i32; 2], i32)>>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().ok().and_then(|m| m.get(&(fp.size, fp.below)).cloned()) {
        return v;
    }
    let v = site::spots_for(w, sample, fp);
    if let Ok(mut m) = cache.lock() {
        m.insert((fp.size, fp.below), v.clone());
    }
    v
}

/// Run `f` with the site as the `'static` reference `RunOpts` takes; the site is dropped after
/// (`farm::run` keeps no reference past its return).
pub fn with_site<T>(site: Site, f: impl FnOnce(&'static Site) -> T) -> T {
    let raw = Box::into_raw(Box::new(site));
    // SAFETY: `raw` is a live allocation for the whole call; `f` (a farm run) holds the reference
    // only while it runs, and the box is rebuilt and dropped once it returned.
    let r: &'static Site = unsafe { &*raw };
    let out = f(r);
    unsafe { drop(Box::from_raw(raw)) };
    out
}

/// The clusters (Villager.spawnGolemIfNeeded's 10-box: centres within 10.6 on x and z, feet within
/// 11.95 on y), by villager index into `vs` (feet positions).
pub fn clusters(vs: &[[f64; 3]]) -> Vec<usize> {
    let n = vs.len();
    let mut g: Vec<usize> = (0..n).collect();
    fn root(g: &mut [usize], mut i: usize) -> usize {
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for a in 0..n {
        for b in 0..n {
            if (vs[a][0] - vs[b][0]).abs() < 10.6 && (vs[a][2] - vs[b][2]).abs() < 10.6 && (vs[a][1] - vs[b][1]).abs() < 11.95 {
                let (ra, rb) = (root(&mut g, a), root(&mut g, b));
                g[ra] = rb;
            }
        }
    }
    (0..n).map(|i| root(&mut g, i)).collect()
}

/// Whether a box of golem positions (centres x, z and feet y ranges) lies inside villager `v`'s
/// smallest follow cube (R = 14.16: |dx|, |dz| < R + 1.0, feet in (vy - R - 2.7, vy + 1.95 + R)).
fn in_smallest_cube(v: [f64; 3], gx: (f64, f64), gy: (f64, f64), gz: (f64, f64)) -> bool {
    let h = R_MIN + 1.0;
    let okx = (gx.0 - v[0]).abs() < h && (gx.1 - v[0]).abs() < h;
    let okz = (gz.0 - v[2]).abs() < h && (gz.1 - v[2]).abs() < h;
    let oky = gy.0 > v[1] - R_MIN - 2.7 && gy.1 < v[1] + 1.95 + R_MIN;
    okx && oky && okz
}

/// The static facts of the built world: per cluster (its villagers, scared summoners and the mark
/// success bound p), the sure-sighting flag, and the off-pad spawn cells (refused in the void).
struct Spawn {
    clusters: Vec<(Vec<usize>, f64, bool)>,
    stray: Vec<BlockPos>,
    worst_f: f64,
}

fn spawn_facts(w: &World, spec: &FarmSpec, c: &Cand, built: &crate::design::Built, offset: [i32; 3]) -> Spawn {
    use redsim::entity::EntityKind;
    use redsim::entity::mob::types::MobType;
    let vs: Vec<(usize, [f64; 3], BlockPos)> = w.entities.list.iter().enumerate().filter(|(_, e)| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager)).map(|(i, e)| (i, [e.pos.x, e.pos.y, e.pos.z], e.block_position())).collect();
    let feet: Vec<[f64; 3]> = vs.iter().map(|v| v.1).collect();
    let roots = clusters(&feet);
    // the blocks where they are in `w` (a site moved the design by `offset`)
    let moved: Vec<(BlockPos, redsim::Block)> = spec.blocks.iter().map(|(p, b)| (BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]), *b)).collect();
    let summoners = site::panicking(w, &moved);
    let pad: std::collections::HashSet<BlockPos> = built.ports.iter().flat_map(|p| p.pad_cells.iter().map(|q| BlockPos::new(q[0] + offset[0], q[1] + 1 + offset[1], q[2] + offset[2]))).collect();
    let mut stray = Vec::new();
    let mut f_of: BTreeMap<usize, f64> = BTreeMap::new();
    let mut worst_f: f64 = 1.0;
    for (k, (_, _, cell)) in vs.iter().enumerate() {
        if !summoners.contains(cell) {
            continue;
        }
        let cells = redsim::entity::mob::spawn::golem_spawn_cells_from(w, *cell);
        let ok: Vec<BlockPos> = cells.iter().filter(|(_, r)| r.is_ok()).map(|(p, _)| *p).collect();
        for p in &ok {
            if !pad.contains(p) && !stray.contains(p) {
                stray.push(*p);
            }
        }
        let f = ok.len() as f64 / 289.0;
        worst_f = worst_f.min(f);
        f_of.insert(k, f);
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (k, r) in roots.iter().enumerate() {
        groups.entry(*r).or_default().push(k);
    }
    let mut out = Vec::new();
    for (_, members) in groups {
        let miss: f64 = members.iter().filter_map(|k| f_of.get(k)).map(|f| (1.0 - f).powi(10)).product();
        let p = 1.0 - miss;
        // sure sightings: an in-sight pit whose whole box lies in the smallest cube of at least
        // n - 2 of the cluster's villagers (the golem stays there until its lava death, >= 240 gt)
        let sure = match (c.module.kill, built.ports.iter().find_map(|p| p.pit)) {
            (Kill::Pit { .. }, Some((lo, hi))) => {
                let (lo, hi) = ([lo[0] + offset[0], lo[1] + offset[1], lo[2] + offset[2]], [hi[0] + offset[0], hi[1] + offset[1], hi[2] + offset[2]]);
                let gx = (lo[0] as f64 + 0.7, hi[0] as f64 + 1.0 - 0.7);
                let gz = (lo[2] as f64 + 0.7, hi[2] as f64 + 1.0 - 0.7);
                let gy = (lo[1] as f64, hi[1] as f64);
                let inside = members.iter().filter(|&&k| in_smallest_cube(feet[k], gx, gy, gz)).count();
                members.len() >= 3 && inside + 2 >= members.len()
            }
            _ => false,
        };
        out.push((members, p, sure));
    }
    Spawn { clusters: out, stray, worst_f }
}

/// A cluster's long-run rate bound (iron/h): 480 x 600 / (600 + 100 (1 - p) / p + 100 [sure]).
pub fn cluster_bound(p: f64, sure: bool) -> f64 {
    if p <= 0.0 {
        return 0.0;
    }
    CEILING * 600.0 / (600.0 + 100.0 * (1.0 - p) / p + if sure { 100.0 } else { 0.0 })
}

/// Static evaluation.
pub fn eval_static(c: &Cand, ctx: &Ctx) -> Result<Evaluation, String> {
    let (circ, spec, built) = spec_of(c, ctx)?;
    let bill = mobfarm::bill(&circ);
    let mut m = Metrics::new();
    let mut lines = BTreeMap::new();
    let placed = circ.clock.design.blocks.values().filter(|s| !matches!(s.split('[').next().unwrap_or("").trim_start_matches("minecraft:"), "air" | "cave_air" | "void_air")).count() as f64;
    let (w, offset, effort) = match c.place {
        Place::Void => {
            let refused = redsim::entity::mob::refuse::static_checks(&spec);
            if !refused.is_empty() {
                return Err(format!("refused: {}", refused.join("; ")));
            }
            let w = rfarm::build(&spec, &RunOpts::steady(1, 0, WalkMode::Random))?;
            (w, [0, 0, 0], site::Effort { placed: placed as u32, dug: 0, lava: bill.lava, villagers: bill.villagers })
        }
        Place::Dug => {
            let ground = built.ground.ok_or("a dug design without its ground")?;
            let st = site_of(&spec, ground, c.surface || c.l3_pits > 0)?;
            let effort = st.effort(bill.lava, bill.villagers);
            let moved = st.spec(&spec);
            let offset = st.offset;
            lines.insert("spot".into(), format!("{SAMPLE} at offset {:?}: {} placed + {} covers, {} dug; the cheapest of {} spots ({:.0}-{:.0} s of blocks)", st.offset, st.placed, st.fills.len(), st.dug, st.spots, st.build_range.0, st.build_range.1));
            let w = with_site(st, |s| {
                // the terrain judge's refusals (the day-1 lane's `static_checks_in`)
                let refused = redsim::entity::mob::refuse::static_checks_in(&moved, Some(s));
                if !refused.is_empty() {
                    return Err(format!("refused: {}", refused.join("; ")));
                }
                let mut o = RunOpts::steady(1, 0, WalkMode::Random);
                o.site = Some(s);
                rfarm::build(&moved, &o)
            })?;
            m.insert("blocks_dug".into(), ex(effort.dug as f64));
            (w, offset, effort)
        }
    };
    let sp = spawn_facts(&w, &spec, c, &built, offset);
    if c.place == Place::Void && !sp.stray.is_empty() {
        return Err(format!("{} golem spawn cells off the pads (first {:?}): a part is a spawn base", sp.stray.len(), sp.stray[0]));
    }
    if sp.clusters.iter().any(|(_, p, _)| *p <= 0.0) {
        return Err("a cluster whose scared villagers' searches find no spawn cell".into());
    }
    let bound: f64 = sp.clusters.iter().map(|(_, p, s)| cluster_bound(*p, *s)).sum();
    let n_clusters = sp.clusters.len() as f64;
    let (lo, hi) = circ.bounds();
    let volume = ((hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1)) as f64;
    // build minutes at the stage's prices (ironspec iteration 3): every block placed, the site's
    // glass covers, the natural blocks dug, the items, lava, villagers and the zombie
    let states: Vec<&str> = circ.clock.design.blocks.values().map(|s| s.as_str()).collect();
    let ents: Vec<(&str, &str)> = circ.clock.design.entities.iter().map(|e| (e.kind.as_str(), e.nbt.as_deref().unwrap_or(""))).collect();
    let covers: Vec<&str> = vec!["minecraft:glass"; effort.placed.saturating_sub(placed as u32) as usize];
    let priced = crate::price::bill(&states, &ents, effort.dug, &covers, ctx.stage)?;
    let build_s = priced.seconds;
    lines.insert("prices".into(), format!("{:.1} min at the {} prices: {}", build_s / 60.0, ["day-1", "middle", "late"][ctx.stage], priced.line()));
    m.insert("blocks".into(), ex(effort.placed as f64));
    m.insert("build_minutes".into(), ex(build_s / 60.0));
    m.insert("bill_iron".into(), ex(bill.iron.ceil()));
    m.insert("quartz".into(), ex(bill.quartz as f64));
    m.insert("lava_sources".into(), ex(bill.lava as f64));
    m.insert("villagers".into(), ex(bill.villagers as f64));
    m.insert("materials".into(), ex(if !bill.end.is_empty() { 3.0 } else if !bill.nether.is_empty() { 2.0 } else { 1.0 }));
    m.insert("modules".into(), ex(n_clusters));
    m.insert("shared_parts".into(), ex(if c.shared_calm && c.modules > 1 { 1.0 } else { 0.0 }));
    m.insert("volume".into(), ex(volume));
    m.insert("volume_per_module".into(), ex(volume / n_clusters.max(1.0)));
    m.insert("build_minutes_per_module".into(), ex(build_s / 60.0 / n_clusters.max(1.0)));
    m.insert("blocks_per_module".into(), ex(effort.placed as f64 / n_clusters.max(1.0)));
    // one AFK spot runs every module when the farm fits in the entity-ticking square of the
    // simulation distance (10 chunks: 19 x 19 chunks around the player's; 18 chunks = 288 blocks
    // whatever the farm's alignment to the chunk grid)
    let extent = (hi[0] - lo[0] + 1).max(hi[2] - lo[2] + 1) as f64;
    m.insert("afk_spots".into(), ex((extent / 288.0).ceil().max(1.0)));
    m.insert("chunk_columns".into(), ex(mobfarm::chunk_columns(&circ) as f64));
    m.insert("mark_success".into(), ex(sp.clusters.iter().map(|x| x.1).fold(1.0, f64::min)));
    m.insert("spawn_share".into(), ex(sp.worst_f));
    m.insert("sure_sightings".into(), ex(sp.clusters.iter().filter(|x| x.2).count() as f64));
    m.insert("rate_bound".into(), ex(bound));
    lines.insert("dims".into(), format!("{} x {} x {}", hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1));
    lines.insert("bill".into(), bill.text());
    Ok(Evaluation { metrics: m, lines })
}

/// The proven optimistic values (designs/briefs/farm-optimizer.md, "The bound").
pub fn bound(st: &Metrics) -> Metrics {
    let g = |k: &str| st.get(k).map(|v| v.v).unwrap_or(f64::NAN);
    let rate = g("rate_bound");
    let mut b = Metrics::new();
    let at_most = |x: f64| Value { v: x, lo: f64::NEG_INFINITY, hi: x };
    let at_least = |x: f64| Value { v: x, lo: x, hi: f64::INFINITY };
    b.insert("iron_per_hour".into(), at_most(rate));
    b.insert("iron_per_hour_per_module".into(), at_most(rate / g("modules").max(1.0)));
    b.insert("iron_per_hour_per_block".into(), at_most(rate / g("blocks").max(1.0)));
    // T >= the build (its iron mined in it at the stage's prices: iteration 3) + the earliest 24
    // iron: 5 golems (5 iron at most each) one per 600 gt per cluster from the first mark, so at
    // least 4 x 600 gt = 2 min, and at least 24 iron at the bound's rate
    let t_lb = g("build_minutes") + (KIT_IRON / rate.max(1e-9) * 60.0).max(4.0 * 600.0 / 1200.0 / g("modules").max(1.0));
    b.insert("t_kit_minutes".into(), at_least(t_lb));
    b.insert("share_of_ceiling".into(), at_most(rate / (CEILING * g("modules").max(1.0))));
    b
}

/// The quick level: 4 runs of 1 game day (steady state, golems walking at random).
pub fn eval_quick(c: &Cand, ctx: &Ctx, st: &Metrics) -> Result<Evaluation, String> {
    let (_circ, spec, built) = spec_of(c, ctx)?;
    let reports: Vec<RunReport> = match c.place {
        Place::Void => (0..4).map(|k| rfarm::run(&spec, &rfarm::judge_opts(rfarm::judge_seed(0, k), 24000, WalkMode::Random))).collect::<Result<_, _>>()?,
        Place::Dug => {
            let st_site = site_of(&spec, built.ground.unwrap_or(8), c.surface || c.l3_pits > 0)?;
            let moved = st_site.spec(&spec);
            with_site(st_site, |s| {
                (0..4)
                    .map(|k| {
                        let o = rfarm::judge_opts_in(rfarm::judge_seed(0, k), 24000, WalkMode::Random, Some(s));
                        rfarm::run(&moved, &o)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })?
        }
    };
    if let Some(f) = reports.iter().flat_map(|r| r.failures.iter().chain(r.errors.iter())).next() {
        return Err(f.clone());
    }
    Ok(rates(c, st, &spec, &reports, &reports, None, ctx.stage))
}

/// The rate metrics of a set of runs: the gross rate with its interval (over `runs`), the net
/// rate (the stage's rule: day 1 less the iron lightning destroys; middle and late the break
/// hazard over W, from the lightning measure of `lightning_runs`, runs of `spec` itself), the
/// day-1 T at that rate.
fn rates(c: &Cand, st: &Metrics, spec: &FarmSpec, runs: &[RunReport], lightning_runs: &[RunReport], card: Option<&[(String, String)]>, stage: usize) -> Evaluation {
    let g = |k: &str| st.get(k).map(|v| v.v).unwrap_or(f64::NAN);
    let xs: Vec<f64> = runs.iter().map(|r| r.iron_per_hour()).collect();
    let gross = Value::sampled(&xs);
    let golems = Value::sampled(&runs.iter().map(|r| r.golems_per_hour()).collect::<Vec<_>>());
    let mut m = Metrics::new();
    let mut lines = BTreeMap::new();
    m.insert("iron_per_hour_gross".into(), gross);
    m.insert("golems_per_hour".into(), golems);
    let storms = redsim::weather::storms();
    let (destroyed, hazard) = match redsim::weather::lightning::measure_farm(spec, lightning_runs) {
        Ok((me, _sky)) => (me.iron * storms.share * 72000.0, me.breaks * storms.share * 72000.0),
        Err(e) => {
            lines.insert("lightning".into(), format!("not measured: {e}"));
            (0.0, 0.0)
        }
    };
    // the stage's net (brief rev 9): day 1 loses the destroyed iron; middle and late lose what a
    // break stops over one game day (middle) or an 8-hour absence (late)
    let stage_net = |r: f64| -> f64 {
        if stage == 0 {
            return (r - destroyed).max(0.0);
        }
        let w = if stage == 2 { 8.0 } else { 1.0 / 3.0 };
        let lw = hazard * w;
        if lw < 1e-12 { r } else { r * (1.0 - (-lw).exp()) / lw }
    };
    let net = Value { v: stage_net(gross.v), lo: stage_net(gross.lo), hi: stage_net(gross.hi) };
    m.insert("iron_per_hour".into(), net);
    let n = g("modules").max(1.0);
    m.insert("iron_per_hour_per_module".into(), Value { v: net.v / n, lo: net.lo / n, hi: net.hi / n });
    m.insert("iron_destroyed_per_hour".into(), Value::exact(destroyed));
    m.insert("break_hazard_per_hour".into(), Value::exact(hazard));
    m.insert("iron_per_hour_per_block".into(), Value { v: net.v / g("blocks").max(1.0), lo: net.lo / g("blocks").max(1.0), hi: net.hi / g("blocks").max(1.0) });
    m.insert("share_of_ceiling".into(), Value { v: golems.v / (120.0 * g("modules").max(1.0)), lo: golems.lo / (120.0 * g("modules").max(1.0)), hi: golems.hi / (120.0 * g("modules").max(1.0)) });
    // T (iteration 3): the build, its iron mined in it, then 24 iron at the rate (the measured T of
    // the terrain judge replaces it at the full level)
    let t = |r: f64| g("build_minutes") + KIT_IRON / r.max(1e-9) * 60.0;
    m.insert("t_kit_minutes".into(), Value { v: t(net.v), lo: t(net.hi), hi: t(net.lo) });
    if let Some(card) = card {
        for (k, v) in card {
            lines.insert(k.replace(' ', "_"), v.clone());
        }
        // the card rows' numbers (designs/circuits/mobfarm/cards.tsv) from the judge's lines
        let num = |s: &str| s.trim().split(|ch: char| !(ch.is_ascii_digit() || ch == '.')).find(|t| !t.is_empty()).and_then(|t| t.parse::<f64>().ok());
        let find = |k: &str| card.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        if let Some(r) = find("rain") {
            for part in r.split(';') {
                let part = part.trim();
                if let Some(rest) = part.strip_prefix("rain day") {
                    if let Some(v) = num(rest) {
                        m.insert("rain_iron_per_hour".into(), Value::exact(stage_net(v)));
                    }
                } else if let Some(rest) = part.strip_prefix("thunder day")
                    && let Some(v) = num(rest)
                {
                    m.insert("thunder_iron_per_hour".into(), Value::exact(stage_net(v)));
                }
            }
        }
        if let Some(v) = find("storage").and_then(num) {
            m.insert("storage_hours".into(), Value::exact(v));
        }
        if let Some(v) = find("chunk columns").and_then(num) {
            m.insert("chunk_columns".into(), Value::exact(v));
        }
        if let Some(a) = find("restarts after days away") {
            lines.insert("days_away".into(), if a.starts_with("yes") { "Starts again by itself when you come back after days away".into() } else { "After days away, it waits for you to hide the zombie by hand one night".into() });
        }
        if let Some(a) = find("restarts after a missed night") {
            lines.insert("first_night".into(), if a.starts_with("yes") { "Starts by itself on its first night".into() } else { "On its first night, hide the zombie by hand once the villagers lie down".into() });
        }
    }
    Evaluation { metrics: m, lines }
}

/// The full judge (void): `circuit::mobfarm`'s base checks and its 8 builds x 8 seeds x 5 days,
/// the card from its `outcome`, the net rate from build 0's runs' lightning measure.
pub fn eval_full(c: &Cand, ctx: &Ctx, st: &Metrics) -> Result<Evaluation, String> {
    if c.place == Place::Dug {
        return eval_full_terrain(c, ctx, st);
    }
    let (circ, _spec, _) = spec_of(c, ctx)?;
    let jo = JudgeOpts { threads: 16, ..JudgeOpts::default() };
    let mut builds = Vec::new();
    for t in [0u8, 1, 2, 3] {
        let tc = circ.turned_pure(t).map_err(|e| format!("rotation {t}: {e:#}"))?;
        for off in clock::offsets(2) {
            let rc = tc.translated(off).map_err(|e| format!("{e:#}"))?;
            let sp = mobfarm::spec_of(&rc)?;
            builds.push((rc, sp));
        }
    }
    let base = mobfarm::base_checks(&builds[0].0, &builds[0].1, &jo)?;
    if let Some(f) = base.failures().first() {
        return Err(f.clone());
    }
    // the 8 builds at once (each runs its seeds at once: the judge's own `sim_verify` does the
    // same), their results read in build order, so the first failing build is the same one
    // (tickexact 2026-11-08: one build after the other left most of the machine idle)
    let results: Vec<std::result::Result<Vec<RunReport>, String>> = std::thread::scope(|s| {
        let hs: Vec<_> = builds.iter().enumerate().map(|(k, (_, sp))| {
            let jo = &jo;
            s.spawn(move || mobfarm::build_runs(sp, jo, k as u32, WalkMode::Random))
        }).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a build panicked".to_string()))).collect()
    });
    let mut all: Vec<Vec<RunReport>> = Vec::new();
    for (k, runs) in results.into_iter().enumerate() {
        let runs = runs?;
        if let Some(f) = runs.iter().flat_map(|r| r.failures.iter().chain(r.errors.iter())).next() {
            return Err(format!("build {k}: {f}"));
        }
        all.push(runs);
    }
    let pooled: Vec<RunReport> = all.iter().flatten().cloned().collect();
    let out = mobfarm::outcome(&builds[0].0, &builds[0].1, &all[0], &pooled, Some(&base), None);
    if !out.pass {
        return Err(out.why);
    }
    // the rate over all 64 runs (the card's interval), the lightning measure over build 0's runs
    // (runs of build 0's own spec, as the judge measures it)
    let mut e = rates(c, st, &builds[0].1, &pooled, &all[0], Some(&out.metrics), ctx.stage);
    // the judged build's design, kept with its numbers (any front member can be shown after the
    // code moves on)
    if let Ok(b) = crate::design::build(c, &ctx.calm_parts) {
        e.lines.insert("design_text".into(), b.text);
    }
    Ok(e)
}

/// The full judge in the terrain (the day-1 lane's `circuit::mobfarm::judge`: the design at each
/// of the nearest valid spots of village-edge, 8 seeds x 5 days each, natural spawning on Hard,
/// the weather's cycle, the base checks at the cheapest spot); the card's numbers become metrics:
/// the rate and its 95% interval, the measured `t kit minutes`, the lightning loss (day 1's net
/// subtracts the iron it destroys).
fn eval_full_terrain(c: &Cand, ctx: &Ctx, st: &Metrics) -> Result<Evaluation, String> {
    let (circ, _spec, built) = spec_of(c, ctx)?;
    let jo = JudgeOpts { threads: 16, ..JudgeOpts::default() };
    let rep = mobfarm::judge(&circ, &circuit::verify::Options::default(), &jo)?;
    if !rep.pass {
        return Err(format!("terrain judge: {}", rep.verdict));
    }
    let base = rep.base().ok_or("terrain judge: no base build")?;
    let card: Vec<(String, String)> = base.outcome.metrics.clone();
    let find = |k: &str| card.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let num = |s: &str| s.trim().split(|ch: char| !(ch.is_ascii_digit() || ch == '.')).find(|t| !t.is_empty()).and_then(|t| t.parse::<f64>().ok());
    let rate = find("iron per hour").as_deref().and_then(num).ok_or("terrain judge: no iron per hour")?;
    let (lo, hi) = find("iron range").map(|r| {
        // "95% X-Y (N runs)"
        let v: Vec<f64> = r.split(|ch: char| !(ch.is_ascii_digit() || ch == '.')).filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect();
        (v.get(1).copied().unwrap_or(rate), v.get(2).copied().unwrap_or(rate))
    }).unwrap_or((rate, rate));
    // the iron lightning destroys, per hour: per storm times storms per hour
    let lost_per_storm = find("lightning").and_then(|l| l.split("about ").nth(1).and_then(num)).unwrap_or(0.0);
    let storms = redsim::weather::storms();
    let destroyed = lost_per_storm * storms.share * 72000.0 / storms.mean_len.max(1.0);
    let net = |r: f64| (r - destroyed).max(0.0);
    let mut m = Metrics::new();
    let mut lines = BTreeMap::new();
    m.insert("iron_per_hour_gross".into(), Value { v: rate, lo, hi });
    m.insert("iron_per_hour".into(), Value { v: net(rate), lo: net(lo), hi: net(hi) });
    m.insert("iron_destroyed_per_hour".into(), Value::exact(destroyed));
    let n = st.get("modules").map(|v| v.v).unwrap_or(1.0).max(1.0);
    m.insert("iron_per_hour_per_module".into(), Value { v: net(rate) / n, lo: net(lo) / n, hi: net(hi) / n });
    if let Some(t) = find("t kit minutes").as_deref().and_then(num) {
        m.insert("t_kit_minutes".into(), Value::exact(t));
    }
    if let Some(b) = find("build minutes").as_deref().and_then(num) {
        lines.insert("judge_build_minutes".into(), format!("{b:.1}"));
    }
    for (k, v) in &card {
        lines.insert(k.replace(' ', "_"), v.clone());
    }
    let _ = built;
    if let Ok(b) = crate::design::build(c, &ctx.calm_parts) {
        lines.insert("design_text".into(), b.text);
    }
    Ok(Evaluation { metrics: m, lines })
}
