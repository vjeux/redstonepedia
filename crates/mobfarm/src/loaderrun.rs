//! (loaderlay, Nov 13 2026; farmopt 21:44: "run them in redsim with arrivals and golem arrivals printed
//! per pad") `mctas mobfarm loader-layout ... --redsim TICKS`: the layout's portal skeleton run in redsim
//! (`World::nether`, `entity::portal`: crossings both ways, the closest-portal search, the arrival
//! placement, the Nether's PORTAL ticket): the loader's cells whole, each copy's portal cells with their
//! frame obsidian (the copies' other cells enter no portal search and no item's flight), the loader's
//! block entities. A portal block is a terrain block, as the farm judge parses it
//! (`circuit::studies::mobfarm::farm_block`). The loop's items are what the loader's droppers hold (a
//! clock's own items sit in its hoppers). A side whose clock is an observer pair starts as the build
//! starts it: its observer farther from the portal is taken out and put back (O1 sees it), and the cell
//! is read back; a side with no observer pair needs no start. Launches are read back on each side: an
//! item entity no crossing made. At `--golem-at` an iron golem is put in each pad at three places of its
//! bottom row (the second cell from each end and the middle: its 1.4-wide box inside the portal). Every
//! crossing is printed by the portal it came out of. A world open (`liveness::restart`, save order, 2
//! idle ticks) at `--world-open-at` (default half the run) puts the gap bar on a world-open row
//! (ironspec 01:50b): its evidence printed (chunks unloaded and loaded again, the scheduled ticks out
//! and back), the reload model keeping the Nether part in memory (unmodelled there, said in the row),
//! and how the row differs from the reference timing row (ironspec 02:33 (2)). The verdicts: every
//! item arrival lands at the loader's own portal on each side; every golem at its own copy's twin;
//! launches on both sides; the loop's items all there at every tick; each side's worst gap between
//! arrivals within the 300-gt PORTAL ticket; the world open happened; no world error.

use std::collections::{BTreeMap, BTreeSet};

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, NetherSpec, Run, RunOpts, WalkMode};
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;

use crate::loaderlayout::{built, portals, Laid, Portal, C};

/// The run's options.
#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub ticks: i64,
    pub seed: i64,
    pub golem_at: i64,
    /// (ironspec 01:50b, the timing rule: every timing bar on the worst-jitter + world-open row) the
    /// tick of the world open the gap bar spans: `liveness::restart` with `Restart::load` (save order,
    /// 2 idle ticks: the larger of a world open's 1 or 2).
    pub world_open: i64,
}

fn bp(c: C) -> BlockPos {
    BlockPos::new(c[0], c[1], c[2])
}

/// The portal (of `ps`, one dimension) whose rectangle holds the point: the entity's block inside the
/// portal's cell box grown 1 across its plane (an arrival stands in its portal's cells, centred on the
/// plane).
fn portal_of(ps: &[&Portal], pos: [f64; 3]) -> Option<String> {
    let b = [pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32];
    ps.iter()
        .find(|p| {
            let (lo, hi) = p.lo_hi();
            let g = if p.axis == 'x' { [0, 1, 1] } else { [1, 1, 0] };
            (0..3).all(|k| b[k] >= lo[k] - g[k] && b[k] <= hi[k] + g[k])
        })
        .map(|p| p.name.clone())
}

/// The observers of `cells` that face another observer facing back: (the pair's two cells).
fn observer_pairs(cells: &BTreeMap<C, String>) -> Vec<(C, C)> {
    let step = |s: &str| -> Option<C> {
        let f = s.split_once("facing=")?.1.split([',', ']']).next()?;
        match f {
            "north" => Some([0, 0, -1]),
            "south" => Some([0, 0, 1]),
            "west" => Some([-1, 0, 0]),
            "east" => Some([1, 0, 0]),
            "up" => Some([0, 1, 0]),
            "down" => Some([0, -1, 0]),
            _ => None,
        }
    };
    let mut out = Vec::new();
    for (p, s) in cells.iter().filter(|(_, s)| s.starts_with("minecraft:observer")) {
        let Some(d) = step(s) else { continue };
        let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        if let Some(t) = cells.get(&q).filter(|t| t.starts_with("minecraft:observer"))
            && step(t) == Some([-d[0], -d[1], -d[2]])
            && p < &q
        {
            out.push((*p, q));
        }
    }
    out
}

