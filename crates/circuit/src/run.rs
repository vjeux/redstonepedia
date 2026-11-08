//! Recording a run: on redsim (the screen: exact for the blocks it models) and in the real game (the
//! judge: the JVM bridge the door and clock judges use), both placing the design exactly like
//! `clock sim` / `clock verify` and applying the inputs between ticks.

use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, Kind, Pos};
use anyhow::{Result, anyhow};
use mctas::sim::{Region, SandboxOptions, Worker};
use std::collections::HashMap;

/// A burning TNT this far (blocks) from the design's box can no longer reach it when it explodes
/// (blocks up to about 7 away, entities up to 8): the game runner removes it as out of range.
pub const OUT_OF_RANGE: f64 = 16.0;

/// The watched box: the circuit's box plus `margin`, swept along `travel` by `distance` + margin
/// (a flying machine) or by its reach + 1 (a piston extender).
pub fn watch_box(c: &Circuit, margin: i32) -> (Pos, Pos) {
    let (lo, hi) = c.bounds();
    let mut from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let mut to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let sweep = match (&c.kind, c.travel) {
        (Some(Kind::FlyingMachine), Some(_)) | (Some(Kind::Elevator(_) | Kind::ElevatorFloors(..)), Some(_)) => c.distance.unwrap_or(16) as i32 + margin,
        (Some(Kind::PistonExtender(n)), Some(_)) => *n as i32 + 1,
        _ => 0,
    };
    if let Some(t) = c.travel {
        for i in 0..3 {
            if t[i] > 0 {
                to[i] += sweep;
            } else if t[i] < 0 {
                from[i] -= sweep;
            }
        }
    }
    (from, to)
}

/// Run a circuit on redsim with a schedule (its own kick inputs included). A flying machine that
/// reaches the box's far end along `travel` ends its run there ([`Trace::ended`]). A block that
/// cannot survive where it stands (a floating lever, torch, dust or repeater) fails it.
pub fn sim_trace(c: &Circuit, s: &Schedule, margin: i32) -> std::result::Result<Trace, String> {
    sim_trace_with(c, s, margin, true)
}

