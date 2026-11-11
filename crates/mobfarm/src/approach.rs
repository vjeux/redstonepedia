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
    /// The departure crest's dip meetings (with the slowest met) and reversals onto a slope.
    pub d_meet: usize,
    pub d_meet_min: Option<f64>,
    pub d_rev: usize,
    pub d_rise: usize,
    pub d_rise_min: Option<f64>,
    /// Meetings by the met cart's place (the flat run below the climb, the climb, T, the descent).
    pub cls: [usize; 7],
    /// The most carts on the release run upstream of the line at once.
    pub upstream: usize,
    /// The largest backward travel on the approach and backward entries onto the run's boosters.
    pub back_travel: f64,
    pub back_boost: usize,
    pub back_vmax: f64,
    /// Meetings by cause (frozen, over T, head-on, stalled, catch-up, other) and when (in a
    /// freeze, within 200 gt of a thaw, other): by the met cart's place (flat, climb, T, descent),
    /// and for the "meetings on the climb / on T / below T" counts (`meet_n`).
    pub causes: [[usize; 6]; 7],
    pub whens: [[usize; 3]; 7],
    pub n_causes: [[usize; 6]; 3],
    pub n_whens: [[usize; 3]; 3],
    /// Contacts at a waiting cart by its place: count, the mover's highest stored speed the tick
    /// before, how many at 0.6 (H's bar) and at 1.0 or more, the closest centres, the highest onto
    /// an empty waiting cart, the merges.
    pub bc: [(usize, f64, usize, usize, f64, f64, usize); 7],
    /// Carts pushed back over T (backward crossings onto the climb) and the fastest's stored speed
    /// (v_T, the hump's bound).
    pub vt: (usize, f64),
    /// The approach's profile by plane and train place: (plane code, place 1..3, stored min, max,
    /// +gt min, max from the train's launch slot).
    pub prof: Vec<(String, usize, f64, f64, i64, i64)>,
    /// C''''s hump: the highest backward climb on its falling face above the dip, and backward carts
    /// onto its top or beyond.
    pub hump_back: (f64, usize),
    /// S3's meter: releases and trains metered in, the least gt from a Dh 3 edge to the next slot
    /// tick, openings passing 2 or more carts, the follower's least distance short of H's plane at
    /// H's shut.
    pub meter: Option<(usize, usize, Option<i64>, usize, Option<f64>)>,
    /// cc767b78's head model (`--head-perm` with day-1's latches): launches for A with carts
    /// stopped at H, the most carts stopped on the descent at once and the farthest back z.
    pub hp: Option<(usize, usize, f64)>,
    /// cc767b78's run-derived summon margins: the least margin and the fills under the bar.
    pub sm: Option<(Option<i64>, usize)>,
    /// The head model's recovery: A-cycles from the fault's slot to clean (None: never within
    /// the run), and the golems the run's A ticks made (each summon margin entry marked golem).
    pub hclean: Option<Option<i64>>,
    pub golems: Option<usize>,
}

