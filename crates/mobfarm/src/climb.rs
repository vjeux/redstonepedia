//! One-chunk (b): the climb bench (ironspec and farmopt 2026-11-11, paper v0 "Iron carousel in one
//! chunk"): one batch of 3 ridden carts from rest at the foot hold H, up a straight powered climb of
//! `cells` boosters (no curves) to today's approach top T, down today's two slopes and the snap
//! turn into today's 3 pockets, every cell as the S3 file lays it (s3-wired-pair-c-a: the design
//! frame = the loop's + (6,0,6), the approach along x 3 heading -z, the stand's strip along z 6
//! heading +x). The pockets' gates bench-driven by position triggers as the queue-loop bench's
//! stand-stop (gate k from the east lets k carts by, shutting on its k-th leaving cart's centre past
//! the gate cell + `close_at`), H a fence gate the bench opens at T + `h_open` (T: a world time that
//! is a multiple of 100, the summon tick). Printed: each cart's tick at rest (from T) and its margin
//! before T + 100, each arrival into a waiting cart's speed, the riders' damage, dismounts and eye
//! point inside a collision shape, each rider's least box gap to the golem box while a golem
//! exists (T..T+2), and with `freeze`, the bench's chunks at level 32 (redstone running, entities
//! frozen) for a span and what each cart did.

use crate::layouts::Builder;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{EntityKind, Vec3};
use redsim::pos::BlockPos;

/// The x of the approach column and the strip's z (the design frame).
const AX: i32 = 3;
const SZ: i32 = 6;

