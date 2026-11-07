//! Bedrock breakers (the bedrock track, 2026-11-07; brief `designs/briefs/bedrock-breaker.md`,
//! the work in `designs/bedrock/NOTES.md`): kind `bedrock-breaker N`, a machine on top of the
//! Nether roof that arms a swap piston on each of N bedrock by itself, and a player who presses
//! one key (Attack + Use) on an armed piston: it breaks and a piston facing down goes in its cell;
//! the retract still queued for the old piston runs on the new one and removes the bedrock
//! (`PistonBaseBlock.triggerEvent` reads the facing when the event runs).
//!
//! The design declares, for each bedrock `k` (`k` empty for the first, then `.2`, `.3`, ...):
//! `port bedrock[.k]` (the target, a bedrock block of the design's roof), `port swap[.k]` (its
//! swap piston, right above it) and `port press[.k]` (the air cell the player lies in, under a
//! closed trapdoor), plus the clock's kick, `port start`, placed last. Bedrock blocks are the
//! world's (a player cannot place bedrock): the roof does not count.
//!
//! The judge (each build: positions x rotations): the machine is placed and kicked, [`SETTLE`] gt
//! pass, then for each swap piston the press (break it, place a piston facing down, 3 gt later the
//! bedrock must be gone) is tried in each of [`SPAN`] windows (two clock periods), each try on a
//! fresh build; the longest wait between two windows where it works, the span's ends included,
//! must be at most [`MAX_GAP`] gt. Nobody pressing, every bedrock must stay. On the base build, a
//! miss (a press in a window that does not work, the bedrock must stay) and the one-press repair
//! (the original piston put back 2 gt later): a press in one of the next [`REPAIR_WINDOWS`]
//! windows must work again. Metrics: armed every, bedrock, miss (the base build's). Judged in the
//! game (`circuit verify`) and by its redsim twin ([`sim_verify`]).

use crate::check::Outcome;
use crate::registry::{Agree, KindDef, RunAt, Site, StudyDef, Twin};
use crate::stimulus::Schedule;
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Result, anyhow, bail};
use mctas::sim::{Pool, SandboxOptions, Worker};
use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::world::{World, flags};
use serde_json::json;

/// The study's registration (crate::registry): `circuit: bedrock-breaker N`.
pub static STUDY: StudyDef = StudyDef {
    id: "bedrock",
    title: "Bedrock breakers",
    site_title: "Bedrock breakers",
    site_intro: "Bedrock cannot be mined, but a piston can remove it. A piston whose retract is already queued can be broken and replaced, in the same game tick, by a piston facing down: the queued retract then runs on the new piston and removes the block under it. These machines sit on top of the Nether roof and arm that moment by themselves, every 6 game ticks, with no lag and no TNT. The player lies under a trapdoor next to the piston and presses one key bound to both Attack and Use, with an Efficiency V pickaxe and Haste II from a beacon, in rhythm with the machine; a press at the wrong moment is undone with one more press. Our test, in the real game, presses in every tick of two clock periods at every piston, in every orientation, and checks the miss. For each number of bedrock side by side we keep the design with the fewest blocks placed.",
    site_uses: "Open a hole in the Nether roof's top layer from above, or clear a strip of it, in survival, without lag, TNT or luck.",
    site_blurb: "Remove the Nether roof's bedrock with one key press, armed every 6 ticks.",
    kinds: &[KindDef {
        word: "bedrock-breaker",
        parse: parse_kind,
        inputs: &[],
        check: check_kind,
        verify: verify_in_pool,
        twin: Twin::Exact(sim_verify),
        agree: Agree::Metrics,
        ticks: |_| SETTLE + SPAN + 3,
        axis: |args, _| format!("{} bedrock", args.first().map(String::as_str).unwrap_or("?")),
        runs: &["base"],
        run: run_kind,
        site: Site {
            title: |k| match k.split_whitespace().nth(1).and_then(|n| n.parse::<u32>().ok()) {
                Some(1) | None => "Bedrock breaker: removes a bedrock block of the Nether roof".into(),
                Some(n) => format!("Bedrock breaker: removes {n} bedrock blocks side by side"),
            },
            summary: |_| "arms a piston standing on the bedrock every 6 game ticks; one key press (Attack and Use on the same key) at that moment swaps it for a piston facing down, and the bedrock under it disappears".into(),
            // (the site reads a metric's first word as its name: `armed every 6 gt` is `armed`)
            metric: |m| match m {
                "armed" => Some("the longest wait, in game ticks, between two moments where a press removes the bedrock: a press at a random moment works once in that many"),
                "bedrock" => Some("how many bedrock blocks side by side one build removes"),
                "miss" => Some("what a press at the wrong moment costs"),
                _ => None,
            },
        },
    }],
};

