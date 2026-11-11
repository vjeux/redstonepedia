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
    /// Departure gaps of 4..6 gt at the take-off stand-in (R1's ledge band).
    pub takeoff_bad: usize,
    pub problems: usize,
    /// gap12, G2 in (after cart 2's front), G2 before cart 3: each (min, max).
    pub margins: Option<[i64; 6]>,
    pub latch: Option<(i64, i64)>,
    pub merges: usize,
    /// Latch sets before +55 (out of turn) and slots with more than one set.
    pub latch_early: usize,
    pub latch_doubles: usize,
    /// Carts off the rails (the flight watch's derails) and declared-state mismatches.
    pub derails: usize,
    pub declared_bad: usize,
    /// The departure corner (9,-3,0), carts 0..2 of each slot, from G0's opening: front in
    /// (min, max), tail out (min, max).
    pub divert: Option<[[i64; 4]; 3]>,
    /// Summons onto the pad out of turn (with the pad pasted) and the pad's golems.
    pub summons_oot: usize,
    pub pad_golems: usize,
    /// Carts at rest on the approach's flat top T (5 ticks or more), by cause (a, b, c, contact,
    /// other), and the reversals on the climb.
    pub t_rests: usize,
    pub t_causes: [usize; 5],
    pub climb_rev: usize,
    /// Meetings (a pair coming into contact) on the climb and on T, and the slowest met cart's
    /// speed at each (ironspec's crawl rule).
    pub meet_n: [usize; 3],
    pub meet_min: [Option<f64>; 3],
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
        } else if l.starts_with("departures at the take-off ") {
            s.takeoff_bad = num_after(l, "4..6 gt: ").map_or(0, |v| v.max(0) as usize);
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
        } else if l.starts_with("merged carts") {
            s.merges = l.split("): ").nth(1).and_then(|t| t.split_whitespace().next()).and_then(|t| t.parse().ok()).unwrap_or(0);
        } else if l.starts_with("latch sets: ") {
            s.latch_early = num_after(l, "out of turn (REL off, before +55) ").unwrap_or(0) as usize;
            s.latch_doubles = num_after(l, "more than one with REL off in a slot ").unwrap_or(0) as usize;
        } else if let Some(r) = l.strip_prefix("carts off the rails: ") {
            s.derails = r.split(|c: char| !c.is_ascii_digit()).next().and_then(|t| t.parse().ok()).unwrap_or(0);
        } else if l.starts_with("declared states") {
            s.declared_bad = l.split("): ").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse().ok()).unwrap_or(0);
        } else if let Some(r) = l.strip_prefix("carts at rest on T: ") {
            let v: Vec<usize> = r.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).take(6).filter_map(|t| t.parse().ok()).collect();
            s.t_rests = v.first().copied().unwrap_or(0);
            for i in 0..5 {
                s.t_causes[i] = v.get(i + 1).copied().unwrap_or(0);
            }
        } else if let Some(r) = l.strip_prefix("meetings on the climb: ") {
            let (cl, rest) = r.split_once("; on T: ").unwrap_or((r, ""));
            let (t, de) = rest.split_once("; below T on the descent: ").unwrap_or((rest, ""));
            for (i, part) in [cl, t, de].iter().enumerate() {
                s.meet_n[i] = part.split(|c: char| !c.is_ascii_digit()).next().and_then(|t| t.parse().ok()).unwrap_or(0);
                s.meet_min[i] = part.split_once("slowest met ").and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse().ok());
            }
        } else if let Some(r) = l.strip_prefix("climb reversals: ") {
            s.climb_rev = r.split(|c: char| !c.is_ascii_digit()).next().and_then(|t| t.parse().ok()).unwrap_or(0);
        } else if let Some(r) = l.strip_prefix("summons onto the pad out of turn") {
            // ("(...): N of M")
            if let Some(t) = r.split("): ").nth(1) {
                let mut w = t.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty());
                s.summons_oot = w.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                s.pad_golems = w.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            }
        } else if l.starts_with("divert corner (departures of 3") {
            // ("cart k in +A out +B (from G0's opening: in E out F)", each a value or a..b)
            let rng = |t: &str| -> Option<(i64, i64)> {
                let t = t.trim();
                match t.split_once("..") {
                    Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
                    None => t.parse().ok().map(|v| (v, v)),
                }
            };
            let mut d = [[0i64; 4]; 3];
            let mut ok = 0;
            for (k, part) in l.split("(from G0's opening: in ").skip(1).enumerate().take(3) {
                let (a, rest) = part.split_once(" out ").unwrap_or((part, ""));
                let b = rest.split(')').next().unwrap_or("");
                if let (Some(i), Some(o)) = (rng(a), rng(b)) {
                    d[k] = [i.0, i.1, o.0, o.1];
                    ok += 1;
                }
            }
            if ok == 3 {
                s.divert = Some(d);
            }
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
    // (whole unloads under the reload-order rule: the save order, its reverse and 8 seeded orders)
    for n in [40, 240] {
        for t in [66, 79, 92, 12, 30, 48] {
            for ord in ["save", "reverse", "seed:1", "seed:2", "seed:3", "seed:4", "seed:5", "seed:6", "seed:7", "seed:8"] {
                v.push((format!("unload all {n} +{t} {ord}"), vec![format!("5,{t},unload,{n},{ord}")]));
            }
        }
    }
    // (the flat top T (ox - 3, 6, 4), the approach's one rest cell: the cart in the queue's head
    // slot set at rest on T at slot 5 +95, before slot 6's release, at five places along T; the
    // next release group, one short, must push it through the stand in its place)
    if o.approach == 5 {
        for zt in [4.02, 4.25, 4.5, 4.75, 4.98] {
            v.push((format!("T rest z {zt}"), vec![format!("5,95,place,head,{},6.5,{zt}", ox as f64 - 2.5)]));
        }
        // (the same cart stranded with the next train whole: pocket 3's cart set on T after the
        // batch is at rest, slot 5 +95: REL lets two go, the full train behind pushes it in)
        for zt in [4.02, 4.5, 4.98] {
            v.push((format!("T rest (S3) z {zt}"), vec![format!("5,95,place,s3,{},6.5,{zt}", ox as f64 - 2.5)]));
        }
    }
    // (C' wired: its detector D missed for the slot (a plain rail in its place), an extra edge (D
    // powered 4 gt with no cart) before, inside and after the train's passage, and D's edge during a
    // freeze of its chunk, the stand's (stage 32 alone; stage 33 inside a 3 x 3 box))
    if o.cprime.is_some() {
        let d = c(ox - 3, 3, 7);
        v.push(("C' D missed".into(), vec![format!("5,0,{d},rail[shape=ascending_north],100")]));
        for t in [0, 20, 50, 66, 90] {
            v.push((format!("C' D extra +{t}"), vec![format!("5,{t},{d},detector_rail[powered=true,shape=ascending_north],4")]));
        }
        if partial {
            for (b, stage) in [("0,0,0,0", 32), ("-1,-1,1,1", 33)] {
                for n in [40, 240] {
                    for t in [30, 36, 40] {
                        v.push((format!("C' D edge, chunk stage {stage} {n} +{t}"), vec![format!("5,{t},unload,{b},{n}")]));
                    }
                }
            }
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
pub fn fault_table(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, seeds: &[i64], cycles: i64, threads: usize, perfect: bool, filter: Option<&str>, all_corners: bool, rel_sweep: Option<(i64, i64)>) -> Result<String, String> {
    let partial = o.shift != [0, 0];
    let cases0: Vec<(String, Vec<String>)> = fault_cases(o, partial).into_iter().filter(|(n, _)| filter.is_none_or(|f| f.split('|').any(|p| n.contains(p)))).collect();
    // (`all_corners`: every case at REL nominal, early and late x staggers 2, 3, 4, 6; `rel_sweep`
    // A..B: every case at the REL phases A..=B gt around the timing's own (each gt), the stagger
    // and the rest as given)
    let mut variants: Vec<(String, LoopTiming)> = vec![(String::new(), *tm)];
    if let Some((a, b)) = rel_sweep {
        variants.clear();
        for on in a..=b {
            let mut t = *tm;
            t.rel_on += on;
            variants.push((format!("REL {:+} st {}: ", on, tm.release_stagger), t));
        }
    } else if all_corners {
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
        let merges: usize = ss.iter().map(|s| s.merges).sum();
        let early: usize = ss.iter().map(|s| s.latch_early).sum();
        let derails: usize = ss.iter().map(|s| s.derails).sum();
        let dbad: usize = ss.iter().map(|s| s.declared_bad).sum();
        let oot: usize = ss.iter().map(|s| s.summons_oot).sum();
        let pg: usize = ss.iter().map(|s| s.pad_golems).sum();
        let pad = if pg > 0 || oot > 0 { format!(" | pad summons out of turn {oot} of {pg}") } else { String::new() };
        let tr: usize = ss.iter().map(|s| s.t_rests).sum();
        let tc: Vec<usize> = (0..5).map(|i| ss.iter().map(|s| s.t_causes[i]).sum()).collect();
        let rv: usize = ss.iter().map(|s| s.climb_rev).sum();
        let mn: Vec<usize> = (0..3).map(|i| ss.iter().map(|s| s.meet_n[i]).sum()).collect();
        let mm: Vec<String> = (0..3).map(|i| ss.iter().filter_map(|s| s.meet_min[i]).fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.min(v)))).map_or("-".into(), |v| format!("{v:.3}"))).collect();
        let pad = if o.approach > 0 { format!("{pad} | T rests {tr} (a {} b {} c {} k {} o {}) | climb rev {rv} | met climb {} (slowest {}) T {} (slowest {}) descent {} (slowest {})", tc[0], tc[1], tc[2], tc[3], tc[4], mn[0], mm[0], mn[1], mm[1], mn[2], mm[2]) } else { pad };
        let flag = if clean.is_none() || back > 0 || tail.is_some_and(|g| g < 5) || group > 5 || window > 3 || merges > 0 || derails > 0 || dbad > 0 || oot > 0 { " <--" } else { "" };
        // (seeds breaking a hard line: back-crossings, merges, derails, a departure under 5 gt, more
        // than 3 out in one window)
        let hard = ss.iter().filter(|s| s.back > 0 || s.merges > 0 || s.derails > 0 || s.dep_min.is_some_and(|g| g < 5) || s.window_max > 3).count();
        let tko: usize = ss.iter().map(|s| s.takeoff_bad).sum();
        out.push(format!("{name:24} clean from {:>2} | hard {hard}/{} | back {back} | merges {merges} | latch early {early} | dep min {} ({contact}) | take-off 4..6 gt {tko} | tail min {} ({touch}) group {group} | out per window {window} | derails {derails} | declared {dbad}{pad} | problems {}{flag}", opt(clean), ss.len(), opt(dep), opt(tail), probs.join("/")));
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
        let merges: usize = ss.iter().map(|s| s.merges).sum();
        let early: usize = ss.iter().map(|s| s.latch_early).sum();
        let doubles: usize = ss.iter().map(|s| s.latch_doubles).sum();
        // (the divert corner against day-1's +4-shifted windows, golem +21+10k ..= +30+10k with G0
        // opening at golem +13: from G0's opening +8+10k ..= +17+10k; each cart's earliest front and
        // latest tail, and the margins)
        let dv: Vec<[[i64; 4]; 3]> = ss.iter().filter_map(|s| s.divert).collect();
        let divert = if dv.is_empty() {
            "divert -".to_string()
        } else {
            let mut parts = Vec::new();
            let mut worst = i64::MAX;
            for k in 0..3 {
                let fi = dv.iter().map(|d| d[k][0]).min().unwrap();
                let fa = dv.iter().map(|d| d[k][1]).max().unwrap();
                let ti = dv.iter().map(|d| d[k][2]).min().unwrap();
                let ta = dv.iter().map(|d| d[k][3]).max().unwrap();
                let (m1, m2) = (fi - (8 + 10 * k as i64), (17 + 10 * k as i64) - ta);
                worst = worst.min(m1).min(m2);
                parts.push(format!("c{k} in {fi}..{fa} out {ti}..{ta} ({m1}/{m2})"));
            }
            format!("divert {} min {worst}", parts.join(" "))
        };
        out.push(format!("{n:22} clean {clean}/{} | D2 {}..{} | G2 in {}..{} | before cart 3 {}..{} | latch +{}..+{} (early {early}, doubled {doubles}) | merges {merges} | dep min {} | tail min {} | {divert}", ss.len(), opt(m(0, true)), opt(m(1, false)), opt(m(2, true)), opt(m(3, false)), opt(m(4, true)), opt(m(5, false)), opt(la), opt(lb), opt(ss.iter().filter_map(|s| s.dep_min).min()), opt(ss.iter().filter_map(|s| s.tail_min).min())));
    }
    Ok(out.join("\n"))
}

