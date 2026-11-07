//! The self-arming roof machine as a DESIGN (`mctas::design`, the door and clock studies' layer
//! format) and its fast judge on redsim, the exact Rust model, through the clock study's driver
//! (`clock::sim::drive_mut`: the clock judge's placement, its `port start` kick and its
//! `unsupported` buildability check). In every window a player's press (break the swap piston,
//! place a piston facing the target, both between two ticks) is tried on a CLONE of the world; the
//! windows where the target is gone 3 ticks later are the armed ones. A miss followed by the
//! one-press repair (the swap piston put back as built) must arm again. The minimizer walks the
//! door search's own edit neighbourhood (`piston_door::search::generic_edits`: removals of 1-3
//! blocks, delays, substitutions, moves, swaps, relocations) and keeps every edit that still
//! passes at every test position and orientation.
//!
//! Ports: `target` (the bedrock to remove, next to the swap piston), `swap` (the swap piston),
//! `press` (the air cell the player lies in, next to the swap piston, under a ceiling block that
//! makes them crawl) and the clock study's `start` kick. Bedrock blocks are the roof: never
//! counted, never edited.

use anyhow::{Context, Result};
use clock::Clock;
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::Pos;
use piston_door::search::{Family, generic_edits};
use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::world::{World, flags};
use std::collections::HashSet;

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn dir_name(v: Pos) -> Option<&'static str> {
    Some(match v {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        _ => return None,
    })
}

