//! `mctas circuit memory loop-gen`: the single-lane carousel's nightly scheduler (day-1 lane
//! 2026-11-10; vjeux 06:01: one queue of 21 carts, the front 3 released per release pulse; ironfarm
//! 06:20: the napper keeps its exact place, so the release index mod 7 names the batch; ironspec:
//! the 7-lane policy re-keyed to the release order).
//!
//! The policy (pointer P +4 per visit, release ring R +1 per release, a visit when the batch leaving
//! the station is P's) needs only the difference D = P - R (mod 7): ONE 7-state dropper/hopper ring.
//! Each release fires it once (D - 1); a visit happens at a release that finds D = 0 inside the night
//! window, and then the ring fires 3 more times (D 0 -> 6 -> 3: the pointer's +4), from a delay chain.
//!
//! Layout (x east, z south, components at y 0 on supports at y -1, the ring's loop at y 1):
//! - D ring: droppers d0..d6 and hoppers in two columns x 0 (z 0..6) and x 1 (z 6..0), d0 at
//!   (0, 0, 0) holding the token; its loop fired through the stone (0, 1, -1).
//! - D = 0 tap: a comparator (-1, 0, 0) on d0, a repeater (-2, 0, 0), the block T0 (-3, 0, 0).
//! - The gate: V = release - (NOT T0 or NOT win): a subtract comparator G (-5, 0, 1) facing north,
//!   its rear the release (a repeater (-5, 0, 0) from the block B1 (-5, 0, -1)), its side the dust
//!   (-4, 0, 1) lit by the torches on T0 (-4, 0, 0) and on the window cell (-2, 0, 1) (torch
//!   (-3, 0, 1)).
//! - The release: its input cell (-5, 0, -3), a repeater into B1; B1 also lights the dust on top of
//!   it, a wire at y 1 east to a repeater into the loop's stone.
//! - V: the gate's output (-5, 0, 2), a repeater into V's stone (-5, 0, 4); a delay chain west along
//!   z 4 of 3 stages of 20 gt, each stage's end stone lighting a column of dust north at y 1 to a
//!   repeater into an OR line along z -5 at y 1, which runs east and down into the loop's stone
//!   (the pointer's +4: 3 more firings 20 gt apart).

use crate::schedgen::{B, DUST, rep, seg};
use anyhow::Result;
use mctas::design::Design;
use std::collections::BTreeMap;
use std::path::Path;

const STONE: &str = "minecraft:stone";

fn comp(facing: &str, mode: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode={mode},powered=false]")
}

fn wall_torch(facing: &str) -> String {
    format!("minecraft:redstone_wall_torch[facing={facing},lit=true]")
}

