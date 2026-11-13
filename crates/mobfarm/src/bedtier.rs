//! The bed stop (the napper's stop under the station: F1 + Q2, one bed, 24 villagers). A booster
//! entry and an always-on activator that drops every arrival on a ledge, two braked powered rails
//! down a slope (the park: every arrival stops on the first), seven glass-covered flats to the pickup cell
//! beside the bed's head, the curve east, a capped booster, the slope down, R. The rider walks the
//! ledge onto the bed's roof and lies down; woken through the hatch at F (the zombie seen), it can
//! stand only on the pickup cell, which the released cart sweeps.
//!
//! Normal operation only (vjeux 2026-11-12; ironspec 01:16): under L1‴ a restart cannot put a woken
//! napper back to bed before the cart reaches it (wake to pickup 55-75 gt, under SleepInBed's 100-gt
//! LAST_WOKEN guard, a saved memory), so the stop handles no strand. The spurs-and-kicks strand fix
//! (explore/farmopt-station 47bbde753..f8147b303) is the pearl alternative's, for chunks unloaded
//! while game time runs.
//!
//! The unit benches drive the controller's signals (F, the release, the re-arm, the queue's
//! release) and check the sensors and the napper: `bench` one visit, `two_visit_bench` a visit and
//! the next napper's from the queue.

use crate::layouts::Builder;
use anyhow::{bail, Result};
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, RunOpts, WalkMode};
use redsim::entity::Vec3;
use redsim::pos::BlockPos;
use redsim::world::World;

fn put(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("bed stop: {s} at {c:?} meets {old}");
    }
    b.set(c[0], c[1], c[2], s);
    Ok(())
}

/// The stop's local frame: loop = local + (0,-27,-1) (the stop 9 down, ironspec 12:23; onechunk's
/// module explore/farmopt-onechunk2 f52f712d7).
/// The entry (a booster, always on), heading west along z -4.
pub const ENTRY: [i32; 3] = [3, 2, -4];
/// The activator, always on; its south side is the landing.
pub const ACTIVATOR: [i32; 3] = [2, 2, -4];
/// The park: two braked powered rails down the slope west, on glass; their drives are levers north
/// of them ([`PARK_CTRL`]).
pub const PARK: [[i32; 3]; 2] = [[1, 1, -4], [0, 0, -4]];
/// The park rails' drives (A5): a lever north of each rail on glass (the bench's controller, the
/// real build's line end), powered at the release and back off at R.
pub const PARK_CTRL: [[i32; 3]; 2] = [[1, 1, -5], [0, 0, -5]];
/// The one cell a woken napper can stand on: the pickup cell.
pub const SWEPT: [[i32; 3]; 1] = [[-2, 0, 4]];
pub const BED_HEAD: [i32; 3] = [-3, 0, 4];
/// The hatch: two iron trapdoors in the zombie box's one opening west of the bed's head (bottom
/// half (-4,0,4), top half (-4,1,4)), no floor under them; open (their levers on) their plates cut
/// the eye lines; shut (F) the sleeper's and a napper's on the pickup cell pass (a napper standing
/// on the bed has its line under the top one's slab: nothing needs it to see the waker since HOLD
/// went, ironspec 10:53).
pub const HATCH: [i32; 3] = [-4, 0, 4];
/// The zombie's post: one glass-cased cell west of the hatch; its boat at the cell's centre.
pub const ZOMBIE_POST: [i32; 3] = [-5, 0, 4];
pub const HATCH_HI: [i32; 3] = [-4, 1, 4];
/// The hatch's levers (the bench's controllers, the real build's F line ends, on glass): north of the
/// bottom trapdoor, and south of the top one (in the post case's wall).
pub const HATCH_CTRL: [i32; 3] = [-4, 0, 3];
pub const HATCH_CTRL_HI: [i32; 3] = [-4, 1, 5];
/// R's detector (loop (4,-28,4)), heading east.
pub const EXIT_DET: [i32; 3] = [4, -1, 5];
/// The two-visit bench's queue cell: the old corner queue's (loop (9,-8,-6) at the old offset,
/// 9fc3155f's budget 28b1bbd37e), 17.7 from the bed's head; the next napper waits there awake in its
/// cart (the bench design carries its rail). The stop-9-down queue is b8949c96's way-in: the
/// two-visit rows on it wait for its lay on onechunk's module and 7b90c3f1's lift.
pub const QUEUE: [i32; 3] = [9, 10, -5];
/// (A6, ironspec 10:40) The cell over the bed's head: the bed column's first non-solid cell, so the
/// HOME path's target (`findSurfacePosition`). The walk's last step drops the napper into it (a
/// complete path: the walk never arms CANT_REACH), and it lies down on the way (REST) or lands and
/// waits standing on the bed (IDLE: 0 from its bed, no walk target at REST, it lies down at once).
pub const BED_TOP: [i32; 3] = [-3, 1, 4];
/// (ironspec 15:58) The second landing, a chest south of the entry (the trapdoor held open over it,
/// the pathfinder's way in from the approach), and the trapdoor's lever on the ceiling over it.
pub const LANDING2: [i32; 3] = [3, 2, -3];
pub const LANDING2_CTRL: [i32; 3] = [3, 4, -3];
/// The activator's power (always on since A4: a lever on glass in the bench; the 09:00 day-gate
/// control turns it off by day).
pub const ACT_CTRL: [i32; 3] = [2, 2, -5];
/// T5b's activator lever (ironspec 22:20, the restart trap's fix): the activator in the entry cell,
/// held on by a floor lever north of it (where the entry booster's torch was).
pub const ACT_CTRL_T5B: [i32; 3] = [3, 2, -5];
/// (A) (ironspec 00:07 (4A), approved for benching 00:37): the eject within SleepInBed's reach. The
/// flats along x -2 raised to y 1; the activator E over the bed foot's north-east corner: a
/// southbound cart's dismount, its W, E, NW and NE blocked, sets the rider on the bed foot's top (its
/// SW), 1.00 from the bed head's centre, where SleepInBed fires at once whatever path it carries.
pub const E_A: [i32; 3] = [-2, 1, 2];
/// (A)'s activator lever: on glass east of E, glass over it (the dismount's E blocked).
pub const ACT_CTRL_A: [i32; 3] = [-1, 1, 2];
/// (A)'s park: one braked powered rail ascending north (its high end at E), the pickup cell P south
/// of it; its release lever on glass east of it.
pub const PARK_A: [i32; 3] = [-2, 0, 3];
pub const PARK_CTRL_A: [i32; 3] = [-1, 0, 3];
/// (A)'s arming detector, the flat before E: this cart's passage arms the gate (F waits for it plus
/// the worst-case braking interval, ironspec 00:37 (2b)).
pub const ARM_A: [i32; 3] = [-2, 1, 1];
/// (A) with the run-up (B3, ironspec 02:27): E at the flats' own level (-2,0,2), its lever hung from
/// glass over it; after E the curve east (-2,0,3), a booster, a climb, the U-turn at y 1, the park
/// (1,0,4) one braked powered rail descending west, two run-up flats along z 4 into P (-2,0,4), a
/// curve turning south onto A6's exit curve. The run-up's friction brings the released cart to P
/// under 0.2 b/t (the 0.2-step clip).
pub const E_A2: [i32; 3] = [-2, 0, 2];
/// (A) with the run-up's activator power (ironspec 03:22, no block a wind burst can change): a
/// redstone torch on glass west of E (the glass under it never powered), glass over it (the
/// dismount's W at E's level a 1-high cell); always on, no off state. Its placement updates no
/// plain rail with exactly 3 potential connections (ironspec 03:26): east of E it updated the
/// loop's flat (-1,0,3).
pub const ACT_CTRL_A2: [i32; 3] = [-3, 0, 2];
pub const ARM_A2: [i32; 3] = [-2, 0, 1];
pub const PARK_A2: [i32; 3] = [1, 0, 4];
/// (the park's drive: day-1's repeater output east of the U-turn's support, which is a redstone
/// conductor (smooth stone) powering the park rail west of it; the bench's stand-in a wall lever
/// there attached to the conductor. Its updates and the conductor's reach the park and the U-turn's
/// curve (2 connections) only: a drive into the park's support updated the exit slope (1,-1,5), a
/// rail with exactly 3 potential connections, from a signal-source block (ironspec 03:26))
pub const PARK_CTRL_A2: [i32; 3] = [3, 0, 4];

/// The stop's variant cells in a built world: (A) when E holds an activator rail, T5b when the
/// entry does, else A6.
#[derive(Debug, Clone)]
pub struct Layout {
    pub name: &'static str,
    pub act: [i32; 3],
    pub act_ctrl: [i32; 3],
    pub park_ctrl: Vec<[i32; 3]>,
    pub arm: Option<[i32; 3]>,
}

pub fn layout(w: &World) -> Layout {
    let at = |c: [i32; 3]| format!("{}", w.get(BlockPos::new(c[0], c[1], c[2])));
    if at(E_A2).contains("activator_rail") {
        Layout { name: "(A) run-up", act: E_A2, act_ctrl: ACT_CTRL_A2, park_ctrl: vec![PARK_CTRL_A2], arm: Some(ARM_A2) }
    } else if at(E_A).contains("activator_rail") {
        Layout { name: "(A)", act: E_A, act_ctrl: ACT_CTRL_A, park_ctrl: vec![PARK_CTRL_A], arm: Some(ARM_A) }
    } else if at(ENTRY).contains("activator_rail") {
        Layout { name: "T5b", act: ENTRY, act_ctrl: ACT_CTRL_T5B, park_ctrl: PARK_CTRL.to_vec(), arm: None }
    } else {
        Layout { name: "A6", act: ACTIVATOR, act_ctrl: ACT_CTRL, park_ctrl: PARK_CTRL.to_vec(), arm: None }
    }
}

/// The activator's cell and its lever in the built world: T5b's (the entry cell's activator) when
/// the entry holds an activator rail, else the stop's (the activator west of the entry booster).
pub fn act_cells(w: &World) -> ([i32; 3], [i32; 3]) {
    let l = layout(w);
    (l.act, l.act_ctrl)
}

