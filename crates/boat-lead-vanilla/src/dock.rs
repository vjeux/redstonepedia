use anyhow::Result;
use serde_json::{Value,json};
use std::path::Path;

pub fn generate(path:&Path)->Result<()> {
    let mut steps:Vec<Value>=vec![json!({"rcon":"tag @a add mctas_probe"}),json!({"rcon":"give @a lead 2"})];
    for (suffix,x) in [("a",0),("b",10)] {
        for (dx,dz) in [(-1,0),(1,0),(0,-1),(0,1)] {
            steps.push(json!({"rcon":format!("setblock {} 201 {} oak_fence",x+dx,dz)}));
        }
        steps.push(json!({"rcon":format!("summon bamboo_raft {} 201 0.5 {{Tags:[\"mctas_probe\",\"boat_{suffix}\"]}}",x as f64+0.5)}));
        steps.push(json!({"rcon":format!("summon llama {} 201 0.5 {{Tags:[\"mctas_probe\",\"holder_{suffix}\"],Age:0,Tame:1b}}",x as f64+0.5)}));
    }
    steps.push(json!({"step":12}));
    steps.push(json!({"rcon":"tick unfreeze"}));
    steps.push(json!({"note":"NORMAL TICKING DOCK: both roots tick throughout attachment and mounting. Four ordinary fence posts constrain horizontal motion. Actor staging teleports are labeled fixture interventions, not a survival assembly claim. Actual launch positions/yaw/motion are measured, not reset."}));
    steps.push(json!({"rcon":"tp @a 10.5 202.5 -0.5"}));
    actors(&mut steps,&["select 0","look 11.1 201.5 0.5 3","hit","click use"]);
    steps.push(json!({"rcon":"tp @a 0.5 202.5 -0.5"}));
    actors(&mut steps,&["sneak 1","look 0.5 203 0.5 3","hit","click use","sneak 0","look 1.1 201.5 0.5 3","hit","click use"]);
    steps.push(json!({"rcon":"tp @a 10.5 202.5 -0.5"}));
    actors(&mut steps,&["select 1","look 10.5 203 0.5 3","hit","click use","probe_state"]);
    steps.push(json!({"screenshot":"assembled.png"}));
    steps.push(json!({"rcon":"data get entity @e[tag=boat_a,limit=1]"}));
    steps.push(json!({"rcon":"data get entity @e[tag=boat_b,limit=1]"}));
    steps.push(json!({"note":"LAUNCH: remove the complete command-built dock, including posts; normal 20TPS already running, no motion injection or tick exemption."}));
    steps.push(json!({"rcon":"fill -12 200 -12 22 203 12 air"}));
    steps.push(json!({"wait":1000})); steps.push(json!({"screenshot":"after-20t.png"}));
    steps.push(json!({"wait":3000})); steps.push(json!({"screenshot":"after-80t.png"}));
    steps.push(json!({"wait":57000})); steps.push(json!({"rcon":"tick freeze"}));
    let scenario=json!({"name":"normal-ticking physical dock: mixed raft pair", "setup":["fill -12 200 -17 22 200 12 stone","setworldspawn 5 201 -16","forceload add -64 -64 64 64","time set day"],"steps":steps});
    std::fs::write(path,serde_json::to_string_pretty(&scenario)?)?;
    Ok(())
}
fn actors(steps:&mut Vec<Value>,commands:&[&str]) {steps.extend(commands.iter().map(|command|json!({"actor":command})));}
