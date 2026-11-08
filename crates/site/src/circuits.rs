//! The circuits (the contraption studies, docs/circuits.md): every design the judge archived in
//! `designs/circuits/<study>/archive/` (its `verdicts.tsv` row: kind, axis, metrics, robustness,
//! the judge line; `captions.tsv`), with the catalog's folds (`catalog.tsv`: equivalent layouts,
//! tunable groups; the dominance fold within one kind and axis value), the judge's own test run
//! here for each page (the viewer's demo and the timing diagram), and the extendable families
//! expanded to every member. The judge's calibration and selftest designs are not published.

use anyhow::Result;
use circuit::Circuit as Design;
use circuit::archive::{archive_dir, read_rows, studies};
use crate::cache::{self, Domain};
use crate::simulate::{self, Details};
use site_model::{Build, Circuit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const ROOT: &str = "designs/circuits";

fn captions(folder: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(folder.join("captions.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(f, _)| *f != "file")
        .map(|(f, c)| (f.to_string(), c.trim().to_string()))
        .collect()
}

/// The design's own description: its comment lines, minus the archive's bookkeeping ones.
fn comment_caption(text: &str) -> String {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix('#').map(str::trim))
        .filter(|l| !(l.starts_with("archived") || l.starts_with("verified") || l.starts_with("CIRCUIT:") || l.starts_with("FAIL:") || l.is_empty()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Metric names of more than one word.
const MULTI_WORD_METRICS: [&str; 66] = [
    // (a mob farm's, the ironfarm track: split at their first space they read `iron` = `per hour
    // per block 1.57`; the longer names first, `iron per hour` is a prefix of two others; the
    // mobinfra track's `real game` and `thunder loss`)
    "iron per hour per block", "iron per hour per module", "iron per hour", "iron range", "golems per hour", "seconds per golem", "share of ceiling", "worst policy", "restarts after a missed night", "restarts after days away", "lava sources", "chunk columns", "hoppers per 1000 iron/h", "real game", "thunder loss", "build time", "time to an iron kit",
    "first box", "first item", "per pulse", "per period", "hoppers awake", "safe drop", "effective volume", "items smelted per hour per block", "items smelted per hour", "items per hour per cane", "per cane", "harvest height", "game rts", "at rest", "stop distance", "departure delay", "leaves at", "other way passes at", "first craft", "recipe class", "trip up", "calls and sends", "unload rate", "load rate", "per bit", "items per hour per block", "items per hour", "open wait", "close delay", "stop in doorway", "presses tried", "opens within", "closes within",
    // (an auto-smelter's, study 17, and sorter's `chest` run: split at their first space they read
    // `items/min` = `per furnace 6.0` and the card lost its rate; the brewer's and the composter's
    // the same way)
    "rows items/min per furnace", "items/min per furnace", "items per coal", "blocks per furnace", "end pieces", "minutes for a double chest", "of the ceiling", "first ingot", "coal left", "coal needed", "potions/min per stand", "first potion", "blocks per stand", "blaze powder", "items composted/min", "first bone meal",
];

/// The flying machines in a design: clusters of touching slime blocks, honey blocks, pistons,
/// sticky pistons and observers (6-neighbours) that hold a slime or honey block (a flying
/// machine's engine and what it carries; a sweeper farm's one machine and its wings: clk-rt's
/// cane-sweeper-10x24). 0 without slime or honey.
pub fn flying_machines(text: &str) -> u32 {
    let Ok(d) = mcview::Design::parse(text) else { return 0 };
    let id = |s: &str| s.split('[').next().unwrap_or(s).trim_start_matches("minecraft:").to_string();
    let sticky = |s: &str| matches!(id(s).as_str(), "slime_block" | "honey_block");
    let part = |s: &str| sticky(s) || matches!(id(s).as_str(), "piston" | "sticky_piston" | "observer");
    let cells: std::collections::BTreeSet<[i32; 3]> = d.blocks.iter().filter(|(_, s)| part(s)).map(|(p, _)| *p).collect();
    let mut seen: std::collections::BTreeSet<[i32; 3]> = std::collections::BTreeSet::new();
    let mut machines = 0;
    for start in &cells {
        if seen.contains(start) {
            continue;
        }
        let mut stack = vec![*start];
        seen.insert(*start);
        let mut has_sticky = false;
        while let Some(p) = stack.pop() {
            has_sticky |= d.blocks.get(&p).is_some_and(|s| sticky(s));
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                let q = [p[0] + dx, p[1] + dy, p[2] + dz];
                if cells.contains(&q) && seen.insert(q) {
                    stack.push(q);
                }
            }
        }
        if has_sticky {
            machines += 1;
        }
    }
    machines
}

/// `L 2 gt, delay 1 gt, min-pulse 1 gt` -> [(L, 2 gt), (delay, 1 gt), (min-pulse, 1 gt)]. A part
/// that starts with a number continues the previous value (`bill 16 iron, 3 quartz`, `period 10
/// gt, 1 cell(s)`); `first box 13847 gt`, `per period 1`... are one name.
pub fn parse_metrics(s: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for p in s.split(", ").map(str::trim).filter(|p| !p.is_empty()) {
        // (an adder slice's metric holds two parts, `slice 54 blocks per bit, latency 18 + 12 (N-1)
        // gt`: its law is no second latency, compute::slice_check)
        let slice_law = p.starts_with("latency ") && out.last().is_some_and(|l| l.0 == "slice");
        if p.starts_with(|c: char| c.is_ascii_digit()) || slice_law {
            if let Some(last) = out.last_mut() {
                last.1.push_str(", ");
                last.1.push_str(p);
                continue;
            }
        }
        if let Some(m) = MULTI_WORD_METRICS.iter().find(|m| p.strip_prefix(**m).is_some_and(|r| r.starts_with(' '))) {
            out.push((m.to_string(), p[m.len() + 1..].to_string()));
            continue;
        }
        out.push(match p.split_once(' ') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (p.to_string(), String::new()),
        });
    }
    out
}

/// The metrics of a `CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, <verdict>` line:
/// the comma-separated `name value` parts between the kind and the block count.
pub fn parse_verdict_metrics(line: &str, kind: &str) -> Vec<(String, String)> {
    let Some(rest) = line.trim().strip_prefix("CIRCUIT:") else { return Vec::new() };
    let Some(i) = rest.find(&format!(" {kind} ")) else { return Vec::new() };
    let body = &rest[i + kind.len() + 2..];
    let mut out = Vec::new();
    for part in body.split(", ") {
        let part = part.trim();
        if part.ends_with(" blocks") || part.chars().next().is_some_and(|c| c.is_ascii_digit()) && part.contains('x') && !part.contains(' ') {
            break;
        }
        match part.split_once(' ') {
            Some((k, v)) => out.push((k.to_string(), v.to_string())),
            None => out.push((part.to_string(), String::new())),
        }
    }
    out
}

/// The first number in a text (`L = 4 gt` -> 4).
fn number(v: &str) -> Option<i64> {
    let s: String = v.trim().chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    s.parse().ok()
}

/// The block a crop farm's crop stands on (the block under its first `port crop` cell), in words
/// (`dirt`, `mud`, a kelp farm's `hopper`), and whether the crop cell itself holds water in the
/// design (kelp: planted in a water source); empty without a crop port.
fn crop_ground(text: &str) -> (String, bool) {
    let Some(d) = text.lines().any(|l| l.trim_start().starts_with("port crop")).then(|| mctas::design::Design::parse(text).ok()).flatten() else { return (String::new(), false) };
    let Some(p) = d.ports.iter().find(|(k, _)| k.starts_with("crop")).map(|(_, p)| *p) else { return (String::new(), false) };
    let soil = d.blocks.get(&[p[0], p[1] - 1, p[2]]).map(|s| s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").replace('_', " ")).unwrap_or_default();
    let wet = d.blocks.get(&p).is_some_and(|s| s.starts_with("minecraft:water") || s.contains("waterlogged=true"));
    (soil, wet)
}

/// What a design file adds to its row: block kinds, input and output names, its box, a machine's
/// travel direction in words.
struct Parsed {
    kinds: Vec<String>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    dims: Option<(String, u32)>,
    direction: String,
}

fn parse(path: &Path) -> Option<(Design, Parsed)> {
    let c = Design::load(path).ok()?;
    let (lo, hi) = c.bounds();
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    // only flying machines travel on their own; an extender's `travel` is its push direction
    let direction = match (&c.kind, c.travel) {
        (Some(circuit::Kind::FlyingMachine), Some(t)) => match t[1] {
            1 => "up",
            -1 => "down",
            _ => "horizontal",
        }
        .to_string(),
        _ => String::new(),
    };
    let p = Parsed {
        kinds: c.kinds().into_iter().collect(),
        inputs: c.inputs.iter().map(|i| i.name.clone()).collect(),
        outputs: c.outputs.iter().map(|o| o.name.clone()).collect(),
        dims: Some((format!("{}x{}x{}", d[0], d[1], d[2]), (d[0] * d[1] * d[2]).max(0) as u32)),
        direction,
    };
    Some((c, p))
}

/// [`flags_of`] for unit tests elsewhere in the crate: the flags a design with these metrics gets.
#[cfg(test)]
pub fn test_flags(metrics: &[(&str, &str)]) -> Vec<String> {
    let m: Vec<(String, String)> = metrics.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    flags_of(&m, "", "")
}

fn flags_of(metrics: &[(String, String)], robustness: &str, verdict: &str) -> Vec<String> {
    let mut f = Vec::new();
    if robustness == "robust" || verdict.trim_end().ends_with(", robust") {
        f.push("robust".to_string());
    }
    if metrics.iter().any(|(k, v)| k == "latency" && number(v) == Some(0)) || verdict.contains("instant") {
        f.push("instant".to_string());
    }
    // a piston extender safe to flip back at any time (clk-misc's `flips safe`: the lever turned
    // off before the block is out strands nothing), a builder's property
    if metrics.iter().any(|(k, v)| k == "flips" && v.starts_with("safe")) {
        f.push("flip-safe".to_string());
    }
    f
}

/// The judge's test as the viewer and the page show it ([`simulate::circuit_run`], cached by the
/// design's text: src/cache.rs), its phases put in words here (the labels are text a visitor
/// reads: they stay out of the cache).
fn details(c: &Design, file: &str, text: &str, min_pulse: Option<u32>) -> Details {
    let mut d: Details = cache::get_or(Domain::Circuit, &[file.as_bytes(), text.as_bytes()], || simulate::circuit_run(c));
    // study 11's phase labels in words (`clock 40` -> `clock, a pulse every 40 gt`): the view's
    // demo badge and the timing diagram
    let phases: Vec<(u32, u32, String)> = d.raw_phases.iter().map(|(a, b, l)| (*a, *b, crate::explain::phase_label_min(&d.kind_name, l, min_pulse))).collect();
    d.phases = phases.iter().map(|(a, b, l)| format!("{a}-{b} {}", l.replace(';', ","))).collect::<Vec<_>>().join("; ");
    if let Some(w) = d.wave.as_mut() {
        w.phases = phases;
    }
    d
}

/// Each input's button as the 3D view labels it, in `inputs` order (critique 058332315: a page
/// named "D0 … K2" where the view's buttons say "Dial 1 … Code 3"): webrender's own
/// `mcview::circuit::input_label` (a lock frame's tag "DIAL 1" is its button "Dial 1", as the
/// viewer's bar writes it; any other input's tag is its button). Empty when the viewer cannot
/// read the design.
fn viewer_input_labels(text: &str, inputs: &[String]) -> Vec<String> {
    let Ok(d) = mcview::design::Design::parse(text) else { return Vec::new() };
    let vi = mcview::circuit::inputs(&d);
    inputs
        .iter()
        .map(|n| {
            vi.iter()
                .find(|ci| &ci.name == n)
                .map(|ci| {
                    let tag = mcview::circuit::input_label(&d, ci);
                    match tag.split_once(' ') {
                        Some((w, k)) if ci.how == mcview::circuit::InputHow::Frame && !w.is_empty() => format!("{}{} {k}", &w[..1], w[1..].to_lowercase()),
                        _ => tag,
                    }
                })
                .unwrap_or_else(|| n.to_uppercase())
        })
        .collect()
}

/// Threads of the build's parallel passes (`mctas site build --threads`, default 32; 0: not set,
/// up to 32): the box is shared (several agents, MCTAS_MAX_WORKERS caps their game workers but
/// not these passes), and two passes run at once (the clocks beside the circuits, the stills
/// beside the lamp checks).
static THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Set the threads of every [`par_map`] (the build's `--threads`).
pub fn set_threads(n: usize) {
    THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
}

fn threads() -> usize {
    match THREADS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(32),
        n => n,
    }
}

/// `f` over `items` on [`threads`] threads, results in order.
pub(crate) fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = threads().clamp(1, 64).min(items.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<R>>> = items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let r = f(&items[i]);
                if let Ok(mut slot) = slots[i].lock() {
                    *slot = Some(r);
                }
            });
        }
    });
    slots.into_iter().map(|m| m.into_inner().ok().flatten().expect("every item mapped")).collect()
}

/// The designs the site does not publish (crates/site/hidden.tsv): (study, file pattern, why);
/// the `!` exceptions are not in this list ([`hidden_exceptions`]).
pub fn hidden_rules() -> Vec<(String, String, String)> {
    hidden_lines().into_iter().filter(|(ex, ..)| !ex).map(|(_, s, p, w)| (s, p, w)).collect()
}