/// The design (`bench`: with a run-out past R and the queue cell's rail; `trapdoor_caps`: the
/// approach flats covered by top-half trapdoors, the contact mode's positive control;
/// `eject_at_entry`: T5b, the restart trap's fix candidate; `curve_glass`: the wall east of the
/// pickup cell glass as before the curve fix (the contact's positive control).
pub fn design(bench: bool, trapdoor_caps: bool, eject_at_entry: bool, curve_glass: bool) -> Result<String> {
    let mut b = Builder::new(if bench { "bed-tier-fold-bench" } else { "bed-tier-fold" });
    let rail = |s: &str| format!("rail[shape={s},waterlogged=false]");
    let cap = "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]";
    let wall = |b: &mut Builder, c: [i32; 3]| -> Result<()> {
        if !b.d.blocks.contains_key(&c) {
            put(b, c, "glass")?;
        }
        Ok(())
    };
    // the arrival, west along z -4: the entry a booster (always on), the activator (always on),
    // both capped (top-half trapdoors: a rider passes, nobody stands there). (A5, the IDLE statics:
    // no solid-render block anywhere in the stop, so no stroll candidate: every rail on glass, the
    // boosters lit by torches beside them, the activator, the park, the hatch and the pocket floor
    // driven by levers (the bench's controllers, the real build's line ends) on glass)
    put(&mut b, ENTRY, "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [ENTRY[0], 1, ENTRY[2]], "glass")?;
    put(&mut b, [ENTRY[0], 2, ENTRY[2] - 1], "redstone_torch[lit=true]")?;
    put(&mut b, [ENTRY[0], 1, ENTRY[2] - 1], "glass")?;
    put(&mut b, ACTIVATOR, "activator_rail[powered=true,shape=east_west,waterlogged=false]")?;
    // (the activator's lever on its north side, not under it: under it, it would sit beside the
    // park rail and power it; the glass over it keeps the north side out of the dismount's choices
    // (1 of headroom))
    put(&mut b, [ACTIVATOR[0], 1, ACTIVATOR[2]], "glass")?;
    put(&mut b, ACT_CTRL, &ctl_block(ACT_CTRL, "redstone_block"))?;
    put(&mut b, [ACT_CTRL[0], 1, ACT_CTRL[2]], "glass")?;
    if eject_at_entry {
        // (T5b, ironspec 22:11 / 22:20, the restart trap's fix: the eject at the entry cell onto
        // the second chest. The activator in the entry's cell, held on by a floor lever on the
        // glass north of it (day-1's activator line end, where the booster's torch was); a plain
        // rail where the activator was (no booster after the eject: a powered rail beside the park
        // carries its power into the park's braked rails); glass over the lever's cell so the
        // dismount's first choice (north) is blocked and its second (south) is the second chest. A
        // path computed in the entry cell has the second chest's top as its node 1: from the
        // landing, shouldTargetNextNodeInDirection skips the entry node (dot -0.66). The old
        // activator lever and its glass go)
        b.d.blocks.remove(&ENTRY);
        b.d.blocks.remove(&ACT_CTRL_T5B);
        b.d.blocks.remove(&ACTIVATOR);
        put(&mut b, ENTRY, "activator_rail[powered=true,shape=east_west,waterlogged=false]")?;
        put(&mut b, ACT_CTRL_T5B, &ctl_block(ACT_CTRL_T5B, "redstone_block"))?;
        put(&mut b, [ACT_CTRL_T5B[0], 3, ACT_CTRL_T5B[2]], "glass")?;
        put(&mut b, ACTIVATOR, &rail("east_west"))?;
        b.d.blocks.remove(&ACT_CTRL);
        b.d.blocks.remove(&[ACT_CTRL[0], 1, ACT_CTRL[2]]);
    }
    // (golem-proofing, ironspec 04:33: a closed top-half trapdoor is a golem floor (a legacy-solid
    // top). Neither the entry nor the activator is capped since the chest landing: the rider's HOME
    // path, computed while it rides, starts from the first cell over the rider that is not air or
    // passable (`getStart` for a mob off the ground): under a cap that is the cap's cell, a 1-node
    // path, partial (the stamp armed), and after the eject the rider walked back toward that node
    // and stood 120-240 gt against the cap. Uncapped, the path starts on the rail. The entry is open
    // above (the path from the approach passes over it into the second landing's trapdoor cell))
    put(&mut b, [ACTIVATOR[0], 3, ACTIVATOR[2] - 1], "glass")?;
    // the park, two braked powered rails down the slope (every arrival stops on the first; the
    // release powers both: the full second cell's boost narrows the spread of release speeds that
    // the first cell's stop position leaves), a capped flat, the curve south
    for (p, c) in PARK.iter().zip(PARK_CTRL.iter()) {
        put(&mut b, *p, "powered_rail[powered=false,shape=ascending_east,waterlogged=false]")?;
        put(&mut b, [p[0], p[1] - 1, p[2]], "glass")?;
        put(&mut b, *c, &ctl_block(*c, "stone"))?;
        put(&mut b, [c[0], c[1] - 1, c[2]], "glass")?;
    }
    let za = ENTRY[2];
    // (the flat after the park keeps its top-half trapdoor: the cart coming off the park's slope
    // still rides high (y 0.55) as its box enters the flat's cell, and a full block at y 1 there
    // stops it; a floor stone button over the trapdoor fills the golem's spawn cell instead. The
    // curve's cover is glass: the cart is down at y 0.06 by then, no rider past the activator)
    put(&mut b, [-1, 0, za], &rail("east_west"))?;
    put(&mut b, [-1, -1, za], "glass")?;
    put(&mut b, [-1, 1, za], cap)?;
    put(&mut b, [-1, 2, za], "stone_button[face=floor,facing=north,powered=false]")?;
    put(&mut b, [-2, 0, za], &rail("south_east"))?;
    put(&mut b, [-2, -1, za], "glass")?;
    put(&mut b, [-2, 1, za], "glass")?;
    // (glass two cells over the park: no villager stands on the parked cart's top)
    put(&mut b, [1, 3, za], "glass")?;
    // seven glass-covered flats down x -2 to the pickup cell (the bed's head west of it): the released
    // cart slows on its 0.96 empty drag to under 0.2 b/t there (a cart faster than that stops dead
    // against a standing villager)
    for z in za + 1..=SWEPT[0][2] {
        put(&mut b, [-2, 0, z], &rail("north_south"))?;
        put(&mut b, [-2, -1, z], "glass")?;
        // (the covers (farmopt 13:44: the cheapest set meeting the bar): z 0..2 carry the walk, z 3
        // keeps the flat north of the pickup cell 1 high (no stand-up cell on the approach); z -3..-1
        // under the walk's north wall serve nothing and are gone (no villager reaches those flats on
        // foot: the pickup cell's way north is 1 high). `trapdoor_caps`: top-half trapdoors over all
        // seven, the contact mode's control)
        if trapdoor_caps && z != SWEPT[0][2] {
            put(&mut b, [-2, 1, z], cap)?;
        } else if (0..SWEPT[0][2]).contains(&z) {
            put(&mut b, [-2, 1, z], "glass")?;
        }
    }
    // (the flats' covers are glass, full blocks at y 1, over the empty cart (its box tops out at y
    // 0.76; it carries no rider past the activator), not top-half trapdoors: the pathfinder takes
    // a trapdoor cell for passable, so a panicking napper on the pickup cell (on a rail, it may path
    // along rails) could take a flee path north up the approach, toward the arriving cart, and press
    // into it: after a restart its rebuilt brain draws a new flee target, and the cart stopped dead
    // against it (G1/ironspec's contact mode). With glass there is no path north; its flee path
    // runs south, away from the cart. (-2,1,3) is also the ledge's floor under (-2,2,3). `trapdoor_caps`:
    // the old covers, the positive control)
    // the curve east (capped), a capped booster (always on: a timing part, the exit's speed), a
    // capped flat, the slope down, two flats, R
    put(&mut b, [-2, 0, 5], &rail("north_east"))?;
    put(&mut b, [-2, -1, 5], "glass")?;
    put(&mut b, [-2, 1, 5], cap)?;
    put(&mut b, [-1, 0, 5], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [-1, -1, 5], "glass")?;
    put(&mut b, [-1, 0, 6], "redstone_torch[lit=true]")?;
    put(&mut b, [-1, -1, 6], "glass")?;
    put(&mut b, [-1, 1, 5], cap)?;
    put(&mut b, [0, 0, 5], &rail("east_west"))?;
    put(&mut b, [0, -1, 5], "glass")?;
    put(&mut b, [0, 1, 5], cap)?;
    // (golem-proofing: the rider's box reaches the cells over these caps, so a floor stone button on
    // each fills the spawn cell (the floor's cell above must be air or liquid; the button is no golem
    // floor, has no collision box and, unpressed, powers nothing; ironspec's carpet, which redsim
    // doesn't model, does the same). Over the curve's cap (-2,1,5) the pocket's south tripwire hook
    // fills it)
    for x in -1..=0 {
        put(&mut b, [x, 2, 5], "stone_button[face=floor,facing=north,powered=false]")?;
    }
    put(&mut b, [1, -1, 5], &rail("ascending_west"))?;
    put(&mut b, [1, -2, 5], "glass")?;
    for x in 2..=3 {
        put(&mut b, [x, -1, 5], &rail("east_west"))?;
        put(&mut b, [x, -2, 5], "glass")?;
    }
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [EXIT_DET[0], -2, EXIT_DET[2]], "glass")?;
    // the bed, its head beside the pickup cell, its foot north. The foot is roofed by a top-half
    // trapdoor (0.25 of headroom over the bed's top: nobody stands on the foot); the head is open
    // above (A6): the walk's last step drops the napper into the column over it
    put(&mut b, BED_HEAD, "red_bed[facing=south,occupied=false,part=head]")?;
    put(&mut b, [-3, 0, 3], "red_bed[facing=south,occupied=false,part=foot]")?;
    put(&mut b, [-3, -1, 3], "glass")?;
    put(&mut b, [-3, -1, 4], "glass")?;
    put(&mut b, [-3, 1, 3], cap)?;
    // (golem-proofing: the foot's roof is a golem floor and the walk crosses the cell over it, so a
    // floor stone button fills that cell (no collision box, unpressed: no power))
    put(&mut b, [-3, 2, 3], "stone_button[face=floor,facing=north,powered=false]")?;
    // (the drop column over the bed's head (A6): (-3,1..3,4) open, the bed column's first non-solid
    // cell (-3,1,4) the HOME path's target; glass round it at y 2..3 (south (-3,2..3,5), west
    // (-4,2..3,4), east (-2,2..3,4) over the pickup cell's head room, which stays open at y 1) and over
    // it (-3,4,4); north the walk's last cell over the bed's foot)
    for c in [[-3, 2, 5], [-3, 3, 5], [-4, 3, 4], [-2, 2, 4], [-2, 3, 4], [-3, 4, 4]] {
        put(&mut b, c, "glass")?;
    }
    // (golem-proofing: the curve's cap (-2,1,5) is a golem floor; glass fills the cell over it)
    put(&mut b, [-2, 2, 5], "glass")?;
    // the hatch west of the bed's head and the zombie box behind it (one cell, x -5; the west wall
    // on x -6): two iron trapdoors, open (powered by the levers at (-4,0,3) and (-4,1,5)) their
    // plates across the cells' east sides, cutting every eye line; shut (F) the sleeper's eye line
    // and a standing napper's on the pickup cell pass under the top one's slab. The zombie
    // (persistent) in a boat at its post cell's centre, glass all round, leaves over it
    put(&mut b, HATCH, "iron_trapdoor[facing=west,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_HI, "iron_trapdoor[facing=west,half=top,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_CTRL, &ctl_block(HATCH_CTRL, "redstone_block"))?;
    put(&mut b, [HATCH_CTRL[0], -1, HATCH_CTRL[2]], "glass")?;
    put(&mut b, HATCH_CTRL_HI, &ctl_block(HATCH_CTRL_HI, "redstone_block"))?;
    // (no floor under the hatch cell (G1 on 941c5aa2b): a wake with the hatch open (a dawn wake, or a
    // lie-down in day time 0..9 after a night skip, the wake at 10 before F) stood the napper up in
    // the hatch cell when its yaw put that side first (`findStandUpPosition` treats an open
    // trapdoor as empty), and the zombie's boat, reaching 0.19 into the cell plus its 0.2 pickup
    // reach, seated it. With no floor the cell is never a safe stand-up cell, so the napper stands up
    // on the pickup cell whatever the hatch and its yaw)
    put(&mut b, [-5, -1, 4], "glass")?;
    put(&mut b, [-5, 2, 4], "glass")?;
    for y in 0..=1 {
        put(&mut b, [-5, y, 3], "glass")?;
        put(&mut b, [-5, y, 5], "glass")?;
        put(&mut b, [-6, y, 4], "glass")?;
    }
    put(&mut b, [-4, 0, 5], "glass")?;
    put(&mut b, [-4, 1, 3], "glass")?;
    // (the zombie's cover: persistent leaves at distance 7, as the build leaves them with no log near)
    put(&mut b, [-5, 3, 4], "oak_leaves[distance=7,persistent=true,waterlogged=false]")?;
    // (the leaves are no golem floor: SpawnUtil's LEGACY_IRON_GOLEM excludes LeavesBlock (G1 on
    // b36302af8; redsim's `golem_excluded`); the first A6 lay's flood failures came from the spawn
    // cell over the bed foot's roof, now a button, and the flood is clean with the leaves open above)
    // (the boat at the post cell's centre (-4.5, 0, 4.5): cbcac083a's (-5.0, 0, 4.5) was the cell's
    // west edge, x written without its + 0.5)
    b.entity("oak_boat", [ZOMBIE_POST[0] as f64 + 0.5, ZOMBIE_POST[1] as f64, ZOMBIE_POST[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    // the pit: glass north of the bed's foot, south of its head, east of the flats and the pickup
    // cell (x -1, z 0..4: the ledge's floor at y 1; z 5 is the exit's booster and its cap)
    for y in 0..=1 {
        put(&mut b, [-3, y, 2], "glass")?;
        wall(&mut b, [-3, y, 5])?;
        for z in 0..=5 {
            wall(&mut b, [-1, y, z])?;
        }
    }
    if !curve_glass {
        // (ironspec 23:19 (4b), the curve contact; ADOPTED 00:32 (2): a cart picking up near 0.154
        // b/t caught this wall's glass (x -1, the cell east of the pickup cell) with its box's east
        // face as it turned the curve (-2,0,5) east (0.148 -> 0.104 b/t; 14,400 of the band sweep's
        // 194,400 visits). A closed top-half iron trapdoor there (its collision 0.8125..1.0, above the
        // cart's top at 0.7625) still blocks the napper on the pickup cell (its box 0..1.95), with
        // glass over it (no golem spawn space on its top); its stored state pinned (half=top,
        // open=false, powered=false: nothing powers it). `curve_glass`: the pre-fix glass, the
        // contact's positive control)
        b.d.blocks.remove(&[-1, 0, 4]);
        put(&mut b, [-1, 0, 4], "iron_trapdoor[facing=west,half=top,open=false,powered=false,waterlogged=false]")?;
    }
    // the landing L (2,2,-3) south of the activator (its north side under glass at 1 of headroom: the
    // dismount's first choice, blocked), the ledge (A6, all at y 2 (loop -16): out of the stroll
    // band of the pad's portal cells at loop -8): south to (2,2,0), west along z 0 to (-2,2,0) over
    // the approach's glass covers, south to (-2,2,2), west to (-3,2,2), south onto the bed's foot
    // roof (-3,2,3) (1.80 from the bed head's centre: it may lie down there), south into the column
    // over the bed's head: a 1.44 drop onto the bed with no way back up, the HOME path's target,
    // reached. Walls two high round every cell (the parked cart's top and the caps' tops lead
    // nowhere)
    // (the ledge's floor is glass: no golem floor, and no stable stroll destination (a stroll's
    // candidate needs a solid-render block under it), so a napper idling on the ledge draws no
    // unreachable target there)
    put(&mut b, [2, 1, -3], "glass")?;
    // (the landing's floor a chest (collision 14/16 high, not a full cube: no solid-render block, no
    // stroll candidate; nobody opens it): the activator's dismount sets the rider down on it at y
    // 2.875, above the cart's box (its top 2.7625 on the activator), so the moving cart never
    // touches it. A restart in the dismount's first gt clears the rider's 60-gt boarding cooldown
    // (not saved), and on a glass floor the rider stood level with the cart, 0.01 outside its
    // pickup reach (its box inflated 0.2 sideways): any drift north re-seated it, and the parked
    // cart kept it (160 of 21,600 restart rows at entry + 5, entries 0.4 b/t and up). Glass three
    // over it: no golem fits on the chest (its box would run up through y 5))
    put(&mut b, LEDGE[0], "chest[facing=south,type=single,waterlogged=false]")?;
    put(&mut b, [LEDGE[0][0], LEDGE[0][1] + 3, LEDGE[0][2]], "glass")?;
    // (the second landing (ironspec 15:58, the way-in's rail descent): a calm REST HOME path that
    // starts on an approach rail can't pass the activator's cell (1 high: the landing's north wall
    // (2,3,-4) keeps a napper off the arrival rails and the parked cart), so it ended partial and
    // armed a stamp on the way in. A chest south of the entry and an iron trapdoor held open in the
    // cell over it, its plate on that cell's north side: the pathfinder takes the trapdoor's cell for
    // passable, so the path runs from the entry onto the chests and down the ledge (complete); the
    // plate stops a napper on the chests walking north onto the entry and back up the approach. Its
    // lever on the ceiling over it (on glass), the chests' east side glass to y 4)
    put(&mut b, LANDING2, "chest[facing=south,type=single,waterlogged=false]")?;
    put(&mut b, [LANDING2[0], LANDING2[1] + 1, LANDING2[2]], "iron_trapdoor[facing=south,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, LANDING2_CTRL, "lever[face=ceiling,facing=north,powered=true]")?;
    put(&mut b, [LANDING2_CTRL[0], LANDING2_CTRL[1] + 1, LANDING2_CTRL[2]], "glass")?;
    for y in 2..=4 {
        put(&mut b, [LANDING2[0] + 1, y, LANDING2[2]], "glass")?;
    }
    put(&mut b, [2, 1, -2], "glass")?;
    put(&mut b, [2, 1, -1], "glass")?;
    put(&mut b, [2, 1, 0], "glass")?;
    put(&mut b, [1, 1, 0], "glass")?;
    put(&mut b, [0, 1, 0], "glass")?;
    let nodes: [[i32; 3]; 12] = LEDGE;
    // (cells that stay open: every cell of the walk and its head cell, the column over the bed's
    // head, the activator, the gate's lever)
    let mut open: Vec<[i32; 3]> = nodes.iter().flat_map(|c| [*c, [c[0], c[1] + 1, c[2]]]).collect();
    open.extend([[-3, 2, 4], [-3, 3, 4], BED_TOP, ACTIVATOR]);
    for c in nodes.iter().chain([[-3, 2, 4]].iter()) {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in 0..=1 {
                let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                if !open.contains(&n) {
                    wall(&mut b, n)?;
                }
            }
        }
    }
    if bench {
        // the run-out past R and a stop; the queue cell's rail (the two-visit bench's)
        for x in EXIT_DET[0] + 1..=EXIT_DET[0] + 6 {
            put(&mut b, [x, -1, EXIT_DET[2]], &rail("east_west"))?;
            put(&mut b, [x, -2, EXIT_DET[2]], "glass")?;
        }
        put(&mut b, [EXIT_DET[0] + 7, -1, EXIT_DET[2]], "glass")?;
        put(&mut b, QUEUE, &rail("east_west"))?;
        put(&mut b, [QUEUE[0], QUEUE[1] - 1, QUEUE[2]], "glass")?;
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    // (ironspec 09:31/10:40: the bed's column kept clear for the assembly check: (-3,1,4), the cell
    // over the bed, is the bed column's first non-solid cell, so the HOME path's target
    // (`findSurfacePosition`), and the walk's drop runs down (-3,3,4) and (-3,2,4) into it; a block
    // in any of them moves the target or blocks the drop. ironspec 11:43: the leaves over the
    // zombie's post (-5,3,4) keep its eye's sky light at 14 or less (no burning by day): a block
    // that must stay, its own line (G1 on b36302af8: keep-clear means the cell holds no block))
    b.d.meta.insert("keep-clear".into(), "-3,1,4 -3,2,4 -3,3,4".into());
    b.d.meta.insert("keep-block".into(), "-5,3,4=oak_leaves[distance=7,persistent=true] (the zombie's cover: its eye's sky light 14 or less, no burning by day)".into());
    // (onechunk 14:23: standable cells no napper can reach, left out of its verify's on-foot set)
    b.d.meta.insert(
        "unreachable".into(),
        "-4,1,5=the top hatch's lever cell: glass east (-3,1,5), west (-5,1,5) and under it (-4,0,5), the hatch's top trapdoor north (no floor under the hatch); the only napper cell touching it is the bed top (-3,1,4), diagonally, both orthogonals blocked (the trapdoor, the glass (-3,1,5)); flee-check from 43 places: no draw on it; no bench visit ever stood there (100,000+ visits)".into(),
    );
    // (ironspec/farmopt 11:01: the waker's in-sight places, declared: every place a napper has the
    // zombie in sight within 8 once F shows it, from the bench's sight log (none before F in any
    // row); `x,y,z+...=REASON`, the least eye distance in brackets)
    b.d.meta.insert(
        "waker-places".into(),
        "-3,0,4=asleep in the bed, woken by F (2.04); -2,0,4=standing up on the pickup cell and seated there (3.03); -2,0,5+0,0,5+1,0,5+1,-1,5+2,-1,5=riding out past the hatch to R (3.08..6.66)".into(),
    );
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    let mut lines: Vec<String> = [
        "The bed stop (local frame; under the station, the stop 9 down: loop = local + (0,-27,-1); the bed's head at (-3,0,4)): a booster entry and an activator heading west (always on, both open above), two braked park rails down a slope, a capped flat, a curve south, seven flats (glass over the last four), the pickup cell beside the bed's head, the curve east, a capped booster, a capped flat, the slope down, two flats, R. No solid-render block anywhere in the stop (rails, torches and levers on glass; the landing a chest): no IDLE stroll candidate.",
        "The activator sets the rider down onto the chest south of it (the landing; a second chest south of the entry, an iron trapdoor held open over it (the pathfinder's way in from the approach, its plate keeping nappers off the entry)); it walks the glass ledge round over the approach's covers onto the bed's foot roof and steps south into the open column over the bed's head: a 1.44 drop onto the bed, the bed column's first non-solid cell (-3,1,4), so the HOME path's target, reached (the walk arms no CANT_REACH stamp). It lies down on the way (REST) or waits standing on the bed (IDLE) until REST. The hatch's two iron trapdoors stand open (their levers on) and hide the zombie; F (the lie-down + 2, day-1's controller since ironspec 19:33) SHUTS them (their levers off): the zombie wakes it, it stands up on the pickup cell (its only stand-up cell), the park releases at the wake edge (+ 0, ironspec 19:18), and the cart seats it (at 0.143..0.207 b/t in every bench row) and runs on to R, whose rise opens the hatch again and resets the park's release (its levers off: the park braked for the next arrival; ironspec 22:56 (ii)). Precondition and single release (ironspec 22:58 (1)): one cart per visit (the queue's (a2) guard: RS_ready = R's rise + 8, never a second napper before the current one's R), and the park released only at the wake edge (nothing else releases it: no abort; a napper that never sleeps keeps its cart parked until its nap). Operating entry band [0.11, 0.24] (ironspec 19:31, widened 21:25). Keep (-3,1,4), (-3,2,4) and (-3,3,4) clear; the persistent leaves at (-5,3,4) must stay (keep-block: the zombie's cover).",
        "switch: -4,0,3=the hatch's lower lever (day-1's line end: F shuts, R opens); -4,1,5=the hatch's upper lever (the same line); 2,2,-5=the activator's lever (day-1's line end, held on); 1,1,-5=the park's upper lever (day-1's line end: the release); 0,0,-5=the park's lower lever (the same line); 3,4,-3=the second landing trapdoor's ceiling lever (held on; its state not load-bearing); -1,2,-4=a golem-proofing stone button over the flat's cap (never pressed); -1,2,5=a golem-proofing stone button over the exit booster's cap (never pressed); 0,2,5=a golem-proofing stone button over the exit flat's cap (never pressed); -3,2,3=a golem-proofing stone button over the bed foot's roof (never pressed)",
        "always-on: 3,2,-5=the entry booster's redstone torch, on glass (3,1,-5): it never toggles; -1,0,6=the exit booster's redstone torch, on glass (-1,-1,6): it never toggles",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if eject_at_entry {
        // (T5b's header: the same stop with the eject in the entry cell)
        lines[0] = lines[0].replace(
            "a booster entry and an activator heading west (always on, both open above)",
            "an activator entry heading west (T5b, ironspec 22:20, the restart trap's fix: the eject in the entry cell; always on, open above), a plain rail (no booster: a powered rail beside the park would power its brakes)",
        );
        lines[1] = lines[1].replace(
            "The activator sets the rider down onto the chest south of it (the landing; a second chest south of the entry, an iron trapdoor held open over it (the pathfinder's way in from the approach, its plate keeping nappers off the entry)); it walks",
            "The activator, in the entry's cell, sets the rider down onto the second chest south of it (the landing, under an iron trapdoor held open: the pathfinder's way in from the approach, its plate keeping nappers off the entry; a riding path computed in the entry cell has this chest's top as its node 1, so the napper skips the entry node from here); it steps west onto the first chest and walks",
        );
        lines[2] = lines[2].replace("2,2,-5=the activator's lever (day-1's line end, held on)", "3,2,-5=the activator's lever (day-1's line end, held on; T5b: where the entry booster's torch was)");
        lines[3] = "always-on: -1,0,6=the exit booster's redstone torch, on glass (-1,-1,6): it never toggles".to_string();
    }
    if !curve_glass {
        lines[0].push_str(" The wall east of the pickup cell (-1,0,4) is a closed top-half iron trapdoor, iron_trapdoor[facing=west,half=top,open=false,powered=false,waterlogged=false] (nothing powers it), above the cart's top (the curve contact's fix, ironspec 23:19 (4b), adopted 00:32).");
    }
    // (onechunk's stranded-napper bar in f1q2-run, ironspec 00:37 (2b) / 00:39: P's cell and the
    // bound, read from this line)
    lines.push("a6-pickup: -2,0,4=the pickup cell (the napper's only stand-up cell); stranded bound: no cart at it by the wake edge + 60 gt fails (the cart reaches it 36..40 gt after the release in every bench row)".to_string());
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    Ok(b.text(&refs))
}

/// (A) (ironspec 00:07 (4A), approved for benching 00:37): the stop with the eject within
/// SleepInBed's reach of the bed head. The arrival comes down one slope to y 1 and runs the flats
/// along x -2 to the activator E (-2,1,2) over the bed foot's north-east corner: the rider is set
/// down on the bed foot's top (the dismount's SW; W, E, NW, NE blocked), 1.00 from the bed head's
/// centre, and lies down at once whatever path it carries (startSleeping erases WALK_TARGET and
/// CANT_REACH). The empty cart rolls down the park's slope (-2,0,3) and brakes there, one cell
/// north of the pickup cell P (-2,0,4); the release (the wake edge, F gated on the arming detector
/// (-2,1,1) plus the braking interval) rolls it into P. The landing chests, the ledge and the drop
/// column go; the bed, the hatch, the zombie post, P and the exit are A6's.
pub fn design_a(bench: bool) -> Result<String> {
    let mut b = Builder::new(if bench { "bed-tier-a-bench" } else { "bed-tier-a" });
    let rail = |s: &str| format!("rail[shape={s},waterlogged=false]");
    let cap = "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]";
    let wall = |b: &mut Builder, c: [i32; 3]| -> Result<()> {
        if !b.d.blocks.contains_key(&c) {
            put(b, c, "glass")?;
        }
        Ok(())
    };
    // the arrival at y 2 west along z -4: two plain rails, one slope down to y 1, two flats, the
    // curve south, the flats down x -2 to the arming detector and E (every rail on glass: no
    // solid-render block in the stop, no IDLE stroll candidate)
    put(&mut b, ENTRY, &rail("east_west"))?;
    put(&mut b, [ENTRY[0], 1, ENTRY[2]], "glass")?;
    put(&mut b, ACTIVATOR, &rail("east_west"))?;
    put(&mut b, [ACTIVATOR[0], 1, ACTIVATOR[2]], "glass")?;
    put(&mut b, [1, 1, -4], &rail("ascending_east"))?;
    put(&mut b, [1, 0, -4], "glass")?;
    for x in -1..=0 {
        put(&mut b, [x, 1, -4], &rail("east_west"))?;
        put(&mut b, [x, 0, -4], "glass")?;
    }
    put(&mut b, [-2, 1, -4], &rail("south_east"))?;
    put(&mut b, [-2, 0, -4], "glass")?;
    for z in -3..=0 {
        put(&mut b, [-2, 1, z], &rail("north_south"))?;
        put(&mut b, [-2, 0, z], "glass")?;
    }
    put(&mut b, ARM_A, "detector_rail[powered=false,shape=north_south,waterlogged=false]")?;
    put(&mut b, [ARM_A[0], 0, ARM_A[2]], "glass")?;
    put(&mut b, E_A, "activator_rail[powered=true,shape=north_south,waterlogged=false]")?;
    put(&mut b, [E_A[0], 0, E_A[2]], "glass")?;
    put(&mut b, ACT_CTRL_A, &ctl_block(ACT_CTRL_A, "redstone_block"))?;
    put(&mut b, [ACT_CTRL_A[0], 0, ACT_CTRL_A[2]], "glass")?;
    // (the dismount at E, a southbound cart: W, E, NW, NE, SW, SE, N, S at E's level, then one
    // above, then one below (POSE_DISMOUNT_HEIGHTS 0, 1, -1); every place at E's level and the one
    // above made invalid (a full block in the cell, or a 1-high cell under glass), so the first valid
    // place is the level below's SW: the bed foot (its top 0.5625 is a floor only from its own
    // cell; from the cell over it the fallback needs a full-height block below))
    put(&mut b, [ACT_CTRL_A[0], 2, ACT_CTRL_A[2]], "glass")?;
    for c in [[-3, 1, 2], [-3, 1, 1], [-1, 1, 1], [-2, 2, 1], [-3, 2, 2], [-3, 2, 1], [-1, 2, 1], [-1, 2, 3]] {
        put(&mut b, c, "glass")?;
    }
    // the park: one braked powered rail ascending north (its high end at E), its lever east of it;
    // a top-half trapdoor over it (the cart passes under it; no villager stands in the park cell,
    // so neither the bed foot nor P leads into it, and the dismount's S at E's level is no floor),
    // glass over the trapdoor (no golem on its top; the dismount's S one above no floor)
    put(&mut b, PARK_A, "powered_rail[powered=false,shape=ascending_north,waterlogged=false]")?;
    put(&mut b, [PARK_A[0], -1, PARK_A[2]], "glass")?;
    put(&mut b, PARK_CTRL_A, &ctl_block(PARK_CTRL_A, "stone"))?;
    put(&mut b, [PARK_CTRL_A[0], -1, PARK_CTRL_A[2]], "glass")?;
    put(&mut b, [PARK_A[0], 1, PARK_A[2]], cap)?;
    put(&mut b, [PARK_A[0], 2, PARK_A[2]], "glass")?;
    // the pickup cell P beside the bed's head, the curve east (capped), the capped booster, the
    // capped flat, the slope down, two flats, R (A6's)
    put(&mut b, SWEPT[0], &rail("north_south"))?;
    put(&mut b, [SWEPT[0][0], -1, SWEPT[0][2]], "glass")?;
    put(&mut b, [-2, 0, 5], &rail("north_east"))?;
    put(&mut b, [-2, -1, 5], "glass")?;
    put(&mut b, [-2, 1, 5], cap)?;
    put(&mut b, [-2, 2, 5], "glass")?;
    put(&mut b, [-1, 0, 5], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [-1, -1, 5], "glass")?;
    put(&mut b, [-1, 0, 6], "redstone_torch[lit=true]")?;
    put(&mut b, [-1, -1, 6], "glass")?;
    put(&mut b, [-1, 1, 5], cap)?;
    put(&mut b, [0, 0, 5], &rail("east_west"))?;
    put(&mut b, [0, -1, 5], "glass")?;
    put(&mut b, [0, 1, 5], cap)?;
    for x in -1..=0 {
        put(&mut b, [x, 2, 5], "stone_button[face=floor,facing=north,powered=false]")?;
    }
    put(&mut b, [1, -1, 5], &rail("ascending_west"))?;
    put(&mut b, [1, -2, 5], "glass")?;
    for x in 2..=3 {
        put(&mut b, [x, -1, 5], &rail("east_west"))?;
        put(&mut b, [x, -2, 5], "glass")?;
    }
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [EXIT_DET[0], -2, EXIT_DET[2]], "glass")?;
    // the bed, its head beside P, its foot north under E's SW: the foot's roof a top-half trapdoor
    // at y 2 (the dismount's box on the foot's top, 0.5625..2.5125, clear under it), a floor stone
    // button over it (no golem on its top)
    put(&mut b, BED_HEAD, "red_bed[facing=south,occupied=false,part=head]")?;
    put(&mut b, [-3, 0, 3], "red_bed[facing=south,occupied=false,part=foot]")?;
    put(&mut b, [-3, -1, 3], "glass")?;
    put(&mut b, [-3, -1, 4], "glass")?;
    put(&mut b, [-3, 2, 3], cap)?;
    put(&mut b, [-3, 3, 3], "stone_button[face=floor,facing=north,powered=false]")?;
    // (the head's column: glass over it at y 2 and round its y 1 cell's open sides but P's head
    // room: a napper on the bed top has nowhere to go but back down)
    for c in [[-3, 2, 4], [-3, 1, 5], [-4, 2, 4], [-2, 2, 4]] {
        put(&mut b, c, "glass")?;
    }
    // the hatch west of the bed's head and the zombie's post behind it (A6's)
    put(&mut b, HATCH, "iron_trapdoor[facing=west,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_HI, "iron_trapdoor[facing=west,half=top,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_CTRL, &ctl_block(HATCH_CTRL, "redstone_block"))?;
    put(&mut b, [HATCH_CTRL[0], -1, HATCH_CTRL[2]], "glass")?;
    put(&mut b, HATCH_CTRL_HI, &ctl_block(HATCH_CTRL_HI, "redstone_block"))?;
    put(&mut b, [-5, -1, 4], "glass")?;
    put(&mut b, [-5, 2, 4], "glass")?;
    for y in 0..=1 {
        put(&mut b, [-5, y, 3], "glass")?;
        put(&mut b, [-5, y, 5], "glass")?;
        put(&mut b, [-6, y, 4], "glass")?;
    }
    put(&mut b, [-4, 0, 5], "glass")?;
    put(&mut b, [-4, 1, 3], "glass")?;
    put(&mut b, [-5, 3, 4], "oak_leaves[distance=7,persistent=true,waterlogged=false]")?;
    b.entity("oak_boat", [ZOMBIE_POST[0] as f64 + 0.5, ZOMBIE_POST[1] as f64, ZOMBIE_POST[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    // the pit's walls: north of the bed's foot at y 0 (E's support is the foot's NE), south of its
    // head, east of P (A6's curve-fix trapdoor there, glass over it), and E's flats' sides
    put(&mut b, [-3, 0, 2], "glass")?;
    for y in 0..=1 {
        wall(&mut b, [-3, y, 5])?;
    }
    put(&mut b, [-1, 0, 4], "iron_trapdoor[facing=west,half=top,open=false,powered=false,waterlogged=false]")?;
    put(&mut b, [-1, 1, 4], "glass")?;
    put(&mut b, [-1, 1, 3], "glass")?;
    if bench {
        for x in EXIT_DET[0] + 1..=EXIT_DET[0] + 6 {
            put(&mut b, [x, -1, EXIT_DET[2]], &rail("east_west"))?;
            put(&mut b, [x, -2, EXIT_DET[2]], "glass")?;
        }
        put(&mut b, [EXIT_DET[0] + 7, -1, EXIT_DET[2]], "glass")?;
        put(&mut b, QUEUE, &rail("east_west"))?;
        put(&mut b, [QUEUE[0], QUEUE[1] - 1, QUEUE[2]], "glass")?;
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("keep-block".into(), "-5,3,4=oak_leaves[distance=7,persistent=true] (the zombie's cover: its eye's sky light 14 or less, no burning by day)".into());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    let lines: Vec<String> = vec![
        "The bed stop (A), ironspec 00:07 (4A) / 00:37 (local frame; loop = local + (0,-27,-1); the bed's head at (-3,0,4)): the arrival west along z -4 at y 2, one slope down to y 1, the curve south, flats down x -2, the arming detector (-2,1,1), the activator E (-2,1,2) over the bed foot's north-east corner (always on), the park (-2,0,3) one braked powered rail ascending north, the pickup cell P (-2,0,4) beside the bed's head, the curve east, a capped booster, a capped flat, the slope down, two flats, R. No solid-render block anywhere in the stop.".to_string(),
        "E sets the rider down on the bed foot's top (a southbound cart's dismount: W, E, NW and NE blocked, SW the first valid place), 1.00 from the bed head's centre: SleepInBed fires at once (REST, not a passenger, the bed free, LAST_WOKEN 100+ ago) whatever path it carries, and startSleeping erases WALK_TARGET and CANT_REACH. The empty cart brakes on the park's slope. The hatch's two iron trapdoors stand open (their levers on) and hide the zombie; F (the lie-down + 2, and not before the arming detector's edge + the worst-case braking interval, ironspec 00:37 (2b)) SHUTS them: the zombie wakes it, it stands up on P (its only stand-up cell), the park releases at the wake edge and the cart rolls into P, seats it and runs on to R, whose rise opens the hatch again and resets the park's release and the arming latch. Precondition and single release as A6's: one cart per visit, the park released only at the wake edge.".to_string(),
        "switch: -4,0,3=the hatch's lower lever (day-1's line end: F shuts, R opens); -4,1,5=the hatch's upper lever (the same line); -1,1,2=the activator's lever (day-1's line end, held on); -1,0,3=the park's lever (day-1's line end: the release); -1,2,5=a golem-proofing stone button over the exit booster's cap (never pressed); 0,2,5=a golem-proofing stone button over the exit flat's cap (never pressed); -3,3,3=a golem-proofing stone button over the bed foot's roof (never pressed)".to_string(),
        "always-on: -1,0,6=the exit booster's redstone torch, on glass (-1,-1,6): it never toggles".to_string(),
        "a6-pickup: -2,0,4=the pickup cell (the napper's only stand-up cell); stranded bound: no cart at it by the wake edge + 60 gt fails (to restate from (A)'s rows)".to_string(),
    ];
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    Ok(b.text(&refs))
}

/// (A) with the run-up (B3, ironspec 02:27): A6's arrival at its own levels to E (-2,0,2) over the
/// bed foot's north-east corner (the dismount's SW at E's level is the bed foot's top, a floor from
/// its own cell), the curve east (-2,0,3), a booster and a climb to the U-turn at y 1, the park (1,0,4)
/// one braked powered rail descending west, two run-up flats along z 4, P (-2,0,4) a curve turning
/// south onto A6's exit. The run-up's empty-cart drag brings the released cart to P under 0.2 b/t
/// (a step over 0.2 can clip into the standing napper's box and stop dead).
pub fn design_a2(bench: bool) -> Result<String> {
    let mut b = Builder::new(if bench { "bed-tier-a2-bench" } else { "bed-tier-a2" });
    let rail = |s: &str| format!("rail[shape={s},waterlogged=false]");
    let cap = "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]";
    // (golem-proofing over the caps, ironspec 03:22 / day-1's lever map / G1's head-space flag: a
    // plain rail on each cap's top (no collision: no solid block in a rider's box; rails keep a
    // golem's spawn space out (PREVENT_MOB_SPAWNING_INSIDE); no wind reaction), not A6's stone
    // buttons (a wind burst presses an unpowered button); each rail alone, no rail beside it)
    let button = "rail[shape=north_south,waterlogged=false]";
    let wall = |b: &mut Builder, c: [i32; 3]| -> Result<()> {
        if !b.d.blocks.contains_key(&c) {
            put(b, c, "glass")?;
        }
        Ok(())
    };
    // the arrival at A6's levels: the entry booster (always on), plain rails down the old park's
    // slope, A6's capped flat, the curve south, the flats, the arming detector, E
    put(&mut b, ENTRY, "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [ENTRY[0], 1, ENTRY[2]], "glass")?;
    put(&mut b, [ENTRY[0], 2, ENTRY[2] - 1], "redstone_torch[lit=true]")?;
    put(&mut b, [ENTRY[0], 1, ENTRY[2] - 1], "glass")?;
    put(&mut b, ACTIVATOR, &rail("east_west"))?;
    put(&mut b, [ACTIVATOR[0], 1, ACTIVATOR[2]], "glass")?;
    for p in PARK {
        put(&mut b, p, &rail("ascending_east"))?;
        put(&mut b, [p[0], p[1] - 1, p[2]], "glass")?;
    }
    put(&mut b, [-1, 0, -4], &rail("east_west"))?;
    put(&mut b, [-1, -1, -4], "glass")?;
    put(&mut b, [-1, 1, -4], cap)?;
    put(&mut b, [-1, 2, -4], button)?;
    put(&mut b, [-2, 0, -4], &rail("south_east"))?;
    put(&mut b, [-2, -1, -4], "glass")?;
    put(&mut b, [-2, 1, -4], "glass")?;
    put(&mut b, [1, 3, -4], "glass")?;
    for z in -3..=0 {
        put(&mut b, [-2, 0, z], &rail("north_south"))?;
        put(&mut b, [-2, -1, z], "glass")?;
    }
    put(&mut b, ARM_A2, "detector_rail[powered=false,shape=north_south,waterlogged=false]")?;
    put(&mut b, [ARM_A2[0], -1, ARM_A2[2]], "glass")?;
    put(&mut b, E_A2, "activator_rail[powered=true,shape=north_south,waterlogged=false]")?;
    put(&mut b, [E_A2[0], -1, E_A2[2]], "glass")?;
    put(&mut b, ACT_CTRL_A2, &ctl_block(ACT_CTRL_A2, "redstone_block"))?;
    put(&mut b, [ACT_CTRL_A2[0], -1, ACT_CTRL_A2[2]], "glass")?;
    put(&mut b, [ACT_CTRL_A2[0], 1, ACT_CTRL_A2[2]], "glass")?;
    // (the dismount at E, a southbound cart: W the torch's cell under glass (1 high), E, NW, NE at
    // E's level full blocks, then SW, the bed foot's top: the first valid place, 1.00 from the bed
    // head's centre)
    for c in [[-1, 0, 2], [-3, 0, 1], [-1, 0, 1]] {
        put(&mut b, c, "glass")?;
    }
    // K, the curve east after E (glass over it: P's way north is 1 high), a flat, a booster (always
    // on: the climb's speed), the climb, the U-turn at y 1 on glass
    put(&mut b, [-2, 0, 3], &rail("north_east"))?;
    put(&mut b, [-2, -1, 3], "glass")?;
    put(&mut b, [-2, 1, 3], "glass")?;
    put(&mut b, [-1, 0, 3], &rail("east_west"))?;
    put(&mut b, [-1, -1, 3], "glass")?;
    put(&mut b, [0, 0, 3], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [0, -1, 3], "glass")?;
    put(&mut b, [0, 0, 2], "redstone_torch[lit=true]")?;
    put(&mut b, [0, -1, 2], "glass")?;
    put(&mut b, [1, 0, 3], &rail("ascending_east"))?;
    put(&mut b, [1, -1, 3], "glass")?;
    put(&mut b, [2, 1, 3], &rail("south_west"))?;
    put(&mut b, [2, 0, 3], "glass")?;
    put(&mut b, [2, 1, 4], &rail("north_west"))?;
    // (the U-turn's south support a redstone conductor, smooth stone: the park's drive; glass over
    // the curve (its cell 1 high: no stroll target on the conductor; the cart passes under))
    put(&mut b, [2, 0, 4], "smooth_stone")?;
    put(&mut b, [2, 2, 4], "glass")?;
    // the park: one braked powered rail descending west, powered through the conductor east of it
    // by day-1's repeater at (3,0,4) (the bench's stand-in a wall lever on the conductor's east face)
    put(&mut b, PARK_A2, "powered_rail[powered=false,shape=ascending_east,waterlogged=false]")?;
    put(&mut b, [PARK_A2[0], -1, PARK_A2[2]], "glass")?;
    put(&mut b, PARK_CTRL_A2, &ctl_block(PARK_CTRL_A2, "stone"))?;
    put(&mut b, [PARK_CTRL_A2[0], PARK_CTRL_A2[1] - 1, PARK_CTRL_A2[2]], "glass")?;
    // the run-up: two flats along z 4 into P, glass over them (the empty cart passes under; a
    // napper on P can't step east)
    for x in -1..=0 {
        put(&mut b, [x, 0, 4], &rail("east_west"))?;
        put(&mut b, [x, -1, 4], "glass")?;
        put(&mut b, [x, 1, 4], "glass")?;
    }
    // P, a curve turning south onto A6's exit curve, and A6's exit
    put(&mut b, SWEPT[0], &rail("south_east"))?;
    put(&mut b, [SWEPT[0][0], -1, SWEPT[0][2]], "glass")?;
    put(&mut b, [-2, 0, 5], &rail("north_east"))?;
    put(&mut b, [-2, -1, 5], "glass")?;
    put(&mut b, [-2, 1, 5], cap)?;
    put(&mut b, [-2, 2, 5], "glass")?;
    put(&mut b, [-1, 0, 5], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    put(&mut b, [-1, -1, 5], "glass")?;
    put(&mut b, [-1, 0, 6], "redstone_torch[lit=true]")?;
    put(&mut b, [-1, -1, 6], "glass")?;
    put(&mut b, [-1, 1, 5], cap)?;
    put(&mut b, [0, 0, 5], &rail("east_west"))?;
    put(&mut b, [0, -1, 5], "glass")?;
    put(&mut b, [0, 1, 5], cap)?;
    for x in -1..=0 {
        put(&mut b, [x, 2, 5], button)?;
    }
    put(&mut b, [1, -1, 5], &rail("ascending_west"))?;
    put(&mut b, [1, -2, 5], "glass")?;
    for x in 2..=3 {
        put(&mut b, [x, -1, 5], &rail("east_west"))?;
        put(&mut b, [x, -2, 5], "glass")?;
    }
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [EXIT_DET[0], -2, EXIT_DET[2]], "glass")?;
    // the bed, its foot's roof a top-half trapdoor at y 2 (the dismount's box clear under it), a
    // floor button over it; glass over the head (its top 1.44 high: not standable) and round it
    put(&mut b, BED_HEAD, "red_bed[facing=south,occupied=false,part=head]")?;
    put(&mut b, [-3, 0, 3], "red_bed[facing=south,occupied=false,part=foot]")?;
    put(&mut b, [-3, -1, 3], "glass")?;
    put(&mut b, [-3, -1, 4], "glass")?;
    put(&mut b, [-3, 2, 3], cap)?;
    put(&mut b, [-3, 3, 3], button)?;
    for c in [[-3, 2, 4], [-3, 1, 5], [-4, 2, 4], [-2, 2, 4]] {
        put(&mut b, c, "glass")?;
    }
    // the hatch and the zombie's post (A6's)
    put(&mut b, HATCH, "iron_trapdoor[facing=west,half=bottom,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_HI, "iron_trapdoor[facing=west,half=top,open=true,powered=true,waterlogged=false]")?;
    put(&mut b, HATCH_CTRL, &ctl_block(HATCH_CTRL, "redstone_block"))?;
    put(&mut b, [HATCH_CTRL[0], -1, HATCH_CTRL[2]], "glass")?;
    put(&mut b, HATCH_CTRL_HI, &ctl_block(HATCH_CTRL_HI, "redstone_block"))?;
    put(&mut b, [-5, -1, 4], "glass")?;
    put(&mut b, [-5, 2, 4], "glass")?;
    for y in 0..=1 {
        put(&mut b, [-5, y, 3], "glass")?;
        put(&mut b, [-5, y, 5], "glass")?;
        put(&mut b, [-6, y, 4], "glass")?;
    }
    put(&mut b, [-4, 0, 5], "glass")?;
    put(&mut b, [-4, 1, 3], "glass")?;
    put(&mut b, [-5, 3, 4], "oak_leaves[distance=7,persistent=true,waterlogged=false]")?;
    b.entity("oak_boat", [ZOMBIE_POST[0] as f64 + 0.5, ZOMBIE_POST[1] as f64, ZOMBIE_POST[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
    for y in 0..=1 {
        wall(&mut b, [-3, y, 5])?;
    }
    if bench {
        for x in EXIT_DET[0] + 1..=EXIT_DET[0] + 6 {
            put(&mut b, [x, -1, EXIT_DET[2]], &rail("east_west"))?;
            put(&mut b, [x, -2, EXIT_DET[2]], "glass")?;
        }
        put(&mut b, [EXIT_DET[0] + 7, -1, EXIT_DET[2]], "glass")?;
        put(&mut b, QUEUE, &rail("east_west"))?;
        put(&mut b, [QUEUE[0], QUEUE[1] - 1, QUEUE[2]], "glass")?;
    }
    b.d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    b.d.meta.insert("keep-block".into(), "-5,3,4=oak_leaves[distance=7,persistent=true] (the zombie's cover: its eye's sky light 14 or less, no burning by day)".into());
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    let lines: Vec<String> = vec![
        "The bed stop (A) with the run-up (B3, ironspec 02:27; local frame; loop = local + (0,-27,-1); the bed's head at (-3,0,4)): A6's arrival (the entry booster, plain rails down the old park's slope, the capped flat, the curve south, the flats), the arming detector (-2,0,1), the activator E (-2,0,2) over the bed foot's north-east corner (always on: a redstone torch on glass west of it, glass over the torch), the curve east (-2,0,3), a flat, a booster, the climb, the U-turn at y 1, the park (1,0,4) one braked powered rail descending west, two run-up flats along z 4, the pickup cell P (-2,0,4) a curve turning south, A6's exit (the curve east, the capped booster, the capped flat, the slope down, two flats, R). One solid-render block in the stop: the park's conductor (2,0,4) under the U-turn's curve, glass over the curve (the cell over the conductor 1 high: no stroll target).".to_string(),
        "E sets the rider down on the bed foot's top (a southbound cart's dismount: W, E, NW and NE full blocks, SW the first valid place), 1.00 from the bed head's centre: SleepInBed fires at once whatever path it carries. The empty cart runs the loop and brakes on the park. The hatch's two iron trapdoors stand open and hide the zombie; F (the lie-down + 2, and not before the arming detector's edge + 70: the napper's 60-gt boarding cooldown less the park -> P travel, plus the detector -> E time and a margin) SHUTS them: the zombie wakes it, it stands up on P (its only stand-up cell), the park releases at the wake edge, and the cart runs the run-up into P under 0.2 b/t, seats it and runs on to R, whose rise opens the hatch again and resets the park's release and the arming latch. Precondition and single release as A6's: one cart per visit, the park released only at the wake edge.".to_string(),
        "switch: -4,0,3=the hatch's lower lever (day-1's line end stand-in: F shuts, R opens; no lever in the build); -4,1,5=the hatch's upper lever (the same line); 3,0,4=the park's drive (day-1's repeater output stand-in: a wall lever on the conductor (2,0,4); the release; no lever in the build)".to_string(),
        "always-on: 3,2,-5=the entry booster's redstone torch, on glass (3,1,-5): it never toggles; -3,0,2=the activator's redstone torch, on glass (-3,-1,2): it never toggles (ironspec 03:22: no lever a wind burst can flip); 0,0,2=the loop booster's redstone torch, on glass (0,-1,2): it never toggles; -1,0,6=the exit booster's redstone torch, on glass (-1,-1,6): it never toggles".to_string(),
        "a6-pickup: -2,0,4=the pickup cell (the napper's only stand-up cell); stranded bound: no cart at it by the wake edge + 60 gt fails (to restate from these rows)".to_string(),
    ];
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    Ok(b.text(&refs))
}

/// One visit's unit-bench row (ticks counted from the cart's spawn at the entry).
#[derive(Debug, Default, Clone)]
pub struct Row {
    pub seed: u64,
    pub speed: f64,
    /// At each lie-down, the napper's distance (feet, the tick before) to the bed head's centre
    /// (SleepInBed's bar: under 2).
    pub lie_dists: Vec<f64>,
    /// The rider's first standing cell after the eject, and when.
    pub landed: Option<([i32; 3], i64)>,
    /// Where the arriving cart stopped (its centre) and when.
    pub parked: Option<([f64; 3], i64)>,
    pub slept_at: Option<i64>,
    pub woke_at: Option<i64>,
    /// Every lie-down and every wake, in order.
    pub sleeps: Vec<i64>,
    pub wakes: Vec<i64>,
    /// The first tick the napper had no HOME memory (it arrived with one).
    pub home_lost_at: Option<i64>,
    /// (a skip row) The skip's tick, the cells the napper waited in awake before its next nap, the
    /// day time of that nap, its CANT_REACH stamp's age at the first REST tick after the skip (None
    /// inside: no stamp), and its LAST_SLEPT (game time) at the end.
    pub skipped_at: Option<i64>,
    /// (design A) The abort: its tick, DAY's time high at it, and where the napper was.
    pub aborted: Option<(i64, i64, &'static str)>,
    /// (design A) The napper's CANT_REACH stamp's age at an abort's release (None inside: no
    /// stamp), and the cell it stood up in at each wake.
    pub stamp_at_release: Option<Option<i64>>,
    pub stood_up: Vec<[i32; 3]>,
    /// (ironspec 11:43, point 1) At each wake, `findStandUpPosition`'s own answer for both sides
    /// (the side follows the yaw), and the cart's cell then.
    pub standup_check: Vec<Option<[i32; 3]>>,
    pub cart_at_wake: Vec<[i32; 3]>,
    /// Every drop the napper made off the ground (its fall distance at the landing, and the
    /// health it lost there), its first tick in SleepInBed's reach at night awake and on foot (the
    /// lie-down's first chance), the cells it stood in awake for 100 gt or more before its first nap
    /// or its release (with the gt there), the nearest the napper's CANT_REACH stamp came to 1,000
    /// at a calm REST tick (its largest age there), and the abort's kind.
    pub drops: Vec<(f64, f32)>,
    pub eligible_at: Option<i64>,
    pub long_waits: Vec<([i32; 3], i64)>,
    pub rest_stamp_max: Option<i64>,
    /// (G1 09:43) The walk targets a napper took waiting on foot in IDLE before its nap ([MIN,..]:
    /// an entity), and whether its stamp was armed there.
    pub idle_walk_targets: Vec<[i32; 3]>,
    pub idle_stamp: bool,
    /// The napper was on foot in IDLE before its first nap (ironspec 09:45's outcome (b)).
    pub idle_before_nap: bool,
    /// (the post-exit ride, the bench's run-out: no stamp clearer there) The stamp's age at the exit
    /// (None inside: no stamp), HOME lost (gt after the exit), the stamp's first arming after the
    /// exit (gt after it), its largest age at a calm REST tick in the 1,500 gt after it.
    pub stamp_at_exit: Option<Option<i64>>,
    pub post_home_lost: Option<i64>,
    pub post_stamp_armed: Option<i64>,
    pub post_stamp_max: Option<i64>,
    /// (ironspec 11:01, the waker's places) Every place the napper had the zombie in sight within 8:
    /// the least distance, and whether every such tick came after F; its walk targets while
    /// panicking (where it was, the target); its stamp's arming after the wake (gt after the wake
    /// edge, where it was).
    pub waker_seen: std::collections::BTreeMap<String, (f64, bool)>,
    pub panic_targets: Vec<(String, [i32; 3])>,
    pub stamp_armed: Option<(i64, String)>,
    /// (ironspec 14:23) Every node cell of every path the napper held, riding or on foot.
    pub path_nodes: std::collections::BTreeSet<[i32; 3]>,
    /// (ironspec 14:58) The tick its cart first entered A6's entry cell, and its stamp's age then
    /// (None inside: no stamp).
    pub entry_at: Option<i64>,
    pub stamp_at_entry: Option<Option<i64>>,
    /// (the real approach) The queue's release tick (held for dusk after a skip before it).
    pub queue_release_at: Option<i64>,
    /// (ironspec 17:16) At the queue's release: the napper's stamp age and whether it holds a walk
    /// target or a path (a held run).
    pub state_at_queue_release: Option<(Option<i64>, bool)>,
    /// (ironspec 16:29) The napper's escape: on foot on the entry or the approach, out of the stop's
    /// box, or gone (the tick and the cell; MIN: gone).
    pub escaped: Option<(i64, [i32; 3])>,
    pub waited: Vec<[i32; 3]>,
    pub slept_after_skip: Option<i64>,
    pub stamp_at_rest: Option<Option<i64>>,
    pub last_slept: Option<i64>,
    /// After the restart: the directions of the woken napper's paths off the pickup cell (north is
    /// toward the arriving cart).
    pub paths_after_restart: Vec<&'static str>,
    /// The same after the wake in any row.
    pub paths_after_wake: Vec<&'static str>,
    /// The napper's cell on its first awake tick after the nap.
    pub stood: Option<[i32; 3]>,
    /// Every cell the awake napper stood on between the wake and the pickup.
    pub cells_after_wake: Vec<[i32; 3]>,
    /// The release tick, and the cart's horizontal speed as it crossed each swept cell's centre.
    pub release_at: Option<i64>,
    pub swept_speed: Vec<([i32; 3], f64)>,
    /// The pickup: the cell the napper stood on and the tick.
    pub pickup: Option<([i32; 3], i64)>,
    /// The cart's horizontal speed in the tick it seated the napper.
    pub pickup_speed: Option<f64>,
    /// (ironspec 17:46/18:34) The stand-in guard's reach lines (one per stand-in the bench writes).
    pub standin: Vec<String>,
    /// (ironspec 17:35 (a)) Seated before its wake or off the bed while asleep.
    pub pickup_unwoken: bool,
    /// (ironspec 20:31 (2)) Where the parked cart was moved to (`park_shift`).
    pub park_moved: Option<[f64; 3]>,
    /// ((A)) The arming detector's first powered tick (this cart's passage), the landing's exact
    /// position (feet), and the cart's speed on the release tick (the at-rest bar).
    pub armed_at: Option<i64>,
    pub landing_pos: Option<[f64; 3]>,
    pub dismount_at: Option<i64>,
    pub speed_at_release: Option<f64>,
    /// (ironspec 17:35 (a)) The released cart in the pickup cell before the napper's wake.
    pub cart_at_pickup_before_wake: bool,
    /// The cart over R's detector (the stop clear).
    pub r_at: Option<i64>,
    /// (ironspec 22:14 (3a)) R's real rise: the first tick the detector rail at EXIT_DET is powered.
    pub r_real_at: Option<i64>,
    /// The cart a block past R (the visit's end), and whether the napper rode it.
    pub out_at: Option<i64>,
    pub seated_out: bool,
    /// (ironspec 21:25, every cart-motion bench) The cart's contacts with blocks, the spawn to out:
    /// (tick, the cart's position, "horizontal" or "vertical off a rail"); a contact at a cell the
    /// design's `# contacts:` header doesn't declare fails the row.
    pub contacts: Vec<(i64, [f64; 3], &'static str)>,
    /// (ironspec 22:10) The stamp's largest age over the visit, and the tick (from the entry) it
    /// cleared inside A6.
    pub stamp_visit_max: i64,
    /// The longest stand-still after the eject (gt awake on foot, unmoved), before the first
    /// lie-down or the pickup.
    pub stand_after_eject: i64,
    /// (the head-space check) Ticks x villagers with their box, and with their eye box, meeting a
    /// declared new solid cell.
    pub head_box_hits: usize,
    pub head_eye_hits: usize,
    /// (ironspec 04:07) The bench's laid overrides, each cell's state before and after.
    pub overrides: Vec<String>,
    pub stamp_cleared_at: Option<i64>,
    /// (ironspec 22:56 (iii)) The napper's cells a bolt could reach (sees the sky, or inside a
    /// column top's bolt hit box).
    pub lightning: Vec<String>,
    /// (`--pulse-cell`) Each pulse: the block change, whether its collision shape changed, the
    /// napper's last path recompute before and after.
    pub pulses: Vec<String>,
    /// (ironspec 00:27) The fluid cells whose tick the bench scheduled at load.
    pub fluid_scheduled: usize,
    /// (record-match) The line, and its block and mob verdicts (count / don't).
    pub record_line: Option<(String, bool, bool)>,
    /// (ironspec 00:38: the PATH set after every load) Each restart's tick and the napper's path
    /// right after it (next index, nodes).
    pub path_after_load: Vec<String>,
    pub problems: Vec<String>,
}

/// The bench's fault and control rows.
/// (ironspec 21:43 / 23:46) The design of record a bench is compared against: its cells and
/// entities, the offset (record-match's: a record cell + offset = the bench's cell), the margin over
/// the bench's world box, the declarations (stand-ins, candidates, bench entities) and its name.
#[derive(Debug, Default, Clone)]
pub struct RecordCtx {
    pub cells: std::collections::BTreeMap<crate::recordmatch::Cell, String>,
    pub ents: Vec<crate::recordmatch::Ent>,
    pub offset: crate::recordmatch::Cell,
    pub margin: i32,
    pub declared: crate::recordmatch::Declared,
    pub name: String,
}

#[derive(Debug, Default, Clone)]
pub struct Faults {
    /// The cart sent empty (no villager): a control.
    pub empty: bool,
    /// The villager arrives with no HOME memory: a control (it never walks to the bed).
    pub no_home: bool,
    /// The waker hidden again once the napper is awake (the hatch back open-plated): the refuge's
    /// control (the napper should go back to bed).
    pub hide_after_wake: bool,
    /// A save, stop and start between ticks T - 1 and T (T gt after the entry) in this load order
    /// (`liveness::restart`: a world open, N idle ticks of game time with nothing ticking; the lead's rows
    /// run all 20 restart rows).
    pub restart: Option<(i64, crate::liveness::Restart)>,
    /// The visit's day time exactly (in place of the seeded night hour).
    pub hour: Option<i64>,
    /// A night skip at the start of tick T after the entry (G1 on 941c5aa2b): the day time jumps to
    /// the next day time 0 (`ServerLevel`'s sleep skip: game time doesn't jump), so REST holds until
    /// day time 10 and `WakeUp` stands a sleeper up then, before F if it lay down after the skip.
    pub skip: Option<i64>,
    /// DAY's falling edge at dusk, the day time the daylight detector falls below L 6 (tick-exact's
    /// table: clear 12,230, rain 12,002, thunder 11,556; None: clear). DAY is high from day time 0
    /// (a skip lands there, the weather cleared) to this.
    pub day_off: Option<i64>,
    /// The release's positive control: the park released `release_after` gt after F, or 300 gt
    /// after the entry, whether or not the napper is up on the pickup cell (an empty cart must
    /// leave).
    pub no_presence_gate: bool,
    /// (G1's CHANGES 4 (3)) The early-pickup bar's positive control: the park released at the
    /// lie-down edge (+ `release_after`), the napper still in bed.
    pub early_release: bool,
    /// (ironspec 21:25) The cells where the design declares a cart contact (`# contacts:`, a stop
    /// or a buffer), in the bench's frame.
    pub declared_contacts: Vec<[i32; 3]>,
    /// (ironspec 00:03 (2c)) `--pulse-cell`: blocks set at a tick (bench frame), to measure the
    /// navigation's recompute trigger.
    pub pulses: Vec<(BlockPos, i64, String)>,
    /// (ironspec 00:31, the re-runs on record7 itself) Declared levers held ON from the start (the
    /// stop's frame): REST_l's line end through REST.
    pub levers_on: Vec<[i32; 3]>,
    /// record7's built release: the NOT-ready lever (the stop's frame), ON through the wait, OFF at
    /// the release (R1 = N and not NOT-ready, built), ON again once the cart leaves the seat; no R1
    /// stand-ins laid.
    pub built_release: Option<[i32; 3]>,
    /// DAY's declared lever, following the daylight detector's real rule: set to DAY (day time in
    /// [day_on, day_off)) only at game time % 20 == 0, never in between (ironspec 00:25).
    pub day_lever: Option<[i32; 3]>,
    /// (ironspec 21:43 / 23:46) record-match against the design of record, computed on the world of
    /// the visit (`record_seed`, `record_speed`) after every bench write and before the first tick.
    pub record: Option<std::sync::Arc<RecordCtx>>,
    pub record_seed: u64,
    pub record_speed: f64,
    /// (ironspec 00:31) The module's declared emulated cells (its `emulated:` meta line, the stop's
    /// frame), laid before the run (record-match counts them as the declared candidate
    /// "emulated").
    /// ((A), ironspec 03:30 / G1's head-space flag) New solid cells (the stop's frame) checked every
    /// tick against every villager's box and eye box (its eye height, +- 0.4 x its width, as
    /// `isInWall`): bar 0 eye overlap.
    pub head_cells: Vec<[i32; 3]>,
    pub lay_emulated: Vec<([i32; 3], String)>,
    /// (ironspec 23:56) Bars this run's declared scope excludes (`visits`, `pickups`, `riderless`):
    /// their 0 samples print NOT JUDGED without failing.
    pub scope_excludes: Vec<String>,
    /// ((A), ironspec 00:37 (2b)) F waits for the arming detector's edge + this many gt (the
    /// worst-case braking interval + 3).
    pub arm_delay: i64,
    /// ((A)'s at-rest bar's positive control) The park released this many gt after the arming
    /// detector's edge, whatever the napper does.
    pub release_after_arm: Option<i64>,
    /// The 09:00 shape (a control since ironspec 09:45): DAY gates the activator and the abort
    /// releases an unslept napper by day. Without it the activator is always on and an unslept
    /// napper waits for REST.
    pub day_gate: bool,
    /// DAY's rise after day time 0 (the stale window after a skip, day-1 08:44: the detector's
    /// update phase and the weather ramps, then the DAY line; clear 23 + line, storm 53 + line):
    /// DAY is high from this day time to `day_off`. None: 0.
    pub day_on: Option<i64>,
    /// The seated term's control (ironspec 08:15, condition 1): seated read from the live DAY level
    /// (DAY high and the cart parked) instead of the activator's state latched when the cart passed
    /// it: a napper ejected at night with DAY rising mid-walk must then send an empty cart.
    pub live_day_seated: bool,
    /// (G1/ironspec 11:43: the released napper may enter the stop still panicking) Its
    /// NEAREST_HOSTILE held on the post's zombie through this tick after the entry (then the
    /// sensor's own updates): the panic's tail in the stop.
    pub panic_entry: Option<i64>,
    /// (G1's note 1 on b36302af8) The released cart set two cells north of the pickup cell heading
    /// south at this speed (b/t): the pickup margin's sweep.
    pub pickup_inject: Option<f64>,
    /// (ironspec 14:58, the boundary contract) The real approach: the napper's cart spawned at the
    /// queue's seat at rest and released by its input `wait` gt later (a module file shifted into
    /// the stop's frame, `shift_spec`).
    pub approach: Option<Approach>,
    /// The contract's positive control: the napper's CANT_REACH stamp armed as its cart enters A6's
    /// entry (a stale arrival with an armed stamp must lose HOME at dusk).
    pub forced_stamp: bool,
    /// (ironspec 17:16) The napper's leg state on arrival: a stamp armed this many gt before the
    /// spawn at the queue seat.
    pub arrival_stamp: Option<i64>,
    /// (ironspec 17:16) The napper's leg state on arrival: a held HOME run (the bed its walk target
    /// at the spawn, the run computed from the queue seat).
    pub arrival_run: bool,
    /// (ironspec 17:52) The held run's remaining hold at the arrival: the sink's end set this many
    /// gt after it starts (b8949c96's measured drop: 88-160 gt after the arrival).
    pub arrival_hold: Option<i64>,
    /// (ironspec 17:46) Cells declared emulated: a stand-in write there may differ from the built
    /// cell in a golem or path property (its statics run with the stand-in).
    pub declared_emulated: Vec<[i32; 3]>,
    /// (ironspec 19:18) The timing bar for entry to R per entry speed (v_slow); 0: 209.
    pub r_bar: i64,
    /// (ironspec 20:31 (2)) The parked cart moved this far along its rail (x on an east-west rail,
    /// z on a north-south one, y with the slope) before the release; `park_shifts` sweeps it.
    pub park_shift: Option<f64>,
    pub park_shifts: Vec<f64>,
}

/// The walk's cells (A6), all at y 2: the landing south of the activator, the ledge round to x -2,
/// south to (-2,2,2), the gate, the bed's foot roof; then the drop over the bed's head onto the bed.
pub const LEDGE: [[i32; 3]; 12] = [[2, 2, -3], [2, 2, -2], [2, 2, -1], [2, 2, 0], [1, 2, 0], [0, 2, 0], [-1, 2, 0], [-2, 2, 0], [-2, 2, 1], [-2, 2, 2], [-3, 2, 2], [-3, 2, 3]];

/// (ironspec 14:58) The real approach in the stop's frame: the napper's cart at the queue's seat,
/// the queue's release input, the gt from the spawn to the release.
/// (day-1's R1, b8949c96 17:58) The queue release's cells in the bench's frame: the two repeater
/// cells (the release cell beside the queue's rail, the one behind it), the driver's cell behind
/// them, and the repeaters' facing (away from the rail: their output toward it).
pub fn release_cells(a: &Approach) -> ([[i32; 3]; 2], [i32; 3], &'static str) {
    let rail = [a.seat[0].floor() as i32, a.seat[1].floor() as i32, a.seat[2].floor() as i32];
    let r0 = a.release;
    let d = [(rail[0] - r0[0]).signum(), 0, (rail[2] - r0[2]).signum()];
    let r1 = [r0[0] - d[0], r0[1], r0[2] - d[2]];
    let driver = [r1[0] - d[0], r1[1], r1[2] - d[2]];
    let facing = match d {
        [1, _, 0] => "west",
        [-1, _, 0] => "east",
        [0, _, 1] => "north",
        _ => "south",
    };
    ([r0, r1], driver, facing)
}

#[derive(Debug, Clone, Copy)]
pub struct Approach {
    pub seat: [f64; 3],
    pub release: [i32; 3],
    pub wait: i64,
    /// day-1's gate (15:52): the release also waits for REST_l and NOT DAY, both up by this day
    /// time (a napper picked before a skip is held at the queue until dusk).
    pub dusk: i64,
}

/// A module file (loop frame) moved into the stop's local frame by `d` (loop = local - d): its
/// blocks, entities and box; its Nether part dropped (no golem travels in the stop's rows).
pub fn shift_spec(spec: &FarmSpec, d: [i32; 3]) -> FarmSpec {
    let sh = |p: BlockPos| BlockPos::new(p.x + d[0], p.y + d[1], p.z + d[2]);
    FarmSpec {
        blocks: spec.blocks.iter().map(|(p, b)| (sh(*p), *b)).collect(),
        entities: spec.entities.iter().map(|(t, p, n)| (t.clone(), [p[0] + d[0] as f64, p[1] + d[1] as f64, p[2] + d[2] as f64], n.clone())).collect(),
        output: sh(spec.output),
        lo: sh(spec.lo),
        hi: sh(spec.hi),
        block_entities: spec.block_entities.iter().map(|(p, s)| (sh(*p), s.clone())).collect(),
        nether: None,
        ..spec.clone()
    }
}

/// The abort's logic delay in the bench (design A): the controller's latch and the drives, DAY and
/// presence to the floor opening and the park's release.
pub const ABORT_LOGIC_GT: i64 = 4;

fn block(s: &str) -> Block {
    Block::parse(&format!("minecraft:{s}")).expect("a known block")
}

/// A controller cell's block for the bench's on ("redstone_block") and off ("stone") (A5): a lever
/// (the stop holds no solid-render block, so no IDLE stroll candidate), on the floor on glass; any
/// other cell as given.
/// (ironspec 17:46, the stand-in guard; 22:37 / 23:13: ONE implementation) A bench's emulated write
/// must answer the block-intrinsic properties as the built cell does: `recordmatch::guard_intrinsics`
/// (golem in both roles, the full path classification, late-path effects, sight), the shared
/// definition the record-match helper and f1q2-run's guard call; update reach, power reach and
/// survival are this bench's own checks (`standin_reach_check`, `survival_check`).
pub fn guard_gaps(w: &World, p: BlockPos, new: &Block) -> Vec<String> {
    let old = w.get(p);
    let above = w.get(BlockPos::new(p.x, p.y + 1, p.z));
    let below = w.get(BlockPos::new(p.x, p.y - 1, p.z));
    crate::recordmatch::guard_intrinsics(new, &old, &above, &below)
        .iter()
        .zip(crate::recordmatch::PROPERTIES.iter())
        .filter_map(|(v, name)| match v {
            Some(Err(why)) => Some(format!("{name}: {why}")),
            _ => None,
        })
        .collect()
}

/// (ironspec 17:46) Refuses an emulated write whose block differs from the built cell's in a
/// block-intrinsic property of the shared guard, unless the cell is declared emulated (its statics
/// run with the stand-in).
pub fn emulated_write_check(w: &World, p: BlockPos, new: &Block, declared: &[[i32; 3]]) -> Result<(), String> {
    let old = w.get(p);
    let differ = guard_gaps(w, p, new);
    if differ.is_empty() || declared.contains(&[p.x, p.y, p.z]) {
        return Ok(());
    }
    Err(format!(
        "refused: the emulated write {new} at ({},{},{}) differs from the built {old} in the shared guard's properties ({}); declare the cell emulated and run the statics with it",
        p.x,
        p.y,
        p.z,
        differ.join(", ")
    ))
}

/// (ironspec 20:31 (4b), the stand-in guard's sixth property: survival) Every laid or written
/// block at `cells` must survive where it stands (`can_survive`), and no rail may pop
/// (`rails::would_pop`); else the run fails.
/// (ironspec 23:13 / 00:31) A module's declared emulated cells, its `emulated: CELLS=BLOCK (why); …`
/// meta line (record7's: the line-bench levers and their glass, laid by the bench, not in the
/// file's cells), each CELLS one `x,y,z` or several joined by `+`; moved into the bench's frame by
/// `shift` (bench = file - shift).
pub fn emulated_entries(text: &str, shift: [i32; 3]) -> Result<Vec<([i32; 3], String)>, String> {
    let mut out = Vec::new();
    for l in text.lines() {
        let Some(body) = l.trim_start().strip_prefix("emulated:") else { continue };
        let mut entries: Vec<String> = Vec::new();
        let mut cur = String::new();
        for part in body.split(';') {
            let head = part.trim_start();
            let starts = head.split_once('=').is_some_and(|(c, _)| c.split(['+', ',']).count() % 3 == 0 && c.split(['+', ',']).all(|t| t.trim().parse::<i32>().is_ok()));
            if starts && !cur.trim().is_empty() {
                entries.push(cur.trim().to_string());
                cur = String::new();
            }
            if !cur.is_empty() {
                cur.push(';');
            }
            cur.push_str(part);
        }
        if !cur.trim().is_empty() {
            entries.push(cur.trim().to_string());
        }
        for e in entries {
            let Some((cells, rest)) = e.split_once('=') else { continue };
            let st = rest.trim().split([' ', '(']).next().unwrap_or("").trim().to_string();
            if st.is_empty() {
                return Err(format!("emulated {e:?}: CELLS=BLOCK (why)"));
            }
            for c in cells.split('+') {
                let v: Vec<i32> = c.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|err| format!("emulated {e:?}: {err}"))?;
                match v.as_slice() {
                    &[x, y, z] => out.push(([x - shift[0], y - shift[1], z - shift[2]], st.clone())),
                    _ => return Err(format!("emulated {e:?}: x,y,z")),
                }
            }
        }
    }
    Ok(out)
}

/// (ironspec 21:25) The design's declared cart contacts, `# contacts: CELLS=REASON; …`, each CELLS
/// one `x,y,z` or several joined by `+` (record7's header), moved into the bench's frame by `shift`
/// (bench = file - shift).
pub fn parse_contacts(text: &str, shift: [i32; 3]) -> Result<Vec<[i32; 3]>, String> {
    let mut out = Vec::new();
    for d in crate::pushscan::header_entries(text, "contacts:") {
        let cells = d.split_once('=').map_or(d.as_str(), |(c, _)| c);
        for c in cells.split('+') {
            let v: Vec<i32> = c.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("contacts {d:?}: x,y,z[+x,y,z…]=REASON ({e})"))?;
            match v.as_slice() {
                &[x, y, z] => out.push([x - shift[0], y - shift[1], z - shift[2]]),
                _ => return Err(format!("contacts {d:?}: x,y,z[+x,y,z…]=REASON")),
            }
        }
    }
    Ok(out)
}

/// A declared lever's powered state set (its block kept, only `powered` written); the cell must
/// hold a lever.
pub fn set_lever(w: &mut World, p: BlockPos, on: bool) -> Result<(), String> {
    let s = format!("{}", w.get(p));
    if !s.contains("lever[") {
        return Err(format!("({},{},{}) holds {s}, not a declared lever", p.x, p.y, p.z));
    }
    let t = if on { s.replace("powered=false", "powered=true") } else { s.replace("powered=true", "powered=false") };
    if t != s {
        // (the state's own text, namespaced as Display prints it)
        let nb = Block::parse(&t).map_err(|e| format!("({},{},{}) {t}: {e}", p.x, p.y, p.z))?;
        w.set_block(p, nb, redsim::world::flags::ALL);
    }
    Ok(())
}

pub fn survival_check(w: &World, cells: &[BlockPos]) -> Result<(), String> {
    for &p in cells {
        let b = w.get(p);
        if b.is_air() {
            continue;
        }
        if !redsim::behavior::can_survive(w, &b, p) {
            return Err(format!("refused: {b} at ({},{},{}) can't survive there (no support)", p.x, p.y, p.z));
        }
        if redsim::rails::would_pop(w, p) {
            return Err(format!("refused: the rail {b} at ({},{},{}) would pop", p.x, p.y, p.z));
        }
    }
    Ok(())
}

/// (ironspec 18:34, the stand-in guard's reach properties) The cells a stand-in block's state
/// change reaches: `update` (the face neighbours whose `neighborChanged` fires: the cell's own, and
/// for a lever its attached block's, for a repeater the block it faces into and that block's), and
/// `power` (the cells it powers: a lever its six neighbours and its attached block, a repeater the
/// cell in front; through a conducting block, that block's neighbours too).
pub fn standin_reach(w: &World, p: BlockPos, b: &Block) -> Result<(Vec<BlockPos>, Vec<BlockPos>), String> {
    let s = format!("{b}");
    let prop = |k: &str| s.split(|c| c == '[' || c == ',' || c == ']').find_map(|kv| kv.strip_prefix(&format!("{k}=")).map(str::to_string));
    let off = |q: BlockPos, d: &str| match d {
        "north" => BlockPos::new(q.x, q.y, q.z - 1),
        "south" => BlockPos::new(q.x, q.y, q.z + 1),
        "east" => BlockPos::new(q.x + 1, q.y, q.z),
        "west" => BlockPos::new(q.x - 1, q.y, q.z),
        "up" => BlockPos::new(q.x, q.y + 1, q.z),
        _ => BlockPos::new(q.x, q.y - 1, q.z),
    };
    let six = |q: BlockPos| ["north", "south", "east", "west", "up", "down"].iter().map(|d| off(q, d)).collect::<Vec<_>>();
    let opp = |d: &str| match d {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        "up" => "down",
        _ => "up",
    };
    // (G1's CHANGES 4 (2c): redsim's own conductor test (`isRedstoneConductor`: slime conducts,
    // glass doesn't))
    let conducts = |q: BlockPos| redsim::behavior::is_conductor(w, &w.get(q), q);
    let mut update = six(p);
    let mut power = Vec::new();
    if s.contains("lever") {
        let face = prop("face").unwrap_or_default();
        let facing = prop("facing").unwrap_or_default();
        let attached = match face.as_str() {
            "floor" => off(p, "down"),
            "ceiling" => off(p, "up"),
            _ => off(p, opp(&facing)),
        };
        update.extend(six(attached));
        power.extend(six(p));
        if conducts(attached) {
            power.extend(six(attached));
        }
    } else if s.contains("repeater") || s.contains("comparator") {
        let front = off(p, opp(&prop("facing").unwrap_or_default()));
        update.push(front);
        update.extend(six(front));
        power.push(front);
        if conducts(front) {
            power.extend(six(front));
        }
    } else {
        // (G1's CHANGES 4 (2b): a kind whose reach this model doesn't know is refused, not
        // passed as powering nothing)
        return Err(format!("refused: the stand-in {b} at ({},{},{}) is a kind the reach model doesn't cover (levers, repeaters and comparators only)", p.x, p.y, p.z));
    }
    update.retain(|q| *q != p);
    update.sort_by_key(|q| (q.y, q.x, q.z));
    update.dedup();
    power.retain(|q| *q != p);
    power.sort_by_key(|q| (q.y, q.x, q.z));
    power.dedup();
    Ok((update, power))
}

/// (ironspec 18:34; 03:26, tick-exact's `RailBlock.updateState` citation) A PLAIN rail with rails on
/// 3 or more sides (each side's rail at its level, one up or one down; any rail kind counts as a
/// potential connection; 3 exactly re-shapes, the check keeps dfb89499's stricter >= 3): an update
/// from a signal-source block may re-shape it. Powered, detector and activator rails never
/// re-shape on a neighbour update (only their power state follows).
pub fn rail_t_at(w: &World, p: BlockPos) -> bool {
    let is_rail = |q: BlockPos| format!("{}", w.get(q)).contains("rail");
    if !format!("{}", w.get(p)).starts_with("minecraft:rail[") {
        return false;
    }
    [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dz)| (-1..=1).any(|dy| is_rail(BlockPos::new(p.x + dx, p.y + dy, p.z + dz)))).count() >= 3
}

/// (ironspec 18:34) The reach half of the stand-in guard: refuses a stand-in whose updates reach
/// a rail T not declared state-stable, or whose power reaches a redstone-sensitive cell outside
/// `targets` (the built input's own reach) that isn't declared. Returns the reach as a line.
pub fn standin_reach_check(w: &World, p: BlockPos, b: &Block, targets: &[BlockPos], declared: &[[i32; 3]]) -> Result<String, String> {
    let (update, power) = standin_reach(w, p, b)?;
    let dec = |q: &BlockPos| declared.contains(&[q.x, q.y, q.z]);
    if let Some(t) = update.iter().find(|q| rail_t_at(w, **q) && !dec(q)) {
        return Err(format!("refused: the stand-in {b} at ({},{},{}) updates the rail T at ({},{},{}) (RailBlock.updateState); declare it state-stable", p.x, p.y, p.z, t.x, t.y, t.z));
    }
    let sensitive = |q: &BlockPos| {
        let s = format!("{}", w.get(*q));
        ["rail", "repeater", "comparator", "piston", "lamp", "door", "trapdoor", "dispenser", "dropper", "hopper", "observer", "redstone_wire", "torch", "note_block", "lever", "bell", "tnt"].iter().any(|k| s.contains(k))
    };
    if let Some(q) = power.iter().find(|q| sensitive(q) && !targets.contains(q) && !dec(q)) {
        return Err(format!("refused: the stand-in {b} at ({},{},{}) powers {} at ({},{},{}), outside the built input's reach; declare it", p.x, p.y, p.z, w.get(*q), q.x, q.y, q.z));
    }
    let powered: Vec<String> = power.iter().filter(|q| !w.get(**q).is_air()).map(|q| format!("({},{},{}) {}", q.x, q.y, q.z, w.get(*q))).collect();
    Ok(format!("the stand-in {b} at ({},{},{}): its updates reach {} cells, no undeclared rail T; it powers {}", p.x, p.y, p.z, update.len(), if powered.is_empty() { "no block".to_string() } else { powered.join(", ") }))
}

pub fn ctl_block(c: [i32; 3], st: &str) -> String {
    let on = match st {
        "redstone_block" => true,
        "stone" => false,
        other => return other.to_string(),
    };
    if !([HATCH_CTRL, HATCH_CTRL_HI, ACT_CTRL, ACT_CTRL_T5B, ACT_CTRL_A, PARK_CTRL_A].contains(&c) || PARK_CTRL.contains(&c)) {
        // ((A) with the run-up: the activator's lever hung from the glass over E, the park's on
        // the glass south of it)
        if c == ACT_CTRL_A2 {
            return "redstone_torch[lit=true]".to_string();
        }
        if c == PARK_CTRL_A2 {
            return format!("lever[face=wall,facing=east,powered={on}]");
        }
        return st.to_string();
    }
    format!("lever[face=floor,facing=north,powered={on}]")
}

/// The direction of a villager's current path from the pickup cell: its first node off the cell
/// (north is up the approach, toward the arriving cart).
fn path_dir(w: &World, i: usize) -> Option<&'static str> {
    use redsim::entity::mob::ai::brain;
    if !brain::has_exact(w, i) {
        return None;
    }
    let p = brain::ex(w, i).mem.path.as_ref()?;
    let n = p.nodes.iter().skip(p.next_node_index).find(|n| (n.x, n.z) != (SWEPT[0][0], SWEPT[0][2]))?;
    Some(if n.z < SWEPT[0][2] {
        "north"
    } else if n.z > SWEPT[0][2] {
        "south"
    } else if n.x > SWEPT[0][0] {
        "east"
    } else {
        "west"
    })
}

/// One visit: the visitor's cart onto the entry at `speed` (-x) at night (or by day with `day`), F
/// (the hatch's controls off: the zombie seen) `after_sleep` gt after the napper's lie-down, the
/// release (the park's supports to redstone blocks) `release_after` gt after F, until the cart is a
/// block past R or 2400 gt; the controller on game ticks.
pub fn bench(spec: &FarmSpec, seed: u64, speed: f64, release_after: i64, day: bool, after_sleep: i64, f: &Faults) -> Result<Row, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    // (the daylight detectors the start held back (they'd read the settle's sky), placed as
    // `Run::arrive` places them: the bench spawns its own entities, so it must place them itself;
    // record7e's DAY detector was missing from the bench world before this, ironspec 03:29)
    run.place_detectors();
    // (ironspec 00:27: every fluid cell's tick scheduled at load (its onPlace), the count printed;
    // a world holding fluid with none scheduled fails)
    let mut fluid_scheduled = 0usize;
    for (p, _) in &spec.blocks {
        let b = run.world.get(*p);
        let fs = redsim::fluid::fluid_state_of(&b);
        if !fs.is_empty() {
            let d = redsim::fluid::tick_delay(&run.world, fs.fluid);
            run.world.schedule_fluid_tick(*p, fs.fluid, d);
            fluid_scheduled += 1;
        }
    }
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    // (ironspec's 11:05 audit: a cell outside the world's box is dropped silently; none may be)
    let dropped = spec.blocks.iter().filter(|(p, _)| !run.world.contains(*p)).count();
    let mut zid = run.world.entities.list.iter().find(|e| e.id >= first && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Zombie)).map(|e| e.id);
    let mut row = Row { seed, speed, fluid_scheduled, ..Default::default() };
    if dropped > 0 {
        row.problems.push(format!("{dropped} of the design's cells outside the world's box"));
    }
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    // (wait for the visit's hour: night 13,800 + seed spread, or day 2,000, or `f.hour`)
    let want = f.hour.unwrap_or(if day { 2000 } else { 13_800 } + (seed as i64 * 37) % 400);
    let mut guard = 0;
    while run.world.day_time().rem_euclid(24000) != want {
        run.tick();
        guard += 1;
        if guard > 30_000 {
            return Err("never reached the visit's hour".into());
        }
    }
    // the visitor at the entry, HOME = the bed (as the game loads the memory)
    let h = BED_HEAD;
    let nbt = if f.empty {
        "{Rotation:[0f,0f]}".to_string()
    } else if f.no_home {
        "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}".to_string()
    } else {
        format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2])
    };
    let start = f.approach.map_or([ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5], |a| a.seat);
    let mut cart_id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&nbt))?;
    let mut vill = run.world.entities.list.iter().find(|e| e.id == cart_id).and_then(|e| e.passengers.first().copied());
    if vill.is_none() && !f.empty {
        return Err("no rider".into());
    }
    run.own.push(cart_id);
    run.own.extend(vill);
    {
        let w = &mut run.world;
        let i = idx(w, cart_id).ok_or("cart gone")?;
        w.entities.list[i].pos = Vec3::new(start[0], start[1], start[2]);
        // (the real approach: the cart at rest on the queue's braked rail until the release)
        w.entities.list[i].delta_movement = if f.approach.is_some() { Vec3::new(0.0, 0.0, 0.0) } else { Vec3::new(-speed, 0.0, 0.0) };
        let ps = w.entities.list[i].passengers.clone();
        for pid in ps {
            if let Some(j) = idx(w, pid) {
                redsim::entity::position_rider(w, i, j);
            }
        }
    }
    let set = |w: &mut World, c: [i32; 3], st: &str| w.set_block(BlockPos::new(c[0], c[1], c[2]), block(&ctl_block(c, st)), redsim::world::flags::ALL);
    // (ironspec 17:46, the stand-in guard: every emulated write this bench makes, checked against
    // the built cell's golem and path properties before the run: the controller's levers, and the
    // queue's release with --module-offset)
    let (act_cell, act_ctrl) = act_cells(&run.world);
    let lay = layout(&run.world);
    for c in [HATCH_CTRL, HATCH_CTRL_HI, act_ctrl].into_iter().chain(lay.park_ctrl.iter().copied()) {
        // (a built always-on source the bench never writes ((A) with the run-up's activator torch)
        // is no stand-in: nothing to check)
        if ctl_block(c, "redstone_block") == ctl_block(c, "stone") {
            continue;
        }
        for st in ["redstone_block", "stone"] {
            emulated_write_check(&run.world, BlockPos::new(c[0], c[1], c[2]), &block(&ctl_block(c, st)), &f.declared_emulated)?;
        }
        // (the reach half: the built lever's own power reach is the target set)
        let p = BlockPos::new(c[0], c[1], c[2]);
        let on = block(&ctl_block(c, "redstone_block"));
        let built = standin_reach(&run.world, p, &run.world.get(p))?.1;
        row.standin.push(standin_reach_check(&run.world, p, &on, &built, &f.declared_emulated)?);
    }
    // (the queue's release, the conforming stand-in (ironspec 18:10): never a redstone block in the
    // release cell (a solid block there makes the cell over it an off-pad golem cell and a second
    // PANIC candidate at the seat). day-1's R1 (b8949c96 17:58): two repeaters on glass in a row
    // feeding the queue's rail from the side, the release cell beside the rail and the one behind
    // it, facing away from the rail (their output toward it), driven from behind: here a lever on
    // glass behind them, thrown at the release (the stand-in for day-1's gate output). The
    // repeaters and the lever keep their cells' golem and path properties; the glass under the
    // three must be declared (--declare-emulated) until day-1's release cells land in the module)
    if let Some(a) = f.approach
        && f.built_release.is_none()
    {
        let (cells, driver, facing) = release_cells(&a);
        let rep = format!("repeater[delay=1,facing={facing},locked=false,powered=false]");
        let lev = format!("lever[face=floor,facing={facing},powered=true]");
        for c in &cells {
            emulated_write_check(&run.world, BlockPos::new(c[0], c[1], c[2]), &block(&rep), &f.declared_emulated)?;
        }
        emulated_write_check(&run.world, BlockPos::new(driver[0], driver[1], driver[2]), &block(&lev), &f.declared_emulated)?;
        for c in cells.iter().chain([driver].iter()) {
            run.world.set_block(BlockPos::new(c[0], c[1] - 1, c[2]), block("glass"), redsim::world::flags::ALL);
        }
        for c in &cells {
            run.world.set_block(BlockPos::new(c[0], c[1], c[2]), block(&rep), redsim::world::flags::ALL);
        }
        run.world.set_block(BlockPos::new(driver[0], driver[1], driver[2]), block(&format!("lever[face=floor,facing={facing},powered=false]")), redsim::world::flags::ALL);
        // (the reach half: the driver's power reaches only the back repeater, each repeater only
        // the next one, the front one only the queue's rail; no undeclared rail T in their updates)
        let rail = BlockPos::new(a.seat[0].floor() as i32, a.seat[1].floor() as i32, a.seat[2].floor() as i32);
        let bp = |c: [i32; 3]| BlockPos::new(c[0], c[1], c[2]);
        row.standin.push(standin_reach_check(&run.world, bp(driver), &block(&lev), &[bp(cells[1])], &f.declared_emulated)?);
        let rep_on = format!("repeater[delay=1,facing={facing},locked=false,powered=true]");
        row.standin.push(standin_reach_check(&run.world, bp(cells[1]), &block(&rep_on), &[bp(cells[0])], &f.declared_emulated)?);
        row.standin.push(standin_reach_check(&run.world, bp(cells[0]), &block(&rep_on), &[rail], &f.declared_emulated)?);
    }
    // (ironspec 20:31 (4b), the guard's sixth property: survival: every laid block and every
    // stand-in survives where it stands, no rail pops)
    {
        let mut cells: Vec<BlockPos> = spec.blocks.iter().map(|(p, _)| *p).collect();
        if let Some(a) = f.approach
            && f.built_release.is_none()
        {
            let (cs, d, _) = release_cells(&a);
            for c in cs.iter().chain([d].iter()) {
                cells.push(BlockPos::new(c[0], c[1], c[2]));
                cells.push(BlockPos::new(c[0], c[1] - 1, c[2]));
            }
        }
        survival_check(&run.world, &cells)?;
    }
    // (ironspec 00:31: the module's declared emulated cells (record7's line-bench levers and their
    // glass), laid as its own bench lays them; each must survive where it stands)
    for (c, st) in &f.lay_emulated {
        let p = BlockPos::new(c[0], c[1], c[2]);
        let (before, after) = (run.world.get(p), block(st));
        // (ironspec 04:07: an override that doesn't change its cell fails the run)
        if before == after {
            return Err(format!("the emulated cell ({},{},{}) already holds {before}: an override that changes nothing fails the run (ironspec 04:07)", c[0], c[1], c[2]));
        }
        run.world.set_block(p, after, redsim::world::flags::ALL);
        if seed == f.record_seed && row.overrides.len() < 40 {
            row.overrides.push(format!("({},{},{}) {before} -> {}", c[0], c[1], c[2], run.world.get(p)));
        }
    }
    if !f.lay_emulated.is_empty() {
        survival_check(&run.world, &f.lay_emulated.iter().map(|(c, _)| BlockPos::new(c[0], c[1], c[2])).collect::<Vec<_>>())?;
    }
    // (ironspec 21:43 / 23:46: record-match against the design of record, on this world at the
    // visit's start after every bench write, the declared levers' night states aside (inputs, set
    // just below); the visit `record_seed` at `record_speed` only)
    if let Some(rc) = &f.record
        && seed == f.record_seed
        && (speed - f.record_speed).abs() < 1e-9
    {
        let m = crate::recordmatch::compare_world(&run.world, rc.margin, &rc.cells, &rc.ents, rc.offset, &rc.declared, &rc.name);
        let mut line = m.line();
        for d in m.undeclared().iter().take(12) {
            line.push_str(&format!("\n  undeclared ({},{},{}): bench {} vs record {}", d.cell[0], d.cell[1], d.cell[2], d.bench, d.record));
        }
        for d in m.undeclared_entities().iter().take(12) {
            line.push_str(&format!("\n  undeclared entity {} at {:?}: {}", d.ent.kind, d.ent.pos, if d.missing { "the bench lacks it" } else { "the record lacks it" }));
        }
        let block_ok = !matches!(m.counts(), crate::recordmatch::Counts::No(_));
        let mob_ok = !matches!(m.counts_mob(), crate::recordmatch::Counts::No(_));
        row.record_line = Some((line, block_ok, mob_ok));
    }
    // (ironspec 00:31, the re-runs on record7 itself: its declared line-bench levers in their night
    // states: REST_l's held ON, NOT-ready ON through the wait (the built R1 releases on its fall),
    // DAY at the detector's state (night: off))
    for c in &f.levers_on {
        set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), true)?;
    }
    if let Some(c) = f.built_release {
        set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), true)?;
    }
    let day_now = |w: &World| -> bool {
        let d = w.day_time().rem_euclid(24000);
        d >= f.day_on.unwrap_or(0) && d < f.day_off.unwrap_or(12_230)
    };
    if let Some(c) = f.day_lever {
        let on = day_now(&run.world);
        set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), on)?;
    }
    let mut was_asleep = false;
    let mut last_awake: Option<Vec3> = None;
    let mut last_v_pos: Option<Vec3> = None;
    let mut stand_run = 0i64;
    let mut last_cart = [0f64; 3];
    let mut still = 0;
    let mut crossed: Vec<[i32; 3]> = Vec::new();
    let mut hidden = false;
    let mut live = 0i64;
    let mut f_live: Option<i64> = None;
    let mut f_fired = false;
    let mut had_home = !f.empty && !f.no_home;
    // (a skip or a set hour: a napper that misses REST waits on the stop until the next REST)
    let limit = if f.skip.is_some() || f.hour.is_some() { 14_000 } else { 2400 } + f.approach.map_or(0, |a| a.wait + 200 + if f.skip.is_some() { 12_500 } else { 0 });
    let mut entry_seen = false;
    let mut arrival_set = false;
    let mut hold_set = false;
    let mut shift_pending: Option<f64> = None;
    // (ironspec 17:35 (a): the napper asleep on the tick before it was seated)
    let mut asleep_prev = false;
    let mut queue_released = false;
    let mut departed = false;
    let mut rest_seen = false;
    // (design A's DAY and abort state; the activator's power set for the visit's start)
    let mut day_state = { let d = run.world.day_time().rem_euclid(24000); d >= f.day_on.unwrap_or(0) && d < f.day_off.unwrap_or(12_230) };
    let mut day_since = 0i64;
    let mut abort_at: Option<i64> = None;
    let mut rider_kept = false;
    // (the napper's drops: its health and fall distance off the ground; its stay per cell)
    let mut airborne: Option<f32> = None;
    let mut max_fall = 0f64;
    let mut stay: std::collections::HashMap<[i32; 3], i64> = std::collections::HashMap::new();
    set(&mut run.world, act_ctrl, if f.day_gate && day_state { "stone" } else { "redstone_block" });
    let mut trace_last_path = String::new();
    let mut prev_stamp: Option<i64> = None;
    let mut bolt_seen: std::collections::HashMap<[i32; 3], (bool, Option<[i32; 3]>)> = std::collections::HashMap::new();
    for t in 1..=limit {
        // (a night skip at the start of tick t: the day time to the next day time 0, game time on)
        if f.skip == Some(t) {
            let dt = run.world.day_time();
            run.world.mob.day_offset += 24000 - dt.rem_euclid(24000);
            row.skipped_at = Some(t);
        }
        // (a save, stop and start between ticks t - 1 and t: a world open's N idle ticks of game time with
        // nothing ticking; every entity back as a new object under a new id, in the row's load order)
        if let Some((at, how)) = f.restart
            && t == at
        {
            crate::liveness::restart(&mut run.world, how)?;
            cart_id = crate::liveness::follow(&run.world, cart_id);
            vill = vill.map(|v| crate::liveness::follow(&run.world, v));
            zid = zid.map(|z| crate::liveness::follow(&run.world, z));
            // (ironspec 00:38: the PATH set right after every load)
            if let Some(vi) = vill.and_then(|v| idx(&run.world, v))
                && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
            {
                let p = redsim::entity::mob::ai::brain::ex(&run.world, vi).mem.path.as_ref().map(|p| (p.next_node_index, p.nodes.iter().map(|n| (n.x, n.y, n.z)).collect::<Vec<_>>()));
                row.path_after_load.push(format!("t {t}: {p:?}"));
            }
        }
        live += 1;
        // (the real approach: the queue's release, a redstone block in its input cell, once the
        // hold has elapsed and day-1's gate sees night (REST_l and NOT DAY: day time from `dusk`))
        if let Some(a) = f.approach
            && !queue_released
            && t >= a.wait
            && (a.dusk..24_000).contains(&run.world.day_time().rem_euclid(24000))
        {
            queue_released = true;
            row.queue_release_at = Some(t);
            if let Some(vi) = vill.and_then(|v| idx(&run.world, v))
                && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
            {
                let m = &redsim::entity::mob::ai::brain::ex(&run.world, vi).mem;
                row.state_at_queue_release = Some((m.cant_reach_since.map(|c| run.world.time - c), m.walk_target.is_some() || m.path.is_some()));
            }
            if let Some(c) = f.built_release {
                // (record7's built R1: NOT-ready falls; N on, the built repeaters power the queue rail)
                set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), false)?;
            } else {
                let (_, driver, facing) = release_cells(&a);
                run.world.set_block(BlockPos::new(driver[0], driver[1], driver[2]), block(&format!("lever[face=floor,facing={facing},powered=true]")), redsim::world::flags::ALL);
            }
        }
        // (record7's NOT-ready ON again from the departure: once the cart has left the seat's cell)
        if let (Some(c), Some(a)) = (f.built_release, f.approach)
            && queue_released
            && !departed
            && idx(&run.world, cart_id).is_some_and(|ci| {
                let b = run.world.entities.list[ci].block_position();
                [b.x, b.y, b.z] != [a.seat[0].floor() as i32, a.seat[1].floor() as i32, a.seat[2].floor() as i32]
            })
        {
            departed = true;
            set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), true)?;
        }
        // (DAY's lever: the daylight detector updates at game time % 20 only)
        if let Some(c) = f.day_lever
            && run.world.time % 20 == 0
        {
            let on = day_now(&run.world);
            set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), on)?;
        }
        // (the napper's leg state on arrival at the queue (ironspec 17:16, dwell 0/50): a stamp
        // armed `arrival_stamp` gt before the spawn and/or a held HOME run (its walk target set to
        // the bed, so the sink computes and keeps a run from the seat, partial there))
        if !arrival_set
            && (f.arrival_stamp.is_some() || f.arrival_run)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
            && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
        {
            arrival_set = true;
            let now = run.world.time;
            let m = &mut redsim::entity::mob::ai::brain::ex_mut(&mut run.world, vi).mem;
            if let Some(age) = f.arrival_stamp {
                m.cant_reach_since = Some(now - age);
            }
            if f.arrival_run {
                m.walk_target = Some(redsim::entity::mob::ai::brain::WalkTarget::at_point(Vec3::new(BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64, BED_HEAD[2] as f64 + 0.5), 0.5, 1));
            }
        }
        // (the held run's remaining hold (ironspec 17:52: b8949c96 measures the leg's run dropped
        // 88-160 gt after the arrival): once the sink runs, its end set that many gt on)
        if let Some(h) = f.arrival_hold
            && !hold_set
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
            && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
        {
            let now = run.world.time;
            if let Some(n) = redsim::entity::mob::ai::brain::ex_mut(&mut run.world, vi).nodes.iter_mut().find(|n| n.running && matches!(n.kind, redsim::entity::mob::ai::brain::tree::Kind::MoveToTargetSink { .. })) {
                n.end = now + h;
                hold_set = true;
            }
        }
        // (A6's entry: the napper's stamp as its cart first enters the entry cell; the positive
        // control (ironspec 14:58) arms it there if it isn't)
        if !entry_seen
            && let Some(ci) = idx(&run.world, cart_id)
            && run.world.entities.list[ci].block_position() == BlockPos::new(ENTRY[0], ENTRY[1], ENTRY[2])
        {
            entry_seen = true;
            row.entry_at = Some(t);
            if let Some(vi) = vill.and_then(|v| idx(&run.world, v))
                && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
            {
                let now = run.world.time;
                if f.forced_stamp && redsim::entity::mob::ai::brain::ex(&run.world, vi).mem.cant_reach_since.is_none() {
                    redsim::entity::mob::ai::brain::ex_mut(&mut run.world, vi).mem.cant_reach_since = Some(now);
                }
                row.stamp_at_entry = Some(redsim::entity::mob::ai::brain::ex(&run.world, vi).mem.cant_reach_since.map(|c| now - c));
            }
        }
        // (debug: MCTAS_BT_TP=x,y,z,T teleports the visitor there at tick T)
        if let Ok(s) = std::env::var("MCTAS_BT_TP") {
            let v: Vec<f64> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if v.len() == 4 && t == v[3] as i64
                && let Some(vi) = vill.and_then(|id| idx(&run.world, id))
            {
                run.world.entities.list[vi].pos = Vec3::new(v[0], v[1], v[2]);
            }
        }
        // ((A)'s arming detector: the first tick it is powered, this cart's passage)
        if let Some(a) = lay.arm
            && row.armed_at.is_none()
            && format!("{}", run.world.get(BlockPos::new(a[0], a[1], a[2]))).contains("powered=true")
        {
            row.armed_at = Some(t);
        }
        // (F: `after_sleep` gt after the lie-down (the bed observer's edge): the hatch shut, the
        // zombie seen; (A): and not before the arming detector's edge + `arm_delay` (the worst-case
        // braking interval, ironspec 00:37 (2b)))
        let armed_ok = lay.arm.is_none() || row.armed_at.is_some_and(|a| t >= a + f.arm_delay);
        if !f_fired && f_live.is_some_and(|x| live >= x) && armed_ok {
            f_fired = true;
            set(&mut run.world, HATCH_CTRL, "stone");
            set(&mut run.world, HATCH_CTRL_HI, "stone");
        }
        // (with `day_gate` (the 09:00 shape, a control now): DAY from the daylight detector (high
        // from `day_on` to `day_off`) gates the activator, and the abort, level-sensitive (DAY high,
        // no lie-down since the arrival, the park not released, the napper seated in its parked
        // cart: the activator unpowered when its cart passed), releases the park `ABORT_LOGIC_GT`
        // later. Without it (ironspec 09:45, outcome (b)) the activator is always on and nothing
        // releases an unslept napper: it waits for REST)
        let day = { let d = run.world.day_time().rem_euclid(24000); d >= f.day_on.unwrap_or(0) && d < f.day_off.unwrap_or(12_230) };
        if day != day_state {
            day_state = day;
            if f.day_gate {
                set(&mut run.world, act_ctrl, if day { "stone" } else { "redstone_block" });
            }
            if day {
                day_since = t;
            }
        }
        let w = &run.world;
        if f.day_gate && day_state && idx(w, cart_id).is_some_and(|ci| w.entities.list[ci].block_position() == BlockPos::new(act_cell[0], act_cell[1], act_cell[2])) {
            rider_kept = true;
        }
        if f.day_gate && day && row.slept_at.is_none() && row.release_at.is_none() && abort_at.is_none() {
            // (seated: the activator's state latched when the cart passed it; the control reads
            // the live DAY level instead)
            let in_cart = (rider_kept || f.live_day_seated) && row.parked.is_some();
            if in_cart {
                abort_at = Some(t + ABORT_LOGIC_GT);
                row.aborted = Some((t, t - day_since, "seated in its parked cart"));
            }
        }
        if abort_at == Some(t) {
            for &c in &lay.park_ctrl {
                set(&mut run.world, c, "redstone_block");
            }
            row.release_at = Some(t);
            row.stamp_at_release = vill.and_then(|v| idx(&run.world, v)).filter(|&vi| redsim::entity::mob::ai::brain::has_exact(&run.world, vi)).map(|vi| redsim::entity::mob::ai::brain::ex(&run.world, vi).mem.cant_reach_since.map(|c| run.world.time - c));
        }
        if f.hide_after_wake && !hidden && row.woke_at.is_some() {
            set(&mut run.world, HATCH_CTRL, "redstone_block");
            set(&mut run.world, HATCH_CTRL_HI, "redstone_block");
            hidden = true;
        }
        // (the release (ironspec 09:45, outcome (a)): `release_after` gt after the bed observer's
        // wake edge, whatever woke the napper (F's hatch, IDLE's WakeUp at day time 10, a dawn): the
        // pickup cell is its only stand-up cell, so the edge is its presence there. The control
        // (no presence gate): `release_after` gt after F, or 300 gt after the entry, up or not)
        // (G1's CHANGES 4 (3), the early-pickup bar's positive control: the park released at the
        // lie-down edge, the napper still in bed)
        let go = if let Some(n) = f.release_after_arm {
            // ((A)'s at-rest bar's positive control: the park released `n` gt after the arming
            // detector's edge, whatever the napper does)
            row.armed_at.is_some_and(|a| t >= a + n)
        } else if f.early_release {
            row.slept_at.is_some_and(|s| t >= s + release_after)
        } else if f.no_presence_gate {
            (f_fired && f_live.is_some_and(|x| live >= x + release_after)) || t >= 300
        } else {
            row.woke_at.is_some_and(|wk| t >= wk + release_after)
        };
        if row.release_at.is_none() && go {
            // ((A)'s at-rest bar, ironspec 00:37 (2b): the cart at rest at the park on the release
            // tick)
            if let Some(i) = idx(&run.world, cart_id) {
                let d = run.world.entities.list[i].delta_movement;
                let v = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
                row.speed_at_release = Some(v);
                if lay.arm.is_some() && v >= 0.001 {
                    row.problems.push(format!("t {t}: the cart moving at the release ({v:.4} b/t; the at-rest bar: under 0.001)"));
                }
            }
            for &c in &lay.park_ctrl {
                set(&mut run.world, c, "redstone_block");
            }
            row.release_at = Some(t);
            // (G1's note 1 on b36302af8, the pickup margin: the released cart set on the flat two
            // cells north of the pickup cell (-2,0,2) heading south at the forced speed)
            if let Some(v) = f.pickup_inject
                && let Some(i) = idx(&run.world, cart_id)
            {
                let w = &mut run.world;
                // (A6: two cells north of the pickup cell on the flats; (A): the park's slope, one
                // cell north of it, at the slope's middle; (A) with the run-up: the run-up's first
                // flat (0,0,4) heading west)
                let (pos, vel) = if lay.name == "(A) run-up" {
                    (Vec3::new(0.5, 0.0625, 4.5), Vec3::new(-v, 0.0, 0.0))
                } else if lay.arm.is_some() {
                    (Vec3::new(PARK_A[0] as f64 + 0.5, PARK_A[1] as f64 + 0.5625, PARK_A[2] as f64 + 0.5), Vec3::new(0.0, 0.0, v))
                } else {
                    (Vec3::new(SWEPT[0][0] as f64 + 0.5, SWEPT[0][1] as f64 + 0.0625, SWEPT[0][2] as f64 - 1.5), Vec3::new(0.0, 0.0, v))
                };
                w.entities.list[i].pos = pos;
                w.entities.list[i].delta_movement = vel;
            }
        }
        // (ironspec 20:31 (2): the parked cart moved along its rail by `park_shift` before the
        // release: x on an east-west rail, z on a north-south one, y with its slope)
        if let Some(d) = shift_pending.take()
            && let Some(i) = idx(&run.world, cart_id)
        {
            let w = &mut run.world;
            let p = w.entities.list[i].pos;
            let s = format!("{}", w.get(BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)));
            let (dx, dy, dz) = if s.contains("ascending_east") {
                (1.0, 1.0, 0.0)
            } else if s.contains("ascending_west") {
                (1.0, -1.0, 0.0)
            } else if s.contains("ascending_north") {
                (0.0, -1.0, 1.0)
            } else if s.contains("ascending_south") {
                (0.0, 1.0, 1.0)
            } else if s.contains("north_south") {
                (0.0, 0.0, 1.0)
            } else {
                (1.0, 0.0, 0.0)
            };
            w.entities.list[i].pos = Vec3::new(p.x + d * dx, p.y + d * dy, p.z + d * dz);
            w.entities.list[i].delta_movement = Vec3::new(0.0, 0.0, 0.0);
            row.park_moved = Some([p.x + d * dx, p.y + d * dy, p.z + d * dz]);
        }
        let before = idx(&run.world, cart_id).map(|i| run.world.entities.list[i].pos);
        // (the panic's tail: NEAREST_HOSTILE held on the zombie through `panic_entry` gt)
        if let Some(k) = f.panic_entry
            && t <= k
            && let (Some(vi), Some(z)) = (vill.and_then(|v| idx(&run.world, v)), zid)
        {
            redsim::entity::mob::ai::brain::brain_mut(&mut run.world, vi).nearest_hostile = Some(z);
        }
        // (ironspec 00:03 (2c): `--pulse-cell X,Y,Z@T=STATE`, a block set at tick T before the
        // tick, to measure the navigation's recompute trigger (vanilla's: a COLLISION-shape change
        // near the path's midpoint, `ServerLevel.sendBlockUpdated` -> `shouldRecomputePath`))
        for (p, at, st) in &f.pulses {
            if *at == t {
                let old = run.world.get(*p);
                let new = block(st);
                // (ironspec 04:07: an override that doesn't change its cell fails the run)
                if old == new {
                    return Err(format!("--pulse-cell ({},{},{}) at t {t}: the cell already holds {old} (an override that changes nothing fails the run, ironspec 04:07)", p.x, p.y, p.z));
                }
                let changed = redsim::entity::collide::boxes_at(&run.world, &old, *p) != redsim::entity::collide::boxes_at(&run.world, &new, *p);
                let before = vill.and_then(|v| idx(&run.world, v)).map(|vi| redsim::entity::mob::ai::ai(&run.world, vi).nav.time_last_recompute);
                run.world.set_block(*p, new.clone(), redsim::world::flags::ALL);
                let after = vill.and_then(|v| idx(&run.world, v)).map(|vi| redsim::entity::mob::ai::ai(&run.world, vi).nav.time_last_recompute);
                row.pulses.push(format!("t {t}: ({},{},{}) {old} -> {new}: collision shape changed {changed}; the napper's last path recompute {before:?} -> {after:?}", p.x, p.y, p.z));
            }
        }
        // (`MCTAS_BT_TRACE=shapes`: every collision-shape change within 12 of the napper, and every
        // recompute of its path, per tick)
        let shapes_trace = std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "shapes");
        let shape_snap = |w: &World, c: Vec3| -> std::collections::HashMap<BlockPos, (String, Vec<redsim::entity::Aabb>)> {
            let mut m = std::collections::HashMap::new();
            for x in (c.x.floor() as i32 - 12)..=(c.x.floor() as i32 + 12) {
                for y in (c.y.floor() as i32 - 8)..=(c.y.floor() as i32 + 8) {
                    for z in (c.z.floor() as i32 - 12)..=(c.z.floor() as i32 + 12) {
                        let p = BlockPos::new(x, y, z);
                        let b = w.get(p);
                        m.insert(p, (format!("{b}"), redsim::entity::collide::boxes_at(w, &b, p)));
                    }
                }
            }
            m
        };
        let pre = if shapes_trace { vill.and_then(|v| idx(&run.world, v)).map(|vi| (shape_snap(&run.world, run.world.entities.list[vi].pos), redsim::entity::mob::ai::ai(&run.world, vi).nav.time_last_recompute)) } else { None };
        run.tick();
        if let Some((snap, recompute0)) = pre
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
        {
            for (p, (s0, b0)) in &snap {
                let b = run.world.get(*p);
                let b1 = redsim::entity::collide::boxes_at(&run.world, &b, *p);
                if *b0 != b1 {
                    eprintln!("t {t}: collision shape changed at ({},{},{}): {s0} -> {b}", p.x, p.y, p.z);
                }
            }
            let r = redsim::entity::mob::ai::ai(&run.world, vi).nav.time_last_recompute;
            if r != recompute0 {
                eprintln!("t {t}: the napper's path recomputed (time_last_recompute {recompute0} -> {r})");
            }
        }
        let w = &run.world;
        // (the head-space check, ironspec 03:30: every villager's box and eye box against each
        // declared new solid cell, every tick)
        if !f.head_cells.is_empty() {
            for e in w.entities.list.iter().filter(|e| !e.removed) {
                let redsim::entity::EntityKind::Mob(m) = &e.kind else { continue };
                if m.ty != redsim::entity::mob::types::MobType::Villager {
                    continue;
                }
                let (wd, ht) = e.kind.dims();
                let (hw, ey) = (wd as f64 / 2.0, e.pos.y + e.kind.eye_height() as f64);
                let bx = redsim::entity::Aabb::new(e.pos.x - hw, e.pos.y, e.pos.z - hw, e.pos.x + hw, e.pos.y + ht as f64, e.pos.z + hw);
                let ew = wd as f64 * 0.4;
                let eb = redsim::entity::Aabb::new(e.pos.x - ew, ey - 5e-7, e.pos.z - ew, e.pos.x + ew, ey + 5e-7, e.pos.z + ew);
                for c in &f.head_cells {
                    let p = BlockPos::new(c[0], c[1], c[2]);
                    let boxes = redsim::entity::collide::boxes_at(w, &w.get(p), p);
                    if boxes.iter().any(|b| b.intersects(bx)) {
                        row.head_box_hits += 1;
                    }
                    if boxes.iter().any(|b| b.intersects(eb)) {
                        row.head_eye_hits += 1;
                        if row.head_eye_hits <= 3 {
                            row.problems.push(format!("t {t}: a villager's eye box at ({:.2},{:.2},{:.2}) meets the new solid cell ({},{},{}) (the head-space bar: 0)", e.pos.x, ey, e.pos.z, c[0], c[1], c[2]));
                        }
                    }
                }
            }
        }
        let Some(ci) = idx(w, cart_id) else {
            row.problems.push(format!("t {t}: the cart is gone"));
            break;
        };
        let c = &w.entities.list[ci];
        // (ironspec 21:25: the cart's contacts with blocks, per tick; a vertical one counts only off
        // a rail (on a rail the cart rides its own track))
        {
            let bp = c.block_position();
            let on_rail = redsim::rails::is_rail(&w.get(bp)) || redsim::rails::is_rail(&w.get(BlockPos::new(bp.x, bp.y - 1, bp.z)));
            let kind = if c.horizontal_collision {
                Some("horizontal")
            } else if c.vertical_collision && !on_rail {
                Some("vertical off a rail")
            } else {
                None
            };
            if let Some(k) = kind {
                let cell = [bp.x, bp.y, bp.z];
                if !f.declared_contacts.contains(&cell) && !row.contacts.iter().any(|x| x.2 == k && [x.1[0].floor() as i32, x.1[1].floor() as i32, x.1[2].floor() as i32] == cell) {
                    row.problems.push(format!("t {t}: the cart's {k} contact with a block at ({:.2},{:.2},{:.2}) (cell {},{},{}), not declared in the design's `# contacts:` (ironspec 21:25)", c.pos.x, c.pos.y, c.pos.z, bp.x, bp.y, bp.z));
                }
                row.contacts.push((t, [c.pos.x, c.pos.y, c.pos.z], k));
            }
        }
        let vi = vill.and_then(|id| idx(w, id));
        if vill.is_some() && vi.is_none() {
            row.problems.push(format!("t {t}: the villager is gone"));
            row.escaped.get_or_insert((t, [i32::MIN; 3]));
            break;
        }
        let mut riding = false;
        if let Some(vi) = vi {
            let v = &w.entities.list[vi];
            let vcell = [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32];
            riding = v.vehicle.is_some();
            // (the napper's HOME every tick; its loss is a problem)
            let has_home = redsim::entity::mob::ai::brain::brain(w, vi).home.is_some();
            if had_home && !has_home && row.home_lost_at.is_none() {
                row.home_lost_at = Some(t);
            }
            had_home = has_home;
            let asleep = redsim::entity::mob::mob(w, vi).sleeping_pos.is_some();
            // (ironspec 16:29, the escape row: the napper on foot on the entry or the approach (x 3
            // and east at z -4 and north, y 2 and up), or out of the stop's box)
            if !riding
                && !asleep
                && row.escaped.is_none()
                && ((vcell[0] >= ENTRY[0] && vcell[2] <= ENTRY[2] && vcell[1] >= ENTRY[1]) || !(-6..=5).contains(&vcell[0]) || !(-5..=6).contains(&vcell[2]) || !(-2..=6).contains(&vcell[1]))
            {
                row.escaped = Some((t, vcell));
                row.problems.push(format!("t {t}: the napper escaped on foot to {vcell:?}"));
            }
            // (a skip row: where the napper waits awake before its nap, and its CANT_REACH stamp's
            // age at the first REST tick after the skip)
            if row.skipped_at.is_some() && row.slept_after_skip.is_none() && !asleep && !riding && !row.waited.contains(&vcell) {
                row.waited.push(vcell);
            }
            if row.skipped_at.is_some() && asleep && row.slept_after_skip.is_none() && row.sleeps.last().is_some_and(|&s| row.skipped_at.is_some_and(|k| s >= k)) {
                row.slept_after_skip = Some(w.day_time().rem_euclid(24000));
            }
            if (row.skipped_at.is_some() || f.hour.is_some()) && !rest_seen && w.day_time().rem_euclid(24000) == 12_000 {
                rest_seen = true;
                let x = redsim::entity::mob::ai::brain::has_exact(w, vi).then(|| redsim::entity::mob::ai::brain::ex(w, vi).mem.cant_reach_since).flatten();
                row.stamp_at_rest = Some(x.map(|c| w.time - c));
            }
            row.last_slept = redsim::entity::mob::ai::brain::brain(w, vi).last_slept;
            // (ironspec 22:10, a held walk into A6 (C7): the stamp's largest age over the visit (the
            // spawn to out) and the tick it clears inside A6 (from the entry))
            if redsim::entity::mob::ai::brain::has_exact(w, vi) {
                let cr = redsim::entity::mob::ai::brain::ex(w, vi).mem.cant_reach_since;
                if let Some(c0) = cr {
                    row.stamp_visit_max = row.stamp_visit_max.max(w.time - c0);
                }
                if prev_stamp.is_some() && cr.is_none() && row.stamp_cleared_at.is_none() && row.entry_at.is_none_or(|e| t >= e) {
                    row.stamp_cleared_at = Some(t - row.entry_at.unwrap_or(0));
                }
                prev_stamp = cr;
            }
            // (ironspec 22:56 (iii): lightning never removes a napper mid-visit: its block never sees
            // the sky (no bolt aims at it), and no column top within a bolt's hit box of its box (x,
            // z +-3, y -3..+9 around the strike, `hit_box_at`); per distinct cell, the built world's
            // own heightmap (with --module-offset: the module over the stop))
            {
                let vb = BlockPos::new(v.pos.x.floor() as i32, v.pos.y.floor() as i32, v.pos.z.floor() as i32);
                let key = [vb.x, vb.y, vb.z];
                if !bolt_seen.contains_key(&key) {
                    let sky = redsim::weather::can_see_sky(w, vb).unwrap_or(true);
                    let bb = v.bb;
                    let mut reach: Option<[i32; 3]> = None;
                    for cx in (bb.min_x.floor() as i32 - 4)..=(bb.max_x.floor() as i32 + 4) {
                        for cz in (bb.min_z.floor() as i32 - 4)..=(bb.max_z.floor() as i32 + 4) {
                            let h = redsim::weather::motion_blocking_height(w, cx, cz);
                            if reach.is_none() && redsim::weather::lightning::hit_box_at(Vec3::new(cx as f64 + 0.5, h as f64, cz as f64 + 0.5)).intersects(bb) {
                                reach = Some([cx, h, cz]);
                            }
                        }
                    }
                    bolt_seen.insert(key, (sky, reach));
                }
                if let Some((sky, reach)) = bolt_seen.get(&key).copied()
                    && (sky || reach.is_some())
                {
                    let m = format!("the napper at cell ({},{},{}) {} (ironspec 22:56 (iii))", key[0], key[1], key[2], if sky { "sees the sky: a bolt can aim at it".to_string() } else { format!("is inside the hit box of a bolt on the column top {reach:?}") });
                    if !row.lightning.contains(&m) {
                        row.lightning.push(m);
                    }
                }
            }
            // (the napper's drops: the fall distance off the ground at the landing (or at a lie-down
            // in the air) and the health lost there; its stay per cell before its first nap or its
            // release; its lie-down's first chance; its stamp's age at calm REST ticks)
            let hp = redsim::entity::mob::mob(w, vi).health;
            if !riding && !asleep && !v.on_ground {
                if airborne.is_none() {
                    airborne = Some(hp);
                    max_fall = 0.0;
                }
                max_fall = max_fall.max(v.fall_distance);
            } else if let Some(h0) = airborne.take()
                && max_fall > 0.6
            {
                row.drops.push((max_fall, h0 - hp));
            }
            let dtod = w.day_time().rem_euclid(24000);
            let rest = !(10..12_000).contains(&dtod);
            if !riding && !asleep && row.sleeps.is_empty() && row.release_at.is_none() {
                *stay.entry(vcell).or_default() += 1;
            }
            let bc = Vec3::new(BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.5, BED_HEAD[2] as f64 + 0.5);
            if rest && !riding && !asleep && row.eligible_at.is_none() && v.pos.distance_to_sqr(bc) < 4.0 {
                row.eligible_at = Some(t);
            }
            if rest && !asleep && redsim::entity::mob::ai::brain::brain(w, vi).nearest_hostile.is_none() && redsim::entity::mob::ai::brain::has_exact(w, vi)
                && let Some(c0) = redsim::entity::mob::ai::brain::ex(w, vi).mem.cant_reach_since
            {
                let age = w.time - c0;
                row.rest_stamp_max = Some(row.rest_stamp_max.map_or(age, |m: i64| m.max(age)));
            }
            // (G1 09:43: a napper waiting through IDLE on foot before its nap: every walk target it
            // takes there (expected none), and whether its stamp arms there)
            if !rest && !riding && !asleep && row.sleeps.is_empty() && row.release_at.is_none() && redsim::entity::mob::ai::brain::has_exact(w, vi) {
                row.idle_before_nap = true;
                let x = redsim::entity::mob::ai::brain::ex(w, vi);
                if let Some(wt) = &x.mem.walk_target {
                    let p = match wt.target {
                        redsim::entity::mob::ai::brain::Tracker::Block(b) => [b.x, b.y, b.z],
                        redsim::entity::mob::ai::brain::Tracker::Point(q) => [q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32],
                        redsim::entity::mob::ai::brain::Tracker::Entity { .. } => [i32::MIN, 0, 0],
                    };
                    if !row.idle_walk_targets.contains(&p) {
                        row.idle_walk_targets.push(p);
                    }
                }
                if x.mem.cant_reach_since.is_some() {
                    row.idle_stamp = true;
                }
            }
            if !riding && row.dismount_at.is_none() && row.release_at.is_none() {
                // (the dismount itself: its tick and the rider's position there, before any
                // lie-down moves it)
                row.dismount_at = Some(t);
                row.landing_pos = Some([v.pos.x, v.pos.y, v.pos.z]);
            }
            if !riding && row.landed.is_none() && v.on_ground {
                row.landed = Some((vcell, t));
            }
            if asleep && !was_asleep {
                row.sleeps.push(t);
                // (SleepInBed's reach: the napper's feet on the tick before, to the bed head's centre)
                if let Some(p) = last_awake {
                    let c = Vec3::new(BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.5, BED_HEAD[2] as f64 + 0.5);
                    row.lie_dists.push(p.distance_to_sqr(c).sqrt());
                }
            }
            if !asleep && !riding {
                last_awake = Some(v.pos);
            }
            if asleep && row.slept_at.is_none() {
                row.slept_at = Some(t);
                f_live = Some(live + after_sleep);
            }
            if was_asleep && !asleep {
                row.woke_at = Some(t);
                row.wakes.push(t);
                row.stood = Some(vcell);
                row.stood_up.push(vcell);
                let bed = BlockPos::new(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2]);
                if let Block::Bed { facing, .. } = w.get(bed) {
                    for side in [facing.clockwise(), facing.clockwise().opposite()] {
                        let s = redsim::entity::mob::villager::stand_up_with_side(w, redsim::entity::mob::types::MobType::Villager, bed, facing, side);
                        let cell = s.map(|q| [q.x.floor() as i32, (q.y + 0.01).floor() as i32, q.z.floor() as i32]);
                        if !row.standup_check.contains(&cell) {
                            row.standup_check.push(cell);
                        }
                    }
                }
                let cc = c.block_position();
                if !row.cart_at_wake.contains(&[cc.x, cc.y, cc.z]) {
                    row.cart_at_wake.push([cc.x, cc.y, cc.z]);
                }
            }
            if row.woke_at.is_some() && !riding && !asleep && row.pickup.is_none() && !row.cells_after_wake.contains(&vcell) {
                row.cells_after_wake.push(vcell);
            }
            // (ironspec 14:23: every node of every path the napper's walks take, riding or on foot:
            // the declared-unreachable cell must never be one)
            if redsim::entity::mob::ai::brain::has_exact(w, vi)
                && let Some(p) = &redsim::entity::mob::ai::brain::ex(w, vi).mem.path
            {
                for n in &p.nodes {
                    row.path_nodes.insert([n.x, n.y, n.z]);
                }
            }
            // (after the wake: the woken napper's path off the pickup cell, by direction; north is up
            // the approach, into the arriving cart: a problem in any row)
            if row.woke_at.is_some()
                && !riding
                && !asleep
                && vcell == SWEPT[0]
                && let Some(d) = path_dir(w, vi)
            {
                if !row.paths_after_wake.contains(&d) {
                    row.paths_after_wake.push(d);
                }
                if f.restart.is_some_and(|(at, _)| t >= at) && !row.paths_after_restart.contains(&d) {
                    row.paths_after_restart.push(d);
                }
            }
            was_asleep = asleep;
            // (ironspec 11:01, the waker's places: every place the napper has the zombie in sight
            // within 8 (its hostile range): asleep, riding (its cart's cell) or on foot (its feet's
            // cell), with the least distance; its walk targets while panicking (the flee's and the
            // panic stroll's); its stamp's arming after the wake)
            if let Some(zi) = zid.and_then(|z| idx(w, z)) {
                let z = &w.entities.list[zi];
                let ze = Vec3::new(z.pos.x, z.pos.y + z.kind.eye_height() as f64, z.pos.z);
                let ve = Vec3::new(v.pos.x, v.pos.y + if asleep { 0.2 } else { v.kind.eye_height() as f64 }, v.pos.z);
                let d = ve.distance_to_sqr(ze).sqrt();
                if d <= 8.0 && redsim::entity::mob::los::has_line_of_sight(w, ze, ve) {
                    let place = if asleep {
                        "asleep".to_string()
                    } else if riding {
                        let cc = c.block_position();
                        format!("riding {},{},{}", cc.x, cc.y, cc.z)
                    } else {
                        format!("on foot {},{},{}", vcell[0], vcell[1], vcell[2])
                    };
                    let e = row.waker_seen.entry(place).or_insert((f64::MAX, f_fired));
                    e.0 = e.0.min(d);
                    e.1 &= f_fired;
                }
            }
            if redsim::entity::mob::ai::brain::has_exact(w, vi) {
                let x = redsim::entity::mob::ai::brain::ex(w, vi);
                if x.active == redsim::entity::mob::ai::brain::Act::Panic
                    && let Some(wt) = &x.mem.walk_target
                {
                    let p = match wt.target {
                        redsim::entity::mob::ai::brain::Tracker::Block(b) => [b.x, b.y, b.z],
                        redsim::entity::mob::ai::brain::Tracker::Point(q) => [q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32],
                        redsim::entity::mob::ai::brain::Tracker::Entity { .. } => [i32::MIN, 0, 0],
                    };
                    let from = if riding { format!("riding {},{},{}", c.block_position().x, c.block_position().y, c.block_position().z) } else { format!("on foot {},{},{}", vcell[0], vcell[1], vcell[2]) };
                    if !row.panic_targets.contains(&(from.clone(), p)) {
                        row.panic_targets.push((from, p));
                    }
                }
                if let Some(wk) = row.woke_at
                    && row.stamp_armed.is_none()
                    && let Some(c0) = x.mem.cant_reach_since
                    && c0 >= w.time - (t - wk)
                {
                    let at = if riding { format!("riding {},{},{}", c.block_position().x, c.block_position().y, c.block_position().z) } else { format!("on foot {},{},{}", vcell[0], vcell[1], vcell[2]) };
                    row.stamp_armed = Some((c0 - (w.time - (t - wk)), at));
                }
            }
            if std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "path") && redsim::entity::mob::ai::brain::has_exact(w, vi) {
                // (the held path's nodes in order, printed when they change)
                let p = redsim::entity::mob::ai::brain::ex(w, vi).mem.path.as_ref().map(|p| (p.next_node_index, p.nodes.iter().map(|n| (n.x, n.y, n.z)).collect::<Vec<_>>()));
                let s = format!("{p:?}");
                if s != trace_last_path {
                    eprintln!("t {t}: villager {:.3} {:.3} {:.3} riding {riding}: path (next index, nodes) {s}", v.pos.x, v.pos.y, v.pos.z);
                    trace_last_path = s;
                }
            }
            if std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "1" || x == "all") && (t % 5 == 0 || std::env::var("MCTAS_BT_TRACE").is_ok_and(|x| x == "all")) {
                let b = redsim::entity::mob::ai::brain::brain(w, vi);
                let nav = redsim::entity::mob::ai::brain::has_exact(w, vi).then(|| redsim::entity::mob::ai::brain::ex(w, vi).mem.path.as_ref().map(|p| (p.next_node_index, p.nodes.len(), p.nodes.last().map(|n| (n.x, n.y, n.z))))).flatten();
                eprintln!("t {t}: villager {:.3} {:.3} {:.3} v {:.3} {:.3} {:.3} riding {riding} asleep {asleep} home {:?} hostile {:?} path {:?} | cart {:.2} {:.2} {:.2} v {:.3} {:.3} {:.3}", v.pos.x, v.pos.y, v.pos.z, v.delta_movement.x, v.delta_movement.y, v.delta_movement.z, b.home, b.nearest_hostile, nav, c.pos.x, c.pos.y, c.pos.z, c.delta_movement.x, c.delta_movement.y, c.delta_movement.z);
            }
            if riding && row.release_at.is_some() && row.pickup.is_none() {
                row.pickup = Some((row.cells_after_wake.last().copied().unwrap_or(vcell), t));
                row.pickup_speed = Some(c.delta_movement.x.hypot(c.delta_movement.z));
                // (ironspec 17:35 (a), bar 0: a pickup before the wake, or off the bed while asleep)
                if row.woke_at.is_none() || asleep_prev {
                    row.pickup_unwoken = true;
                    row.problems.push(format!("t {t}: seated {} (the unbridged pickup)", if asleep_prev { "off the bed while asleep" } else { "before its wake" }));
                }
            }
            // (ironspec 00:03 / 00:07 (3): no stand-still after the eject over 20 gt: the longest
            // run of ticks awake on foot, unmoved (under 0.01), from the dismount to the first
            // lie-down or the pickup)
            if row.dismount_at.is_some() && !riding && !asleep && row.slept_at.is_none() && row.pickup.is_none() {
                let moved = last_v_pos.map_or(1.0, |p: Vec3| (v.pos.x - p.x).abs() + (v.pos.y - p.y).abs() + (v.pos.z - p.z).abs());
                stand_run = if moved < 0.01 { stand_run + 1 } else { 0 };
                row.stand_after_eject = row.stand_after_eject.max(stand_run);
            }
            last_v_pos = Some(v.pos);
            asleep_prev = asleep;
            if row.release_at.is_none() && row.landed.is_some() && riding && t > 5 {
                row.problems.push(format!("t {t}: the napper back in a cart before the release (at {vcell:?})"));
                break;
            }
        }
        // the cart: parked (still for 10 gt before the release), its speed over the swept cells
        let cp = [c.pos.x, c.pos.y, c.pos.z];
        let moved = (cp[0] - last_cart[0]).abs() + (cp[2] - last_cart[2]).abs() + (cp[1] - last_cart[1]).abs();
        still = if moved < 1e-4 { still + 1 } else { 0 };
        if still == 10 && row.parked.is_none() && row.release_at.is_none() {
            row.parked = Some((cp, t));
            shift_pending = f.park_shift;
        }
        last_cart = cp;
        // (ironspec 17:35 (a): the released cart in the pickup cell before the napper's wake)
        if row.release_at.is_some()
            && row.woke_at.is_none()
            && SWEPT.iter().any(|s| c.block_position() == BlockPos::new(s[0], s[1], s[2]))
        {
            row.cart_at_pickup_before_wake = true;
        }
        if let Some(bp) = before
            && row.release_at.is_some()
        {
            for s in SWEPT {
                let zc = s[2] as f64 + 0.5;
                if (bp.z < zc) != (c.pos.z < zc) && (c.pos.x - (s[0] as f64 + 0.5)).abs() < 0.3 && !crossed.contains(&s) {
                    crossed.push(s);
                    row.swept_speed.push((s, c.delta_movement.x.hypot(c.delta_movement.z)));
                }
            }
        }
        // (R and out: the exit runs east along R's row)
        let on_exit_row = (c.pos.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3;
        if on_exit_row && c.pos.x > EXIT_DET[0] as f64 + 0.5 && row.r_at.is_none() {
            row.r_at = Some(t);
        }
        // (ironspec 22:14 (3a): R's real rise, the detector rail powered (the cart's box meeting its
        // detection box), beside the emulated R above)
        if row.r_real_at.is_none() && format!("{}", w.get(BlockPos::new(EXIT_DET[0], EXIT_DET[1], EXIT_DET[2]))).contains("powered=true") {
            row.r_real_at = Some(t);
        }
        if on_exit_row && c.pos.x > EXIT_DET[0] as f64 + 1.5 {
            row.out_at = Some(t);
            row.seated_out = riding;
            break;
        }
    }
    if row.out_at.is_none() {
        row.problems.push("the cart not out in 2400 gt".into());
    }
    // (ironspec 10:40: the post-exit ride: the napper's HOME and its CANT_REACH stamp at calm REST
    // ticks for 1,500 gt after the exit, its cart run on to the run-out's stop; a stamp carried out
    // of the stop would drop HOME at its first calm REST tick past 1,200)
    if row.out_at.is_some()
        && let Some(v) = vill
    {
        let out_time = run.world.time;
        if let Some(vi) = idx(&run.world, v)
            && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
        {
            row.stamp_at_exit = Some(redsim::entity::mob::ai::brain::ex(&run.world, vi).mem.cant_reach_since.map(|c0| out_time - c0));
        }
        for _ in 0..1500 {
            run.tick();
            let w = &run.world;
            let Some(vi) = idx(w, v) else { break };
            let br = redsim::entity::mob::ai::brain::brain(w, vi);
            if br.home.is_none() && row.post_home_lost.is_none() {
                row.post_home_lost = Some(w.time - out_time);
            }
            let dtod = w.day_time().rem_euclid(24000);
            if redsim::entity::mob::ai::brain::has_exact(w, vi)
                && let Some(c0) = redsim::entity::mob::ai::brain::ex(w, vi).mem.cant_reach_since
            {
                if c0 > out_time && row.post_stamp_armed.is_none() {
                    row.post_stamp_armed = Some(c0 - out_time);
                }
                if !(10..12_000).contains(&dtod) && br.nearest_hostile.is_none() {
                    let age = w.time - c0;
                    row.post_stamp_max = Some(row.post_stamp_max.map_or(age, |m: i64| m.max(age)));
                }
            }
        }
    }
    let mut waits: Vec<([i32; 3], i64)> = stay.into_iter().filter(|&(_, n)| n >= 100).collect();
    waits.sort();
    row.long_waits = waits;
    if row.drops.iter().any(|&(_, d)| d > 0.0) {
        row.problems.push(format!("the napper hurt by a drop: {:?}", row.drops));
    }
    if row.rest_stamp_max.is_some_and(|a| a > 1000) {
        row.problems.push(format!("the napper's CANT_REACH stamp {} gt old at a calm REST tick", row.rest_stamp_max.unwrap_or(0)));
    }
    if row.out_at.is_some() && !row.seated_out {
        row.problems.push("a riderless cart left the stop".into());
    }
    if let Some(t) = row.home_lost_at {
        row.problems.push(format!("the napper's HOME lost at t {t}"));
    }
    if row.paths_after_wake.contains(&"north") {
        row.problems.push(if row.paths_after_restart.contains(&"north") { "a northward path off the pickup cell after the restart" } else { "a northward path off the pickup cell" }.into());
    }
    let expect_seated = !f.empty && !f.no_home && !f.hide_after_wake;
    if expect_seated && !row.seated_out {
        row.problems.push("the napper not seated".into());
    }
    if !f.hide_after_wake {
        for c in &row.cells_after_wake {
            if !SWEPT.contains(c) {
                row.problems.push(format!("the woken napper stood on {c:?}, outside the swept cells"));
            }
        }
    }
    if !run.world.errors.is_empty() {
        row.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    Ok(row)
}

