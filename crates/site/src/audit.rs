//! `mctas site dominated`: the circuits a builder has no reason to pick (vjeux 12:12Z on a
//! slime-free piston extender: "Why would anyone use this monstrosity? It's bigger and slower than
//! the other ones we have"). A design X is listed when another design Y of the same kind and main
//! value is smaller (blocks) and at least as fast on every speed metric of the study (strictly
//! faster on one), and X's only advantage over Y is a material restriction (it avoids a block kind
//! Y uses: slime, honey, torches, pistons...). A design with any other property Y lacks (one
//! button, flat, 1 wide, tileable, robust, a badge) is kept.

use site_model::{Build, Site};
use std::collections::BTreeSet;

/// The study's speed metrics: (metric, true when higher is better).
fn speed_metrics(kind: &str) -> &'static [(&'static str, bool)] {
    match kind.split_whitespace().next().unwrap_or("") {
        "piston-extender" => &[("extend", false), ("retract", false)],
        "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" => &[("delay", false), ("min-pulse", false)],
        "tff" => &[("latency", false), ("min-period", false), ("min-pulse", false)],
        "delay" => &[("min-pulse", false), ("min-gap", false)],
        "gate" | "rs-latch" | "d-latch" | "dff" | "dff-enable" | "tff-reset" | "shift" | "counter" => &[("latency", false)],
        "flying-machine" => &[("latency", false)],
        "shulker-loader" | "shulker-unloader" => &[("items/s", true), ("first box", false)],
        "sorter" => &[("items/s", true), ("first item", false)],
        "smelter" => &[("items/min per furnace", true), ("first item", false), ("minutes for a double chest", false)],
        "brewer" => &[("potions/min per stand", true), ("first potion", false)],
        // (a crop farm: its harvest per block, per plant and in all; without them a farm of more
        // blocks read as dominated by any smaller one: melon-x by the dry 7-block melon farm)
        "harvester" => &[("items per hour per block", true), ("items per hour per cane", true), ("items per hour", true)],
        // (an iron farm: its iron per block and in all; without them its tier's scaled build, 4
        // copies of the pick, read as dominated by the pick itself)
        "mob-farm" => &[("iron per hour per block", true), ("iron per hour", true)],
        "composter" => &[("items composted/min", true), ("first bone meal", false)],
        // (an automatic door: without them every bigger door read as dominated by the smallest,
        // lock-v9's 6 gt wait by the 44-block door's 0, and by lock-v12's 13)
        "auto-door" => &[("open wait", false), ("close delay", false)],
        // (a minecart stop: without them a bigger, faster unloader read as dominated by the
        // smallest, sorter's ul-gate at 4.79 items/s by cart-unloader-t3's 2.48)
        "cart-unloader" => &[("unload rate", true), ("departure delay", false)],
        "cart-loader" => &[("load rate", true), ("departure delay", false)],
        "lock" => &[("opens within", false), ("closes within", false)],
        "bud" => &[("latency", false), ("re-arm", false)],
        "tnt-duper" => &[("rate", true), ("latency", false)],
        "tnt-duper-triggered" => &[("latency", false), ("gap", false)],
        "burst" => &[("latency", false), ("repeat", false)],
        "divider" => &[("latency", false), ("min-period", false)],
        "monostable" => &[("delay", false), ("re-arm", false)],
        "off-delay" => &[("latency", false), ("min-pulse", false)],
        "on-delay" => &[("latency", false)],
        "pulse-train" => &[("latency", false), ("stop", false)],
        _ => &[("latency", false)],
    }
}

/// A metric's leading number (`27 gt` -> 27, `2.50` -> 2.5, `4-6 gt` -> 6: the worst).
fn number(b: &Build, k: &str) -> Option<f64> {
    let v = b.circuit.as_ref()?.metrics.iter().find(|(n, _)| n == k)?.1.clone();
    let first = v.split_whitespace().next()?;
    let worst = first.rsplit('-').next().unwrap_or(first);
    worst.parse().ok()
}

