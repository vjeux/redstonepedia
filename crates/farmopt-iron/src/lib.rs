//! The iron farm on the farm optimizer (designs/briefs/farm-optimizer.md): its grammar as parts
//! ([`parts`]: cage, pad, kill, collection; [`calm`]: the catalog timing part), its candidates
//! ([`cand`]), their design files ([`design`]), their evaluation and bound ([`eval`]), and the
//! `mctas farmopt` commands ([`cli`]).

pub mod calm;
pub mod cand;
pub mod catalogs;
pub mod cli;
pub mod price;
pub mod design;
pub mod eval;
pub mod parts;
pub mod sight;
pub mod split;

use cand::Cand;
use farmopt::spec::Spec;
use farmopt::{Evaluation, Metrics};

/// The iron farm as a [`farmopt::Farm`].
pub struct IronFarm {
    pub ctx: eval::Ctx,
    /// Also enumerate the stage's exact seeds (calibration points: they never enter a front).
    pub seeds: bool,
    /// The copies bar's metrics per N (the single module's judged rate, N copies priced together).
    bar_cache: std::sync::Mutex<std::collections::HashMap<u8, Option<farmopt::Metrics>>>,
}

impl IronFarm {
    pub fn new(stage: &str) -> IronFarm {
        let _ = design::STAGE.set(price::stage_ix(stage));
        IronFarm { ctx: eval::Ctx { calm_parts: calm::parts(stage), stage: price::stage_ix(stage) }, seeds: false, bar_cache: Default::default() }
    }

    /// The copies bar's module: the grammar's own single M2-open-ring module (3 watching
    /// villagers, a 6 x 6 pad 7 below, the fire-safe blade 22 below, the smallest Nether timer),
    /// built exactly like every candidate.
    pub fn bar_module() -> Cand {
        use crate::parts::{Cage, CalmKind, Kill, Module, Pad, Place};
        let m = Module { cage: Cage { scared: 3, blind: 0, calm: CalmKind::Observer, u1_west: false, regular: false }, pad: Pad { n: 6, floor: -7, dx: 0 }, kill: Kill::Blade { k: -22 } };
        Cand { seed: None, module: m, modules: 1, tiling: cand::Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0, family_gen: 0, chain: 0 }
    }

    /// The bar's module: the spec's `bar: <key>` (a single module of the stage), else the
    /// grammar's M2-open-ring module.
    pub fn bar_cand(spec: &Spec) -> Cand {
        spec.extra.get("bar").and_then(|k| cand::single_from_key(&spec.stage, k.trim())).unwrap_or_else(Self::bar_module)
    }

    /// The single bar module judged in full and its statics (cached on disk per stage, version
    /// and module: target/farmopt/bar-<stage>-<version>-<key>.json).
    fn bar_single(&self, spec: &Spec) -> Option<(farmopt::Metrics, farmopt::Metrics)> {
        let c = Self::bar_cand(spec);
        let cache = std::path::PathBuf::from(format!("target/farmopt/bar-{}-{}-{}.json", spec.stage, version_of(self.ctx.stage), cand::key(&c)));
        if let Ok(t) = std::fs::read_to_string(&cache)
            && let Some(v) = farmopt::metrics_pair_from_json(&t)
        {
            return Some(v);
        }
        let st = eval::eval_static(&c, &self.ctx).ok()?;
        let full = eval::eval_full(&c, &self.ctx, &st.metrics).ok()?;
        let v = (full.metrics, st.metrics);
        if let Some(t) = farmopt::metrics_pair_to_json(&v) {
            let _ = std::fs::write(&cache, t);
        }
        Some(v)
    }
}

/// The evaluator's version: bump it whenever the grammar or the evaluation changes (the journal
/// keeps no entry of another version).
/// The site's card renderer (`site::cards::render` with no other designs), set by the CLI that
/// links the site; the front reports render each member's card with it when set.
pub type CardRender = fn(&str, &std::collections::BTreeMap<String, String>) -> Result<String, String>;
pub static CARD_RENDER: std::sync::OnceLock<CardRender> = std::sync::OnceLock::new();

pub const VERSION: &str = "iron-v12";
/// Middle and late (iron-v7: the kind's tier in every design, late's 8-hour storage and its
/// storage-capped net).
pub const VERSION_VOID: &str = "iron-v8";

