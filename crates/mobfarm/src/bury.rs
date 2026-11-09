//! A void farm dug into rock (vjeux 18:18, the + layouts' underground version: "channels dug
//! through rock, nothing on the surface"): the design's containment blocks give way to natural
//! rock, every cell the farm needs open is dug (`terrain: rock`), and the natural ground lies
//! above its top, so the terrain judge, its access stair and the build's travel price it.

use anyhow::{Result, anyhow};
use redsim::entity::EntityKind;
use redsim::entity::mob::exit;
use redsim::entity::mob::farm::{self, FarmSpec, WalkMode};
use redsim::entity::mob::types::MobType;
use redsim::pos::BlockPos;
use std::collections::BTreeSet;

/// The cells a void farm needs open: every golem box in the exit flood's drifts (any input) and in
/// `seeds` runs of `ticks`, every item, villager, boat and zombie box in those runs, every fluid
/// cell at their end, the cells on each villager's sight line to the zombie (its panic needs the
/// sight), and the air next to its redstone (a wire's step up, a component's update).
pub fn envelope(spec: &FarmSpec, design: &mctas::design::Design, seeds: u32, ticks: u32) -> Result<(BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>)> {
    *exit::ENVELOPE.lock().map_err(|_| anyhow!("the envelope lock"))? = Some(BTreeSet::new());
    let flood = exit::golem_exit(spec, 0x51ab, 8);
    let mut cells = exit::ENVELOPE.lock().map_err(|_| anyhow!("the envelope lock"))?.take().unwrap_or_default();
    let mut fluids: BTreeSet<[i32; 3]> = BTreeSet::new();
    flood.map_err(|e| anyhow!(e))?;
    let (lo, hi) = bounds(design);
    for seed in 0..seeds {
        let o = farm::judge_opts(0x7e57 + seed as i64, ticks, WalkMode::Random);
        let mut r = farm::Run::new(spec, &o).map_err(|e| anyhow!(e))?;
        for t in 0..ticks {
            r.tick();
            for e in r.world.entities.list.iter().filter(|e| !e.removed) {
                add_box(&mut cells, e.bb);
            }
            // the sight lines, every second of the run (a villager lying in its bed looks from
            // lower down: at night its panic needs that line open too)
            if t % 20 == 0 {
                let eyes = |ty: MobType| -> Vec<redsim::entity::Vec3> {
                    r.world.entities.list.iter().filter(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == ty)).map(|e| redsim::entity::Vec3::new(e.pos.x, e.pos.y + e.kind.eye_height() as f64, e.pos.z)).collect()
                };
                for v in eyes(MobType::Villager) {
                    // (only a zombie the villager can panic at: within 8 blocks,
                    // `VillagerHostilesSensor`; a line to another module's zombie opens nothing
                    // the farm needs)
                    for z in eyes(MobType::Zombie).into_iter().filter(|z| z.distance_to_sqr(v) <= 10.0 * 10.0) {
                        for k in 0..=200 {
                            let f = k as f64 / 200.0;
                            let p = redsim::entity::Vec3::new(v.x + (z.x - v.x) * f, v.y + (z.y - v.y) * f, v.z + (z.z - v.z) * f);
                            cells.insert([p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]);
                        }
                    }
                }
            }
        }
        for x in lo[0] - 2..=hi[0] + 2 {
            for y in lo[1] - 2..=hi[1] + 2 {
                for z in lo[2] - 2..=hi[2] + 2 {
                    if matches!(r.world.get(BlockPos::new(x, y, z)), redsim::block::Block::Water { .. } | redsim::block::Block::Lava { .. }) {
                        cells.insert([x, y, z]);
                        fluids.insert([x, y, z]);
                    }
                }
            }
        }
    }
    // (not beside a bed: an open cell there with the rock as its floor is where a woken villager
    // stands up, outside its cell; in the void it had no floor)
    let beds: Vec<[i32; 3]> = design.blocks.iter().filter(|(_, s)| name(s).ends_with("_bed")).map(|(p, _)| *p).collect();
    let by_bed = |q: &[i32; 3]| beds.iter().any(|b| (b[0] - q[0]).abs() <= 1 && (b[1] - q[1]).abs() <= 1 && (b[2] - q[2]).abs() <= 1);
    for (p, s) in &design.blocks {
        if redstone(s) {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let q = [p[0] + dx, p[1] + dy, p[2] + dz];
                        if !design.blocks.contains_key(&q) && !by_bed(&q) {
                            cells.insert(q);
                        }
                    }
                }
            }
        }
    }
    Ok((cells, fluids))
}

fn add_box(cells: &mut BTreeSet<[i32; 3]>, bb: redsim::entity::Aabb) {
    let e = 1.0e-7;
    for x in (bb.min_x + e).floor() as i32..=(bb.max_x - e).floor() as i32 {
        for y in (bb.min_y + e).floor() as i32..=(bb.max_y - e).floor() as i32 {
            for z in (bb.min_z + e).floor() as i32..=(bb.max_z - e).floor() as i32 {
                cells.insert([x, y, z]);
            }
        }
    }
}