/// The loop's items in a world: the kinds the loader's droppers held at the start, in containers and as
/// item entities.
fn loop_items_in(w: &redsim::world::World, kinds: &BTreeSet<String>) -> u32 {
    use redsim::redstone::container as rc;
    let ids: Vec<u8> = kinds.iter().filter_map(|k| rc::item_id(k)).collect();
    let inv: u32 = w.containers.map.values().map(|c| c.slots.iter().flatten().filter(|s| ids.contains(&s.item)).map(|s| s.count as u32).sum::<u32>()).sum();
    let ents: u32 = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .map(|e| match &e.kind {
            EntityKind::Item(s) if kinds.contains(s.item.trim_start_matches("minecraft:")) => s.count as u32,
            _ => 0,
        })
        .sum();
    inv + ents
}

/// The items an SNBT container line holds: (item id without the namespace, count).
fn snbt_items(s: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find("id:\"") {
        let after = &rest[i + 4..];
        let Some(end) = after.find('"') else { break };
        let id = after[..end].trim_start_matches("minecraft:").to_string();
        let tail = &after[end..];
        let count = tail.find("count:").and_then(|c| tail[c + 6..].split(|ch: char| !ch.is_ascii_digit()).next().and_then(|n| n.parse().ok())).unwrap_or(1);
        out.push((id, count));
        rest = &after[end..];
    }
    out
}

/// The loop's items: what the loader's droppers hold in its `nbt:` / `nether nbt:` lines (a clock's own
/// items, in its hoppers, are not the loop's): (kinds, total).
fn loop_items(l: &Laid) -> (BTreeSet<String>, u32) {
    let mut kinds = BTreeSet::new();
    let mut n = 0;
    for (cells, bes) in [(&l.blocks, &l.block_entities), (&l.nether, &l.nether_block_entities)] {
        for (c, s) in bes {
            if cells.get(c).is_some_and(|b| b.starts_with("minecraft:dropper")) {
                for (id, k) in snbt_items(s) {
                    kinds.insert(id);
                    n += k;
                }
            }
        }
    }
    (kinds, n)
}

/// The skeleton's spec.
fn spec_of(parts: &[Laid]) -> Result<FarmSpec, String> {
    let ps = portals(parts, &[])?;
    let (ow, _) = built(parts, false, &[])?;
    let (nw, _) = built(parts, true, &[])?;
    let li = parts.iter().position(|p| p.name == "loader").ok_or("no loader part")?;
    let mut o: BTreeMap<C, String> = parts[li].blocks.clone();
    let mut n: BTreeMap<C, String> = parts[li].nether.clone();
    for p in ps.iter().filter(|p| p.part != li) {
        let m = if p.nether { &nw } else { &ow };
        let dst = if p.nether { &mut n } else { &mut o };
        for c in p.cells.iter().copied().chain(p.frame(m)) {
            if let Some((s, _)) = m.get(&c) {
                dst.insert(c, s.clone());
            }
        }
    }
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for c in o.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k] - 4);
            hi[k] = hi[k].max(c[k] + 4);
        }
    }
    // (the farm judge's rule, circuit::studies::mobfarm::farm_block: a portal block is a terrain block)
    let block = |s: &str| -> Result<Block, String> { if s.starts_with("minecraft:nether_portal[") { redsim::terrain::land::block_of(s) } else { Block::parse(s) } };
    let parse = |m: &BTreeMap<C, String>| -> Result<Vec<(BlockPos, Block)>, String> { m.iter().map(|(c, s)| Ok((bp(*c), block(s)?))).collect() };
    let spec = FarmSpec {
        blocks: parse(&o)?,
        entities: Vec::new(),
        // (the runner's output chest: a corner of the box, away from every cell)
        output: BlockPos::new(lo[0], lo[1], lo[2]),
        lo: bp(lo),
        hi: bp(hi),
        block_entities: parts[li].block_entities.iter().map(|(c, s)| (bp(*c), s.clone())).collect(),
        rock: false,
        nether: Some(NetherSpec { anchor: BlockPos::new(0, 0, 0), blocks: parse(&n)?, output: None, block_entities: parts[li].nether_block_entities.iter().map(|(c, s)| (bp(*c), s.clone())).collect() }),
        land: false,
    };
    Ok(spec)
}