/// The `!` lines of crates/site/hidden.tsv: designs published whatever the other lines say.
pub fn hidden_exceptions() -> Vec<(String, String, String)> {
    hidden_lines().into_iter().filter(|(ex, ..)| *ex).map(|(_, s, p, w)| (s, p, w)).collect()
}

fn hidden_lines() -> Vec<(bool, String, String, String)> {
    include_str!("../hidden.tsv")
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with('~'))
        .filter_map(|l| {
            let (pat, why) = l.split_once('\t')?;
            let (exception, pat) = match pat.strip_prefix('!') {
                Some(p) => (true, p),
                None => (false, pat),
            };
            let (study, file) = pat.split_once('/')?;
            Some((exception, study.to_string(), file.to_string(), why.to_string()))
        })
        .collect()
}

/// The `~` lines of crates/site/hidden.tsv: designs published only as a variant of another
/// (`~<study>/<file> TAB under <host>: <why>`): (study, file, host).
pub fn fold_lines() -> Vec<(String, String, String)> {
    include_str!("../hidden.tsv")
        .lines()
        .filter_map(|l| {
            let (pat, why) = l.strip_prefix('~')?.split_once('\t')?;
            let (study, file) = pat.split_once('/')?;
            let host = why.strip_prefix("under ")?.split(':').next()?.trim();
            Some((study.to_string(), file.to_string(), host.to_string()))
        })
        .collect()
}

/// [`seq_code`] for unit tests elsewhere in the crate.
#[cfg(test)]
pub fn test_seq_code(text: &str) -> String {
    seq_code(text)
}

/// A button sequence lock's code as built (`a b a`): for each code frame `port input.kN` in step
/// order, the item frame entity hanging in that cell, its `ItemRotation` naming the button (0 =
/// the first button input in the file, 1 = the second...: clk-item's sequence-lock judge).
fn seq_code(text: &str) -> String {
    let port = |l: &str| -> Option<(String, [f64; 3], bool)> {
        let (name, rest) = l.trim_start().strip_prefix("port input.")?.split_once(':')?;
        let v: Vec<&str> = rest.split_whitespace().collect();
        let p = [v.first()?.parse().ok()?, v.get(1)?.parse().ok()?, v.get(2)?.parse().ok()?];
        Some((name.trim().to_string(), p, v.get(3) == Some(&"frame")))
    };
    let inputs: Vec<(String, [f64; 3], bool)> = text.lines().filter_map(port).collect();
    let buttons: Vec<&String> = inputs.iter().filter(|(_, _, frame)| !frame).map(|(n, ..)| n).collect();
    let mut steps: Vec<(u32, [f64; 3])> = inputs.iter().filter(|(n, _, frame)| *frame && n.starts_with('k')).filter_map(|(n, p, _)| Some((n[1..].parse().ok()?, *p))).collect();
    steps.sort_by_key(|s| s.0);
    let rotation_at = |p: [f64; 3]| -> Option<usize> {
        text.lines().find_map(|l| {
            let rest = l.trim_start().strip_prefix("entity: minecraft:item_frame ")?;
            let v: Vec<&str> = rest.split_whitespace().collect();
            let at: Vec<f64> = v.iter().take(3).filter_map(|x| x.parse().ok()).collect();
            (at.len() == 3 && (0..3).all(|k| (at[k] - (p[k] + 0.5)).abs() < 0.01)).then_some(())?;
            let r = rest.split("ItemRotation:").nth(1)?;
            r.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
        })
    };
    let code: Option<Vec<String>> = steps.iter().map(|(_, p)| rotation_at(*p).and_then(|r| buttons.get(r).map(|b| b.to_string()))).collect();
    code.filter(|c| !c.is_empty()).map(|c| c.join(" ")).unwrap_or_default()
}

/// A port (`port input: 0 1 0`) above or below the design's own layers (`layer 0:`...): its input
/// or output stacks over it, so the design is no flat build in use (critique: "lies flat (1
/// block high)" on a design wired from above and below).
fn ports_off_layers(text: &str) -> bool {
    let layers: Vec<i64> = text.lines().filter_map(|l| l.trim().strip_prefix("layer ")?.trim_end_matches(':').trim().parse().ok()).collect();
    let (Some(lo), Some(hi)) = (layers.iter().min(), layers.iter().max()) else { return false };
    text.lines().filter_map(|l| l.trim().strip_prefix("port ")).filter_map(|rest| rest.split_once(':')).filter_map(|(_, xyz)| xyz.split_whitespace().nth(1)?.parse::<i64>().ok()).any(|y| y < *lo || y > *hi)
}

/// An extendable family's values from its base design's headers: `extend axis: a + bn` and
/// `extend n: lo..hi` -> (a + b·lo, a + b·hi, b); None without them or for another formula.
fn extend_range(text: &str) -> Option<(i64, i64, i64)> {
    let header = |k: &str| text.lines().find_map(|l| l.trim_start().strip_prefix(&format!("extend {k}:")).map(|v| v.trim().to_string()));
    let axis = header("axis")?;
    let (a, b) = match axis.split_once('+') {
        Some((a, b)) => (a.trim().parse::<i64>().ok()?, b.trim().strip_suffix('n')?.trim().parse::<i64>().ok()?),
        None => return None,
    };
    let n = header("n")?;
    let (lo, hi) = n.split_once("..")?;
    let (lo, hi) = (lo.trim().parse::<i64>().ok()?, hi.trim().parse::<i64>().ok()?);
    Some((a + b * lo, a + b * hi, b))
}

/// `*` matches any text (several allowed); the rest literally.
pub fn glob(pattern: &str, s: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == s;
    }
    let mut rest = match s.strip_prefix(parts[0]) {
        Some(r) => r,
        None => return false,
    };
    for (i, p) in parts.iter().enumerate().skip(1) {
        if i == parts.len() - 1 {
            return rest.ends_with(p);
        }
        match rest.find(p) {
            Some(k) => rest = &rest[k + p.len()..],
            None => return false,
        }
    }
    true
}

/// Why the site does not publish `study/file` (crates/site/hidden.tsv), if it does not.
pub fn hidden(study: &str, file: &str) -> Option<String> {
    hidden_design(study, file, "")
}

/// [`hidden`] with the design's class (its verdict's axis, e.g. `L = 10 gt,
/// vibration-sensitive`): a rule `<study or *>/@<class>` hides every design of that class.
pub fn hidden_design(study: &str, file: &str, class: &str) -> Option<String> {
    let matches = |s: &str, p: &str| {
        (s == study || s == "*")
            && match p.strip_prefix('@') {
                Some(c) => !class.is_empty() && class.split(", ").any(|x| x.trim() == c),
                None => glob(p, file) || glob(&format!("{p}.txt"), file),
            }
    };
    if hidden_exceptions().iter().any(|(s, p, _)| matches(s, p)) {
        return None;
    }
    hidden_rules().into_iter().find(|(s, p, _)| matches(s, p)).map(|(_, _, why)| why)
}

/// A crop farm's still with its crop grown ([`archived`]): on every `port crop` cell a column of
/// its crop, as tall as somewhere between 2 and `height` (the farm's harvest height), varied from
/// cell to cell, kept inside the design's box and in air only. Bamboo (the stalk, its leaves at the
/// top), sugar cane and cactus; None for other crops.
pub fn grown_still(text: &str, height: i32) -> Option<String> {
    let mut d = mctas::design::Design::parse(text).ok()?;
    let crop = d.meta.get("circuit")?.split_whitespace().nth(1)?.trim_start_matches("minecraft:").to_string();
    let (_, hi) = d.bounds()?;
    let cells: Vec<[i32; 3]> = d.ports.iter().filter(|(n, _)| n.as_str() == "crop" || n.starts_with("crop.")).map(|(_, p)| *p).collect();
    if cells.is_empty() || height < 2 {
        return None;
    }
    for p in cells {
        let span = (height - 1).max(1);
        let h = (2 + (p[0] * 7 + p[2] * 13 + p[1] * 3).rem_euclid(span)).min(hi[1] - p[1] + 1);
        for k in 0..h {
            let at = [p[0], p[1] + k, p[2]];
            if k > 0 && d.blocks.contains_key(&at) {
                break;
            }
            let state = match crop.as_str() {
                "bamboo" => {
                    let leaves = if k == h - 1 { "large" } else if k == h - 2 && h >= 3 { "small" } else { "none" };
                    format!("minecraft:bamboo[age=1,leaves={leaves},stage=0]")
                }
                "sugar_cane" => "minecraft:sugar_cane[age=0]".to_string(),
                "cactus" => "minecraft:cactus[age=0]".to_string(),
                _ => return None,
            };
            d.blocks.insert(at, state);
        }
    }
    Some(d.to_text())
}

/// The archived circuits of every study.
/// A design parsed, its judge's test and its 3D view check (None: it does not parse), the
/// seconds that took, and why the 3D view cannot run it (raw, None: it can).
type Simulated = (Option<(Design, Parsed)>, Option<Details>, f64, Option<String>);

/// The judge's test of design `r` of archive `dir` and whether the 3D view runs it (the viewer's
/// own session decides: shown as built when the browser's simulator cannot follow it); both
/// cached by the design's text (src/cache.rs).
/// The speed the farm judge recorded in a verdict's metrics (`game rts N`, w86 2026-10-23: the
/// robustness runs' random tick speed, `circuit::farm::game_rts`), if it is there.
pub fn recorded_game_rts(metrics: &str) -> Option<u32> {
    metrics.split(", ").find_map(|m| m.trim().strip_prefix("game rts "))?.split_whitespace().next()?.parse().ok()
}

fn simulate_row(dir: &Path, r: &circuit::archive::Row) -> Simulated {
    let loaded = parse(&dir.join(&r.file));
    let t_sim = std::time::Instant::now();
    // the shortest input the design handles (`min-pulse 20 gt`): the demo marks shorter ones
    let min_pulse = r.metrics.split(", ").find_map(|m| m.strip_prefix("min-pulse ")).and_then(|v| v.split_whitespace().next()?.parse::<u32>().ok());
    let text = std::fs::read_to_string(dir.join(&r.file)).unwrap_or_default();
    let mut det = loaded.as_ref().map(|(c, _)| details(c, &r.file, &text, min_pulse));
    let mut raw = None;
    // a big crop farm's judge's speed (`circuit::farm::game_rts`: `rts_for`, no faster than its
    // collection carries, from its 720,000 gt vanilla twin run; the coordinator and webrender
    // 2026-10-22: the page gives it to the view, `data-farm-rts`, without which a big farm is
    // shown as built). The farm judge records it in the verdict (w86 2026-10-23: `game rts N`):
    // read there; the twin runs only for a verdict without it (once, cached with the farm judge's
    // code, the circuit domain, keyed on the design). A farm of up to RTS_PLANTS plants runs at
    // the view's own default (web check holds the judge to it).
    let farm_rts: Option<u32> = loaded.as_ref().map(|(c, _)| c).filter(|c| matches!(c.kind, Some(circuit::Kind::Harvester(_))) && circuit::farm::rts_for(c) < circuit::farm::RTS).map(|c| {
        recorded_game_rts(&r.metrics).unwrap_or_else(|| {
            cache::get_or(Domain::Circuit, &[b"farm-rts", text.as_bytes()], || {
                let vanilla = circuit::farm::vanilla_run(c, 3);
                circuit::farm::game_rts(c, c, &vanilla)
            })
        })
    });
    if let Some(d) = det.as_mut() {
        d.farm_rts = farm_rts;
        let rts_key = farm_rts.unwrap_or(0).to_le_bytes();
        let v: simulate::ViewCheck = cache::get_or(Domain::View, &[text.as_bytes(), &simulate::VIEW_RUN.to_le_bytes(), &rts_key], || simulate::view_check_with(&text, farm_rts));
        // (the run counts shulker boxes: a shulker machine's only; the viewer's machine supply now
        // feeds the item sorter too, whose run read "WARNING ... NO box")
        let shulker = loaded.as_ref().is_some_and(|(c, _)| c.kind.as_ref().is_some_and(|k| k.name().starts_with("shulker-")));
        d.view_run = v.view_run.filter(|_| shulker);
        raw = v.unsimulated;
        d.unmodeled = raw.as_deref().map(crate::explain::plain_reason).unwrap_or_default();
        // a crop farm whose crop the view does not plant (mcview grows sugar cane; bamboo and kelp
        // wait for redsim's): it would run with nothing planted, so it is shown as built, its page
        // says so and does not autoplay
        if raw.is_none() && text.lines().any(|l| l.trim_start().starts_with("circuit: harvester")) && mcview::design::Design::parse(&text).ok().is_some_and(|md| mcview::circuit::harvester_ports(&md).is_none()) {
            let crop = text.lines().find_map(|l| l.trim_start().strip_prefix("circuit: harvester")).map(|c| c.trim().trim_start_matches("minecraft:").replace('_', " ")).unwrap_or_default();
            let crop = match crop.as_str() {
                "pumpkin" => "pumpkins".to_string(),
                "melon" => "melons".to_string(),
                _ => crop,
            };
            d.unmodeled = format!("the browser's simulator does not grow {crop} yet");
        }
        // where the view's demo stops (the judge's stimulus, as the page autoplays it)
        if raw.is_none() && !d.stimulus.is_empty() {
            let stimulus = d.stimulus.clone();
            d.view_halt = cache::get_or(Domain::View, &[b"demo", text.as_bytes(), stimulus.as_bytes()], || simulate::demo_halt(&text, &stimulus)).unwrap_or_default();
        }
    }
    (loaded, det, t_sim.elapsed().as_secs_f64(), raw)
}

