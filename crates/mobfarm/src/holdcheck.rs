//! (onechunk 2026-11-12; ironspec 09:22, its criterion 10:05) Each hold's launch threshold
//! against its pulse: a cart with a villager seated in it at rest on the hold's rail (unpowered),
//! at every rest position the benches produce (along the rail's axis), then the rail powered and
//! the cart followed; its departure is the first tick its box's centre is off the hold's cell. The
//! threshold is the least pulse width at which every rest's departure comes BEFORE the pulse ends
//! (a departure after it is a failure): the largest departure tick + 1. Printed against the pulse
//! the controller gives (the margin: pulse - threshold), for R_1, the napper's hold and the
//! S-bend's R.

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::world::flags;

/// One hold: NAME=x,y,z:PULSE[@a1,a2,...]: the rest positions along the rail's axis (z for a north-
/// south rail, x for an east-west one; none: the cell's centre).
pub struct Hold {
    pub name: String,
    pub cell: [i32; 3],
    pub pulse: i64,
    pub rests: Vec<f64>,
}

impl Hold {
    pub fn parse(s: &str) -> Result<Hold, String> {
        let (name, rest) = s.split_once('=').ok_or_else(|| format!("{s}: NAME=x,y,z:PULSE[@a1,a2,...]"))?;
        let (c, p) = rest.split_once(':').ok_or_else(|| format!("{s}: NAME=x,y,z:PULSE[@a1,a2,...]"))?;
        let v: Vec<i32> = c.split(',').map(|t| t.trim().parse().map_err(|_| format!("{s}: bad cell"))).collect::<Result<_, _>>()?;
        if v.len() != 3 {
            return Err(format!("{s}: the cell needs x,y,z"));
        }
        let (p, rests) = match p.split_once('@') {
            Some((p, r)) => (p, r.split(',').map(|t| t.trim().parse::<f64>().map_err(|_| format!("{s}: bad rest {t}"))).collect::<Result<Vec<_>, _>>()?),
            None => (p, Vec::new()),
        };
        let pulse: i64 = p.trim().parse().map_err(|_| format!("{s}: bad pulse"))?;
        Ok(Hold { name: name.to_string(), cell: [v[0], v[1], v[2]], pulse, rests })
    }
}

fn state_with_power(state: &str, on: bool) -> String {
    if state.contains("powered=") {
        state.replace(if on { "powered=false" } else { "powered=true" }, if on { "powered=true" } else { "powered=false" })
    } else {
        state.to_string()
    }
}

/// One rest: the cart placed at rest at `a` along the rail's axis, settled 20 gt on the unpowered
/// rail, then the rail powered (held on up to `max` gt); Some(the departure tick, counted from the
/// power going on: 1 = the first powered tick) or None (no departure within `max`), and the rest
/// position the cart settled at.
fn departure(spec: &FarmSpec, h: &Hold, a: Option<f64>, max: i64) -> Result<(Option<i64>, [f64; 3]), String> {
    let mut sp = spec.clone();
    let p = redsim::pos::BlockPos::new(h.cell[0], h.cell[1], h.cell[2]);
    let probe = Run::new_homeless(&FarmSpec { entities: Vec::new(), ..sp.clone() }, &RunOpts { ai: false, day0: 6000, ..RunOpts::steady(1, 10, WalkMode::Random) })?;
    let rail = probe.world.get(p).to_string();
    if !rail.contains("powered_rail") {
        return Err(format!("{}: ({},{},{}) holds {rail}, not a powered rail", h.name, h.cell[0], h.cell[1], h.cell[2]));
    }
    let ns = rail.contains("north_south") || rail.contains("ascending_north") || rail.contains("ascending_south");
    let (mut x, mut z) = (h.cell[0] as f64 + 0.5, h.cell[2] as f64 + 0.5);
    if let Some(a) = a {
        if ns {
            z = a;
        } else {
            x = a;
        }
    }
    // (on a slope the cart rides over the cell's floor by its progress up the slope)
    let frac = if ns { z - h.cell[2] as f64 } else { x - h.cell[0] as f64 };
    let rise = if rail.contains("ascending_south") || rail.contains("ascending_east") {
        frac
    } else if rail.contains("ascending_north") || rail.contains("ascending_west") {
        1.0 - frac
    } else {
        0.0
    };
    let y = h.cell[1] as f64 + 0.0625 + rise;
    sp.entities = vec![("minecraft:minecart".to_string(), [x, y, z], Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string()))];
    let mut run = Run::new_homeless(&sp, &RunOpts { ai: false, day0: 6000, ..RunOpts::steady(1, (40 + max) as u32, WalkMode::Random) })?;
    let off = redsim::block::Block::parse(&state_with_power(&rail, false)).map_err(|e| format!("{rail}: {e}"))?;
    let on = redsim::block::Block::parse(&state_with_power(&rail, true)).map_err(|e| format!("{rail}: {e}"))?;
    run.world.set_block(p, off, flags::RAW);
    let cart = |w: &redsim::world::World| -> Option<[f64; 3]> {
        w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| [e.pos.x, e.pos.y, e.pos.z])
    };
    for _ in 0..20 {
        run.tick();
    }
    let rest = cart(&run.world).ok_or("the cart vanished while settling")?;
    run.world.set_block(p, on, flags::RAW);
    for t in 1..=max {
        run.tick();
        let c = cart(&run.world).ok_or("the cart vanished")?;
        if [c[0].floor() as i32, c[1].floor() as i32, c[2].floor() as i32] != h.cell {
            return Ok((Some(t), rest));
        }
    }
    Ok((None, rest))
}