fn dir_vec(f: &str) -> Option<Pos> {
    Some(match f {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}

pub fn is_roof(s: &str) -> bool {
    s.starts_with("minecraft:bedrock")
}

#[derive(Clone)]
pub struct Machine {
    pub design: Design,
    pub clock: Clock,
    pub target: Pos,
    pub swap: Pos,
    pub press: Pos,
    /// The swap piston as built (the repair puts it back).
    pub swap_state: String,
    /// The direction from the swap piston to the target: the facing of the piston a press places.
    pub toward: &'static str,
}

impl Machine {
    pub fn new(design: Design) -> Result<Machine> {
        let port = |n: &str| design.ports.get(n).copied().with_context(|| format!("the design has no `port {n}:`"));
        let (target, swap, press) = (port("target")?, port("swap")?, port("press")?);
        let swap_state = design.blocks.get(&swap).cloned().context("no block at `port swap`")?;
        let toward = dir_name([target[0] - swap[0], target[1] - swap[1], target[2] - swap[2]]).context("`port target` must be next to `port swap`")?;
        let clock = Clock::from_design(design.clone())?;
        Ok(Machine { design, clock, target, swap, press, swap_state, toward })
    }

    /// Blocks a player places (the bedrock roof does not count).
    pub fn blocks(&self) -> usize {
        self.design.blocks.values().filter(|s| !is_roof(s)).count()
    }

    /// The swap piston's head cell (where it extends): must stay free.
    pub fn head(&self) -> Option<Pos> {
        let f = BlockState::parse(&self.swap_state).ok()?.get("facing")?.to_string();
        Some(add(self.swap, dir_vec(&f)?))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Verdict {
    /// Windows (the time between tick t and t+1, as `clock::sim` numbers them) where a press works.
    pub arms: Vec<u32>,
    /// The longest wait for an armed window from `steady` to the end (None: fewer than 3 there).
    pub gap: Option<u32>,
    /// A miss then the one-press repair: did a later press work?
    pub repaired: Option<bool>,
    pub why: Option<String>,
}

impl Verdict {
    pub fn ok(&self, max_gap: u32) -> bool {
        self.why.is_none() && self.gap.is_some_and(|g| g <= max_gap) && self.repaired != Some(false)
    }

    pub fn summary(&self, steady: u32) -> String {
        if let Some(w) = &self.why {
            return format!("FAIL {w}");
        }
        let arms: Vec<String> = self.arms.iter().filter(|t| **t >= steady).map(|t| t.to_string()).collect();
        format!(
            "gap {} repair {} armed {}",
            self.gap.map(|g| g.to_string()).unwrap_or("-".into()),
            match self.repaired {
                Some(true) => "ok",
                Some(false) => "FAILS",
                None => "-",
            },
            arms.join(",")
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct JudgeOptions {
    pub ticks: u32,
    pub steady: u32,
    pub repair: bool,
}

impl Default for JudgeOptions {
    fn default() -> Self {
        JudgeOptions { ticks: 160, steady: 60, repair: true }
    }
}

fn is_piston(b: &Block) -> bool {
    let s = b.to_string();
    s.starts_with("minecraft:piston[") || s.starts_with("minecraft:sticky_piston[")
}

fn is_bedrock(b: &Block) -> bool {
    b.to_string().starts_with("minecraft:bedrock")
}

/// Break the block at `p` like a player (`ServerPlayerGameMode.destroyBlock`: removeBlock) and
/// place `state` there (`BlockItem.place`: setBlock with flags 11, the block's onPlace runs).
fn swap_in(w: &mut World, p: Pos, state: &str) {
    w.remove_block(bp(p), false);
    if let Ok(b) = Block::parse(state) {
        w.set_block(bp(p), b, flags::ALL | flags::IMMEDIATE);
    }
}

fn press(w: &mut World, m: &Machine) {
    swap_in(w, m.swap, &format!("minecraft:piston[extended=false,facing={}]", m.toward));
}

/// Does a press now remove the target (looked at 3 ticks later, on a clone)?
fn press_works(w: &World, m: &Machine) -> bool {
    let mut w2 = w.clone();
    press(&mut w2, m);
    for _ in 0..3 {
        w2.tick();
    }
    !is_bedrock(&w2.get(bp(m.target)))
}

/// A miss in this window, then the one-press repair 2 ticks later; true when a press within the
/// next 60 ticks works.
fn repair_works(mut w: World, m: &Machine) -> bool {
    press(&mut w, m);
    for _ in 0..2 {
        w.tick();
    }
    if !is_bedrock(&w.get(bp(m.target))) {
        return false;
    }
    let back = BlockState::parse(&m.swap_state).map(|b| b.with("extended", "false").to_string()).unwrap_or_else(|_| m.swap_state.clone());
    swap_in(&mut w, m.swap, &back);
    for _ in 0..60 {
        w.tick();
        if is_piston(&w.get(bp(m.swap))) && press_works(&w, m) {
            return true;
        }
    }
    false
}

/// The machine in redsim: the press tried in every window, the repair after a miss.
pub fn judge(m: &Machine, o: &JudgeOptions) -> Verdict {
    let mut v = Verdict::default();
    let Some((lo, hi)) = m.design.bounds() else {
        v.why = Some("empty design".into());
        return v;
    };
    let from = [lo[0] - 3, lo[1] - 3, lo[2] - 3];
    let to = [hi[0] + 3, hi[1] + 3, hi[2] + 3];
    match clock::sim::unsupported(&m.clock, from, to) {
        Ok(u) if !u.is_empty() => {
            v.why = Some(format!("a player could not build it: {} at {:?}", u[0].1, u[0].0));
            return v;
        }
        Err(e) => {
            v.why = Some(e);
            return v;
        }
        _ => {}
    }
    let mut miss: Option<World> = None;
    let mut arms = Vec::new();
    let r = clock::sim::drive_mut(&m.clock, from, to, o.ticks, |t, w| {
        if is_piston(&w.get(bp(m.swap))) {
            if press_works(w, m) {
                arms.push(t);
            } else if o.repair && t >= o.steady && miss.is_none() {
                miss = Some(w.clone());
            }
        }
        true
    });
    match r {
        Err(e) => {
            v.why = Some(e);
            return v;
        }
        Ok(Some((t, why))) => {
            v.why = Some(format!("tick {t}: {why}"));
            return v;
        }
        Ok(None) => {}
    }
    v.arms = arms;
    let steady: Vec<u32> = v.arms.iter().copied().filter(|t| *t >= o.steady).collect();
    if steady.len() >= 3 {
        let inner = steady.windows(2).map(|p| p[1] - p[0]).max().unwrap_or(0);
        let lead = steady[0] - o.steady;
        let tail = o.ticks - steady[steady.len() - 1];
        v.gap = Some(inner.max(lead).max(tail));
    }
    if o.repair {
        v.repaired = Some(miss.is_some_and(|w| repair_works(w, m)));
    }
    v
}

// ---- orientations and positions ----------------------------------------------------------------

/// Turned clockwise (seen from above) by `turns` quarter turns around the origin, after an
/// optional mirror image (x -> -x), then moved by `offset`.
pub fn transformed(d: &Design, turns: i32, mirror: bool, offset: Pos) -> Result<Design> {
    let t = turns.rem_euclid(4);
    let pos = |p: Pos| -> Pos {
        let (mut x, mut z) = (if mirror { -p[0] } else { p[0] }, p[2]);
        for _ in 0..t {
            (x, z) = (-z, x);
        }
        [x + offset[0], p[1] + offset[1], z + offset[2]]
    };
    let state = |s: &str| -> Result<String> {
        let mut b = BlockState::parse(s)?;
        if mirror {
            let mut m = b.clone();
            for (k, v) in &b.props {
                let swap = |v: &str| match v {
                    "east" => "west".to_string(),
                    "west" => "east".to_string(),
                    o => o.to_string(),
                };
                match k.as_str() {
                    "facing" => {
                        m.props.insert(k.clone(), swap(v));
                    }
                    "east" => {
                        m.props.insert("west".into(), v.clone());
                    }
                    "west" => {
                        m.props.insert("east".into(), v.clone());
                    }
                    _ => {}
                }
            }
            b = m;
        }
        Ok(b.rotate_y(t).to_string())
    };
    let mut out = d.clone();
    out.blocks = d.blocks.iter().map(|(p, s)| Ok((pos(*p), state(s)?))).collect::<Result<_>>()?;
    out.ports = d.ports.iter().map(|(k, p)| (k.clone(), pos(*p))).collect();
    out.block_entities = d.block_entities.iter().map(|(p, s)| (pos(*p), s.clone())).collect();
    Ok(out)
}

/// The test variants: every offset (the door judge's positions), each in `turns` quarter turns
/// and optionally mirrored.
pub fn variants(offsets: &[Pos], rotations: bool, mirror: bool) -> Vec<(i32, bool, Pos)> {
    let mut out = Vec::new();
    for &o in offsets {
        for t in 0..if rotations { 4 } else { 1 } {
            for mi in if mirror { vec![false, true] } else { vec![false] } {
                out.push((t, mi, o));
            }
        }
    }
    out
}

/// The worst gap over the variants, or the first failing variant's reason.
/// The module suffixes of a design: "" (ports `target`/`swap`/`press`), then "2", "3", ... while
/// `port target<N>` exists: a batch machine, several swap pistons on one clock.
pub fn module_suffixes(d: &Design) -> Vec<String> {
    let mut out = vec![String::new()];
    for n in 2.. {
        if !d.ports.contains_key(&format!("target{n}")) {
            break;
        }
        out.push(n.to_string());
    }
    out
}

/// The design seen from module `suffix`: its ports as the plain `target`/`swap`/`press`.
pub fn module_view(d: &Design, suffix: &str) -> Result<Design> {
    let mut v = d.clone();
    if !suffix.is_empty() {
        for n in ["target", "swap", "press"] {
            let p = d.ports.get(&format!("{n}{suffix}")).copied().with_context(|| format!("the design has no `port {n}{suffix}:`"))?;
            v.ports.insert(n.to_string(), p);
        }
    }
    Ok(v)
}

/// The design with every module other than `suffix` USED: its target bedrock gone and a piston
/// facing down left on the hole (what a successful press leaves behind).
pub fn others_used(d: &Design, suffix: &str) -> Result<Design> {
    let mut v = d.clone();
    for s in module_suffixes(d) {
        if s == suffix {
            continue;
        }
        let m = Machine::new(module_view(d, &s)?)?;
        v.blocks.remove(&m.target);
        v.blocks.insert(m.swap, "minecraft:piston[facing=down]".to_string());
    }
    Ok(v)
}

pub fn judge_all(d: &Design, vs: &[(i32, bool, Pos)], o: &JudgeOptions, max_gap: u32) -> std::result::Result<u32, String> {
    let mut worst = 0;
    let modules = module_suffixes(d);
    for &(t, mi, off) in vs {
        let dv = transformed(d, t, mi, off).map_err(|e| e.to_string())?;
        for s in &modules {
            // A batch machine: each module fresh, and each module after all the others were used.
            let mut views = vec![(module_view(&dv, s).map_err(|e| e.to_string())?, "")];
            if modules.len() > 1 {
                views.push((module_view(&others_used(&dv, s).map_err(|e| e.to_string())?, s).map_err(|e| e.to_string())?, " (the others used)"));
            }
            for (view, how) in views {
                let m = Machine::new(view).map_err(|e| e.to_string())?;
                let v = judge(&m, o);
                if !v.ok(max_gap) {
                    let which = if s.is_empty() && modules.len() == 1 { String::new() } else { format!("module {}{how}, ", if s.is_empty() { "1" } else { s }) };
                    return Err(format!("{which}turns {t} mirror {mi} offset {off:?}: {}", v.summary(o.steady)));
                }
                worst = worst.max(v.gap.unwrap_or(0));
            }
        }
    }
    Ok(worst)
}

// ---- the minimizer --------------------------------------------------------------------------

pub struct MinimizeOptions {
    pub families: Vec<Family>,
    pub radius: i32,
    /// Variants every accepted edit must pass; the first `screen` are tried first.
    pub variants: Vec<(i32, bool, Pos)>,
    pub screen: usize,
    pub max_gap: u32,
    pub judge: JudgeOptions,
    pub threads: usize,
    /// When no edit removes a block: judge the count-neutral edits and follow the first one that
    /// passes and then lets a removal through (one plateau step at a time, up to this many).
    pub plateau: usize,
}

fn keep_and_forbidden(m: &Machine) -> (HashSet<Pos>, HashSet<Pos>) {
    let mut keep: HashSet<Pos> = m.design.blocks.iter().filter(|(_, s)| is_roof(s)).map(|(p, _)| *p).collect();
    let mut forbidden: HashSet<Pos> = HashSet::new();
    for s in module_suffixes(&m.design) {
        let Ok(mm) = module_view(&m.design, &s).and_then(Machine::new) else { continue };
        keep.insert(mm.target);
        keep.insert(mm.swap);
        keep.insert(add(mm.press, [0, 1, 0]));
        forbidden.insert(mm.press);
        if let Some(h) = mm.head() {
            forbidden.insert(h);
        }
    }
    (keep, forbidden)
}

/// Judge candidate designs in parallel; the passing ones with their worst gap.
fn judge_many(cands: Vec<(Design, String)>, o: &MinimizeOptions) -> Vec<(Design, String, u32)> {
    let threads = o.threads.max(1);
    let chunk = cands.len().div_ceil(threads).max(1);
    let screen: Vec<_> = o.variants.iter().take(o.screen.max(1)).copied().collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = cands
            .chunks(chunk)
            .map(|part| {
                let screen = &screen;
                s.spawn(move || {
                    let mut ok = Vec::new();
                    for (d, desc) in part {
                        if judge_all(d, screen, &o.judge, o.max_gap).is_err() {
                            continue;
                        }
                        if let Ok(g) = judge_all(d, &o.variants, &o.judge, o.max_gap) {
                            ok.push((d.clone(), desc.clone(), g));
                        }
                    }
                    ok
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

fn count(d: &Design) -> usize {
    d.blocks.values().filter(|s| !is_roof(s)).count()
}

/// Greedy descent on the block count over the door search's edit neighbourhood, cheapest
/// families first: single removals, then pairs, then triples, swaps and compound edits (a
/// count-neutral edit plus a removal); a family is only tried when the cheaper ones found nothing.
pub fn minimize(base: Design, o: &MinimizeOptions, log: &dyn Fn(&str)) -> Result<Design> {
    let worst = judge_all(&base, &o.variants, &o.judge, o.max_gap).map_err(|e| anyhow::anyhow!("the base design fails: {e}"))?;
    log(&format!("base: {} blocks, worst gap {worst}", count(&base)));
    let mut cur = base;
    let mut plateau_left = o.plateau;
    let mut seen: HashSet<String> = HashSet::new();
    let tiers: Vec<Vec<Family>> = [
        vec![Family::Singles],
        vec![Family::Pairs],
        o.families.iter().copied().filter(|f| !matches!(f, Family::Singles | Family::Pairs)).collect(),
    ]
    .into_iter()
    .map(|t| t.into_iter().filter(|f| o.families.contains(f)).collect::<Vec<_>>())
    .filter(|t| !t.is_empty())
    .collect();
    'descent: loop {
        let m = Machine::new(cur.clone())?;
        let (keep, forbidden) = keep_and_forbidden(&m);
        for tier in &tiers {
            let (improving, _) = generic_edits(&cur, &keep, &forbidden, tier, o.radius);
            let cands: Vec<(Design, String)> =
                improving.iter().map(|e| (e.apply(&cur), e.desc.clone())).filter(|(d, _)| count(d) < count(&cur) && !seen.contains(&d.content_key())).collect();
            log(&format!("{} blocks: judging {} edits ({tier:?})", count(&cur), cands.len()));
            let mut ok = judge_many(cands, o);
            ok.sort_by_key(|(d, _, g)| (count(d), *g));
            if let Some((d, desc, g)) = ok.into_iter().next() {
                log(&format!("  -> {} blocks (worst gap {g}): {desc}", count(&d)));
                seen.insert(d.content_key());
                cur = d;
                continue 'descent;
            }
        }
        if plateau_left == 0 {
            break;
        }
        // Plateau: a count-neutral edit that passes, never revisiting a design.
        let (_, neutral) = generic_edits(&cur, &keep, &forbidden, &o.families, o.radius);
        let cands: Vec<(Design, String)> =
            neutral.iter().map(|e| (e.apply(&cur), e.desc.clone())).filter(|(d, _)| count(d) == count(&cur) && !seen.contains(&d.content_key())).collect();
        log(&format!("  no removal passes; plateau: judging {} neutral edits", cands.len()));
        let mut ok = judge_many(cands, o);
        ok.sort_by_key(|(_, _, g)| *g);
        let Some((d, desc, g)) = ok.into_iter().next() else { break };
        log(&format!("  plateau step (worst gap {g}): {desc}"));
        seen.insert(d.content_key());
        cur = d;
        plateau_left -= 1;
    }
    Ok(cur)
}

// ---- the clock graft ------------------------------------------------------------------------

/// The period (game ticks) a clock archive file's header line states (`# CLOCK: ... period N gt`).
pub fn clock_period(path: &std::path::Path) -> Option<u32> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().find(|l| l.starts_with("# CLOCK:"))?;
    let rest = line.split(" period ").nth(1)?;
    rest.split_whitespace().next()?.parse().ok()
}

/// Every clock of the archive (`designs/clocks/archive`, the clock study's judged clocks) whose
/// stated period is in `periods`, grafted onto `base` in place of the cells inside `remove`
/// (the base's own clock): each clock in 4 quarter turns (mirrored too with `mirror`), its lowest
/// layer on the roof's top + 1, at every position whose cells are free in the region `lo..=hi`.
/// The roof is extended under the region. Returns the designs (named `<clock>@t<turns>[m]<pos>`).
pub fn grafts(base: &Design, remove: (Pos, Pos), clocks: &[std::path::PathBuf], region: (Pos, Pos), mirror: bool) -> Result<Vec<Design>> {
    let inside = |p: &Pos, (a, b): (Pos, Pos)| (0..3).all(|i| p[i] >= a[i].min(b[i]) && p[i] <= a[i].max(b[i]));
    let mut core = base.clone();
    core.blocks.retain(|p, s| is_roof(s) || !inside(p, remove));
    core.ports.remove("start");
    core.meta.remove("port start");
    let roof_y = core.ports.get("target").map(|t| t[1]).context("no target")?;
    let (rlo, rhi) = region;
    for x in rlo[0] - 2..=rhi[0] + 2 {
        for z in rlo[2] - 2..=rhi[2] + 2 {
            core.blocks.entry([x, roof_y, z]).or_insert_with(|| "minecraft:bedrock".to_string());
        }
    }
    let mut out = Vec::new();
    for path in clocks {
        let Ok(c) = Clock::load(path) else { continue };
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("clock").to_string();
        for t in 0..4 {
            for mi in if mirror { vec![false, true] } else { vec![false] } {
                let cd = transformed(&c.design, t, mi, [0, 0, 0])?;
                let Some((clo, chi)) = cd.bounds() else { continue };
                let size = [chi[0] - clo[0], chi[1] - clo[1], chi[2] - clo[2]];
                for x in rlo[0]..=rhi[0] - size[0] {
                    for z in rlo[2]..=rhi[2] - size[2] {
                        for y in [roof_y + 1, roof_y + 2] {
                            let off = [x - clo[0], y - clo[1], z - clo[2]];
                            let placed = cd.translated(off);
                            if placed.blocks.keys().any(|p| core.blocks.contains_key(p)) {
                                continue;
                            }
                            let mut d = core.clone();
                            d.blocks.extend(placed.blocks.iter().map(|(p, s)| (*p, s.clone())));
                            if let Some(sp) = placed.ports.get("start") {
                                d.ports.insert("start".into(), *sp);
                                if let Some(args) = placed.meta.get("port start") {
                                    d.meta.insert("port start".into(), args.clone());
                                }
                            }
                            d.name = format!("{name}@t{t}{}{},{},{}", if mi { "m" } else { "" }, off[0], off[1], off[2]);
                            out.push(d);
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Judge designs in parallel at the given variants: the passing ones with their worst gap and
/// block count, fewest blocks first.
pub fn judge_designs(ds: Vec<Design>, vs: &[(i32, bool, Pos)], o: &JudgeOptions, max_gap: u32, threads: usize) -> Vec<(Design, u32, usize)> {
    let chunk = ds.len().div_ceil(threads.max(1)).max(1);
    let mut ok: Vec<(Design, u32, usize)> = std::thread::scope(|s| {
        let hs: Vec<_> = ds
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .filter_map(|d| judge_all(d, &vs[..1.min(vs.len())], o, max_gap).ok().and_then(|_| judge_all(d, vs, o, max_gap).ok()).map(|g| (d.clone(), g, count(d))))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    ok.sort_by_key(|(d, g, n)| (*n, *g, d.name.clone()));
    ok
}

// ---- conversions --------------------------------------------------------------------------

/// A machine design from a simulator scenario: its blocks, a bedrock roof layer under their
/// footprint (one block wider), the scenario's target, the swap piston (its chord's first cell),
/// `press` and the start kick (`start` = the block placed last).
pub fn design_from_scenario(sc: &crate::Scenario, name: &str, press: Pos, start: (Pos, String)) -> Result<Design> {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    for (x, y, z, s) in &sc.blocks {
        d.blocks.insert([*x, *y, *z], s.clone());
    }
    d.blocks.insert(start.0, start.1.clone());
    let (lo, hi) = d.bounds().context("no blocks")?;
    let ty = sc.target[1];
    for x in lo[0] - 3..=hi[0] + 3 {
        for z in lo[2] - 3..=hi[2] + 3 {
            d.blocks.entry([x, ty, z]).or_insert_with(|| "minecraft:bedrock".to_string());
        }
    }
    d.blocks.insert(sc.target, "minecraft:bedrock".to_string());
    let swap = sc.chord.first().map(|a| a.pos).context("the scenario has no chord")?;
    d.ports.insert("target".into(), sc.target);
    d.ports.insert("swap".into(), swap);
    d.ports.insert("press".into(), press);
    d.ports.insert("start".into(), start.0);
    d.meta.insert("port start".into(), "last".into());
    Ok(d)
}

/// The yaw (degrees) of a horizontal look along `v`.
fn yaw_of(v: Pos) -> f32 {
    match v {
        [0, _, 1] => 0.0,
        [-1, _, 0] => 90.0,
        [0, _, -1] => 180.0,
        _ => -90.0,
    }
}

/// Append a miss (the press in window `miss`) and the one-press repair in window `repair` to a
/// scenario made by [`scenario_from_design`]: from the same spot, the bot turns to look along the
/// swap piston's facing REVERSED (a placed piston faces away from the look), 37.6 degrees toward
/// the swap piston and 10 degrees down, so the view ray still enters the face it pressed, and the
/// original piston goes back.
pub fn add_miss_repair(sc: &mut crate::Scenario, miss: u32, repair: u32) -> Result<()> {
    let swap = sc.chord.first().map(|a| a.pos).context("no chord")?;
    let toward = sc.chord.get(1).and_then(|a| a.state.clone()).context("no place in the chord")?;
    let stance = sc.stance.clone().context("no stance")?;
    let orig = sc.blocks.iter().find(|(x, y, z, _)| [*x, *y, *z] == swap).map(|b| b.3.clone()).context("no swap piston in the build")?;
    let facing = BlockState::parse(&orig)?.get("facing").context("swap piston without facing")?.to_string();
    let f = dir_vec(&facing).context("facing")?;
    let sgn = |d: f64| if d.abs() < 0.25 { 0 } else { d.signum() as i32 };
    let to_swap = [sgn(swap[0] as f64 + 0.5 - stance.pos[0]), 0, sgn(swap[2] as f64 + 0.5 - stance.pos[2])];
    // A piston facing up goes back by looking DOWN (more than 45 degrees) at the face it pressed.
    let look_back = if facing == "up" {
        [yaw_of(if to_swap[0] != 0 { [to_swap[0], 0, 0] } else { [0, 0, to_swap[2]] }), 50.0]
    } else {
        // Look against the facing (yaw of -f), turned toward the swap piston (whose face the bot
        // sees) by 37.6 degrees: the nearest look direction stays -f.
        let back = yaw_of([-f[0], 0, -f[2]]);
        let toward_yaw = yaw_of(if to_swap[0] != 0 && f[0] == 0 { [to_swap[0], 0, 0] } else { [0, 0, to_swap[2]] });
        let mut delta = (toward_yaw - back).rem_euclid(360.0);
        if delta > 180.0 {
            delta -= 360.0;
        }
        [back + delta.signum() * 37.6, 10.0]
    };
    let act = |what: &str, state: Option<String>, at: u32, look: Option<[f32; 2]>| crate::Action {
        what: what.into(),
        pos: swap,
        state,
        at,
        click: None,
        face: None,
        look,
        every: 0,
        times: 0,
    };
    sc.script.push(act("break", None, miss, None));
    sc.script.push(act("place", Some(toward.clone()), miss, None));
    sc.script.push(act("probe", Some(toward), miss + 1, None));
    let look = Some(look_back);
    sc.script.push(act("break", None, repair, look));
    sc.script.push(act("place", Some(format!("minecraft:piston[facing={facing}]")), repair, look));
    sc.script.push(act("probe", Some(format!("minecraft:piston[facing={facing}]")), repair + 1, None));
    sc.name = format!("{}-miss{miss}-repair{repair}", sc.name);
    Ok(())
}

/// A scenario (for `bedrock scan|watch` on the game, or `bedrock vanilla`) from a machine design,
/// moved so its target is at `at`. The player lies in the press cell against the swap piston and
/// looks up at its face (pitch -55: a piston placed that way faces down). Vanilla (`dimension`
/// set): the build is cleared and rebuilt around the target in that dimension, the start block is
/// left out and the start port's NEIGHBOUR placed last instead (a command-placed observer fires
/// once as it is placed: the one pulse a player's start block gives); the game's own scenario
/// (no dimension) places the start block with a command in window 0.
pub fn scenario_from_design(d: &Design, at: Pos, dimension: Option<&str>) -> Result<crate::Scenario> {
    let m = Machine::new(d.clone())?;
    let off = [at[0] - m.target[0], at[1] - m.target[1], at[2] - m.target[2]];
    let d = d.translated(off);
    let m = Machine::new(d.clone())?;
    let start = d.ports.get("start").copied();
    let (lo, hi) = d.bounds().context("no blocks")?;
    let mut blocks: Vec<(i32, i32, i32, String)> = Vec::new();
    let in_dim = |c: String| match dimension {
        Some(dim) => format!("execute in minecraft:{dim} run {c}"),
        None => c,
    };
    for (p, s) in &d.blocks {
        if Some(*p) == start {
            continue;
        }
        if dimension.is_some() && is_roof(s) {
            continue;
        }
        blocks.push((p[0], p[1], p[2], s.clone()));
    }
    let mut setup = Vec::new();
    let mut script = Vec::new();
    if dimension.is_some() {
        setup.push(in_dim(format!("forceload add {} {} {} {}", lo[0] - 4, lo[2] - 4, hi[0] + 4, hi[2] + 4)));
        setup.push(in_dim(format!("fill {} {} {} {} {} {} minecraft:air", lo[0] - 3, at[1] + 1, lo[2] - 3, hi[0] + 3, hi[1] + 3, hi[2] + 3)));
        // The roof's top layer under the build back to bedrock (an earlier trial at the same place
        // may have carved holes), then the holes of used modules (`others_used`): the real roof
        // has a hole under a used module's down-facing piston. `fill` (not `setblock`): a fill
        // that changes nothing is not an error.
        setup.push(in_dim(format!("fill {} {} {} {} {} {} minecraft:bedrock", lo[0] - 3, at[1], lo[2] - 3, hi[0] + 3, at[1], hi[2] + 3)));
        for (p, s) in &d.blocks {
            let below = [p[0], p[1] - 1, p[2]];
            if s.starts_with("minecraft:piston[") && s.contains("facing=down") && below[1] == at[1] && !d.blocks.contains_key(&below) {
                setup.push(in_dim(format!("fill {0} {1} {2} {0} {1} {2} minecraft:air", below[0], below[1], below[2])));
            }
        }
    }
    // The kick, like the player's last block: placed with a command in window 0, after the build
    // has settled (a command-placed observer fires once as it is placed: those pulses are over by
    // then).
    if let Some(s) = start {
        script.push(crate::Action {
            what: "cmd".into(),
            pos: [0, 0, 0],
            state: Some(in_dim(format!("setblock {} {} {} {}", s[0], s[1], s[2], d.blocks[&s]))),
            at: 0,
            click: None,
            face: None,
            look: None,
            every: 0,
            times: 0,
        });
    }
    let v = [m.swap[0] - m.press[0], 0, m.swap[2] - m.press[2]];
    let pos = [m.press[0] as f64 + 0.5 + 0.2 * v[0] as f64, m.press[1] as f64, m.press[2] as f64 + 0.5 + 0.2 * v[2] as f64];
    let stance = crate::vanilla::Stance { pos, look: [yaw_of(v), -55.0], pose: Some("crawling".into()), legit: true };
    let place = format!("minecraft:piston[facing={}]", m.toward);
    let chord = vec![
        crate::Action { what: "break".into(), pos: m.swap, state: None, at: 0, click: None, face: None, look: None, every: 0, times: 0 },
        crate::Action { what: "place".into(), pos: m.swap, state: Some(place), at: 0, click: None, face: None, look: None, every: 0, times: 0 },
    ];
    Ok(crate::Scenario {
        name: format!("{}{}", d.name, if dimension.is_some() { "-vanilla" } else { "" }),
        notes: format!("generated by `mctas bedrock scenario` from the machine design {}", d.name),
        target: m.target,
        blocks,
        settle: 40,
        script,
        chord,
        release: Vec::new(),
        horizon: 20,
        stance: Some(stance),
        dimension: dimension.map(|s| s.to_string()),
        setup,
        also: Vec::new(),
        strict_observers: dimension.is_some(),
        survival: false,
    })
}
