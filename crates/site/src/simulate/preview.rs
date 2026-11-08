//! A work-in-progress design's verdict for its preview page (`mctas site preview`,
//! src/preview.rs): the kind's own judge on redsim, exactly what `mctas circuit sim` runs (a
//! study's whole-judge twin, every build, where the study has one: the mob farms, the crop farms,
//! the minecart stations; else the kind's stimulus on the base build), as the row `mctas circuit
//! archive` would write. Its own cache domain (build.rs hashes this file with the judge's crates),
//! so adding or changing it never recomputes the site build's circuit runs.

use circuit::Circuit as Design;
use circuit::verify::{Options, sim};
use serde::{Deserialize, Serialize};

/// What the preview's judge said about a design.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Judged {
    /// The redsim judge ran it to a verdict (else `why` says why it could not).
    pub ran: bool,
    /// The verdict passes.
    pub pass: bool,
    /// The verdict came from the study's whole judge on redsim (every build: robustness included);
    /// else from the kind's stimulus on the base build only (no robustness verdict).
    pub every_build: bool,
    /// As in `verdicts.tsv`: the kind, the study's axis value, the metrics, the box, the family of
    /// changing blocks, the robustness verdict and the judge's one-line summary.
    pub kind: String,
    pub axis: String,
    pub metrics: String,
    pub dims: String,
    pub volume: i64,
    pub family: String,
    pub robustness: String,
    pub summary: String,
    /// Why the judge could not run it (a kind judged in the game only, a design redsim does not
    /// model), or why it fails.
    pub why: String,
}

/// The kinds whose redsim run is a study's whole judge, every build (crate::verify::sim's twins).
fn every_build(kind: &str) -> bool {
    let word = kind.split_whitespace().next().unwrap_or("");
    matches!(word, "harvester" | "station" | "cart-loader" | "cart-unloader") || circuit::registry::find(word).is_some_and(|(_, k)| matches!(k.twin, circuit::registry::Twin::Exact(_)))
}

/// The preview's verdict on a design's text (a pure function of it: cached by src/cache.rs).
pub fn preview_judge(text: &str) -> Judged {
    let c = match mctas::design::Design::parse(text).and_then(Design::from_design) {
        Ok(c) => c,
        Err(e) => return Judged { why: format!("the design does not load as a circuit: {e:#}"), ..Default::default() },
    };
    let Some(kind) = c.kind.clone() else { return Judged { why: "no `circuit: <kind>` line".into(), ..Default::default() } };
    let o = Options::default();
    let r = match sim(&c, &o) {
        Ok(r) => r,
        Err(e) => return Judged { kind: kind.name(), why: format!("{e:#}"), ..Default::default() },
    };
    let summary = r.summary(&c);
    let (trace, out) = match (&r.trace, &r.outcome) {
        (Ok(t), Some(out)) => (t, out),
        (Err(why), _) => return Judged { kind: kind.name(), summary, why: why.clone(), ..Default::default() },
        (Ok(_), None) => return Judged { kind: kind.name(), summary, why: "the judge ran it but has no verdict for it".into(), ..Default::default() },
    };
    let every = every_build(&kind.name());
    if !out.pass {
        return Judged { ran: true, every_build: every, kind: kind.name(), summary, why: out.why.clone(), ..Default::default() };
    }
    let d = circuit::verify::dims(&c, trace);
    let family = circuit::verify::family(&c, trace);
    // a twin's verdict leads its outcome's note (`robust`, `robust: <note>`, `position-dependent`)
    let robustness = if every { out.why.split(';').next().unwrap_or("").trim().to_string() } else { String::new() };
    Judged {
        ran: true,
        pass: true,
        every_build: every,
        kind: kind.name(),
        axis: circuit::archive::axis(&kind, out, &family),
        metrics: out.metrics_text(),
        dims: format!("{}x{}x{}", d[0], d[1], d[2]),
        volume: (d[0] * d[1] * d[2]) as i64,
        family,
        robustness,
        summary,
        why: String::new(),
    }
}
