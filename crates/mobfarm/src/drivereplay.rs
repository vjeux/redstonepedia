//! `mobfarm drive-replay`: redsim's side of tick-exact's closed-loop replay (ironspec 09:21 / 09:25 (a), the
//! lane-timing reconciliation; cc767b78, bench, tool unreviewed). A bench's drive (`--drive-out`) played open
//! loop on its scene (`--scene-out`), as tick-exact's game replayer plays it: the run started as every drive
//! starts it ([`redsim::entity::mob::drive`]: `Run::start` with the drive's options at `--day0`, the settle
//! run out, the scene's entity lines spawned in file order, `t0` the world's time then), each act at `t`
//! applied when the world's time is `t0 + t`, before the next tick. `--noai` sets every villager NoAI after the
//! summon (tick-exact's `MCTAS_REPLAY_NOAI=1`: its brain built at the summon's day, then frozen), the
//! deterministic control. Per tick it prints the traced entities' rows (after the tick: position, motion, the
//! rider, the block at its feet) and, with `--pushes`, every mob-on-cart push of the tick (redsim's push log:
//! the pusher, the cart, the motion added), each named by the drive (`eN`, `eN.pK`).

use anyhow::{Context, Result, anyhow, ensure};
use redsim::entity::EntityKind;
use redsim::entity::mob::drive::{Cursor, Drive};
use redsim::entity::mob::farm::{Run, RunOpts};
use std::path::PathBuf;

/// The replay's options.
pub struct Opts {
    pub scene: PathBuf,
    pub drive: PathBuf,
    /// Run ticks after t0.
    pub ticks: i64,
    /// The day time at t0 (the settle ends there): tick-exact's `--day-start`.
    pub day0: i64,
    pub noai: bool,
    /// Traced entity lines and their run-tick windows: (line N, from, to).
    pub trace: Vec<(usize, i64, i64)>,
    pub pushes: bool,
    /// The game replayer's rows to compare against (tick-exact's trace file), and the tolerance.
    pub compare: Option<PathBuf>,
    pub tol: f64,
    /// The ticks at which the queued pairs' gaps print ([`gaps`]).
    pub gaps_at: Vec<i64>,
    /// The frame the run uses (the scene and the drive moved by it; rows printed back): the game's (0, 80, 0)
    /// for tick-exact's replays.
    pub offset: [i32; 3],
    /// (the push-off counterfactual: `MobWorld::rider_cart_push_off`, a measurement switch, not the game)
    pub push_off: bool,
    /// tick-exact's `MCTAS_REPLAY_HOLDCART="x,y,z;T0;T1"` (the lift-top held-cart row): an EMPTY minecart
    /// summoned at that design-frame position with no motion at act time T0, put back there at rest at every act
    /// time after it before T1, and removed at T1.
    pub hold_cart: Option<([f64; 3], i64, i64)>,
}

/// `X,Y,Z;T0;T1` (a design-frame position, then the act times).
pub fn parse_hold_cart(s: &str) -> Result<Option<([f64; 3], i64, i64)>> {
    if s.trim().is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = s.split(';').map(str::trim).collect();
    ensure!(parts.len() == 3, "--hold-cart {s}: X,Y,Z;T0;T1");
    let p: Vec<f64> = parts[0].split(',').map(|v| v.trim().parse::<f64>()).collect::<std::result::Result<_, _>>().with_context(|| format!("--hold-cart {s}: the position"))?;
    ensure!(p.len() == 3, "--hold-cart {s}: X,Y,Z");
    let (t0, t1): (i64, i64) = (parts[1].parse()?, parts[2].parse()?);
    ensure!(t0 < t1, "--hold-cart {s}: T0 before T1");
    Ok(Some(([p[0], p[1], p[2]], t0, t1)))
}

/// `eN[,eN...]:A-B` (or `N:A-B`): the traced lines and their run-tick window.
pub fn parse_trace(s: &str) -> Result<Vec<(usize, i64, i64)>> {
    let mut out = Vec::new();
    for part in s.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (who, win) = part.split_once(':').ok_or_else(|| anyhow!("--trace {part}: eN[,eN..]:FROM-TO"))?;
        let (a, b) = win.split_once('-').ok_or_else(|| anyhow!("--trace {part}: the window FROM-TO"))?;
        let (a, b): (i64, i64) = (a.trim().parse()?, b.trim().parse()?);
        ensure!(a <= b, "--trace {part}: FROM after TO");
        for e in who.split(',').map(str::trim) {
            let n: usize = e.trim_start_matches('e').parse().with_context(|| format!("--trace {part}: {e} is no line eN"))?;
            out.push((n, a, b));
        }
    }
    Ok(out)
}

