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

/// One-chunk's stand cycle (farmopt 18:52: the departure side's gate shuts): the departing batch at
/// rest in today's pockets (gates shut) leaves at REL up the S3 departure and v4's climb back (the
/// top at 32 + `east`, `past` flats after it, the bench's sink at their end), while the arriving
/// batch, parked, is handed off at G_a down onechunk's re-laid descent (`design_gravity_v4r`'s cells)
/// into the same pockets. Loop frame + (6,0,6).
pub fn design_stand_cycle(dep: [bool; 3], arr: [bool; 3], east: i32, past: i32, dd: Option<char>, d2: bool) -> String {
    design_stand_cycle_with(dep, arr, east, past, dd, d2, None)
}

/// The pockets' braked cells for the vertical drop (vjeux 21:00): P1 (13,-2), P2 (9,-1), P3 (5,0),
/// each a powered rail ascending west (unpowered: a cart rests on it; powered at REL: it leaves),
/// with its switched support under it; and the hold cells straight above them.
pub const DROP_POCKETS: [(i32, i32); 3] = [(13, -2), (9, -1), (5, 0)];

/// [`design_stand_cycle`], or with `drop` (the hold's trapdoor height) the vertical drop: no
/// descent, no approach and no stand gates; the pockets' cells ([`DROP_POCKETS`]) powered rails
/// braking on their switched supports (stone; the bench puts a redstone block there to release);
/// the departing batch at rest mid-cell on them; the arriving batch off-rail on closed trapdoors at
/// (x, `drop`, 6) straight above each pocket's cell, glass on the cell's four sides at the trapdoor's
/// level (a fixed x, z), the shaft below clear.
pub fn design_stand_cycle_with(dep: [bool; 3], arr: [bool; 3], east: i32, past: i32, dd: Option<char>, d2: bool, drop: Option<i32>) -> String {
    let mut b = Builder::new("stand-cycle-bench");
    for (x, y, s) in STRIP {
        // (the drop keeps (4,1): the anchor that keeps P3's slope (5,0) ascending west)
        if drop.is_some() && x < 4 {
            continue;
        }
        b.set(x, y, SZ, s);
        b.set(x, y - 1, SZ, "glass");
    }
    if d2 {
        // (7b90c3f1's D2: P2's flat a detector)
        b.set(8, 0, SZ, "detector_rail[powered=false,shape=east_west,waterlogged=false]");
    }
    if let Some(h) = drop {
        for (x, y) in DROP_POCKETS {
            b.set(x, y, SZ, "powered_rail[powered=false,shape=ascending_west,waterlogged=false]");
            b.set(x, y - 1, SZ, "stone");
            b.set(x, h, SZ, "oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]");
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                b.set(x + dx, h, SZ + dz, "glass");
            }
        }
    } else {
        for (x, y) in GATES {
            b.set(x, y, SZ, GATE_SHUT);
        }
    }
    for (x, y, z) in [(6, -2, 6), (10, -3, 6), (14, -3, 7)] {
        b.set(x, y, z, "stone");
    }
    if drop.is_none() {
        b.set(AX - 1, 2, SZ, "glass");
        b.set(AX - 1, 3, SZ, "glass");
    }
    let boost = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str| {
        b.set(x, y, z, &format!("powered_rail[powered=true,shape={shape},waterlogged=false]"));
        b.set(x, y - 1, z, "redstone_block");
    };
    let flat = |b: &mut Builder, x: i32, y: i32, z: i32, shape: &str| {
        b.set(x, y, z, &format!("rail[shape={shape},waterlogged=false]"));
        b.set(x, y - 1, z, "glass");
    };
    // the departure: G0's corner and the five committed boosters (`dd` B: the first a sloped
    // detector on glass, loop (9,-3,1)); then v4's re-laid crest (`dd` given): (15,2,12) a slope (`dd`
    // A: a sloped detector on glass, loop (9,2,6)), (15,3,13) a slope, (15,4,14) a flat; or the
    // bench's legacy crest (no `dd`): a flat (15,2,12), slopes (15,2,13), (15,3,14); the curve (15,4,15)
    flat(&mut b, 15, -3, SZ, "south_west");
    for i in 0..5 {
        if i == 0 && dd == Some('B') {
            b.set(15, -3, 7, "detector_rail[powered=false,shape=ascending_south,waterlogged=false]");
            b.set(15, -4, 7, "glass");
        } else {
            boost(&mut b, 15, -3 + i, 7 + i, "ascending_south");
        }
    }
    if dd.is_some() {
        if dd == Some('A') {
            b.set(15, 2, 12, "detector_rail[powered=false,shape=ascending_south,waterlogged=false]");
            b.set(15, 1, 12, "glass");
        } else {
            boost(&mut b, 15, 2, 12, "ascending_south");
        }
        boost(&mut b, 15, 3, 13, "ascending_south");
        boost(&mut b, 15, 4, 14, "north_south");
    } else {
        flat(&mut b, 15, 2, 12, "north_south");
        boost(&mut b, 15, 2, 13, "ascending_south");
        boost(&mut b, 15, 3, 14, "ascending_south");
    }
    flat(&mut b, 15, 4, 15, "north_west");
    for i in 1..=14 {
        boost(&mut b, 15 - i, 3 + i, 15, "ascending_west");
    }
    flat(&mut b, 0, 18, 15, "north_east");
    for i in 1..=14 {
        boost(&mut b, 0, 17 + i, 15 - i, "ascending_north");
    }
    flat(&mut b, 0, 32, 0, "south_east");
    for i in 1..=east {
        boost(&mut b, i, 31 + i, 0, "ascending_east");
    }
    let top = 32 + east;
    for x in east + 1..=14 {
        boost(&mut b, x, top, 0, "east_west");
    }
    flat(&mut b, 15, top, 0, "south_west");
    for z in 1..=past {
        flat(&mut b, 15, top, z, "north_south");
    }
    b.set(15, top, past + 1, "stone");
    if let Some(h) = drop {
        // the departing batch at rest mid-cell on the braked pockets, front first; the arriving
        // batch off-rail on the hold's trapdoors straight above
        for (k, (x, y)) in DROP_POCKETS.iter().enumerate() {
            let nbt = if dep[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
            b.entity("minecart", [*x as f64 + 0.5, *y as f64 + 0.5625, SZ as f64 + 0.5], Some(nbt));
        }
        for (k, (x, _)) in DROP_POCKETS.iter().enumerate() {
            let nbt = if arr[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
            b.entity("minecart", [*x as f64 + 0.5, h as f64 + 0.1875, SZ as f64 + 0.5], Some(nbt));
        }
        b.set(-8, 0, SZ, "glass");
        b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
        b.d.ports.insert("output".into(), [-6, 1, SZ]);
        return b.text(&["One-chunk's stand cycle with the vertical drop: the departing batch leaves the braked pockets at REL up the S3 departure and v4's climb back; the arriving batch drops from trapdoors straight above the pockets (supports and trapdoors bench-driven on explicit times)."]);
    }
    // the approach from the committed crest down to the snap turn, and the re-laid descent
    b.set(AX, 6, 10, "rail[shape=north_south,waterlogged=false]");
    b.set(AX, 5, 10, "glass");
    for (y, z) in [(5, 9), (4, 8), (3, 7)] {
        b.set(AX, y, z, "rail[shape=ascending_south,waterlogged=false]");
        b.set(AX, y - 1, z, "glass");
    }
    boost(&mut b, AX, 6, 11, "north_south");
    b.set(AX, 6, 12, "rail[shape=north_east,waterlogged=false]");
    b.set(AX, 5, 12, "glass");
    boost(&mut b, 4, 6, 12, "east_west");
    for (i, x) in (5..=12).enumerate() {
        boost(&mut b, x, 6 + i as i32, 12, "ascending_east");
    }
    b.set(13, 14, 12, "rail[shape=north_west,waterlogged=false]");
    b.set(13, 13, 12, "glass");
    b.set(13, 14, 11, "detector_rail[powered=false,shape=ascending_north,waterlogged=false]");
    b.set(13, 13, 11, "glass");
    b.set(13, 15, 10, "rail[shape=ascending_north,waterlogged=false]");
    b.set(13, 14, 10, "glass");
    b.set(13, 16, 9, "stone");
    // the departing batch at rest in its pockets, front first; then the arriving batch, parked
    for (k, (x, y)) in [(13.885, -1.8225), (9.885, -0.8225), (5.885, 0.1775)].iter().enumerate() {
        let nbt = if dep[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        b.entity("minecart", [*x, *y, SZ as f64 + 0.5], Some(nbt));
    }
    for k in 0..3usize {
        let nbt = if arr[k] { "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}" } else { "{Rotation:[0f,0f]}" };
        let ki = k as i32;
        b.set(-6, 0, SZ + 3 + 3 * ki, "rail[shape=north_south,waterlogged=false]");
        b.set(-6, -1, SZ + 3 + 3 * ki, "glass");
        b.entity("minecart", [-5.5, 0.0625, (SZ + 3 + 3 * ki) as f64 + 0.5], Some(nbt));
    }
    b.set(-8, 0, SZ, "glass");
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.ports.insert("output".into(), [-6, 1, SZ]);
    b.text(&["One-chunk's stand cycle: the departing batch leaves today's pockets at REL up the S3 departure and v4's climb back while the arriving batch comes down onechunk's re-laid descent into the same pockets (gates bench-driven on explicit times)."])
}

/// How G0 shuts after REL in a stand-cycle run.
#[derive(Clone, Copy, Debug)]
pub enum G0Key {
    /// At REL + this (live), or never.
    Fixed(Option<i64>),
    /// 7b90c3f1's rule: on D1's (12,-1,6) first falling edge after REL (it rose after REL), + this;
    /// the backstop at REL + `backstop` if D1 never rose after REL.
    D1(i64),
    /// On the departure detector's first falling edge after REL (it rose after REL), + this.
    Dd(i64),
    /// (a) On the departure detector's rising edges after REL counted up to the departing batch's
    /// count (the carts present at REL), + this after the last; the backstop if they never reach it.
    DdCount(i64),
    /// (b) A lockable delay restarted by every departure-detector rise: armed at each fall after
    /// REL, G0 shuts when it runs this long with no new rise; the backstop if Dd never rose.
    DdRetrig(i64),
    /// (d) 7b90c3f1's zero-delay key: on D2's (8,0,6) first rising edge after REL + this (a
    /// lockable arming): the arriving cart 0's pass at P2's flat.
    D2Arm(i64),
}

/// One stand-cycle run's options.
#[derive(Clone, Debug)]
pub struct CycleOpts {
    /// REL, gt after T (live): G0 opens (its slope powered at once), G1 `g1_delay` later and G2
    /// `g2_delay` later (their slopes `slope_delay` after they open).
    pub rel: i64,
    pub g1_delay: i64,
    pub g2_delay: i64,
    pub slope_delay: i64,
    /// The hold's release, gt after T (live): the arriving carts handed off at G_a at `speed` b/t,
    /// cart 1 `every` gt after cart 0, cart 2 `every2` after cart 1.
    pub hold: i64,
    pub every: i64,
    pub every2: i64,
    pub speed: f64,
    /// Per-cart hand-off speeds (the measured hold: leader 0.33..0.40, followers 0.4).
    pub hand_speeds: Option<[f64; 3]>,
    pub g0_key: G0Key,
    pub backstop: i64,
    /// G1's and G2's shuts: (A, B) gt after D1's first rising edge after G0 shut (the arriving
    /// cart 0), or None: `shut_fixed` [1] and [2] gt after the hold's release (live).
    pub fill_key: Option<(i64, i64)>,
    pub shut_fixed: [Option<i64>; 3],
    /// The edge-keyed delays as redstone runs them (world ticks, through a freeze), or qualified
    /// (ironspec 19:04): a falling edge counts only if the 20 gt before it were all live, and every
    /// delay and the backstop count live ticks only.
    pub qualified: bool,
    /// The departure detector's cell (design frame), if the design has one.
    pub dd: Option<BlockPos>,
    /// D2 (8,0,6) a detector in the design; the fill's G2 keyed on D2's second rise after REL +
    /// `d2_arm` (+ `fill_key`'s B) instead of D1's first rise after G0 shut.
    pub d2: bool,
    pub d2_arm: i64,
    pub fill_d2: bool,
    /// The departing riders' feet height to cross by T+100.
    pub above: f64,
    /// Carts absent from the run (departing 0..2, arriving 3..5): a batch of 1 or 2.
    pub absent: [bool; 6],
    /// The vertical drop (vjeux 21:00; `design_stand_cycle_with`'s `drop` design): (the hold's
    /// trapdoor y, the trapdoors open at REL + this, the pockets' supports and slope sources off at
    /// REL + this), on the live clock; no gates, the pockets released by their supports at REL,
    /// + G1's and G2's staggers.
    pub drop: Option<(i32, i64, i64)>,
    pub ticks: i64,
    pub seed: i64,
    pub freeze: Option<(i64, i64)>,
    pub top_y: i32,
    pub past: i32,
    pub trace: bool,
}

/// One stand-cycle run's figures for the grid's summary: per gate (G0, G1, G2) the actual shut's
/// margin after the last passing cart's clear and before its stopping cart's face (None: no shut,
/// or no such cart), gate edges, wrong pockets, contacts and the fastest, the departing riders'
/// margin over the height (None: no riders; i64::MIN: not all crossed), the arriving batch's rest
/// margin (i64::MIN: not at rest), the least curve margin, riders hurt or unseated.
#[derive(Clone, Debug, Default)]
pub struct CycleStats {
    pub left: [Option<i64>; 3],
    pub right: [Option<i64>; 3],
    pub edges: usize,
    pub wrong: usize,
    pub contacts: usize,
    pub contact_max: f64,
    pub climb: Option<i64>,
    pub rest: i64,
    pub curve: Option<i64>,
    pub hurt: usize,
    pub unseated: usize,
    /// (d) The REL + arm arming's lead over the last departing cart's D2 rise.
    pub d2_arm_margin: Option<i64>,
    /// The drop: the closest an arriving cart came to a departing one (hundredths of a block), the
    /// longest fall (gt; i64::MAX: a cart never landed), the lowest rider's feet at the release
    /// (hundredths).
    pub drop_closest: Option<i64>,
    pub drop_fall: Option<i64>,
    pub drop_feet: Option<i64>,
}

/// One stand-cycle run's report lines: per gate the shut window (no earlier than the last passing
/// cart's box leaving the gate's cell, departing or arriving; no later than the arriving cart it
/// stops reaching it), the gate edges on any cart at the explicit shuts, each arriving cart in its
/// own pocket and its rest tick against T+100, the departing carts' crest ticks, contacts, riders.
pub fn stand_cycle_bench(spec: &FarmSpec, o: &CycleOpts) -> Result<(Vec<String>, CycleStats), String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(o.seed, (o.ticks + 600) as u32, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let carts: Vec<u32> = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.id).collect();
    if carts.len() != 6 {
        return Err(format!("{} carts", carts.len()));
    }
    let riders: Vec<Option<u32>> = carts.iter().map(|id| run.world.entities.list.iter().find(|e| e.id == *id).and_then(|e| e.passengers.first().copied())).collect();
    // (absent carts: removed with their riders before the warm-up)
    for n in 0..6 {
        if o.absent[n] {
            for e in run.world.entities.list.iter_mut() {
                if e.id == carts[n] || Some(e.id) == riders[n] {
                    e.removed = true;
                }
            }
        }
    }
    let parse = |s: &str| Block::parse(&format!("minecraft:{s}")).map_err(|e| e.to_string());
    let (g_open, g_shut, rs, stone) = (parse(GATE_OPEN)?, parse(GATE_SHUT)?, parse("redstone_block")?, parse("stone")?);
    for t in 0..400 {
        run.tick();
        if t >= 60 && run.world.time % 100 == 0 {
            break;
        }
    }
    let pos = |w: &redsim::world::World, id: u32| w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| (e.pos, e.delta_movement));
    let place_at = |w: &mut redsim::world::World, id: u32, p: Vec3, v: Vec3| {
        let Some(i) = w.entities.list.iter().position(|e| e.id == id) else { return };
        w.entities.set_pos(i, p);
        w.entities.list[i].delta_movement = v;
        let ps = w.entities.list[i].passengers.clone();
        for r in ps {
            if let Some(j) = w.entities.list.iter().position(|e| e.id == r) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    };
    // (G0, G1, G2: the gate cells, their slopes' sources)
    let gx = [14, 10, 6];
    let gy = [-2, -1, 0];
    let gcell = [BlockPos::new(14, -2, SZ), BlockPos::new(10, -1, SZ), BlockPos::new(6, 0, SZ)];
    let gsrc = [BlockPos::new(14, -3, SZ + 1), BlockPos::new(10, -3, SZ), BlockPos::new(6, -2, SZ)];
    let open_at = [o.rel, o.rel + o.g1_delay, o.rel + o.g2_delay];
    let power_at = [open_at[0], open_at[1] + o.slope_delay, open_at[2] + o.slope_delay];
    let hand = [o.hold, o.hold + o.every, o.hold + o.every + o.every2];
    let ga = Vec3::new(13.5, 15.5625, 10.5);
    let in_cell = |p: Vec3, g: usize| p.x + 0.49 > gx[g] as f64 && p.x - 0.49 < gx[g] as f64 + 1.0 && p.y + 0.7 > gy[g] as f64 && p.y < gy[g] as f64 + 1.5 && (p.z - (SZ as f64 + 0.5)).abs() < 0.99;
    let d1_cell = BlockPos::new(12, -1, SZ);
    let powered_at = |w: &redsim::world::World, p: BlockPos| w.get(p).to_string().contains("powered=true");
    // (a shut time: on the world clock (redstone through a freeze) or the live clock)
    #[derive(Clone, Copy)]
    enum Due {
        World(i64),
        Live(i64),
    }
    let reached = |d: Option<Due>, dt: i64, live: i64| match d {
        Some(Due::World(t)) => dt >= t,
        Some(Due::Live(t)) => live >= t,
        None => false,
    };
    let mut due: [Option<Due>; 3] = [None; 3];
    if let G0Key::Fixed(Some(s)) = o.g0_key {
        due[0] = Some(Due::Live(o.rel + s));
    }
    if o.fill_key.is_none() {
        for g in 1..3 {
            due[g] = o.shut_fixed[g].map(|s| Due::Live(o.hold + s));
        }
    }
    let mut handed = [false; 3];
    let mut opened = [false; 3];
    let mut is_shut = [false; 3];
    let mut shut_tick: [Option<i64>; 3] = [None; 3];
    let mut shut_how: [String; 3] = [String::new(), String::new(), String::new()];
    let mut edges: Vec<String> = Vec::new();
    let mut dep_last_in: [[Option<i64>; 3]; 3] = [[None; 3]; 3];
    let mut arr_first_in: [[Option<i64>; 3]; 3] = [[None; 3]; 3];
    let mut arr_last_in: [[Option<i64>; 3]; 3] = [[None; 3]; 3];
    let mut rest_since: [Option<i64>; 3] = [None; 3];
    let mut dep_crest: [Option<i64>; 3] = [None; 3];
    let mut dep_above: [Option<i64>; 3] = [None; 3];
    let mut sunk: [Option<i64>; 3] = [None; 3];
    let mut prev_v = [0.0f64; 6];
    let mut touching = [[false; 6]; 6];
    let mut contacts: Vec<String> = Vec::new();
    let mut contact_max = 0.0f64;
    let mut hurt = 0usize;
    let mut health: [Option<f32>; 6] = [None; 6];
    let mut unseated = 0usize;
    let mut frozen_ticks = 0i64;
    let mut frozen_hist: Vec<bool> = Vec::new();
    let mut curves = CurveSpacing::default();
    curves.init(&run.world);
    let mut rel_dt: Option<i64> = None;
    let (mut d1_was, mut dd_was) = (false, false);
    let (mut d1_rose_after_rel, mut dd_rose_after_rel) = (false, false);
    let mut dd_rises = 0usize;
    let d2_cell = BlockPos::new(8, 0, SZ);
    let mut d2_was = false;
    let mut d2_armed_rises = 0usize;
    let mut d2_pulses: Vec<(i64, Option<i64>)> = Vec::new();
    let mut last_dep_d2: Option<i64> = None;
    // (the drop: the trapdoors' tick, each arriving cart's position then, its landing on its
    // pocket's rail, the closest approach of an arriving cart to a departing one after the drop)
    let mut drop_at: Option<i64> = None;
    let mut drop_from: [Option<Vec3>; 3] = [None; 3];
    let mut drop_feet: [Option<f64>; 3] = [None; 3];
    let mut landed: [Option<(i64, f64)>; 3] = [None; 3];
    let mut closest: Option<(f64, i64, usize, usize)> = None;
    let mut fall_v: [f64; 3] = [0.0; 3];
    let mut arm_dt: Option<i64> = None;
    let mut retrig_cancels = 0usize;
    let n_dep = (0..3).filter(|n| !o.absent[*n]).count();
    let mut d1_pulses: Vec<(i64, Option<i64>)> = Vec::new();
    let mut dd_pulses: Vec<(i64, Option<i64>)> = Vec::new();
    let mut ignored_falls: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let name = |n: usize| format!("{} cart {}{}", if n < 3 { "departing" } else { "arriving" }, n % 3, if riders[n].is_some() { "" } else { " (EMPTY)" });
    for dt in 0..o.ticks {
        let frozen_now = o.freeze.is_some_and(|(f, l)| dt >= f && dt < f + l);
        let live = dt - frozen_ticks;
        if rel_dt.is_none() && live >= o.rel {
            rel_dt = Some(dt);
        }
        for k in 0..3 {
            if o.drop.is_some() {
                break;
            }
            if !frozen_now && !handed[k] && live >= hand[k] && !o.absent[3 + k] {
                handed[k] = true;
                place_at(&mut run.world, carts[3 + k], ga, Vec3::new(0.0, 0.0, o.hand_speeds.map_or(o.speed, |s| s[k])));
            }
        }
        // (the vertical drop: the hold's trapdoors open at REL + `release`, live)
        if let Some((h, release, _)) = o.drop
            && !frozen_now
            && !handed.iter().any(|x| *x)
            && live >= o.rel + release
        {
            for (k, (x, _)) in DROP_POCKETS.iter().enumerate() {
                let p = BlockPos::new(*x, h, SZ);
                let open = run.world.get(p).to_string().replace("open=false", "open=true").replace("powered=false", "powered=true");
                if let Ok(bk) = Block::parse(&open) {
                    run.world.set_block(p, bk, redsim::world::flags::ALL);
                }
                if !o.absent[3 + k] {
                    handed[k] = true;
                    drop_at = Some(dt);
                    drop_from[k] = pos(&run.world, carts[3 + k]).map(|(p, _)| p);
                    drop_feet[k] = riders[3 + k].and_then(|r| run.world.entities.list.iter().find(|e| e.id == r && !e.removed)).map(|e| e.pos.y);
                }
            }
        }
        // (the backstop: D1 never rose after REL (D1), Dd never rose (the restartable delay), no
        // departing cart at all (the count))
        let backstop_applies = match o.g0_key {
            G0Key::D1(_) => !d1_rose_after_rel,
            G0Key::DdRetrig(_) => !dd_rose_after_rel,
            G0Key::DdCount(_) => n_dep == 0,
            _ => false,
        };
        if backstop_applies
            && due[0].is_none()
            && !is_shut[0]
            && let Some(r) = rel_dt
        {
            let at = if o.qualified { Due::Live(o.rel + o.backstop) } else { Due::World(r + o.backstop) };
            if reached(Some(at), dt, live) {
                due[0] = Some(at);
                shut_how[0] = "the backstop".into();
            }
        }
        for g in 0..3 {
            // (the drop: no gates; each pocket's support and its slope's source powered from REL
            // (+ the stagger) to REL + `off`)
            if let Some((_, _, off)) = o.drop {
                let on = live >= open_at[g] && live < o.rel + off;
                let want_src = if on { rs.clone() } else { stone.clone() };
                let (px, py) = DROP_POCKETS[g];
                for p in [gsrc[g], BlockPos::new(px, py - 1, SZ)] {
                    if run.world.get(p) != want_src {
                        run.world.set_block(p, want_src.clone(), redsim::world::flags::ALL);
                    }
                }
                continue;
            }
            if !opened[g] && live >= open_at[g] {
                opened[g] = true;
            }
            if opened[g] && !is_shut[g] && reached(due[g], dt, live) {
                is_shut[g] = true;
                shut_tick[g] = Some(dt);
                // (a gate edge: a cart's box in the gate's cell as it shuts, at the last tick's
                // positions)
                for (n, id) in carts.iter().enumerate() {
                    if let Some((p, _)) = pos(&run.world, *id)
                        && in_cell(p, g)
                    {
                        edges.push(format!("T+{dt}: G{g} shut on the {} at {:.2},{:.2},{:.2}", name(n), p.x, p.y, p.z));
                    }
                }
            }
            let open = opened[g] && !is_shut[g];
            let want = if open { g_open.clone() } else { g_shut.clone() };
            if run.world.get(gcell[g]) != want {
                run.world.set_block(gcell[g], want, redsim::world::flags::ALL);
            }
            let want_src = if open && live >= power_at[g] { rs.clone() } else { stone.clone() };
            if run.world.get(gsrc[g]) != want_src {
                run.world.set_block(gsrc[g], want_src, redsim::world::flags::ALL);
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
                redsim::reload::unload_some(&mut run.world, [0, 0, 0, 0]);
            }
            if dt == from + len {
                redsim::reload::load_some(&mut run.world, [0, 0, 0, 0]);
            }
        }
        frozen_hist.push(frozen_now);
        run.tick();
        curves.observe(&run.world, dt);
        // the detectors' edges and the keyed shuts
        {
            let after_rel = rel_dt.is_some_and(|r| dt >= r);
            let quiet20 = frozen_hist.iter().rev().take(21).all(|f| !*f);
            let d1 = powered_at(&run.world, d1_cell);
            if d1 && !d1_was {
                d1_pulses.push((dt, None));
                if after_rel {
                    d1_rose_after_rel = true;
                }
                // (the fill: G1 (and G2 unless keyed on D2) on D1's first rise after G0 shut)
                if let Some((a, b)) = o.fill_key
                    && is_shut[0]
                    && shut_tick[0].is_some_and(|s| dt >= s)
                    && due[1].is_none()
                {
                    due[1] = Some(if o.qualified { Due::Live(live + a) } else { Due::World(dt + a) });
                    shut_how[1] = format!("D1's rise at T+{dt} + {a}");
                    if !o.fill_d2 {
                        due[2] = Some(if o.qualified { Due::Live(live + b) } else { Due::World(dt + b) });
                        shut_how[2] = format!("D1's rise at T+{dt} + {b}");
                    }
                }
            }
            if !d1 && d1_was {
                if let Some(p) = d1_pulses.last_mut() {
                    p.1 = Some(dt);
                }
                if let G0Key::D1(k) = o.g0_key
                    && due[0].is_none()
                    && d1_rose_after_rel
                {
                    if o.qualified && !quiet20 {
                        ignored_falls.push(format!("D1 fall T+{dt} (a freeze in the 20 gt before)"));
                    } else {
                        due[0] = Some(if o.qualified { Due::Live(live + k) } else { Due::World(dt + k) });
                        shut_how[0] = format!("D1's fall at T+{dt} + {k}");
                    }
                }
            }
            d1_was = d1;
            // (D2: G0 on its first rise after the arming (d); the fill's G2 on its second)
            if o.d2 {
                if arm_dt.is_none()
                    && let Some(r) = rel_dt
                    && (if o.qualified { live >= o.rel + o.d2_arm } else { dt >= r + o.d2_arm })
                {
                    arm_dt = Some(dt);
                }
                let d2 = powered_at(&run.world, d2_cell);
                if d2 && !d2_was {
                    d2_pulses.push((dt, None));
                    // (whose rise: the cart nearest D2's cell; a departing cart's rise is the
                    // departure's, before the arming)
                    let near = (0..6)
                        .filter_map(|n| pos(&run.world, carts[n]).map(|(p, _)| (n, (p.x - 8.5).powi(2) + (p.y - 0.06).powi(2) + (p.z - 6.5).powi(2))))
                        .min_by(|a, b| a.1.total_cmp(&b.1));
                    if let Some((n, _)) = near
                        && n < 3
                    {
                        last_dep_d2 = Some(dt);
                    }
                    let armed = match rel_dt {
                        Some(r) => if o.qualified { live >= o.rel + o.d2_arm } else { dt >= r + o.d2_arm },
                        None => false,
                    };
                    if armed {
                        d2_armed_rises += 1;
                        if let G0Key::D2Arm(_) = o.g0_key
                            && d2_armed_rises == 1
                            && !is_shut[0]
                            && due[0].is_none()
                        {
                            due[0] = Some(if o.qualified { Due::Live(live) } else { Due::World(dt) });
                            shut_how[0] = format!("D2's first rise after the arming, T+{dt}");
                        }
                        // (7b90c3f1's chain (0ab47d8a75): G2 on a D2 rise while G1 is shut)
                        if o.fill_d2
                            && is_shut[1]
                            && shut_tick[1].is_some_and(|s| dt >= s)
                            && due[2].is_none()
                            && let Some((_, b)) = o.fill_key
                        {
                            due[2] = Some(if o.qualified { Due::Live(live + b) } else { Due::World(dt + b) });
                            shut_how[2] = format!("D2's rise at T+{dt} (G1 shut) + {b}");
                        }
                    }
                }
                if !d2 && d2_was
                    && let Some(p) = d2_pulses.last_mut()
                {
                    p.1 = Some(dt);
                }
                d2_was = d2;
            }
            if let Some(ddp) = o.dd {
                let dd = powered_at(&run.world, ddp);
                if dd && !dd_was {
                    dd_pulses.push((dt, None));
                    if after_rel {
                        dd_rose_after_rel = true;
                        dd_rises += 1;
                        match o.g0_key {
                            // (a) the count: the last departing cart's rise
                            G0Key::DdCount(k) if !is_shut[0] && due[0].is_none() && dd_rises == n_dep => {
                                due[0] = Some(if o.qualified { Due::Live(live + k) } else { Due::World(dt + k) });
                                shut_how[0] = format!("Dd's rise {dd_rises} of {n_dep} at T+{dt} + {k}");
                            }
                            // (b) a rise restarts the delay
                            G0Key::DdRetrig(_) if !is_shut[0] && due[0].is_some() => {
                                due[0] = None;
                                retrig_cancels += 1;
                            }
                            _ => {}
                        }
                    }
                }
                if !dd && dd_was {
                    if let Some(p) = dd_pulses.last_mut() {
                        p.1 = Some(dt);
                    }
                    if let G0Key::Dd(k) | G0Key::DdRetrig(k) = o.g0_key
                        && !is_shut[0]
                        && (due[0].is_none() || matches!(o.g0_key, G0Key::DdRetrig(_)))
                        && dd_rose_after_rel
                    {
                        if o.qualified && !quiet20 {
                            ignored_falls.push(format!("Dd fall T+{dt} (a freeze in the 20 gt before)"));
                        } else {
                            due[0] = Some(if o.qualified { Due::Live(live + k) } else { Due::World(dt + k) });
                            shut_how[0] = format!("Dd's fall at T+{dt} + {k}{}", if matches!(o.g0_key, G0Key::DdRetrig(_)) { format!(" (restartable; {retrig_cancels} restarts)") } else { String::new() });
                        }
                    }
                }
                dd_was = dd;
            }
        }
        let w = &run.world;
        let st: Vec<Option<(Vec3, Vec3)>> = carts.iter().map(|id| pos(w, *id)).collect();
        let mut sink_now: Vec<usize> = Vec::new();
        for n in 0..6 {
            let Some((p, v)) = st[n] else { continue };
            for g in 0..3 {
                if !in_cell(p, g) {
                    continue;
                }
                if n < 3 {
                    if live >= o.rel {
                        dep_last_in[n][g] = Some(dt);
                    }
                } else if handed[n - 3] {
                    arr_first_in[n - 3][g].get_or_insert(dt);
                    arr_last_in[n - 3][g] = Some(dt);
                }
            }
            let s = (v.x * v.x + v.z * v.z).sqrt();
            if n < 3 {
                if dep_crest[n].is_none() && p.x > 15.0 && p.z > 12.0 && p.z < 13.0 && p.y > 1.9 {
                    dep_crest[n] = Some(dt);
                }
                if dep_above[n].is_none()
                    && let Some(r) = riders[n]
                    && let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed)
                    && e.pos.y >= o.above
                {
                    dep_above[n] = Some(dt);
                }
                if p.y > o.top_y as f64 - 0.1 && p.x > 14.5 && p.z >= o.past as f64 - 0.4 {
                    sunk[n] = Some(dt);
                    sink_now.push(n);
                }
            } else if handed[n - 3] {
                // (the drop: at rest counts the fall's speed too; the landing: the first tick on
                // the pocket's rail, the speed of the fall just before it)
                let s3 = if o.drop.is_some() { (v.x * v.x + v.y * v.y + v.z * v.z).sqrt() } else { s };
                if o.drop.is_some() {
                    let (_, py) = DROP_POCKETS[n - 3];
                    if landed[n - 3].is_none() && p.y < py as f64 + 1.0 {
                        landed[n - 3] = Some((dt, fall_v[n - 3]));
                    }
                    fall_v[n - 3] = -v.y;
                    for m in 0..3 {
                        if let Some((pd, _)) = st[m] {
                            let d = ((p.x - pd.x).powi(2) + (p.y - pd.y).powi(2) + (p.z - pd.z).powi(2)).sqrt();
                            if closest.is_none_or(|c| d < c.0) {
                                closest = Some((d, dt, n - 3, m));
                            }
                        }
                    }
                }
                if s3 < 1e-3 && (p.z - (SZ as f64 + 0.5)).abs() < 0.6 && p.y < 3.0 {
                    rest_since[n - 3].get_or_insert(dt);
                } else {
                    rest_since[n - 3] = None;
                }
            }
        }
        for a in 0..6 {
            for c in 0..6 {
                if a == c || (a >= 3 && !handed[a - 3]) || (c >= 3 && !handed[c - 3]) {
                    continue;
                }
                let (Some((pa, va)), Some((pc, vc))) = (st[a], st[c]) else { continue };
                let d = ((pa.x - pc.x).powi(2) + (pa.y - pc.y).powi(2) + (pa.z - pc.z).powi(2)).sqrt();
                let sc = (vc.x * vc.x + vc.z * vc.z).sqrt();
                let now = d < 1.25;
                if now && !touching[a][c] && sc < 0.05 && dt > 0 {
                    let sa = prev_v[a].max((va.x * va.x + va.z * va.z).sqrt());
                    contact_max = contact_max.max(sa);
                    contacts.push(format!("T+{dt}: the {} into the {} at {sa:.3}", name(a), name(c)));
                }
                touching[a][c] = now;
            }
        }
        for n in 0..6 {
            prev_v[n] = st[n].map_or(0.0, |(_, v)| (v.x * v.x + v.z * v.z).sqrt());
            let Some(r) = riders[n] else { continue };
            let Some(e) = w.entities.list.iter().find(|e| e.id == r && !e.removed) else { continue };
            let hp = if let EntityKind::Mob(m) = &e.kind { m.health } else { 0.0 };
            if health[n].is_some_and(|h0| hp < h0) {
                hurt += 1;
            }
            health[n] = Some(hp);
            if e.vehicle.is_none() {
                unseated += 1;
            }
        }
        if o.trace {
            out.push(format!("T+{dt}: {}", st.iter().map(|s| s.map_or("-".to_string(), |(p, v)| format!("{:.2},{:.2},{:.2} v{:.3}", p.x, p.y, p.z, (v.x * v.x + v.z * v.z).sqrt()))).collect::<Vec<_>>().join(" | ")));
        }
        for n in sink_now {
            let ps: Vec<u32> = run.world.entities.list.iter().find(|e| e.id == carts[n]).map(|e| e.passengers.clone()).unwrap_or_default();
            for e in run.world.entities.list.iter_mut() {
                if e.id == carts[n] || ps.contains(&e.id) {
                    e.removed = true;
                }
            }
        }
    }
    // the shut windows: gate g stops the g-th present arriving cart (G0 the first, G1 the second,
    // G2 the third: a missing cart shifts the rest forward); the present arriving carts ahead of it
    // and every departing cart pass it
    let present: Vec<usize> = (0..3).filter(|k| !o.absent[3 + *k]).collect();
    let mut stats = CycleStats::default();
    let mut wins: Vec<String> = Vec::new();
    for g in 0..3 {
        let mut early: Option<(i64, String)> = None;
        for n in 0..3 {
            if let Some(t) = dep_last_in[n][g]
                && early.as_ref().is_none_or(|e| t + 1 > e.0)
            {
                early = Some((t + 1, format!("departing cart {n}")));
            }
        }
        for &k in present.iter().take(g) {
            if let Some(t) = arr_last_in[k][g]
                && early.as_ref().is_none_or(|e| t + 1 > e.0)
            {
                early = Some((t + 1, format!("arriving cart {k}")));
            }
        }
        let late = present.get(g).and_then(|&k| arr_first_in[k][g]);
        let rel_h = |t: i64| format!("T+{t} (REL+{}, H+{})", t - o.rel, t - o.hold);
        if let Some(s) = shut_tick[g] {
            stats.left[g] = early.as_ref().map(|e| s - e.0);
            stats.right[g] = late.map(|l| l - s);
        }
        // (the margins of the actual shut: after the last passer's clear, before the stopper)
        let margins = match shut_tick[g] {
            Some(s) => format!(
                ", shut at T+{s}{} (after the leader by {}, before the follower by {})",
                if shut_how[g].is_empty() { String::new() } else { format!(" on {}", shut_how[g]) },
                early.as_ref().map_or("-".to_string(), |e| format!("{}", s - e.0)),
                late.map_or("-".to_string(), |l| format!("{}", l - s))
            ),
            None => ", shut never".to_string(),
        };
        wins.push(format!(
            "G{g}: from {} ({}) to {} (the arriving cart it stops at its face){}{margins}",
            early.as_ref().map_or("-".to_string(), |e| rel_h(e.0)),
            early.as_ref().map_or("-".to_string(), |e| e.1.clone()),
            late.map_or("-".to_string(), rel_h),
            match (&early, late) { (Some(e), Some(l)) => format!(", window {} gt", l - e.0 + 1), _ => String::new() },
        ));
    }
    if let Some((h, release, off)) = o.drop {
        // (the drop's line instead of the gates' windows)
        out.push(format!(
            "vertical drop (REL T+{}, P2 +{}, P3 +{}; the hold's trapdoors y {h} open at REL+{release}{}, the pockets re-braked at REL+{off}): released {}; riders' feet then {}; landed {}; at rest {}; the closest an arriving cart came to a departing one {}",
            o.rel,
            o.g1_delay,
            o.g2_delay,
            drop_at.map_or(" (never)".to_string(), |t| format!(" = T+{t}")),
            (0..3).map(|k| drop_from[k].map_or(format!("cart {k} -"), |p| format!("cart {k} at {:.3},{:.3},{:.3}", p.x, p.y, p.z))).collect::<Vec<_>>().join(", "),
            (0..3).map(|k| drop_feet[k].map_or("-".to_string(), |y| format!("{y:.3}"))).collect::<Vec<_>>().join(", "),
            (0..3).map(|k| match (landed[k], drop_at) {
                (Some((t, v)), Some(d)) => format!("cart {k} T+{t} (fall {} gt, {v:.3} b/gt)", t - d),
                _ => format!("cart {k} never"),
            }).collect::<Vec<_>>().join(", "),
            (0..3).map(|k| rest_since[k].map_or(format!("cart {k} NO"), |t| format!("cart {k} T+{t}"))).collect::<Vec<_>>().join(", "),
            closest.map_or("-".to_string(), |(d, t, a, m)| format!("{d:.2} (arriving cart {a} to departing cart {m} at T+{t})"))
        ));
        stats.drop_closest = closest.map(|c| (c.0 * 100.0).floor() as i64);
        stats.drop_fall = (0..3).map(|k| landed[k].zip(drop_at).map(|(l, d)| l.0 - d)).collect::<Option<Vec<_>>>().and_then(|v| v.into_iter().max()).or(Some(i64::MAX));
        stats.drop_feet = (0..3).filter_map(|k| drop_feet[k]).map(|y| (y * 100.0).floor() as i64).min();
    } else {
    out.push(format!(
        "stand cycle (REL T+{}, G1 +{}, G2 +{}, slopes +{}; the hold T+{}, hand-offs {}:{} at {}; G0 {:?}, fill {:?}{}): shut windows {}",
        o.rel,
        o.g1_delay,
        o.g2_delay,
        o.slope_delay,
        o.hold,
        o.every,
        o.every2,
        o.speed,
        o.g0_key,
        o.fill_key,
        if o.qualified { ", qualified edges" } else { "" },
        wins.join("; ")
    ));
    }
    let cr = curves.report();
    out.push(format!("curve rule (follower entry minus leader clear, bar >= 0; no touching): {}", if cr.is_empty() { "no curve passed by two carts".to_string() } else { cr.join("; ") }));
    let pulses = |v: &[(i64, Option<i64>)]| v.iter().map(|(a, b)| format!("T+{a}..{}", b.map_or("-".to_string(), |b| format!("T+{b}")))).collect::<Vec<_>>().join(", ");
    out.push(format!(
        "D1 pulses {}{}{}{}",
        if d1_pulses.is_empty() { "none".to_string() } else { pulses(&d1_pulses) },
        if o.dd.is_some() { format!("; Dd pulses {}", if dd_pulses.is_empty() { "none".to_string() } else { pulses(&dd_pulses) }) } else { String::new() },
        if o.d2 { format!("; D2 pulses {}{}", if d2_pulses.is_empty() { "none".to_string() } else { pulses(&d2_pulses) }, match (arm_dt, last_dep_d2) { (Some(a), Some(l)) => format!(" (the arming at T+{a}, {} gt after the last departing D2 rise)", a - l), (Some(a), None) => format!(" (the arming at T+{a}, no departing D2 rise)"), _ => String::new() }) } else { String::new() },
        if ignored_falls.is_empty() { String::new() } else { format!("; not counted: {}", ignored_falls.join(", ")) }
    ));
    // (the j-th present arriving cart in pocket P(j+1))
    let wrong: Vec<String> = present
        .iter()
        .enumerate()
        .filter_map(|(j, &k)| {
            let want = if o.drop.is_some() { DROP_POCKETS[j].0 as f64 + 0.5 } else { [13.885, 9.885, 5.885][j] };
            match st_final(&run.world, carts[3 + k]) {
                Some(p) if (p.x - want).abs() < 0.3 && (p.z - (SZ as f64 + 0.5)).abs() < 0.3 => None,
                Some(p) => Some(format!("arriving cart {k} at {:.2},{:.2},{:.2} (P{} expected)", p.x, p.y, p.z, j + 1)),
                None => Some(format!("arriving cart {k} gone")),
            }
        })
        .collect();
    let last = (0..3).filter(|k| !o.absent[3 + *k]).map(|k| rest_since[k].unwrap_or(i64::MAX)).max().unwrap_or(i64::MAX);
    let fmt = |a: &[Option<i64>; 3]| (0..3).map(|k| a[k].map_or("never".to_string(), |t| format!("T+{t}"))).collect::<Vec<_>>().join(", ");
    let last_above = (0..3).filter(|k| riders[*k].is_some() && !o.absent[*k]).map(|k| dep_above[k].unwrap_or(i64::MAX)).max();
    // (the next summon: the first T+100k in game time at which the bench's entities tick; a freeze
    // over T+100 skips that one, a freeze before it delays the carts but not the boundary)
    let boundary = (1..10i64).map(|k| 100 * k).find(|b| (*b as usize) >= frozen_hist.len() || !frozen_hist[*b as usize]).unwrap_or(100);
    out.push(format!(
        "departing: riders above feet {} at {} ({}); crest {}, top {}",
        o.above,
        fmt(&dep_above),
        match last_above {
            None => "no riders".to_string(),
            Some(i64::MAX) => "NOT ALL by the run's end".to_string(),
            Some(t) => format!("all by T+{t}, margin {} gt before the next summon T+{boundary}", boundary - t),
        },
        fmt(&dep_crest),
        fmt(&sunk)
    ));
    out.push(format!(
        "arriving: {}; batch at rest {}; gate edges {}{}; contacts {} (the fastest {:.3}){}; riders hurt {hurt}, unseated ticks {unseated}",
        if wrong.is_empty() { "every cart in its own pocket".to_string() } else { format!("WRONG POCKET: {}", wrong.join("; ")) },
        if last == i64::MAX { "NO".to_string() } else { format!("T+{last}, margin {} gt before the next summon T+{boundary}", boundary - last) },
        edges.len(),
        if edges.is_empty() { String::new() } else { format!(" ({})", edges.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        contacts.len(),
        contact_max,
        if contacts.is_empty() { String::new() } else { format!(" ({})", contacts.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    stats.edges = edges.len();
    stats.wrong = wrong.len();
    stats.contacts = contacts.len();
    stats.contact_max = contact_max;
    stats.climb = last_above.map(|t| if t == i64::MAX { i64::MIN } else { boundary - t });
    stats.rest = if last == i64::MAX { i64::MIN } else { boundary - last };
    let (cl, touch) = curves.least();
    stats.curve = if touch > 0 { Some(i64::MIN) } else { cl };
    stats.hurt = hurt;
    stats.unseated = unseated;
    stats.d2_arm_margin = match (arm_dt, last_dep_d2) {
        (Some(a), Some(l)) => Some(a - l),
        _ => None,
    };
    Ok((out, stats))
}

/// The grid's summary over many stand-cycle runs: per gate the least margins, the totals.
pub fn cycle_summary(label: &str, runs: &[CycleStats]) -> String {
    let mn = |f: &dyn Fn(&CycleStats) -> Option<i64>| runs.iter().filter_map(f).min();
    let show = |v: Option<i64>| match v {
        None => "-".to_string(),
        Some(i64::MIN) => "FAIL".to_string(),
        Some(x) => x.to_string(),
    };
    format!(
        "{label}: {} runs; least margins G0 after the leader {} / before the follower {}; G1 {} / {}; G2 {} / {}; gate edges {}, wrong pockets {}, contacts {} (the fastest {:.3}); departing riders over the height by the next summon: least {}; arriving at rest: least {}; curve rule least {}; riders hurt {}, unseated ticks {}; D2 arming after the last departing D2 rise: least {}{}",
        runs.len(),
        show(mn(&|s| s.left[0])),
        show(mn(&|s| s.right[0])),
        show(mn(&|s| s.left[1])),
        show(mn(&|s| s.right[1])),
        show(mn(&|s| s.left[2])),
        show(mn(&|s| s.right[2])),
        runs.iter().map(|s| s.edges).sum::<usize>(),
        runs.iter().map(|s| s.wrong).sum::<usize>(),
        runs.iter().map(|s| s.contacts).sum::<usize>(),
        runs.iter().map(|s| s.contact_max).fold(0.0, f64::max),
        show(mn(&|s| s.climb)),
        show(mn(&|s| Some(s.rest))),
        show(mn(&|s| s.curve)),
        runs.iter().map(|s| s.hurt).sum::<usize>(),
        runs.iter().map(|s| s.unseated).sum::<usize>(),
        show(mn(&|s| s.d2_arm_margin)),
        if runs.iter().any(|s| s.drop_fall.is_some()) {
            format!(
                "; drop: the closest arriving-to-departing {}, the longest fall {} gt, the lowest waiting rider's feet {}",
                mn(&|s| s.drop_closest).map_or("-".to_string(), |c| format!("{:.2}", c as f64 / 100.0)),
                runs.iter().filter_map(|s| s.drop_fall).max().map_or("-".to_string(), |f| if f == i64::MAX { "NEVER LANDED".to_string() } else { f.to_string() }),
                mn(&|s| s.drop_feet).map_or("-".to_string(), |c| format!("{:.2}", c as f64 / 100.0))
            )
        } else {
            String::new()
        }
    )
}

fn st_final(w: &redsim::world::World, id: u32) -> Option<Vec3> {
    w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| e.pos)
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
    /// Per-cart hand-off speeds (b/t), overriding the hand-off velocity's magnitude.
    pub hand_speeds: Option<[f64; 3]>,
    /// The stand's gates on timers from the hold's release (T + h_open) instead of position
    /// triggers: G2 shuts at release + .0, G1 at release + .1 (live time: paused while frozen).
    pub gate_shut: Option<(i64, i64)>,
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
                        // (the per-cart hand-off speeds: the velocity's direction at cart k's speed)
                        let vk = match o.hand_speeds {
                            Some(sp) => {
                                let n = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
                                if n > 0.0 { Vec3::new(v.x * sp[k] / n, v.y * sp[k] / n, v.z * sp[k] / n) } else { v }
                            }
                            None => v,
                        };
                        run.world.entities.list[i].delta_movement = vk;
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
        for (k, (gx, _)) in GATES.iter().rev().enumerate() {
            if k == 0 || !open[2 - k] || o.gate_shut.is_some() {
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
    /// The least margin over every curve passed by two or more carts, and the touching ticks.
    fn least(&self) -> (Option<i64>, usize) {
        let mut least: Option<i64> = None;
        for i in 0..self.cells.len() {
            let mut ps = self.passes[i].clone();
            ps.sort_by_key(|x| x.0);
            for w in ps.windows(2) {
                let m = w[1].0 - w[0].1;
                least = Some(least.map_or(m, |l| l.min(m)));
            }
        }
        (least, self.touches.len())
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