/// Game ticks between the kick and the first window tried (the clock's start is over).
pub const SETTLE: u32 = 60;
/// The windows tried for each swap piston: two periods of the 12 gt clock.
pub const SPAN: u32 = 12;
/// The longest wait allowed between two windows where a press works.
pub const MAX_GAP: u32 = 6;
/// After the repair, the press must work again within this many windows.
pub const REPAIR_WINDOWS: u32 = 24;

/// `bedrock-breaker N`: how many bedrock blocks one build removes, at least 1.
fn parse_kind(w: &[&str]) -> Result<Vec<String>> {
    match w.first().and_then(|s| s.parse::<u32>().ok()) {
        Some(n) if n >= 1 => Ok(vec![n.to_string()]),
        _ => bail!("bedrock-breaker: give how many bedrock blocks it removes (`circuit: bedrock-breaker 1`)"),
    }
}

/// One swap piston: its bedrock, its cell, the player's cell and its state as built.
#[derive(Clone, Debug)]
pub struct Swap {
    pub name: String,
    pub bedrock: Pos,
    pub swap: Pos,
    pub press: Pos,
    pub state: String,
}

/// The design's swap pistons, in port name order (`bedrock`, `bedrock.2`, ...).
pub fn swaps(c: &Circuit) -> std::result::Result<Vec<Swap>, String> {
    let d = &c.clock.design;
    let mut out = Vec::new();
    for (name, pos) in &d.ports {
        let Some(sub) = name.strip_prefix("bedrock") else { continue };
        let port = |base: &str| d.ports.get(&format!("{base}{sub}")).copied().ok_or_else(|| format!("port {name}: no `port {base}{sub}:`"));
        let (swap, press) = (port("swap")?, port("press")?);
        let state = d.blocks.get(&swap).cloned().unwrap_or_default();
        out.push(Swap { name: name.clone(), bedrock: *pos, swap, press, state });
    }
    out.sort_by_key(|s| (s.name.len(), s.name.clone()));
    if out.is_empty() {
        return Err("bedrock-breaker: needs `port bedrock: x y z` (a bedrock target), `port swap:` (its piston, above it) and `port press:` (the player's cell)".into());
    }
    Ok(out)
}

fn is_piston(s: &str) -> bool {
    s.starts_with("minecraft:piston[") || s.starts_with("minecraft:sticky_piston[")
}

fn is_bedrock(s: &str) -> bool {
    s.starts_with("minecraft:bedrock")
}

/// The ports: N swap pistons, each on its bedrock with an air cell for the player next to it.
fn check_kind(c: &Circuit, args: &[String]) -> Result<()> {
    let n: usize = args.first().and_then(|n| n.parse().ok()).unwrap_or(0);
    let s = swaps(c).map_err(|e| anyhow!(e))?;
    if s.len() != n {
        bail!("bedrock-breaker {n}: the design declares {} bedrock targets", s.len());
    }
    let d = &c.clock.design;
    for w in &s {
        if !d.blocks.get(&w.bedrock).is_some_and(|b| is_bedrock(b)) {
            bail!("port {}: the target must be a bedrock block of the design", w.name);
        }
        if !is_piston(&w.state) {
            bail!("port swap for {}: the swap piston must be a piston, the design has {:?}", w.name, w.state);
        }
        if [w.swap[0] - w.bedrock[0], w.swap[1] - w.bedrock[1], w.swap[2] - w.bedrock[2]] != [0, 1, 0] {
            bail!("port swap for {}: the swap piston stands right above its bedrock", w.name);
        }
        let near = (w.press[0] - w.swap[0]).abs() + (w.press[1] - w.swap[1]).abs() + (w.press[2] - w.swap[2]).abs() == 1;
        if !near || d.blocks.contains_key(&w.press) {
            bail!("port press for {}: the player's cell is an air cell next to the swap piston", w.name);
        }
    }
    Ok(())
}

