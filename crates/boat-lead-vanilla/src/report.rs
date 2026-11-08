use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub fn report(dir: &Path) -> Result<()> {
    let rows = crate::trace::read(&dir.join("trace.jsonl"))?;
    let events = crate::trace::read(&dir.join("interventions.jsonl"))?;
    let launch = events
        .iter()
        .rfind(|r| {
            r["kind"] == "rcon"
                && r["command"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("fill ") && s.ends_with(" air"))
        })
        .and_then(|r| r["tick"].as_i64())
        .context("dock release")?;
    let finish = events
        .last()
        .and_then(|r| r["tick"].as_i64())
        .context("finish")?;
    crate::budget::budget(&dir.join("trace.jsonl"), &dir.join("budget.jsonl"))?;
    let budget = read_optional(&dir.join("budget.jsonl"))?;
    let mut boats: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
    for row in &rows {
        if row["phase"] == "boat_post" && row["tick"].as_i64().unwrap() > launch {
            boats
                .entry(row["entity"]["id"].as_i64().unwrap())
                .or_default()
                .push(row);
        }
    }
    let mut reports = vec![];
    for (id, rs) in boats {
        let active: Vec<_> = budget
            .iter()
            .filter(|r| {
                r["entity"] == id && r["active"] == true && r["tick"].as_i64().unwrap() > launch
            })
            .collect();
        let mut windows = vec![];
        for (lo, hi) in [(1, 400), (401, 800), (801, 1200)] {
            let ys: Vec<f64> = rs
                .iter()
                .filter(|r| {
                    let relative = r["tick"].as_i64().unwrap() - launch;
                    relative >= lo && relative <= hi
                })
                .map(|r| r["entity"]["position"][1].as_f64().unwrap())
                .collect();
            windows.push(json!({"ticks":[lo,hi],"samples":ys.len(),"first_y":ys.first(),"last_y":ys.last(),"mean_y":if ys.is_empty(){None}else{Some(ys.iter().sum::<f64>()/ys.len() as f64)}}));
        }
        let contact = rs
            .iter()
            .filter(|r| {
                r["entity"]["on_ground"] == true
                    || r["entity"]["in_water"] == true
                    || r["entity"]["in_lava"] == true
            })
            .count();
        let gaps = rs
            .windows(2)
            .filter(|w| w[1]["tick"].as_i64().unwrap() != w[0]["tick"].as_i64().unwrap() + 1)
            .count();
        reports.push(json!({"id":id,"tags":rs[0]["entity"]["tags"],"flight_ticks_observed":rs.len(),"tick_gaps":gaps,
            "first_tick":rs.first().unwrap()["tick"],"last_tick":rs.last().unwrap()["tick"],
            "first_missing_lead_tick":rs.iter().find(|r|r["entity"]["holder"].is_null()).map(|r|r["tick"].clone()),
            "first_removed_tick":rs.iter().find(|r|r["entity"]["removed"]==true).map(|r|r["tick"].clone()),
            "first_dead_tick":rs.iter().find(|r|r["entity"]["alive"]!=true).map(|r|r["tick"].clone()),
            "first_y":rs.first().unwrap()["entity"]["position"][1],"last_y":rs.last().unwrap()["entity"]["position"][1],
            "max_y":rs.iter().map(|r|r["entity"]["position"][1].as_f64().unwrap()).fold(f64::NEG_INFINITY,f64::max),
            "max_vertical_delta":rs.iter().map(|r|r["entity"]["delta"][1].as_f64().unwrap()).fold(f64::NEG_INFINITY,f64::max),
            "active_spring_samples":active.len(),"undamped_samples":active.iter().filter(|r|r["damping"]==1.0).count(),
            "last_active_tick":active.last().map(|r|r["tick"].clone()),
            "on_ground_or_fluid_samples":contact,"supporting_block_samples":if rs[0]["entity"].get("support_block").is_none(){None}else{Some(rs.iter().filter(|r|r["entity"]["support_block"].as_str().is_some_and(|s|s!="Optional.empty")).count())},
            "unloaded_samples":rs.iter().filter(|r|r["entity"]["touching_unloaded_chunk"]==true).count(),
            "missing_lead_samples":rs.iter().filter(|r|r["entity"]["holder"].is_null()).count(),
            "nonserver_authority_samples":rs.iter().filter(|r|r["entity"]["authoritative"]!=true).count(),
            "altitude_windows":windows}));
    }
    let report = json!({"evidence":"fixed command-arranged mechanics fixture; no survival or universal-impossibility claim",
        "launch_tick":launch,"finish_tick":finish,"elapsed_ticks":finish-launch,"boats":reports});
    let text = serde_json::to_string_pretty(&report)?;
    std::fs::write(dir.join("report.json"), &text)?;
    println!("{text}");
    Ok(())
}
fn read_optional(path: &Path) -> Result<Vec<Value>> {
    let text = std::fs::read_to_string(path)?;
    text.lines()
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}