/// G1's two-visit row on the lead's geometry (ironspec 00:52). Visit 1's napper A arrives at the
/// entry; the next napper C waits awake in its cart on the queue cell. R's pulse (A's cart past
/// R's detector) re-arms the stop (the park braked, the hatch open) and releases the queue:
/// C's cart reaches the entry `transit` gt later heading west at the visit's speed (emulated: the
/// trapdoor's 8-block drop and the run west, 9fc3155f's ~37 gt; the bed is free all that time).
/// A rides from R into its lift (emulated: its cart straight up from where it is `lift_delay` gt
/// after R, at `lift_speed` b/t, to local y 20, loop 2; farmopt's pick (c), past 17 about 30 gt
/// after R). The controller as in `bench`: F 10 gt after each visit's lie-down edge (re-armed per
/// arrival), the release 30 gt after F. A restart (`liveness::restart`) between two ticks.
/// Measured: A's distance to the bed head's centre at C's lie-down (the bar: over 17), both HOMEs
/// every tick and at the end, each visit's R.
///
/// The daylight row (`day_hold`, ironspec 02:33): A, picked before a night skip, reaches the queue
/// cell at day time `.0`; the queue's NOT DAY gate holds it there until day time `.1` (DAY off),
/// then it drops (its cart at the entry `transit` gt later); C, the night's first scheduled
/// napper, reaches the queue cell `.2` gt after A dropped and waits there for A's R.
#[derive(Debug, Clone, Copy)]
pub struct TwoOpts {
    pub restart: Option<(i64, crate::liveness::Restart)>,
    pub transit: i64,
    pub lift_delay: i64,
    pub lift_speed: f64,
    pub day_hold: Option<(i64, i64, i64)>,
    /// (the daylight row) C reaches the queue cell at this day time (its real first-napper time)
    /// instead of `day_hold.2` gt after A's drop.
    pub c_at: Option<i64>,
    /// (design A, ironspec 07:43) A night skip at tick T of A's visit: A's visit aborts (DAY high, A
    /// unslept) and C, at the queue, must be held until DAY falls (no cascade).
    pub skip: Option<i64>,
    /// DAY's fall at dusk outside the daylight rows (clear 12,230).
    pub day_off: i64,
    /// DAY's rise after day time 0 (the stale window after a skip; 0: at once).
    pub day_on: i64,
    /// The 09:00 shape (a control): DAY gates the activator, the abort releases an unslept napper.
    pub day_gate: bool,
    /// (ironspec 09:31) A at the entry at this day time (a stale-window arrival) instead of the
    /// night hour, C at the queue `day_hold.2`... (`c_at` if set) with the queue released on A's R.
    pub a_hour: Option<i64>,
    /// The controller's timing (ironspec 17:35-19:33's cuts): F this many gt after the lie-down
    /// edge (the CLI's default 2), the park's release this many gt after the wake edge (default 0);
    /// 10 / 30 are the pre-cut control.
    pub after_sleep: i64,
    pub release_after: i64,
    /// (ironspec 21:25) The cells where the design declares a cart contact (`# contacts:`).
    pub declared_contacts: &'static [[i32; 3]],
}