/// One seed's run; (text, verdict).
pub fn run(parts: &[Laid], o: &Opts) -> Result<(String, Verdict), String> {
    let ps = portals(parts, &[])?;
    let li = parts.iter().position(|p| p.name == "loader").ok_or("no loader part")?;
    let (lo_ow, lo_n) = (ps.iter().find(|p| p.part == li && !p.nether).ok_or("no loader overworld portal")?, ps.iter().find(|p| p.part == li && p.nether).ok_or("no loader Nether portal")?);
    let ow_ps: Vec<&Portal> = ps.iter().filter(|p| !p.nether).collect();
    let n_ps: Vec<&Portal> = ps.iter().filter(|p| p.nether).collect();
    let spec = spec_of(parts)?;
    let (kinds, items0) = loop_items(&parts[li]);
    if items0 == 0 {
        return Err("the loader's droppers hold no item (the loop's items are what its droppers hold): nothing to follow, refused".into());
    }
    let mut s = format!(
        "redsim run (seed {}, {} gt; bench, tool unreviewed): the portal skeleton ({} overworld cells: the loader's and each copy's portal cells with their frame obsidian; {} Nether cells); the loop's items (its droppers'): {items0} ({})\n",
        o.seed,
        o.ticks,
        spec.blocks.len(),
        spec.nether.as_ref().map_or(0, |n| n.blocks.len()),
        kinds.iter().cloned().collect::<Vec<_>>().join(", ")
    );
    let ro = RunOpts::steady(o.seed, (o.ticks + 200) as u32, WalkMode::Random);
    let mut run = Run::start(&spec, &ro)?;
    // (each world's level random and drop seed, set before the settle: a clock that starts as it is
    // built (a hopper clock) fires its droppers while the layout settles, and a dropper's spit draws
    // from them)
    for wi in 0..2 {
        let w = if wi == 0 { &mut run.world } else { run.world.nether.as_deref_mut().ok_or("no Nether world")? };
        let k = if wi == 0 { 0 } else { 0x4e45 };
        w.level_random = Some(redsim::rng::LegacyRandom::new(o.seed ^ k));
        w.drop_seed = Some(o.seed ^ k);
    }
    while run.settle_tick() {}
    // (the clocks' start, as the build starts an observer pair: each side's observer farther from its
    // portal taken out and put back, read back; a side with no observer pair (a hopper clock) needs none)
    let air = Block::parse("minecraft:air")?;
    for (wi, cells, portal) in [(0usize, &parts[li].blocks, lo_ow), (1usize, &parts[li].nether, lo_n)] {
        let side = if wi == 0 { "overworld" } else { "Nether" };
        let pairs = observer_pairs(cells);
        let (a, b) = match pairs.as_slice() {
            [] => {
                s.push_str(&format!("  the {side} clock: no observer pair (nothing taken out: a clock that starts as it is built)\n"));
                continue;
            }
            [(a, b)] => (*a, *b),
            _ => return Err(format!("the loader's {side} side: {} observer pairs (its clock: at most one)", pairs.len())),
        };
        let plane = |c: &C| if portal.axis == 'x' { (c[2] - portal.cells[0][2]).abs() } else { (c[0] - portal.cells[0][0]).abs() };
        let o2 = if plane(&a) >= plane(&b) { a } else { b };
        let w = if wi == 0 { &mut run.world } else { run.world.nether.as_deref_mut().ok_or("no Nether world")? };
        let st = w.get(bp(o2)).clone();
        w.set_block(bp(o2), air.clone(), redsim::world::flags::ALL);
        w.set_block(bp(o2), st.clone(), redsim::world::flags::ALL);
        let back = w.get(bp(o2)).clone();
        s.push_str(&format!("  the {side} clock's start: O2 ({},{},{}) taken out and put back; read back: {back}{}\n", o2[0], o2[1], o2[2], if back == st { "" } else { " (CHANGED: refused)" }));
        if back != st {
            return Err("the clock's O2 did not read back".into());
        }
    }
    let start = run.world.time;
    let mut seen = run.world.crossings.len();
    let mut golems: BTreeMap<u32, (String, [f64; 3])> = BTreeMap::new();
    let mut arrivals: [BTreeMap<String, usize>; 2] = [BTreeMap::new(), BTreeMap::new()];
    let mut times: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    let mut golem_out: Vec<(String, [f64; 3], String, i64)> = Vec::new();
    let mut first_launch: [Option<i64>; 2] = [None, None];
    let mut launches = [0usize; 2];
    let mut copies: BTreeSet<u32> = BTreeSet::new();
    let mut known: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
    let mut min_items = u32::MAX;
    let mut opened: Option<bool> = None;
    for _ in 0..o.ticks {
        let rel = run.world.time - start;
        if rel == o.world_open && opened.is_none() {
            // (ironspec 01:50b: the gap bar spans a world open; the overworld's entities come back under
            // new ids, so its items then are not launches)
            let r = crate::liveness::restart(&mut run.world, crate::liveness::Restart::load(redsim::reload::Order::Save))?;
            let happened = r.chunks >= 1 && r.ticks_back == r.ticks_out;
            opened = Some(happened);
            s.push_str(&format!(
                "  the world open at t {rel} (save order, {} idle ticks: the larger): {} chunks unloaded and loaded again, scheduled ticks {} out / {} back, {} entities held, game time t {} after it; the Nether part {}: {}\n",
                crate::liveness::WORLD_OPEN_IDLE,
                r.chunks,
                r.ticks_out,
                r.ticks_back,
                r.held,
                r.at - start,
                if r.nether { "kept in memory (the reload model saves only the overworld: unmodelled there)" } else { "absent" },
                if happened { "it happened" } else { "FAIL: it did not happen" }
            ));
            for e in run.world.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Item(_))) {
                known[0].insert(e.id);
            }
        }
        if rel == o.golem_at {
            for p in ps.iter().filter(|p| p.part != li && !p.nether) {
                let (lo, hi) = p.lo_hi();
                // (the second cell from each end and the middle: a 1.4-wide golem's box inside the portal)
                let along = |a: i32, b: i32| [(a + 1).min(b), (a + b) / 2, (b - 1).max(a)];
                let xs = if p.axis == 'x' { along(lo[0], hi[0]) } else { [lo[0]; 3] };
                let zs = if p.axis == 'x' { [lo[2]; 3] } else { along(lo[2], hi[2]) };
                for k in 0..3 {
                    let at = [xs[k] as f64 + 0.5, lo[1] as f64, zs[k] as f64 + 0.5];
                    let id = redsim::entity::spawn(&mut run.world, "minecraft:iron_golem", at, Some("{PersistenceRequired:1b}"))?;
                    golems.insert(id, (p.name.clone(), at));
                }
            }
        }
        run.tick();
        let n = run.world.nether.as_deref().ok_or("no Nether world")?;
        if run.world.away.is_none() && n.away.is_none() {
            min_items = min_items.min(loop_items_in(&run.world, &kinds) + loop_items_in(n, &kinds));
        }
        let cr: Vec<(i64, u32, u32)> = run.world.crossings[seen..].to_vec();
        seen = run.world.crossings.len();
        for (_, from, copy) in cr {
            copies.insert(copy);
            let (dim, pos, kind) = if let Some(i) = redsim::entity::index_of(n, copy) {
                let e = &n.entities.list[i];
                (1usize, [e.pos.x, e.pos.y, e.pos.z], matches!(e.kind, EntityKind::Item(_)))
            } else if let Some(i) = redsim::entity::index_of(&run.world, copy) {
                let e = &run.world.entities.list[i];
                (0usize, [e.pos.x, e.pos.y, e.pos.z], matches!(e.kind, EntityKind::Item(_)))
            } else {
                s.push_str(&format!("  t {rel}: crossing {from} -> {copy}: the copy is in neither world\n"));
                continue;
            };
            let at = portal_of(if dim == 1 { &n_ps } else { &ow_ps }, pos).unwrap_or_else(|| format!("no portal ({:.2},{:.2},{:.2})", pos[0], pos[1], pos[2]));
            if let Some((pad, p0)) = golems.get(&from) {
                golem_out.push((pad.clone(), *p0, at, rel));
            } else if kind {
                *arrivals[dim].entry(at).or_default() += 1;
                times[dim].push(rel);
            } else {
                s.push_str(&format!("  t {rel}: a non-item crossing {from} -> {copy} into {at}\n"));
            }
        }
        // (a launch: a new item entity no crossing made, on each side)
        for (wi, w) in [(0usize, &run.world), (1usize, n)] {
            for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Item(_))) {
                if known[wi].insert(e.id) && !copies.contains(&e.id) {
                    launches[wi] += 1;
                    first_launch[wi].get_or_insert(rel);
                }
            }
        }
    }
    let n = run.world.nether.as_deref().ok_or("no Nether world")?;
    let items_end = loop_items_in(&run.world, &kinds) + loop_items_in(n, &kinds);
    let mut ok = true;
    // (the world-open row: the open must have happened inside the run)
    match opened {
        Some(true) => {}
        Some(false) => ok = false,
        None => {
            ok = false;
            s.push_str(&format!("  the world open at t {}: never reached in {} gt: FAIL (the gap bar needs its world-open row)\n", o.world_open, o.ticks));
        }
    }
    for (dim, own, label) in [(1usize, &lo_n.name, "into the Nether"), (0usize, &lo_ow.name, "into the overworld")] {
        let a = &arrivals[dim];
        let total: usize = a.values().sum();
        let elsewhere: usize = a.iter().filter(|(k, _)| *k != own).map(|(_, v)| v).sum();
        let mut gaps: Vec<i64> = times[dim].windows(2).map(|w| w[1] - w[0]).collect();
        if let Some(l) = times[dim].last() {
            gaps.push(o.ticks - 1 - l);
        }
        let worst = gaps.iter().copied().max().unwrap_or(o.ticks);
        let first = times[dim].first().copied();
        let pass = total > 0 && elsewhere == 0 && worst <= 300;
        ok &= pass;
        s.push_str(&format!(
            "  item arrivals {label}: {total} ({}), the first at t {}, the worst gap after it {worst} gt, the world open included (the PORTAL ticket: 300, {} gt left): {}\n",
            a.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "),
            first.map_or("-".to_string(), |f| f.to_string()),
            300 - worst,
            if pass { "PASS" } else { "FAIL" }
        ));
    }
    // (ironspec 02:33 (2): a row that differs from the reference timing row says how)
    s.push_str(&format!(
        "  the gap bar's row against the reference timing row (ironfarm's flags: each clocked event +-2 gt in the tightening direction, a k + j = 3 world open at every phase, the worst over phases): it differs: ONE world open at t {} (save order, 2 idle ticks), no +-2 gt shift of the loader's clocked events; the game's 01:20 restart rows (tick-exact) are the loader's world-open evidence\n",
        o.world_open
    ));
    s.push_str(&format!(
        "  launches (a new item entity no crossing made; read back: the clocks run): overworld side {} (the first at t {}), Nether side {} (the first at t {}; its clock ticks once an arrival tickets it)\n",
        launches[0],
        first_launch[0].map_or("never".into(), |t| t.to_string()),
        launches[1],
        first_launch[1].map_or("never".into(), |t| t.to_string())
    ));
    if first_launch[0].is_none() || first_launch[1].is_none() {
        ok = false;
    }
    let kept = items_end == items0 && min_items == items0;
    ok &= kept;
    s.push_str(&format!("  the items: {items0} laid, {items_end} at the end, at least {min_items} at every tick: {}\n", if kept { "PASS" } else { "FAIL" }));
    let mut per_pad: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut gfail = 0;
    let crossed: BTreeSet<&String> = golem_out.iter().map(|g| &g.0).collect();
    for (pad, p0, at, t) in &golem_out {
        let want = ps.iter().find(|p| p.name == *pad).map(|p| ps.iter().find(|q| q.part == p.part && q.nether).map(|q| q.name.clone()).unwrap_or_default()).unwrap_or_default();
        if *at != want {
            gfail += 1;
        }
        per_pad.entry(pad.clone()).or_default().push(format!("({:.1},{:.1},{:.1}) -> {at} at t {t}{}", p0[0], p0[1], p0[2], if *at == want { "" } else { " FAIL" }));
    }
    let placed = golems.len();
    if golem_out.len() != placed || gfail > 0 {
        ok = false;
    }
    s.push_str(&format!("  golems: {placed} placed at t {} ({} pads), {} crossed, {gfail} at another portal than their copy's twin: {}\n", o.golem_at, crossed.len(), golem_out.len(), if golem_out.len() == placed && gfail == 0 { "PASS" } else { "FAIL" }));
    for (pad, v) in &per_pad {
        s.push_str(&format!("    {pad}: {}\n", v.join("; ")));
    }
    let tickets: Vec<String> = n.portal_tickets.iter().map(|(c, t)| format!("({},{}) renewed t {}", c[0], c[1], t - start)).collect();
    s.push_str(&format!("  the Nether's PORTAL tickets at the end: {}\n", if tickets.is_empty() { "none".into() } else { tickets.join(", ") }));
    let errs: Vec<String> = run.world.errors.iter().map(|e| format!("overworld: {e}")).chain(n.errors.iter().map(|e| format!("Nether: {e}"))).collect();
    s.push_str(&format!("  world errors (redsim's model errors: what it can't model): {}\n", if errs.is_empty() { "none".into() } else { errs.join("; ") }));
    // (ironspec 02:45 (3): a bench's exit reflects the adopted bars only; a run redsim can't model
    // judges nothing: NOT MODELED by name, its bars above information)
    let v = if !errs.is_empty() {
        Verdict::NotModeled
    } else if ok {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    s.push_str(&format!(
        "  redsim verdict (seed {}): {}\n",
        o.seed,
        match v {
            Verdict::Pass => "PASS".to_string(),
            Verdict::Fail => "FAIL".to_string(),
            Verdict::NotModeled => format!("NOT MODELED ({} model errors: the bars above are information, not a judgment)", errs.len()),
        }
    ));
    Ok((s, v))
}