/// Marks `faster` the shown designs that are bigger than another of the same study, kind and value
/// but strictly faster than every smaller one on the study's speed metrics (the coordinator: keep
/// pext-n3-flat-cascade19, 27/27 gt, next to the 18-block record's 29/35, and say it is faster).
pub fn mark_faster(builds: &mut [Build]) -> usize {
    let shown = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty()));
    let key = |b: &Build| b.circuit.as_ref().map(|c| (c.study.clone(), c.kind.clone(), c.axis.clone())).unwrap_or_default();
    let mut marked = Vec::new();
    for (i, x) in builds.iter().enumerate() {
        if !shown(x) {
            continue;
        }
        let Some(c) = &x.circuit else { continue };
        let metrics = speed_metrics(&c.kind);
        let smaller: Vec<&Build> = builds.iter().filter(|y| shown(y) && key(y) == key(x) && y.blocks < x.blocks).collect();
        if smaller.is_empty() {
            continue;
        }
        // strictly better than each smaller design on the study's first speed metric (a shulker
        // machine's items a second, an extender's time out...), never worse on another (critique:
        // "faster than smaller versions" at the same 2.50 items a second, only its first box sooner)
        let beats = |y: &Build| {
            let mut primary_better = false;
            for (n, (m, higher)) in metrics.iter().enumerate() {
                match (number(x, m), number(y, m)) {
                    (Some(a), Some(b)) => {
                        let (a, b) = if *higher { (-a, -b) } else { (a, b) };
                        if a > b {
                            return false;
                        }
                        if n == 0 && a < b {
                            primary_better = true;
                        }
                    }
                    (None, Some(_)) => return false,
                    _ => {}
                }
            }
            primary_better
        };
        if smaller.iter().all(|y| beats(y)) {
            marked.push(i);
        }
    }
    for &i in &marked {
        builds[i].flags.push("faster".into());
    }
    marked.len()
}

/// Flags that only restrict materials (avoiding a material is not a builder's property here).
const MATERIAL_FLAGS: [&str; 4] = ["slime-free", "honey-free", "torch-free", "piston-free"];

/// The material classes a design can avoid (vjeux's "slime-free, honey-free, torch-free,
/// piston-free..."): a design avoids a class when it uses none of its blocks.
const MATERIALS: [(&str, &[&str]); 6] = [
    ("slime", &["slime_block"]),
    ("honey", &["honey_block"]),
    ("torches", &["redstone_torch", "redstone_wall_torch"]),
    ("pistons", &["piston", "sticky_piston"]),
    ("copper bulbs", &["copper_bulb", "exposed_copper_bulb", "weathered_copper_bulb", "oxidized_copper_bulb", "waxed_copper_bulb", "waxed_exposed_copper_bulb", "waxed_weathered_copper_bulb", "waxed_oxidized_copper_bulb"]),
    // (a sculk sensor: only silk touch takes one, from the deep dark; a design without it is no
    // duplicate of a smaller one with it: pulse-limiter-u-L10 beside the sculk filter's 5 blocks)
    ("sculk sensors", &["sculk_sensor", "calibrated_sculk_sensor"]),
];

/// What a builder gets from X beyond its materials: its chips, every badge, its flags (an
/// extendable family, one button, 1 wide...) and its categorical metrics (a counter's output, a
/// flip-flop's edge, a burst's re-trigger class), the bookkeeping and the material ones aside.
fn properties(b: &Build) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = site_model::html::props(b).chips.into_iter().filter(|c| !c.starts_with("tunable") && !c.starts_with('+') && !matches!(c.as_str(), "variant" | "a bigger version" | "another layout" | "layout")).collect();
    s.extend(b.flags.iter().filter(|f| !matches!(f.as_str(), "variant" | "layout" | "tunable")).cloned());
    if let Some(c) = &b.circuit {
        // the base design of an extendable family grows by one unit per n: a builder's property
        if c.extends {
            s.insert("extendable".into());
        }
        // what its card leads with is a property too (its chips leave out what the lead says)
        for part in c.lead.split([',', '+', '·']).map(str::trim).filter(|p| !p.is_empty()) {
            s.insert(part.to_string());
        }
        // what a machine carries or pulls (a flying machine's cargo) is its own property
        if !c.feature.is_empty() {
            s.insert(format!("feature={}", c.feature));
        }
        for (k, v) in &c.metrics {
            match k.as_str() {
                "badges" => s.extend(v.split('+').filter(|x| !x.is_empty()).map(String::from)),
                "output" | "edge" | "enable" | "class" | "retrigger" | "function" | "two-way" => {
                    s.insert(format!("{k}={}", v.split_whitespace().next().unwrap_or("")));
                }
                // (an automatic door that stays open for someone standing in the doorway: no door
                // that closes on them dominates it, the coordinator 2026-10-22: lock-v12 the
                // obvious door's variant; its value's first word is "the" either way)
                "stop in doorway" => {
                    s.insert(format!("{k}={}", v.trim()));
                }
                _ => {}
            }
        }
    }
    s.retain(|x| !MATERIAL_FLAGS.contains(&x.as_str()));
    s
}