#[derive(Debug, Default, Clone)]
pub struct TwoRow {
    /// The world's day time at tick 0 (A's arrival).
    pub start_day: i64,
    /// (design A) The skip's tick, A's abort's tick.
    pub skipped_at: Option<i64>,
    pub a_aborted: Option<i64>,
    /// (the daylight row) A's drop from the queue, C's arrival at the queue cell.
    pub a_drop: Option<i64>,
    pub c_queued: Option<i64>,
    pub a_entry: Option<i64>,
    pub a_slept: Option<i64>,
    pub a_r: Option<i64>,
    pub c_entry: Option<i64>,
    pub c_slept: Option<i64>,
    pub a_dist_at_c_sleep: Option<f64>,
    pub c_r: Option<i64>,
    pub a_seated: bool,
    pub c_seated: bool,
    pub home_lost: Vec<(&'static str, i64)>,
    pub homes_end: (bool, bool),
    /// A napper awake on the pickup cell had a path north (toward the arriving cart), any row.
    pub north_path: bool,
    /// At each lie-down (A's and C's), the napper's feet the tick before to the bed head's centre.
    pub lie_dists: Vec<f64>,
    /// The nearest C came to a ledge cell while A was on foot in the stop.
    pub c_ledge_min: Option<f64>,
    /// (the 16-block rule, ValidateNearbyPoi) Per villager (A, C), calm-REST ticks within 16 of the
    /// bed head while another villager slept there; A's last tick inside 16 of the bed head before
    /// C's lie-down (ironspec 10:53: from A's leaving the ball to C's lie-down, bar 20 gt).
    pub calm16: [usize; 2],
    pub a_last_in_ball: Option<i64>,
    /// (G1's note 3 on b36302af8) A HOME loss inside the stop's box, and one outside it (the
    /// bench's queue cell, its lift's top).
    pub lost_in_stop: bool,
    pub lost_outside: bool,
    pub problems: Vec<String>,
}

pub fn two_visit_bench(spec: &FarmSpec, seed: u64, speed: f64, o: &TwoOpts) -> Result<TwoRow, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    // (the held-back daylight detectors, as the one-visit bench places them)
    run.place_detectors();
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    let want = o.day_hold.map(|d| d.0).or(o.a_hour).unwrap_or(13_800 + (seed as i64 * 37) % 400);
    let mut guard = 0;
    while run.world.day_time().rem_euclid(24000) != want {
        run.tick();
        guard += 1;
        if guard > 30_000 {
            return Err("never reached the visit's hour".into());
        }
    }
    let set = |w: &mut World, c: [i32; 3], st: &str| w.set_block(BlockPos::new(c[0], c[1], c[2]), block(&ctl_block(c, st)), redsim::world::flags::ALL);
    // (ironspec 17:46, the stand-in guard: the controller's levers checked against the built cells)
    let (act_cell, act_ctrl) = act_cells(&run.world);
    if layout(&run.world).arm.is_some() {
        return Err("the two-visit bench doesn't drive (A)'s park and arming gate yet (its one-visit bench does)".into());
    }
    for c in [HATCH_CTRL, HATCH_CTRL_HI, act_ctrl].into_iter().chain(PARK_CTRL) {
        for st in ["redstone_block", "stone"] {
            emulated_write_check(&run.world, BlockPos::new(c[0], c[1], c[2]), &block(&ctl_block(c, st)), &[])?;
        }
    }
    // (ironspec 20:31 (4b): survival: every laid block survives where it stands, no rail pops)
    survival_check(&run.world, &spec.blocks.iter().map(|(p, _)| *p).collect::<Vec<_>>())?;
    let h = BED_HEAD;
    let nbt = format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2]);
    // (move a cart to `at` with velocity `v`, its rider with it)
    let place = |w: &mut World, cid: u32, at: [f64; 3], v: Vec3| {
        if let Some(i) = w.entities.list.iter().position(|e| e.id == cid && !e.removed) {
            w.entities.list[i].pos = Vec3::new(at[0], at[1], at[2]);
            w.entities.list[i].delta_movement = v;
            let ps = w.entities.list[i].passengers.clone();
            for pid in ps {
                if let Some(j) = w.entities.list.iter().position(|e| e.id == pid) {
                    redsim::entity::position_rider(w, i, j);
                }
            }
        }
    };
    let spawn = |run: &mut redsim::entity::mob::farm::Run, at: [f64; 3], v: Vec3| -> Result<(u32, u32), String> {
        let cid = redsim::entity::spawn(&mut run.world, "minecraft:minecart", at, Some(&nbt))?;
        let vid = run.world.entities.list.iter().find(|e| e.id == cid).and_then(|e| e.passengers.first().copied()).ok_or("no rider")?;
        run.own.push(cid);
        run.own.push(vid);
        place(&mut run.world, cid, at, v);
        Ok((cid, vid))
    };
    let entry = [ENTRY[0] as f64 + 0.5, ENTRY[1] as f64 + 0.0625, ENTRY[2] as f64 + 0.5];
    let queue = [QUEUE[0] as f64 + 0.5, QUEUE[1] as f64 + 0.0625, QUEUE[2] as f64 + 0.5];
    if !matches!(run.world.get(BlockPos::new(QUEUE[0], QUEUE[1], QUEUE[2])), Block::PlainRail { .. }) {
        return Err("no queue rail: the two-visit row needs a `bedtier --bench` design".into());
    }
    // (the normal row: A at the entry, C waiting at the queue; the daylight row: A waiting at the
    // queue, C to come)
    let (mut cart1, mut a) = if o.day_hold.is_some() { spawn(&mut run, queue, Vec3::new(0.0, 0.0, 0.0))? } else { spawn(&mut run, entry, Vec3::new(-speed, 0.0, 0.0))? };
    let mut c2: Option<(u32, u32)> = if o.day_hold.is_some() { None } else { Some(spawn(&mut run, queue, Vec3::new(0.0, 0.0, 0.0))?) };
    let head = Vec3::new(BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.5, BED_HEAD[2] as f64 + 0.5);
    let mut row = TwoRow { start_day: want, a_entry: o.day_hold.is_none().then_some(0), ..Default::default() };
    let dropped = spec.blocks.iter().filter(|(p, _)| !run.world.contains(*p)).count();
    if dropped > 0 {
        row.problems.push(format!("{dropped} of the design's cells outside the world's box"));
    }
    let mut visit: u8 = 1;
    let mut live = 0i64;
    let mut arrival_live = 0i64;
    let mut f_live: Option<i64> = None;
    let mut f_fired = false;
    let mut released = false;
    let mut was_occ = false;
    let mut homes = [true, true];
    let mut last_awake2: [Option<Vec3>; 2] = [None, None];
    let mut was_asleep2 = [false, false];
    let mut lift_x: Option<f64> = None;
    let past_r = |w: &World, cid: u32| -> bool {
        w.entities.list.iter().find(|e| e.id == cid && !e.removed).is_some_and(|e| (e.pos.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3 && e.pos.x > EXIT_DET[0] as f64 + 0.5)
    };
    let limit = 4000 + o.day_hold.map_or(0, |d| (d.1 - d.0).rem_euclid(24000)) + if o.skip.is_some() || o.a_hour.is_some() { 13_000 } else { 0 };
    // (design A: DAY's fall at dusk (the daylight rows' gate time, else `day_off`); the activator's
    // power and the abort's state; the queue's release gated on NOT DAY (the queue's gate))
    let day_fall = o.day_hold.map_or(o.day_off, |d| d.1);
    let mut day_state = { let d = run.world.day_time().rem_euclid(24000); d >= o.day_on && d < day_fall };
    set(&mut run.world, act_ctrl, if o.day_gate && day_state { "stone" } else { "redstone_block" });
    let mut abort_at: Option<i64> = None;
    let mut visit_slept = false;
    let mut visit_woke: Option<i64> = None;
    let mut rider_kept = false;
    let mut arrival_t = 0i64;
    let mut c_drop: Option<i64> = None;
    for t in 1..=limit {
        // (a night skip at the start of tick t)
        if o.skip == Some(t) {
            let dt = run.world.day_time();
            run.world.mob.day_offset += 24000 - dt.rem_euclid(24000);
            row.skipped_at = Some(t);
        }
        if let Some((at, how)) = o.restart
            && t == at
        {
            crate::liveness::restart(&mut run.world, how)?;
            cart1 = crate::liveness::follow(&run.world, cart1);
            a = crate::liveness::follow(&run.world, a);
            c2 = c2.map(|(cc, cv)| (crate::liveness::follow(&run.world, cc), crate::liveness::follow(&run.world, cv)));
        }
        live += 1;
        // (the daylight row: the NOT DAY gate drops A at DAY off (its cart at the entry `transit`
        // gt later); C reaches the queue cell `.2` gt after the drop)
        if let Some((_, day_off, gap)) = o.day_hold {
            if row.a_drop.is_none() && run.world.day_time().rem_euclid(24000) == day_off {
                row.a_drop = Some(t);
            }
            if row.a_entry.is_none() && row.a_drop.is_some_and(|d| t >= d + o.transit) {
                place(&mut run.world, cart1, entry, Vec3::new(-speed, 0.0, 0.0));
                row.a_entry = Some(t);
                arrival_live = live;
                arrival_t = t;
            }
            if c2.is_none() && row.a_drop.is_some_and(|d| match o.c_at { Some(at) => t > d && run.world.day_time().rem_euclid(24000) == at, None => t >= d + gap }) {
                c2 = Some(spawn(&mut run, queue, Vec3::new(0.0, 0.0, 0.0))?);
                row.c_queued = Some(t);
            }
        }
        // (the queue's release: C drops at A's R, or when C reaches the queue if that is later, and
        // only while DAY is low (the queue's NOT DAY gate: an abort's R by day holds C until dusk);
        // its cart at the entry `transit` gt after the drop)
        let night_now = { let d = run.world.day_time().rem_euclid(24000); !(d >= o.day_on && d < day_fall) };
        if visit == 1 && c_drop.is_none() && c2.is_some() && night_now && row.a_r.is_some_and(|r| t >= r.max(row.c_queued.unwrap_or(0))) {
            c_drop = Some(t);
        }
        if visit == 1
            && let Some((cc, _)) = c2
            && c_drop.is_some_and(|d| t >= d + o.transit)
        {
            place(&mut run.world, cc, entry, Vec3::new(-speed, 0.0, 0.0));
            row.c_entry = Some(t);
            visit = 2;
            arrival_live = live;
            arrival_t = t;
        }
        // (F: a lie-down edge after the visit's arrival, +10; the release 30 after F)
        let occ = matches!(run.world.get(BlockPos::new(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2])), Block::Bed { occupied: true, .. });
        // (design A: DAY gates the activator; the abort as in `bench`, for the visit at the stop)
        let day = !night_now;
        if day != day_state {
            day_state = day;
            if o.day_gate {
                set(&mut run.world, act_ctrl, if day { "stone" } else { "redstone_block" });
            }
        }
        // (the seated term (the day-gate control): the activator unpowered when the visit's cart
        // passed it, latched)
        {
            let w = &run.world;
            let cid = if visit == 1 { cart1 } else { c2.map_or(cart1, |(cc, _)| cc) };
            if o.day_gate && day_state && idx(w, cid).is_some_and(|ci| w.entities.list[ci].block_position() == BlockPos::new(act_cell[0], act_cell[1], act_cell[2])) {
                rider_kept = true;
            }
        }
        if row.a_entry.is_some() {
            if occ && !was_occ && f_live.is_none() && live > arrival_live {
                f_live = Some(live + o.after_sleep);
                visit_slept = true;
            }
            // (the bed observer's wake edge: the napper up, on the pickup cell)
            if visit_slept && was_occ && !occ && visit_woke.is_none() {
                visit_woke = Some(t);
            }
            was_occ = occ;
            if !f_fired && f_live.is_some_and(|x| live >= x) {
                f_fired = true;
                set(&mut run.world, HATCH_CTRL, "stone");
                set(&mut run.world, HATCH_CTRL_HI, "stone");
            }
            // (the release 30 gt after the wake edge, whatever woke the napper)
            if !released && visit_woke.is_some_and(|wk| t >= wk + o.release_after) {
                released = true;
                for pc in PARK_CTRL {
                    set(&mut run.world, pc, "redstone_block");
                }
            }
            if o.day_gate && day && !visit_slept && !released && abort_at.is_none() {
                let w = &run.world;
                let cid = if visit == 1 { cart1 } else { c2.map_or(cart1, |(cc, _)| cc) };
                let parked = idx(w, cid).is_some_and(|ci| w.entities.list[ci].delta_movement.horizontal_distance_sqr() < 1e-6) && t > arrival_t + 20;
                if rider_kept && parked {
                    abort_at = Some(t + ABORT_LOGIC_GT);
                    if visit == 1 {
                        row.a_aborted = Some(t);
                    }
                }
            }
            if abort_at == Some(t) {
                released = true;
                for pc in PARK_CTRL {
                    set(&mut run.world, pc, "redstone_block");
                }
            }
        }
        run.tick();
        // (ironspec 21:25: each cart's contacts with blocks while it runs through the stop (A's
        // before its R, C's from its entry to its R; the emulated lift and the queue's wait aside);
        // a contact the design doesn't declare fails the row)
        for (id, live_in_stop) in [(Some(cart1), row.a_entry.is_some() && row.a_r.is_none()), (c2.map(|x| x.0), row.c_entry.is_some() && row.c_r.is_none())] {
            if let (Some(id), true) = (id, live_in_stop)
                && let Some(ci) = idx(&run.world, id)
            {
                let e = &run.world.entities.list[ci];
                let bp = e.block_position();
                let on_rail = redsim::rails::is_rail(&run.world.get(bp)) || redsim::rails::is_rail(&run.world.get(BlockPos::new(bp.x, bp.y - 1, bp.z)));
                let kind = if e.horizontal_collision {
                    Some("horizontal")
                } else if e.vertical_collision && !on_rail {
                    Some("vertical off a rail")
                } else {
                    None
                };
                if let Some(k) = kind
                    && !o.declared_contacts.contains(&[bp.x, bp.y, bp.z])
                {
                    let m = format!("the cart's {k} contact at cell {},{},{}, not declared (ironspec 21:25)", bp.x, bp.y, bp.z);
                    if !row.problems.iter().any(|p| p.ends_with(&m)) {
                        row.problems.push(format!("t {t}: {m}"));
                    }
                }
            }
        }
        if let Some(r) = row.a_r
            && t >= r + o.lift_delay
        {
            let x = *lift_x.get_or_insert_with(|| run.world.entities.list.iter().find(|e| e.id == cart1).map_or(EXIT_DET[0] as f64 + 0.5, |e| e.pos.x));
            let y = (EXIT_DET[1] as f64 + 0.0625 + o.lift_speed * (t - r - o.lift_delay) as f64).min(20.0625);
            place(&mut run.world, cart1, [x, y, EXIT_DET[2] as f64 + 0.5], Vec3::new(0.0, 0.0, 0.0));
        }
        let w = &run.world;
        if std::env::var_os("MCTAS_BT2_TRACE").is_some() && (t % 10 == 0 || std::env::var("MCTAS_BT2_TRACE").is_ok_and(|s| s.split(',').filter_map(|x| x.parse::<i64>().ok()).collect::<Vec<_>>().windows(2).any(|r| t >= r[0] && t <= r[1]))) {
            let p = |id: u32| w.entities.list.iter().find(|e| e.id == id).map(|e| format!("({:.2},{:.2},{:.2}){}{}", e.pos.x, e.pos.y, e.pos.z, if e.removed { " removed" } else { "" }, e.vehicle.map_or(String::new(), |v| format!(" in {v}"))));
            // (each napper's brain: HOME, CANT_REACH_WALK_TARGET_SINCE, WALK_TARGET, the activity)
            let brain_of = |id: u32| {
                idx(w, id).filter(|&i| redsim::entity::mob::ai::brain::has_exact(w, i)).map(|i| {
                    let x = redsim::entity::mob::ai::brain::ex(w, i);
                    format!("home {:?} cant_reach {:?} walk_target {} act {:?}", redsim::entity::mob::ai::brain::brain(w, i).home.map(|h| [h.x, h.y, h.z]), x.mem.cant_reach_since, x.mem.walk_target.is_some(), x.active)
                })
            };
            let br = brain_of(a);
            eprintln!("t {t} day {}: A {:?} cart1 {:?} {:?} | C {:?} {:?}", w.day_time().rem_euclid(24000), p(a), p(cart1), br, c2.map(|(cc, cv)| (p(cv), p(cc))), c2.and_then(|(_, cv)| brain_of(cv)));
        }
        let mut who = vec![(0usize, a, cart1)];
        if let Some((cc, cv)) = c2 {
            who.push((1, cv, cc));
        }
        for (k, id, cart) in who {
            let Some(i) = idx(w, id) else {
                row.problems.push(format!("t {t}: villager {} gone", if k == 0 { "A" } else { "C" }));
                return Ok(row);
            };
            let has = redsim::entity::mob::ai::brain::brain(w, i).home.is_some();
            if homes[k] && !has {
                row.home_lost.push((if k == 0 { "A" } else { "C" }, t));
                // (G1's note 3 on b36302af8: a loss inside the stop's box (local x -6..5, y -2..6, z
                // -5..6) against one outside it: the bench's queue cell, its lift's top)
                let p = w.entities.list[i].pos;
                if (-6.0..6.0).contains(&p.x) && (-2.0..7.0).contains(&p.y) && (-5.0..7.0).contains(&p.z) {
                    row.lost_in_stop = true;
                } else {
                    row.lost_outside = true;
                }
            }
            homes[k] = has;
            let asleep = redsim::entity::mob::mob(w, i).sleeping_pos.is_some();
            let seated = w.entities.list[i].vehicle == Some(cart) && released;
            // (a napper awake on the pickup cell with a path north, toward the cart: any row)
            let v = &w.entities.list[i];
            if v.vehicle.is_none()
                && !asleep
                && [v.pos.x.floor() as i32, (v.pos.y + 0.01).floor() as i32, v.pos.z.floor() as i32] == SWEPT[0]
                && path_dir(w, i) == Some("north")
            {
                row.north_path = true;
            }
            // (SleepInBed's reach at each lie-down: the feet the tick before, to the bed head's centre)
            if asleep && !was_asleep2[k] {
                if let Some(p) = last_awake2[k] {
                    row.lie_dists.push(p.distance_to_sqr(head).sqrt());
                }
            }
            if !asleep && v.vehicle.is_none() {
                last_awake2[k] = Some(v.pos);
            }
            was_asleep2[k] = asleep;
            // (the 16-block rule: a calm villager in REST within 16 of the bed head while another
            // sleeps there loses HOME to ValidateNearbyPoi; and A's last tick in the ball before C's
            // lie-down)
            let occ_now = matches!(w.get(BlockPos::new(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2])), Block::Bed { occupied: true, .. });
            let in_ball = w.entities.list[i].pos.distance_to_sqr(head) < 256.0;
            if occ_now && !asleep && in_ball && redsim::entity::mob::ai::brain::has_exact(w, i) && redsim::entity::mob::ai::brain::ex(w, i).active == redsim::entity::mob::ai::brain::Act::Rest {
                row.calm16[k] += 1;
            }
            if k == 0 && in_ball && row.a_r.is_some() && row.c_slept.is_none() {
                row.a_last_in_ball = Some(t);
            }
            // (ironspec 08:15, condition 4: the nearest other villager to the ledge's cells while A
            // is on foot in the stop, against the IDLE walk toward villagers within 8)
            if k == 1 && row.a_entry.is_some() && row.a_r.is_none()
                && let Some(ai) = idx(w, a)
                && w.entities.list[ai].vehicle.is_none()
            {
                let p = w.entities.list[i].pos;
                let d = LEDGE.iter().map(|c| p.distance_to_sqr(Vec3::new(c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5)).sqrt()).fold(f64::MAX, f64::min);
                row.c_ledge_min = Some(row.c_ledge_min.map_or(d, |m: f64| m.min(d)));
            }
            if k == 0 {
                if asleep && row.a_slept.is_none() {
                    row.a_slept = Some(t);
                }
                if visit == 1 && seated {
                    row.a_seated = true;
                }
            } else {
                if asleep && row.c_slept.is_none() && row.c_entry.is_some() {
                    row.c_slept = Some(t);
                    if let Some(ai) = idx(w, a) {
                        row.a_dist_at_c_sleep = Some(w.entities.list[ai].pos.distance_to_sqr(head).sqrt());
                    }
                }
                if visit == 2 && seated {
                    row.c_seated = true;
                }
            }
        }
        // (A's cart past R: R's pulse re-arms the stop and releases the queue)
        if row.a_r.is_none() && past_r(w, cart1) {
            row.a_r = Some(t);
            for pc in PARK_CTRL {
                set(&mut run.world, pc, "stone");
            }
            set(&mut run.world, HATCH_CTRL, "redstone_block");
            set(&mut run.world, HATCH_CTRL_HI, "redstone_block");
            f_live = None;
            f_fired = false;
            released = false;
            abort_at = None;
            visit_slept = false;
            visit_woke = None;
            rider_kept = false;
        }
        if visit == 2
            && row.c_r.is_none()
            && let Some((cc, _)) = c2
            && past_r(&run.world, cc)
        {
            row.c_r = Some(t);
            break;
        }
    }
    row.homes_end = (homes[0], homes[1]);
    if !row.a_seated || row.a_r.is_none() {
        row.problems.push("A not seated out".into());
    }
    if row.c_slept.is_none() {
        row.problems.push("C never lay down".into());
    }
    if let Some(d) = row.a_dist_at_c_sleep
        && d <= 17.0
    {
        row.problems.push(format!("A within 17 of the bed's head at C's lie-down ({d:.2}; an inherited margin over (a1)'s 16)"));
    }
    if !row.c_seated || row.c_r.is_none() {
        row.problems.push("C not seated out".into());
    }
    if !row.home_lost.is_empty() {
        row.problems.push(format!("HOME lost {:?}", row.home_lost));
    }
    if row.north_path {
        row.problems.push("a northward path off the pickup cell".into());
    }
    if !run.world.errors.is_empty() {
        row.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    Ok(row)
}

