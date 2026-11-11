//! `mctas mobfarm run --log <types>`: a judge run's entities followed tick by tick (farmopt
//! 2026-11-10, bed-plate-lava-out-of-sight's never-collected drop): each entity of the listed
//! types as it appears and goes (a villager discarded and a zombie villager added at its spot in
//! the same tick is a conversion), the cell changes of each with `--log-moves`, and the mob events
//! of those entities (deaths, hurts, drops, pickups, burns, POI releases). The run is the judge's
//! run tick for tick (`farm::Run::new` and `tick`, as `run --watch`); it reads the world's mob
//! events without draining them.

use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts};
use redsim::entity::mob::MobEvent;
use redsim::entity::{Entity, EntityKind};
use std::collections::HashMap;

/// What `--log` follows.
pub struct LogOpts {
    /// Entity kinds: mob ids (`villager`, `zombie_villager`, `iron_golem`...), `item` (every item)
    /// or `item:<id>` (`item:iron_ingot`), `boat`, `minecart`, `arrow`, or `all`.
    pub types: Vec<String>,
    /// Only entities whose block cell lies in this world box (inclusive).
    pub bx: Option<([i32; 3], [i32; 3])>,
    /// The game-time window printed (the run ticks from its start all the same).
    pub from: i64,
    pub to: i64,
    /// Print each followed entity's position every time it changes block cell.
    pub moves: bool,
}

/// `x0,y0,z0,x1,y1,z1` as an inclusive box.
pub fn parse_box(s: &str) -> Result<([i32; 3], [i32; 3]), String> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>().map_err(|e| format!("--log-box {s}: {e}"))).collect::<Result<_, _>>()?;
    if v.len() != 6 {
        return Err(format!("--log-box {s}: expected x0,y0,z0,x1,y1,z1"));
    }
    Ok(([v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5])], [v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])]))
}

/// The entity's kind as `--log` names it (`villager`, `item:iron_ingot`, `boat`...).
fn kind_name(e: &Entity) -> String {
    match &e.kind {
        EntityKind::Mob(m) => m.ty.def().id.to_string(),
        EntityKind::Item(it) => format!("item:{}", it.item.trim_start_matches("minecraft:")),
        EntityKind::Boat(_) => "boat".into(),
        EntityKind::Minecart(_) => "minecart".into(),
        EntityKind::ArmorStand(_) => "armor_stand".into(),
        EntityKind::PrimedTnt(_) => "tnt".into(),
        EntityKind::Arrow(_) => "arrow".into(),
    }
}

fn wanted(o: &LogOpts, name: &str) -> bool {
    o.types.iter().any(|t| t == "all" || t == name || (t == "item" && name.starts_with("item:")))
}

fn in_box(o: &LogOpts, e: &Entity) -> bool {
    let c = e.block_position();
    o.bx.is_none_or(|(lo, hi)| (lo[0]..=hi[0]).contains(&c.x) && (lo[1]..=hi[1]).contains(&c.y) && (lo[2]..=hi[2]).contains(&c.z))
}

/// One entity's line: its kind, id, position, motion and state.
fn describe(e: &Entity) -> String {
    let state = match &e.kind {
        EntityKind::Mob(m) => format!(" health {:.1}{}{}", m.health, if m.dead { " dead" } else { "" }, if m.sleeping_pos.is_some() { " asleep" } else { "" }),
        EntityKind::Item(it) => format!(" x{} age {}", it.count, it.age),
        _ => String::new(),
    };
    let ride = e.vehicle.map(|v| format!(" riding #{v}")).unwrap_or_default();
    format!(
        "{} #{} at {:.3} {:.3} {:.3} motion {:.3} {:.3} {:.3}{state}{ride}",
        kind_name(e),
        e.id,
        e.pos.x,
        e.pos.y,
        e.pos.z,
        e.delta_movement.x,
        e.delta_movement.y,
        e.delta_movement.z
    )
}

/// The run, printed: appearances, departures, cell changes and events of the followed entities.
pub fn log_run(spec: &FarmSpec, o: &RunOpts, ticks: u32, lo: &LogOpts) -> Result<(), String> {
    let mut r = Run::new(spec, o)?;
    // id -> (kind, last cell, last line)
    let mut alive: HashMap<u32, (String, [i32; 3], String)> = HashMap::new();
    let mut names: HashMap<u32, String> = HashMap::new();
    let mut seen = 0usize;
    let show = |t: i64| t >= lo.from && t <= lo.to;
    for _ in 0..=ticks {
        let t = r.world.time;
        let mut now: HashMap<u32, (String, [i32; 3], String)> = HashMap::new();
        for e in &r.world.entities.list {
            if e.removed {
                continue;
            }
            let name = kind_name(e);
            names.entry(e.id).or_insert_with(|| name.clone());
            if !wanted(lo, &name) || !in_box(lo, e) {
                continue;
            }
            let c = e.block_position();
            now.insert(e.id, (name, [c.x, c.y, c.z], describe(e)));
        }
        if show(t) {
            let mut ids: Vec<&u32> = now.keys().collect();
            ids.sort();
            for id in ids {
                let (_, cell, line) = &now[id];
                match alive.get(id) {
                    None => println!("gt {t}: + {line}"),
                    Some((_, was, _)) if lo.moves && was != cell => println!("gt {t}:   {line}"),
                    _ => {}
                }
            }
            let mut gone: Vec<&u32> = alive.keys().filter(|id| !now.contains_key(id)).collect();
            gone.sort();
            for id in gone {
                let still = r.world.entities.list.iter().any(|e| e.id == *id && !e.removed);
                println!("gt {t}: - {} ({})", alive[id].2, if still { "left the box" } else { "removed" });
            }
            let from = seen.min(r.world.mob.events.len());
            for ev in &r.world.mob.events[from..] {
                if let Some(s) = event_line(ev, &names, lo) {
                    println!("gt {t}: {s}");
                }
            }
        }
        seen = r.world.mob.events.len();
        alive = now;
        if t >= lo.to {
            break;
        }
        r.tick();
    }
    if let Some(e) = r.world.errors.first() {
        println!("model errors: {e}");
    }
    Ok(())
}

/// A mob event's line when it concerns a followed kind (by the entity's kind when first seen).
fn event_line(ev: &MobEvent, names: &HashMap<u32, String>, lo: &LogOpts) -> Option<String> {
    let follows = |id: &u32| names.get(id).is_some_and(|n| wanted(lo, n));
    let name = |id: &u32| names.get(id).cloned().unwrap_or_else(|| "?".into());
    match ev {
        MobEvent::Death { id, ty, src, pos, .. } if follows(id) => Some(format!("death #{id} {} by {} at {:.3} {:.3} {:.3}", ty.def().id, src.name(), pos[0], pos[1], pos[2])),
        MobEvent::Hurt { id, ty, src, amount, .. } if follows(id) => Some(format!("hurt #{id} {} {amount} by {}", ty.def().id, src.name())),
        MobEvent::Drop { from, item, what, count, .. } if follows(from) || follows(item) => Some(format!("drop #{from} {} -> #{item} {what} x{count}", name(from))),
        MobEvent::PickedUp { by, item, what, count, .. } if follows(by) || follows(item) => Some(format!("pickup #{by} {} took #{item} {what} x{count}", name(by))),
        MobEvent::Burnt { item, what, count, .. } if follows(item) => Some(format!("burnt #{item} {what} x{count}")),
        MobEvent::Released { id, .. } if follows(id) => Some(format!("released a POI #{id} {}", name(id))),
        _ => None,
    }
}