/// The material classes Y uses and X avoids entirely.
fn avoided(x: &Build, y: &Build) -> Vec<String> {
    let uses = |b: &Build, blocks: &[&str]| b.kinds.iter().any(|k| blocks.contains(&k.as_str()));
    MATERIALS.iter().filter(|(_, blocks)| uses(y, blocks) && !uses(x, blocks)).map(|(name, _)| name.to_string()).collect()
}

/// One dominated design.
pub struct Dominated<'a> {
    pub x: &'a Build,
    pub y: &'a Build,
    /// (metric, X's value, Y's value) where Y is better.
    pub slower: Vec<(String, f64, f64)>,
    /// Block kinds Y uses that X avoids.
    pub avoids: Vec<String>,
}

/// What [`dominated`] looks for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// Bigger and strictly slower, only a material restriction in its favour (the rule).
    SlowerMaterial,
    /// Bigger, the same speed, only a material restriction in its favour.
    SameSpeedMaterial,
    /// Bigger and no faster, with nothing in its favour, not even a material (vjeux's own words:
    /// "bigger and slower than the other ones we have"; the coordinator 19:05Z: a slime-free
    /// extender beaten by a smaller slime-free one at the same reach).
    Plain,
}

/// Every circuit X beaten by a smaller, at-least-as-fast Y of the same kind and main value, with
/// at most its materials in its favour, as `pattern` says.
pub fn dominated(site: &Site, pattern: Pattern) -> Vec<Dominated<'_>> {
    let mut out = Vec::new();
    let circuits: Vec<&Build> = site.builds.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.layout_of.is_none())).collect();
    for x in &circuits {
        let cx = x.circuit.as_ref().unwrap();
        // (a design already folded under another is off the lists)
        if cx.host.is_some() {
            continue;
        }
        let mut best: Option<Dominated> = None;
        for y in &circuits {
            let cy = y.circuit.as_ref().unwrap();
            if y.id == x.id || cy.study != cx.study || cy.kind != cx.kind || cy.axis != cx.axis || cy.direction != cx.direction || y.blocks >= x.blocks {
                continue;
            }
            // smaller by its kind's own rule, and as robust (the ranking's: a piston extender is
            // ranked by its box, pext-n2-rider-flat's 6×2 under rider7-flat's 5×3 with a block
            // fewer, the pick of its reach; a robust design first)
            if crate::kinds::size_key(y) > crate::kinds::size_key(x) || (crate::kinds::is_robust(x) && !crate::kinds::is_robust(y)) {
                continue;
            }
            // only within its own group and with the same kind of input (a sand elevator never
            // beats a general extender; a lever basic never beats a gate taking a real signal: the
            // V18 rule)
            let device = |b: &Build| ["lever", "button"].into_iter().find(|f| b.flags.iter().any(|x| x == f));
            if y.group != x.group || device(x) != device(y) {
                continue;
            }
            // at least as fast everywhere
            let mut slower = Vec::new();
            let mut better_somewhere = false;
            for (k, higher) in speed_metrics(&cx.kind) {
                let (Some(a), Some(b)) = (number(x, k), number(y, k)) else { continue };
                if if *higher { b > a } else { b < a } {
                    slower.push((k.to_string(), a, b));
                }
                if if *higher { a > b } else { a < b } {
                    better_somewhere = true;
                }
            }
            if better_somewhere {
                continue;
            }
            // nothing but (maybe) its materials in X's favour
            if !properties(x).is_subset(&properties(y)) {
                continue;
            }
            let avoids = avoided(x, y);
            let fits = match pattern {
                Pattern::SlowerMaterial => !slower.is_empty() && !avoids.is_empty(),
                Pattern::SameSpeedMaterial => slower.is_empty() && !avoids.is_empty(),
                Pattern::Plain => avoids.is_empty(),
            };
            if !fits {
                continue;
            }
            let cand = Dominated { x, y, slower, avoids };
            if best.as_ref().is_none_or(|b| (cand.y.blocks, &cand.y.name) < (b.y.blocks, &b.y.name)) {
                best = Some(cand);
            }
        }
        if let Some(d) = best {
            out.push(d);
        }
    }
    out.sort_by(|a, b| (&a.x.circuit.as_ref().unwrap().study, &a.x.name).cmp(&(&b.x.circuit.as_ref().unwrap().study, &b.x.name)));
    out
}