/// `--orders`: `all` (the helper's 10 rows) or a list of save, reverse, seed:K.
pub fn orders(s: &str) -> Result<Vec<crate::liveness::Restart>> {
    if s.trim() == "all" {
        return Ok(crate::liveness::Restart::rows());
    }
    s.split(',').map(|x| crate::liveness::Restart::parse(x).ok_or_else(|| anyhow::anyhow!("--orders: {x} is not save, reverse or seed:K"))).collect()
}

/// ironspec's placement-order rule (farmopt 03:38): every cell the guide's build order leaves unlike
/// the design (a rail whose natural shape under the order is not its designed one: a curve ending
/// at no rail, a slope with no rail above its high end), as `crate::placing::diff_from_file` finds
/// it after a build in order (20 gt between placements). Each needs a listed order or a temporary
/// helper rail.
pub fn placement_order_cells(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Built { gap: Some(20) }, &crate::placing::BuildPlan::default())?;
    crate::placing::diff_from_file(&run.world, spec, &ro)
}

/// ironspec 04:46: the zombie's boat and its rider at bench start (the benches' start: the judge's
/// placement, the design's entities spawned) and 20 gt later, each against the post cell.
pub fn zombie_post_lines(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let mut out = Vec::new();
    for t in [0, 20] {
        for _ in 0..t {
            run.tick();
        }
        for e in run.world.entities.list.iter().filter(|e| !e.removed && e.id >= first) {
            let inside = e.pos.x >= ZOMBIE_POST[0] as f64 && e.pos.x <= ZOMBIE_POST[0] as f64 + 1.0 && e.pos.z >= ZOMBIE_POST[2] as f64 && e.pos.z <= ZOMBIE_POST[2] as f64 + 1.0;
            let name = match &e.kind {
                redsim::entity::EntityKind::Boat(_) => "the boat".to_string(),
                redsim::entity::EntityKind::Mob(m) => format!("the {:?}", m.ty),
                _ => "an entity".to_string(),
            };
            // (a mob's eye: the sky light there (ironspec's zombie-post check: under 15, so it never
            // sees the sky and never burns))
            let sky = matches!(e.kind, redsim::entity::EntityKind::Mob(_)).then(|| {
                let eye = BlockPos::new(e.pos.x.floor() as i32, (e.pos.y + e.kind.eye_height() as f64).floor() as i32, e.pos.z.floor() as i32);
                format!(", eye cell ({},{},{}) sky light {}", eye.x, eye.y, eye.z, redsim::daylight::sky_light(&run.world, eye).map_or_else(|x| x, |l| l.to_string()))
            });
            out.push(format!("t {t}: {name} at ({:.3}, {:.3}, {:.3}): {}{}", e.pos.x, e.pos.y, e.pos.z, if inside { "inside the post cell" } else { "OUTSIDE the post cell" }, sky.unwrap_or_default()));
        }
    }
    Ok(out)
}

