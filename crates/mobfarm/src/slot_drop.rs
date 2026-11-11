//! Slot feed from above (ironspec 00:00, day-1 00:10): can a register slot take a cart dropped from
//! a ledge 2 or 3 blocks above it, at the register's timing, with no derail, merge or swap and no
//! landing on a resting cart? The design's register benched alone: the carts in its slots kept
//! with their riders, every other cart removed; the slots advance at the loop's times, and a cart
//! that leaves the last slot is removed (the release run). At the drop time the target slot's own
//! cart is removed (the napper's earlier departure; kept with `occupied`), and a ridden cart starts
//! on a ledge rail H above the slot rail, west of the slot at its z, moving east at V: it rolls off
//! the ledge's east end and falls onto the slot's cell (day-1's drop end, raised).

use crate::queue::{loop_layout, LoopOpts, LoopTiming};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// One drop case.
#[derive(Clone, Debug)]
pub struct DropCase {
    /// The target slot (an index into the register's slots, 0 = the tail's end).
    pub slot: usize,
    /// The ledge's rail height above the slot rail.
    pub h: i32,
    /// The cart's speed east as it starts on the ledge (b/t).
    pub v: f64,
    /// The cycle (from 1) and the tick within it at which the cart starts on the ledge.
    pub cycle: i64,
    pub dt: i64,
    /// The slot's own cart left in place (the fault: a cart already in the slot).
    pub occupied: bool,
    /// Cycles run after the drop's cycle.
    pub after: i64,
    /// The dropped cart's path for 60 gt in the report.
    pub trace: bool,
}

