//! (farmopt 2026-11-12 03:38, ironspec's guard row on the kill room's design line: b8949c96's) The
//! kill room's guard bench, run in redsim: iron golems put through the station's pad portal into
//! the Nether kill room (v4's chamber in the carousel assembly: lava held on signs beside the
//! portal), with zombified piglins seeded in the chamber (24 by default: the cramming cap), each
//! golem followed from its arrival until it dies or crosses back. The guard: every golem dies
//! inside its 300-gt portal cooldown and none crosses back, whatever the seeded crowd does; it
//! holds in case the cooldown read is wrong (26.3's `Entity.setAsInsidePortal`: every tick on its
//! cooldown inside a portal restarts the cooldown, so pad-born zombified piglins never reach the
//! kill room, ironspec 2026-11-12).
//!
//! The positive control (`--hold`): the chamber's lava taken out (air, a stone floor under each
//! cell, where the signs held it): a golem held out of the lava past its 300 gt walks back into the
//! portal and crosses back, so a bench that saw no crossing could see one.
//!
//! `--close`: the chamber's open cells above and the portal's west side closed with cobblestone
//! (onechunk's group layout closes v4's chamber above and on its portal's west side: 14
//! cobblestone), the bench's own blocks, to compare.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use std::collections::{BTreeMap, BTreeSet};

type P = [i32; 3];

#[derive(Clone, Debug)]
pub struct Opts {
    /// Golems put into the pad portal, one every `every` gt.
    pub golems: usize,
    pub every: i64,
    /// Zombified piglins seeded in the chamber at the start.
    pub piglins: usize,
    pub ticks: i64,
    pub hold: bool,
    pub close: bool,
    pub close_adjacent: bool,
    pub seed: i64,
}

/// The design's cells the bench uses: the overworld portals' bottom cells (where golems are put),
/// the Nether portal cells, the chamber's cells beside them (lava or air, up to the first block,
/// on either side of the portal plane), its lava cells, and the cells `--close` fills.
#[derive(Clone, Debug, Default)]
pub struct Cells {
    pub pad: Vec<P>,
    pub nether_portal: Vec<P>,
    pub chamber: Vec<P>,
    pub west: Vec<P>,
    pub lava: Vec<P>,
    pub close: Vec<P>,
    /// The portal's west-side column right beside its plane (`--close-adjacent`).
    pub close_adjacent: Vec<P>,
    /// Under the chamber's lowest cells (the signs holding the lava): the control's stone floor.
    pub floor: Vec<P>,
}

fn is_portal(s: &str) -> bool {
    s.starts_with("minecraft:nether_portal")
}

fn is_lava(s: &str) -> bool {
    s.starts_with("minecraft:lava")
}

/// The bench's cells read from the design (the overworld as written; the Nether part as written,
/// its anchor 0 0 0 as the carousel assembly's).
pub fn cells(d: &Design) -> Result<Cells, String> {
    let mut c = Cells::default();
    for (p, s) in &d.blocks {
        if is_portal(s) && !d.blocks.get(&[p[0], p[1] - 1, p[2]]).is_some_and(|b| is_portal(b)) {
            c.pad.push(*p);
        }
    }
    let nb = &d.nether_blocks;
    let mut chamber: BTreeMap<P, i32> = BTreeMap::new();
    let mut west: BTreeMap<P, i32> = BTreeMap::new();
    for (p, s) in nb {
        if !is_portal(s) {
            continue;
        }
        c.nether_portal.push(*p);
        // the plane's normal: axis z -> x, axis x -> z
        let n: P = if s.contains("axis=z") { [1, 0, 0] } else { [0, 0, 1] };
        for (sign, set) in [(1, &mut chamber), (-1, &mut west)] {
            for k in 1..=2 {
                let q = [p[0] + sign * k * n[0], p[1], p[2] + sign * k * n[2]];
                match nb.get(&q) {
                    None => {
                        set.insert(q, k);
                    }
                    Some(b) if is_lava(b) => {
                        set.insert(q, k);
                    }
                    Some(_) => break,
                }
            }
        }
    }
    // the kill side is the side with the lava
    let lava_east = chamber.keys().any(|q| nb.get(q).is_some_and(|b| is_lava(b)));
    let lava_west = west.keys().any(|q| nb.get(q).is_some_and(|b| is_lava(b)));
    if !lava_east && lava_west {
        std::mem::swap(&mut chamber, &mut west);
    }
    if c.pad.is_empty() || c.nether_portal.is_empty() || chamber.is_empty() {
        return Err(format!("the design has no pad portal ({}), Nether portal ({}) or chamber beside it ({})", c.pad.len(), c.nether_portal.len(), chamber.len()));
    }
    c.lava = chamber.keys().copied().filter(|q| nb.get(q).is_some_and(|b| is_lava(b))).collect();
    // the chamber's floor: under each of its lowest cells (a sign holding the lava, or nothing)
    let low = chamber.keys().map(|q| q[1]).min().unwrap_or(0);
    c.floor = chamber.keys().filter(|q| q[1] == low).map(|q| [q[0], q[1] - 1, q[2]]).filter(|r| nb.get(r).is_none_or(|b| b.contains("sign"))).collect();
    c.chamber = chamber.keys().copied().collect();
    c.west = west.iter().filter(|(q, _)| !nb.contains_key(*q)).map(|(q, _)| *q).collect();
    c.close_adjacent = west.iter().filter(|(q, k)| **k == 1 && !nb.contains_key(*q)).map(|(q, _)| *q).collect();
    // --close: the open cells over the chamber (the two above the portal's top) and the portal's
    // west side one cell out (its adjacent column kept: an arriving golem, 1.4 wide and centred on
    // the plane, reaches 0.2 into it)
    let top = c.nether_portal.iter().map(|p| p[1]).max().unwrap_or(0);
    let mut close: BTreeSet<P> = west.iter().filter(|(q, k)| **k == 2 && !nb.contains_key(*q)).map(|(q, _)| *q).collect();
    for q in &c.chamber {
        for dy in 0..=1 {
            let r = [q[0], top + 1 + dy, q[2]];
            if !nb.contains_key(&r) {
                close.insert(r);
            }
        }
    }
    c.close = close.into_iter().collect();
    Ok(c)
}