/// The waker's distance and sight (ironspec 07:11: they must hold after any move): from the
/// zombie's eye to the sleeper's eye (in the bed: the head cell's centre at y + 0.6875, its eye 0.2
/// up) and to a napper standing on the pickup cell (eye 1.62), with the hatch open (as built) and
/// shut (F: its controls off).
pub fn waker_lines(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.tick();
    let eye = run
        .world
        .entities
        .list
        .iter()
        .find(|e| e.id >= first && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Zombie))
        .map(|e| Vec3::new(e.pos.x, e.pos.y + e.kind.eye_height() as f64, e.pos.z))
        .ok_or("no zombie")?;
    let sleeper = Vec3::new(BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.6875 + 0.2, BED_HEAD[2] as f64 + 0.5);
    let stander = Vec3::new(SWEPT[0][0] as f64 + 0.5, SWEPT[0][1] as f64 + 1.62, SWEPT[0][2] as f64 + 0.5);
    let on_bed = Vec3::new(BED_TOP[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.5625 + 1.62, BED_TOP[2] as f64 + 0.5);
    let mut out = Vec::new();
    for (state, ctrl) in [("hiding (its trapdoors open)", "redstone_block"), ("showing (its trapdoors shut: F)", "stone")] {
        for c in [HATCH_CTRL, HATCH_CTRL_HI] {
            run.world.set_block(BlockPos::new(c[0], c[1], c[2]), block(&ctl_block(c, ctrl)), redsim::world::flags::ALL);
        }
        for _ in 0..4 {
            run.tick();
        }
        for (who, p) in [("the sleeper", sleeper), ("a napper standing on the pickup cell", stander), ("a napper standing on the bed (A6's waiter)", on_bed)] {
            out.push(format!("hatch {state}: the zombie's eye to {who}'s eye {:.2}, sight {}", eye.distance_to_sqr(p).sqrt(), if redsim::entity::mob::los::has_line_of_sight(&run.world, eye, p) { "clear" } else { "blocked" }));
        }
        // (ironspec 10:04, condition 2: every walk cell (the ledge, the bed's foot roof) sees nothing
        // either way)
        let seen: Vec<String> = LEDGE.iter().chain([LANDING2].iter()).filter(|c| redsim::entity::mob::los::has_line_of_sight(&run.world, eye, Vec3::new(c[0] as f64 + 0.5, c[1] as f64 + 1.62, c[2] as f64 + 0.5))).map(|c| format!("{c:?}")).collect();
        out.push(format!("hatch {state}: the walk's {} cells and the second landing seeing the zombie (eye 1.62 over the cell's centre): {}", LEDGE.len(), if seen.is_empty() { "none".to_string() } else { seen.join(" ") }));
    }
    // (ironspec 11:30: the waker burns only at sky light 15 in its eye's cell, which needs a clear
    // column straight up; the hatch is beside the column, not in it)
    let ec = BlockPos::new(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32);
    let top = spec.blocks.iter().map(|(p, _)| p.y).max().unwrap_or(ec.y);
    let col: Vec<String> = (ec.y..=top).map(|y| format!("{}: {:?}", y, run.world.get(BlockPos::new(ec.x, y, ec.z)))).collect();
    out.push(format!("the zombie's eye cell ({},{},{}) and the column over it to the stop's top: {}", ec.x, ec.y, ec.z, col.join(", ")));
    Ok(out)
}