/// Runs one drop case; the report's lines, the last one a one-line verdict.
pub fn run_slot_drop(spec: &FarmSpec, o: &LoopOpts, tm: &LoopTiming, d: &DropCase, seed: i64) -> Result<String, String> {
    let lay = loop_layout(o);
    let n_slots = lay.slot_cells.len();
    if d.slot >= n_slots {
        return Err(format!("slot {} is past the register's {n_slots} slots", d.slot));
    }
    let cycles = d.cycle + 1 + d.after;
    let ticks = 100 * cycles + 50;
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed, ticks as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let set = |w: &mut redsim::world::World, c: BlockPos, p: bool| {
        let b = if p { on.clone() } else { off.clone() };
        if w.get(c) != b {
            w.set_block(c, b, redsim::world::flags::ALL);
        }
    };
    // (which slot a cart is in: the slot cell and the flat after it along the register's run; the
    // run's direction at each slot from the next slot's cell, or the previous one's at the head)
    let cells = lay.slot_cells.clone();
    let dir = |k: usize| -> (i32, i32) {
        let (a, b) = if k + 1 < cells.len() { (cells[k], cells[k + 1]) } else { (cells[k - 1], cells[k]) };
        ((b[0] - a[0]).signum(), (b[2] - a[2]).signum())
    };
    let slot_of = |p: &Vec3| -> Option<usize> {
        (0..cells.len()).find(|&k| {
            let c = cells[k];
            let (ddx, ddz) = dir(k);
            // the slot cell, and the flat one cell further along the run
            let in_cell = |cx: i32, cz: i32| p.x.floor() as i32 == cx && p.z.floor() as i32 == cz && p.y > c[1] as f64 - 0.3 && p.y < c[1] as f64 + 1.3;
            in_cell(c[0], c[2]) || in_cell(c[0] + ddx, c[2] + ddz)
        })
    };
    // the register's carts kept (with their riders), every other cart removed
    let all: Vec<(u32, Vec3, Vec<u32>)> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_)) && !e.removed).map(|e| (e.id, e.pos, e.passengers.clone())).collect();
    let mut carts: Vec<u32> = Vec::new();
    for (id, p, riders) in all {
        if slot_of(&p).is_some() {
            carts.push(id);
        } else {
            redsim::entity::kill(&mut run.world, id)?;
            for r in riders {
                redsim::entity::kill(&mut run.world, r)?;
            }
        }
    }
    let pos_of = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| e.pos);
    // the ledge: three flat east-west rails H above the slot rail, the ledge end one cell west of
    // the slot cell, each on a stone block
    let s = cells[d.slot];
    let ly = s[1] + d.h;
    let ledge: Vec<BlockPos> = (1..=3).map(|k| BlockPos::new(s[0] - k, ly, s[2])).collect();
    for c in &ledge {
        let stone = Block::parse("minecraft:stone").map_err(|e| e.to_string())?;
        run.world.set_block(BlockPos::new(c.x, c.y - 1, c.z), stone, redsim::world::flags::ALL);
        let rail = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]").map_err(|e| e.to_string())?;
        run.world.set_block(*c, rail, redsim::world::flags::ALL);
    }
    let adv = |k: i64| tm.advance_at + k * tm.advance_every;
    let t_drop = d.cycle * 100 + d.dt;
    let mut dropped: Option<u32> = None;
    let mut dropped_rider: Option<u32> = None;
    let mut merges = crate::merge::MergeTally::default();
    let mut lines: Vec<String> = Vec::new();
    let mut fell: Vec<String> = Vec::new();
    let mut doubles: Vec<String> = Vec::new();
    let mut landed: Option<(i64, Vec3)> = None;
    let mut rest_at: Option<(i64, Vec3)> = None;
    let mut slot_path: Vec<String> = Vec::new();
    let mut expected_slot = d.slot;
    let mut off_track = false;
    let mut left_seated: Option<bool> = None;
    let mut trace_lines: Vec<String> = Vec::new();
    let mut order_bad: Vec<String> = Vec::new();
    // the register's order: carts by slot at the first advance, the dropped one inserted
    for t in 0..ticks {
        let c = t / 100;
        let dt = t % 100;
        // the register's advances: every slot powered for the pulse
        for k in 0..3 {
            let a = adv(k);
            if dt == a {
                for ctl in &lay.slots {
                    set(&mut run.world, *ctl, true);
                }
                // (at each advance's start: two carts in one slot is a double; the dropped cart's
                // slot against where it should be)
                let mut by_slot: Vec<Vec<u32>> = vec![Vec::new(); cells.len()];
                for id in &carts {
                    if let Some(p) = pos_of(&run.world, *id)
                        && let Some(k2) = slot_of(&p)
                    {
                        by_slot[k2].push(*id);
                    }
                }
                for (k2, ids) in by_slot.iter().enumerate() {
                    if ids.len() > 1 {
                        doubles.push(format!("slot {k2} at cycle {c} +{dt}: carts {ids:?}"));
                    }
                }
                if let Some(id) = dropped
                    && t > t_drop
                {
                    let k2 = pos_of(&run.world, id).and_then(|p| slot_of(&p));
                    slot_path.push(format!("{}@{c}+{dt}", k2.map_or("-".to_string(), |k| k.to_string())));
                    if k2 != Some(expected_slot) && expected_slot < cells.len() {
                        order_bad.push(format!("cycle {c} +{dt}: the dropped cart in slot {} where slot {expected_slot} was due", k2.map_or("none".to_string(), |k| k.to_string())));
                    }
                    if expected_slot < cells.len() {
                        expected_slot += 1;
                    }
                }
            }
            if dt == a + tm.pulse {
                for ctl in &lay.slots {
                    set(&mut run.world, *ctl, false);
                }
            }
        }
        // the drop: the slot's own cart removed (unless `occupied`), the ridden cart on the ledge
        if t == t_drop {
            if !d.occupied {
                let mine: Vec<u32> = carts.iter().copied().filter(|id| pos_of(&run.world, *id).is_some_and(|p| slot_of(&p) == Some(d.slot))).collect();
                for id in mine {
                    let riders: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == id).map(|e| e.passengers.clone()).unwrap_or_default();
                    redsim::entity::kill(&mut run.world, id)?;
                    for r in riders {
                        redsim::entity::kill(&mut run.world, r)?;
                    }
                    carts.retain(|c2| *c2 != id);
                }
            }
            let start = [ledge[1].x as f64 + 0.5, ly as f64 + 0.0625, s[2] as f64 + 0.5];
            let before = run.world.entities.next_id;
            redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&format!("{{Motion:[{:.4}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", d.v)))?;
            let ids: Vec<u32> = (before..run.world.entities.next_id).collect();
            run.own.extend(ids.iter().copied());
            let cart = run.world.entities.list.iter().find(|e| ids.contains(&e.id) && matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id);
            dropped = cart;
            dropped_rider = cart.and_then(|id| run.world.entities.list.iter().find(|e| e.id == id).and_then(|e| e.passengers.first().copied()));
            if let Some(id) = cart {
                carts.push(id);
            }
        }
        run.tick();
        merges.observe(&run.world, &format!("cycle {c} +{dt}"));
        // a cart past the last slot (into the release run) removed with its rider
        let last = cells[cells.len() - 1];
        let (lx, lz) = dir(cells.len() - 1);
        let gone: Vec<u32> = carts.iter().copied().filter(|id| pos_of(&run.world, *id).is_some_and(|p| {
            let along = (p.x - (last[0] as f64 + 0.5)) * lx as f64 + (p.z - (last[2] as f64 + 0.5)) * lz as f64;
            along > 2.5 && (p.x.floor() as i32 - last[0]).abs() <= 1
        })).collect();
        for id in gone {
            let riders: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == id).map(|e| e.passengers.clone()).unwrap_or_default();
            redsim::entity::kill(&mut run.world, id)?;
            for r in &riders {
                redsim::entity::kill(&mut run.world, *r)?;
            }
            carts.retain(|c2| *c2 != id);
            if Some(id) == dropped {
                expected_slot = usize::MAX;
                // (the dropped cart leaves the register with its rider: seated as it leaves)
                left_seated = Some(riders.iter().any(|r| Some(*r) == dropped_rider));
            }
        }
        for id in &carts {
            if let Some(p) = pos_of(&run.world, *id)
                && p.y < -6.0
                && !fell.iter().any(|f| f.starts_with(&format!("cart {id} ")))
            {
                fell.push(format!("cart {id} at cycle {c} +{dt}: {:.2},{:.2},{:.2}", p.x, p.y, p.z));
            }
        }
        // the dropped cart: its landing (first tick on the register's track, out of the ledge's
        // cells), its first rest in a slot, and whether it ever sits off any rail for long
        if let Some(id) = dropped
            && t > t_drop
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed)
        {
            let p = e.pos;
            if t - t_drop <= 60 && trace_lines.len() < 60 && (t - t_drop) % 2 == 0 {
                trace_lines.push(format!("+{} {:.2},{:.2},{:.2} v{:.3}", t - t_drop, p.x, p.y, p.z, e.delta_movement.horizontal_distance()));
            }
            let in_slot = slot_of(&p);
            if landed.is_none() && p.x.floor() as i32 >= s[0] && p.y < ly as f64 - 0.5 {
                landed = Some((t - t_drop, p));
            }
            if rest_at.is_none() && landed.is_some() && in_slot.is_some() && e.delta_movement.horizontal_distance() < 0.005 {
                rest_at = Some((t, p));
            }
            if t - t_drop > 40 && in_slot.is_none() && rest_at.is_none() {
                off_track = true;
            }
        }
    }
    let seated = left_seated.unwrap_or_else(|| match (dropped, dropped_rider) {
        (Some(c), Some(r)) => run.world.entities.list.iter().any(|e| e.id == c && !e.removed && e.passengers.contains(&r)),
        _ => false,
    });
    // the advance after the rest: the rest's margin before it
    let rest_margin = rest_at.map(|(t, _)| {
        let c = t / 100;
        let next = (0..3).map(|k| c * 100 + adv(k)).find(|&a| a > t).unwrap_or((c + 1) * 100 + adv(0));
        next - t
    });
    lines.push(format!("slot drop: slot {} (cell {},{},{}), ledge {} above, {:.2} b/t east, start at cycle {} +{}, {}", d.slot, s[0], s[1], s[2], d.h, d.v, d.cycle, d.dt, if d.occupied { "the slot's own cart left in place" } else { "the slot emptied first" }));
    lines.push(format!("landed: {}", landed.map_or("never".to_string(), |(dt, p)| format!("{dt} gt after the start at {:.2},{:.2},{:.2}", p.x, p.y, p.z))));
    lines.push(format!("at rest in a slot: {}", rest_at.map_or("never".to_string(), |(t, p)| format!("slot {} at {:.2},{:.2},{:.2}, {} gt before the next advance", slot_of(&p).map_or("-".to_string(), |k| k.to_string()), p.x, p.y, p.z, rest_margin.unwrap_or(-1)))));
    lines.push(format!("the dropped cart's slot at each advance: {}", slot_path.join(" ")));
    lines.push(format!("fell: {} {}", fell.len(), fell.join("; ")));
    lines.push(format!("doubles (two carts in one slot at an advance): {} {}", doubles.len(), doubles.iter().take(3).cloned().collect::<Vec<_>>().join("; ")));
    lines.push(format!("out of order (the dropped cart not in its due slot): {} {}", order_bad.len(), order_bad.iter().take(3).cloned().collect::<Vec<_>>().join("; ")));
    lines.push(merges.line());
    lines.push(format!("rider seated at the end: {seated}"));
    if d.trace {
        lines.push(format!("trace: {}", trace_lines.join(" | ")));
    }
    let clean = landed.is_some() && rest_at.is_some() && rest_at.is_some_and(|(_, p)| slot_of(&p) == Some(d.slot)) && fell.is_empty() && doubles.is_empty() && order_bad.is_empty() && merges.pairs.is_empty() && seated && !off_track;
    lines.push(format!("verdict: {}", if clean { "clean" } else { "NOT clean" }));
    Ok(lines.join("\n"))
}