pub fn generate(out: &Path, modules: &Path, offdelay: &Path) -> Result<()> {
    let mut b = B { blocks: BTreeMap::new(), ports: BTreeMap::new(), nbt: BTreeMap::new() };
    // ---- the D ring
    for k in 0..14i32 {
        let (x, z) = if k < 7 { (0, k) } else { (1, 13 - k) };
        let facing = match k {
            0..=5 => "south",
            6 => "east",
            7..=12 => "north",
            _ => "west",
        };
        let s = if k % 2 == 0 { format!("minecraft:dropper[facing={facing},triggered=false]") } else { format!("minecraft:hopper[enabled=true,facing={facing}]") };
        b.put([x, 0, z], &s)?;
        b.on([x, 1, z], DUST)?;
    }
    b.nbt.insert([0, 0, 0], "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}".into());
    b.put([0, 1, -1], STONE)?;
    // ---- the D = 0 tap and its block T0
    b.on([-1, 0, 0], &comp("east", "compare"))?;
    b.on([-2, 0, 0], &rep("east", 1))?;
    b.put([-3, 0, 0], STONE)?;
    // ---- the gate
    b.put([-4, 0, 0], &wall_torch("west"))?;
    b.on([-4, 0, 1], DUST)?;
    b.on([-5, 0, 1], &comp("north", "subtract"))?;
    b.on([-5, 0, 0], &rep("north", 1))?;
    b.ports.insert("input.win".into(), [-2, 0, 1]);
    b.put([-2, 0, 1], STONE)?;
    b.put([-3, 0, 1], &wall_torch("west"))?;
    // ---- the release: input cell, repeater into B1, B1's top dust east at y 1 into the loop
    b.ports.insert("input.release".into(), [-5, 0, -3]);
    b.put([-5, 0, -3], STONE)?;
    b.on([-5, 0, -2], &rep("north", 1))?;
    b.put([-5, 0, -1], STONE)?;
    b.on([-5, 1, -1], DUST)?;
    b.wire([-5, 1, -1], &seg([-4, 1, -1], [-2, 1, -1]), 0)?;
    b.on([-1, 1, -1], &rep("west", 1))?;
    // ---- V: the gate's output south, a repeater into V's stone
    b.on([-5, 0, 2], DUST)?;
    b.on([-5, 0, 3], &rep("north", 1))?;
    b.put([-5, 0, 4], STONE)?;
    // ---- the delay chain west along z 4: the 3 extra firings must be done before the next release
    // can fire the ring (61 gt after this one at the soonest): stage 1 = 4 gt (its pulse reaches the
    // loop 18 gt after the release, once the release's own step is done), stages 2 and 3 = 14 gt
    // (each step: the dropper 4 gt, the hopper locked while the loop is powered, 10 gt, then its
    // 8-gt cooldown; the next pulse 2 gt after the hopper pushes)
    let mut x = -6;
    let mut ends = Vec::new();
    for stage in [vec![2u8], vec![4, 3], vec![4, 3]] {
        for d in stage {
            b.on([x, 0, 4], &rep("east", d))?;
            x -= 1;
        }
        b.put([x, 0, 4], STONE)?;
        ends.push(x);
        x -= 1;
    }
    // each end stone's top dust north at y 1 to z -3, a repeater into the OR line at z -5
    for &e in &ends {
        b.on([e, 1, 4], DUST)?;
        b.wire([e, 1, 4], &seg([e, 1, 3], [e, 1, -3]), 0)?;
        b.on([e, 1, -4], &rep("south", 1))?;
    }
    // the OR line along z -5 at y 1 from the westmost branch east to x 0, a repeater every 8,
    // then south into a repeater at (0, 1, -2) facing north into the loop's stone
    let west = *ends.iter().min().unwrap();
    let mut line: Vec<[i32; 3]> = seg([west, 1, -5], [0, 1, -5]);
    line.extend(seg([0, 1, -4], [0, 1, -3]));
    let mut prev = [west - 1, 1, -5];
    for (i, c) in line.iter().enumerate() {
        let is_branch = ends.contains(&c[0]) && c[2] == -5;
        if i > 0 && !is_branch && c[2] == -5 && (c[0] - west) % 8 == 4 {
            b.on(*c, &rep("west", 1))?;
        } else {
            b.on(*c, DUST)?;
        }
        prev = *c;
    }
    let _ = prev;
    b.on([0, 1, -2], &rep("north", 1))?;
    // ---- the visit window X = [T + ~12, T + ~88] (ironfarm: v down before the next release, the
    // napper's batch at the siding junction T + 20..60): a comparator latch just south of V's stone
    // (V powers its loop: the set), reset by a 4th chain stage (44 gt after the 3rd) coming back
    // east along z 6; the same reset, 8 gt later, steps the level ring of the output stage, so the
    // slot holds through the window. Output stage: the slot-out module (level ring, its taps
    // subtract comparators against NOT X: s_j = X AND l_j), mirrored east-west at (-3, -1, 17).
    // The latch: C (comparator, subtract) at (-5, 0, 6) facing north, R the repeater after it, a
    // loop of dust around its east side back into its rear (C's east side stays empty).
    b.on([-5, 0, 6], &comp("north", "subtract"))?;
    b.on([-5, 0, 7], &rep("north", 1))?;
    for c in [[-5, 0, 8], [-4, 0, 8], [-3, 0, 8], [-3, 0, 7], [-3, 0, 6], [-3, 0, 5], [-4, 0, 5], [-5, 0, 5]] {
        b.on(c, DUST)?;
    }
    // the reset: from the 3rd stage's end stone, a repeater south into a corner stone, 5 repeaters
    // east along z 6 into the reset stone (-7, 0, 6), a repeater into C's west side (4 + 40 gt)
    let e3 = *ends.last().unwrap();
    b.on([e3, 0, 5], &rep("north", 1))?;
    b.put([e3, 0, 6], STONE)?;
    for x in e3 + 1..=-8 {
        b.on([x, 0, 6], &rep("west", if x == -8 { 3 } else { 4 }))?;
    }
    b.put([-7, 0, 6], STONE)?;
    b.on([-6, 0, 6], &rep("west", 1))?;
    // the output stage, mirrored: its X block (-4, 0, 10) fed by a repeater (-3, 0, 10) facing east
    // from the latch's loop corner (-3, 0, 8) south and east; its step cell (-7, 1, 17) at the end
    // of the reset stone's top dust south along x = -7 at y 1 (an 8-gt repeater on the way)
    let (ox, oz) = (-3, 17);
    b.stamp_mirrored(&modules.join("slotout.txt"), [ox, -1, oz])?;
    b.ports.insert("output.s0".into(), [ox, 0, oz - 3]);
    b.ports.insert("output.s1".into(), [ox - 2, 0, oz - 3]);
    b.ports.insert("output.s2".into(), [ox - 1, 0, oz + 4]);
    b.ports.insert("output.v".into(), [ox - 1, 0, oz - 7]);
    b.on([-3, 0, 9], DUST)?;
    b.on([-2, 0, 9], DUST)?;
    b.on([-2, 0, 10], DUST)?;
    b.on([ox, 0, oz - 7], &rep("east", 1))?;
    b.on([-7, 1, 6], DUST)?;
    for z in 7..=17 {
        if z == 12 {
            b.on([-7, 1, z], &rep("north", 4))?;
        } else {
            b.on([-7, 1, z], DUST)?;
        }
    }
    let _ = offdelay;
    let mut d = Design::parse("name: carousel-loop-scheduler\n")?;
    d.blocks = b.blocks;
    d.ports = b.ports;
    d.block_entities = b.nbt;
    d.save(out)?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}
