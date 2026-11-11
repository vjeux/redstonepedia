//! farmopt's approach lab tables (2026-11-10): the pocket stand's fault table and its tolerance
//! corners, each a batch of `queue::run_loop` runs read back from their fixed-form lines. The
//! fault table runs ironspec's list of stand faults at slot 5 (REL, D1 and C missed or extra,
//! extra REL pulses and glitches, whole and partial unloads at the chunk lines), optionally with
//! the perfect tail (a queue-only reset at every slot from 6: whatever the stand sends comes back
//! as the queue's next trains of 3), and prints per fault: the first slot from which every slot
//! starts with a clean 3 (one cart at rest in each pocket, nothing else in the stand's strip), the
//! carts back through a gate, the departures' and the tail entry's smallest gaps and groups.

use crate::queue::{run_loop, LoopOpts, LoopTiming};
use redsim::entity::mob::farm::FarmSpec;

/// One run's lines, read back.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    /// The first slot from which every later slot start holds a clean 3 (None: never).
    pub clean_from: Option<i64>,
    pub back: usize,
    pub dep_min: Option<i64>,
    pub dep_contact: usize,
    pub tail_min: Option<i64>,
    pub tail_touch: usize,
    pub tail_group_max: usize,
    /// The most carts out of the stand in one release window (the bar is 3).
    pub window_max: usize,
    pub problems: usize,
    /// gap12, G2 in (after cart 2's front), G2 before cart 3: each (min, max).
    pub margins: Option<[i64; 6]>,
    pub latch: Option<(i64, i64)>,
}

fn num_after(l: &str, key: &str) -> Option<i64> {
    l.split(key).nth(1)?.split(|c: char| !(c.is_ascii_digit() || c == '-')).find(|t| !t.is_empty())?.parse().ok()
}

/// Reads a `run_loop` output (with `verbose`).
pub fn summarize(out: &str) -> Summary {
    let mut s = Summary::default();
    let mut starts: Vec<(i64, bool)> = Vec::new();
    for l in out.lines() {
        if let Some(r) = l.strip_prefix("slot ")
            && l.contains(" start: pockets S1 ")
        {
            let slot: i64 = r.split(' ').next().and_then(|t| t.parse().ok()).unwrap_or(-1);
            starts.push((slot, l.contains("pockets S1 1 S2 1 S3 1, 0 more")));
        } else if let Some(r) = l.strip_prefix("back through a gate: ") {
            s.back = r.split(' ').next().and_then(|t| t.parse().ok()).unwrap_or(0);
        } else if l.starts_with("stand departures: ") {
            s.dep_min = num_after(l, "gap min ");
            s.dep_contact = l.split("gt (gaps under 60 gt), ").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse().ok()).unwrap_or(0);
            s.window_max = num_after(l, "most in one window ").map_or(0, |v| v.max(0) as usize);
        } else if l.starts_with("tail entry: ") {
            s.tail_min = num_after(l, "gap min ");
            s.tail_touch = l.split("within groups, ").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse().ok()).unwrap_or(0);
            if let Some(g) = l.split("; groups ").nth(1).and_then(|t| t.split(';').next()) {
                s.tail_group_max = g.split(' ').filter_map(|t| t.split('x').next()?.parse::<usize>().ok()).max().unwrap_or(0);
            }
        } else if l.starts_with("stand margins: ") {
            let v: Vec<i64> = l.split_whitespace().filter_map(|w| w.parse().ok()).collect();
            if v.len() == 9 && v[8] > 0 {
                s.margins = Some([v[0], v[1], v[2], v[3], v[6], v[7]]);
            }
        } else if let Some(r) = l.strip_prefix("latch set (T2 out after +55): +") {
            let a: Option<i64> = r.split("..").next().and_then(|t| t.parse().ok());
            let b: Option<i64> = r.split("..+").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse().ok());
            s.latch = a.zip(b);
        } else if let Some(n) = l.strip_suffix(" problems").or_else(|| l.strip_suffix(" problems:")) {
            s.problems = n.parse().unwrap_or(0);
        }
    }
    // (the first slot from which every later start is clean)
    let mut from = None;
    for (slot, clean) in starts.iter().rev() {
        if *clean {
            from = Some(*slot);
        } else {
            break;
        }
    }
    s.clean_from = from;
    s
}

