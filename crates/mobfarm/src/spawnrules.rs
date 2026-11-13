//! `mctas mobfarm spawn-rules`: the hostile checks' C1 test (`hostile::can_spawn_at`: the
//! placement, `natural::rules_can_pass` with every static part exact, the draws at their best and
//! the light at its worst, no collision) against the game's own at every interior cell of the farm
//! built in a terrain sample: the bridge's `spawnrules` op runs `SpawnPlacements
//! .isSpawnPositionOk`, the spawn box's `noCollision` and `SpawnPlacements.checkSpawnRules(type,
//! level, NATURAL, pos, random)` (each try with its own random) in the compare's terrain sandbox
//! (the farm built at its site, its settle run), for every type listed at the cells. Light: the
//! sandbox's at the end of the settle (day time `day0`); a darkness rule at a cell the sky reaches
//! can pass in redsim (the night's darkest) and not in the game's daylight: those are reported
//! apart.

use anyhow::{Result, anyhow, bail};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

use crate::exact::{GameFarm, Opts, Place, load_place, opts_of};
use redsim::entity::mob::natural::{Category, kind_of, mobs_at};
use redsim::pos::BlockPos;

pub fn run(version: &str, design: &std::path::Path, terrain: &str, tries: u32, day_time: i64) -> Result<()> {
    let opts = Opts { seed: 1, ticks: 0, weather: false, probe: false, batch: 100, full: false, bare: false, draws_at: None, terrain: Some(terrain.to_string()), day0: None, build: None, no_spawning: false, only: Vec::new() };
    let (c, spec, place) = load_place(design, &opts)?;
    let Place::Terrain { site, .. } = place else { bail!("spawn-rules needs a terrain sample") };
    let mut o = opts_of(&opts);
    o.site = Some(site);
    // redsim's side: the farm built at its site after its settle (as the card's exposures line)
    let w = circuit::mobfarm::settled_world(site, &spec);
    let summoners = {
        let mut with = w.clone();
        for (k, p, n) in &spec.entities {
            let _ = redsim::entity::spawn(&mut with, k, *p, n.as_deref());
        }
        redsim::entity::mob::site::panicking(&with, &spec.blocks)
    };
    let farm = circuit::mobfarm::hostile_farm(&w, &spec, site.afk, &summoners);
    // every type listed at an interior cell, and redsim's verdict per (type, cell)
    let mut ours: BTreeMap<String, BTreeSet<[i32; 3]>> = BTreeMap::new();
    let mut listed: BTreeMap<String, BTreeSet<[i32; 3]>> = BTreeMap::new();
    for &p in &farm.interior {
        for cat in [Category::Monster, Category::Creature, Category::Ambient] {
            for s in mobs_at(&w, cat, p) {
                let Some(k) = kind_of(&s.ty) else { continue };
                listed.entry(k.id.to_string()).or_default().insert([p.x, p.y, p.z]);
                if redsim::entity::mob::hostile::can_spawn_at(&w, k, p) {
                    ours.entry(k.id.to_string()).or_default().insert([p.x, p.y, p.z]);
                }
            }
        }
    }
    // the game's side: the compare's terrain sandbox, the farm built, its settle run, then the
    // clock set to `day_time` (default midnight: the darkest sky, what C1 assumes for a darkness
    // rule) and one tick so the level's sky darkening follows
    let mut game = GameFarm::start(version, &c, &spec, &o, false, false, place)?;
    game.trace(redsim::entity::mob::farm::SETTLE_TICKS as u32, false)?;
    game.op(json!({"op": "world", "day_time": day_time}))?;
    game.trace(1, false)?;
    let mut total_cells = 0;
    let mut disagree = 0;
    println!("{} in {}: {} interior cells, {} types listed there", design.display(), terrain, farm.interior.len(), listed.len());
    for (ty, cells) in &listed {
        let cells_json: Vec<[i32; 3]> = cells.iter().copied().collect();
        let v = game.op(json!({"op": "spawnrules", "types": [ty], "cells": cells_json, "tries": tries}))?;
        let rows = v["types"][0]["cells"].as_array().ok_or_else(|| anyhow!("bad spawnrules answer: {v}"))?;
        let mut theirs = BTreeSet::new();
        let mut parts: BTreeMap<[i32; 3], (bool, bool, i64)> = BTreeMap::new();
        for r in rows {
            let p = [r[0].as_i64().unwrap_or(0) as i32, r[1].as_i64().unwrap_or(0) as i32, r[2].as_i64().unwrap_or(0) as i32];
            let (placement, free, pass) = (r[3].as_bool().unwrap_or(false), r[4].as_bool().unwrap_or(false), r[5].as_i64().unwrap_or(0));
            parts.insert(p, (placement, free, pass));
            if placement && free && pass > 0 {
                theirs.insert(p);
            }
        }
        let empty = BTreeSet::new();
        let mine = ours.get(ty).unwrap_or(&empty);
        let only_ours: Vec<&[i32; 3]> = mine.difference(&theirs).collect();
        let only_theirs: Vec<&[i32; 3]> = theirs.difference(mine).collect();
        total_cells += cells.len();
        disagree += only_ours.len() + only_theirs.len();
        let show = |v: &[&[i32; 3]]| {
            v.iter()
                .take(6)
                .map(|p| {
                    let (pl, fr, pa) = parts.get(*p).copied().unwrap_or((false, false, 0));
                    format!("({},{},{}) [game: placement {pl}, free {fr}, rules {pa}/{tries}]", p[0], p[1], p[2])
                })
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!(
            "  {ty}: listed at {} cells; can spawn: game {}, C1 {}; C1 only {} {}; game only {} {}",
            cells.len(),
            theirs.len(),
            mine.len(),
            only_ours.len(),
            show(&only_ours),
            only_theirs.len(),
            show(&only_theirs)
        );
        if ty.as_str() == "minecraft:zombie" || ty.as_str() == "minecraft:bat" {
            println!("    (difficulty {}, sky darkening {})", v["difficulty"], v["sky_darken"]);
        }
        for p in only_ours.iter().chain(only_theirs.iter()).take(4) {
            let b = BlockPos::new(p[0], p[1], p[2]);
            let column = game.op(json!({"op": "get", "from": [p[0], p[1] - 1, p[2]], "to": [p[0], p[1] + 1, p[2]]}))?;
            println!("    game's states below..above ({},{},{}): {}", p[0], p[1], p[2], column["palette"]);
            let k = kind_of(ty).ok_or_else(|| anyhow!("no kind {ty}"))?;
            let (rules, placement, free) = redsim::entity::mob::hostile::spawn_parts(&w, k, b);
            if !free {
                let d = k.ty.def();
                let hw = d.width as f64 / 2.0;
                let bb = redsim::entity::Aabb::new(b.x as f64 + 0.5 - hw, b.y as f64, b.z as f64 + 0.5 - hw, b.x as f64 + 0.5 + hw, b.y as f64 + d.height as f64, b.z as f64 + 0.5 + hw);
                for q in [b.offset(-1, 0, -1), b.offset(0, 0, -1), b.offset(1, 0, -1), b.offset(-1, 0, 0), b.offset(1, 0, 0), b.offset(-1, 0, 1), b.offset(0, 0, 1), b.offset(1, 0, 1), b, b.below()] {
                    let s = w.get(q);
                    let hit = redsim::entity::collide::boxes_at(&w, &s, q).iter().any(|x| x.intersects(bb));
                    if hit {
                        let g = game.op(json!({"op": "get", "from": [q.x, q.y, q.z], "to": [q.x, q.y, q.z]}))?;
                        println!("      redsim's {s} at ({},{},{}) meets the box; the game's {}", q.x, q.y, q.z, g["palette"]);
                    }
                }
            }
            println!(
                "    redsim at ({},{},{}): {} on {} under {}; sky {:?} block {:?}; world surface {}; C1 rules {rules}, placement {placement}, free {free}",
                p[0],
                p[1],
                p[2],
                w.get(b),
                w.get(b.below()),
                w.get(b.above()),
                w.sky_light(b),
                w.block_light(b),
                w.height(redsim::terrain::Heightmap::WorldSurface, b.x, b.z)
            );
        }
    }
    println!("{} (type, cell) pairs: {} disagree", total_cells, disagree);
    // a positive control: the open ground beside the farm (a cow on grass in daylight's light
    // rule, a zombie at night), where both sides must say yes
    let mut control = Vec::new();
    for dx in [-12, 12] {
        for dz in [-12, 12] {
            let (x, z) = (site.afk.x.floor() as i32 + dx, site.afk.z.floor() as i32 + dz);
            let y = w.height(redsim::terrain::Heightmap::MotionBlocking, x, z);
            control.push([x, y, z]);
        }
    }
    for ty in ["minecraft:cow", "minecraft:zombie"] {
        let v = game.op(json!({"op": "spawnrules", "types": [ty], "cells": control, "tries": tries}))?;
        let rows = v["types"][0]["cells"].as_array().cloned().unwrap_or_default();
        for (r, p) in rows.iter().zip(&control) {
            let b = BlockPos::new(p[0], p[1], p[2]);
            let k = kind_of(ty).ok_or_else(|| anyhow!("no kind {ty}"))?;
            println!(
                "  control {ty} at ({},{},{}) on {}: game placement {}, free {}, rules {}/{tries}; C1 {}",
                p[0],
                p[1],
                p[2],
                w.get(b.below()),
                r[3],
                r[4],
                r[5],
                redsim::entity::mob::hostile::can_spawn_at(&w, k, b)
            );
        }
    }
    // (a light read the spawn rules needed that failed: the comparison is refused, cc767b78, ironspec 12:54)
    let errors = w.take_light_errors();
    anyhow::ensure!(errors.is_empty(), "spawn-rules: {} light read(s) failed (pending since the world's last tick or take: some may predate this comparison): {}", errors.len(), errors.join("; "));
    Ok(())
}
