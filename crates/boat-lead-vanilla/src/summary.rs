use anyhow::{Context, Result};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet},path::Path};

pub fn summarize(path:&Path)->Result<()> {
    let rows=crate::trace::read(path)?;
    let mut by:BTreeMap<i64,Vec<&Value>>=BTreeMap::new();
    for row in &rows { by.entry(row["entity"]["id"].as_i64().context("id")?).or_default().push(row); }
    for (id,rs) in by {
        let stable:Vec<_>=rs.iter().filter(|r|r["entity"]["alive"]==true && (r["phase"]=="boat_post" || r["phase"]=="ride_post")).collect();
        let ys:Vec<_>=stable.iter().filter_map(|r|r["entity"]["position"][1].as_f64()).collect();
        let known:BTreeSet<_>=stable.iter().filter_map(|r|r["entity"]["known"][1].as_f64()).map(f64::to_bits).collect();
        let authority:BTreeSet<_>=stable.iter().map(|r|format!("server={} controller={} root={} root_controller={} vehicle={} passengers={}",
            r["entity"]["authoritative"],r["entity"]["controller"],r["entity"]["root"],r["entity"]["root_controller"],r["entity"]["vehicle"],r["entity"]["passengers"])).collect();
        let seats:Vec<_>=rs.iter().filter(|r|r["phase"]=="leash_pre").flat_map(|r| {
            r["passengers"].as_array().into_iter().flatten().map(move |p| json!({"passenger":p["id"],"offset":p["position"][1].as_f64().unwrap()-r["entity"]["position"][1].as_f64().unwrap(),"known":p["known"],"delta":p["delta"]}))
        }).collect();
        let unique_seats:BTreeSet<_>=seats.iter().map(Value::to_string).collect();
        println!("{}",json!({"id":id,"type":rs[0]["entity"]["type"],"tags":rs[0]["entity"]["tags"],"post_samples":stable.len(),
            "first_y":ys.first(),"last_y":ys.last(),"min_y":ys.iter().copied().reduce(f64::min),"max_y":ys.iter().copied().reduce(f64::max),
            "post_known_y":known.iter().take(12).map(|bits|json!({"y":f64::from_bits(*bits),"bits":format!("{bits:016x}")})).collect::<Vec<_>>(),
            "authority_states":authority,"passenger_at_leash_phase":unique_seats.iter().take(10).collect::<Vec<_>>() }));
    }
    Ok(())
}
