//! (ironspec 17:32, vjeux's decisions inbox) A sentinel villager as the farm's day/night signal: a
//! villager with its own bed, sealed in a one-cell room beside it, and an observer facing the
//! bed's head, whose OCCUPIED state flips at the lie-down (the dusk pulse) and at the wake (the
//! morning pulse). The bench: the pulses' day times over several days and seeds, the nights it
//! fails, wanders, panics or wakes early, where it stands up, a level-32 freeze and a save and
//! reload across its nap, dusk and dawn, and its death.

use crate::layouts::Builder;
use anyhow::{bail, Result};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use redsim::world::World;

fn put(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("sentinel: {s} at {c:?} meets {old}");
    }
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

/// The bed's head (its foot north of it), the villager's cell west of the head, the observer
/// south of the head facing it, the observer's output block south of it.
pub const HEAD: [i32; 3] = [0, 0, 1];
pub const FOOT: [i32; 3] = [0, 0, 0];
pub const CELL: [i32; 3] = [-1, 0, 1];
pub const OBSERVER: [i32; 3] = [0, 0, 2];
pub const OUT: [i32; 3] = [0, 0, 3];

/// The sentinel's room: stone all round the bed and the villager's cell (two high over the cell,
/// one over the bed, so nothing stands on it), the observer in the south wall facing the head,
/// its output into a redstone lamp; the villager in its cell, HOME = the bed (`preset`) or none
/// (it claims the bed itself, AcquirePoi).
pub fn design(preset: bool) -> Result<String> {
    let mut b = Builder::new("sentinel");
    put(&mut b, HEAD, "red_bed[facing=south,occupied=false,part=head]")?;
    put(&mut b, FOOT, "red_bed[facing=south,occupied=false,part=foot]")?;
    put(&mut b, OBSERVER, "observer[facing=north,powered=false]")?;
    put(&mut b, OUT, "redstone_lamp[lit=false]")?;
    let inside = [HEAD, FOOT, CELL, [CELL[0], 1, CELL[2]]];
    for x in -2..=1 {
        for z in -1..=2 {
            for y in -1..=2 {
                let c = [x, y, z];
                if inside.contains(&c) || b.d.blocks.contains_key(&c) {
                    continue;
                }
                put(&mut b, c, "stone")?;
            }
        }
    }
    let nbt = if preset {
        format!("{{Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}", HEAD[0], HEAD[1], HEAD[2])
    } else {
        "{}".to_string()
    };
    b.entity("villager", [CELL[0] as f64 + 0.5, 0.0, CELL[2] as f64 + 0.5], Some(&nbt));
    // (a stone floor east of the room for the bench's outsider: a villager with no HOME, free to
    // walk, that must never claim the sealed bed)
    for x in 4..=8 {
        for z in -2..=2 {
            put(&mut b, [x, -1, z], "stone")?;
        }
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [6, 0, 6]);
    Ok(b.text(&[
        "A sentinel villager (ironspec 17:32): its own bed and a one-cell room beside it, sealed in stone; an observer facing the bed's head pulses at the lie-down and the wake.",
    ]))
}

/// One pulse or event of the sentinel.
#[derive(Debug, Clone)]
pub struct Event {
    pub t: i64,
    pub day: i64,
    pub day_time: i64,
    pub what: String,
}

#[derive(Debug, Default, Clone)]
pub struct Report {
    pub seed: u64,
    pub events: Vec<Event>,
    pub problems: Vec<String>,
    pub stand_ups: Vec<(i64, [i32; 3])>,
    pub panic_ticks: i64,
    pub off_cell_ticks: i64,
    pub home: Option<[i32; 3]>,
    pub head_free_tickets_end: Option<u32>,
}