/// The stand's strip as the S3 file lays it: (x, y, state) on z 6, with each gate's block over its
/// slope's high end.
const STRIP: [(i32, i32, &str); 13] = [
    (3, 2, "rail[shape=ascending_west,waterlogged=false]"),
    (4, 1, "rail[shape=ascending_west,waterlogged=false]"),
    (5, 0, "rail[shape=ascending_west,waterlogged=false]"),
    (6, -1, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (7, -1, "rail[shape=ascending_east,waterlogged=false]"),
    (8, 0, "rail[shape=east_west,waterlogged=false]"),
    (9, -1, "rail[shape=ascending_west,waterlogged=false]"),
    (10, -2, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (11, -2, "rail[shape=ascending_east,waterlogged=false]"),
    (12, -1, "detector_rail[powered=false,shape=east_west,waterlogged=false]"),
    (13, -2, "rail[shape=ascending_west,waterlogged=false]"),
    (14, -3, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]"),
    (15, -3, "rail[shape=east_west,waterlogged=false]"),
];
/// The pockets' gates (x, y of the gate block) on z 6, west to east: G2, G1, G0.
const GATES: [(i32, i32); 3] = [(6, 0), (10, -1), (14, -2)];
const GATE_SHUT: &str = "oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]";
const GATE_OPEN: &str = "oak_fence_gate[facing=east,in_wall=false,open=true,powered=true]";
const H_SHUT: &str = "oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]";
const H_OPEN: &str = "oak_fence_gate[facing=north,in_wall=false,open=true,powered=true]";

/// T's cell z for a crest at height `crest` (today's 5: T at z 9, two slopes down to the snap turn;
/// 3, the lowest with today's snap turn (its high end at 3): T right before it, at z 7).
fn crest_z(crest: i32) -> i32 {
    7 + (crest - 3)
}

/// The climb's foot slope (its lowest booster) for `cells` boosters ending at T.
fn foot(cells: i32, crest: i32) -> (i32, i32) {
    (crest - cells, crest_z(crest) + cells)
}

/// The cornered route (onechunk's skeleton v1, loop frame + (6,0,6) here): Ga's hand-off cells B
/// (a flat always-on booster on a redstone block), S (a flat plain spacer) and Dh (a flat
/// detector rail) at rails `h_rails` along x 14 heading south, then leg 1 south along x 14 (its
/// first slope plain, the rest boosters) up to the first corner (14, h_rails + l1, 14), leg 2 west
/// along z 14 (all boosters) up to the second corner (3, 3, 14), `l1 + l2` = 3 - h_rails, then the
/// approach's flats north along x 3 to T (3,3,7) and today's snap turn and pockets. The 3 carts
/// parked off the track for the bench to hand off at B.
pub fn design_route(h_rails: i32, l1: i32, with_riders: [bool; 3]) -> Result<String, String> {
    let l2 = 3 - h_rails - l1;
    if l1 < 2 || l2 < 1 {
        return Err(format!("the route: leg 1 {l1} slopes, leg 2 {l2} (from rails {h_rails} to 3)"));
    }
    let mut b = Builder::new("climb-route-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    // T (crest 3) and the approach's flats north along x 3 from the second corner
    b.set(AX, 3, 7, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 2, 7, "glass");
    for z in 8..14 {
        b.set(AX, 3, z, "rail[shape=north_south,waterlogged=false]");
        b.set(AX, 2, z, "glass");
    }
    // the second corner (from leg 2 heading west, turning north)
    b.set(AX, 3, 14, "rail[shape=north_east,waterlogged=false]");
    b.set(AX, 2, 14, "glass");
    // leg 2: boosters west along z 14, x 13..4, the top one at x 4 rising to 3
    for i in 0..l2 {
        let (x, y) = (4 + i, 2 - i);
        b.set(x, y, 14, "powered_rail[powered=true,shape=ascending_west,waterlogged=false]");
        b.set(x, y - 1, 14, "redstone_block");
    }
    // the first corner (from leg 1 heading south, turning west)
    let cy = 3 - l2;
    b.set(4 + l2, cy, 14, "rail[shape=north_west,waterlogged=false]");
    b.set(4 + l2, cy - 1, 14, "glass");
    let lx = 4 + l2;
    // leg 1: slopes south along x lx up to the corner, the first plain
    for i in 0..l1 {
        let (y, z) = (cy - 1 - i, 13 - i);
        if i == l1 - 1 {
            b.set(lx, y, z, "rail[shape=ascending_south,waterlogged=false]");
            b.set(lx, y - 1, z, "glass");
        } else {
            b.set(lx, y, z, "powered_rail[powered=true,shape=ascending_south,waterlogged=false]");
            b.set(lx, y - 1, z, "redstone_block");
        }
    }
    // Ga's hand-off: Dh, S, B north of leg 1's foot
    let fz = 13 - l1 + 1;
    b.set(lx, h_rails, fz - 1, "detector_rail[powered=false,shape=north_south,waterlogged=false]");
    b.set(lx, h_rails - 1, fz - 1, "glass");
    b.set(lx, h_rails, fz - 2, "rail[shape=north_south,waterlogged=false]");
    b.set(lx, h_rails - 1, fz - 2, "glass");
    b.set(lx, h_rails, fz - 3, "powered_rail[powered=true,shape=north_south,waterlogged=false]");
    b.set(lx, h_rails - 1, fz - 3, "redstone_block");
    b.set(lx, h_rails, fz - 4, "rail[shape=north_south,waterlogged=false]");
    b.set(lx, h_rails - 1, fz - 4, "glass");
    b.set(lx, h_rails, fz - 5, "stone");
    // the carts parked off the track (the spare chunk)
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.d.meta.insert("climb_route".into(), format!("h_rails={h_rails} l1={l1} l2={l2} leg1_x={lx} b_z={}", fz - 3));
    Ok(b.text(&["One-chunk (b): the climb on onechunk's cornered route: Ga's hand-off (B, S, Dh), leg 1 south, a corner, leg 2 west, a corner, the approach's flats north to T, today's 3 pockets (gates bench-driven)."]))
}

/// The folded route (onechunk's skeleton (b), farmopt 16:36; here with today's strip where the S3
/// file lays it, so two blocks south of onechunk's): Dh (a flat detector rail) at (15,-17,16), a
/// flat curve (15,-17,17) turning west, leg 1 west along z 17 (11 boosters, x 14..4, rising -17 to
/// -6), a curve (3,-6,17) turning north, leg 2 north along x 3 (9 boosters, z 16..8, rising to 3),
/// T (3,3,7), today's snap turn and pockets. The carts parked for the bench to hand off at Dh.
pub fn design_folded(with_riders: [bool; 3]) -> String {
    let (l1, l2) = (11, 9);
    let mut b = Builder::new("climb-folded-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    b.set(AX, 3, 7, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 2, 7, "glass");
    // leg 2: boosters north along x 3, z 8..16, the top one at z 8 rising to 3
    for i in 0..l2 {
        let (y, z) = (2 - i, 8 + i);
        b.set(AX, y, z, "powered_rail[powered=true,shape=ascending_north,waterlogged=false]");
        b.set(AX, y - 1, z, "redstone_block");
    }
    let zl = 8 + l2;
    let cy = 3 - l2;
    b.set(AX, cy, zl, "rail[shape=north_east,waterlogged=false]");
    b.set(AX, cy - 1, zl, "glass");
    // leg 1: boosters west along z 17, x 4..14, the top one at x 4 rising to the corner's level
    for i in 0..l1 {
        let (x, y) = (4 + i, cy - 1 - i);
        b.set(x, y, zl, "powered_rail[powered=true,shape=ascending_west,waterlogged=false]");
        b.set(x, y - 1, zl, "redstone_block");
    }
    let (dx, dy) = (4 + l1, cy - l1);
    b.set(dx, dy, zl, "rail[shape=north_west,waterlogged=false]");
    b.set(dx, dy - 1, zl, "glass");
    b.set(dx, dy, zl - 1, "detector_rail[powered=false,shape=north_south,waterlogged=false]");
    b.set(dx, dy - 1, zl - 1, "glass");
    b.set(dx, dy, zl - 2, "rail[shape=north_south,waterlogged=false]");
    b.set(dx, dy - 1, zl - 2, "glass");
    b.set(dx, dy, zl - 3, "stone");
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.d.meta.insert("climb_route".into(), format!("folded: Dh ({dx},{dy},{}) curve ({dx},{dy},{zl}) leg1 x {}..4 z {zl} curve (3,{cy},{zl}) leg2 x 3 z {}..8 T (3,3,7)", zl - 1, 4 + l1 - 1, zl - 1));
    b.text(&["One-chunk (b) folded: Dh, a flat curve west, leg 1 (11 boosters), a curve north, leg 2 (9 boosters) to T at 3, today's snap turn and pockets."])
}

/// The gravity queue's descent (vjeux's gravity queue, paper v0 2098083320824660; ironspec and
/// farmopt 16:40): the hold above the golem zone hands its carts off at the top of a straight
/// descent of `cells` slopes down north along x 3 (each `B` of `pattern` a booster on a redstone
/// block, `P` a plain slope on glass; empty: all boosters) to T (3,3,7), the crest at +3, then
/// today's snap turn and pockets (the S3 file's cells). The hand-off flat at the top
/// (3, 3 + cells, 8 + cells), a stop past it; the carts parked off the track for the bench to hand
/// off there.
pub fn design_gravity(cells: i32, pattern: &str, with_riders: [bool; 3]) -> String {
    let mut b = Builder::new("gravity-descent-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    b.set(AX, 3, 7, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 2, 7, "glass");
    for i in 0..cells {
        let (y, z) = (3 + i, 8 + i);
        if pattern.chars().nth(i as usize) == Some('P') {
            b.set(AX, y, z, "rail[shape=ascending_south,waterlogged=false]");
            b.set(AX, y - 1, z, "glass");
        } else {
            b.set(AX, y, z, "powered_rail[powered=true,shape=ascending_south,waterlogged=false]");
            b.set(AX, y - 1, z, "redstone_block");
        }
    }
    let (ty, tz) = (3 + cells, 8 + cells);
    b.set(AX, ty, tz, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, ty - 1, tz, "glass");
    b.set(AX, ty, tz + 1, "stone");
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.d.meta.insert("gravity_descent".into(), format!("hand-off ({AX},{ty},{tz}) heading -z; {cells} slopes x {AX} z {}..8; T ({AX},3,7)", tz - 1));
    b.text(&["The gravity queue's descent bench: the hold's hand-off at the top, a straight boosted descent down north to the crest at +3, today's snap turn and 3 pockets (gates bench-driven)."])
}

/// The gravity queue's descent on onechunk's skeleton v3 (explore/farmopt-onechunk 63270bb35,
/// farmopt 17:10; loop frame + (6,0,6) here): the hand-off where the hold's last gate Ga lets go,
/// a flat at (13,24,7) heading south (riders' feet at 24.06); the entry slope (13,23,8) and leg A
/// south along x 13 (z 9..13, rails 22..18); a flat curve (13,18,14) turning west; leg B west along
/// z 14 (x 12..4, rails 17..9); a flat curve (3,9,14) turning north; leg C north along x 3 (z
/// 13..11, rails 8..6) into the committed crest (3,6,10) and the one S3 file's approach down to the
/// snap turn ((3,5,9), (3,4,8), (3,3,7), plain) and today's pockets. Every slope a booster on a
/// redstone block (`plain_legs`: leg C plain, as the committed approach's slopes are), the curves
/// plain on glass. (The skeleton's curve heights, 19 and 10, leave leg C one level short of the
/// crest at 6; the entry cell here is a slope, so the curves sit at 18 and 9.)
pub fn design_gravity_v3(with_riders: [bool; 3], plain_c: bool) -> String {
    let mut b = Builder::new("gravity-descent-v3-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    // the committed approach from the crest down to the snap turn
    b.set(AX, 6, 10, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 5, 10, "glass");
    for (y, z) in [(5, 9), (4, 8), (3, 7)] {
        b.set(AX, y, z, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, y - 1, z, "glass");
    }
    let boost = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str, plain: bool| {
        if plain {
            b.set(x, y, z, &format!("rail[shape={shape},waterlogged=false]"));
            b.set(x, y - 1, z, "glass");
        } else {
            b.set(x, y, z, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]"));
            b.set(x, y - 1, z, "redstone_block");
        }
    };
    // leg C: north along x 3, z 13..11, rails 8..6
    for (i, z) in (11..=13).enumerate() {
        boost(&mut b, AX, 6 + i as i32, z, "ascending_south", plain_c);
    }
    b.set(AX, 9, 14, "rail[shape=north_east,waterlogged=false]");
    b.set(AX, 8, 14, "glass");
    // leg B: west along z 14, x 4..12, rails 9..17
    for (i, x) in (4..=12).enumerate() {
        boost(&mut b, x, 9 + i as i32, 14, "ascending_east", false);
    }
    b.set(13, 18, 14, "rail[shape=north_west,waterlogged=false]");
    b.set(13, 17, 14, "glass");
    // leg A and the entry slope: south along x 13, z 8..13, rails 23..18
    for (i, z) in (8..=13).enumerate() {
        boost(&mut b, 13, 23 - i as i32, z, "ascending_north", false);
    }
    // the hand-off flat (Ga's exit, 9fc3155f's hold moved 3 cells north: Ga (13,24,4)), three
    // flat boosters (13,24,5..7) to the entry slope, a stop north of Ga
    b.set(13, 24, 4, "rail[shape=north_south,waterlogged=false]");
    b.set(13, 23, 4, "glass");
    for z in 5..=7 {
        b.set(13, 24, z, "powered_rail[powered=true,shape=north_south,waterlogged=false]");
        b.set(13, 23, z, "redstone_block");
    }
    b.set(13, 24, 3, "stone");
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["The gravity queue's descent on skeleton v3: Ga's exit, leg A south, a curve, leg B west, a curve, leg C north into the committed crest (3,6,10), the approach down to the snap turn, today's 3 pockets (gates bench-driven)."])
}

/// The gravity queue's climb back (onechunk's skeleton v3, farmopt 17:10; loop frame + (6,0,6)
/// here): the batch at rest in today's 3 pockets (gates shut, the S3 file's rest spots); the one S3
/// file's departure: G0's corner (15,-3,6), five boosters south (15,-3..1,7..11), the departure
/// crest (15,2,12); then two boosters up south (15,2..3,13..14), a flat curve (15,4,15) turning
/// west, 14 boosters up west along z 15 (x 14..1, rails 4..17), a flat curve (0,18,15) turning
/// north, 14 boosters up north along x 0 (z 14..1, rails 18..31), a flat curve (0,32,0) turning
/// east, 14 boosters up east along z 0 (x 1..14, rails 32..45) and the top (15,46,0) (the sleeper's
/// switch), a stop past it.
pub fn design_climb_back(with_riders: [bool; 3], east_slopes: i32, past: i32) -> String {
    let mut b = Builder::new("climb-back-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    for (x, y) in GATES {
        b.set(x, y, SZ, GATE_SHUT);
    }
    for (x, y, z) in [(6, -2, 6), (10, -3, 6), (14, -3, 7)] {
        b.set(x, y, z, "stone");
    }
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    let boost = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str| {
        b.set(x, y, z, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]"));
        b.set(x, y - 1, z, "redstone_block");
    };
    let flat = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str| {
        b.set(x, y, z, &format!("rail[shape={shape},waterlogged=false]"));
        b.set(x, y - 1, z, "glass");
    };
    // the departure: G0's corner, five boosters, the crest
    flat(&mut b, 15, -3, SZ, "south_west");
    for i in 0..5 {
        boost(&mut b, 15, -3 + i, 7 + i, "ascending_south");
    }
    flat(&mut b, 15, 2, 12, "north_south");
    // up south to the first curve
    boost(&mut b, 15, 2, 13, "ascending_south");
    boost(&mut b, 15, 3, 14, "ascending_south");
    flat(&mut b, 15, 4, 15, "north_west");
    // west along z 15
    for i in 1..=14 {
        boost(&mut b, 15 - i, 3 + i, 15, "ascending_west");
    }
    flat(&mut b, 0, 18, 15, "north_east");
    // north along x 0
    for i in 1..=14 {
        boost(&mut b, 0, 17 + i, 15 - i, "ascending_north");
    }
    flat(&mut b, 0, 32, 0, "south_east");
    // east along z 0: `east_slopes` boosters up (v3 14: the top at 46; v4 5, then flat boosters:
    // the top at 37)
    for i in 1..=east_slopes {
        boost(&mut b, i, 31 + i, 0, "ascending_east");
    }
    let top = 32 + east_slopes;
    for x in east_slopes + 1..=14 {
        boost(&mut b, x, top, 0, "east_west");
    }
    if past > 0 {
        // (v4: the top turns south into the tail chain's buffer and receiver: plain flats past it
        // along x 15, z 1..past, a stop after; the receiver's own cells are ironfarm's)
        flat(&mut b, 15, top, 0, "south_west");
        for z in 1..=past {
            flat(&mut b, 15, top, z, "north_south");
        }
        b.set(15, top, past + 1, "stone");
    } else {
        flat(&mut b, 15, top, 0, "east_west");
        b.set(16, top, 0, "stone");
    }
    // the batch at rest in its pockets, front first
    for (k, (x, y)) in [(13.885, -1.8225), (9.885, -0.8225), (5.885, 0.1775)].iter().enumerate() {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        b.entity("minecart", [*x, *y, SZ as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["The gravity queue's climb back: the batch at rest in today's pockets, the S3 departure to its crest, then three flat curves and 44 boosters up to the top (15,46,0)."])
}

/// One climb-back run's options.
#[derive(Clone, Debug)]
pub struct ClimbBackOpts {
    /// The stand's release, gt after T: G0 and G1 open (G0's slope powered at once, G1's 8 gt after),
    /// G2 `g2_delay` later (its slope 8 gt after it opens); G1 `g1_delay` after REL.
    pub rel: i64,
    pub g1_delay: i64,
    pub g2_delay: i64,
    /// Every gate's slope powered as its gate opens (the meter powers G1's and G2's 8 gt after).
    pub slopes_at_open: bool,
    /// G1's and G2's slopes powered this many gt after their gates open (the meter's 8).
    pub slope_delay: i64,
    pub ticks: i64,
    pub seed: i64,
    /// Feet height the riders must pass by T+100 (the golem zone's top + the lock: 24.5).
    pub above: f64,
    pub freeze: Option<(i64, i64)>,
    /// The top's rail height (v3 46; v4 37): the bench's sink.
    pub top_y: i32,
    /// v4: the flat cells past the top (the receiver's table at 4, 8 and 12 past it; the sink at the
    /// last); 0: the sink at the top.
    pub past: i32,
    /// The batch's carts: 3, or 2 (cart 2 leaves the world before the release).
    pub n_carts: usize,
    pub trace: bool,
}

/// One climb-back run's report lines: each cart's tick above `above` and at the top against T+100,
/// the curve rule (tick-exact's curve-pair margin at every curve: the follower's entry minus the
/// leader's clear, bar >= 0, and no touching), contacts, the riders.
pub fn climb_back_bench(spec: &FarmSpec, o: &ClimbBackOpts) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 3 {
        return Err(format!("{} carts", carts.len()));
    }
    let riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (g_open, g_shut, rs, stone) = (parse(GATE_OPEN)?, parse(GATE_SHUT)?, parse("redstone_block")?, parse("stone")?);
    for t in 0..400 {
        run.tick();
        if t >= 60 && run.world.time % 100 == 0 {
            break;
        }
    }
    let mut curves = CurveSpacing::default();
    curves.init(&run.world);
    // (a 2-cart batch: cart 2 and its rider leave the world before the release)
    if o.n_carts < 3 {
        let r2 = riders[2];
        for e in run.world.entities.list.iter_mut() {
            if e.id == carts[2] || Some(e.id) == r2 {
                e.removed = true;
            }
        }
    }
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    // (G2, G1, G0 west to east; their slopes' sources)
    let gate_at = [BlockPos::new(GATES[0].0, GATES[0].1, SZ), BlockPos::new(GATES[1].0, GATES[1].1, SZ), BlockPos::new(GATES[2].0, GATES[2].1, SZ)];
    let src = [BlockPos::new(6, -2, SZ), BlockPos::new(10, -3, SZ), BlockPos::new(14, -3, SZ + 1)];
    let opens = [o.rel + o.g2_delay, o.rel + o.g1_delay, o.rel];
    let powers = if o.slopes_at_open { opens } else { [opens[0] + o.slope_delay, opens[1] + o.slope_delay, opens[2]] };
    let mut above_at: [Option<i64>; 3] = [None; 3];
    let mut top_at: [Option<i64>; 3] = [None; 3];
    let mut top_v = [0.0f64; 3];
    let mut past_at: [[Option<(i64, f64)>; 3]; 3] = [[None; 3]; 3];
    let top_y = o.top_y;
    let mut crest_at: [Option<i64>; 3] = [None; 3];
    let mut prev_v = [0.0f64; 3];
    let mut touching = [[false; 3]; 3];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut hurt = 0usize;
    let mut health: [Option<f32>; 3] = [None; 3];
    let mut dismounts = 0usize;
    let mut eye_ticks = [0u32; 3];
    let mut off_rail: Vec<String> = Vec::new();
    let mut frozen_ticks = 0i64;
    let mut freeze_pos: Vec<[Option<Vec3>; 3]> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let chunks = [0, 0, 0, 0];
    for dt in 0..o.ticks {
        let frozen_now = o.freeze.is_some_and(|(f, l)| dt >= f && dt < f + l);
        let live = dt - frozen_ticks;
        // (the stand's release on the live clock, paused while frozen; the bench holds each gate
        // and slope source in its state every tick: a gate set open with no signal would shut at
        // its next neighbour update)
        {
            for g in 0..3 {
                let want_open = live >= opens[g];
                let want_gate = if want_open { g_open.clone() } else { g_shut.clone() };
                if run.world.get(gate_at[g]) != want_gate {
                    run.world.set_block(gate_at[g], want_gate, redsim::world::flags::ALL);
                }
                let want_src = if live >= powers[g] { rs.clone() } else { stone.clone() };
                if run.world.get(src[g]) != want_src {
                    run.world.set_block(src[g], want_src, redsim::world::flags::ALL);
                }
            }
        }
        if let Some((from, len)) = o.freeze {
            if frozen_now {
                frozen_ticks += 1;
            }
            if dt == from {
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
            if dt == from + len {
                redsim::reload::load_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
        }
        run.tick();
        curves.observe(&run.world, dt);
        // (the top is the bench's sink: a cart reaching it leaves the world with its rider, as the
        // tail chain's receiver takes it; its tick and speed are kept)
        let mut top_now: Vec<u32> = Vec::new();
        let w = &run.world;
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        for k in 0..3 {
            let Some((p, _)) = st[k] else {
                // (a cart the sink took, or the 2-cart batch's absent third, is not lost)
                let taken = top_at[k].is_some() || (k == 2 && o.n_carts < 3);
                if !taken && !off_rail.iter().any(|s| s.starts_with(&format!("cart {k} "))) {
                    off_rail.push(format!("cart {k} gone at T+{dt}"));
                }
                continue;
            };
            let feet = riders[k].and_then(|r| w.entities.list.iter().find(|e| e.id == r && !e.removed)).map_or(p.y, |e| e.pos.y);
            // (the lock applies to villagers: an empty cart's crossing doesn't count)
            if above_at[k].is_none() && feet >= o.above && riders[k].is_some() {
                above_at[k] = Some(dt);
            }
            if crest_at[k].is_none() && p.x > 15.0 && p.z > 12.0 && p.z < 13.0 && p.y > 1.9 {
                crest_at[k] = Some(dt);
            }
            if top_at[k].is_none() && p.y > top_y as f64 - 0.1 && p.x > 14.5 {
                top_at[k] = Some(dt);
                top_v[k] = st[k].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt()).max(prev_v[k]);
                if o.past == 0 {
                    top_now.push(carts[k]);
                }
            }
            // (past the top, v4: the entry into the cells 4, 8 and 12 past it, then the sink)
            if o.past > 0 && p.y > top_y as f64 - 0.1 && p.x > 14.5 {
                for (i, c) in [4.0, 8.0, 12.0].iter().enumerate() {
                    if past_at[k][i].is_none() && p.z + 0.49 >= *c {
                        past_at[k][i] = Some((dt, st[k].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt()).max(prev_v[k])));
                    }
                }
                if p.z >= o.past as f64 - 0.4 {
                    top_now.push(carts[k]);
                }
            }
            let cell = BlockPos::new(p.x.floor() as i32, (p.y + 0.01).floor() as i32, p.z.floor() as i32);
            let rail_here = |c: BlockPos| w.get(c).to_string().contains("rail");
            if !rail_here(cell) && !rail_here(BlockPos::new(cell.x, cell.y - 1, cell.z)) && !off_rail.iter().any(|s| s.starts_with(&format!("cart {k} "))) {
                off_rail.push(format!("cart {k} off the rails at T+{dt} ({:.2},{:.2},{:.2})", p.x, p.y, p.z));
            }
        }
        for a in 0..3 {
            for c in 0..3 {
                if a == c {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                let now = d < 1.25;
                // (not at the bench's end stop past the top: the tail chain's own gate takes them)
                if now && !touching[a][c] && sc < 0.05 && pa.y < 40.0 && pc.y < 40.0 {
                    let sa = prev_v[a].max((va.x * va.x + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: cart {a} into waiting cart {c} at {sa:.3}"));
                }
                touching[a][c] = now;
            }
        }
        for k in 0..3 {
            prev_v[k] = st[k].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt());
            let Some(r) = riders[k] else { continue };
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else { continue };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if health[k].is_some_and(|h0| hp < h0) {
                hurt += 1;
            }
            health[k] = Some(hp);
            if e.vehicle.is_none() {
                dismounts += 1;
            }
            let (ex, ey, ez) = (e.pos.x, e.pos.y + 1.62, e.pos.z);
            let bp = BlockPos::new(ex.floor() as i32, ey.floor() as i32, ez.floor() as i32);
            let s = w.get(bp);
            if !s.is_air() && redsim::entity::collide::boxes_at(w, &s, bp).iter().any(|bx| bx.min_x < ex && ex < bx.max_x && bx.min_y < ey && ey < bx.max_y && bx.min_z < ez && ez < bx.max_z) {
                eye_ticks[k] += 1;
            }
        }
        if o.trace {
            out.push(format!("T+{dt}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")));
        }
        // (the sink: the carts at the top and their riders leave)
        for id in top_now {
            let ps: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == id).map(|e| e.passengers.clone()).unwrap_or_default();
            for e in run.world.entities.list.iter_mut() {
                if e.id == id || ps.contains(&e.id) {
                    e.removed = true;
                }
            }
        }
    }
    let fmt = |a: &[Option<i64>; 3]| (0..3).map(|k| a[k].map_or("never".to_string(), |t| format!("T+{t}"))).collect::<Vec<_>>().join(", ");
    let last_above = (0..o.n_carts.min(3)).filter(|k| riders[*k].is_some()).map(|k| above_at[k].unwrap_or(i64::MAX)).max().unwrap_or(i64::MAX);
    out.push(format!(
        "arrival at the top (rail {top_y}), tick and stored speed: {}; spacing per pair (cart 1 - cart 0, cart 2 - cart 1): {}",
        (0..3).map(|k| top_at[k].map_or(format!("cart {k} never"), |t| format!("cart {k} T+{t} {:.3}", top_v[k]))).collect::<Vec<_>>().join(", "),
        match (top_at[0], top_at[1], top_at[2]) { (Some(a), Some(b), Some(c)) => format!("{} gt, {} gt", b - a, c - b), _ => "-".into() }
    ));
    if o.past > 0 {
        // (the receiver's table: each cart's entry tick after the stand's release edge (REL) and
        // its stored speed at the top cell and the cells 4, 8 and 12 past it)
        out.push(format!(
            "after REL (T+{}): {}",
            o.rel,
            (0..o.n_carts.min(3))
                .map(|k| format!(
                    "cart {k}{} top {} | +4 {} | +8 {} | +12 {}",
                    if riders[k].is_some() { "" } else { " (EMPTY)" },
                    top_at[k].map_or("-".to_string(), |t| format!("R+{} {:.3}", t - o.rel, top_v[k])),
                    past_at[k][0].map_or("-".to_string(), |(t, v)| format!("R+{} {v:.3}", t - o.rel)),
                    past_at[k][1].map_or("-".to_string(), |(t, v)| format!("R+{} {v:.3}", t - o.rel)),
                    past_at[k][2].map_or("-".to_string(), |(t, v)| format!("R+{} {v:.3}", t - o.rel))
                ))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    out.push(format!(
        "climb back (REL T+{}, G1 +{}, G2 +{}): above feet {} at {} (all by {}); crest {}; top {}",
        o.rel,
        o.g1_delay,
        o.g2_delay,
        o.above,
        fmt(&above_at),
        if last_above == i64::MAX { "NEVER".to_string() } else { format!("T+{last_above}, margin {} gt before T+100", 100 - last_above) },
        fmt(&crest_at),
        fmt(&top_at)
    ));
    let cr = curves.report();
    out.push(format!("curve rule (follower entry minus leader clear, bar >= 0; no touching): {}", if cr.is_empty() { "no curve passed by two carts".to_string() } else { cr.join("; ") }));
    out.push(format!(
        "contacts into a waiting cart: {} (the fastest {:.3}){}; off the rails: {}; riders: hurt {hurt}, unseated ticks {dismounts}, eye-in-block ticks {:?}{}",
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        if off_rail.is_empty() { "none".to_string() } else { off_rail.join("; ") },
        eye_ticks,
        if riders.iter().any(|r| r.is_none()) { format!(" (empty: {:?})", (0..3).filter(|k| riders[*k].is_none()).collect::<Vec<_>>()) } else { String::new() }
    ));
    if freeze_pos.len() == 2 {
        let f = o.freeze.unwrap_or((0, 0));
        out.push(format!("freeze T+{}..T+{} (level 32): {}", f.0, f.0 + f.1, (0..3).map(|k| match (freeze_pos[0][k], freeze_pos[1][k]) { (Some(a), Some(b)) => format!("cart {k} {:.2},{:.2},{:.2} -> {:.2},{:.2},{:.2}", a.x, a.y, a.z, b.x, b.y, b.z), _ => format!("cart {k} -") }).collect::<Vec<_>>().join("; ")));
    }
    Ok(out)
}

/// The gravity descent with the hold 9 levels lower (ironspec's inverted variant, pad below the
/// stand; until onechunk's v4 cells): Ga's exit flat and three flat boosters at rail height
/// 6 + `cells` (the hold's riders 9 levels under v3's), a straight descent of `cells` boosters down
/// north along x 3 into the committed crest (3,6,10), the one S3 file's approach ((3,5,9), (3,4,8),
/// (3,3,7), plain) to the snap turn and today's pockets.
pub fn design_gravity_c6(cells: i32, with_riders: [bool; 3]) -> String {
    let mut b = Builder::new("gravity-descent-c6-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    b.set(AX, 6, 10, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 5, 10, "glass");
    for (y, z) in [(5, 9), (4, 8), (3, 7)] {
        b.set(AX, y, z, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, y - 1, z, "glass");
    }
    for i in 0..cells {
        b.set(AX, 6 + i, 11 + i, "powered_rail[powered=true,shape=ascending_south,waterlogged=false]");
        b.set(AX, 5 + i, 11 + i, "redstone_block");
    }
    let (ty, tz) = (6 + cells, 11 + cells);
    for z in tz..tz + 3 {
        b.set(AX, ty, z, "powered_rail[powered=true,shape=north_south,waterlogged=false]");
        b.set(AX, ty - 1, z, "redstone_block");
    }
    b.set(AX, ty, tz + 3, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, ty - 1, tz + 3, "glass");
    b.set(AX, ty, tz + 4, "stone");
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["The gravity descent with the hold 9 levels lower: Ga's exit and three flat boosters, a straight boosted descent into the committed crest (3,6,10), the approach, today's 3 pockets (gates bench-driven)."])
}

/// The gravity descent on onechunk's skeleton v4 (the inverted variant, explore/farmopt-onechunk
/// 84f23d5d0; loop frame + (6,0,6) here): the hand-off at Ga's rail (13,15,5) heading south (the
/// hold's riders at feet 16.18 in the hold), a flat (13,15,6), six slopes down south (z 7..12,
/// rails 14..9), a flat (13,9,13), a flat curve (13,9,14) turning west, three slopes down west
/// (x 12..10, rails 8..6), flats west (x 9..4, rail 6), a flat curve (3,6,14) turning north, flats
/// north (3,6,13..11), the committed crest (3,6,10), the one S3 file's approach and today's
/// pockets. The skeleton lays plain rails; here every slope and flat is a booster on a redstone
/// block (`plain_flats`: the flats plain), the curves plain.
pub fn design_gravity_v4(with_riders: [bool; 3], plain_flats: bool) -> String {
    let mut b = Builder::new("gravity-descent-v4-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    b.set(AX, 6, 10, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 5, 10, "glass");
    for (y, z) in [(5, 9), (4, 8), (3, 7)] {
        b.set(AX, y, z, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, y - 1, z, "glass");
    }
    let cell = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str, plain: bool| {
        if plain {
            b.set(x, y, z, &format!("rail[shape={shape},waterlogged=false]"));
            b.set(x, y - 1, z, "glass");
        } else {
            b.set(x, y, z, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]"));
            b.set(x, y - 1, z, "redstone_block");
        }
    };
    // north into the crest: flats (3,6,13..11)
    for z in 11..=13 {
        cell(&mut b, AX, 6, z, "north_south", plain_flats);
    }
    b.set(AX, 6, 14, "rail[shape=north_east,waterlogged=false]");
    b.set(AX, 5, 14, "glass");
    // west along z 14: flats x 4..9 (rail 6), slopes x 10..12 (rails 6..8)
    for x in 4..=9 {
        cell(&mut b, x, 6, 14, "east_west", plain_flats);
    }
    for (i, x) in (10..=12).enumerate() {
        cell(&mut b, x, 6 + i as i32, 14, "ascending_east", false);
    }
    b.set(13, 9, 14, "rail[shape=north_west,waterlogged=false]");
    b.set(13, 8, 14, "glass");
    // south along x 13: the flat (13,9,13), slopes z 7..12 (rails 14..9), the flat (13,15,6)
    cell(&mut b, 13, 9, 13, "north_south", plain_flats);
    for (i, z) in (7..=12).enumerate() {
        cell(&mut b, 13, 14 - i as i32, z, "ascending_north", false);
    }
    cell(&mut b, 13, 15, 6, "north_south", plain_flats);
    // Ga's rail (the hand-off) and a stop north of it
    b.set(13, 15, 5, "rail[shape=north_south,waterlogged=false]");
    b.set(13, 14, 5, "glass");
    b.set(13, 15, 4, "stone");
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["The gravity descent on skeleton v4 (inverted): Ga's rail, south down x 13, a curve, west along z 14, a curve, north into the committed crest (3,6,10), the approach, today's 3 pockets (gates bench-driven)."])
}

/// The gravity descent re-laid as continuous slopes (farmopt 18:14: 9fc3155f's hold moves Ga to
/// loop (7,16,4) and the descent starts at (7,15,5); onechunk's cells to follow — until then this
/// is my emulation, loop frame + (6,0,6) here): Ga's rail (13,15,10) the hand-off heading south,
/// four slopes down south (z 11..14, rails 14..11), a flat curve (13,11,15) turning west, five slopes
/// down west (x 12..8, rails 10..6) and flats (x 7..4, rail 6), a flat curve (3,6,15) turning north,
/// flats north (3,6,14..11), the committed crest (3,6,10), the one S3 file's approach and today's
/// pockets. Every slope and flat a booster on a redstone block, the curves plain.
pub fn design_gravity_v5(with_riders: [bool; 3]) -> String {
    let mut b = Builder::new("gravity-descent-v5-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    b.set(AX, 6, 10, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 5, 10, "glass");
    for (y, z) in [(5, 9), (4, 8), (3, 7)] {
        b.set(AX, y, z, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, y - 1, z, "glass");
    }
    let boost = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str| {
        b.set(x, y, z, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]"));
        b.set(x, y - 1, z, "redstone_block");
    };
    for z in 11..=14 {
        boost(&mut b, AX, 6, z, "north_south");
    }
    b.set(AX, 6, 15, "rail[shape=north_east,waterlogged=false]");
    b.set(AX, 5, 15, "glass");
    for x in 4..=7 {
        boost(&mut b, x, 6, 15, "east_west");
    }
    for (i, x) in (8..=12).enumerate() {
        boost(&mut b, x, 6 + i as i32, 15, "ascending_east");
    }
    b.set(13, 11, 15, "rail[shape=north_west,waterlogged=false]");
    b.set(13, 10, 15, "glass");
    for (i, z) in (11..=14).enumerate() {
        boost(&mut b, 13, 14 - i as i32, z, "ascending_north");
    }
    b.set(13, 15, 10, "rail[shape=north_south,waterlogged=false]");
    b.set(13, 14, 10, "glass");
    b.set(13, 15, 9, "stone");
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["The gravity descent re-laid as continuous slopes (emulated until onechunk's cells): Ga's rail (13,15,10), four slopes south, a curve, five slopes and flats west, a curve, flats north into the committed crest (3,6,10), the approach, today's 3 pockets."])
}

/// The gravity descent re-laid by onechunk (explore/farmopt-onechunk 635596597, skeleton v4
/// inverted; loop frame + (6,0,6) here; 13 cells): G_a (13,15,10), the hold's last gate slope (plain,
/// the hand-off), a slope down south (13,14,11), a flat curve (13,14,12) turning west, eight slopes
/// down west (x 12..5, rails 13..6), a flat (4,6,12), a flat curve (3,6,12) turning north, a flat
/// (3,6,11), the committed crest (3,6,10), the one S3 file's approach and today's pockets. The
/// skeleton lays plain rails on glass; here every slope and flat is a booster on a redstone block,
/// the curves and G_a plain.
pub fn design_gravity_v4r(with_riders: [bool; 3]) -> String {
    let mut b = Builder::new("gravity-descent-v4r-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    b.set(AX, 6, 10, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 5, 10, "glass");
    for (y, z) in [(5, 9), (4, 8), (3, 7)] {
        b.set(AX, y, z, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, y - 1, z, "glass");
    }
    let boost = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str| {
        b.set(x, y, z, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]"));
        b.set(x, y - 1, z, "redstone_block");
    };
    boost(&mut b, AX, 6, 11, "north_south");
    b.set(AX, 6, 12, "rail[shape=north_east,waterlogged=false]");
    b.set(AX, 5, 12, "glass");
    boost(&mut b, 4, 6, 12, "east_west");
    for (i, x) in (5..=12).enumerate() {
        boost(&mut b, x, 6 + i as i32, 12, "ascending_east");
    }
    b.set(13, 14, 12, "rail[shape=north_west,waterlogged=false]");
    b.set(13, 13, 12, "glass");
    // Ds (onechunk 0633be8d9): a sloped detector rail on glass, the first cell after G_a (no boost,
    // no brake)
    b.set(13, 14, 11, "detector_rail[powered=false,shape=ascending_north,waterlogged=false]");
    b.set(13, 13, 11, "glass");
    // G_a: the hold's last gate slope (plain), the hand-off; a stop north of it
    b.set(13, 15, 10, "rail[shape=ascending_north,waterlogged=false]");
    b.set(13, 14, 10, "glass");
    b.set(13, 16, 9, "stone");
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["The gravity descent re-laid by onechunk (13 cells): G_a, a slope south, a curve, eight slopes west, a flat, a curve, a flat into the committed crest (3,6,10), the approach, today's 3 pockets (gates bench-driven)."])
}

/// The bench's design: H's queue (3 slopes down north), H's slope and gate, the climb, T, the
/// approach's two slopes down, the snap turn, the strip and its gates; a marker block in the chunk
/// west of the bench (the forced box's spare chunk, so a freeze leaves the bench at level 32).
pub fn design(cells: i32, with_riders: [bool; 3]) -> String {
    design_with(cells, 5, "", with_riders, false)
}

/// The bench's design with a climb pattern (one letter per booster cell from the foot: `B` a
/// booster on a redstone block, `P` a plain slope on glass; empty: all boosters) and, with
/// `spaced`, the 2nd and 3rd carts parked off the track (x -6..-4, the spare chunk) for the bench
/// to set at H one at a time (the per-gate hold's release emulated: a cart at rest at Ga every S gt).
pub fn design_with(cells: i32, crest: i32, pattern: &str, with_riders: [bool; 3], spaced: bool) -> String {
    let mut b = Builder::new("climb-bench");
    for (x, y, s) in STRIP {
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    // the gates: G2 and G1 open (their slopes powered from their sources, as the meter powers an
    // open gate's slope 8 gt after it opens), G0 shut (its source, beside its slope, stone)
    b.set(GATES[0].0, GATES[0].1, SZ, GATE_OPEN);
    b.set(GATES[1].0, GATES[1].1, SZ, GATE_OPEN);
    b.set(GATES[2].0, GATES[2].1, SZ, GATE_SHUT);
    b.set(6, -2, SZ, "redstone_block");
    b.set(10, -3, SZ, "redstone_block");
    b.set(14, -3, SZ + 1, "stone");
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    // T and the approach's slopes down to the snap turn (today's crest 5: two)
    let zt = crest_z(crest);
    b.set(AX, crest, zt, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, crest - 1, zt, "glass");
    for k in 1..=(crest - 3) {
        b.set(AX, crest - k, zt - k, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, crest - k - 1, zt - k, "glass");
    }
    // the climb: `cells` boosters ascending north on redstone blocks, the top one rising to T
    let (fy, fz) = foot(cells, crest);
    for k in 0..cells {
        let plain = pattern.chars().nth(k as usize) == Some('P');
        if plain {
            b.set(AX, fy + k, fz - k, "rail[shape=ascending_north,waterlogged=false]");
            b.set(AX, fy + k - 1, fz - k, "glass");
        } else {
            b.set(AX, fy + k, fz - k, "powered_rail[powered=true,shape=ascending_north,waterlogged=false]");
            b.set(AX, fy + k - 1, fz - k, "redstone_block");
        }
    }
    // H: a booster slope down north into the climb's foot (a cart let go from rest gets going at
    // once), its gate over the high end; the queue: three slopes down north and a flat, then a stop
    b.set(AX, fy, fz + 1, "powered_rail[powered=true,shape=ascending_south,waterlogged=false]");
    b.set(AX, fy - 1, fz + 1, "redstone_block");
    b.set(AX, fy + 1, fz + 1, H_SHUT);
    for k in 0..3 {
        b.set(AX, fy + 1 + k, fz + 2 + k, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, fy + k, fz + 2 + k, "glass");
    }
    b.set(AX, fy + 4, fz + 5, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, fy + 3, fz + 5, "glass");
    b.set(AX, fy + 4, fz + 6, "stone");
    // the 3 carts on the queue's slopes, front first
    for k in 0..3usize {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        if spaced && k > 0 {
            // (parked: a flat rail on glass in the spare chunk, far from the bench)
            b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
            b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
            b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
        } else {
            b.entity("minecart", [AX as f64 + 0.5, (fy + 1 + ki) as f64 + 0.6, (fz + 2 + ki) as f64 + 0.5], Some(nbt));
        }
    }
    // the spare chunk's marker (x -8: chunk -1)
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["One-chunk (b): the climb bench: H's queue, a straight powered climb to today's approach top, the snap turn and today's 3 pockets (gates bench-driven)."])
}

/// One climb run's options.
#[derive(Clone, Debug)]
pub struct ClimbOpts {
    pub cells: i32,
    /// T's height (today's 5; 3 the lowest with today's snap turn).
    pub crest: i32,
    /// H opens at T + this (gt).
    pub h_open: i64,
    /// H shuts again this many gt after it opened (0: never).
    pub h_for: i64,
    /// The per-gate hold's release emulated (with a `spaced` design): cart k set at rest at Ga
    /// (cart 0's rest spot against H at T) at T + h_open + k * h_every, H open from T + h_open.
    pub h_every: i64,
    /// A gate shuts once a leaving cart's centre is this far past its cell's west edge.
    pub close_at: f64,
    /// The run's length after T (gt).
    pub ticks: i64,
    pub seed: i64,
    /// The golem's box (design frame, x0,y0,z0,x1,y1,z1) while a golem exists (T..T+2).
    pub golem_box: Option<[f64; 6]>,
    /// The freeze: the bench's chunks at level 32 from T + FROM for LEN gt.
    pub freeze: Option<(i64, i64)>,
    /// The hand-off (design_route, design_folded, design_gravity): cart k set at this point with
    /// this velocity at T + h_open + k * spacing; the freeze's chunk box.
    pub hand_off: Option<(Vec3, Vec3, i64, [i32; 4])>,
    /// Cart 2's hand-off this many gt after cart 1's (default: the same spacing as cart 1's).
    pub hand_off_2: Option<i64>,
    /// The stand's gates on timers from the hold's release (T + h_open) instead of position
    /// triggers: G2 shuts at release + .0, G1 at release + .1 (live time: paused while frozen).
    pub gate_shut: Option<(i64, i64)>,
    /// (7b90c3f1) The stand's gates on D1's real powered rising edge (P1's detector rail, crossed
    /// only by the batch's front cart) plus lockable delays: G1 at the edge + .0, G2 at the edge + .1
    /// (live time: paused while frozen).
    pub gate_shut_d1: Option<(i64, i64)>,
    /// (7b90c3f1) A fault row: cart K never handed off (removed at T).
    pub missing: Option<usize>,
    /// The batch's carts (3, or 2: the third stays parked).
    pub n_carts: usize,
    pub trace: bool,
}

/// One climb run's report lines.
pub fn bench(spec: &FarmSpec, o: &ClimbOpts) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 3 {
        return Err(format!("{} carts", carts.len()));
    }
    // (front first: the spawn order is the queue's, front first)
    let riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (g_shut, g_open, h_shut, h_open) = (parse(GATE_SHUT)?, parse(GATE_OPEN)?, parse(H_SHUT)?, parse(H_OPEN)?);
    let stone = parse("stone")?;
    let (fy, fz) = foot(o.cells, o.crest);
    let zt = crest_z(o.crest);
    let h_cell = BlockPos::new(AX, fy + 1, fz + 1);
    // the warm-up: the carts settle against H; T: the first world time that is a multiple of 100,
    // at least 60 gt on
    let mut t0 = 0i64;
    for t in 0..400 {
        run.tick();
        if t >= 60 && run.world.time % 100 == 0 {
            t0 = t;
            break;
        }
    }
    let mut out: Vec<String> = Vec::new();
    // (the per-gate hold emulated: cart 0's rest spot against H at T, where carts 1 and 2 are set
    // at rest later)
    let ga_spot: Option<Vec3> = run.world.entities.list.iter().find(|e| e.id == carts[0]).map(|e| e.pos);
    let place_at = |w: &mut redsim::world::World, id: u32, p: Vec3| {
        let Some(i) = w.entities.list.iter().position(|e| e.id == id) else { return };
        w.entities.set_pos(i, p);
        w.entities.list[i].delta_movement = Vec3::new(0.0, 0.0, 0.0);
        let ps = w.entities.list[i].passengers.clone();
        for r in ps {
            if let Some(j) = w.entities.list.iter().position(|e| e.id == r) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    };
    let mut open = [true, true, false];
    let mut seen = [[false; 3]; 3];
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    let mut rest_since: [Option<i64>; 3] = [None; 3];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut touching = [[false; 3]; 3];
    let mut prev_v = [0.0f64; 3];
    let mut eye_ticks = [0u32; 3];
    let mut hurt: Vec<String> = Vec::new();
    let mut health: [Option<f32>; 3] = [None; 3];
    let mut dismount: Vec<String> = Vec::new();
    let mut golem_gap = [f64::INFINITY; 3];
    let mut top_v = [0.0f64; 3];
    let mut crest_at: [Option<i64>; 3] = [None; 3];
    // (the least distance between each cart and the one it follows, front cart k and back cart
    // k+1, while the back one is still on the approach (off the strip): a catch is under 1.0)
    let mut least_gap = [f64::INFINITY; 2];
    let h_feet: Option<f64> = riders[0].and_then(|r| run.world.entities.list.iter().find(|e| e.id == r).map(|e| e.pos.y));
    let mut froze: Vec<String> = Vec::new();
    let mut eye_where: Vec<String> = Vec::new();
    let mut freeze_pos: Vec<[Option<Vec3>; 3]> = Vec::new();
    let mut frozen_ticks = 0i64;
    let mut handed = [false; 3];
    // (7b90c3f1: the fault row: the missing cart removed at T and never handed off; D1's rising
    // edges (P1's detector rail) and the first one's live tick; each stand gate's shut tick)
    if let Some(k) = o.missing
        && k < 3
    {
        let riders_k: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == carts[k]).map(|e| e.passengers.clone()).unwrap_or_default();
        for r in riders_k {
            let _ = redsim::entity::kill(&mut run.world, r);
        }
        let _ = redsim::entity::kill(&mut run.world, carts[k]);
        handed[k] = true;
    }
    let d1_cell = BlockPos::new(12, -1, SZ);
    let mut d1_was = matches!(run.world.get(d1_cell), Block::DetectorRail { powered: true, .. });
    let mut d1_edges: Vec<i64> = Vec::new();
    let mut d1_first: Option<i64> = None;
    let mut shut_dt: [Option<i64>; 3] = [None; 3];
    let mut gate_edges: Vec<String> = Vec::new();
    // (each cart's entry into each gate's cell, G2, G1, G0: the tick and its stored speed)
    let mut gate_entry: [[Option<(i64, f64)>; 3]; 3] = [[None; 3]; 3];
    let mut merges: Vec<String> = Vec::new();
    let chunks = match o.hand_off {
        Some((_, _, _, c)) => c,
        None => {
            let xs: Vec<i32> = [2i32, 16].iter().map(|x| x >> 4).collect();
            let zs: Vec<i32> = [SZ - 1, fz + 6].iter().map(|z| z >> 4).collect();
            [xs[0], zs[0], xs[1].max(xs[0]), zs[1].max(zs[0])]
        }
    };
    for dt in 0..o.ticks {
        // the hand-off: cart k at the hand-off point with the hand-off velocity, in live time
        // (the hold's release waits out a freeze: its clock paused, nothing handed off while the
        // column is frozen)
        let frozen_now = o.freeze.is_some_and(|(f, l)| dt >= f && dt < f + l);
        if let Some((p, v, sp, _)) = o.hand_off {
            let live = dt - frozen_ticks;
            for k in 0..o.n_carts.min(3) {
                let at = match k {
                    0 => o.h_open,
                    1 => o.h_open + sp,
                    _ => o.h_open + sp + o.hand_off_2.unwrap_or(sp),
                };
                if !frozen_now && !handed[k] && live >= at {
                    handed[k] = true;
                    place_at(&mut run.world, carts[k], p);
                    if let Some(i) = run.world.entities.list.iter().position(|e| e.id == carts[k]) {
                        run.world.entities.list[i].delta_movement = v;
                    }
                }
            }
        }
        // H: opened at T + h_open, shut again after h_for
        if dt == o.h_open && o.hand_off.is_none() {
            run.world.set_block(h_cell, h_open.clone(), redsim::world::flags::ALL);
        }
        if o.h_for > 0 && dt == o.h_open + o.h_for {
            run.world.set_block(h_cell, h_shut.clone(), redsim::world::flags::ALL);
        }
        // the per-gate hold's next cart at rest at Ga every h_every gt
        if o.h_every > 0
            && let Some(p) = ga_spot
        {
            for k in 1..3i64 {
                if dt == o.h_open + k * o.h_every {
                    place_at(&mut run.world, carts[k as usize], p);
                }
            }
        }
        // the freeze: the bench's chunks unforced (level 32: blocks tick, entities don't)
        if let Some((from, len)) = o.freeze {
            if dt >= from && dt < from + len {
                frozen_ticks += 1;
            }
            if dt == from {
                // (a bench world has no forced chunk box: the chunks its box meets, the marker's
                // spare chunk west of the bench among them)
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
            if dt == from + len {
                redsim::reload::load_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
        }
        // the pockets' gates: on timers from the release (live time, paused while frozen), or
        // bench position triggers
        if let Some((a, b)) = o.gate_shut {
            let live = dt - frozen_ticks;
            open[0] = live < o.h_open + a;
            open[1] = live < o.h_open + b;
        }
        // (7b90c3f1: D1-keyed shuts: D1's real powered state; its first rising edge starts both
        // lockable delays (live time, paused while frozen))
        {
            let d1_on = matches!(run.world.get(d1_cell), Block::DetectorRail { powered: true, .. });
            let live = dt - frozen_ticks;
            if d1_on && !d1_was {
                d1_edges.push(dt);
                if d1_first.is_none() {
                    d1_first = Some(live);
                }
            }
            d1_was = d1_on;
            if let Some((a1, a2)) = o.gate_shut_d1 {
                open[1] = !d1_first.is_some_and(|e| live >= e + a1);
                open[0] = !d1_first.is_some_and(|e| live >= e + a2);
            }
        }
        for i in 0..3 {
            if !open[i] && shut_dt[i].is_none() && i < 2 {
                shut_dt[i] = Some(dt);
            }
        }
        for (k, (gx, _)) in GATES.iter().rev().enumerate() {
            if k == 0 || !open[2 - k] || o.gate_shut.is_some() || o.gate_shut_d1.is_some() {
                continue;
            }
            let edge = *gx as f64 + o.close_at;
            for (n, id) in carts.iter().enumerate() {
                if let Some((p, _)) = pos(&run.world, *id)
                    && (p.z - (SZ as f64 + 0.5)).abs() < 0.6
                    && p.y < 3.0
                    && p.x >= edge
                    && !seen[k][n]
                {
                    seen[k][n] = true;
                    if seen[k].iter().filter(|b| **b).count() >= k {
                        open[2 - k] = false;
                    }
                }
            }
        }
        for (i, (gx, gy)) in GATES.iter().enumerate() {
            let want = if open[i] { g_open.clone() } else { g_shut.clone() };
            let p = BlockPos::new(*gx, *gy, SZ);
            if run.world.get(p) != want {
                run.world.set_block(p, want, redsim::world::flags::ALL);
                // (a shut gate's slope unpowered: its source back to stone)
                if !open[i] && i < 2 {
                    let src = if i == 0 { BlockPos::new(6, -2, SZ) } else { BlockPos::new(10, -3, SZ) };
                    run.world.set_block(src, stone.clone(), redsim::world::flags::ALL);
                    // (a gate edge on a cart: a cart's box meeting the gate's cell as it shuts)
                    for (n, id) in carts.iter().enumerate() {
                        if let Some((cp, _)) = pos(&run.world, *id)
                            && cp.x + 0.49 > *gx as f64
                            && cp.x - 0.49 < *gx as f64 + 1.0
                            && cp.y + 0.7 > *gy as f64
                            && cp.y < *gy as f64 + 1.5
                            && (cp.z - (SZ as f64 + 0.5)).abs() < 0.99
                        {
                            gate_edges.push(format!("T+{dt}: {} shut on cart {n} at {:.2},{:.2},{:.2}", ["G2", "G1", "G0"][i], cp.x, cp.y, cp.z));
                        }
                    }
                }
            }
        }
        run.tick();
        let w = &run.world;
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        for k in 0..3 {
            let Some((p, v)) = st[k] else { continue };
            let s = (v.x * v.x + v.z * v.z).sqrt();
            top_v[k] = top_v[k].max(s);
            if (p.z - (SZ as f64 + 0.5)).abs() < 0.6 && p.y < 3.0 {
                for (g, (gx, _)) in GATES.iter().enumerate() {
                    if gate_entry[k][g].is_none() && p.x + 0.49 >= *gx as f64 && p.x < *gx as f64 + 1.0 {
                        gate_entry[k][g] = Some((dt, prev_v[k].max(s)));
                    }
                }
            }
            if crest_at[k].is_none() && (p.x - (AX as f64 + 0.5)).abs() < 0.6 && p.z < zt as f64 + 0.5 && p.y > o.crest as f64 - 0.1 {
                crest_at[k] = Some(dt);
            }
            if s < 1e-3 && (p.z - (SZ as f64 + 0.5)).abs() < 0.6 && p.y < 3.0 {
                rest_since[k].get_or_insert(dt);
            } else {
                rest_since[k] = None;
            }
        }
        for k in 0..2 {
            if let (Some((pa, _)), Some((pb, _))) = (st[k], st[k + 1])
                && ((pb.z - (SZ as f64 + 0.5)).abs() > 0.6 || pb.y >= 3.0)
                && (o.hand_off.is_some() || (pb.x < AX as f64 + 1.0 && pb.z < fz as f64 + 1.0))
            {
                let d = ((pa.x - pb.x).powi(2) + (pa.y - pb.y).powi(2) + (pa.z - pb.z).powi(2)).sqrt();
                least_gap[k] = least_gap[k].min(d);
            }
        }
        // arrivals into a waiting cart: two carts' centres within 1.0 (their boxes touching), the
        // slower at under 0.05: the other's stored speed the tick before
        for a in 0..3 {
            for c in 0..3 {
                if a == c {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                // (the boxes, 0.98 wide and 0.7 tall, touching: a slope puts the two centres up
                // to 1 block apart in y as well, so the centres within 1.25)
                let now = d < 1.25;
                if now && !touching[a][c] && sc < 0.05 {
                    let sa = prev_v[a].max((va.x * va.x + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: cart {a} into waiting cart {c} at {sa:.3}"));
                }
                if a < c && d < 0.6 && !merges.iter().any(|m| m.contains(&format!("carts {a} and {c}"))) {
                    merges.push(format!("T+{dt}: carts {a} and {c} {d:.2} apart at {:.2},{:.2},{:.2}", pa.x, pa.y, pa.z));
                }
                touching[a][c] = now;
            }
        }
        for k in 0..3 {
            prev_v[k] = st[k].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt());
        }
        // the riders: damage, dismounts, the eye point inside a collision shape, the golem box gap
        for k in 0..3 {
            let Some(r) = riders[k] else { continue };
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else {
                dismount.push(format!("T+{dt}: cart {k}'s rider gone"));
                continue;
            };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if let Some(h0) = health[k]
                && hp < h0
            {
                hurt.push(format!("T+{dt}: cart {k}'s rider {h0} -> {hp}"));
            }
            health[k] = Some(hp);
            if e.vehicle.is_none() && !dismount.iter().any(|d| d.contains(&format!("cart {k}'s"))) {
                dismount.push(format!("T+{dt}: cart {k}'s rider not seated"));
            }
            let (ex, ey, ez) = (e.pos.x, e.pos.y + 1.62, e.pos.z);
            let mut blind = false;
            for y in ey.floor() as i32 - 1..=ey.floor() as i32 + 1 {
                for z in ez.floor() as i32 - 1..=ez.floor() as i32 + 1 {
                    for x in ex.floor() as i32 - 1..=ex.floor() as i32 + 1 {
                        let bp = BlockPos::new(x, y, z);
                        let s = w.get(bp);
                        if s.is_air() {
                            continue;
                        }
                        for bx in redsim::entity::collide::boxes_at(w, &s, bp) {
                            if bx.min_x < ex && ex < bx.max_x && bx.min_y < ey && ey < bx.max_y && bx.min_z < ez && ez < bx.max_z {
                                blind = true;
                            }
                        }
                    }
                }
            }
            if blind {
                eye_ticks[k] += 1;
                if eye_where.len() < 4 {
                    eye_where.push(format!("T+{dt} cart {k} eye {ex:.2},{ey:.2},{ez:.2}"));
                }
            }
            if let Some(g) = o.golem_box
                && dt <= 2
            {
                // (the villager's box: 0.6 wide, 1.95 tall; the gap along the axis that separates
                // the boxes most)
                let vb = [e.pos.x - 0.3, e.pos.y, e.pos.z - 0.3, e.pos.x + 0.3, e.pos.y + 1.95, e.pos.z + 0.3];
                let gap = (0..3).map(|i| (g[i] - vb[i + 3]).max(vb[i] - g[i + 3]).max(0.0)).fold(0.0, f64::max);
                golem_gap[k] = golem_gap[k].min(gap);
            }
        }
        if o.trace {
            out.push(format!("T+{dt}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")));
        }
    }
    // the freeze's effect: each cart's move over the frozen span
    if freeze_pos.len() == 2 {
        for k in 0..3 {
            if let (Some(a), Some(b)) = (freeze_pos[0][k], freeze_pos[1][k]) {
                froze.push(format!("cart {k} {:.2},{:.2},{:.2} -> {:.2},{:.2},{:.2}", a.x, a.y, a.z, b.x, b.y, b.z));
            }
        }
    }
    let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(&run.world, *id)).collect();
    let rest: Vec<String> = (0..o.n_carts.min(3))
        .map(|k| {
            let p = st[k].map_or("gone".to_string(), |(p, _)| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z));
            match rest_since[k] {
                Some(r) => format!("cart {k}{} at rest from T+{r} (margin {} gt before T+100) at {p}, crest T+{}, top speed {:.3}", if riders[k].is_some() { "" } else { " (EMPTY)" }, 100 - r, crest_at[k].map_or("-".into(), |c| c.to_string()), top_v[k]),
                None => format!("cart {k}{} NOT at rest by T+{} ({p}), crest T+{}", if riders[k].is_some() { "" } else { " (EMPTY)" }, o.ticks, crest_at[k].map_or("-".into(), |c| c.to_string())),
            }
        })
        .collect();
    out.push(format!(
        "climb bench (H opens at T+{}{}; {} cells, crest {}; H's rider feet at y {}): {}",
        o.h_open,
        if o.h_every > 0 { format!(", a cart every {} gt", o.h_every) } else { String::new() },
        o.cells,
        o.crest,
        h_feet.map_or("-".into(), |y| format!("{y:.2}")),
        rest.join("; ")
    ));
    out.push(format!(
        "the least distance to the cart ahead before the strip: cart 1 {}, cart 2 {} (a catch under 1.0); crest gaps {}",
        if least_gap[0].is_finite() { format!("{:.2}", least_gap[0]) } else { "-".into() },
        if least_gap[1].is_finite() { format!("{:.2}", least_gap[1]) } else { "-".into() },
        match (crest_at[0], crest_at[1], crest_at[2]) { (Some(a), Some(b), Some(c)) => format!("{}, {} gt", b - a, c - b), _ => "-".into() }
    ));
    let n = o.n_carts.min(3);
    // (each cart in its own pocket: cart 0 at P1's rest spot, cart 1 at P2's, cart 2 at P3's)
    let wrong: Vec<String> = (0..n)
        .filter_map(|k| {
            let want = [13.885, 9.885, 5.885][k];
            match st[k] {
                Some((p, _)) if (p.x - want).abs() < 0.3 && (p.z - (SZ as f64 + 0.5)).abs() < 0.3 => None,
                Some((p, _)) => Some(format!("cart {k} at {:.2},{:.2},{:.2} (P{} expected)", p.x, p.y, p.z, k + 1)),
                None => Some(format!("cart {k} gone")),
            }
        })
        .collect();
    out.push(format!("pockets: {}", if wrong.is_empty() { "every cart at its own pocket's rest spot".to_string() } else { format!("WRONG POCKET: {}", wrong.join("; ")) }));
    let last = rest_since.iter().take(n).map(|r| r.unwrap_or(i64::MAX)).max().unwrap_or(i64::MAX);
    out.push(format!(
        "batch at rest: {}; arrivals into a waiting cart: {} (the fastest {:.3}; bar 0.2){}",
        if last == i64::MAX { "NO".to_string() } else { format!("T+{last}, margin {} gt before T+100", 100 - last) },
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    // (7b90c3f1: the D1-keyed shuts' report: per stand gate (G1, G2) its shut tick, the leader gap
    // (from the last cart entering the gate's cell before the shut) and the follower margin (to the
    // first cart entering it after); D1's rising edges (the bar: 1 a cycle))
    if o.gate_shut_d1.is_some() || o.gate_shut.is_some() {
        let per_gate = |g: usize| -> String {
            let Some(s) = shut_dt[g] else { return format!("{} never shut", ["G2", "G1"][g]) };
            let lead = (0..3).filter_map(|k| gate_entry[k][g].map(|(t, _)| (t, k))).filter(|(t, _)| *t <= s).max();
            let foll = (0..3).filter_map(|k| gate_entry[k][g].map(|(t, _)| (t, k))).filter(|(t, _)| *t > s).min();
            format!(
                "{} shut T+{s}: leader gap {}, follower margin {}",
                ["G2", "G1"][g],
                lead.map_or("-".to_string(), |(t, k)| format!("{} (cart {k})", s - t)),
                foll.map_or("-".to_string(), |(t, k)| format!("{} (cart {k})", t - s))
            )
        };
        out.push(format!(
            "stand shuts{}: D1 rising edges {} ({}); {}; {}",
            o.gate_shut_d1.map_or(String::new(), |(a, b)| format!(" on D1's edge + {a} (G1) / + {b} (G2)")),
            d1_edges.len(),
            d1_edges.iter().map(|t| format!("T+{t}")).collect::<Vec<_>>().join(", "),
            per_gate(1),
            per_gate(0)
        ));
    }
    out.push(format!(
        "entry into each gate's cell (G2 x 6, G1 x 10, G0 x 14), tick and stored speed: {}; gate edges on a cart: {}{}; merges: {}{}",
        (0..n)
            .map(|k| format!(
                "cart {k}{} {}",
                if riders[k].is_some() { "" } else { " (EMPTY)" },
                (0..3).map(|g| gate_entry[k][g].map_or("-".to_string(), |(t, v)| format!("{} T+{t} {v:.3}", ["G2", "G1", "G0"][g]))).collect::<Vec<_>>().join(", ")
            ))
            .collect::<Vec<_>>()
            .join("; "),
        gate_edges.len(),
        if gate_edges.is_empty() { String::new() } else { format!(" ({})", gate_edges.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        merges.len(),
        if merges.is_empty() { String::new() } else { format!(" ({})", merges.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    out.push(format!(
        "riders: hurt {}{}; dismounts {}{}; eye point inside a collision shape (ticks per cart) {:?}{}{}",
        hurt.len(),
        if hurt.is_empty() { String::new() } else { format!(" ({})", hurt.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        dismount.len(),
        if dismount.is_empty() { String::new() } else { format!(" ({})", dismount.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        eye_ticks,
        if eye_where.is_empty() { String::new() } else { format!(" ({})", eye_where.join("; ")) },
        if o.golem_box.is_some() { format!("; the least box gap to the golem box while it exists (T..T+2), per rider: {}", golem_gap.iter().map(|g| if g.is_finite() { format!("{g:.2}") } else { "-".into() }).collect::<Vec<_>>().join(", ")) } else { String::new() }
    ));
    if let Some((from, len)) = o.freeze {
        out.push(format!("freeze T+{from}..T+{} (the bench's chunks {:?} at level 32; the world's errors: {}): {}", from + len, chunks, if run.world.errors.is_empty() { "none".to_string() } else { run.world.errors.join("; ") }, froze.join("; ")));
    }
    let _ = t0;
    Ok(out)
}

/// The descent bench's design (ironspec 2026-11-11, one-chunk (b) 3): today's 3 pockets with the
/// batch at rest in them (the S3 file's rest positions), G0's corner south, a straight descent of
/// `pattern.len()` slopes down south along x 15 (each `B` a booster on a redstone block, `P` a
/// plain slope on glass), then R1's entry: a brake (an unpowered powered rail, flat), `flats` plain
/// flats and a shut fence gate (the hold's stop).
pub fn descent_design(pattern: &str, flats: i32, with_riders: [bool; 3]) -> String {
    let mut b = Builder::new("descent-bench");
    for (x, y, s) in STRIP.iter().take(12) {
        b.set(*x, *y, SZ, s);
        b.set(*x, *y - 1, SZ, "glass");
    }
    for (x, y) in GATES {
        b.set(x, y, SZ, GATE_SHUT);
    }
    // (the gate slopes' power cells, stone at rest (the meter's sources): G0's beside its slope,
    // G1's and G2's under theirs)
    for (x, y, z) in [(14, -3, 7), (10, -3, 6), (6, -2, 6)] {
        b.set(x, y, z, "stone");
    }
    b.set(AX - 1, 2, SZ, "glass");
    b.set(AX - 1, 3, SZ, "glass");
    // G0's corner, then the descent south along x 15
    b.set(15, -3, SZ, "rail[shape=south_west,waterlogged=false]");
    b.set(15, -4, SZ, "glass");
    let n = pattern.len() as i32;
    for (k, ch) in pattern.chars().enumerate() {
        let k = k as i32;
        let (y, z) = (-4 - k, SZ + 1 + k);
        if ch == 'B' {
            b.set(15, y, z, "powered_rail[powered=true,shape=ascending_north,waterlogged=false]");
            b.set(15, y - 1, z, "redstone_block");
        } else {
            b.set(15, y, z, "rail[shape=ascending_north,waterlogged=false]");
            b.set(15, y - 1, z, "glass");
        }
    }
    // R1's entry: the brake, the flats, the stop
    let (ey, ez) = (-3 - n, SZ + 1 + n);
    b.set(15, ey, ez, "powered_rail[powered=false,shape=north_south,waterlogged=false]");
    b.set(15, ey - 1, ez, "glass");
    for f in 0..flats {
        b.set(15, ey, ez + 1 + f, "rail[shape=north_south,waterlogged=false]");
        b.set(15, ey - 1, ez + 1 + f, "glass");
    }
    b.set(15, ey, ez + 1 + flats, "oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]");
    b.set(15, ey - 1, ez + 1 + flats, "glass");
    // the batch at rest in its pockets (P1, P2, P3: the S3 file's rest positions), front first
    for (k, (x, y)) in [(13.885, -1.8225), (9.885, -0.8225), (5.885, 0.1775)].iter().enumerate() {
        let nbt = if with_riders[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        b.entity("minecart", [*x, *y, SZ as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["One-chunk (b): the descent bench: today's 3 pockets with a batch at rest, G0's corner, a straight descent and R1's entry (a brake, flats, a stop)."])
}

/// One descent run's options.
#[derive(Clone, Debug)]
pub struct DescentOpts {
    pub n: i32,
    pub flats: i32,
    /// REL at T + this: G0 and G1 open (G0's slope powered at once, G1's 8 gt after), G2 `g2_delay`
    /// gt later (its slope 8 gt after).
    pub rel: i64,
    pub g2_delay: i64,
    pub ticks: i64,
    pub seed: i64,
    pub freeze: Option<(i64, i64)>,
    pub trace: bool,
}

/// One descent run's report lines: each cart's speed at the brake's first cell (its stored speed
/// the tick it enters the cell), each contact into a waiting cart, off-rail carts, the riders.
pub fn descent_bench(spec: &FarmSpec, o: &DescentOpts) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 3 {
        return Err(format!("{} carts", carts.len()));
    }
    let riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (g_shut, g_open, rs, stone) = (parse(GATE_SHUT)?, parse(GATE_OPEN)?, parse("redstone_block")?, parse("stone")?);
    for _ in 0..400 {
        run.tick();
        if run.world.time % 100 == 0 {
            break;
        }
    }
    let (ey, ez) = (-3 - o.n, SZ + 1 + o.n);
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    let src = [BlockPos::new(14, -3, 7), BlockPos::new(10, -3, 6), BlockPos::new(6, -2, 6)];
    // (GATES west to east is G2, G1, G0; the departure order G0, G1, G2)
    let gate_at = [BlockPos::new(GATES[2].0, GATES[2].1, SZ), BlockPos::new(GATES[1].0, GATES[1].1, SZ), BlockPos::new(GATES[0].0, GATES[0].1, SZ)];
    let opens = [o.rel, o.rel, o.rel + o.g2_delay];
    let mut at_brake: [Option<(i64, f64)>; 3] = [None; 3];
    let mut prev_v = [0.0f64; 3];
    let mut touching = [[false; 3]; 3];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut off_rail: Vec<String> = Vec::new();
    let mut hurt: Vec<String> = Vec::new();
    let mut health: [Option<f32>; 3] = [None; 3];
    let mut dismount: Vec<String> = Vec::new();
    let mut corner_at: [Option<i64>; 3] = [None; 3];
    let mut out: Vec<String> = Vec::new();
    let mut freeze_pos: Vec<[Option<Vec3>; 3]> = Vec::new();
    let chunks = [0, 0, 0, (ez + 4) >> 4];
    for dt in 0..o.ticks {
        for k in 0..3 {
            if dt == opens[k] {
                run.world.set_block(gate_at[k], g_open.clone(), redsim::world::flags::ALL);
            }
            if dt == opens[k] + if k == 0 { 0 } else { 8 } {
                run.world.set_block(src[k], rs.clone(), redsim::world::flags::ALL);
            }
        }
        let _ = (&g_shut, &stone);
        if let Some((from, len)) = o.freeze {
            if dt == from {
                // (a bench world has no forced chunk box: the chunks its box meets, the marker's
                // spare chunk west of the bench among them)
                if run.world.entity_chunks.is_none() {
                    let (mn, sz) = (run.world.min, run.world.size);
                    run.world.entity_chunks = Some([mn.x.div_euclid(16), mn.z.div_euclid(16), (mn.x + sz[0] - 1).div_euclid(16), (mn.z + sz[2] - 1).div_euclid(16)]);
                }
                redsim::reload::unload_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
            if dt == from + len {
                redsim::reload::load_some(&mut run.world, chunks);
                freeze_pos.push(std::array::from_fn(|k| pos(&run.world, carts[k]).map(|p| p.0)));
            }
        }
        run.tick();
        let w = &run.world;
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        for k in 0..3 {
            let Some((p, v)) = st[k] else {
                if !off_rail.iter().any(|s| s.contains(&format!("cart {k} "))) {
                    off_rail.push(format!("cart {k} gone at T+{dt}"));
                }
                continue;
            };
            let s = (v.x * v.x + v.z * v.z).sqrt();
            let cell = BlockPos::new(p.x.floor() as i32, (p.y + 0.01).floor() as i32, p.z.floor() as i32);
            if corner_at[k].is_none() && cell.x == 15 && cell.z == SZ {
                corner_at[k] = Some(dt);
            }
            if at_brake[k].is_none() && cell.x == 15 && cell.z == ez && (p.y - ey as f64).abs() < 0.6 {
                at_brake[k] = Some((dt, prev_v[k].max(s)));
            }
            // (off the rails: no rail at the cart's cell or the one below)
            let rail_here = |c: BlockPos| w.get(c).to_string().contains("rail");
            if !rail_here(cell) && !rail_here(BlockPos::new(cell.x, cell.y - 1, cell.z)) && !off_rail.iter().any(|s| s.contains(&format!("cart {k} "))) {
                off_rail.push(format!("cart {k} off the rails at T+{dt} ({:.2},{:.2},{:.2})", p.x, p.y, p.z));
            }
        }
        for a in 0..3 {
            for c in 0..3 {
                if a == c {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                // (the boxes, 0.98 wide and 0.7 tall, touching: a slope puts the two centres up
                // to 1 block apart in y as well, so the centres within 1.25)
                let now = d < 1.25;
                if now && !touching[a][c] && sc < 0.05 {
                    let sa = prev_v[a].max((va.x * va.x + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: cart {a} into waiting cart {c} at {sa:.3} ({:.2},{:.2},{:.2})", pa.x, pa.y, pa.z));
                }
                touching[a][c] = now;
            }
        }
        for k in 0..3 {
            prev_v[k] = st[k].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt());
        }
        for k in 0..3 {
            let Some(r) = riders[k] else { continue };
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else {
                if !dismount.iter().any(|d| d.contains(&format!("cart {k}'s"))) {
                    dismount.push(format!("T+{dt}: cart {k}'s rider gone"));
                }
                continue;
            };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if let Some(h0) = health[k]
                && hp < h0
            {
                hurt.push(format!("T+{dt}: cart {k}'s rider {h0} -> {hp}"));
            }
            health[k] = Some(hp);
            if e.vehicle.is_none() && !dismount.iter().any(|d| d.contains(&format!("cart {k}'s"))) {
                dismount.push(format!("T+{dt}: cart {k}'s rider not seated"));
            }
        }
        if o.trace {
            out.push(format!("T+{dt}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")));
        }
    }
    let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(&run.world, *id)).collect();
    out.push(format!(
        "descent bench ({} slopes; REL T+{}, G2 +{}): at the brake's first cell {}",
        o.n,
        o.rel,
        o.g2_delay,
        (0..3).map(|k| format!("cart {k}{} {}", if riders[k].is_some() { "" } else { " (EMPTY)" }, at_brake[k].map_or("never".to_string(), |(t, v)| format!("T+{t} at {v:.3}")))).collect::<Vec<_>>().join(", ")
    ));
    let mut gaps: Vec<String> = Vec::new();
    let mut ab: Vec<(i64, usize)> = (0..3).filter_map(|k| at_brake[k].map(|(t, _)| (t, k))).collect();
    ab.sort();
    for w2 in ab.windows(2) {
        gaps.push(format!("{}", w2[1].0 - w2[0].0));
    }
    out.push(format!(
        "corner passes {:?}; brake arrivals gt apart {}; contacts into a waiting cart: {} (the fastest {:.3}; bar 0.2){}; off the rails: {}; riders hurt {}, dismounts {}; at the end: {}",
        corner_at,
        if gaps.is_empty() { "-".into() } else { gaps.join(", ") },
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) },
        if off_rail.is_empty() { "none".to_string() } else { off_rail.join("; ") },
        hurt.len(),
        dismount.len(),
        st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")
    ));
    if freeze_pos.len() == 2 {
        let f = o.freeze.unwrap_or((0, 0));
        out.push(format!("freeze T+{}..T+{} (chunks {:?} at level 32): {}", f.0, f.0 + f.1, chunks, (0..3).map(|k| match (freeze_pos[0][k], freeze_pos[1][k]) { (Some(a), Some(b)) => format!("cart {k} {:.2},{:.2},{:.2} -> {:.2},{:.2},{:.2}", a.x, a.y, a.z, b.x, b.y, b.z), _ => format!("cart {k} -") }).collect::<Vec<_>>().join("; ")));
    }
    Ok(out)
}

/// Each curve's passages, for tick-exact's curve-pair rule (a follower must not enter a curve
/// before the leader's centre is 0.98 past the curve's centre; G1's bar), as 7b90c3f1's
/// queue-loop-run line measures it (explore/farmopt-s3's queue.rs, copied here for the climb back):
/// per plain curve rail and 3-arm junction, each cart's entry (its centre within 0.99 of the cell's
/// centre, within 1 in height) and clear (0.98 past the centre along its exit, after passing within
/// 0.5 of it), and the least margin, follower entry minus leader clear, over consecutive passages,
/// with the leader's speed at its entry; plus the ticks on which two carts' boxes touch while both
/// centres are within 1.5 of the curve's centre (must be 0).
#[derive(Default)]
struct CurveSpacing {
    cells: Vec<BlockPos>,
    /// (curve, cart): entry tick, speed at entry, passed the centre
    open: std::collections::HashMap<(usize, u32), (i64, f64, bool)>,
    passes: Vec<Vec<(i64, i64, u32, f64)>>,
    touches: Vec<(usize, i64)>,
}

impl CurveSpacing {
    fn init(&mut self, w: &redsim::world::World) {
        let (mn, sz) = (w.min, w.size);
        for x in mn.x..mn.x + sz[0] {
            for y in mn.y..mn.y + sz[1] {
                for z in mn.z..mn.z + sz[2] {
                    let p = BlockPos::new(x, y, z);
                    let s = w.get(p).to_string();
                    let rail = |q: BlockPos| w.get(q).to_string().contains("rail[");
                    let arms = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dz)| (-1..=1).any(|dy| rail(BlockPos::new(x + dx, y + dy, z + dz)))).count();
                    if s.starts_with("minecraft:rail[") && (arms >= 3 || ["north_east", "north_west", "south_east", "south_west"].iter().any(|c| s.contains(&format!("shape={c}")))) {
                        self.cells.push(p);
                    }
                }
            }
        }
        self.passes = vec![Vec::new(); self.cells.len()];
    }
    fn observe(&mut self, w: &redsim::world::World, t: i64) {
        let carts: Vec<&redsim::entity::Entity> = w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Minecart(_))).collect();
        for (i, p) in self.cells.iter().enumerate() {
            let (cx, cz) = (p.x as f64 + 0.5, p.z as f64 + 0.5);
            let near: Vec<&&redsim::entity::Entity> = carts.iter().filter(|e| (e.pos.y - p.y as f64).abs() <= 1.0 && ((e.pos.x - cx).powi(2) + (e.pos.z - cz).powi(2)).sqrt() < 1.5).collect();
            if near.iter().enumerate().any(|(a, ea)| near.iter().skip(a + 1).any(|eb| ea.bb.intersects(eb.bb))) {
                self.touches.push((i, t));
            }
            for e in &carts {
                if (e.pos.y - p.y as f64).abs() > 1.0 {
                    continue;
                }
                let v = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
                let d = ((e.pos.x - cx).powi(2) + (e.pos.z - cz).powi(2)).sqrt();
                let key = (i, e.id);
                match self.open.get(&key).copied() {
                    None if d < 0.99 => {
                        self.open.insert(key, (t, v, d < 0.5));
                    }
                    Some((t0, v0, passed)) => {
                        if !passed && d < 0.5 {
                            self.open.insert(key, (t0, v0, true));
                        } else if passed && d > 0.98 {
                            self.passes[i].push((t0, t, e.id, v0));
                            self.open.remove(&key);
                        } else if !passed && d > 1.5 {
                            self.open.remove(&key);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    /// One line per curve passed by two or more carts (loop cells: the design frame less
    /// (6,0,6)), the least margin first; then the touch count.
    fn report(&self) -> Vec<String> {
        let mut rows: Vec<(i64, String)> = Vec::new();
        for (i, p) in self.cells.iter().enumerate() {
            let mut ps = self.passes[i].clone();
            if ps.len() < 2 {
                continue;
            }
            ps.sort_by_key(|x| x.0);
            let worst = ps.windows(2).map(|w| (w[1].0 - w[0].1, w[0].1, w[0].3, w[1].3)).min_by_key(|x| x.0);
            let touch = self.touches.iter().filter(|x| x.0 == i).count();
            if let Some((m, at, vl, vf)) = worst {
                rows.push((m, format!("({},{},{}) {} passes, least margin {m} gt at t {at} (leader {vl:.2}, follower {vf:.2} b/gt){}", p.x - 6, p.y, p.z - 6, ps.len(), if touch > 0 { format!(", {touch} touching ticks") } else { String::new() })));
            }
        }
        rows.sort_by_key(|r| r.0);
        rows.into_iter().map(|r| r.1).collect()
    }
}
