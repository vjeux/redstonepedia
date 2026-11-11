//! The ledge's follower-gap sweep on redsim (ironspec Nov 11 08:59: price a ledge geometry with no
//! bad band before tick-exact bridges it): a design's declared flight (its first `flights:` entry),
//! two carts sent off its take-off, the follower a number of gt behind the leader, at each stored
//! speed, empty and ridden. Each pair is scored: clean (both on rail past the landing, never
//! merged), stuck (a cart at rest off any rail), merged (two carts' horizontal distance under 0.01)
//! or derailed (a cart off rail past the flight's longest airtime, or fallen far below the landing).
//! The gap that counts is the measured one, the two carts' crossings of the take-off plane (the
//! take-off cell's far edge) interpolated between ticks, as tick-exact's bridge counts it.
//!
//! The follower crosses the leader's spawn point about `gap` gt after it: spawned at the next whole
//! tick, that many ticks' rail travel (0.4 a tick, the cap a cart on rail moves at whatever its
//! stored speed) ahead of the point. The design's other entities are left out.

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

/// One pair's outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Clean,
    Stuck,
    Merged,
    Derailed,
}

impl Outcome {
    fn tag(self) -> &'static str {
        match self {
            Outcome::Clean => "clean",
            Outcome::Stuck => "stuck",
            Outcome::Merged => "merged",
            Outcome::Derailed => "derailed",
        }
    }
}

/// One pair run: the leader spawned 2 cells before the take-off moving toward the landing at
/// `speed`, the follower about `gap` gt behind; `ticks` gt. The outcome, the measured gap at the
/// take-off plane (if both crossed it) and where it went wrong.
pub fn pair(spec: &FarmSpec, fl: &crate::flight::Flight, speed: f64, gap: f64, ridden: bool, ticks: u32, seed: i64) -> Result<(Outcome, Option<f64>, String), String> {
    let t0 = fl.take_off;
    let l0 = *fl.landing.first().ok_or("a flight with no landing")?;
    let (dx, dz) = ((l0[0] - t0[0]).signum() as f64, (l0[2] - t0[2]).signum() as f64);
    let spawn = [t0[0] as f64 + 0.5 - 2.0 * dx, t0[1] as f64 + 0.0625, t0[2] as f64 + 0.5 - 2.0 * dz];
    let mut s = spec.clone();
    s.entities.clear();
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed, ticks + 20, WalkMode::Random) };
    let mut run = Run::start(&s, &ro)?;
    while run.settle_tick() {}
    let yaw = if dx < 0.0 { 90.0 } else if dx > 0.0 { 270.0 } else if dz > 0.0 { 0.0 } else { 180.0 };
    let nbt = |v: f64| {
        let pass = if ridden { ",Passengers:[{id:\"minecraft:villager\"}]" } else { "" };
        format!("{{Rotation:[{yaw}f,0f],Motion:[{:.4}d,0d,{:.4}d]{pass}}}", dx * v, dz * v)
    };
    let lead = redsim::entity::spawn(&mut run.world, "minecraft:minecart", spawn, Some(&nbt(speed)))?;
    let s_at = gap.ceil().max(0.0) as u32;
    let ahead = (s_at as f64 - gap) * 0.4;
    let fpos = [spawn[0] + dx * ahead, spawn[1], spawn[2] + dz * ahead];
    let mut follow: Option<u32> = None;
    let mut still: [u32; 2] = [0, 0];
    let mut off_rail: [u32; 2] = [0, 0];
    // (each cart's crossing of the take-off plane, the take-off cell's far edge, interpolated)
    let plane = if dx != 0.0 { t0[0] as f64 + if dx > 0.0 { 1.0 } else { 0.0 } } else { t0[2] as f64 + if dz > 0.0 { 1.0 } else { 0.0 } };
    let along = |p: &redsim::entity::Vec3| if dx != 0.0 { p.x } else { p.z };
    let dir = if dx != 0.0 { dx } else { dz };
    let mut prev: [Option<f64>; 2] = [None, None];
    let mut cross: [Option<f64>; 2] = [None, None];
    let measured = |c: &[Option<f64>; 2]| c[0].zip(c[1]).map(|(a, b)| b - a);
    let on_rail = |w: &redsim::world::World, p: &redsim::entity::Vec3| {
        let b = BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let below = BlockPos::new(b.x, b.y - 1, b.z);
        redsim::rails::is_rail(&w.get(b)) || redsim::rails::is_rail(&w.get(below))
    };
    let low = fl.landing.iter().map(|c| c[1]).min().unwrap_or(t0[1]) as f64 - 12.0;
    // (clean early: both carts on rail two cells past the landing's first cell)
    let past_landing = |p: &redsim::entity::Vec3| (p.x - (l0[0] as f64 + 0.5)) * dx + (p.z - (l0[2] as f64 + 0.5)) * dz > 2.0;
    for t in 0..ticks {
        if t == s_at && follow.is_none() {
            follow = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", fpos, Some(&nbt(speed)))?);
        }
        run.tick();
        let ids: Vec<u32> = std::iter::once(lead).chain(follow).collect();
        let carts: Vec<(u32, redsim::entity::Vec3, f64)> = ids.iter().filter_map(|id| run.world.entities.list.iter().find(|e| e.id == *id && matches!(e.kind, EntityKind::Minecart(_))).map(|e| (e.id, e.pos, e.delta_movement.horizontal_distance()))).collect();
        if carts.len() == 2 {
            let (a, b) = (carts[0].1, carts[1].1);
            if ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt() < 0.01 && (a.y - b.y).abs() < 0.5 {
                return Ok((Outcome::Merged, measured(&cross), format!("t {t} at {:.2},{:.2},{:.2}", a.x, a.y, a.z)));
            }
        }
        for (k, (_, p, v)) in carts.iter().enumerate() {
            let a = along(p);
            if let Some(pa) = prev[k]
                && cross[k].is_none()
                && (pa - plane) * dir < 0.0
                && (a - plane) * dir >= 0.0
                && a != pa
            {
                cross[k] = Some(t as f64 + (plane - pa) / (a - pa));
            }
            prev[k] = Some(a);
            if p.y < low {
                return Ok((Outcome::Derailed, measured(&cross), format!("cart {k} fell to {:.2},{:.2},{:.2} at t {t}", p.x, p.y, p.z)));
            }
            let rail = on_rail(&run.world, p);
            off_rail[k] = if rail { 0 } else { off_rail[k] + 1 };
            still[k] = if *v < 0.005 { still[k] + 1 } else { 0 };
            if !rail && still[k] >= 10 {
                return Ok((Outcome::Stuck, measured(&cross), format!("cart {k} at rest off rail at {:.2},{:.2},{:.2} (t {t})", p.x, p.y, p.z)));
            }
            if off_rail[k] > fl.max_air + 10 {
                return Ok((Outcome::Derailed, measured(&cross), format!("cart {k} off rail {} gt at {:.2},{:.2},{:.2}", off_rail[k], p.x, p.y, p.z)));
            }
        }
        if follow.is_some() && carts.len() == 2 && carts.iter().all(|(_, p, _)| past_landing(p) && on_rail(&run.world, p)) {
            return Ok((Outcome::Clean, measured(&cross), format!("both past the landing at t {t}")));
        }
    }
    Ok((Outcome::Stuck, measured(&cross), "not both past the landing by the end".to_string()))
}