/// The piston a press places: facing its bedrock (down).
fn pressed() -> &'static str {
    "minecraft:piston[extended=false,facing=down]"
}

/// The swap piston as built, retracted (what the repair puts back).
fn built(s: &Swap) -> String {
    s.state.replace("extended=true", "extended=false")
}

/// The machine's box, the roof left out (bedrock is the world's).
fn dims(c: &Circuit) -> [i32; 3] {
    let cells: Vec<Pos> = c.clock.design.blocks.iter().filter(|(_, s)| !is_bedrock(s)).map(|(p, _)| *p).collect();
    if cells.is_empty() {
        return [0, 0, 0];
    }
    let lo = |i: usize| cells.iter().map(|p| p[i]).min().unwrap_or(0);
    let hi = |i: usize| cells.iter().map(|p| p[i]).max().unwrap_or(0);
    [hi(0) - lo(0) + 1, hi(1) - lo(1) + 1, hi(2) - lo(2) + 1]
}

/// The longest wait for a working window over windows `0..span`, the ends included (`None`: it
/// never works).
pub fn longest_wait(arms: &[u32], span: u32) -> Option<u32> {
    let first = *arms.first()?;
    let inner = arms.windows(2).map(|p| p[1] - p[0]).max().unwrap_or(0);
    Some(inner.max(first + 1).max(span - arms[arms.len() - 1]))
}