/// Every design archived under `root` (`<study>/archive/verdicts.tsv`) as a build; `mctas site
/// preview` (src/preview.rs) runs it on a folder of its own holding one design and its row.
pub(crate) fn archived(root: &Path, notes: &mut Vec<String>, keep_hidden: bool) -> Vec<Build> {
    let mut out = Vec::new();
    // every published design of every study simulated in one parallel pass (study after study,
    // the cores waited behind each study's slowest design: the TNT dupers' 25 s, the bursts' 16 s)
    let jobs: Vec<(String, PathBuf, circuit::archive::Row)> = studies()
        .into_iter()
        .flat_map(|(study, _)| {
            let dir = archive_dir(root, study);
            read_rows(&dir).unwrap_or_default().into_iter().filter(|r| keep_hidden || hidden_design(study, &r.file, &r.axis).is_none()).map(move |r| (study.to_string(), dir.clone(), r))
        })
        .collect();
    let mut pre: BTreeMap<(String, String), Simulated> = jobs.iter().map(|(s, _, r)| (s.clone(), r.file.clone())).zip(par_map(&jobs, |(_, dir, r)| simulate_row(dir, r))).collect();
    // every archived (study, file, class), to report hidden.tsv lines that match nothing
    let mut all_rows: Vec<(String, String, String)> = Vec::new();
    for (study, _title) in studies() {
        let dir = archive_dir(root, study);
        let rows = match read_rows(&dir) {
            Ok(r) => r,
            Err(e) => {
                notes.push(format!("{}: {e:#}", dir.display()));
                continue;
            }
        };
        all_rows.extend(rows.iter().map(|r| (study.to_string(), r.file.clone(), r.axis.clone())));
        // the designs the site does not publish (crates/site/hidden.tsv)
        let before = rows.len();
        let rows: Vec<circuit::archive::Row> = rows.into_iter().filter(|r| keep_hidden || hidden_design(study, &r.file, &r.axis).is_none()).collect();
        if rows.len() < before {
            notes.push(format!("{study}: {} designs not published (crates/site/hidden.tsv)", before - rows.len()));
        }
        if rows.is_empty() {
            continue;
        }
        let caps = captions(&dir);
        // the extendable families' archived members (`extend.tsv`: family, n, axis, blocks, type
        // key): a flying machine of the payload family carries n extra blocks, of the train family
        // pulls n trailers
        let members_of: BTreeMap<String, (String, i64)> = std::fs::read_to_string(dir.join("extend.tsv"))
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f.len() >= 5).then(|| (f[4].to_string(), (f[0].to_string(), f[1].parse().unwrap_or(0))))
            })
            .collect();
        // the catalog's fold: within one kind and axis value
        let mut groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        for (i, r) in rows.iter().enumerate() {
            groups.entry((r.kind.clone(), r.axis.clone())).or_default().push(i);
        }
        let mut host: BTreeMap<usize, usize> = BTreeMap::new();
        for g in groups.values() {
            // as `mctas circuit catalog` folds (palette alone is never a distinction)
            let items: Vec<(usize, i64, String)> = g.iter().map(|&i| (rows[i].blocks, rows[i].volume, rows[i].file.clone())).collect();
            let robust: Vec<bool> = g.iter().map(|&i| circuit::archive::is_robust(&rows[i])).collect();
            for (k, h) in clock::catalog::fold_hosts_robust(&items, &robust, |a, b| circuit::archive::beats_on_variation(&rows[g[a]], &rows[g[b]])).into_iter().enumerate() {
                if let Some(h) = h {
                    host.insert(g[k], g[h]);
                }
            }
        }
        let id_of = |r: &circuit::archive::Row| format!("circuits/{study}/{}", r.file.trim_end_matches(".txt"));
        // each design's judge test and 3D view check, from the pass over every study above
        let loaded_all: Vec<Simulated> = rows.iter().map(|r| pre.remove(&(study.to_string(), r.file.clone())).unwrap_or_else(|| simulate_row(&dir, r))).collect();
        for (i, (r, (loaded, det, took, raw))) in rows.iter().zip(loaded_all).enumerate() {
            let path = dir.join(&r.file);
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if let Some(raw) = raw {
                notes.push(format!("{study}/{}: the 3D view shows it as built: {raw}", r.file));
            }
            if let Some(v) = det.as_ref().and_then(|d| d.view_run.as_ref()) {
                notes.push(match (v.first, v.cycle) {
                    (Some(f), Some(c)) => format!("{study}/{}: the 3D view's run sends out {} boxes in {} gt (the first at {f} gt, then one every {c:.0} gt)", r.file, v.boxes, v.ticks),
                    (Some(f), None) => format!("{study}/{}: the 3D view's run sends out 1 box in {} gt (at {f} gt)", r.file, v.ticks),
                    _ => format!("{study}/{}: WARNING the 3D view's run sends out NO box in {} gt (its chests supply {}; {} items reached the reject chests)", r.file, v.ticks, v.item, v.rejected),
                });
            }
            if took > 10.0 {
                notes.push(format!("{study}/{}: the judge's test took {took:.1} s to simulate", r.file));
            }
            // a flying machine that drags extra blocks (`port payload`) or flies between stations
            // (a one-way machine started by a lever has station ports too, for its lever's
            // station: "there and back" is a two-way machine's, by its axis)
            let (cargo, payloads) = loaded.as_ref().map(|(c, _)| (!c.payloads.is_empty(), c.payloads.len())).unwrap_or((false, 0));
            let stations = r.axis.contains("two-way");
            let parsed = loaded.map(|(_, p)| p);
            let metrics = parse_metrics(&r.metrics);
            // the declared stand (`stand: N`): the judge writes it only when it is higher than the
            // safe drop (circuit::robust::drop_metrics), so a duper declared at exactly its safe
            // drop (the lever engine: stand 6, safe drop 6) read as standing on the ground
            let mut metrics = metrics;
            let safe = metrics.iter().find(|(k, _)| k == "safe drop").and_then(|(_, v)| v.split_whitespace().next()?.parse::<u32>().ok());
            if !metrics.iter().any(|(k, _)| k == "stands")
                && let Some(n) = text.lines().find_map(|l| l.trim().strip_prefix("stand:")).and_then(|v| v.trim().parse::<u32>().ok()).filter(|n| *n > 0 && safe.is_some_and(|s| *n >= s))
            {
                metrics.push(("stands".into(), format!("{n} blocks above the ground (stand: in the design)")));
            }
            let mut flags = flags_of(&metrics, &r.robustness, &r.summary);
            let host_id = host.get(&i).map(|&h| id_of(&rows[h]));
            if host_id.is_some() {
                flags.push("variant".into());
            }
            // the design's own input device (vjeux: "one button" matters to a builder), and its
            // box's shape
            if let Some(d) = &det {
                if d.devices.iter().any(|x| x == "button") {
                    flags.push("button".into());
                } else if d.devices.iter().any(|x| x == "lever") {
                    flags.push("lever".into());
                }
            }
            // a piston extender with no slime or honey: its own distinction (vjeux's list view
            // named "slimeless" as a property he cares about; the coordinator 19:05Z)
            if r.kind.starts_with("piston-extender") && !r.kinds.split('+').any(|k| k == "slime_block" || k == "honey_block") {
                flags.push("slime-free".into());
            }
            // what sets such a flying machine apart from the 6-block one (critique C13: fly cards
            // of 6, 7, 15 and 24 blocks said the same); an extender's pushed block is a payload too
            let flying = r.kind.starts_with("flying-machine");
            if cargo && flying {
                flags.push("cargo".into());
            }
            if stations && flying {
                flags.push("there-and-back".into());
            }
            let id = id_of(r);
            let basics = (study == "gates" && flags.iter().any(|f| f == "lever")) || (r.kind.starts_with("piston-extender") && r.kinds.split('+').any(|k| k == "sand" || k == "gravel" || k == "red_sand"));
            let kinds = parsed.as_ref().map(|p| p.kinds.clone()).unwrap_or_else(|| r.kinds.split('+').filter(|k| !k.is_empty()).map(String::from).collect());
            out.push(Build {
                id: id.clone(),
                kind: "circuit".into(),
                // a piston extender that lifts sand and lets it fall back is its own thing (critique:
                // "SAND ONLY, UPWARD ONLY ... not a general extender")
                group: if r.kind.starts_with("piston-extender") && r.kinds.split('+').any(|k| k == "sand" || k == "gravel" || k == "red_sand") { "Sand elevator".to_string() } else { site_model::html::kind_words(&r.kind) },
                name: r.file.trim_end_matches(".txt").to_string(),
                page: format!("{id}.html"),
                design: format!("designs/circuits/{study}/{}", r.file),
                blocks: r.blocks as u32,
                volume: Some(r.volume.max(0) as u32),
                dims: Some(r.dims.clone()).filter(|d| !d.is_empty()),
                kinds,
                caption: caps.get(&r.file).cloned().unwrap_or_else(|| comment_caption(&text)),
                flags,
                circuit: Some(Circuit {
                    study: study.to_string(),
                    // its study's place, the basics last (the gates with their own levers: the
                    // coordinator's decision (c); the sand elevators)
                    order: crate::explain::studies().iter().position(|s| s.0 == study).unwrap_or(99) as u32 * 100 + if basics { 50 } else { 0 },
                    kind: r.kind.clone(),
                    axis: r.axis.clone(),
                    // (a randomizer's axis is `1 of N`: its value is N, from its kind; an adder's
                    // axis names which adder, "4-bit adder slice" is no value of 4)
                    value: if r.kind.starts_with("randomizer ") { number(&r.kind) } else if r.kind.starts_with("adder") { None } else { number(&r.axis) },
                    metrics,
                    verdict: r.summary.clone(),
                    inputs: parsed.as_ref().map(|p| p.inputs.clone()).unwrap_or_default(),
                    input_labels: viewer_input_labels(&text, &parsed.as_ref().map(|p| p.inputs.clone()).unwrap_or_default()),
                    outputs: parsed.as_ref().map(|p| p.outputs.clone()).unwrap_or_default(),
                    direction: parsed.as_ref().map(|p| p.direction.clone()).unwrap_or_default(),
                    family: r.family.clone(),
                    robustness: r.robustness.clone(),
                    host: host_id,
                    archived: r.archived.clone(),
                    devices: det.as_ref().map(|d| d.devices.clone()).unwrap_or_default(),
                    stimulus: det.as_ref().map(|d| d.stimulus.clone()).unwrap_or_default(),
                    levels: det.as_ref().map(|d| d.levels.clone()).unwrap_or_default(),
                    phases: det.as_ref().map(|d| d.phases.clone()).unwrap_or_default(),
                    wave: det.as_ref().and_then(|d| d.wave.clone()),
                    table: det.as_ref().map(|d| d.table.clone()).unwrap_or_default(),
                    // (the lamp the 3D view adds on air outputs: the viewer's own rule, no lamp where
                    // the judge puts its chests, a farm's, a smelter's: fly's QA, "not counting the
                    // test lamp" on every farm page)
                    output_lamp: det.as_ref().is_some_and(|d| d.output_lamp) && mcview::Design::parse(&text).is_ok_and(|d| !mcview::circuit::consumer_cells(&d).is_empty()),
                    events: det.as_ref().map(|d| d.events.clone()).unwrap_or_default(),
                    unmodeled: det.as_ref().map(|d| d.unmodeled.clone()).unwrap_or_default(),
                    view_halt: det.as_ref().map(|d| d.view_halt.clone()).unwrap_or_default(),
                    view_run: det.as_ref().and_then(|d| d.view_run.clone()),
                    extends: text.lines().any(|l| l.trim_start().starts_with("extend ")),
                    extend_range: extend_range(&text),
                    tile: text.lines().find_map(|l| l.trim_start().strip_prefix("tile:")).map(|v| v.trim().to_string()).unwrap_or_default(),
                    crops: text.lines().filter(|l| l.trim_start().starts_with("port crop")).count() as u32,
                    flying_machines: flying_machines(&text),
                    // a display's lit still: its face settled at a digit (the coordinator
                    // 2026-10-22), the design as built in its downloads
                    view_text: mcview::Design::parse(&text).ok().filter(|d| mcview::circuit::display_face(d).is_some()).and_then(|_| crate::simulate::lit_snapshot(&text, crate::simulate::STILL_LEVEL)).unwrap_or_default(),
                    soil: crop_ground(&text).0,
                    crop_wet: crop_ground(&text).1,
                    extend_n: text.lines().find_map(|l| l.trim_start().strip_prefix("extend n:")).and_then(|v| v.trim().split_once("..")).and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?))),
                    ports_off_layers: ports_off_layers(&text),
                    seq_code: if r.kind.starts_with("sequence-lock") { seq_code(&text) } else { String::new() },
                    counter_on: det.as_ref().and_then(|d| d.counter_on),
                    farm_rts: det.as_ref().and_then(|d| d.farm_rts),
                    // (only for designs that hold items: the same blocks with other contents)
                    // (designs that hold items: the same blocks with other contents; shulker
                    // machines: the same blocks timed with other items, 1-, 16- or 64-stack)
                    layout_key: if r.type_key.contains('{') { strip_contents(&r.type_key) } else if study == "shulker" { r.type_key.clone() } else { String::new() },
                    feature: match members_of.get(&r.type_key) {
                        Some((fam, n)) if r.kind.starts_with("flying-machine") && fam.starts_with("fly-payload") => format!("carries {n} extra block{}", if *n == 1 { "" } else { "s" }),
                        Some((fam, n)) if r.kind.starts_with("flying-machine") && fam.starts_with("fly-train") => format!("pulls {n} trailer{}", if *n == 1 { "" } else { "s" }),
                        // (no family: its own payload ports, fly-payload-start-1-11b's one block)
                        _ if r.kind.starts_with("flying-machine") && payloads > 0 && !r.axis.contains("two-way") => format!("carries {payloads} extra block{}", if payloads == 1 { "" } else { "s" }),
                        // (an iron farm: how it works, its design's `how:` line, the end of its title)
                        _ if r.kind.starts_with("mob-farm") => text.lines().find_map(|l| l.trim().strip_prefix("how:")).map(|v| v.trim().to_string()).unwrap_or_default(),
                        _ => String::new(),
                    },
                    ..Default::default()
                }),
                archived: site_model::normalize_time(&r.archived),
                ..Default::default()
            });
            // (the lit still's path, next to the designs: `designs/stills/<study>/<file>`)
            if let Some(c) = out.last_mut().and_then(|b| b.circuit.as_mut())
                && c.view_text.is_empty()
                && c.kind.starts_with("harvester")
            {
                // a crop farm's field grown (the coordinator 2026-10-22: "the views see mostly
                // stone walls and an empty field"): every plant somewhere between a fresh cut and
                // the harvest height. Shown as built, its page view and card show it; replayed,
                // only its card (cards never simulate), and its page view loads the design as
                // built so a played view starts from the judge's planting
                let height = c.metrics.iter().find(|(k, _)| k == "harvest height").and_then(|(_, v)| v.split_whitespace().next()?.parse::<i32>().ok()).unwrap_or(3);
                if let Some(grown) = grown_still(&text, height) {
                    if c.unmodeled.is_empty() {
                        c.card_text = grown;
                        c.card_src = format!("designs/stills/{study}/{}", r.file);
                    } else {
                        c.view_text = grown;
                    }
                }
            }
            if let Some(c) = out.last_mut().and_then(|b| b.circuit.as_mut())
                && !c.view_text.is_empty()
            {
                c.view_src = format!("designs/stills/{study}/{}", r.file);
            }
        }
    }
    // a hidden.tsv line that matches no archived design is stale: its designs left or were
    // renamed, and its pattern would hide a future design by the same name (take it out)
    if !keep_hidden {
        let stale: Vec<String> = hidden_lines()
            .into_iter()
            .filter(|(_, s, p, _)| {
                !all_rows.iter().any(|(study, file, class)| {
                    (s == study || s == "*")
                        && match p.strip_prefix('@') {
                            Some(c) => class.split(", ").any(|x| x.trim() == c),
                            None => glob(p, file) || glob(&format!("{p}.txt"), file),
                        }
                })
            })
            .map(|(ex, s, p, _)| format!("{}{s}/{p}", if ex { "!" } else { "" }))
            .collect();
        if !stale.is_empty() {
            notes.push(format!("hidden.tsv: {} lines match no archived design (take them out; a stale pattern hides a future design): {}", stale.len(), stale.join(", ")));
        }
    }
    out
}

