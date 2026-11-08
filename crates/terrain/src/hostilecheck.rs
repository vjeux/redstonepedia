//! `mctas terrain hostile`: a design pasted at a spot of a terrain sample, then the worst-case
//! checks (`redsim::entity::mob::hostile`) with the farm read from the design: its blocks, its
//! villagers (standing and sleeping boxes at their spec positions), its zombie and vehicle, its
//! beds, the golem spawn cells inside its footprint, the AFK spot it declares.

use crate::pastecheck;
use anyhow::{Context, Result};
use redsim::entity::mob::hostile::{self, Farm, Refusal};
use redsim::entity::{Aabb, Vec3};
use redsim::pos::BlockPos;
use redsim::terrain::Terrain;

/// The farm of a pasted design.
pub fn farm_of(w: &redsim::World, design: &mctas::design::Design, paste: &pastecheck::Paste, afk: [f64; 3]) -> Farm {
    let [ox, oy, oz] = paste.offset;
    let (ox, oy, oz) = (ox as f64, oy as f64, oz as f64);
    let mut farm = Farm { afk: Vec3::new(afk[0] + ox, afk[1] + oy, afk[2] + oz), ..Farm::default() };
    for p in design.blocks.keys() {
        let q = BlockPos::new(p[0] + paste.offset[0], p[1] + paste.offset[1], p[2] + paste.offset[2]);
        farm.blocks.push(q);
        if let redsim::Block::Bed { head: true, .. } = w.get(q) {
            farm.beds.push(q);
        }
    }
    for e in &design.entities {
        let (x, y, z) = (e.pos[0] + ox, e.pos[1] + oy, e.pos[2] + oz);
        match e.kind.as_str() {
            "minecraft:villager" => {
                // standing and sleeping (`LivingEntity.SLEEPING_DIMENSIONS` 0.2)
                farm.villagers.push(Aabb::new(x - 0.3, y, z - 0.3, x + 0.3, y + 1.95, z + 0.3));
                farm.villagers.push(Aabb::new(x - 0.1, y, z - 0.1, x + 0.1, y + 0.2, z + 0.1));
                farm.villager_eyes.push(Vec3::new(x, y + 1.62, z));
                farm.villager_eyes.push(Vec3::new(x, y + 0.2, z));
            }
            k if k.ends_with("_boat") || k.ends_with("_raft") || k.contains("minecart") => {
                farm.zombie.push(Aabb::new(x - 0.6875, y, z - 0.6875, x + 0.6875, y + 0.5625, z + 0.6875));
                if e.nbt.as_deref().is_some_and(|n| n.contains("zombie")) {
                    farm.zombie.push(Aabb::new(x - 0.3, y + 0.2, z - 0.3, x + 0.3, y + 2.15, z + 0.3));
                }
            }
            "minecraft:zombie" => farm.zombie.push(Aabb::new(x - 0.3, y, z - 0.3, x + 0.3, y + 1.95, z + 0.3)),
            _ => {}
        }
    }
    // the golem cells the farm's own floors give (the search's spots inside the footprint)
    for s in farm.villagers.iter().map(|v| BlockPos::new(v.center().x.floor() as i32, v.min_y.floor() as i32, v.center().z.floor() as i32)).collect::<Vec<_>>() {
        for dz in -8..=8 {
            for dx in -8..=8 {
                if let Some(c) = redsim::entity::mob::spawn::possible_spawn_position(w, s.offset(dx, 6, dz), 6)
                    && c.x >= paste.lo.x
                    && c.x <= paste.hi.x
                    && c.z >= paste.lo.z
                    && c.z <= paste.hi.z
                    && !farm.golem_cells.contains(&c)
                {
                    farm.golem_cells.push(c);
                }
            }
        }
    }
    // the interior: the cells the farm's entities occupy and the golem cells (and the two above)
    let mut interior: Vec<BlockPos> = Vec::new();
    for b in farm.villagers.iter().chain(farm.zombie.iter()) {
        for y in b.min_y.floor() as i32..=(b.max_y - 1.0e-6).floor() as i32 {
            for z in b.min_z.floor() as i32..=(b.max_z - 1.0e-6).floor() as i32 {
                for x in b.min_x.floor() as i32..=(b.max_x - 1.0e-6).floor() as i32 {
                    interior.push(BlockPos::new(x, y, z));
                }
            }
        }
    }
    for c in &farm.golem_cells {
        for dy in 0..3 {
            interior.push(c.offset(0, dy, 0));
        }
    }
    interior.sort_by_key(|p| (p.x, p.y, p.z));
    interior.dedup();
    farm.interior = interior;
    farm
}

/// Paste and check.
pub fn run(sample: &Terrain, design: &mctas::design::Design, at: [i32; 2], ground: i32, afk: [f64; 3]) -> Result<(pastecheck::Paste, Farm, Vec<Refusal>)> {
    let (w, paste) = pastecheck::paste_redsim(sample, design, at, ground).context("pasting")?;
    let farm = farm_of(&w, design, &paste, afk);
    let refusals = hostile::check(&w, &farm, true);
    Ok((paste, farm, refusals))
}