/// [`sim_trace`], with or without the judge's consumer lamps (`verify::lamp_part` runs without).
pub fn sim_trace_with(c: &Circuit, s: &Schedule, margin: i32, consumers: bool) -> std::result::Result<Trace, String> {
    let actions = c.actions(s);
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let (mut from, to) = watch_box(c, margin);
    // the judge's consumer in every air output cell (crate::CONSUMER)
    let cc = if consumers { c.with_consumers().map_err(|e| format!("{e:#}"))? } else { c.clone() };
    // the judge's input devices (R7): the driven inputs' stones and repeaters
    let mut cc = cc.with_drivers();
    // a flying machine over the ground (Circuit::grounded): grass over dirt under the whole
    // watched box, `drop` air blocks below the machine; the frames leave the ground out
    let lowest = c.lowest_y();
    if c.grounded {
        for (k, st) in crate::Ground::Grass.layers().iter().take(2).enumerate() {
            let y = lowest - 1 - c.drop - k as i32;
            for x in from[0]..=to[0] {
                for z in from[2]..=to[2] {
                    cc.clock.design.blocks.insert([x, y, z], st.to_string());
                }
            }
        }
        from[1] = from[1].min(lowest - c.drop - 4);
    }
    if let Some((p, st)) = clock::sim::unsupported(&cc.clock, from, to)?.first() {
        return Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(st)));
    }
    let tnt = matches!(c.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered));
    if tnt && c.explodes() {
        return sim_tnt(c, &cc, &actions, s, &probes, margin, KICK_DEFAULT, &[], crate::Ground::Grass, false);
    }
    // a randomizer: the level random mirrored from the build's seed (the game sandbox's, so a
    // dropper's random slot is the game's; crate::random); a design holding a block that ticks at
    // random (an unwaxed copper bulb oxidizing: mobinfra 2026-11-08): the sandbox's seed 0
    // mirrored and its random ticks (randomTickSpeed 3) run, as the bridge's sandbox does (the
    // design's own blocks: the judge's grass ground under a flying machine draws but changes
    // nothing it watches)
    let ticking = c.clock.design.block_list().iter().any(|(_, s)| redsim::Block::parse(s).is_ok_and(|b| redsim::crops::randomly_ticking(&b)));
    let level = if matches!(c.kind, Some(Kind::Randomizer(_))) {
        Some((crate::random::seed(from, to, c.salt), crate::random::chunks(from, to)))
    } else if ticking {
        Some((0, crate::random::chunks(from, to)))
    } else {
        None
    };
    match clock::sim::drive_seeded(&cc.clock, &actions, s.ticks, &probes, from, to, tnt && !c.explodes(), level) {
        Ok(d) => {
            let ended = match d.stopped {
                None => None,
                Some((t, why)) if c.kind == Some(Kind::FlyingMachine) && (why.contains("outside the box") || why.contains("escapes")) => Some(format!("reached the corridor end at frame {t}")),
                Some((_, why)) => return Err(why),
            };
            let mut cells = d.cells;
            if c.grounded {
                for f in cells.iter_mut() {
                    f.retain(|(p, _)| p[1] >= lowest);
                }
            }
            let entities: Vec<(u32, Vec<(String, [f64; 3])>)> = d.entities.into_iter().enumerate().map(|(f, e)| (f as u32, e)).collect();
            let mut t = Trace::new(c, cells, d.signals);
            t.entities = entities;
            t.ended = ended;
            // a primed TNT spawns at its cell's bottom center (redsim has no motion for it)
            t.primed = d.primed.iter().map(|(f, p)| crate::trace::Primed { frame: *f, pos: [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], ..Default::default() }).collect();
            Ok(t)
        }
        Err((_, why)) => Err(why),
    }
}

/// The kick of every primed TNT in a redsim run with explosions (`sim_trace`; the all-randoms
/// sweep tries every kick: `tnt_kick_trace`).
pub const KICK_DEFAULT: f64 = 0.0;

/// How far the ground reaches around a TNT duper on redsim (a TNT past it falls into the void and
/// explodes far below the machine).
pub const GROUND_REACH: i32 = 24;

/// A TNT duper with explosions on, on redsim (the exact explosion port, every ray at its largest
/// roll), standing on `ground`, every primed TNT kicked by `kick`; with `removal`, a TNT
/// OUT_OF_RANGE away is removed like the real-game judge does (without, each one flies on until it
/// explodes: the strict run).
pub fn tnt_kick_trace(c: &Circuit, s: &Schedule, margin: i32, kick: f64, ground: crate::Ground, removal: bool) -> std::result::Result<Trace, String> {
    let actions = c.actions(s);
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let cc = c.with_consumers().map_err(|e| format!("{e:#}"))?;
    sim_tnt(c, &cc, &actions, s, &probes, margin, kick, &[], ground, removal)
}

/// [`tnt_kick_trace`] with the k-th primed TNT kicked by `kicks[k]` (each TNT its own random
/// angle; `kick` past the end), the strict run.
pub fn tnt_kicks_trace(c: &Circuit, s: &Schedule, margin: i32, kick: f64, kicks: &[f64], ground: crate::Ground) -> std::result::Result<Trace, String> {
    let actions = c.actions(s);
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let cc = c.with_consumers().map_err(|e| format!("{e:#}"))?;
    sim_tnt(c, &cc, &actions, s, &probes, margin, kick, kicks, ground, false)
}

