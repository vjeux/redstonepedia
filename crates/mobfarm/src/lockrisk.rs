//! `mctas mobfarm lock-risk` (cc767b78; farmopt 2026-11-11 21:27, the design-space doc's F5 "close,
//! with instant golem removal"): the lock risk of villagers waiting inside the golem sensor's box
//! (rule 4b) of a golem that lives only L ticks. A villager's `NearestLivingEntitySensor` stores its
//! list every 20 gt and its `GolemSensor` reads that list every 200 gt (a golem dead since still
//! counts), both at the villager's own fixed phases (random at its creation and at every reload):
//! with golems every 100 gt living L ticks, a villager whose 20-gt scan falls on a golem's life (L/20
//! of them) holds it 20 gt, and its 200-gt scan falls in one of those holds (20/100) for a fraction
//! L/100 of villagers, which then are locked again every 200 gt: locked for good (600 gt each) while
//! they wait in range, the rest never. The bench: carts with riders on a grid around a golem spot, a
//! golem placed at the spot every 100 gt and taken out L gt later (the portal), the riders'
//! `GolemSeen` events counted per rider. Bench, not wired.

use crate::layouts::Builder;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::mob::MobEvent;
use redsim::entity::EntityKind;

/// The scene: riders on rails in a grid of `xs` (sideways from the spot, along x) by `zs` (rows),
/// at `y` relative to the golem's feet, the golem's spot at (0,0,0) on glass. Each cart on its own
/// north-south brake (an unpowered powered rail) between two stone blocks (z ± 1): a cart beside
/// another (1 apart in x) is pushed along its rail, and the brake and the stones keep it in its cell
/// (rows 2 or more apart: the stones are shared).
pub fn design(xs: &[i32], zs: &[i32], y: i32) -> String {
    let mut b = Builder::new("lock-risk-bench");
    b.set(0, -1, 0, "glass");
    for &x in xs {
        for &z in zs {
            b.set(x, y, z, "powered_rail[powered=false,shape=north_south,waterlogged=false]");
            b.set(x, y - 1, z, "glass");
            b.set(x, y, z - 1, "stone");
            b.set(x, y, z + 1, "stone");
            b.entity("minecart", [x as f64 + 0.5, y as f64 + 0.0625, z as f64 + 0.5], Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"));
        }
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [0, 0, 0]);
    b.text(&["The lock-risk bench (cc767b78, the doc's F5): riders on rails around a golem spot; the bench places a golem there every 100 gt and takes it out L gt later."])
}

/// One run's options.
#[derive(Clone, Debug)]
pub struct LockOpts {
    /// The golem's life (gt): the bench places it at the spot between two ticks and takes it out L
    /// ticks later.
    pub life: i64,
    /// Golems every this many gt.
    pub every: i64,
    pub ticks: i64,
    pub seed: i64,
    /// Each golem's place and the riders' `GolemSeen` events and final state (stderr).
    pub trace: bool,
}

/// One run's per-rider counts: (x, z of its cart, its `GolemSeen` events) and the golems placed.
pub fn bench(spec: &FarmSpec, o: &LockOpts) -> Result<(Vec<(f64, f64, usize)>, usize), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 800) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    // (the riders: each villager and its cart's place)
    let riders: Vec<(u32, f64, f64)> = run
        .world
        .entities
        .list
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Minecart(_)))
        .filter_map(|e| e.passengers.first().map(|r| (*r, e.pos.x, e.pos.z)))
        .collect();
    let mut seen: std::collections::HashMap<u32, usize> = riders.iter().map(|(r, _, _)| (*r, 0)).collect();
    run.world.mob.events.clear();
    let mut golem: Option<(u32, i64)> = None;
    let mut placed = 0usize;
    // (warm-up: 400 gt for the sensors to run once)
    for _ in 0..400 {
        run.tick();
    }
    run.world.mob.events.clear();
    let trace = o.trace;
    for t in 0..o.ticks {
        if let Some((g, t0)) = golem
            && t - t0 >= o.life
        {
            if trace && let Some(e) = run.world.entities.list.iter().find(|e| e.id == g) {
                eprintln!("t {t}: the golem {g} taken out at {:.2},{:.2},{:.2} (removed {})", e.pos.x, e.pos.y, e.pos.z, e.removed);
            }
            let _ = redsim::entity::kill(&mut run.world, g);
            golem = None;
        }
        if t % o.every == 0 && golem.is_none() {
            let nid = run.world.entities.next_id;
            let g = redsim::entity::spawn(&mut run.world, "minecraft:iron_golem", [0.5, 0.0, 0.5], None)?;
            for e in nid..run.world.entities.next_id {
                run.own.push(e);
            }
            golem = Some((g, t));
            placed += 1;
        }
        run.tick();
        if trace && t % 50 == 0 && let Some((g, _)) = golem && let Some(e) = run.world.entities.list.iter().find(|e| e.id == g) {
            eprintln!("t {t}: the golem at {:.2},{:.2},{:.2} removed {}", e.pos.x, e.pos.y, e.pos.z, e.removed);
        }
        for ev in run.world.mob.events.drain(..) {
            if let MobEvent::GolemSeen { villager, .. } = ev
                && let Some(c) = seen.get_mut(&villager)
            {
                if trace {
                    eprintln!("t {t}: GolemSeen by {villager}");
                }
                *c += 1;
            }
        }
    }
    if trace {
        for (r, x, z) in &riders {
            if let Some(j) = run.world.entities.list.iter().position(|e| e.id == *r && !e.removed) {
                let e = &run.world.entities.list[j];
                let fr = match &e.kind {
                    EntityKind::Mob(m) => m.follow_range,
                    _ => -1.0,
                };
                eprintln!("rider {r} (cart at {x:.2},{z:.2}): at {:.2},{:.2},{:.2}, follow range {fr:.3}, golems in its list {:?}, GolemSeen {}", e.pos.x, e.pos.y, e.pos.z, redsim::entity::mob::villager::nearest_golems(&run.world, j), seen.get(r).unwrap_or(&0));
            }
        }
    }
    Ok((riders.iter().map(|(r, x, z)| (*x, *z, *seen.get(r).unwrap_or(&0))).collect(), placed))
}