/// The outcome of one build from each swap piston's working windows.
fn judge(n: usize, arms: &[Vec<u32>], kept: Result<(), String>, span: u32) -> Outcome {
    let mut table = Vec::new();
    let mut why = Vec::new();
    let mut worst = 0;
    for (k, a) in arms.iter().enumerate() {
        table.push(format!("  piston {}: the press works in windows {}", k + 1, if a.is_empty() { "none".to_string() } else { a.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ") }));
        match longest_wait(a, span) {
            Some(g) if g <= MAX_GAP => worst = worst.max(g),
            Some(g) => why.push(format!("piston {}: a press waits up to {g} gt for a working window (at most {MAX_GAP})", k + 1)),
            None => why.push(format!("piston {}: the press never works", k + 1)),
        }
    }
    if let Err(e) = kept {
        why.push(e);
    }
    let metrics = vec![("armed every".to_string(), format!("{worst} gt")), ("bedrock".to_string(), n.to_string())];
    if why.is_empty() { Outcome { pass: true, why: String::new(), metrics, table } } else { Outcome { pass: false, why: why.join("; "), metrics, table } }
}

// ---- the game ---------------------------------------------------------------------------------

/// What one game run does at the swap pistons.
#[derive(Clone, Copy, Debug)]
enum Plan {
    /// Nobody presses.
    Rest,
    /// Piston `k` pressed in window `at`.
    Press { k: usize, at: u32 },
    /// Piston `k` pressed in window `miss`, put back 2 gt later, pressed again in window `at`.
    Repair { k: usize, miss: u32, at: u32 },
}

/// The bridge's `build` op: a player's break, then a place, between two ticks.
fn chord(w: &mut Worker, sb: u32, at: Pos, state: &str) -> Result<()> {
    let steps = json!([{"do": "break", "pos": at}, {"do": "place", "pos": at, "state": state}]);
    w.call(json!({"op": "build", "sandbox": sb, "steps": steps, "min_ticks": 0, "settle": 0, "stop_on_fail": false}))?;
    Ok(())
}

fn block(w: &mut Worker, sb: u32, p: Pos) -> Result<String> {
    Ok(w.get_region(sb, p, p)?.get(p).unwrap_or("?").to_string())
}

/// One game run of build `c` on a fresh sandbox: placed, kicked, [`SETTLE`] gt, then the plan;
/// which bedrock targets are still there at the end. `Err(why)`: a player could not build it.
fn game_run(w: &mut Worker, c: &Circuit, s: &[Swap], plan: Plan, check_build: bool) -> Result<std::result::Result<Vec<bool>, String>> {
    let (from, to) = crate::run::watch_box(c, 3);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: crate::shulker::build_seed(from, to, 0) })?;
    let r = game_run_in(w, sb, c, s, plan, check_build, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

#[allow(clippy::too_many_arguments)]
fn game_run_in(w: &mut Worker, sb: u32, c: &Circuit, s: &[Swap], plan: Plan, check_build: bool, from: Pos, to: Pos) -> Result<std::result::Result<Vec<bool>, String>> {
    clock::verify::place(w, sb, &c.clock)?;
    if check_build {
        let last = c.clock.withheld();
        let placed = w.get_region(sb, from, to)?;
        let popped = c.clock.design.blocks.iter().filter(|(q, _)| Some(**q) != last).find(|(q, _)| placed.get(**q).is_none_or(mctas::blockstate::is_air)).map(|(q, st)| (*q, st.clone()));
        let floating = w.unsupported(sb, from, to)?.into_iter().find(|q| Some(*q) != last).map(|q| (q, c.clock.design.blocks.get(&q).cloned().unwrap_or_default()));
        if let Some((q, st)) = popped.or(floating) {
            return Ok(Err(format!("{} at {q:?} has no support (a player could not build it)", clock::short(&st))));
        }
    }
    let actions = c.clock.actions();
    for t in 0..c.clock.free_from() {
        for (_, a) in actions.iter().filter(|(at, _)| *at == t) {
            clock::verify::apply(w, sb, a)?;
        }
        w.step(sb, 1)?;
    }
    for (_, a) in actions.iter().filter(|(at, _)| *at == c.clock.free_from()) {
        clock::verify::apply(w, sb, a)?;
    }
    w.step(sb, SETTLE)?;
    match plan {
        Plan::Rest => {
            w.step(sb, SPAN + 3)?;
        }
        Plan::Press { k, at } => {
            w.step(sb, at)?;
            chord(w, sb, s[k].swap, pressed())?;
            w.step(sb, 3)?;
        }
        Plan::Repair { k, miss, at } => {
            w.step(sb, miss)?;
            chord(w, sb, s[k].swap, pressed())?;
            w.step(sb, 2)?;
            if !is_bedrock(&block(w, sb, s[k].bedrock)?) {
                return Ok(Err(format!("piston {}: the miss in window {miss} removed the bedrock", k + 1)));
            }
            chord(w, sb, s[k].swap, &built(&s[k]))?;
            w.step(sb, at - miss - 2)?;
            chord(w, sb, s[k].swap, pressed())?;
            w.step(sb, 3)?;
        }
    }
    let mut kept = Vec::new();
    for x in s {
        kept.push(is_bedrock(&block(w, sb, x.bedrock)?));
    }
    Ok(Ok(kept))
}

/// One build in the game: the rest run (and the build check), then every swap piston in every
/// window of the span.
fn game_build(w: &mut Worker, c: &Circuit, n: usize) -> Result<std::result::Result<BuildOk, String>> {
    let s = match swaps(c) {
        Ok(s) => s,
        Err(why) => return Ok(Err(why)),
    };
    let kept = match game_run(w, c, &s, Plan::Rest, true)? {
        Err(why) => return Ok(Err(why)),
        Ok(k) if k.iter().all(|b| *b) => Ok(()),
        Ok(_) => Err("a bedrock target disappeared with nobody pressing".to_string()),
    };
    let mut arms = Vec::new();
    for k in 0..s.len() {
        let mut a = Vec::new();
        for at in 0..SPAN {
            if let Ok(kept) = game_run(w, c, &s, Plan::Press { k, at }, false)?
                && !kept[k]
            {
                a.push(at);
            }
        }
        arms.push(a);
    }
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    Ok(Ok(BuildOk { outcome: judge(n, &arms, kept, SPAN), dims: dims(c), family }))
}

/// The miss and the repair in the game, for every swap piston of build `c`.
fn game_repair(w: &mut Worker, c: &Circuit, arms: &[Vec<u32>]) -> Result<std::result::Result<(), String>> {
    let s = swaps(c).map_err(|e| anyhow!(e))?;
    for (k, a) in arms.iter().enumerate() {
        let Some(miss) = (0..SPAN).find(|t| !a.contains(t)) else { continue };
        let mut ok = false;
        for at in miss + 2..miss + 2 + REPAIR_WINDOWS {
            match game_run(w, c, &s, Plan::Repair { k, miss, at }, false)? {
                Err(why) => return Ok(Err(why)),
                Ok(kept) if !kept[k] => {
                    ok = true;
                    break;
                }
                Ok(_) => {}
            }
        }
        if !ok {
            return Ok(Err(format!("piston {}: after a miss in window {miss} and the repair, no press works for {REPAIR_WINDOWS} windows", k + 1)));
        }
    }
    Ok(Ok(()))
}

fn kind_n(c: &Circuit) -> Result<(Kind, usize)> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let Kind::Study(sk) = &kind else { bail!("{}: not a bedrock breaker", kind.name()) };
    let n = sk.num(0)? as usize;
    Ok((kind, n))
}

/// The judge (`mctas circuit verify`): every build in the game, then the miss and the repair on
/// the base build.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let (kind, n) = kind_n(c)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        game_build(w, &rc, n)
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.map_err(|e| anyhow!("rotation {t} at {off:?}: {e:#}"))? });
    }
    let rep = report(c, kind, builds);
    if !rep.pass {
        return Ok(rep);
    }
    let arms = base_arms(&rep);
    let repaired = pool.map(vec![()], |w, _, _| game_repair(w, c, &arms)).into_iter().next().unwrap_or_else(|| Err(anyhow!("no worker")))?;
    Ok(finish(rep, repaired))
}