/// Each entity's drive name: line N's entity `eN`, its passenger K `eN.pK` (after a reload, the new ids).
fn names(w: &redsim::world::World, lines: &[u32]) -> std::collections::HashMap<u32, String> {
    let now = |id: u32| w.renumbered.get(&id).copied().unwrap_or(id);
    let mut m = std::collections::HashMap::new();
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
/// frame: the run's frame minus `off`).
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

/// The scene moved by `off` (the game's frame: tick-exact's replayer holds the scene 80 blocks up, y + 80,
/// and prints its rows back in the design frame; positions near y 102 round differently from near y 22, which
/// a touching-box push turns on): blocks, block entities, the output, the box, the Nether anchor and the
/// entities' positions; their data verbatim, as the game replayer summons it (a Brain memory's position
/// unshifted: the drive's `home` acts write the shifted ones). x and z only by whole chunks (the drive's chunk
/// boxes move with them).
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
fn shift_drive(d: &mut Drive, off: [i32; 3]) -> Result<()> {
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

/// The replay: its lines (the header, then the rows and pushes by tick).
pub fn run(o: &Opts) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(&o.drive).with_context(|| format!("reading {}", o.drive.display()))?;
    let mut d = Drive::parse(&text).map_err(|e| anyhow!("{}: {e}", o.drive.display()))?;
    // (the scene as the game replayer summons it: each entity's own data verbatim, its villagers' Brain
    // memories included; its `port home:` line, redsim's bench-side injection of a HOME into every villager,
    // is dropped: the game knows no such line)
    let mut design = mctas::design::Design::load(&o.scene)?;
    let dropped_home = design.ports.remove("home").is_some();
    design.meta.remove("port home");
    let c = circuit::Circuit::from_design(design).with_context(|| format!("scene {}", o.scene.display()))?;
    let mut spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
    let off = o.offset;
    if off != [0, 0, 0] {
        shift_spec(&mut spec, off)?;
        shift_drive(&mut d, off)?;
    }
    let opts = RunOpts { day0: o.day0, ..d.opts() };
    let mut run = Run::start(&spec, &opts).map_err(|e| anyhow!(e))?;
    while run.settle_tick() {}
    let lines = redsim::entity::mob::drive::spawn_lines(&mut run, &spec).map_err(|e| anyhow!(e))?;
    let t0 = run.world.time;
    if let Some(want) = d.t0 {
        ensure!(want == t0, "the replay's t0 {t0} is not the drive's {want} (its settle differs)");
    }
    let noai = if o.noai { set_noai(&mut run.world) } else { 0 };
    run.world.mob.rider_cart_push_off = o.push_off;
    let mut out = vec![format!(
        "drive-replay (cc767b78, bench, tool unreviewed): scene {} drive {} ({} acts), seed {}, day {} at t0 {t0}, {} entity lines, {} ticks{}{}",
        o.scene.display(),
        o.drive.display(),
        d.acts.len(),
        d.seed,
        run.world.day_time(),
        lines.len(),
        o.ticks,
        if o.noai { format!(", NoAI on {noai} villagers (after the summon)") } else { String::new() },
        if dropped_home { ", the scene's `port home:` dropped (its villagers' own Brain data as summoned)" } else { "" }
    ) + &if off != [0, 0, 0] { format!(", run at the frame moved by ({}, {}, {}) (rows printed back in the design frame)", off[0], off[1], off[2]) } else { String::new() }];
    if o.pushes {
        redsim::entity::minecart::log_cart_pushes(&mut run.world);
    }
    let mut cur = Cursor::default();
    let mut held: Option<u32> = None;
    // (rows labelled as tick-exact's game replayer labels them: the row after the k-th tick is t = k - 1,
    // so the first row after the summon is t0; an act at t was applied after t ticks, before row t's tick)
    let mut mine: Vec<(i64, usize, [f64; 6])> = Vec::new();
    for _ in 0..o.ticks {
        cur.apply_due(&mut run.world, &lines, &d, t0);
        // (the held cart: summoned, held, removed at its act times, before the tick)
        if let Some((p, h0, h1)) = o.hold_cart {
            let t_act = run.world.time - t0;
            let at = [p[0] + off[0] as f64, p[1] + off[1] as f64, p[2] + off[2] as f64];
            if t_act == h0 {
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", at, None).map_err(|e| anyhow!("--hold-cart: {e}"))?;
                held = Some(id);
                out.push(format!("t{t_act}: HOLDCART an empty minecart held at ({:.4},{:.4},{:.4}) (#{id})", p[0], p[1], p[2]));
            } else if t_act > h0
                && t_act < h1
                && let Some(i) = held.and_then(|id| redsim::entity::index_of(&run.world, id))
            {
                run.world.entities.set_pos(i, redsim::entity::Vec3::new(at[0], at[1], at[2]));
                run.world.entities.list[i].delta_movement = redsim::entity::Vec3::ZERO;
            } else if t_act == h1 && let Some(i) = held.and_then(|id| redsim::entity::index_of(&run.world, id)) {
                run.world.entities.list[i].removed = true;
                held = None;
                out.push(format!("t{t_act}: HOLDCART removed"));
            }
        }
        run.tick();
        let t = run.world.time - t0 - 1;
        for &(n, a, b) in &o.trace {
            if (a..=b).contains(&t) {
                out.push(row(&run.world, &lines, n, t, off));
                if let Some(v) = state(&run.world, &lines, n, off) {
                    mine.push((t, n, v));
                }
            }
        }
        if o.pushes {
            let named = names(&run.world, &lines);
            let who = |id: u32| named.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
            for p in redsim::entity::minecart::take_cart_pushes(&mut run.world) {
                out.push(format!("t{t}: +push {} (its vehicle {}) -> {} [{:?}, 0.0, {:?}] dist {:.6}", who(p.mob), p.vehicle.map_or("none".to_string(), who), who(p.cart), p.push[0], p.push[1], p.dist));
            }
        }
    }
    if let Some(f) = &o.compare {
        let game_text = std::fs::read_to_string(f).with_context(|| format!("reading {}", f.display()))?;
        out.extend(compare(&mine, &game_text, o.tol, f));
        out.extend(gaps(&mine, &parse_rows(&game_text), &o.gaps_at));
    } else if !o.gaps_at.is_empty() {
        out.extend(gaps(&mine, &[], &o.gaps_at));
    }
    Ok(out)
}

/// The queued pairs' gaps at the given ticks (ironspec 10:01: a differing settled spacing makes the push count
/// differ for a geometric reason): for each tick, each pair of consecutive traced lines (by line number), their
/// position differences (dy, dz) and whether both were at rest, redsim's beside the game's when its rows are
/// given.
pub fn gaps(mine: &[(i64, usize, [f64; 6])], game: &[(i64, usize, [f64; 6])], at: &[i64]) -> Vec<String> {
    let get = |rows: &[(i64, usize, [f64; 6])], t: i64, n: usize| rows.iter().find(|(u, m, _)| *u == t && *m == n).map(|(_, _, v)| *v);
    let mut lines: Vec<usize> = mine.iter().map(|(_, n, _)| *n).collect();
    lines.sort();
    lines.dedup();
    let mut out = Vec::new();
    if at.is_empty() {
        return out;
    }
    out.push("the queued pairs' gaps (dy, dz = the later line's position minus the earlier's; rest = both motions 0):".to_string());
    let rest = |v: &[f64; 6]| v[3] == 0.0 && v[4] == 0.0 && v[5] == 0.0;
    for &t in at {
        for pair in lines.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let side = |rows: &[(i64, usize, [f64; 6])]| match (get(rows, t, a), get(rows, t, b)) {
                (Some(va), Some(vb)) => format!("dy {:.9} dz {:.9}{}", vb[1] - va[1], vb[2] - va[2], if rest(&va) && rest(&vb) { " rest" } else { " moving" }),
                _ => "-".to_string(),
            };
            let g = if game.is_empty() { String::new() } else { format!("; game {}", side(game)) };
            out.push(format!("  t{t} e{a}-e{b}: redsim {}{g}", side(mine)));
        }
    }
    out
}