fn block(s: &str) -> Block {
    Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

/// The fault rows: a level-32 freeze (entities stop, redstone runs), a save and reload (the whole
/// world unloaded, everything stops, entities back as new objects), the sentinel killed.
#[derive(Debug, Clone, Default)]
pub struct Faults {
    /// (day time at which it starts, its length in gt), on the first day it falls on after day 1.
    pub freeze: Option<(i64, i64)>,
    pub reload: Option<(i64, i64)>,
    /// Day time on day 2 at which the sentinel is removed.
    pub kill: Option<i64>,
    /// A villager with no HOME spawned on the floor east of the room at the start.
    pub outsider: bool,
}

pub fn bench(spec: &FarmSpec, seed: u64, days: i64, f: &Faults) -> Result<Report, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, ((days + 2) * 24000) as u32, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let mut vid = run.world.entities.list.iter().find(|e| matches!(e.kind, EntityKind::Mob(_))).map(|e| e.id).ok_or("no sentinel")?;
    let outsider = if f.outsider { Some(redsim::entity::spawn(&mut run.world, "minecraft:villager", [6.5, 0.0, 0.5], None)?) } else { None };
    let mut rep = Report { seed, ..Default::default() };
    let at = |c: [i32; 3]| BlockPos::new(c[0], c[1], c[2]);
    let occupied = |w: &World| matches!(w.get(at(HEAD)), Block::Bed { occupied: true, .. });
    let obs_on = |w: &World| w.get(at(OBSERVER)).to_string().contains("powered=true");
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    let t0 = run.world.time;
    let d0 = run.world.day_time();
    let end = t0 + days * 24000;
    let (mut was_occ, mut was_obs, mut was_asleep) = (occupied(&run.world), obs_on(&run.world), false);
    let mut freeze_from: Option<i64> = None;
    let mut reload_from: Option<i64> = None;
    let mut killed = false;
    let ev = |rep: &mut Report, w: &World, t: i64, what: String| {
        let dt = w.day_time();
        rep.events.push(Event { t: t - t0, day: (dt - d0).div_euclid(24000), day_time: dt.rem_euclid(24000), what });
    };
    while run.world.time < end {
        let t = run.world.time;
        let dt = run.world.day_time().rem_euclid(24000);
        let day = (run.world.day_time() - d0).div_euclid(24000);
        // the faults, from day 1 on
        if day >= 1 {
            if let Some((when, n)) = f.freeze
                && freeze_from.is_none()
                && dt == when
            {
                // (a level-32 freeze: the sentinel's chunk out of the entity-ticking box; redstone runs)
                run.world.entity_chunks = Some([100, 100, 101, 101]);
                freeze_from = Some(t);
                ev(&mut rep, &run.world, t, format!("freeze {n} gt"));
            }
            if let Some(from) = freeze_from
                && let Some((_, n)) = f.freeze
                && t == from + n
            {
                run.world.entity_chunks = None;
                ev(&mut rep, &run.world, t, "unfreeze".into());
            }
            if let Some((when, n)) = f.reload
                && reload_from.is_none()
                && dt == when
            {
                redsim::reload::unload_all(&mut run.world);
                reload_from = Some(t);
                ev(&mut rep, &run.world, t, format!("save and unload, {n} gt"));
            }
            if let Some(from) = reload_from
                && let Some((_, n)) = f.reload
                && t == from + n
            {
                redsim::reload::load_all_ordered(&mut run.world, redsim::reload::Order::Save);
                vid = run.world.renumbered.get(&vid).copied().unwrap_or(vid);
                ev(&mut rep, &run.world, t, "load".into());
            }
            if let Some(when) = f.kill
                && !killed
                && day == 2
                && dt == when
                && let Some(i) = idx(&run.world, vid)
            {
                // (a death by damage: Villager.die releases its POIs; a sleeper wakes, as the game does)
                redsim::entity::mob::living::hurt(&mut run.world, i, redsim::entity::mob::Damage::OutOfWorld, 1000.0);
                killed = true;
                ev(&mut rep, &run.world, t, "sentinel removed".into());
            }
        }
        run.tick();
        let w = &run.world;
        if w.away.is_some() {
            continue;
        }
        let (occ, obs) = (occupied(w), obs_on(w));
        if occ != was_occ {
            ev(&mut rep, w, run.world.time, if occ { "bed occupied (lie-down)".into() } else { "bed free (wake)".into() });
        }
        if obs && !was_obs {
            ev(&mut rep, w, run.world.time, "observer pulse".into());
        }
        (was_occ, was_obs) = (occ, obs);
        if let Some(i) = idx(w, vid) {
            let v = &w.entities.list[i];
            let cell = [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32];
            let asleep = redsim::entity::mob::mob(w, i).sleeping_pos.is_some();
            if was_asleep && !asleep {
                rep.stand_ups.push((run.world.time - t0, cell));
            }
            if !asleep && cell != CELL {
                rep.off_cell_ticks += 1;
            }
            use redsim::entity::mob::ai::brain;
            if brain::has_exact(w, i) && format!("{:?}", brain::ex(w, i).active).contains("Panic") {
                rep.panic_ticks += 1;
            }
            rep.home = brain::brain(w, i).home.map(|h| [h.x, h.y, h.z]);
            was_asleep = asleep;
        }
    }
    rep.head_free_tickets_end = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, at(HEAD));
    if let Some(o) = outsider
        && let Some(i) = idx(&run.world, run.world.renumbered.get(&o).copied().unwrap_or(o))
    {
        use redsim::entity::mob::ai::brain;
        let e = &run.world.entities.list[i];
        rep.problems.push(format!("(outsider: HOME {:?}, at {:.1} {:.1} {:.1})", brain::brain(&run.world, i).home, e.pos.x, e.pos.y, e.pos.z));
    }
    if !run.world.errors.is_empty() {
        rep.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    Ok(rep)
}
