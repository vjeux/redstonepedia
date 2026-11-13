//! (tickexact 2026-11-12, ironspec 16:12 / 16:17: the rail-node rule, the game's answer) `mctas
//! mobai path-probe-game`: on the official 26.3 game (the bridge's sandbox: the design built as
//! `mobfarm compare` builds it, lifted by `mobfarm::exact::LIFT`, its entities not summoned; `--set`
//! cells laid over it), for each `--at` position a cart held there (NoGravity, no motion) with a
//! villager riding (NoAI), not ticked; the villager's navigation searches toward each `--targets`
//! cell as MoveToTargetSink's `createPath(target, 0)` would (the bridge's `path_nodes`: the same
//! region, length and node budget). Printed per position: the rider's position and block, the cell
//! below it (the composition's state), and per target the path's nodes (the first is the start
//! node), complete or partial; `--nodes` also every node the evaluator made.

use anyhow::{Result, anyhow};
use mobfarm::exact::{GameFarm, LIFT, Opts, load_place};
use serde_json::json;

#[allow(clippy::too_many_arguments)]
pub fn path_probe_game(version: &str, design: &std::path::Path, parts: &[(std::path::PathBuf, [i32; 3])], at: &[[f64; 3]], targets: &[[i32; 3]], set: &[([i32; 3], String)], reach: i64, nodes: bool, tick: bool) -> Result<()> {
    let opts = Opts { seed: 1, ticks: 1, weather: false, probe: false, batch: 100, full: true, bare: true, draws_at: None, terrain: None, day0: None, build: None, no_spawning: true, only: Vec::new() };
    let (c, spec, place) = load_place(design, &opts)?;
    let o = mobfarm::exact::opts_of(&opts);
    let mut game = GameFarm::start_with(version, &c, &spec, &o, false, false, place, false)?;
    // the composition, for the cells below the riders
    let mut comp: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
    for (p, s) in mctas::design::Design::load(design)?.blocks.iter() {
        comp.insert([p[0], p[1], p[2]], s.to_string());
    }
    // the parts laid over the design, each moved by its offset (its non-air cells), then `--set`
    let mut laid: Vec<([i32; 3], String)> = Vec::new();
    for (f, off) in parts {
        for (p, s) in mctas::design::Design::load(f)?.blocks.iter() {
            laid.push(([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s.to_string()));
        }
    }
    laid.extend(set.iter().cloned());
    if !laid.is_empty() {
        for chunk in laid.chunks(512) {
            let blocks: Vec<serde_json::Value> = chunk.iter().map(|(p, s)| json!([p[0], p[1] + LIFT, p[2], s])).collect();
            game.op(json!({"op": "set", "blocks": blocks}))?;
        }
        for (p, s) in &laid {
            comp.insert(*p, s.clone());
        }
    }
    println!(
        "== path-probe-game on the official {version} game: {} (built as the exact compare builds it, its entities not summoned){}{}; a cart held at each of {} position(s) with a villager riding, not ticked; createPath(target, {reach}) toward {} target(s)",
        design.display(),
        parts.iter().map(|(f, o)| format!("; {} laid over it moved by ({},{},{})", f.display(), o[0], o[1], o[2])).collect::<String>(),
        if set.is_empty() { String::new() } else { format!("; laid over it: {}", set.iter().map(|(p, s)| format!("({},{},{}) {s}", p[0], p[1], p[2])).collect::<Vec<_>>().join(", ")) },
        at.len(),
        targets.len()
    );
    for (i, p) in at.iter().enumerate() {
        let tag = format!("ppg{i}");
        let summoned = game.op(json!({"op": "summon", "mobs": true, "entities": [{"type": "minecraft:minecart", "pos": [p[0], p[1] + LIFT as f64, p[2]], "nbt": format!("{{NoGravity:1b,Motion:[0.0d,0.0d,0.0d],Tags:[\"{tag}\"],Passengers:[{{id:\"minecraft:villager\",NoAI:1b,PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]}}")}]}))?;
        let cart_id = summoned["net_ids"].as_array().and_then(|a| a.first()).and_then(|x| x.as_i64()).ok_or_else(|| anyhow!("the summon gave no entity number: {summoned}"))?;
        if tick {
            // one tick: the cart's own (held: no gravity, no motion) and positionRider's placing of
            // the rider at its seat
            game.op(json!({"op": "step", "ticks": 1}))?;
        }
        let state = game.op(json!({"op": "mobstate"}))?;
        // the summoned cart's rider: the villager numbered right after the cart (the vehicle is
        // added first, then its passengers)
        let rider_id = cart_id + 1;
        let v = state["mobs"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|m| m["type"].as_str() == Some("minecraft:villager") && m["id"].as_i64() == Some(rider_id))
            .ok_or_else(|| anyhow!("no villager #{rider_id} (the rider of cart #{cart_id}) near {p:?}"))?
            .clone();
        let id = v["id"].as_i64().unwrap_or(0);
        let f = |k: &str| v[k].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(f64::NAN);
        let (rx, ry, rz) = (f("x"), f("y") - LIFT as f64, f("z"));
        let rb = [rx.floor() as i32, ry.floor() as i32, rz.floor() as i32];
        let below = [rb[0], rb[1] - 1, rb[2]];
        let st = |c: [i32; 3]| comp.get(&c).cloned().unwrap_or_else(|| "minecraft:air".into());
        println!(
            "  cart at ({},{},{}): the rider (#{id}) at ({rx:.4},{ry:.4},{rz:.4}), its block ({},{},{}) {}, the cell below ({},{},{}) {}",
            p[0],
            p[1],
            p[2],
            rb[0],
            rb[1],
            rb[2],
            st(rb),
            below[0],
            below[1],
            below[2],
            st(below)
        );
        for t in targets {
            let r = game.op(json!({"op": "path_nodes", "entity": id, "target": [t[0], t[1] + LIFT, t[2]], "reach": reach}))?;
            let path = r["path"].as_str().map(|s| {
                // the nodes back in the design's frame
                let mut out = Vec::new();
                for tok in s.split_whitespace() {
                    let n: Vec<i32> = tok.split(',').filter_map(|x| x.parse().ok()).collect();
                    if n.len() == 3 {
                        out.push(format!("{},{},{}", n[0], n[1] - LIFT, n[2]));
                    } else {
                        out.push(tok.to_string());
                    }
                }
                out
            });
            match path {
                Some(nodes_) => {
                    let n = nodes_.len().saturating_sub(1);
                    println!(
                        "    to ({},{},{}): {} nodes, {}; start {}; nodes {}",
                        t[0],
                        t[1],
                        t[2],
                        n,
                        nodes_.last().map(|s| if s == "reached" { "COMPLETE" } else { "partial" }).unwrap_or("?"),
                        nodes_.first().cloned().unwrap_or_default(),
                        nodes_[..n].join(" ")
                    );
                }
                None => println!("    to ({},{},{}): no path (null)", t[0], t[1], t[2]),
            }
            if nodes {
                for node in r["nodes"].as_array().into_iter().flatten() {
                    let s = node.as_str().unwrap_or("");
                    println!("      node {s}");
                }
            }
        }
        game.op(json!({"op": "exec", "command": format!("kill @e[tag={tag}v]")}))?;
        game.op(json!({"op": "exec", "command": format!("kill @e[tag={tag}]")}))?;
    }
    Ok(())
}