/// A traced entity's state after the tick: position (back in the design frame) and motion.
fn state(w: &redsim::world::World, lines: &[u32], n: usize, off: [i32; 3]) -> Option<[f64; 6]> {
    let id = lines.get(n).map(|id| w.renumbered.get(id).copied().unwrap_or(*id))?;
    let e = w.entities.list.iter().find(|e| e.id == id && !e.removed)?;
    Some([e.pos.x - off[0] as f64, e.pos.y - off[1] as f64, e.pos.z - off[2] as f64, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z])
}

/// The game replayer's rows (`tN: eK pos (x,y,z) motion [dx d, dy d, dz d] …`): (t, line, state).
pub fn parse_rows(text: &str) -> Vec<(i64, usize, [f64; 6])> {
    let mut out = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        let Some((t, rest)) = l.strip_prefix('t').and_then(|r| r.split_once(": e")) else { continue };
        let Ok(t) = t.parse::<i64>() else { continue };
        let Some((n, rest)) = rest.split_once(' ') else { continue };
        let Ok(n) = n.parse::<usize>() else { continue };
        let num = |s: &str| s.trim().trim_end_matches('d').parse::<f64>().ok();
        let pos = rest.split_once("pos (").and_then(|(_, r)| r.split_once(')')).map(|(p, _)| p.split(',').filter_map(num).collect::<Vec<_>>());
        let mot = rest.split_once("motion [").and_then(|(_, r)| r.split_once(']')).map(|(p, _)| p.split(',').filter_map(num).collect::<Vec<_>>());
        if let (Some(p), Some(m)) = (pos, mot)
            && p.len() == 3
            && m.len() == 3
        {
            out.push((t, n, [p[0], p[1], p[2], m[0], m[1], m[2]]));
        }
    }
    out
}

