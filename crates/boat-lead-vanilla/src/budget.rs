use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::Write, path::Path};

type V = [f64; 3];
fn v(x: &Value) -> Result<V> { Ok([x[0].as_f64().context("x")?, x[1].as_f64().context("y")?, x[2].as_f64().context("z")?]) }
fn sub(a: V, b: V) -> V { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn len(a: V) -> f64 { (a[0]*a[0]+a[1]*a[1]+a[2]*a[2]).sqrt() }
fn center(e: &Value) -> Result<V> {
    let mut c=v(&e["position"])?;
    c[1] += e["height"].as_f64().context("height")? * 0.5;
    Ok(c)
}

/// A diagnostic of observed pre/post phases, never an entity-motion simulator.
pub fn budget(trace: &Path, out: &Path) -> Result<()> {
    let rows=crate::trace::read(trace)?;
    let mut pre=BTreeMap::new();
    let mut file=std::fs::File::create(out)?;
    let mut count=0;
    let mut max_error=0.0_f64;
    for row in &rows {
        let key=(row["tick"].as_i64().context("tick")?, row["entity"]["id"].as_i64().context("id")?);
        if row["phase"]=="leash_pre" { pre.insert(key,row); }
        if row["phase"]!="leash_post" { continue; }
        let Some(before)=pre.remove(&key) else { continue; };
        if before.get("holder").is_none() { continue; }
        let record=force(before,row)?;
        max_error=max_error.max(record["max_component_error"].as_f64().unwrap());
        writeln!(file,"{record}")?;
        count+=1;
    }
    println!("{}",json!({"force_samples":count,"max_component_error":max_error,"output":out}));
    Ok(())
}

fn force(before: &Value, after: &Value) -> Result<Value> {
    let e=&before["entity"];
    let h=&before["holder"];
    let dcenter=len(sub(center(h)?,center(e)?));
    let a=v(&e["child_anchor"])?;
    let b=v(&h["holder_anchor"])?;
    let diff=sub(b,a);
    let r=len(diff);
    let delta=v(&e["delta"])?;
    let known=v(&e["known"])?;
    let holder_known=v(&h["known"])?;
    let active=dcenter<=12.0 && dcenter>6.0-h["width"].as_f64().unwrap()-e["width"].as_f64().unwrap() && r>=6.0;
    let displacement=if active { diff.map(|x| x / r * (r-6.0)) } else { [0.0;3] };
    let dot=known[0]*displacement[0]+known[1]*displacement[1]+known[2]*displacement[2];
    let k=if dot>=0.0 { 0.3_f32 as f64 } else { 1.0 };
    let elasticity=[0.8,0.2,0.8];
    let spring=std::array::from_fn::<_,3,_>(|i| displacement[i]*k*elasticity[i]);
    let relative=if active { sub(holder_known,known).map(|x|x*0.11) } else { [0.0;3] };
    let expected=std::array::from_fn::<_,3,_>(|i| spring[i]+relative[i]);
    let observed=sub(v(&after["entity"]["delta"])?,delta);
    let error=sub(expected,observed).map(f64::abs).into_iter().fold(0.0_f64,f64::max);
    Ok(json!({"tick":before["tick"],"entity":e["id"],"holder":h["id"],"Dcenter":dcenter,
        "attachment_distance":r,"height_offset":diff[1],"active":active,"dot":dot,"damping":k,
        "pre_motion":delta,"child_known":known,"holder_known":holder_known,"spring":spring,"relative":relative,
        "expected_leash_delta":expected,"observed_leash_delta":observed,"max_component_error":error,
        "post_motion":after["entity"]["delta"],"authority":e["authoritative"],"controller":e["controller"],
        "contact":{"ground":e["on_ground"],"water":e["in_water"],"lava":e["in_lava"]}}))
}
