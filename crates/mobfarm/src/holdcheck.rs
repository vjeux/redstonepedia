//! (onechunk 2026-11-12; ironspec 09:22) Each hold's launch threshold against its pulse: a cart
//! with a villager seated in it is left to settle on the hold's rail (unpowered), then the rail is
//! powered for N gt (its state set powered=true without updates, then false, as a controller's
//! pulse leaves it) and the cart is followed; the threshold is the least N after which the cart has
//! left the hold for good (its cell differs from the hold's 60 gt after the pulse ends, and it is
//! more than 1 block from where it rested). Printed against the pulse the controller gives (the
//! margin: pulse - threshold), for R_1, the napper's hold and the S-bend's R.

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::world::flags;

/// One hold: NAME=x,y,z:PULSE.
pub struct Hold {
    pub name: String,
    pub cell: [i32; 3],
    pub pulse: i64,
}

impl Hold {
    pub fn parse(s: &str) -> Result<Hold, String> {
        let (name, rest) = s.split_once('=').ok_or_else(|| format!("{s}: NAME=x,y,z:PULSE"))?;
        let (c, p) = rest.split_once(':').ok_or_else(|| format!("{s}: NAME=x,y,z:PULSE"))?;
        let v: Vec<i32> = c.split(',').map(|t| t.trim().parse().map_err(|_| format!("{s}: bad cell"))).collect::<Result<_, _>>()?;
        if v.len() != 3 {
            return Err(format!("{s}: the cell needs x,y,z"));
        }
        let pulse: i64 = p.trim().parse().map_err(|_| format!("{s}: bad pulse"))?;
        Ok(Hold { name: name.to_string(), cell: [v[0], v[1], v[2]], pulse })
    }
}

fn state_with_power(state: &str, on: bool) -> String {
    if state.contains("powered=") {
        state.replace(if on { "powered=false" } else { "powered=true" }, if on { "powered=true" } else { "powered=false" })
    } else {
        state.to_string()
    }
}

/// One trial: the cart settled, the rail powered `n` gt; Some(the cart's final position) and its
/// rest position, or an error.
fn trial(spec: &FarmSpec, h: &Hold, n: i64) -> Result<([f64; 3], [f64; 3]), String> {
    let mut sp = spec.clone();
    let p = redsim::pos::BlockPos::new(h.cell[0], h.cell[1], h.cell[2]);
    let mut probe = Run::new_homeless(&FarmSpec { entities: Vec::new(), ..sp.clone() }, &RunOpts { ai: false, day0: 6000, ..RunOpts::steady(1, 200, WalkMode::Random) })?;
    let rail = probe.world.get(p).to_string();
    if !rail.contains("powered_rail") {
        return Err(format!("{}: ({},{},{}) holds {rail}, not a powered rail", h.name, h.cell[0], h.cell[1], h.cell[2]));
    }
    let y = h.cell[1] as f64 + if rail.contains("ascending") { 0.5 } else { 0.0625 };
    sp.entities = vec![("minecraft:minecart".to_string(), [h.cell[0] as f64 + 0.5, y, h.cell[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string()))];
    let mut run = Run::new_homeless(&sp, &RunOpts { ai: false, day0: 6000, ..RunOpts::steady(1, (200 + n) as u32, WalkMode::Random) })?;
    // (the rail unpowered while the cart settles)
    let off = redsim::block::Block::parse(&state_with_power(&rail, false)).map_err(|e| format!("{rail}: {e}"))?;
    let on = redsim::block::Block::parse(&state_with_power(&rail, true)).map_err(|e| format!("{rail}: {e}"))?;
    run.world.set_block(p, off.clone(), flags::RAW);
    let cart = |w: &redsim::world::World| -> Option<[f64; 3]> {
        w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Minecart(_))).map(|e| [e.pos.x, e.pos.y, e.pos.z])
    };
    for _ in 0..60 {
        run.tick();
    }
    let rest = cart(&run.world).ok_or("the cart vanished while settling")?;
    run.world.set_block(p, on, flags::RAW);
    for _ in 0..n {
        run.tick();
    }
    run.world.set_block(p, off, flags::RAW);
    for _ in 0..60 {
        run.tick();
    }
    let end = cart(&run.world).ok_or("the cart vanished")?;
    probe.tick();
    Ok((rest, end))
}

/// The report: each hold's threshold (1..=max gt) against its pulse.
pub fn report(spec: &FarmSpec, holds: &[Hold], max: i64) -> Result<(String, bool), String> {
    let mut s = String::from("hold launch thresholds (a cart with a villager settled on the hold's rail, unpowered; the rail powered N gt, then unpowered; launched: 60 gt after the pulse its cell is not the hold's and it is over 1 block from its rest):\n");
    let mut ok = true;
    for h in holds {
        let mut thr = None;
        let mut rest0 = [0.0; 3];
        for n in 1..=max {
            let (rest, end) = trial(spec, h, n)?;
            rest0 = rest;
            let cell = [end[0].floor() as i32, end[1].floor() as i32, end[2].floor() as i32];
            let d = ((end[0] - rest[0]).powi(2) + (end[1] - rest[1]).powi(2) + (end[2] - rest[2]).powi(2)).sqrt();
            if cell != h.cell && d > 1.0 {
                thr = Some(n);
                break;
            }
        }
        let line = match thr {
            Some(t) => {
                let pass = h.pulse >= t;
                ok &= pass;
                format!(
                    "  {} ({},{},{}): rest ({:.3},{:.3},{:.3}); threshold {t} gt; its pulse {} gt: margin {} ({})\n",
                    h.name,
                    h.cell[0],
                    h.cell[1],
                    h.cell[2],
                    rest0[0],
                    rest0[1],
                    rest0[2],
                    h.pulse,
                    h.pulse - t,
                    if pass { "PASS" } else { "FAIL" }
                )
            }
            None => {
                ok = false;
                format!("  {} ({},{},{}): rest ({:.3},{:.3},{:.3}); NOT launched by any pulse up to {max} gt (FAIL)\n", h.name, h.cell[0], h.cell[1], h.cell[2], rest0[0], rest0[1], rest0[2])
            }
        };
        s.push_str(&line);
    }
    Ok((s, ok))
}