/// ironspec's stand faults at slot 5, as `--act` strings in the design's frame (the loop's cells
/// moved by its shift; chunk boxes as given, meaningful at `--shift 6,6`).
pub fn fault_cases(o: &LoopOpts, partial: bool) -> Vec<(String, Vec<String>)> {
    let (sx, sz) = (o.shift[0], o.shift[1]);
    let c = |x: i32, y: i32, z: i32| format!("{},{},{}", x + sx, y, z + sz);
    let ox = -o.row_gap + 3;
    let rel = c(ox + 8, -2, 2);
    let mut v: Vec<(String, Vec<String>)> = vec![("normal".into(), vec![])];
    v.push(("REL missed".into(), vec![format!("5,5,{rel},glass,60")]));
    for h in [45, 65] {
        v.push((format!("D1 missed {h}"), vec![format!("5,55,{},glass,{h}", c(ox + 6, -1, 1))]));
    }
    for t in [50, 66, 72] {
        v.push((format!("D1 extra +{t}"), vec![format!("5,{t},{},detector_rail[powered=true,shape=east_west],4", c(ox + 6, -1, 0))]));
    }
    v.push(("C missed".into(), vec![format!("5,0,{},glass,100", c(ox + 5, -2, 2))]));
    for t in [50, 66, 72] {
        v.push((format!("C extra +{t}"), vec![format!("5,{t},{},redstone_block,4", c(ox + 5, -2, 2))]));
    }
    for t in [66, 79, 85, 92] {
        v.push((format!("REL extra 40 +{t}"), vec![format!("5,{t},{rel},redstone_block,40")]));
    }
    for t in [66, 79, 92] {
        v.push((format!("REL glitch 2 +{t}"), vec![format!("5,{t},{rel},redstone_block,2")]));
    }
    for n in [40, 240] {
        for t in [66, 79, 92, 12, 30, 48] {
            v.push((format!("unload all {n} +{t}"), vec![format!("5,{t},unload,{n}")]));
        }
    }
    if partial {
        for b in ["0,0,0,0", "0,0,0,1", "0,1,0,1"] {
            for n in [40, 240] {
                for t in [66, 79, 92, 12, 30, 48] {
                    v.push((format!("chunks {b} {n} +{t}"), vec![format!("5,{t},unload,{b},{n}")]));
                }
            }
        }
    }
    v
}

fn runs(spec: &FarmSpec, o: &LoopOpts, jobs: Vec<(usize, LoopTiming, i64)>, cycles: i64, threads: usize) -> Result<Vec<(usize, Summary)>, String> {
    let mut got: Vec<(usize, Summary)> = Vec::new();
    for chunk in jobs.chunks(threads.max(1)) {
        let part: Vec<Result<(usize, Summary), String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk.iter().map(|(i, tm, seed)| sc.spawn(move || run_loop(spec, o, tm, cycles, *seed, true, None, None).map(|r| (*i, summarize(&r))))).collect();
            hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a bench thread panicked".into()))).collect()
        });
        for r in part {
            got.push(r?);
        }
    }
    Ok(got)
}

fn opt(v: Option<i64>) -> String {
    v.map_or("-".to_string(), |x| x.to_string())
}