/// The base build's working windows per swap piston, read back from its table.
fn base_arms(rep: &VerifyReport) -> Vec<Vec<u32>> {
    let Some(Ok(k)) = rep.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0]).map(|b| &b.result) else { return Vec::new() };
    k.outcome
        .table
        .iter()
        .filter_map(|l| l.split("windows ").nth(1))
        .map(|w| w.split(", ").filter_map(|t| t.trim().parse().ok()).collect())
        .collect()
}

fn report(c: &Circuit, kind: Kind, builds: Vec<Build>) -> VerifyReport {
    let mut schedule = Schedule::new(0);
    schedule.ticks = SETTLE + SPAN + 3;
    crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() })
}

/// The miss and the repair's verdict, and its metric on the base build.
fn finish(mut rep: VerifyReport, repaired: std::result::Result<(), String>) -> VerifyReport {
    if let Err(why) = repaired {
        rep.pass = false;
        rep.verdict = format!("miss: {why}");
        return rep;
    }
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        k.outcome.metrics.push(("miss".to_string(), "one press puts the piston back".to_string()));
        k.outcome.table.push("  miss + repair: a press works again (every piston)".to_string());
    }
    rep
}

// ---- the redsim twin ----------------------------------------------------------------------------

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A player's break (`ServerPlayerGameMode.destroyBlock`: removeBlock) and place (`BlockItem.place`:
/// setBlock with every flag, the block's onPlace runs) of `state` at `p`.
fn swap_in(w: &mut World, p: Pos, state: &str) {
    w.remove_block(bp(p), false);
    if let Ok(b) = Block::parse(state) {
        w.set_block(bp(p), b, flags::ALL | flags::IMMEDIATE);
    }
}

/// Does a press of `s` now remove its bedrock (3 ticks later, on a copy of the world)?
fn sim_press_works(w: &World, s: &Swap) -> bool {
    let mut w2 = w.clone();
    swap_in(&mut w2, s.swap, pressed());
    for _ in 0..3 {
        w2.tick();
    }
    !is_bedrock(&w2.get(bp(s.bedrock)).to_string())
}