#[allow(clippy::too_many_arguments)]
fn sim_tnt(c: &Circuit, cc: &Circuit, actions: &[(u32, clock::Action)], s: &Schedule, probes: &[Pos], margin: i32, kick: f64, kicks: &[f64], ground: crate::Ground, removal: bool) -> std::result::Result<Trace, String> {
    let (lo, hi) = c.clock.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
    // the strict run (no removal) grows its ground until no burning TNT ever leaves it (a TNT
    // past the ground's edge would fall into a void a real world does not have; coordinator)
    let mut r = GROUND_REACH;
    loop {
        let from = [lo[0] - r - 2, lo[1] - 6 - c.drop, lo[2] - r - 2];
        let to = [hi[0] + r + 2, hi[1] + 16, hi[2] + r + 2];
        let o = clock::sim::TntOptions {
            kick,
            kicks: kicks.to_vec(),
            max_rolls: true,
            out_of_range: if removal { OUT_OF_RANGE } else { f64::INFINITY },
            design_box: (lo, hi),
            machine_box: Some((lo, hi)),
            floor: c.ground_blocks(ground, r, c.drop),
            watch: Some(watch_box(c, margin)),
            escape: if removal { None } else { Some(r as f64 - 1.0) },
        };
        let d = clock::sim::drive_tnt(&cc.clock, actions, s.ticks, probes, from, to, &o).map_err(|(_, why)| why)?;
        if let Some((f, p)) = d.escaped {
            if r >= MAX_GROUND_REACH {
                return Err(format!("a burning TNT left a ground {r} blocks wide around the machine at frame {f} ({:.1}, {:.1}, {:.1})", p[0], p[1], p[2]));
            }
            r *= 2;
            continue;
        }
        if let Some((_, why)) = d.driven.stopped {
            return Err(why);
        }
        let mut t = Trace::new(c, d.driven.cells, d.driven.signals);
        t.primed = d
            .tnt
            .iter()
            .map(|r| crate::trace::Primed { frame: r.frame, pos: r.first_pos, motion: Some(r.first_motion), end: r.end, launch: r.launch, moved: r.moved, last_pos: r.last_pos, end_frame: r.end_frame, gone: r.gone })
            .collect();
        t.exploded = d.explosions.len() as u32;
        t.explosions = d.explosions;
        t.out_of_range = d.out_of_range;
        t.blasts = d.blasts.iter().zip(&d.blast_use).map(|(&(frame, pos, _, _), &(in_fluid, open_ground))| crate::trace::BlastUse { frame, pos, in_fluid, open_ground }).collect();
        return Ok(t);
    }
}

/// The widest ground the strict redsim run grows to (blocks around the machine).
pub const MAX_GROUND_REACH: i32 = 96;

/// How far around a TNT duper the real-game sandbox loads chunks and lays its ground (no TNT is
/// removed: a burning TNT that reaches the loaded area's edge, where it would freeze and never
/// explode, fails the run; coordinator 2026-10-20).
pub const GAME_REACH: i32 = 80;

/// Run a circuit (already turned and moved) in the real game, in a fresh sandbox: `Ok(Err(why))`
/// when the run itself breaks a rule (an entity, a block reaching the edge of the watched box).
pub fn game_trace(w: &mut Worker, c: &Circuit, s: &Schedule, margin: i32) -> Result<std::result::Result<Trace, String>> {
    let (from, to) = watch_box(c, margin);
    // TNT dupers with explosions: load far enough around for every burning TNT to keep ticking
    // until it explodes (GAME_REACH)
    let pad = if c.explodes() { GAME_REACH + 2 } else { 0 };
    let chunks = [(from[0] - pad).div_euclid(16), (from[2] - pad).div_euclid(16), (to[0] + pad).div_euclid(16), (to[2] + pad).div_euclid(16)];
    // a randomizer: the level random seeded from the build's watch box (each build its own
    // seed; redsim mirrors it: crate::random)
    let seed = if matches!(c.kind, Some(Kind::Randomizer(_))) { crate::random::seed(from, to, c.salt) } else { 0 };
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    // TNT dupers stand on the ground (Circuit::ground_blocks): grass over dirt over bedrock over
    // the whole loaded area, so every TNT lands and explodes on it like in a real world; a flying
    // machine over the ground (Circuit::grounded) the same, `drop` air blocks below it
    if c.explodes() || c.grounded {
        let (x0, z0, x1, z1) = (chunks[0] * 16, chunks[1] * 16, chunks[2] * 16 + 15, chunks[3] * 16 + 15);
        let (lo, _) = c.clock.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
        for (k, st) in crate::Ground::Grass.layers().iter().enumerate() {
            let y = lo[1] - 1 - c.drop - k as i32;
            w.fill(sb, [x0, y, z0], [x1, y, z1], st, mctas::sim::PlaceMode::Raw)?;
        }
    }
    let r = game_in(w, sb, c, s, from, to, margin, [chunks[0] * 16, chunks[1] * 16, chunks[2] * 16 + 15, chunks[3] * 16 + 15]);
    let _ = w.drop_sandbox(sb);
    r
}