/// Every archived circuit of the studies. The judge's calibration and selftest designs
/// (`designs/circuits/calibration/`, `selftest/`) stay in the repository for the judge and are
/// not published (vjeux: "They are not useful to people").
pub fn export(repo: &Path) -> Result<(Vec<Build>, Vec<String>)> {
    let (mut builds, mut notes) = export_with(repo, false)?;
    // one world-best design per kind (vjeux 21:47Z); the rest stay in the repository
    notes.extend(crate::kinds::select_circuits(&mut builds));
    Ok((builds, notes))
}

/// [`export`], with the designs crates/site/hidden.tsv keeps off the site too (`keep_hidden`:
/// the dominated-design audit).
pub fn export_with(repo: &Path, keep_hidden: bool) -> Result<(Vec<Build>, Vec<String>)> {
    let root = repo.join(ROOT);
    let mut notes = Vec::new();
    let mut builds = archived(&root, &mut notes, keep_hidden);
    // the studies' whole lines (a loader, its track and an unloader: sorter's cart transport)
    builds.extend(systems(&root, &mut notes, keep_hidden));
    fold_same_builds(&root, &mut builds);
    apply_catalog(&root, &mut builds, &mut notes);
    builds.sort_by(|a, b| {
        // study (in the studies' order), kind, then the kind's main number, then blocks: every
        // list follows this order (vjeux 12:09Z: sort by the main number, then blocks)
        let order = studies();
        let s = |x: &Build| {
            x.circuit
                .as_ref()
                .map(|c| (order.iter().position(|s| s.0 == c.study).unwrap_or(usize::MAX), x.group.clone(), c.value.unwrap_or(i64::MAX)))
                .unwrap_or((usize::MAX, String::new(), i64::MAX))
        };
        (s(a), a.blocks, &a.name).cmp(&(s(b), b.blocks, &b.name))
    });
    // a bigger design that is faster than every smaller one of its value says so
    let n = crate::audit::mark_faster(&mut builds);
    notes.push(format!("circuits: {n} designs marked faster than the smallest of their value"));
    let shown = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty()));
    // each shulker machine's card leads with the variation it is the best design for (critique:
    // cards with the same numbers differed only by badges): per badge, the fastest design having
    // it (then the smallest); the fastest of all leads with "fastest"
    let mut leads: Vec<(usize, String)> = Vec::new();
    for kind in ["shulker-loader", "shulker-unloader"] {
        let rate = |b: &Build| b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "items/s")).and_then(|(_, v)| v.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0);
        let list: Vec<usize> = builds.iter().enumerate().filter(|(_, b)| shown(b) && b.circuit.as_ref().is_some_and(|c| c.kind.starts_with(kind))).map(|(i, _)| i).collect();
        let better = |a: usize, b: usize| (-rate(&builds[a]), builds[a].blocks, builds[a].name.clone()).partial_cmp(&(-rate(&builds[b]), builds[b].blocks, builds[b].name.clone())).unwrap_or(std::cmp::Ordering::Equal);
        let badges_of = |i: usize| -> Vec<String> { builds[i].circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "badges")).map(|(_, v)| v.split('+').filter(|s| !s.is_empty()).map(String::from).collect()).unwrap_or_default() };
        let mut best: BTreeMap<String, usize> = BTreeMap::new();
        for &i in &list {
            for badge in badges_of(i) {
                match best.get(&badge) {
                    Some(&j) if better(j, i) != std::cmp::Ordering::Greater => {}
                    _ => {
                        best.insert(badge, i);
                    }
                }
            }
        }
        let fastest = list.iter().copied().min_by(|&a, &b| better(a, b));
        let smallest = list.iter().copied().min_by_key(|&j| (builds[j].blocks, builds[j].name.clone()));
        // how many cards carry each badge: the rarest say the most about a card (critique: the
        // smallest loader led with "always on + 16-stack items", not "tiles every 3 blocks")
        let mut count: BTreeMap<String, usize> = BTreeMap::new();
        for &j in &list {
            for bd in badges_of(j) {
                *count.entry(bd).or_default() += 1;
            }
        }
        // what each card adds over the smaller ones at least as fast (critique: cards of 31, 37
        // and 65 blocks all led with the same rate): its new badges, else the variation it is the
        // best design for
        for &i in &list {
            let smaller: Vec<usize> = list.iter().copied().filter(|&j| j != i && (builds[j].blocks, &builds[j].name) < (builds[i].blocks, &builds[i].name) && rate(&builds[j]) >= rate(&builds[i])).collect();
            let covered: BTreeSet<String> = smaller.iter().flat_map(|&j| badges_of(j)).collect();
            let mine = badges_of(i);
            let mut new: Vec<String> = mine.iter().filter(|bd| !covered.contains(*bd)).cloned().collect();
            new.sort_by_key(|bd| (count.get(bd).copied().unwrap_or(0), bd.clone()));
            // (no new badge alone: a pair no smaller card has together, "sends out a part-full box,
            // and restarts by itself too"; the rarer first, one most cards share said with "too" so
            // it does not contradict the shared line: critique)
            let common = |bd: &String| count.get(bd).copied().unwrap_or(0) * 3 >= list.len() * 2;
            let pair = || -> Option<String> {
                for (x, a) in mine.iter().enumerate() {
                    for b in &mine[x + 1..] {
                        if !smaller.iter().any(|&j| { let o = badges_of(j); o.contains(a) && o.contains(b) }) {
                            let (rare, other) = if count.get(a).copied().unwrap_or(0) <= count.get(b).copied().unwrap_or(0) { (a, b) } else { (b, a) };
                            return Some(if common(other) {
                                format!("{}, and {} too", site_model::html::badge_words(rare), site_model::html::badge_words(other))
                            } else {
                                format!("{} + {}", site_model::html::badge_words(rare), site_model::html::badge_words(other))
                            });
                        }
                    }
                }
                None
            };
            // the metric it wins comes before shared badges (critique: the 96-block speed loader
            // led with the 31-block auto loader's badges; what it adds is 3× the speed)
            let max_smaller = list.iter().copied().filter(|&j| j != i && (builds[j].blocks, &builds[j].name) < (builds[i].blocks, &builds[i].name)).map(|j| rate(&builds[j])).fold(0.0f64, f64::max);
            let speedup = if max_smaller > 0.0 { rate(&builds[i]) / max_smaller } else { 0.0 };
            let lead = if Some(i) == fastest {
                Some("fastest".to_string())
            } else if speedup >= 1.5 {
                Some(format!("{}× faster than any smaller one", if speedup >= 2.0 { format!("{:.0}", speedup.round()) } else { format!("{speedup:.1}") }))
            } else if Some(i) == smallest {
                // the smallest says so, with the rarest thing it does
                Some(match new.first() {
                    Some(bd) => format!("smallest · {}", site_model::html::badge_words(bd)),
                    None => "smallest".to_string(),
                })
            } else if !new.is_empty() {
                Some(new.iter().take(2).map(|bd| site_model::html::badge_words(bd)).collect::<Vec<_>>().join(" + "))
            } else if let Some(p) = pair() {
                Some(p)
            } else if builds[i].flags.iter().any(|f| f == "lever") && !mine.iter().any(|bd| bd == "always-on") {
                // (what a reference machine adds over the smaller always-on ones: critique)
                Some("has its own lever (can be switched off)".to_string())
            } else if let Some((me, them)) = {
                // fewer awake hoppers than every smaller one at least as fast that does everything
                // it does: less lag (the coordinator 20:26Z: clk-rt keeps shulker-auto-loader for
                // its 3 hoppers against pitch3's 4)
                let awake = |b: &Build| b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "hoppers awake")).and_then(|(_, v)| v.split_whitespace().next()?.parse::<u32>().ok());
                let does_all: Vec<usize> = smaller.iter().copied().filter(|&j| { let o = badges_of(j); mine.iter().all(|bd| o.contains(bd)) }).collect();
                let theirs = does_all.iter().filter_map(|&j| awake(&builds[j])).min();
                match (awake(&builds[i]), theirs) {
                    (Some(a), Some(b)) if a < b && does_all.iter().all(|&j| awake(&builds[j]).is_some_and(|w| w > a)) => Some((a, b)),
                    _ => None,
                }
            } {
                Some(format!("less lag: {me} active hoppers instead of {them}"))
            } else if let Some(gain) = {
                // its first box sooner than every smaller one at least as fast: what it adds
                let first = |b: &Build| b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "first box")).and_then(|(_, v)| v.split_whitespace().next()?.parse::<u32>().ok());
                let mine_first = first(&builds[i]);
                let theirs = smaller.iter().filter_map(|&j| first(&builds[j])).min();
                match (mine_first, theirs) {
                    (Some(a), Some(b)) if a < b => Some(b - a),
                    _ => None,
                }
            } {
                Some(format!("first box {} sooner", site_model::real_time(gain)))
            } else {
                mine.iter().find(|bd| best.get(*bd) == Some(&i)).map(|bd| site_model::html::badge_words(bd))
            };
            if let Some(l) = lead {
                leads.push((i, l));
            }
        }
    }
    // piston extenders of one reach: each card leads with what sets it apart from the others of
    // its reach (critique: the 7- and 8-block slime-free cards read the same): a property some of
    // the others lack (no slime, 1 block wide, its own button...) and the speed it wins
    let mut by_reach: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        if shown(b) && b.group != "Sand elevator" && b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("piston-extender")) {
            let c = b.circuit.as_ref().unwrap();
            by_reach.entry((c.kind.clone(), c.axis.clone())).or_default().push(i);
        }
    }
    const SETS_APART: [&str; 6] = ["no slime", "1 block wide", "lies flat (1 block high)", "has its own button", "has its own lever", "comes in every size"];
    for list in by_reach.values().filter(|l| l.len() > 1) {
        // (an extendable family's base is marked later: its `extends`)
        let chips: Vec<Vec<String>> = list
            .iter()
            .map(|&i| {
                let mut v = site_model::html::props(&builds[i]).chips;
                if builds[i].circuit.as_ref().is_some_and(|c| c.extends) {
                    v.push("comes in every size".into());
                }
                v
            })
            .collect();
        let metric = |i: usize, k: &str| -> Option<f64> { builds[i].circuit.as_ref()?.metrics.iter().find(|(n, _)| n == k)?.1.split_whitespace().next()?.parse().ok() };
        for (x, &i) in list.iter().enumerate() {
            let mut parts: Vec<String> = Vec::new();
            // the strictly smallest of its reach says so
            if list.iter().all(|&j| j == i || builds[j].blocks > builds[i].blocks) {
                parts.push("smallest".into());
            }
            parts.extend(SETS_APART.iter().filter(|p| chips[x].iter().any(|c| c == *p) && chips.iter().enumerate().any(|(y, o)| y != x && !o.iter().any(|c| c == *p))).map(|p| p.to_string()));
            for (k, words) in [("extend", "fastest out"), ("retract", "fastest back")] {
                if let Some(v) = metric(i, k) {
                    if list.iter().all(|&j| j == i || metric(j, k).is_some_and(|w| v < w)) {
                        parts.push(words.to_string());
                    }
                }
            }
            if !parts.is_empty() {
                leads.push((i, parts.into_iter().take(3).collect::<Vec<_>>().join(", ")));
            }
        }
    }
    // toggle switches: each card leads with what sets it apart from the smaller ones (a material
    // it avoids, the edge it reacts on); the smallest says so (critique: a 7-block toggle lost
    // every number to the 3-block one, with nothing saying what it adds)
    {
        let toggles: Vec<usize> = builds.iter().enumerate().filter(|(_, b)| shown(b) && b.circuit.as_ref().is_some_and(|c| c.kind == "tff")).map(|(i, _)| i).collect();
        const AVOIDS: [(&str, &[&str]); 5] = [
            ("copper", &["copper_bulb", "waxed_copper_bulb", "exposed_copper_bulb", "weathered_copper_bulb", "oxidized_copper_bulb", "waxed_exposed_copper_bulb", "waxed_weathered_copper_bulb", "waxed_oxidized_copper_bulb"]),
            ("pistons", &["piston", "sticky_piston"]),
            ("slime", &["slime_block"]),
            ("hoppers", &["hopper"]),
            ("dispensers", &["dispenser"]),
        ];
        let uses = |i: usize, blocks: &[&str]| builds[i].kinds.iter().any(|k| blocks.contains(&k.as_str()));
        let edge = |i: usize| builds[i].circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "edge")).map(|(_, v)| v.clone()).unwrap_or_default();
        let smallest = toggles.iter().copied().min_by_key(|&i| (builds[i].blocks, builds[i].name.clone()));
        for &i in &toggles {
            let smaller: Vec<usize> = toggles.iter().copied().filter(|&j| j != i && (builds[j].blocks, &builds[j].name) < (builds[i].blocks, &builds[i].name)).collect();
            let mut parts: Vec<String> = Vec::new();
            if Some(i) == smallest {
                parts.push("smallest".into());
            }
            // the materials it avoids that every smaller one needs one of (critique: the crafter
            // toggle lost every number to the copper one, nothing said it needs no copper,
            // pistons or dispenser)
            let mine_avoids: Vec<(&str, &[&str])> = AVOIDS.iter().copied().filter(|(_, blocks)| !uses(i, blocks)).collect();
            if !smaller.is_empty() && smaller.iter().all(|&j| mine_avoids.iter().any(|(_, blocks)| uses(j, blocks))) {
                let named: Vec<&str> = mine_avoids.iter().filter(|(_, blocks)| smaller.iter().any(|&j| uses(j, blocks))).map(|(w, _)| *w).collect();
                let words = match named.len() {
                    1 => named[0].to_string(),
                    n => format!("{} or {}", named[..n - 1].join(", "), named[n - 1]),
                };
                parts.push(format!("no {words}"));
            }
            if edge(i) == "falling" {
                parts.push("flips on release".into());
            }
            if !parts.is_empty() {
                leads.push((i, parts.into_iter().take(2).collect::<Vec<_>>().join(", ")));
            }
        }
    }
    // a group or an extendable family whose lengths a smaller one of its kind already covers in
    // part says which it adds (critique: "18- to 34-tick pulse · 24 to 28 blocks" and "30-tick
    // pulse · 33 blocks" after "16- to 40-tick pulse · 13 blocks", with no reason)
    {
        let values_of = |i: usize| -> BTreeSet<i64> {
            let b = &builds[i];
            let Some(c) = &b.circuit else { return BTreeSet::new() };
            if let Some((lo, hi, step)) = c.extend_range.filter(|r| r.2 > 0) {
                return (0..).map(|k| lo + k * step).take_while(|v| *v <= hi).collect();
            }
            if !c.tunable.is_empty() {
                return builds.iter().filter_map(|o| o.circuit.as_ref().filter(|oc| oc.tunable == c.tunable && oc.host.is_none()).and_then(|oc| oc.value)).collect();
            }
            c.value.into_iter().collect()
        };
        let mut by_kind: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, b) in builds.iter().enumerate() {
            if let Some(c) = b.circuit.as_ref().filter(|_| shown(b)) {
                if matches!(c.kind.as_str(), "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "delay") || c.kind.starts_with("monostable") || c.kind.starts_with("off-delay") {
                    by_kind.entry(c.kind.clone()).or_default().push(i);
                }
            }
        }
        for list in by_kind.values() {
            for &i in list {
                let Some(c) = &builds[i].circuit else { continue };
                if !c.lead.is_empty() || (c.tunable.is_empty() && c.extend_range.is_none()) {
                    continue;
                }
                let covered: BTreeSet<i64> = list.iter().copied().filter(|&j| j != i && builds[j].blocks < builds[i].blocks).flat_map(values_of).collect();
                if covered.is_empty() {
                    continue;
                }
                let mine = values_of(i);
                let new: Vec<i64> = mine.iter().copied().filter(|v| !covered.contains(v)).collect();
                // (nothing new; or everything new and beyond the smaller ones: its range in the
                // title already says it)
                let beyond = new.iter().all(|v| *v > covered.iter().copied().max().unwrap_or(0));
                if new.is_empty() || (new.len() == mine.len() && beyond) {
                    continue;
                }
                let covered_max = covered.iter().copied().max().unwrap_or(0);
                let list = |v: &[i64]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
                let words = if new.iter().all(|v| *v > covered_max) {
                    format!("goes past {covered_max} ticks, up to {}", new.last().copied().unwrap_or(0))
                } else if new.len() <= 3 {
                    format!("adds {} ticks", list(&new))
                } else {
                    format!("adds {}… ticks", list(&new[..3]))
                };
                leads.push((i, words));
            }
        }
    }
    for (i, l) in leads {
        if let Some(c) = builds[i].circuit.as_mut() {
            c.lead = l;
        }
    }
    // a gate and its mirror (A AND (NOT B) / B AND (NOT A)) are one card: the bigger folds under
    // the smaller as a variant, the kept one says it comes mirrored (critique: two cards with one
    // picture and one subtitle)
    let gate_fn = |b: &Build| b.circuit.as_ref().filter(|c| c.kind.starts_with("gate ")).map(|c| c.kind.split_whitespace().nth(1).unwrap_or("").to_string());
    let mut mirrors: Vec<(usize, usize)> = Vec::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(f) = gate_fn(b) else { continue };
        let Some(plain) = f.strip_suffix("-ba") else { continue };
        if !shown(b) {
            continue;
        }
        let best_plain = builds.iter().enumerate().filter(|(_, o)| shown(o) && gate_fn(o).as_deref() == Some(plain) && o.flags.iter().any(|x| x == "lever") == b.flags.iter().any(|x| x == "lever")).min_by_key(|(_, o)| (o.blocks, o.name.clone())).map(|(j, _)| j);
        if let Some(j) = best_plain {
            // (keep the smaller; a tie keeps the plain function)
            if builds[i].blocks < builds[j].blocks {
                mirrors.push((j, i));
            } else {
                mirrors.push((i, j));
            }
        }
    }
    for (fold, keep) in mirrors {
        let keep_id = builds[keep].id.clone();
        builds[fold].flags.push("variant".into());
        if let Some(c) = builds[fold].circuit.as_mut() {
            c.host = Some(keep_id);
        }
        if !builds[keep].flags.iter().any(|f| f == "mirrored") {
            builds[keep].flags.push("mirrored".into());
        }
    }
    // one build archived under two kinds (the same blocks and ports: `same_build`) shows once on
    // the Circuits tab, with both names (critique: "one pulse when switched on" and "signal
    // shortener" were the same 2 blocks, twice): the later one is a `kind-twin`, hidden unless its
    // own kind is picked
    let id_of = |b: &Build| b.circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, b.name)).unwrap_or_default();
    let mut twins: Vec<(usize, usize)> = Vec::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(c) = &b.circuit else { continue };
        if !shown(b) || c.same_build.is_empty() {
            continue;
        }
        for (j, o) in builds.iter().enumerate() {
            if j != i && shown(o) && c.same_build.contains(&id_of(o)) && o.group != b.group {
                let key = |x: &Build| (x.circuit.as_ref().map(|c| c.order).unwrap_or(0), x.group.clone(), x.name.clone());
                if key(o) < key(b) {
                    twins.push((i, j));
                }
            }
        }
    }
    for (later, first) in twins {
        let words = builds[later].group.clone();
        if !builds[later].flags.iter().any(|f| f == "kind-twin") {
            builds[later].flags.push("kind-twin".into());
        }
        if let Some(c) = builds[first].circuit.as_mut() {
            if !c.also.contains(&words) {
                c.also.push(words);
            }
        }
    }
    // each design's place within its kind (the Circuits tab shows the first of every kind first)
    let mut by_kind: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        if shown(b) {
            by_kind.entry(b.group.clone()).or_default().push(i);
        }
    }
    for b in builds.iter_mut() {
        if let Some(c) = b.circuit.as_mut() {
            c.rank = 1_000_000;
        }
    }
    for list in by_kind.values_mut() {
        list.sort_by_key(|&i| {
            let c = builds[i].circuit.as_ref();
            // (gates: the most common function first, critique)
            let value = c.and_then(site_model::html::sort_value).unwrap_or(i64::MAX);
            (c.map(|c| c.order).unwrap_or(0), value, builds[i].blocks, builds[i].volume.unwrap_or(u32::MAX), builds[i].name.clone())
        });
        for (k, &i) in list.iter().enumerate() {
            if let Some(c) = builds[i].circuit.as_mut() {
                c.rank = k as u32;
            }
        }
    }
    // the dominated-design audit, at every build (a study re-archived: new candidates show here)
    if !keep_hidden {
        let probe = site_model::Site { builds: builds.clone(), ..Default::default() };
        notes.extend(crate::audit::notes(&probe));
    }
    Ok((builds, notes))
}