/// `mctas site dominated --hide` (the coordinator 2026-10-22: "the duplicate pattern vjeux objected
/// to"): one crates/site/hidden.tsv line per design of the "nothing in its favour" list
/// (`<study>/<name> TAB dominated: <winner> is N blocks smaller, same speed`), never a design the
/// site keeps (`kept`: a pick, a size or a variant of its kind, by build id) nor one `present`
/// already holds (a line of the file as it is now); and what it left out, with why.
pub fn hide_lines(site: &Site, kept: &BTreeSet<String>, present: &BTreeSet<String>) -> (Vec<String>, Vec<String>) {
    let (mut lines, mut left) = (Vec::new(), Vec::new());
    for d in dominated(site, Pattern::Plain) {
        let study = d.x.circuit.as_ref().map(|c| c.study.as_str()).unwrap_or("");
        let pattern = format!("{study}/{}", d.x.name);
        if kept.contains(&d.x.id) {
            left.push(format!("{pattern}: the site keeps it (a pick, a size or a variant of its kind)"));
            continue;
        }
        if present.contains(&pattern) {
            left.push(format!("{pattern}: already in crates/site/hidden.tsv"));
            continue;
        }
        let n = d.x.blocks - d.y.blocks;
        let speed = if d.slower.is_empty() {
            "same speed".to_string()
        } else {
            let gt = |v: f64| if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v}") };
            format!("and faster ({})", d.slower.iter().map(|(k, a, b)| format!("{k} {} vs {}", gt(*b), gt(*a))).collect::<Vec<_>>().join(", "))
        };
        lines.push(format!("{pattern}\tdominated: {} is {n} block{} smaller, {speed}", d.y.name, if n == 1 { "" } else { "s" }));
    }
    (lines, left)
}

/// The build's notes: every dominated design still published, for whoever builds the site to
/// check and add to crates/site/hidden.tsv (the coordinator: re-run the audit whenever a study
/// re-archives; the build does it every time).
pub fn notes(site: &Site) -> Vec<String> {
    let mut out = Vec::new();
    for (pattern, what) in [(Pattern::SlowerMaterial, "bigger AND slower"), (Pattern::SameSpeedMaterial, "bigger, the same speed"), (Pattern::Plain, "bigger and no faster")] {
        for d in dominated(site, pattern) {
            out.push(format!(
                "dominated ({what}, {}): {}/{} ({} blocks) is beaten by {} ({} blocks){}: check it, then hide it in crates/site/hidden.tsv (mctas site dominated)",
                if d.avoids.is_empty() { "nothing in its favour".to_string() } else { format!("only avoids {}", d.avoids.join(", ")) },
                d.x.circuit.as_ref().map(|c| c.study.as_str()).unwrap_or(""),
                d.x.name,
                d.x.blocks,
                d.y.name,
                d.y.blocks,
                if d.slower.is_empty() { String::new() } else { format!(", {}", d.slower.iter().map(|(k, a, b)| format!("{k} {a} vs {b}")).collect::<Vec<_>>().join(", ")) }
            ));
        }
    }
    out
}

