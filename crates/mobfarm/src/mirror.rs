//! The z-mirror switch for the controller's block benches (onechunk rev 3, farmopt Nov 12 07:47: N = 4
//! uses a z-mirrored southern pair, adopted only after each lane's timing benches pass on the mirrored
//! parts; same-tick update orders and rail junction settling are not mirror-symmetric). With the switch
//! on, a bench runs its whole spec mirrored z -> -z (positions; block states by a half turn and a
//! front-to-back mirror, which together swap north and south; entities z -> 1 - z), and every cell the
//! bench drives or reads goes through `bp` / `at`, so the bench's logic and its printed cells stay in its
//! own frame.
//!
//! Limits (G1 on 47bf6d024): entities' SNBT is copied as is, so an entity spawned with z motion or a yaw
//! needs its `Motion` z negated and its `Rotation` yaw mirrored by the bench itself; the switch is one
//! static for the process, so unit tests exercise `state()` only (cargo runs tests in parallel threads);
//! and these runs show redsim's update order is mirror-safe for a circuit, not the game's.

use std::sync::atomic::{AtomicBool, Ordering};

use redsim::block::Block;
use redsim::entity::mob::farm::FarmSpec;
use redsim::pos::BlockPos;

static MIRROR_Z: AtomicBool = AtomicBool::new(false);

/// Turn the switch on or off (the CLI's `--mirror-z`).
pub fn set(on: bool) {
    MIRROR_Z.store(on, Ordering::Relaxed);
}

pub fn on() -> bool {
    MIRROR_Z.load(Ordering::Relaxed)
}

/// A cell of the bench's own frame, in the world.
pub fn bp(x: i32, y: i32, z: i32) -> BlockPos {
    BlockPos::new(x, y, if on() { -z } else { z })
}

pub fn at(p: [i32; 3]) -> BlockPos {
    bp(p[0], p[1], p[2])
}

/// An entity's z of the bench's frame, in the world (the cell z -> -z maps [z, z + 1) onto [-z, 1 - z)).
pub fn zf(z: f64) -> f64 {
    if on() { 1.0 - z } else { z }
}

/// A block state mirrored z -> -z: a half turn, then front to back (x -> -x).
pub fn state(s: &str) -> String {
    clock::canon::mirror_state(&clock::canon::rotate_state(s, 2))
}

/// The spec a bench runs: a copy, mirrored when the switch is on.
pub fn spec(s: &FarmSpec) -> Result<FarmSpec, String> {
    let mut m = s.clone();
    if !on() {
        return Ok(m);
    }
    for (p, b) in m.blocks.iter_mut() {
        *p = BlockPos::new(p.x, p.y, -p.z);
        let s = b.to_string();
        *b = Block::parse(&state(&s)).map_err(|e| format!("mirroring {s}: {e}"))?;
    }
    for (_, pos, _) in m.entities.iter_mut() {
        pos[2] = 1.0 - pos[2];
    }
    m.output = BlockPos::new(m.output.x, m.output.y, -m.output.z);
    let (lo, hi) = (m.lo, m.hi);
    m.lo = BlockPos::new(lo.x, lo.y, -hi.z);
    m.hi = BlockPos::new(hi.x, hi.y, -lo.z);
    for (p, _) in m.block_entities.iter_mut() {
        *p = BlockPos::new(p.x, p.y, -p.z);
    }
    Ok(m)
}

/// The label a bench prints when the switch is on.
pub fn tag() -> &'static str {
    if on() { " [z-MIRRORED: z -> -z, north <-> south]" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_z_states() {
        let r = state("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]");
        assert!(r.contains("facing=north") && r.contains("delay=1"), "{r}");
        let c = state("minecraft:comparator[facing=east,mode=subtract,powered=false]");
        assert!(c.contains("facing=east") && c.contains("mode=subtract"), "{c}");
        let t = state("minecraft:rail[shape=south_east,waterlogged=false]");
        assert!(t.contains("shape=north_east"), "{t}");
        let a = state("minecraft:powered_rail[powered=false,shape=ascending_north,waterlogged=false]");
        assert!(a.contains("shape=ascending_south"), "{a}");
        let w = state("minecraft:redstone_wire[east=none,north=side,power=0,south=none,west=up]");
        assert!(w.contains("south=side") && w.contains("north=none") && w.contains("west=up") && w.contains("east=none"), "{w}");
        let q = state("minecraft:redstone_wall_torch[facing=north,lit=true]");
        assert!(q.contains("facing=south"), "{q}");
    }
}