fn bp(p: P) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

fn mobs(w: &redsim::world::World, id: &str) -> Vec<(u32, f32, [f64; 3])> {
    w.entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .filter_map(|e| match &e.kind {
            EntityKind::Mob(m) if m.ty.game_id() == id => Some((e.id, m.health, [e.pos.x, e.pos.y, e.pos.z])),
            _ => None,
        })
        .collect()
}

/// One golem's life in the Nether.
#[derive(Clone, Debug)]
struct Life {
    arrived: i64,
    died: Option<i64>,
    gone: Option<i64>,
    at: [f64; 3],
}

pub fn bench(spec: &FarmSpec, c: &Cells, o: &Opts) -> Result<String, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 100) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    if run.world.nether.is_none() {
        return Err("no Nether part".into());
    }
    run.world.mob.golem_spawning = false;
    let stone = Block::parse("minecraft:stone")?;
    let air = Block::parse("minecraft:air")?;
    let cobble = Block::parse("minecraft:cobblestone")?;
    let mut s = String::new();
    {
        let n = run.world.nether.as_deref_mut().unwrap();
        n.mob.golem_spawning = false;
        if o.hold {
            for q in &c.lava {
                n.set_block(bp(*q), air, redsim::world::flags::ALL);
            }
            for q in &c.floor {
                n.set_block(bp(*q), stone, redsim::world::flags::ALL);
            }
            s.push_str(&format!("the control (--hold): the chamber's {} lava cell(s) taken out, a stone floor in the {} cell(s) under the chamber (the signs)\n", c.lava.len(), c.floor.len()));
        }
        if o.close {
            for q in &c.close {
                n.set_block(bp(*q), cobble, redsim::world::flags::ALL);
            }
            s.push_str(&format!("--close: {} cobblestone over the chamber and on the portal's west side one cell out (the bench's own: onechunk's group layout closes them)\n", c.close.len()));
        }
        if o.close_adjacent {
            for q in &c.close_adjacent {
                n.set_block(bp(*q), cobble, redsim::world::flags::ALL);
            }
            s.push_str(&format!("--close-adjacent: {} cobblestone in the portal's west-side column right beside its plane\n", c.close_adjacent.len()));
        }
    }
    // the seeded crowd: zombified piglins in the chamber's cells, in turn
    let seeds: Vec<P> = if c.chamber.is_empty() { Vec::new() } else { (0..o.piglins).map(|i| c.chamber[i % c.chamber.len()]).collect() };
    {
        let n = run.world.nether.as_deref_mut().unwrap();
        for (i, q) in seeds.iter().enumerate() {
            let jitter = (i / c.chamber.len().max(1)) as f64 * 0.1;
            redsim::entity::spawn(n, "minecraft:zombified_piglin", [q[0] as f64 + 0.4 + jitter, q[1] as f64, q[2] as f64 + 0.4 + jitter], Some("{PortalCooldown:300}"))?;
        }
    }
    let piglins0: BTreeSet<u32> = mobs(run.world.nether.as_deref().unwrap(), "minecraft:zombified_piglin").iter().map(|m| m.0).collect();
    let start = run.world.time;
    let mut ours: BTreeSet<u32> = BTreeSet::new();
    let mut put = 0usize;
    let mut crossed_out = 0usize;
    let mut back: Vec<(i64, u32)> = Vec::new();
    let mut lives: BTreeMap<u32, Life> = BTreeMap::new();
    let mut seen_ow: BTreeSet<u32> = BTreeSet::new();
    let mut seen_nether: BTreeSet<u32> = BTreeSet::new();
    let mut piglin_ow: BTreeSet<u32> = BTreeSet::new();
    let mut elsewhere = 0usize;
    let portal: BTreeSet<P> = c.nether_portal.iter().copied().collect();
    // (the control's walk back: a golem alive 700 gt after its arrival, out of the lava, is put
    // back into the Nether portal's bottom cell, the way a golem that wandered back would stand)
    let back_cell = c.nether_portal.iter().copied().min_by_key(|p| (p[1], p[0], p[2])).unwrap_or([0, 0, 0]);
    let mut walked_back: BTreeSet<u32> = BTreeSet::new();
    // the Nether's mob events: deaths by kind and source, fights (melee hits)
    let mut cursor = 0usize;
    let mut deaths: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut hits: BTreeMap<(String, String), usize> = BTreeMap::new();
    for _ in 0..o.ticks {
        let rel = run.world.time - start;
        if put < o.golems && rel % o.every == 0 {
            let p = c.pad[put % c.pad.len()];
            let id = redsim::entity::spawn(&mut run.world, "minecraft:iron_golem", [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], None)?;
            ours.insert(id);
            seen_ow.insert(id);
            put += 1;
        }
        run.tick();
        let rel = run.world.time - start;
        let ow = mobs(&run.world, "minecraft:iron_golem");
        let ow_ids: BTreeSet<u32> = ow.iter().map(|m| m.0).collect();
        let nw = mobs(run.world.nether.as_deref().unwrap(), "minecraft:iron_golem");
        let nw_ids: BTreeSet<u32> = nw.iter().map(|m| m.0).collect();
        // golems that left the overworld (ours, gone: crossed)
        for id in seen_ow.difference(&ow_ids).copied().collect::<Vec<_>>() {
            if ours.contains(&id) {
                crossed_out += 1;
            }
            seen_ow.remove(&id);
        }
        // a golem new to the overworld that is not ours: one that crossed back
        for (id, _, _) in &ow {
            if !seen_ow.contains(id) {
                seen_ow.insert(*id);
                if !ours.contains(id) {
                    back.push((rel, *id));
                }
            }
        }
        // the Nether's golems: arrivals, deaths, departures
        for (id, hp, at) in &nw {
            if !seen_nether.contains(id) {
                seen_nether.insert(*id);
                let near = portal.iter().any(|q| (at[0] - (q[0] as f64 + 0.5)).abs() < 2.0 && (at[2] - (q[2] as f64 + 0.5)).abs() < 2.0 && (at[1] - q[1] as f64).abs() < 3.5);
                if !near {
                    elsewhere += 1;
                }
                lives.insert(*id, Life { arrived: rel, died: None, gone: None, at: *at });
            }
            if let Some(l) = lives.get_mut(id) {
                l.at = *at;
                if *hp <= 0.0 && l.died.is_none() {
                    l.died = Some(rel);
                }
            }
        }
        for (id, l) in lives.iter_mut() {
            if l.gone.is_none() && !nw_ids.contains(id) {
                l.gone = Some(rel);
            }
        }
        for (id, _, _) in mobs(&run.world, "minecraft:zombified_piglin") {
            piglin_ow.insert(id);
        }
        {
            let n = run.world.nether.as_deref_mut().unwrap();
            if n.mob.events.len() < cursor {
                cursor = 0;
            }
            for ev in &n.mob.events[cursor..] {
                match ev {
                    redsim::entity::mob::MobEvent::Death { ty, src, .. } => *deaths.entry((ty.game_id().trim_start_matches("minecraft:").to_string(), format!("{src:?}"))).or_default() += 1,
                    redsim::entity::mob::MobEvent::Hurt { ty, src, .. } if matches!(src, redsim::entity::mob::Damage::MobAttack | redsim::entity::mob::Damage::Cramming) => {
                        *hits.entry((ty.game_id().trim_start_matches("minecraft:").to_string(), format!("{src:?}"))).or_default() += 1
                    }
                    _ => {}
                }
            }
            cursor = n.mob.events.len();
            if o.hold {
                let due: Vec<u32> = lives.iter().filter(|(id, l)| l.died.is_none() && l.gone.is_none() && rel - l.arrived >= 700 && !walked_back.contains(*id)).map(|(id, _)| *id).collect();
                for id in due {
                    if let Some(i) = n.entities.list.iter().position(|e| e.id == id && !e.removed) {
                        n.entities.set_pos(i, redsim::entity::Vec3::new(back_cell[0] as f64 + 0.5, back_cell[1] as f64, back_cell[2] as f64 + 0.5));
                        walked_back.insert(id);
                    }
                }
            }
        }
    }
    let nether_piglins = mobs(run.world.nether.as_deref().unwrap(), "minecraft:zombified_piglin");
    let alive_piglins = nether_piglins.iter().filter(|m| m.1 > 0.0).count();
    let mut died: Vec<i64> = lives.values().filter_map(|l| l.died.map(|d| d - l.arrived)).collect();
    died.sort_unstable();
    let survivors: Vec<(u32, i64, [f64; 3])> = lives.iter().filter(|(_, l)| l.died.is_none() && l.gone.is_none()).map(|(id, l)| (*id, o.ticks - l.arrived, l.at)).collect();
    let left_alive: Vec<(u32, i64)> = lives.iter().filter(|(_, l)| l.died.is_none() && l.gone.is_some()).map(|(id, l)| (*id, l.gone.unwrap() - l.arrived)).collect();
    let late: Vec<i64> = died.iter().copied().filter(|&d| d >= 300).collect();
    s.push_str(&format!(
        "kill-room guard bench (redsim, seed {}): {} golem(s) put into the pad portal, one every {} gt, over {} gt; {} zombified piglin(s) seeded in the chamber's {} cell(s) (PortalCooldown 300)\n",
        o.seed,
        put,
        o.every,
        o.ticks,
        seeds.len(),
        c.chamber.len()
    ));
    s.push_str(&format!("  golems that crossed into the Nether: {crossed_out} of {put}; arrived: {} ({} not at the kill room's portal)\n", lives.len(), elsewhere));
    let pct = |q: f64| if died.is_empty() { 0 } else { died[((died.len() - 1) as f64 * q).round() as usize] };
    s.push_str(&format!(
        "  died in the Nether: {} (gt from arrival to death: min {}, median {}, max {}); {} at or past the 300-gt cooldown\n",
        died.len(),
        died.first().copied().unwrap_or(0),
        pct(0.5),
        died.last().copied().unwrap_or(0),
        late.len()
    ));
    s.push_str(&format!("  crossed back to the overworld: {} golem(s){}; left the Nether alive: {}\n", back.len(), back.first().map_or(String::new(), |(t, id)| format!(" (first: id {id} at t {t})")), left_alive.len()));
    s.push_str(&format!("  alive in the Nether at the end: {} golem(s){}\n", survivors.len(), survivors.first().map_or(String::new(), |(id, age, at)| format!(" (id {id}, {age} gt after its arrival, at ({:.1},{:.1},{:.1}))", at[0], at[1], at[2]))));
    s.push_str(&format!(
        "  zombified piglins: {} alive in the Nether at the end of {} seeded; {} seen in the overworld (crossed)\n",
        alive_piglins,
        seeds.len(),
        piglin_ow.len()
    ));
    let pass = back.is_empty() && survivors.is_empty() && left_alive.is_empty() && late.is_empty() && crossed_out == put && lives.len() == put;
    if o.hold {
        s.push_str(&format!("  the control: {} (a crossing back expected)\n", if back.is_empty() { "NO crossing back: the bench could not see one (FAIL)" } else { "golems held out of the lava crossed back: the bench sees a crossing (PASS)" }));
    } else {
        s.push_str(&format!("  the guard: {}\n", if pass { "every golem died inside its 300-gt cooldown and none crossed back (PASS)" } else { "FAIL" }));
    }
    let tally = |m: &BTreeMap<(String, String), usize>| m.iter().map(|((ty, src), n)| format!("{ty} {src} {n}")).collect::<Vec<_>>().join(", ");
    s.push_str(&format!("  deaths in the Nether by kind and source: {}\n", if deaths.is_empty() { "none".to_string() } else { tally(&deaths) }));
    s.push_str(&format!("  hits in the Nether (melee and cramming, by the one hit): {}\n", if hits.is_empty() { "none".to_string() } else { tally(&hits) }));
    if o.hold {
        s.push_str(&format!("  the control's walk back: {} golem(s) alive 700 gt after arrival put back into the portal's bottom cell {:?}\n", walked_back.len(), back_cell));
    }
    let errs: Vec<&String> = run.world.errors.iter().chain(run.world.nether.as_deref().unwrap().errors.iter()).collect();
    for e in errs.iter().take(6) {
        s.push_str(&format!("  model note: {e}\n"));
    }
    Ok(s)
}