fn bounds(d: &mctas::design::Design) -> ([i32; 3], [i32; 3]) {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for p in d.blocks.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

fn name(s: &str) -> &str {
    s.trim_start_matches("minecraft:").split('[').next().unwrap_or("")
}

/// A block whose job is a redstone signal or its update.
fn redstone(s: &str) -> bool {
    let n = name(s);
    n.contains("redstone") || n.contains("repeater") || n.contains("comparator") || n.contains("observer") || n.contains("piston") || n.ends_with("_button") || n == "lever" || n.contains("copper_bulb") || n.ends_with("_pressure_plate") || n == "lightning_rod" || n == "target" || n == "dropper" || n == "dispenser" || n == "crafter" || n.ends_with("trapdoor")
}

/// A block that only holds things in (rock does the same): stone, cobblestone and their slabs,
/// glass, fences.
fn containment(s: &str) -> bool {
    matches!(name(s), "cobblestone" | "stone" | "cobbled_deepslate" | "cobblestone_slab" | "stone_slab" | "glass" | "oak_fence")
}

/// The buried design: containment blocks outside the envelope and away from redstone removed
/// (natural rock in their place), the envelope's open cells dug (`air`), the natural ground layer
/// `cover` layers above the design's top (`ground:`), only its own cells dug (`terrain: rock`),
/// the AFK spot on the surface above its middle. Returns (design, blocks removed, cells dug open).
pub fn bury(design: &mctas::design::Design, cells: &BTreeSet<[i32; 3]>, fluids: &BTreeSet<[i32; 3]>, cover: i32) -> (mctas::design::Design, usize, usize) {
    let mut d = design.clone();
    let near_redstone = |p: &[i32; 3]| (-1..=1).any(|dx| (-1..=1).any(|dy| (-1..=1).any(|dz| design.blocks.get(&[p[0] + dx, p[1] + dy, p[2] + dz]).is_some_and(|s| redstone(s)))));
    let drop: Vec<[i32; 3]> = design.blocks.iter().filter(|(p, s)| containment(s) && !cells.contains(*p) && !near_redstone(p)).map(|(p, _)| *p).collect();
    for p in &drop {
        d.blocks.remove(p);
    }
    let mut opened = 0;
    for c in cells {
        if !d.blocks.contains_key(c) {
            d.blocks.insert(*c, "minecraft:air".to_string());
            opened += 1;
        }
    }
    // an open cell beside a bed at its level that the rock would floor (none in the void) is where
    // a woken villager stands up, outside its cell: the cell under it is dug too, so it has no
    // floor (`DismountHelper`: no floor in the cell or the one below), as in the void
    let beds: Vec<[i32; 3]> = design.blocks.iter().filter(|(_, s)| name(s).ends_with("_bed")).map(|(p, _)| *p).collect();
    let deepen: Vec<[i32; 3]> = d
        .blocks
        .iter()
        .filter(|(c, s)| name(s) == "air" && beds.iter().any(|b| b[1] == c[1] && (b[0] - c[0]).abs() <= 1 && (b[2] - c[2]).abs() <= 1))
        .map(|(c, _)| [c[0], c[1] - 1, c[2]])
        .filter(|q| !d.blocks.contains_key(q))
        .collect();
    for q in deepen {
        d.blocks.insert(q, "minecraft:air".to_string());
        opened += 1;
    }
    // an open cell the rock now floors (in the void nothing lay below it) with two open cells above
    // is a new golem spawn spot (and a mob's): a torch there (no collision, so no sight is cut, and
    // it lights the dark); never in water's way (water breaks a torch)
    let open = |d: &mctas::design::Design, q: &[i32; 3]| d.blocks.get(q).is_some_and(|s| name(s) == "air");
    let spots: Vec<[i32; 3]> = d
        .blocks
        .keys()
        .filter(|c| {
            let below = [c[0], c[1] - 1, c[2]];
            open(&d, c) && !fluids.contains(*c) && !d.blocks.contains_key(&below) && !design.blocks.contains_key(&below) && open(&d, &[c[0], c[1] + 1, c[2]]) && open(&d, &[c[0], c[1] + 2, c[2]])
        })
        .copied()
        .collect();
    for c in &spots {
        d.blocks.insert(*c, "minecraft:torch".to_string());
    }
    let (lo, hi) = bounds(design);
    // (`ground:` as the site reads it: the player's feet on the natural surface, its top block one
    // below; the farm's top under that block, and the ground 8 above every villager: a golem search
    // starts at the villager's cell or one above it (woken in a 1-cell pod) and scans down from 6
    // above that, so a natural top block at the start + 6 or lower is a spot outside the farm)
    let top_villager = design.entities.iter().filter(|e| e.kind.trim_start_matches("minecraft:") == "villager").map(|e| e.pos[1].floor() as i32).max().unwrap_or(hi[1]);
    let ground = (hi[1] + 1 + cover).max(top_villager + 8);
    d.meta.insert("terrain".into(), "rock".into());
    d.meta.insert("ground".into(), ground.to_string());
    d.meta.insert("afk".into(), format!("{} {} {}", (lo[0] + hi[0]) / 2, ground + 1, (lo[2] + hi[2]) / 2));
    (d, drop.len(), opened)
}