/// The evaluator's version for a stage.
pub fn version_of(stage: usize) -> &'static str {
    if stage == 0 { VERSION } else { VERSION_VOID }
}

impl farmopt::Farm for IronFarm {
    type Cand = Cand;

    fn id(&self) -> &'static str {
        "iron"
    }

    fn version(&self) -> &'static str {
        version_of(self.ctx.stage)
    }

    fn params(&self) -> Vec<farmopt::Param> {
        cand::params()
    }

    fn enumerate(&self, spec: &Spec) -> anyhow::Result<Vec<Cand>> {
        let mut v = cand::enumerate(spec, self.ctx.calm_parts.len());
        if self.seeds {
            v.extend(cand::seeds(&spec.stage));
        }
        Ok(v)
    }

    fn key(&self, c: &Cand) -> String {
        cand::key(c)
    }

    fn param(&self, c: &Cand, name: &str) -> Option<String> {
        cand::param(c, name)
    }

    fn neighbors(&self, c: &Cand, _spec: &Spec) -> Vec<Cand> {
        cand::neighbors(c)
    }

    fn design(&self, c: &Cand, _spec: &Spec) -> anyhow::Result<String> {
        Ok(design::build(c, &self.ctx.calm_parts)?.text)
    }

    fn eval_static(&self, c: &Cand, _spec: &Spec) -> Result<Evaluation, String> {
        eval::eval_static(c, &self.ctx)
    }

    fn bound(&self, _c: &Cand, _spec: &Spec, st: &Metrics) -> Metrics {
        eval::bound(st)
    }

    fn eval_quick(&self, c: &Cand, _spec: &Spec, st: &Metrics) -> Result<Evaluation, String> {
        eval::eval_quick(c, &self.ctx, st)
    }

    fn eval_full(&self, c: &Cand, _spec: &Spec, st: &Metrics) -> Result<Evaluation, String> {
        eval::eval_full(c, &self.ctx, st)
    }

    fn tie_metric(&self) -> Option<&'static str> {
        Some("blocks")
    }

    /// The farm is the build without its count of modules (copies of one unit, as tiled).
    fn group(&self, c: &Cand) -> Option<(String, u32)> {
        if c.seed.is_some() || c.modules <= 1 && c.l3_pits > 0 {
            return None;
        }
        let k = cand::key(c);
        let n = c.modules as u32;
        Some((k.replace(&format!("-n{n}"), "-n*"), n))
    }

    /// Iron per hour per build minute (the brief's test for a farm's own N).
    fn curve(&self, m: &farmopt::Metrics) -> Option<f64> {
        let r = m.get("iron_per_hour")?.v;
        let b = m.get("build_minutes")?.v;
        (b > 0.0).then_some(r / b)
    }

    /// The copies bar (ironspec iteration 4): N copies of the best single module on the site,
    /// M2-open-ring (designs/farms/iron/middle/m2-open-ring.txt, its judged rate from verdicts.tsv),
    /// priced at the stage's prices: per module, what a middle or late member must beat to be more
    /// than copies.
    /// The copies bar at the candidate's N (ironspec iteration 12): the single module's judged
    /// rate per module, N copies' build minutes, volume and blocks per module (the villagers' route
    /// at that N). The single module's full judge is cached on disk.
    fn bar_of(&self, spec: &Spec, c: &Cand) -> Option<farmopt::Metrics> {
        // `bar: none`: a measuring run (every candidate judged, none pruned by the copies bar)
        if spec.stage == "day-1" || c.seed.is_some() || spec.extra.get("bar").is_some_and(|b| b.trim() == "none") {
            return None;
        }
        let n = c.modules.max(1);
        if let Some(v) = self.bar_cache.lock().ok()?.get(&n) {
            return v.clone();
        }
        let v = (|| {
            let (single, _) = self.bar_single(spec)?;
            let cn = Cand { modules: n, tiling: cand::Tiling::Row(27), ..Self::bar_cand(spec) };
            let st = eval::eval_static(&cn, &self.ctx).ok()?;
            let mut m = st.metrics.clone();
            let rate = single.get("iron_per_hour").copied()?;
            m.insert("iron_per_hour_per_module".into(), rate);
            // the volume of N copies in their most compact no-coupling arrangement (ironspec
            // iteration 12: copies keep their gaps): a row at pitch 27 or a stack at pitch 43,
            // whichever box is smaller
            if n >= 2 {
                let cs = Cand { modules: n, tiling: cand::Tiling::Stack(43), ..Self::bar_cand(spec) };
                if let Ok(ss) = eval::eval_static(&cs, &self.ctx)
                    && let (Some(a), Some(b)) = (m.get("volume_per_module").copied(), ss.metrics.get("volume_per_module").copied())
                    && b.v < a.v
                {
                    m.insert("volume_per_module".into(), b);
                }
            }
            Some(m)
        })();
        if let Ok(mut cache) = self.bar_cache.lock() {
            cache.insert(n, v.clone());
        }
        v
    }

    fn bars(&self, spec: &Spec) -> Vec<String> {
        if spec.stage == "day-1" {
            return Vec::new();
        }
        if spec.extra.get("bar").is_some_and(|b| b.trim() == "none") {
            return vec!["No copies bar (`bar: none`: a measuring run, nothing pruned by it)".into()];
        }
        // the bar is copies of the best single module of the space (ironspec: the spec's `bar:`
        // names it; default the grammar's own M2-open-ring module), built exactly like every
        // candidate (cobblestone walls, torch caps, the lightning rod, the calm placed jointly with
        // its cage, the stage's prices) and judged in full
        let c = Self::bar_cand(spec);
        let key = cand::key(&c);
        let Some((full, st)) = self.bar_single(spec) else {
            return vec![format!("Copies bar: the single module {key} is refused or fails the judge")];
        };
        let g = |m: &farmopt::Metrics, k: &str| m.get(k).map(|v| v.v).unwrap_or(f64::NAN);
        // the bar at each N (ironspec iteration 12, check A): N copies priced together, the
        // villagers by the cheapest route at THAT N (breeding amortizes with N), uncoupled (pitch
        // 27); its rate per module is the single module's
        let ns: Vec<u8> = if spec.stage == "late" { vec![1, 2, 4, 8, 16] } else { vec![2, 4, 8, 16] };
        let mut at_n = Vec::new();
        for n in ns {
            let probe = Cand { modules: n, ..c.clone() };
            if let Some(m) = self.bar_of(spec, &probe) {
                at_n.push(format!("N = {n}: {:.1} build min, {:.0} blocks, volume {:.0}", g(&m, "build_minutes_per_module"), g(&m, "blocks_per_module"), g(&m, "volume_per_module")));
            }
        }
        let line = format!(
            "Copies bar (per module: N copies of the single module {key} (the spec's `bar:`, the best single module of the space; default the grammar's M2-open-ring module), no coupling, built and priced like every candidate, judged in full): {:.0} iron/h per module, {:.1} build minutes per module at {} prices, {:.0} blocks, volume {:.0} per module. Priced at each N (the villagers' route at that N; volume: the most compact no-coupling box of N copies, a row at 27 or a stack at 43), per module: {}. A member that does not beat it per module at its own N is just copies; a candidate that cannot beat it by the merge threshold is pruned before its runs.",
            g(&full, "iron_per_hour"),
            g(&st, "build_minutes"),
            spec.stage,
            g(&st, "blocks"),
            g(&st, "volume"),
            at_n.join("; ")
        );
        vec![line]
    }

    /// The site's own card renderer (site::cards::render: its number formats, plurals and
    /// phrases, handed in by the CLI as `CARD_RENDER` so this crate needs no site dependency), fed
    /// the candidate's metrics (numbers as the judge's card writes them) and lines.
    fn render_card(&self, row: &str, m: &farmopt::Metrics, lines: &std::collections::BTreeMap<String, String>) -> Option<String> {
        let render = CARD_RENDER.get()?;
        let mut map: std::collections::BTreeMap<String, String> = lines.iter().filter(|(k, _)| k.as_str() != "design_text").map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, v) in m {
            let x = v.v;
            let text = if (x - x.round()).abs() < 1e-9 { format!("{:.0}", x) } else if x.abs() >= 100.0 { format!("{:.0}", x) } else { format!("{:.1}", x) };
            map.insert(k.clone(), text);
        }
        Some(render(row, &map).unwrap_or_else(|e| format!("({e})")))
    }

    fn cards(&self, spec: &Spec) -> Option<(std::path::PathBuf, String)> {
        let word = match spec.stage.as_str() {
            "late" => "fastest",
            s => s,
        };
        Some((std::path::PathBuf::from("designs/circuits/mobfarm/cards.tsv"), format!("mob-farm iron_ingot {word}")))
    }
}
