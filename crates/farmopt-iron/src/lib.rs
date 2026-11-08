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

use cand::Cand;
use farmopt::spec::Spec;
use farmopt::{Evaluation, Metrics};

/// The iron farm as a [`farmopt::Farm`].
pub struct IronFarm {
    pub ctx: eval::Ctx,
    /// Also enumerate the stage's exact seeds (calibration points: they never enter a front).
    pub seeds: bool,
}

impl IronFarm {
    pub fn new(stage: &str) -> IronFarm {
        IronFarm { ctx: eval::Ctx { calm_parts: calm::parts(stage), stage: price::stage_ix(stage) }, seeds: false }
    }
}

/// The evaluator's version: bump it whenever the grammar or the evaluation changes (the journal
/// keeps no entry of another version).
pub const VERSION: &str = "iron-v1";

impl farmopt::Farm for IronFarm {
    type Cand = Cand;

    fn id(&self) -> &'static str {
        "iron"
    }

    fn version(&self) -> &'static str {
        VERSION
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

    fn cards(&self, spec: &Spec) -> Option<(std::path::PathBuf, String)> {
        let word = match spec.stage.as_str() {
            "late" => "fastest",
            s => s,
        };
        Some((std::path::PathBuf::from("designs/circuits/mobfarm/cards.tsv"), format!("mob-farm iron_ingot {word}")))
    }
}