/// The fault table: every case of [`fault_cases`] (or those whose name contains `filter`) on each
/// seed, `tm`'s own acts kept (C' emulated with T3 removed, say); `perfect`: the queue reset at
/// every slot from 6.
#[allow(clippy::too_many_arguments)]
pub fn fault_table(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, seeds: &[i64], cycles: i64, threads: usize, perfect: bool, filter: Option<&str>, all_corners: bool) -> Result<String, String> {
    let partial = o.shift != [0, 0];
    let cases0: Vec<(String, Vec<String>)> = fault_cases(o, partial).into_iter().filter(|(n, _)| filter.is_none_or(|f| f.split('|').any(|p| n.contains(p)))).collect();
    // (`all_corners`: every case at REL nominal, early and late x staggers 2, 3, 4, 6)
    let mut variants: Vec<(String, LoopTiming)> = vec![(String::new(), *tm)];
    if all_corners {
        variants.clear();
        for (cn, on, for_) in [("REL 10..50", 0, 0), ("REL 0..44", -10, -6), ("REL 16..76", 6, 26)] {
            for st in [2, 3, 4, 6] {
                let mut t = *tm;
                t.rel_on += on;
                t.rel_for += for_;
                t.release_stagger = st;
                variants.push((format!("{cn} st {st}: "), t));
            }
        }
    }
    let mut cases: Vec<(String, Vec<String>, LoopTiming)> = Vec::new();
    for (vn, vt) in &variants {
        for (n, a) in &cases0 {
            cases.push((format!("{vn}{n}"), a.clone(), *vt));
        }
    }
    let base: Vec<_> = tm.acts.iter().flatten().copied().collect();
    let mut jobs = Vec::new();
    for (i, (_, acts, vt)) in cases.iter().enumerate() {
        let mut t = *vt;
        let mut a = [None; 8];
        for (k, x) in base.iter().enumerate() {
            a[k] = Some(*x);
        }
        for (k, s) in acts.iter().enumerate() {
            a[base.len() + k] = Some(crate::queue::parse_act(s)?);
        }
        t.acts = a;
        if perfect {
            t.reset_queue_at = Some(6);
            t.reset_queue_every = true;
        }
        for s in seeds {
            jobs.push((i, t, *s));
        }
    }
    let got = runs(spec, o, jobs, cycles, threads)?;
    let mut out = vec![format!("fault table: {} cases x seeds {:?}, {cycles} slots, the fault at slot 5{}; per case over the seeds: clean 3 from slot (the latest seed; never = -), carts back through a gate, departures' min gap (contacts), tail entry min gap within groups (touching), its biggest group, problems", cases.len(), seeds, if perfect { ", the perfect tail (queue reset every slot from 6)" } else { ", the bench's own tail" })];
    for (i, (name, _, _)) in cases.iter().enumerate() {
        let ss: Vec<&Summary> = got.iter().filter(|(k, _)| *k == i).map(|(_, s)| s).collect();
        let clean = if ss.iter().any(|s| s.clean_from.is_none()) { None } else { ss.iter().filter_map(|s| s.clean_from).max() };
        let back: usize = ss.iter().map(|s| s.back).sum();
        let dep = ss.iter().filter_map(|s| s.dep_min).min();
        let contact: usize = ss.iter().map(|s| s.dep_contact).sum();
        let tail = ss.iter().filter_map(|s| s.tail_min).min();
        let touch: usize = ss.iter().map(|s| s.tail_touch).sum();
        let group = ss.iter().map(|s| s.tail_group_max).max().unwrap_or(0);
        let window = ss.iter().map(|s| s.window_max).max().unwrap_or(0);
        let probs: Vec<String> = ss.iter().map(|s| s.problems.to_string()).collect();
        let flag = if clean.is_none() || back > 0 || tail.is_some_and(|g| g < 5) || group > 5 || window > 3 { " <--" } else { "" };
        out.push(format!("{name:24} clean from {:>2} | back {back} | dep min {} ({contact}) | tail min {} ({touch}) group {group} | out per window {window} | problems {}{flag}", opt(clean), opt(dep), opt(tail), probs.join("/")));
    }
    Ok(out.join("\n"))
}

/// The tolerance corners: REL nominal, early (rel-on -10, rel-for -6) and late (rel-on +6,
/// rel-for +26), each at release staggers 2, 3, 4 and 6, over the seeds: clean runs, the stand's
/// margins, the latch's set times, the departures' and tail entry's gaps.
pub fn corner_table(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, seeds: &[i64], cycles: i64, threads: usize) -> Result<String, String> {
    let corners = [("REL 10..50", 0, 0), ("REL 0..44", -10, -6), ("REL 16..76", 6, 26)];
    let staggers = [2, 3, 4, 6];
    let mut jobs = Vec::new();
    let mut names = Vec::new();
    for (ci, (_, on, for_)) in corners.iter().enumerate() {
        for (si, st) in staggers.iter().enumerate() {
            let mut t = *tm;
            t.rel_on += on;
            t.rel_for += for_;
            t.release_stagger = *st;
            let i = ci * staggers.len() + si;
            names.push(format!("{} stagger {st}", corners[ci].0));
            for s in seeds {
                jobs.push((i, t, *s));
            }
        }
    }
    let got = runs(spec, o, jobs, cycles, threads)?;
    let mut out = vec![format!("corner table: seeds {:?}, {cycles} slots each: clean runs; cart 1 to 2 at D2; G2 shut after cart 2's front was in it; G2 shut before cart 3; latch set; departures' and tail entry's min gaps", seeds)];
    for (i, n) in names.iter().enumerate() {
        let ss: Vec<&Summary> = got.iter().filter(|(k, _)| *k == i).map(|(_, s)| s).collect();
        let clean = ss.iter().filter(|s| s.problems == 0).count();
        let m = |j: usize, lo: bool| { let v = ss.iter().filter_map(|s| s.margins.map(|m| m[j])); if lo { v.min() } else { v.max() } };
        let la = ss.iter().filter_map(|s| s.latch.map(|l| l.0)).min();
        let lb = ss.iter().filter_map(|s| s.latch.map(|l| l.1)).max();
        out.push(format!("{n:22} clean {clean}/{} | D2 {}..{} | G2 in {}..{} | before cart 3 {}..{} | latch +{}..+{} | dep min {} | tail min {}", ss.len(), opt(m(0, true)), opt(m(1, false)), opt(m(2, true)), opt(m(3, false)), opt(m(4, true)), opt(m(5, false)), opt(la), opt(lb), opt(ss.iter().filter_map(|s| s.dep_min).min()), opt(ss.iter().filter_map(|s| s.tail_min).min())));
    }
    Ok(out.join("\n"))
}
