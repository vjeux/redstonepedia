//! `mobfarm drive-replay`: redsim's side of tick-exact's game replay (ironspec 09:21 / 09:25 (a), the lane-timing
//! reconciliation; G1's 12:19 review items 1-11; cc767b78). A bench's drive (`--drive-out`) played OPEN LOOP on its
//! scene (`--scene-out`), as tick-exact's game replayer plays it: `Run::start` with the drive's options at `--day0`,
//! the settle run out, the scene's entity lines spawned in file order, `t0` checked against the drive's; each act at
//! `t` applied after `t` ticks, before the next (`drive::Cursor`). The scene as the game summons it: each entity's
//! data verbatim; its `port home:` line (redsim's bench-side HOME injection) dropped. `--noai` sets every villager
//! NoAI right after the summon (the game replayer's `MCTAS_REPLAY_NOAI=1`). `--offset DX,DY,DZ` runs it in the
//! game's frame (the game replayer holds the scene at y + 80; positions near y 102 round differently from near
//! y 22, which a touching-box push turns on): blocks, block entities, output, box, Nether anchor, entity positions
//! and the drive's act positions moved, x and z by whole chunks, a round trip checked; rows printed back in the
//! design frame. The entities' data isn't moved (as the game replayer leaves it), so a scene whose entity data
//! carries block positions (Brain memories) is refused with `--offset` unless `--noai` freezes those brains.
//!
//! It fails (a non-zero exit, each failure named) on: a compared row that differs (motions by their bits, printed
//! in full on both sides, a zero's sign aside as the game rows print zero unsigned; positions within 5e-7 + 1 ULP,
//! the game rows' 6-decimal print; NaN never matches), a traced row with no game row, a game row the trace doesn't
//! cover, a gone entity on one side only, a drive `# expect` line (f1q2-run's recorded column / hold / pocket state
//! at a write) whose state differs, a requested gap with no rows, and a push-off run that skipped no push. Vacuous
//! or out-of-run options are refused before the run.

use anyhow::{Context, Result, anyhow, bail, ensure};
use redsim::entity::EntityKind;
use redsim::entity::mob::drive::{Cursor, Drive};
use redsim::entity::mob::farm::{Run, RunOpts};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// The replay's options.
#[derive(Clone)]
pub struct Opts {
    pub scene: PathBuf,
    pub drive: PathBuf,
    /// Run ticks after t0.
    pub ticks: i64,
    /// The day time at t0 (the settle ends there): tick-exact's `--day-start`, a bench's `RunOpts::day0`. None:
    /// the drive's own `# day0` line gives it (a drive with neither is refused; a contradicting one too).
    pub day0: Option<i64>,
    pub noai: bool,
    /// Traced entity lines and their run-tick windows: (line N, from, to).
    pub trace: Vec<(usize, i64, i64)>,
    pub pushes: bool,
    /// The game replayer's rows to compare against (tick-exact's trace file).
    pub compare: Option<PathBuf>,
    /// The position tolerance beside 1 ULP (the game rows print positions to 6 decimals: 5e-7).
    pub pos_tol: f64,
    /// The ticks at which the queued pairs' gaps print ([`gaps`]).
    pub gaps_at: Vec<i64>,
    /// The frame the run uses (the scene and the drive moved by it; rows printed back): the game's (0, 80, 0)
    /// for tick-exact's replays.
    pub offset: [i32; 3],
    /// "rider doPush onto carts: off" (`MobWorld::rider_cart_push_off`, a counterfactual with no game twin).
    pub push_off: bool,
    /// tick-exact's `MCTAS_REPLAY_HOLDCART="x,y,z;T0;T1"` (the lift-top held-cart row): an EMPTY minecart
    /// summoned at that design-frame position with no motion at act time T0, put back there at rest at every act
    /// time after it before T1, and removed at T1.
    pub hold_cart: Option<([f64; 3], i64, i64)>,
    /// Design-frame cells whose visits (each entity line's box meeting the cell, first to last tick) are recorded.
    pub pass_cells: Vec<[i32; 3]>,
    /// The entity lines that are nappers (the rest batch carts), for day-1's margins.
    pub nappers: Vec<usize>,
    /// Keep the scene's `port home:` line (redsim's bench-side HOME injection, which the game replayer
    /// doesn't know): the recording's own setup, which its `# expect` lines need.
    pub keep_port_home: bool,
    /// (G1 15:17 N10) A prefix of the drive on purpose: its acts and `# expect` lines past `ticks` are counted
    /// and named, not failures.
    pub partial: bool,
    /// The run is [`push_shift`]'s on/off pair (its own outputs; G1 15:17 N11: the flags it would drop refused).
    pub push_shift: bool,
}

/// `x,y,z;x,y,z;..`: the watched cells (design frame).
pub fn parse_cells(s: &str) -> Result<Vec<[i32; 3]>> {
    let mut out = Vec::new();
    for c in s.split(';').map(str::trim).filter(|c| !c.is_empty()) {
        let v: Vec<i32> = c.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("--pass-cells {c}: x,y,z"))?;
        ensure!(v.len() == 3, "--pass-cells {c}: x,y,z");
        ensure!(!out.contains(&[v[0], v[1], v[2]]), "--pass-cells {c}: given twice");
        out.push([v[0], v[1], v[2]]);
    }
    Ok(out)
}

/// `X,Y,Z;T0;T1` (a finite design-frame position, then the act times, T0 before T1).
pub fn parse_hold_cart(s: &str) -> Result<Option<([f64; 3], i64, i64)>> {
    if s.trim().is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = s.split(';').map(str::trim).collect();
    ensure!(parts.len() == 3, "--hold-cart {s}: X,Y,Z;T0;T1");
    let p: Vec<f64> = parts[0].split(',').map(|v| v.trim().parse::<f64>()).collect::<std::result::Result<_, _>>().with_context(|| format!("--hold-cart {s}: the position"))?;
    ensure!(p.len() == 3 && p.iter().all(|c| c.is_finite()), "--hold-cart {s}: a finite X,Y,Z");
    let (t0, t1): (i64, i64) = (parts[1].parse()?, parts[2].parse()?);
    ensure!(0 <= t0 && t0 < t1, "--hold-cart {s}: 0 <= T0 < T1");
    Ok(Some(([p[0], p[1], p[2]], t0, t1)))
}

/// `eN[,eN...]:A-B[;...]` (or `N`): the traced lines and their run-tick windows; one `e` at most, no line twice.
pub fn parse_trace(s: &str) -> Result<Vec<(usize, i64, i64)>> {
    let mut out: Vec<(usize, i64, i64)> = Vec::new();
    for part in s.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (who, win) = part.split_once(':').ok_or_else(|| anyhow!("--trace {part}: eN[,eN..]:FROM-TO"))?;
        let (a, b) = win.split_once('-').ok_or_else(|| anyhow!("--trace {part}: the window FROM-TO"))?;
        let (a, b): (i64, i64) = (a.trim().parse().with_context(|| format!("--trace {part}: FROM"))?, b.trim().parse().with_context(|| format!("--trace {part}: TO"))?);
        ensure!(0 <= a && a <= b, "--trace {part}: 0 <= FROM <= TO");
        for e in who.split(',').map(str::trim) {
            let digits = e.strip_prefix('e').unwrap_or(e);
            ensure!(!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()), "--trace {part}: {e} is no line eN");
            let n: usize = digits.parse()?;
            ensure!(!out.iter().any(|&(m, ..)| m == n), "--trace {part}: e{n} traced twice");
            out.push((n, a, b));
        }
    }
    Ok(out)
}