/// The sweep: per speed and ridden flag, every requested gap from `from` to `to` in `step`; the
/// bands as runs of one outcome over the measured gaps, and the first detail of each bad run.
pub fn sweep(spec: &FarmSpec, fl: &crate::flight::Flight, speeds: &[f64], from: f64, to: f64, step: f64, ridden: &[bool], ticks: u32) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let mut bad_total = 0usize;
    for &r in ridden {
        for &v in speeds {
            let mut runs: Vec<(Outcome, f64, f64, String)> = Vec::new();
            let mut g = from;
            while g <= to + 1e-9 {
                let (o, m, why) = pair(spec, fl, v, g, r, ticks, 1)?;
                if o != Outcome::Clean {
                    bad_total += 1;
                }
                let mg = m.unwrap_or(f64::NAN);
                match runs.last_mut() {
                    Some(last) if last.0 == o => last.2 = mg,
                    _ => runs.push((o, mg, mg, why)),
                }
                g += step;
            }
            lines.push(format!(
                "{} at {v:.2} b/t (measured gaps at the take-off plane): {}",
                if r { "ridden" } else { "empty" },
                runs.iter().map(|(o, a, b, why)| if *o == Outcome::Clean { format!("{} {a:.2}..{b:.2}", o.tag()) } else { format!("{} {a:.2}..{b:.2} ({why})", o.tag()) }).collect::<Vec<_>>().join("; ")
            ));
        }
    }
    lines.push(format!("ledge sweep: take-off ({},{},{}), landing {:?}, {} pairs not clean", fl.take_off[0], fl.take_off[1], fl.take_off[2], fl.landing, bad_total));
    Ok(lines)
}
