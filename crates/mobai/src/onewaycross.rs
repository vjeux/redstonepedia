//! (tickexact 2026-11-12, ironspec 16:29 (b): A6's one-way route) `mctas mobai oneway-cross`: on the
//! official 26.3 game (the bridge's sandbox: the design built as `mobfarm compare` builds it, lifted
//! by LIFT, its entities not summoned; `--parts` and `--set` laid over it), a villager standing at
//! `--at` (the second landing chest's top), its walk target forced at each of `--targets` (north of
//! the trapdoor's plate) in turn, re-set every 10 gt (the bridge's `memory` op: a WalkTarget at the
//! cell), for `--ticks` gt per try, `--tries` tries from `--at` with its x moved by up to 0.25 (the
//! teleport back between tries). Printed per try: the villager's least z, where it ended, and
//! whether its centre crossed north of `--north-z` (the plate's north face); the totals.

use anyhow::{Result, anyhow};
use mobfarm::exact::{GameFarm, LIFT, Opts, load_place};
use serde_json::json;

#[allow(clippy::too_many_arguments)]
pub fn oneway_cross(version: &str, design: &std::path::Path, parts: &[(std::path::PathBuf, [i32; 3])], set: &[([i32; 3], String)], at: [f64; 3], targets: &[[i32; 3]], tries: u32, ticks: u32, north_z: f64, speed: f64, day_time: i64) -> Result<()> {
    let opts = Opts { seed: 1, ticks: 1, weather: false, probe: false, batch: 100, full: true, bare: true, draws_at: None, terrain: None, day0: None, build: None, no_spawning: true, only: Vec::new() };
    let (c, spec, place) = load_place(design, &opts)?;
    let o = mobfarm::exact::opts_of(&opts);
    let mut game = GameFarm::start_with(version, &c, &spec, &o, false, false, place, false)?;
    let mut laid: Vec<([i32; 3], String)> = Vec::new();
    for (f, off) in parts {
        for (p, s) in mctas::design::Design::load(f)?.blocks.iter() {
            laid.push(([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s.to_string()));
        }
    }
    laid.extend(set.iter().cloned());
    for chunk in laid.chunks(512) {
        let blocks: Vec<serde_json::Value> = chunk.iter().map(|(p, s)| json!([p[0], p[1] + LIFT, p[2], s])).collect();
        game.op(json!({"op": "set", "blocks": blocks}))?;
    }
    game.op(json!({"op": "exec", "command": format!("time set {day_time}")}))?;
    println!(
        "== oneway-cross on the official {version} game: {}{}{}; a villager at ({},{},{}), its walk target forced at {} target(s) in turn (speed {speed}, re-set every 10 gt), {tries} tries of {ticks} gt; a crossing: its centre north of z {north_z}",
        design.display(),
        parts.iter().map(|(f, o)| format!("; {} laid over it moved by ({},{},{})", f.display(), o[0], o[1], o[2])).collect::<String>(),
        if set.is_empty() { String::new() } else { format!("; laid over it: {}", set.iter().map(|(p, s)| format!("({},{},{}) {s}", p[0], p[1], p[2])).collect::<Vec<_>>().join(", ")) },
        at[0],
        at[1],
        at[2],
        targets.len()
    );
    let summoned = game.op(json!({"op": "summon", "mobs": true, "entities": [{"type": "minecraft:villager", "pos": [at[0], at[1] + LIFT as f64, at[2]], "nbt": "{PersistenceRequired:1b,Tags:[\"owx\"]}"}]}))?;
    let id = summoned["net_ids"].as_array().and_then(|a| a.first()).and_then(|x| x.as_i64()).ok_or_else(|| anyhow!("the summon gave no entity number: {summoned}"))?;
    let read = |game: &mut GameFarm| -> Result<Option<[f64; 3]>> {
        let st = game.op(json!({"op": "mobstate"}))?;
        Ok(st["mobs"].as_array().into_iter().flatten().find(|m| m["id"].as_i64() == Some(id)).map(|m| {
            let f = |k: &str| m[k].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(f64::NAN);
            [f("x"), f("y") - LIFT as f64, f("z")]
        }))
    };
    let (mut crossings, mut least) = (0u32, f64::MAX);
    for k in 0..tries {
        let tgt = targets[k as usize % targets.len()];
        let dx = [0.0, -0.2, 0.2, -0.1, 0.1][k as usize % 5];
        game.op(json!({"op": "teleport", "entity": id, "pos": [at[0] + dx, at[1] + LIFT as f64, at[2]]}))?;
        let mut min_z = f64::MAX;
        let mut end = [f64::NAN; 3];
        for t in 0..ticks {
            if t % 10 == 0 {
                game.op(json!({"op": "memory", "entity": id, "memories": {"walk_target": {"value": [tgt[0], tgt[1] + LIFT, tgt[2]], "speed": speed, "close": 0}}}))?;
            }
            game.op(json!({"op": "step", "ticks": 1}))?;
            if let Some(p) = read(&mut game)? {
                min_z = min_z.min(p[2]);
                end = p;
            }
        }
        let crossed = min_z < north_z;
        if crossed {
            crossings += 1;
        }
        least = least.min(min_z);
        if k < 12 || crossed {
            println!(
                "  try {k}: from x {:.2}, walk target ({},{},{}): least z {min_z:.4}, ended at ({:.3},{:.3},{:.3}){}",
                at[0] + dx,
                tgt[0],
                tgt[1],
                tgt[2],
                end[0],
                end[1],
                end[2],
                if crossed { " CROSSED" } else { "" }
            );
        }
    }
    println!("  {tries} tries: {crossings} crossings north of z {north_z}; the least z of any try {least:.4}");
    Ok(())
}
