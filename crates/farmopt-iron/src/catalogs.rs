//! The part catalogs (vjeux 2026-11-08 11:24: "breakdown the various part of the farms to
//! optimize them individually ... like we did for the clocks"): every part of the iron grammar
//! measured on its own objectives, one row per variant, its Pareto members marked, written to
//! `designs/farms/iron/parts/<part>.tsv` (`mctas farmopt parts`). The farm search assembles them;
//! what depends on two parts at once (a pad's spawn share needs its cage's summoners, a kill's dwell
//! the villagers' follow cubes) is measured with the reference part it depends on, and checked
//! again exactly on every assembled build.

use crate::calm::Part;
use crate::eval::{self, Ctx};
use crate::parts::{self, CalmKind, Cage, Kill, Module, Pad, Place};
use anyhow::Result;
use mobfarm::layouts::Builder;
use std::fmt::Write as _;

fn count(b: &Builder) -> usize {
    b.d.blocks.values().filter(|s| !s.ends_with(":air") && s.as_str() != "minecraft:air").count()
}

/// One catalog row: its key, its metrics (name, value), whether it is on the part's front.
pub struct Row {
    pub key: String,
    pub metrics: Vec<(&'static str, f64)>,
    pub front: bool,
}

/// Mark the rows on the front of `objs` (metric name, max?).
fn mark_front(rows: &mut [Row], objs: &[(&str, bool)]) {
    let pts: Vec<Vec<f64>> = rows.iter().map(|r| objs.iter().map(|(m, max)| r.metrics.iter().find(|(k, _)| k == m).map(|(_, v)| if *max { *v } else { -*v }).unwrap_or(f64::NEG_INFINITY)).collect()).collect();
    for i in farmopt::pareto::front(&pts) {
        rows[i].front = true;
    }
}

/// The cages: blocks (void: glass, stone roofs, beds, trapdoors, the hide and its driver), villagers,
/// scared villagers (the summoners: 10 attempts each per mark). Front: fewest blocks for the most
/// scared and the most villagers, per calm kind.
pub fn cages(stage: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for calm in [CalmKind::Plate, CalmKind::Observer] {
        if calm == CalmKind::Observer && stage == "day-1" {
            continue;
        }
        let (scareds, wests): (Vec<u8>, Vec<bool>) = match calm {
            CalmKind::Plate => (vec![1, 2], vec![false]),
            CalmKind::Observer => (vec![2, 3, 4], vec![false, true]),
        };
        let mut part = Vec::new();
        for &scared in &scareds {
            for blind in 0..=3u8 {
                for &u1_west in &wests {
                    let c = Cage { scared, blind, calm, u1_west };
                    let villagers = scared + blind + 1;
                    if villagers < 3 {
                        continue;
                    }
                    let mut b = Builder::new("cage");
                    let m = Module { cage: c, pad: Pad { n: 6, floor: -7, dx: 0 }, kill: Kill::Pit { k: -13 } };
                    parts::module(&mut b, [0, 0, 0], Place::Void, &m, true);
                    let key = format!("{}-s{scared}b{blind}{}", if calm == CalmKind::Plate { "plate" } else { "obs" }, if u1_west { "-u1w" } else { "" });
                    part.push(Row { key, metrics: vec![("blocks", cage_blocks(&b) as f64), ("villagers", villagers as f64), ("scared", scared as f64)], front: false });
                }
            }
        }
        mark_front(&mut part, &[("blocks", false), ("scared", true), ("villagers", true)]);
        rows.extend(part);
    }
    rows
}

/// The blocks of a module's cage: every block at y >= -1 north of the pad (z <= 1) or in the plate
/// pod and the hide (z -7..1).
fn cage_blocks(b: &Builder) -> usize {
    b.d.blocks.iter().filter(|(p, s)| p[1] >= -1 && p[2] <= 1 && !s.ends_with(":air")).count()
}

/// The pads under a reference cage (the observer cage with 3 scared villagers, or the day-1 bed
/// plate with 2): the mark's success bound p from the scared villagers' exact spawn cells, the
/// pad's blocks. Front: the highest p for the fewest blocks.
pub fn pads(stage: &str, ctx: &Ctx) -> Vec<Row> {
    let cage = if stage == "day-1" { Cage { scared: 2, blind: 0, calm: CalmKind::Plate, u1_west: false } } else { Cage { scared: 3, blind: 0, calm: CalmKind::Observer, u1_west: true } };
    let mut rows = Vec::new();
    for n in [6u8, 8, 10] {
        for floor in [-7i8, -6, -5] {
            for dx in [-1i8, 0, 1] {
                let m = Module { cage, pad: Pad { n, floor, dx }, kill: Kill::Pit { k: floor - 6 } };
                let c = crate::cand::Cand { seed: None, module: m, modules: 1, tiling: crate::cand::Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0 };
                let Ok(e) = eval::eval_static(&c, ctx) else { continue };
                let p = e.metrics.get("mark_success").map(|v| v.v).unwrap_or(0.0);
                let share = e.metrics.get("spawn_share").map(|v| v.v).unwrap_or(0.0);
                let blocks = (n as usize) * (n as usize) + 4 * (n as usize + 2) * 2;
                rows.push(Row { key: format!("pad{n}f{}x{dx}", -floor), metrics: vec![("mark_success", p), ("spawn_share", share), ("blocks", blocks as f64)], front: false });
            }
        }
    }
    mark_front(&mut rows, &[("mark_success", true), ("blocks", false)]);
    rows
}

/// The kills: lava sources (a bucket trip each on day 1), blocks (void), whether the golem dies in
/// the villagers' smallest follow cube (in sight: every villager reset by every golem, the bound's
/// +100 gt) and the drop's depth. Front: fewest lava and blocks, out of sight.
pub fn kills() -> Vec<Row> {
    let mut rows = Vec::new();
    let cage = Cage { scared: 2, blind: 0, calm: CalmKind::Plate, u1_west: false };
    for kill in (0..6).map(|i| Kill::Pit { k: -13 - 2 * i }).chain([-19i8, -22, -25].map(|k| Kill::Blade { k })) {
        let m = Module { cage, pad: Pad { n: 6, floor: -7, dx: 0 }, kill };
        let mut b = Builder::new("kill");
        parts::module(&mut b, [0, 0, 0], Place::Void, &m, true);
        let kill_blocks = b.d.blocks.iter().filter(|(p, s)| p[1] < -7 && !s.ends_with(":air")).count();
        let k = kill.k() as f64;
        // in sight: the kill's golem feet above the smallest cube's floor (villager feet 0.5625 -
        // 14.16 - 2.7)
        let in_sight = k > 0.5625 - eval::R_MIN - 2.7;
        let key = match kill {
            Kill::Pit { k } => format!("pit{}", -k),
            Kill::Blade { k } => format!("blade{}", -k),
        };
        rows.push(Row { key, metrics: vec![("lava_sources", kill.lava() as f64), ("blocks", kill_blocks as f64), ("out_of_sight", if in_sight { 0.0 } else { 1.0 }), ("depth", -k)], front: false });
    }
    mark_front(&mut rows, &[("lava_sources", false), ("blocks", false), ("out_of_sight", true)]);
    rows
}

/// The calm parts: each catalog timer placed and routed behind a reference observer cage (3 scared,
/// U1 east or west), its blocks with the wiring. Front: fewest blocks.
pub fn calms(parts_list: &[Part]) -> Vec<Row> {
    let mut rows = Vec::new();
    for (i, part) in parts_list.iter().enumerate() {
        for u1_west in [false, true] {
            let m = Module { cage: Cage { scared: 3, blind: 0, calm: CalmKind::Observer, u1_west }, pad: Pad { n: 6, floor: -7, dx: 0 }, kill: Kill::Blade { k: -22 } };
            let mut b = Builder::new("calm");
            let p = parts::module(&mut b, [0, 0, 0], Place::Void, &m, true);
            let before = count(&b);
            b.d.ports.insert("output".into(), p.output);
            b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
            let ok = crate::calm::wire(&mut b, part, &p, [0, 0, 0]).is_ok();
            let wired = if ok { (count(&b) - before) as f64 } else { f64::INFINITY };
            rows.push(Row { key: format!("{i}:{}{}", part.name, if u1_west { "-u1w" } else { "" }), metrics: vec![("part_blocks", part.blocks as f64), ("wired_blocks", wired), ("hide_gt", part.length as f64)], front: false });
        }
    }
    mark_front(&mut rows, &[("wired_blocks", false)]);
    rows
}

/// The collections: the kill's hoppers into one chest (the pit 1, the blade 2): hoppers (5 iron
/// each, a server cost each tick), slots, the hours they hold at one module's ceiling (480 iron and
/// 120 poppies an hour). The late stage's shared sorter, iron-block crafter and composter come
/// from the ironfarm lane's `collect_bottom` once it is a part.
pub fn collections() -> Vec<Row> {
    let mut rows = Vec::new();
    for (key, hoppers, slots) in [("pit-hopper-chest", 1.0, 27.0 + 5.0), ("blade-2-hoppers-chest", 2.0, 27.0 + 10.0)] {
        let hours = (slots - 2.0) * 64.0 / 600.0;
        rows.push(Row { key: key.into(), metrics: vec![("hoppers", hoppers), ("slots", slots), ("storage_hours", hours)], front: false });
    }
    mark_front(&mut rows, &[("hoppers", false), ("storage_hours", true)]);
    rows
}

fn tsv(rows: &[Row]) -> String {
    let mut s = String::new();
    if let Some(r) = rows.first() {
        let _ = writeln!(s, "key\tfront\t{}", r.metrics.iter().map(|(k, _)| *k).collect::<Vec<_>>().join("\t"));
    }
    for r in rows {
        let _ = writeln!(s, "{}\t{}\t{}", r.key, if r.front { "yes" } else { "" }, r.metrics.iter().map(|(_, v)| format!("{v:.3}")).collect::<Vec<_>>().join("\t"));
    }
    s
}

/// Write every part catalog of a stage; returns one summary line per part.
pub fn write_all(stage: &str, dir: &std::path::Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(dir)?;
    let ctx = Ctx { calm_parts: crate::calm::parts(stage), stage: crate::price::stage_ix(stage) };
    let sets: Vec<(&str, Vec<Row>)> = vec![("cage", cages(stage)), ("pad", pads(stage, &ctx)), ("kill", kills()), ("calm", calms(&ctx.calm_parts)), ("collection", collections())];
    let mut lines = Vec::new();
    for (name, rows) in sets {
        std::fs::write(dir.join(format!("{stage}-{name}.tsv")), tsv(&rows))?;
        let front: Vec<&Row> = rows.iter().filter(|r| r.front).collect();
        let best: Vec<String> = front.iter().take(4).map(|r| format!("{} ({})", r.key, r.metrics.iter().map(|(k, v)| format!("{k} {v:.2}")).collect::<Vec<_>>().join(", "))).collect();
        lines.push(format!("{name}: {} variants, {} on the front; {}", rows.len(), front.len(), best.join("; ")));
    }
    Ok(lines)
}

/// One member of a site catalog (designs/farms/iron/parts/<part>/members.tsv, the page-copy lane's
/// format): its file stem, player-facing name, the lowest stage whose materials it fits, its
/// metrics (`key value, key value`), and the design of the part alone.
struct Member {
    stem: String,
    name: String,
    stage: &'static str,
    metrics: Vec<(String, String)>,
    design: String,
}

/// The part alone: the module's blocks that `keep` names (module coordinates), the entities when
/// `entities`, as a design file (no circuit line: no judge runs on it).
fn alone(m: &Module, title: &str, caption: &str, keep: &dyn Fn([i32; 3], &str) -> bool, entities: bool, calm: Option<&Part>) -> Result<(String, Vec<String>, Vec<(String, String)>)> {
    let mut b = Builder::new(title);
    let p = parts::module(&mut b, [0, 0, 0], Place::Void, m, true);
    if let Some(part) = calm {
        b.d.ports.insert("output".into(), p.output);
        b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
        crate::calm::wire(&mut b, part, &p, [0, 0, 0])?;
        b.d.ports.clear();
        b.d.meta.clear();
    }
    let mut out = Builder::new(title);
    for (q, s) in &b.d.blocks {
        if !s.ends_with(":air") && keep(*q, s) {
            out.d.blocks.insert(*q, s.clone());
        }
    }
    for (q, s) in &b.d.block_entities {
        if out.d.blocks.contains_key(q) {
            out.d.block_entities.insert(*q, s.clone());
        }
    }
    if entities {
        out.d.entities = b.d.entities.clone();
    }
    let states: Vec<String> = out.d.blocks.values().cloned().collect();
    let ents: Vec<(String, String)> = out.d.entities.iter().map(|e| (e.kind.clone(), e.nbt.clone().unwrap_or_default())).collect();
    Ok((out.text(&[caption]), states, ents))
}

fn seconds(states: &[String], ents: &[(String, String)], stage: usize) -> f64 {
    let s: Vec<&str> = states.iter().map(|x| x.as_str()).collect();
    let e: Vec<(&str, &str)> = ents.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    crate::price::bill(&s, &e, 0, &[], stage).map(|b| b.seconds).unwrap_or(f64::NAN)
}

/// The site catalogs: designs/farms/iron/parts/<part>/members.tsv (`file name stage metrics
/// used_by`, best first) and one design per member, the part alone. Returns one line per part.
pub fn write_site(dir: &std::path::Path) -> Result<Vec<String>> {
    let pad6 = Pad { n: 6, floor: -7, dx: 0 };
    let blade = Kill::Blade { k: -22 };
    let cage_of = |scared: u8, blind: u8, calm: CalmKind| Cage { scared, blind, calm, u1_west: calm == CalmKind::Observer };
    let mut parts_out: Vec<(&str, Vec<Member>)> = Vec::new();
    let middle_parts = crate::calm::parts("middle");
    let ctx_mid = Ctx { calm_parts: middle_parts.clone(), stage: 1 };

    // the cage: pods, villagers, the zombie in its boat, the hide (y >= -1, north of the pad)
    let mut cages_m = Vec::new();
    for (scared, blind, calm, stage) in [(2u8, 0u8, CalmKind::Plate, "day-1"), (2, 1, CalmKind::Plate, "day-1"), (3, 0, CalmKind::Observer, "middle"), (4, 0, CalmKind::Observer, "middle")] {
        let m = Module { cage: cage_of(scared, blind, calm), pad: pad6, kill: blade };
        let villagers = scared + blind + 1;
        let (name, how) = match calm {
            CalmKind::Plate => (format!("bed-plate cage, {scared} watching villager{}{}", if scared > 1 { "s" } else { "" }, if blind > 0 { format!(" + {blind} sleeper{}", if blind > 1 { "s" } else { "" }) } else { String::new() }), "the bed villager's pressure plate holds the hide open by day"),
            CalmKind::Observer => (format!("bed-observer cage, {scared} watching villagers"), "an observer on the sleeper's bed starts the timer that hides the zombie"),
        };
        let stem = format!("{}-{scared}-watching{}", if calm == CalmKind::Plate { "bed-plate" } else { "bed-observer" }, if blind > 0 { format!("-{blind}-sleeping") } else { String::new() });
        let (design, states, ents) = alone(&m, &stem, &format!("{name}: {villagers} villagers in 1-block pods, {scared} watching the zombie in its boat; {how}."), &|q, _| q[1] >= -1 && q[2] <= 1, true, None)?;
        let st = crate::price::stage_ix(stage);
        cages_m.push(Member { stem, name, stage, metrics: vec![("villagers".into(), villagers.to_string()), ("watching villagers".into(), scared.to_string()), ("blocks".into(), states.len().to_string()), ("build seconds".into(), format!("{:.0}", seconds(&states, &ents, st)))], design });
    }
    parts_out.push(("cage", cages_m));

    // the spawn area: the dry floor around the hole and its rims (floor - 1 .. floor + 2, south of
    // the pods), its spawn success from the exact cells under the 3 watching villagers
    let mut pads_m = Vec::new();
    for n in [6u8, 8, 10] {
        let m = Module { cage: cage_of(3, 0, CalmKind::Observer), pad: Pad { n, floor: -7, dx: 0 }, kill: blade };
        let c = crate::cand::Cand { seed: None, module: m, modules: 1, tiling: crate::cand::Tiling::Row(0), calm_part: 0, shared_calm: false, place: Place::Void, ground: 0, surface: false, l3_pits: 0 };
        let p = eval::eval_static(&c, &ctx_mid).ok().and_then(|e| e.metrics.get("mark_success").map(|v| v.v)).unwrap_or(f64::NAN);
        let stem = format!("dry-floor-{n}x{n}");
        let name = format!("{n}x{n} dry floor 7 below the beds");
        let fl = -7;
        let (design, states, ents) = alone(&m, &stem, &format!("{name}: golems spawn on the dry ring around the drop hole and walk into it."), &|q, _| q[2] >= 2 && (fl - 1..=fl + 3).contains(&q[1]), false, None)?;
        pads_m.push(Member { stem, name, stage: "day-1", metrics: vec![("spawn success per check".into(), format!("{:.0}%", p * 100.0)), ("floor cells".into(), (n as u32 * n as u32 - 16).to_string()), ("blocks".into(), states.len().to_string()), ("build seconds".into(), format!("{:.0}", seconds(&states, &ents, 0)))], design });
    }
    parts_out.push(("spawn-area", pads_m));

    // the kill: everything below the floor's rims
    let mut kills_m = Vec::new();
    for (kill, stage) in [(Kill::Pit { k: -13 }, "day-1"), (Kill::Pit { k: -17 }, "day-1"), (Kill::Blade { k: -22 }, "day-1")] {
        let m = Module { cage: cage_of(2, 0, CalmKind::Plate), pad: pad6, kill };
        let depth = (-7 - kill.k()) as i32;
        let in_sight = (kill.k() as f64) > 0.5625 - eval::R_MIN - 2.7;
        let (stem, name) = match kill {
            Kill::Pit { .. } => (format!("lava-corner-{depth}-drop"), format!("lava corner after a {depth}-block drop{}", if in_sight { " (the villagers still see it)" } else { " (out of the villagers' sight)" })),
            Kill::Blade { .. } => (format!("lava-blade-{depth}-drop"), format!("lava blade after a {depth}-block drop, fire-safe chamber")),
        };
        let (design, states, ents) = alone(&m, &stem, &format!("{name}."), &|q, _| q[1] < -8, false, None)?;
        kills_m.push(Member { stem, name, stage, metrics: vec![("lava buckets".into(), kill.lava().to_string()), ("in sight of the villagers".into(), (if in_sight { "yes" } else { "no" }).into()), ("drop".into(), format!("{depth} blocks")), ("blocks".into(), states.len().to_string()), ("build seconds".into(), format!("{:.0}", seconds(&states, &ents, 0)))], design });
    }
    parts_out.push(("kill", kills_m));

    // the sleep trigger: the bed plate (its pod and the hide), the catalog timers behind a bed
    // observer (the part, its wiring, U1's pod and the hide)
    let mut trig = Vec::new();
    {
        let m = Module { cage: cage_of(2, 0, CalmKind::Plate), pad: pad6, kill: blade };
        let (design, states, ents) = alone(&m, "bed-plate", "Bed plate: the bed villager's pod with a pressure plate; standing on it by day holds the trapdoor across the zombie's view open, lying down at night closes it.", &|q, _| q[1] >= -1 && q[2] <= -5, false, None)?;
        trig.push(Member { stem: "bed-plate".into(), name: "bed plate (no redstone)".into(), stage: "day-1", metrics: vec![("hide time".into(), "while the bed villager sleeps".into()), ("blocks".into(), states.len().to_string()), ("build seconds".into(), format!("{:.0}", seconds(&states, &ents, 0)))], design });
    }
    for part in middle_parts.iter().take(2) {
        let m = Module { cage: cage_of(3, 0, CalmKind::Observer), pad: pad6, kill: blade };
        let before = {
            let mut b = Builder::new("x");
            parts::module(&mut b, [0, 0, 0], Place::Void, &m, true);
            b.d.blocks.keys().copied().collect::<std::collections::BTreeSet<_>>()
        };
        let stem = timer_name(&part.name, part.length).replace(['(', ')'], "").replace(' ', "-");
        let name = format!("{} behind a bed observer", timer_name(&part.name, part.length));
        let (design, states, ents) = alone(&m, &stem, &format!("{name}: the sleeper's bed changes at dusk, the observer pulses, the timer holds the trapdoor across the zombie's view for {} ticks while the villagers lie down.", part.length), &|q, s| !before.contains(&q) || s.contains("trapdoor[facing=north,half=bottom") || (q[2] == -6 && q[1] >= 0), false, Some(part))?;
        trig.push(Member { stem, name, stage: "middle", metrics: vec![("hide time".into(), format!("{} ticks", part.length)), ("blocks with wiring".into(), states.len().to_string()), ("build seconds".into(), format!("{:.0}", seconds(&states, &ents, 1)))], design });
    }
    parts_out.push(("sleep-trigger", trig));

    // the collection: the kill's hoppers into a chest
    let mut coll = Vec::new();
    for (kill, stem, name) in [(Kill::Pit { k: -13 }, "one-hopper-chest", "one hopper into a chest"), (Kill::Blade { k: -22 }, "two-hoppers-chest", "two hoppers into a chest")] {
        let m = Module { cage: cage_of(2, 0, CalmKind::Plate), pad: pad6, kill };
        let mut b = Builder::new(stem);
        let p = parts::module(&mut b, [0, 0, 0], Place::Void, &m, true);
        b.set(p.output[0], p.output[1], p.output[2], parts::CHEST);
        let mut out = Builder::new(stem);
        for (q, s) in &b.d.blocks {
            if s.contains("hopper") || s.contains("chest") {
                out.d.blocks.insert(*q, s.clone());
            }
        }
        let hoppers = out.d.blocks.values().filter(|s| s.contains("hopper")).count();
        let states: Vec<String> = out.d.blocks.values().cloned().collect();
        let hours = (27.0 + 5.0 * hoppers as f64 - 2.0) * 64.0 / 600.0;
        coll.push(Member { stem: stem.into(), name: name.into(), stage: "day-1", metrics: vec![("hoppers".into(), hoppers.to_string()), ("storage hours at 480 iron/h".into(), format!("{hours:.1}")), ("blocks".into(), states.len().to_string()), ("build seconds".into(), format!("{:.0}", seconds(&states, &[], 0)))], design: out.text(&[&format!("{name}: the kill's drops go into the chest.")]) });
    }
    parts_out.push(("collection", coll));

    let mut lines = Vec::new();
    for (part, members) in parts_out {
        let d = dir.join(part);
        std::fs::create_dir_all(&d)?;
        let mut tsv = String::from("file\tname\tstage\tmetrics\tused_by\n");
        for m in &members {
            std::fs::write(d.join(format!("{}.txt", m.stem)), &m.design)?;
            let _ = writeln!(tsv, "{}.txt\t{}\t{}\t{}\t", m.stem, m.name, m.stage, m.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "));
        }
        std::fs::write(d.join("members.tsv"), tsv)?;
        lines.push(format!("{part}: {} members ({})", members.len(), members.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join("; ")));
    }
    Ok(lines)
}

/// A catalog timer's player-facing name from its archived name (`off-delay-ring-L24`).
fn timer_name(name: &str, length: u32) -> String {
    let what = if name.contains("ring") { "comparator ring timer" } else if name.contains("bulb") { "copper bulb timer" } else if name.contains("repeater") { "repeater line timer" } else { "timer" };
    format!("{what} ({length} ticks)")
}