/// C', the train-synchronous D1 fallback, wired (`queue-loop --cprime`, with `--approach 5` and
/// `--stand-guard`; the loop frame with G2 at x `ox`), routed clear of day-1's controller (its line
/// along z 7 at y -1/-2 from x 0 to 8, and its line along x 5 at y -6/-7 across the stand chunk):
/// the approach climb's detector rail D (ox - 3, 3, 7) strongly powers its support S0 (made stone);
/// R1 (-2,2,7) reads S0 into the dust U0 (-1,2,7); a spiral staircase in the column x -2..-1, z 5..7
/// takes it down to y -4 (six steps north, west, north, east, south, west; every step's support a
/// stone: a dust reads the one a step above it only through a conductor under that one, and the
/// cell above each lower dust stays air); then east along z 7 at y -4 through M1..M4 to N
/// (6,-4,6) (y -4: two above day-1's x 5 line, so nothing of this links to it); the comparator K
/// (6,-4,5, subtract) reads N at its rear and N delayed by RS1 (4,-4,6) and RS2 (5,-4,5, 3 rt) at
/// its west side: one pulse per rise of N, into the stone K_n (6,-4,4), which carries the wall
/// torch T_n (5,-4,4, C's Rq cell, facing west); T_n lights the dust (5,-4,3) into B3 except during
/// the pulse: T3 lights for the pulse and sets the latch through F as C did. C's timer (REL's tap
/// TP, S, the chains K1..K5, Dr, Xd, its supports), Rq's dust and C's K_n go. `delays`: [R1, M1,
/// M2, M3, M4, RS1] in redstone ticks (D's rise to T3 lit: 2 x (R1 + M1 + M2 + M3 + M4) + 6 gt).
/// Returns (cells removed, cells added), loop frame; a cell in both is replaced.
pub fn cprime_cells(ox: i32, delays: [i32; 6]) -> (Vec<[i32; 3]>, Vec<([i32; 3], String)>) {
    let at = |x: i32, y: i32, z: i32| [ox + x, y, z];
    let removed: Vec<[i32; 3]> = [
        (7, -3, 1), (7, -4, 1), (6, -3, 1), (6, -4, 1), (6, -5, 1), (6, -4, 2), (6, -5, 2), (7, -4, 2), (7, -5, 2), (7, -4, 3), (7, -5, 3), (7, -4, 4), (7, -5, 4),
        (7, -5, 1), (7, -6, 1), (8, -5, 1), (8, -5, 2), (8, -5, 3), (8, -5, 4), (8, -6, 2), (8, -6, 3), (8, -6, 4), (8, -5, 5), (8, -6, 5), (7, -5, 5), (7, -6, 5), (6, -5, 5), (6, -4, 5),
        // (C's K_n and its cap, Rq's input dust; replaced: S0, Rq, the dust's support)
        (7, -4, 5), (7, -3, 5), (5, -4, 5), (-3, 2, 7), (5, -4, 4), (5, -5, 5),
    ]
    .iter()
    .map(|&(x, y, z)| at(x, y, z))
    .collect();
    let wire = |n: &str, e: &str, s: &str, w: &str| format!("redstone_wire[east={e},north={n},power=0,south={s},west={w}]");
    let rep = |d: i32, f: &str| format!("repeater[delay={d},facing={f},locked=false,powered=false]");
    let mut add: Vec<([i32; 3], String)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, s: String| add.push((at(x, y, z), s));
    // S0 under D: stone (D strongly powers it); R1 on glass
    put(-3, 2, 7, "stone".into());
    put(-2, 2, 7, rep(delays[0], "west"));
    put(-2, 1, 7, "glass".into());
    // the spiral: U0, L1 .. L6, each on a stone (L6, the last, on glass)
    let spiral: [((i32, i32, i32), String, bool); 7] = [
        ((-1, 2, 7), wire("side", "none", "none", "side"), true),
        ((-1, 1, 6), wire("none", "none", "up", "side"), true),
        ((-2, 0, 6), wire("side", "up", "none", "none"), true),
        ((-2, -1, 5), wire("none", "side", "up", "none"), true),
        ((-1, -2, 5), wire("none", "none", "side", "up"), true),
        ((-1, -3, 6), wire("up", "none", "none", "side"), true),
        ((-2, -4, 6), wire("none", "up", "side", "none"), false),
    ];
    for ((x, y, z), w, stone) in spiral {
        put(x, y, z, w);
        put(x, y - 1, z, if stone { "stone".into() } else { "glass".into() });
    }
    // east along z 7 at y -4: two dusts, M1..M4, three dusts, then N north of the last
    put(-2, -4, 7, wire("side", "side", "none", "none"));
    put(-1, -4, 7, wire("none", "side", "none", "side"));
    for (i, x) in (0..=3).enumerate() {
        put(x, -4, 7, rep(delays[1 + i], "west"));
    }
    put(4, -4, 7, wire("side", "side", "none", "side"));
    put(5, -4, 7, wire("none", "side", "none", "side"));
    put(6, -4, 7, wire("side", "none", "none", "side"));
    for x in -2..=6 {
        put(x, -5, 7, "glass".into());
    }
    put(6, -4, 6, wire("side", "none", "side", "none"));
    put(6, -5, 6, "glass".into());
    // K (subtract, rear N south of it) into K_n; the side: RS1 north of the dust (4,-4,7), a dust,
    // RS2 into K's west side
    put(6, -4, 5, "comparator[facing=south,mode=subtract,powered=false]".into());
    put(6, -5, 5, "glass".into());
    put(4, -4, 6, rep(delays[5], "south"));
    put(4, -5, 6, "glass".into());
    put(4, -4, 5, wire("none", "side", "side", "none"));
    put(4, -5, 5, "glass".into());
    put(5, -4, 5, rep(3, "west"));
    put(5, -5, 5, "glass".into());
    // K_n and T_n (lit: the dust (5,-4,3) into B3 powered, T3 off)
    put(6, -4, 4, "stone".into());
    put(5, -4, 4, "redstone_wall_torch[facing=west,lit=true]".into());
    (removed, add)
}