/// The replay's rows against the game's: per line the first tick where a position or motion component
/// differs by more than `tol`, and the rows compared (the game prints positions to 6 decimals).
pub fn compare(mine: &[(i64, usize, [f64; 6])], game_text: &str, tol: f64, file: &std::path::Path) -> Vec<String> {
    let game = parse_rows(game_text);
    let at: std::collections::HashMap<(i64, usize), [f64; 6]> = game.iter().map(|(t, n, v)| ((*t, *n), *v)).collect();
    let mut lines: Vec<usize> = mine.iter().map(|(_, n, _)| *n).collect();
    lines.sort();
    lines.dedup();
    let names = ["x", "y", "z", "vx", "vy", "vz"];
    let mut out = vec![format!("the comparison against {} ({} game rows), tolerance {tol:e}:", file.display(), game.len())];
    for n in lines {
        let mut compared = 0;
        let mut first: Option<String> = None;
        // (the motions print in full on both sides: the first tick where one differs at all, the ULP-level
        // arithmetic a touching-box push can turn on)
        let mut first_exact: Option<String> = None;
        for (t, m, v) in mine.iter().filter(|(_, m, _)| *m == n) {
            let Some(g) = at.get(&(*t, *m)) else { continue };
            compared += 1;
            if first.is_none()
                && let Some(k) = (0..6).find(|&k| (v[k] - g[k]).abs() > tol)
            {
                first = Some(format!("first differs at t{t}: {} redsim {:.9} game {:.9} (Δ {:.3e})", names[k], v[k], g[k], v[k] - g[k]));
            }
            if first_exact.is_none()
                && let Some(k) = (3..6).find(|&k| v[k] != g[k] && !(v[k] == 0.0 && g[k] == 0.0))
            {
                first_exact = Some(format!("; its motion first differs at all at t{t}: {} redsim {:?} game {:?}", names[k], v[k], g[k]));
            }
        }
        out.push(format!(
            "  e{n}: {compared} rows compared; {}{}",
            first.unwrap_or_else(|| if compared > 0 { "all within the tolerance".to_string() } else { "NOT COMPARED (no common rows)".to_string() }),
            first_exact.unwrap_or_default()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trace flag's forms: lines with or without `e`, several lines, several windows.
    #[test]
    fn trace_windows() {
        assert_eq!(parse_trace("e16:440-720").unwrap(), vec![(16, 440, 720)]);
        assert_eq!(parse_trace("e11,12:0-5;e3:7-7").unwrap(), vec![(11, 0, 5), (12, 0, 5), (3, 7, 7)]);
        assert!(parse_trace("e16:9-1").is_err() && parse_trace("e16").is_err() && parse_trace("x:1-2").is_err());
    }

    /// The held cart's flag: `X,Y,Z;T0;T1` or nothing; a short position, T0 after T1 or a missing time refused.
    #[test]
    fn hold_cart_values() {
        assert_eq!(parse_hold_cart("").unwrap(), None);
        assert_eq!(parse_hold_cart("-4.5,23.0625,8.5;378;389").unwrap(), Some(([-4.5, 23.0625, 8.5], 378, 389)));
        assert!(parse_hold_cart("-4.5,23.0625;378;389").is_err() && parse_hold_cart("-4.5,23,8.5;389;378").is_err() && parse_hold_cart("-4.5,23,8.5;378").is_err());
    }

    /// The game's rows parse (tick-exact's format), and the comparison names the first differing tick and
    /// component per line, within the tolerance otherwise.
    #[test]
    fn game_rows_and_the_comparison() {
        let game = "    t0: e12 pos (-0.500000,18.556641,2.494141) motion [0.0d, 0.0d, -0.00808203125d] rider e12p0\n    t1: e12 pos (-0.500000,18.544720,2.482220) motion [0.0d, 0.0d, -0.01644289257812475d] rider e12p0\n";
        let rows = parse_rows(game);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[1].0, rows[1].1), (1, 12));
        assert!((rows[1].2[5] + 0.01644289257812475).abs() < 1e-15);
        let same = vec![(0, 12, rows[0].2), (1, 12, rows[1].2)];
        let lines = compare(&same, game, 1e-6, std::path::Path::new("g"));
        assert!(lines[1].contains("2 rows compared; all within the tolerance"), "{lines:?}");
        let mut off = same.clone();
        off[1].2[2] += 0.001;
        let lines = compare(&off, game, 1e-6, std::path::Path::new("g"));
        assert!(lines[1].contains("first differs at t1: z"), "{lines:?}");
    }
}