/// A rail block's shape (plain, powered, activator or detector).
fn rail_shape(b: &Block) -> Option<redsim::block::RailShape> {
    match b {
        Block::Rail { shape, .. } | Block::PlainRail { shape, .. } | Block::DetectorRail { shape, .. } => Some(*shape),
        _ => None,
    }
}

/// The cells a rail's shape points to (its raw connections; an ascending end one level up).
fn raw_connections(p: BlockPos, s: redsim::block::RailShape) -> [BlockPos; 2] {
    use redsim::block::RailShape::*;
    let at = |dx: i32, dy: i32, dz: i32| BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
    match s {
        NorthSouth => [at(0, 0, -1), at(0, 0, 1)],
        EastWest => [at(-1, 0, 0), at(1, 0, 0)],
        AscendingEast => [at(-1, 0, 0), at(1, 1, 0)],
        AscendingWest => [at(-1, 1, 0), at(1, 0, 0)],
        AscendingNorth => [at(0, 1, -1), at(0, 0, 1)],
        AscendingSouth => [at(0, 0, -1), at(0, 1, 1)],
        SouthEast => [at(0, 0, 1), at(1, 0, 0)],
        SouthWest => [at(0, 0, 1), at(-1, 0, 0)],
        NorthWest => [at(0, 0, -1), at(-1, 0, 0)],
        NorthEast => [at(0, 0, -1), at(1, 0, 0)],
    }
}

/// farmopt's rail rules (ironspec's detector rule, 03:54; the sloped switched rail's neighbours,
/// 03:55): the stop's Ts (a rail with rails on 3 or more sides, each side's rail at its level, one
/// up or one down: vanilla's neighbour search), any T among a detector's raw connections, and any T
/// face-adjacent to the cell above a sloped switched rail (powered, activator or detector:
/// `BaseRailBlock.affectNeighborsAfterRemoval` updates it).
pub fn rail_rule_lines(spec: &FarmSpec) -> Vec<String> {
    use std::collections::HashMap;
    let rails: HashMap<BlockPos, (redsim::block::RailShape, &Block)> = spec.blocks.iter().filter_map(|(p, b)| rail_shape(b).map(|s| (*p, (s, b)))).collect();
    let rail_near = |c: BlockPos| [c, BlockPos::new(c.x, c.y + 1, c.z), BlockPos::new(c.x, c.y - 1, c.z)].into_iter().find(|q| rails.contains_key(q));
    let sides = |p: BlockPos| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dz)| rail_near(BlockPos::new(p.x + dx, p.y, p.z + dz)).is_some()).count();
    let mut out = Vec::new();
    let mut ts: Vec<BlockPos> = rails.keys().copied().filter(|&p| sides(p) >= 3).collect();
    ts.sort_by_key(|p| (p.y, p.x, p.z));
    out.push(format!("Ts (a rail with rails on 3+ sides): {}", if ts.is_empty() { "none".to_string() } else { ts.iter().map(|p| format!("({},{},{})", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ") }));
    let mut dets: Vec<(BlockPos, redsim::block::RailShape)> = rails.iter().filter(|(_, (_, b))| matches!(b, Block::DetectorRail { .. })).map(|(p, (s, _))| (*p, *s)).collect();
    dets.sort_by_key(|(p, _)| (p.y, p.x, p.z));
    let at: std::collections::HashMap<BlockPos, &Block> = spec.blocks.iter().map(|(p, b)| (*p, b)).collect();
    for (p, s) in dets {
        let conns: Vec<String> = raw_connections(p, s).into_iter().map(|c| match rail_near(c) {
            Some(q) => format!("({},{},{}) {} sides", q.x, q.y, q.z, sides(q)),
            None => format!("({},{},{}) no rail", c.x, c.y, c.z),
        }).collect();
        let bad = raw_connections(p, s).into_iter().filter_map(rail_near).any(|q| sides(q) >= 3);
        out.push(format!("detector ({},{},{}): raw connections {}{}", p.x, p.y, p.z, conns.join(", "), if bad { " — A T: needs an official probe" } else { "; none with 3" }));
        // (G1's full reach set: the 6 face neighbours of the detector and of the cell below it, each
        // cell's block and, for a rail, its sides; no plain rail at 3)
        let below = BlockPos::new(p.x, p.y - 1, p.z);
        for base in [p, below] {
            let cells: Vec<String> = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)]
                .iter()
                .map(|(dx, dy, dz)| BlockPos::new(base.x + dx, base.y + dy, base.z + dz))
                .map(|c| {
                    let name = at.get(&c).map_or("air".to_string(), |b| format!("{b:?}"));
                    let n = if rails.contains_key(&c) { format!(", {} sides{}", sides(c), if sides(c) >= 3 && matches!(at.get(&c), Some(Block::PlainRail { .. })) { " — A PLAIN RAIL AT 3" } else { "" }) } else { String::new() };
                    format!("({},{},{}) {name}{n}", c.x, c.y, c.z)
                })
                .collect();
            out.push(format!("  reach set around ({},{},{}): {}", base.x, base.y, base.z, cells.join("; ")));
        }
    }
    let mut sloped: Vec<BlockPos> = rails.iter().filter(|(_, (s, b))| matches!(b, Block::Rail { .. } | Block::DetectorRail { .. }) && matches!(s, redsim::block::RailShape::AscendingEast | redsim::block::RailShape::AscendingWest | redsim::block::RailShape::AscendingNorth | redsim::block::RailShape::AscendingSouth)).map(|(p, _)| *p).collect();
    sloped.sort_by_key(|p| (p.y, p.x, p.z));
    for p in sloped {
        let q = BlockPos::new(p.x, p.y + 1, p.z);
        let near: Vec<String> = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().map(|(dx, dy, dz)| BlockPos::new(q.x + dx, q.y + dy, q.z + dz)).filter(|n| rails.contains_key(n) && sides(*n) >= 3).map(|n| format!("({},{},{})", n.x, n.y, n.z)).collect();
        out.push(format!("sloped switched rail ({},{},{}): Ts face-adjacent to the cell above it: {}", p.x, p.y, p.z, if near.is_empty() { "none".to_string() } else { near.join(" ") }));
    }
    out
}

fn list<T: std::str::FromStr>(s: Option<&str>) -> Result<Vec<T>>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    // (each comma-separated item a value, or a range `a..b` (both ends in) with an optional step
    // `a..b/s`, for integer lists)
    let mut out = Vec::new();
    for x in s.into_iter().flat_map(|s| s.split(',')) {
        let x = x.trim();
        if let Some((a, rest)) = x.split_once("..") {
            let (b, step) = rest.split_once('/').unwrap_or((rest, "1"));
            let (a, b, step): (i64, i64, i64) = (a.trim().parse()?, b.trim().parse()?, step.trim().parse()?);
            let mut v = a;
            while v <= b {
                out.push(v.to_string().parse()?);
                v += step.max(1);
            }
        } else {
            out.push(x.parse()?);
        }
    }
    Ok(out)
}

/// Runs `f` over `items` on `jobs` threads (each with a 256 MB stack), the results in the items'
/// order.
fn par_map<T: Sync, R: Send>(items: &[T], jobs: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: Vec<std::sync::Mutex<Option<R>>> = items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1).min(items.len().max(1)) {
            let _ = std::thread::Builder::new().stack_size(256 << 20).spawn_scoped(s, || {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let r = f(&items[i]);
                    *out[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
                }
            });
        }
    });
    out.into_iter().map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()).expect("every item ran")).collect()
}

/// `mctas mobfarm bedtier-bench`: one row (each visit printed) or a sweep (each row's clean count)
/// with the totals. `sweeps`: the restart phases (each at every order of `orders`), the visit day
/// times, the skip phases, the restarts after each skip, the panic tails at the entry.
#[allow(clippy::too_many_arguments)]
pub fn bench_cmd(spec: &FarmSpec, from: u64, to: u64, speeds: &str, after_sleep: i64, release_after: i64, day: bool, base: &Faults, sweeps: [Option<&str>; 7], orders: &[crate::liveness::Restart], jobs: usize) -> Result<()> {
    let sp: Vec<f64> = list(Some(speeds))?;
    // (ironspec 19:18: entry to R per entry speed against the lane's timing bar)
    let r_bar = if base.r_bar > 0 { base.r_bar } else { 209 };
    let f_approach = base.approach.is_some();
    let (rp, hs, kp, ra): (Vec<i64>, Vec<i64>, Vec<i64>, Vec<i64>) = (list(sweeps[0])?, list(sweeps[1])?, list(sweeps[2])?, list(sweeps[3])?);
    let pe: Vec<i64> = list(sweeps[4])?;
    let pi: Vec<f64> = list(sweeps[5])?;
    let qw: Vec<i64> = list(sweeps[6])?;
    let mut rows: Vec<(String, Faults)> = Vec::new();
    for &p in &rp {
        for &how in orders {
            rows.push((format!("restart +{p} {}", how.label()), Faults { restart: Some((p, how)), ..base.clone() }));
        }
    }
    for &k in &kp {
        if ra.is_empty() {
            rows.push((format!("skip +{k}"), Faults { skip: Some(k), ..base.clone() }));
        }
        // (ironspec 07:56, point 3: a restart during the post-skip walk, `ra` gt after the skip)
        for &a in &ra {
            for &how in orders {
                rows.push((format!("skip +{k}, restart +{} {}", k + a, how.label()), Faults { skip: Some(k), restart: Some((k + a, how)), ..base.clone() }));
            }
        }
    }
    if rows.is_empty() {
        rows.push(("night".to_string(), base.clone()));
    }
    if !hs.is_empty() {
        rows = rows.into_iter().flat_map(|(n, f)| hs.iter().map(move |&h| (format!("{n}, hour {h}"), Faults { hour: Some(h), ..f.clone() }))).collect();
    }
    if !pe.is_empty() {
        rows = rows.into_iter().flat_map(|(n, f)| pe.iter().map(move |&k| (format!("{n}, panicking through entry + {k}"), Faults { panic_entry: Some(k), ..f.clone() }))).collect();
    }
    if !pi.is_empty() {
        rows = rows.into_iter().flat_map(|(n, f)| pi.iter().map(move |&v| (format!("{n}, the released cart at {v} b/t two cells before the pickup cell"), Faults { pickup_inject: Some(v), ..f.clone() }))).collect();
    }
    if !qw.is_empty()
        && let Some(a) = base.approach
    {
        rows = rows.into_iter().flat_map(|(n, f)| qw.iter().map(move |&w| (format!("{n}, released {w} gt after the spawn at the queue"), Faults { approach: Some(Approach { wait: w, ..a }), ..f.clone() }))).collect();
    }
    // (ironspec 20:31 (2): the release from every park position, the parked cart moved along its
    // rail by each shift before the release)
    if !base.park_shifts.is_empty() {
        let shifts = base.park_shifts.clone();
        rows = rows.into_iter().flat_map(|(n, f)| shifts.clone().into_iter().map(move |d| (format!("{n}, parked {d:+.3} along its rail"), Faults { park_shift: Some(d), ..f.clone() }))).collect();
    }
    let one = rows.len() == 1;
    // (each seed and speed's night exit, for each row's exit delay)
    let mut night_r = std::collections::HashMap::new();
    let mut pairs: Vec<(u64, f64)> = Vec::new();
    for seed in from..=to {
        for &v in &sp {
            pairs.push((seed, v));
        }
    }
    if !one {
        for (&(seed, v), r) in pairs.iter().zip(par_map(&pairs, jobs, |&(seed, v)| bench(spec, seed, v, release_after, day, after_sleep, &Faults { approach: base.approach, declared_emulated: base.declared_emulated.clone(), ..Faults::default() }))) {
            if let Ok(r) = r
                && let Some(rr) = r.r_at
            {
                night_r.insert((seed, v.to_bits()), rr);
            }
        }
    }
    // (every visit, run on `jobs` threads, read back in the rows' order)
    let tasks: Vec<(usize, u64, f64)> = (0..rows.len()).flat_map(|ri| pairs.iter().map(move |&(s, v)| (ri, s, v))).collect();
    let mut results = par_map(&tasks, jobs, |&(ri, seed, v)| bench(spec, seed, v, release_after, day, after_sleep, &rows[ri].1)).into_iter();
    let (mut tot, mut clean, mut seated, mut not_out, mut lost) = (0, 0, 0, 0, 0);
    // (after a restart, the woken napper's paths off the pickup cell: visits with each direction)
    let mut dirs: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
    let mut dirs_wake: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
    // (the lie-downs' reach to the bed head's centre: the most, against SleepInBed's 2)
    let mut lie_max = 0f64;
    let mut lie_min = f64::MAX;
    let (mut min_pick, mut max_pick) = (f64::MAX, 0f64);
    let mut delays: Vec<i64> = Vec::new();
    let mut sk = SkipTotals::default();
    // (record-match, computed once: the visit `from` at the first speed, on the base row's world
    // after every bench write; the rows themselves don't repeat it)
    if base.record.is_some()
        && let Some(&v0) = sp.first()
    {
        let fr = Faults { record_seed: from, record_speed: v0, ..base.clone() };
        match bench(spec, from, v0, release_after, day, after_sleep, &fr) {
            Ok(r) => sk.record_line = r.record_line,
            Err(e) => println!("record-match's visit refused: {e}"),
        }
    }
    let rows: Vec<(String, Faults)> = rows.into_iter().map(|(n, f)| (n, Faults { record: None, ..f })).collect();
    for (name, f) in &rows {
        let (mut ok, mut bad) = (0, Vec::new());
        let (mut napped, mut seated_here, mut lie_downs) = (0, 0, Vec::<i64>::new());
        for seed in from..=to {
            for &v in &sp {
                tot += 1;
                match results.next().unwrap_or_else(|| Err("no result".into())) {
                    Ok(r) => {
                        seated += r.seated_out as usize;
                        seated_here += r.seated_out as usize;
                        not_out += r.out_at.is_none() as usize;
                        lost += r.home_lost_at.is_some() as usize;
                        for d in &r.paths_after_restart {
                            *dirs.entry(d).or_default() += 1;
                        }
                        for d in &r.paths_after_wake {
                            *dirs_wake.entry(d).or_default() += 1;
                        }
                        for &d in &r.lie_dists {
                            lie_max = lie_max.max(d);
                            lie_min = lie_min.min(d);
                        }
                        if let Some(s) = r.pickup_speed {
                            min_pick = min_pick.min(s);
                            max_pick = max_pick.max(s);
                        }
                        if let Some(s) = r.slept_at {
                            napped += 1;
                            if let Some(h) = f.hour {
                                lie_downs.push(h + s);
                            }
                        }
                        if let (Some(rr), Some(nr)) = (r.r_at, night_r.get(&(seed, v.to_bits()))) {
                            delays.push(rr - nr);
                        }
                        // (a skip row's outcome)
                        if let Some((_, d, kind)) = r.aborted {
                            *sk.aborts.entry(kind).or_default() += 1;
                            sk.abort_delays.push(d + ABORT_LOGIC_GT);
                        }
                        if let Some(s) = r.stamp_at_release {
                            match s {
                                Some(a) => sk.release_stamps.push(a),
                                None => sk.release_no_stamp += 1,
                            }
                        }
                        sk.stood_up.extend(r.stood_up.iter().copied());
                        sk.standup_check.extend(r.standup_check.iter().copied());
                        sk.cart_at_wake.extend(r.cart_at_wake.iter().copied());
                        // (every row: the drops, the lie-down's first chance, the long waits, the
                        // stamp at calm REST ticks, riderless carts out, boat seatings)
                        for &(fd, dmg) in &r.drops {
                            sk.drops += 1;
                            sk.drop_max = sk.drop_max.max(fd);
                            sk.hurt += (dmg > 0.0) as usize;
                        }
                        if let (Some(e), Some(s)) = (r.eligible_at, r.slept_at) {
                            sk.gaps.push(s - e);
                        }
                        for &(c, n) in &r.long_waits {
                            let x = sk.waits.entry(c).or_default();
                            x.0 += 1;
                            x.1 = x.1.max(n);
                        }
                        if let Some(a) = r.rest_stamp_max {
                            sk.rest_stamp_max = sk.rest_stamp_max.max(a);
                        }
                        sk.riderless += r.problems.iter().any(|p| p.contains("riderless")) as usize;
                        sk.boat_all += r.problems.iter().any(|p| p.contains("back in a cart")) as usize;
                        for &p in &r.idle_walk_targets {
                            *sk.idle_targets.entry(p).or_default() += 1;
                        }
                        sk.idle_stamps += r.idle_stamp as usize;
                        sk.post_lost += r.post_home_lost.is_some() as usize;
                        if let Some(a) = r.post_stamp_max {
                            sk.post_stamp_max = sk.post_stamp_max.max(a);
                        }
                        if let Some(a) = r.post_home_lost {
                            sk.post_lost_at.push(a);
                        }
                        if let Some(a) = r.post_stamp_armed {
                            sk.post_armed.push(a);
                        }
                        match r.stamp_at_exit {
                            Some(None) => sk.exit_no_stamp += 1,
                            Some(Some(a)) => sk.exit_stamps.push(a),
                            None => {}
                        }
                        for (p, &(d, after_f)) in &r.waker_seen {
                            let e = sk.waker_seen.entry(p.clone()).or_insert((0, f64::MAX, true));
                            e.0 += 1;
                            e.1 = e.1.min(d);
                            e.2 &= after_f;
                        }
                        for pt in &r.panic_targets {
                            *sk.panic_targets.entry(pt.clone()).or_default() += 1;
                        }
                        if let Some((k, at)) = &r.stamp_armed {
                            sk.armed_after_wake.push(*k);
                            *sk.armed_at.entry(at.clone()).or_default() += 1;
                        }
                        sk.most_drops = sk.most_drops.max(r.drops.len());
                        sk.path_nodes.extend(r.path_nodes.iter().copied());
                        match r.stamp_at_entry {
                            Some(None) => sk.entry_no_stamp += 1,
                            Some(Some(a)) => sk.entry_stamps.push(a),
                            None => {}
                        }
                        if let Some(e) = r.entry_at {
                            sk.entry_ats.push(e);
                        }
                        if let Some((_, c)) = r.escaped {
                            *sk.escapes.entry(c).or_default() += 1;
                        }
                        sk.pickup_unwoken += r.pickup_unwoken as usize;
                        sk.cart_before_wake += r.cart_at_pickup_before_wake as usize;
                        if sk.standin.is_empty() {
                            sk.standin = r.standin.clone();
                        }
                        // (ironspec 20:31 (2): the pickup speed and the seating by park position)
                        if let Some(m) = r.park_moved.or(r.parked.map(|p| p.0)) {
                            let e = sk.pickup_by_park.entry(format!("{:.2},{:.2},{:.2}", m[0], m[1], m[2])).or_default();
                            e.2 += 1;
                            if let Some(v) = r.pickup_speed.filter(|_| r.seated_out) {
                                e.0.push(v);
                                e.1 += 1;
                            }
                        }
                        if let Some((s, held)) = r.state_at_queue_release {
                            sk.release_states.push((s, held));
                        }
                        if let (Some(q), Some(e)) = (r.queue_release_at, r.entry_at) {
                            sk.release_to_entry.push(e - q);
                        }
                        // (ironspec 09:45's outcome classes: (a) slept in the REST it came in and
                        // picked up; (b) waited on foot through IDLE, napped at the next REST and
                        // left; (c) released unslept by the abort)
                        let class = match (r.slept_at.is_some(), r.idle_before_nap, r.aborted.is_some()) {
                            (true, false, _) => "a",
                            (true, true, _) => "b",
                            (false, _, true) => "c",
                            _ => "none",
                        };
                        *sk.classes.entry(class).or_default() += 1;
                        // (G1's CHANGES 4 (4): every visit counted at its speed; a visit that never
                        // reaches R, or (with --module-offset) whose entry isn't recorded, fails
                        // that speed, never silently dropped or measured from the spawn)
                        *sk.visits_by_speed.entry(format!("{:.3}", r.speed)).or_default() += 1;
                        sk.fluid_scheduled = sk.fluid_scheduled.max(r.fluid_scheduled);
                        if sk.record_line.is_none() {
                            sk.record_line = r.record_line.clone();
                        }
                        if sk.path_after_load.len() < 8 {
                            sk.path_after_load.extend(r.path_after_load.iter().map(|p| format!("seed {} speed {}: {p}", r.seed, r.speed)));
                        }
                        if sk.overrides.is_empty() && !r.overrides.is_empty() {
                            sk.overrides = r.overrides.clone();
                        }
                        if r.pickup.is_some() {
                            sk.pickups += 1;
                        }
                        if let Some(p) = r.landing_pos {
                            *sk.landings.entry(format!("{},{},{}", p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32)).or_default() += 1;
                            let c = [BED_HEAD[0] as f64 + 0.5, BED_HEAD[1] as f64 + 0.5, BED_HEAD[2] as f64 + 0.5];
                            let d = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt();
                            sk.landing_dist_max = sk.landing_dist_max.max(d);
                        }
                        if let (Some(a), Some((_, pk))) = (r.armed_at, r.parked) {
                            sk.arm_to_park.push(pk - a);
                        }
                        if let Some(v) = r.speed_at_release {
                            sk.release_speed_max = sk.release_speed_max.max(v);
                        }
                        // ((A): the pickup at least 60 gt after the dismount, the napper's boarding
                        // cooldown (`Entity.boardingCooldown`, set on stopRiding))
                        if let (Some(d), Some((_, pk))) = (r.dismount_at, r.pickup) {
                            sk.cooldown_margin = Some(sk.cooldown_margin.map_or(pk - d - 60, |m: i64| m.min(pk - d - 60)));
                        }
                        if !r.contacts.is_empty() {
                            sk.contact_visits += 1;
                            for (_, p, k) in &r.contacts {
                                *sk.contacts.entry(format!("{k} at cell {},{},{}", p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32)).or_default() += 1;
                            }
                        }
                        sk.stamp_visit_max = sk.stamp_visit_max.max(r.stamp_visit_max);
                        sk.stand_max = sk.stand_max.max(r.stand_after_eject);
                        sk.head_box_hits += r.head_box_hits;
                        sk.head_eye_hits += r.head_eye_hits;
                        if r.stand_after_eject > 20 {
                            sk.stand_over_20 += 1;
                        }
                        if !r.lightning.is_empty() {
                            sk.lightning_visits += 1;
                            for m in &r.lightning {
                                *sk.lightning.entry(m.clone()).or_default() += 1;
                            }
                        }
                        if let (Some(e), Some(rr)) = (r.r_at, r.r_real_at) {
                            sk.r_real_by_speed.entry(format!("{:.3}", r.speed)).or_default().push(e - rr);
                        } else if r.r_at.is_some() {
                            sk.r_real_missing += 1;
                        }
                        if let Some(c) = r.stamp_cleared_at {
                            sk.stamp_cleared.push(c);
                        }
                        if let Some(rr) = r.r_at {
                            sk.r_ats.push(rr);
                            match (f_approach, r.entry_at) {
                                (false, _) => sk.r_by_speed.entry(format!("{:.3}", r.speed)).or_default().push(rr),
                                (true, Some(e)) => sk.r_by_speed.entry(format!("{:.3}", r.speed)).or_default().push(rr - e),
                                (true, None) => sk.entry_missing += 1,
                            }
                        }
                        // (G1's note (b) on c0a745cfa: the timing band on the real R too, the
                        // detector's rise)
                        if let Some(rr) = r.r_real_at {
                            let from = if f_approach { r.entry_at } else { Some(0) };
                            if let Some(e) = from {
                                sk.r_real_entry_by_speed.entry(format!("{:.3}", r.speed)).or_default().push(rr - e);
                            }
                        }
                        if let Some(s) = r.slept_at {
                            sk.lie_ticks.push(s);
                        }
                        if let Some(wk) = r.woke_at {
                            sk.wake_ticks.push(wk);
                        }
                        if let Some(k) = r.skipped_at {
                            let e = sk.skip_class.entry(class).or_insert((k, k));
                            e.0 = e.0.min(k);
                            e.1 = e.1.max(k);
                        }
                        if let Some(k) = r.skipped_at {
                            sk.visits += 1;
                            sk.boat += r.problems.iter().any(|p| p.contains("back in a cart")) as usize;
                            if r.sleeps.iter().any(|&s| s < k) {
                                sk.slept_before += 1;
                            }
                            match r.slept_after_skip {
                                Some(d) if d < 12_000 => sk.tail_naps.push(d),
                                Some(d) => sk.rest_naps.push(d),
                                None => sk.no_nap += r.sleeps.iter().all(|&s| s < k) as usize,
                            }
                            sk.waited.extend(r.waited.iter().copied());
                            match r.stamp_at_rest {
                                Some(Some(a)) => sk.stamps.push(a),
                                Some(None) => sk.no_stamp += 1,
                                None => {}
                            }
                            sk.home_lost += r.home_lost_at.is_some() as usize;
                            sk.seated += r.seated_out as usize;
                        }
                        if one {
                            println!("seed {seed} speed {v}: landed {:?}, parked {:?}, slept {:?}, woke {:?}, release {:?}, pickup {:?} at {:?} b/t, R {:?}, out {:?}, seated {}{}", r.landed, r.parked.map(|(p, t)| (format!("{:.2},{:.2},{:.2}", p[0], p[1], p[2]), t)), r.slept_at, r.woke_at, r.release_at, r.pickup, r.pickup_speed.map(|s| format!("{s:.3}")), r.r_at, r.out_at, r.seated_out, if r.problems.is_empty() { String::new() } else { format!(" PROBLEMS {:?}", r.problems) });
                            for m in &r.pulses {
                                println!("  pulse {m}");
                            }
                        }
                        if r.problems.is_empty() {
                            ok += 1;
                        } else {
                            bad.push(format!("seed {seed} speed {v}: {:?}", r.problems));
                        }
                    }
                    Err(e) => {
                        // (a refusal or a run error: printed in every mode, and it fails the run)
                        if one {
                            println!("seed {seed} speed {v}: ERROR {e}");
                        }
                        sk.errors += 1;
                        bad.push(format!("seed {seed} speed {v}: ERROR {e}"))
                    }
                }
            }
        }
        clean += ok;
        if !one && !bad.is_empty() {
            println!("{name}: {ok} of {} clean; {}", ok + bad.len(), bad.iter().take(3).cloned().collect::<Vec<_>>().join("; "));
        }
        if f.hour.is_some() && f.skip.is_none() {
            lie_downs.sort();
            println!("{name}: napped {napped}, seated out {seated_here}; lie-downs at day time {:?}..{:?} (REST ends at 24010)", lie_downs.first(), lie_downs.last());
        }
    }
    delays.sort();
    let pct = |p: f64| delays.get(((delays.len() as f64 - 1.0) * p).round() as usize).copied();
    println!("bed stop bench: {tot} visits ({} rows), {clean} clean, seated {seated}; not out {not_out}; HOME lost {lost}; pickup speeds {:.3}..{:.3}; exit later than the night's (gt): min {:?}, median {:?}, p99 {:?}, max {:?}; after a restart, visits with a path off the pickup cell by direction {dirs:?}; after the wake in any row {dirs_wake:?}; lie-downs' reach to the bed head's centre {:.4}..{:.4} (SleepInBed's bar 2)", rows.len(), if min_pick == f64::MAX { 0.0 } else { min_pick }, max_pick, delays.first(), pct(0.5), pct(0.99), delays.last(), if lie_min == f64::MAX { 0.0 } else { lie_min }, lie_max);
    if sk.visits > 0 {
        sk.tail_naps.sort();
        sk.rest_naps.sort();
        sk.stamps.sort();
        let span = |v: &Vec<i64>| format!("{} ({:?}..{:?})", v.len(), v.first(), v.last());
        println!(
            "skip rows: {} visits, seated out {}, in the zombie's boat {}, HOME lost {}; asleep at the skip {}; napped after the skip in REST's tail (day time 0..9) {}, at the next REST {}, not at all {}; waited awake in {:?}; CANT_REACH stamp at the next REST: none {}, aged {} gt",
            sk.visits,
            sk.seated,
            sk.boat,
            sk.home_lost,
            sk.slept_before,
            span(&sk.tail_naps),
            span(&sk.rest_naps),
            sk.no_nap,
            sk.waited,
            sk.no_stamp,
            span(&sk.stamps)
        );
    }
    if !sk.aborts.is_empty() {
        sk.abort_delays.sort();
        sk.release_stamps.sort();
        println!("aborts (design A: DAY high, no lie-down since the arrival, the napper present): {:?}; abort delay (DAY high to the park's release) {:?}..{:?} gt; the napper's CANT_REACH stamp at the release: none {}, aged {} ({:?}..{:?}) gt", sk.aborts, sk.abort_delays.first(), sk.abort_delays.last(), sk.release_no_stamp, sk.release_stamps.len(), sk.release_stamps.first(), sk.release_stamps.last());
    }
    println!("stand-up cells at every wake: {:?}; findStandUpPosition's own answer at every wake, both sides: {:?}; the cart's cell at the wakes: {:?}", sk.stood_up, sk.standup_check, sk.cart_at_wake);
    sk.gaps.sort();
    sk.r_ats.sort();
    sk.lie_ticks.sort();
    sk.wake_ticks.sort();
    println!("first lie-down at entry + {:?}..{:?}, first wake at entry + {:?}..{:?}; skip offsets by outcome class (min, max): {:?}", sk.lie_ticks.first(), sk.lie_ticks.last(), sk.wake_ticks.first(), sk.wake_ticks.last(), sk.skip_class);
    println!("entry to R (the stop clear for the next napper): {:?} / median {:?} / {:?} gt over {} visits; outcome classes (ironspec 09:45: a slept and picked up, b waited through IDLE then napped, c the abort's unslept return): {:?}", sk.r_ats.first(), sk.r_ats.get(sk.r_ats.len() / 2), sk.r_ats.last(), sk.r_ats.len(), sk.classes);
    // (ironspec 19:18 (2a): entry to R per entry speed, and v_slow against the timing bar)
    let mut speed_failed = false;
    if sk.visits_by_speed.len() > 1 {
        // (every speed with visits; n of m reached R; a speed missing any R fails v_slow)
        let (rbs, vbs) = (&mut sk.r_by_speed, &sk.visits_by_speed);
        let mut per: Vec<(f64, Option<(i64, i64, i64)>, usize, usize)> = vbs
            .iter()
            .map(|(k, &m)| {
                let v = rbs.get_mut(k).map(|v| {
                    v.sort();
                    v.clone()
                });
                let n = v.as_ref().map_or(0, |v| v.len());
                (k.parse::<f64>().unwrap_or(0.0), v.filter(|v| !v.is_empty()).map(|v| (v[0], v[v.len() / 2], v[v.len() - 1])), n, m)
            })
            .collect();
        per.sort_by(|a, b| a.0.total_cmp(&b.0));
        let ok = |p: &(f64, Option<(i64, i64, i64)>, usize, usize)| p.2 == p.3 && p.1.is_some_and(|s| s.2 <= r_bar);
        let v_slow = per.iter().take_while(|p| ok(p)).last().map(|p| p.0);
        let own = per.iter().filter(|p| ok(p)).map(|p| p.0).fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))));
        speed_failed = per.iter().any(|p| p.2 < p.3);
        println!(
            "entry to R by entry speed (ironspec 19:18; min / median / max gt over every row at that speed, n of m visits out): {}",
            per.iter().map(|p| format!("{:.3} {} ({} of {} out)", p.0, p.1.map_or("none".to_string(), |s| format!("{}/{}/{}", s.0, s.1, s.2)), p.2, p.3)).collect::<Vec<_>>().join(", ")
        );
        println!("v_slow (every speed up to it with every visit out and a maximum of {r_bar} gt or less): {v_slow:?}; the largest such speed: {own:?}");
    }
    if !sk.r_real_entry_by_speed.is_empty() {
        let mut all: Vec<i64> = sk.r_real_entry_by_speed.values().flatten().copied().collect();
        all.sort();
        println!(
            "entry to the REAL R (the detector's rise; G1's note (b): the timing band on the real R) by entry speed, min / median / max gt: {}; over every visit: {:?}..{:?}",
            sk.r_real_entry_by_speed
                .iter_mut()
                .map(|(k, v)| {
                    v.sort();
                    format!("{k} {}/{}/{}", v[0], v[v.len() / 2], v[v.len() - 1])
                })
                .collect::<Vec<_>>()
                .join(", "),
            all.first(),
            all.last()
        );
    }
    if sk.entry_missing > 0 {
        println!("visits out with no recorded A6 entry: {} (a failure: entry to R not measured from the spawn)", sk.entry_missing);
    }
    println!(
        "the carts' contacts with blocks, the spawn to out (ironspec 21:25; declared in the design's `# contacts:`: {:?}; an undeclared one fails its row): {} visits with a contact; by cell (contact ticks) {:?}",
        base.declared_contacts, sk.contact_visits, sk.contacts
    );
    sk.stamp_cleared.sort();
    println!(
        "lightning (ironspec 22:56 (iii): a napper's cell that sees the sky, or inside a bolt's hit box from a column top, in the built world (the module over the stop with --module-offset; the bench's stop alone is open to the sky)): {} visits; {:?}",
        sk.lightning_visits,
        sk.lightning.iter().take(12).collect::<Vec<_>>()
    );
    println!(
        "R's real rise (ironspec 22:14 (3a): the detector rail at (4,-1,5) powered) against the emulated R (the cart's centre past x 4.5), the emulated minus the real, min / median / max gt per speed: {}; visits with an emulated R and no real rise: {}",
        sk.r_real_by_speed
            .iter_mut()
            .map(|(k, v)| {
                v.sort();
                format!("{k} {}/{}/{}", v[0], v[v.len() / 2], v[v.len() - 1])
            })
            .collect::<Vec<_>>()
            .join(", "),
        sk.r_real_missing
    );
    println!(
        "the stamp over each visit (ironspec 22:10 (i)/(ii), a held walk into A6): its largest age {} gt (bar 1,000, physical 1,200); cleared inside A6 in {} visits, at entry + {:?}..{:?} gt",
        sk.stamp_visit_max,
        sk.stamp_cleared.len(),
        sk.stamp_cleared.first(),
        sk.stamp_cleared.last()
    );
    println!("every row: drops {} (fall distance at most {:.3}; hurt {}; most in one visit {}: the bed's bounces count); the lie-down's first chance (its gt after the first tick in SleepInBed's reach at night, awake on foot): {:?}..{:?} over {} naps; seated in the zombie's boat {}; riderless carts out {}; the stamp's largest age at a calm REST tick {} gt (bar 1,000); cells waited on 100+ gt awake before the nap or the release (visits, longest gt): {:?}; walk targets taken waiting on foot in IDLE (visits): {:?}; visits whose stamp armed in that wait {}", sk.drops, sk.drop_max, sk.hurt, sk.most_drops, sk.gaps.first(), sk.gaps.last(), sk.gaps.len(), sk.boat_all, sk.riderless, sk.rest_stamp_max, sk.waits, sk.idle_targets, sk.idle_stamps);
    sk.exit_stamps.sort();
    sk.post_armed.sort();
    sk.post_lost_at.sort();
    sk.armed_after_wake.sort();
    println!("the waker's places (where the napper had the zombie in sight within 8: visits, least distance, all after F): {:?}", sk.waker_seen.iter().map(|(p, (n, d, f))| format!("{p}: {n}, {d:.2}, {f}")).collect::<Vec<_>>());
    println!("walk targets taken while panicking (where it was -> the target: visits): {:?}", sk.panic_targets.iter().map(|((p, t), n)| format!("{p} -> {t:?}: {n}")).collect::<Vec<_>>());
    println!("the stamp's first arming after the wake: {} visits, at wake + {:?}..{:?} gt, where (visits): {:?}", sk.armed_after_wake.len(), sk.armed_after_wake.first(), sk.armed_after_wake.last(), sk.armed_at);
    // (ironspec 14:23: the declared-unreachable cells never a path node)
    let unreachable_hit: Vec<[i32; 3]> = [[-4, 1, 5]].into_iter().filter(|c| sk.path_nodes.contains(c)).collect();
    sk.entry_stamps.sort();
    sk.entry_ats.sort();
    println!("at A6's entry (the cart's first tick in the entry cell, spawn + {:?}..{:?} gt): no stamp {}, stamped {} (ages {:?}..{:?} gt)", sk.entry_ats.first(), sk.entry_ats.last(), sk.entry_no_stamp, sk.entry_stamps.len(), sk.entry_stamps.first(), sk.entry_stamps.last());
    println!("escapes (ironspec 16:29: the napper on foot on the entry or the approach, out of the stop's box, or gone ([MIN..]): visits by first cell): {} {:?}", sk.escapes.values().sum::<usize>(), sk.escapes);
    println!("pickups before the wake or off the bed while asleep (ironspec 17:35 (a), bar 0): {}; the released cart in the pickup cell before the wake: {}", sk.pickup_unwoken, sk.cart_before_wake);
    for l in &sk.standin {
        println!("stand-in guard (ironspec 17:46/18:34; golem and path properties checked, late-path left to the statics on declared cells): {l}");
    }
    // (ironspec 20:31 (2): the release from every park position: the seating band, 0.02 inside
    // each edge (0.100 / 0.272 / 0.311): pickup speeds in [0.120, 0.252] or from 0.331)
    let mut park_failed = false;
    if sk.pickup_by_park.len() > 1 || !base.park_shifts.is_empty() {
        let inside = |v: f64| (0.120..=0.252).contains(&v) || v >= 0.331;
        let mut all_ok = true;
        let lines: Vec<String> = sk
            .pickup_by_park
            .iter()
            .map(|(k, (vs, seated, n))| {
                let lo = vs.iter().copied().fold(f64::MAX, f64::min);
                let hi = vs.iter().copied().fold(0.0, f64::max);
                let ok = *seated == *n && vs.iter().all(|&v| inside(v));
                all_ok &= ok;
                format!("({k}) seated {seated}/{n}{}{}", if vs.is_empty() { String::new() } else { format!(" at {lo:.3}..{hi:.3}") }, if ok { "" } else { " OUT" })
            })
            .collect();
        println!("the release by park position (ironspec 20:31: every position seats inside the band, 0.02 from each edge: {}): {}", if all_ok { "met" } else { "NOT met" }, lines.join("; "));
        park_failed = !all_ok;
    }
    if !sk.release_states.is_empty() {
        let stamped: Vec<i64> = sk.release_states.iter().filter_map(|s| s.0).collect();
        let runs = sk.release_states.iter().filter(|s| s.1).count();
        let mut re = sk.release_to_entry.clone();
        re.sort();
        println!(
            "at the queue's release ({} visits): stamped {} (ages {:?}..{:?} gt), holding a walk target or a path {}; release to A6's entry {:?}..{:?} gt",
            sk.release_states.len(),
            stamped.len(),
            stamped.iter().min(),
            stamped.iter().max(),
            runs,
            re.first(),
            re.last()
        );
    }
    println!("every node of every path the nappers held ({} distinct cells): the declared-unreachable cell (-4,1,5) among them: {}; the uncovered approach flats (-2,0,-3..-1) among them: {}; the cells: {:?}", sk.path_nodes.len(), if unreachable_hit.is_empty() { "no" } else { "YES" }, (-3..=-1).filter(|&z| sk.path_nodes.contains(&[-2, 0, z])).count(), sk.path_nodes);
    println!("at the exit: no stamp {}, stamped {} (ages {:?}..{:?} gt); then on the bench's run-out (1,500 gt seated in a stopped cart, no stamp clearer): the stamp armed at exit + {:?}..{:?} gt over {} visits, its largest age at a calm REST tick {} gt, HOME lost {} (at exit + {:?}..{:?} gt)", sk.exit_no_stamp, sk.exit_stamps.len(), sk.exit_stamps.first(), sk.exit_stamps.last(), sk.post_armed.first(), sk.post_armed.last(), sk.post_armed.len(), sk.post_stamp_max, sk.post_lost, sk.post_lost_at.first(), sk.post_lost_at.last());
    // (G1's CHANGES 4: the bars added since 17:35 fail the run, not only print: the park bar, the
    // early-pickup bar, a speed with a visit that never reached R, an out visit with no entry)
    let mut failed: Vec<String> = Vec::new();
    if park_failed {
        failed.push("the release by park position NOT met".into());
    }
    if sk.pickup_unwoken > 0 {
        failed.push("pickups before the wake or off the bed while asleep".into());
    }
    if speed_failed {
        failed.push("a speed with visits that never reached R".into());
    }
    if sk.entry_missing > 0 {
        failed.push("visits out with no recorded A6 entry".into());
    }
    if sk.errors > 0 {
        failed.push("visits refused or in error (the ERROR lines)".into());
    }
    // (ironspec 23:56, NO VACUOUS PASSES: every bar prints its sample count; 0 samples is NOT
    // JUDGED, a failure unless the run's declared scope (--scope-excludes) excludes the bar)
    let excluded = |bar: &str| base.scope_excludes.iter().any(|b| b == bar);
    let total_visits: usize = sk.visits_by_speed.values().sum();
    let samples = [("visits", total_visits, "every visit bar (stand-stills, HOME, escapes, contacts, lightning, the stamp)"), ("pickups", sk.pickups, "the early-pickup bar and the pickup band")];
    println!("samples per bar: {}", samples.iter().map(|(k, n, what)| format!("{k} {n} ({what}){}", if *n == 0 { if excluded(k) { " NOT JUDGED, excluded by the declared scope" } else { " NOT JUDGED" } } else { "" })).collect::<Vec<_>>().join("; "));
    for (k, n, what) in samples {
        if n == 0 && !excluded(k) {
            failed.push(format!("NOT JUDGED: 0 {k} ({what}); --scope-excludes {k} declares it out of this run's scope"));
        }
    }
    // (ironspec 22:56/22:58: benches print riderless carts out = 0; any other count fails, unless
    // the scope excludes it (the empty-cart control))
    if sk.riderless > 0 && !excluded("riderless") {
        failed.push(format!("{} riderless carts out", sk.riderless));
    }
    // (ironspec 00:03 / 00:07 (3): no stand-still after the eject over 20 gt)
    println!("the stand-still after the eject (gt awake on foot, unmoved, from the dismount to the first lie-down or the pickup; bar 20): the longest {} gt; visits over 20: {}", sk.stand_max, sk.stand_over_20);
    if sk.stand_over_20 > 0 && !excluded("stand") {
        failed.push(format!("{} visits stood still over 20 gt after the eject", sk.stand_over_20));
    }
    if !base.head_cells.is_empty() {
        println!("the head-space check (ironspec 03:30: every villager's box and eye box against the new solid cells {:?}, every tick): box overlaps {} tick-villagers, eye overlaps {} (bar 0)", base.head_cells, sk.head_box_hits, sk.head_eye_hits);
        if sk.head_eye_hits > 0 && !excluded("head") {
            failed.push(format!("{} eye overlaps with a new solid cell", sk.head_eye_hits));
        }
    }
    if !sk.overrides.is_empty() {
        println!("the bench's laid overrides (ironspec 04:07: each changes its cell, before -> after): {}", sk.overrides.join("; "));
    }
    // (ironspec 00:27: every fluid cell's tick scheduled at load, the count printed)
    println!("fluid cells whose tick the bench scheduled at load (ironspec 00:27; the most in one visit's world): {}", sk.fluid_scheduled);
    // ((A), ironspec 00:37 (1)/(2b): the landings (every one within SleepInBed's 2 of the bed
    // head's centre, never the park cell), the arming edge to the cart parked (the braking
    // interval: the gate's delay is its worst case + 3), the fastest cart on a release tick)
    if !sk.arm_to_park.is_empty() || !sk.landings.is_empty() {
        sk.arm_to_park.sort();
        println!(
            "the landings by cell (the dismount's position): {:?}; the farthest from the bed head's centre {:.3} (SleepInBed's 2); the arming edge to the cart parked: {:?}..{:?} gt over {} visits (the gate's delay {} gt); the fastest cart on a release tick {:.4} b/t (the at-rest bar: under 0.001); the pickup's least margin over the dismount + 60 (the boarding cooldown) {:?} gt",
            sk.landings,
            sk.landing_dist_max,
            sk.arm_to_park.first(),
            sk.arm_to_park.last(),
            sk.arm_to_park.len(),
            base.arm_delay,
            sk.release_speed_max,
            sk.cooldown_margin
        );
        if sk.landings.keys().any(|k| k == &format!("{},{},{}", PARK_A[0], PARK_A[1], PARK_A[2]) || k == &format!("{},{},{}", PARK_A2[0], PARK_A2[1], PARK_A2[2])) {
            failed.push("a rider landed in (A)'s park cell".into());
        }
    }
    // (ironspec 21:43 / 23:46: record-match against the design of record; the rows count only by
    // its verdicts)
    match (&sk.record_line, &base.record) {
        (Some((line, block_ok, mob_ok)), _) => {
            println!("{line}");
            if !block_ok {
                failed.push("record-match: the rows don't count (undeclared cells or unguarded stand-ins)".into());
            }
            if !mob_ok {
                failed.push("record-match: the mob bars don't count (undeclared entity differences)".into());
            }
        }
        (None, Some(_)) => failed.push("record-match not computed (the record's visit, --from at the first speed, is not among the rows)".into()),
        (None, None) => println!("record-match: no --record given (these rows count toward no module bar)"),
    }
    if !sk.path_after_load.is_empty() {
        println!("the PATH right after each load (ironspec 00:38; next index, nodes; the first visits'): {}", sk.path_after_load.join(" | "));
    }
    if !failed.is_empty() {
        bail!("bars failed: {}", failed.join("; "));
    }
    Ok(())
}