/// `designs/circuits/catalog.tsv` (`mctas circuit catalog`): per design (stem) its tunable group
/// and the shown design it is a variant or an equivalent layout of (stem).
fn read_catalog(root: &Path) -> BTreeMap<String, (String, String)> {
    let text = std::fs::read_to_string(root.join("catalog.tsv")).unwrap_or_default();
    let mut lines = text.lines();
    let Some(head) = lines.next() else { return BTreeMap::new() };
    let cols: Vec<&str> = head.split('\t').collect();
    let col = |n: &str| cols.iter().position(|c| *c == n);
    let (Some(ni), vi, gi) = (col("name"), col("variant_of"), col("group")) else { return BTreeMap::new() };
    let stem = |s: &str| s.rsplit('/').next().unwrap_or(s).trim_end_matches(".txt").to_string();
    lines
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let get = |i: Option<usize>| i.and_then(|i| f.get(i)).map(|s| s.trim()).unwrap_or("");
            let name = f.get(ni)?.trim();
            Some((stem(name), (get(gi).to_string(), if get(vi).is_empty() { String::new() } else { stem(get(vi)) })))
        })
        .collect()
}

/// The same STRUCTURE archived under two kinds is one build listed twice (the copper bulb T
/// flip-flop is `tff-bulb` and `counter2-bulb`; the two-bulb chain is `counter4-bulb2` and, read
/// as a divider, `counter2-bulb2`). Each one gets `same_build` (the others' ids: the page says so
/// instead of letting a reader think they are two machines), and within ONE tunable group only the
/// member whose value the structure really counts to stays a version, so a picker never offers two
/// versions for the same value (critique C27).
fn fold_same_builds(root: &Path, builds: &mut [Build]) {
    // the structure: every cell with its block state, plus the port cells (the same blocks with
    // other ports is another machine), plus what the blocks hold (the item counters are one set
    // of blocks with other items in the dropper: other machines, not twins)
    let key_of = |b: &Build| -> Option<String> {
        b.circuit.as_ref()?;
        let path = root.join(repo_path_rel(b));
        let d = Design::load(&path).ok()?;
        let blocks: Vec<String> = d.clock.design.blocks.iter().map(|(p, s)| format!("{p:?}={s}")).collect();
        let ports: Vec<String> = d.inputs.iter().map(|i| format!("i{:?}", i.pos)).chain(d.outputs.iter().map(|o| format!("o{:?}", o.pos))).collect();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let contents: String = text.lines().filter(|l| !l.trim_start().starts_with('#')).filter(|l| l.contains('{')).map(|l| l.split_once('{').map(|(_, r)| r).unwrap_or("")).collect::<Vec<_>>().join(";");
        Some(format!("{}|{}|{contents}", blocks.join(";"), ports.join(";")))
    };
    let mut same: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        if let Some(k) = key_of(b) {
            same.entry(k).or_default().push(i);
        }
    }
    for (_k, twins) in same {
        if twins.len() < 2 {
            continue;
        }
        let facts: Vec<(String, String)> = twins
            .iter()
            .map(|&j| {
                let c = builds[j].circuit.as_ref();
                (c.map(|c| c.kind.clone()).unwrap_or_default(), c.map(|c| format!("circuits/{}/{}", c.study, builds[j].name)).unwrap_or_default())
            })
            .collect();
        for (n, &i) in twins.iter().enumerate() {
            // the same structure under the SAME kind is a duplicate for `archive --dedupe`, not a
            // second machine: only another kind is worth saying
            let others: Vec<String> = facts.iter().enumerate().filter(|(m, (k, _))| *m != n && *k != facts[n].0).map(|(_, (_, id))| id.clone()).collect();
            if let Some(c) = builds[i].circuit.as_mut() {
                c.same_build = others;
            }
        }
    }
}

