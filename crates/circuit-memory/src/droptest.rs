//! `mctas circuit memory drop-test`: the bed siding's rejoin as a side drop (day-1 lane 2026-11-10,
//! the parent 12:11: the junctions joining farmopt's siding to the loop). A queue slot like the
//! release group's (a braked powered rail sloping up south at (0,0,0), the peak (0,1,1) south of it,
//! the next slot's up-slope (0,0,-1) and peak (0,1,-2) north), and a side track at y 1 from the
//! west ending at (-1,1,0) over the slot's cell, a stop block at (1,1,0). A ridden cart comes along
//! the side track moving +x at the given speed; the run prints where it goes, tick by tick, and where
//! it rests: the drop works if it rests on the slot's rail (x -0.5..0.5 centre, y near the slope,
//! z inside the slot's cell) and stays there. `--occupied` puts a ridden cart at rest in the slot
//! south of the peak (the next slot back), to see that the drop doesn't disturb it.

use anyhow::{Result, anyhow};
use redsim::pos::BlockPos;

pub fn run(speed: f64, occupied: bool, ticks: u32, stop: bool, staged: Option<u32>) -> Result<bool> {
    let min = BlockPos::new(-8, -3, -6);
    let mut w = redsim::world::World::new(min, [16, 10, 16]);
    // (placed raw, as written: the track's states exactly as in the loop file)
    let mut blocks: Vec<(BlockPos, redsim::Block)> = Vec::new();
    let mut put = |p: [i32; 3], s: &str| -> Result<()> {
        let b = redsim::Block::parse(s).map_err(|e| anyhow!("{s}: {e}"))?;
        blocks.push((BlockPos::new(p[0], p[1], p[2]), b));
        Ok(())
    };
    let glass = "minecraft:glass";
    let slot = "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]";
    let up = "minecraft:rail[shape=ascending_north,waterlogged=false]";
    let peak = "minecraft:rail[shape=north_south,waterlogged=false]";
    // the slot and its neighbours (the real track at x -3 around slot 16, moved to x 0): going
    // north (-z), a slot A, its up-slope R, a peak K, the next slot A
    for (z, s, y) in [(-2, peak, 1), (-1, up, 0), (0, slot, 0), (1, peak, 1), (2, up, 0), (3, slot, 0), (4, peak, 1), (5, up, 0)] {
        put([0, y - 1, z], glass)?;
        put([0, y, z], s)?;
    }
    // the side track from the west, ending over the slot's cell (`staged`: the cart waits at rest on
    // a braked powered rail at (-2,1,0) against a stop (-3,1,0) and is released at tick N: a
    // redstone block put under the powered rail, which kicks it east off the stop)
    let first = if staged.is_some() { -2 } else { -6 };
    for x in first..=-1 {
        put([x, 0, 0], glass)?;
        put([x, 1, 0], if staged.is_some() && x == -2 { "minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]" } else { "minecraft:rail[shape=east_west,waterlogged=false]" })?;
    }
    if staged.is_some() {
        put([-3, 1, 0], "minecraft:stone")?;
    }
    if stop {
        put([1, 1, 0], "minecraft:stone")?;
    }
    blocks.sort_by_key(|(p, _)| *p);
    w.place_raw(&blocks);
    let (start, motion) = if staged.is_some() { ([-1.5, 1.0625, 0.5], 0.0) } else { ([-4.5, 1.0625, 0.5], speed) };
    let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", start, Some(&format!("{{Motion:[{motion}d,0d,0d],Passengers:[{{id:\"minecraft:villager\"}}]}}"))).map_err(|e| anyhow!(e))?;
    let other = if occupied {
        Some(redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 0.5625, 3.5], Some("{Passengers:[{id:\"minecraft:villager\"}]}")).map_err(|e| anyhow!(e))?)
    } else {
        None
    };
    let pos = |w: &redsim::world::World, id: u32| redsim::entity::index_of(w, id).map(|i| (w.entities.list[i].pos, w.entities.list[i].delta_movement));
    let mut last = None;
    for t in 0..=ticks {
        if let Some((p, v)) = pos(&w, cart) {
            let o = other.and_then(|o| pos(&w, o)).map(|(q, _)| format!("  other {:.2},{:.2},{:.2}", q.x, q.y, q.z)).unwrap_or_default();
            println!("t={t:3} cart {:.3},{:.3},{:.3} v {:.3},{:.3},{:.3}{o}", p.x, p.y, p.z, v.x, v.y, v.z);
            last = Some(p);
        } else {
            println!("t={t:3} the cart is gone");
            return Ok(false);
        }
        if staged == Some(t) {
            let b = redsim::Block::parse("minecraft:redstone_block").map_err(|e| anyhow!(e))?;
            w.set_block(BlockPos::new(-2, 0, 0), b, redsim::world::flags::ALL);
            println!("t={t:3} released");
        }
        w.tick();
    }
    let p = last.ok_or_else(|| anyhow!("no cart"))?;
    let ok = (p.x - 0.5).abs() < 0.3 && p.z >= 0.0 && p.z < 1.0 && p.y > 0.05 && p.y < 1.0;
    println!("rests at {:.3},{:.3},{:.3}: {}", p.x, p.y, p.z, if ok { "IN THE SLOT" } else { "NOT in the slot" });
    Ok(ok)
}