/// A file's MD5, as hex.
fn md5_hex(path: &std::path::Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(redsim::stems::md5(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

/// Each entity's drive name: line N's entity `eN`, its passenger K `eN.pK` (after a reload, the new ids).
fn names(w: &redsim::world::World, lines: &[u32]) -> HashMap<u32, String> {
    let now = |id: u32| w.renumbered.get(&id).copied().unwrap_or(id);
    let mut m = HashMap::new();
    for (n, id) in lines.iter().enumerate() {
        let id = now(*id);
        m.insert(id, format!("e{n}"));
        if let Some(e) = w.entities.list.iter().find(|e| e.id == id && !e.removed) {
            for (k, p) in e.passengers.iter().enumerate() {
                m.insert(*p, format!("e{n}.p{k}"));
            }
        }
    }
    m
}

/// One traced row, after the tick at run tick `t`, in tick-exact's row's fields (positions back in the design
/// frame: the run's frame minus `off`); `tT: eN gone` once the entity is removed.
pub fn row(w: &redsim::world::World, lines: &[u32], n: usize, t: i64, off: [i32; 3]) -> String {
    let now = |id: u32| w.renumbered.get(&id).copied().unwrap_or(id);
    let Some(id) = lines.get(n).map(|id| now(*id)) else { return format!("t{t}: e{n} (no such line)") };
    let Some(e) = w.entities.list.iter().find(|e| e.id == id && !e.removed) else { return format!("t{t}: e{n} gone") };
    let (p, v) = (e.pos, e.delta_movement);
    let rider = e.passengers.first().map_or("-".to_string(), |_| format!("e{n}p0"));
    let feet = e.block_position();
    format!(
        "t{t}: e{n} pos ({:.6},{:.6},{:.6}) motion [{:?}d, {:?}d, {:?}d] rider {rider} on_ground {} block {} at ({},{},{})",
        p.x - off[0] as f64,
        p.y - off[1] as f64,
        p.z - off[2] as f64,
        v.x,
        v.y,
        v.z,
        e.on_ground,
        w.get(feet),
        feet.x - off[0],
        feet.y - off[1],
        feet.z - off[2]
    )
}

/// A traced entity's state after the tick: position (back in the design frame) and motion; None once gone.
fn state(w: &redsim::world::World, lines: &[u32], n: usize, off: [i32; 3]) -> Option<[f64; 6]> {
    let id = lines.get(n).map(|id| w.renumbered.get(id).copied().unwrap_or(*id))?;
    let e = w.entities.list.iter().find(|e| e.id == id && !e.removed)?;
    Some([e.pos.x - off[0] as f64, e.pos.y - off[1] as f64, e.pos.z - off[2] as f64, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z])
}

/// The scene moved by `off`: blocks, block entities, the output, the box, the Nether anchor and the entities'
/// positions; their data verbatim, as the game replayer summons it. x and z only by whole chunks.
fn shift_spec(spec: &mut redsim::entity::mob::farm::FarmSpec, off: [i32; 3]) -> Result<()> {
    ensure!(off[0] % 16 == 0 && off[2] % 16 == 0, "--offset {off:?}: x and z by whole chunks (multiples of 16)");
    let mv = |p: redsim::pos::BlockPos| p.offset(off[0], off[1], off[2]);
    for (p, _) in spec.blocks.iter_mut() {
        *p = mv(*p);
    }
    for (p, _) in spec.block_entities.iter_mut() {
        *p = mv(*p);
    }
    spec.output = mv(spec.output);
    spec.lo = mv(spec.lo);
    spec.hi = mv(spec.hi);
    if let Some(n) = spec.nether.as_mut() {
        n.anchor = mv(n.anchor);
    }
    for (_, pos, _) in spec.entities.iter_mut() {
        for (k, c) in pos.iter_mut().enumerate() {
            *c += off[k] as f64;
        }
    }
    Ok(())
}

/// The drive moved by `off` (its block and entity positions; its chunk boxes by whole chunks).
fn shift_drive(d: &mut Drive, off: [i32; 3]) {
    use redsim::entity::mob::drive::Act;
    let (cx, cz) = (off[0] / 16, off[2] / 16);
    for (_, a) in d.acts.iter_mut() {
        match a {
            Act::Set(p, _) | Act::Home(_, p) | Act::PoiTake(p) => *p = p.offset(off[0], off[1], off[2]),
            Act::Tp(_, v) => *v = v.add(off[0] as f64, off[1] as f64, off[2] as f64),
            Act::UnloadSome(b) | Act::LoadSome(b) => *b = [b[0] + cx, b[1] + cz, b[2] + cx, b[3] + cz],
            Act::LastSlept(..) | Act::Unhome(_) | Act::Forget(_) | Act::Unload | Act::Load | Act::LoadOrdered(_) => {}
        }
    }
}

/// The offset's round trip (G1 12:19 (6)): the scene and the drive moved by `off` and back are what they were.
fn offset_round_trip(spec: &redsim::entity::mob::farm::FarmSpec, d: &Drive, off: [i32; 3]) -> Result<()> {
    let back = [-off[0], -off[1], -off[2]];
    let (mut s2, mut d2) = (spec.clone(), d.clone());
    shift_spec(&mut s2, off)?;
    shift_spec(&mut s2, back)?;
    shift_drive(&mut d2, off);
    shift_drive(&mut d2, back);
    ensure!(format!("{s2:?}") == format!("{spec:?}"), "--offset {off:?}: the scene's round trip differs");
    ensure!(format!("{:?}", d2.acts) == format!("{:?}", d.acts), "--offset {off:?}: the drive's round trip differs");
    Ok(())
}

/// Every villager of the run NoAI (after the summon, as tick-exact's replayer merges `NoAI:1b`).
fn set_noai(w: &mut redsim::world::World) -> usize {
    let mut n = 0;
    for e in w.entities.list.iter_mut().filter(|e| !e.removed) {
        if let EntityKind::Mob(m) = &mut e.kind
            && m.ty.def().id == "villager"
        {
            m.no_ai = true;
            n += 1;
        }
    }
    n
}

/// One of the drive's `# expect t x y z: carts ..; holds ..; pockets ..` lines (f1q2-run's recorded state at a
/// write: the carts in the written cell's column, each hold's (x,16,0) and pocket's (x,0,0) cart, by drive name).
#[derive(Clone, Debug, PartialEq)]
pub struct Expect {
    pub t: i64,
    pub cell: [i32; 3],
    pub state: String,
}

/// The drive's `# expect` lines (other comments ignored; a malformed `# expect` line refused).
pub fn parse_expects(text: &str) -> Result<Vec<Expect>> {
    let mut out = Vec::new();
    for l in text.lines() {
        let Some(rest) = l.strip_prefix("# expect ") else { continue };
        // (the drive's own legend line, `# expect t: ...`, is no recorded state)
        if rest.starts_with("t:") {
            continue;
        }
        let (head, state) = rest.split_once(": ").ok_or_else(|| anyhow!("a malformed expect line: {l}"))?;
        let v: Vec<i64> = head.split_whitespace().map(|x| x.parse::<i64>()).collect::<std::result::Result<_, _>>().with_context(|| format!("a malformed expect line: {l}"))?;
        ensure!(v.len() == 4, "a malformed expect line: {l}");
        out.push(Expect { t: v[0], cell: [v[1] as i32, v[2] as i32, v[3] as i32], state: state.to_string() });
    }
    Ok(out)
}

/// redsim's state in f1q2-run's `drive_expect` form at the written cell `cell` (design frame; the run's frame
/// is `off` away), the holds and pockets of `lanes`.
fn expect_state(w: &redsim::world::World, lines: &[u32], cell: [i32; 3], off: [i32; 3], lanes: &[i32]) -> String {
    let named = names(w, lines);
    let name = |id: u32| named.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
    let carts_at = |c: [i32; 3]| -> Vec<String> {
        w.entities
            .list
            .iter()
            .filter(|e| {
                let b = e.block_position();
                !e.removed && matches!(e.kind, EntityKind::Minecart(_)) && [b.x - off[0], b.y - off[1], b.z - off[2]] == c
            })
            .map(|e| format!("{}{}", name(e.id), e.passengers.first().map_or(" (empty)".to_string(), |r| format!(" with {}", name(*r)))))
            .collect()
    };
    let at: Vec<String> = (-1..=1).flat_map(|dy| carts_at([cell[0], cell[1] + dy, cell[2]]).into_iter().map(move |s| format!("{s} at y{dy:+}"))).collect();
    let occ = |y: i32| {
        lanes
            .iter()
            .map(|x| {
                let v = carts_at([*x, y, 0]);
                format!("{x}:{}", if v.is_empty() { "none".to_string() } else { v.join("+") })
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("carts {}; holds {}; pockets {}", if at.is_empty() { "none".to_string() } else { at.join(", ") }, occ(16), occ(0))
}

/// The lanes an expect line's holds name (`holds -1:e3 with e3.p0 3:none 7:..`: the tokens `lane:...`).
fn expect_lanes(state: &str) -> Result<Vec<i32>> {
    let holds = state.split("; holds ").nth(1).and_then(|s| s.split("; pockets ").next()).ok_or_else(|| anyhow!("an expect state without holds: {state}"))?;
    let lanes: Vec<i32> = holds.split_whitespace().filter_map(|tok| tok.split_once(':').and_then(|(x, _)| x.parse::<i32>().ok())).collect();
    ensure!(!lanes.is_empty(), "an expect state's holds name no lane: {state}");
    Ok(lanes)
}

/// A replay's output: its lines, each entity line's visits to the watched cells, and its failures (each named;
/// the CLI exits non-zero on any).
pub struct Replay {
    pub lines: Vec<String>,
    pub visits: Vec<Visit>,
    pub failures: Vec<String>,
}

/// The options checked before the run (G1 12:19 (2), (5), (8), (11)): nothing vacuous, nothing outside the run.
fn validate(o: &Opts) -> Result<()> {
    ensure!(o.ticks > 0, "--ticks {}: at least 1", o.ticks);
    for &(n, a, b) in &o.trace {
        ensure!(b < o.ticks, "--trace e{n}:{a}-{b}: past the run's last row t{}", o.ticks - 1);
    }
    ensure!(o.compare.is_none() || !o.trace.is_empty(), "--compare needs --trace (the rows it compares)");
    ensure!(o.gaps_at.is_empty() || o.trace.len() >= 2, "--gaps-at needs two or more traced lines");
    for &t in &o.gaps_at {
        ensure!(o.trace.iter().filter(|&&(_, a, b)| (a..=b).contains(&t)).count() >= 2, "--gaps-at {t}: fewer than two traced lines have a row there");
    }
    ensure!(!(o.push_off && o.compare.is_some()), "--push-off is a counterfactual with no game twin: no --compare");
    ensure!(o.pos_tol.is_finite() && o.pos_tol >= 0.0, "--pos-tol {}: finite and not negative", o.pos_tol);
    if let Some((_, h0, h1)) = o.hold_cart {
        ensure!(h1 < o.ticks, "--hold-cart {h0};{h1}: its removal past the run's last act time {}", o.ticks - 1);
    }
    for &n in &o.nappers {
        ensure!(!o.pass_cells.is_empty(), "--nappers e{n} without --pass-cells");
    }
    // (G1 15:17 N11: the on/off pair prints its own comparison; a flag it would override or drop is refused)
    if o.push_shift {
        for (on, f) in [(o.push_off, "--push-off"), (!o.trace.is_empty(), "--trace"), (o.pushes, "--pushes"), (!o.gaps_at.is_empty(), "--gaps-at"), (o.compare.is_some(), "--compare")] {
            ensure!(!on, "--push-shift runs its own on/off pair: {f} would be overridden or dropped");
        }
        ensure!(!o.pass_cells.is_empty(), "--push-shift needs --pass-cells (the cells whose entries it compares)");
    }
    Ok(())
}

/// The replay.
pub fn run(o: &Opts) -> Result<Replay> {
    validate(o)?;
    let text = std::fs::read_to_string(&o.drive).with_context(|| format!("reading {}", o.drive.display()))?;
    let mut d = Drive::parse(&text).map_err(|e| anyhow!("{}: {e}", o.drive.display()))?;
    let expects = parse_expects(&text)?;
    let (scene_md5, drive_md5) = (md5_hex(&o.scene)?, md5_hex(&o.drive)?);
    let game_md5 = o.compare.as_ref().map(|f| md5_hex(f)).transpose()?;
    let mut design = mctas::design::Design::load(&o.scene)?;
    // (the game replayer knows no `port home:` line, so it is dropped unless --keep-port-home asks for
    // the recording's own setup: redsim's bench applied it, so the drive's `# expect` lines need it)
    let dropped_home = !o.keep_port_home && design.ports.remove("home").is_some();
    if !o.keep_port_home {
        design.meta.remove("port home");
    }
    let c = circuit::Circuit::from_design(design).with_context(|| format!("scene {}", o.scene.display()))?;
    let mut spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
    let off = o.offset;
    if off != [0, 0, 0] {
        // (G1 12:19 (10), ironspec 15:05 (b): the entities' data isn't moved, so a block position in it (a Brain
        // memory, a raider's patrol target, any int-array position) would point into the unmoved frame; --noai
        // freezes villagers only, so only a villager's Brain memory positions pass with it, and none without it)
        let int_arrays = |t: &str| t.matches("[I;").count();
        let brain_positions = |t: &str| if t.contains("minecraft:villager") && t.contains("Brain:{memories:") { t.matches("pos:[I;").count() } else { 0 };
        let unfrozen: Vec<&str> = spec.entities.iter().filter(|(_, _, nbt)| nbt.as_deref().is_some_and(|t| int_arrays(t) > if o.noai { brain_positions(t) } else { 0 })).map(|(ty, ..)| ty.as_str()).collect();
        ensure!(
            unfrozen.is_empty(),
            "--offset {off:?}: {} scene entities carry int-array positions in their data, which the offset doesn't move{}",
            unfrozen.len(),
            if o.noai { " and --noai doesn't freeze (it freezes villagers' Brain memories only)" } else { " (with --noai, villagers' Brain memories are frozen; any other is refused)" }
        );
        offset_round_trip(&spec, &d, off)?;
        shift_spec(&mut spec, off)?;
        shift_drive(&mut d, off);
    }
    // (the day start: the drive's own `# day0`, or --day0 when it has none; both given must agree; neither is
    // refused: onechunk's seed-1 pair differed in 238 of 3,760 expects at 11000 and in none at its 10778)
    let day0 = match (d.day0, o.day0) {
        (Some(a), Some(b)) => {
            ensure!(a == b, "--day0 {b} contradicts the drive's `# day0 {a}`");
            a
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => bail!("the drive gives no `# day0` and no --day0 was given: the day start decides the run (give the recording's own)"),
    };
    let opts = RunOpts { day0, ..d.opts() };
    let mut run = Run::start(&spec, &opts).map_err(|e| anyhow!(e))?;
    while run.settle_tick() {}
    let lines = redsim::entity::mob::drive::spawn_lines(&mut run, &spec).map_err(|e| anyhow!(e))?;
    let t0 = run.world.time;
    // (G1 15:17 H12: the recording's t0 required, and the day at t0 checked)
    let want = d.t0.ok_or_else(|| anyhow!("the drive gives no `# t0`: the replay's settle can't be checked against the recording's"))?;
    ensure!(want == t0, "the replay's t0 {t0} is not the drive's {want} (its settle differs)");
    ensure!(run.world.day_time() == day0, "the replay's day time at t0 is {}, not day0 {day0}", run.world.day_time());
    for &(n, ..) in &o.trace {
        ensure!(n < lines.len(), "--trace e{n}: the scene has {} entity lines", lines.len());
    }
    for &n in &o.nappers {
        ensure!(n < lines.len(), "--nappers e{n}: the scene has {} entity lines", lines.len());
    }
    let noai = if o.noai { set_noai(&mut run.world) } else { 0 };
    ensure!(!o.noai || noai > 0, "--noai: the scene has no villager to freeze");
    // (a memory act on an entity that holds no villager brain: tick-exact's pre-aa63b0b25 re-emits name a zombie's
    // seat (`act 0 home e11.p0` on scene-f8's zombie boat); the game replayer that recorded those rows ignored it,
    // so it is dropped here, each one named)
    let mut dropped: Vec<String> = Vec::new();
    {
        use redsim::entity::mob::drive::{Act, Who};
        let w = &run.world;
        let name = |who: &Who| match who {
            Who::Line(n) => format!("e{n}"),
            Who::Rider(n, k) => format!("e{n}.p{k}"),
            Who::Id(id) => format!("id{id}"),
        };
        d.acts.retain(|(t, a)| {
            let who = match a {
                Act::Home(who, _) | Act::LastSlept(who, _) | Act::Unhome(who) | Act::Forget(who) => who,
                _ => return true,
            };
            let villager = redsim::entity::mob::drive::index_of(w, &lines, *who).is_some_and(|i| matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.brain.is_some()));
            if !villager {
                dropped.push(format!("act {t} on {}", name(who)));
            }
            villager
        });
    }
    run.world.mob.rider_cart_push_off = o.push_off;
    let mut out = Vec::new();
    if o.push_off {
        out.push("PUSH-OFF COUNTERFACTUAL (rider doPush onto carts: off; not the game, no game twin)".to_string());
    }
    out.push(
        format!(
            "drive-replay (cc767b78): scene {} (md5 {scene_md5}) drive {} (md5 {drive_md5}; {} acts, {} expect lines), seed {}, day {} at t0 {t0}, {} entity lines, {} ticks",
            o.scene.display(),
            o.drive.display(),
            d.acts.len(),
            expects.len(),
            d.seed,
            run.world.day_time(),
            lines.len(),
            o.ticks
        ) + &if o.noai { format!(", NoAI on {noai} villagers (after the summon)") } else { String::new() }
            + if dropped_home { ", the scene's `port home:` dropped (its villagers' own Brain data as summoned)" } else { "" }
            + &if off != [0, 0, 0] { format!(", run at the frame moved by ({}, {}, {}) (round trip checked; rows printed back in the design frame)", off[0], off[1], off[2]) } else { String::new() }
            + &if dropped.is_empty() { String::new() } else { format!("; dropped as the recording replayer ignored them (memory acts on no villager): {}", dropped.join(", ")) }
            + &game_md5.as_ref().map_or(String::new(), |m| format!("; the game rows' md5 {m}")),
    );
    if o.pushes {
        redsim::entity::minecart::log_cart_pushes(&mut run.world);
    }
    let mut failures: Vec<String> = Vec::new();
    let mut cur = Cursor::default();
    let mut held: Option<u32> = None;
    let mut visits: Vec<Visit> = Vec::new();
    let mut open: HashMap<(usize, usize), usize> = HashMap::new();
    // (the drive's `# expect` lines are redsim's own state at the recording, so they're judged when the replay
    // keeps the recording's conditions: live AI, the design frame, the game's pushes, no held cart)
    let changed: Vec<&str> = [(o.noai, "--noai"), (off != [0, 0, 0], "--offset"), (o.push_off, "--push-off"), (o.hold_cart.is_some(), "--hold-cart"), (dropped_home, "the scene's `port home:` dropped (--keep-port-home keeps it)")].into_iter().filter(|(on, _)| *on).map(|(_, f)| f).collect();
    let mut by_t: BTreeMap<i64, Vec<&Expect>> = BTreeMap::new();
    if changed.is_empty() {
        for e in &expects {
            by_t.entry(e.t).or_default().push(e);
        }
    }
    let mut expects_judged = 0usize;
    // (rows labelled as tick-exact's game replayer labels them: the row after the k-th tick is t = k - 1, so the
    // first row after the summon is t0; an act at t was applied after t ticks, before row t's tick)
    let mut mine: BTreeMap<(i64, usize), Option<[f64; 6]>> = BTreeMap::new();
    for _ in 0..o.ticks {
        let t_act = run.world.time - t0;
        // (the drive's expectations at this act time: f1q2-run's state at its writes, before them)
        for e in by_t.get(&t_act).map(Vec::as_slice).unwrap_or(&[]) {
            let lanes = expect_lanes(&e.state)?;
            let got = expect_state(&run.world, &lines, e.cell, off, &lanes);
            expects_judged += 1;
            if got != e.state {
                failures.push(format!("expect t{} at ({},{},{}): the drive recorded \"{}\", the replay has \"{got}\"", e.t, e.cell[0], e.cell[1], e.cell[2], e.state));
            }
        }
        cur.apply_due(&mut run.world, &lines, &d, t0);
        // (the held cart: summoned, held, removed at its act times, before the tick)
        if let Some((p, h0, h1)) = o.hold_cart {
            let at = [p[0] + off[0] as f64, p[1] + off[1] as f64, p[2] + off[2] as f64];
            if t_act == h0 {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", at, None).map_err(|e| anyhow!("--hold-cart: {e}"))?;
                held = Some(id);
                out.push(format!("t{t_act}: HOLDCART an empty minecart held at ({:.4},{:.4},{:.4}) (#{id})", p[0], p[1], p[2]));
            } else if t_act > h0 && t_act <= h1 {
                let i = held.and_then(|id| redsim::entity::index_of(&run.world, id)).ok_or_else(|| anyhow!("--hold-cart: the held cart vanished before act time {t_act}"))?;
                if t_act < h1 {
                    run.world.entities.set_pos(i, redsim::entity::Vec3::new(at[0], at[1], at[2]));
                    run.world.entities.list[i].delta_movement = redsim::entity::Vec3::ZERO;
                } else {
                    run.world.entities.list[i].removed = true;
                    held = None;
                    out.push(format!("t{t_act}: HOLDCART removed"));
                }
            }
        }
        run.tick();
        let t = run.world.time - t0 - 1;
        for &(n, a, b) in &o.trace {
            if (a..=b).contains(&t) {
                out.push(row(&run.world, &lines, n, t, off));
                mine.insert((t, n), state(&run.world, &lines, n, off));
            }
        }
        if o.pushes {
            let named = names(&run.world, &lines);
            let who = |id: u32| named.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
            for p in redsim::entity::minecart::take_cart_pushes(&mut run.world) {
                out.push(format!("t{t}: +push {} (its vehicle {}) -> {} [{:?}, 0.0, {:?}] dist {:.6}", who(p.mob), p.vehicle.map_or("none".to_string(), who), who(p.cart), p.push[0], p.push[1], p.dist));
            }
        }
        // (each entity line's visits to the watched cells, design frame: the ticks its box meets the cell's
        // volume, first to last, day-1's "its box in J's cell")
        if !o.pass_cells.is_empty() {
            for (n, id) in lines.iter().enumerate() {
                let id = run.world.renumbered.get(id).copied().unwrap_or(*id);
                let bb = run.world.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| e.bb);
                for (ci, c) in o.pass_cells.iter().enumerate() {
                    let (x, y, z) = ((c[0] + off[0]) as f64, (c[1] + off[1]) as f64, (c[2] + off[2]) as f64);
                    let inside = bb.is_some_and(|b| b.min_x < x + 1.0 && b.max_x > x && b.min_y < y + 1.0 && b.max_y > y && b.min_z < z + 1.0 && b.max_z > z);
                    match (inside, open.get(&(n, ci)).copied()) {
                        (true, Some(v)) => visits[v].last = t,
                        (true, None) => {
                            open.insert((n, ci), visits.len());
                            visits.push(Visit { cell: *c, line: n, first: t, last: t });
                        }
                        (false, _) => {
                            open.remove(&(n, ci));
                        }
                    }
                }
            }
        }
    }
    let differing = failures.iter().filter(|f| f.starts_with("expect ")).count();
    out.push(if !changed.is_empty() {
        format!("the drive's expect lines: {} not judged (the replay changes the recording's conditions: {})", expects.len(), changed.join(", "))
    } else {
        let unjudged = expects.len() - expects_judged;
        // (G1 15:17 N10: a drive replayed short says so, and fails unless --partial asks for a prefix)
        if unjudged > 0 && !o.partial {
            failures.push(format!("{unjudged} `# expect` lines past the run's {} ticks not judged: replay the whole drive, or say --partial", o.ticks));
        }
        format!("the drive's expect lines: {expects_judged} judged, {differing} differing{}", if unjudged > 0 { format!(", {unjudged} past the run's {} ticks (not judged{})", o.ticks, if o.partial { ": --partial" } else { "" }) } else { String::new() })
    });
    // (G1 15:17 N7: the drive's own `expect <t> <what>` / `expect-end <what>` lines are a bench's words this replay
    // can't read: counted, named NOT JUDGED, and failing)
    let bench = d.expects.len() + d.ends.len();
    if bench > 0 {
        out.push(format!("the drive's own expect lines: {} `expect` and {} `expect-end` (a bench's words): NOT JUDGED by this replay", d.expects.len(), d.ends.len()));
        failures.push(format!("{bench} drive `expect` / `expect-end` lines not judged (this replay judges `# expect` lines only)"));
    }
    // (G1 15:17 N10: the acts past the run, counted and named; failing unless --partial)
    let past = d.acts.len() - cur.next;
    if past > 0 {
        let first = d.acts[cur.next].0;
        out.push(format!("{past} acts past the run's {} ticks (not applied; the first at t{first}){}", o.ticks, if o.partial { ": --partial" } else { "" }));
        if !o.partial {
            failures.push(format!("{past} acts past the run's {} ticks not applied (the first at t{first}): replay the whole drive, or say --partial", o.ticks));
        }
    }
    if o.push_off {
        let skipped = run.world.mob.rider_cart_pushes_skipped;
        out.push(format!("PUSH-OFF COUNTERFACTUAL: {skipped} rider pushes onto carts skipped"));
        if skipped == 0 {
            failures.push("the push-off run skipped no rider push: it measured nothing".to_string());
        }
    }
    if let Some(f) = &o.compare {
        let game = parse_rows(&std::fs::read_to_string(f).with_context(|| format!("reading {}", f.display()))?)?;
        let (lines_out, fails) = compare(&mine, &game, o.pos_tol, f);
        out.extend(lines_out);
        failures.extend(fails);
        let (g_out, g_fails) = gaps(&mine, Some(&game), &o.gaps_at);
        out.extend(g_out);
        failures.extend(g_fails);
    } else if !o.gaps_at.is_empty() {
        let (g_out, g_fails) = gaps(&mine, None, &o.gaps_at);
        out.extend(g_out);
        failures.extend(g_fails);
    }
    for c in &o.pass_cells {
        let mut at: Vec<&Visit> = visits.iter().filter(|v| v.cell == *c).collect();
        at.sort_by_key(|v| (v.first, v.line));
        // (G1 15:17 N13: a watched cell no line visited measured nothing: a failure, like the push-off's 0 skips)
        if at.is_empty() {
            failures.push(format!("watched cell ({},{},{}): no entity line visited it (a vacuous watch)", c[0], c[1], c[2]));
        }
        let kind = |n: usize| if o.nappers.contains(&n) { "napper " } else { "" };
        out.push(format!("visits to ({},{},{}): {}", c[0], c[1], c[2], at.iter().map(|v| format!("{}e{} t{}..{}", kind(v.line), v.line, v.first, v.last)).collect::<Vec<_>>().join(", ")));
        if !o.nappers.is_empty() {
            for m in margins(&visits, *c, &o.nappers) {
                out.push(format!("  {m}"));
            }
        }
    }
    for &n in &o.nappers {
        if !visits.iter().any(|v| v.line == n) {
            failures.push(format!("--nappers e{n}: it visited no watched cell (no margin measured)"));
        }
    }
    out.push(if failures.is_empty() { "drive-replay: 0 failures".to_string() } else { format!("drive-replay: {} FAILURE(S): {}", failures.len(), failures.iter().take(12).cloned().collect::<Vec<_>>().join(" | ")) });
    Ok(Replay { lines: out, visits, failures })
}

/// One entity line's visit to a watched cell: the first and last tick its box meets the cell (design frame).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visit {
    pub cell: [i32; 3],
    pub line: usize,
    pub first: i64,
    pub last: i64,
}

/// (day-1 12:02: the intra-batch outbound margin per J) For each napper visit to `cell`: its last tick (a), the
/// first tick (b) of the next batch cart's visit (the first non-napper visit starting at or after the napper's
/// start), and the margin (b) - (a) - 4 (the junction's measured switch time); negative when the batch cart meets
/// the cell before the napper leaves it.
pub fn margin_values(visits: &[Visit], cell: [i32; 3], nappers: &[usize]) -> Vec<(usize, i64, Option<(usize, i64)>)> {
    let mut at: Vec<&Visit> = visits.iter().filter(|v| v.cell == cell).collect();
    at.sort_by_key(|v| (v.first, v.line));
    let mut out = Vec::new();
    for (k, v) in at.iter().enumerate() {
        if !nappers.contains(&v.line) {
            continue;
        }
        let next = at[k + 1..].iter().find(|w| !nappers.contains(&w.line)).map(|w| (w.line, w.first));
        out.push((v.line, v.last, next));
    }
    out
}

fn margins(visits: &[Visit], cell: [i32; 3], nappers: &[usize]) -> Vec<String> {
    margin_values(visits, cell, nappers)
        .into_iter()
        .map(|(n, a, next)| match next {
            Some((m, b)) => format!("napper e{n} last t{a}, next batch e{m} first t{b}: margin {} (b - a - 4)", b - a - 4),
            None => format!("napper e{n} last t{a}: no batch cart after it"),
        })
        .collect()
}

/// (G1 11:43 / ironspec 11:47 / day-1 12:02: the push → time shift) The same replay with the riders' cart pushes
/// on (the game) and off ("rider doPush onto carts: off", a counterfactual with no game twin): per watched cell,
/// each entity line's k-th visit (first and last tick) on and off and the shift of its first tick, the cell's
/// consecutive visits' gaps (one's first after the previous one's last) on and off, and with `--nappers` each
/// napper visit's margin to the next batch cart on and off. Open loop: past the first difference, the drive's acts
/// no longer match either run, so the pairing measures divergence, not the push.
pub fn push_shift(o: &Opts) -> Result<Replay> {
    ensure!(o.push_shift, "push_shift: the options' push_shift flag (validated as the pair's)");
    validate(o)?;
    let on = run(&Opts { push_off: false, push_shift: false, ..o.clone() })?;
    let off = run(&Opts { push_off: true, push_shift: false, ..o.clone() })?;
    let mut out = vec![format!("push shift (cc767b78; OPEN LOOP): the replay with the riders' cart pushes ON (the game) and OFF (rider doPush onto carts: off, a counterfactual with no game twin); {}", on.lines.iter().find(|l| l.starts_with("drive-replay (")).cloned().unwrap_or_default())];
    out.extend(off.lines.iter().filter(|l| l.starts_with("PUSH-OFF COUNTERFACTUAL:")).cloned());
    let stat = |v: &[i64]| -> String {
        if v.is_empty() {
            return "-".to_string();
        }
        let mut s = v.to_vec();
        s.sort();
        format!("min {} median {} max {} (n {})", s[0], s[s.len() / 2], s[s.len() - 1], s.len())
    };
    for c in &o.pass_cells {
        let of = |r: &Replay| {
            let mut v: Vec<Visit> = r.visits.iter().filter(|v| v.cell == *c).copied().collect();
            v.sort_by_key(|v| (v.first, v.line));
            v
        };
        let (a, b) = (of(&on), of(&off));
        let mut shifts: Vec<i64> = Vec::new();
        let mut rows: Vec<String> = Vec::new();
        let mut seen: HashMap<usize, usize> = HashMap::new();
        for v in &a {
            let k = *seen.entry(v.line).and_modify(|k| *k += 1).or_insert(0);
            match b.iter().filter(|w| w.line == v.line).nth(k) {
                Some(w) => {
                    shifts.push(v.first - w.first);
                    rows.push(format!("e{}#{k} on t{}..{} off t{}..{} ({:+})", v.line, v.first, v.last, w.first, w.last, v.first - w.first));
                }
                None => rows.push(format!("e{}#{k} on t{}..{} off -", v.line, v.first, v.last)),
            }
        }
        let gaps = |v: &[Visit]| v.windows(2).map(|w| w[1].first - w[0].last).collect::<Vec<i64>>();
        out.push(format!("({},{},{}): {} visits on, {} off; the first tick's shift on - off: {}; consecutive gaps (next first - last) on: {}; off: {}", c[0], c[1], c[2], a.len(), b.len(), stat(&shifts), stat(&gaps(&a)), stat(&gaps(&b))));
        out.push(format!("  {}", rows.join(", ")));
        if !o.nappers.is_empty() {
            let m = |r: &Replay| margin_values(&r.visits, *c, &o.nappers).into_iter().filter_map(|(_, a, next)| next.map(|(_, b)| b - a - 4)).collect::<Vec<i64>>();
            // (G1 15:17 N15: a napper visit with no later batch cart has no margin: counted, not dropped)
            let none = |r: &Replay| margin_values(&r.visits, *c, &o.nappers).iter().filter(|(_, _, next)| next.is_none()).count();
            out.push(format!("  napper margins (b - a - 4) on: {}; off: {}; napper visits with no later batch cart (no margin) on {}, off {}", stat(&m(&on)), stat(&m(&off)), none(&on), none(&off)));
            for line in margins(&on.visits, *c, &o.nappers) {
                out.push(format!("    on: {line}"));
            }
            for line in margins(&off.visits, *c, &o.nappers) {
                out.push(format!("    off: {line}"));
            }
        }
    }
    let mut failures = on.failures;
    failures.extend(off.failures);
    Ok(Replay { lines: out, visits: Vec::new(), failures })
}

/// The queued pairs' gaps at the given ticks (ironspec 10:01): for each tick, each pair of consecutive traced lines
/// (by line number), their position differences (dy, dz) and whether both were at rest, redsim's beside the
/// game's when its rows are given; a requested tick without both rows on a side fails.
pub fn gaps(mine: &BTreeMap<(i64, usize), Option<[f64; 6]>>, game: Option<&BTreeMap<(i64, usize), Option<[f64; 6]>>>, at: &[i64]) -> (Vec<String>, Vec<String>) {
    let mut out = Vec::new();
    let mut failures = Vec::new();
    if at.is_empty() {
        return (out, failures);
    }
    let mut lines: Vec<usize> = mine.keys().map(|&(_, n)| n).collect();
    lines.sort();
    lines.dedup();
    out.push("the queued pairs' gaps (dy, dz = the later line's position minus the earlier's; rest = both motions 0):".to_string());
    let rest = |v: &[f64; 6]| v[3] == 0.0 && v[4] == 0.0 && v[5] == 0.0;
    let side = |rows: &BTreeMap<(i64, usize), Option<[f64; 6]>>, t: i64, a: usize, b: usize| match (rows.get(&(t, a)).copied().flatten(), rows.get(&(t, b)).copied().flatten()) {
        (Some(va), Some(vb)) => Some(format!("dy {:.9} dz {:.9}{}", vb[1] - va[1], vb[2] - va[2], if rest(&va) && rest(&vb) { " rest" } else { " moving" })),
        _ => None,
    };
    for &t in at {
        for pair in lines.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let r = side(mine, t, a, b);
            let g = game.map(|g| side(g, t, a, b));
            if r.is_none() || g.as_ref().is_some_and(|g| g.is_none()) {
                failures.push(format!("gap t{t} e{a}-e{b}: no rows for both lines"));
            }
            let g = g.map_or(String::new(), |g| format!("; game {}", g.unwrap_or_else(|| "-".to_string())));
            out.push(format!("  t{t} e{a}-e{b}: redsim {}{g}", r.unwrap_or_else(|| "-".to_string())));
        }
    }
    (out, failures)
}

/// The game replayer's rows (`tT: eN pos (x,y,z) motion [dx d, dy d, dz d] …`, or `tT: eN gone`): (t, line) →
/// state (None: gone). Lines that aren't rows (headers, `tT: HOLDCART ...`) are skipped; a row line that doesn't
/// parse, or a (t, line) given twice, is refused.
pub fn parse_rows(text: &str) -> Result<BTreeMap<(i64, usize), Option<[f64; 6]>>> {
    let mut out = BTreeMap::new();
    for l in text.lines() {
        let l = l.trim();
        let Some((t, rest)) = l.strip_prefix('t').and_then(|r| r.split_once(": ")) else { continue };
        let Ok(t) = t.parse::<i64>() else { continue };
        // (a row names a line: `eN` then its fields)
        let Some(after) = rest.strip_prefix('e') else { continue };
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            continue;
        }
        let n: usize = digits.parse()?;
        let fields = after[digits.len()..].trim_start();
        let st = if fields == "gone" {
            None
        } else {
            let num = |s: &str| -> Result<f64> { s.trim().strip_suffix('d').unwrap_or(s.trim()).parse::<f64>().with_context(|| format!("a game row's number {s:?}: {l}")) };
            let pos = fields.strip_prefix("pos (").and_then(|r| r.split_once(')')).map(|(p, _)| p).ok_or_else(|| anyhow!("a game row without pos: {l}"))?;
            let mot = fields.split_once("motion [").and_then(|(_, r)| r.split_once(']')).map(|(m, _)| m).ok_or_else(|| anyhow!("a game row without motion: {l}"))?;
            let p: Vec<f64> = pos.split(',').map(num).collect::<Result<_>>()?;
            let m: Vec<f64> = mot.split(',').map(num).collect::<Result<_>>()?;
            ensure!(p.len() == 3 && m.len() == 3, "a game row needs three position and three motion components: {l}");
            Some([p[0], p[1], p[2], m[0], m[1], m[2]])
        };
        ensure!(out.insert((t, n), st).is_none(), "the game rows give t{t} e{n} twice");
    }
    Ok(out)
}

/// The last place's size of `x` (its ULP).
fn ulp(x: f64) -> f64 {
    let a = x.abs();
    f64::from_bits(a.to_bits() + 1) - a
}

/// The replay's rows against the game's (G1 12:19 (1)-(4), 15:17 N3 / N4): every traced row needs a game row and
/// every game row a traced row; motions match by their bits, a zero's sign aside (no game row can show one:
/// `DoubleTag.valueOf`, net/minecraft/nbt/DoubleTag 46–48, `return data == 0.0 ? ZERO : new DoubleTag(data);` with
/// `ZERO = new DoubleTag(0.0)` at line 10, turns every −0.0 into +0.0 on its way through `data get`, and the game
/// replayer prints the reply as it comes, crates/mobai/src/drivereplay.rs:1714 on tick-exact's tree; tick-exact
/// 15:29), and a NaN on either side never matches; positions within `pos_tol` + 1 ULP of the game's (its
/// 6-decimal print), NaN never; a gone entity matches only a gone one. Per line: the rows compared, the mismatches
/// and the first one; each mismatch, missing or extra row a failure.
pub fn compare(mine: &BTreeMap<(i64, usize), Option<[f64; 6]>>, game: &BTreeMap<(i64, usize), Option<[f64; 6]>>, pos_tol: f64, file: &std::path::Path) -> (Vec<String>, Vec<String>) {
    let names = ["x", "y", "z", "vx", "vy", "vz"];
    // (the zero's sign: see above, DoubleTag 46–48)
    let zero = |x: f64| if x == 0.0 { 0.0f64 } else { x };
    let mut out = vec![format!("the comparison against {} ({} game rows): motions by bits, positions within {pos_tol:e} + 1 ULP:", file.display(), game.len())];
    let mut failures = Vec::new();
    let mut per: BTreeMap<usize, (usize, usize, Option<String>)> = BTreeMap::new();
    for (&(t, n), m) in mine {
        let entry = per.entry(n).or_insert((0, 0, None));
        let Some(g) = game.get(&(t, n)) else {
            failures.push(format!("e{n} t{t}: traced, but the game rows have no row there"));
            continue;
        };
        entry.0 += 1;
        let diff = match (m, g) {
            (None, None) => None,
            (Some(_), None) => Some("gone in the game only".to_string()),
            (None, Some(_)) => Some("gone in redsim only".to_string()),
            (Some(v), Some(gv)) => (0..6).find_map(|k| {
                let bad = if k < 3 { !((v[k] - gv[k]).abs() <= pos_tol + ulp(gv[k])) } else { v[k].is_nan() || gv[k].is_nan() || zero(v[k]).to_bits() != zero(gv[k]).to_bits() };
                bad.then(|| format!("{} redsim {:?} game {:?}", names[k], v[k], gv[k]))
            }),
        };
        if let Some(d) = diff {
            entry.1 += 1;
            if entry.2.is_none() {
                entry.2 = Some(format!("t{t}: {d}"));
            }
            failures.push(format!("e{n} t{t}: {d}"));
        }
    }
    for &(t, n) in game.keys() {
        if !mine.contains_key(&(t, n)) {
            failures.push(format!("game row e{n} t{t}: not traced (the trace must cover every game row)"));
        }
    }
    for (n, (compared, bad, first)) in per {
        out.push(format!("  e{n}: {compared} rows compared, {bad} differ{}", first.map_or(String::new(), |f| format!("; the first at {f}"))));
    }
    (out, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trace flag: lines with one optional `e`, several lines, several windows; `eee3`, a line twice, a
    /// window backwards or negative refused.
    #[test]
    fn trace_windows() {
        assert_eq!(parse_trace("e16:440-720").unwrap(), vec![(16, 440, 720)]);
        assert_eq!(parse_trace("e11,12:0-5;e3:7-7").unwrap(), vec![(11, 0, 5), (12, 0, 5), (3, 7, 7)]);
        for bad in ["e16:9-1", "e16", "x:1-2", "eee3:0-5", "e3:0-5;e3:7-9", "e3:-1-5", "e:0-5"] {
            assert!(parse_trace(bad).is_err(), "{bad} refused");
        }
    }

    /// The held cart's flag: `X,Y,Z;T0;T1` or nothing; a short position, NaN, T0 after T1, a negative T0 or a
    /// missing time refused.
    #[test]
    fn hold_cart_values() {
        assert_eq!(parse_hold_cart("").unwrap(), None);
        assert_eq!(parse_hold_cart("-4.5,23.0625,8.5;378;389").unwrap(), Some(([-4.5, 23.0625, 8.5], 378, 389)));
        for bad in ["-4.5,23.0625;378;389", "-4.5,23,8.5;389;378", "-4.5,23,8.5;378", "NaN,23,8.5;378;389", "-4.5,inf,8.5;378;389", "-4.5,23,8.5;-1;5"] {
            assert!(parse_hold_cart(bad).is_err(), "{bad} refused");
        }
    }

    /// The watched cells' flag: `x,y,z;..`, empty for none; a short cell, a non-number or a cell twice refused.
    #[test]
    fn pass_cell_values() {
        assert!(parse_cells("").unwrap().is_empty());
        assert_eq!(parse_cells("-1,25,9; 3,25,9").unwrap(), vec![[-1, 25, 9], [3, 25, 9]]);
        assert!(parse_cells("-1,25").is_err() && parse_cells("a,b,c").is_err() && parse_cells("1,2,3;1,2,3").is_err());
    }

    /// The game's rows parse strictly (a malformed row or a row twice refused; `gone` rows kept), and the
    /// comparison fails on a 1-ULP motion difference (named at its tick), a NaN, a gone entity on one side, a
    /// traced row with no game row and a game row the trace doesn't cover, and passes positions within the print.
    #[test]
    fn game_rows_and_the_comparison() {
        let game_text = "== header\n    t0: e12 pos (-0.500000,18.556641,2.494141) motion [0.0d, 0.0d, -0.00808203125d] rider e12p0\n    t1: e12 pos (-0.500000,18.544720,2.482220) motion [0.0d, 0.0d, -0.01644289257812475d] rider e12p0\n    t1: HOLDCART removed\n    t2: e12 gone\n";
        let game = parse_rows(game_text).unwrap();
        assert_eq!(game.len(), 3);
        assert_eq!(game[&(2, 12)], None);
        assert!(parse_rows("t0: e1 pos (1,2) motion [0.0d, 0.0d, 0.0d]").is_err(), "a short position refused");
        assert!(parse_rows("t0: e1 pos (1,2,3) motion [0.0d, x, 0.0d]").is_err(), "a bad number refused");
        assert!(parse_rows("t0: e1 gone\nt0: e1 gone").is_err(), "a row twice refused");
        // (redsim's rows: the game's positions 4e-7 off (within the 6-decimal print), its motions exact, -0.0 for 0.0)
        let mut mine: BTreeMap<(i64, usize), Option<[f64; 6]>> = BTreeMap::new();
        mine.insert((0, 12), Some([-0.5, 18.5566414, 2.494141, -0.0, -0.0, -0.00808203125]));
        mine.insert((1, 12), Some([-0.5, 18.54472, 2.48222, 0.0, 0.0, -0.01644289257812475]));
        mine.insert((2, 12), None);
        let f = std::path::Path::new("g");
        let (_, fails) = compare(&mine, &game, 5e-7, f);
        assert!(fails.is_empty(), "within the print, zeros unsigned: {fails:?}");
        let ulp_off = f64::from_bits((-0.01644289257812475f64).to_bits() + 1);
        let mut m2 = mine.clone();
        m2.insert((1, 12), Some([-0.5, 18.54472, 2.48222, 0.0, 0.0, ulp_off]));
        let (_, fails) = compare(&m2, &game, 5e-7, f);
        assert!(fails.len() == 1 && fails[0].starts_with("e12 t1: vz"), "1 ULP in a motion fails at its tick: {fails:?}");
        let mut m3 = mine.clone();
        m3.insert((0, 12), Some([-0.5, f64::NAN, 2.494141, 0.0, 0.0, -0.00808203125]));
        assert_eq!(compare(&m3, &game, 5e-7, f).1.len(), 1, "NaN never matches");
        // (G1 15:17 N3, ironspec 15:05 (a)) a NaN motion on BOTH sides, the same bits, still fails
        let nan_game = parse_rows("t0: e12 pos (-0.500000,18.556641,2.494141) motion [NaN, 0.0d, -0.00808203125d]").unwrap();
        let mut nan_mine: BTreeMap<(i64, usize), Option<[f64; 6]>> = BTreeMap::new();
        nan_mine.insert((0, 12), Some([-0.5, 18.556641, 2.494141, f64::NAN, 0.0, -0.00808203125]));
        let (_, fails) = compare(&nan_mine, &nan_game, 5e-7, f);
        assert!(fails.len() == 1 && fails[0].starts_with("e12 t0: vx"), "a NaN pair in vx fails: {fails:?}");
        // (G1 15:17 N5) the tolerance's edges: 5e-7 + 2 ULP off fails; a 1/128 tie printed to 6 decimals passes
        let edge = |mine_x: f64, game_x: &str| {
            let g = parse_rows(&format!("t0: e1 pos ({game_x},0.000000,0.000000) motion [0.0d, 0.0d, 0.0d]")).unwrap();
            let mut m: BTreeMap<(i64, usize), Option<[f64; 6]>> = BTreeMap::new();
            m.insert((0, 1), Some([mine_x, 0.0, 0.0, 0.0, 0.0, 0.0]));
            compare(&m, &g, 5e-7, f).1
        };
        let gx: f64 = "3.500000".parse().unwrap();
        let two_ulps = f64::from_bits((gx + 5e-7).to_bits() + 2);
        assert_eq!(edge(two_ulps, "3.500000").len(), 1, "5e-7 + 2 ULP off fails");
        assert!(edge(3.0078125, "3.007813").is_empty(), "a 1/128 tie (3.0078125 printed 3.007813) passes");
        assert!(edge(3.0078125, "3.007812").is_empty(), "and printed the other way, 3.007812");
        let mut m4 = mine.clone();
        m4.insert((2, 12), Some([-0.5, 18.5, 2.47, 0.0, 0.0, 0.0]));
        assert!(compare(&m4, &game, 5e-7, f).1[0].contains("gone in the game only"));
        let mut m5 = mine.clone();
        m5.remove(&(2, 12));
        m5.insert((3, 12), None);
        let (_, fails) = compare(&m5, &game, 5e-7, f);
        assert!(fails.iter().any(|x| x.contains("traced, but the game rows have no row")) && fails.iter().any(|x| x.contains("not traced")), "{fails:?}");
    }

    /// The drive's `# expect` lines parse (its legend skipped; a malformed one refused) and name their lanes.
    #[test]
    fn expect_lines() {
        let text = "# expect t: the event happened in the tick ...\n# expect 0 -1 26 9: carts none; holds -1:e3 with e3.p0 3:e4 with e4.p0 7:none; pockets -1:e0 with e0.p0 3:none 7:none\nact 0 set -1 26 9 minecraft:stone\n";
        let e = parse_expects(text).unwrap();
        assert_eq!(e, vec![Expect { t: 0, cell: [-1, 26, 9], state: "carts none; holds -1:e3 with e3.p0 3:e4 with e4.p0 7:none; pockets -1:e0 with e0.p0 3:none 7:none".to_string() }]);
        assert_eq!(expect_lanes(&e[0].state).unwrap(), vec![-1, 3, 7]);
        assert!(parse_expects("# expect 0 1 2: carts none").is_err());
    }

    /// The end-to-end control (G1 12:19 (9)): a replay written out as the "game" rows passes against itself with no
    /// failure, and the same rows with one motion moved 1 ULP fail, naming that tick; a --push-off run skips pushes
    /// and is refused with --compare.
    #[test]
    fn end_to_end_against_its_own_rows() {
        let dir = std::env::temp_dir().join(format!("cc767b78-drivereplay-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let scene = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/farms/iron/middle/carousel-gate.txt");
        let drive = dir.join("drive.txt");
        std::fs::write(&drive, "# drive v1\n# seed 1\n# days 1\n# walk random\n# ai on\n# t0 200\n").unwrap();
        let o = Opts { scene: scene.clone(), drive: drive.clone(), ticks: 40, day0: Some(11_000), noai: true, trace: vec![(1, 0, 39), (2, 0, 39)], pushes: false, compare: None, pos_tol: 5e-7, gaps_at: Vec::new(), offset: [0, 0, 0], push_off: false, hold_cart: None, pass_cells: Vec::new(), nappers: Vec::new(), keep_port_home: false, partial: false, push_shift: false };
        let r = run(&o).unwrap();
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        let rows: Vec<String> = r.lines.iter().filter(|l| l.starts_with('t') && l.contains(": e")).cloned().collect();
        assert_eq!(rows.len(), 80);
        let game = dir.join("game.txt");
        std::fs::write(&game, rows.join("\n")).unwrap();
        let r2 = run(&Opts { compare: Some(game.clone()), ..o.clone() }).unwrap();
        assert!(r2.failures.is_empty(), "its own rows pass: {:?}", r2.failures);
        // (one motion 1 ULP off in the game rows)
        let row = rows.iter().position(|l| l.starts_with("t20: e1 ")).unwrap();
        let l = &rows[row];
        let (head, tail) = l.split_once("motion [").unwrap();
        let first: f64 = tail.split('d').next().unwrap().parse().unwrap();
        let moved = f64::from_bits(first.to_bits() + 1);
        let mut planted = rows.clone();
        planted[row] = format!("{head}motion [{moved:?}d{}", &tail[tail.find('d').unwrap() + 1..]);
        std::fs::write(&game, planted.join("\n")).unwrap();
        let r3 = run(&Opts { compare: Some(game.clone()), ..o.clone() }).unwrap();
        assert!(r3.failures.len() == 1 && r3.failures[0].starts_with("e1 t20: vx"), "1 ULP planted fails at t20: {:?}", r3.failures);
        assert!(run(&Opts { push_off: true, compare: Some(game.clone()), ..o.clone() }).is_err(), "push-off refuses --compare");
        assert!(run(&Opts { compare: Some(game), trace: Vec::new(), ..o.clone() }).is_err(), "--compare without --trace refused");
        assert!(run(&Opts { ticks: 0, ..o.clone() }).is_err() && run(&Opts { trace: vec![(1, 0, 40)], ..o.clone() }).is_err(), "--ticks 0 and a trace past the run refused");
        // (G1 15:17 H12) the day start: neither the drive's `# day0` nor --day0 refused; both disagreeing refused; the
        // drive's own taken; a drive without `# t0` refused
        assert!(run(&Opts { day0: None, ..o.clone() }).is_err(), "no day0 anywhere: refused");
        let with_day0 = dir.join("drive-day0.txt");
        std::fs::write(&with_day0, "# drive v1\n# seed 1\n# days 1\n# walk random\n# ai on\n# t0 200\n# day0 10778\n").unwrap();
        assert!(run(&Opts { drive: with_day0.clone(), ..o.clone() }).is_err(), "--day0 11000 against the drive's 10778: refused");
        let r = run(&Opts { drive: with_day0, day0: None, ..o.clone() }).unwrap();
        assert!(r.lines.iter().any(|l| l.contains(", day 10778 at t0 200,")), "the drive's day0 taken: {:?}", r.lines.first());
        let no_t0 = dir.join("drive-no-t0.txt");
        std::fs::write(&no_t0, "# drive v1\n# seed 1\n# days 1\n# walk random\n# ai on\n").unwrap();
        assert!(run(&Opts { drive: no_t0, ..o.clone() }).is_err(), "a drive without `# t0`: refused");
        // (G1 15:17 N7) the drive's own `expect` / `expect-end` lines: NOT JUDGED, failing
        let bench = dir.join("drive-bench-expect.txt");
        std::fs::write(&bench, "# drive v1\n# seed 1\n# days 1\n# walk random\n# ai on\n# t0 200\nexpect 10 golem g0 1 3 -1\nexpect-end golems 239\n").unwrap();
        let r = run(&Opts { drive: bench, ..o.clone() }).unwrap();
        assert!(r.failures.len() == 1 && r.failures[0].starts_with("2 drive `expect` / `expect-end` lines not judged"), "{:?}", r.failures);
        // (G1 15:17 N10) acts past the run: failing, unless --partial (then counted and named)
        let long = dir.join("drive-long.txt");
        std::fs::write(&long, "# drive v1\n# seed 1\n# days 1\n# walk random\n# ai on\n# t0 200\nact 5 last-slept e1.p0 -100\nact 60 last-slept e1.p0 -100\nact 61 last-slept e1.p0 -100\n").unwrap();
        let r = run(&Opts { drive: long.clone(), ..o.clone() }).unwrap();
        assert!(r.failures.len() == 1 && r.failures[0].starts_with("2 acts past the run's 40 ticks not applied (the first at t60)"), "{:?}", r.failures);
        let r = run(&Opts { drive: long, partial: true, ..o.clone() }).unwrap();
        assert!(r.failures.is_empty() && r.lines.iter().any(|l| l == "2 acts past the run's 40 ticks (not applied; the first at t60): --partial"), "{:?}", r.failures);
        // (G1 15:17 N13) a watched cell no line visits, and a napper that visits none: failures
        let r = run(&Opts { pass_cells: vec![[100, 0, 100]], nappers: vec![1], ..o.clone() }).unwrap();
        assert!(r.failures.iter().any(|f| f.starts_with("watched cell (100,0,100): no entity line visited it")) && r.failures.iter().any(|f| f.starts_with("--nappers e1: it visited no watched cell")), "{:?}", r.failures);
        // (G1 15:17 N11) --push-shift refuses the flags its pair would override or drop
        assert!(push_shift(&Opts { push_shift: true, pass_cells: vec![[100, 0, 100]], ..o.clone() }).is_err(), "--push-shift with --trace refused");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// (G1 12:19 (7)) The drive's `# expect` lines are judged at the recording's conditions: a true one passes, a
    /// planted false one fails naming its tick and cell, one past the run is counted unjudged; with --noai (a
    /// changed condition) none is judged, and the run says so.
    #[test]
    fn expects_are_judged_at_the_recording_conditions() {
        let dir = std::env::temp_dir().join(format!("cc767b78-drivereplay-expect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let scene = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/farms/iron/middle/carousel-gate.txt");
        let drive = dir.join("drive.txt");
        // (a cell no cart reaches: "carts none", and its lane's hold and pocket empty; then the same cell with a
        // cart that isn't there; then one past the run)
        std::fs::write(&drive, "# drive v1\n# seed 1\n# days 1\n# walk random\n# ai on\n# t0 200\n# expect 5 100 0 100: carts none; holds 100:none; pockets 100:none\n# expect 6 100 0 100: carts e1 (empty) at y+0; holds 100:none; pockets 100:none\n# expect 50 100 0 100: carts none; holds 100:none; pockets 100:none\n").unwrap();
        let o = Opts { scene, drive, ticks: 10, day0: Some(11_000), noai: false, trace: Vec::new(), pushes: false, compare: None, pos_tol: 5e-7, gaps_at: Vec::new(), offset: [0, 0, 0], push_off: false, hold_cart: None, pass_cells: Vec::new(), nappers: Vec::new(), keep_port_home: true, partial: true, push_shift: false };
        let r = run(&o).unwrap();
        assert!(r.failures.len() == 1 && r.failures[0].starts_with("expect t6 at (100,0,100)"), "{:?}", r.failures);
        assert!(r.lines.iter().any(|l| l == "the drive's expect lines: 2 judged, 1 differing, 1 past the run's 10 ticks (not judged: --partial)"), "{:?}", r.lines.iter().filter(|l| l.contains("expect lines")).collect::<Vec<_>>());
        // (G1 15:17 N10: without --partial the expect past the run fails too)
        let r = run(&Opts { partial: false, ..o.clone() }).unwrap();
        assert!(r.failures.len() == 2 && r.failures[1].starts_with("1 `# expect` lines past the run's 10 ticks not judged"), "{:?}", r.failures);
        let r = run(&Opts { noai: true, ..o }).unwrap();
        assert!(r.failures.is_empty(), "{:?}", r.failures);
        assert!(r.lines.iter().any(|l| l == "the drive's expect lines: 3 not judged (the replay changes the recording's conditions: --noai)"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
