use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

/// The approved fixed discriminating fixtures; no parameter sweep or optimization.
pub fn generate(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, boat, mixed, leashed) in [
        ("pair-raft-ordinary", "bamboo_raft", false, true),
        ("pair-oak-ordinary", "oak_boat", false, true),
        ("pair-raft-no-lead", "bamboo_raft", false, false),
        ("pair-raft-mixed-no-lead", "bamboo_raft", true, false),
        ("pair-raft-mixed", "bamboo_raft", true, true),
        ("pair-oak-mixed", "oak_boat", true, true),
    ] {
        let mut steps = vec![
            json!({"rcon":"tag @a add mctas_probe"}),
            json!({"rcon":"give @a lead 2"}),
        ];
        for (suffix, x) in [("a", 0.5), ("b", 10.5)] {
            steps.push(json!({"rcon":format!("summon {boat} {x} 201 0.5 {{Tags:[\"mctas_probe\",\"boat_{suffix}\"]}}")}));
            steps.push(json!({"rcon":format!("summon llama {x} 201 0.5 {{Tags:[\"mctas_probe\",\"holder_{suffix}\"],Age:0,Tame:1b}}")}));
        }
        steps.push(json!({"step":12}));
        steps.push(json!({"rcon":"tp @a 10.5 201 -2.5"}));
        steps.push(json!({"actor":"select 0"}));
        if leashed {
            // First B -> llama in A. Setup is frozen so the relaxed apparatus does not converge
            // before the graph closes. Player interaction still uses genuine client packets.
            actors(
                &mut steps,
                &[
                    "look 10.5 201.2 0.5 3",
                    "hit",
                    "click use",
                    "walk 0 -2",
                    "sneak 1",
                    "look 0.5 202 0.5 3",
                    "hit",
                    "click use",
                    "sneak 0",
                ],
            );
            // A initially -> player, then either transfer to llama B or retain while boarding it.
            actors(
                &mut steps,
                &["look 0.5 201.2 0.5 3", "hit", "click use", "walk 10 -2"],
            );
            if !mixed {
                actors(
                    &mut steps,
                    &[
                        "sneak 1",
                        "look 10.5 202 0.5 3",
                        "hit",
                        "click use",
                        "sneak 0",
                    ],
                );
            }
        }
        if mixed {
            actors(
                &mut steps,
                &[
                    "select 1",
                    "look 10.5 202.2 0.5 3",
                    "hit",
                    "click use",
                    "probe_state",
                ],
            );
        } else {
            actors(&mut steps, &["walk 5 -16", "look 5.5 201 0.5 3"]);
        }
        steps.push(json!({"graph":{"mixed":mixed,"leashed":leashed}}));
        steps.push(json!({"screenshot":"assembled.png"}));
        steps.push(json!({"rcon":"data get entity @e[tag=boat_a,limit=1]"}));
        steps.push(json!({"rcon":"data get entity @e[tag=boat_b,limit=1]"}));
        steps.push(json!({"note":"LAUNCH: command-removal mechanics fixture, NOT a survival construction claim; no motion injection. Fixed initial center separation 10, equal height and yaw0. No optimization."}));
        steps.push(json!({"rcon":"fill -12 200 -12 22 200 12 air"}));
        steps.push(json!({"rcon":"tick unfreeze"}));
        steps.push(json!({"wait":1000}));
        steps.push(json!({"screenshot":"after-20t.png"}));
        steps.push(json!({"wait":3000}));
        steps.push(json!({"screenshot":"after-80t.png"}));
        steps.push(json!({"wait":57000}));
        steps.push(json!({"rcon":"tick freeze"}));
        let scenario = json!({"name":name,"setup":["fill -12 200 -17 22 200 12 stone","setworldspawn 5 201 -16","forceload add -64 -64 64 64","time set day"],"steps":steps});
        std::fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_string_pretty(&scenario)?,
        )?;
    }
    Ok(())
}
fn actors(steps: &mut Vec<Value>, commands: &[&str]) {
    steps.extend(commands.iter().map(|command| json!({"actor":command})));
}