/// The catalog's folds on the archived circuits: equivalent layouts (same kind, value, block count
/// and block kinds as a shown design: `layout_of`, hidden under it), variants (`host`: the
/// dominance fold), and tunable groups (`tunable`; the group's smallest design shows with its
/// span, the others are hidden until a value search hits them).
fn apply_catalog(root: &Path, builds: &mut [Build], notes: &mut Vec<String>) {
    let cat = read_catalog(root);
    if cat.is_empty() {
        notes.push("no designs/circuits/catalog.tsv: no tunable groups, the site's own variant fold".into());
        return;
    }
    let device = |b: &Build| -> &'static str {
        if b.flags.iter().any(|f| f == "lever") {
            "lever"
        } else if b.flags.iter().any(|f| f == "button") {
            "button"
        } else {
            "signal"
        }
    };
    let by_name: BTreeMap<String, (u32, Vec<String>, String, &'static str)> = builds.iter().filter_map(|b| b.circuit.as_ref().map(|c| (b.name.clone(), (b.blocks, b.kinds.clone(), format!("circuits/{}/{}", c.study, b.name), device(b))))).collect();
    for b in builds.iter_mut() {
        let Some((group, of)) = cat.get(&b.name) else { continue };
        let (blocks, kinds, dev) = (b.blocks, b.kinds.clone(), device(b));
        let Some(c) = b.circuit.as_mut() else { continue };
        c.tunable = group.clone();
        // the catalog's fold replaces the site's own
        c.host = None;
        c.layout_of = None;
        b.flags.retain(|f| f != "variant" && f != "layout");
        if of.is_empty() {
            continue;
        }
        match by_name.get(of) {
            // a design with its own lever never hides one driven by a signal, nor the other way
            // round (critique V18: gate-or-2b, the instant OR, was folded under gate-or-lever-1b)
            Some((_, _, _, hdev)) if *hdev != dev => {
                notes.push(format!("{}: not folded under {of} (another input device: {dev} vs {hdev})", b.name));
            }
            Some((hb, hk, id, _)) if *hb == blocks && *hk == kinds => {
                c.layout_of = Some(id.clone());
                b.flags.push("layout".into());
            }
            Some((_, _, id, _)) => {
                c.host = Some(id.clone());
                b.flags.push("variant".into());
            }
            // (its host hidden on purpose, crates/site/hidden.tsv: the design stands on its own,
            // the coordinator 2026-10-22: the 38-block columns door held until the reviewer's G3,
            // the obvious door it folded under the card)
            None if hidden_design(&c.study, of, "").is_some() => {}
            None => notes.push(format!("{}: variant_of {of} is not an archived circuit", b.name)),
        }
    }
    // the site's own layout fold where the catalog names no host (the coordinator 19:15Z: the N = 2
    // slime-free extenders sf7-1, sf7-2 and sf7-3 read as three identical cards): designs of the
    // same study, kind and value with the same blocks, block kinds and input device are other
    // layouts of one card, the smallest box kept (vjeux via pext 11:55Z: the smallest volume), then
    // the first by name
    let mut order: Vec<usize> = (0..builds.len()).filter(|&i| cat.get(&builds[i].name).is_some_and(|(_, of)| of.is_empty())).collect();
    order.sort_by(|&a, &b| (builds[a].volume.unwrap_or(u32::MAX), &builds[a].name).cmp(&(builds[b].volume.unwrap_or(u32::MAX), &builds[b].name)));
    let mut kept: BTreeMap<(String, String, String, u32, Vec<String>, &'static str), String> = BTreeMap::new();
    let mut layouts = 0;
    for i in order {
        let b = &builds[i];
        let Some(c) = &b.circuit else { continue };
        if c.host.is_some() || c.layout_of.is_some() {
            continue;
        }
        let key = (c.study.clone(), c.kind.clone(), c.axis.clone(), b.blocks, b.kinds.clone(), device(b));
        match kept.get(&key) {
            Some(id) => {
                let id = id.clone();
                let b = &mut builds[i];
                if let Some(c) = b.circuit.as_mut() {
                    c.layout_of = Some(id);
                }
                b.flags.push("layout".into());
                layouts += 1;
            }
            None => {
                kept.insert(key, format!("circuits/{}/{}", c.study, b.name));
            }
        }
    }
    if layouts > 0 {
        notes.push(format!("circuits: {layouts} designs folded as other layouts of a twin the catalog left unfolded (same kind, value, blocks, block kinds, input device)"));
    }
    // the coordinator's folds (hidden.tsv `~` lines): a design published only as a variant of
    // another (rs-crafter under rs-dropper: bigger and slower, nothing in its favour)
    for (study, file, host) in fold_lines() {
        let host_id = format!("circuits/{study}/{host}");
        if !builds.iter().any(|b| b.id == host_id) {
            notes.push(format!("hidden.tsv: ~{study}/{file} is folded under {host}, which is not published"));
            continue;
        }
        match builds.iter_mut().find(|b| b.circuit.as_ref().is_some_and(|c| c.study == study) && b.name == file) {
            Some(b) => {
                if let Some(c) = b.circuit.as_mut() {
                    c.host = Some(host_id);
                    c.layout_of = None;
                }
                if !b.flags.iter().any(|f| f == "variant") {
                    b.flags.push("variant".into());
                }
            }
            None => notes.push(format!("hidden.tsv: ~{study}/{file} matches no archived design (take it out)")),
        }
    }
    // one build with other items preloaded (the item counters: counter17-items ... counter576-
    // items, the critique's "NINE separate cards"): one tunable group, unless the catalog groups
    // them already
    let mut same: BTreeMap<(String, String, String), Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        if let Some(c) = &b.circuit {
            if c.tunable.is_empty() && !c.layout_key.is_empty() && c.host.is_none() && c.layout_of.is_none() {
                same.entry((c.study.clone(), c.kind.split_whitespace().next().unwrap_or("").to_string(), c.layout_key.clone())).or_default().push(i);
            }
        }
    }
    for members in same.values().filter(|m| m.len() > 1) {
        let id = format!("same-build:{}", builds[members[0]].name);
        notes.push(format!("{}: {} designs are one build with other contents, one group", builds[members[0]].name, members.len()));
        for &i in members {
            if let Some(c) = builds[i].circuit.as_mut() {
                c.tunable = id.clone();
                c.group_differs = "the items it holds (the count it reacts on)".into();
            }
        }
    }
    // tunable groups: the smallest shown design stands for the group
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        if let Some(c) = &b.circuit {
            if !c.tunable.is_empty() && c.host.is_none() && c.layout_of.is_none() {
                groups.entry(c.tunable.clone()).or_default().push(i);
            }
        }
    }
    for (_group, members) in groups {
        // two members that are the SAME build (one structure, two kinds) are one version: keep the
        // one whose value the structure counts to, so the picker never offers two chips for one
        // value (critique C27: `counter2-bulb2` is `counter4-bulb2` read as a divider)
        let id_of = |i: usize| builds[i].circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, builds[i].name)).unwrap_or_default();
        let value_of = |i: usize| builds[i].circuit.as_ref().and_then(|c| c.value).unwrap_or(i64::MIN);
        let dropped: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| {
                // (a shulker machine timed with other items is the same build on purpose: every
                // version stays)
                if builds[i].circuit.as_ref().is_some_and(|c| c.study == "shulker") {
                    return false;
                }
                let twins = builds[i].circuit.as_ref().map(|c| c.same_build.clone()).unwrap_or_default();
                members.iter().any(|&j| j != i && twins.contains(&id_of(j)) && (value_of(j), builds[j].name.clone()) > (value_of(i), builds[i].name.clone()))
            })
            .collect();
        // a dropped twin is not a card of its own either (critique: counter2-bulb2 showed as a
        // counter-2 card): it hides under the twin that stays, its page says it is the same build
        let keeper: Vec<(usize, String)> = dropped
            .iter()
            .filter_map(|&i| {
                let twins = builds[i].circuit.as_ref().map(|c| c.same_build.clone()).unwrap_or_default();
                members.iter().copied().filter(|&j| j != i && twins.contains(&id_of(j)) && !dropped.contains(&j)).max_by_key(|&j| value_of(j)).map(|j| (i, id_of(j)))
            })
            .collect();
        for (i, keep) in keeper {
            if let Some(c) = builds[i].circuit.as_mut() {
                c.host = Some(keep);
            }
            builds[i].flags.push("variant".into());
        }
        for &i in &dropped {
            if let Some(c) = builds[i].circuit.as_mut() {
                c.tunable.clear();
            }
        }
        let members: Vec<usize> = members.into_iter().filter(|i| !dropped.contains(i)).collect();
        if members.len() < 2 {
            if let Some(c) = builds[members[0]].circuit.as_mut() {
                c.tunable.clear();
            }
            continue;
        }
        let rep = *members.iter().min_by_key(|&&i| (builds[i].blocks, builds[i].circuit.as_ref().and_then(|c| c.value).unwrap_or(i64::MAX), builds[i].name.clone())).unwrap_or(&members[0]);
        let mut values: Vec<i64> = members.iter().filter_map(|&i| builds[i].circuit.as_ref().and_then(|c| c.value)).collect();
        values.sort();
        values.dedup();
        let axis = builds[rep].circuit.as_ref().map(|c| c.axis.clone()).unwrap_or_default();
        // `L = 12 gt retriggerable` -> (`L`, ` gt`); `off L = 12 gt` -> (`L`, ` gt`): the letter
        // before ` = ` (study 11 puts `on` / `off` in front) and the unit after the number
        let (prefix, unit) = match axis.split_once(" = ") {
            Some((p, v)) => (
                p.split_whitespace().last().unwrap_or("").to_string(),
                if v.split_whitespace().nth(1) == Some("gt") { " gt" } else { "" },
            ),
            None => (String::new(), ""),
        };
        let span = match (values.first(), values.last()) {
            (Some(lo), Some(hi)) if values.len() > 1 => format!("{prefix} {lo}-{hi}{unit}, {} values", values.len()).trim().to_string(),
            _ => format!("{} versions", members.len()),
        };
        // the group's title and note: the first member's `group title:` / `group note:` headers
        let (mut title, mut note) = (String::new(), String::new());
        for &i in &members {
            let text = std::fs::read_to_string(root.join(repo_path_rel(&builds[i]))).unwrap_or_default();
            for l in text.lines() {
                if let Some(t) = l.strip_prefix("group title:").filter(|_| title.is_empty()) {
                    title = t.trim().to_string();
                }
                if let Some(t) = l.strip_prefix("group note:").filter(|_| note.is_empty()) {
                    note = t.trim().to_string();
                }
            }
        }
        for &i in &members {
            if let Some(c) = builds[i].circuit.as_mut() {
                // without a `group title:` header: the kind for people (`Piston extender (reach 2)`)
                c.group_title = if title.is_empty() { crate::explain::kind_title(&c.kind) } else { title.clone() };
                c.group_note = note.clone();
            }
        }
        // what the members differ in, from their designs (vjeux: say WHAT differs); the same
        // blocks with other contents differ in what is loaded (critique: the item counter said
        // "only in their layout")
        let feats: Vec<VariantFeatures> = members.iter().map(|&i| Design::load(&root.join(repo_path_rel(&builds[i]))).map(|d| variant_features(&d)).unwrap_or_default()).collect();
        let (differs, labels) = variant_differences(&feats);
        let one_layout = members.iter().all(|&i| builds[i].circuit.as_ref().is_some_and(|c| !c.layout_key.is_empty() && Some(&c.layout_key) == builds[members[0]].circuit.as_ref().map(|x| &x.layout_key)));
        let shulker = members.iter().all(|&i| builds[i].circuit.as_ref().is_some_and(|c| c.study == "shulker"));
        let differs = if one_layout && shulker {
            "the items it is timed with (how many stack in one slot)".to_string()
        } else if one_layout {
            "how many items are loaded in it (the count it reacts on)".to_string()
        } else {
            differs
        };
        for (k, &i) in members.iter().enumerate() {
            if let Some(c) = builds[i].circuit.as_mut() {
                c.group_differs = differs.clone();
                c.variant_label = labels[k].clone();
            }
        }
        // the group's sizes (critique: "every 2nd to 32nd pulse · 3 blocks" while the 32nd needs
        // 15 blocks): its card says the range of blocks when they differ
        let lo_b = members.iter().map(|&i| builds[i].blocks).min().unwrap_or(0);
        let hi_b = members.iter().map(|&i| builds[i].blocks).max().unwrap_or(0);
        if let Some(c) = builds[rep].circuit.as_mut() {
            c.span = span;
            c.span_blocks = if hi_b > lo_b { format!("{lo_b} to {hi_b} blocks") } else { String::new() };
        }
        builds[rep].flags.push("tunable".into());
    }
}

/// A circuit build's design path relative to designs/circuits.
fn repo_path_rel(b: &Build) -> String {
    repo_path(b).trim_start_matches(&format!("{ROOT}/")).to_string()
}