/// A skip sweep's totals (`bench_cmd`).
#[derive(Default)]
struct SkipTotals {
    idle_targets: std::collections::BTreeMap<[i32; 3], usize>,
    idle_stamps: usize,
    post_lost: usize,
    post_stamp_max: i64,
    post_lost_at: Vec<i64>,
    post_armed: Vec<i64>,
    exit_no_stamp: usize,
    exit_stamps: Vec<i64>,
    waker_seen: std::collections::BTreeMap<String, (usize, f64, bool)>,
    panic_targets: std::collections::BTreeMap<(String, [i32; 3]), usize>,
    armed_after_wake: Vec<i64>,
    armed_at: std::collections::BTreeMap<String, usize>,
    path_nodes: std::collections::BTreeSet<[i32; 3]>,
    entry_no_stamp: usize,
    entry_stamps: Vec<i64>,
    entry_ats: Vec<i64>,
    escapes: std::collections::BTreeMap<[i32; 3], usize>,
    pickup_unwoken: usize,
    cart_before_wake: usize,
    standin: Vec<String>,
    /// park x -> (pickup speeds of the seated, seated, visits)
    pickup_by_park: std::collections::BTreeMap<String, (Vec<f64>, usize, usize)>,
    release_states: Vec<(Option<i64>, bool)>,
    release_to_entry: Vec<i64>,
    most_drops: usize,
    classes: std::collections::BTreeMap<&'static str, usize>,
    lie_ticks: Vec<i64>,
    wake_ticks: Vec<i64>,
    skip_class: std::collections::BTreeMap<&'static str, (i64, i64)>,
    r_ats: Vec<i64>,
    r_by_speed: std::collections::BTreeMap<String, Vec<i64>>,
    visits_by_speed: std::collections::BTreeMap<String, usize>,
    entry_missing: usize,
    errors: usize,
    contact_visits: usize,
    contacts: std::collections::BTreeMap<String, usize>,
    stamp_visit_max: i64,
    stamp_cleared: Vec<i64>,
    r_real_by_speed: std::collections::BTreeMap<String, Vec<i64>>,
    r_real_entry_by_speed: std::collections::BTreeMap<String, Vec<i64>>,
    lightning_visits: usize,
    lightning: std::collections::BTreeMap<String, usize>,
    /// (ironspec 00:27) the most fluid cells a visit's world scheduled at load
    fluid_scheduled: usize,
    /// (record-match) the line and its block / mob verdicts, from the visit it was computed on
    record_line: Option<(String, bool, bool)>,
    /// (ironspec 00:38) the PATH right after each load, the first few visits' restarts
    path_after_load: Vec<String>,
    /// the visits whose napper was picked up (the early-pickup bar's samples)
    pickups: usize,
    /// ((A)) the landings by cell, the farthest from the bed head's centre, the arming edge to the
    /// cart parked (gt), the fastest cart on a release tick
    landings: std::collections::BTreeMap<String, usize>,
    landing_dist_max: f64,
    arm_to_park: Vec<i64>,
    release_speed_max: f64,
    /// ((A)) the least (pickup - dismount - 60) over the visits: the boarding cooldown's margin
    cooldown_margin: Option<i64>,
    /// the longest stand-still after the eject, and the visits over 20 gt
    stand_max: i64,
    stand_over_20: usize,
    head_box_hits: usize,
    head_eye_hits: usize,
    /// (ironspec 04:07) the laid overrides of the first visit that laid any, before -> after
    overrides: Vec<String>,
    r_real_missing: usize,
    drops: usize,
    drop_max: f64,
    hurt: usize,
    gaps: Vec<i64>,
    waits: std::collections::BTreeMap<[i32; 3], (usize, i64)>,
    rest_stamp_max: i64,
    riderless: usize,
    boat_all: usize,
    visits: usize,
    seated: usize,
    boat: usize,
    home_lost: usize,
    slept_before: usize,
    tail_naps: Vec<i64>,
    rest_naps: Vec<i64>,
    no_nap: usize,
    waited: std::collections::BTreeSet<[i32; 3]>,
    stamps: Vec<i64>,
    no_stamp: usize,
    /// (design A) The aborts by where the napper was, and each abort's delay (DAY high to the
    /// floor's opening and the park's release).
    aborts: std::collections::BTreeMap<&'static str, usize>,
    abort_delays: Vec<i64>,
    /// The aborted nappers' stamps at the release, and every stand-up cell.
    release_stamps: Vec<i64>,
    release_no_stamp: usize,
    stood_up: std::collections::BTreeSet<[i32; 3]>,
    standup_check: std::collections::BTreeSet<Option<[i32; 3]>>,
    cart_at_wake: std::collections::BTreeSet<[i32; 3]>,
}

/// `mctas mobfarm bedtier-two`: the two-visit rows (a row without a restart, one per restart phase
/// and order; or the daylight rows, one per held napper's queue arrival x DAY-off time x C's
/// queue arrival) with the totals.
#[allow(clippy::too_many_arguments)]
pub fn two_cmd(spec: &FarmSpec, from: u64, to: u64, speeds: &str, restart_phases: Option<&str>, orders: &[crate::liveness::Restart], transit: i64, lift_delay: i64, lift_speed: f64, day_hold: Option<&str>, day_offs: &str, gap_c: i64, c_at: Option<i64>, skip_phases: Option<&str>, day_on: i64, jobs: usize, day_gate: bool, a_hours: Option<&str>, after_sleep: i64, release_after: i64, declared_contacts: &'static [[i32; 3]]) -> Result<()> {
    let sp: Vec<f64> = list(Some(speeds))?;
    let base = TwoOpts { restart: None, transit, lift_delay, lift_speed, day_hold: None, c_at: None, skip: None, day_off: 12_230, day_on, day_gate, a_hour: None, after_sleep, release_after, declared_contacts };
    let mut rows: Vec<(String, TwoOpts)> = Vec::new();
    if let Some(hs) = a_hours {
        // (ironspec 09:31: A admitted on a stale DAY, at the entry at each day time; C at the queue
        // through A's ledge wait, released on A's R: never two nappers in the stop)
        for h in list::<i64>(Some(hs))? {
            rows.push((format!("A at the entry at day time {h}"), TwoOpts { a_hour: Some(h), ..base }));
        }
    } else if let Some(ks) = skip_phases {
        for k in list::<i64>(Some(ks))? {
            rows.push((format!("skip +{k} in A's visit"), TwoOpts { skip: Some(k), ..base }));
        }
    } else if let Some(dh) = day_hold {
        for off in list::<i64>(Some(day_offs))? {
            for h in list::<i64>(Some(dh))? {
                rows.push((format!("held from day time {h}, DAY off {off}, C {gap_c} gt after the drop"), TwoOpts { day_hold: Some((h, off, gap_c)), ..base }));
                if let Some(at) = c_at {
                    rows.push((format!("held from day time {h}, DAY off {off}, C at day time {at}"), TwoOpts { day_hold: Some((h, off, gap_c)), c_at: Some(at), ..base }));
                }
            }
        }
    } else {
        rows.push(("no restart".to_string(), base));
        for p in list::<i64>(restart_phases)? {
            for &how in orders {
                rows.push((format!("restart +{p} {}", how.label()), TwoOpts { restart: Some((p, how)), ..base }));
            }
        }
    }
    let (mut n, mut clean, mut lost, mut close) = (0, 0, 0, 0);
    let mut least: Option<f64> = None;
    let mut gap: Vec<i64> = Vec::new();
    let mut c_waits: Vec<i64> = Vec::new();
    let (mut lie_min, mut lie_max) = (f64::MAX, 0f64);
    let (mut skip_rows, mut a_aborts, mut c_entries) = (0usize, 0usize, Vec::<i64>::new());
    let (mut north, mut c_ledge): (usize, Option<f64>) = (0, None);
    let mut calm16 = [0usize; 2];
    let (mut lost_in, mut lost_out) = (0usize, 0usize);
    let mut dep_gaps: Vec<i64> = Vec::new();
    let mut ball_gaps: Vec<i64> = Vec::new();
    let mut ball_exits: Vec<i64> = Vec::new();
    let mut tasks: Vec<(usize, u64, f64)> = Vec::new();
    for ri in 0..rows.len() {
        for seed in from..=to {
            for &v in &sp {
                tasks.push((ri, seed, v));
            }
        }
    }
    let mut results = par_map(&tasks, jobs, |&(ri, seed, v)| two_visit_bench(spec, seed, v, &rows[ri].1)).into_iter();
    for (name, o) in &rows {
        let (mut ok, mut bad) = (0, Vec::new());
        // (the daylight rows: per row, A's lie-down and R as day times, C's wait at the queue, C's
        // arrival after A's drop (never two at the queue))
        let (mut a_sleeps, mut a_rs, mut waits, mut after_drop) = (Vec::<i64>::new(), Vec::<i64>::new(), Vec::<i64>::new(), Vec::<i64>::new());
        for seed in from..=to {
            for &v in &sp {
                n += 1;
                match results.next().unwrap_or_else(|| Err("no result".into())) {
                    Ok(r) => {
                        if let Some(d) = r.a_dist_at_c_sleep {
                            least = Some(least.map_or(d, |x: f64| x.min(d)));
                            close += (d <= 17.0) as usize;
                        }
                        if let (Some(ar), Some(cs)) = (r.a_r, r.c_slept) {
                            gap.push(cs - ar);
                        }
                        if let (Some(q), Some(e)) = (r.c_queued, r.c_entry) {
                            c_waits.push(e - q - o.transit);
                            waits.push(e - q - o.transit);
                        }
                        if let (Some(q), Some(d)) = (r.c_queued, r.a_drop) {
                            after_drop.push(q - d);
                        }
                        if let Some(s) = r.a_slept {
                            a_sleeps.push((r.start_day + s).rem_euclid(24000));
                        }
                        if let Some(x) = r.a_r {
                            a_rs.push((r.start_day + x).rem_euclid(24000));
                        }
                        lost += (!r.home_lost.is_empty() || !r.homes_end.0 || !r.homes_end.1) as usize;
                        lost_in += r.lost_in_stop as usize;
                        lost_out += (r.lost_outside && !r.lost_in_stop) as usize;
                        north += r.north_path as usize;
                        calm16[0] += r.calm16[0];
                        calm16[1] += r.calm16[1];
                        if let (Some(ar), Some(cr)) = (r.a_r, r.c_r) {
                            dep_gaps.push(cr - ar);
                        }
                        if let (Some(al), Some(cs)) = (r.a_last_in_ball, r.c_slept) {
                            ball_gaps.push(cs - al);
                        }
                        if let (Some(al), Some(ar)) = (r.a_last_in_ball, r.a_r) {
                            ball_exits.push(al + 1 - ar);
                        }
                        if let Some(d) = r.c_ledge_min {
                            c_ledge = Some(c_ledge.map_or(d, |m: f64| m.min(d)));
                        }
                        for &d in &r.lie_dists {
                            lie_min = lie_min.min(d);
                            lie_max = lie_max.max(d);
                        }
                        // (a skip row: A's abort, and C's entry by day time (held until DAY falls))
                        if r.skipped_at.is_some() {
                            skip_rows += 1;
                            a_aborts += r.a_aborted.is_some() as usize;
                            if let Some(e) = r.c_entry {
                                c_entries.push((r.start_day + e).rem_euclid(24000));
                            }
                        }
                        if r.problems.is_empty() {
                            ok += 1;
                        } else {
                            bad.push(format!("seed {seed} speed {v}: {:?} (A's drop {:?}, A at the entry {:?}, A's R {:?}, C queued {:?}, C at the entry {:?}, C lay down {:?}, A at {:?})", r.problems, r.a_drop, r.a_entry, r.a_r, r.c_queued, r.c_entry, r.c_slept, r.a_dist_at_c_sleep.map(|d| format!("{d:.2}"))));
                        }
                    }
                    Err(e) => bad.push(format!("seed {seed} speed {v}: ERROR {e}")),
                }
            }
        }
        clean += ok;
        if !bad.is_empty() {
            println!("{name}: {ok} of {} clean; {}", ok + bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; "));
        }
        if o.day_hold.is_some() {
            let span = |v: &mut Vec<i64>| {
                v.sort();
                format!("{:?}..{:?}", v.first(), v.last())
            };
            println!("{name}: {ok} clean; A lay down at day time {}, A's R at day time {}, C's wait at the queue {} gt, C queued {} gt after A's drop", span(&mut a_sleeps), span(&mut a_rs), span(&mut waits), span(&mut after_drop));
        }
    }
    gap.sort();
    c_waits.sort();
    println!("two visits (queue released on R, C at the entry {transit} gt later, A's lift from R + {lift_delay} at {lift_speed} b/t): {n} rows, {clean} clean; A within 17 at C's lie-down (an inherited margin over (a1)'s 16) {close}; HOME lost or missing at the end {lost}; A's least distance at C's lie-down {:?}; A's R to C's lie-down min {:?} median {:?} max {:?} gt; lie-downs' reach to the bed head's centre {:.4}..{:.4} (SleepInBed's bar 2); rows with a north path off the pickup cell {north}; C's nearest to a ledge cell while A was on foot in the stop {:?} (bar > 8)", least.map(|d| format!("{d:.2}")), gap.first(), gap.get(gap.len() / 2), gap.last(), if lie_min == f64::MAX { 0.0 } else { lie_min }, lie_max, c_ledge.map(|d| format!("{d:.2}")));
    if day_hold.is_some() {
        println!("daylight rows (the NOT DAY gate opens at day times {day_offs}): C's wait at the queue for A's R min {:?} median {:?} max {:?} gt", c_waits.first(), c_waits.get(c_waits.len() / 2), c_waits.last());
    }
    dep_gaps.sort();
    ball_gaps.sort();
    ball_exits.sort();
    println!("HOME losses by place (G1's note 3): rows with a loss inside the stop's box {lost_in}; rows with losses only outside it (the bench's queue cell, its lift's top: no stamp clearer there; the real build's are the queue's and the loop's) {lost_out}");
    println!("(a1) calm-REST ticks of any napper within 16 of the bed head while it was occupied (ValidateNearbyPoi; ironspec 19:07: measured directly, bar 0): A {} and C {}; its margin: A's leaving the ball to C's lie-down {:?}..{:?} gt over {} rows (an inherited margin, bar 20); A's R to its leaving the ball {:?}..{:?} gt over {} rows (the inherited 152, retired for (a1); this run's lift: --lift-delay/--lift-speed); consecutive departures (A's R to C's R) {:?}..{:?} gt over {} rows", calm16[0], calm16[1], ball_gaps.first(), ball_gaps.last(), ball_gaps.len(), ball_exits.first(), ball_exits.last(), ball_exits.len(), dep_gaps.first(), dep_gaps.last(), dep_gaps.len());
    if skip_rows > 0 {
        c_entries.sort();
        println!("skip rows in A's visit (design A, ironspec 07:43: no cascade): {skip_rows} rows, A aborted {a_aborts}; C's entry at day time {:?}..{:?} (DAY falls at 12,230: C held at the queue through the day)", c_entries.first(), c_entries.last());
    }
    Ok(())
}
