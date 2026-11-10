//! `mctas circuit memory release-gen`: the single-lane loop's RELEASE CONTROLLER (day-1 lane
//! 2026-11-10; vjeux 07:24 "what's the setup that sends them 3 by 3"; ironfarm 07:27-07:38). Real
//! redstone: the golem pulse in; out: REL (one ~8-gt pulse per release, starting 2 gt after the
//! golem), the release group's window (REL extended to ~32 gt) and the advance pulses (REL + 38,
//! + 58, + 78) that ironfarm's queue register needs.
//!
//! Core (components at y 1 on supports at y 0):
//! - TMR: a 3-comparator countdown ring (I2 (0,0) -> cA -> S1 (2,0) -> cB -> S2 (2,2) -> cC -> dust
//!   -> I2), refilled to 15 by each release and losing 1 per 6-gt round.
//! - BUSY = S1 - 2 (subtract comparator B (3,0) on S1, a constant 2 from comparator K (3,-1)
//!   reading a hopper of 23 items), through a repeater (4,0) into GO's west side: high until ~95 gt
//!   after a release.
//! - EXPIRE: a wall torch on S2, lit when the ring is empty (~106 gt after a release with no
//!   golem): the fallback.
//! - GO (5,0) = X - BUSY, X = golem OR REQ OR EXPIRE (dust from (5,1)): its output is REL; REL
//!   refills TMR, so BUSY cuts it ~8 gt later: one self-timed pulse per release.
//! - REQ: a waxed copper bulb (7,1) toggled ON by X's rising edge (subtract comparator E (6,1):
//!   X minus X 6 gt late) and OFF by REL 8 gt late (repeater (7,0)); a comparator (8,1) reads it
//!   back into X: a golem arriving while BUSY is held until BUSY ends, never dropped.

use crate::schedgen::{B, DUST, rep};
use anyhow::Result;
use mctas::design::Design;
use std::collections::BTreeMap;
use std::path::Path;

const STONE: &str = "minecraft:stone";

fn comp(facing: &str, mode: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode={mode},powered=false]")
}

pub fn generate(out: &Path, offdelay: &Path) -> Result<()> {
    let mut b = B { blocks: BTreeMap::new(), ports: BTreeMap::new(), nbt: BTreeMap::new() };
    let y = 1;
    // ---- TMR ring
    b.put([0, y, 0], STONE)?; // I2
    b.on([1, y, 0], &comp("west", "compare"))?; // cA: rear I2, out east into S1
    b.put([2, y, 0], STONE)?; // S1
    b.on([2, y, 1], &comp("north", "compare"))?; // cB: rear S1, out south into S2
    b.put([2, y, 2], STONE)?; // S2
    b.on([1, y, 2], &comp("east", "compare"))?; // cC: rear S2, out west
    b.on([0, y, 2], DUST)?;
    b.on([0, y, 1], DUST)?;
    b.on([0, y, -1], &rep("north", 1))?; // the refill, input (0,-2)
    b.put([2, y, 3], "minecraft:redstone_wall_torch[facing=south,lit=true]")?; // EXPIRE
    // ---- BUSY
    b.on([3, y, 0], &comp("west", "subtract"))?; // B: rear S1
    b.on([3, y, -1], &comp("north", "compare"))?; // K: rear the hopper, out south into B
    b.put([3, y, -2], "minecraft:hopper[enabled=true,facing=down]")?;
    b.nbt.insert([3, y, -2], "{Items:[{Slot:0b,id:\"minecraft:stone\",count:23}]}".into());
    b.on([4, y, 0], &rep("west", 1))?; // B's output into GO's west side at full strength: GO
                                       // subtracts it from X, so BUSY must be 15 to block any X
    // ---- GO and X
    b.on([5, y, 0], &comp("south", "subtract"))?; // rear (5,1), out north (5,-1)
    for c in [[5, y, 1], [5, y, 2], [5, y, 3], [4, y, 3], [3, y, 3], [5, y, 4]] {
        b.on(c, DUST)?;
    }
    b.ports.insert("input.golem".into(), [5, y, 5]);
    // ---- E (X's rising edge) and the REQ bulb
    b.on([6, y, 1], &comp("west", "subtract"))?; // rear X (5,1), out east into the bulb
    b.on([6, y, 2], &rep("south", 3))?; // X (6,3) 6 gt late into E's south side
    b.on([6, y, 3], DUST)?;
    b.put([7, y, 1], "minecraft:waxed_copper_bulb[lit=false,powered=false]")?;
    // (the bulb's reset comes from the delay chain below)
    b.on([8, y, 1], &comp("west", "compare"))?; // reads the bulb, out east (9,1)
    for c in [[9, y, 1], [9, y, 2], [9, y, 3], [7, y, 3]] {
        b.on(c, DUST)?;
    }
    b.on([8, y, 3], &rep("east", 1))?; // REQ back into X at full strength
    // ---- REL: GO's output north; the refill route west along z -3 (a repeater restores it)
    for c in [[5, y, -1], [5, y, -2], [5, y, -3], [4, y, -3], [3, y, -3], [2, y, -3], [1, y, -3], [0, y, -3], [0, y, -2], [5, y, -4]] {
        b.on(c, DUST)?;
    }
    // ---- the delay chain north along x 5: stones at REL + 8, 16, 24, 32, 38 (A1), 46, 54, 58 (A2),
    // 66, 74, 78 (A3)
    let stages: [(u8, bool, bool); 11] = [
        (4, true, false),
        (4, true, false),
        (4, true, false),
        (4, true, false),
        (3, true, true),
        (4, true, false),
        (4, false, false),
        (2, false, true),
        (4, false, false),
        (4, false, false),
        (2, false, true),
    ];
    let mut z = -5;
    for (i, (delay, reset_tap, adv_tap)) in stages.into_iter().enumerate() {
        b.on([5, y, z], &rep("south", delay))?;
        b.put([5, y, z - 1], STONE)?;
        if reset_tap {
            b.on([6, y, z - 1], &rep("west", 1))?; // out east into the reset line (x 7)
        }
        if adv_tap {
            b.on([4, y, z - 1], &rep("east", 1))?; // out west into the advance line (x 3)
        }
        if i < 3 {
            b.on([4, y, z - 1], &rep("east", 1))?; // out west into the group line (x 3, z -5..-11)
        }
        z -= 2;
    }
    // the release group: REL through R_g (4,-4) (a diode, so nothing flows back into REL) and the
    // chain's +8/+16/+24 stones: ON from REL + 2 to REL + 36
    b.on([4, y, -4], &rep("south", 1))?;
    b.on([4, y, -5], DUST)?;
    for zz in -11..=-5 {
        b.on([3, y, zz], DUST)?;
    }
    b.on([2, y, -11], DUST)?;
    b.ports.insert("output.group".into(), [1, y, -11]);
    // the bulb's reset: REL + 8 .. REL + 56 continuous (overlapping 10-gt pulses), so an X edge
    // inside it toggles nothing (the merge window); its rising edge turns REQ off
    for zz in -16..=-1 {
        b.on([7, y, zz], DUST)?;
    }
    b.on([7, y, 0], &rep("north", 1))?; // into the bulb
    // the advance line: x 3 from z -15 to -27, out north
    for zz in -27..=-14 {
        b.on([3, y, zz], DUST)?;
    }
    b.ports.insert("output.adv".into(), [3, y, -28]);
    let _ = offdelay;
    let mut d = Design::parse("name: carousel-release-controller\n")?;
    d.blocks = b.blocks;
    d.ports = b.ports;
    d.block_entities = b.nbt;
    d.save(out)?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}