/// A run's verdict: its bars met or not, or a run redsim can't model (a world error), which judges
/// nothing (ironspec 02:45 (3)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    NotModeled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_lines_give_their_items() {
        assert_eq!(snbt_items(r#"{Items:[{Slot:0b,id:"minecraft:leather",count:1}]}"#), vec![("leather".to_string(), 1)]);
        assert_eq!(
            snbt_items(r#"{Items:[{Slot:0b,id:"minecraft:cobblestone",count:2},{Slot:1b,id:"minecraft:stone"}]}"#),
            vec![("cobblestone".to_string(), 2), ("stone".to_string(), 1)]
        );
        assert!(snbt_items("{}").is_empty());
    }

    #[test]
    fn the_loop_items_are_the_droppers_not_the_clock_hoppers() {
        let mut l = Laid { name: "loader".into(), ..Default::default() };
        l.blocks.insert([0, 0, 0], "minecraft:dropper[facing=north,triggered=false]".into());
        l.blocks.insert([1, 0, 0], "minecraft:hopper[enabled=true,facing=east]".into());
        l.block_entities.insert([0, 0, 0], r#"{Items:[{Slot:0b,id:"minecraft:cobblestone",count:2}]}"#.into());
        l.block_entities.insert([1, 0, 0], r#"{Items:[{Slot:0b,id:"minecraft:stone",count:13}]}"#.into());
        l.nether.insert([3, 0, 0], "minecraft:dropper[facing=south,triggered=false]".into());
        l.nether_block_entities.insert([3, 0, 0], r#"{Items:[{Slot:0b,id:"minecraft:cobblestone",count:1}]}"#.into());
        let (kinds, n) = loop_items(&l);
        assert_eq!((kinds.into_iter().collect::<Vec<_>>(), n), (vec!["cobblestone".to_string()], 3));
    }

    #[test]
    fn observer_pairs_and_portal_membership() {
        let mut cells: BTreeMap<C, String> = BTreeMap::new();
        cells.insert([1, 13, 2], "minecraft:observer[facing=south,powered=false]".into());
        cells.insert([1, 13, 3], "minecraft:observer[facing=north,powered=false]".into());
        cells.insert([5, 13, 2], "minecraft:observer[facing=south,powered=false]".into());
        assert_eq!(observer_pairs(&cells), vec![([1, 13, 2], [1, 13, 3])]);
        let p = Portal { name: "loader.overworld".into(), part: 0, nether: false, cells: (0..5).flat_map(|x| (90..95).map(move |y| [x, y, 19])).collect(), axis: 'x' };
        assert_eq!(portal_of(&[&p], [3.5, 94.4, 19.3]), Some("loader.overworld".into()));
        assert_eq!(portal_of(&[&p], [3.5, 94.4, 20.9]), Some("loader.overworld".into()));
        assert_eq!(portal_of(&[&p], [5.5, 94.4, 19.5]), None);
    }
}