/// The twin's run of build `c`: per swap piston, the windows of the span where a press works;
/// whether every bedrock stayed with nobody pressing; a world in a missed window per piston (for
/// the repair).
fn sim_run(c: &Circuit, s: &[Swap]) -> std::result::Result<(Vec<Vec<u32>>, Result<(), String>, Vec<Option<World>>), String> {
    let (from, to) = crate::run::watch_box(c, 3);
    let u = clock::sim::unsupported(&c.clock, from, to)?;
    if let Some((q, st)) = u.first() {
        return Err(format!("{} at {q:?} has no support (a player could not build it)", clock::short(st)));
    }
    let start = c.clock.free_from() + SETTLE;
    let mut arms = vec![Vec::new(); s.len()];
    let mut misses: Vec<Option<World>> = vec![None; s.len()];
    let mut kept = Ok(());
    let r = clock::sim::drive_mut(&c.clock, from, to, start + SPAN + 3, |t, w| {
        if t >= start && t < start + SPAN {
            for (k, x) in s.iter().enumerate() {
                if is_piston(&w.get(bp(x.swap)).to_string()) && sim_press_works(w, x) {
                    arms[k].push(t - start);
                } else if misses[k].is_none() {
                    misses[k] = Some(w.clone());
                }
            }
        }
        if kept.is_ok() && s.iter().any(|x| !is_bedrock(&w.get(bp(x.bedrock)).to_string())) {
            kept = Err(format!("tick {t}: a bedrock target disappeared with nobody pressing"));
        }
        true
    })?;
    if let Some((t, why)) = r {
        return Err(format!("tick {t}: {why}"));
    }
    Ok((arms, kept, misses))
}

/// The miss (already in `w`'s window) and the repair on the twin: a press works again within
/// [`REPAIR_WINDOWS`] windows.
fn sim_repair(mut w: World, s: &Swap) -> bool {
    swap_in(&mut w, s.swap, pressed());
    for _ in 0..2 {
        w.tick();
    }
    if !is_bedrock(&w.get(bp(s.bedrock)).to_string()) {
        return false;
    }
    swap_in(&mut w, s.swap, &built(s));
    for _ in 0..REPAIR_WINDOWS {
        w.tick();
        if is_piston(&w.get(bp(s.swap)).to_string()) && sim_press_works(&w, s) {
            return true;
        }
    }
    false
}

/// The whole judge on redsim (`circuit sim`, the searches): the same builds and windows, then the
/// miss and the repair on the base build.
pub fn sim_verify(c: &Circuit, o: &Options) -> std::result::Result<VerifyReport, String> {
    let (kind, n) = kind_n(c).map_err(|e| e.to_string())?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let mut builds = Vec::new();
    let mut repaired = Ok(());
    for t in &turns {
        let tc = c.turned_pure(*t).map_err(|e| format!("rotation {t}: {e:#}"))?;
        for off in &offsets {
            let rc = tc.translated(*off).map_err(|e| format!("{e:#}"))?;
            let s = swaps(&rc)?;
            let result = match sim_run(&rc, &s) {
                Err(why) => Err(why),
                Ok((arms, kept, misses)) => {
                    if *t == 0 && *off == [0, 0, 0] {
                        for (k, m) in misses.into_iter().enumerate() {
                            if repaired.is_ok() && !m.is_some_and(|w| sim_repair(w, &s[k])) {
                                repaired = Err(format!("piston {}: after a miss and the repair, no press works for {REPAIR_WINDOWS} windows", k + 1));
                            }
                        }
                    }
                    Ok(BuildOk { outcome: judge(n, &arms, kept, SPAN), dims: dims(&rc), family: family.clone() })
                }
            };
            builds.push(Build { turns: *t, offset: *off, result });
        }
    }
    let rep = report(c, kind, builds);
    if !rep.pass {
        return Ok(rep);
    }
    Ok(finish(rep, repaired))
}

/// `mctas circuit run`: one build, the game's or the twin's.
fn run_kind(pool: Option<&Pool>, c: &Circuit, at: &RunAt) -> Result<Outcome> {
    if at.run != "base" {
        bail!("bedrock-breaker: no run {:?} (base)", at.run);
    }
    let (_, n) = kind_n(c)?;
    match pool {
        None => {
            let rc = c.turned_pure(at.turns)?.translated(at.offset)?;
            let s = swaps(&rc).map_err(|e| anyhow!(e))?;
            let (arms, kept, _) = sim_run(&rc, &s).map_err(|e| anyhow!(e))?;
            Ok(judge(n, &arms, kept, SPAN))
        }
        Some(pool) => {
            let (turns, offset) = (at.turns, at.offset);
            let r = pool.map(vec![()], |w, _, _| {
                let rc = c.rotated(w, turns)?.translated(offset)?;
                game_build(w, &rc, n)
            });
            match r.into_iter().next().unwrap_or_else(|| Err(anyhow!("no worker")))? {
                Ok(k) => Ok(k.outcome),
                Err(why) => bail!(why),
            }
        }
    }
}