/// `mctas site dominated`: the lists, one line each.
pub fn report(site: &Site) -> String {
    let mut s = String::new();
    let line = |d: &Dominated| -> String {
        let slower = if d.slower.is_empty() {
            "same speed".to_string()
        } else {
            d.slower.iter().map(|(k, a, b)| format!("{k} {a} vs {b}")).collect::<Vec<_>>().join(", ")
        };
        format!(
            "{}\t{} ({} blocks)\tbeaten by {} ({} blocks)\t+{} blocks, {}\t{}\n",
            d.x.circuit.as_ref().map(|c| c.study.as_str()).unwrap_or(""),
            d.x.name,
            d.x.blocks,
            d.y.name,
            d.y.blocks,
            d.x.blocks - d.y.blocks,
            slower,
            if d.avoids.is_empty() { "nothing in its favour".to_string() } else { format!("only avoids {}", d.avoids.join(", ")) }
        )
    };
    for (pattern, title) in [
        (Pattern::SlowerMaterial, "bigger AND slower, only a material restriction in their favour"),
        (Pattern::SameSpeedMaterial, "bigger, the same speed (or no speed metric), only a material restriction in their favour"),
        (Pattern::Plain, "bigger and no faster, nothing in their favour (not even a material)"),
    ] {
        let list = dominated(site, pattern);
        s.push_str(&format!("# {title}: {}\n", list.len()));
        for d in &list {
            s.push_str(&line(d));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circuit(id: &str, study: &str, kind: &str, axis: &str, blocks: u32, volume: u32, metrics: &str) -> Build {
        Build {
            id: format!("circuits/{study}/{id}"),
            name: id.into(),
            blocks,
            volume: Some(volume),
            circuit: Some(site_model::Circuit { study: study.into(), kind: kind.into(), axis: axis.into(), robustness: "robust".into(), metrics: crate::circuits::parse_metrics(metrics), ..Default::default() }),
            ..Default::default()
        }
    }

    /// `mctas site dominated --hide` (the coordinator 2026-10-22): an older design next to its
    /// smaller compaction gets a line; a pick the site keeps, a line the file has, and a design
    /// smaller by its kind's own rule (a piston extender's box) get none.
    #[test]
    fn hide_lines_skip_picks_and_smaller_boxes() {
        let old = circuit("pulse-limiter-u-L12", "pulse", "pulse-limiter", "L = 12 gt", 10, 12, "L 12 gt, delay 2 gt");
        let new = circuit("pulse-limiter-u-L12-v12-9b", "pulse", "pulse-limiter", "L = 12 gt", 9, 12, "L 12 gt, delay 2 gt");
        // (pext-n2-rider-flat, 8 blocks in 6×2, is the pick of its reach over rider7-flat, 7 in 5×3)
        let flat = circuit("pext-n2-rider-flat", "pext", "piston-extender 2", "N = 2", 8, 12, "N 2, extend 9 gt, retract 9 gt");
        let tall = circuit("pext-n2-rider7-flat", "pext", "piston-extender 2", "N = 2", 7, 15, "N 2, extend 9 gt, retract 9 gt");
        let site = Site { builds: vec![old.clone(), new, flat, tall], ..Default::default() };
        let none = BTreeSet::new();
        let (lines, left) = hide_lines(&site, &none, &none);
        assert_eq!(lines, vec!["pulse/pulse-limiter-u-L12\tdominated: pulse-limiter-u-L12-v12-9b is 1 block smaller, same speed".to_string()]);
        assert!(left.is_empty());
        let kept: BTreeSet<String> = [old.id.clone()].into();
        assert!(hide_lines(&site, &kept, &none).0.is_empty());
        let present: BTreeSet<String> = ["pulse/pulse-limiter-u-L12".to_string()].into();
        let (lines, left) = hide_lines(&site, &none, &present);
        assert!(lines.is_empty() && left[0].ends_with("already in crates/site/hidden.tsv"), "{left:?}");
    }
}
