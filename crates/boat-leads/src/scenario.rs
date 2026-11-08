use anyhow::{Result, ensure};
use mctas::design::EntitySpec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vessel {
    #[serde(rename = "type")]
    pub kind: String,
    pub pos: [f64; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub motion: [f64; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkEvent {
    pub tick: u32,
    pub entity: usize,
    pub holder: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub vessels: Vec<Vessel>,
    #[serde(default)]
    pub links: Vec<[usize; 2]>,
    #[serde(default)]
    pub events: Vec<LinkEvent>,
    pub ticks: u32,
    pub floor: String,
    #[serde(default)]
    pub origin: [i32; 3],
}

impl Scenario {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.vessels.is_empty(), "no vessels");
        ensure!((1..=12000).contains(&self.ticks), "ticks outside 1..=12000");
        ensure!(self.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "name must be an ordinary file stem");
        ensure!(self.origin[0] % 16 == 0 && self.origin[2] % 16 == 0, "origin x/z must be chunk-aligned");
        ensure!(matches!(self.floor.as_str(), "stone" | "packed_ice" | "water"), "unsupported floor");
        for b in &self.vessels {
            ensure!(matches!(b.kind.as_str(), "minecraft:oak_boat" | "minecraft:oak_chest_boat"), "foundation probes only support wood/chest boats");
            ensure!(b.pos.iter().chain(b.motion.iter()).all(|v| v.is_finite()) && b.yaw.is_finite(), "non-finite input");
        }
        for [a, b] in &self.links {
            ensure!(*a < self.vessels.len() && *b < self.vessels.len() && a != b, "bad link indices");
        }
        for e in &self.events {
            ensure!(e.entity < self.vessels.len() && e.holder.is_none_or(|h| h < self.vessels.len() && h != e.entity), "bad event indices");
            ensure!((1..=self.ticks).contains(&e.tick), "event tick outside run");
        }
        Ok(())
    }

    pub fn specs(&self) -> Vec<EntitySpec> {
        self.vessels.iter().map(|b| EntitySpec {
            kind: b.kind.clone(),
            pos: std::array::from_fn(|k| b.pos[k] + self.origin[k] as f64),
            nbt: Some(format!("{{Rotation:[{}f,0f],Motion:[{}d,{}d,{}d]}}", b.yaw, b.motion[0], b.motion[1], b.motion[2])),
        }).collect()
    }
}

fn vessel(x: f64, y: f64, z: f64) -> Vessel {
    Vessel { kind: "minecraft:oak_boat".into(), pos: [x, y, z], yaw: 0.0, motion: [0.0; 3] }
}

fn pair(name: &str, distance: f64, links: Vec<[usize; 2]>) -> Scenario {
    Scenario { name: name.into(), vessels: vec![vessel(-distance / 2.0, 5.0, 0.0), vessel(distance / 2.0, 5.0, 0.0)],
        links, events: vec![], ticks: 100, floor: "stone".into(), origin: [0; 3] }
}

/// Fixed discriminating fixtures selected from source branches, NOT a parameter search.
pub fn calibration_cases() -> Vec<Scenario> {
    let mut cases = vec![pair("no-lead-control", 8.0, vec![]), pair("slack-pair", 2.0, vec![[0, 1], [1, 0]]),
        pair("taut-open", 8.0, vec![[0, 1]]), pair("taut-mutual", 8.0, vec![[0, 1], [1, 0]])];
    let mut reverse = cases[3].clone();
    reverse.name = "taut-reverse-order".into();
    reverse.vessels.reverse();
    cases.push(reverse);
    let mut angles = cases[3].clone();
    angles.name = "taut-oblique-yaws".into();
    angles.vessels[0].yaw = 37.0;
    angles.vessels[1].yaw = -73.0;
    cases.push(angles);
    let mut vertical = pair("taut-height-difference", 4.0, vec![[0, 1], [1, 0]]);
    vertical.vessels[0].pos[1] = 2.0;
    vertical.vessels[1].pos[1] = 8.0;
    cases.push(vertical);
    let mut ground = cases[3].clone();
    ground.name = "taut-on-ground".into();
    for b in &mut ground.vessels { b.pos[1] = 1.0; }
    cases.push(ground.clone());
    ground.name = "taut-on-ice".into();
    ground.floor = "packed_ice".into();
    cases.push(ground);
    let mut chest = cases[3].clone();
    chest.name = "taut-chest-boats".into();
    for b in &mut chest.vessels { b.kind = "minecraft:oak_chest_boat".into(); }
    cases.push(chest);
    let mut water = cases[3].clone();
    water.name = "taut-water".into();
    water.floor = "water".into();
    for b in &mut water.vessels { b.pos[1] = 0.5; }
    cases.push(water);
    let mut contact = pair("nearby-push-control", 1.5, vec![]);
    for b in &mut contact.vessels { b.pos[1] = 1.0; }
    cases.push(contact);
    let mut head_on = pair("head-on-collision", 3.0, vec![]);
    head_on.vessels[0].motion[0] = 1.0;
    head_on.vessels[1].motion[0] = -1.0;
    for b in &mut head_on.vessels { b.pos[1] = 1.0; }
    cases.push(head_on);
    let mut stacked = pair("stacked-collision", 0.0, vec![]);
    stacked.vessels[0].pos[1] = 1.0;
    stacked.vessels[1].pos[1] = 1.5625;
    cases.push(stacked);
    let mut detached = cases[3].clone();
    detached.name = "detach-and-retie".into();
    detached.events = vec![LinkEvent { tick: 10, entity: 0, holder: None }, LinkEvent { tick: 12, entity: 0, holder: Some(1) }];
    cases.push(detached);
    let mut shifted = cases[5].clone();
    shifted.name = "taut-far-position".into();
    shifted.origin = [160, 64, -160];
    cases.push(shifted);
    cases
}