/// (ironfarm's convention-free test, Nov 12 10:44) The rail powered for exactly `n` gt, then
/// unpowered (RAW both ways): the cart's speed (the horizontal length of its motion) at the first
/// tick off the hold's cell, or None when it never leaves within `max` gt.
fn exit_speed(spec: &FarmSpec, h: &Hold, a: Option<f64>, n: i64, max: i64) -> Result<Option<f64>, String> {
    let mut sp = spec.clone();
    let p = redsim::pos::BlockPos::new(h.cell[0], h.cell[1], h.cell[2]);
    let probe = Run::new_homeless(&FarmSpec { entities: Vec::new(), ..sp.clone() }, &RunOpts { ai: false, day0: 6000, ..RunOpts::steady(1, 10, WalkMode::Random) })?;
    let rail = probe.world.get(p).to_string();
    let ns = rail.contains("north_south") || rail.contains("ascending_north") || rail.contains("ascending_south");
    let (mut x, mut z) = (h.cell[0] as f64 + 0.5, h.cell[2] as f64 + 0.5);
    if let Some(a) = a {
        if ns {
            z = a;
        } else {
            x = a;
        }
    }
    let frac = if ns { z - h.cell[2] as f64 } else { x - h.cell[0] as f64 };
    let rise = if rail.contains("ascending_south") || rail.contains("ascending_east") {
        frac
    } else if rail.contains("ascending_north") || rail.contains("ascending_west") {
        1.0 - frac
    } else {
        0.0
    };
    sp.entities = vec![("minecraft:minecart".to_string(), [x, h.cell[1] as f64 + 0.0625 + rise, z], Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string()))];
    let mut run = Run::new_homeless(&sp, &RunOpts { ai: false, day0: 6000, ..RunOpts::steady(1, (40 + max) as u32, WalkMode::Random) })?;
    let off = redsim::block::Block::parse(&state_with_power(&rail, false)).map_err(|e| format!("{rail}: {e}"))?;
    let on = redsim::block::Block::parse(&state_with_power(&rail, true)).map_err(|e| format!("{rail}: {e}"))?;
    run.world.set_block(p, off.clone(), flags::RAW);
    let cart = |w: &redsim::world::World| -> Option<([f64; 3], f64)> {
        w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| ([e.pos.x, e.pos.y, e.pos.z], (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt()))
    };
    for _ in 0..20 {
        run.tick();
    }
    run.world.set_block(p, on, flags::RAW);
    for t in 1..=max {
        if t == n + 1 {
            run.world.set_block(p, off.clone(), flags::RAW);
        }
        run.tick();
        let (c, v) = cart(&run.world).ok_or("the cart vanished")?;
        if [c[0].floor() as i32, c[1].floor() as i32, c[2].floor() as i32] != h.cell {
            return Ok(Some(v));
        }
    }
    Ok(None)
}