/// "name a/b/c/d/e/f when x/y/z; ..." (the runner's cause lines), the first N parts.
fn cause_rows<const N: usize>(r: &str) -> ([[usize; 6]; N], [[usize; 3]; N]) {
    let (mut c, mut w) = ([[0usize; 6]; N], [[0usize; 3]; N]);
    for (i, part) in r.split("; ").take(N).enumerate() {
        let t: Vec<&str> = part.split_whitespace().collect();
        for (j, v) in t.get(1).map_or("", |s| *s).split('/').take(6).enumerate() {
            c[i][j] = v.parse().unwrap_or(0);
        }
        for (j, v) in t.get(3).map_or("", |s| *s).split('/').take(3).enumerate() {
            w[i][j] = v.parse().unwrap_or(0);
        }
    }
    (c, w)
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
        } else if let Some(r) = l.strip_prefix("backward travel on the approach: largest ") {
            s.back_travel = r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            s.back_boost = r.rsplit_once(": ").and_then(|(_, v)| v.trim().parse().ok()).unwrap_or(0);
            s.back_vmax = r.split_once("fastest backward: stored ").and_then(|(_, v)| v.split(',').next()).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
        } else if let Some(r) = l.strip_prefix("carts on the release run upstream of the line z 9|10 at once: at most ") {
            s.upstream = r.trim().parse().unwrap_or(0);
        } else if let Some(r) = l.strip_prefix("head-perm summon margins ") {
            let least = r.rsplit_once("least margin ").and_then(|(_, v)| v.split(" gt").next()).and_then(|v| v.trim().parse().ok());
            let under = r.rsplit_once("fills under the bar: ").and_then(|(_, v)| v.split(|c: char| !c.is_ascii_digit()).next()).and_then(|v| v.parse().ok()).unwrap_or(0);
            s.sm = Some((least, under));
            s.golems = Some(r.matches(", golem)").count());
        } else if let Some(r) = l.strip_prefix("head-perm cycles-to-clean ") {
            s.hclean = Some(r.rsplit_once("; clean from slot ").and_then(|(_, v)| v.split_once(" (")).and_then(|(_, v)| v.split(' ').next()).and_then(|v| v.parse().ok()));
        } else if let Some(r) = l.strip_prefix("head-perm day-1 latches ") {
            let n = r.split_once("launches for A with carts stopped at H: ").and_then(|(_, v)| v.split(|c: char| !c.is_ascii_digit()).next()).and_then(|v| v.parse().ok()).unwrap_or(0);
            let most = r.split_once("the most carts stopped on the descent at once ").and_then(|(_, v)| v.split(|c: char| !c.is_ascii_digit()).next()).and_then(|v| v.parse().ok()).unwrap_or(0);
            let z = r.split_once("the farthest back z ").and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            s.hp = Some((n, most, z));
        } else if let Some(r) = l.strip_prefix("meter (S3(ii)'s hold, emulated): ") {
            let rel = r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let tin = r.split_once("releases, ").and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse().ok()).unwrap_or(0);
            let m = s.meter.get_or_insert((0, 0, None, 0, None));
            (m.0, m.1) = (rel, tin);
        } else if let Some(r) = l.strip_prefix("meter Dh 3 (the stand's 3rd cart in): gt before its batch's golem summon tick (the last before the stand's next REL) ") {
            let m = s.meter.get_or_insert((0, 0, None, 0, None));
            m.2 = r.split("..").next().and_then(|v| v.trim().parse().ok());
        } else if let Some(r) = l.strip_prefix("meter H per opening: ") {
            let m = s.meter.get_or_insert((0, 0, None, 0, None));
            m.3 = r.rsplit_once("2+: ").and_then(|(_, v)| v.split(|c: char| !c.is_ascii_digit()).next()).and_then(|v| v.parse().ok()).unwrap_or(0);
            m.4 = r.split_once("short of H's plane when H shut ").and_then(|(_, v)| v.split("..").next()).and_then(|v| v.trim().parse().ok());
        } else if let Some(r) = l.strip_prefix("hump: highest backward climb on its falling face ") {
            s.hump_back = (r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0), r.rsplit_once(": ").and_then(|(_, v)| v.trim().parse().ok()).unwrap_or(0));
        } else if let Some(r) = l.strip_prefix("approach profile summary (") {
            if let Some((_, r)) = r.split_once("): ") {
                for e in r.split_whitespace() {
                    let f: Vec<&str> = e.split(':').collect();
                    if f.len() == 6 {
                        s.prof.push((f[0].to_string(), f[1].parse().unwrap_or(0), f[2].parse().unwrap_or(0.0), f[3].parse().unwrap_or(0.0), f[4].parse().unwrap_or(0), f[5].parse().unwrap_or(0)));
                    }
                }
            }
        } else if let Some(r) = l.strip_prefix("pushed back over T (backward crossings onto the climb): ") {
            s.vt = (r.split(';').next().and_then(|t| t.trim().parse().ok()).unwrap_or(0), r.split_once("stored ").and_then(|(_, v)| v.split(',').next()).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0));
        } else if let Some(r) = l.strip_prefix("waiting-cart contact summary (") {
            if let Some((_, r)) = r.split_once("): ") {
                for (i, part) in r.split("; ").take(7).enumerate() {
                    let v: Vec<&str> = part.split_whitespace().nth(1).unwrap_or("").split('/').collect();
                    if v.len() == 7 {
                        s.bc[i] = (v[0].parse().unwrap_or(0), v[1].parse().unwrap_or(0.0), v[2].parse().unwrap_or(0), v[3].parse().unwrap_or(0), v[4].parse().unwrap_or(9.0), v[5].parse().unwrap_or(0.0), v[6].parse().unwrap_or(0));
                    }
                }
            }
        } else if let Some(r) = l.strip_prefix("meetings by cause (") {
            if let Some((_, r)) = r.split_once("): ") {
                (s.causes, s.whens) = cause_rows::<7>(r);
            }
        } else if let Some(r) = l.strip_prefix("meetings on the climb, on T, below T by cause (") {
            if let Some((_, r)) = r.split_once("): ") {
                (s.n_causes, s.n_whens) = cause_rows::<3>(r);
            }
        } else if let Some(r) = l.strip_prefix("meetings by the met cart's place: ") {
            // (each count after its own key, the later ones after the previous field's close: a bare
            // "climb " first matches inside "flat run below the climb N", and the climb column then
            // repeated the flat count in every row (c4336062's 388 "climb" meetings, 2026-11-11))
            for (i, key) in ["flat run below the climb ", "), climb ", "), T ", "), descent ", "), hump-down ", "), hump-up ", "), run "].iter().enumerate() {
                s.cls[i] = r.split_once(key).and_then(|(_, v)| v.split(|c: char| !c.is_ascii_digit()).next()).and_then(|t| t.parse().ok()).unwrap_or(0);
            }
        } else if let Some(r) = l.strip_prefix("departure: meetings in the dip: ") {
            s.d_meet = r.split(|c: char| !c.is_ascii_digit()).next().and_then(|t| t.parse().ok()).unwrap_or(0);
            s.d_meet_min = r.split_once("slowest met ").and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.trim_end_matches(')').parse().ok());
            s.d_rev = r.split_once("reversals onto a slope: ").and_then(|(_, v)| v.split(|c: char| !c.is_ascii_digit()).next()).and_then(|t| t.parse().ok()).unwrap_or(0);
            if let Some((_, v)) = r.split_once("meetings on the rise: ") {
                s.d_rise = v.split(|c: char| !c.is_ascii_digit()).next().and_then(|t| t.parse().ok()).unwrap_or(0);
                s.d_rise_min = v.split_once("slowest met ").and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.trim_end_matches(')').parse().ok());
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
    // (the departure crest's plain dip: the queue head's cart set in the dip 30 gt after slot 5's
    // REL rises (before the departing batch reaches it), at rest or crawling 0.02, at the dip's
    // first three cells' centres; farmopt 2026-11-11, the allowed cells from push-in rows)
    if partial && o.approach > 0 && (o.approach_climb_boost || o.humped()) {
        // (the stand chunk frozen 240 gt from every gt of the train's arrival window, slot 5 +30
        // ..= +80: farmopt's (d) sweep on C with the head's arrival latch, widened from +40..+75 so
        // it spans the border crossings of every hump height, C's first cart at +34 to hump 5's
        // third at about +79)
        for t in 30..=80 {
            v.push((format!("stand freeze 240 +{t}"), vec![format!("5,{t},unload,0,0,0,0,240")]));
        }
        // (the hump's chunk (loop z 10..25: the boosters, the hump, the dip's north cell) frozen
        // 240 gt from every gt the train crosses the boosters, +3..=+30: a cart frozen on a booster
        // (h4's flat one among them) and the train behind it at the thaw)
        if o.humped() {
            for t in 3..=30 {
                v.push((format!("hump freeze 240 +{t}"), vec![format!("5,{t},unload,0,1,0,1,240")]));
            }
        }
        // (ironspec's h0-against-h5 rows: each chunk the approach crosses frozen 40, 200, 1,500 or
        // 5,000 gt from every gt a train is in it: the stand's (0,0) over the train's arrival window
        // +30..=+80, the run's (0,1) (the boosters, the hump or S's flat, the dip's north end) from
        // the launch +0..=+50; the family's name carries the length so a table can take one length
        // at its own --cycles)
        for n in [40, 200, 1500, 5000] {
            for (cn, b, t0, t1) in [("stand", "0,0,0,0", 30, 80), ("run", "0,1,0,1", 0, 50)] {
                for t in t0..=t1 {
                    v.push((format!("freeze{n} {cn} +{t}"), vec![format!("5,{t},unload,{b},{n}")]));
                }
            }
        }
    }
    // (S3(ii)'s hold H emulated on the approach (cc767b78's cells): a closed fence gate over the snap
    // turn (xb, 3, 0) from slot 5 +30 to slot 6 +30, slot 5's whole train held against it (the
    // waiting-cart contacts line); and an empty cart in each train place, held at H and in normal
    // flow (`strip,X,Y,Z,S0,S1,K`: the riders of the K-th cart into the run's first cell (the head's
    // peak) in slot 5 removed: the table's own act, set as `strip_riders`))
    if o.approach > 0 && o.stand_stop {
        let lay = crate::queue::loop_layout(o);
        let head = lay.slot_cells.last().map_or(19, |c| c[2] - o.shift[1]);
        let xb = -o.row_gap;
        let gate = format!("5,30,{},oak_fence_gate[facing=north,in_wall=false,open=false,powered=false],100", c(xb, 3, 0));
        v.push(("H hold".into(), vec![gate.clone()]));
        for k in 1..=3 {
            let strip = format!("strip,{xb},1,{},5,5,{k}", head - 2);
            v.push((format!("H hold, empty {k}"), vec![gate.clone(), strip.clone()]));
            v.push((format!("riderless {k}"), vec![strip]));
        }
        if o.humped() {
            // (C'''/C''+hump: a cart left in the dip (its flat cells, at rest or crawling on) at
            // slot 5 +35, before the train reaches the dip: the train pushes it up the climb, no cart
            // rolling back)
            let fx = xb as f64 + 0.5;
            let dy = o.dip_y as f64 + 0.0625;
            let foot = 2 * o.approach as i32 - o.dip_y as i32;
            for z in (foot + 1)..(foot + 1 + o.dip_flat as i32) {
                for (what, v0) in [("at rest", 0.0), ("crawling", -0.02)] {
                    v.push((format!("dip push-in z {}.5 {what}", z), vec![format!("5,35,place,head,{fx},{dy},{z}.5,{v0}")]));
                }
            }
        }
        if o.approach_climb_boost {
            // (a cart forced back over T (set on T's climb edge moving back at 0.1 or 0.3) at every
            // 5 gt of the slot: where it ends (the dip, the hump's top, beyond); the consequence
            // row of S3's "0 back over T", for every hump height down to none)
            let fx = xb as f64 + 0.5;
            let ty = o.approach as f64 + 1.0625;
            for t in (0..100).step_by(5) {
                for v0 in [0.1, 0.3] {
                    v.push((format!("back over T {v0} +{t}"), vec![format!("5,{t},place,head,{fx},{ty},{},{v0}", o.approach as f64 - 0.1)]));
                }
            }
        }
    }
    if o.depart_crest > 0 && o.crest_plain_dip {
        let d = o.depart_crest as i32;
        let zc = 9 - d;
        let dd = if o.crest_descent > 0 { o.crest_descent as i32 } else { d };
        let low = -3 + (zc - 1) - dd;
        let xd = -o.row_gap + 12;
        for k in 0..3 {
            let z = (zc + dd + 1 + k) as f64 + 0.5;
            for (what, v0) in [("at rest", 0.0), ("crawling", 0.02)] {
                v.push((format!("dip stop z {z} {what}"), vec![format!("5,r30,place,head,{},{},{z},{v0}", xd as f64 + 0.5, low as f64 + 0.0625)]));
            }
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
pub fn fault_table(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, seeds: &[i64], cycles: i64, threads: usize, perfect: bool, filter: Option<&str>, all_corners: bool, rel_sweep: Option<(i64, i64)>, fault_slot: i64, meter_corners: bool) -> Result<String, String> {
    let partial = o.shift != [0, 0];
    let cases0: Vec<(String, Vec<String>)> = fault_cases(o, partial).into_iter().filter(|(n, _)| filter.is_none_or(|f| f.split('|').any(|p| n.contains(p)))).collect();
    // (`all_corners`: every case at REL nominal, early and late x staggers 2, 3, 4, 6; `rel_sweep`
    // A..B: every case at the REL phases A..=B gt around the timing's own (each gt), the stagger
    // and the rest as given)
    let mut variants: Vec<(String, LoopTiming)> = vec![(String::new(), *tm)];
    if let (true, Some(m)) = (meter_corners, tm.meter) {
        // (`meter_corners`: S3's corners for the summon margin: the launch ±2 gt (release start),
        // H's opening start ±2 (HOLD_ON and D12/D23 together, as cc767b78's --meter-sweep) and its
        // fixed opening's length ±2)
        variants.clear();
        for rs in [-2, 0, 2] {
            for s in [-2, 0, 2] {
                for l in [-2, 0, 2] {
                    let mut t = *tm;
                    t.release_start += rs;
                    let mut mm = m;
                    mm.d12 += s;
                    mm.d23 += s;
                    mm.hold_on += s;
                    mm.open_len += l;
                    t.meter = Some(mm);
                    variants.push((format!("launch {rs:+} H start {s:+} len {l:+}: "), t));
                }
            }
        }
    } else if let Some((a, b)) = rel_sweep {
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
            // (`strip,X,Y,Z,S0,S1,K`: the case's riderless cart, set as the run's `strip_riders`)
            if let Some(r) = s.strip_prefix("strip,") {
                let n: Vec<i64> = r.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>().map_err(|e| format!("{s}: {e}"))?;
                if n.len() != 6 {
                    return Err(format!("{s}: strip takes X,Y,Z,S0,S1,K"));
                }
                // (`fault_slot`: the case's slot 5 moved there, its slot-relative timing kept)
                let (s0, s1) = if n[3] == 5 && n[4] == 5 { (fault_slot, fault_slot) } else { (n[3], n[4]) };
                t.strip_riders = Some(([n[0] as i32, n[1] as i32, n[2] as i32], s0, s1, n[5] as usize));
                continue;
            }
            let s = match s.strip_prefix("5,") {
                Some(rest) if fault_slot != 5 => format!("{fault_slot},{rest}"),
                _ => s.clone(),
            };
            a[base.len() + k] = Some(crate::queue::parse_act(&s)?);
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
    let mut out = vec![format!("fault table: {} cases x seeds {:?}, {cycles} slots, the fault at slot {fault_slot}{}; per case over the seeds: clean 3 from slot (the latest seed; never = -), carts back through a gate, departures' min gap (contacts), tail entry min gap within groups (touching), its biggest group, problems", cases.len(), seeds, if perfect { ", the perfect tail (queue reset every slot from 6)".to_string() } else if let (Some(r), true) = (tm.reset_queue_at, tm.reset_queue_every) { format!(", the perfect tail (queue reset every slot from {r})") } else { ", the bench's own tail".to_string() })];
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
        let bp = (0..7).map(|i| ss.iter().map(|s| s.cls[i]).sum::<usize>().to_string()).collect::<Vec<_>>().join("/");
        let pad = if o.approach > 0 { format!("{pad} | T rests {tr} (a {} b {} c {} k {} o {}) | climb rev {rv} | met climb {} (slowest {}) T {} (slowest {}) descent {} (slowest {}) | by place flat/climb/T/descent/hump-down/hump-up/run {bp} | upstream max {} | back travel {:.2} onto boosters {} back v {:.3}", tc[0], tc[1], tc[2], tc[3], tc[4], mn[0], mm[0], mn[1], mm[1], mn[2], mm[2], ss.iter().map(|s| s.upstream).max().unwrap_or(0), ss.iter().map(|s| s.back_travel).fold(0.0, f64::max), ss.iter().map(|s| s.back_boost).sum::<usize>(), ss.iter().map(|s| s.back_vmax).fold(0.0, f64::max)) } else { pad };
        let pad = if o.approach > 0 {
            // (the meetings by cause: "met climb" (the meetings on the climb count, `meet_n`) and
            // the by-place climb column, frozen/over T/head-on/stalled/catch-up/other, then in a
            // freeze/within 200 gt of a thaw/other)
            let j6 = |f: &dyn Fn(&Summary) -> [usize; 6]| (0..6).map(|i| ss.iter().map(|s| f(s)[i]).sum::<usize>().to_string()).collect::<Vec<_>>().join("/");
            let j3 = |f: &dyn Fn(&Summary) -> [usize; 3]| (0..3).map(|i| ss.iter().map(|s| f(s)[i]).sum::<usize>().to_string()).collect::<Vec<_>>().join("/");
            format!("{pad} | met climb by cause fz/oT/ho/st/cu/ot {} when fr/th/ot {} | by place climb by cause {} when {} | hump-up by cause {} when {}", j6(&|s| s.n_causes[0]), j3(&|s| s.n_whens[0]), j6(&|s| s.causes[1]), j3(&|s| s.whens[1]), j6(&|s| s.causes[5]), j3(&|s| s.whens[5]))
        } else {
            pad
        };
        // (box contacts: per place with any, the follower's highest stored speed at first contact
        // over the seeds, the count at 1.0 or more (the bridged merge threshold), the closest centres)
        let pad = if o.approach > 0 {
            let names = ["flat", "climb", "T", "descent", "hump-down", "hump-up", "run"];
            let bcs: Vec<String> = (0..7)
                .filter(|i| ss.iter().any(|s| s.bc[*i].0 > 0))
                .map(|i| format!("{} {:.3}/{}/{:.3}/{}", names[i], ss.iter().map(|s| s.bc[i].1).fold(0.0, f64::max), ss.iter().map(|s| s.bc[i].2).sum::<usize>(), ss.iter().map(|s| s.bc[i].5).fold(0.0, f64::max), ss.iter().map(|s| s.bc[i].6).sum::<usize>()))
                .collect();
            format!("{pad} | waiting-cart contacts stored max/at 0.6+/onto empty max/merged {} | over T {} v_T {:.3}", if bcs.is_empty() { "-".to_string() } else { bcs.join(" ") }, ss.iter().map(|s| s.vt.0).sum::<usize>(), ss.iter().map(|s| s.vt.1).fold(0.0, f64::max))
        } else {
            pad
        };
        // (the profile at T, the snap turn and G2 by train place over the seeds: stored min..max
        // and +gt min..max from the train's launch slot)
        let pad = if o.approach > 0 {
            let mut v = Vec::new();
            for code in ["top", "T", "snap", "G2"] {
                let by: Vec<String> = (1..=3)
                    .filter_map(|j| {
                        let xs: Vec<&(String, usize, f64, f64, i64, i64)> = ss.iter().flat_map(|s| s.prof.iter()).filter(|p| p.0 == code && p.1 == j).collect();
                        if xs.is_empty() {
                            return None;
                        }
                        let (s0, s1) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.2), b.max(p.3)));
                        let (d0, d1) = xs.iter().fold((i64::MAX, i64::MIN), |(a, b), p| (a.min(p.4), b.max(p.5)));
                        Some(if code == "G2" { format!("+{d0}..{d1}") } else { format!("{s0:.2}..{s1:.2} +{d0}..{d1}") })
                    })
                    .collect();
                if !by.is_empty() {
                    v.push(format!("{code} {}", by.join(", ")));
                }
            }
            if v.is_empty() { pad } else { format!("{pad} | by train place 1st, 2nd, 3rd: {}", v.join("; ")) }
        } else {
            pad
        };
        let pad = if o.humped() { format!("{pad} | hump back climb {:.2} over top {}", ss.iter().map(|s| s.hump_back.0).fold(0.0, f64::max), ss.iter().map(|s| s.hump_back.1).sum::<usize>()) } else { pad };
        // (S3's meter: trains metered in over releases, the least Dh 3 margin to the next tick,
        // openings passing 2+ carts, the follower's least distance short of H's plane at the shut)
        let ms: Vec<(usize, usize, Option<i64>, usize, Option<f64>)> = ss.iter().filter_map(|s| s.meter).collect();
        let pad = if ms.is_empty() {
            pad
        } else {
            format!(
                "{pad} | meter in {}/{} rel, Dh3 before tick min {}, 2+ per opening {}, follower short min {}",
                ms.iter().map(|m| m.1).sum::<usize>(),
                ms.iter().map(|m| m.0).sum::<usize>(),
                ms.iter().filter_map(|m| m.2).min().map_or("-".into(), |v| v.to_string()),
                ms.iter().map(|m| m.3).sum::<usize>(),
                ms.iter().filter_map(|m| m.4).fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.min(v)))).map_or("-".into(), |v| format!("{v:.2}"))
            )
        };
        // (cc767b78's head model: launches with carts still at H, the most carts at H at once and
        // the farthest back rest z against T's cell)
        let hs: Vec<(usize, usize, f64)> = ss.iter().filter_map(|s| s.hp).collect();
        let pad = if hs.is_empty() {
            pad
        } else {
            format!("{pad} | head-perm: launches with carts at H {}, most at H {} (farthest back z {:.2})", hs.iter().map(|h| h.0).sum::<usize>(), hs.iter().map(|h| h.1).max().unwrap_or(0), hs.iter().map(|h| h.2).fold(0.0, f64::max))
        };
        let sms: Vec<(Option<i64>, usize)> = ss.iter().filter_map(|s| s.sm).collect();
        let pad = if sms.is_empty() { pad } else { format!("{pad} | summon margin least {}, fills under the bar {}", sms.iter().filter_map(|m| m.0).min().map_or("-".into(), |v| v.to_string()), sms.iter().map(|m| m.1).sum::<usize>()) };
        // (the head model's recovery, the worst seed's A-cycles from the fault to clean, and the
        // golems over the seeds: ironspec's "recovery and golems lost printed")
        let hcs: Vec<Option<i64>> = ss.iter().filter_map(|s| s.hclean).collect();
        let pad = if hcs.is_empty() { pad } else { format!("{pad} | recovery {} A-cycles, golems {}", if hcs.iter().any(|h| h.is_none()) { "never".to_string() } else { hcs.iter().flatten().max().map_or("-".into(), |v| v.to_string()) }, ss.iter().filter_map(|s| s.golems).sum::<usize>()) };
        let pad = if o.depart_crest > 0 {
            let dm: usize = ss.iter().map(|s| s.d_meet).sum();
            let dr: usize = ss.iter().map(|s| s.d_rev).sum();
            let dmin = ss.iter().filter_map(|s| s.d_meet_min).fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.min(v)))).map_or("-".into(), |v| format!("{v:.3}"));
            let rm: usize = ss.iter().map(|s| s.d_rise).sum();
            let rmin = ss.iter().filter_map(|s| s.d_rise_min).fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.min(v)))).map_or("-".into(), |v| format!("{v:.3}"));
            format!("{pad} | dip met {dm} (slowest {dmin}) | dep rev {dr} | rise met {rm} (slowest {rmin})")
        } else {
            pad
        };
        let flag = if clean.is_none() || back > 0 || tail.is_some_and(|g| g < 5) || group > 5 || window > 3 || merges > 0 || derails > 0 || dbad > 0 || oot > 0 { " <--" } else { "" };
        // (seeds breaking a hard line: back-crossings, merges, derails, a departure under 5 gt, more
        // than 3 out in one window)
        let hard = ss.iter().filter(|s| s.back > 0 || s.merges > 0 || s.derails > 0 || s.dep_min.is_some_and(|g| g < 5) || s.window_max > 3).count();
        let tko: usize = ss.iter().map(|s| s.takeoff_bad).sum();
        out.push(format!("{name:24} clean from {:>2} | hard {hard}/{} | back {back} | merges {merges} | latch early {early} | dep min {} ({contact}) | take-off 4..6 gt {tko} | tail min {} ({touch}) group {group} | out per window {window} | derails {derails} | declared {dbad}{pad} | problems {}{flag}", opt(clean), ss.len(), opt(dep), opt(tail), probs.join("/")));
    }
    // (the approach's meetings totalled per case family (the case's name without its variant and
    // its trailing "+T"): rows with climb meetings, "met climb" by cause and when, the by-place
    // climb and the hump's rising face, the box contacts at 1.0 or more)
    if o.approach > 0 {
        let family = |name: &str| {
            let n = name.split_once(": ").map_or(name, |(_, b)| b);
            match n.rsplit_once(" +") {
                Some((a, t)) if t.chars().all(|ch| ch.is_ascii_digit()) => a.to_string(),
                _ => n.to_string(),
            }
        };
        let mut fams: Vec<String> = Vec::new();
        for (name, _, _) in &cases {
            let f = family(name);
            if !fams.contains(&f) {
                fams.push(f);
            }
        }
        out.push("totals per case family (met climb rows with any/rows; met climb by cause frozen/over T/head-on/stalled/catch-up/other; when in a freeze/after a thaw/other; by place climb; hump-up; contacts at 1.0+ by place):".to_string());
        for f in fams {
            let ids: Vec<usize> = cases.iter().enumerate().filter(|(_, (n, _, _))| family(n) == f).map(|(i, _)| i).collect();
            let ss: Vec<&Summary> = got.iter().filter(|(k, _)| ids.contains(k)).map(|(_, s)| s).collect();
            let rows = ids.iter().filter(|i| got.iter().any(|(k, s)| k == *i && s.meet_n[0] > 0)).count();
            let sum6 = |f: &dyn Fn(&Summary) -> [usize; 6]| (0..6).map(|i| ss.iter().map(|s| f(s)[i]).sum::<usize>().to_string()).collect::<Vec<_>>().join("/");
            let sum3 = |f: &dyn Fn(&Summary) -> [usize; 3]| (0..3).map(|i| ss.iter().map(|s| f(s)[i]).sum::<usize>().to_string()).collect::<Vec<_>>().join("/");
            let met: usize = ss.iter().map(|s| s.meet_n[0]).sum();
            let c06: Vec<String> = (0..7).map(|i| ss.iter().map(|s| s.bc[i].2).sum::<usize>().to_string()).collect();
            let vmax: Vec<String> = (0..7).map(|i| format!("{:.2}", ss.iter().map(|s| s.bc[i].1).fold(0.0, f64::max))).collect();
            out.push(format!(
                "  {f}: met climb {met} in {rows}/{} rows, by cause {}, when {} | by place climb {} | hump-up {} | waiting-cart contacts at 0.6+ flat/climb/T/descent/hump-down/hump-up/run {} (stored max {}) | merged {} | pushed back over T {} (v_T max {:.3}) | hump back climb max {:.2}, over its top {}",
                ids.len(),
                sum6(&|s| s.n_causes[0]),
                sum3(&|s| s.n_whens[0]),
                ss.iter().map(|s| s.cls[1]).sum::<usize>(),
                ss.iter().map(|s| s.cls[5]).sum::<usize>(),
                c06.join("/"),
                vmax.join("/"),
                ss.iter().map(|s| s.bc.iter().map(|b| b.6).sum::<usize>()).sum::<usize>(),
                ss.iter().map(|s| s.vt.0).sum::<usize>(),
                ss.iter().map(|s| s.vt.1).fold(0.0, f64::max),
                ss.iter().map(|s| s.hump_back.0).fold(0.0, f64::max),
                ss.iter().map(|s| s.hump_back.1).sum::<usize>()
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The runner's by-place line with these counts (and a log naming places, as the runner's does).
    fn by_place(c: [usize; 7]) -> String {
        format!(
            "meetings by the met cart's place: flat run below the climb {} (slowest 0.044), climb {} (slowest -), T {} (slowest 0.010), descent {} (slowest -), hump-down {} (slowest -), hump-up {} (slowest -), run {} (slowest -) (flat slot 5 +43: cart 2 at z 9.84 y 1.06 v 0.805, pusher cart 1 at z 10.82 v 0.908; climb slot 6 +50: cart 1 at z 6.50 y 2.50 v 0.000, pusher cart 0 at z 7.48 v 0.400)",
            c[0], c[1], c[2], c[3], c[4], c[5], c[6]
        )
    }

    /// The per-column positive control (farmopt 2026-11-11): one meeting injected in one place reads
    /// back in that place's column alone. A column repeating another's (the "climb " key matching
    /// inside "flat run below the climb N": c4336062's 388 climb meetings were flat ones) fails here.
    #[test]
    fn each_place_column_counts_only_its_own_meetings() {
        for i in 0..7 {
            let mut c = [0usize; 7];
            c[i] = 1;
            assert_eq!(summarize(&by_place(c)).cls, c, "a meeting in place {i} alone");
        }
        assert_eq!(summarize(&by_place([4, 0, 0, 0, 0, 0, 0])).cls, [4, 0, 0, 0, 0, 0, 0]);
        assert_eq!(summarize(&by_place([0, 3, 0, 0, 0, 0, 0])).cls, [0, 3, 0, 0, 0, 0, 0]);
        assert_eq!(summarize(&by_place([2, 5, 7, 11, 13, 17, 19])).cls, [2, 5, 7, 11, 13, 17, 19]);
    }

    /// The cause lines read back per place, cause and when (one injected count each).
    #[test]
    fn each_cause_column_counts_only_its_own_meetings() {
        let names = ["flat", "climb", "T", "descent", "hump-down", "hump-up", "run"];
        for p in 0..7 {
            for k in 0..6 {
                let row = |i: usize| {
                    let mut c = [0usize; 6];
                    let mut w = [0usize; 3];
                    if i == p {
                        c[k] = 1;
                        w[k % 3] = 1;
                    }
                    format!("{} {} when {}", names[i], c.map(|v| v.to_string()).join("/"), w.map(|v| v.to_string()).join("/"))
                };
                let l = format!("meetings by cause (frozen/over T/head-on/stalled/catch-up/other, when in a freeze/after a thaw/other): {} (examples: climb over T: slot 5 +80: cart 3 z 6.20 y 3.50 vz 0.400 (back over T) / cart 4 z 7.10 y 2.60 vz -0.400)", (0..7).map(row).collect::<Vec<_>>().join("; "));
                let s = summarize(&l);
                for i in 0..7 {
                    let mut c = [0usize; 6];
                    let mut w = [0usize; 3];
                    if i == p {
                        c[k] = 1;
                        w[k % 3] = 1;
                    }
                    assert_eq!((s.causes[i], s.whens[i]), (c, w), "place {i}, injected at place {p} cause {k}");
                }
            }
        }
        let n = summarize("meetings on the climb, on T, below T by cause (frozen/over T/head-on/stalled/catch-up/other, when in a freeze/after a thaw/other): climb 0/2/0/0/0/0 when 0/2/0; T 1/0/0/0/0/0 when 1/0/0; descent 0/0/0/0/0/3 when 0/0/3");
        assert_eq!((n.n_causes, n.n_whens), ([[0, 2, 0, 0, 0, 0], [1, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 3]], [[0, 2, 0], [1, 0, 0], [0, 0, 3]]));
    }
}