/// A frame's non-air cells from `min_y` up (a TNT duper's ground below its lowest layer is left
/// out: its craters are not the machine).
fn frame_cells(f: &Region, extras: &HashMap<Pos, String>, min_y: i32) -> Vec<(Pos, String)> {
    let mut v: Vec<(Pos, String)> = f.iter().filter(|(p, s)| p[1] >= min_y && !mctas::blockstate::is_air(s)).map(|(p, s)| (p, clock::wave::with_extra(s, extras.get(&p).map(|e| e.as_str())))).collect();
    v.sort_by_key(|(p, _)| *p);
    v
}

/// `loaded`: the sandbox's loaded area (x0, z0, x1, z1): a burning TNT reaching its edge fails the
/// run (it would freeze there and never explode).
#[allow(clippy::too_many_arguments)]
fn game_in(w: &mut Worker, sb: u32, c: &Circuit, s: &Schedule, from: Pos, to: Pos, margin: i32, loaded: [i32; 4]) -> Result<std::result::Result<Trace, String>> {
    let min_y = if c.explodes() || c.grounded { c.lowest_y() } else { i32::MIN };
    // the judge's consumer in every air output cell (crate::CONSUMER)
    let cc = c.with_consumers()?.with_drivers();
    clock::verify::place(w, sb, &cc.clock)?;
    // every block must be able to stand where it is (a player could build it): the placement
    // removes a floating dust (its shape update) and keeps a floating lever, torch or repeater,
    // which would pop at its first shape update
    let placed = w.get_region(sb, from, to)?;
    let popped = cc.clock.design.blocks.iter().filter(|(p, _)| Some(**p) != cc.clock.withheld()).find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, cc.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let mut actions = c.clock.actions();
    actions.extend(c.actions(s));
    actions.sort_by_key(|(t, _)| *t);
    let extras_of = |infos: Vec<mctas::sim::BlockEntityInfo>| -> HashMap<Pos, String> {
        infos.into_iter().filter_map(|b| clock::wave::be_extra_from_nbt(&b.kind, &b.nbt).map(|e| (b.pos, e))).collect()
    };
    let best_of = |flat: &[u8]| -> Vec<u8> { flat.iter().step_by(2).copied().collect() };
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let volume = (size[0] * size[1] * size[2]).max(1) as u32;
    // TNT dupers: one tick at a time, every new primed TNT recorded and removed before it can
    // explode
    let tnt = matches!(c.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered));
    let seg = if tnt { 1 } else { (400_000 / volume).clamp(10, 2000) };
    let mut primed: Vec<crate::trace::Primed> = Vec::new();
    let mut seen_tnt: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut tnt_index: HashMap<String, usize> = HashMap::new();
    let explode = tnt && c.explodes();
    let mut burning: HashMap<String, i32> = HashMap::new();
    let mut exploded = 0u32;
    let mut out_of_range = 0u32;
    let design_box: ([i32; 3], [i32; 3]) = c.clock.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
    let mut explosions: Vec<u32> = Vec::new();
    let mut blasts: Vec<crate::trace::BlastUse> = Vec::new();
    let mut cells: Vec<Vec<(Pos, String)>> = Vec::with_capacity(s.ticks as usize + 1);
    let mut signals: Vec<Vec<u8>> = Vec::with_capacity(s.ticks as usize + 1);
    // an elevator: its rider read at the end of each phase (crate::elevator)
    let snaps: Vec<u32> = if matches!(c.kind, Some(Kind::Elevator(_) | Kind::ElevatorFloors(..))) { s.phases.iter().map(|p| p.1.saturating_sub(1)).collect() } else { Vec::new() };
    let mut entity_frames: Vec<(u32, Vec<(String, [f64; 3])>)> = Vec::new();
    let mut t = 0u32;
    loop {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            clock::verify::apply(w, sb, &a)?;
        }
        // frame t: the state after t ticks with its inputs applied (replaces the frame the last
        // step recorded at t)
        let f = w.get_region(sb, from, to)?;
        let ex = extras_of(w.block_entities(sb, from, to)?);
        let sg: Vec<u8> = if probes.is_empty() { Vec::new() } else { w.signals(sb, &probes)?.into_iter().map(|(b, _)| b).collect() };
        if cells.len() as u32 == t + 1 {
            *cells.last_mut().unwrap() = frame_cells(&f, &ex, min_y);
            *signals.last_mut().unwrap() = sg;
        } else {
            cells.push(frame_cells(&f, &ex, min_y));
            signals.push(sg);
        }
        if snaps.contains(&t) && entity_frames.last().is_none_or(|e| e.0 != t) {
            entity_frames.push((t, w.entities(sb)?.into_iter().map(|e| (e.kind.trim_start_matches("minecraft:").to_string(), e.pos)).collect()));
        }
        let next_snap = snaps.iter().copied().filter(|&f| f > t).min().unwrap_or(u32::MAX);
        let next = actions.first().map(|(at, _)| *at).unwrap_or(s.ticks).min(s.ticks).min(t + seg).min(next_snap);
        if next <= t {
            break;
        }
        let st = w.step_probe_be(sb, next - t, false, &probes, Some((from, to)), true)?;
        if st.block_entities.len() != st.frames.len() {
            return Err(anyhow!("block-entity frames ({}) do not match the block frames ({}): rebuild the bridge (mctas setup)", st.block_entities.len(), st.frames.len()));
        }
        for (f, dump) in st.frames.iter().zip(&st.block_entities) {
            let ex = extras_of(mctas::sim::parse_block_entities(dump)?);
            cells.push(frame_cells(f, &ex, min_y));
        }
        signals.extend(st.signals.iter().map(|v| best_of(v)));
        if tnt {
            // every primed TNT is followed to fuse 1 (its position there: the precision) and
            // removed then, before it can explode (with `explode: on`: left to explode; a TNT seen
            // before and gone now has exploded)
            let ents = w.entities(sb)?;
            let alive: std::collections::HashSet<&str> = ents.iter().filter(|e| e.kind.trim_start_matches("minecraft:") == "tnt").map(|e| e.uuid.as_str()).collect();
            let mut vanished_early: Option<(String, i32)> = None;
            let mut went_off: Vec<String> = Vec::new();
            burning.retain(|u, last_fuse| {
                let gone = !alive.contains(u.as_str());
                if gone {
                    // an explosion only if its last sighting had fuse <= 1 (clockinfra): a TNT gone
                    // earlier left the loaded world or was removed otherwise
                    if *last_fuse <= 1 {
                        exploded += 1;
                        explosions.push(next as u32);
                        went_off.push(u.clone());
                    } else if vanished_early.is_none() {
                        vanished_early = Some((u.clone(), *last_fuse));
                    }
                }
                !gone
            });
            // where each went off: its last sighting (fuse 1) plus its last move (gravity first,
            // as `PrimedTnt.tick`; a TNT at rest stays), the fluid in that cell and what the blast
            // would destroy on open ground there (the TNT judge's usefulness rule)
            for u in &went_off {
                let Some(&k) = tnt_index.get(u) else { continue };
                let (p, m) = primed[k].end.unwrap_or((primed[k].last_pos, [0.0; 3]));
                let mv = [m[0], m[1] - 0.04, m[2]];
                let moving = mv.iter().map(|v| v * v).sum::<f64>() >= 0.05 * 0.05;
                let at = if moving { [p[0] + mv[0], p[1] + mv[1], p[2] + mv[2]] } else { p };
                let center = redsim::entity::Vec3::new(at[0], at[1] + 0.98f32 as f64 * 0.0625, at[2]);
                let fluid: std::collections::HashSet<Pos> = cells.last().map(|f| f.iter().filter(|(_, s)| is_fluid_state(s)).map(|(q, _)| *q).collect()).unwrap_or_default();
                let fluid_at = |q: redsim::pos::BlockPos| fluid.contains(&[q.x, q.y, q.z]);
                let cell = redsim::pos::BlockPos::new(center.x.floor() as i32, center.y.floor() as i32, center.z.floor() as i32);
                blasts.push(crate::trace::BlastUse { frame: next, pos: [center.x, center.y, center.z], in_fluid: fluid_at(cell), open_ground: redsim::explosion::open_ground(center, redsim::explosion::TNT_POWER, &fluid_at) });
            }
            if let Some((u, f)) = vanished_early {
                return Ok(Err(format!("frame {next}: a primed TNT ({u}) vanished with fuse {f} left (out of the loaded world?), not an explosion")));
            }
            for e in ents {
                if e.kind.trim_start_matches("minecraft:") != "tnt" {
                    continue;
                }
                if seen_tnt.insert(e.uuid.clone()) {
                    tnt_index.insert(e.uuid.clone(), primed.len());
                    primed.push(crate::trace::Primed { frame: next, pos: e.pos, motion: Some(e.motion), moved: next, last_pos: e.pos, ..Default::default() });
                }
                // its launch: the largest horizontal speed seen, from 0.4 blocks per gt; and the
                // last frame it moved
                if let Some(&k) = tnt_index.get(&e.uuid) {
                    let h = |m: &[f64; 3]| (m[0] * m[0] + m[2] * m[2]).sqrt();
                    if h(&e.motion) >= 0.4 && primed[k].launch.is_none_or(|l| h(&e.motion) > h(&l)) {
                        primed[k].launch = Some(e.motion);
                    }
                    if e.pos.iter().zip(&primed[k].last_pos).any(|(a, b)| a.to_bits() != b.to_bits()) {
                        primed[k].moved = next;
                        primed[k].last_pos = e.pos;
                    }
                }
                let fuse = e.state.split("fuse: ").nth(1).and_then(|s| s.split(|c: char| !c.is_ascii_digit() && c != '-').next()).and_then(|s| s.parse::<i32>().ok());
                let Some(fuse) = fuse else {
                    return Err(anyhow!("a primed TNT's fuse is not in its saved data ({}): the bridge's entity data changed", e.state));
                };
                if explode {
                    // a burning TNT near the loaded area's edge would freeze there (entities only
                    // tick in loaded chunks) and never explode: the run cannot judge it (no TNT is
                    // removed: every one explodes where it lands)
                    let _ = design_box;
                    let edge = 2.0;
                    if e.pos[0] < loaded[0] as f64 + edge || e.pos[0] > loaded[2] as f64 + 1.0 - edge || e.pos[2] < loaded[1] as f64 + edge || e.pos[2] > loaded[3] as f64 + 1.0 - edge {
                        return Ok(Err(format!("frame {next}: a burning TNT reached the edge of the loaded area at ({:.1}, {:.1}, {:.1}), {GAME_REACH} blocks around the machine: it would freeze there and never explode", e.pos[0], e.pos[1], e.pos[2])));
                    }
                    burning.insert(e.uuid.clone(), fuse);
                }
                // the burning TNT is a real entity in the game only (redsim records primes): an
                // entity-reacting block it touches would see it in the game and not on redsim
                let bb = e.bbox;
                if let Some(frame) = cells.last() {
                    let hit = frame.iter().find(|(p, s)| {
                        (s.contains("pressure_plate") || s.contains("tripwire") || s.contains("detector_rail"))
                            && (0..3).all(|i| (p[i] as f64) < bb[i + 3] && (p[i] as f64 + 1.0) > bb[i])
                    });
                    if let Some((p, s)) = hit {
                        return Ok(Err(format!("frame {next}: a primed TNT touches {} at {p:?} (entity-reacting blocks see burning TNT in the game, not on redsim)", clock::short(s))));
                    }
                }
                if fuse <= 1 {
                    if let Some(&k) = tnt_index.get(&e.uuid) {
                        primed[k].end = Some((e.pos, e.motion));
                        primed[k].end_frame = next;
                    }
                    if !explode {
                        w.exec(sb, &format!("kill {}", e.uuid))?;
                    }
                }
            }
        }
        t = next;
    }
    if tnt {
        // the TNT still burning when the run ends: removed (the judge's, not the design's)
        for e in w.entities(sb)? {
            if e.kind.trim_start_matches("minecraft:") == "tnt" {
                w.exec(sb, &format!("kill {}", e.uuid))?;
            }
        }
    }
    // nothing may reach the outer layer of the watched box (it escaped the margin), except a flying
    // machine reaching its far end along `travel` (it flew the corridor: the run ends there); and no
    // entity may appear (a drop, a popped block) except the design's own
    let far = |p: &Pos| -> bool { c.kind == Some(Kind::FlyingMachine) && c.travel.is_some_and(|tr| (0..3).any(|i| (tr[i] > 0 && p[i] == to[i]) || (tr[i] < 0 && p[i] == from[i]))) };
    let mut ended = None;
    for (k, frame) in cells.iter().enumerate() {
        if let Some((p, st)) = frame.iter().find(|(p, _)| (0..3).any(|i| p[i] == from[i] || p[i] == to[i])) {
            if far(p) {
                ended = Some(k);
                break;
            }
            return Ok(Err(format!("frame {k}: {} at {p:?} reached the edge of the watched box (margin {margin})", clock::short(st))));
        }
    }
    if let Some(k) = ended {
        cells.truncate(k);
        signals.truncate(k);
    }
    let ents = w.entities(sb)?;
    // a TNT duper on the ground: the items its craters drop and the TNT still burning are part
    // of the scenario (a machine block a blast destroys shows in the frames instead)
    let ents: Vec<_> = if explode { ents.into_iter().filter(|e| !matches!(e.kind.trim_start_matches("minecraft:"), "item" | "tnt")).collect() } else { ents };
    let foreign = ents.len().saturating_sub(c.clock.design.entities.len());
    if foreign > 0 && ended.is_none() {
        let e = &ents[0];
        return Ok(Err(format!("{} entit{} at the end ({} at {:.1?}...): a drop or a popped block", ents.len(), if ents.len() == 1 { "y" } else { "ies" }, clock::short(&e.kind), e.pos)));
    }
    let mut t = Trace::new(c, cells, signals);
    t.ended = ended.map(|k| format!("reached the corridor end at frame {k}"));
    t.primed = primed;
    t.exploded = exploded;
    t.out_of_range = out_of_range;
    t.explosions = explosions;
    t.blasts = blasts;
    t.entities = entity_frames;
    Ok(Ok(t))
}

/// A cell state (a frame's string) holding a fluid: water, lava, a bubble column, a waterlogged
/// block.
fn is_fluid_state(s: &str) -> bool {
    let n = s.trim_start_matches("minecraft:");
    n.starts_with("water") || n.starts_with("lava") || n.starts_with("bubble_column") || s.contains("waterlogged=true")
}