/// A verdict's type key without the blocks' contents (`dropper[facing=west]{Items:[...]}` ->
/// `dropper[facing=west]`).
fn strip_contents(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut depth = 0u32;
    for ch in key.chars() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// What can differ between the members of a tunable group, read from one member's design.
#[derive(Clone, Debug, Default, PartialEq)]
struct VariantFeatures {
    /// How the inputs are driven: `lever`, `stone button`, `oak button`, `redstone block`.
    device: String,
    /// Every repeater's delay, sorted.
    repeaters: Vec<u32>,
    /// Comparators in subtract mode, of how many.
    subtract: usize,
    comparators: usize,
    blocks: usize,
}

fn variant_features(c: &Design) -> VariantFeatures {
    let blocks = &c.clock.design.blocks;
    let word = |s: &str| s.split(['[', '{']).next().unwrap_or(s).trim_start_matches("minecraft:").replace('_', " ");
    let device = c
        .inputs
        .iter()
        .map(|i| match &i.how {
            circuit::InputHow::Lever => "lever".to_string(),
            circuit::InputHow::Button => blocks.get(&i.pos).map(|s| word(s)).unwrap_or_else(|| "button".into()),
            circuit::InputHow::Strength { .. } => "strength".to_string(),
            circuit::InputHow::Frame { .. } => "item frame".to_string(),
            circuit::InputHow::Block { on, .. } => word(on),
            circuit::InputHow::Driven { .. } => "repeater".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut repeaters: Vec<u32> = blocks
        .values()
        .filter(|s| s.starts_with("minecraft:repeater"))
        .map(|s| s.split("delay=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|d| d.parse().ok()).unwrap_or(1))
        .collect();
    repeaters.sort();
    let comparators: Vec<&String> = blocks.values().filter(|s| s.starts_with("minecraft:comparator")).collect();
    VariantFeatures { device, repeaters, subtract: comparators.iter().filter(|s| s.contains("mode=subtract")).count(), comparators: comparators.len(), blocks: c.blocks() }
}

/// What a group's members differ in, and each member's label for it (in `feats` order).
fn variant_differences(feats: &[VariantFeatures]) -> (String, Vec<String>) {
    let varies = |f: &dyn Fn(&VariantFeatures) -> String| feats.iter().map(f).collect::<BTreeSet<_>>().len() > 1;
    let device = varies(&|x| x.device.clone());
    let rep_count = varies(&|x| x.repeaters.len().to_string());
    // the delays differ beyond the count: two members with as many repeaters, other delays
    let rep_delays = feats.iter().enumerate().any(|(i, a)| feats[i + 1..].iter().any(|b| a.repeaters.len() == b.repeaters.len() && a.repeaters != b.repeaters)) || (!rep_count && varies(&|x| format!("{:?}", x.repeaters)));
    let cmp_modes = feats.iter().enumerate().any(|(i, a)| feats[i + 1..].iter().any(|b| a.comparators == b.comparators && a.subtract != b.subtract));
    let cmp_count = varies(&|x| x.comparators.to_string());
    let length = varies(&|x| x.blocks.to_string());
    let mut aspects: Vec<&str> = Vec::new();
    if device {
        aspects.push("the input device");
    }
    if rep_delays {
        aspects.push("the repeater delays");
    }
    if rep_count {
        aspects.push("the length of the repeater line");
    }
    if cmp_modes {
        aspects.push("the comparator modes");
    }
    if cmp_count {
        aspects.push("the number of stages");
    }
    if aspects.is_empty() && length {
        // (not "the length": next to timer lengths it read as the timer's, critique C49)
        aspects.push("their size in blocks");
    }
    if aspects.is_empty() {
        aspects.push("their layout");
    }
    let text = match aspects.len() {
        1 => aspects[0].to_string(),
        n => format!("{} and {}", aspects[..n - 1].join(", "), aspects[n - 1]),
    };
    let labels = feats
        .iter()
        .map(|x| {
            let mut parts: Vec<String> = Vec::new();
            if device {
                parts.push(x.device.clone());
            }
            if rep_count || rep_delays {
                parts.push(if x.repeaters.is_empty() { "no repeater".to_string() } else { format!("delays {}", x.repeaters.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("+")) });
            }
            if cmp_modes {
                parts.push(if x.subtract == 0 { "compare".to_string() } else { format!("{} subtract", x.subtract) });
            }
            if cmp_count {
                parts.push(format!("{} stage{}", x.comparators, if x.comparators == 1 { "" } else { "s" }));
            }
            if parts.is_empty() {
                parts.push(format!("{} blocks", x.blocks));
            }
            parts.join(", ")
        })
        .collect();
    (text, labels)
}

/// How the extendable circuit families are expanded at build time.
#[derive(Clone, Debug)]
pub struct FamilyOptions {
    /// Where the generated members go (`<site>/designs/circuits/members`); `None`: the formula only.
    pub members_dir: Option<PathBuf>,
    pub threads: usize,
}

/// The extendable circuit families (`extend` headers on archived designs, docs/circuits.md): one
/// card per family group (`extend group:`), every member of the verified n range generated with
/// the family's own generator and screened in redsim (`judged` when `extend.tsv` has it, `redsim`
/// when the screen passes with the formula's axis value, else `formula`), for the family pages and
/// the circuit value search.
pub fn families(repo: &Path, builds: &[Build], clock_cards: &[site_model::Card], o: &FamilyOptions) -> (Vec<site_model::Card>, Vec<site_model::Member>, Vec<String>) {
    let root = repo.join(ROOT);
    let mut notes = Vec::new();
    struct Fam {
        build: usize,
        path: PathBuf,
        f: clock::extend::Family,
        letter: String,
        unit: &'static str,
        judged: Vec<i64>,
    }
    let mut fams: Vec<Fam> = Vec::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(c) = &b.circuit else { continue };
        if c.study.is_empty() {
            continue;
        }
        let path = repo.join(repo_path(b));
        let Ok(design) = Design::load(&path) else { continue };
        let f = match circuit::archive::family_of(&design) {
            Ok(Some(f)) => f,
            Ok(None) => continue,
            Err(e) => {
                notes.push(format!("{}: extend headers: {e:#}", b.name));
                continue;
            }
        };
        let (letter, unit) = match c.axis.split_once(" = ") {
            Some((l, v)) => (l.trim().to_string(), if v.trim().ends_with(" gt") { " gt" } else { "" }),
            None if c.kind.starts_with("counter") || c.kind.starts_with("piston-extender") => ("N".to_string(), ""),
            // (an item elevator's family: its height, `4 + 2n blocks up`; members `6 blocks up`)
            None if c.axis.ends_with(" blocks up") => (String::new(), " blocks up"),
            None => (String::new(), if c.axis.contains("gt") { " gt" } else { "" }),
        };
        let judged: Vec<i64> = std::fs::read_to_string(root.join(&c.study).join("archive").join(circuit::archive::EXTEND_TSV))
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter_map(|l| {
                let x: Vec<&str> = l.split('\t').collect();
                (x.first() == Some(&f.id.as_str())).then(|| x.get(1).and_then(|n| n.parse().ok())).flatten()
            })
            .collect();
        fams.push(Fam { build: i, path, f, letter, unit, judged });
    }
    // the cards: one per group (else per family), ids clear of the clock cards'
    let mut cards: Vec<site_model::Card> = Vec::new();
    let mut card_of: Vec<usize> = Vec::new();
    for fam in &fams {
        let key = fam.f.group.clone().unwrap_or_else(|| fam.f.id.clone());
        let id = if clock_cards.iter().any(|c| c.id == key) { format!("circuit-{key}") } else { key.clone() };
        let ci = match cards.iter().position(|c| c.id == id) {
            Some(ci) => ci,
            None => {
                let b = &builds[fam.build];
                let heading = match &fam.f.group_period {
                    Some(p) if p.contains('=') => p.clone(),
                    Some(p) => format!("{} = {p}{}", fam.letter, fam.unit),
                    None => format!("{} = {}{}", fam.letter, fam.f.period.text(), fam.unit),
                };
                cards.push(site_model::Card {
                    id: id.clone(),
                    title: fam.f.group_title.clone().unwrap_or_else(|| format!("{} ({})", key, crate::explain::kind_title(&b.group).to_lowercase())),
                    heading: heading.trim_start_matches(" = ").to_string(),
                    page: format!("families/{id}.html"),
                    note: fam.f.group_note.clone().unwrap_or_default(),
                    kind: "circuit".into(),
                    study: b.circuit.as_ref().map(|c| c.study.clone()).unwrap_or_default(),
                    ..Default::default()
                });
                cards.len() - 1
            }
        };
        let b = &builds[fam.build];
        cards[ci].families.push(site_model::Family {
            id: fam.f.id.clone(),
            build: b.id.clone(),
            period: crate::clocks::formula(&fam.f.period),
            period_text: fam.f.period.text(),
            blocks: crate::clocks::formula(&fam.f.blocks),
            blocks_text: fam.f.blocks.text(),
            rule: fam.f.rule.clone(),
            n_lo: fam.f.n_lo,
            n_hi: fam.f.n_hi,
            base: fam.f.base,
            judged: fam.judged.clone(),
            extra: Vec::new(),
        });
        card_of.push(ci);
    }
    // the members: every n of each family, generated in parallel
    let jobs: Vec<(usize, i64)> = fams.iter().enumerate().flat_map(|(k, fam)| (fam.f.n_lo..=fam.f.n_hi).map(move |n| (k, n))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<site_model::Member>>> = jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let job_notes: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let threads = o.threads.max(1).min(jobs.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let (k, n) = jobs[i];
                let fam = &fams[k];
                let b = &builds[fam.build];
                let value = fam.f.period.eval(n);
                let mut m = site_model::Member {
                    run: None,
                    card: cards[card_of[k]].id.clone(),
                    family: fam.f.id.clone(),
                    n,
                    period: value.max(0) as u32,
                    blocks: fam.f.blocks.eval(n).max(0) as u32,
                    design: String::new(),
                    dims: None,
                    volume: None,
                    status: if fam.judged.contains(&n) { site_model::STATUS_JUDGED.into() } else { site_model::STATUS_FORMULA.into() },
                    build: None,
                    picture: None,
                    axis: format!("{} = {value}{}", fam.letter, fam.unit).trim_start_matches(" = ").to_string(),
                    base: String::new(),
                };
                if n == fam.f.base {
                    m.design = b.design.clone();
                    m.dims = b.dims.clone();
                    m.volume = b.volume;
                    m.build = Some(b.id.clone());
                    m.status = site_model::STATUS_JUDGED.into();
                } else if let Some(dir) = &o.members_dir {
                    let tmp = std::env::temp_dir().join(format!("mctas-site-cf-{}-{k}", std::process::id()));
                    match fam.f.build(&fam.path, n, &tmp) {
                        Ok(p) => {
                            let name = format!("{}-n{n}.txt", fam.f.id);
                            if std::fs::create_dir_all(dir).and_then(|_| std::fs::copy(&p, dir.join(&name)).map(|_| ())).is_ok() {
                                m.design = format!("designs/circuits/members/{name}");
                                if let Some((c, parsed)) = parse(&p) {
                                    if let Some((d, v)) = parsed.dims {
                                        m.dims = Some(d);
                                        m.volume = Some(v);
                                    }
                                    // the redsim screen: the formula's axis value, exactly
                                    let ok = circuit::verify::sim(&c, &circuit::verify::Options::default()).ok().and_then(|r| r.outcome).filter(|o| o.pass).is_some_and(|o| {
                                        let kind = c.kind.clone();
                                        kind.is_some_and(|k| number(&circuit::archive::axis(&k, &o, &parsed.kinds.join("+"))) == Some(value))
                                    });
                                    if ok && m.status != site_model::STATUS_JUDGED {
                                        m.status = site_model::STATUS_REDSIM.into();
                                    } else if !ok {
                                        job_notes.lock().unwrap().push(format!("{} n = {n}: the redsim screen does not give {} (kept as formula)", fam.f.id, m.axis));
                                    }
                                }
                            }
                            let _ = std::fs::remove_file(&p);
                        }
                        Err(e) => job_notes.lock().unwrap().push(format!("{} n = {n}: {e:#}", fam.f.id)),
                    }
                }
                *slots[i].lock().unwrap() = Some(m);
            });
        }
    });
    notes.extend(job_notes.into_inner().unwrap_or_default());
    let members: Vec<site_model::Member> = slots.into_iter().filter_map(|s| s.into_inner().ok().flatten()).collect();
    (cards, members, notes)
}

/// The repository path of a circuit build's design (for the GitHub link and the page's text).
pub fn repo_path(b: &Build) -> String {
    // designs/circuits/<study>/<file> on the site, <study>/archive/<file> in the repository; a
    // whole line, designs/circuits/<study>/system/<file>, the same path in both
    let rest = b.design.trim_start_matches("designs/circuits/");
    match rest.split_once('/') {
        Some((study, f)) if f.starts_with("system/") => format!("{ROOT}/{study}/{f}"),
        Some((study, f)) => format!("{ROOT}/{study}/archive/{f}"),
        None => b.design.clone(),
    }
}

/// The studies' whole lines (`designs/circuits/<study>/system/*.txt`: sorter's crate::cartsys, a
/// loader, the track and an unloader in one design, no circuit kind; sorter 2026-10-22): one build
/// each, the site's kind `cart-line`, measured by the line's own two runs on redsim (sorter: game =
/// redsim): a double chest carried end to end, then the trickle (a farm's slow output, then
/// nothing: the cart must end parked at the loader). A line whose run fails is not published, and
/// the notes say so.
fn systems(root: &Path, notes: &mut Vec<String>, keep_hidden: bool) -> Vec<Build> {
    let mut out = Vec::new();
    for (study, _) in studies() {
        let Ok(dir) = std::fs::read_dir(root.join(study).join("system")) else { continue };
        let mut files: Vec<PathBuf> = dir.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        files.sort();
        for path in files {
            let file = path.file_name().and_then(|f| f.to_str()).unwrap_or("").to_string();
            if !keep_hidden && hidden_design(study, &file, "").is_some() {
                continue;
            }
            match system_build(study, &path) {
                Ok(b) => out.push(b),
                Err(e) => notes.push(format!("{}: not published ({e:#})", path.display())),
            }
        }
    }
    out
}

/// One whole line's build ([`systems`]).
fn system_build(study: &str, path: &Path) -> Result<Build> {
    use circuit::cartsys;
    let text = std::fs::read_to_string(path)?;
    let design = mctas::design::Design::parse(&text)?;
    let clock = clock::Clock::load(path)?;
    let log = match cartsys::sim(&clock, false)? {
        Ok(l) => l,
        Err(why) => anyhow::bail!("its run: {why}"),
    };
    let (ok, verdict, table) = cartsys::judge(&clock, &log);
    if !ok {
        anyhow::bail!("its run: {verdict}");
    }
    let minutes = |gt: u32| gt as f64 / 1200.0;
    // (an iron count made of rails' sixteenths is not a whole number: "~65 iron", the
    // coordinator's card)
    let count = |v: f64, item: &str| if (v - v.round()).abs() > 1e-9 { format!("~{:.0} {item}", v) } else { format!("{v:.0} {item}") };
    let (iron, gold, redstone, quartz) = cartsys::bill(&design);
    let mut metrics: Vec<(String, String)> = vec![
        ("minutes for a double chest".into(), format!("{:.1} min", minutes(log.end))),
        ("items/s".into(), format!("{:.2}", log.supplied as f64 * 20.0 / log.end.max(1) as f64)),
        ("trips".into(), log.trips.div_ceil(2).to_string()),
    ];
    if let Some(q) = table.iter().find_map(|l| l.strip_prefix("by the quarter: ")) {
        metrics.push(("by the quarter".into(), q.to_string()));
    }
    metrics.push(("bill".into(), format!("{}, {}, {}, {quartz} quartz (track and cart included)", count(iron, "iron"), count(gold, "gold"), count(redstone, "redstone"))));
    // the trickle: every item delivered and the cart parked at the loader, or why not
    if let Ok(Ok(t)) = cartsys::sim(&clock, true) {
        let (ok, why, _) = cartsys::judge(&clock, &t);
        let last = t.flow.last().map(|x| x.0).unwrap_or(t.end);
        metrics.push((
            "trickle".into(),
            if ok && t.parked.is_some() {
                format!("{} items fed {} at a time every {} s: all delivered by {:.1} min, then the cart parked at the loader", t.supplied, cartsys::TRICKLE, cartsys::SEG / 20, minutes(last))
            } else {
                format!("failed: {why}")
            },
        ));
    }
    // what the header of `mctas circuit cart-line` says: the track ("128 blocks of track with a
    // powered rail every 8") and the stations it was made of (their archive paths)
    let comments: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix('#')).collect();
    let word_before = |key: &str| comments.iter().find_map(|l| l.split_once(key).and_then(|(a, _)| a.split_whitespace().last()?.parse::<i64>().ok()));
    let word_after = |key: &str| comments.iter().find_map(|l| l.split_once(key).and_then(|(_, b)| b.split(|c: char| !c.is_ascii_digit()).next()?.parse::<i64>().ok()));
    let track = word_before(" blocks of track");
    if let Some(n) = track {
        let every = word_after("a powered rail every ").map(|k| format!(", a powered rail every {k}")).unwrap_or_default();
        metrics.push(("track".into(), format!("{n} blocks{every}")));
    }
    let mut parts: Vec<String> = Vec::new();
    for l in &comments {
        for (i, _) in l.match_indices("designs/circuits/") {
            let rest = &l[i..];
            let Some(end) = rest.find(".txt") else { continue };
            let segs: Vec<&str> = rest[..end].split('/').collect();
            if let [_, _, s, "archive", name] = segs.as_slice() {
                parts.push(format!("circuits/{s}/{name}"));
            }
        }
    }
    let name = path.file_stem().and_then(|f| f.to_str()).unwrap_or("").to_string();
    let file = format!("{name}.txt");
    let (lo, hi) = design.bounds().ok_or_else(|| anyhow::anyhow!("no blocks"))?;
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let kinds: Vec<String> = design.blocks.values().map(|s| s.split('[').next().unwrap_or(s).trim_start_matches("minecraft:").to_string()).collect::<BTreeSet<_>>().into_iter().collect();
    // the view: the line as built with its judge's double chests (mcview's `cart-line`, shown as
    // built: the viewer does not run its cart yet); the downloads stay the design
    let mut view = design.clone();
    view.meta.insert("circuit".into(), "cart-line".into());
    Ok(Build {
        id: format!("circuits/{study}/{name}"),
        kind: "circuit".into(),
        group: site_model::html::kind_words("cart-line"),
        name: name.clone(),
        page: format!("circuits/{study}/{name}.html"),
        design: format!("designs/circuits/{study}/system/{file}"),
        blocks: design.blocks.len() as u32,
        volume: Some((size[0] * size[1] * size[2]) as u32),
        dims: Some(format!("{}x{}x{}", size[0], size[1], size[2])),
        kinds,
        circuit: Some(Circuit {
            study: study.to_string(),
            order: studies().iter().position(|s| s.0 == study).unwrap_or(99) as u32 * 100,
            kind: "cart-line".into(),
            axis: track.map(|n| format!("{n} blocks of track")).unwrap_or_default(),
            value: track,
            metrics,
            verdict,
            parts,
            view_src: format!("designs/stills/{study}/{file}"),
            view_text: view.to_text(),
            ..Default::default()
        }),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_differences_name_what_varies() {
        let f = |device: &str, reps: &[u32], blocks: usize| VariantFeatures { device: device.into(), repeaters: reps.to_vec(), blocks, ..Default::default() };
        let (t, l) = variant_differences(&[f("lever", &[], 7), f("stone button", &[], 7), f("oak button", &[], 7)]);
        assert_eq!(t, "the input device");
        assert_eq!(l, vec!["lever", "stone button", "oak button"]);
        let (t, l) = variant_differences(&[f("redstone block", &[1, 4], 17), f("redstone block", &[4, 4], 17), f("redstone block", &[4, 4, 4], 21)]);
        assert_eq!(t, "the repeater delays and the length of the repeater line");
        assert_eq!(l[2], "delays 4+4+4");
        let (t, _) = variant_differences(&[f("redstone block", &[2], 5), f("redstone block", &[4], 5)]);
        assert_eq!(t, "the repeater delays");
        // a chain's versions differ in how many stages they have, not in the comparator modes
        let g = |comparators: usize, subtract: usize, blocks: usize| VariantFeatures { device: "redstone block".into(), comparators, subtract, blocks, ..Default::default() };
        let (t, l) = variant_differences(&[g(1, 0, 3), g(2, 0, 6), g(3, 0, 9)]);
        assert_eq!(t, "the number of stages");
        assert_eq!(l, vec!["1 stage", "2 stages", "3 stages"]);
        // as many comparators, other modes: that IS the comparator modes
        let (t, _) = variant_differences(&[g(2, 0, 6), g(2, 1, 6)]);
        assert_eq!(t, "the comparator modes");
    }

    /// The farm judge's speed in a verdict (w86 2026-10-23: `game rts N`, before the bill): parsed
    /// whole (split at its first space it read ("game", "rts 4")), the bill after it intact, and
    /// read for the page view's `data-farm-rts`.
    #[test]
    fn a_farm_verdict_carries_its_judge_speed() {
        let v = "items per hour per block 4.414, items per hour 11000.9, harvest height 7, items smelted per hour per block 1.104, items smelted per hour 2750.2, game rts 4, bill 4372 iron, 4 quartz, 12 redstone, 92 slime";
        let m = parse_metrics(v);
        assert!(m.contains(&("game rts".to_string(), "4".to_string())), "{m:?}");
        assert!(m.contains(&("items smelted per hour".to_string(), "2750.2".to_string())), "{m:?}");
        assert_eq!(m.last(), Some(&("bill".to_string(), "4372 iron, 4 quartz, 12 redstone, 92 slime".to_string())));
        assert_eq!(recorded_game_rts(v), Some(4));
        assert_eq!(recorded_game_rts("items per hour 646.0, bill 1836 iron"), None);
        assert!(!crate::explain::metric_definition("harvester sugar_cane", "game rts").is_empty());
    }

    #[test]
    fn hidden_designs_match_their_patterns() {
        assert!(glob("pext-*-sf*", "pext-n5-sf-sweep136.txt"));
        assert!(!glob("pext-*-sf*", "pext-n2-sand6.txt"));
        assert!(hidden("pext", "pext-n1-sticky.txt").is_some());
        assert!(hidden("pulse", "pext-n1-sticky.txt").is_none());
        // a line names one design: the others of its kind stay
        assert!(hidden("pext", "pext-n3-sf43.txt").is_some());
        assert!(hidden("pext", "pext-n3-sf38.txt").is_none());
        assert!(glob("a*b*c", "aXbYc") && !glob("a*b*c", "aXbY"));
        // a class rule: every vibration-sensitive sculk design, in any study
        assert!(hidden_design("pulse", "pulse-rise-sculk-L30.txt", "L = 30 gt, vibration-sensitive").is_some());
        assert!(hidden_design("pulse", "pulse-rise-sculk-filter-L10.txt", "L = 10 gt").is_none());
        assert!(hidden("pulse", "pulse-rise-sculk-L30.txt").is_none(), "a class rule needs the class");
    }

    #[test]
    fn verdict_metrics() {
        let m = parse_verdict_metrics("CIRCUIT: sub-rising-4 rising-edge L 2 gt, delay 1 gt, min-pulse 1 gt, 5 blocks, 3x2x1, robust", "rising-edge");
        assert_eq!(m, vec![("L".into(), "2 gt".into()), ("delay".into(), "1 gt".into()), ("min-pulse".into(), "1 gt".into())]);
        assert_eq!(parse_metrics("L 2 gt, delay 1 gt"), m[..2].to_vec());
        // an adder slice's two-part metric (compute::slice_check): its law is no second latency
        let a = parse_metrics("latency 18 gt, glitches 0, carry carry in and out, slice 54 blocks per bit, latency 18 + 12 (N-1) gt");
        assert_eq!(a.iter().filter(|(k, _)| k == "latency").count(), 1);
        assert_eq!(a.last(), Some(&("slice".to_string(), "54 blocks per bit, latency 18 + 12 (N-1) gt".to_string())));
        let s = parse_metrics("boxes/min 0.087, first box 13847 gt, hoppers awake 3, badges 16-stack+drop-free, bill 16 iron, 3 quartz, 1 slime");
        assert_eq!(s[1], ("first box".to_string(), "13847 gt".to_string()));
        assert_eq!(s[2], ("hoppers awake".to_string(), "3".to_string()));
        assert_eq!(s[4], ("bill".to_string(), "16 iron, 3 quartz, 1 slime".to_string()));
        assert_eq!(parse_metrics("rate 200 TNT/min, period 6 gt, per period 1")[2], ("per period".to_string(), "1".to_string()));
        // a mob farm's weather lines (mobinfra): `real game` and `thunder loss` are one name each,
        // and their values keep their own `;`-joined parts
        let f = parse_metrics("iron per hour 300, real game 8 builds in the 26.3 server on Hard: 3 days of natural weather 74.8 golems/h (redsim 75.0; 95% 72.4-77.6); a forced day of rain 75.1 golems/h (redsim 74.0; 99% 70.0-78.0); 4.06 iron per dead golem, thunder loss 3.2, rain clear day 300 iron/h; rain day 298 iron/h; thunder day 297 iron/h");
        assert_eq!(f.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), ["iron per hour", "real game", "thunder loss", "rain"]);
        // an auto-smelter's names keep their words (they read `items/min` = `per furnace 6.0`)
        let sm = parse_metrics("items/min per furnace 6.0, first item by 220 gt, items per coal 6.0, spread 18 each, hoppers awake 3, at rest yes, blocks per furnace 4.0, minutes for a double chest 72.2, of the ceiling 100% (72.2 min: 8 furnaces always busy), first ingot by 312 gt, coal left 64 (0 in the machine)");
        let get = |k: &str| sm.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
        assert_eq!(get("items/min per furnace"), "6.0");
        assert_eq!(get("items per coal"), "6.0");
        assert_eq!(get("blocks per furnace"), "4.0");
        assert_eq!(get("minutes for a double chest"), "72.2");
        assert_eq!(get("of the ceiling"), "100% (72.2 min: 8 furnaces always busy)");
        assert_eq!(get("first ingot"), "by 312 gt");
        assert_eq!(get("coal left"), "64 (0 in the machine)");
        assert_eq!(parse_metrics("speed 10 gt/block, period 10 gt, 1 cell(s), latency 2 gt")[1], ("period".to_string(), "10 gt, 1 cell(s)".to_string()));
        let g = parse_verdict_metrics("CIRCUIT: cal-torch-not gate not fn not, latency 2 gt, glitches 0, 3 blocks, 4x2x1, robust", "gate not");
        assert_eq!(g[0], ("fn".into(), "not".into()));
        assert_eq!(g.len(), 3);
        assert!(parse_verdict_metrics("FAIL: x delay: no", "delay").is_empty());
    }

    #[test]
    fn numbers_and_captions() {
        assert_eq!(number("L = 4 gt"), Some(4));
        assert_eq!(number("xor"), None);
        assert_eq!(comment_caption("# archived 2026\nname: x\n# Input -> dust -> torch.\n"), "Input -> dust -> torch.");
        assert_eq!(flags_of(&[("latency".into(), "0 gt".into())], "robust", ""), vec!["robust", "instant"]);
    }
}