/// `--n-pulse`: per hold and rest, the exit speed at each pulse length N (and at 30, the full
/// speed); the least N at which every rest leaves within 1% of its full speed.
pub fn n_pulse_report(spec: &FarmSpec, holds: &[Hold], ns: &[i64], max: i64) -> Result<String, String> {
    let mut s = String::from("the least pulse at full speed (ironfarm's test: the rail powered exactly N gt, then unpowered; each rest's speed at its first tick off the hold's cell; full = within 1% of N = 30's):\n");
    for h in holds {
        let rests: Vec<Option<f64>> = if h.rests.is_empty() { vec![None] } else { h.rests.iter().map(|a| Some(*a)).collect() };
        let mut least: Option<i64> = None;
        let mut rows: Vec<String> = Vec::new();
        let mut full_by_rest: Vec<Option<f64>> = Vec::new();
        for a in &rests {
            full_by_rest.push(exit_speed(spec, h, *a, 30, max)?);
        }
        let mut sorted = ns.to_vec();
        sorted.sort();
        for &n in &sorted {
            let mut all_full = true;
            let mut cells: Vec<String> = Vec::new();
            for (k, a) in rests.iter().enumerate() {
                let v = exit_speed(spec, h, *a, n, max)?;
                let full = full_by_rest[k];
                let ok = matches!((v, full), (Some(v), Some(f)) if v >= f * 0.99);
                all_full &= ok;
                cells.push(format!("{}: {}", a.map_or("centre".to_string(), |a| format!("{a:.3}")), v.map_or("never left".to_string(), |v| format!("{v:.3}"))));
            }
            rows.push(format!("N {n}: {}{}", cells.join(", "), if all_full { " (all full)" } else { "" }));
            if all_full && least.is_none() {
                least = Some(n);
            }
        }
        let fulls: Vec<String> = rests.iter().zip(&full_by_rest).map(|(a, f)| format!("{}: {}", a.map_or("centre".to_string(), |a| format!("{a:.3}")), f.map_or("never left".to_string(), |v| format!("{v:.3}")))).collect();
        s.push_str(&format!(
            "  {} ({},{},{}): full speed (N 30) {}; {}; the least N at full speed from every rest: {}\n",
            h.name,
            h.cell[0],
            h.cell[1],
            h.cell[2],
            fulls.join(", "),
            rows.join("; "),
            least.map_or("none of those tried".to_string(), |n| n.to_string())
        ));
    }
    Ok(s)
}

/// The report: each hold's threshold (the largest departure tick over its rests + 1) against its
/// pulse.
pub fn report(spec: &FarmSpec, holds: &[Hold], max: i64) -> Result<(String, bool), String> {
    let mut s = String::from("hold launch thresholds (ironspec's criterion: every rest's departure, the first tick off the hold's cell counted from the power going on, before the pulse ends; a cart with a villager settled 20 gt at each rest on the unpowered rail, then the rail powered):\n");
    let mut ok = true;
    for h in holds {
        let rests: Vec<Option<f64>> = if h.rests.is_empty() { vec![None] } else { h.rests.iter().map(|a| Some(*a)).collect() };
        let mut worst: Option<i64> = Some(0);
        let mut rows: Vec<String> = Vec::new();
        // (G1's note 4 on d6063dc48: the threshold assumes a launch is monotone in the pulse
        // length; the hold's own pulse is run too, the rail powered exactly that long)
        let mut own_ok = true;
        for a in rests {
            let (d, rest) = departure(spec, h, a, max)?;
            let own = exit_speed(spec, h, a, h.pulse, max)?;
            own_ok &= own.is_some();
            rows.push(format!("rest ({:.3},{:.3},{:.3}) departs t {}, with its pulse exactly ({} gt) {}", rest[0], rest[1], rest[2], d.map_or("NEVER".to_string(), |t| t.to_string()), h.pulse, own.map_or("NEVER leaves".to_string(), |v| format!("leaves at speed {v:.3}"))));
            worst = match (worst, d) {
                (Some(w), Some(t)) => Some(w.max(t)),
                _ => None,
            };
        }
        let line = match worst {
            Some(w) => {
                let thr = w + 1;
                let pass = h.pulse >= thr && own_ok;
                ok &= pass;
                format!("  {} ({},{},{}): {}; threshold {thr} gt; its pulse {} gt: margin {}{} ({})\n", h.name, h.cell[0], h.cell[1], h.cell[2], rows.join(", "), h.pulse, h.pulse - thr, if own_ok { "" } else { "; a rest that never leaves with the pulse itself" }, if pass { "PASS" } else { "FAIL" })
            }
            None => {
                ok = false;
                format!("  {} ({},{},{}): {}; a rest that never departs within {max} gt (FAIL)\n", h.name, h.cell[0], h.cell[1], h.cell[2], rows.join(", "))
            }
        };
        s.push_str(&line);
    }
    Ok((s, ok))
}
