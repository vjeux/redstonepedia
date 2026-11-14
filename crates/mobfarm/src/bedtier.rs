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

/// (G1 01:14b, ironspec's override rule 04:07) A bench stand-in placed only on an air cell of the
/// design: one placed on a design block fails by name (the stand-in, the cell, the design's block),
/// never replaces it.
fn standin(b: &mut Builder, c: [i32; 3], s: &str) -> Result<()> {
    if let Some(old) = b.d.blocks.get(&c) {
        bail!("bed stop: the bench's stand-in {s} at {c:?} would replace the design's {old} (ironspec's override rule: stand-ins go on air cells only)");
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
/// rail with exactly 3 potential connections, from a signal-source block (ironspec 03:26)). Since
/// ironspec 21:15 (g): the stand-in sits in DAY-1's P_HOLD Q torch's cell (2,-1,4), a ceiling lever
/// under the slime: the torch strongly powers the slime from below, as the lever does.
pub const PARK_CTRL_A2: [i32; 3] = [2, -1, 4];
/// (the re-laid stop, ironspec 21:51 (5)) K's drive's bench stand-in: a ceiling lever under K's
/// conductor slime (-2,-1,3); on = K powered = north_east (the arrival from E)
pub const K_CTRL: [i32; 3] = [-2, -2, 3];
/// (the re-laid stop, ironspec 21:42) P_STRING's cell, P + 1
pub const P_STRING_CELL: [i32; 3] = [-2, 1, 4];
/// (day-1's form-(b) latch transplanted, `--latch-b`) the logic block's two inputs into the latch:
/// RLS's line and ARM's edge (the bench's stand-ins: floor levers the emulation writes)
pub const LATCH_RLS_IN: [i32; 3] = [-1, -2, -2];
pub const LATCH_ARM_IN: [i32; 3] = [6, -2, -5];
/// (A)'s wake sensor (G1 12:04 / day-1 12:03): an observer under the bed head, facing up at it;
/// its output face down into (-3,-2,4).
pub const WAKE_SENSOR: [i32; 3] = [-3, -1, 4];
/// (9fc3155f 23:21, the re-laid stop's candidate) the bed sensor west of the head, facing east: the
/// head and the foot then both stand on glass, which fails `LandRandomPos`'s `isNotStable` draw
pub const WAKE_SENSOR_WEST: [i32; 3] = [-4, 0, 4];
/// (ironspec 17:35, the sensed park; day-1 17:36) The candidate cell for the park-entry detector
/// rail: (0,0,3), the last straight cell the cart passes into the park that touches no powered rail
/// ((1,0,3), the climb, sits beside the park's powered rail (1,0,4), which a detector there would
/// power; (2,1,3) and (2,1,4) are curves); its read can leave north into the empty (0,0,2).
pub const PARK_SENSE: [i32; 3] = [0, 0, 3];
/// (G1's Q15) The wake sensor's window: a lie-down or wake edge with no rise within it is a missed
/// edge, a rise with no edge within it before is spurious: the observer's pulse rises 2 gt after the
/// bed head's change (tick-exact 26.3), is 2 wide, and the bench stamps edges at the tick boundary:
/// 10 leaves 6 gt over that.
pub const SENSOR_WINDOW: i64 = 10;
/// (ironspec 15:23 / day-1 15:29: the rider-keyed RS_ready) A tripwire string one cell over R's rail
/// (4,-1,5): a seated rider's box presses it, an empty cart's top (y -0.24) never reaches its band;
/// its hooks N-S off the rail line on glass backings; DAY-1 reads the north hook by a repeater at
/// (5,0,4) (its cell).
pub const RS_STRING: [i32; 3] = [4, 0, 5];
pub const RS_HOOK_N: [i32; 3] = [4, 0, 4];
pub const RS_HOOK_S: [i32; 3] = [4, 0, 6];
pub const RS_BACK_N: [i32; 3] = [4, 0, 3];
pub const RS_BACK_S: [i32; 3] = [4, 0, 7];
/// (G1's Q3 / Q15) The budget's term for the slope's departure -> R (ironspec's provisional
/// budget 73 + 170 + 15 + d_RS 10 + 7 + 10 = 285 <= 300; still PROVISIONAL: the 408-visit band on
/// bfa0c8098's fold measures 13..15 against it, on the bar; ironspec's budget item). The budget's
/// "d_RS 10" is that provisional sum's term, not day-1's controller value: the bench's RS_ready
/// figures use `--d-rs` (day-1's knob, 0 unless set; day-1 04:40).
pub const HOLD_TO_R_BUDGET: i64 = 15;
/// (G1's Q2 on 94e90d6b2) The slope hold's tap on its built path: DAY-1's repeater (module
/// (1,-29,3)) facing north, its output into the slope's slime support (1,-2,5); the bench file
/// lays it with its stand-in input lever at TAP_CTRL (a declared bench override).
pub const TAP_REP: [i32; 3] = [1, -2, 4];
pub const TAP_CTRL: [i32; 3] = [1, -2, 3];

/// The stop's variant cells in a built world: (A) when E holds an activator rail, T5b when the
/// entry does, else A6.
#[derive(Debug, Clone)]
pub struct Layout {
    pub name: &'static str,
    pub act: [i32; 3],
    pub act_ctrl: [i32; 3],
    pub park_ctrl: Vec<[i32; 3]>,
    pub arm: Option<[i32; 3]>,
    /// The waker window's controllers (F turns them off: the window opens): A6's two hatch levers,
    /// or (A)'s moved waker's shutter lever (`--waker-south`).
    pub hatch_ctrl: Vec<[i32; 3]>,
    /// (the re-laid stop) the park's braked rail; (1,0,4) on the fold of record
    pub park: [i32; 3],
    /// (the re-laid stop) K's drive's stand-in lever, when the bench file lays it
    pub k_ctrl: Option<[i32; 3]>,
    /// (the re-laid stop) the rest-keyed SENSED tripwire over the park, when laid
    pub park_string: Option<[i32; 3]>,
    /// (`--latch-b`) day-1's form-(b) latch laid (B2 stone and Q's wall torch at their cells)
    pub latch: bool,
}

/// (ironspec 07:12 / 07:39, the waker beyond the flee's 6: 58f770cc's Z1) The moved waker's post
/// (a zombie in a boat, its feet 6.73 from P's, 6.43 from the sleeper's), its shutter (an iron
/// trapdoor, open (powered) its plate cuts every line out of the post; shut (F) the sleeper's line
/// passes under its slab) and the shutter's lever (the bench's controller, day-1's F line end).
pub const WAKER_POST_S: [i32; 3] = [-5, 0, 10];
pub const WAKER_SHUT: [i32; 3] = [-5, 1, 9];
pub const WAKER_CTRL: [i32; 3] = [-5, 1, 8];

pub fn layout(w: &World) -> Layout {
    layout_at(w, [0, 0, 0])
}

/// [`layout`] on a world built at `off` (the bench's `--offset`): the cells read at design + off,
/// the layout returned in the design frame.
pub fn layout_at(w: &World, off: [i32; 3]) -> Layout {
    let at = |c: [i32; 3]| format!("{}", w.get(BlockPos::new(c[0] + off[0], c[1] + off[1], c[2] + off[2])));
    let hatch = vec![HATCH_CTRL, HATCH_CTRL_HI];
    if at(E_A2).contains("activator_rail") {
        let hatch_ctrl = if at(WAKER_SHUT).contains("iron_trapdoor") { vec![WAKER_CTRL] } else { hatch };
        // (the re-laid stop: P straight; the park the braked ascending rail on the siding z 3)
        if at(SWEPT[0]).contains("north_south")
            && let Some(px) = (1..=3).find(|&x| at([x, 0, 3]).contains("powered_rail") && at([x, 0, 3]).contains("ascending_east"))
        {
            let park = [px, 0, 3];
            let k_ctrl = at(K_CTRL).contains("lever").then_some(K_CTRL);
            let park_string = at([px, 1, 3]).contains("tripwire[").then_some([px, 1, 3]);
            let latch = at([2, -2, -2]).contains("stone") && at([6, -2, -2]).contains("redstone_wall_torch");
            let park_ctrl = if latch { Vec::new() } else { vec![[px + 1, 0, 1]] };
            return Layout { name: "(A) re-laid", act: E_A2, act_ctrl: ACT_CTRL_A2, park_ctrl, arm: Some(ARM_A2), hatch_ctrl, park, k_ctrl, park_string, latch };
        }
        Layout { name: "(A) run-up", act: E_A2, act_ctrl: ACT_CTRL_A2, park_ctrl: vec![PARK_CTRL_A2], arm: Some(ARM_A2), hatch_ctrl, park: PARK_A2, k_ctrl: None, park_string: None, latch: false }
    } else if at(E_A).contains("activator_rail") {
        Layout { name: "(A)", act: E_A, act_ctrl: ACT_CTRL_A, park_ctrl: vec![PARK_CTRL_A], arm: Some(ARM_A), hatch_ctrl: hatch, park: PARK_A2, k_ctrl: None, park_string: None, latch: false }
    } else if at(ENTRY).contains("activator_rail") {
        Layout { name: "T5b", act: ENTRY, act_ctrl: ACT_CTRL_T5B, park_ctrl: PARK_CTRL.to_vec(), arm: None, hatch_ctrl: hatch, park: PARK_A2, k_ctrl: None, park_string: None, latch: false }
    } else {
        Layout { name: "A6", act: ACTIVATOR, act_ctrl: ACT_CTRL, park_ctrl: PARK_CTRL.to_vec(), arm: None, hatch_ctrl: hatch, park: PARK_A2, k_ctrl: None, park_string: None, latch: false }
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
pub fn design_a2(bench: bool, loop_booster: bool, head_open: bool, exit_south: bool, waker_south: bool, exit_boost: bool, hold_slope: bool, wake_sensor: bool, rs_ready: bool, park_sense: bool, relaid: Option<i32>, sensor_west: bool, stamp_route: bool, latch_b: bool, seal_p: bool) -> Result<String> {
    anyhow::ensure!(!seal_p || relaid.is_some(), "--seal-p is the re-laid stop's: with --relaid");
    anyhow::ensure!(!latch_b || relaid.is_some(), "--latch-b is laid on the re-laid stop: with --relaid 1");
    anyhow::ensure!(!sensor_west || relaid.is_some(), "--bed-sensor-west is the re-laid stop's candidate: with --relaid");
    anyhow::ensure!(!stamp_route || relaid == Some(1), "--stamp-route is the re-laid stop's (park at x 1): with --relaid 1");
    anyhow::ensure!(relaid.is_none() || (park_sense && rs_ready && hold_slope), "--relaid re-lays the --park-sense / --rs-ready / --hold-slope fold");
    anyhow::ensure!(!park_sense || (exit_south && !loop_booster), "the sensed park's detector goes on the plain (0,0,3) of the --exit-south layout without the loop booster");
    anyhow::ensure!(!rs_ready || exit_south, "--rs-ready lays its string over R (4,-1,5), the --exit-south row");
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
    // (E's floor glass, not solid-render: E is no stable stroll destination from P (ironspec 07:16
    // (ii): the panic stroll drew it, unreachable); a top-half trapdoor over E (its collision at
    // 1.8125..2.0, under it 1.81 of clear height) keeps E out of findStandUpPosition's answers
    // (both passes: the 1.95-tall box won't fit) where the magma floor did it before; the seated
    // rider's eye (cart + 1.62) passes under it; a plain rail on its top (golem-proofing))
    put(&mut b, [E_A2[0], -1, E_A2[2]], "glass")?;
    put(&mut b, [E_A2[0], 1, E_A2[2]], cap)?;
    put(&mut b, [E_A2[0], 2, E_A2[2]], button)?;
    put(&mut b, ACT_CTRL_A2, &ctl_block(ACT_CTRL_A2, "redstone_block"))?;
    put(&mut b, [ACT_CTRL_A2[0], -1, ACT_CTRL_A2[2]], "glass")?;
    put(&mut b, [ACT_CTRL_A2[0], 1, ACT_CTRL_A2[2]], "glass")?;
    // (the dismount at E, a southbound cart: W the torch's cell under glass (1 high), E, NW, NE at
    // E's level blocked, then SW, the bed foot's top: the first valid place, 1.00 from the bed
    // head's centre; E's own blocker (-1,0,2) a top-half trapdoor (its collision 0.8125..1.0, over
    // the cart's box, 0.76: glass there met the box of a cart turning east at K, a contact that
    // stalled 2 of 72 band visits; a 1.95 box still won't fit under it))
    put(&mut b, [-1, 0, 2], "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")?;
    for c in [[-3, 0, 1], [-1, 0, 1]] {
        put(&mut b, c, "glass")?;
    }
    // K, the curve east after E (glass over it: P's way north is 1 high), a flat, a booster (always
    // on: the climb's speed), the climb, the U-turn at y 1 on glass
    put(&mut b, [-2, 0, 3], &rail("north_east"))?;
    put(&mut b, [-2, -1, 3], "glass")?;
    put(&mut b, [-2, 1, 3], "glass")?;
    // (with the sensed park, 9fc3155f 17:49's MUTUAL HOLD (-1,0,3) <-> (-1,0,4), each T held only
    // through the other, is broken by (-1,0,3) an activator rail: a fixed straight shape (no
    // plain-rail T); it carries only the empty cart (after E's eject), so the detector (0,0,3)
    // powering it once that cart is past ejects nobody (G1 19:19: not (-1,0,4), which the loaded
    // cart sits on at the pickup))
    put(&mut b, [-1, 0, 3], &if park_sense { "activator_rail[powered=false,shape=east_west,waterlogged=false]".to_string() } else { rail("east_west") })?;
    put(&mut b, [-1, -1, 3], "glass")?;
    if loop_booster {
        put(&mut b, [0, 0, 3], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
        put(&mut b, [0, -1, 3], "glass")?;
        put(&mut b, [0, 0, 2], "redstone_torch[lit=true]")?;
        put(&mut b, [0, -1, 2], "glass")?;
    } else if park_sense {
        // (ironspec 17:35 / 17:39: the sensed park: a detector rail on the last straight cell the
        // cart passes into the park that touches no powered rail; 9fc3155f 17:47: its two Ts re-settle
        // to their own shapes (update-only); DAY-1's PARKED latch reads it from the empty (0,0,2))
        put(&mut b, PARK_SENSE, "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
        put(&mut b, [0, -1, 3], "glass")?;
    } else {
        put(&mut b, [0, 0, 3], &rail("east_west"))?;
        put(&mut b, [0, -1, 3], "glass")?;
    }
    put(&mut b, [1, 0, 3], &rail("ascending_east"))?;
    put(&mut b, [1, -1, 3], "glass")?;
    put(&mut b, [2, 1, 3], &rail("south_west"))?;
    put(&mut b, [2, 0, 3], "glass")?;
    put(&mut b, [2, 1, 4], &rail("north_west"))?;
    // (the U-turn's south support a redstone conductor: the park's drive; SLIME, not smooth stone
    // (ironspec 07:16 (ii) / 07:39: a solid-render floor makes the rail cell over it a stable flee
    // and stroll destination, its 1-high headroom notwithstanding: the random-position filter reads
    // the floor and the cell's static path type, no headroom; the flee drew (2,1,4) on 2 of 20
    // wakes, a partial path: an arming); slime conducts, isn't solid-render and carries a rail;
    // glass over the curve (the cart passes under))
    put(&mut b, [2, 0, 4], "slime_block")?;
    put(&mut b, [2, 2, 4], "glass")?;
    // the park: one braked powered rail descending west, powered through the conductor east of it:
    // DAY-1's P_HOLD Q torch at (2,-1,4) on its base (2,-2,4) strongly powers the slime from below
    // (ironspec 21:15 (g); the bench's stand-in a ceiling lever in the torch's cell)
    put(&mut b, PARK_A2, "powered_rail[powered=false,shape=ascending_east,waterlogged=false]")?;
    put(&mut b, [PARK_A2[0], -1, PARK_A2[2]], "glass")?;
    // (G1's CHANGES C13 / ironspec 09:33: the design of record holds NO lever (the wind-burst
    // rule): the drive's stand-in lever is laid only in the bench's file; the record leaves
    // (2,-1,4), (2,-2,4), (3,0,4) and (3,-1,4) for DAY-1's latch cells)
    if bench {
        put(&mut b, PARK_CTRL_A2, &ctl_block(PARK_CTRL_A2, "stone"))?;
    }
    // the run-up: two flats along z 4 into P, WATERLOGGED (a minecart in water moves at most
    // getMaxSpeed() = 4 / 20 = 0.2 b per tick per axis: every step into the napper on P is within the
    // pickup's 0.2 inflation, so the cart can't clip into the napper's box before the pickup check
    // (vanilla's minecart collides with pushable entities: Boat.canVehicleCollide); the empty cart
    // leaves the park slope at ~0.35); glass over the one next to P only (a napper on P can't step
    // east; the flat at the park slope's foot stays open: a cart coming down the slope rides with its
    // box's top above y 1 until it's on the flat, and a block over that flat stopped it)
    for x in -1..=0 {
        put(&mut b, [x, 0, 4], "rail[shape=east_west,waterlogged=true]")?;
        put(&mut b, [x, -1, 4], "glass")?;
    }
    put(&mut b, [-1, 1, 4], "glass")?;
    // P, a curve turning south onto A6's exit curve, and A6's exit
    put(&mut b, SWEPT[0], &rail("south_east"))?;
    put(&mut b, [SWEPT[0][0], -1, SWEPT[0][2]], "glass")?;
    if exit_south {
        // (the exit one cell further south: P's curve onto a straight (-2,0,5), the curve east at
        // (-2,0,6), the booster (-1,0,6), back north to z 5 at x 0 (a curve each) onto A6's exit
        // slope: no curve-to-curve junction beside the bed head (there the cart overshot to x
        // -1.51, its box's west face against the head's base: a contact that stalled the slowest
        // carts); the cells the old exit left at (-1,0,5) stay AIR with no floor (a block there met
        // the carts' boxes on both curves; no floor: no new standable cell))
        if exit_boost {
            // (day-1's stop-occupancy budget: entry -> R's slow tail (up to ~203) came from carts
            // leaving the hand-off at ~0.07 and crawling the exit's curve; a BOOSTER on the straight
            // right after P, (-2,0,5), powered through its support, a SLIME block (a conductor, not
            // solid-render: no stable floor for the cell over it), by a redstone torch under it at
            // (-2,-2,5) on glass: the torch's updates reach no plain rail with 3 connections (P,
            // three away, isn't touched))
            put(&mut b, [-2, 0, 5], "powered_rail[powered=true,shape=north_south,waterlogged=false]")?;
            put(&mut b, [-2, -1, 5], "slime_block")?;
            put(&mut b, [-2, -2, 5], "redstone_torch[lit=true]")?;
            put(&mut b, [-2, -3, 5], "glass")?;
        } else {
            put(&mut b, [-2, 0, 5], &rail("north_south"))?;
            put(&mut b, [-2, -1, 5], "glass")?;
        }
        put(&mut b, [-2, 1, 5], cap)?;
        put(&mut b, [-2, 2, 5], "glass")?;
        put(&mut b, [-2, 0, 6], &rail("north_east"))?;
        put(&mut b, [-2, -1, 6], "glass")?;
        put(&mut b, [-2, 1, 6], cap)?;
        put(&mut b, [-2, 2, 6], "glass")?;
        put(&mut b, [-1, 0, 6], "powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
        put(&mut b, [-1, -1, 6], "glass")?;
        put(&mut b, [-1, 0, 7], "redstone_torch[lit=true]")?;
        put(&mut b, [-1, -1, 7], "glass")?;
        put(&mut b, [-1, 1, 6], cap)?;
        put(&mut b, [-1, 2, 6], button)?;
        put(&mut b, [0, 0, 6], &rail("north_west"))?;
        put(&mut b, [0, -1, 6], "glass")?;
        put(&mut b, [0, 1, 6], cap)?;
        put(&mut b, [0, 0, 5], &rail("south_east"))?;
        put(&mut b, [0, -1, 5], "glass")?;
        put(&mut b, [0, 1, 5], cap)?;
        put(&mut b, [0, 2, 5], button)?;
        if hold_slope {
            // (ironspec 09:36's hold with the napper aboard, at the first cell that can brake, hold
            // and launch a loaded cart: P is a curve and powered rails are straight, and a flat
            // braking rail can't restart a cart at rest without a conductor at an end; the exit
            // slope can: unpowered it brakes the cart to rest (x0.5 a tick, zeroed under 0.03, the
            // slope's 0.0078 zeroed with it), powered at the lane's tap the slope's gravity starts
            // it and the boost carries it east; 7.8 from Z1 (beyond the flee's 6); driven by DAY-1's
            // tap cells in the build (the bench: a redstone block written beside it at the tap,
            // `--pulse-cell 1,-1,6@T=redstone_block`))
            put(&mut b, [1, -1, 5], "powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
        } else {
            put(&mut b, [1, -1, 5], &rail("ascending_west"))?;
        }
        // (the slope's support: with the hold, DAY-1's built tap cell, a SLIME block fed by its
        // repeater at (1,-2,4) (module (1,-29,3), facing north; day-1 10:38): placed here as the
        // support, its drive day-1's; the bench file lays that repeater and its stand-in input
        // lever (G1's Q2: the tap on the built path, through the stand-in guard))
        put(&mut b, [1, -2, 5], if hold_slope { "slime_block" } else { "glass" })?;
        if hold_slope && bench {
            put(&mut b, TAP_REP, "repeater[delay=1,facing=north,locked=false,powered=false]")?;
            put(&mut b, [TAP_REP[0], TAP_REP[1] - 1, TAP_REP[2]], "glass")?;
            put(&mut b, TAP_CTRL, &ctl_block(TAP_CTRL, "stone"))?;
            put(&mut b, [TAP_CTRL[0], TAP_CTRL[1] - 1, TAP_CTRL[2]], "glass")?;
        }
        for x in 2..=3 {
            put(&mut b, [x, -1, 5], &rail("east_west"))?;
            put(&mut b, [x, -2, 5], "glass")?;
        }
        // (ironspec 15:23 / day-1 15:29: the rider-keyed RS_ready: a tripwire string one cell over
        // R's rail, pressed by a seated rider's box, never by an empty cart's (its top at y -0.24);
        // the hooks N-S off the rail line, each on a glass backing; DAY-1 reads the north hook by a
        // repeater at (5,0,4), its cell)
        if rs_ready {
            put(&mut b, RS_BACK_N, "glass")?;
            put(&mut b, RS_HOOK_N, "tripwire_hook[attached=true,facing=south,powered=false]")?;
            put(&mut b, RS_STRING, "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]")?;
            put(&mut b, RS_HOOK_S, "tripwire_hook[attached=true,facing=north,powered=false]")?;
            put(&mut b, RS_BACK_S, "glass")?;
        }
        // ((-1,0,5), the old exit's cell between P's straight and the new curve: a top-half
        // trapdoor: it keeps the run-up's water ((-1,0,4) is waterlogged) from flowing out through
        // it (with the cell empty and unfloored the water fell out of the build), its collision over
        // the carts' boxes on both curves, no stand-up or stable place)
        put(&mut b, [-1, 0, 5], "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")?;
    } else {
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
    }
    put(&mut b, EXIT_DET, "detector_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(&mut b, [EXIT_DET[0], -2, EXIT_DET[2]], "glass")?;
    // the bed, its foot's roof a top-half trapdoor at y 2 (the dismount's box clear under it), a
    // floor button over it; glass over the head (its top 1.44 high: not standable) and round it
    put(&mut b, BED_HEAD, "red_bed[facing=south,occupied=false,part=head]")?;
    put(&mut b, [-3, 0, 3], "red_bed[facing=south,occupied=false,part=foot]")?;
    put(&mut b, [-3, -1, 3], "glass")?;
    // (G1 12:04 / day-1 12:03, the wake sensor: an observer under the bed head facing up at it,
    // in the glass's place (the same full collision, no sight line through it): OCCUPIED's edges
    // (SleepInBed's start, the wake) each give a pulse out of its bottom face into (-3,-2,4),
    // where DAY-1's F line and the park's release start)
    put(&mut b, WAKE_SENSOR, if wake_sensor { "observer[facing=up,powered=false]" } else { "glass" })?;
    put(&mut b, [-3, 2, 3], cap)?;
    put(&mut b, [-3, 3, 3], button)?;
    for c in [[-3, 2, 4], [-3, 1, 5], [-4, 2, 4], [-2, 2, 4]] {
        if head_open && c == [-3, 2, 4] {
            // (ironspec 06:31 real, the re-claim path: no glass over the head, so the cell over it
            // is a path node within HOME's valid range 1 of the head for a napper whose path
            // starts over the foot (getStart's floor(y + 0.5)))
            continue;
        }
        put(&mut b, c, "glass")?;
    }
    // the hatch and the zombie's post (A6's), or (--waker-south) the moved waker
    if waker_south {
        // (58f770cc's Z1, ironspec 07:12 / 07:39: the waker beyond SetWalkTargetAwayFrom's 6 from
        // P, the stand-up cell and every exit position in its sight; within the sensor's 8 of the
        // sleeper: a zombie in a boat at WAKER_POST_S (feet ~(-4.5,-0.5,10.5): 6.73 from P's feet,
        // 6.43 from the sleeper's), walled in glass, glass and persistent leaves over it (no sky:
        // no sunburn), its one window the shutter WAKER_SHUT north of it (an iron trapdoor, top
        // half: powered (the night state) it stands open, its plate across the cell's face, every
        // line out cut; F unpowers it: shut, its slab at 1.8125 and the sleeper's line (eye
        // ~(-2.5,0.89,4.5) to ~(-4.5,1.24,10.5)) passes under it through the open channel
        // (-3,0,5), (-3..-4,0,6), (-4,1,6..9)); the screens (-3,1,6), (-3,1,7), (-2,1,7), (-1,1,7)
        // cut the lines to the exit's rider positions within 6 (on (-2,0,5), (-2,0,6), (-1,0,6));
        // the bed head's west side glass where A6's hatch was)
        let p = WAKER_POST_S;
        put(&mut b, [p[0], -1, p[2]], "glass")?;
        put(&mut b, [p[0], 2, p[2]], "glass")?;
        put(&mut b, [p[0], 3, p[2]], "oak_leaves[distance=7,persistent=true,waterlogged=false]")?;
        for y in 0..=1 {
            put(&mut b, [p[0] - 1, y, p[2]], "glass")?;
            put(&mut b, [p[0] + 1, y, p[2]], "glass")?;
            put(&mut b, [p[0], y, p[2] + 1], "glass")?;
        }
        put(&mut b, [p[0], 0, p[2] - 1], "glass")?;
        // (the shutter: in the bench's file powered by its stand-in lever's night state (the
        // window shut); in the record stored UNPOWERED, its one stable state with no drive (G1's
        // Q16: a stored powered state with no source would change at its first update), F's built
        // line (DAY-1's) holding it powered except during the open window)
        put(&mut b, WAKER_SHUT, if bench { "iron_trapdoor[facing=north,half=top,open=true,powered=true,waterlogged=false]" } else { "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]" })?;
        // (the shutter's drive (G1 11:52: a redstone block can't be switched off, so in the record it
        // would be a stand-in, which one-key-one-meaning forbids): the bench's file holds its
        // stand-in lever, a declared bench override; the record leaves (-5,1,8) EMPTY for F's built
        // line's output (DAY-1's cells) and stores the shutter in the night state that line holds)
        if bench {
            put(&mut b, WAKER_CTRL, &ctl_block(WAKER_CTRL, "redstone_block"))?;
        }
        put(&mut b, [WAKER_CTRL[0], 0, WAKER_CTRL[2]], "glass")?;
        b.entity("oak_boat", [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"));
        for c in [[-3, 1, 6], [-3, 1, 7], [-2, 1, 7], [-1, 1, 7]] {
            put(&mut b, c, "glass")?;
        }
        for c in [[-4, 0, 4], [-4, 1, 4], [-4, 0, 3], [-4, 1, 3]] {
            put(&mut b, c, "glass")?;
        }
        // ((-4,0,5) stays AIR with no floor: the sleeper's eye (~(-2.5,0.76,4.48)) to the waker's
        // eye (~(-4.5,1.23,10.5)) grazes that cell's corner (x -3.0 at z 6.0); glass there cut the
        // line: no wake on 6 of 6)
        // ((-3,0,5) stays air with no floor: the sleeper's line crosses it at y ~0.92; (-3,1,5)
        // glass over it (the bed's own wall list places it), so still no stand-up place, and
        // nothing beside the cart's box)
    } else {
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
    if exit_south {
        // (the bed head's south neighbour, beside the exit's straight: a top-half trapdoor, its
        // collision over the cart's box (0.8125 against the box's top 0.76: the P-to-exit junction's
        // overshoot met glass here); still no stand-up place (1 high under it, (-3,1,5) glass over
        // its top))
        put(&mut b, [-3, 0, 5], "iron_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")?;
    }
    for y in 0..=1 {
        wall(&mut b, [-3, y, 5])?;
    }
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
    if waker_south {
        b.d.meta.insert("keep-block".into(), "-5,3,10=oak_leaves[distance=7,persistent=true] (Z1's cover over its glass roof (-5,2,10): its eye cell (-5,1,10) at sky light 14, no burning by day)".into());
        // (the waker's in-sight places, declared: every place a napper has Z1 in sight within 8
        // once F opens the window: the sleeper only (58f770cc's ray at F on every visit; 9fc3155f's
        // x20 sight: P and the exit screened); none with the window shut)
        b.d.meta.insert("waker-places".into(), "-3,0,4=asleep in the bed, woken once DAY-1's OPEN opens the window (6.36 eye to eye; the window open only from OPEN's set to the woken napper's press on P)".into());
    } else {
        b.d.meta.insert("keep-block".into(), "-5,3,4=oak_leaves[distance=7,persistent=true] (the zombie's cover: its eye's sky light 14 or less, no burning by day)".into());
    }
    if head_open && exit_south {
        // (the bed's column kept clear: (-3,1,4) over the head is the HOME path's target (the
        // column's first non-solid cell); the head open (ironspec 06:24))
        b.d.meta.insert("keep-clear".into(), "-3,1,4 -3,2,4 -3,3,4".into());
        // (the cells the stop's empty cart rides riderless: from E, where the napper is set down on
        // the bed foot's top, round the loop and the U-turn to its park, down the run-up to P, where
        // the napper boards again)
        b.d.meta.insert("riderless".into(), "-2,0,2+-2,0,3+-1,0,3+0,0,3+1,0,3+2,1,3+2,1,4+1,0,4+0,0,4+-1,0,4+-2,0,4=the stop's empty cart from E (the napper set down on the bed foot's top) round the loop and the U-turn to its park, then down the run-up to P (the napper seated again at P)".into());
    }
    // (9fc3155f 17:21 / ironspec 17:26 / 18:30: the stop-level golem floors guarded: a lone string
    // (tripwire, no hooks: no collision, no signal) in the spawn cell over E's east trapdoor
    // (-1,0,2) and over the hand-off's east trapdoor (-1,0,5): the golem's feet cell must be air
    // (tick-exact 18:13, SpawnUtil 77-95); glass over the exit curve's cap (0,1,6) (its neighbours
    // carry lone guard rails) and over Z1's shutter. Glass (or a slab) in both (-1,1,2) and (-1,1,5)
    // turned the woken napper's PANIC flee onto the bed top (-3,1,4), a partial path that armed
    // CANT_REACH after the wake on 363 of 408 visits; a glass floor at (-1,0,2) met the cart turning
    // at (-2,0,3) at 0.22 (8 visits stranded); the strings: 0 and 0 on the band)
    if exit_south {
        for c in [[-1, 1, 2], [-1, 1, 5]] {
            put(&mut b, c, "tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]")?;
        }
        put(&mut b, [0, 2, 6], "glass")?;
        // (G1 19:10: the two string cells, standable tops no napper reaches, declared for
        // flee-check's on-foot set; the bench prints any napper's feet in them, bar 0)
        b.d.meta.insert("unreachable".into(), "-1,1,2=E's east trapdoor top under its guard string: E's cell (-2,0,2) under the cap (-2,1,2) leaves 1.75 of headroom, (-1,0,1) is ARM's glass and (-1,0,3) a rail cell with no standable approach (the stop's only stand-up cell is P); -1,1,5=the hand-off's east trapdoor top under its guard string: diagonal to P, (-1,0,4) unstandable under the glass (-1,1,4), (-2,0,5) under the cap (-2,1,5) leaves 1.75, (0,1,5) is the curve's cap".into());
    }
    if waker_south {
        put(&mut b, [-5, 2, 9], "glass")?;
    }
    b.d.ports.insert("output".into(), [12, 0, 6]);
    b.d.ports.insert("home".into(), BED_HEAD);
    #[allow(unused_mut)]
    let mut lines: Vec<String> = vec![
        "The bed stop (A) with the run-up (B3, ironspec 02:27; local frame; loop = local + (0,-27,-1); the bed's head at (-3,0,4)): A6's arrival (the entry booster, plain rails down the old park's slope, the capped flat, the curve south, the flats), the arming detector (-2,0,1), the activator E (-2,0,2) over the bed foot's north-east corner (always on: a redstone torch on glass west of it, glass over the torch), the curve east (-2,0,3), a flat, a booster, the climb, the U-turn at y 1, the park (1,0,4) one braked powered rail descending west, two run-up flats along z 4, the pickup cell P (-2,0,4) a curve turning south, A6's exit (the curve east, the capped booster, the capped flat, the slope down, two flats, R). One solid-render block in the stop: the park's conductor (2,0,4) under the U-turn's curve, glass over the curve (the cell over the conductor 1 high: no stroll target).".to_string(),
        if waker_south {
            "E sets the rider down on the bed foot's top (a southbound cart's dismount: W, E, NW and NE full blocks, SW the first valid place), 1.00 from the bed head's centre: SleepInBed fires at once whatever path it carries. The empty cart runs the loop and brakes on the park. Z1's shutter (-5,1,9) keeps the window shut at night; DAY-1's controller (its gate list rev 20:00, artifact 1608254923947368) opens it once the napper sleeps (OPEN on PARKED and SO and not P_STRING, PARKED the sensed park AND COOL), the zombie wakes it, it stands up on P (its only stand-up cell) and presses P_STRING (the window shuts again); RLS (P_STRING and SINCE and WIN and PARKED) releases the park (RLS + p, p = 2, ironspec 17:53) and the cart runs the run-up into P under 0.2 b/t, seats it and runs on to R (the rider-keyed RS_ready string over it). One cart per visit, the park released once per visit.".to_string()
        } else {
            "E sets the rider down on the bed foot's top (a southbound cart's dismount: W, E, NW and NE full blocks, SW the first valid place), 1.00 from the bed head's centre: SleepInBed fires at once whatever path it carries. The empty cart runs the loop and brakes on the park. The hatch's two iron trapdoors stand open and hide the zombie; F (the lie-down + 2, and not before the arming detector's edge + 70: the napper's 60-gt boarding cooldown less the park -> P travel, plus the detector -> E time and a margin) SHUTS them: the zombie wakes it, it stands up on P (its only stand-up cell), the park releases at the wake edge, and the cart runs the run-up into P under 0.2 b/t, seats it and runs on to R, whose rise opens the hatch again and resets the park's release and the arming latch. Precondition and single release as A6's: one cart per visit, the park released only at the wake edge.".to_string()
        },
        if waker_south && bench { format!("The bench file lays the stand-in levers (bench overrides, never in the record): (-5,1,8) the shutter's drive (the controller unpowers it to open the window and powers it to shut it) and (2,-1,4) the park's drive (a ceiling lever under the slime (2,0,4), in the cell of DAY-1's P_HOLD Q torch, which strongly powers the slime from below as the lever does; ironspec 21:15 (g)){}; the record leaves (-5,1,8), (2,-1,4) and (2,-2,4) empty for DAY-1's cells ((3,0,4) and (3,-1,4) empty too).", if hold_slope { ", and the slope tap's input lever (1,-2,3) into DAY-1's repeater (1,-2,4) (facing north, into the slime support (1,-2,5); the record leaves the repeater's cell to DAY-1)" } else { "" }) } else if waker_south { "The drives, no lever and no stand-in in this file (G1 C13 / 11:52 / Q16; ironspec 09:57) -- (-5,1,8) is the cell DAY-1's SHUT output (OPEN's torch, rev 20:00 G20) drives, EMPTY in this file (DAY-1's cells; a bench lays its stand-in lever there as a declared override); the shutter (-5,1,9) is stored UNPOWERED (its one stable state with no drive: the plate in the cell's top half, the window OPEN); DAY-1's SHUT (NOT OPEN) holds it POWERED at night (the plate standing open across the window's face: the window SHUT, no line from the zombie to anyone) except during the open window: OPEN unpowers it (the sleeper's line passes under the plate), the woken napper's P_STRING press resets OPEN and powers it again (the window SHUTS: the 'close' of the clear - close figures); (2,-1,4) DAY-1's P_HOLD Q torch on its base (2,-2,4) (ironspec 21:15 (g)), both empty here: the torch strongly powers the slime (2,0,4) from below, and so the park rail (1,0,4); (3,0,4) and (3,-1,4) stay empty; until those lines are built, rows on this file name the bench's stand-ins.".to_string() } else { "switch: -4,0,3=the hatch's lower lever (day-1's line end stand-in: F shuts, R opens; no lever in the build); -4,1,5=the hatch's upper lever (the same line); 3,0,4=the park's drive (day-1's repeater output stand-in: a wall lever on the conductor (2,0,4); the release; no lever in the build)".to_string() },
        format!("always-on: 3,2,-5=the entry booster's redstone torch, on glass (3,1,-5): it never toggles; -3,0,2=the activator's redstone torch, on glass (-3,-1,2): it never toggles (ironspec 03:22: no lever a wind burst can flip); {}{}", if loop_booster { "0,0,2=the loop booster's redstone torch, on glass (0,-1,2): it never toggles; " } else { "" }, if exit_south { "-1,0,7=the exit booster's redstone torch, on glass (-1,-1,7): it never toggles" } else { "-1,0,6=the exit booster's redstone torch, on glass (-1,-1,6): it never toggles" }) + if exit_boost { "; -2,-2,5=the hand-off booster's redstone torch, on glass (-2,-3,5), under its slime support (-2,-1,5): it never toggles" } else { "" },
        "a6-pickup: -2,0,4=the pickup cell (the napper's only stand-up cell); stranded bound: no cart at it by the wake edge + 60 gt fails (the physical event it stands for: the woken napper waiting on P re-sleeps at the wake + ~100, LAST_WOKEN's cooldown, and once re-asleep the visit is stranded; 60 keeps 40 gt before that; the longest wake -> boarding on DAY-1's controller is 42 gt over 408 visits; the merged lie-down + wake path is a fault path judged by boarding before the re-sleep, ironspec 19:09)".to_string(),
    ];
    if wake_sensor {
        lines.push("The wake sensor (G1 12:04 / day-1 12:03): (-3,-1,4) an observer facing up at the bed head (-3,0,4), in the glass's place under it: the head's OCCUPIED set (SleepInBed's start) and cleared (the wake) each give a pulse out of its bottom face into (-3,-2,4), where DAY-1's controller (SO and SINCE set by its pulses) and its release (RLS = P_STRING and SINCE and WIN and PARKED; the park at RLS + p: the first (-2,0,5) entry clears C + 19 (NEAREST_HOSTILE's worst clear after the shutter's flip C, tick-exact 17:52) by >= 1 after k + j, ironspec 17:53) start; the bench prints its pulses against the napper's own edges.".to_string());
    }
    if rs_ready {
        lines.push("The rider-keyed RS_ready (ironspec 15:23, day-1 15:29): a tripwire string at (4,0,5), one cell over R's detector (4,-1,5), its hooks at (4,0,4) facing south and (4,0,6) facing north on glass (4,0,3) / (4,0,7), off the rail line: a seated rider's box presses it, an empty cart's top (y -0.24) never reaches its band (y 0.0625..0.156); hooks and string have no collision and block no sight line; DAY-1 reads the north hook by a repeater at (5,0,4) on (5,-1,4) (its cells, not laid here)".to_string());
    }
    if exit_south {
        lines.push("contacts: -3,0,4=the bed head's east face (its 0.5625-high box), met flush by the cart's west face at the curve-to-straight hand-off from P (-2,0,4) into (-2,0,5), the bed unchanged across it | class=design | face=east | ticks<=1 why=a second tick is the cart stopped at P, a stall | events<=1 why=one hand-off per visit, a second is the cart back at P | depth<=0.0440+eps=0.0001 why=main's depth is the clipped amount, the part of the cart's westward request at P's turn that the face stops (the boxes flush, overlap 0.0000), the deepest 0.0440 at every one of 21 build positions and over the 2040 visits of seeds 1-40 (a spread of 0 at four decimals), 0.0001 one unit of the stated fourth decimal (main's judge compares to contacts::EPS 1e-9, its reference check to half a unit), so a deeper clip (the turn's request changed) fails | speed_out>=0.0336+eps=0.0001 why=the least 0.0336 at every one of 21 build positions and over the 2040 visits of seeds 1-40 (a spread of 0 at four decimals), 0.0001 one unit of the stated fourth decimal, main's speed_out being the contact tick's returned move, so a slower hand-off fails, and the exit boost's carry threshold 0.0101 (--boost-probe on 2af606b9d sets the cart's velocity on (-2,0,5), another measure: 0.0101 carried and 0.0100 stalled) is a floor this bound doesn't lean on (the carry itself is judged directly: NO STALL by IRONSPEC_1042 after every event and every visit's cart at R), the game's booster start pending tick-exact | ref=e25e3b01c `mctas mobfarm bedtier-bench designs/farms/iron/parts/bed-tier-min/bed-tier-fold-bench.txt --speeds 0.05..0.55/0.01 --from 1 --to 8 --tap 265,10 --close-after 4 --release-after 8 --offset=X,Y,Z` (main's one judge, 774111605's code, 21 build positions (the design frame, ironspec's P1-P4 as written and as restated on the column convention with P2 at loop + (6,0,6) and + (6,24,6), every module copy A-D at loop + (0,0,0) and + (0,80,0) among them), identical extremes and exit 0 at each: 296 events over 296 of 408 visits (the 37 of 51 entry speeds that meet the face, every seed of each), 1 tick each, at most 1 per cart, the deepest 0.0440, speed_in 0.0450..0.0839, speed_out 0.0336..0.0808, the contact decided by the entry speed and judged per speed by the `contact-speeds:` line below (G1's B3: each listed speed meets the face on every seed, each other speed of the band never, a geometry change failing either way))".to_string());
        lines.push("contact-speeds: -3,0,4=meets 0.05,0.06,0.07,0.09,0.10,0.11,0.12,0.13,0.15,0.16,0.17,0.18,0.19,0.20,0.21,0.22,0.24,0.27,0.28,0.29,0.30,0.31,0.32,0.34,0.35,0.37,0.39,0.41,0.42,0.43,0.44,0.46,0.47,0.48,0.51,0.54,0.55; of 0.05..0.55/0.01".to_string());
        // (P's two connections are DESIGN-CRITICAL: K (-2,0,3) is a T by rail presence (N, E, S) and
        // keeps north_east under any source change only while P is full (ironspec 10:47 / 10:58's
        // `# integrity:` key, checked by the static))
        lines.push(if park_sense {
            // (9fc3155f 17:50 / PlaceClasses, ironspec 10:54: one entry per ARM rail, its `holds`
            // clauses belonging to it: P holds K; the fixed activator (-1,0,3) holds the run-up's
            // (-1,0,4); the park's rail (1,0,4) holds the climb (1,0,3))
            "integrity: -2,0,4=connections -1,0,4+-2,0,5; holds -2,0,3=north_east; -1,0,3=connections -2,0,3+0,0,3; holds -1,0,4=east_west; 1,0,4=connections 0,0,4+2,1,4; holds 1,0,3=ascending_east".to_string()
        } else {
            "integrity: -2,0,4=connections -1,0,4+-2,0,5; holds -2,0,3=north_east; holds -1,0,3=east_west; holds -1,0,4=east_west".to_string()
        });
        if park_sense {
            // (ironspec 17:35 / 17:39: PARKED sensed; 9fc3155f 17:47 / 17:49: its t-exposure
            // update-only, the arms keyed, the mutual hold broken by the fixed activator (-1,0,3))
            lines.push("update-only: 0,0,3=the sensed park's detector rail (ironspec 17:39): its updates reach the T (1,0,3), which re-settles to its held ascending_east (the full arm (1,0,4), the park's powered rail; 9fc3155f 17:47); its other neighbour (-1,0,3) is an activator rail (no T)".to_string());
            // (ironspec 20:33: the T beside the detector declared, both states one shape)
            lines.push("switches: 1,0,3=both states ascending_east: the park detector (0,0,3), face-adjacent, powers it on each arriving cart's pass; a plain rail ignores power, and the update re-settles it to ascending_east, held by the full arm (1,0,4), the park rail (9fc3155f 17:47, tick-exact 19:29); the bench's integrity print holds it every tick".to_string());
            lines.push("The sensed park (ironspec 17:35 / 17:39): a detector rail at (0,0,3), the last straight cell the cart passes into the park that touches no powered rail ((1,0,3), the climb, sits beside the park's powered rail (1,0,4)); DAY-1's PARKED latch (set by it after ARM, reset at the release, ANDed with COOL) reads it from the empty (0,0,2) (DAY-1's cells). Its power reach: it strongly powers the glass (0,-1,3) under it (inert) and powers its neighbours (-1,0,3) (an activator rail that only the empty cart rides, after E's eject: the detector, pressed by the cart's box, powers it while that cart may still be on it (G1's C1: the planted E-off row shows it), and an empty cart ejects nobody; a ridden cart there would lose its rider, the E-off fault), (1,0,3) and (0,0,4) (plain rails), (0,1,3) (air) and (0,0,2) (DAY-1's read): no powered rail or other component; the park's rail (1,0,4) sits diagonal to it. Only the arriving empty cart crosses it: the released cart leaves the park west along z 4 and the loaded cart leaves P south. Its block updates reach the T (1,0,3), which re-settles to its own shape, held by the full arm (1,0,4); the mutual hold (-1,0,3) <-> (-1,0,4) (9fc3155f 17:49) is broken by the activator (-1,0,3)'s fixed shape, which holds (-1,0,4) (G1 19:19: not an activator at (-1,0,4), where the loaded cart sits at the pickup).".to_string());
        }
    }
    // (ironspec 21:42 / 21:51 / 22:15 / 22:24 on 58f770cc's 21:44 sketch, with the rest-keyed SENSED
    // of 22:32: THE RE-LAID STOP. P straight north_south, approached from the north through K, a
    // driven T; the empty cart's out-and-back siding along z 3 to a braked park at (park_x,0,3)
    // under a rest-keyed tripwire; P_STRING east-west at P + 1; (-1,0,4) and (-1,2,4) solid; the
    // head-top guard (-3,2,4). Only what must change, changes: E, the bed, the exit from (-2,0,5)
    // on and the waker stay)
    if let Some(px) = relaid {
        anyhow::ensure!((1..=3).contains(&px), "--relaid takes the park's x, 1..3");
        let over = |b: &mut Builder, c: [i32; 3], s: &str| {
            b.d.blocks.remove(&c);
            b.set(c[0], c[1], c[2], s);
        };
        // (the U-turn, the old park, its conductor and the run-up go; the climb's cells are re-laid)
        for c in [[2, 1, 3], [2, 1, 4], [2, 0, 4], [2, 2, 4], [1, 0, 4], [1, -1, 4], [0, 0, 4], [0, -1, 4], [-1, 1, 4], [2, 0, 3], [1, 0, 3], [1, -1, 3]] {
            b.d.blocks.remove(&c);
        }
        if bench {
            b.d.blocks.remove(&PARK_CTRL_A2);
        }
        // P straight and DRY (ironspec 22:58 (2) / 23:40 (1): no water at the stop)
        over(&mut b, [-2, 0, 4], "rail[shape=north_south,waterlogged=false]");
        // ((-1,0,4) sealed and its cap (-1,2,4): a napper on P can't step or jump east)
        over(&mut b, [-1, 0, 4], "glass");
        over(&mut b, [-1, 2, 4], "glass");
        // (P_STRING east-west at P + 1, ironspec 21:42: the west hook over the bed head on (-4,1,4),
        // the east hook on new glass (0,1,4))
        over(&mut b, [-3, 1, 4], "tripwire_hook[attached=true,facing=east,powered=false]");
        // (vcompose 03:28: the hook over the bed head is a designed block, so the bed column's
        // keep-clear starts above it; the hook has no collision box, so (-3,1,4) stays the column's
        // first non-solid cell, the HOME path's target, and 9fc3155f's reach probe completes onto it)
        b.d.meta.insert("keep-clear".into(), "-3,2,4 -3,3,4".into());
        over(&mut b, P_STRING_CELL, "tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
        over(&mut b, [-1, 1, 4], "tripwire_hook[attached=true,facing=west,powered=false]");
        over(&mut b, [0, 1, 4], "glass");
        // (the head-top guard: OUT FOR GOOD, ironspec 23:40 (3)(a): it takes the bed head node's
        // headroom, so no HOME path could ever complete; the walk-off closes another way)
        // (9fc3155f 23:21's candidate, `--bed-sensor-west`: the bed sensor moved west of the head so
        // both bed halves stand on glass (no `isNotStable` pass, no bed-top stroll target))
        if sensor_west {
            over(&mut b, WAKE_SENSOR_WEST, "observer[facing=east,powered=false]");
            over(&mut b, WAKE_SENSOR, "glass");
            lines.push("The bed sensor (9fc3155f 23:21's candidate): the observer at (-4,0,4) facing east watches the bed head (-3,0,4) from the west, its output out of its back at (-5,0,4) (DAY-1's wake line re-routes there); (-3,-1,4) under the head is glass, so the head and the foot both stand on glass and neither passes LandRandomPos's isNotStable draw.".to_string());
        }
        // (ironspec 23:40 (3)(c), candidate R1: the stamp-free HOME route from K's rider-only siding,
        // over the roof, ending in a one-way drop onto the bed head's node)
        if seal_p {
            // (58f770cc 00:02 / ironspec 00:05 / 00:08: SEAL P: the exit's first cap, the iron
            // trapdoor over (-2,0,5), becomes glass: (-2,0,5) keeps no standing headroom, so the
            // reach-sweep's closure from P is {P} (K's glass head, (-1,0,4) sealed, the bed top's
            // jump under the (-2,2,4) cap))
            over(&mut b, [-2, 1, 5], "glass");
            lines.push("Seal-P (ironspec 00:05 / 00:08): the exit's first cap (-2,1,5) is glass, not an iron trapdoor: (-2,0,5) keeps no standing headroom, so no walker leaves P by the exit channel (K's head (-2,1,3) glass, (-1,0,4) sealed, the bed top's jump blocked by P's cap (-2,2,4)): the reach-sweep's closure from P is {P}; riders pass it seated (no suffocation: glass).".to_string());
        }
        if stamp_route {
            for c in [[2, 1, 4], [1, 2, 4], [0, 2, 4]] {
                over(&mut b, c, "glass");
            }
            lines.push("The stamp-free HOME route R1 (ironspec 23:40 (3)(c), 58f770cc 23:51): from K's siding (rider-only: K's head (-2,1,3) is glass and (-1,0,4) sealed, so no walker reaches it from P) the pathfinder walks (-1,0,3), (0,0,3), the park (1,0,3), the slime's top (2,1,3), up the glass steps (2,1,4) / (1,2,4) / (0,2,4) to the roof (feet (2,2,4), (1,3,4), (0,3,4), (-1,3,4), (-2,3,4)), then west into (-3,3,4) and DROPS 2.4375 onto the bed head's node (1.125 < h <= 3: no walker climbs back).".to_string());
        }
        // (day-1's form-(b) P_HOLD latch, a/1902620467569082, latching in 26.3 (tick-exact 23:47,
        // a/1841767613704681), transplanted one level down (58f770cc 23:39, ironspec 23:45): rig
        // (x, 0, z) -> stop (x + 2, -2, z - 2), every wire and repeater on glass at y -3; the rig's
        // DRIVE and PARK lamp replaced by the OR line's climb into the drive repeater (2,0,2) (two
        // up-steps on glass, nothing conducting over a lower wire); the rig's TAP replaced by Q's
        // own tap line into the slope's tap repeater (1,-2,4); RLS_IN (-1,-2,-2) and ARM_IN
        // (6,-2,-5) are the logic block's inputs (the bench's stand-ins: floor levers)
        if latch_b {
            anyhow::ensure!(px == 1, "--latch-b is laid for the park at x 1 (the drive (2,0,2))");
            let dusts: Vec<[i32; 3]> = vec![
                // RLS -> B2's west face
                [0, -2, -2], [1, -2, -2],
                // RLS -> the branch and the reset chain's feed
                [-1, -2, -3], [-1, -2, -4], [0, -2, -4], [1, -2, -4], [2, -2, -4], [-1, -2, -5], [-1, -2, -6],
                // the reset chain's tail into the RESET repeater (ARM_IN joins at (5,-2,-5))
                [5, -2, -6], [5, -2, -5], [5, -2, -4],
                // QBAR -> BASE's west face
                [4, -2, -2],
                // Q's feedback into B2's south face
                [6, -2, -1], [6, -2, 0], [5, -2, 0], [4, -2, 0], [3, -2, 0], [2, -2, 0], [2, -2, -1],
                // the OR line from RLS, joining Q's line at (2,-2,0)
                [-1, -2, -1], [-1, -2, 0], [0, -2, 0], [1, -2, 0],
                // the climb into the drive repeater (2,0,2): two up-steps on glass
                [0, -2, 1], [1, -1, 1], [2, 0, 1],
                // Q's tap line into the slope's tap repeater (1,-2,4)
                [7, -2, -2], [8, -2, -2], [8, -2, -1], [8, -2, 0], [8, -2, 1], [8, -2, 2], [7, -2, 2], [6, -2, 2], [5, -2, 2], [4, -2, 2], [3, -2, 2], [2, -2, 2], [2, -2, 3], [1, -2, 3],
            ];
            // (the components the wires meet: a repeater connects along its axis, a torch / lever
            // on any side; the stones take no connection)
            let mut comps: std::collections::BTreeMap<[i32; 3], (&str, &str)> = std::collections::BTreeMap::new();
            comps.insert([2, -2, -3], ("repeater", "north"));
            comps.insert([5, -2, -3], ("repeater", "north"));
            for x in 0..=4 {
                comps.insert([x, -2, -6], ("repeater", "west"));
            }
            comps.insert(TAP_REP, ("repeater", "north"));
            comps.insert([2, 0, 2], ("repeater", "north"));
            comps.insert([3, -2, -2], ("torch", ""));
            comps.insert([6, -2, -2], ("torch", ""));
            comps.insert(LATCH_RLS_IN, ("lever", ""));
            comps.insert(LATCH_ARM_IN, ("lever", ""));
            let solid: std::collections::BTreeSet<[i32; 3]> = [[2, -2, -2], [5, -2, -2]].into_iter().collect();
            let dset: std::collections::BTreeSet<[i32; 3]> = dusts.iter().copied().collect();
            for &c in &dusts {
                let mut sides: Vec<(&str, &str)> = Vec::new();
                for (name, dx, dz, axis) in [("north", 0, -1, "ns"), ("south", 0, 1, "ns"), ("west", -1, 0, "ew"), ("east", 1, 0, "ew")] {
                    let n = [c[0] + dx, c[1], c[2] + dz];
                    let up = [n[0], n[1] + 1, n[2]];
                    let down = [n[0], n[1] - 1, n[2]];
                    let above_c = [c[0], c[1] + 1, c[2]];
                    let conn = if dset.contains(&n) {
                        Some("side")
                    } else if dset.contains(&up) && !solid.contains(&above_c) {
                        Some("up")
                    } else if dset.contains(&down) && !solid.contains(&n) {
                        Some("side")
                    } else if let Some((kind, facing)) = comps.get(&n) {
                        match *kind {
                            "repeater" => ((axis == "ns") == (*facing == "north" || *facing == "south")).then_some("side"),
                            _ => Some("side"),
                        }
                    } else {
                        None
                    };
                    if let Some(k) = conn {
                        sides.push((name, k));
                    }
                }
                // (one connection draws a line through: the far side points too)
                if sides.len() == 1 {
                    let far = match sides[0].0 {
                        "north" => "south",
                        "south" => "north",
                        "west" => "east",
                        _ => "west",
                    };
                    sides.push((far, "side"));
                }
                let get = |s: &str| sides.iter().find(|(n, _)| *n == s).map_or("none", |(_, k)| *k);
                over(&mut b, c, &format!("redstone_wire[east={},north={},power=0,south={},west={}]", get("east"), get("north"), get("south"), get("west")));
                over(&mut b, [c[0], c[1] - 1, c[2]], "glass");
            }
            for (&c, &(kind, facing)) in &comps {
                if kind == "repeater" && c != [2, 0, 2] {
                    let delay = if c[2] == -6 && (0..=4).contains(&c[0]) { 2 } else { 1 };
                    over(&mut b, c, &format!("repeater[delay={delay},facing={facing},locked=false,powered=false]"));
                    over(&mut b, [c[0], c[1] - 1, c[2]], "glass");
                }
            }
            over(&mut b, [2, -2, -2], "stone");
            over(&mut b, [5, -2, -2], "stone");
            // (9fc3155f 05:00: the two stones' tops were golem floors off the pad and unreachable
            // stroll targets: glass over each (glass is no golem floor, and the stone's top keeps no
            // standing room); the stones stay solid conductors for their torches)
            over(&mut b, [2, -1, -2], "glass");
            over(&mut b, [5, -1, -2], "glass");
            over(&mut b, [3, -2, -2], "redstone_wall_torch[facing=east,lit=true]");
            over(&mut b, [6, -2, -2], "redstone_wall_torch[facing=east,lit=true]");
            if bench {
                standin(&mut b, LATCH_RLS_IN, "lever[face=floor,facing=north,powered=false]")?;
                standin(&mut b, [LATCH_RLS_IN[0], LATCH_RLS_IN[1] - 1, LATCH_RLS_IN[2]], "glass")?;
                standin(&mut b, LATCH_ARM_IN, "lever[face=floor,facing=north,powered=false]")?;
                standin(&mut b, [LATCH_ARM_IN[0], LATCH_ARM_IN[1] - 1, LATCH_ARM_IN[2]], "glass")?;
            }
            over(&mut b, [2, -1, 1], "glass");
            lines.push("The P_HOLD latch (day-1's form (b), a/1902620467569082; latching in 26.3, tick-exact a/1841767613704681), transplanted one level down (rig (x,0,z) -> stop (x+2,-2,z-2), every wire and repeater on glass at y -3): B2 (2,-2,-2) stone, QBAR (3,-2,-2) wall torch, BASE (5,-2,-2) stone, Q (6,-2,-2) wall torch; the branch repeater (2,-2,-3) and the reset repeater (5,-2,-3) facing north; the reset chain five delay-2 repeaters (0..4,-2,-6) facing west (20 gt); Q's feedback along z 0 into B2's south face; the OR line RLS + Q climbs (0,-2,1) -> (1,-1,1) on glass (1,-2,1) -> (2,0,1) on glass (2,-1,1) into the drive repeater (2,0,2); Q's tap line along x 8 and z 2 into the slope's tap repeater (1,-2,4) (TAP = Q + 2); RLS_IN (-1,-2,-2) and ARM_IN (6,-2,-5) are the logic block's inputs (ARM held >= 3 gt; RLS's source a component, never a torch on a block: Q backfeeds the OR line); expected PARK lit [RLS + 2, RLS + 26).".to_string());
        }
        // (K, the driven T: stored north_east (the arrival's shape); its drive's conductor a slime
        // under it, which powers only K; the bench's stand-in a ceiling lever under the slime)
        over(&mut b, [-2, 0, 3], "rail[shape=north_east,waterlogged=false]");
        over(&mut b, [-2, -1, 3], "slime_block");
        if bench {
            standin(&mut b, K_CTRL, "lever[face=ceiling,facing=north,powered=true]")?;
        }
        // (the siding: plain rails out to the park, no detector beside K or the park; all dry,
        // ironspec 23:40 (1): no water at the stop ((-1,0,3) wet seats 32 / 408; P wet stalls 144))
        over(&mut b, [-1, 0, 3], "rail[shape=east_west,waterlogged=false]");
        for x in 0..px {
            over(&mut b, [x, 0, 3], "rail[shape=east_west,waterlogged=false]");
            over(&mut b, [x, -1, 3], "glass");
        }
        // (the park: a braked powered rail ascending east; its conductor the slime east of it, fed
        // from the north by a plain repeater (26.3: FACING names its input side); the bench's
        // stand-in a redstone block written at the repeater's input)
        over(&mut b, [px, 0, 3], "powered_rail[powered=false,shape=ascending_east,waterlogged=false]");
        over(&mut b, [px, -1, 3], "glass");
        over(&mut b, [px + 1, 0, 3], "slime_block");
        // (9fc3155f 03:34: the slime's top (px + 1, 1, 3) was a golem floor off the pad; 9fc3155f 05:07:
        // glass straight over it also cut the night path of (7,0,3), which walks through that cell:
        // the glass two above, at (px + 1, 3, 3), leaves the top 2 blocks of room: a villager (1.95)
        // still walks it, a golem (2.7) can't spawn there; glass is no golem floor itself; the build's
        // temporary rail at (px + 1, 1, 3) and the park rail's ascent unchanged under it)
        over(&mut b, [px + 1, 3, 3], "glass");
        over(&mut b, [px + 1, 0, 2], "repeater[delay=1,facing=north,locked=false,powered=false]");
        over(&mut b, [px + 1, -1, 2], "glass");
        if bench && !latch_b {
            standin(&mut b, [px + 1, 0, 1], &ctl_block([px + 1, 0, 1], "stone"))?;
            standin(&mut b, [px + 1, -1, 1], "glass")?;
        }
        // (the rest-keyed SENSED: a tripwire over the park cell, its band met by the resting cart's
        // box on the slope, hooks north-south on glass, diagonal to the park rail)
        over(&mut b, [px, 1, 3], "tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]");
        over(&mut b, [px, 1, 2], "tripwire_hook[attached=true,facing=south,powered=false]");
        over(&mut b, [px, 1, 4], "tripwire_hook[attached=true,facing=north,powered=false]");
        over(&mut b, [px, 1, 1], "glass");
        over(&mut b, [px, 1, 5], "glass");
        lines.retain(|l| !(l.starts_with("integrity:") || l.starts_with("update-only: 0,0,3") || l.starts_with("switches: 1,0,3") || l.starts_with("The sensed park") || l.starts_with("contacts:") || l.starts_with("contact-speeds:")));
        // (G1 04:08a "also": the re-laid variant's own prose, not the fold's (its park (1,0,4), the
        // run-up along z 4, the P_HOLD torch (2,-1,4)))
        lines.retain(|l| !(l.starts_with("The bed stop (A) with the run-up") || l.starts_with("E sets the rider down on the bed foot's top") || l.starts_with("The drives, no lever and no stand-in in this file")));
        lines.insert(0, format!("The bed stop (A), re-laid (ironspec 21:42 -> 00:08; local frame; loop = local + (0,-27,-1); the bed's head at (-3,0,4)): A6's arrival (the entry booster, the slope down, the capped flat, the curve south, the flats), the arming detector ARM (-2,0,1), the activator E (-2,0,2) (always on: its torch (-3,0,2) on glass), K (-2,0,3) the driven T (north_east for the arrival onto the siding, south_east for the return to P), the siding (-1,0,3) / (0,0,3) out and back, the park ({px},0,3) a braked powered rail ascending east on its slime ({},0,3) under glass ({},2,3), its drive a plain repeater ({},0,2) facing north, the pickup cell P (-2,0,4) straight north_south, A6's exit (the cap (-2,1,5), the curve, the capped booster, the slope hold (1,-1,5), R (4,-1,5)).", px + 1, px + 1, px + 1));
        lines.insert(1, "E sets the rider down on the bed head's top (the landing in `stand-up:`): SleepInBed fires at once. The empty cart runs E -> K -> the siding and brakes on the park; DAY-1's controller (rev 20:00) opens the window once the napper sleeps; the zombie wakes it; it stands up on P (its only stand-up cell) and presses P_STRING; RLS releases the park (PARK [RLS + 2, RLS + 26) by day-1's form-(b) latch in the -latch file, the bench's named stand-in in the plain file); the cart runs back through K (south_east) into P, seats the napper and runs on to R (the rider-keyed RS_ready string over it). One cart per visit, the park released once per visit.".to_string());
        lines.insert(2, "The drives: K's (its slime (-2,-1,3)), the park's (the repeater (2,0,2)'s input (2,0,1)) and the window's (-5,1,8) are DAY-1's cells, empty in the plain file; the -latch file lays day-1's form-(b) P_HOLD latch one level down (rig (x,0,z) -> stop (x+2,-2,z-2)), its inputs RLS_IN (-1,-2,-2) and ARM_IN (6,-2,-5) empty; a bench lays its stand-in levers there as declared overrides.".to_string());
        // (the re-laid stop's cart contacts, re-declared from its reference run (the 408 band on the
        // re-laid bench, 58f770cc Nov 14 02:05): the siding's flush run along the sealed glass
        // (-1,0,4), and the cart in P flush with the bed's east faces (foot and head, one clip on both
        // cells))
        let refcmd = "`mctas mobfarm bedtier-bench designs/farms/iron/parts/bed-tier-min/relaid/bed-tier-relaid-bench.txt --speeds 0.05..0.55/0.01 --from 1 --to 8 --tap 265,10 --adopted-terms 52,0,12,54,6,6,2,6,2,10,1 --park-width 24 --reference-check`";
        let bed_face = |cell: &str, part: &str| format!("{cell}=the bed {part}'s east face (its 0.5625-high box), met flush by the cart's west face in P (-2,0,4) as it turns out of K, the bed unchanged across it | class=design | face=east | ticks<=1 why=a second tick is the cart stopped against the bed, a stall | events<=1 why=one pass into P per visit | depth<=0.0850+eps=0.0001 why=main's depth is the clipped amount of the cart's westward request out of K's curve that the face stops, the deepest 0.0850 over the 408 visits (one unit of the fourth decimal as eps) | speed_out>=0.0746+eps=0.0001 why=the least 0.0746 over the 408 visits, the carry itself judged directly (NO STALL after every event, the pickup seating every visit) | ref=tree 79ca6b667dd6 (car 1) {refcmd}: 336 events over 336 of 408 visits (42 of 51 entry speeds, every seed of each), 1 tick, at most 1 per cart, speed_in 0.0912..0.1444, speed_out 0.0746..0.1346");
        lines.push(format!("contacts: -1,0,4=the sealed glass's north face, met flush by the cart's south face on the siding (-1,0,3) as it leaves K's curve east and returns west into it | class=design | face=north | ticks<=1 why=a second tick is the cart stopped against the glass | events<=2 why=the out-and-back siding: the arriving empty cart once, the released one once | depth<=0.3795+eps=0.0001 why=main's depth is the clipped amount of the cart's southward request at K's turn that the face stops, the deepest 0.3795 over the 408 visits | speed_out>=0.1440+eps=0.0001 why=the least 0.1440 over the 408 visits (never a stall) | ref=tree 79ca6b667dd6 (car 1) {refcmd}: 416 events over 400 of 408 visits (50 of 51 entry speeds), 1 tick, at most 2 per cart, speed_in 0.2033..0.5657, speed_out 0.1440..0.5611; {}; {}", bed_face("-3,0,3", "foot"), bed_face("-3,0,4", "head")));
        let bed_speeds = "meets 0.05,0.06,0.07,0.08,0.09,0.10,0.11,0.12,0.13,0.14,0.15,0.16,0.17,0.18,0.19,0.20,0.21,0.22,0.23,0.24,0.25,0.26,0.27,0.28,0.29,0.31,0.32,0.33,0.34,0.35,0.36,0.38,0.39,0.40,0.41,0.42,0.44,0.45,0.47,0.48,0.49,0.50";
        // (G1's B3 format: one `contact-speeds:` line per declaration)
        lines.push("contact-speeds: -1,0,4=meets 0.05,0.06,0.07,0.08,0.09,0.10,0.11,0.12,0.13,0.14,0.15,0.16,0.17,0.18,0.20,0.21,0.22,0.23,0.24,0.25,0.26,0.27,0.28,0.29,0.30,0.31,0.32,0.33,0.34,0.35,0.36,0.37,0.38,0.39,0.40,0.41,0.42,0.43,0.44,0.45,0.46,0.47,0.48,0.49,0.50,0.51,0.52,0.53,0.54,0.55; of 0.05..0.55/0.01".to_string());
        lines.push(format!("contact-speeds: -3,0,3={bed_speeds}; of 0.05..0.55/0.01"));
        lines.push(format!("contact-speeds: -3,0,4={bed_speeds}; of 0.05..0.55/0.01"));
        // (G1 01:14b / tick-exact 02:17 / ironspec 02:35 (4): the park rail's build order, checked by
        // main's build-order: every plain placement order of the slime and the rail ends east_west
        // (the rail joins the siding flat; nothing above it to the east to ascend to), so the park
        // is built with a temporary rail on the slime; laid before its slime it pops)
        // (vcompose 03:19 / ironspec 03:15: the stand-up feet the stop leaves a villager at, exact,
        // for the flee static's walk starts; the bench measures both and fails on a mismatch)
        lines.push("stand-up: -2.4986,0.5625,4.4868=the eject's landing: E sets the napper down on the bed head's top (-3,0,4) on every visit (measured over the 408 band: -2.4974..-2.4988, 4.4867..4.4884, the bench's check tolerance 0.01: the villager's box stays inside the same cell, so getStart's start node is the same); -1.5,0,4.5=the wake's stand-up on P (-2,0,4) (the bed's stand-up position, exact on all 408 visits; the napper's only on-foot cell after a wake)".to_string());
        // (dfb89499 02:47 / ironspec 02:54 (4): the sequence in build-order's declared-sequence
        // syntax, `~` a temporary cell; the prose and the check on the note line)
        lines.push(format!("build-order: park={sx},0,3=minecraft:slime_block;~{sx},1,3=minecraft:rail[shape=north_south,waterlogged=false];{px},0,3=minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false];{sx},1,3=air", sx = px + 1));
        lines.push(format!("build-order-note: the slime ({sx},0,3) first, then a TEMPORARY rail on it at ({sx},1,3), then the park's powered rail ({px},0,3) (it ascends east to meet the temporary rail), then the temporary rail removed (the park keeps ascending_east): every plain placement order of the slime and the park rail ends east_west (main's build-order: 2 orders x 2 facings, all east_west), and the park rail laid before its slime pops (tick-exact's laid-latch rig 02:17); the check: `mctas mobfarm build-order <this file> --t {px},0,3 --cell {sx},0,3=minecraft:slime_block --cell {px},0,3=<the park rail> --want ascending_east --sequence 'the build|ascending_east|{sx},0,3=minecraft:slime_block;{sx},1,3=minecraft:rail[shape=north_south,waterlogged=false];{px},0,3=<the park rail>;{sx},1,3=air' --sequence 'the park rail before its slime (the positive control)|ascending_east|{px},0,3=<the park rail>;{sx},0,3=minecraft:slime_block'` (the build sequence ends ascending_east; the control fails by name; the plain orders end east_west, the expected finding until build-order's declared-sequence mode)", sx = px + 1));
        // (9fc3155f 01:50 / 02:46: E's allowed power sources, keyed by the SOURCE cell: its
        // always-on torch and ARM's detector rail beside it (redundant: E is always powered))
        lines.push("allow-powered: -3,0,2=E's always-on redstone torch, on glass (-3,-1,2): it powers the activator (-2,0,2) on every tick; -2,0,1=ARM's detector rail beside E: it also powers the activator while a cart presses it, redundant (E is always powered by its torch), so it never toggles E (declared, not moved: moving ARM would shift the edge COOL, the latch's ARM_IN and K's SET key on)".to_string());
        // (G1 02:01: every guard rail's cap is IRON: a wooden trapdoor opens by a villager's hand)
        lines.push("guard-caps: every guard rail stands on an IRON trapdoor cap, which only redstone opens, never a hand (G1 02:01): (-2,2,2) on (-2,1,2), (-1,2,-4) on (-1,1,-4), (0,2,5) on (0,1,5), (-1,2,6) on (-1,1,6), (-3,3,3) on (-3,2,3); the lone guard strings (-1,1,2) / (-1,1,5) stand over iron trapdoors too; no cap in the stop is wooden".to_string());
        // (9fc3155f 01:01 / 58f770cc 01:02: the guard cells reworded for the re-laid stop: no walker
        // reaches them from P or the bed; (-1,1,2) is a node of the riders' HOME route from K's
        // siding (a rider-only start), its lone string read by nothing)
        // (G1 04:08a "also": the text names the exit's first cap as laid: seal-P's glass, or the iron
        // trapdoor without --seal-p)
        let cap = if seal_p { "seal-P's (-2,1,5) glass" } else { "the iron trapdoor cap (-2,1,5) (no seal-P: the pathfinder routes through it, so this file's closure from P is NOT {P})" };
        b.d.meta.insert("unreachable".into(), format!("-1,1,2=E's east trapdoor top under its lone guard string (no hooks: nothing reads a press): no walker reaches it from P or the bed (K's head (-2,1,3) glass, (-1,0,4) glass, {cap}); it is a node of the riders' HOME route from K's siding, a rider-only start (the riders stay seated); -1,1,5=the hand-off's east trapdoor top under its lone guard string: diagonal to P, (-1,0,4) glass, (-2,0,5) under {cap}, (0,1,5) the curve's cap"));
        // (G1 04:08a "also": the re-laid stop's own riderless cells, every one a rail: E, K, the
        // siding, the park, back through K to P)
        b.d.meta.insert("riderless".into(), "-2,0,2+-2,0,3+-1,0,3+0,0,3+1,0,3+-2,0,4=the stop's empty cart from E (the napper set down on the bed head's top) through K onto the siding to the park, then back through K to P (the napper seated again at P)".into());
        let mut integ = vec!["-2,0,4=connections -2,0,3+-2,0,5".to_string()];
        for x in -1..px {
            integ.push(format!("{x},0,3=connections {},0,3+{},0,3", x - 1, x + 1));
        }
        lines.push(format!("integrity: {}", integ.join("; ")));
        lines.push("switches: -2,0,3=K, a driven T (ironspec 21:51 (5)): north_east powered (the arrival from E), south_east unpowered (the return into P); its drive the slime (-2,-1,3) under it, which powers only K (the bench's stand-in a ceiling lever at (-2,-2,3); DAY-1's K drive in the build: SET by ARM's edge, RESET by the park string's press)".to_string());
        lines.push("Load-bearing, FRAGILE (ironspec 23:08 (5)): the four screens (-3,1,6), (-3,1,7), (-2,1,7), (-1,1,7) cut every line from Z1 (the stop's own waker at (-5,0,10), inside the stop's footprint to z 11) to the exit's rider positions within 6 ((-2,0,5), (-2,0,6), (-1,0,6)); fault: a screen lost lets a rider see Z1 (NEAREST_HOSTILE armed on the hold and the exit); repair: the glass replaced.".to_string());
        lines.push(format!("The re-laid stop (ironspec 21:42 / 21:51 / 22:58, 58f770cc 21:44 / 22:32): P (-2,0,4) straight north_south and dry, reached from the north through K; the empty cart's out-and-back siding along z 3, all dry (ironspec 23:40 (1): no water at the stop): K, (-1,0,3) .. ({},0,3) plain, the park ({px},0,3) a braked powered rail ascending east; its drive the slime ({},0,3) by the repeater ({},0,2) from its input ({},0,1) (DAY-1's PARK line; the bench's stand-in a redstone block there); SENSED a tripwire over the park cell ({px},1,3), hooks ({px},1,2) / ({px},1,4) on glass ({px},1,1) / ({px},1,5): pressed while the cart rests on the slope (its box's top past 1.0625), released when it leaves; P_STRING east-west at (-2,1,4), hooks (-3,1,4) on (-4,1,4) and (-1,1,4) on (0,1,4); (-1,0,4) and (-1,2,4) glass; no head-top guard (ironspec 23:40 (3)(a)); the bed column's keep-clear (-3,2,4) (-3,3,4) guards the riders' HOME route's final drop onto the head node (-3,1,4), which holds P_STRING's west hook by design (no collision box: the route completes onto it, 9fc3155f 01:01; vcompose 03:28 / ironspec 03:30 (3)).", px - 1, px + 1, px + 1, px + 1));
    }
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    Ok(b.text(&refs))
}

/// (ironspec 06:14 / G1, tick-exact's on-farm re-claim question) The strand's dusk on (A)'s bed
/// foot: a napper stood on the foot's top in the afternoon (IDLE) with HOME = the bed and a planted
/// CANT_REACH stamp older than 1,200, run through dusk: the first REST tick's
/// SetWalkTargetFromBlockMemory release branch (HOME erased, the ticket given back), then the
/// re-claim (AcquirePoi) and the lie-down. Printed per tick of note: HOME, the bed's free tickets and
/// every villager holding it. `blocked`: the known positive, a second villager (sealed in the
/// run-out, HOME = the same bed) holding the bed's ticket, so the re-claim can't happen.
/// (ironspec 11:37 (3)) The exit boost's real carry threshold, a FORCED ROW: a LOADED cart (a seated
/// villager: a ridden cart's friction) set on the exit boost (-2,0,5)'s centre moving south at each
/// speed, run until it reaches R (4,-1,5) or 400 gt pass; per speed: reached the next booster
/// (-1,0,6) and R or not, the least speed on the way. The bound `speed_out>=` cites the least speed
/// that carries and the greatest that stalls.
pub fn boost_probe(spec: &FarmSpec, speeds: &[f64]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let (mut least_ok, mut most_fail): (Option<f64>, Option<f64>) = (None, None);
    for &v in speeds {
        let ro = RunOpts { ai: true, ..RunOpts::steady(1, 24000, WalkMode::Random) };
        let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
        run.place_detectors();
        let start = [-1.5, 0.0625, 5.5];
        let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}"))?;
        {
            let w = &mut run.world;
            let i = w.entities.list.iter().position(|e| e.id == id).ok_or("the probe cart is gone")?;
            w.entities.list[i].pos = Vec3::new(start[0], start[1], start[2]);
            w.entities.list[i].delta_movement = Vec3::new(0.0, 0.0, v);
            for pid in w.entities.list[i].passengers.clone() {
                if let Some(j) = w.entities.list.iter().position(|e| e.id == pid) {
                    redsim::entity::position_rider(w, i, j);
                }
            }
        }
        let (mut at_booster, mut at_r, mut least) = (None, None, v);
        for t in 1..=400 {
            run.tick();
            let Some(c) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) else { break };
            let s = c.delta_movement.x.hypot(c.delta_movement.z);
            if at_booster.is_none() {
                least = least.min(s);
            }
            let b = c.block_position();
            if b == BlockPos::new(-1, 0, 6) && at_booster.is_none() {
                at_booster = Some(t);
            }
            if b == BlockPos::new(4, -1, 5) {
                at_r = Some(t);
                break;
            }
        }
        // (carried: the next booster (-1,0,6) within 60 gt, so the probe judges the exit boost alone,
        // with or without the slope hold downstream; a stalled cart creeps there after 200 gt or never;
        // G1's Q15: the carried runs reach it at 8..9 gt and the stalled ones after 220 (the probe
        // re-run on this tip's fold: `bedtier-bench --boost-probe`, its per-start print), so 60
        // separates them with room both sides)
        let ok = at_booster.is_some_and(|tb| tb <= 60);
        if ok {
            least_ok = Some(least_ok.map_or(v, |m: f64| m.min(v)));
        } else {
            most_fail = Some(most_fail.map_or(v, |m: f64| m.max(v)));
        }
        out.push(format!("boost probe: a loaded cart set on (-2,0,5) moving south at {v:.4} b/t: the next booster (-1,0,6) at t {at_booster:?}, R at t {at_r:?}, its least speed before the booster {least:.4}: {}", if ok { "carried" } else { "STALLED" }));
    }
    out.push(format!("boost probe: the least speed that carries {least_ok:?}; the greatest that stalls {most_fail:?}{}", if most_fail.is_none() { " (no stall in the sweep: extend it down)" } else { "" }));
    // (G1's N14: a sweep that brackets no threshold (no stall, or no carry) fails: it measured
    // nothing)
    if most_fail.is_none() || least_ok.is_none() {
        return Err(format!("{}\nboost probe: no threshold bracketed (a carried and a stalled speed are both needed)", out.join("\n")));
    }
    Ok(out)
}

pub fn reclaim_row(spec: &FarmSpec, seed: u64, blocked: bool, at: Option<[f64; 3]>) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed as i64, 3 * 24000, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    run.place_detectors();
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    let mut out = Vec::new();
    // (ironspec 01:41b: this row plants its CANT_REACH stamp: not evidence until re-run with a real
    // partial path)
    out.push("NOT EVIDENCE (ironspec 01:41b): this row plants the napper's CANT_REACH stamp (1,300 old) instead of arming it by a real partial path".to_string());
    let want = 11_800 + (seed as i64 * 7) % 100;
    let mut guard = 0;
    while run.world.day_time().rem_euclid(24000) != want {
        run.tick();
        guard += 1;
        if guard > 30_000 {
            return Err("never reached the afternoon".into());
        }
    }
    let h = BED_HEAD;
    let home = format!("Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}", h[0], h[1], h[2]);
    let spot = at.unwrap_or([BED_HEAD[0] as f64 + 0.5, 0.5625, BED_HEAD[2] as f64 - 0.5]);
    let nid = redsim::entity::spawn(&mut run.world, "minecraft:villager", spot, Some(&format!("{{{home}}}")))?;
    let other = if blocked {
        // (the known positive: a second villager holding the bed, sealed on the run-out; a loaded
        // HOME takes no ticket (as in the game: ironspec 06:31 real), so its ticket is TAKEN here
        // explicitly, a declared stand-in for a claimant that acquired it: before -> after printed)
        let o = redsim::entity::spawn(&mut run.world, "minecraft:villager", [EXIT_DET[0] as f64 + 4.5, -1.0, EXIT_DET[2] as f64 + 0.5], Some(&format!("{{{home}}}")))?;
        let bp = BlockPos::new(h[0], h[1], h[2]);
        let before = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bp);
        redsim::entity::mob::ai::poi::take(&mut run.world, bp);
        let after = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bp);
        if before == after {
            return Err(format!("the known positive's ticket take changed nothing ({before:?} -> {after:?}): the positive can't block"));
        }
        out.push(format!("the known positive's stand-in: villager {o} holds HOME = the bed and its ticket taken (the bed's free tickets {before:?} -> {after:?})"));
        Some(o)
    } else {
        None
    };
    let idx = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    let bed = BlockPos::new(h[0], h[1], h[2]);
    let holders = |w: &World| -> Vec<u32> {
        (0..w.entities.list.len())
            .filter(|&i| !w.entities.list[i].removed && matches!(&w.entities.list[i].kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager))
            .filter(|&i| redsim::entity::mob::ai::brain::brain(w, i).home == Some(bed))
            .map(|i| w.entities.list[i].id)
            .collect()
    };
    // (the planted stamp: armed 1,300 gt ago)
    if let Some(vi) = idx(&run.world, nid)
        && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
    {
        let now = run.world.time;
        redsim::entity::mob::ai::brain::ex_mut(&mut run.world, vi).mem.cant_reach_since = Some(now - 1300);
    }
    let tickets = |w: &mut World| redsim::entity::mob::ai::poi::free_tickets(w, bed);
    let t0 = tickets(&mut run.world);
    out.push(format!("start (day time {want}): the napper {nid} at ({:.2},{:.2},{:.2}), HOME = the bed, a stamp armed 1,300 gt ago; the bed's free tickets {t0:?} (ticket state uncertain: pre-fix redsim), holders {:?}{}", spot[0], spot[1], spot[2], holders(&run.world), other.map_or(String::new(), |o| format!(" (the known positive: villager {o} holds the bed)"))));
    // (G1: the re-claim's path, probed on a copy of the world (the napper's own navigation, the
    // bed's head as AcquirePoi's target with HOME's valid range 1): its start node, end node, the
    // end's distance to the head, `reached`; the attempts themselves and their backoff are
    // redsim's MCTAS_TRACE_ACQUIRE lines)
    let probe = |w: &World, vi: usize| -> String {
        let mut wc = w.clone();
        let p = redsim::entity::mob::ai::with_nav(&mut wc, vi, |w, nm, nav, arena| nav.create_path_positions(w, nm, &[bed], 1, arena));
        match p {
            None => "no path".to_string(),
            Some(p) => {
                let s = p.nodes.first().map(|n| (n.x, n.y, n.z));
                let e = p.nodes.last().map(|n| (n.x, n.y, n.z));
                let d = e.map(|(x, y, z)| (((x - bed.x).pow(2) + (y - bed.y).pow(2) + (z - bed.z).pow(2)) as f64).sqrt());
                let m = e.map(|(x, y, z)| (x - bed.x).abs() + (y - bed.y).abs() + (z - bed.z).abs());
                format!("start node {s:?}, end node {e:?} ({} nodes), the end's distance to the head {:.2} (manhattan {m:?}), reached {}", p.nodes.len(), d.unwrap_or(f64::NAN), p.reached)
            }
        }
    };
    if let Some(vi) = idx(&run.world, nid) {
        out.push(format!("t 0: the re-claim path probe: {}", probe(&run.world, vi)));
    }
    let (mut last_home, mut last_act, mut last_sleep) = (Some(bed), None, false);
    let mut released_at: Option<i64> = None;
    for t in 1..=1200 {
        run.tick();
        let Some(vi) = idx(&run.world, nid) else {
            out.push(format!("t {t}: the napper is gone"));
            break;
        };
        let b = redsim::entity::mob::ai::brain::brain(&run.world, vi);
        let (home, act) = (b.home, b.activity);
        let asleep = matches!(&run.world.entities.list[vi].kind, redsim::entity::EntityKind::Mob(m) if m.sleeping_pos.is_some());
        if home.is_none() && released_at.is_none() {
            released_at = Some(t);
        }
        if home != last_home || Some(act) != last_act || asleep != last_sleep {
            let tk = tickets(&mut run.world);
            let pr = probe(&run.world, vi);
            let w = &run.world;
            out.push(format!(
                "t {t} (day time {}): activity {}, HOME {home:?}, asleep {asleep}, at ({:.2},{:.2},{:.2}); the bed's free tickets {tk:?}, holders {:?}; the re-claim path probe: {pr}",
                w.day_time().rem_euclid(24000),
                act.name(),
                w.entities.list[vi].pos.x,
                w.entities.list[vi].pos.y,
                w.entities.list[vi].pos.z,
                holders(w)
            ));
            last_home = home;
            last_act = Some(act);
            last_sleep = asleep;
        } else if released_at.is_some_and(|r| (t - r) % 100 == 0) {
            out.push(format!("t {t}: still HOME {home:?}, asleep {asleep}; the re-claim path probe: {}", probe(&run.world, vi)));
        }
        if asleep && t > 20 {
            break;
        }
    }
    let reclaimed = released_at.map(|_| last_home.is_some());
    out.push(format!(
        "verdict: {}",
        match (released_at, last_home, last_sleep) {
            (None, _, _) => "HOME never released in 1,200 gt".to_string(),
            (Some(r), Some(_), true) => format!("released at t {r}, re-claimed and asleep"),
            (Some(r), Some(_), false) => format!("released at t {r}, re-claimed, not asleep by the end"),
            (Some(r), None, _) => format!("released at t {r}, NOT re-claimed by t 1200"),
        }
    ));
    // (G1's C12: the verdict judged, never print-only: the blocked positive must NOT re-claim (else
    // it doesn't bite); a free row is judged against the run's declared expectation)
    if blocked && reclaimed == Some(true) {
        return Err(format!("the blocked positive re-claimed the bed: it doesn't bite ({})", out.last().cloned().unwrap_or_default()));
    }
    if released_at.is_none() {
        return Err(format!("HOME never released in 1,200 gt: the row tests nothing ({})", out.last().cloned().unwrap_or_default()));
    }
    out.push(format!("re-claimed: {}", reclaimed == Some(true)));
    Ok(out)
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
    /// The napper's HOME at the visit's end (the farm-stop bar).
    pub home_at_end: bool,
    /// A release at the stop healed by a re-claim before the visit's end (printed, priced).
    pub healed_release: Option<i64>,
    /// The re-claim's tick after a release (the delay = this - home_lost_at).
    pub reclaimed_at: Option<i64>,
    /// (ironspec 06:40 (2)) The waker's consequence from the wake to the pickup + 40: each change
    /// of the walk target, path and stamp; the first stamp seen after the wake and when it cleared.
    pub waker_lines: Vec<String>,
    pub waker_stamp: Option<i64>,
    pub waker_cleared: Option<i64>,
    /// (ironspec 00:27b) every arming / clearing of the stamp after the wake, with its walk target's
    /// kind (HOME, flee / stroll), the napper's cell and the path's ends
    pub stamp_events: Vec<String>,
    /// (G1) The napper's feet to the waker zombie's from the wake to the pickup + 40: the least
    /// distance, the least with eye-to-eye sight, and the ticks under 6 with sight (flee triggers).
    pub waker_dist_min: Option<f64>,
    pub waker_seen_dist_min: Option<f64>,
    pub waker_flee_ticks: usize,
    /// Ticks within 6 of the waker with NEAREST_HOSTILE remembered (ironspec 07:47 (a): bar 0).
    pub waker_memory_ticks: usize,
    /// (ironspec 07:03) The declared contacts' per-tick lines (tick, depth, speed before / after),
    /// their tick count this visit (bound: one) and the least speed right after one.
    pub declared_contact_lines: Vec<String>,
    pub declared_contact_ticks: usize,
    pub post_contact_speed: Option<f64>,
    /// (ironspec Nov 13 10:42 (3) / 10:58: the ONE bounded judge on main) every contact hit of the
    /// cart this visit, horizontal (`contacts::cart_hits`) and vertical (`cart_vertical_hits`), its
    /// cell in the design frame; and the cart's motion per tick (world tick, horizontal speed, vy:
    /// the judge's NO STALL reads it).
    pub hits: Vec<crate::contacts::Hit>,
    pub motion: Vec<(i64, f64, f64)>,
    /// The first tick the cart is in the exit booster's cell ((A) --exit-south: (-1,0,6)).
    pub booster_at: Option<i64>,
    /// (ironspec 09:04) After the wake: the first tick NEAREST_HOSTILE is empty, the first tick
    /// out of PANIC, and the cart's first tick in (-2,0,5) after the pickup.
    pub mem_empty_at: Option<i64>,
    pub panic_end_at: Option<i64>,
    pub cell25_at: Option<i64>,
    /// (rev 20:00 G20 / G1's N5) the longest run of end-of-ticks with the window open on the napper
    /// standing awake on P
    pub open_on_p_max: usize,
    /// (G1 19:26) ticks with a ridden cart on the activator (-1,0,3), and what happened
    pub ridden_on_act: usize,
    pub ridden_act_lines: Vec<String>,
    /// (G1 19:10) ticks with the napper's feet (on foot) in a guard string's cell
    pub guard_cell_ticks: usize,
    /// (G1 17:40) the window's first end-of-tick open (the shutter read unpowered)
    pub open_state_at: Option<i64>,
    /// the window's first end-of-tick shut after it opened (the state, one after the write's C)
    pub close_state_at: Option<i64>,
    /// (ironspec 17:35) the cart's first end-of-tick on the sensed park's cell, before the release
    pub park_sense_at: Option<i64>,
    /// (G1 17:49 (2)(a)) every rising edge of the sensed park's detector in the visit
    pub park_sense_presses: Vec<i64>,
    /// (G1 02:27 (3)) the park string's state changes while the cart rests on the park (after
    /// SENSED's first press, before the release): (tick, pressed after it, the cart's x)
    pub string_flicker: Vec<(i64, bool, f64)>,
    /// (G1 02:27 (3)) K's drive OFF edges in the visit (its reset; bar: one)
    pub k_offs: usize,
    /// (G1 02:27 (3), `--latch-b`) the park rail's lit edges after RLS (bar: one, no double release)
    pub park_rises: usize,
    /// (G1 03:46 B1) the visit ran the laid latch (its park lightings judged), else the stand-in
    pub latch_mode: bool,
    /// (G1 04:08a item 4) the laid latch's park lightings before RLS (bar 0)
    pub park_rises_before_rls: usize,
    /// (G1 02:47) the empty cart's speed crossing the siding's centre x -0.5 eastward, toward the park
    pub siding_speed: Option<f64>,
    /// (vcompose 03:19) the exact feet where the eject leaves the napper on the ground, and where
    /// its first wake stands it up (the flee static's walk starts)
    pub landed_exact: Option<[f64; 3]>,
    pub woke_exact: Option<[f64; 3]>,
    /// (G1's C4) the sensed park's cell held no detector rail on some tick (the stand-in read)
    pub sense_stand_in: bool,
    /// (ironspec 21:15) ticks the park rail rose with the bench's drive off (RLS low)
    pub park_pulses_rls_low: Vec<i64>,
    /// the cart's first end-of-tick off the park rail after the release
    pub park_left_at: Option<i64>,
    /// (ironspec 21:20) the iteration whose write ended PARK's stretch
    pub park_off_at: Option<i64>,
    /// (G1 21:20 (1)) the tick the planted step-off moved the napper off P
    pub step_off_at: Option<i64>,
    /// (day-1 21:33) the tick the planted shove hit the woken napper
    pub shove_at: Option<i64>,
    /// (ironspec 21:21 (3)) P_STRING's (press, drop) spans
    pub p_spans: Vec<(i64, i64)>,
    /// (day-1 21:33) a real P_STRING laid at (-2,1,4) was read (not the stand-in)
    pub p_string_real: bool,
    /// (the re-laid stop) the layout's park cell and name
    pub park_cell: [i32; 3],
    pub layout_name: &'static str,
    /// (G1's CHANGES 8 B1) the layout's stand-in cells (waker shutter, park drive, K's drive)
    pub ctl_cells: Vec<[i32; 3]>,
    /// (ironspec 21:51 (5)) K's shape changes (tick, shape) and the cart's ticks in K: the arriving
    /// cart's first and last, the released cart's first
    pub k_shapes: Vec<(i64, String)>,
    pub k_arrive: Option<(i64, i64)>,
    pub k_return_first: Option<i64>,
    /// (the re-laid stop) the cart's highest x on the park before the release, and the park's top x
    pub park_max_x: Option<f64>,
    /// (ironspec 23:00 (3)) the no-HOME row: the erase's tick, and the napper's on-foot cells after
    /// it (first tick on each), its highest feet y
    pub no_home_from: Option<i64>,
    pub no_home_cells: Vec<(i64, [i32; 3], f64)>,
    /// (G1's CHANGES 8 C4) PARKED's first set (the adopted emulation, stamped t - 1)
    pub parked_set_at: Option<i64>,
    /// (G1's D5) the R-press row's hand-lit Q, when it fired
    pub hand_q_at: Option<i64>,
    /// (ironspec 01:17 (2), `--latch-b`) the park rail's first lit and first dark tick after RLS
    pub park_lit: Option<(i64, Option<i64>)>,
    /// (ironspec 02:16, `--plant-empty`) where the planted empty cart ended, and the napper's state
    pub planted_end: Option<String>,
    /// (tick-exact 22:44) the napper's feet y range while awake on P before the pickup
    pub p_feet_y: Option<(f64, f64)>,
    /// (ironspec 22:58 / G1 23:00) the empty cart's speeds: into the park, out of the siding's water
    /// on the way out; after the release into the water, out of it at K, and into P
    pub park_entry_speed: Option<f64>,
    pub water_out_arrive: Option<f64>,
    pub water_in_return: Option<f64>,
    pub water_out_return: Option<f64>,
    pub p_entry_speed: Option<f64>,
    /// (ironspec 17:32 (3)) the cart's first end-of-tick in P after the release
    pub cart_at_p: Option<i64>,
    /// (ironspec 17:32 (3); tick-exact 17:39: 60 gt from the dismount) the first end-of-tick with
    /// the napper's boarding cooldown back at 0 after the eject set it
    pub cooldown_end: Option<i64>,
    /// The waker's window shut on the wake edge (`--hide-after-wake`): its tick.
    pub close_at: Option<i64>,
    /// The cart's horizontal speed (and the tick) when its pickup box first met the napper on foot
    /// after the release (seated only above 0.1).
    pub napper_contact: Option<(f64, i64)>,
    /// (ironspec 23:40 (1)) the napper's feet (x, z) at the first contact; the first tick end the
    /// cart's un-inflated box overlapped the napper on foot; the napper's greatest displacement from
    /// its contact position before it was seated
    pub napper_at_contact: Option<(f64, f64)>,
    /// (ironspec 23:54 (a)) the cart's and the napper's y ranges at the first contact
    pub contact_y: Option<(f64, f64, f64, f64)>,
    pub push_overlap_at: Option<i64>,
    pub push_disp: f64,
    /// A lie-down after the wake, before the pickup (the re-sleep lock: bar none).
    pub resleep_at: Option<i64>,
    /// Its print: LAST_WOKEN, the start, the erases.
    pub resleep_line: Option<String>,
    /// (ironspec 09:38) The exit-slope hold: the loaded cart's rest (tick, position), its departure,
    /// its creep while held, the rider's NEAREST_HOSTILE ticks over the hold, its stamp at the
    /// departure, the hold's prints (sight), the launch's (cell, speed) to R.
    pub hold_rest_at: Option<i64>,
    pub hold_rest_pos: Option<[f64; 3]>,
    pub hold_depart_at: Option<i64>,
    pub hold_creep: f64,
    pub hold_hostile_ticks: usize,
    pub hold_stamp_at_depart: Option<Option<i64>>,
    pub hold_lines: Vec<String>,
    /// (G1 12:04 / day-1 12:03) The wake sensor's (the observer under the bed head) rising edges,
    /// by tick.
    pub sensor_rises: Vec<i64>,
    /// The design holds the wake sensor (an observer at WAKE_SENSOR).
    pub sensor_present: bool,
    /// (G1's Q13) ticks with a declared `# integrity:` shape off.
    pub integrity_breaks: usize,
    /// (G1's Q2) the tap's stand-in writes (tick, on).
    pub tap_writes: Vec<(i64, bool)>,
    /// (G1's Q2) the loaded cart reached the slope (1,-1,5) while the slope was powered (the tap's
    /// pulse on: it rolls through, an early launch).
    pub hold_arrival_powered: bool,
    /// (G1's Q3) the held rider's sight to the waker CLEAR in either shutter state at the rest.
    pub hold_sight_clear: bool,
    /// (the emulated DAY-1 latch) its events, whether TIMEOUT released the visit, and whether the
    /// release found the napper asleep (normal-operation bar 0)
    pub latch_lines: Vec<String>,
    pub latch_timeout: bool,
    pub released_asleep: bool,
    /// the bed head OCCUPIED at the visit's end (the bed not free for the next napper)
    pub bed_occupied_at_end: bool,
    /// (G1 13:02) the P-eye static's lines (the record seed's visits)
    pub p_eye_lines: Vec<String>,
    /// (G1 13:26) the tick boundary the window opened at (F or the latch's OPEN)
    pub open_at: Option<i64>,
    pub launch_cells: Vec<([i32; 3], f64)>,
    /// (G1 C16) The cells holding water at the visit's end (the design's own water is the bar).
    pub water_cells: usize,
    /// Boardings after the release (1 expected; more is a pickup fault).
    pub boardings: usize,
    /// The least distance from HOME's head centre to the napper's feet while it stands after the
    /// wake (until its last boarding).
    pub stand_min_home: Option<f64>,
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
    /// The cart's horizontal step on the pickup tick (position delta).
    pub pickup_step: Option<f64>,
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
    /// (ironspec 15:23 / day-1 15:29: the rider-keyed RS_ready, when the design lays its string)
    /// The string's rising edges this visit, each with whether the napper was riding in R's column
    /// at that tick (the only allowed presser); the rider box's least overlap with the string's band
    /// (y 0.0625..0.15625 over (4,0,5)) and the cart's least gap under it while the cart was in R's
    /// column; the string's state last tick.
    pub rs_press: Vec<(i64, bool)>,
    /// (G1 04:31 (1)(b)) the rider's distance to the bed head's centre when RS_ready sets
    pub ride_dist_at_rs_set: Option<f64>,
    pub rs_present: bool,
    pub rs_on_prev: bool,
    pub rs_rider_overlap_min: Option<f64>,
    pub rs_cart_gap_min: Option<f64>,
    /// (ironspec 15:46 (2): the STALE guard's evidence) the napper's least gap to P_STRING's band
    /// (the awake guard's string at P + 1, (-2,1,4): x -2..-1, y 1.0625..1.15625, z 4..5) over the 12
    /// gt before its lie-down (the ring holds the last 12 ticks' gaps).
    pub pstring_gap_before_lie: Option<f64>,
    pub pstring_ring: Vec<(i64, f64)>,
    /// (G1 16:00: an awake HOME-holder within 16 of an occupied bed loses HOME) the boarded rider's
    /// distance to the bed head's centre at R's rise and at its last tick in the bench, and the first
    /// tick it is over 16.
    pub ride_dist_at_r: Option<f64>,
    pub ride_dist_last: Option<(i64, f64)>,
    pub ride_over16_at: Option<i64>,
    /// (G1 16:09: the step-off) after the wake and before the pickup: the napper's awake ticks on P
    /// in REST (calm) and in other activities, and its first awake cell off P (a step-off: the cart
    /// would leave empty).
    pub rest_on_p: usize,
    pub other_on_p: usize,
    pub stepped_off: Option<(i64, [i32; 3])>,
    /// (ironspec 16:35: the self-heal) the planted stamp release's tick, and the napper's cells on
    /// foot from its landing to its HOME's re-acquisition.
    pub release_home: Option<i64>,
    pub reclaim_cells: Vec<[i32; 3]>,
    /// (G1's B1) the adopted controller's RLS (the release pulse) this visit.
    pub rls_at: Option<i64>,
    /// (ironspec 16:35: the lock case) the visit's --erase-home-at applied (HOME erased, the ticket
    /// taken by a stand-in claimant).
    pub erase_planted: bool,
    /// (G1 15:29 / b8949c96 15:45: R4's planter) the cart's states on each tick its centre is within
    /// 1.5 of a top-half trapdoor's column at its layer or the one above: `x,y,z,vx,vy,vz,rider`
    /// (the design frame, the stored motion), with --cart-states.
    pub cart_states: Vec<String>,
    /// The cart a block past R (the visit's end), and whether the napper rode it.
    pub out_at: Option<i64>,
    pub seated_out: bool,
    /// (ironspec 21:25, every cart-motion bench) The cart's contacts with blocks, the spawn to out:
    /// (tick, the cart's position, "clipped-move", the CONTACTED block's cell, by main's shared
    /// `contacts::cart_contacts` (G1's Q1)); a contacted cell the design's `# contacts:` header
    /// doesn't declare fails the row.
    pub contacts: Vec<(i64, [f64; 3], &'static str, [i32; 3])>,
    /// The declared contacted cells this visit met (a declaration no visit meets is STALE).
    pub declared_hit: Vec<[i32; 3]>,
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
    /// (day-1 10:49) One line per visit: seed, entry speed, release, pickup, rest on the slope,
    /// departure, R, the hold's wait.
    pub visit_table: bool,
    /// (G1's C20) The run is on the built world (the module over the stop, --module-offset): the
    /// lightning bar is judged there (the stop alone is open to the sky by construction).
    pub built_world: bool,
    /// (ironspec 11:39 / G1: the build positions) The whole design built at design + offset (every
    /// laid cell, entity and stand-in write), every design-frame cell read there and the rows
    /// printed back in the design frame; the stop's own bench only.
    pub offset: [i32; 3],
    /// (day-1 12:03) The window shut this many gt after the wake edge (0 or 1: the earliest, the
    /// bench's stand-in; a real line through the wake sensor closes later).
    pub close_after: i64,
    /// (G1's Q13) The design's `# integrity:` shapes (design frame), judged every tick.
    pub integrity: Vec<([i32; 3], String)>,
    /// (G1's Q2) The slope hold's tap on the built path: its stand-in input lever (TAP_CTRL, into
    /// DAY-1's repeater TAP_REP) on at tick T for W gt, again every PERIOD gt when given.
    pub tap: Option<(i64, i64, Option<i64>)>,
    /// (G1's N13 planted row) the periodic train's pulse to leave out (k = 0 at T)
    pub tap_skip: Option<i64>,
    /// (ironspec Nov 13 10:42 (3) / 10:58) The design's bounded `# contacts:` / `# stops:`
    /// declarations, parsed by main's `contacts::declared_contacts_bounded` (bench = file - shift):
    /// the ONE judge reads them over every visit's hits.
    pub contact_decls: Vec<crate::contacts::Decl>,
    /// (G1's B3) the contacts decided by the entry speed: (cells, the meeting speeds, the reference
    /// band), from the design's `# contact-speeds:` lines.
    pub contact_speeds: Vec<(Vec<BlockPos>, Vec<f64>, Vec<f64>)>,
    /// (G1's Q15) the design's `# a6-pickup:` stranded bound (gt after the wake), read from the key.
    pub stranded_bound: Option<i64>,
    /// Fail when a declaration's stated bounds don't equal this run's measured extremes (the
    /// reference re-run check, `contacts::reference_check`: for the declaration's own `ref=` run).
    pub reference_check: bool,
    /// (G1 15:29 / b8949c96 15:45) write the cart's states near the top-half trapdoors to this file
    /// (`bench_cmd`, one line per tick: `x,y,z,vx,vy,vz,rider`, the design frame).
    pub cart_states: Option<std::path::PathBuf>,
    /// (G1 12:38 / ironspec 12:38) DAY-1's guard-free latch emulated on the real wake sensor's
    /// pulses, replacing the bench's stand-in F / close / release (superseded at 15:37).
    pub latch: Option<Latch>,
    /// (G1's B1) DAY-1's adopted stop controller emulated on the real cells (the default; None:
    /// the named stand-ins, `--controller stand-in`).
    pub adopted: Option<Adopted>,
    /// (ironspec 17:35, the sensed park) PARKED also waits for the cart's first end-of-tick on this
    /// cell + this many gt (a NAMED STAND-IN for a detector rail there feeding PARKED's set, ANDed with
    /// the timer); None: the timer alone.
    pub parked_sense: Option<([i32; 3], i64)>,
    /// (G1's B1 control twin) block P's headroom (-2,1,4) one tick after the lie-down (before the
    /// wake): the woken napper can't stand on P (the adopted controller holds; the stand-ins release
    /// an empty cart).
    pub block_p_at_wake: bool,
    /// (G1 19:26's planted row) E (-2,0,2) replaced by a plain rail at the first tick: it never
    /// ejects, so the RIDDEN cart rides on over the activator (-1,0,3)
    pub plant_e_off: bool,
    /// (ironspec 21:10 (3), the power-up row) the park drive ON from the first tick (P_HOLD come up
    /// SET): `Some(false)` reset at ARM's edge (DAY-1 21:10's ARM-reset), `Some(true)` kept on (the
    /// control: no reset)
    pub park_initial_on: Option<bool>,
    /// (rev 20:00 G21 / ironspec 21:15 (g)) PARK's stretch: the bench's drive off this many gt after
    /// it rose (DAY-1 21:18: [RLS + 4, RLS + 12), 8 wide); None leaves it on
    pub park_width: Option<i64>,
    /// (ironspec 21:20) a world open K gt after RLS (the restart's form as --restart's)
    pub restart_at_rls: Option<(i64, crate::liveness::Restart)>,
    /// (ironspec 01:50b / 02:33 (2)) the timing row's phases: one row per K with the world open at
    /// RLS + K (`--restart-at-rls A..B,HOW`)
    pub restart_at_rls_phases: Vec<i64>,
    /// (G1 03:46 B1) the flicker test's single-change twin: K's drive and PARKED read the park
    /// string's RAW state instead of SENSED's latch (it must fail by name in the flickering visits)
    pub consumers_raw: bool,
    /// (G1 21:20 (1)) the napper moved off P onto the bed foot's top K gt after RLS (an empty
    /// departure at RLS + 0 / + 1 / + 2: PARK's firing printed)
    pub step_off_at_rls: Option<i64>,
    /// (ironspec 21:21 (4)) a planted hurt K gt after ARM's edge (the earliest wake, ARM + 9)
    pub hurt_at_arm: Option<i64>,
    /// (day-1 21:33) an impulse (DX, 0.1, DZ) given to the woken napper the tick after the wake
    pub shove_at_wake: Option<(f64, f64)>,
    /// (ironspec 21:51 (5)) K's drive held: Some(true) north_east, Some(false) south_east
    pub k_stuck: Option<bool>,
    /// (ironspec 22:38 (1)(b)) the pickup's first-contact speed band (judged per visit)
    pub pickup_band: Option<(f64, f64)>,
    /// (ironspec 23:34) one line per visit with the observer's pulse ticks (the west sensor's diff)
    pub pulse_log: bool,
    /// (ironspec 23:54 (b)) the boarding cooldown the eject sets (gt; tick-exact pins it)
    pub boarding_cooldown: i64,
    /// (ironspec 21:15 (c)) the R-press row's positive control: Q lit by hand at the rider's
    /// RS_ready press, for the stretch's width
    pub light_q_at_rs: bool,
    /// (G1 12:38: the merge row) A hurt this many gt after the lie-down (a reverse merge at <= 3).
    pub hurt_after_lie: Option<i64>,
    /// (ironspec 02:16, row 39's fault record) an EMPTY cart planted at this stop-local position at
    /// the visit's start (a cart left on the siding, the stop blocked); the visit runs 14,000 gt
    pub plant_empty: Option<[f64; 3]>,
    /// (vcompose 03:19) the header's `stand-up:` feet positions (stop-local, exact)
    pub stand_up_decl: Vec<[f64; 3]>,
    /// ((A), ironspec 00:37 (2b)) F waits for the arming detector's edge + this many gt (the
    /// worst-case braking interval + 3).
    pub arm_delay: i64,
    /// ((A)'s at-rest bar's positive control) The park released this many gt after the arming
    /// detector's edge, whatever the napper does.
    pub release_after_arm: Option<i64>,
    /// (the farm-stop bar's known positive) The napper's HOME erased at this tick.
    pub erase_home_at: Option<i64>,
    /// (ironspec 23:00 (3)) the no-HOME row: HOME erased K gt after the wake, the ticket taken
    pub erase_home_on_p: Option<i64>,
    /// (G1 00:02) the no-HOME row's ticket state: "taken" keeps a stand-in claimant; anything else
    /// leaves it free (the real erasure path)
    pub erase_home_ticket: String,
    /// (tick-exact 00:07, AK's state) the day time set at the no-HOME erase (6000: IDLE by day)
    pub erase_home_day_time: Option<i64>,
    /// (`--latch-b`) RLS_IN's width in gt (the AND's output, held from the release)
    pub latch_rls_width: i64,
    /// (G1 04:08a item 5) on the laid latch, the next visit's RLS at this tick (its ARM before it by
    /// this visit's RLS - ARM gap), so Q's own tap line launches the held cart; `--tap` is refused there
    pub latch_next_rls: Option<i64>,
    /// (day-1 04:40 / G1 04:42 (1)) RS_ready's d_RS, read from the design file: the delay the file
    /// builds between the RS string's hooks and their output cells (the bench refuses a file that lays
    /// a delay element there until it models it); 0 on every file laid today
    pub d_rs: i64,
    /// (ironspec 16:35: the self-heal) the napper's HOME released at this tick of each visit (the
    /// stamp release: the memory erased, a fresh stamp, the bed's ticket free).
    pub release_home_at: Option<i64>,
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

fn block(s: &str) -> Result<Block, String> {
    // (G1 10:13: a state that doesn't parse is a named error, never a panic)
    Block::parse(&format!("minecraft:{s}")).map_err(|e| format!("the block state {s:?} doesn't parse: {e}"))
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

/// (G1's Q13 on 94e90d6b2: the `# integrity:` key judged, not a label) The design's declared rail
/// shapes: `holds X,Y,Z=SHAPE` (that shape on every tick) and `X,Y,Z=connections A+B` (the rail at X
/// joining the cells A and B: the shape their two directions make), strictly parsed (bench = file -
/// shift).
pub fn parse_integrity(text: &str, shift: [i32; 3]) -> Result<Vec<([i32; 3], String)>, String> {
    let cell = |s: &str, d: &str| -> Result<[i32; 3], String> {
        let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("integrity {d:?}: a cell x,y,z ({e})"))?;
        match v.as_slice() {
            &[x, y, z] => Ok([x - shift[0], y - shift[1], z - shift[2]]),
            _ => Err(format!("integrity {d:?}: a cell x,y,z")),
        }
    };
    let mut out = Vec::new();
    // (the entries split on ';' here: `holds ...` entries don't start with a cell list, so the
    // shared header_entries would merge them into the entry before)
    let entries: Vec<String> = text.lines().filter_map(|l| l.trim_start().strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix("integrity:"))).flat_map(|b| b.split(';').map(|s| s.trim().to_string()).collect::<Vec<_>>()).filter(|s| !s.is_empty()).collect();
    for d in entries {
        if let Some(rest) = d.strip_prefix("holds ") {
            let (c, shape) = rest.split_once('=').ok_or(format!("integrity {d:?}: holds x,y,z=SHAPE"))?;
            let shape = shape.trim();
            if !["north_south", "east_west", "north_east", "north_west", "south_east", "south_west", "ascending_north", "ascending_south", "ascending_east", "ascending_west"].contains(&shape) {
                return Err(format!("integrity {d:?}: {shape:?} is no rail shape"));
            }
            out.push((cell(c, &d)?, shape.to_string()));
        } else if let Some((c, rest)) = d.split_once("=connections ") {
            let x = cell(c, &d)?;
            let (a, b) = rest.split_once('+').ok_or(format!("integrity {d:?}: x,y,z=connections A+B"))?;
            let (a, b) = (cell(a, &d)?, cell(b, &d)?);
            let dir = |p: [i32; 3]| -> Result<&'static str, String> {
                match (p[0] - x[0], p[2] - x[2]) {
                    (0, -1) => Ok("north"),
                    (0, 1) => Ok("south"),
                    (1, 0) => Ok("east"),
                    (-1, 0) => Ok("west"),
                    _ => Err(format!("integrity {d:?}: ({},{},{}) isn't beside ({},{},{})", p[0], p[1], p[2], x[0], x[1], x[2])),
                }
            };
            // (a connection one block up makes the rail ascend toward it: the park's (1,0,4) joins
            // (0,0,4) and the U-turn's (2,1,4), ascending_east)
            let up: Vec<[i32; 3]> = [a, b].into_iter().filter(|p| p[1] == x[1] + 1).collect();
            if let [u] = up.as_slice() {
                out.push((x, format!("ascending_{}", dir(*u)?)));
                continue;
            }
            let mut ds = [dir(a)?, dir(b)?];
            ds.sort_by_key(|s| ["north", "south", "east", "west"].iter().position(|t| t == s));
            let shape = match ds {
                ["north", "south"] => "north_south",
                ["east", "west"] => "east_west",
                ["north", "east"] => "north_east",
                ["north", "west"] => "north_west",
                ["south", "east"] => "south_east",
                ["south", "west"] => "south_west",
                _ => return Err(format!("integrity {d:?}: two different directions")),
            };
            out.push((x, shape.to_string()));
        } else {
            return Err(format!("integrity {d:?}: `holds x,y,z=SHAPE` or `x,y,z=connections A+B`"));
        }
    }
    Ok(out)
}

/// (G1 12:38 / ironspec 12:38-12:58) DAY-1's guard-free latch as the bench emulates it: TIMEOUT
/// at ARM + `t_max`; the shutter `logic_lat` gt behind OPEN (its torch); the park's release
/// `rel_after` gt after OPEN's fall (>= logic_lat + 4: the first (-2,0,5) entry >= close + 27).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Latch {
    pub t_max: i64,
    pub logic_lat: i64,
    pub rel_after: i64,
}

/// (G1's B1 on 1662bed98; DAY-1's gate list rev 20:00 (artifact 1608254923947368), its stop block of
/// record G16-G22 with G1's B2 onset (day-1 17:35)) DAY-1's adopted stop controller, emulated on the
/// real cells. G22 P_STRING (a NAMED STAND-IN: the awake napper's box meeting the band of the string
/// at P + 1, (-2,1,4), released `lag` gt after it last meets it). G16 SINCE: set by an observer pulse
/// with P_STRING on at its head change (the pulse's rise - 2) and the press begun within `onset` gt
/// of that change (None: no onset term); reset on NOT P_STRING. G17 WIN: from the latest pulse + `d`
/// (RLS's sample: the D line 8 + the latch and AND overhead) until a new pulse or the pulse + `w`.
/// G18 PARKED: ARM's edge + `parked` (a timer; 0: the stand-in "the cart at rest in the park cell");
/// A70: ARM's edge + `a70` AND PARKED; both reset at RLS. G19 SO: set by any pulse, reset at RLS.
/// G20 OPEN: S = A70 and SO and not P_STRING, R = RLS or P_STRING; the shutter NOT OPEN through a
/// torch (`open_set` / `open_reset` gt of logic, + `torch`). G21 RLS = P_STRING and SINCE and WIN and
/// PARKED, `rls` gt of logic after; one pulse a visit; the park released at RLS + `park`. Defaults:
/// ctl-shutter's designed timing (17:24: the close at the wake + 8, RLS + 20, the park + 22).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Adopted {
    pub parked: i64,
    pub a70: i64,
    pub d: i64,
    pub w: i64,
    pub open_set: i64,
    pub open_reset: i64,
    pub torch: i64,
    pub rls: i64,
    pub park: i64,
    pub lag: i64,
    pub onset: Option<i64>,
}

impl Default for Adopted {
    fn default() -> Self {
        // (ironspec 17:32 (4): OPEN's set uses PARKED in place of A70 (a70 0); 17:53: p = 2 stands,
        // COOL's tap ARM + 52; 17:39: PARKED sensed, ANDed with COOL (the `parked` term))
        Adopted { parked: 52, a70: 0, d: 12, w: 54, open_set: 6, open_reset: 6, torch: 2, rls: 6, park: 2, lag: 10, onset: Some(1) }
    }
}

/// (G1's Q15 on 1662bed98) The design's `# a6-pickup:` stranded bound: "... by the wake edge + N gt
/// fails"; None when the key is absent; an `a6-pickup:` line with no readable bound is an error.
pub fn parse_stranded_bound(text: &str) -> Result<Option<i64>, String> {
    for l in text.lines() {
        let Some(body) = l.trim_start().strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix("a6-pickup:")) else { continue };
        let n = body.split_once("by the wake edge + ").and_then(|(_, r)| r.split_once(" gt")).and_then(|(n, _)| n.trim().parse::<i64>().ok());
        return match n {
            Some(n) if n > 0 => Ok(Some(n)),
            _ => Err(format!("a6-pickup {body:?}: its stranded bound reads \"by the wake edge + N gt\"")),
        };
    }
    Ok(None)
}

/// (G1 03:46 B2) The role of a `--plant-empty` cell on the re-laid stop: the siding's two plain rails
/// (-1,0,3) / (0,0,3) or the park (`park`); any other cell is refused by name.
pub fn plant_empty_role(cell: [i32; 3], park: [i32; 3]) -> Result<&'static str, String> {
    if cell == [-1, 0, 3] || cell == [0, 0, 3] {
        Ok("the siding")
    } else if cell == park {
        Ok("the park")
    } else {
        Err(format!("--plant-empty {cell:?}: not a siding or park rail (the siding (-1,0,3) / (0,0,3), the park {park:?}; G1 03:46 B2)"))
    }
}

/// (vcompose 03:19) The header's `# stand-up: x,y,z=WHY[; x,y,z=WHY]` feet positions (decimals, the
/// file's own frame).
pub fn parse_stand_up(text: &str) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for l in text.lines() {
        let Some(body) = l.trim_start().strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix("stand-up:")) else { continue };
        for e in body.split("; ") {
            if let Some((c, _)) = e.split_once('=') {
                let v: Vec<f64> = c.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).collect();
                if v.len() == 3 {
                    out.push([v[0], v[1], v[2]]);
                }
            }
        }
    }
    out
}

/// (G1's B3 on 1662bed98) A contact decided by the cart's entry speed, not by chance: the design's
/// `# contact-speeds: CELLS=meets V,V,...; of A..B/S` lines (one per declaration; `of` is the
/// reference band: a speed in it and not in `meets` must never meet the contact, a speed in `meets`
/// must meet it on every seed, a speed outside the band isn't judged); strict (bench = file - shift).
pub fn parse_contact_speeds(text: &str, shift: [i32; 3]) -> Result<Vec<(Vec<BlockPos>, Vec<f64>, Vec<f64>)>, String> {
    let mut out = Vec::new();
    for l in text.lines() {
        let Some(body) = l.trim_start().strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix("contact-speeds:")) else { continue };
        let (cells, rest) = body.split_once('=').ok_or_else(|| format!("contact-speeds {body:?}: CELLS=meets V,...; of A..B/S"))?;
        let mut cs = Vec::new();
        for c in cells.split('+') {
            let v: Vec<i32> = c.trim().split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("contact-speeds {body:?}: a cell x,y,z ({e})"))?;
            match v.as_slice() {
                &[x, y, z] => cs.push(BlockPos::new(x - shift[0], y - shift[1], z - shift[2])),
                _ => return Err(format!("contact-speeds {body:?}: a cell x,y,z")),
            }
        }
        let (m, o) = rest.split_once(';').ok_or_else(|| format!("contact-speeds {body:?}: meets V,...; of A..B/S"))?;
        let m = m.trim().strip_prefix("meets ").ok_or_else(|| format!("contact-speeds {body:?}: `meets V,...` first"))?;
        let o = o.trim().strip_prefix("of ").ok_or_else(|| format!("contact-speeds {body:?}: `; of A..B/S` second"))?;
        let meets: Vec<f64> = list(Some(m.trim())).map_err(|e| format!("contact-speeds {body:?}: meets: {e}"))?;
        let of: Vec<f64> = list(Some(o.trim())).map_err(|e| format!("contact-speeds {body:?}: of: {e}"))?;
        if meets.iter().any(|v| !of.iter().any(|b| (b - v).abs() < 1e-9)) {
            return Err(format!("contact-speeds {body:?}: a `meets` speed outside the `of` band"));
        }
        out.push((cs, meets, of));
    }
    Ok(out)
}

/// (ironspec Nov 13 10:42 (3) / 10:58) The design's bounded contact declarations, parsed by main's
/// ONE reader (`contacts::declared_contacts_bounded`: strict, the design class's every bound with
/// its margin), their cells shifted into the bench's frame (bench = file - shift).
pub fn bounded_decls(text: &str, shift: [i32; 3]) -> Result<Vec<crate::contacts::Decl>, String> {
    let mut ds = crate::contacts::declared_contacts_bounded(text)?;
    for d in &mut ds {
        for c in &mut d.cells {
            *c = BlockPos::new(c.x - shift[0], c.y - shift[1], c.z - shift[2]);
        }
    }
    Ok(ds)
}

/// One declaration's measured extremes over `hits` (the judge's events: maximal runs of consecutive
/// ticks per cart and face on its cells): the events, the samples met, the longest event's ticks, the
/// most events of one cart, the deepest tick, the speed in at an event's first tick and out at its
/// last (least..largest): what a `ref=` run states as the declaration's values.
pub fn decl_extremes(d: &crate::contacts::Decl, hits: &[crate::contacts::Hit]) -> String {
    let mut groups: std::collections::BTreeMap<(u32, &'static str), Vec<&crate::contacts::Hit>> = std::collections::BTreeMap::new();
    for h in hits.iter().filter(|h| d.cells.contains(&h.cell) && d.face.is_none_or(|f| f == h.face)) {
        groups.entry((h.cart, h.face.name())).or_default().push(h);
    }
    let samples: std::collections::BTreeSet<u64> = groups.values().flatten().filter_map(|h| h.sample).collect();
    let (mut events, mut max_ticks, mut max_depth) = (0usize, 0i64, 0.0f64);
    let mut per_cart: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    let (mut sin, mut sout): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for ((cart, _), hs) in &mut groups {
        hs.sort_by_key(|h| h.tick);
        let mut runs: Vec<Vec<&crate::contacts::Hit>> = Vec::new();
        for h in hs.iter() {
            match runs.last_mut() {
                Some(r) if r.last().is_some_and(|l| h.tick <= l.tick + 1) => r.push(h),
                _ => runs.push(vec![h]),
            }
        }
        for r in runs {
            events += 1;
            max_ticks = max_ticks.max(r[r.len() - 1].tick - r[0].tick + 1);
            max_depth = max_depth.max(r.iter().map(|h| h.depth).fold(0.0, f64::max));
            sin.push(r[0].speed_in);
            sout.push(r[r.len() - 1].speed_out);
            *per_cart.entry(*cart).or_default() += 1;
        }
    }
    let range = |v: &[f64]| if v.is_empty() { "none".to_string() } else { format!("{:.4}..{:.4}", v.iter().copied().fold(f64::MAX, f64::min), v.iter().copied().fold(f64::MIN, f64::max)) };
    format!("{events} events over {} samples; the longest {max_ticks} ticks; at most {} per cart; the deepest {max_depth:.4}; speed_in {}; speed_out {}", samples.len(), per_cart.values().copied().max().unwrap_or(0), range(&sin), range(&sout))
}

/// A controller cell set to its on ("redstone_block") or off ("stone") state the way the game
/// moves a lever: `LeverBlock.pull` (the lever's own neighbours and its attached block's
/// neighbours updated, as a repeater line end updates its front block's), not a raw block write
/// (whose updates reach only the lever's neighbours: a component powered through the attached
/// conductor never heard the change). A cell that isn't a lever is written raw. Returns whether the
/// cell changed (G1's Q12: an expected edge that writes nothing is the caller's failure).
pub fn ctl_set(w: &mut World, c: [i32; 3], st: &str) -> bool {
    ctl_set_at(w, c, [0, 0, 0], st)
}

/// [`ctl_set`] on a world built at `off` (the bench's `--offset`): `c` in the design frame (its kind
/// read from the design cell), written at c + off.
pub fn ctl_set_at(w: &mut World, c: [i32; 3], off: [i32; 3], st: &str) -> bool {
    let p = BlockPos::new(c[0] + off[0], c[1] + off[1], c[2] + off[2]);
    let want = match block(&ctl_block(c, st)) {
        Ok(b) => b,
        Err(e) => {
            // (a named error the row reports from the world's errors, never a panic)
            w.errors.push(e);
            return false;
        }
    };
    let cur = w.get(p);
    if cur == want {
        return false;
    }
    if matches!(cur, Block::Lever { .. }) && matches!(want, Block::Lever { .. }) && redsim::redstone::attach::attached::with_powered(cur, !redsim::redstone::attach::attached::powered(&cur)) == want {
        redsim::redstone::attach::lever::pull(w, cur, p);
    } else {
        w.set_block(p, want, redsim::world::flags::ALL);
    }
    true
}

/// A declared lever's powered state set (its block kept, only `powered` written); the cell must
/// hold a lever.
pub fn set_lever(w: &mut World, p: BlockPos, on: bool) -> Result<bool, String> {
    let s = format!("{}", w.get(p));
    if !s.contains("lever[") {
        return Err(format!("({},{},{}) holds {s}, not a declared lever", p.x, p.y, p.z));
    }
    let t = if on { s.replace("powered=false", "powered=true") } else { s.replace("powered=true", "powered=false") };
    if t != s {
        // (the lever moved as the game moves it, `LeverBlock.pull`: its attached block's
        // neighbours updated too)
        let cur = w.get(p);
        redsim::redstone::attach::lever::pull(w, cur, p);
        let after = format!("{}", w.get(p));
        if after != t {
            return Err(format!("({},{},{}) pulled to {after}, not {t}", p.x, p.y, p.z));
        }
        return Ok(true);
    }
    // (G1's C14: a no-op is reported to the caller, which refuses it where an edge is expected)
    Ok(false)
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
pub fn standin_reach_check(w: &World, p: BlockPos, b: &Block, targets: &[BlockPos], declared: &[[i32; 3]], off: [i32; 3]) -> Result<String, String> {
    // (`declared` in the world's frame; `off` only moves the printed cells back to the design frame
    // (the bench's `--offset`))
    let (update, power) = standin_reach(w, p, b)?;
    let dec = |q: &BlockPos| declared.contains(&[q.x, q.y, q.z]);
    let d = |q: &BlockPos| (q.x - off[0], q.y - off[1], q.z - off[2]);
    if let Some(t) = update.iter().find(|q| rail_t_at(w, **q) && !dec(q)) {
        let (pp, tt) = (d(&p), d(t));
        return Err(format!("refused: the stand-in {b} at ({},{},{}) updates the rail T at ({},{},{}) (RailBlock.updateState); declare it state-stable", pp.0, pp.1, pp.2, tt.0, tt.1, tt.2));
    }
    let sensitive = |q: &BlockPos| {
        let s = format!("{}", w.get(*q));
        ["rail", "repeater", "comparator", "piston", "lamp", "door", "trapdoor", "dispenser", "dropper", "hopper", "observer", "redstone_wire", "torch", "note_block", "lever", "bell", "tnt"].iter().any(|k| s.contains(k))
    };
    if let Some(q) = power.iter().find(|q| sensitive(q) && !targets.contains(q) && !dec(q)) {
        let (pp, qq) = (d(&p), d(q));
        return Err(format!("refused: the stand-in {b} at ({},{},{}) powers {} at ({},{},{}), outside the built input's reach; declare it", pp.0, pp.1, pp.2, w.get(*q), qq.0, qq.1, qq.2));
    }
    let powered: Vec<String> = power.iter().filter(|q| !w.get(**q).is_air()).map(|q| { let qq = d(q); format!("({},{},{}) {}", qq.0, qq.1, qq.2, w.get(*q)) }).collect();
    let pp = d(&p);
    Ok(format!("the stand-in {b} at ({},{},{}): its updates reach {} cells, no undeclared rail T; it powers {}", pp.0, pp.1, pp.2, update.len(), if powered.is_empty() { "no block".to_string() } else { powered.join(", ") }))
}

pub fn ctl_block(c: [i32; 3], st: &str) -> String {
    let on = match st {
        "redstone_block" => true,
        "stone" => false,
        other => return other.to_string(),
    };
    if !([HATCH_CTRL, HATCH_CTRL_HI, ACT_CTRL, ACT_CTRL_T5B, ACT_CTRL_A, PARK_CTRL_A, WAKER_CTRL, TAP_CTRL].contains(&c) || PARK_CTRL.contains(&c)) {
        // ((A) with the run-up: the activator's lever hung from the glass over E, the park's on
        // the glass south of it)
        if c == ACT_CTRL_A2 {
            return "redstone_torch[lit=true]".to_string();
        }
        if c == PARK_CTRL_A2 {
            return format!("lever[face=ceiling,facing=east,powered={on}]");
        }
        if c == K_CTRL {
            return format!("lever[face=ceiling,facing=north,powered={on}]");
        }
        // (the re-laid stop's park drive stand-in: a floor lever at the drive repeater's input)
        if c[1] == 0 && c[2] == 1 && (2..=4).contains(&c[0]) {
            return format!("lever[face=floor,facing=north,powered={on}]");
        }
        // (`--latch-b`: the logic block's inputs into the laid latch, floor levers in the bench)
        if c == LATCH_RLS_IN || c == LATCH_ARM_IN {
            return format!("lever[face=floor,facing=north,powered={on}]");
        }
        return st.to_string();
    }
    format!("lever[face=floor,facing=north,powered={on}]")
}

/// The direction of a villager's current path from the pickup cell: its first node off the cell
/// (north is up the approach, toward the arriving cart).
fn path_dir(w: &World, i: usize) -> Option<&'static str> {
    path_dir_at(w, i, [0, 0, 0])
}

/// [`path_dir`] on a world built at `off` (the bench's `--offset`): the path's nodes read back in
/// the design frame.
fn path_dir_at(w: &World, i: usize, off: [i32; 3]) -> Option<&'static str> {
    use redsim::entity::mob::ai::brain;
    if !brain::has_exact(w, i) {
        return None;
    }
    let p = brain::ex(w, i).mem.path.as_ref()?;
    let n = p.nodes.iter().skip(p.next_node_index).map(|n| (n.x - off[0], n.z - off[2])).find(|n| *n != (SWEPT[0][0], SWEPT[0][2]))?;
    Some(if n.1 < SWEPT[0][2] {
        "north"
    } else if n.1 > SWEPT[0][2] {
        "south"
    } else if n.0 > SWEPT[0][0] {
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
    // (ironspec 11:39 / G1: the build positions: `--offset` builds the whole design at design + off
    // (every laid cell, entity and stand-in write) and reads every design-frame cell there, the rows
    // printed back in the design frame; the frame-dependent motion is the game's)
    let off = f.offset;
    // (the offset is the stop's own bench: no module, queue, record or line-bench levers with it)
    if off != [0, 0, 0] && (f.approach.is_some() || f.record.is_some() || !f.lay_emulated.is_empty() || f.built_release.is_some() || !f.levers_on.is_empty() || f.day_lever.is_some()) {
        return Err("--offset runs the stop's own bench only (no --module-offset, --record, line-bench levers or emulated module cells with it)".into());
    }
    let shifted;
    let spec = if off != [0, 0, 0] {
        shifted = shift_spec(spec, off);
        &shifted
    } else {
        spec
    };
    let wp = |x: i32, y: i32, z: i32| BlockPos::new(x + off[0], y + off[1], z + off[2]);
    let wc = |c: [i32; 3]| BlockPos::new(c[0] + off[0], c[1] + off[1], c[2] + off[2]);
    let dcell = |p: BlockPos| [p.x - off[0], p.y - off[1], p.z - off[2]];
    let dbp = |p: BlockPos| BlockPos::new(p.x - off[0], p.y - off[1], p.z - off[2]);
    let dpos = |v: Vec3| Vec3::new(v.x - off[0] as f64, v.y - off[1] as f64, v.z - off[2] as f64);
    let (ox, oy, oz) = (off[0] as f64, off[1] as f64, off[2] as f64);
    // (the declared emulated cells moved to the world's frame, for the guard's comparisons)
    let declared_w: Vec<[i32; 3]> = f.declared_emulated.iter().map(|c| [c[0] + off[0], c[1] + off[1], c[2] + off[2]]).collect();
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
    // ((A)'s exit-slope hold present: the slope (1,-1,5) a powered rail (ironspec 09:38))
    let hold_slope_on = format!("{}", run.world.get(wp(1, -1, 5))).contains("powered_rail");
    // (ironspec 15:23: the rider-keyed RS_ready's string present (the design's --rs-ready))
    let rs_present = format!("{}", run.world.get(wc(RS_STRING))).contains("tripwire[");
    // (G1 15:29 / b8949c96 15:45: the top-half trapdoors, for the cart states near them)
    let trapdoors: Vec<[i32; 3]> = if f.cart_states.is_some() {
        let mut v = Vec::new();
        for x in -8..=10 {
            for y in -4..=5 {
                for z in -7..=12 {
                    let s = format!("{}", run.world.get(wp(x, y, z)));
                    if s.contains("iron_trapdoor") && s.contains("half=top") {
                        v.push([x, y, z]);
                    }
                }
            }
        }
        v
    } else {
        Vec::new()
    };
    // (Z1's design: the waker's shutter at WAKER_SHUT (an iron trapdoor))
    let z1_design = format!("{}", run.world.get(wc(WAKER_SHUT))).contains("iron_trapdoor");
    // (G1's Q12: no silent no-op: Z1 shuts its window on the wake edge as its normal operation)
    if f.hide_after_wake && z1_design {
        return Err("--hide-after-wake on Z1's design is a no-op (Z1 shuts the window on the wake edge as its normal operation; G1's Q12)".into());
    }
    let mut row = Row { seed, speed, fluid_scheduled, ..Default::default() };
    // (G1's N9: natural spawning OFF by construction: the bench never sets it)
    if run.world.mob.natural.is_some() {
        return Err("natural spawning is on in the bench's world: the bench's scope says off (ironspec 14:57)".into());
    }
    // (9fc3155f 23:21: the bed sensor may sit west of the head ((-4,0,4) facing east), so neither
    // bed half stands on a solid-render block; the bench reads whichever cell holds the observer)
    let sensor_cell = if format!("{}", run.world.get(wc(WAKE_SENSOR_WEST))).contains("observer") { WAKE_SENSOR_WEST } else { WAKE_SENSOR };
    row.sensor_present = format!("{}", run.world.get(wc(sensor_cell))).contains("observer");
    if f.latch.is_some() && !row.sensor_present {
        return Err("--latch runs on the real wake sensor: a design with the observer (bedtier --wake-sensor)".into());
    }
    if f.adopted.is_some() && !row.sensor_present {
        return Err("the adopted controller (the default) keys SO on the real wake sensor: a design with the observer (bedtier --wake-sensor), or --controller stand-in".into());
    }
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
    // the visitor at the entry, HOME = the bed (as the game loads the memory; at the build position)
    let h = [BED_HEAD[0] + off[0], BED_HEAD[1] + off[1], BED_HEAD[2] + off[2]];
    let nbt = if f.empty {
        "{Rotation:[0f,0f]}".to_string()
    } else if f.no_home {
        "{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}".to_string()
    } else {
        format!("{{Rotation:[0f,0f],Passengers:[{{id:\"minecraft:villager\",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}", h[0], h[1], h[2])
    };
    let start = f.approach.map_or([ENTRY[0] as f64 + 0.5 + ox, ENTRY[1] as f64 + 0.0625 + oy, ENTRY[2] as f64 + 0.5 + oz], |a| a.seat);
    let mut cart_id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", start, Some(&nbt))?;
    let mut vill = run.world.entities.list.iter().find(|e| e.id == cart_id).and_then(|e| e.passengers.first().copied());
    if vill.is_none() && !f.empty {
        return Err("no rider".into());
    }
    run.own.push(cart_id);
    run.own.extend(vill);
    // (ironspec 02:16, row 39's fault record: an empty cart left on the siding before the visit;
    // G1 03:46 B2: planted only on a siding or park rail, its role printed and asserted)
    let planted = match f.plant_empty {
        Some(p) => {
            let cell = [p[0].floor() as i32, (p[1] + 0.01).floor() as i32, p[2].floor() as i32];
            let role = plant_empty_role(cell, layout_at(&run.world, off).park)?;
            let at = [p[0] + ox, p[1] + oy, p[2] + oz];
            let blk = format!("{}", run.world.get(wc(cell)));
            if !blk.contains("rail") {
                return Err(format!("--plant-empty {cell:?}: {role} holds {blk}, not a rail (G1 03:46 B2)"));
            }
            let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", at, Some("{Rotation:[0f,0f]}"))?;
            run.own.push(id);
            row.latch_lines.push(format!("t 0: an empty cart planted at ({:.2},{:.2},{:.2}) on {role} {cell:?} ({blk})", p[0], p[1], p[2]));
            Some(id)
        }
        None => None,
    };
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
    let set = |w: &mut World, c: [i32; 3], st: &str| ctl_set_at(w, c, off, st);
    // (G1 13:02: the P-eye static: from every eye position of a napper standing awake on P
    // (-2,0,4) (feet on its floor, a 5 x 5 grid over the cell), Z1 in view (within 8, eye to eye)
    // with the window open (the shutter unpowered, on a copy) and shut (as the night holds it); the
    // record seed's visits only)
    if z1_design
        && seed == f.record_seed
        && let (Some(zi), Some(vi)) = (zid.and_then(|z| idx(&run.world, z)), vill.and_then(|v| idx(&run.world, v)))
    {
        let w = &run.world;
        let z = &w.entities.list[zi];
        let ze = Vec3::new(z.pos.x, z.pos.y + z.kind.eye_height() as f64, z.pos.z);
        let eh = w.entities.list[vi].kind.eye_height() as f64;
        let mut open_w = w.clone();
        let sh = wc(WAKER_SHUT);
        let open_state = format!("{}", open_w.get(sh)).replace("open=true", "open=false").replace("powered=true", "powered=false");
        open_w.set_block(sh, block(open_state.trim_start_matches("minecraft:"))?, 0);
        let shut_w = {
            let mut s = w.clone();
            let st = format!("{}", s.get(sh)).replace("open=false", "open=true").replace("powered=false", "powered=true");
            s.set_block(sh, block(st.trim_start_matches("minecraft:"))?, 0);
            s
        };
        let (mut clear_open, mut clear_shut, mut n, mut dmin, mut dmax) = (0usize, 0usize, 0usize, f64::MAX, 0f64);
        let mut blocked_open: std::collections::BTreeMap<String, usize> = Default::default();
        for xi in 0..5 {
            for zi2 in 0..5 {
                let ex = SWEPT[0][0] as f64 + 0.1 + 0.2 * xi as f64 + ox;
                let ez = SWEPT[0][2] as f64 + 0.1 + 0.2 * zi2 as f64 + oz;
                let ve = Vec3::new(ex, SWEPT[0][1] as f64 + oy + eh, ez);
                let d = ve.distance_to_sqr(ze).sqrt();
                n += 1;
                dmin = dmin.min(d);
                dmax = dmax.max(d);
                let ho = redsim::entity::mob::los::first_hit(&open_w, ve, ze);
                let hs = redsim::entity::mob::los::first_hit(&shut_w, ve, ze);
                // (G1's Q15: 8 = VillagerHostilesSensor's zombie distance, the range NEAREST_HOSTILE
                // is set within, with a clear eye-to-eye line)
                if ho.is_none() && d <= 8.0 {
                    clear_open += 1;
                }
                if hs.is_none() && d <= 8.0 {
                    clear_shut += 1;
                }
                if let Some(p) = ho {
                    let q = dbp(p);
                    *blocked_open.entry(format!("({},{},{}) {}", q.x, q.y, q.z, open_w.get(p))).or_default() += 1;
                }
            }
        }
        // (G1's N4: the P-eye static is a bar: Z1 in view from P in either window state fails)
        if clear_open > 0 || clear_shut > 0 {
            row.problems.push(format!("the P-eye static: Z1 in view (within 8, a clear eye-to-eye line) from {clear_open} of {n} eye positions on P with the window open, {clear_shut} with it shut (bar 0)"));
        }
        row.p_eye_lines.push(format!("the P-eye static (G1 13:02: a napper standing awake on P (-2,0,4), its eye {eh:.2} over P's floor, {n} positions over the cell; Z1's eye ({:.2},{:.2},{:.2})): {dmin:.2}..{dmax:.2} away; Z1 in view (within 8, eye to eye) with the window OPEN from {clear_open} of {n}, with it SHUT from {clear_shut} of {n}; the open window's blocking cells (positions): {blocked_open:?}", ze.x - ox, ze.y - oy, ze.z - oz));
    }
    // (ironspec 17:46, the stand-in guard: every emulated write this bench makes, checked against
    // the built cell's golem and path properties before the run: the controller's levers, and the
    // queue's release with --module-offset)
    let (act_cell, act_ctrl) = { let l = layout_at(&run.world, off); (l.act, l.act_ctrl) };
    let lay = layout_at(&run.world, off);
    row.park_cell = lay.park;
    row.layout_name = lay.name;
    row.ctl_cells = lay.hatch_ctrl.iter().chain(lay.park_ctrl.iter()).chain(lay.k_ctrl.iter()).copied().collect();
    row.latch_mode = lay.latch;
    // (G1 04:08a items 4 / 5: on the laid latch the stand-in PARK stretch and the `--tap` lever are
    // refused: the lever's cell (1,-2,3) is the latch's own tap wire, and the latch times PARK itself;
    // the hold launches on Q's own tap, the next visit's RLS at `--latch-next-rls`)
    // (G1 04:42 (1): d_RS from the file: the RS string's hooks power their back blocks at the press
    // tick, and no delay element follows them in the file: a repeater or comparator beside either
    // back block would be one (refused until the bench models it))
    if format!("{}", run.world.get(wc(RS_HOOK_N))).contains("tripwire_hook") {
        for back in [RS_BACK_N, RS_BACK_S] {
            for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let c = [back[0] + d[0], back[1] + d[1], back[2] + d[2]];
                let b = format!("{}", run.world.get(wc(c)));
                if b.contains("repeater") || b.contains("comparator") {
                    return Err(format!("the file lays a delay element {b} at {c:?} on RS_ready's output (beside {back:?}): d_RS isn't 0, and the bench doesn't model it yet (G1 04:42 (1))"));
                }
            }
        }
    }
    if lay.latch {
        if f.tap.is_some() {
            return Err("--tap on the laid latch: refused (its stand-in lever's cell (1,-2,3) is the latch's own tap wire, so Q's tap would never be exercised; the hold launches on Q's tap at --latch-next-rls T; G1 04:08a item 5)".into());
        }
        if f.park_width.is_some() {
            return Err("--park-width on the laid latch: refused (the stand-in PARK's stretch; the laid latch times PARK itself; G1 04:08a item 4)".into());
        }
    } else if f.latch_next_rls.is_some() {
        return Err("--latch-next-rls: only on the laid latch (--latch-b's files)".into());
    }
    // (G1 04:08a item 13: controls that don't isolate their change are refused where they'd mislead)
    if lay.park_string.is_some() && f.block_p_at_wake {
        return Err("--block-p-after-lie on the re-laid stop: refused (its block over P at (-2,1,4) would replace P_STRING, not a single-change twin; --k-stuck north_east is the re-laid stop's no-pickup control; G1 04:08a item 13)".into());
    }
    if f.hide_after_wake && (f.adopted.is_some() || f.latch.is_some()) {
        return Err("--hide-after-wake under the adopted controller: refused (it does nothing there; G1 04:08a item 13)".into());
    }
    // (G1's Q2: the slope hold's tap stand-in, the input lever into DAY-1's repeater, through the
    // same guard; its reach must be the repeater alone)
    let tap_built = format!("{}", run.world.get(wc(TAP_REP))).contains("repeater");
    if f.tap.is_some() && !tap_built {
        return Err("--tap: no repeater at (1,-2,4) (the tap's built path is the bench file's: `bedtier --bench --hold-slope`)".into());
    }
    let tap_ctl: Vec<[i32; 3]> = if tap_built && !lay.latch { vec![TAP_CTRL] } else { Vec::new() };
    // (G1 04:08a item 6: the re-laid stop's stand-ins through the same guard: K's drive lever and the
    // laid latch's two input levers, each against its built reach)
    let new_ctl: Vec<[i32; 3]> = lay.k_ctrl.iter().copied().chain(if lay.latch { vec![LATCH_RLS_IN, LATCH_ARM_IN] } else { Vec::new() }).collect();
    for c in lay.hatch_ctrl.iter().copied().chain([act_ctrl]).chain(lay.park_ctrl.iter().copied()).chain(tap_ctl.iter().copied()).chain(new_ctl.iter().copied()) {
        // (a built always-on source the bench never writes ((A) with the run-up's activator torch)
        // is no stand-in: nothing to check)
        if ctl_block(c, "redstone_block") == ctl_block(c, "stone") {
            continue;
        }
        for st in ["redstone_block", "stone"] {
            emulated_write_check(&run.world, wc(c), &block(&ctl_block(c, st))?, &declared_w)?;
        }
        // (the reach half: the built lever's own power reach is the target set; the tap's: DAY-1's
        // repeater alone)
        let p = wc(c);
        let on = block(&ctl_block(c, "redstone_block"))?;
        let built = if c == TAP_CTRL { vec![wc(TAP_REP)] } else { standin_reach(&run.world, p, &run.world.get(p))?.1 };
        // (K's drive lever: its update reaching K's T IS its job (K's shape follows its drive), so
        // K is its declared target, not an undeclared state-stable T; any other T it reached would
        // still refuse)
        let mut dec_c = declared_w.clone();
        if lay.k_ctrl == Some(c) {
            let k = wc([-2, 0, 3]);
            dec_c.push([k.x, k.y, k.z]);
        }
        row.standin.push(standin_reach_check(&run.world, p, &on, &built, &dec_c, off)?);
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
            emulated_write_check(&run.world, BlockPos::new(c[0], c[1], c[2]), &block(&rep)?, &f.declared_emulated)?;
        }
        emulated_write_check(&run.world, BlockPos::new(driver[0], driver[1], driver[2]), &block(&lev)?, &f.declared_emulated)?;
        // (G1's Q5: the glass under the release's cells through the same guard (its check was
        // dropped in 2862457ae))
        for c in cells.iter().chain([driver].iter()) {
            emulated_write_check(&run.world, BlockPos::new(c[0], c[1] - 1, c[2]), &block("glass")?, &f.declared_emulated)?;
        }
        for c in cells.iter().chain([driver].iter()) {
            run.world.set_block(BlockPos::new(c[0], c[1] - 1, c[2]), block("glass")?, redsim::world::flags::ALL);
        }
        for c in &cells {
            run.world.set_block(BlockPos::new(c[0], c[1], c[2]), block(&rep)?, redsim::world::flags::ALL);
        }
        run.world.set_block(BlockPos::new(driver[0], driver[1], driver[2]), block(&format!("lever[face=floor,facing={facing},powered=false]"))?, redsim::world::flags::ALL);
        // (the reach half: the driver's power reaches only the back repeater, each repeater only
        // the next one, the front one only the queue's rail; no undeclared rail T in their updates)
        let rail = BlockPos::new(a.seat[0].floor() as i32, a.seat[1].floor() as i32, a.seat[2].floor() as i32);
        let bp = |c: [i32; 3]| BlockPos::new(c[0], c[1], c[2]);
        row.standin.push(standin_reach_check(&run.world, bp(driver), &block(&lev)?, &[bp(cells[1])], &f.declared_emulated, [0, 0, 0])?);
        let rep_on = format!("repeater[delay=1,facing={facing},locked=false,powered=true]");
        row.standin.push(standin_reach_check(&run.world, bp(cells[1]), &block(&rep_on)?, &[bp(cells[0])], &f.declared_emulated, [0, 0, 0])?);
        row.standin.push(standin_reach_check(&run.world, bp(cells[0]), &block(&rep_on)?, &[rail], &f.declared_emulated, [0, 0, 0])?);
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
        let (before, after) = (run.world.get(p), block(st)?);
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
    // (the waker's consequence trace: the last printed walk target / path / stamp)
    let mut waker_last = String::new();
    // (ironspec 00:27b: the stamp's state on the previous tick, for its arming / clearing events)
    let mut stamp_prev_armed = false;
    let mut waker_act_last = String::new();
    let mut waker_stamp_at_wake: Option<Option<i64>> = None;
    let (mut prev_riding, mut prev_walk_target, mut prev_cant_reach) = (false, false, false);
    // (the cart's horizontal speed on the previous tick: a declared contact's "before")
    let mut prev_cart_speed = 0f64;
    // (the bed's head and foot states after the previous tick: the contact's bed check)
    let mut prev_bed: Option<(String, String)> = None;
    let mut sensor_prev = false;
    // (ironspec 17:32: the close read from the shutter's state; the boarding cooldown seen set)
    let (mut shutter_open_seen, mut close_by_state, mut cooldown_seen) = (false, false, false);
    let mut sense_prev = false;
    // (ironspec 21:15 (g): the bench's park drive on (its own release / power-up), PARK's stretch
    // end, the hand-lit Q control, and the park rail's last state for the RLS-low pulse count)
    let (mut drive_on, mut park_off_due, mut hand_q_due, mut hand_q_off, mut park_rail_prev): (bool, Option<i64>, Option<i64>, Option<i64>, bool) = (false, None, None, None, false);
    // (the re-laid stop: K's stand-in lever, stored on)
    let mut k_on = true;
    let mut prev_ccell = [i32::MIN; 3];
    // (`--latch-b`: RLS_IN's off tick)
    let mut latch_rls_off: Option<i64> = None;
    // (G1 04:08a item 5: the next visit's RLS_IN off, `--latch-next-rls`)
    let mut next_rls_off: Option<i64> = None;
    // (ironspec 23:54 (a): the napper's last on-foot box, y)
    let mut prev_foot_y: Option<(f64, f64)> = None;
    // (the laid latch's at-rest bar: the cart's 3D speed on the previous tick)
    let mut prev_cart_speed3: f64 = 0.0;
    // (G1's C1: the activator (-1,0,3)'s last power state; the cart's last passenger count)
    let (mut act_prev_powered, mut prev_pass_n) = (false, usize::MAX);
    let mut open_on_p_run = 0usize;
    let mut tap_on = false;
    let mut hold_entered = false;
    // (the emulated DAY-1 latch's state, `f.latch`)
    let (mut latch_seen, mut latch_slept, mut latch_once, mut latch_window_open) = (0usize, false, false, false);
    let (mut latch_open_at, mut latch_fall_at, mut release_due): (Option<i64>, Option<i64>, Option<i64>) = (None, None, None);
    // (DAY-1's adopted controller's state, `f.adopted` (rev 20:00): SO, SINCE, the latest observer
    // pulse (WIN's start), P_STRING's stand-in (on, its last meet, its press), OPEN, RLS, the park
    // due, the window as written)
    let (mut ad_seen, mut ad_so, mut ad_pulse): (usize, bool, Option<i64>) = (0, false, None);
    let mut ad_since = false;
    let (mut p_on, mut p_last_meet, mut p_press): (bool, Option<i64>, Option<i64>) = (false, None, None);
    let (mut ad_open, mut ad_released) = (false, false);
    // (ironspec 21:29: the Q-keyed consuming resets of PARKED and SO, at RLS + 6)
    let (mut ad_consumed, mut ad_consume_due): (bool, Option<i64>) = (false, None);
    let (mut ad_open_set_at, mut ad_open_reset_at, mut ad_rls_due, mut ad_park_due): (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = (None, None, None, None);
    let mut ad_window_open = false;
    let mut ad_armed_seen = false;
    let mut hurt_done = false;
    // (G1's Q8: each planted fault's application, checked at the visit's end)
    let (mut restart_done, mut erase_done, mut inject_done) = (false, false, false);
    // (G1's CHANGES 8 D3: a planted write's wake, stamped at the write's t - 1)
    let mut planted_wake: Option<i64> = None;
    let mut panic_done = false;
    let mut still = 0;
    let mut crossed: Vec<[i32; 3]> = Vec::new();
    let mut hidden = false;
    let mut live = 0i64;
    let mut f_live: Option<i64> = None;
    let mut f_fired = false;
    let mut had_home = !f.empty && !f.no_home;
    // (a skip or a set hour: a napper that misses REST waits on the stop until the next REST)
    let limit = if f.skip.is_some() || f.hour.is_some() || f.plant_empty.is_some() { 14_000 } else { 2400 } + f.approach.map_or(0, |a| a.wait + 200 + if f.skip.is_some() { 12_500 } else { 0 });
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
    // (the activator's initial state, not an edge: the file may already hold it, so an unchanged
    // write is expected here; every later edge is checked (G1's B4))
    set(&mut run.world, act_ctrl, if f.day_gate && day_state { "stone" } else { "redstone_block" });
    let mut trace_last_path = String::new();
    let mut prev_stamp: Option<i64> = None;
    let mut bolt_seen: std::collections::HashMap<[i32; 3], (bool, Option<[i32; 3]>)> = std::collections::HashMap::new();
    for t in 1..=limit {
        // (G1's C2 on bfa0c8098: P_STRING's stand-in read BEFORE any iteration-t write (a planted
        // hurt, a lever): the state after tick t - 1, so a press and the wake it follows share the
        // post-tick footing)
        let p_cell_state = format!("{}", run.world.get(BlockPos::new(off[0] - 2, off[1] + 1, off[2] + 4)));
        // (ironspec 21:30 (5) / day-1 21:33: a REAL P_STRING laid at (-2,1,4) (--set-cell's E-W
        // candidate) is read from the string itself, released when it unpowers; else the stand-in)
        let p_real = p_cell_state.starts_with("minecraft:tripwire[") || p_cell_state.starts_with("tripwire[");
        let p_meet_pre = f.adopted.is_some()
            && if p_real {
                p_cell_state.contains("powered=true")
            } else {
                vill.and_then(|v| idx(&run.world, v)).is_some_and(|vi| {
                    let bb = run.world.entities.list[vi].bb;
                    let (x0, y0, z0) = (bb.min_x - off[0] as f64, bb.min_y - off[1] as f64, bb.min_z - off[2] as f64);
                    let (x1, y1, z1) = (bb.max_x - off[0] as f64, bb.max_y - off[1] as f64, bb.max_z - off[2] as f64);
                    x1 > -2.0 && x0 < -1.0 && y1 > 1.0625 && y0 < 1.15625 && z1 > 4.0 && z0 < 5.0
                })
            };
        if p_real {
            row.p_string_real = true;
        }
        // (a night skip at the start of tick t: the day time to the next day time 0, game time on)
        if f.skip == Some(t) {
            let dt = run.world.day_time();
            run.world.mob.day_offset += 24000 - dt.rem_euclid(24000);
            row.skipped_at = Some(t);
        }
        // (a save, stop and start between ticks t - 1 and t: a world open's N idle ticks of game time with
        // nothing ticking; every entity back as a new object under a new id, in the row's load order)
        // (ironspec 21:20: a world open K gt after RLS, `--restart-at-rls K,HOW`: the redstone runs
        // ahead of the entities by the open's block-only ticks, so PARK's remaining window shrinks by
        // them as the entities see it)
        let restart_now = f.restart.filter(|(at, _)| t == *at).map(|(_, h)| h).or_else(|| f.restart_at_rls.and_then(|(k, h)| row.rls_at.filter(|r| t == r + k + 1).map(|_| h)));
        if let Some(how) = restart_now {
            crate::liveness::restart(&mut run.world, how)?;
            restart_done = true;
            let lead = match how {
                crate::liveness::Restart::Load { idle, .. } => idle,
                crate::liveness::Restart::Stress { extra, .. } => 1 + extra,
            };
            if let Some(due) = park_off_due
                && due > t
            {
                park_off_due = Some((due - lead).max(t));
                row.latch_lines.push(format!("t {t}: a world open inside PARK's stretch: the redstone {lead} gt ahead, PARK's end as the entities see it moves to t {}", (due - lead).max(t) - 1));
            }
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
            // (G1's Q10: a write before tick t is stamped t - 1, the tick boundary it sits on, as
            // the post-tick reads are (R, the wake): intervals against them count every tick)
            row.queue_release_at = Some(t - 1);
            if let Some(vi) = vill.and_then(|v| idx(&run.world, v))
                && redsim::entity::mob::ai::brain::has_exact(&run.world, vi)
            {
                let m = &redsim::entity::mob::ai::brain::ex(&run.world, vi).mem;
                row.state_at_queue_release = Some((m.cant_reach_since.map(|c| run.world.time - c), m.walk_target.is_some() || m.path.is_some()));
            }
            if let Some(c) = f.built_release {
                // (record7's built R1: NOT-ready falls; N on, the built repeaters power the queue rail;
                // an edge: a no-op write is refused (G1's C14))
                if !set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), false)? {
                    return Err(format!("the built release lever ({},{},{}) was already off at the queue release (an expected edge wrote nothing)", c[0], c[1], c[2]));
                }
            } else {
                let (_, driver, facing) = release_cells(&a);
                run.world.set_block(BlockPos::new(driver[0], driver[1], driver[2]), block(&format!("lever[face=floor,facing={facing},powered=true]"))?, redsim::world::flags::ALL);
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
            if !set_lever(&mut run.world, BlockPos::new(c[0], c[1], c[2]), true)? {
                return Err(format!("the built release lever ({},{},{}) was already on at the departure (an expected edge wrote nothing)", c[0], c[1], c[2]));
            }
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
                m.walk_target = Some(redsim::entity::mob::ai::brain::WalkTarget::at_point(Vec3::new(BED_HEAD[0] as f64 + 0.5 + ox, BED_HEAD[1] as f64 + oy, BED_HEAD[2] as f64 + 0.5 + oz), 0.5, 1));
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
            && run.world.entities.list[ci].block_position() == wc(ENTRY)
        {
            entry_seen = true;
            // (G1's C5: this read is before the tick, the cart's state after the previous tick: the
            // entry's post-tick time is t - 1, on R's footing (R is read after its tick))
            row.entry_at = Some(t - 1);
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
        // ((A)'s arming detector: the first tick it is powered, this cart's passage; G1's Q10: read
        // before tick t, the detector's state after tick t - 1: stamped t - 1, as the entry (C5))
        if let Some(a) = lay.arm
            && row.armed_at.is_none()
            && format!("{}", run.world.get(wc(a))).contains("powered=true")
        {
            row.armed_at = Some(t - 1);
        }
        // (G1 12:38 / ironspec 12:38-12:58: DAY-1's guard-free latch, emulated on the REAL wake
        // sensor's pulses (the observer in the design): SLEPT a T flip-flop toggled by each pulse,
        // reset at each visit's release (day-1 13:22); OPEN set once per visit on (ARMED70 AND
        // SLEPT), reset when SLEPT falls
        // or at TIMEOUT (ARM + t_max); the shutter = NOT OPEN, `logic_lat` gt behind OPEN; the
        // park's release `rel_after` gt after OPEN's fall, or after TIMEOUT when OPEN never set
        // (the missed-nap path); every visit releases once. The forced fault rows: a hurt
        // `--hurt-after-lie k` gt after the lie-down (a reverse merge at k <= 3))
        // (G1's CHANGES 8 D3: the hurt lands at the lie-down + k exactly on B2's pre-tick rule: a
        // write before tick t is stamped t - 1, so it fires at iteration s + k + 1; the wake it causes
        // is stamped t - 1 too)
        if let Some(k) = f.hurt_after_lie
            && !hurt_done
            && row.slept_at.is_some_and(|s| t >= s + k + 1)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
        {
            hurt_done = true;
            let woke = redsim::entity::mob::living::hurt(&mut run.world, vi, redsim::entity::mob::Damage::InWall, 1.0);
            planted_wake = Some(t - 1);
            row.latch_lines.push(format!("t {}: the forced hurt ({k} gt after the lie-down at t {:?}): damage taken {woke}, asleep after it {}", t - 1, row.slept_at, redsim::entity::mob::mob(&run.world, vi).sleeping_pos.is_some()));
        }
        // (ironspec 21:21 (4): the hurt K gt after ARM's edge, the earliest wake the forced rows
        // carry (ARM + 9), on a napper already asleep)
        if let Some(k) = f.hurt_at_arm
            && !hurt_done
            && row.armed_at.is_some_and(|a| t >= a + k + 1)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
            && redsim::entity::mob::mob(&run.world, vi).sleeping_pos.is_some()
        {
            hurt_done = true;
            let woke = redsim::entity::mob::living::hurt(&mut run.world, vi, redsim::entity::mob::Damage::InWall, 1.0);
            planted_wake = Some(t - 1);
            row.latch_lines.push(format!("t {}: the forced hurt at ARM + {} (ARM at t {:?}): damage taken {woke}, asleep after it {}", t - 1, t - 1 - row.armed_at.unwrap_or(0), row.armed_at, redsim::entity::mob::mob(&run.world, vi).sleeping_pos.is_some()));
        }
        // (G1 21:20 (1): the napper moved off P onto the bed foot's top K gt after RLS: an empty
        // departure with PARK firing anyway)
        // (day-1 21:33: a forced move, an impulse given to the woken napper the tick after the wake:
        // does it carry the napper onto (-1,0,4) through a hook-replaced wall?)
        if let Some((dx, dz)) = f.shove_at_wake
            && row.shove_at.is_none()
            && row.woke_at.is_some_and(|w0| t == w0 + 1)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
        {
            let d = run.world.entities.list[vi].delta_movement;
            run.world.entities.list[vi].delta_movement = Vec3::new(d.x + dx, d.y + 0.1, d.z + dz);
            row.shove_at = Some(t - 1);
            row.latch_lines.push(format!("t {}: the woken napper shoved by ({dx}, 0.1, {dz}) (a knockback's impulse)", t - 1));
        }
        if let Some(k) = f.step_off_at_rls
            && row.step_off_at.is_none()
            && row.rls_at.is_some_and(|r| t == r + k + 1)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
        {
            let p = run.world.entities.list[vi].pos;
            run.world.entities.set_pos(vi, Vec3::new(off[0] as f64 - 2.5, off[1] as f64 + 0.5625, off[2] as f64 + 3.5));
            row.step_off_at = Some(t - 1);
            row.latch_lines.push(format!("t {}: the napper stepped off P at RLS + {k} (from {:.2},{:.2},{:.2} onto the bed foot's top)", t - 1, p.x - off[0] as f64, p.y - off[1] as f64, p.z - off[2] as f64));
        }
        if let Some(l) = f.latch {
            while latch_seen < row.sensor_rises.len() {
                let r = row.sensor_rises[latch_seen];
                latch_seen += 1;
                // (day-1 13:22: SLEPT toggles on every pulse, reset at each visit's release, not at
                // ARM: an ARM-edge reset (ARM + 2..4) can't clear the lie-down pulse (ARM + 5..6) by
                // 2 gt)
                latch_slept = !latch_slept;
                row.latch_lines.push(format!("t {r}: the wake sensor's pulse: SLEPT -> {latch_slept}"));
            }
            // (armed_at is a post-tick stamp (G1's Q10); a write before tick t sits at t - 1)
            let armed70 = row.armed_at.is_some_and(|a| t - 1 >= a + f.arm_delay);
            let timeout = row.armed_at.is_some_and(|a| t - 1 >= a + l.t_max);
            if !latch_once && armed70 && latch_slept && !timeout {
                latch_once = true;
                latch_open_at = Some(t);
                row.latch_lines.push(format!("t {t}: OPEN set (ARMED70 and SLEPT)"));
            }
            if latch_open_at.is_some() && latch_fall_at.is_none() && (!latch_slept || timeout) {
                latch_fall_at = Some(t);
                release_due = Some(t + l.rel_after);
                row.latch_lines.push(format!("t {t}: OPEN reset ({}); the release due at t {}", if timeout { "TIMEOUT" } else { "SLEPT fell" }, t + l.rel_after));
                if timeout {
                    row.latch_timeout = true;
                }
            }
            if !latch_once && timeout {
                latch_once = true;
                latch_fall_at = Some(t);
                release_due = Some(t + l.rel_after);
                row.latch_timeout = true;
                row.latch_lines.push(format!("t {t}: TIMEOUT with OPEN never set (the missed-nap path); the release due at t {}", t + l.rel_after));
            }
            let want_open = latch_open_at.is_some_and(|o| t >= o + l.logic_lat) && !latch_fall_at.is_some_and(|fa| t >= fa + l.logic_lat);
            if want_open != latch_window_open {
                latch_window_open = want_open;
                for c in lay.hatch_ctrl.clone() {
                    if !set(&mut run.world, c, if want_open { "stone" } else { "redstone_block" }) {
                        row.problems.push(format!("t {t}: the latch's shutter edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                    }
                }
                if want_open {
                    f_fired = true;
                    // (G1 13:26: the window's opening, a write before tick t: stamped t - 1)
                    row.open_at.get_or_insert(t - 1);
                    row.waker_lines.push(format!("t {t} (F, the emulated latch): the window open"));
                } else {
                    row.close_at.get_or_insert(t - 1);
                    hidden = true;
                    row.waker_lines.push(format!("t {t}: the window shut (the emulated latch: OPEN's fall + {} gt)", l.logic_lat));
                }
            }
            if release_due == Some(t) && row.release_at.is_none() {
                if let Some(vi) = vill.and_then(|v| idx(&run.world, v))
                    && redsim::entity::mob::mob(&run.world, vi).sleeping_pos.is_some()
                {
                    row.released_asleep = true;
                    row.problems.push(format!("t {t}: the park released with the napper asleep (normal-operation bar 0)"));
                }
                if !lay.latch
                    && let Some(i) = idx(&run.world, cart_id)
                {
                    let d = run.world.entities.list[i].delta_movement;
                    row.speed_at_release = Some((d.x * d.x + d.y * d.y + d.z * d.z).sqrt());
                }
                for &c in &lay.park_ctrl {
                    if !set(&mut run.world, c, "redstone_block") {
                        row.problems.push(format!("t {t}: the latch's release edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                    }
                }
                // (`--latch-b`: the laid latch takes RLS's line at RLS_IN, the AND's output held
                // `latch_rls_width` gt; PARK is then the latch's own OR line)
                if lay.latch {
                    if !set(&mut run.world, LATCH_RLS_IN, "redstone_block") {
                        row.problems.push(format!("t {t}: RLS_IN's stand-in lever wrote nothing"));
                    }
                    latch_rls_off = Some(t + f.latch_rls_width);
                    row.latch_lines.push(format!("t {}: RLS_IN on (the laid latch's RLS line, {} gt)", t - 1, f.latch_rls_width));
                }
                row.release_at = Some(t - 1);
                // (day-1 13:22: the release resets SLEPT (and ARMED70) for the next visit)
                latch_slept = false;
                row.latch_lines.push(format!("t {t}: the park released (SLEPT reset)"));
            }
        }
        // (G1's B1: DAY-1's adopted stop controller, emulated on the real cells (`Adopted`'s doc;
        // P_STRING and PARKED are named stand-ins))
        // (G1 19:26's planted row: E replaced by a plain rail of its shape: it never ejects, the
        // napper stays seated over the activator (-1,0,3); G1 04:08a item 13: written below at the
        // layout's own E with its role asserted)
        // (ironspec 21:10 (3), the POWER-UP row: P_HOLD's latch come up SET, the park drive ON before
        // the first arrival; DAY-1 21:10: its RESET ORs ARM's edge, emulated here as the drive off at
        // ARM's rise unless --park-initial-on keep (the control: no ARM reset, the first cart
        // unbraked))
        if let Some(keep) = f.park_initial_on {
            if t == 1 {
                for &c in &lay.park_ctrl {
                    if !set(&mut run.world, c, "redstone_block") {
                        row.problems.push(format!("t {t}: the power-up SET drive at ({},{},{}) wrote nothing", c[0], c[1], c[2]));
                    }
                }
                drive_on = true;
                row.latch_lines.push("t 0: the park drive ON at power-up (P_HOLD come up SET)".into());
            } else if !keep && row.armed_at.is_some_and(|a| t == a + 5) && row.release_at.is_none() {
                for &c in &lay.park_ctrl {
                    if !set(&mut run.world, c, "stone") {
                        row.problems.push(format!("t {t}: the power-up reset at ({},{},{}) wrote nothing", c[0], c[1], c[2]));
                    }
                }
                drive_on = false;
                row.latch_lines.push(format!("t {}: ARM's edge (ARM + 4 through its delay-1 diode and the reset's delay-1 repeater, DAY-1 21:18) resets P_HOLD: the park drive OFF", t - 1));
            }
        }
        // (the stretched PARK's end, and the R-press row's positive control: Q lit by hand at the
        // rider's RS_ready press, `--light-q-at-rs`, for the same width)
        // (the re-laid stop, ironspec 21:51 (5): K's drive emulated on its stand-in lever: on
        // (north_east) for the arrival, off (south_east) from the park string's first press for
        // the return; `--k-stuck north_east|south_east` holds it)
        if let Some(kc) = lay.k_ctrl {
            let want_on = match f.k_stuck {
                Some(on) => on,
                // (G1 03:46 B1's twin, `--consumers-raw`: K's drive on the RAW string, on whenever
                // the string reads unpressed: every re-check release flips K and fails by name)
                None if f.consumers_raw => !sense_prev,
                None => row.park_sense_at.is_none(),
            };
            if want_on != k_on {
                if !set(&mut run.world, kc, if want_on { "redstone_block" } else { "stone" }) {
                    row.problems.push(format!("t {t}: K's drive edge wrote nothing"));
                }
                k_on = want_on;
                if !want_on {
                    row.k_offs += 1;
                }
                row.latch_lines.push(format!("t {}: K's drive {} (K {})", t - 1, if want_on { "ON" } else { "OFF" }, if want_on { "north_east" } else { "south_east" }));
            }
        }
        if park_off_due == Some(t) {
            for &c in &lay.park_ctrl {
                if !set(&mut run.world, c, "stone") {
                    row.problems.push(format!("t {t}: PARK's end at ({},{},{}) wrote nothing", c[0], c[1], c[2]));
                }
            }
            drive_on = false;
            row.park_off_at = Some(t);
            row.latch_lines.push(format!("t {}: PARK's stretch ends: the park drive OFF", t - 1));
        }
        // (`--latch-b`: the laid latch's inputs: RLS_IN off after its width; ARM_IN held 3 gt from
        // ARM's edge (tick-exact 23:47: ARM must hold >= 3 gt to clear Q))
        if lay.latch {
            if latch_rls_off == Some(t) {
                set(&mut run.world, LATCH_RLS_IN, "stone");
                row.latch_lines.push(format!("t {}: RLS_IN off", t - 1));
            }
            if row.armed_at.is_some_and(|a| t == a + 1) {
                if !set(&mut run.world, LATCH_ARM_IN, "redstone_block") {
                    row.problems.push(format!("t {t}: ARM_IN's stand-in lever wrote nothing"));
                }
                row.latch_lines.push(format!("t {}: ARM_IN on (3 gt)", t - 1));
            }
            if row.armed_at.is_some_and(|a| t == a + 4) {
                set(&mut run.world, LATCH_ARM_IN, "stone");
            }
            // (G1 04:08a item 5: the hold's launch on Q's OWN tap: the next visit's ARM and RLS on
            // the latch's own inputs, `--latch-next-rls T` (ARM_IN at T less this visit's RLS - ARM
            // gap, held 3 gt; RLS_IN at T for its width), so Q's tap line drives the slope as built)
            if let (Some(nt), Some(a), Some(rl)) = (f.latch_next_rls, row.armed_at, row.rls_at) {
                let na = nt - (rl - a);
                if t == na + 1 {
                    if !set(&mut run.world, LATCH_ARM_IN, "redstone_block") {
                        row.problems.push(format!("t {t}: the next visit's ARM_IN wrote nothing"));
                    }
                    row.latch_lines.push(format!("t {}: the next visit's ARM_IN on (3 gt; RLS - ARM = {} as this visit's)", t - 1, rl - a));
                }
                if t == na + 4 {
                    set(&mut run.world, LATCH_ARM_IN, "stone");
                }
                if t == nt {
                    if !set(&mut run.world, LATCH_RLS_IN, "redstone_block") {
                        row.problems.push(format!("t {t}: the next visit's RLS_IN wrote nothing"));
                    }
                    row.latch_lines.push(format!("t {}: the next visit's RLS_IN on ({} gt): Q's tap launches the held cart", t - 1, f.latch_rls_width));
                    next_rls_off = Some(t + f.latch_rls_width);
                }
                if next_rls_off == Some(t) {
                    set(&mut run.world, LATCH_RLS_IN, "stone");
                }
            }
        }
        if hand_q_due == Some(t) {
            for &c in &lay.park_ctrl {
                if !set(&mut run.world, c, "redstone_block") {
                    row.problems.push(format!("t {t}: the hand-lit Q at ({},{},{}) wrote nothing", c[0], c[1], c[2]));
                }
            }
            hand_q_off = Some(t + f.park_width.unwrap_or(12));
            row.latch_lines.push(format!("t {}: Q lit BY HAND at the RS_ready press (the positive control; RLS low)", t - 1));
        }
        if hand_q_off == Some(t) {
            for &c in &lay.park_ctrl {
                set(&mut run.world, c, "stone");
            }
            row.latch_lines.push(format!("t {}: the hand-lit Q off", t - 1));
        }
        if f.block_p_at_wake
            && row.slept_at.is_some_and(|s| t == s + 1)
            && !set(&mut run.world, [-2, 1, 4], "glass")
        {
            row.problems.push(format!("t {t}: the control twin's block over P wrote nothing"));
        }
        // (G1 04:08a item 13: the planted E-off row's target is the layout's own activator, its role
        // asserted (an activator rail) before the write)
        if f.plant_e_off && t == 1 {
            let e_now = format!("{}", run.world.get(wc(lay.act)));
            if !e_now.contains("activator_rail") {
                return Err(format!("--plant-e-off: the layout's E {:?} holds {e_now}, not an activator rail (G1 04:08a item 13)", lay.act));
            }
            if !set(&mut run.world, lay.act, "rail[shape=north_south,waterlogged=false]") {
                row.problems.push(format!("t {t}: the planted E-off at {:?} wrote nothing", lay.act));
            }
            row.latch_lines.push(format!("t 0: E {:?} ({e_now}) planted OFF (a plain rail)", lay.act));
        }
        if let Some(ad) = f.adopted {
            // (ARM's edge starts the visit: PARKED and A70 count from it (G18))
            if row.armed_at.is_some() && !ad_armed_seen {
                ad_armed_seen = true;
                ad_released = false;
                ad_consumed = false;
            }
            // (G22 P_STRING, a named stand-in: the awake napper's box meeting the band at (-2,1,4),
            // released exactly `lag` (10) gt after its last meet (the latest a real string releases;
            // one releases 1..10 gt after: G1 21:29);
            // this read is before tick t, so its post-tick time is t - 1, on the wake's footing; G1's
            // C2: taken at the iteration's start, before any planted write)
            let meet = p_meet_pre;
            if meet {
                p_last_meet = Some(t - 1);
                if !p_on {
                    p_on = true;
                    p_press = Some(t - 1);
                    row.latch_lines.push(format!("t {}: P_STRING pressed (adopted; the stand-in: the napper's box meets its band)", t - 1));
                }
            } else if p_on && (if row.p_string_real { true } else { p_last_meet.is_some_and(|m| t - 1 - m >= ad.lag) }) {
                p_on = false;
                // (G16: SINCE resets on NOT P_STRING)
                ad_since = false;
                // (ironspec 21:21 (3): P_STRING's spans, for the lie-down exclusion's drop - pulse)
                if let Some(pp) = p_press {
                    row.p_spans.push((pp, t - 1));
                }
                row.latch_lines.push(format!("t {}: P_STRING released ({} gt after its last meet; SINCE reset)", t - 1, ad.lag));
            }
            // (the observer's pulses: G19 SO set by any pulse; G17 WIN restarted by each; G16 SINCE
            // set by a pulse with P_STRING on at its head change, and (G1's B2 onset) the press begun
            // within `onset` gt of that change; the change is the pulse's rise - 2 (the observer's
            // 2-gt delay, tick-exact 26.3); a merged lie-down + wake is one pulse)
            while ad_seen < row.sensor_rises.len() {
                let r = row.sensor_rises[ad_seen];
                ad_seen += 1;
                ad_pulse = Some(r);
                ad_so = true;
                let change = r - 2;
                let onset_ok = ad.onset.is_none_or(|o| p_press.is_some_and(|ps| (ps - change).abs() <= o));
                ad_since = p_on && onset_ok;
                row.latch_lines.push(format!("t {r}: the observer's pulse (its change t {change}): SO set, WIN restarted, SINCE {} (P_STRING {}{})", if ad_since { "set" } else { "not set" }, if p_on { "on" } else { "off" }, if p_on && !onset_ok { format!(", its press at t {:?} not within {:?} gt of the change", p_press, ad.onset) } else { String::new() }));
            }
            // (G18 PARKED = SENSED AND COOL (rev 20:00): COOL is ARM's edge through the `parked` tap,
            // SENSED the sensed park's press through d; with `parked` 0 the stand-in: the cart at
            // rest in the park cell; reset at RLS. A70: the `a70` tap AND PARKED (0: dropped)
            let parked = !ad_consumed
                && if ad.parked > 0 {
                    // (G1 03:46 B1's twin, `--consumers-raw`: PARKED also needs the string pressed
                    // NOW, the raw state, instead of SENSED's latch alone)
                    row.armed_at.is_some_and(|a| t - 1 >= a + ad.parked) && f.parked_sense.is_none_or(|(_, dl)| row.park_sense_at.is_some_and(|p| t - 1 >= p + dl)) && (!f.consumers_raw || sense_prev)
                } else {
                    idx(&run.world, cart_id).is_some_and(|i| {
                        let c = &run.world.entities.list[i];
                        let cp = dpos(c.pos);
                        [cp.x.floor() as i32, (cp.y + 0.01).floor() as i32, cp.z.floor() as i32] == lay.park && c.delta_movement.horizontal_distance_sqr() < 1e-6
                    })
                };
            let a70 = parked && row.armed_at.is_some_and(|a| t - 1 >= a + ad.a70);
            // (G1's CHANGES 8 C4: PARKED's first set, stamped t - 1 like its terms)
            if parked && row.parked_set_at.is_none() {
                row.parked_set_at = Some(t - 1);
            }
            // (G20 OPEN: S = A70 and SO and not P_STRING, R = RLS or P_STRING)
            if ad_open && p_on {
                ad_open = false;
                ad_open_reset_at = Some(t);
                row.latch_lines.push(format!("t {}: OPEN reset (P_STRING)", t - 1));
            } else if !ad_open && !ad_consumed && a70 && ad_so && !p_on {
                ad_open = true;
                ad_open_set_at = Some(t);
                ad_open_reset_at = None;
                row.latch_lines.push(format!("t {}: OPEN set (A70 and SO and not P_STRING; adopted)", t - 1));
            }
            // (G17 WIN: open from the latest pulse + `d` (RLS's sample) until a new pulse or the
            // pulse + `w`; G21 RLS = P_STRING and SINCE and WIN and PARKED, `rls` gt of logic after
            // (its AND reads P_STRING that far back); one pulse a visit)
            let win = ad_pulse.is_some_and(|l| t - 1 >= l + ad.d && t - 1 < l + ad.w);
            if !ad_released && ad_rls_due.is_none() && p_on && ad_since && win && parked {
                ad_rls_due = Some(t + ad.rls);
                row.latch_lines.push(format!("t {}: P_STRING and SINCE (press t {:?}) and WIN (pulse t {:?} + {}) and PARKED: RLS due at t {}", t - 1, p_press, ad_pulse, ad.d, t + ad.rls - 1));
            }
            if ad_rls_due == Some(t) {
                ad_released = true;
                // (ironspec 21:29, DAY-1's Q-keyed form: PARKED's and SO's consuming resets come from
                // PARK (P_HOLD's Q through a delay-1 diode) at RLS + 6, not from RLS itself)
                ad_consume_due = Some(t + 6);
                row.rls_at = Some(t - 1);
                ad_park_due = Some(t + ad.park);
                if ad_open {
                    ad_open = false;
                    ad_open_reset_at = Some(t);
                    row.latch_lines.push(format!("t {}: OPEN reset (RLS)", t - 1));
                }
                row.latch_lines.push(format!("t {}: RLS; the park due at t {}; PARKED / SO consumed at t {}", t - 1, t + ad.park - 1, t + 5));
                // (`--latch-b`: the laid latch takes RLS's line at RLS_IN (the AND's output held
                // `latch_rls_width` gt); its drive repeater lights the park at RLS + 2 by itself)
                if lay.latch {
                    if !set(&mut run.world, LATCH_RLS_IN, "redstone_block") {
                        row.problems.push(format!("t {t}: RLS_IN's stand-in lever wrote nothing"));
                    }
                    latch_rls_off = Some(t + f.latch_rls_width);
                    row.latch_lines.push(format!("t {}: RLS_IN on (the laid latch's RLS line, {} gt)", t - 1, f.latch_rls_width));
                }
            }
            if ad_consume_due == Some(t) {
                ad_consumed = true;
                ad_so = false;
                row.latch_lines.push(format!("t {}: PARKED and SO reset by PARK (RLS + 6, the Q-keyed form)", t - 1));
            }
            // (the shutter = NOT OPEN: `open_set` / `open_reset` gt of logic, then the torch)
            let want_open = ad_open_set_at.is_some_and(|s| t >= s + ad.open_set + ad.torch) && !ad_open_reset_at.is_some_and(|r| t >= r + ad.open_reset + ad.torch);
            if want_open != ad_window_open {
                ad_window_open = want_open;
                for c in lay.hatch_ctrl.clone() {
                    if !set(&mut run.world, c, if want_open { "stone" } else { "redstone_block" }) {
                        row.problems.push(format!("t {t}: the adopted controller's shutter edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                    }
                }
                if want_open {
                    f_fired = true;
                    row.open_at.get_or_insert(t - 1);
                    row.waker_lines.push(format!("t {t} (the adopted controller): the window open"));
                } else {
                    row.close_at.get_or_insert(t - 1);
                    hidden = true;
                    row.waker_lines.push(format!("t {t}: the window shut (the adopted controller: OPEN's reset + {} gt)", ad.open_reset + ad.torch));
                }
            }
            // (the park's release at RLS + `park`)
            if ad_park_due == Some(t) && row.release_at.is_none() {
                if let Some(vi) = vill.and_then(|v| idx(&run.world, v))
                    && redsim::entity::mob::mob(&run.world, vi).sleeping_pos.is_some()
                {
                    row.released_asleep = true;
                    row.problems.push(format!("t {t}: the park released with the napper asleep (normal-operation bar 0)"));
                }
                if !lay.latch
                    && let Some(i) = idx(&run.world, cart_id)
                {
                    let d = run.world.entities.list[i].delta_movement;
                    row.speed_at_release = Some((d.x * d.x + d.y * d.y + d.z * d.z).sqrt());
                }
                for &c in &lay.park_ctrl {
                    if !set(&mut run.world, c, "redstone_block") {
                        row.problems.push(format!("t {t}: the adopted controller's release edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                    }
                }
                row.release_at = Some(t - 1);
                row.latch_lines.push(format!("t {t}: the park released (RLS + {})", ad.park));
                // (rev 20:00 G21 / ironspec 21:15 (g): PARK's drive is a stretched pulse, P_HOLD's
                // Q torch lit from RLS + 4 until its reset; `--park-width W` turns the bench's drive
                // off W gt after it rose (default: left on))
                drive_on = true;
                if let Some(wd) = f.park_width {
                    park_off_due = Some(t + wd);
                }
            }
        }
        // (F: `after_sleep` gt after the lie-down (the bed observer's edge): the hatch shut, the
        // zombie seen; (A): and not before the arming detector's edge + `arm_delay` (the worst-case
        // braking interval, ironspec 00:37 (2b)))
        // (armed_at is a post-tick stamp (G1's Q10): F's write before tick t sits at t - 1)
        let armed_ok = lay.arm.is_none() || row.armed_at.is_some_and(|a| t - 1 >= a + f.arm_delay);
        if f.latch.is_none() && f.adopted.is_none() && !f_fired && f_live.is_some_and(|x| live >= x) && armed_ok {
            f_fired = true;
            // (G1 13:26: the window's opening, a write before tick t: stamped t - 1)
            row.open_at.get_or_insert(t - 1);
            // (G1: the sleeper's eye-to-eye COLLIDER ray to the waker in both window states, the
            // blocking cell named: before F (window closed) and after F's write (window open))
            let ray = |w: &World| -> String {
                let (Some(vi), Some(zi)) = (vill.and_then(|v| idx(w, v)), zid.and_then(|z| idx(w, z))) else { return "no napper or no waker".into() };
                let (v, z) = (&w.entities.list[vi], &w.entities.list[zi]);
                let veh = if matches!(&v.kind, redsim::entity::EntityKind::Mob(m) if m.sleeping_pos.is_some()) { 0.2 } else { v.kind.eye_height() as f64 };
                let ve = redsim::entity::Vec3::new(v.pos.x, v.pos.y + veh, v.pos.z);
                let ze = redsim::entity::Vec3::new(z.pos.x, z.pos.y + z.kind.eye_height() as f64, z.pos.z);
                let hit = redsim::entity::mob::los::first_hit(w, ve, ze);
                let (vd, zd) = (dpos(ve), dpos(ze));
                format!("the napper's eye ({:.2},{:.2},{:.2}) to the waker's eye ({:.2},{:.2},{:.2}), {:.2} apart: {}", vd.x, vd.y, vd.z, zd.x, zd.y, zd.z, ((ve.x - ze.x).powi(2) + (ve.y - ze.y).powi(2) + (ve.z - ze.z).powi(2)).sqrt(), hit.map_or("CLEAR".to_string(), |p| { let q = dbp(p); format!("blocked at ({},{},{}) {}", q.x, q.y, q.z, w.get(p)) }))
            };
            let before = ray(&run.world);
            for c in lay.hatch_ctrl.clone() {
                if !set(&mut run.world, c, "stone") {
                    row.problems.push(format!("t {t}: F's edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                }
            }
            let after = ray(&run.world);
            row.waker_lines.push(format!("t {t} (F): window closed: {before}; window open: {after}"));
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
            if f.day_gate && !set(&mut run.world, act_ctrl, if day { "stone" } else { "redstone_block" }) {
                row.problems.push(format!("t {t}: the day gate's edge at ({},{},{}) wrote nothing (G1's Q12 / B4)", act_ctrl[0], act_ctrl[1], act_ctrl[2]));
            }
            if day {
                day_since = t;
            }
        }
        let w = &run.world;
        if f.day_gate && day_state && idx(w, cart_id).is_some_and(|ci| w.entities.list[ci].block_position() == wc(act_cell)) {
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
                if !set(&mut run.world, c, "redstone_block") {
                    row.problems.push(format!("t {t}: the abort's release edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                }
            }
            row.release_at = Some(t - 1);
            row.stamp_at_release = vill.and_then(|v| idx(&run.world, v)).filter(|&vi| redsim::entity::mob::ai::brain::has_exact(&run.world, vi)).map(|vi| redsim::entity::mob::ai::brain::ex(&run.world, vi).mem.cant_reach_since.map(|c| run.world.time - c));
        }
        // (the window shut on the wake edge: Z1's design does it as its normal operation (its
        // shutter's drive powered again by the wake: F's line), any other design only under the
        // --hide-after-wake fault; `close_after` gt after the wake (1: the earliest, the bench's
        // stand-in; a real line through the wake sensor closes later, day-1 12:03))
        if f.latch.is_none() && f.adopted.is_none() && (f.hide_after_wake || z1_design) && !hidden && row.woke_at.is_some_and(|wk| t >= wk + f.close_after.max(1)) {
            for c in lay.hatch_ctrl.clone() {
                if !set(&mut run.world, c, "redstone_block") {
                    row.problems.push(format!("t {t}: the close's edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                }
            }
            row.waker_lines.push(format!("t {t}: the window shut ({})", if z1_design { "Z1: F's line powers the shutter again on the wake edge, its normal operation" } else { "--hide-after-wake: the waker's controllers powered again" }));
            // (G1's B2 on 1662bed98: a write before tick t is stamped t - 1, as open_at is)
            row.close_at = Some(t - 1);
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
            row.armed_at.is_some_and(|a| t - 1 >= a + n)
        } else if f.early_release {
            // (G1 04:08a item 10: pinned to the lie-down edge itself, whatever --release-after says
            // (its default moved 0 -> 20 for the stand-in's normal release))
            row.slept_at.is_some_and(|s| t >= s)
        } else if f.no_presence_gate {
            // (G1's Q15: the no-presence-gate fault row's own bound, 300 gt: past every normal wake
            // (the arming edge at entry + 24, the wake by ARM + 90))
            (f_fired && f_live.is_some_and(|x| live >= x + release_after)) || t >= 300
        } else {
            row.woke_at.is_some_and(|wk| t >= wk + release_after)
        };
        if f.latch.is_none() && f.adopted.is_none() && row.release_at.is_none() && go {
            // ((A)'s at-rest bar, ironspec 00:37 (2b): the cart at rest at the park on the release
            // tick)
            if let Some(i) = idx(&run.world, cart_id) {
                let d = run.world.entities.list[i].delta_movement;
                let v = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
                row.speed_at_release = Some(v);
                if lay.arm.is_some() && v >= 0.001 {
                    row.problems.push(format!("t {t}: the cart moving at the release ({v:.4} b/t; the at-rest bar: under 0.001, G1's C19)"));
                }
            }
            for &c in &lay.park_ctrl {
                if !set(&mut run.world, c, "redstone_block") {
                    row.problems.push(format!("t {t}: the release edge at ({},{},{}) wrote nothing (G1's Q12)", c[0], c[1], c[2]));
                }
            }
            row.release_at = Some(t - 1);
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
                    (Vec3::new(0.5 + ox, 0.0625 + oy, 4.5 + oz), Vec3::new(-v, 0.0, 0.0))
                } else if lay.arm.is_some() {
                    (Vec3::new(PARK_A[0] as f64 + 0.5 + ox, PARK_A[1] as f64 + 0.5625 + oy, PARK_A[2] as f64 + 0.5 + oz), Vec3::new(0.0, 0.0, v))
                } else {
                    (Vec3::new(SWEPT[0][0] as f64 + 0.5 + ox, SWEPT[0][1] as f64 + 0.0625 + oy, SWEPT[0][2] as f64 - 1.5 + oz), Vec3::new(0.0, 0.0, v))
                };
                w.entities.list[i].pos = pos;
                w.entities.list[i].delta_movement = vel;
                inject_done = true;
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
            row.park_moved = Some([p.x + d * dx - ox, p.y + d * dy - oy, p.z + d * dz - oz]);
        }
        let before = idx(&run.world, cart_id).map(|i| run.world.entities.list[i].pos);
        // (the panic's tail: NEAREST_HOSTILE held on the zombie through `panic_entry` gt)
        if let Some(k) = f.panic_entry
            && t <= k
            && let (Some(vi), Some(z)) = (vill.and_then(|v| idx(&run.world, v)), zid)
        {
            redsim::entity::mob::ai::brain::brain_mut(&mut run.world, vi).nearest_hostile = Some(z);
            panic_done = true;
        }
        // (G1's Q2: the slope hold's tap on its built path: the stand-in input lever into DAY-1's
        // repeater, on for W gt at T and, with a PERIOD, at every T + k PERIOD (k of either sign:
        // a free-running train at phase T, as the lanes' trains run); each write through `set`
        // (a lever pull))
        if let Some((t0, wd, per)) = f.tap {
            let on = match per {
                // (G1's N13 planted row, --tap-skip K: the train's K-th pulse (k = 0 at T) left out:
                // a held cart then waits a full period, which the bar must catch)
                Some(p) if p > 0 => (t - t0).rem_euclid(p) < wd && f.tap_skip != Some((t - t0).div_euclid(p)),
                _ => t >= t0 && t - t0 < wd,
            };
            if on != tap_on {
                if !set(&mut run.world, TAP_CTRL, if on { "redstone_block" } else { "stone" }) {
                    row.problems.push(format!("t {t}: the tap's edge wrote nothing (G1's Q12)"));
                }
                tap_on = on;
                row.tap_writes.push((t - 1, on));
            }
        }
        // (ironspec 00:03 (2c): `--pulse-cell X,Y,Z@T=STATE`, a block set at tick T before the
        // tick, to measure the navigation's recompute trigger (vanilla's: a COLLISION-shape change
        // near the path's midpoint, `ServerLevel.sendBlockUpdated` -> `shouldRecomputePath`))
        for (p, at, st) in &f.pulses {
            if *at == t {
                // (the cell named in the design frame, written at its build position)
                let wpp = BlockPos::new(p.x + off[0], p.y + off[1], p.z + off[2]);
                let old = run.world.get(wpp);
                let new = block(st)?;
                // (ironspec 04:07: an override that doesn't change its cell fails the run)
                if old == new {
                    return Err(format!("--pulse-cell ({},{},{}) at t {t}: the cell already holds {old} (an override that changes nothing fails the run, ironspec 04:07)", p.x, p.y, p.z));
                }
                let changed = redsim::entity::collide::boxes_at(&run.world, &old, wpp) != redsim::entity::collide::boxes_at(&run.world, &new, wpp);
                let before = vill.and_then(|v| idx(&run.world, v)).map(|vi| redsim::entity::mob::ai::ai(&run.world, vi).nav.time_last_recompute);
                run.world.set_block(wpp, new.clone(), redsim::world::flags::ALL);
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
        // (the farm-stop bar's known positive: the planted erasure, after this tick's own tick,
        // with the bed's ticket TAKEN as by another claimant (ironspec 07:02 real: a positive a
        // healing geometry can't re-claim through), before -> after printed, a no-op refused)
        // (ironspec 23:00 (3) / 23:04: the NO-HOME row: the napper's HOME erased K gt after its wake,
        // while it stands awake on P, the bed's ticket taken by a declared stand-in claimant (no
        // re-claim): tick-exact's AK wanderer stepped from P onto the bed head's top; the cells it
        // stands on afterwards are printed per visit)
        if let Some(k) = f.erase_home_on_p
            && !erase_done
            && row.woke_at.is_some_and(|w0| t == w0 + k + 1)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
        {
            redsim::entity::mob::ai::brain::brain_mut(&mut run.world, vi).home = None;
            let bp = wc(BED_HEAD);
            // (G1 00:02: the real erasure path, SetWalkTargetFromBlockMemory's erase, calls
            // releasePoi: the ticket ends FREE (AK's state); `--erase-home-ticket taken` keeps the
            // night row's declared stand-in claimant instead)
            let free = f.erase_home_ticket != "taken";
            if free {
                if redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bp) == Some(0) {
                    redsim::entity::mob::ai::poi::release(&mut run.world, bp);
                }
            } else {
                redsim::entity::mob::ai::poi::take(&mut run.world, bp);
            }
            erase_done = true;
            row.erase_planted = true;
            row.no_home_from = Some(t - 1);
            // (tick-exact 00:07: AK's state is by DAY, IDLE: the plant can also move the day time
            // (the level's day offset) at the erase, so the HOME-less napper runs IDLE's strolls)
            if let Some(dt) = f.erase_home_day_time {
                let now = run.world.day_time().rem_euclid(24000);
                run.world.mob.day_offset += (dt - now).rem_euclid(24000);
                row.latch_lines.push(format!("t {}: the day time moved to {dt} (AK's IDLE state)", t - 1));
            }
            row.latch_lines.push(format!("t {}: the napper's HOME erased {k} gt after its wake, the bed's ticket {} (the no-HOME row; free tickets now {:?})", t - 1, if free { "left FREE (the real erasure path)" } else { "taken by a declared stand-in claimant" }, redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bp)));
        }
        if f.erase_home_at == Some(t)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
        {
            redsim::entity::mob::ai::brain::brain_mut(&mut run.world, vi).home = None;
            let bp = wc(BED_HEAD);
            let before = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bp);
            redsim::entity::mob::ai::poi::take(&mut run.world, bp);
            let after = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bp);
            if before == after {
                return Err(format!("--erase-home-at {t}: the bed's ticket take changed nothing ({before:?} -> {after:?}): the positive can't block a re-claim"));
            }
            eprintln!("t {t}: the planted erasure: the napper's HOME erased, the bed's ticket taken by a declared stand-in claimant (free tickets {before:?} -> {after:?})");
            erase_done = true;
            row.erase_planted = true;
        }
        // (ironspec 16:35: the self-heal, the FREE-ticket case: the held napper's stamp release (its
        // HOME erased, a fresh CANT_REACH stamp, as redsim's too-long release does) with the bed's
        // ticket free: AcquirePoi must re-acquire it at the stop and the visit nap normally;
        // --erase-home-at is the TAKEN case, the lock)
        if f.release_home_at == Some(t)
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
        {
            let bp = wc(BED_HEAD);
            let free = redsim::entity::mob::ai::poi::free_tickets(&mut run.world, bp);
            if free != Some(1) {
                return Err(format!("--release-home-at {t}: the bed's free tickets are {free:?}, not 1: the free-ticket case needs the ticket free (the taken case is --erase-home-at)"));
            }
            redsim::entity::mob::ai::brain::brain_mut(&mut run.world, vi).home = None;
            // (the stamp in world time, as redsim's release writes it)
            let now = run.world.time;
            redsim::entity::mob::ai::brain::ex_mut(&mut run.world, vi).mem.cant_reach_since = Some(now);
            row.release_home = Some(t);
            println!("t {t}: the planted stamp release (ironspec 16:35, the self-heal): the napper's HOME erased, a fresh CANT_REACH stamp, the bed's ticket free ({free:?})");
        }
        let w = &run.world;
        // (ironspec 05:20, a bench rule: the world's day set before the summon (the visit's hour is
        // reached before the visitor spawns) and the brain's activity asserted at t 1: REST at
        // night, IDLE by day)
        if t == 1
            && let Some(vi) = vill.and_then(|v| idx(&run.world, v))
            && matches!(&run.world.entities.list[vi].kind, redsim::entity::EntityKind::Mob(_))
        {
            let act = redsim::entity::mob::ai::brain::brain(&run.world, vi).activity;
            let d = run.world.day_time().rem_euclid(24000);
            use redsim::entity::mob::villager::Activity as A;
            let ok = if (12_000..24_000).contains(&d) { act == A::Rest } else { matches!(act, A::Idle | A::Meet) };
            if !ok {
                row.problems.push(format!("t 1: the napper's activity {} at day time {d} (expected REST at night, IDLE / MEET by day)", act.name()));
            }
        }
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
                    let p = wc(*c);
                    let boxes = redsim::entity::collide::boxes_at(w, &w.get(p), p);
                    if boxes.iter().any(|b| b.intersects(bx)) {
                        row.head_box_hits += 1;
                    }
                    if boxes.iter().any(|b| b.intersects(eb)) {
                        row.head_eye_hits += 1;
                        if row.head_eye_hits <= 3 {
                            row.problems.push(format!("t {t}: a villager's eye box at ({:.2},{:.2},{:.2}) meets the new solid cell ({},{},{}) (the head-space bar: 0)", e.pos.x - ox, ey - oy, e.pos.z - oz, c[0], c[1], c[2]));
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
        // (the cart's cell and position in the design frame: every design-frame comparison and
        // print below reads these)
        let cbp = dbp(c.block_position());
        let cpos = dpos(c.pos);
        // (ironspec 07:03 (3): the exit booster reached ((A) with --exit-south: (-1,0,6)))
        if cbp == BlockPos::new(-1, 0, 6) && row.booster_at.is_none() {
            row.booster_at = Some(t);
        }
        // (ironspec 09:38 / 09:40 (b), the tap hold at the exit slope (1,-1,5), the napper aboard:
        // the loaded cart's rest on the slope, its creep while held, its departure, the rider's
        // NEAREST_HOSTILE and stamp over the hold, the sight to the waker in both shutter states at
        // the rest, the launch's speed per cell to R)
        if hold_slope_on && row.pickup.is_some() {
            let in_hold = cbp == BlockPos::new(1, -1, 5);
            // (G1's Q2: the cart's first entry into the slope with the slope powered: an arrival
            // during the tap's pulse (it rolls through: an early launch))
            if in_hold && row.hold_rest_at.is_none() && !hold_entered {
                hold_entered = true;
                if format!("{}", w.get(wp(1, -1, 5))).contains("powered=true") {
                    row.hold_arrival_powered = true;
                    row.problems.push(format!("t {t}: the loaded cart reached the slope (1,-1,5) during the tap's pulse (the slope powered: it rolls through, an early launch)"));
                }
            }
            let sp = (c.delta_movement.x.powi(2) + c.delta_movement.y.powi(2) + c.delta_movement.z.powi(2)).sqrt();
            // (G1's Q15: at rest under 1e-4 b/t: the unpowered slope zeroes a speed below 0.03, so a
            // braked cart reads 0; 1e-4 only absorbs float noise)
            if in_hold && row.hold_rest_at.is_none() && sp < 1e-4 && !c.passengers.is_empty() {
                row.hold_rest_at = Some(t);
                row.hold_rest_pos = Some([cpos.x, cpos.y, cpos.z]);
                // (the sight from the held rider to the waker: as the world stands (the shutter
                // shut since the wake), and with the shutter as F leaves it (unpowered: the window
                // open), on a copy)
                if let (Some(vi), Some(zi)) = (vill.and_then(|v| idx(w, v)), zid.and_then(|z| idx(w, z))) {
                    let (v, z) = (&w.entities.list[vi], &w.entities.list[zi]);
                    let ve = redsim::entity::Vec3::new(v.pos.x, v.pos.y + v.kind.eye_height() as f64, v.pos.z);
                    let ze = redsim::entity::Vec3::new(z.pos.x, z.pos.y + z.kind.eye_height() as f64, z.pos.z);
                    let dist = ((v.pos.x - z.pos.x).powi(2) + (v.pos.y - z.pos.y).powi(2) + (v.pos.z - z.pos.z).powi(2)).sqrt();
                    let shut = redsim::entity::mob::los::first_hit(w, ve, ze);
                    let mut w2 = w.clone();
                    let sh = wc(WAKER_SHUT);
                    let open_state = format!("{}", w2.get(sh)).replace("open=true", "open=false").replace("powered=true", "powered=false");
                    w2.set_block(sh, block(open_state.trim_start_matches("minecraft:"))?, 0);
                    let open = redsim::entity::mob::los::first_hit(&w2, ve, ze);
                    let name = |h: Option<BlockPos>, ww: &World| h.map_or("CLEAR".to_string(), |p| { let q = dbp(p); format!("blocked at ({},{},{}) {}", q.x, q.y, q.z, ww.get(p)) });
                    let vd = dpos(ve);
                    row.hold_lines.push(format!("t {t}: the loaded cart at rest on the slope at ({:.3},{:.3},{:.3}); the rider's feet {dist:.2} from the waker's; its eye ({:.2},{:.2},{:.2}) to the waker's eye: shutter as now {}; shutter open (F's state, on a copy) {}", cpos.x, cpos.y, cpos.z, vd.x, vd.y, vd.z, name(shut, w), name(open, &w2)));
                    // (G1's Q3: the held rider never sees the waker, in either shutter state)
                    if shut.is_none() || open.is_none() {
                        row.hold_sight_clear = true;
                        row.problems.push(format!("t {t}: the held rider's line to the waker CLEAR with the shutter {} (bar: blocked in both states)", if shut.is_none() { "as now" } else { "open" }));
                    }
                }
            }
            if let (Some(_), None, Some(p0)) = (row.hold_rest_at, row.hold_depart_at, row.hold_rest_pos) {
                let d = ((cpos.x - p0[0]).powi(2) + (cpos.y - p0[1]).powi(2) + (cpos.z - p0[2]).powi(2)).sqrt();
                // (G1's Q3 / Q15: a departure is a move over 0.01 from the rest position, above the
                // held rest's largest creep, 0.0083 in every reference band)
                if d > 0.01 || !in_hold {
                    row.hold_depart_at = Some(t);
                    // (day-1 10:38: which tap launched it: the last stand-in write powering the
                    // slope at or before the departure, and the wait (a free-running tap launches
                    // a cart waiting a period or more early: the periodic-pulse row shows it))
                    let launching = row.tap_writes.iter().filter(|(at, on)| *on && *at <= t).map(|(at, _)| *at).max();
                    row.hold_lines.push(format!("t {t}: departed the slope: arrived at rest t {:?}, launched by the tap write at t {launching:?}, waited {:?} gt", row.hold_rest_at, row.hold_rest_at.map(|a| t - a)));
                    if let Some(vi) = vill.and_then(|v| idx(w, v))
                        && redsim::entity::mob::ai::brain::has_exact(w, vi)
                    {
                        row.hold_stamp_at_depart = Some(redsim::entity::mob::ai::brain::ex(w, vi).mem.cant_reach_since.map(|s| w.time - s));
                    }
                } else {
                    row.hold_creep = row.hold_creep.max(d);
                    if let Some(vi) = vill.and_then(|v| idx(w, v))
                        && redsim::entity::mob::ai::brain::brain(w, vi).nearest_hostile.is_some()
                    {
                        row.hold_hostile_ticks += 1;
                    }
                }
            }
            if row.hold_depart_at.is_some() && row.r_at.is_none() {
                let cell = [cbp.x, cbp.y, cbp.z];
                if row.launch_cells.last().is_none_or(|l: &([i32; 3], f64)| l.0 != cell) {
                    row.launch_cells.push((cell, c.delta_movement.x.hypot(c.delta_movement.z)));
                }
            }
        }
        // (ironspec 09:04: the cart's first entry into (-2,0,5) after the pickup)
        if row.pickup.is_some() && cbp == BlockPos::new(-2, 0, 5) && row.cell25_at.is_none() {
            row.cell25_at = Some(t);
        }
        // (the cart's block contacts, per tick, by the SHARED judge on landed main (G1's Q1;
        // ironspec 07:47's amended rule): `contacts::cart_contacts` over the cart's recorded SELF
        // moves of this tick (the clipped axes of the attempted move), keyed on the CONTACTED
        // block's cell, not the cart's)
        {
            let mut hits: Vec<(BlockPos, &'static str)> = Vec::new();
            for (p, _) in crate::contacts::cart_contacts(w, ci) {
                if !hits.iter().any(|h| h.0 == p) {
                    hits.push((p, "clipped-move"));
                }
            }
            for (p, k) in hits {
                // (p is the contacted block in the world; its cell in the design frame keys the
                // declaration)
                let cell = dcell(p);
                let q = dbp(p);
                let declared = f.declared_contacts.contains(&cell);
                if !declared && !row.contacts.iter().any(|x| x.2 == k && x.3 == cell) {
                    // (G1's N7: main's ONE judge is the contacts' one verdict; this reader only prints)
                    row.declared_contact_lines.push(format!("t {t}: the cart's {k} contact with {} at ({},{},{}) (the cart at ({:.2},{:.2},{:.2}), cell {},{},{}), not declared in the design's `# contacts:` (a print: main's ONE judge fails it)", w.get(p), q.x, q.y, q.z, cpos.x, cpos.y, cpos.z, cbp.x, cbp.y, cbp.z));
                }
                row.contacts.push((t, [cpos.x, cpos.y, cpos.z], k, cell));
                // (ironspec 07:03 (2): a DECLARED contact printed per visit: its tick, its depth (the
                // cart's box against the contacted block's boxes: the largest overlap, 0 = flush), the
                // speed before and after; G1's bound: depth <= 0.01 and one tick per visit)
                if declared {
                    let mut depth = f64::NEG_INFINITY;
                    for bx in redsim::entity::collide::boxes_at(w, &w.get(p), p) {
                        let ox = c.bb.max_x.min(bx.max_x) - c.bb.min_x.max(bx.min_x);
                        let oy = c.bb.max_y.min(bx.max_y) - c.bb.min_y.max(bx.min_y);
                        let oz = c.bb.max_z.min(bx.max_z) - c.bb.min_z.max(bx.min_z);
                        if oy > 0.0 {
                            depth = depth.max(ox.min(oz));
                        }
                    }
                    let after = c.delta_movement.x.hypot(c.delta_movement.z);
                    row.declared_contact_lines.push(format!("t {t}: the declared {k} contact with {} at ({},{},{}): the cart at ({:.3},{:.3},{:.3}) in cell ({},{},{}), depth {:.4} (0 = flush; negative = a gap), speed {prev_cart_speed:.3} -> {after:.3}", w.get(p), q.x, q.y, q.z, cpos.x, cpos.y, cpos.z, cbp.x, cbp.y, cbp.z, depth));
                    row.declared_contact_ticks += 1;
                    // (G1 10:17: the bed (head and foot: part, facing, OCCUPIED) unchanged across the
                    // contact tick: the contact never moves or alters the napper's POI)
                    let bed_now = (format!("{}", w.get(wc(BED_HEAD))), format!("{}", w.get(wp(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2] - 1))));
                    if prev_bed.as_ref().is_some_and(|b| *b != bed_now) {
                        row.problems.push(format!("t {t}: the bed changed across the declared contact tick: {:?} -> {bed_now:?}", prev_bed));
                    }
                    row.declared_contact_lines.push(format!("t {t}: the bed across the contact tick: unchanged {} (head {}, foot {})", prev_bed.as_ref().is_some_and(|b| *b == bed_now), bed_now.0, bed_now.1));
                    row.declared_hit.push(cell);
                    row.post_contact_speed = Some(row.post_contact_speed.map_or(after, |s: f64| s.min(after)));
                }
            }
            // (ironspec Nov 13 10:42 (3) / 10:58: the ONE bounded judge's input: the cart's hits this
            // tick, horizontal and vertical (main's `contacts::cart_hits` / `cart_vertical_hits`), the
            // contacted cell in the design frame; the judge runs over every visit in the summary)
            for mut h in crate::contacts::cart_hits(w, ci).into_iter().chain(crate::contacts::cart_vertical_hits(w, ci).into_iter().map(crate::contacts::Hit::from)) {
                let dc = dcell(h.cell);
                h.cell = BlockPos::new(dc[0], dc[1], dc[2]);
                row.hits.push(h);
            }
            row.motion.push((w.time, c.delta_movement.x.hypot(c.delta_movement.z), c.delta_movement.y));
            // (ironspec 15:23 / day-1 15:29: the rider-keyed RS_ready's string: its rising edges and
            // whether the riding napper's box was over its cell then; the rider box's overlap with the
            // band and the cart's gap under it while each box is over the string's cell)
            if rs_present {
                row.rs_present = true;
                let over = |bb: &redsim::entity::Aabb| bb.max_x - off[0] as f64 > 4.0 && bb.min_x - (off[0] as f64) < 5.0 && bb.max_z - off[2] as f64 > 5.0 && bb.min_z - (off[2] as f64) < 6.0;
                let rider = vill.and_then(|id| idx(w, id)).map(|i| &w.entities.list[i]).filter(|v| v.vehicle.is_some());
                if over(&c.bb) {
                    let top = c.bb.max_y - off[1] as f64;
                    row.rs_cart_gap_min = Some(row.rs_cart_gap_min.map_or(0.0625 - top, |g: f64| g.min(0.0625 - top)));
                }
                let rider_over = rider.is_some_and(|v| over(&v.bb));
                if let Some(v) = rider.filter(|v| over(&v.bb)) {
                    let (y0, y1) = (v.bb.min_y - off[1] as f64, v.bb.max_y - off[1] as f64);
                    let ov = y1.min(0.15625) - y0.max(0.0625);
                    row.rs_rider_overlap_min = Some(row.rs_rider_overlap_min.map_or(ov, |o: f64| o.min(ov)));
                }
                let on = format!("{}", w.get(wc(RS_STRING))).contains("powered=true");
                if on && !row.rs_on_prev {
                    row.rs_press.push((t, rider_over));
                    if f.light_q_at_rs && rider_over && hand_q_due.is_none() {
                        hand_q_due = Some(t + 1);
                        row.hand_q_at = Some(t);
                    }
                }
                row.rs_on_prev = on;
            }
            // (G1 15:29 / b8949c96 15:45: the cart's states near the top-half trapdoors)
            if !trapdoors.is_empty() {
                let cp = dpos(c.pos);
                let ly = cp.y.floor() as i32;
                if trapdoors.iter().any(|d| (d[1] == ly || d[1] == ly + 1) && (cp.x - (d[0] as f64 + 0.5)).abs() <= 1.5 && (cp.z - (d[2] as f64 + 0.5)).abs() <= 1.5) {
                    let rider = vill.and_then(|id| idx(w, id)).is_some_and(|i| w.entities.list[i].vehicle.is_some());
                    let d = c.delta_movement;
                    row.cart_states.push(format!("{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{}", cp.x, cp.y, cp.z, d.x, d.y, d.z, if rider { "rider" } else { "none" }));
                }
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
            // (the napper's position in the design frame: every design-frame cell, distance and
            // print below reads it; world-frame physics and sight read v.pos)
            let vp = dpos(v.pos);
            let vcell = [vp.x.floor() as i32, (vp.y + 0.01).floor() as i32, vp.z.floor() as i32];
            riding = v.vehicle.is_some();
            // (the napper's HOME every tick; its loss is a problem)
            let has_home = redsim::entity::mob::ai::brain::brain(w, vi).home.is_some();
            if had_home && !has_home && row.home_lost_at.is_none() {
                row.home_lost_at = Some(t);
            }
            // (ironspec 07:30: a released napper's re-claim at the stop, its delay priced)
            if !had_home && has_home && row.home_lost_at.is_some() && row.reclaimed_at.is_none() {
                row.reclaimed_at = Some(t);
            }
            // (ironspec 16:35: the self-heal: the homeless napper's cells on foot from its landing to
            // the re-acquisition)
            if row.release_home.is_some() && !has_home && !riding && row.landed.is_some() && row.reclaim_cells.last() != Some(&vcell) {
                row.reclaim_cells.push(vcell);
            }
            had_home = has_home;
            let asleep = redsim::entity::mob::mob(w, vi).sleeping_pos.is_some();
            // (ironspec 15:46 (2): the stale press: the napper's box's gap to P_STRING's band (the
            // awake guard's string at P + 1, (-2,1,4)) over the 12 gt before its lie-down)
            {
                let bb = v.bb;
                let (x0, y0, z0) = (bb.min_x - off[0] as f64, bb.min_y - off[1] as f64, bb.min_z - off[2] as f64);
                let (x1, y1, z1) = (bb.max_x - off[0] as f64, bb.max_y - off[1] as f64, bb.max_z - off[2] as f64);
                let sep = |lo: f64, hi: f64, a: f64, b: f64| (lo - b).max(a - hi).max(0.0);
                let (dx, dy, dz) = (sep(-2.0, -1.0, x0, x1), sep(1.0625, 1.15625, y0, y1), sep(4.0, 5.0, z0, z1));
                let gap = (dx * dx + dy * dy + dz * dz).sqrt();
                if asleep && row.pstring_gap_before_lie.is_none() {
                    row.pstring_gap_before_lie = Some(row.pstring_ring.iter().filter(|(tt, _)| *tt >= t - 12).map(|(_, g)| *g).fold(f64::INFINITY, f64::min));
                }
                row.pstring_ring.push((t, gap));
                if row.pstring_ring.len() > 12 {
                    row.pstring_ring.remove(0);
                }
            }
            // (G1 16:00: the boarded rider's ride away from the bed: its distance to the bed head's
            // centre (-2.5,0.5,4.5) per tick after the pickup)
            if riding && row.pickup.is_some() {
                let dd = ((vp.x + 2.5).powi(2) + (vp.y - 0.5).powi(2) + (vp.z - 4.5).powi(2)).sqrt();
                if row.r_real_at.is_some() && row.ride_dist_at_r.is_none() {
                    row.ride_dist_at_r = Some(dd);
                }
                row.ride_dist_last = Some((t, dd));
                if dd > 16.0 && row.ride_over16_at.is_none() {
                    row.ride_over16_at = Some(t);
                }
                // (G1 04:31 (1)(b): the rider's distance when RS_ready sets (its first press + d_RS
                // 10), the earliest the next napper can be released toward the bed)
                if let Some(p) = row.rs_press.iter().find(|p| p.1).map(|p| p.0)
                    && t == p + f.d_rs
                {
                    row.ride_dist_at_rs_set = Some(dd);
                }
            }
            // (G1 19:10: no villager's feet ever in a guard string's cell, any tick)
            if (vcell == [-1, 1, 2] || vcell == [-1, 1, 5]) && !riding {
                row.guard_cell_ticks += 1;
                if row.guard_cell_ticks == 1 {
                    row.problems.push(format!("t {t}: the napper's feet in the guard string's cell {vcell:?} (G1 19:10: unreachable by walking)"));
                }
            }
            // (G1 16:09: the step-off: awake after the wake, before the pickup: on P calm (REST) or
            // not, and the first cell off P)
            if !riding && !asleep && row.woke_at.is_some() && row.pickup.is_none() {
                if vcell == [-2, 0, 4] {
                    if redsim::entity::mob::ai::brain::brain(w, vi).activity.name() == "rest" {
                        row.rest_on_p += 1;
                    } else {
                        row.other_on_p += 1;
                    }
                    // (tick-exact 22:44: the napper's feet y on P (a wet P bobs it))
                    let fy = dpos(w.entities.list[vi].pos).y - off[1] as f64;
                    row.p_feet_y = Some(row.p_feet_y.map_or((fy, fy), |(a, b): (f64, f64)| (a.min(fy), b.max(fy))));
                } else if row.stepped_off.is_none() {
                    row.stepped_off = Some((t, vcell));
                    row.problems.push(format!("t {t}: the woken napper stepped off P to {vcell:?} before the pickup (G1 16:09: the cart would leave empty)"));
                }
            }
            // (ironspec 23:00 (3): the no-HOME napper's on-foot cells after the erase, the first tick
            // on each, with its feet y (a bed top shows as y >= 0.5 in the bed's cells))
            if !riding && row.no_home_from.is_some() && row.no_home_cells.last().is_none_or(|(_, c, _)| *c != vcell) && row.no_home_cells.len() < 40 {
                let fy = dpos(w.entities.list[vi].pos).y - off[1] as f64;
                row.no_home_cells.push((t, vcell, fy));
            }
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
                let key = dcell(vb);
                if !bolt_seen.contains_key(&key) {
                    let sky = redsim::weather::can_see_sky(w, vb).unwrap_or(true);
                    let bb = v.bb;
                    let mut reach: Option<[i32; 3]> = None;
                    for cx in (bb.min_x.floor() as i32 - 4)..=(bb.max_x.floor() as i32 + 4) {
                        for cz in (bb.min_z.floor() as i32 - 4)..=(bb.max_z.floor() as i32 + 4) {
                            let h = redsim::weather::motion_blocking_height(w, cx, cz);
                            if reach.is_none() && redsim::weather::lightning::hit_box_at(Vec3::new(cx as f64 + 0.5, h as f64, cz as f64 + 0.5)).intersects(bb) {
                                reach = Some([cx - off[0], h - off[1], cz - off[2]]);
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
            if rest && !riding && !asleep && row.eligible_at.is_none() && vp.distance_to_sqr(bc) < 4.0 {
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
                row.landing_pos = Some([vp.x, vp.y, vp.z]);
            }
            if !riding && row.landed.is_none() && v.on_ground {
                row.landed = Some((vcell, t));
                // (vcompose 03:19: the exact feet where the eject leaves the napper standing)
                row.landed_exact = Some([vp.x, vp.y, vp.z]);
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
                last_awake = Some(vp);
            }
            // (ironspec 06:40 (2), the waker's consequence judged directly: from the wake to the
            // pickup + 40 gt, every change of the napper's walk target, path (complete or partial,
            // its start node) and CANT_REACH stamp, printed per visit; the summary counts the
            // stamps the waker's flee arms and whether each clears before the visit's end; with the
            // slope hold, through the hold's departure + 10 (G1's Q15: a window chosen past the
            // departure so a stamp armed while held is still seen as the rider leaves; not a
            // measured term: the departure -> R itself takes 13..15 gt on the band))
            if row.woke_at.is_some()
                && (row.pickup.is_none_or(|(_, tp)| t <= tp + 40) || (row.hold_rest_at.is_some() && row.hold_depart_at.is_none_or(|d| t <= d + 10)))
                && redsim::entity::mob::ai::brain::has_exact(w, vi)
            {
                // (G1: the panic window per wake: each activity change and each NEAREST_HOSTILE
                // change from the wake on, beside the walk targets)
                let bb = redsim::entity::mob::ai::brain::brain(w, vi);
                let ak = format!("{}|{:?}", bb.activity.name(), bb.nearest_hostile.is_some());
                if ak != waker_act_last {
                    row.waker_lines.push(format!("t {t}: activity {}, NEAREST_HOSTILE {}", bb.activity.name(), if bb.nearest_hostile.is_some() { "remembered" } else { "empty" }));
                    waker_act_last = ak;
                }
                // (ironspec 09:04: per wake, the tick NEAREST_HOSTILE empties and the tick PANIC is
                // over, against the cart's first entry into (-2,0,5) (~5.9 from Z1))
                if bb.nearest_hostile.is_none() && row.mem_empty_at.is_none() {
                    row.mem_empty_at = Some(t);
                }
                if bb.activity != redsim::entity::mob::villager::Activity::Panic && row.panic_end_at.is_none() {
                    row.panic_end_at = Some(t);
                }
                let m = &redsim::entity::mob::ai::brain::ex(w, vi).mem;
                // (printed in the design frame: the bench's `--offset`)
                let wt = m.walk_target.as_ref().map(|x| match x.target {
                    redsim::entity::mob::ai::brain::Tracker::Block(b) => format!("{:?}", redsim::entity::mob::ai::brain::Tracker::Block(dbp(b))),
                    redsim::entity::mob::ai::brain::Tracker::Point(q) => format!("{:?}", redsim::entity::mob::ai::brain::Tracker::Point(dpos(q))),
                    ref other => format!("{other:?}"),
                });
                let pa = m.path.as_ref().map(|p| (p.reached, p.nodes.first().map(|n| (n.x - off[0], n.y - off[1], n.z - off[2])), p.nodes.last().map(|n| (n.x - off[0], n.y - off[1], n.z - off[2])), p.nodes.len()));
                // (the re-laid stop's stamp class: the path's nodes, design frame, to read the route
                // a COMPLETE HOME path takes)
                let pnodes: Vec<(i32, i32, i32)> = m.path.as_ref().map(|p| p.nodes.iter().map(|n| (n.x - off[0], n.y - off[1], n.z - off[2])).collect()).unwrap_or_default();
                let st = m.cant_reach_since;
                // (G1's C11: a FRESH arming is a stamp set after the wake (a value the stamp didn't
                // hold at the wake tick), not one carried in from before it)
                if waker_stamp_at_wake.is_none() {
                    waker_stamp_at_wake = Some(st);
                }
                let key = format!("{wt:?}|{pa:?}|{st:?}");
                if key != waker_last {
                    row.waker_lines.push(format!("t {t}: walk target {}, path {}, stamp {}{}", wt.as_deref().unwrap_or("none"), pa.map_or("none".to_string(), |(r, s, e, n)| format!("{} ({n} nodes, start {s:?}, end {e:?})", if r { "COMPLETE" } else { "PARTIAL" })), st.map_or("none".to_string(), |s| format!("armed at t {}", s - (w.time - t))), if pnodes.is_empty() { String::new() } else { format!("; nodes {pnodes:?}") }));
                    if st.is_some() && Some(st) != waker_stamp_at_wake && row.waker_stamp.is_none() {
                        row.waker_stamp = Some(t);
                    }
                    waker_last = key;
                }
                if st.is_none() && row.waker_stamp.is_some() && row.waker_cleared.is_none() {
                    row.waker_cleared = Some(t);
                }
                // (ironspec 00:27b: every arming and clearing of the stamp after the wake, with the
                // walk target that made it (HOME = the bed; anything else a flee or a stroll) and
                // the path's ends)
                let armed_now = st.is_some();
                if armed_now != stamp_prev_armed && row.woke_at.is_some() {
                    let kind = match wt.as_deref() {
                        Some(s) if s.contains(&format!("x: {}, y: {}, z: {}", BED_HEAD[0], BED_HEAD[1], BED_HEAD[2])) => "HOME",
                        Some(_) => "flee/stroll",
                        None => "no walk target",
                    };
                    let cell = [vp.x.floor() as i32, (vp.y + 0.01).floor() as i32, vp.z.floor() as i32];
                    row.stamp_events.push(format!("t {t} {} by {kind} at {cell:?} (path {})", if armed_now { "ARMED" } else { "CLEARED" }, pa.map_or("none".to_string(), |(r, s, e, n)| format!("{} {n} nodes {s:?} -> {e:?}", if r { "COMPLETE" } else { "PARTIAL" }))));
                    stamp_prev_armed = armed_now;
                }
            }
            // (G1 12:38 / 13:02, ironspec 12:50 / 13:00 (5): the merge row's per-tick trace from the
            // lie-down to the pickup: OCCUPIED, SLEPT, the window, NEAREST_HOSTILE, HURT_BY, the
            // activity, the napper's position, and whether Z1 is in view (within 8, eye to eye))
            if (f.latch.is_some() || f.adopted.is_some())
                && f.hurt_after_lie.is_some()
                && row.slept_at.is_some()
                && row.pickup.is_none()
                && row.latch_lines.len() < 600
            {
                let bb = redsim::entity::mob::ai::brain::brain(w, vi);
                let occ = format!("{}", w.get(wc(BED_HEAD))).contains("occupied=true");
                let seen = zid.and_then(|z| idx(w, z)).map(|zi| {
                    let z = &w.entities.list[zi];
                    let ze = Vec3::new(z.pos.x, z.pos.y + z.kind.eye_height() as f64, z.pos.z);
                    let ve = Vec3::new(v.pos.x, v.pos.y + if asleep { 0.2 } else { v.kind.eye_height() as f64 }, v.pos.z);
                    let d = ve.distance_to_sqr(ze).sqrt();
                    (d, d <= 8.0 && redsim::entity::mob::los::has_line_of_sight(w, ze, ve))
                });
                row.latch_lines.push(format!("t {t}: OCCUPIED {occ}, SLEPT {latch_slept}, window {}, NEAREST_HOSTILE {}, HURT_BY {:?}, activity {}, asleep {asleep}, riding {riding}, at ({:.2},{:.2},{:.2}), Z1 {}", if latch_window_open { "open" } else { "shut" }, if bb.nearest_hostile.is_some() { "set" } else { "empty" }, bb.hurt_by, bb.activity.name(), vp.x, vp.y, vp.z, seen.map_or("absent".to_string(), |(d, s)| format!("{d:.2} away, in view {s}"))));
            }
            // (G1, Z1's flee margin measured: from the wake to the pickup + 40 gt, the napper's feet
            // to the waker zombie's feet each tick, and its eye-to-eye line of sight; a tick under 6
            // with sight is a flee trigger (SetWalkTargetAwayFrom's strict < 6))
            if row.woke_at.is_some()
                && (row.pickup.is_none_or(|(_, tp)| t <= tp + 40) || (row.hold_rest_at.is_some() && row.hold_depart_at.is_none_or(|d| t <= d + 10)))
                && let Some(zi) = zid.and_then(|z| idx(w, z))
            {
                let z = &w.entities.list[zi];
                let d = ((v.pos.x - z.pos.x).powi(2) + (v.pos.y - z.pos.y).powi(2) + (v.pos.z - z.pos.z).powi(2)).sqrt();
                let veh = if matches!(&v.kind, redsim::entity::EntityKind::Mob(m) if m.sleeping_pos.is_some()) { 0.2 } else { v.kind.eye_height() as f64 };
                let ve = redsim::entity::Vec3::new(v.pos.x, v.pos.y + veh, v.pos.z);
                let ze = redsim::entity::Vec3::new(z.pos.x, z.pos.y + z.kind.eye_height() as f64, z.pos.z);
                let seen = redsim::entity::mob::los::has_line_of_sight(w, ve, ze);
                row.waker_dist_min = Some(row.waker_dist_min.map_or(d, |m: f64| m.min(d)));
                if seen {
                    row.waker_seen_dist_min = Some(row.waker_seen_dist_min.map_or(d, |m: f64| m.min(d)));
                    if d < 6.0 {
                        row.waker_flee_ticks += 1;
                    }
                }
                // (ironspec 07:47 (a): NEAREST_HOSTILE is a memory; within 6 of the waker it must
                // be absent whether the waker is in sight or not)
                if d < 6.0 && redsim::entity::mob::ai::brain::brain(w, vi).nearest_hostile.is_some() {
                    row.waker_memory_ticks += 1;
                }
                if row.pickup.is_some_and(|(_, tp)| tp == t) {
                    row.waker_lines.push(format!("t {t} (the pickup): NEAREST_HOSTILE {:?}, the waker {d:.2} feet to feet, in sight {seen}", redsim::entity::mob::ai::brain::brain(w, vi).nearest_hostile));
                }
            }
            if asleep && row.slept_at.is_none() {
                row.slept_at = Some(t);
                f_live = Some(live + after_sleep);
                row.latch_lines.push(format!("t {t}: the napper lay down (the head's change at t {t}; its pulse at t {})", t + 2));
            }
            if was_asleep && !asleep {
                // (G1's CHANGES 8 D3: a wake from a planted pre-tick write is stamped at the write's
                // t - 1, B2's rule; a natural wake at the tick it happened)
                let wt = planted_wake.take().unwrap_or(t);
                row.woke_at = Some(wt);
                row.latch_lines.push(format!("t {wt}: the napper WOKE (stood on {vcell:?}; the wake's pulse due at t {}{})", wt + 2, if wt != t { "; a planted write's wake" } else { "" }));
                row.wakes.push(wt);
                row.stood = Some(vcell);
                // (vcompose 03:19: the exact feet where the wake stands the napper up)
                if row.woke_exact.is_none() {
                    row.woke_exact = Some([vp.x, vp.y, vp.z]);
                }
                row.stood_up.push(vcell);
                let bed = wc(BED_HEAD);
                if let Block::Bed { facing, .. } = w.get(bed) {
                    for side in [facing.clockwise(), facing.clockwise().opposite()] {
                        let s = redsim::entity::mob::villager::stand_up_with_side(w, redsim::entity::mob::types::MobType::Villager, bed, facing, side);
                        let cell = s.map(|q| [q.x.floor() as i32 - off[0], (q.y + 0.01).floor() as i32 - off[1], q.z.floor() as i32 - off[2]]);
                        if !row.standup_check.contains(&cell) {
                            row.standup_check.push(cell);
                        }
                    }
                }
                let cc = cbp;
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
                    row.path_nodes.insert([n.x - off[0], n.y - off[1], n.z - off[2]]);
                }
            }
            // (after the wake: the woken napper's path off the pickup cell, by direction; north is up
            // the approach, into the arriving cart: a problem in any row)
            if row.woke_at.is_some()
                && !riding
                && !asleep
                && vcell == SWEPT[0]
                && let Some(d) = path_dir_at(w, vi, off)
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
                        let cc = cbp;
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
                        redsim::entity::mob::ai::brain::Tracker::Block(b) => dcell(b),
                        redsim::entity::mob::ai::brain::Tracker::Point(q) => [q.x.floor() as i32 - off[0], q.y.floor() as i32 - off[1], q.z.floor() as i32 - off[2]],
                        redsim::entity::mob::ai::brain::Tracker::Entity { .. } => [i32::MIN, 0, 0],
                    };
                    let from = if riding { format!("riding {},{},{}", cbp.x, cbp.y, cbp.z) } else { format!("on foot {},{},{}", vcell[0], vcell[1], vcell[2]) };
                    if !row.panic_targets.contains(&(from.clone(), p)) {
                        row.panic_targets.push((from, p));
                    }
                }
                if let Some(wk) = row.woke_at
                    && row.stamp_armed.is_none()
                    && let Some(c0) = x.mem.cant_reach_since
                    && c0 >= w.time - (t - wk)
                {
                    let at = if riding { format!("riding {},{},{}", cbp.x, cbp.y, cbp.z) } else { format!("on foot {},{},{}", vcell[0], vcell[1], vcell[2]) };
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
            // (G1 / ironspec 09:13, the 0.56 class by its mechanism: the cart's horizontal speed on
            // the first tick after the release that its pickup box (inflated 0.2, 0, 0.2) meets the
            // napper on foot; AbstractMinecart seats it only above 0.1 b/t (horizontalDistanceSqr
            // > 0.01), below that the cart stops against the napper)
            if row.release_at.is_some() && row.pickup.is_none() && !riding && row.napper_contact.is_none() {
                let (cb, vb) = (&c.bb, &v.bb);
                if cb.min_x - 0.2 < vb.max_x && cb.max_x + 0.2 > vb.min_x && cb.min_y < vb.max_y && cb.max_y > vb.min_y && cb.min_z - 0.2 < vb.max_z && cb.max_z + 0.2 > vb.min_z {
                    row.napper_contact = Some((c.delta_movement.x.hypot(c.delta_movement.z), t));
                    row.napper_at_contact = Some((vp.x, vp.z));
                    // (ironspec 23:54 (a): the inflate is 0 in y: both y ranges at the first contact)
                    row.contact_y = Some((cb.min_y - off[1] as f64, cb.max_y - off[1] as f64, vb.min_y - off[1] as f64, vb.max_y - off[1] as f64));
                }
            }
            // (ironspec 23:40 (1): the upper mechanism bar: any push of the napper by the cart before it
            // is seated: the boxes overlapping un-inflated at a tick end with the napper on foot, or the
            // napper displaced between the first contact and the seating)
            if row.release_at.is_some() && row.pickup.is_none() && !riding {
                let (cb, vb) = (&c.bb, &v.bb);
                if cb.min_x < vb.max_x && cb.max_x > vb.min_x && cb.min_y < vb.max_y && cb.max_y > vb.min_y && cb.min_z < vb.max_z && cb.max_z > vb.min_z && row.push_overlap_at.is_none() {
                    row.push_overlap_at = Some(t);
                }
                if let Some((x0, z0)) = row.napper_at_contact {
                    let d = (vp.x - x0).hypot(vp.z - z0);
                    row.push_disp = row.push_disp.max(d);
                }
            }
            // (G1 09:12 / ironspec 09:13, the re-sleep lock: a lie-down after the wake, before the
            // pickup (SleepInBed: LAST_WOKEN's cooldown over, not a passenger, the head not OCCUPIED,
            // within 2.0 of its centre))
            if asleep && row.woke_at.is_some_and(|w0| t > w0) && row.pickup.is_none() && row.resleep_at.is_none() {
                row.resleep_at = Some(t);
                row.problems.push(format!("t {t}: the napper lay back down after its wake (woke t {:?}) before the pickup: the re-sleep lock", row.woke_at));
                // (ironspec 09:19: the positive's print: LAST_WOKEN, gameTime - LAST_WOKEN, the
                // start, and the erases SleepInBed's start makes (WALK_TARGET, CANT_REACH: before
                // = the previous tick's, after = now))
                let bb = redsim::entity::mob::ai::brain::brain(w, vi);
                let wt_now = redsim::entity::mob::ai::brain::has_exact(w, vi) && redsim::entity::mob::ai::brain::ex(w, vi).mem.walk_target.is_some();
                row.resleep_line = Some(format!("t {t}: SleepInBed started (the napper asleep again, sleeping_pos {:?}) at feet ({:.3},{:.3},{:.3}); LAST_WOKEN {:?} (gameTime - LAST_WOKEN = {:?}); WALK_TARGET {} -> {}; CANT_REACH {} -> {}", redsim::entity::mob::mob(w, vi).sleeping_pos.map(dbp), vp.x, vp.y, vp.z, bb.last_woken, bb.last_woken.map(|lw| w.time - lw), prev_walk_target, wt_now, prev_cant_reach, redsim::entity::mob::ai::brain::has_exact(w, vi) && redsim::entity::mob::ai::brain::ex(w, vi).mem.cant_reach_since.is_some()));
            }
            // (G1 09:19 (1): the boardings per visit (more than one: a pickup fault); (2) the least
            // distance from HOME's head centre over the standing window, the wake to the last boarding)
            if riding && !prev_riding && row.release_at.is_some() {
                row.boardings += 1;
            }
            if !riding && row.woke_at.is_some_and(|w0| t >= w0) && !asleep && row.out_at.is_none() && row.boardings < 2 {
                let d = ((vp.x - (BED_HEAD[0] as f64 + 0.5)).powi(2) + (vp.y - (BED_HEAD[1] as f64 + 0.5)).powi(2) + (vp.z - (BED_HEAD[2] as f64 + 0.5)).powi(2)).sqrt();
                row.stand_min_home = Some(row.stand_min_home.map_or(d, |m: f64| m.min(d)));
            }
            prev_riding = riding;
            if !riding {
                prev_foot_y = Some((v.bb.min_y - off[1] as f64, v.bb.max_y - off[1] as f64));
            }
            if redsim::entity::mob::ai::brain::has_exact(w, vi) {
                let m = &redsim::entity::mob::ai::brain::ex(w, vi).mem;
                prev_walk_target = m.walk_target.is_some();
                prev_cant_reach = m.cant_reach_since.is_some();
            }
            if riding && row.release_at.is_some() && row.pickup.is_none() {
                // (the mechanism variable when the cart seats the napper on the tick its pickup box
                // first meets it: the speed it started that tick with)
                if row.napper_contact.is_none() {
                    row.napper_contact = Some((prev_cart_speed, t));
                }
                // (ironspec 23:54 (a): seated on the first contact tick: the y ranges from the cart's
                // box now and the napper's last on-foot box)
                if row.contact_y.is_none()
                    && let Some((v0, v1)) = prev_foot_y
                {
                    row.contact_y = Some((c.bb.min_y - off[1] as f64, c.bb.max_y - off[1] as f64, v0, v1));
                }
                row.pickup = Some((row.cells_after_wake.last().copied().unwrap_or(vcell), t));
                row.pickup_speed = Some(c.delta_movement.x.hypot(c.delta_movement.z));
                // (the cart's actual horizontal step on the pickup tick: the 0.2-step clip's quantity;
                // in water the move is clamped to 0.2 per axis while the velocity can be higher)
                row.pickup_step = Some((c.pos.x - last_cart[0]).hypot(c.pos.z - last_cart[2]));
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
        // (G1's Q15: "still" = under 1e-4 moved in a tick (float noise on a braked cart); 10 gt of it
        // marks the park's rest, the braked rail having zeroed the speed several ticks before)
        let cp = [c.pos.x, c.pos.y, c.pos.z];
        let moved = (cp[0] - last_cart[0]).abs() + (cp[2] - last_cart[2]).abs() + (cp[1] - last_cart[1]).abs();
        still = if moved < 1e-4 { still + 1 } else { 0 };
        if still == 10 && row.parked.is_none() && row.release_at.is_none() {
            row.parked = Some(([cpos.x, cpos.y, cpos.z], t));
            shift_pending = f.park_shift;
        }
        // (ironspec 17:32 (3): the cart's first end-of-tick in P (-2,0,4) after the release, against
        // the napper's boarding-cooldown end)
        if row.release_at.is_some() && row.cart_at_p.is_none() && [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32] == [-2, 0, 4] {
            row.cart_at_p = Some(t);
        }
        // (G1 19:26: a RIDDEN cart on the activator (-1,0,3), which only the empty cart should ride:
        // every such tick counted, a per-visit problem at the first; G1's C1 on bfa0c8098: the
        // detector (0,0,3)'s press, the activator's power edges and the tick the cart's passenger
        // count changes, each printed by tick, so the mechanism is shown, not inferred)
        // (ironspec 21:15 (c)/(g): the park rail (1,0,4) powered while the bench's own drive is off
        // (no release, no power-up SET): a park pulse with RLS low; and the cart's first tick off the
        // park rail after the release, which PARK's stretch must outlast)
        let park_rail_on = format!("{}", w.get(wc(lay.park))).contains("powered_rail[powered=true");
        if park_rail_on && !park_rail_prev && !drive_on && !lay.latch {
            row.park_pulses_rls_low.push(t);
        }
        // (ironspec 01:17 (2): the laid latch's PARK as the park rail sees it: its first lit and
        // first dark end-of-tick after RLS (the standing start-of-tick frame: `t` here)
        if lay.latch && row.rls_at.is_none() && park_rail_on && !park_rail_prev {
            // (G1 04:08a item 4: a park lighting before RLS is a latch fault)
            row.park_rises_before_rls += 1;
        }
        if lay.latch && row.rls_at.is_some() {
            // (this visit's lightings only: `--latch-next-rls` writes the next visit's RLS, whose
            // own lighting is the next visit's)
            if park_rail_on && !park_rail_prev && f.latch_next_rls.is_none_or(|nt| t < nt) {
                row.park_rises += 1;
            }
            if park_rail_on && !park_rail_prev && row.park_lit.is_none() {
                row.park_lit = Some((t, None));
                // (the at-rest bar for the laid latch: the cart's speed on the last tick before
                // the park rail lit, the release as the cart sees it)
                row.speed_at_release = Some(prev_cart_speed3);
            } else if !park_rail_on && park_rail_prev
                && let Some((a, None)) = row.park_lit
            {
                row.park_lit = Some((a, Some(t)));
            }
        }
        park_rail_prev = park_rail_on;
        if row.release_at.is_some() && row.park_left_at.is_none() && [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32] != lay.park {
            row.park_left_at = Some(t);
        }
        let act_powered = format!("{}", w.get(wc([-1, 0, 3]))).contains("activator_rail[powered=true");
        if act_powered && !act_prev_powered {
            row.ridden_act_lines.push(format!("t {t}: the activator (-1,0,3) powered (the cart in {:?} with {} passenger(s))", [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32], c.passengers.len()));
        }
        act_prev_powered = act_powered;
        if c.passengers.len() != prev_pass_n {
            if row.ridden_on_act > 0 || row.ridden_act_lines.iter().any(|l| l.contains("activator")) {
                row.ridden_act_lines.push(format!("t {t}: the cart's passengers {prev_pass_n} -> {} (the cart in {:?})", c.passengers.len(), [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32]));
            }
            prev_pass_n = c.passengers.len();
        }
        if !c.passengers.is_empty() && [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32] == [-1, 0, 3] {
            row.ridden_on_act += 1;
            if row.ridden_on_act == 1 {
                row.ridden_act_lines.push(format!("t {t}: a ridden cart on (-1,0,3), the activator {}", if act_powered { "POWERED" } else { "unpowered" }));
                row.problems.push(format!("t {t}: a ridden cart on the activator (-1,0,3) (G1 19:26: only the empty cart may ride it)"));
            }
        }
        // (ironspec 17:35: the sensed park: its detector rail's rising edges (the real cell when the
        // design lays it; else a NAMED STAND-IN, the cart in the cell); the first before the release
        // latches PARKED's evidence; G1 17:49 (2)(a): every press per visit, those after RLS named)
        // (the re-laid stop: SENSED is the rest-keyed tripwire over the park (ironspec 22:38),
        // read from its powered state; redsim lacks the game's within-tick raised move on the
        // ascending rail (gap (a)), so its press can come later than the game's park-entry press)
        let sense_cell = lay.park_string.unwrap_or_else(|| f.parked_sense.map_or(PARK_SENSE, |(c, _)| c));
        let det = format!("{}", w.get(wc(sense_cell)));
        let det_real = det.contains("detector_rail") || det.contains("tripwire[");
        // (G1's C4: a cell without a detector rail falls back to the stand-in, and says so)
        if !det_real {
            row.sense_stand_in = true;
        }
        let pressed = if det_real { det.contains("powered=true") } else { [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32] == sense_cell };
        if pressed && !sense_prev {
            row.park_sense_presses.push(t);
            row.ridden_act_lines.push(format!("t {t}: the sensed park ({},{},{}) pressed (the cart in {:?} with {} passenger(s))", sense_cell[0], sense_cell[1], sense_cell[2], [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32], c.passengers.len()));
            // (G1 04:08a item 12: SENSED's SET gated on ARMED, as the design's press-set latch is)
            if row.release_at.is_none() && row.park_sense_at.is_none() && row.armed_at.is_some() {
                row.park_sense_at = Some(t);
            }
        }
        // (G1 02:27 (3) / ironspec 02:28 (4): the park string's flicker: its re-check releases and
        // re-presses while the cart rests on the park, between SENSED's first press and the release,
        // each with the cart's x; K's reset and PARKED read the latch, never these edges)
        if lay.park_string.is_some() && row.park_sense_at.is_some() && row.release_at.is_none() && pressed != sense_prev {
            row.string_flicker.push((t, pressed, cpos.x));
        }
        sense_prev = pressed;
        // (G1 02:47: the empty cart's arrival toward the park, its speed crossing the siding's
        // (-1,0,3) centre x -0.5 eastward: tick-exact's slope-string rig launches from there (0.05
        // strands at x 0.73, short of the park; 0.10 reaches x 1.037))
        if lay.park_string.is_some() && row.release_at.is_none() && row.siding_speed.is_none() {
            let prev_x = last_cart[0] - (cp[0] - cpos.x);
            if prev_x < -0.5 && cpos.x >= -0.5 && (cpos.z - 3.5).abs() < 0.3 {
                row.siding_speed = Some(c.delta_movement.horizontal_distance_sqr().sqrt());
            }
        }
        // (ironspec 21:51 (5): K's shape changes and the cart's ticks in K (the arrival: its first
        // and last; the released cart's first); the park's highest x before the release)
        let k_state = format!("{}", w.get(wc([-2, 0, 3])));
        let k_shape = k_state.split("shape=").nth(1).map(|s| s.split([',', ']']).next().unwrap_or("").to_string()).unwrap_or_default();
        if row.k_shapes.last().is_none_or(|(_, s)| *s != k_shape) {
            row.k_shapes.push((t, k_shape));
        }
        let ccell = [cpos.x.floor() as i32, (cpos.y + 0.01).floor() as i32, cpos.z.floor() as i32];
        if ccell == [-2, 0, 3] {
            if row.release_at.is_none() {
                row.k_arrive = Some(row.k_arrive.map_or((t, t), |(a, _)| (a, t)));
            } else if row.k_return_first.is_none() {
                row.k_return_first = Some(t);
            }
        }
        if row.release_at.is_none() && [ccell[0], ccell[2]] == [lay.park[0], lay.park[2]] {
            row.park_max_x = Some(row.park_max_x.map_or(cpos.x, |m: f64| m.max(cpos.x)));
        }
        // (ironspec 22:58 (2)(a) / G1 23:00: the empty cart's speed across the siding's water and into
        // the park on its way out, and back across the water into K and P after the release)
        if ccell != prev_ccell {
            let sp = c.delta_movement.x.hypot(c.delta_movement.z);
            if row.release_at.is_none() {
                if [ccell[0], ccell[2]] == [lay.park[0], lay.park[2]] && row.park_entry_speed.is_none() {
                    row.park_entry_speed = Some(sp);
                }
                if prev_ccell == [-1, 0, 3] && ccell == [0, 0, 3] && row.water_out_arrive.is_none() {
                    row.water_out_arrive = Some(sp);
                }
            } else {
                if ccell == [-1, 0, 3] && prev_ccell == [0, 0, 3] && row.water_in_return.is_none() {
                    row.water_in_return = Some(sp);
                }
                if prev_ccell == [-1, 0, 3] && ccell == [-2, 0, 3] && row.water_out_return.is_none() {
                    row.water_out_return = Some(sp);
                }
                if ccell == [-2, 0, 4] && prev_ccell == [-2, 0, 3] && row.p_entry_speed.is_none() {
                    row.p_entry_speed = Some(sp);
                }
            }
            prev_ccell = ccell;
        }
        last_cart = cp;
        if let Some(ci) = idx(w, cart_id) {
            let ce = &w.entities.list[ci];
            let d = ce.delta_movement;
            prev_cart_speed = d.x.hypot(d.z);
            prev_cart_speed3 = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
        }
        prev_bed = Some((format!("{}", w.get(wc(BED_HEAD))), format!("{}", w.get(wp(BED_HEAD[0], BED_HEAD[1], BED_HEAD[2] - 1)))));
        // (G1 12:04 / day-1 12:03: the wake sensor's pulses, its rising edges by tick)
        {
            let s = format!("{}", w.get(wc(sensor_cell)));
            if s.contains("observer") {
                let on = s.contains("powered=true");
                if on && !sensor_prev {
                    row.sensor_rises.push(t);
                }
                sensor_prev = on;
            }
        }
        // (the shutter's state read each end-of-tick: its first open and first shut after the window
        // opened, printed beside the writes' stamps (C = the closing write's t - 1, ironspec 20:55))
        if z1_design {
            let shut = format!("{}", w.get(wc(WAKER_SHUT))).contains("open=true");
            // (rev 20:00 G20 / G1's N5: the longest run of end-of-ticks with the window open while
            // the napper stands awake on P, bar <= 9)
            let on_p_awake = vill.and_then(|v| idx(w, v)).is_some_and(|vi| {
                let e = &w.entities.list[vi];
                let p = dpos(e.pos);
                e.vehicle.is_none() && redsim::entity::mob::mob(w, vi).sleeping_pos.is_none() && [p.x.floor() as i32, (p.y + 0.01).floor() as i32, p.z.floor() as i32] == [-2, 0, 4]
            });
            if !shut && on_p_awake {
                open_on_p_run += 1;
                row.open_on_p_max = row.open_on_p_max.max(open_on_p_run);
            } else {
                open_on_p_run = 0;
            }
            if !shut {
                if !shutter_open_seen {
                    // (G1 17:40: the window's first open end-of-tick, printed beside the write's stamp)
                    row.open_state_at = Some(t);
                }
                shutter_open_seen = true;
            } else if shutter_open_seen && !close_by_state {
                // (ironspec 20:55 on G1's B2: C stays the write's stamp, t - 1 for a pre-tick write;
                // the state's first shut end-of-tick is printed beside it, one later)
                close_by_state = true;
                row.close_state_at = Some(t);
            }
        }
        // (ironspec 17:32 (3): the napper's boarding cooldown, set at the eject's dismount)
        if let Some(vi) = vill.and_then(|v| idx(w, v)) {
            let cd = w.entities.list[vi].boarding_cooldown;
            if cd > 0 {
                cooldown_seen = true;
            } else if cooldown_seen && row.cooldown_end.is_none() {
                row.cooldown_end = Some(t);
            }
        }
        // (G1's Q13: the design's `# integrity:` shapes judged every tick (the mutual hold
        // (-1,0,3) <-> (-1,0,4) and K's T among them); a shape off its declared one fails the visit)
        for (cell, shape) in &f.integrity {
            let s = format!("{}", w.get(wc(*cell)));
            if !s.contains(&format!("shape={shape}")) {
                row.integrity_breaks += 1;
                if row.integrity_breaks <= 3 {
                    row.problems.push(format!("t {t}: ({},{},{}) holds {s}, declared shape={shape} (the design's `# integrity:`)", cell[0], cell[1], cell[2]));
                }
            }
        }
        // (ironspec 17:35 (a): the released cart in the pickup cell before the napper's wake)
        if row.release_at.is_some()
            && row.woke_at.is_none()
            && SWEPT.iter().any(|s| cbp == BlockPos::new(s[0], s[1], s[2]))
        {
            row.cart_at_pickup_before_wake = true;
        }
        if let Some(bp) = before
            && row.release_at.is_some()
        {
            let bpz = bp.z - oz;
            for s in SWEPT {
                let zc = s[2] as f64 + 0.5;
                if (bpz < zc) != (cpos.z < zc) && (cpos.x - (s[0] as f64 + 0.5)).abs() < 0.3 && !crossed.contains(&s) {
                    crossed.push(s);
                    row.swept_speed.push((s, c.delta_movement.x.hypot(c.delta_movement.z)));
                }
            }
        }
        // (R and out: the exit runs east along R's row)
        let on_exit_row = (cpos.z - (EXIT_DET[2] as f64 + 0.5)).abs() < 0.3;
        if on_exit_row && cpos.x > EXIT_DET[0] as f64 + 0.5 && row.r_at.is_none() {
            row.r_at = Some(t);
        }
        // (ironspec 22:14 (3a): R's real rise, the detector rail powered (the cart's box meeting its
        // detection box), beside the emulated R above)
        if row.r_real_at.is_none() && format!("{}", w.get(wc(EXIT_DET))).contains("powered=true") {
            row.r_real_at = Some(t);
        }
        if on_exit_row && cpos.x > EXIT_DET[0] as f64 + 1.5 {
            row.out_at = Some(t);
            row.seated_out = riding;
            break;
        }
    }
    if row.out_at.is_none() {
        row.problems.push(format!("the cart not out in {limit} gt"));
    }
    // (ironspec 02:16, row 39's record: where the planted empty cart ended, and the napper's state)
    if let Some(pid) = planted {
        let pc = idx(&run.world, pid).map(|i| run.world.entities.list[i].pos);
        let np = vill.and_then(|v| idx(&run.world, v)).map(|i| {
            let e = &run.world.entities.list[i];
            let home = redsim::entity::mob::ai::brain::brain(&run.world, i).home;
            let since = if redsim::entity::mob::ai::brain::has_exact(&run.world, i) { redsim::entity::mob::ai::brain::ex(&run.world, i).mem.cant_reach_since } else { None };
            format!("the napper at ({:.2},{:.2},{:.2}) riding {}, HOME {}, CANT_REACH since {:?} (age {:?} gt)", e.pos.x - ox, e.pos.y - oy, e.pos.z - oz, e.vehicle.is_some(), home.map_or("LOST".to_string(), |h| format!("{h:?}")), since, since.map(|s| run.world.time - s))
        });
        row.planted_end = Some(format!("the planted empty cart ends at {} after the visit's {limit} gt; {}", pc.map_or("gone".to_string(), |p| format!("({:.2},{:.2},{:.2})", p.x - ox, p.y - oy, p.z - oz)), np.unwrap_or_else(|| "the napper gone".to_string())));
    }
    // (the bed free for the next napper: its head not OCCUPIED at the visit's end)
    row.bed_occupied_at_end = format!("{}", run.world.get(wc(BED_HEAD))).contains("occupied=true");
    // (G1's Q8: a planted fault or override that never applied fails the visit)
    {
        let mut unapplied: Vec<String> = Vec::new();
        if (f.restart.is_some() || f.restart_at_rls.is_some()) && !restart_done {
            unapplied.push("--restart / --restart-at-rls".into());
        }
        if f.step_off_at_rls.is_some() && row.step_off_at.is_none() {
            unapplied.push("--step-off-at-rls (no RLS, or no napper on P)".into());
        }
        if f.hurt_at_arm.is_some() && !hurt_done {
            unapplied.push("--hurt-at-arm (no ARM edge)".into());
        }
        if f.skip.is_some() && row.skipped_at.is_none() {
            unapplied.push("--skip".into());
        }
        if f.erase_home_at.is_some() && !erase_done {
            unapplied.push("--erase-home-at".into());
        }
        if f.release_home_at.is_some() && row.release_home.is_none() {
            unapplied.push("--release-home-at".into());
        }
        // (G1's N6: the faults the check missed)
        if f.arrival_hold.is_some() && !hold_set {
            unapplied.push("--arrival-hold (no running sink to hold)".into());
        }
        if f.panic_entry.is_some() && !panic_done {
            unapplied.push("--panic-entry (no napper and waker to hold NEAREST_HOSTILE on)".into());
        }
        if f.forced_stamp && !entry_seen {
            unapplied.push("--forced-stamp (the cart never reached the entry cell)".into());
        }
        if f.hide_after_wake && !z1_design && row.close_at.is_none() {
            unapplied.push("--hide-after-wake (the hatch never shut after a wake)".into());
        }
        if f.pickup_inject.is_some() && !inject_done {
            unapplied.push("the pickup inject (no release)".into());
        }
        if f.park_shift.is_some() && row.park_moved.is_none() {
            unapplied.push("the park shift (the cart never parked)".into());
        }
        if f.hurt_after_lie.is_some() && !hurt_done {
            unapplied.push("--hurt-after-lie (no lie-down)".into());
        }
        if f.tap.is_some() && row.tap_writes.is_empty() {
            unapplied.push("--tap (no write before the visit ended)".into());
        }
        // (G1's CHANGES 8 N13 / D5: the newer plants' own no-op checks)
        if let (Some(k), Some((t0, _, Some(p)))) = (f.tap_skip, f.tap) {
            let skipped = t0 + k * p;
            let held_across = row.hold_rest_at.is_some_and(|a| a <= skipped && row.hold_depart_at.is_none_or(|d| d > skipped));
            if !held_across {
                unapplied.push(format!("--tap-skip {k} (no held cart on the slope across the skipped pulse at t {skipped})"));
            }
        }
        if f.shove_at_wake.is_some() && row.shove_at.is_none() {
            unapplied.push("--shove-at-wake (no wake)".into());
        }
        if f.light_q_at_rs && row.hand_q_at.is_none() {
            unapplied.push("--light-q-at-rs (no RS_ready press with the drive off)".into());
        }
        if f.erase_home_on_p.is_some() && row.no_home_from.is_none() {
            unapplied.push("--erase-home-on-p (no wake)".into());
        }
        // (G1 04:08a item 13: a reachable Q8 for --k-stuck: K must still hold the stuck shape at the
        // visit's end (no K laid, or a K that moved, is an unapplied plant))
        if let Some(on) = f.k_stuck {
            let want = if on { "north_east" } else { "south_east" };
            if !row.k_shapes.last().is_some_and(|s| format!("{s:?}").contains(want)) {
                unapplied.push(format!("--k-stuck (K not held {want} to the visit's end: {:?})", row.k_shapes.last()));
            }
        }
        if f.release_after_arm.is_some() && row.release_at.is_none() {
            unapplied.push("--release-after-arm (no release)".into());
        }
        if row.pulses.len() < f.pulses.len() {
            unapplied.push(format!("--pulse-cell ({} of {} applied)", row.pulses.len(), f.pulses.len()));
        }
        for u in unapplied {
            row.problems.push(format!("the planted {u} never applied (G1's Q8)"));
        }
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
    // (ironspec 07:02 real, the farm-stop bar at the visit's END: a visit ending without the
    // napper having lain down, or with it leaving without HOME, fails; a release at the stop healed
    // by a re-claim is printed and priced, not failed)
    row.home_at_end = had_home;
    if let Some(t) = row.home_lost_at {
        if had_home {
            row.healed_release = Some(t);
        } else {
            row.problems.push(format!("the napper's HOME lost at t {t} and not back by the visit's end"));
        }
    }
    if row.paths_after_wake.contains(&"north") {
        row.problems.push(if row.paths_after_restart.contains(&"north") { "a northward path off the pickup cell after the restart" } else { "a northward path off the pickup cell" }.into());
    }
    // (--hide-after-wake is a FAULT only off Z1's design; on Z1 the shut window is the normal
    // operation, so the seated and swept-cell checks apply)
    let hide_fault = f.hide_after_wake && !z1_design;
    let expect_seated = !f.empty && !f.no_home && !hide_fault;
    if expect_seated && !row.seated_out {
        row.problems.push("the napper not seated".into());
    }
    if !hide_fault {
        for c in &row.cells_after_wake {
            if !SWEPT.contains(c) {
                row.problems.push(format!("the woken napper stood on {c:?}, outside the swept cells"));
            }
        }
    }
    if !run.world.errors.is_empty() {
        row.problems.push(format!("world: {}", run.world.errors.join("; ")));
    }
    // (G1's CHANGES C16, the water bar: every cell holding water at the visit's end (a water block
    // or a waterlogged state) against the design's own water cells: new water (an escape) or water
    // gone fails the row)
    {
        let wet = |s: &str| s.starts_with("minecraft:water") || s.contains("waterlogged=true");
        let design_wet: std::collections::BTreeSet<[i32; 3]> = spec.blocks.iter().filter(|(_, b)| wet(&format!("{b}"))).map(|(p, _)| [p.x, p.y, p.z]).collect();
        let mut new_wet = Vec::new();
        let mut now_wet = std::collections::BTreeSet::new();
        for x in spec.lo.x..=spec.hi.x {
            for y in spec.lo.y..=spec.hi.y {
                for z in spec.lo.z..=spec.hi.z {
                    let s = format!("{}", run.world.get(BlockPos::new(x, y, z)));
                    if wet(&s) {
                        now_wet.insert([x, y, z]);
                        if !design_wet.contains(&[x, y, z]) {
                            new_wet.push(format!("({},{},{}) {s}", x - off[0], y - off[1], z - off[2]));
                        }
                    }
                }
            }
        }
        let gone: Vec<&[i32; 3]> = design_wet.iter().filter(|c| !now_wet.contains(*c)).collect();
        row.water_cells = now_wet.len();
        if !new_wet.is_empty() {
            row.problems.push(format!("new water at the visit's end (an escape; G1 C16): {} cells: {}", new_wet.len(), new_wet.iter().take(4).cloned().collect::<Vec<_>>().join("; ")));
        }
        if !gone.is_empty() {
            row.problems.push(format!("the design's water gone at the visit's end (G1 C16): {gone:?}"));
        }
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
    let set = |w: &mut World, c: [i32; 3], st: &str| ctl_set(w, c, st);
    // (ironspec 17:46, the stand-in guard: the controller's levers checked against the built cells)
    let (act_cell, act_ctrl) = act_cells(&run.world);
    if layout(&run.world).arm.is_some() {
        return Err("the two-visit bench doesn't drive (A)'s park and arming gate yet (its one-visit bench does)".into());
    }
    for c in [HATCH_CTRL, HATCH_CTRL_HI, act_ctrl].into_iter().chain(PARK_CTRL) {
        for st in ["redstone_block", "stone"] {
            emulated_write_check(&run.world, BlockPos::new(c[0], c[1], c[2]), &block(&ctl_block(c, st))?, &[])?;
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
    // (the activator's initial state, not an edge: the file may already hold it, so an unchanged
    // write is expected here; every later edge is checked (G1's B4))
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
            if o.day_gate && !set(&mut run.world, act_ctrl, if day { "stone" } else { "redstone_block" }) {
                row.problems.push(format!("t {t}: the day gate's edge at ({},{},{}) wrote nothing (G1's Q12 / B4)", act_ctrl[0], act_ctrl[1], act_ctrl[2]));
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
                for c in [HATCH_CTRL, HATCH_CTRL_HI] {
                    if !set(&mut run.world, c, "stone") {
                        row.problems.push(format!("t {t}: F's hatch edge at ({},{},{}) wrote nothing (G1's Q12 / B4)", c[0], c[1], c[2]));
                    }
                }
            }
            // (the release 30 gt after the wake edge, whatever woke the napper)
            if !released && visit_woke.is_some_and(|wk| t >= wk + o.release_after) {
                released = true;
                for pc in PARK_CTRL {
                    if !set(&mut run.world, pc, "redstone_block") {
                        row.problems.push(format!("t {t}: the release edge at ({},{},{}) wrote nothing (G1's Q12 / B4)", pc[0], pc[1], pc[2]));
                    }
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
                    if !set(&mut run.world, pc, "redstone_block") {
                        row.problems.push(format!("t {t}: the abort's release edge at ({},{},{}) wrote nothing (G1's Q12 / B4)", pc[0], pc[1], pc[2]));
                    }
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
                // (G1's Q1: main's shared contacts::cart_contacts, keyed on the CONTACTED block's
                // cell, as the one-visit bench)
                for (p, _) in crate::contacts::cart_contacts(&run.world, ci) {
                    if !o.declared_contacts.contains(&[p.x, p.y, p.z]) {
                        let m = format!("the cart's contact with {} at ({},{},{}), not declared (ironspec 21:25; main's contacts::cart_contacts)", run.world.get(p), p.x, p.y, p.z);
                        if !row.problems.iter().any(|q| q.ends_with(&m)) {
                            row.problems.push(format!("t {t}: {m}"));
                        }
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
                if !set(&mut run.world, pc, "stone") {
                    row.problems.push(format!("t {t}: R's re-arm edge at ({},{},{}) wrote nothing (G1's Q12 / B4)", pc[0], pc[1], pc[2]));
                }
            }
            for c in [HATCH_CTRL, HATCH_CTRL_HI] {
                if !set(&mut run.world, c, "redstone_block") {
                    row.problems.push(format!("t {t}: R's hatch edge at ({},{},{}) wrote nothing (G1's Q12 / B4)", c[0], c[1], c[2]));
                }
            }
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

/// (G1 / ironspec 07:23: an iron trapdoor OPENS on a powered neighbour) Every iron trapdoor of the
/// design after the judge's placement settles: its open / powered state and the world's
/// hasNeighborSignal at its cell (a closed cap with a signal would open at its next update).
pub fn trapdoor_signal_lines(spec: &FarmSpec) -> Result<Vec<String>, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let (mut run, _) = crate::placing::start(spec, &ro, crate::placing::Placement::Judge, &crate::placing::BuildPlan::default())?;
    run.place_detectors();
    let mut out = Vec::new();
    for (p, _) in &spec.blocks {
        if let Block::Trapdoor { iron: true, top, open, powered, .. } = run.world.get(*p) {
            let sig = run.world.has_neighbor_signal(*p);
            // (G1's N8: a trapdoor's update sets both `open` and `powered` to the signal: either
            // unlike it is a change at its next update)
            out.push(format!("iron trapdoor ({},{},{}) {}: open {open}, powered {powered}; hasNeighborSignal {sig}{}", p.x, p.y, p.z, if top { "top half" } else { "bottom half" }, if sig != open || sig != powered { " (MISMATCH: it changes at its next update)" } else { "" }));
        }
    }
    Ok(out)
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
        // (the post: A6's, or the moved waker's when the design holds its shutter)
        let post = if run.world.get(BlockPos::new(WAKER_SHUT[0], WAKER_SHUT[1], WAKER_SHUT[2])).kind() == redsim::block::Kind::Trapdoor(true) { WAKER_POST_S } else { ZOMBIE_POST };
        for e in run.world.entities.list.iter().filter(|e| !e.removed && e.id >= first) {
            let inside = e.pos.x >= post[0] as f64 && e.pos.x <= post[0] as f64 + 1.0 && e.pos.z >= post[2] as f64 && e.pos.z <= post[2] as f64 + 1.0;
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
            run.world.set_block(BlockPos::new(c[0], c[1], c[2]), block(&ctl_block(c, ctrl))?, redsim::world::flags::ALL);
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
    // `a..b/s`: integer lists, and decimal ones (each value formatted to the range's decimals and
    // parsed, so it is the same double as the written decimal: `0.05..0.55/0.01` = `0.05,0.06,...`))
    let mut out = Vec::new();
    for x in s.into_iter().flat_map(|s| s.split(',')) {
        let x = x.trim();
        if let Some((a, rest)) = x.split_once("..") {
            let (b, step) = rest.split_once('/').unwrap_or((rest, "1"));
            if let (Ok(a), Ok(b), Ok(step)) = (a.trim().parse::<i64>(), b.trim().parse::<i64>(), step.trim().parse::<i64>()) {
                let mut v = a;
                while v <= b {
                    out.push(v.to_string().parse()?);
                    v += step.max(1);
                }
            } else {
                let dec = |t: &str| t.trim().split_once('.').map_or(0, |(_, f)| f.len());
                let prec = dec(a).max(dec(b)).max(dec(step));
                let (af, bf, sf): (f64, f64, f64) = (a.trim().parse()?, b.trim().parse()?, step.trim().parse()?);
                if sf <= 0.0 || bf < af {
                    bail!("range {x}: a..b/s with a <= b and s > 0");
                }
                let n = ((bf - af) / sf + 1e-9).floor() as i64;
                for i in 0..=n {
                    out.push(format!("{:.prec$}", af + i as f64 * sf).parse()?);
                }
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
    // (G1's CHANGES C7: --scope-excludes names only bars that can be out of a run's scope; the
    // every-visit bars ("visits") never are; an unknown name is refused, not ignored; the contacts'
    // STALE is the ONE judge's, judged on the run's visits, never excluded)
    // (G1's N12: "riderless" is for the empty-cart control (--empty: its carts leave riderless by
    // design), "head" for designs without the head-space cells; "integrity" for designs that declare
    // no rail shapes; each prints EXCLUDED with its n)
    // (G1's N12 on bfa0c8098: only names that waive a 0-sample NOT JUDGED; "stand" and "head" waived
    // nothing and are refused)
    const SCOPE_BARS: [&str; 4] = ["pickups", "riderless", "waker", "integrity"];
    for b in &base.scope_excludes {
        anyhow::ensure!(SCOPE_BARS.contains(&b.as_str()), "--scope-excludes {b}: not a bar a run can exclude (allowed: {}; the every-visit bars never are)", SCOPE_BARS.join(", "));
    }
    // (G1's C15: no environment teleport in a judged run)
    anyhow::ensure!(std::env::var("MCTAS_BT_TP").is_err(), "MCTAS_BT_TP is set: an environment teleport; unset it (no judged run carries one)");
    // (G1's CHANGES 7: every run prints its own command line)
    // (the command line is printed by the CLI before validation: G1's CHANGES 8 item 0)
    // (ironspec 01:41b: no bench plants a CANT_REACH stamp; a row that does is not evidence)
    let planted_stamp = base.arrival_stamp.is_some() || base.forced_stamp || base.release_home_at.is_some();
    if planted_stamp {
        println!("a PLANTED CANT_REACH stamp in this run (--arrival-stamp / --forced-stamp / --release-home-at): NOT EVIDENCE (ironspec 01:41b: every stamp is armed by a real partial path)");
    }
    println!("bench setup: {}--scope-excludes {:?}; natural spawning OFF by construction (the bench's MobWorld never sets `natural`: Z1 and the napper are placed, nothing spawns; ironspec 14:57)", if base.adopted.is_some() { String::new() } else { format!("(the stand-in controller's terms) --arm-delay {} gt, --release-after {release_after} gt, ", base.arm_delay) }, base.scope_excludes);
    // (G1 15:59's bench rule: a verdict on the stop's controller terms emulates DAY-1's adopted list
    // or carries NAMED STAND-INS)
    if let Some(ad) = base.adopted {
        let sensed = match base.parked_sense {
            Some((c, dl)) => format!("SENSED (the detector rail at ({},{},{}) pressed after ARM, latched, reset at RLS, through d = {dl}) AND ", c[0], c[1], c[2]),
            None => String::new(),
        };
        // (G1's CHANGES 8 D1: the header built from the parsed terms and PARK's width, non-default
        // terms flagged)
        let defaults = Adopted::default();
        let mut nondef: Vec<String> = Vec::new();
        for (name, v, d) in [("PARKED", ad.parked, defaults.parked), ("A70", ad.a70, defaults.a70), ("D", ad.d, defaults.d), ("W", ad.w, defaults.w), ("OPEN_SET", ad.open_set, defaults.open_set), ("OPEN_RESET", ad.open_reset, defaults.open_reset), ("TORCH", ad.torch, defaults.torch), ("RLS", ad.rls, defaults.rls), ("PARK", ad.park, defaults.park), ("LAG", ad.lag, defaults.lag)] {
            if v != d {
                nondef.push(format!("{name} {v} (default {d})"));
            }
        }
        if ad.onset != defaults.onset {
            nondef.push(format!("ONSET {:?} (default {:?})", ad.onset, defaults.onset));
        }
        if base.park_width != Some(24) {
            nondef.push(format!("PARK's width {:?} (default 24)", base.park_width));
        }
        println!("the stop's controller terms (this run): W {} (the latest RLS at the pulse + {}), PARK at RLS + {} held {} gt, PARKED's and SO's consuming resets from PARK at RLS + 6; {}", ad.w, ad.w + ad.rls - 1, ad.park, base.park_width.map_or("(to the visit's end)".to_string(), |w| w.to_string()), if nondef.is_empty() { "the defaults (the re-laid stop's ruled terms, ironspec 21:51 / 22:22)".to_string() } else { format!("NON-DEFAULT: {}", nondef.join(", ")) });
        println!("the stop's controller: DAY-1's gate list (rev 20:00, its stop block G16-G22, with G1's B2 onset; G1's B1) emulated on the real cells: SO set by any observer pulse, reset at RLS + 6 (PARK); SINCE set by a pulse with P_STRING on at its head change (the rise - 2){}, reset on NOT P_STRING; WIN from the latest pulse + {} until a new pulse or the pulse + {}; PARKED = {sensed}{}; OPEN: S = PARKED and SO and not P_STRING{}, R = RLS or P_STRING, the shutter {} / {} + {} gt (torch) behind; RLS = P_STRING and SINCE and WIN and PARKED, {} gt of logic after, one a visit; the park released at RLS + {}; no timeouts; NAMED STAND-INS: P_STRING = a BOX-OVERLAP STAND-IN (no string laid: G1 21:29): the awake napper's box meeting the band of a string at P + 1 (-2,1,4), released exactly {} gt after its last meet (a real string releases 1..10 gt after){}; every RLS-dependent figure below carries it; terms at ctl-shutter's designed timing (the close at the wake + 8, RLS + 20, the park + 24)", ad.onset.map_or(String::new(), |o| format!(" and its press begun within {o} gt of the change")), ad.d, ad.w, if ad.parked > 0 { format!("COOL (ARM + {}, a timer)", ad.parked) } else { "the cart at rest in the park cell (1,0,4) (a stand-in, --adopted-terms PARKED 0)".to_string() }, if ad.a70 > 0 { format!(" (and ARM + {}: an A70 term)", ad.a70) } else { String::new() }, ad.open_set, ad.open_reset, ad.torch, ad.rls, ad.park, ad.lag, if ad.parked > 0 { "" } else { "; PARKED = the cart at rest in the park cell" });
    } else if base.latch.is_some() {
        println!("the stop's controller: (superseded latch, before 15:37) DAY-1's 12:38 guard-free latch (SLEPT / OPEN / TIMEOUT) on the real wake sensor; every controller-dependent line below carries this label");
    } else {
        println!("the stop's controller terms here are NAMED STAND-INS, not DAY-1's adopted list (G1 15:59): the window opens at F = max(the lie-down + {after_sleep}, ARM's rise + {}) gt, shuts at the wake + {}, the park releases at the wake + {release_after} (the stand-in levers (the layout's, named after the runs)); no P_STRING, no RLS / PARKED, no RS_ready latch; the controller-dependent lines below (the window, its close, the release, the hold delta, the boarding cooldown, COOL's inputs, NEAREST_HOSTILE's clear) are on these stand-ins; the contact judge, the stand-up cell, the RS_ready string's press and the wake timings don't depend on them", base.arm_delay, base.close_after.max(1));
    }
    if base.offset != [0, 0, 0] {
        println!("bench setup: --offset {},{},{}: the whole stop built at design + offset (every laid cell, entity and stand-in write), every row in the design frame", base.offset[0], base.offset[1], base.offset[2]);
    }
    // ((A)'s exit-slope hold (ironspec 09:38): the design's slope a powered rail; the tap is the
    // bench's `--pulse-cell 1,-1,6@T=...` write (DAY-1's tap cells in the build))
    let hold_design = spec.blocks.iter().any(|(p, b)| *p == BlockPos::new(1, -1, 5) && format!("{b}").contains("powered_rail"));
    let tap_at: Option<i64> = base.tap.map(|(t, _, _)| t);
    if hold_design {
        println!("the exit-slope hold: the tap on its built path (G1's Q2: the stand-in lever (1,-2,3) into DAY-1's repeater (1,-2,4) facing north, into the slope's slime support) {:?} (T, W, PERIOD) of every visit (one shared absolute phase)", base.tap);
    }
    if base.pulses.iter().any(|(p, _, _)| *p == BlockPos::new(1, -1, 6)) {
        anyhow::bail!("--pulse-cell 1,-1,6 is the retired step stand-in for the slope hold's tap: use --tap T,W[,PERIOD] (G1's Q2)");
    }
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
    // (ironspec 01:50b / 02:33 (2), the timing row: a world open at every phase of the RLS window,
    // `--restart-at-rls A..B,ORDER/stress:2` (the entities frozen 3 gt with the redstone running))
    if let Some((_, how)) = base.restart_at_rls
        && !base.restart_at_rls_phases.is_empty()
    {
        for &k in &base.restart_at_rls_phases {
            rows.push((format!("world open RLS + {k} {}", how.label()), Faults { restart_at_rls: Some((k, how)), ..base.clone() }));
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
        // (G1's C6: the reference night is the run's own setup (arm delay, pulses, levers, the day
        // lever, emulated writes, set cells, ...) with only the rows' variations cleared (the
        // restart, skip, hour, panic entry, pickup inject and park shift))
        let night = Faults { restart: None, skip: None, hour: None, panic_entry: None, pickup_inject: None, park_shift: None, ..base.clone() };
        for (&(seed, v), r) in pairs.iter().zip(par_map(&pairs, jobs, |&(seed, v)| bench(spec, seed, v, release_after, day, after_sleep, &night))) {
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
    // (ironspec 07:02 real, the farm-stop bar at the visit's end: visits ending without a lie-down
    // or without HOME; healed releases printed and priced)
    let (mut stop_bar, mut healed, mut stop_judged) = (0usize, 0usize, 0usize);
    let mut reclaim_delays: Vec<i64> = Vec::new();
    // (ironspec 06:40 (2): the waker's consequence: wakes judged, stamps seen after the wake, and
    // those not cleared by the visit's watch end (the pickup + 40))
    let (mut waker_wakes, mut waker_stamps, mut waker_uncleared) = (0usize, 0usize, 0usize);
    let (mut waker_flee, mut waker_memory, mut waker_dmin, mut waker_seen_dmin) = (0usize, 0usize, f64::MAX, f64::MAX);
    let mut hold_needed = i64::MIN;
    let mut window_rows: Vec<String> = Vec::new();
    let (mut clear_after_close, mut delta_ruled, mut board_wait_max, mut resleeps) = (Vec::<i64>::new(), i64::MIN, i64::MIN, 0usize);
    let mut clear_never = 0usize;
    let (mut contact_speeds, mut park_xs) = (Vec::<(f64, String)>::new(), Vec::<(f64, Option<f64>, String)>::new());
    let (mut multi_board, mut stand_home_min, mut resleep_lines) = (0usize, f64::MAX, Vec::<String>::new());
    let mut post_contact_min: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    let (mut picked_up, mut missed_booster) = (0usize, 0usize);
    // ((A) --exit-south: its exit booster at (-1,0,6); every picked-up cart must reach it)
    let exit_booster = spec.blocks.iter().any(|(p, b)| p.x == -1 && p.y == 0 && p.z == 6 && matches!(b, Block::Rail { activator: false, .. }));
    // (after a restart, the woken napper's paths off the pickup cell: visits with each direction)
    let mut dirs: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
    let mut dirs_wake: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
    // (the lie-downs' reach to the bed head's centre: the most, against SleepInBed's 2)
    let mut lie_max = 0f64;
    let mut lie_min = f64::MAX;
    let (mut min_pick, mut max_pick) = (f64::MAX, 0f64);
    // (the pickups' steps: the cart's position delta on the pickup tick, the 0.2-step clip's
    // quantity, and how many exceed 0.2)
    let (mut min_step, mut max_step, mut steps_over) = (f64::MAX, 0f64, 0usize);
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
                        // (the ONE bounded judge's input: this visit's hits, its cart a sample of
                        // its own)
                        {
                            let vi = sk.judge_visits as u32;
                            sk.judge_visits += 1;
                            sk.judge_visit_speed.push(v);
                            for h in &r.hits {
                                let mut h = h.clone();
                                h.cart = vi;
                                h.sample = Some(vi as u64);
                                sk.judge_hits.push(h);
                            }
                            for &(t, sp, vy) in &r.motion {
                                sk.judge_motion.insert((vi, t), (sp, vy));
                            }
                        }
                        // (the rider-keyed RS_ready, the stale press's evidence, the trapdoor states)
                        if r.rs_present {
                            sk.rs_visits += 1;
                            if r.seated_out {
                                sk.rs_seated += 1;
                                sk.rs_pressed += r.rs_press.iter().any(|p| p.1) as usize;
                            }
                            sk.rs_spurious += r.rs_press.iter().filter(|p| !p.1).count();
                            if let (Some(p), Some(rr)) = (r.rs_press.iter().find(|p| p.1).map(|p| p.0), r.r_real_at) {
                                sk.rs_to_r.push(p - rr);
                            }
                            // (ironspec 01:46 (1) / G1 04:08a / day-1 04:40: d_RS judged at day-1's
                            // controller value (`--d-rs`, 0 unless set): RS_ready sets at the
                            // rider's first press + d_RS, after the cart left the landing (the
                            // stop's ENTRY, where the bench summons it at t 0))
                            if let Some(p) = r.rs_press.iter().find(|p| p.1).map(|p| p.0) {
                                sk.rs_set_after_entry.push(p + base.d_rs);
                                // (G1 04:31 (1)(a): the bed empties at the wake; the next napper is
                                // released no earlier than RS_ready's set: a lower bound of the margin
                                // to its first entry into the 16-ball (its travel adds to it))
                                if let Some(wk) = r.woke_at {
                                    sk.bed_empty_to_rs_set.push(p + base.d_rs - wk);
                                }
                                if let Some(d) = r.ride_dist_at_rs_set {
                                    sk.ride_at_rs_set.push(d);
                                }
                                // (ironspec 04:40 (1): the cycle terms)
                                if let Some(s) = r.slept_at {
                                    sk.cyc_entry_lie.push(s);
                                    if let Some(wk) = r.woke_at {
                                        sk.cyc_nap.push(wk - s);
                                    }
                                }
                                if let (Some(wk), Some(rr)) = (r.woke_at, r.r_real_at) {
                                    sk.cyc_wake_r.push(rr - wk);
                                }
                            }
                            if let Some(o) = r.rs_rider_overlap_min {
                                sk.rs_overlap_min = Some(sk.rs_overlap_min.map_or(o, |m: f64| m.min(o)));
                            }
                            if let Some(g) = r.rs_cart_gap_min {
                                sk.rs_gap_min = Some(sk.rs_gap_min.map_or(g, |m: f64| m.min(g)));
                            }
                        }
                        if let Some(g) = r.pstring_gap_before_lie {
                            sk.pstring_gaps.push(g);
                        }
                        sk.cart_states.extend(r.cart_states.iter().cloned());
                        if let Some(d) = r.ride_dist_at_r {
                            sk.ride_at_r.push(d);
                        }
                        if let Some((_, d)) = r.ride_dist_last {
                            sk.ride_last.push(d);
                        }
                        sk.ride_over16 += r.ride_over16_at.is_some() as usize;
                        sk.rest_on_p_max = sk.rest_on_p_max.max(r.rest_on_p);
                        sk.rest_on_p_visits += (r.rest_on_p > 0) as usize;
                        sk.other_on_p_max = sk.other_on_p_max.max(r.other_on_p);
                        sk.step_offs += r.stepped_off.is_some() as usize;
                        // (ironspec 16:35: the self-heal / the lock)
                        if let Some(rl) = r.release_home {
                            sk.heal_visits += 1;
                            if let Some(rc) = r.reclaimed_at {
                                sk.heal_reacquired += 1;
                                if let Some((_, lt)) = r.landed {
                                    sk.heal_land_to_reacquire.push(rc - lt);
                                }
                                sk.heal_release_to_reacquire.push(rc - rl);
                                if let Some(s) = r.slept_at.filter(|&s| s >= rc) {
                                    sk.heal_slept += 1;
                                    sk.heal_reacquire_to_lie.push(s - rc);
                                }
                            }
                            sk.heal_normal += (r.seated_out && r.r_at.is_some() && r.problems.is_empty()) as usize;
                            for c in &r.reclaim_cells {
                                *sk.heal_cells.entry(*c).or_default() += 1;
                            }
                        }
                        if r.erase_planted {
                            sk.lock_visits += 1;
                            sk.lock_slept += r.slept_at.is_some() as usize;
                        }
                        // (G1's B1: the adopted controller's ticks against the wake)
                        if base.adopted.is_some() && r.slept_at.is_some() {
                            sk.ad_no_open += r.open_at.is_none() as usize;
                            sk.ad_no_rls += r.rls_at.is_none() as usize;
                            if let Some(wk) = r.woke_at {
                                if let Some(c) = r.close_at {
                                    sk.ad_close.push(c - wk);
                                }
                                // (G1's B2 / D7: the shutter's first shut end-of-tick, printed)
                                if let Some(c) = r.close_state_at {
                                    sk.ad_close_state.push(c - wk);
                                }
                                if let Some(x) = r.rls_at {
                                    sk.ad_rls.push(x - wk);
                                }
                                if let Some(x) = r.release_at {
                                    sk.ad_park.push(x - wk);
                                }
                            }
                        }
                        // (G1 17:33: the park's rest against ARM: the first of the 10 still ticks)
                        if let (Some(a), Some((_, tp))) = (r.armed_at, r.parked) {
                            sk.park_rest.push((r.speed, tp - 9 - a));
                        }
                        // (ironspec 17:32 (3): the cart at P against the napper's boarding-cooldown end)
                        if let (Some(p), Some(c)) = (r.cart_at_p, r.cooldown_end) {
                            sk.board_cd.push((r.speed, p - c));
                        }
                        // (ironspec 17:41: COOL's inputs: the eject after ARM, the park -> P ride;
                        // 17:39: the sensed park's pass -> the rest)
                        if let (Some(a), Some((_, tl))) = (r.armed_at, r.landed) {
                            sk.eject_after_arm.push(tl - a);
                        }
                        if let (Some(rel), Some(p)) = (r.release_at, r.cart_at_p) {
                            sk.park_to_p.push(p - rel);
                        }
                        if let (Some(ps), Some((_, tp))) = (r.park_sense_at, r.parked) {
                            sk.pass_to_rest.push((r.speed, tp - 9 - ps));
                        }
                        // (G1's CHANGES 8 C4: SENSED's binding computed per visit: PARKED = max(the
                        // press + d, COOL); the shift past COOL printed and judged against the
                        // emulation's PARKED)
                        if let (Some(ad), Some((_, dl)), Some(ps), Some(a)) = (base.adopted, base.parked_sense, r.park_sense_at, r.armed_at) {
                            let cool = a + ad.parked;
                            let expect = (ps + dl).max(cool);
                            if ps + dl > cool {
                                sk.sensed_binds.push(ps + dl - cool);
                            }
                            if let Some(pk) = r.parked_set_at
                                && pk != expect
                            {
                                sk.sensed_bind_wrong.push(format!("seed {seed} speed {v}: PARKED at t {pk}, expected max(press {ps} + d {dl}, COOL {cool}) = {expect}"));
                            }
                        }
                        if r.layout_name == "(A) re-laid" {
                            sk.relaid_park = Some(r.park_cell);
                        }
                        if sk.ctl_cells.is_empty() {
                            sk.ctl_cells = r.ctl_cells.clone();
                        }
                        // (ironspec 01:17 (2): the laid latch's PARK window against RLS)
                        if let (Some(rl), Some((a, b))) = (r.rls_at, r.park_lit) {
                            sk.latch_lit.push((a - rl, b.map(|b| b - rl)));
                            // (ironspec 01:50b: the laid PARK's dark tick against the cart's last
                            // tick on the park cell, per row: the timing row's margin)
                            if let (Some(d), Some(left)) = (b, r.park_left_at) {
                                sk.latch_margin.push((d - left, format!("{name}: seed {seed} speed {v}")));
                            }
                        }
                        // (G1 04:08a item 4: a latch visit with RLS and no park lighting, and any
                        // lighting before RLS, are latch faults)
                        if r.latch_mode {
                            sk.latch_visits += 1;
                            if r.rls_at.is_some() && r.park_lit.is_none() {
                                sk.latch_unlit.push(format!("seed {seed} speed {v}"));
                            }
                            sk.latch_rises_before += r.park_rises_before_rls;
                        }
                        // (ironspec 02:16: row 39's record, the planted empty cart)
                        if let Some(p) = &r.planted_end {
                            sk.planted.push(format!("seed {seed} speed {v}: {p}"));
                        }
                        // (G1 02:27 (3): the park string's flicker while parked, and its consumers)
                        if r.layout_name == "(A) re-laid" {
                            if let Some(s) = r.siding_speed {
                                sk.siding_speeds.push((s, seed, v));
                            }
                            for (k, p) in [("the eject's landing", r.landed_exact), ("the first wake's stand-up", r.woke_exact)] {
                                if let Some(p) = p {
                                    *sk.stand_ups.entry(format!("{k} {:.4},{:.4},{:.4}", p[0], p[1], p[2])).or_insert(0) += 1;
                                }
                            }
                            if sk.flicker_ticks.len() < 3
                                && let Some(pk) = r.parked
                                && pk.0[0] < 1.2
                                && !r.string_flicker.is_empty()
                            {
                                sk.flicker_ticks.push(format!("seed {seed} speed {v} (rest x {:.3}, SENSED's first press t {:?}): {}", pk.0[0], r.park_sense_at, r.string_flicker.iter().map(|(t, p, _)| format!("t {t} {}", if *p { "re-pressed" } else { "RELEASED" })).collect::<Vec<_>>().join(", ")));
                            }
                            let rel = r.string_flicker.iter().filter(|x| !x.1).count();
                            let x0 = r.parked.map(|p| p.0[0]);
                            sk.flicker.push((x0, rel, r.k_offs, r.parked_set_at.is_some(), r.park_rises, r.latch_mode));
                            // (ironspec 03:56 / 04:00: which rests occur in the arrival band)
                            if let (Some(x), Some(s)) = (x0, r.siding_speed) {
                                let key = (x * 20.0).floor() as i64;
                                let e = sk.rest_arrivals.entry(key).or_insert((f64::INFINITY, 0.0));
                                e.0 = e.0.min(s);
                                e.1 = e.1.max(s);
                                // (G1 04:24c (3): redsim's rest per arrival speed, against the game's)
                                let sb = (s * 20.0).round() as i64;
                                let g = sk.rest_by_speed.entry(sb).or_insert((f64::INFINITY, f64::NEG_INFINITY, 0));
                                g.0 = g.0.min(x);
                                g.1 = g.1.max(x);
                                g.2 += 1;
                            }
                        }
                        // (G1 17:49 (2)(a): the sensed park's presses per visit, and after RLS)
                        *sk.sense_presses.entry(r.park_sense_presses.len()).or_default() += 1;
                        if let Some(rl) = r.rls_at.or(r.release_at) {
                            sk.sense_after_rls += r.park_sense_presses.iter().filter(|&&p| p > rl).count();
                        }
                        // (G1 19:10: the guard strings' cells entered on foot)
                        sk.guard_cell_ticks += r.guard_cell_ticks;
                        sk.sense_stand_in_visits += r.sense_stand_in as usize;
                        sk.guard_cell_visits += (r.guard_cell_ticks > 0) as usize;
                        // (ironspec 21:10 (3): the first arrival's brake: the cart's rest before the
                        // release in the park cell (1,0,4))
                        let braked = r.parked.is_some_and(|(p, _)| [p[0].floor() as i32, p[2].floor() as i32] == [r.park_cell[0], r.park_cell[2]]);
                        sk.first_braked += braked as usize;
                        // (ironspec 21:11: the least gap from an ARM edge to P_HOLD's stretch, RLS ..
                        // RLS + 8: this visit's ARM precedes it by RLS - ARM)
                        if let (Some(a), Some(rl)) = (r.armed_at, r.rls_at) {
                            sk.arm_to_stretch = Some(sk.arm_to_stretch.map_or(rl - a, |m: i64| m.min(rl - a)));
                        }
                        // (ironspec 21:15: the RLS-low park pulses and the cart's leave of the park)
                        sk.park_pulses += r.park_pulses_rls_low.len();
                        if !r.park_pulses_rls_low.is_empty() {
                            sk.park_pulse_visits += 1;
                            if sk.park_pulse_lines.len() < 3 {
                                sk.park_pulse_lines.push(format!("seed {seed} speed {v}: t {:?} (the RS press at {:?})", r.park_pulses_rls_low, r.rs_press.first().map(|p| p.0)));
                            }
                        }
                        if let (Some(l), Some(rel)) = (r.park_left_at, r.release_at) {
                            sk.park_leave.push(l - rel);
                        } else if r.release_at.is_some() {
                            // (G1's CHANGES 8 D2: a released cart that never left the park cell fails,
                            // never drops out of the denominator)
                            sk.park_never_left += 1;
                        }
                        // (ironspec 21:51 (5): K's junction margins on the re-laid stop: north_east
                        // before the arriving cart's first tick in K; south_east after its last tick in
                        // K and before the released cart's first tick in K)
                        if r.layout_name == "(A) re-laid" {
                            if let Some((a0, a1)) = r.k_arrive {
                                if let Some(&(tn, _)) = r.k_shapes.iter().rev().find(|(t0, s)| *t0 <= a0 && s == "north_east") {
                                    sk.k_ne_margin.push(a0 - tn);
                                } else {
                                    sk.k_wrong.push(format!("seed {seed} speed {v}: K not north_east at the arrival (shapes {:?})", r.k_shapes));
                                }
                                if let Some(&(ts, _)) = r.k_shapes.iter().find(|(t0, s)| *t0 > a1 && s == "south_east") {
                                    sk.k_se_after.push(ts - a1);
                                    if let Some(rf) = r.k_return_first {
                                        sk.k_se_before.push(rf - ts);
                                    }
                                } else if r.k_return_first.is_some() {
                                    sk.k_wrong.push(format!("seed {seed} speed {v}: K never south_east before the return (shapes {:?})", r.k_shapes));
                                }
                            }
                            if let Some(mx) = r.park_max_x {
                                sk.park_max_x = sk.park_max_x.max(mx - r.park_cell[0] as f64);
                            }
                            if let (Some(rel), Some((_, tp))) = (r.release_at, r.pickup) {
                                sk.ride_to_p.push(tp - rel);
                            }
                            // (ironspec 22:58 (2)(a) / G1 23:00: the siding's water, both ways)
                            if let Some(s) = r.park_entry_speed {
                                sk.park_entry_speeds.push((s, format!("seed {seed} speed {v}")));
                            } else if r.armed_at.is_some() {
                                sk.park_never_entered.push(format!("seed {seed} speed {v}"));
                            }
                            if let Some(s) = r.water_out_arrive {
                                sk.water_out_arrive.push(s);
                            }
                            if let (Some(a), Some(b)) = (r.water_in_return, r.p_entry_speed) {
                                sk.water_pairs.push((a, r.water_out_return.unwrap_or(f64::NAN), b));
                            }
                        }
                        // (tick-exact 22:44: the napper's feet on P, any layout)
                        if let Some((a, b)) = r.p_feet_y {
                            sk.p_feet = Some(sk.p_feet.map_or((a, b), |(lo, hi)| (lo.min(a), hi.max(b))));
                        }
                        // (ironspec 23:00 (3): the no-HOME row's cells, per visit)
                        // (ironspec 00:27b: the stamp's arming / clearing events by kind and cell)
                        for e in &r.stamp_events {
                            let key = e.split(" (path").next().unwrap_or(e).split_once(' ').map_or(e.as_str(), |(_, rest)| rest).split_once(' ').map_or("", |(_, rest)| rest).to_string();
                            *sk.stamp_event_kinds.entry(key).or_insert(0) += 1;
                        }
                        if r.stamp_events.last().is_some_and(|e| e.contains("ARMED")) {
                            sk.stamp_end_armed += 1;
                        }
                        if sk.stamp_event_lines.len() < 4 && !r.stamp_events.is_empty() {
                            sk.stamp_event_lines.push(format!("seed {seed} speed {v}: {}", r.stamp_events.join(" | ")));
                        }
                        if let Some(e) = r.no_home_from {
                            let bed_top = r.no_home_cells.iter().any(|(_, c, y)| [c[0], c[2]] == [-3, 4] || [c[0], c[2]] == [-3, 3] || ([c[0], c[2]] == [-3, 4] && *y >= 0.5));
                            let left_p = r.no_home_cells.iter().any(|(_, c, _)| *c != [-2, 0, 4]);
                            sk.no_home_visits += 1;
                            if bed_top {
                                sk.no_home_bed_top += 1;
                            }
                            if left_p {
                                sk.no_home_left_p += 1;
                            }
                            if sk.no_home_lines.len() < 6 && (left_p || sk.no_home_lines.is_empty()) {
                                sk.no_home_lines.push(format!("seed {seed} speed {v}: erased at t {e}; cells {:?}; boarded {:?}", r.no_home_cells.iter().map(|(t0, c, y)| format!("t {t0} {c:?} y {y:.3}")).collect::<Vec<_>>(), r.pickup.map(|(_, tp)| tp)));
                            }
                        }
                        // (ironspec 21:20: the cart's last end-of-tick on the park cell >= 2 gt
                        // before PARK's end as the entities see it (the off write's tick, moved earlier
                        // by a world open's lead))
                        if let (Some(l), Some(off_t)) = (r.park_left_at, r.park_off_at) {
                            sk.park_end_margin.push(off_t - l);
                        }
                        // (G1 21:20 / ironspec 21:24 (3): the cart AT REST on the park rail by RLS + 4,
                        // judged every visit; the rest = the first of the 10 still ticks)
                        if let (Some((p, pt)), Some(rl)) = (r.parked, r.rls_at)
                            && p[0].floor() as i32 == r.park_cell[0]
                            && p[2].floor() as i32 == r.park_cell[2]
                        {
                            sk.rest_by_park.push(rl + 4 - (pt - 9));
                        } else if r.rls_at.is_some() {
                            sk.rest_by_park_missing += 1;
                        }
                        // (G1 21:20 (1): the planted step-off: PARK fired? the cart left empty?)
                        if let Some(so) = r.step_off_at
                            && sk.step_off_lines.len() < 8
                        {
                            sk.step_off_lines.push(format!("seed {seed} speed {v}: off P at t {so} (RLS t {:?}); PARK released at t {:?}, off at t {:?}; the cart off the park at t {:?}; pickup {:?}; seated out {}", r.rls_at, r.release_at, r.park_off_at, r.park_left_at, r.pickup.map(|p| p.1), r.seated_out));
                        }
                        // (ironspec 21:21 (4): the hurt at ARM + K: the boarding's window, the re-sleep,
                        // the visit's end, the stamp at boarding, HOME)
                        if base.hurt_at_arm.is_some() && sk.hurt_arm_lines.len() < 8 {
                            sk.hurt_arm_lines.push(format!("seed {seed} speed {v}: woke {:?}, re-slept {:?}, pickup {:?}, out {:?}, seated {}, the stamp's age at boarding {:?}, HOME lost at {:?}", r.woke_at, r.resleep_at, r.pickup.map(|p| p.1), r.out_at, r.seated_out, r.stamp_at_entry, r.home_lost_at));
                        }
                        // (ironspec 21:21 (3): the lie-down exclusion: P_STRING's drop - the lie-down's
                        // pulse, for a P_STRING standing at that pulse)
                        if let Some(s) = r.slept_at
                            && let Some(&pl) = r.sensor_rises.iter().find(|&&x| x >= s)
                        {
                            match r.p_spans.iter().find(|&&(a, b)| a <= pl && pl <= b) {
                                Some(&(_, b)) => sk.lie_drop.push(b - pl),
                                None => sk.lie_drop_none += 1,
                            }
                        }
                        if !braked && sk.unbraked_lines.len() < 3 {
                            sk.unbraked_lines.push(format!("seed {seed} speed {v}: rest {:?}", r.parked.map(|(p, t)| (format!("{:.2},{:.2},{:.2}", p[0], p[1], p[2]), t))));
                        }
                        sk.open_on_p_max = sk.open_on_p_max.max(r.open_on_p_max);
                        // (G1 19:26: ridden carts on the activator (-1,0,3))
                        if r.ridden_on_act > 0 {
                            sk.ridden_act_visits += 1;
                            if sk.ridden_act_lines.len() < 4 {
                                sk.ridden_act_lines.push(format!("seed {seed} speed {v}: {}", r.ridden_act_lines.join("; ")));
                            }
                        }
                        // (G1 17:40: the window's opening (the write's stamp, C's footing) against the
                        // lie-down)
                        if let (Some(o), Some(s)) = (r.open_at, r.slept_at) {
                            sk.open_after_lie.push((r.speed, o - s));
                        }
                        seated += r.seated_out as usize;
                        seated_here += r.seated_out as usize;
                        not_out += r.out_at.is_none() as usize;
                        lost += r.home_lost_at.is_some() as usize;
                        if !f.empty {
                            stop_judged += 1;
                            stop_bar += (r.slept_at.is_none() || !r.home_at_end) as usize;
                            healed += r.healed_release.is_some() as usize;
                            if let (Some(a), Some(b)) = (r.home_lost_at, r.reclaimed_at) {
                                reclaim_delays.push(b - a);
                            }
                            if r.woke_at.is_some() {
                                resleeps += r.resleep_at.is_some() as usize;
                                multi_board += (r.boardings > 1) as usize;
                                if let Some(d) = r.stand_min_home {
                                    stand_home_min = stand_home_min.min(d);
                                }
                                if let Some(l) = &r.resleep_line {
                                    resleep_lines.push(format!("seed {seed} speed {v}: {l}"));
                                }
                                if let Some((s, _)) = r.napper_contact {
                                    contact_speeds.push((s, format!("{seed}/{v}")));
                                }
                                // (ironspec 23:40 (1): pushes before seating, per visit)
                                if r.push_overlap_at.is_some() || r.push_disp > 1e-4 {
                                    sk.push_before_seat.push(format!("seed {seed} speed {v}: overlap at {:?}, displaced {:.4}", r.push_overlap_at, r.push_disp));
                                }
                                if r.napper_contact.is_some() {
                                    sk.push_judged += 1;
                                }
                                // (ironspec 23:54 (a): the y overlap at the first contact; (b) the
                                // pickup against the boarding cooldown from the eject)
                                if let Some((c0, c1, v0, v1)) = r.contact_y {
                                    let ov = c1.min(v1) - c0.max(v0);
                                    if sk.contact_y_least.as_ref().is_none_or(|(o, _)| ov < *o) {
                                        sk.contact_y_least = Some((ov, format!("seed {seed} speed {v}: cart y {c0:.4}..{c1:.4}, napper y {v0:.4}..{v1:.4}")));
                                    }
                                }
                                if let (Some(dm), Some((_, tp))) = (r.dismount_at, r.pickup) {
                                    sk.eject_to_pickup.push(tp - dm);
                                }
                                if let Some((p, _)) = r.parked {
                                    park_xs.push((p[0], r.napper_contact.map(|n| n.0), format!("{seed}/{v}")));
                                }
                                waker_wakes += 1;
                                waker_stamps += r.waker_stamp.is_some() as usize;
                                waker_uncleared += (r.waker_stamp.is_some() && r.waker_cleared.is_none()) as usize;
                                waker_flee += r.waker_flee_ticks;
                                waker_memory += r.waker_memory_ticks;
                                // (ironspec 09:04: the hold needed so the cart enters (-2,0,5) only
                                // after the memory is empty: max over wakes of (empty - entry + 1))
                                if let (Some(e), Some(c)) = (r.mem_empty_at.max(r.panic_end_at), r.cell25_at) {
                                    hold_needed = hold_needed.max(e - c + 1);
                                    window_rows.push(format!("seed {seed} speed {v}: wake {:?}, close {:?}, release {:?}, boarding {:?}, first in (-2,0,5) {c}, NEAREST_HOSTILE empty {:?}, PANIC over {:?}, R {:?}", r.woke_at, r.close_at, r.release_at, r.pickup.map(|p| p.1), r.mem_empty_at, r.panic_end_at, r.r_at));
                                }
                                // (G1 09:13 (2) / ironspec 09:14: the measured (clear - close) over
                                // every wake, asserted <= 22; ironspec 09:11 (2): the first entry
                                // into (-2,0,5) >= close + 27 on every wake: the delta it needs)
                                if let (Some(cl), Some(e)) = (r.close_at, r.mem_empty_at) {
                                    clear_after_close.push(e - cl);
                                }
                                // (G1's Q4: a wake whose NEAREST_HOSTILE never clears after the
                                // close is a failing sample, not a dropped one)
                                if r.close_at.is_some() && r.mem_empty_at.is_none() {
                                    clear_never += 1;
                                }
                                if let (Some(cl), Some(c)) = (r.close_at, r.cell25_at) {
                                    delta_ruled = delta_ruled.max(cl + 27 - c);
                                }
                                // (the boarding wait: the FIRST wake to the pickup (G1's D6: the merged
                                // wake, every visit); ironspec 23:00 (2) / G1 23:02: each visit classed
                                // by that wake's tick against COOL: a pre-COOL wake is FORCED (board in
                                // the first window AND <= wake + 90), else NATURAL (the stranded bound))
                                let first_wake = r.wakes.first().copied().or(r.woke_at);
                                if let Some(w0) = first_wake {
                                    let cool = base.adopted.map(|ad| ad.parked);
                                    let forced = matches!((cool, r.armed_at), (Some(c), Some(a)) if w0 < a + c);
                                    let boarded = r.pickup.map(|(_, tp)| tp);
                                    let resleep_first = r.resleep_at.is_some_and(|rs| boarded.is_none_or(|tp| rs < tp));
                                    if let Some(tp) = boarded {
                                        board_wait_max = board_wait_max.max(tp - w0);
                                        if forced {
                                            sk.forced_waits.push(tp - w0);
                                        } else {
                                            sk.natural_waits.push(tp - w0);
                                        }
                                    }
                                    if forced {
                                        sk.forced_visits += 1;
                                        if resleep_first || boarded.is_none_or(|tp| tp - w0 > 90) {
                                            sk.forced_fail.push(format!("seed {seed} speed {v}: the forced wake at t {w0} (ARM {:?}): boarded {:?}, re-slept first {resleep_first}", r.armed_at, boarded));
                                        }
                                    } else {
                                        sk.natural_visits += 1;
                                        if boarded.is_none() {
                                            sk.natural_fail.push(format!("seed {seed} speed {v}: the natural wake at t {w0}: never boarded"));
                                        }
                                    }
                                }
                                if let Some(d) = r.waker_dist_min {
                                    waker_dmin = waker_dmin.min(d);
                                }
                                if let Some(d) = r.waker_seen_dist_min {
                                    waker_seen_dmin = waker_seen_dmin.min(d);
                                }
                            }
                            // (ironspec 07:03 (3): the declared contact's least post-contact speed per
                            // entry, and every cart at the exit booster)
                            if let Some(s) = r.post_contact_speed {
                                let e = post_contact_min.entry(format!("{v:.3}")).or_insert(f64::MAX);
                                *e = e.min(s);
                            }
                            if r.pickup.is_some() {
                                picked_up += 1;
                                missed_booster += r.booster_at.is_none() as usize;
                            }
                        }
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
                        if let Some(s) = r.pickup_step {
                            min_step = min_step.min(s);
                            max_step = max_step.max(s);
                            if s > 0.2 + 1e-9 {
                                steps_over += 1;
                            }
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
                        if let (Some(q), Some(rr)) = (r.queue_release_at, r.r_at) {
                            sk.qrel_to_r.push(rr - q);
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
                            for (_, _, k, cell) in &r.contacts {
                                *sk.contacts.entry(format!("{k} with the block at {},{},{}", cell[0], cell[1], cell[2])).or_default() += 1;
                            }
                        }
                        for d in &r.declared_hit {
                            sk.declared_hit.insert(*d);
                        }
                        // (ironspec 09:38 / 09:40: the exit-slope hold per visit)
                        if hold_design && r.pickup.is_some() {
                            sk.hold_picked += 1;
                            match (r.hold_rest_at, r.hold_depart_at) {
                                (Some(a), Some(d)) => {
                                    sk.hold_rested += 1;
                                    sk.hold_spans.push(d - a);
                                    // (the launch the cart waits for: the tap, or with a PERIOD the
                                    // train's first pulse at or after its rest (G1's full-period row))
                                    let due = match base.tap {
                                        Some((t0, _, Some(p))) if p > 0 => Some(t0 + (a - t0).div_euclid(p) * p + if (a - t0).rem_euclid(p) == 0 { 0 } else { p }),
                                        // (G1 04:08a item 5: on the laid latch the launch is Q's
                                        // own tap at the next visit's RLS)
                                        _ => tap_at.or(base.latch_next_rls),
                                    };
                                    if due.is_some_and(|tp| d < tp) {
                                        sk.hold_early += 1;
                                    }
                                    if let Some(rr) = r.r_at {
                                        sk.hold_to_r.push(rr - d);
                                    }
                                }
                                (Some(_), None) => sk.hold_rested += 1,
                                _ => {}
                            }
                            sk.hold_creep = sk.hold_creep.max(r.hold_creep);
                            sk.hold_hostile += r.hold_hostile_ticks;
                            sk.hold_arrival_powered += r.hold_arrival_powered as usize;
                            sk.hold_sight_clear += r.hold_sight_clear as usize;
                            sk.hold_stamped += r.hold_stamp_at_depart.is_some_and(|s| s.is_some()) as usize;
                            for (cell, s) in &r.launch_cells {
                                let e = sk.launch.entry(format!("{},{},{}", cell[0], cell[1], cell[2])).or_insert((f64::MAX, 0f64));
                                e.0 = e.0.min(*s);
                                e.1 = e.1.max(*s);
                            }
                            if one || sk.hold_lines_shown < 6 {
                                for l in &r.hold_lines {
                                    println!("  hold seed {seed} speed {v}: {l}");
                                    sk.hold_lines_shown += 1;
                                }
                            }
                        } else if hold_design && r.hold_depart_at.is_some() {
                            // (ironspec 20:45 (2): an EMPTY departure's cart (no pickup) launched
                            // from the hold: its speed per cell to R, beside the ridden band)
                            for (cell, s) in &r.launch_cells {
                                let e = sk.launch_empty.entry(format!("{},{},{}", cell[0], cell[1], cell[2])).or_insert((f64::MAX, 0f64));
                                e.0 = e.0.min(*s);
                                e.1 = e.1.max(*s);
                            }
                        }
                        sk.stamp_visit_max = sk.stamp_visit_max.max(r.stamp_visit_max);
                        sk.stand_max = sk.stand_max.max(r.stand_after_eject);
                        sk.head_box_hits += r.head_box_hits;
                        sk.head_eye_hits += r.head_eye_hits;
                        if r.stand_after_eject > 20 {
                            sk.stand_over_20 += 1;
                        }
                        sk.integrity_breaks += r.integrity_breaks;
                        if !sk.p_eye_shown && !r.p_eye_lines.is_empty() {
                            sk.p_eye_shown = true;
                            for l in &r.p_eye_lines {
                                println!("{l}");
                            }
                        }
                        // (the emulated DAY-1 latch's per-visit outcomes; the park -> P ride)
                        sk.latch_timeouts += r.latch_timeout as usize;
                        sk.released_asleep += r.released_asleep as usize;
                        sk.bed_occupied_end += r.bed_occupied_at_end as usize;
                        if let (Some(rel), Some((_, tp))) = (r.release_at, r.pickup) {
                            sk.release_to_board.push(tp - rel);
                        }
                        // (G1's C2 on bfa0c8098: the adopted controller's per-visit latch lines too (the
                        // press, the wake's pulse, SINCE's onset, RLS, PARK), every visit of a hurt row)
                        if (base.latch.is_some() || base.adopted.is_some()) && (one || sk.latch_lines_shown < 4 || base.hurt_after_lie.is_some() || base.hurt_at_arm.is_some()) {
                            for l in &r.latch_lines {
                                println!("  latch seed {seed} speed {v}: {l}");
                            }
                            sk.latch_lines_shown += 1;
                        }
                        // (the wake sensor: its first rise after each napper edge, the missed edges
                        // and the rises with no edge before them, within 10 gt)
                        if r.sensor_present {
                            sk.sensor_seen = true;
                            let edges: Vec<(i64, bool)> = r.sleeps.iter().map(|&s| (s, true)).chain(r.wakes.iter().map(|&w| (w, false))).collect();
                            for &(e, lie) in &edges {
                                match r.sensor_rises.iter().find(|&&x| x >= e && x <= e + SENSOR_WINDOW) {
                                    Some(&x) if lie => sk.sensor_after_sleep.push(x - e),
                                    Some(&x) => sk.sensor_after_wake.push(x - e),
                                    None => sk.sensor_missed += 1,
                                }
                            }
                            for &x in &r.sensor_rises {
                                if !edges.iter().any(|&(e, _)| x >= e && x <= e + SENSOR_WINDOW) {
                                    sk.sensor_spurious += 1;
                                }
                            }
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
                            // (day-1 12:09: its SLEPT latch takes the first wake-sensor pulse after
                            // ARM's edge as the lie-down: the arming edge to the lie-down per visit)
                            if let Some(a) = r.armed_at {
                                sk.arm_to_lie.push(s - a);
                            }
                        }
                        // (day-1 12:22: its TIMEOUT T_max is sized from ARM -> wake)
                        if let (Some(a), Some(wk)) = (r.armed_at, r.woke_at) {
                            sk.arm_to_wake.push(wk - a);
                        }
                        // (G1 13:26: T_open sized from the window's opening to the wake)
                        if let (Some(o), Some(wk)) = (r.open_at, r.woke_at) {
                            sk.open_to_wake.push(wk - o);
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
                        // (ironspec 23:34 / G1 23:27: the west bed sensor's condition, an empty
                        // diff of the observer pulse ticks per visit, under vs west)
                        if base.pulse_log {
                            println!("pulse-log seed {seed} speed {v}: sensor rises {:?}; wakes {:?}; lie-downs {:?}", r.sensor_rises, r.wakes, r.sleeps);
                        }
                        if one {
                            println!("seed {seed} speed {v}: landed {:?}, parked {:?}, slept {:?}, woke {:?}, release {:?}, pickup {:?} at {:?} b/t (the cart's step that tick {:?}), R {:?}, out {:?}, seated {}{}", r.landed, r.parked.map(|(p, t)| (format!("{:.2},{:.2},{:.2}", p[0], p[1], p[2]), t)), r.slept_at, r.woke_at, r.release_at, r.pickup, r.pickup_speed.map(|s| format!("{s:.3}")), r.pickup_step.map(|s| format!("{s:.3}")), r.r_at, r.out_at, r.seated_out, if r.problems.is_empty() { String::new() } else { format!(" PROBLEMS {:?}", r.problems) });
                            for m in &r.pulses {
                                println!("  pulse {m}");
                            }
                            for m in &r.waker_lines {
                                println!("  waker {m}");
                            }
                            for m in &r.declared_contact_lines {
                                println!("  contact {m}");
                            }
                        }
                        if r.problems.is_empty() {
                            ok += 1;
                        } else {
                            bad.push(format!("seed {seed} speed {v}: {:?}", r.problems));
                        }
                        // (day-1 10:49: one line per visit, the hold lead's data)
                        if base.visit_table {
                            println!("visit\tseed {seed}\tentry {v}\tarmed {:?}\tslept {:?}\twoke {:?}\tARM->wake {:?}\tclose {:?}\trelease {:?}\tpickup {:?}\trest on the slope {:?}\tdeparted the slope {:?}\tR {:?}\twait on the slope {:?}", r.armed_at, r.slept_at, r.woke_at, r.armed_at.zip(r.woke_at).map(|(a, w)| w - a), r.close_at, r.release_at, r.pickup.map(|p| p.1), r.hold_rest_at, r.hold_depart_at, r.r_at, r.hold_rest_at.zip(r.hold_depart_at).map(|(a, d)| d - a));
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
    // (G1's CHANGES 8 B1: the stand-in cells named from the layout the runs read)
    println!("the bench's stand-in cells (the layout's own, written by the emulation; never in the record): {:?}{}", sk.ctl_cells, sk.relaid_park.map_or(String::new(), |p| format!("; the re-laid stop: the park ({},{},{}), its string ({},1,{}), K's drive stand-in {:?}", p[0], p[1], p[2], p[0], p[2], K_CTRL)));
    // (ironspec 01:17 (2): the laid latch's PARK, in the standing start-of-tick frame (tick-exact's
    // latch-formb rig times one tick earlier, in the RLS-command frame))
    if !sk.planted.is_empty() {
        println!("row 39's fault record (ironspec 02:16: an empty cart left on the siding; the symptom a cart sitting on the siding and the stop no longer cycling; the repair: remove the empty cart, relink any HOME lost), {} visits:", sk.planted.len());
        for l in &sk.planted {
            println!("  planted {l}");
        }
    }
    // (G1 02:27 (3) / ironspec 02:28 (4): the park string's flicker and its consumers, per rest
    // position: every consumer reads SENSED's latch (its first press), never a re-check edge)
    let mut flicker_bad = 0usize;
    let mut flicker_not_judged = false;
    let mut latch_window_fail: Option<String> = None;
    let mut stand_up_bad = 0usize;
    // (vcompose 03:19: the stand-up positions the stop leaves a villager at, exact feet, against
    // the header's `stand-up:` declaration)
    if !sk.stand_ups.is_empty() {
        println!("the stand-up feet positions (vcompose 03:19, the flee static's walk starts; stop-local, exact): {:?}", sk.stand_ups);
        let declared: Vec<[f64; 3]> = base.stand_up_decl.clone();
        if !declared.is_empty() {
            let undeclared: Vec<&String> = sk.stand_ups.keys().filter(|k| {
                let c = k.rsplit(' ').next().unwrap_or("");
                let v: Vec<f64> = c.split(',').filter_map(|x| x.parse::<f64>().ok()).collect();
                // (the tolerance 0.01: within it the villager's 0.6-wide box stays inside the same
                // cell, so getStart picks the same start node; the eject's landing varies in the
                // fourth decimal with the cart's approach)
                !declared.iter().any(|d| v.len() == 3 && (0..3).all(|i| (d[i] - v[i]).abs() < 0.01))
            }).collect();
            println!("the header's stand-up: {declared:?}; measured positions not declared: {undeclared:?}");
            stand_up_bad = undeclared.len();
        }
    }
    // (tick-exact 02:45: the park string's re-check releases of the lowest rests, against its
    // game cadence of one released tick every 11 gt)
    for l in &sk.flicker_ticks {
        println!("  the park string while parked, {l}");
    }
    // (G1 02:47: the park's arrival speeds against the stranding threshold: tick-exact's game rig
    // from the siding's centre, 0.05 strands at x 0.73 and 0.10 reaches the park)
    if !sk.siding_speeds.is_empty() {
        let mut ss = sk.siding_speeds.clone();
        ss.sort_by(|a, b| a.0.total_cmp(&b.0));
        let lo = ss[0];
        let hi = ss[ss.len() - 1];
        println!("the empty cart's arrival toward the park (G1 02:47: its speed crossing the siding's centre x -0.5 eastward; tick-exact's rig: 0.05 strands at x 0.73, 0.10 reaches x 1.037): {:.4}..{:.4} over {} visits (the least at seed {} entry {}), margin {:+.4} over the least known to reach (0.10), {:+.4} over the known strand (0.05)", lo.0, hi.0, ss.len(), lo.1, lo.2, lo.0 - 0.10, lo.0 - 0.05);
        // (ironspec 03:56 / 04:00: the rests in that band, each with its arrival speeds; rests below
        // x 1.30 get the string's one-tick release every 11 gt, from 1.30 it is pressed steadily)
        let rests: Vec<String> = sk.rest_arrivals.iter().map(|(k, (a, b))| format!("rest x {:.2}..{:.2}{}: arrivals {a:.4}..{b:.4}", *k as f64 / 20.0, (*k + 1) as f64 / 20.0, if (*k as f64 + 1.0) / 20.0 <= 1.30 + 1e-9 { " (flickers)" } else { "" })).collect();
        println!("  the rests in the arrival band (rest not monotonic in speed, ironspec 03:56): {}", rests.join("; "));
        // (G1 04:24c (3): redsim's braked-slope rests diverge from the game's (tick-exact: 0.108
        // further up in the game on a slot): redsim's rest per arrival speed beside the game's, tick-
        // exact's car 7 (a/1406316148191627, arrivals launched from the siding's centre x -0.5); a
        // visit's flicker class (below / from x 1.30) can change with the game's rest)
        const GAME_REST: [(f64, f64); 8] = [(0.05, 0.73), (0.10, 1.037), (0.15, 1.083), (0.20, 1.127), (0.25, 1.179), (0.30, 1.303), (0.35, 1.332), (0.40, 1.2725)];
        let cmp: Vec<String> = sk.rest_by_speed.iter().map(|(k, (lo, hi, n))| {
            let s = *k as f64 / 20.0;
            let game = GAME_REST.iter().find(|g| (g.0 - s).abs() < 1e-9).map_or(if s >= 0.45 - 1e-9 { "1.396..1.520 (0.45..0.60)".to_string() } else { "-".to_string() }, |g| format!("{:.4}{}", g.1, if g.1 < 1.0 { " (short of the park)" } else if g.1 < 1.30 { " (flickers)" } else { " (steady)" }));
            format!("arrival {s:.2}: redsim rest {lo:.3}..{hi:.3} ({n} visits{}), the game's {game}", if *hi < 1.30 { ", flickers" } else if *lo >= 1.30 { ", steady" } else { ", both classes" })
        }).collect();
        println!("  redsim vs the game's park rests per arrival speed (G1 04:24c (3); the game from tick-exact's car 7; single-cart braked rests match the game, cc767b78's regression check 1.862987 vs 1.863, G1 04:36 (2)): {}", cmp.join("; "));
    }
    if !sk.flicker.is_empty() {
        let mut by_x: std::collections::BTreeMap<i64, (usize, usize, usize, usize, usize, usize)> = std::collections::BTreeMap::new();
        let latch_rows = sk.flicker.iter().filter(|f| f.5).count();
        for &(x, rel, koff, pk, rises, latch) in &sk.flicker {
            let key = x.map_or(-1, |x| (x * 20.0).floor() as i64);
            let e = by_x.entry(key).or_insert((0, 0, 0, 0, 0, 0));
            e.0 += 1;
            e.1 += usize::from(rel > 0);
            e.2 = e.2.max(rel);
            e.3 += usize::from(koff != 1);
            e.4 += usize::from(!pk);
            // (G1 03:46 B1: the laid latch's park lightings judged != 1 (none or a second))
            e.5 += usize::from(latch && rises != 1);
        }
        let rows: Vec<String> = by_x.iter().map(|(k, e)| format!("rest x {}: {} visits, {} with re-check releases (most {} per visit), K resets != 1 in {}, PARKED unset in {}, park lightings != 1 in {}", if *k < 0 { "-".to_string() } else { format!("{:.2}..{:.2}", *k as f64 / 20.0, (*k + 1) as f64 / 20.0) }, e.0, e.1, e.2, e.3, e.4, if latch_rows > 0 { e.5.to_string() } else { "N/A (a stand-in PARK, not the laid latch)".to_string() })).collect();
        let bad: usize = by_x.values().map(|e| e.3 + e.4 + e.5).sum();
        println!("the park string's flicker while parked (G1 02:27 (3): its re-check releases between SENSED's first press and the release; K's reset and PARKED read {}; the rests are single-cart braked rests, which match the game, G1 04:36 (2)): {}; consumer faults {bad} (bar 0: one K reset, PARKED set, one park lighting per visit on the laid latch)", if base.consumers_raw { "the RAW string (`--consumers-raw`, G1 03:46 B1's twin: it must fail by name in the flickering visits)" } else { "the latch" }, rows.join("; "));
        flicker_bad = bad;
    } else if sk.relaid_park.is_some() {
        // (G1 03:46: an empty flicker list on a re-laid run is NOT JUDGED, never a pass)
        println!("the park string's flicker: NOT JUDGED (no visit rested on the park with SENSED pressed)");
        flicker_not_judged = true;
    }
    if !sk.latch_lit.is_empty() {
        let lits: Vec<i64> = sk.latch_lit.iter().map(|x| x.0).collect();
        let darks: Vec<i64> = sk.latch_lit.iter().filter_map(|x| x.1).collect();
        let open = sk.latch_lit.iter().filter(|x| x.1.is_none()).count();
        println!("the laid latch's PARK on the park rail (the standing start-of-tick frame; the rig's RLS-command frame is one tick earlier): lit from RLS + {}..{}, dark from RLS + {}..{} over {} visits (expected [RLS + 2, RLS + 26)); still lit at the visit's end {open}", lits.iter().min().unwrap(), lits.iter().max().unwrap(), darks.iter().min().map_or("-".to_string(), |v| v.to_string()), darks.iter().max().map_or("-".to_string(), |v| v.to_string()), sk.latch_lit.len());
        // (G1 04:08a item 4: the laid latch's PARK window JUDGED: lit at RLS + 2 or later, dark by
        // RLS + 26, never lit at the visit's end, no lighting before RLS, every RLS lights it)
        let early = lits.iter().filter(|&&l| l < 2).count();
        let late_dark = darks.iter().filter(|&&d| d > 26).count();
        if early + late_dark + open + sk.latch_rises_before + sk.latch_unlit.len() > 0 {
            latch_window_fail = Some(format!("the laid latch's PARK window off [RLS + 2, RLS + 26): lit before RLS + 2 in {early}, dark after RLS + 26 in {late_dark}, still lit at the visit's end in {open}, lit before RLS {} times, RLS with no lighting in {} ({:?}) (G1 04:08a item 4)", sk.latch_rises_before, sk.latch_unlit.len(), sk.latch_unlit.iter().take(3).collect::<Vec<_>>()));
        }
    } else if sk.latch_visits > 0 {
        latch_window_fail = Some(format!("the laid latch's PARK never lit on {} latch visits ({:?}) (G1 04:08a item 4)", sk.latch_visits, sk.latch_unlit.iter().take(3).collect::<Vec<_>>()));
    }
    // (ironspec 01:50b / 02:33 (2): the timing row's margin on the laid latch: PARK's dark tick
    // against the cart's last tick on the park cell, the worst over every row (world-open phases))
    if !sk.latch_margin.is_empty() {
        let worst = sk.latch_margin.iter().min_by_key(|m| m.0).unwrap();
        let best = sk.latch_margin.iter().max_by_key(|m| m.0).unwrap();
        println!("the laid PARK's dark tick against the cart leaving the park cell (ironspec 01:50b, the timing row's margin; measured with any world open's frozen entity ticks inside it): {}..{} gt over {} visits; the worst at {}", worst.0, best.0, sk.latch_margin.len(), worst.1);
    }
    println!("bed stop bench: {tot} visits ({} rows), {clean} clean, seated {seated}; not out {not_out}; HOME lost {lost}; pickup speeds {:.3}..{:.3}; exit later than the night's (gt): min {:?}, median {:?}, p99 {:?}, max {:?}; after a restart, visits with a path off the pickup cell by direction {dirs:?}; after the wake in any row {dirs_wake:?}; lie-downs' reach to the bed head's centre {:.4}..{:.4} (SleepInBed's bar 2)", rows.len(), if min_pick == f64::MAX { 0.0 } else { min_pick }, max_pick, delays.first(), pct(0.5), pct(0.99), delays.last(), if lie_min == f64::MAX { 0.0 } else { lie_min }, lie_max);
    if max_step > 0.0 {
        println!("the pickups' steps (the cart's horizontal position change on the pickup tick: the 0.2-step clip's quantity; the velocity above may be higher where the move is clamped, in water to 0.2 per axis): {:.3}..{max_step:.3}; over 0.2: {steps_over}", if min_step == f64::MAX { 0.0 } else { min_step });
    }
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
    println!("{} (the stop clear for the next napper): {:?} / median {:?} / {:?} gt over {} visits; outcome classes (ironspec 09:45: a slept and picked up, b waited through IDLE then napped, c the abort's unslept return): {:?}", if f_approach { "the spawn (t 0, NOT the queue release under --queue-wait / the dusk gate: that column is below) to R (G1's C4 / Q11: approach mode's raw count; entry -> R is the per-speed line)" } else { "entry to R" }, sk.r_ats.first(), sk.r_ats.get(sk.r_ats.len() / 2), sk.r_ats.last(), sk.r_ats.len(), sk.classes);
    if f_approach && !sk.qrel_to_r.is_empty() {
        let mut v = sk.qrel_to_r.clone();
        v.sort();
        println!("the queue's release (the write's tick boundary, G1's Q10) to R (G1's Q11): {:?} / median {:?} / {:?} gt over {} visits", v.first(), v.get(v.len() / 2), v.last(), v.len());
    }
    // (ironspec 19:18 (2a): entry to R per entry speed, and v_slow against the timing bar)
    let mut speed_failed = false;
    // (G1's C2: every run, one speed or many, checks that every visit reached R)
    if !sk.visits_by_speed.is_empty() {
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
        "the carts' contacts with blocks, the spawn to out (main's shared contacts::cart_contacts (G1's Q1): the clipped axes of each recorded SELF move, keyed on the CONTACTED block's cell; declared in the design's `# contacts:`: {:?}; an undeclared one fails its row): {} visits with a contact; by contacted cell (contact ticks) {:?}; declared cells met by some visit {:?}",
        base.declared_contacts, sk.contact_visits, sk.contacts, sk.declared_hit
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
    if flicker_bad > 0 {
        failed.push(format!("the park string's flicker reached a consumer in {flicker_bad} visit(s) (G1 02:27 (3))"));
    }
    if flicker_not_judged {
        failed.push("the park string's flicker NOT JUDGED (G1 03:46)".into());
    }
    if let Some(m) = latch_window_fail.take() {
        failed.push(m);
    }
    if stand_up_bad > 0 {
        failed.push(format!("{stand_up_bad} measured stand-up position(s) missing from the header's stand-up: (vcompose 03:19)"));
    }
    // (the declared scope: G1's C7, only the named bars, validated at the start)
    let excluded = |bar: &str| base.scope_excludes.iter().any(|b| b == bar);
    // (G1's C20: lightning judged on the built world; on the stop alone it is printed only, the
    // stop being open to the sky by construction there)
    if base.built_world && sk.lightning_visits > 0 {
        failed.push(format!("{} visits with a napper exposed to lightning in the built world (a sky-seeing cell or a bolt's hit box)", sk.lightning_visits));
    }
    // (G1's Q13: the integrity shapes, judged every tick in every visit)
    // (the park -> P ride: the release to the boarding, per visit (day-1's D_pick))
    if !sk.release_to_board.is_empty() {
        let mut v = sk.release_to_board.clone();
        v.sort();
        println!("the park's release to the boarding at P (day-1's D_pick): {:?}..{:?} gt over {} visits (median {:?})", v.first(), v.last(), v.len(), v.get(v.len() / 2));
    }
    // (the emulated DAY-1 latch's normal-operation bars, ironspec 12:38: 0 releases while asleep,
    // 0 re-sleeps (the re-sleep bar below), the bed free for the next napper)
    if let Some(l) = base.latch {
        println!("(superseded latch, before 15:37) DAY-1's latch (emulated on the wake sensor's pulses: TIMEOUT ARM + {}, the shutter OPEN's fall + {} gt, the release OPEN's fall + {} gt): TIMEOUT releases {}; releases with the napper asleep {} (bar 0); visits ending with the bed occupied {} (bar 0)", l.t_max, l.logic_lat, l.rel_after, sk.latch_timeouts, sk.released_asleep, sk.bed_occupied_end);
        if sk.released_asleep > 0 {
            failed.push(format!("{} releases with the napper asleep (the latch)", sk.released_asleep));
        }
        if sk.bed_occupied_end > 0 {
            failed.push(format!("{} visits ending with the bed occupied (not free for the next napper)", sk.bed_occupied_end));
        }
        if base.hurt_after_lie.is_none() && sk.latch_timeouts > 0 {
            failed.push(format!("{} visits released by TIMEOUT in normal operation (bar 0)", sk.latch_timeouts));
        }
    }
    // (G1's B1: the adopted controller over the run: its close, RLS and park against the wake (DAY-1's
    // ctl-shutter, designed, 17:24: + 8, + 20, + 22); every slept visit opens the window and releases
    // by RLS, never with the napper asleep, the bed free at the end; G1 17:33: the park's rest against
    // PARKED's timer (ARM + PARKED), the band margin per visit and per entry speed)
    if let Some(ad) = base.adopted {
        let mm = |v: &[i64]| if v.is_empty() { "none".to_string() } else { format!("{}..{}", v.iter().min().unwrap(), v.iter().max().unwrap()) };
        println!("the adopted controller (DAY-1's gate list rev 20:00 at this run's terms (above), emulated; P_STRING a box-overlap stand-in, releasing at + 10, unless a laid string is read): the window shut at the wake + {} gt; RLS at the wake + {}; the park released at the wake + {} (ctl-shutter, designed: + 8, + 20, + 22 at p = 2; C here is the closing write's stamp, t - 1 (ironspec 20:55 on G1's B2); the shutter's first shut end-of-tick at the wake + {})", mm(&sk.ad_close), mm(&sk.ad_rls), mm(&sk.ad_park), mm(&sk.ad_close_state));
        println!("the adopted controller's visits: slept visits with no OPEN {}, no RLS {}; releases with the napper asleep {} (bar 0); visits ending with the bed occupied {} (bar 0)", sk.ad_no_open, sk.ad_no_rls, sk.released_asleep, sk.bed_occupied_end);
        if ad.parked > 0 {
            let margins: Vec<i64> = sk.park_rest.iter().map(|&(_, r)| ad.parked - r).collect();
            let mut per_speed: std::collections::BTreeMap<String, i64> = Default::default();
            for &(v, r) in &sk.park_rest {
                let e = per_speed.entry(format!("{v:.4}")).or_insert(i64::MAX);
                *e = (*e).min(ad.parked - r);
            }
            println!("the park's rest against PARKED's timer (G1 17:33: the COOL term is ARM + {} whether or not the cart rests; the rest = the first of the cart's 10 still ticks in the park): the rest at ARM + {} over {} visits, the band margin (ARM + {} - the rest) {}; visits resting after the timer (the fault: a cart held unreleased if it rests later) {} (bar 0, G1's C4); least margin per entry speed {:?}", ad.parked, mm(&sk.park_rest.iter().map(|&(_, r)| r).collect::<Vec<_>>()), sk.park_rest.len(), ad.parked, mm(&margins), margins.iter().filter(|&&m| m < 0).count(), per_speed);
            let late = margins.iter().filter(|&&m| m < 0).count();
            if late > 0 {
                failed.push(format!("{late} carts rested in the park after COOL's timer (ARM + {}): G1's C4", ad.parked));
            }
        }
    }
    // (ironspec 17:32 (3) / tick-exact 17:39: the boarding cooldown, 60 gt from the eject's dismount;
    // the cart's first tick in P after the release less the cooldown's end, bar >= 2 after k + j:
    // k the world-open term (1 on the convention placement, 3 off it), j 0 (the stop in one chunk))
    {
        let on_conv = base.offset[0].rem_euclid(16) == 6 && base.offset[2].rem_euclid(16) == 5;
        let k = if on_conv { 1 } else { 3 };
        let ms: Vec<i64> = sk.board_cd.iter().map(|&(_, m)| m).collect();
        let mut per_speed: std::collections::BTreeMap<String, i64> = Default::default();
        for &(v, m) in &sk.board_cd {
            let e = per_speed.entry(format!("{v:.4}")).or_insert(i64::MAX);
            *e = (*e).min(m);
        }
        let least = ms.iter().min().copied();
        let mm2 = |v: &[i64]| if v.is_empty() { "none".to_string() } else { format!("{}..{}", v.iter().min().unwrap(), v.iter().max().unwrap()) };
        // (ironspec 17:41: COOL = the eject's latest tick after ARM + 60 + k + 2 - p - the fastest
        // park -> P ride; its inputs measured here)
        let p_term = base.adopted.map_or(0, |a| a.park);
        let cool = match (sk.eject_after_arm.iter().max(), sk.park_to_p.iter().min()) {
            (Some(e), Some(r)) => Some(e + 60 + k + 2 - p_term - r),
            _ => None,
        };
        println!("COOL's inputs (ironspec 17:41: COOL = the eject's latest tick after ARM + 60 + k + 2 - p - the fastest park -> P ride): the eject at ARM + {} over {} visits; the park's release -> the cart in P {} gt; k {k}, p {p_term}: COOL = ARM + {} (the bench's PARKED term: {})", mm2(&sk.eject_after_arm), sk.eject_after_arm.len(), mm2(&sk.park_to_p), cool.map_or("none".to_string(), |c| c.to_string()), base.adopted.map_or("no adopted controller".to_string(), |a| format!("ARM + {}", a.parked)));
        if !sk.pass_to_rest.is_empty() {
            let mut per_speed: std::collections::BTreeMap<String, (i64, i64)> = Default::default();
            for &(v, d) in &sk.pass_to_rest {
                let e = per_speed.entry(format!("{v:.4}")).or_insert((i64::MAX, i64::MIN));
                e.0 = e.0.min(d);
                e.1 = e.1.max(d);
            }
            let cell = sk.relaid_park.map(|p| [p[0], 1, p[2]]).unwrap_or_else(|| base.parked_sense.map_or(PARK_SENSE, |(c, _)| c));
            let worst = sk.pass_to_rest.iter().map(|&(_, d)| d).max().unwrap_or(0);
            println!("the sensed park (ironspec 17:39 / 17:51 (c)): {}: the cart's pass of it -> its rest in the park {} gt over {} visits; per entry speed {:?}; SENSED's delay d {} against the worst pass -> rest + 2 = {} (bar: d >= it, G1's C4)", if sk.sense_stand_in_visits > 0 { format!("the cell {cell:?} holds no detector rail on {} visits: read by the STAND-IN (the cart in the cell)", sk.sense_stand_in_visits) } else { format!("read from the real detector rail at {cell:?}") }, mm2(&sk.pass_to_rest.iter().map(|&(_, d)| d).collect::<Vec<_>>()), sk.pass_to_rest.len(), per_speed, base.parked_sense.map_or("none".to_string(), |(_, dl)| dl.to_string()), worst + 2);
            if let Some((_, dl)) = base.parked_sense
                && dl < worst + 2
            {
                failed.push(format!("SENSED's delay d {dl} under the worst pass -> rest {worst} + 2 (ironspec 17:51 (c), G1's C4)"));
            }
        } else if base.adopted.is_some() && base.parked_sense.is_some() {
            // (G1's CHANGES 8 C4: no samples is NOT JUDGED, never a pass)
            failed.push("SENSED's d bar: NOT JUDGED (0 press -> rest samples)".to_string());
        }
        if base.adopted.is_some() && base.parked_sense.is_some() {
            println!("SENSED's binding (G1's CHANGES 8 C4: PARKED = max(the press + d, COOL), computed per visit): binds on {} visits, PARKED past COOL by {} gt; PARKED ticks unlike the expected {} {:?}", sk.sensed_binds.len(), if sk.sensed_binds.is_empty() { "none".to_string() } else { format!("{}..{}", sk.sensed_binds.iter().min().unwrap(), sk.sensed_binds.iter().max().unwrap()) }, sk.sensed_bind_wrong.len(), sk.sensed_bind_wrong.iter().take(3).collect::<Vec<_>>());
            if !sk.sensed_bind_wrong.is_empty() {
                failed.push(format!("{} visits' PARKED unlike max(press + d, COOL) (G1's C4)", sk.sensed_bind_wrong.len()));
            }
        }
        // (G1 04:08a item 8: on the re-laid stop the departing cart's re-presses are by design (SENSED's
        // SET is ARMED-gated), so the count is not a bar there; on the sensed-park fold it still fails)
        let relaid_run = sk.relaid_park.is_some();
        println!("the sensed park's presses (G1 17:49 (2)(a): only the arriving empty cart may press it): visits by presses {:?}; presses after RLS (or the release) {} ({})", sk.sense_presses, sk.sense_after_rls, if relaid_run { "not a bar on the re-laid stop: the departing cart re-presses the park string by design" } else { "bar 0" });
        if sk.sense_after_rls > 0 && !relaid_run {
            failed.push(format!("{} presses of the sensed park after RLS (G1 17:49 (2)(a): only the arriving empty cart may press it)", sk.sense_after_rls));
        }
        if sk.sense_after_rls > 0 {
            // (ironspec 23:40 (2) / G1 23:38 (3): on the out-and-back siding the departing cart presses
            // the park string again; the emulation's SENSED counts a press only between ARM and the
            // release (SET gated by ARMED), so these presses set nothing; day-1's netlist carries the
            // gate (or ARM's edge as SENSED's reset) and benches both)
            println!("the departing cart's re-presses of the park string after the release: {} visits; SENSED re-set after the release by them: 0 (the emulation's SET gated by ARMED; day-1's netlist carries the gate or ARM's edge reset)", sk.sense_after_rls);
        }
        println!("the golem guards' string cells (-1,1,2) / (-1,1,5) (G1 19:10: unreachable by walking; declared `unreachable:`): the napper's feet in them on {} visits, {} ticks (bar 0)", sk.guard_cell_visits, sk.guard_cell_ticks);
        // (rev 20:00 G20 / G1's N5: the window open on a napper standing awake on P, its longest run
        // of end-of-ticks, bar <= 9 (the close bar 8 + the press's own tick))
        println!("the window open on a napper awake on P (rev 20:00 G20, G1's N5): the longest run {} gt (bar <= 9)", sk.open_on_p_max);
        if sk.open_on_p_max > 9 && base.adopted.is_some() {
            failed.push(format!("the window stayed open {} gt on a napper standing awake on P (rev 20:00 G20: <= 9)", sk.open_on_p_max));
        }
        // (ironspec 21:10 (3): the power-up row: the park drive ON from the first tick; the first
        // arrival's brake per visit)
        if let Some(keep) = base.park_initial_on {
            println!("the power-up row (ironspec 21:10 (3); P_HOLD come up SET, the park drive ON from the first tick, {}): the first arrival braked in the park (1,0,4) on {} of {tot} visits; unbraked {:?}", if keep { "KEPT on (the control: no reset)" } else { "cleared at ARM's edge (DAY-1 21:10's reset, emulated)" }, sk.first_braked, sk.unbraked_lines);
            if !keep && sk.first_braked < tot {
                failed.push(format!("the power-up row: {} of {tot} first arrivals unbraked with the ARM-edge reset", tot - sk.first_braked));
            }
        }
        if base.adopted.is_some() {
            println!("ARM's edge against P_HOLD's stretch (ironspec 21:11; the stretch RLS + 4 .. RLS + 12, ARM's edge resets P_HOLD at ARM + 4): this visit's ARM edge precedes RLS by at least {:?} gt (the next visit's ARM comes ~600 gt after RLS, the station's period)", sk.arm_to_stretch);
            // (the re-laid stop: K's junction bars (ironspec 21:51 (5)), the park's climb, the ride)
            // (G1 04:08a item 7: the bars >= 3 + k, k the world-open term as the siblings' (1 on the
            // convention placement, 3 off it); item 11: NOT JUDGED on a re-laid run with no K samples)
            let k_on_conv = base.offset[0].rem_euclid(16) == 6 && base.offset[2].rem_euclid(16) == 5;
            let k_term = if k_on_conv { 1 } else { 3 };
            if sk.relaid_park.is_some() && sk.k_ne_margin.is_empty() && sk.k_wrong.is_empty() {
                println!("K, the driven T: NOT JUDGED (a re-laid run with no K samples)");
                failed.push("K's junction bars NOT JUDGED: a re-laid run with no K samples (G1 04:08a item 11)".into());
            }
            if !sk.k_ne_margin.is_empty() || !sk.k_wrong.is_empty() {
                println!("K, the driven T (ironspec 21:51 (5); its drive the bench's stand-in: on for the arrival, off from the park string's first press): north_east before the arriving cart's first tick in K by {} gt; south_east after that cart's last tick in K by {} gt and before the released cart's first tick in K by {} gt (bars >= 3 + k = {}, k the world-open term {k_term}{}); wrong shapes {:?}", mm2(&sk.k_ne_margin), mm2(&sk.k_se_after), mm2(&sk.k_se_before), 3 + k_term, if k_on_conv { ", the convention placement" } else { ", off the convention placement" }, sk.k_wrong);
                if !sk.k_wrong.is_empty() || sk.k_ne_margin.iter().any(|&m| m < 3 + k_term) || sk.k_se_after.iter().any(|&m| m < 3 + k_term) || sk.k_se_before.iter().any(|&m| m < 3 + k_term) {
                    failed.push(format!("K's junction bars (ironspec 21:51 (5)): {} wrong shapes, least margins {:?} / {:?} / {:?}", sk.k_wrong.len(), sk.k_ne_margin.iter().min(), sk.k_se_after.iter().min(), sk.k_se_before.iter().min()));
                }
                println!("the park's climb (the re-laid stop): the cart's highest point on the park before the release at {:.3} of the cell (its top end at 1.000: never passing it, at the fastest arrival)", sk.park_max_x);
                if sk.park_max_x >= 0.999 {
                    failed.push(format!("a cart climbed to {:.3} of the park cell: past its top", sk.park_max_x));
                }
                println!("the park -> P ride (the re-laid stop; ironspec 21:51 (6): <= 20 gt at W 48): the release -> the pickup {} gt over {} visits", mm2(&sk.ride_to_p), sk.ride_to_p.len());
                if sk.ride_to_p.iter().any(|&r| r > 20) {
                    failed.push(format!("the park -> P ride took up to {:?} gt (bar <= 20 at W 48)", sk.ride_to_p.iter().max()));
                }
                // (ironspec 22:58 (2)(a) / G1 23:00 (1)(a): the arriving empty cart across the siding's
                // water: every arrival reaches and rests in the park cell (else SENSED never presses:
                // the permanent hold); its speeds printed)
                let lo_in = sk.park_entry_speeds.iter().cloned().fold((f64::MAX, String::new()), |a, b| if b.0 < a.0 { b } else { a });
                let hi_in = sk.park_entry_speeds.iter().map(|s| s.0).fold(f64::MIN, f64::max);
                let fmt_rng = |v: &[f64]| if v.is_empty() { "none".to_string() } else { format!("{:.4}..{:.4}", v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max)) };
                println!("the arriving empty cart (ironspec 22:58 (2)(a)): out of the siding's water at {} b/t; into the park cell at {:.4}..{:.4} b/t over {} visits (slowest {}); armed visits that never reached the park cell {} {:?}", fmt_rng(&sk.water_out_arrive), if lo_in.0 == f64::MAX { 0.0 } else { lo_in.0 }, if hi_in == f64::MIN { 0.0 } else { hi_in }, sk.park_entry_speeds.len(), lo_in.1, sk.park_never_entered.len(), sk.park_never_entered.iter().take(3).collect::<Vec<_>>());
                if !sk.park_never_entered.is_empty() {
                    failed.push(format!("{} armed visits whose empty cart never reached the park cell (ironspec 22:58 (2)(a): the permanent hold)", sk.park_never_entered.len()));
                }
                // (G1 23:00 (1)(b): the departing cart: its speed into the water, out at K and into P,
                // per visit; the decay is pinned (x 0.96 empty, then x 0.95 in water))
                if !sk.water_pairs.is_empty() {
                    let ins: Vec<f64> = sk.water_pairs.iter().map(|p| p.0).collect();
                    let outs: Vec<f64> = sk.water_pairs.iter().map(|p| p.1).filter(|x| !x.is_nan()).collect();
                    let ps: Vec<f64> = sk.water_pairs.iter().map(|p| p.2).collect();
                    let slowest = sk.water_pairs.iter().cloned().fold((f64::MAX, 0.0, 0.0), |a, b| if b.0 < a.0 { b } else { a });
                    println!("the departing cart (G1 23:00 (1)(b)): into the siding's water at {} b/t, out of it at K at {} b/t, into P at {} b/t over {} visits (the slowest into the water {:.4} -> out {:.4} -> P {:.4})", fmt_rng(&ins), fmt_rng(&outs), fmt_rng(&ps), sk.water_pairs.len(), slowest.0, slowest.1, slowest.2);
                }
            }
            // (tick-exact 22:44: the woken napper's feet y on P before the pickup)
            if let Some((lo, hi)) = sk.p_feet {
                println!("the woken napper's feet on P before the pickup (tick-exact 22:44; a wet P bobs it, the pickup holding while the feet stay under the cart box's top + 0.7625): y {lo:.4}..{hi:.4}");
            }
            // (ironspec 23:00 (3): the no-HOME row)
            if !sk.stamp_event_kinds.is_empty() {
                println!("the stamp's events after the wake (ironspec 00:27b: which path armed or cleared it; HOME = a walk target at the bed): {:?}; visits ending ARMED {} (judged by the composed night's per-rider age, not here)", sk.stamp_event_kinds, sk.stamp_end_armed);
                for l in &sk.stamp_event_lines {
                    println!("  stamp {l}");
                }
            }
            if sk.no_home_visits > 0 {
                println!("the no-HOME row (ironspec 23:00 (3); HOME erased awake on P, the bed's ticket taken): {} visits, a bed-top step on {}, off P on {} (bar 0 on the re-laid stop: the napper stays on P until it boards)", sk.no_home_visits, sk.no_home_bed_top, sk.no_home_left_p);
                for l in &sk.no_home_lines {
                    println!("  no-HOME {l}");
                }
                if sk.no_home_bed_top > 0 || sk.no_home_left_p > 0 {
                    failed.push(format!("the no-HOME napper left P on {} visits ({} by the bed's top) (ironspec 23:00 (3))", sk.no_home_left_p, sk.no_home_bed_top));
                }
            }
            // (ironspec 21:15 (c)/(g): the R-press row: no park pulse with RLS low; the cart off the
            // park rail inside PARK's stretch)
            println!("park pulses with RLS low (ironspec 21:15: the park rail (1,0,4) rising with the bench's drive off; the rider's RS_ready press comes with RLS low on every visit): {} on {} visits{}; the cart's first tick off the park rail after the release {} gt (PARK's stretch {:?} gt must outlast it)", sk.park_pulses, sk.park_pulse_visits, if sk.park_pulse_lines.is_empty() { String::new() } else { format!(": {}", sk.park_pulse_lines.join(" | ")) }, mm2(&sk.park_leave), base.park_width);
            if sk.park_pulses > 0 {
                failed.push(format!("{} park pulses with RLS low on {} visits (ironspec 21:15's R-press row){}", sk.park_pulses, sk.park_pulse_visits, if base.light_q_at_rs { " (Q lit by hand: the positive control's expected failure)" } else { "" }));
            }
            if let (Some(wd), Some(&worst)) = (base.park_width, sk.park_leave.iter().max())
                && worst >= wd
            {
                failed.push(format!("the cart left the park rail {worst} gt after the release, not inside PARK's {wd}-gt stretch"));
            }
            if sk.park_never_left > 0 {
                println!("released carts that NEVER left the park cell (G1's D2): {}", sk.park_never_left);
                failed.push(format!("{} released carts never left the park cell (PARK's stretch {:?} gt)", sk.park_never_left, base.park_width));
            }
            // (ironspec 21:20: PARK's end as the entities see it - the cart's last tick on the park
            // cell, >= 2, per visit, over this run (the world-open rows move the end earlier))
            if base.park_width.is_some() {
                println!("PARK's end against the cart's last tick on the park cell (ironspec 21:20; the end as the entities see it, a world open's lead taken off): margin {} gt over {} visits (bar >= 2)", mm2(&sk.park_end_margin), sk.park_end_margin.len());
                if let Some(&least) = sk.park_end_margin.iter().min()
                    && least < 2
                {
                    failed.push(format!("the cart's last tick on the park cell only {least} gt before PARK's end (ironspec 21:20: >= 2)"));
                }
            }
            // (G1 21:20 / ironspec 21:24 (3): the cart AT REST on the park rail by RLS + 4, judged)
            println!("the cart at rest on the park rail by RLS + 4 (G1 21:20, ironspec 21:24 (3); the rest = the first of 10 still ticks): margin (RLS + 4 - the rest) {} gt over {} visits; visits with an RLS and no rest on the park rail {} (bar: every visit, margin >= 0)", mm2(&sk.rest_by_park), sk.rest_by_park.len(), sk.rest_by_park_missing);
            if sk.rest_by_park.iter().any(|&m| m < 0) || sk.rest_by_park_missing > 0 {
                failed.push(format!("the cart not at rest on the park rail by RLS + 4 on {} visits", sk.rest_by_park.iter().filter(|&&m| m < 0).count() + sk.rest_by_park_missing));
            }
            for l in &sk.step_off_lines {
                println!("  the planted step-off (G1 21:20 (1)): {l}");
            }
            for l in &sk.hurt_arm_lines {
                println!("  the hurt at ARM + {:?} (ironspec 21:21 (4)): {l}", base.hurt_at_arm);
            }
            // (ironspec 21:21 (3): a confirmation print of ctl-shutter's lie-down exclusion)
            let mut ld = sk.lie_drop.clone();
            ld.sort();
            println!("the lie-down exclusion on the real grid (ironspec 21:21 (3), a confirmation print): P_STRING's drop - the lie-down's pulse {} gt over {} visits with P_STRING up at that pulse; {} visits with P_STRING down at it; RLS reads at the pulse + 12 (any drop reaching + 12 is a finding)", mm2(&ld), ld.len(), sk.lie_drop_none);
        }
        // (G1 19:26: no RIDDEN cart on the activator (-1,0,3) in normal operation; the planted
        // --plant-e-off row shows one)
        println!("ridden carts on the activator (-1,0,3) (G1 19:26: only the empty cart rides it, after E's eject; bar 0 in normal operation): {} visits{}", sk.ridden_act_visits, if sk.ridden_act_lines.is_empty() { String::new() } else { format!(": {}", sk.ridden_act_lines.join(" | ")) });
        if sk.ridden_act_visits > 0 {
            // (G1's C1: the planted --plant-e-off row's own bar fails it too: that row's positive
            // control)
            failed.push(format!("{} visits with a ridden cart on the activator (-1,0,3) (G1 19:26){}", sk.ridden_act_visits, if base.plant_e_off { " (the planted E-off row: its expected failure)" } else { "" }));
        }
        // (G1 17:40: with OPEN's set on PARKED, the window's first open tick - the lie-down, bar
        // >= 4 (the lie-down exclusion), per visit; the late lie-down rows are expected to fail)
        if !sk.open_after_lie.is_empty() {
            let v: Vec<i64> = sk.open_after_lie.iter().map(|&(_, d)| d).collect();
            let mut per_speed: std::collections::BTreeMap<String, i64> = Default::default();
            for &(s, d) in &sk.open_after_lie {
                let e = per_speed.entry(format!("{s:.4}")).or_insert(i64::MAX);
                *e = (*e).min(d);
            }
            let low = v.iter().filter(|&&d| d < 4).count();
            println!("the window's first open tick - the lie-down (G1 17:40: bar >= 4, the lie-down exclusion): {} gt over {} visits; visits under 4: {low}; least per entry speed {:?}", mm2(&v), v.len(), per_speed);
            if low > 0 && base.adopted.is_some() {
                failed.push(format!("{low} visits with the window open less than 4 gt after the lie-down (G1 17:40)"));
            }
        }
        println!("the boarding cooldown (ironspec 17:32 (3); tick-exact 17:39: 60 gt from the eject's dismount): (the cart's first tick in P after the release) - (the napper's cooldown end) {} gt over {} visits; the bar >= 2 after k + j = {} ({} placement: k {k}, j 0): {}; least per entry speed {:?}", least.map_or("none".to_string(), |l| format!("{l}..{}", ms.iter().max().unwrap())), ms.len(), 2 + k, if on_conv { "the convention" } else { "off the convention" }, if least.is_some_and(|l| l < 2 + k) { "FAILED" } else { "passes" }, per_speed);
        if let Some(l) = least
            && l < 2 + k
        {
            failed.push(format!("the cart reached P only {l} gt after the napper's boarding cooldown ended (ironspec 17:32 (3): >= 2 after k + j = {})", 2 + k));
        }
    }
    if base.adopted.is_some() {
        // (G1's N-b on bfa0c8098: the forced rows waive only what they force: a hurt wakes the napper
        // before any window (no OPEN in every hurt row); the merge window's hurt (k <= 3) and the
        // blocked P may also miss RLS; the blocked P ends in bed; nothing waives a release with the
        // napper asleep)
        let merged = base.hurt_after_lie.is_some_and(|k| k <= 3);
        let hurt = base.hurt_after_lie.is_some() || base.hurt_at_arm.is_some();
        if (!hurt && !base.block_p_at_wake && sk.ad_no_open > 0) || (!merged && !base.block_p_at_wake && sk.ad_no_rls > 0) {
            failed.push(format!("the adopted controller: {} slept visits without OPEN, {} without RLS (normal operation: every one)", sk.ad_no_open, sk.ad_no_rls));
        }
        if sk.released_asleep > 0 {
            failed.push(format!("{} releases with the napper asleep (the adopted controller)", sk.released_asleep));
        }
        if !base.block_p_at_wake && sk.bed_occupied_end > 0 {
            failed.push(format!("{} visits ending with the bed occupied (the adopted controller)", sk.bed_occupied_end));
        }
    }
    // (G1's Q13: the integrity shapes, judged every tick in every visit)
    if base.integrity.is_empty() {
        // (G1's N1: no declared shapes is NOT JUDGED, and fails unless the run excludes it)
        println!("the design's `# integrity:` shapes: none declared (NOT JUDGED)");
        if !excluded("integrity") {
            failed.push("the `# integrity:` shapes: NOT JUDGED (none declared); --scope-excludes integrity declares them out of this run's scope".into());
        }
    } else {
        println!("the design's `# integrity:` shapes judged every tick: {} cells ({}); ticks off their declared shape over every visit: {}", base.integrity.len(), base.integrity.iter().map(|(c, s)| format!("({},{},{})={s}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", "), sk.integrity_breaks);
        if sk.integrity_breaks > 0 {
            failed.push(format!("{} ticks with a declared integrity shape off", sk.integrity_breaks));
        }
    }
    // (G1 12:04 / day-1 12:03: the wake sensor judged: every lie-down and wake seen within 10 gt,
    // no rise without an edge)
    if !sk.arm_to_lie.is_empty() {
        println!("the arming edge (ARM (-2,0,1)) to the lie-down (day-1 12:09: its SLEPT latch takes the first wake-sensor pulse after ARM as the lie-down): {:?}..{:?} gt over {} visits; lie-downs before the arming: {}", sk.arm_to_lie.iter().min(), sk.arm_to_lie.iter().max(), sk.arm_to_lie.len(), sk.arm_to_lie.iter().filter(|&&d| d < 0).count());
        if sk.arm_to_lie.iter().any(|&d| d < 0) {
            failed.push("a lie-down before the arming edge (day-1's SLEPT latch would miss it)".into());
        }
    }
    if !sk.arm_to_wake.is_empty() {
        let mut v = sk.arm_to_wake.clone();
        v.sort();
        println!("the arming edge (ARM (-2,0,1)) to the wake (day-1 12:22: its TIMEOUT T_max is sized from it): {:?}..{:?} gt over {} visits (median {:?})", v.first(), v.last(), v.len(), v.get(v.len() / 2));
    }
    if !sk.open_to_wake.is_empty() {
        let mut v = sk.open_to_wake.clone();
        v.sort();
        println!("the window's opening to the wake (G1 13:26: T_open's worst wake latency; the wake sensor's pulse rises 2 gt after the wake): {:?}..{:?} gt over {} visits (median {:?})", v.first(), v.last(), v.len(), v.get(v.len() / 2));
    }
    if sk.sensor_seen {
        let mm = |v: &Vec<i64>| (v.iter().min().copied(), v.iter().max().copied());
        println!(
            "the wake sensor (the observer at (-3,-1,4) facing up at the bed head): its first rise after a lie-down at +{:?}..{:?} gt over {} lie-downs, after a wake at +{:?}..{:?} gt over {} wakes; edges it missed (no rise within 10 gt: the observer's 2-gt pulse (tick-exact 26.3) + 8 for its 2-wide pulse and the bench's tick-boundary stamp): {}; rises with no edge in the 10 gt before: {}",
            mm(&sk.sensor_after_sleep).0, mm(&sk.sensor_after_sleep).1, sk.sensor_after_sleep.len(), mm(&sk.sensor_after_wake).0, mm(&sk.sensor_after_wake).1, sk.sensor_after_wake.len(), sk.sensor_missed, sk.sensor_spurious
        );
        // (G1's N5: a run with lie-downs must see their wakes; a run with none (the by-day negative)
        // judges only missed and spurious edges)
        if sk.sensor_missed > 0 || sk.sensor_spurious > 0 || (!sk.sensor_after_sleep.is_empty() && sk.sensor_after_wake.is_empty()) {
            failed.push(format!("the wake sensor: {} missed edges, {} rises with no edge, {} wakes seen", sk.sensor_missed, sk.sensor_spurious, sk.sensor_after_wake.len()));
        }
    }
    if park_failed {
        failed.push("the release by park position NOT met".into());
    }
    // (ironspec 07:02 real, the farm-stop bar at the visit's END: a visit ending without the napper
    // having lain down, or with it leaving without HOME, fails; a release healed by a re-claim is
    // printed and priced; its known positive is --erase-home-at, the erasure with the bed's ticket
    // held by another claimant)
    println!("the farm-stop bar (visits ending without a lie-down or without HOME; bar 0): {stop_bar} of {stop_judged} visits judged{}; releases healed by a re-claim before the visit's end (priced, not failed): {healed}, re-claim delays (gt, the release to the re-claim; AcquirePoi's attempts in MCTAS_TRACE_ACQUIRE) {reclaim_delays:?} (ticket state uncertain: pre-fix redsim)", if stop_judged == 0 { " (NOT JUDGED)" } else { "" });
    if stop_bar > 0 {
        failed.push(format!("the farm-stop bar: {stop_bar} visits ended without a lie-down or without HOME (bar 0)"));
    }
    println!("the waker's consequence (ironspec 06:40 (2); the per-visit `waker` lines with one row): {waker_wakes} wakes judged{}; a CANT_REACH stamp present after the wake in {waker_stamps}; not cleared by the pickup + 40 gt in {waker_uncleared} (the at-entry bar judges a stamp the napper rides on with)", if waker_wakes == 0 { " (NOT JUDGED)" } else { "" });
    if waker_wakes == 0 && !excluded("waker") {
        // (G1's CHANGES C8: the waker bars with 0 wakes are NOT JUDGED, a failure unless the run's
        // declared scope excludes them)
        failed.push("NOT JUDGED: 0 wakes for the waker bars; --scope-excludes waker declares them out of this run's scope".to_string());
    }
    if waker_wakes > 0 {
        println!("the waker's distance (G1 / ironspec 07:47; the napper's feet to the waker zombie's, from the wake to the pickup + 40 gt): least {}, least with eye-to-eye sight {}; ticks under 6 with sight (flee triggers: 6 is the villager panic package's flee distance, VillagerGoalPackages' SetWalkTargetAwayFrom NEAREST_HOSTILE at 6; NEAREST_HOSTILE itself is set within 8, VillagerHostilesSensor's zombie distance; bar 0): {waker_flee}; ticks under 6 with NEAREST_HOSTILE remembered (bar 0): {waker_memory}", if waker_dmin == f64::MAX { "-".to_string() } else { format!("{waker_dmin:.2}") }, if waker_seen_dmin == f64::MAX { "never in sight".to_string() } else { format!("{waker_seen_dmin:.2}") });
        if waker_flee > 0 || waker_memory > 0 {
            failed.push(format!("the waker: {waker_flee} ticks under 6 with sight, {waker_memory} under 6 with NEAREST_HOSTILE remembered (bar 0)"));
        }
        if waker_stamps > 0 {
            // (ironspec 23:53 (3) / 01:05 / 02:45 (3): on the re-laid stop the stamp bar is the
            // composed night's per-rider age < 1,200 (arms-and-clears); the stop's own stamp events
            // print above, informational here)
            if sk.relaid_park.is_some() {
                println!("the waker: a CANT_REACH stamp after the wake in {waker_stamps} visits (informational on the re-laid stop: its stamp bar is the composed night's per-rider age < 1,200)");
            } else {
                failed.push(format!("the waker: a CANT_REACH stamp after the wake in {waker_stamps} visits (bar 0)"));
            }
        }
        for l in &window_rows {
            println!("  window {l}");
        }
        if hold_needed > i64::MIN {
            println!("the panic window against the cart (ironspec 09:04): the extra hold needed so the cart enters (-2,0,5) only after NEAREST_HOSTILE is empty AND PANIC is over, on every wake: {} gt (0 = already after)", hold_needed.max(0));
        }
        if !clear_after_close.is_empty() {
            let mut d = clear_after_close.clone();
            d.sort();
            let mut hist: std::collections::BTreeMap<i64, usize> = Default::default();
            for x in &d {
                *hist.entry(*x).or_default() += 1;
            }
            let worst = *d.last().unwrap();
            println!("NEAREST_HOSTILE's clear after the window's close (G1 09:13 (2): the sensor's 20-gt period + VillagerCalmDown's tick + the boarding read; asserted <= 22): over {} wakes, min {} max {worst} gt, by value {hist:?}; wakes it never cleared after the close: {clear_never}{}", d.len(), d[0], if worst > 22 { " -- EXCEEDS 22: delta re-derived" } else { "" });
            if worst > 22 {
                failed.push("NEAREST_HOSTILE's clear more than 22 gt after the window's close on a wake (delta = 5 re-derived)".to_string());
            }
        } else if waker_wakes > 0 {
            println!("NEAREST_HOSTILE's clear after the window's close: NOT JUDGED (no wake with a close and a clear)");
            failed.push("NEAREST_HOSTILE's clear after the close: NOT JUDGED (G1's Q4)".to_string());
        }
        // (G1's Q4: a never-clearing wake fails the bar)
        if clear_never > 0 {
            failed.push(format!("{clear_never} wakes whose NEAREST_HOSTILE never cleared after the window's close"));
        }
        if delta_ruled > i64::MIN {
            let terms = if base.adopted.is_some() { "the adopted controller's close and park".to_string() } else if base.latch.is_some() { "(superseded latch, before 15:37) its close and release".to_string() } else { format!("the stand-ins' release (--release-after {release_after}) and close (--close-after {})", base.close_after.max(1)) };
            // (ironspec 17:53: the stop's bar is the cart's first end-of-tick inside (-2,0,5) - (C +
            // 19) >= 1 after k + j (tick-exact 17:52: NEAREST_HOSTILE's clear at worst C + 19); C =
            // the closing write's stamp, t - 1 for a pre-tick write (ironspec 20:55 on G1's B2);
            // "close + 27" retires (printed beside it))
            let on_conv = base.offset[0].rem_euclid(16) == 6 && base.offset[2].rem_euclid(16) == 5;
            let k = if on_conv { 1 } else { 3 };
            let bar = 1 + k;
            let m19 = -delta_ruled + 8;
            println!("the hold delta (ironspec 17:53: the cart's first end-of-tick inside (-2,0,5) - (C + 19) >= 1 after k + j, C the closing write's stamp (t - 1); this run {}: k {k}, j 0, so >= {bar}): {terms} leave a least margin of {m19} gt over this sweep's wakes (negative: missed; G1's Q4: judged); the retired close + 27 figure {} gt", if on_conv { "on the convention" } else { "off the convention" }, -delta_ruled);
            if m19 < bar {
                failed.push(format!("the cart's first entry into (-2,0,5) cleared C + 19 by only {m19} gt on a wake (ironspec 17:53: >= 1 after k + j = {bar})"));
            }
        } else if waker_wakes > 0 {
            // (G1's N2: wakes with no (-2,0,5) entry after a close: the hold delta NOT JUDGED fails)
            failed.push("the hold delta: NOT JUDGED (no wake with a close and a first (-2,0,5) entry)".to_string());
        }
        {
            println!("the re-sleep lock (G1 09:12 / ironspec 09:13): P's feet ~(-1.5, 0.06, 4.5) to HOME's centre (-2.5, 0.5, 4.5): {:.2} (SleepInBed's 2.0: within, so the lock is live); lie-downs after the wake before the pickup (bar 0): {resleeps}; the longest wake -> boarding {} (stated margin < 80 under LAST_WOKEN's 100)", ((1.0f64).powi(2) + (0.44f64).powi(2)).sqrt(), if board_wait_max > i64::MIN { format!("{board_wait_max} gt") } else { "- (no boarding)".to_string() });
            if resleeps > 0 {
                failed.push(format!("{resleeps} visits with the napper lying back down after its wake before the pickup (the re-sleep lock)"));
            }
            // (G1's Q13: the design's `a6-pickup:` stranded bound judged: a boarding by the wake
            // edge + 60 gt on every visit; ironspec 16:09: the cooldown margin, LAST_WOKEN's 100 - the
            // longest wake -> boarding, >= 10; ironspec 19:09: the merged path (--hurt-after-lie) is a
            // fault path judged instead by "boards before the napper can leave P or re-sleep": its
            // margin to LAST_WOKEN's re-sleep at the wake + 100 printed, the re-sleep bar below)
            if board_wait_max > 90 {
                failed.push(format!("a boarding {board_wait_max} gt after the wake: LAST_WOKEN's 100-gt cooldown keeps under 10 gt of margin (ironspec 16:09's bar: >= 10)"));
            }
            // (G1's N-b on bfa0c8098 / ironspec 23:00 (2) / G1 23:02: the stranded bars by the wake's
            // class, derived per visit from the first wake against COOL: NATURAL wakes keep the
            // design's `a6-pickup:` bound; FORCED (pre-COOL) wakes board in the first window and by
            // the wake + 90; Q15: no `a6-pickup:` key is NOT JUDGED under every controller)
            let mm2 = |v: &[i64]| if v.is_empty() { "none".to_string() } else { format!("{}..{}", v.iter().min().unwrap(), v.iter().max().unwrap()) };
            println!("the wake classes (ironspec 23:00 (2), G1 23:02: the first wake against COOL): natural {} visits, wake -> boarding {} gt (the stranded bound {:?}); forced {} visits, wake -> boarding {} gt (bar: the first window and <= the wake + 90), failing {}{}", sk.natural_visits, mm2(&sk.natural_waits), base.stranded_bound, sk.forced_visits, mm2(&sk.forced_waits), sk.forced_fail.len(), if sk.forced_fail.is_empty() { String::new() } else { format!(": {}", sk.forced_fail.iter().take(3).cloned().collect::<Vec<_>>().join(" | ")) });
            if !sk.forced_fail.is_empty() {
                failed.push(format!("{} forced wakes missed the first window or boarded after the wake + 90 (ironspec 23:00 (2))", sk.forced_fail.len()));
            }
            if let Some(sb) = base.stranded_bound {
                if let Some(&nw) = sk.natural_waits.iter().max()
                    && nw > sb
                {
                    failed.push(format!("a natural wake boarded {nw} gt after the wake: over the design's `a6-pickup:` stranded bound ({sb}, read from the key)"));
                }
                if !sk.natural_fail.is_empty() {
                    failed.push(format!("{} natural wakes never boarded", sk.natural_fail.len()));
                }
            } else {
                failed.push("the stranded bound: NOT JUDGED (the design has no `a6-pickup:` key naming it)".into());
            }
            for l in &resleep_lines {
                println!("  re-sleep {l}");
            }
            println!("the re-sleep lock, G1 09:19: boardings per visit over 1 (a second eject at P: a pickup fault; bar 0): {multi_board}; the least distance from HOME's head centre to the standing napper's feet, the wake to its last boarding: {} (SleepInBed's strict 2.0)", if stand_home_min == f64::MAX { "-".to_string() } else { format!("{stand_home_min:.3}") });
            if multi_board > 0 {
                failed.push(format!("{multi_board} visits with more than one boarding after the release (a pickup fault)"));
            }
        }
    }
    if !contact_speeds.is_empty() {
        // (the 0.56 class by its mechanism (G1 / ironspec 09:13): the cart's speed when its pickup
        // box first meets the napper on foot after the release; seated only above 0.1)
        let lo = contact_speeds.iter().cloned().fold((f64::MAX, String::new()), |a, b| if b.0 < a.0 { b } else { a });
        let below = contact_speeds.iter().filter(|s| s.0 <= 0.1).count();
        // (ironspec 23:40 (1): the upper mechanism bar: no push of the napper before seating)
        println!("pushes of the napper by the cart before it is seated (ironspec 23:40 (1): the boxes overlapping un-inflated, or the napper displaced after the first contact): {} of {} visits (bar 0){}", sk.push_before_seat.len(), sk.push_judged, if sk.push_before_seat.is_empty() { String::new() } else { format!(": {}", sk.push_before_seat.iter().take(3).cloned().collect::<Vec<_>>().join(" | ")) });
        if !sk.push_before_seat.is_empty() {
            failed.push(format!("{} visits with a push of the napper before seating (ironspec 23:40 (1))", sk.push_before_seat.len()));
        }
        // (ironspec 23:54 (a): the inflate is 0 in y; (b) the boarding cooldown the eject sets)
        if let Some((ov, l)) = &sk.contact_y_least {
            println!("the y overlap at the first contact (ironspec 23:54 (a): the pickup box is inflated 0 in y): least {ov:.4} ({l})");
            if *ov <= 0.0 {
                failed.push(format!("a first contact with no y overlap ({ov:.4})"));
            }
        }
        if !sk.eject_to_pickup.is_empty() {
            let least = *sk.eject_to_pickup.iter().min().unwrap();
            println!("the eject -> the pickup (ironspec 23:54 (b): the eject's boarding cooldown {} gt, k + j included): least {least} gt over {} visits, margin {}", base.boarding_cooldown, sk.eject_to_pickup.len(), least - base.boarding_cooldown);
            if least < base.boarding_cooldown {
                failed.push(format!("a pickup {least} gt after the eject, inside the {}-gt boarding cooldown", base.boarding_cooldown));
            }
        }
        // (ironspec 22:38 (1)(b): the first-contact speed judged in its band per visit)
        if let Some((blo, bhi)) = base.pickup_band {
            let hi = contact_speeds.iter().map(|s| s.0).fold(f64::MIN, f64::max);
            let out: Vec<&(f64, String)> = contact_speeds.iter().filter(|s| s.0 < blo || s.0 > bhi).collect();
            println!("the first-contact speed band (ironspec 22:38 (1)(b): [{blo}, {bhi}]): {:.4}..{:.4} over {} visits; outside it {} (first {:?})", lo.0, hi, contact_speeds.len(), out.len(), out.first());
            if !out.is_empty() {
                failed.push(format!("{} first contacts outside [{blo}, {bhi}] (ironspec 22:38 (1)(b))", out.len()));
            }
        }
        println!("the cart's speed at the napper contact (the pickup box (inflated 0.2) first meeting the napper on foot after the release; AbstractMinecart seats only above 0.1 b/t): over {} visits, least {:.4} (seed/entry {}), margin to 0.1 {:+.4}; at or under 0.1 (a stall against the napper): {below}", contact_speeds.len(), lo.0, lo.1, lo.0 - 0.1);
    }
    if !park_xs.is_empty() {
        // (the same class by its upstream cause, the park position at the release (ironspec
        // 09:13): its distribution, beside the contact speed it led to)
        let mut xs: Vec<(f64, Option<f64>, String)> = park_xs.clone();
        xs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let fails: Vec<&(f64, Option<f64>, String)> = xs.iter().filter(|p| p.1.is_some_and(|s| s <= 0.1)).collect();
        let x_fail = fails.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let passing_least = xs.iter().filter(|p| p.1.is_some_and(|s| s > 0.1)).map(|p| p.0).fold(f64::MAX, f64::min);
        println!("the park position at the release (the cart's x, at rest 10 gt; the stall's upstream cause): over {} visits, least {:.3} ({}), greatest {:.3}; the greatest x that stalled {}; the least x that seated {}", xs.len(), xs[0].0, xs[0].2, xs[xs.len() - 1].0, if fails.is_empty() { "none".to_string() } else { format!("{x_fail:.3}") }, if passing_least == f64::MAX { "-".to_string() } else { format!("{passing_least:.3}") });
    }
    if exit_booster {
        println!("the declared contact (ironspec 07:03 (3); G1's N7: the old after-tick speed, a print beside main's judge, whose speed_out is the contact tick's returned move): the least speed right after it, per entry speed {post_contact_min:?}; picked-up carts {picked_up}, of them never at the exit booster (-1,0,6): {missed_booster}");
        if missed_booster > 0 {
            failed.push(format!("{missed_booster} picked-up carts never reached the exit booster (-1,0,6) (ironspec 07:03 (3))"));
        }
    }
    // (ironspec 09:38 / 09:40, the exit-slope hold's bars: every picked-up cart rests on the slope;
    // none leaves before the tap; NEAREST_HOSTILE never set over the hold; no stamp at the
    // slope's departure; the launch's speeds per cell and the departure -> R time printed)
    if hold_design {
        sk.hold_spans.sort();
        sk.hold_to_r.sort();
        // (ironspec 20:45 (2): the cart's speed at R from the stop itself: ridden (every picked-up
        // visit) and empty departures, against the lift's reach thresholds (IRONFARM: a ridden start
        // reaches E above 0.08, an empty one above 0.11); margins under 0.03 send the thresholds back
        // for sharpening)
        let at_r = |m: &std::collections::BTreeMap<String, (f64, f64)>| m.get("4,-1,5").copied();
        match at_r(&sk.launch) {
            Some((lo, hi)) => println!("the cart's speed entering R (4,-1,5), ridden (ironspec 20:45 (2)): {lo:.4}..{hi:.4} over {} picked-up visits; the ridden reach threshold 0.08: margin {:.4} ({})", sk.hold_picked, lo - 0.08, if lo >= 0.11 { "stands: >= 0.08 + 0.03" } else { "under 0.03: the threshold to sharpen" }),
            None => println!("the cart's speed entering R (4,-1,5), ridden: no picked-up visit reached R"),
        }
        if let Some((lo, hi)) = at_r(&sk.launch_empty) {
            println!("the cart's speed entering R (4,-1,5), EMPTY departures: {lo:.4}..{hi:.4}; the empty reach threshold 0.11: margin {:.4} ({})", lo - 0.11, if lo >= 0.14 { "stands: >= 0.11 + 0.03" } else { "under 0.03: the threshold to sharpen" });
        }
        println!("the exit-slope hold (ironspec 09:38 / 09:40 (b)): picked-up carts {}, at rest on the slope {} (bar: all); left before the tap {} (bar 0); arrivals during the tap's pulse {} (bar 0); the longest creep while held {:.4} (departure is a move over 0.01: above the held rest's largest per-tick creep, 0.0083 in every reference band (printed here), the brake zeroing the slope's 0.0078 pull below 0.03); held {:?}..{:?} gt (bar: under one tap period); the rider's NEAREST_HOSTILE ticks over the hold {} (bar 0); stamps at the slope's departure {} (bar 0); a CLEAR line to the waker in a shutter state {} (bar 0); departure -> R {:?}..{:?} gt (bar: the provisional budget's {HOLD_TO_R_BUDGET}-gt term, ironspec: 73 + 170 + 15 + d_RS 10 + 7 + 10 = 285 <= 300); the launch's speed per cell (least, greatest) {:?}", sk.hold_picked, sk.hold_rested, sk.hold_early, sk.hold_arrival_powered, sk.hold_creep, sk.hold_spans.first(), sk.hold_spans.last(), sk.hold_hostile, sk.hold_stamped, sk.hold_sight_clear, sk.hold_to_r.first(), sk.hold_to_r.last(), sk.launch);
        if sk.hold_rested < sk.hold_picked {
            failed.push(format!("{} picked-up carts never at rest on the exit slope (the hold)", sk.hold_picked - sk.hold_rested));
        }
        if sk.hold_early > 0 {
            failed.push(format!("{} held carts left the slope before the tap", sk.hold_early));
        }
        if sk.hold_hostile > 0 {
            failed.push(format!("{} ticks with the held rider's NEAREST_HOSTILE set", sk.hold_hostile));
        }
        if sk.hold_stamped > 0 {
            // (ironspec 23:53 (3) / 01:05 / 02:45 (3): the exit riders arm by design on the re-laid
            // stop; their age is judged on the composed night, so informational here)
            if sk.relaid_park.is_some() {
                println!("{} held riders with a CANT_REACH stamp at the slope's departure (informational on the re-laid stop: the exit riders arm from (-2,0,5) on; their age is the composed night's bar)", sk.hold_stamped);
            } else {
                failed.push(format!("{} held riders with a CANT_REACH stamp at the slope's departure", sk.hold_stamped));
            }
        }
        if tap_at.is_none() && base.latch_next_rls.is_none() {
            failed.push("the hold design with no tap (--tap T,W[,PERIOD], G1's Q2; on the laid latch --latch-next-rls T, Q's own tap)".to_string());
        }
        if tap_at.is_none() && base.latch_next_rls.is_some() {
            println!("the hold's launch: Q's own tap line at the next visit's RLS (--latch-next-rls {:?}, G1 04:08a item 5)", base.latch_next_rls);
        }
        // (G1's Q2: an arrival during a pulse rolls through (an early launch); a wait of one period
        // or more missed a launch)
        if sk.hold_arrival_powered > 0 {
            failed.push(format!("{} loaded carts reached the slope during the tap's pulse (an early launch)", sk.hold_arrival_powered));
        }
        if let Some((_, _, Some(p))) = base.tap
            && let Some(&longest) = sk.hold_spans.last()
            && longest >= p
        {
            failed.push(format!("a held cart waited {longest} gt on the slope, a full tap period ({p} gt) or more: it missed a launch"));
        }
        // (G1's Q3: the held rider's sight CLEAR in either shutter state; departure -> R against the
        // budget's 15-gt term)
        if sk.hold_sight_clear > 0 {
            failed.push(format!("{} held riders with a CLEAR line to the waker in a shutter state", sk.hold_sight_clear));
        }
        if let Some(&worst) = sk.hold_to_r.last()
            && worst > HOLD_TO_R_BUDGET
        {
            failed.push(format!("the slope's departure -> R took {worst} gt, over the budget's {HOLD_TO_R_BUDGET}-gt term"));
        }
        if sk.hold_to_r.is_empty() && sk.hold_picked > 0 {
            failed.push("the slope's departure -> R: NOT JUDGED (no held cart reached R)".into());
        }
    }
    if sk.pickup_unwoken > 0 {
        failed.push("pickups before the wake or off the bed while asleep".into());
    }
    // (G1's CHANGES C1: every per-visit problem fails the run, not only a printed count; the
    // control: a known-bad visit (a planted undeclared contact) exits non-zero)
    if clean < tot {
        failed.push(format!("{} of {tot} visits with per-visit problems (their `PROBLEMS` lines; G1 C1)", tot - clean));
    }
    // (G1's C9, informational since ironspec 23:40 (1) / 02:03 (5): the [0.13, 0.2] band retired as a
    // proxy; the mechanism bar is the push before seating, judged per visit above)
    if steps_over > 0 {
        println!("pickups with a step over 0.2 (G1's C9, informational: ironspec 23:40 (1) / 02:03 (5), the push-before-seating bar judges the mechanism): {steps_over}");
    }
    // (ironspec Nov 13 10:42 (3) / 10:58, G1 11:12 / 11:20: the ONE bounded judge on main over every
    // visit's hits (each visit's cart its own id and its own sample, counted in visits): undeclared
    // contacts (cell and face), bounds exceeded (ticks, events, depth, speed), NO STALL (IRONSPEC_1042:
    // a move over 0.003 b/t or a fall within 5 ticks of an event's end), STALE (by `rate=` over this
    // run's visits); each declaration's measured extremes printed, and with --reference-check its
    // stated bounds must equal them (`contacts::reference_check`))
    {
        let v = crate::contacts::judge(&base.contact_decls, &sk.judge_hits, &|c, t| sk.judge_motion.get(&(c, t)).copied(), crate::contacts::StallRule::IRONSPEC_1042, Some((sk.judge_visits as u64, "visits")));
        println!("the ONE bounded contact judge (main's contacts::judge over {} visits, {} hits, NO STALL by IRONSPEC_1042): {} declarations, their events {:?}; undeclared {}; exceeded {}; stalls {}; stale {}; stale not judged {}; wiring errors {}", sk.judge_visits, sk.judge_hits.len(), base.contact_decls.len(), v.events, v.undeclared.len(), v.exceeded.len(), v.stalls.len(), v.stale.len(), v.stale_not_judged.len(), v.errors.len());
        let mut und: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for h in &v.undeclared {
            *und.entry(format!("({},{},{}) {} face", h.cell.x, h.cell.y, h.cell.z, h.face.name())).or_default() += 1;
        }
        if !und.is_empty() {
            println!("  undeclared contacts by cell and face (hit ticks): {und:?}");
        }
        for s in v.exceeded.iter().chain(&v.stalls).chain(&v.stale).chain(&v.stale_not_judged).chain(&v.errors).take(12) {
            println!("  {s}");
        }
        for d in &base.contact_decls {
            let cells: Vec<String> = d.cells.iter().map(|c| format!("({},{},{})", c.x, c.y, c.z)).collect();
            println!("the declaration {} ({:?}{}): measured {}", cells.join("+"), d.class, d.face.map_or(String::new(), |f| format!(", face {}", f.name())), decl_extremes(d, &sk.judge_hits));
            let mism = crate::contacts::reference_check(d, &sk.judge_hits);
            if mism.is_empty() {
                println!("  its stated bounds equal this run's extremes (the reference re-run check)");
            } else {
                for m in &mism {
                    println!("  reference re-run check: {m}");
                }
                if base.reference_check {
                    failed.push(format!("the declaration {}: its stated bounds don't equal this run's extremes (--reference-check: {})", cells.join("+"), mism.join("; ")));
                }
            }
        }
        if v.failed() {
            failed.push(format!("the ONE bounded contact judge: undeclared {}, exceeded {}, stalls {}, stale {}, wiring errors {}", v.undeclared.len(), v.exceeded.len(), v.stalls.len(), v.stale.len(), v.errors.len()));
        }
        // (G1's B3: a contact decided by the entry speed, judged per speed: each `meets` speed hits
        // on every seed, each other speed of the reference band never; a speed off the band isn't
        // judged)
        for (cells, meets, of) in &base.contact_speeds {
            let names: Vec<String> = cells.iter().map(|c| format!("({},{},{})", c.x, c.y, c.z)).collect();
            let face = base.contact_decls.iter().find(|d| d.cells.iter().any(|c| cells.contains(c))).and_then(|d| d.face);
            let hit_samples: std::collections::BTreeSet<u64> = sk.judge_hits.iter().filter(|h| cells.contains(&h.cell) && face.is_none_or(|f| f == h.face)).filter_map(|h| h.sample).collect();
            let (mut wrong_miss, mut wrong_hit, mut off_band) = (Vec::new(), Vec::new(), 0usize);
            let mut per_speed: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();
            for (k, &s) in sk.judge_visit_speed.iter().enumerate() {
                let hit = hit_samples.contains(&(k as u64));
                let e = per_speed.entry(format!("{s:.4}")).or_default();
                e.0 += hit as usize;
                e.1 += 1;
                if !of.iter().any(|b| (b - s).abs() < 1e-9) {
                    off_band += 1;
                    continue;
                }
                let expect = meets.iter().any(|m| (m - s).abs() < 1e-9);
                if expect && !hit {
                    wrong_miss.push(format!("{s:.4}"));
                } else if !expect && hit {
                    wrong_hit.push(format!("{s:.4}"));
                }
            }
            wrong_miss.dedup();
            wrong_hit.dedup();
            println!("the contact {} judged per entry speed (G1's B3: `contact-speeds:`, {} meeting speeds of a {}-speed band): visits met per speed {:?}; listed speeds that missed on some seed {:?}; unlisted band speeds that met {:?}; visits off the band (not judged) {off_band}", names.join("+"), meets.len(), of.len(), per_speed.iter().map(|(s, (h, n))| format!("{s}:{h}/{n}")).collect::<Vec<_>>(), wrong_miss, wrong_hit);
            if !wrong_miss.is_empty() || !wrong_hit.is_empty() {
                failed.push(format!("the contact {} off its declared speed set: listed speeds that missed {:?}, unlisted that met {:?} (a geometry change)", names.join("+"), wrong_miss, wrong_hit));
            }
            // (G1's N-a on bfa0c8098: a run entirely off the declared band judges nothing: NOT
            // JUDGED, never a silent pass)
            if !sk.judge_visit_speed.is_empty() && off_band == sk.judge_visit_speed.len() {
                failed.push(format!("the contact {} per entry speed: NOT JUDGED (all {} visits off its declared band)", names.join("+"), off_band));
            }
        }
        // (ironspec 15:23 / day-1 15:29: the rider-keyed RS_ready string's bars: the riding napper
        // presses it on every seated visit, nothing else ever does, the rider box overlaps its band,
        // the cart's box stays under it; the press - R's detector rise is d_RS's new reference)
        if sk.rs_visits > 0 {
            let mut d = sk.rs_to_r.clone();
            d.sort();
            println!("the rider-keyed RS_ready string (4,0,5) over R (ironspec 15:23, day-1 15:29): pressed by the riding napper on {} of {} seated visits ({} visits lay it); presses by anything else {} (bar 0); its first press - R's detector rise {:?}..{:?} gt (median {:?}); the rider box's least overlap with the band (y 0.0625..0.15625: an attached string's shape, the game's TripWireBlock box 0..16 x 1..2.5 x 0..16 / 16, redsim react.rs's `column(16.0, 1.0, 2.5)`) {} (> 0: it presses); the cart box's least gap under the band {} (> 0: an empty cart never reaches it)", sk.rs_pressed, sk.rs_seated, sk.rs_visits, sk.rs_spurious, d.first(), d.last(), d.get(d.len() / 2), sk.rs_overlap_min.map_or("none".to_string(), |o| format!("{o:.4}")), sk.rs_gap_min.map_or("none".to_string(), |g| format!("{g:.4}")));
            if sk.rs_pressed < sk.rs_seated {
                failed.push(format!("the RS_ready string pressed on {} of {} seated visits (the rider must press it on every one)", sk.rs_pressed, sk.rs_seated));
            }
            if sk.rs_spurious > 0 {
                failed.push(format!("{} RS_ready presses with no riding napper over the string (an empty cart, the napper on foot or the sleeper)", sk.rs_spurious));
            }
            if sk.rs_overlap_min.is_some_and(|o| o <= 0.0) {
                failed.push("a seated rider's box over the RS_ready string missed its band".into());
            }
            // (ironspec 01:46 (1) / G1 04:08a: d_RS 10 judged: RS_ready's set comes after this cart
            // left the landing (it left the stop's ENTRY at its summon, t 0): the next drop never
            // finds it there; bar > 0 on every seated visit)
            if let (Some(lo), Some(hi)) = (sk.rs_set_after_entry.iter().min(), sk.rs_set_after_entry.iter().max()) {
                println!("RS_ready's set (the rider's first press + d_RS {}, read from the file: no delay element laid on RS's output) after this cart left the landing (the stop's ENTRY at t 0): {lo}..{hi} gt over {} visits (bar > 0: by construction, ironspec 01:46 (1))", base.d_rs, sk.rs_set_after_entry.len());
                if *lo <= 0 {
                    failed.push(format!("RS_ready set only {lo} gt after the cart left the landing (d_RS {}, bar > 0)", base.d_rs));
                }
            }
            // (G1 04:31 (1) / ironspec 04:23 (1), the 16-ball at the stop: (a) the bed empties at the
            // napper's wake, and the next napper is released no earlier than RS_ready's set, so this
            // is a lower bound of "the bed empties -> the next napper first enters the ball" (its
            // travel from the queue adds to it, the composed night's); (b) the leaving rider's
            // distance from the bed head's centre then (it must be out of the ball before the next
            // napper lies down: the composed route beyond R decides)
            if let (Some(lo), Some(hi)) = (sk.bed_empty_to_rs_set.iter().min(), sk.bed_empty_to_rs_set.iter().max()) {
                let (dl, dh) = (sk.ride_at_rs_set.iter().copied().fold(f64::INFINITY, f64::min), sk.ride_at_rs_set.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                // (ironspec 04:40 (1): the parent's nap-schedule coupling: the stop's per-visit cycle
                // terms with their spreads, the inputs day-1 re-runs the schedule with)
                let mm = |v: &[i64]| if v.is_empty() { "none".to_string() } else { format!("{}..{} (median {})", v.iter().min().unwrap(), v.iter().max().unwrap(), { let mut s = v.to_vec(); s.sort(); s[s.len() / 2] }) };
                println!("the stop's per-visit cycle terms (ironspec 04:40 (1), day-1's nap schedule): the summon at ENTRY -> the lie-down {}; the nap (lie-down -> wake) {}; the wake -> R's rise {}; the bed empty (the wake) -> RS_ready's set (d_RS {}) {}; over {} visits", mm(&sk.cyc_entry_lie), mm(&sk.cyc_nap), mm(&sk.cyc_wake_r), base.d_rs, mm(&sk.bed_empty_to_rs_set), sk.cyc_nap.len());
                println!("the 16-ball at the stop (G1 04:31 (1), ironspec 04:23 (1)): the bed empties at the wake; RS_ready sets {lo}..{hi} gt later over {} visits (a lower bound of the margin to the next napper's first entry into the ball, bar > 0; its travel from the queue is the composed night's); the leaving rider then {dl:.2}..{dh:.2} from the bed head's centre (inside 16: it must leave the ball before the next napper lies down, judged on the composed route beyond R)", sk.bed_empty_to_rs_set.len());
                if *lo <= 0 {
                    failed.push(format!("RS_ready set {lo} gt after the wake: the next napper could reach the ball with the bed still occupied (G1 04:31 (1))"));
                }
            }
            if sk.rs_gap_min.is_some_and(|g| g <= 0.0) {
                failed.push("a cart's box reached the RS_ready string's band (an empty cart would press it)".into());
            }
        }
        // (ironspec 15:46 (2): the stale press: neither the riding napper's box nor the set-down
        // napper's meets P_STRING's band (-2,1,4) in the 12 gt before a lie-down)
        if !sk.pstring_gaps.is_empty() {
            let lo = sk.pstring_gaps.iter().copied().fold(f64::INFINITY, f64::min);
            println!("the napper's least gap to P_STRING's band (the awake guard's string at P + 1, (-2,1,4)) over the 12 gt before its lie-down (ironspec 15:46 (2), the stale press): {lo:.4} over {} lie-downs (> 0: neither the rider's box nor the set-down napper's meets it)", sk.pstring_gaps.len());
            if lo <= 0.0 {
                failed.push(format!("the napper's box met P_STRING's band in the 12 gt before a lie-down (least gap {lo:.4}): a stale press"));
            }
        }
        // (G1 16:00: an awake HOME-holder within 16 of an occupied bed loses HOME: the boarded
        // rider's ride away, as far as this bench follows it)
        // (ironspec 16:35: the self-heal (the free ticket: re-acquire, lie down, a normal visit) and
        // the lock (the ticket taken: never sleeps))
        if sk.heal_visits > 0 {
            let mm = |v: &[i64]| if v.is_empty() { "none".to_string() } else { format!("{}..{}", v.iter().min().unwrap(), v.iter().max().unwrap()) };
            println!("the self-heal (ironspec 16:35: a stamp-released napper, its HOME erased and a fresh stamp, the bed's ticket free): re-acquired HOME on {} of {} visits ({} gt after the release, {} gt after its landing); lay down after it on {} (re-acquire -> lie-down {} gt); clean visits after (seated, at R, no problem) {}; its cells on foot, landing -> re-acquire (visits): {:?}", sk.heal_reacquired, sk.heal_visits, mm(&sk.heal_release_to_reacquire), mm(&sk.heal_land_to_reacquire), sk.heal_slept, mm(&sk.heal_reacquire_to_lie), sk.heal_normal, sk.heal_cells);
            if sk.heal_reacquired < sk.heal_visits || sk.heal_slept < sk.heal_visits {
                failed.push(format!("the self-heal: {} of {} stamp-released nappers re-acquired HOME, {} slept (bar: all)", sk.heal_reacquired, sk.heal_visits, sk.heal_slept));
            }
        }
        if sk.lock_visits > 0 {
            println!("the lock (ironspec 16:35: a homeless napper with the bed's ticket taken): slept on {} of {} visits (expected 0: it can never re-acquire the bed)", sk.lock_slept, sk.lock_visits);
        }
        if sk.rest_on_p_visits > 0 || sk.step_offs > 0 || sk.other_on_p_max > 0 {
            println!("the woken napper on P before the pickup (G1 16:09: the step-off; redsim's vanilla REST package: with HOME held it runs SetWalkTargetFromBlockMemory(HOME, 1); InsideBrownianWalk runs only without HOME): calm (REST) on P in {} visits, at most {} gt in one; other awake ticks on P at most {} gt; visits that stepped off P before the pickup {} (bar 0)", sk.rest_on_p_visits, sk.rest_on_p_max, sk.other_on_p_max, sk.step_offs);
        }
        if sk.step_offs > 0 {
            failed.push(format!("{} woken nappers stepped off P before the pickup (G1 16:09: a step-off leaves the cart empty, a permanent halt)", sk.step_offs));
        }
        if !sk.ride_at_r.is_empty() || !sk.ride_last.is_empty() {
            let mm = |v: &[f64]| if v.is_empty() { "none".to_string() } else { format!("{:.2}..{:.2}", v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max)) };
            println!("the boarded rider's distance to the bed head's centre (-2.5,0.5,4.5) (G1 16:00: an awake HOME-holder within 16 of an occupied bed loses HOME): at R's rise {}; at its last bench tick {} (the bench ends at the stop's out: the composed route beyond R decides when it passes 16); over 16 within the bench on {} visits (not a bar on the one-visit bench: the bed is free once its napper rides; the 16-ball bar with another sleeper is the composed night's and the two-visit bench's, ironspec 04:23 (1))", mm(&sk.ride_at_r), mm(&sk.ride_last), sk.ride_over16);
        }
        // (G1 15:29 / b8949c96 15:45: the cart states near the trapdoors, for R4's planter)
        if let Some(path) = &base.cart_states {
            let mut text = format!("# the bed stop's cart states near its top-half trapdoors: x,y,z,vx,vy,vz,rider (the design frame, the stored motion), every tick a cart's centre is within 1.5 of a trapdoor's column at its layer or the one above; {} visits\n", sk.judge_visits);
            for l in &sk.cart_states {
                text.push_str(l);
                text.push('\n');
            }
            std::fs::write(path, text)?;
            println!("the cart states near the trapdoors: {} lines written to {}", sk.cart_states.len(), path.display());
        }
        // (G1 15:25: a declaration's rate judged two-sided: main's STALE fails an under-rate; an
        // over-rate at the same P < 0.001 (a geometry change raising the rate) fails here; the band
        // for this run's visits printed)
        for d in &base.contact_decls {
            let Some((h, n, unit)) = &d.rate else { continue };
            let cells: Vec<String> = d.cells.iter().map(|c| format!("({},{},{})", c.x, c.y, c.z)).collect();
            if unit != "visits" {
                failed.push(format!("the declaration {}: rate={h}/{n} {unit}: this bench counts visits (G1 11:20: a wiring error)", cells.join("+")));
                continue;
            }
            let p = *h as f64 / *n as f64;
            let k = sk.judge_visits as u64;
            let met: std::collections::BTreeSet<u64> = sk.judge_hits.iter().filter(|x| d.cells.contains(&x.cell) && d.face.is_none_or(|f| f == x.face)).filter_map(|x| x.sample).collect();
            let m = met.len() as u64;
            let over = if m == 0 { 1.0 } else { 1.0 - crate::contacts::binomial_cdf(m - 1, k, p) };
            let lo = (0..=k).take_while(|&x| crate::contacts::binomial_cdf(x, k, p) < 0.001).last();
            let hi = (1..=k).find(|&x| 1.0 - crate::contacts::binomial_cdf(x - 1, k, p) < 0.001);
            println!("the declaration {}'s rate {h}/{n} {unit}, two-sided at P < 0.001 over this run's {k} visits: met on {m}; fails at {} or fewer (main's STALE) and at {} or more (the over-rate)", cells.join("+"), lo.map_or("none".to_string(), |x| x.to_string()), hi.map_or("none".to_string(), |x| x.to_string()));
            if over < 0.001 {
                failed.push(format!("the declaration {}: met on {m} of {k} visits, over its rate {h}/{n}: P(X >= {m}) = {over:.2e} < 0.001 (the over-rate: a geometry change)", cells.join("+")));
            }
        }
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
    let total_visits: usize = sk.visits_by_speed.values().sum();
    let samples = [("visits", total_visits, "every visit bar (stand-stills, HOME, escapes, contacts, lightning, the stamp)"), ("pickups", sk.pickups, "the early-pickup bar and the pickup band")];
    println!("samples per bar: {}", samples.iter().map(|(k, n, what)| format!("{k} {n} ({what}){}", if *n == 0 { if excluded(k) { " NOT JUDGED, excluded by the declared scope" } else { " NOT JUDGED" } } else { "" })).collect::<Vec<_>>().join("; "));
    for (k, n, what) in samples {
        if n == 0 && !excluded(k) {
            failed.push(format!("NOT JUDGED: 0 {k} ({what}); --scope-excludes {k} declares it out of this run's scope"));
        }
    }
    // (ironspec 22:56/22:58: benches print riderless carts out = 0; any other count fails; G1's N12 /
    // Q7: the exclusion waives it only in the empty-cart control (--empty), whose carts leave
    // riderless by design)
    if sk.riderless > 0 {
        if excluded("riderless") && base.empty {
            println!("--scope-excludes riderless: EXCLUDED (n={}): the empty-cart control's carts leave riderless by design", sk.riderless);
        } else {
            failed.push(format!("{} riderless carts out", sk.riderless));
        }
    }
    // (ironspec 00:03 / 00:07 (3): no stand-still after the eject over 20 gt)
    println!("the stand-still after the eject (gt awake on foot, unmoved, from the dismount to the first lie-down or the pickup; bar 20, ironspec 00:03 / 00:07 (3)): the longest {} gt; visits over 20: {}", sk.stand_max, sk.stand_over_20);
    if sk.stand_over_20 > 0 {
        failed.push(format!("{} visits stood still over 20 gt after the eject", sk.stand_over_20));
    }
    if !base.head_cells.is_empty() {
        println!("the head-space check (ironspec 03:30: every villager's box and eye box against the new solid cells {:?}, every tick): box overlaps {} tick-villagers, eye overlaps {} (bar 0)", base.head_cells, sk.head_box_hits, sk.head_eye_hits);
        if sk.head_eye_hits > 0 {
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
        // (G1's C19: the at-rest bar judged (its positive control: --release-after-arm releases
        // before the cart is at rest and must fail here))
        if sk.release_speed_max >= 0.001 {
            failed.push(format!("a cart moving at {:.4} b/t on a release tick (the at-rest bar: under 0.001, G1's C19: the braked park zeroes a cart's speed, so any speed at the release is a cart still arriving)", sk.release_speed_max));
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
    // (ironspec 01:41b: a planted stamp makes the run not evidence)
    if planted_stamp {
        failed.push("a planted CANT_REACH stamp: NOT EVIDENCE (ironspec 01:41b)".into());
    }
    if !failed.is_empty() {
        bail!("bars failed: {}", failed.join("; "));
    }
    Ok(())
}

/// A skip sweep's totals (`bench_cmd`).
#[derive(Default)]
struct SkipTotals {
    /// (the ONE bounded judge's input over the run: every visit's hits, its cart renamed to the
    /// visit's index (its sample), and each visit's cart motion by (visit, world tick))
    judge_hits: Vec<crate::contacts::Hit>,
    judge_motion: std::collections::HashMap<(u32, i64), (f64, f64)>,
    judge_visits: usize,
    /// (G1's B3) each judged visit's entry speed, by its sample index
    judge_visit_speed: Vec<f64>,
    /// (ironspec 15:23 / day-1 15:29: the rider-keyed RS_ready over the run) visits laying the
    /// string; seated ones; those whose string the riding napper pressed; presses by anything else;
    /// the first press - R's detector rise per visit; the rider box's least band overlap; the cart's
    /// least gap under the band
    rs_visits: usize,
    rs_seated: usize,
    rs_pressed: usize,
    rs_spurious: usize,
    rs_to_r: Vec<i64>,
    /// (G1 04:08a) d_RS 10 judged: RS_ready's set (the rider's first press + 10) after the cart left
    /// the landing at t 0
    rs_set_after_entry: Vec<i64>,
    /// (G1 04:31 (1)) the wake (the bed empties) to RS_ready's set, and the rider's distance then
    bed_empty_to_rs_set: Vec<i64>,
    ride_at_rs_set: Vec<f64>,
    /// (ironspec 04:40 (1)) the cycle terms: the summon at ENTRY (t 0) -> the lie-down, the nap, the
    /// wake -> R's rise
    cyc_entry_lie: Vec<i64>,
    cyc_nap: Vec<i64>,
    cyc_wake_r: Vec<i64>,
    rs_overlap_min: Option<f64>,
    rs_gap_min: Option<f64>,
    /// (ironspec 15:46 (2)) each lie-down's least gap to P_STRING's band over the 12 gt before it
    pstring_gaps: Vec<f64>,
    /// (G1 15:29) the cart states near the trapdoors, every visit
    cart_states: Vec<String>,
    /// (G1 16:00) the boarded riders' distances to the bed head: at R's rise, at their last bench
    /// tick, and the visits that rode over 16 within the bench
    ride_at_r: Vec<f64>,
    ride_last: Vec<f64>,
    ride_over16: usize,
    /// (G1 16:09) the woken napper's calm (REST) ticks on P before the pickup, the most in a visit;
    /// its other awake ticks there; visits whose napper stepped off P
    rest_on_p_max: usize,
    rest_on_p_visits: usize,
    other_on_p_max: usize,
    step_offs: usize,
    /// (ironspec 16:35: the self-heal) visits with the planted stamp release; those that
    /// re-acquired HOME, slept after it and ran a clean visit; the gaps; the cells landing ->
    /// re-acquire; and the lock (the ticket taken): visits planted, and those that slept anyway
    heal_visits: usize,
    heal_reacquired: usize,
    heal_slept: usize,
    heal_normal: usize,
    heal_land_to_reacquire: Vec<i64>,
    heal_release_to_reacquire: Vec<i64>,
    heal_reacquire_to_lie: Vec<i64>,
    heal_cells: std::collections::BTreeMap<[i32; 3], usize>,
    lock_visits: usize,
    lock_slept: usize,
    /// (G1's B1: the adopted controller over the run) per visit with a wake: the close, RLS and the
    /// park's release against the wake; visits that slept with no OPEN, no RLS
    ad_close: Vec<i64>,
    ad_close_state: Vec<i64>,
    ad_rls: Vec<i64>,
    ad_park: Vec<i64>,
    /// (G1 17:33) per visit: its entry speed and the park's rest (the first of the cart's 10 still
    /// ticks) after ARM's edge
    park_rest: Vec<(f64, i64)>,
    /// (ironspec 17:32 (3)) per visit: its entry speed and (the cart's first tick in P after the
    /// release) - (the napper's boarding-cooldown end)
    board_cd: Vec<(f64, i64)>,
    /// (ironspec 17:41: COOL's inputs) per visit: the eject (the napper's landing) after ARM's edge;
    /// the park's release to the cart's first tick in P; (17:39) the sensed park's pass to the rest
    eject_after_arm: Vec<i64>,
    park_to_p: Vec<i64>,
    pass_to_rest: Vec<(f64, i64)>,
    /// (G1 17:49 (2)(a)) visits by their count of sensed-park presses; presses after RLS
    sense_presses: std::collections::BTreeMap<usize, usize>,
    sense_after_rls: usize,
    /// (G1 19:10) ticks and visits with the napper's feet in a guard string's cell
    guard_cell_ticks: usize,
    guard_cell_visits: usize,
    /// (ironspec 21:10 (3)) visits whose cart rested in the park cell before the release; the others
    first_braked: usize,
    unbraked_lines: Vec<String>,
    /// (ironspec 21:11) the least RLS - ARM over the run (ARM's edge before P_HOLD's stretch)
    arm_to_stretch: Option<i64>,
    /// (ironspec 21:15) park pulses with RLS low: count, visits, lines; the cart's leave of the park
    park_pulses: usize,
    park_pulse_visits: usize,
    park_pulse_lines: Vec<String>,
    park_leave: Vec<i64>,
    /// (ironspec 23:00 (2), G1 23:02) the wake classes: natural / forced visits, waits, failures
    natural_visits: usize,
    natural_waits: Vec<i64>,
    natural_fail: Vec<String>,
    forced_visits: usize,
    forced_waits: Vec<i64>,
    forced_fail: Vec<String>,
    /// (G1's D2) released carts that never left the park cell
    park_never_left: usize,
    /// (ironspec 21:51 (5)) K's junction margins and any wrong shape; the park's highest climb (in
    /// the cell's fraction) and the release -> pickup ride
    k_ne_margin: Vec<i64>,
    k_se_after: Vec<i64>,
    k_se_before: Vec<i64>,
    k_wrong: Vec<String>,
    park_max_x: f64,
    ride_to_p: Vec<i64>,
    /// (ironspec 22:58 / G1 23:00) the siding's water both ways: the empty cart's speed into the park
    /// (and arms that never reached it), out of the water on the way out; (into the water, out at K,
    /// into P) after the release
    park_entry_speeds: Vec<(f64, String)>,
    park_never_entered: Vec<String>,
    water_out_arrive: Vec<f64>,
    water_pairs: Vec<(f64, f64, f64)>,
    /// (tick-exact 22:44) the napper's feet y on P (least, greatest)
    p_feet: Option<(f64, f64)>,
    /// (ironspec 23:40 (1)) pushes of the napper by the cart before seating, and the visits judged
    push_before_seat: Vec<String>,
    push_judged: usize,
    /// (ironspec 23:54) the least y overlap at the first contact; the eject -> pickup per visit
    contact_y_least: Option<(f64, String)>,
    eject_to_pickup: Vec<i64>,
    /// (G1's C4) SENSED's binding shifts past COOL, and PARKED ticks unlike max(press + d, COOL)
    sensed_binds: Vec<i64>,
    sensed_bind_wrong: Vec<String>,
    /// (the re-laid stop) its park cell, when the run's layout is re-laid
    relaid_park: Option<[i32; 3]>,
    /// (G1 02:27 (3)) per visit on the re-laid stop: the cart's rest x, the park string's re-check
    /// releases while parked, K's OFF edges, PARKED set, the park rail's lit edges after RLS
    flicker: Vec<(Option<f64>, usize, usize, bool, usize, bool)>,
    /// (G1 02:47) the empty cart's speeds crossing the siding's centre toward the park (speed, seed,
    /// the visit's entry speed)
    siding_speeds: Vec<(f64, u64, f64)>,
    /// (ironspec 03:56) per rest bucket (x * 20), the arrival speeds that rest there (least, most)
    rest_arrivals: std::collections::BTreeMap<i64, (f64, f64)>,
    /// (G1 04:24c (3)) per arrival speed (rounded to 0.05, x 20) redsim's park rests (least, most, n)
    rest_by_speed: std::collections::BTreeMap<i64, (f64, f64, usize)>,
    /// (vcompose 03:19) the exact stand-up feet positions over the visits, with counts
    stand_ups: std::collections::BTreeMap<String, usize>,
    /// (tick-exact 02:45) the park string's release / re-press ticks of a few low rests
    flicker_ticks: Vec<String>,
    /// (G1's CHANGES 8 B1) the stand-in cells the runs wrote (the layout's own)
    ctl_cells: Vec<[i32; 3]>,
    /// (ironspec 01:17 (2)) the laid latch's PARK windows: (lit, dark) after RLS
    latch_lit: Vec<(i64, Option<i64>)>,
    /// (G1 04:08a item 4) latch visits, those with RLS and no park lighting, lightings before RLS
    latch_visits: usize,
    latch_unlit: Vec<String>,
    latch_rises_before: usize,
    /// (ironspec 01:50b) the laid PARK's dark tick less the cart's park exit tick, with its row
    latch_margin: Vec<(i64, String)>,
    /// (ironspec 02:16) row 39's record: each visit's planted empty cart and napper at the end
    planted: Vec<String>,
    /// (ironspec 23:00 (3)) the no-HOME row
    no_home_visits: usize,
    /// (ironspec 00:27b) the stamp's events after the wake: "ARMED|CLEARED by KIND at CELL" counts,
    /// the visits ending armed, a few visits' event lists
    stamp_event_kinds: std::collections::BTreeMap<String, usize>,
    stamp_end_armed: usize,
    stamp_event_lines: Vec<String>,
    no_home_bed_top: usize,
    no_home_left_p: usize,
    no_home_lines: Vec<String>,
    /// (ironspec 21:20) PARK's end (as the entities see it) - the cart's last tick on the park cell
    park_end_margin: Vec<i64>,
    /// (G1 21:20) RLS + 4 - the cart's rest on the park rail; visits with an RLS and no park rest
    rest_by_park: Vec<i64>,
    rest_by_park_missing: usize,
    /// (G1 21:20 (1)) the planted step-off rows' lines; (ironspec 21:21 (4)) the hurt-at-ARM rows'
    step_off_lines: Vec<String>,
    hurt_arm_lines: Vec<String>,
    /// (ironspec 21:21 (3)) P_STRING's drop - the lie-down's pulse; visits with no P_STRING then
    lie_drop: Vec<i64>,
    lie_drop_none: usize,
    /// (G1's C4) visits whose sensed-park cell held no detector rail (the stand-in read)
    sense_stand_in_visits: usize,
    /// (rev 20:00 G20) the longest window-open run on a napper awake on P over the run
    open_on_p_max: usize,
    /// (G1 19:26) visits with a ridden cart on the activator (-1,0,3), their first lines
    ridden_act_visits: usize,
    ridden_act_lines: Vec<String>,
    /// (G1 17:40) per visit: the window's first open tick less the lie-down
    open_after_lie: Vec<(f64, i64)>,
    ad_no_open: usize,
    ad_no_rls: usize,
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
    /// (day-1 12:09) the arming edge to the lie-down per visit (gt; negative: the lie-down first)
    arm_to_lie: Vec<i64>,
    /// (day-1 12:22) the arming edge to the wake per visit (gt)
    arm_to_wake: Vec<i64>,
    /// (G1 13:26) the window's opening to the wake per visit (gt)
    open_to_wake: Vec<i64>,
    wake_ticks: Vec<i64>,
    skip_class: std::collections::BTreeMap<&'static str, (i64, i64)>,
    r_ats: Vec<i64>,
    r_by_speed: std::collections::BTreeMap<String, Vec<i64>>,
    visits_by_speed: std::collections::BTreeMap<String, usize>,
    entry_missing: usize,
    errors: usize,
    contact_visits: usize,
    contacts: std::collections::BTreeMap<String, usize>,
    declared_hit: std::collections::BTreeSet<[i32; 3]>,
    hold_picked: usize,
    hold_rested: usize,
    hold_spans: Vec<i64>,
    hold_early: usize,
    hold_to_r: Vec<i64>,
    hold_creep: f64,
    hold_hostile: usize,
    hold_stamped: usize,
    /// (G1's Q2 / Q3) loaded carts reaching the slope during a pulse; held riders with a CLEAR line
    hold_arrival_powered: usize,
    hold_sight_clear: usize,
    launch: std::collections::BTreeMap<String, (f64, f64)>,
    /// (ironspec 20:45 (2)) the same for empty departures (no pickup) launched from the hold
    launch_empty: std::collections::BTreeMap<String, (f64, f64)>,
    hold_lines_shown: usize,
    stamp_visit_max: i64,
    stamp_cleared: Vec<i64>,
    r_real_by_speed: std::collections::BTreeMap<String, Vec<i64>>,
    r_real_entry_by_speed: std::collections::BTreeMap<String, Vec<i64>>,
    lightning_visits: usize,
    /// (the wake sensor) its first rise after each lie-down and each wake (gt), the edges it
    /// missed (no rise within 10 gt) and its rises with no edge within 10 gt before them
    sensor_after_sleep: Vec<i64>,
    sensor_after_wake: Vec<i64>,
    sensor_missed: usize,
    sensor_spurious: usize,
    sensor_seen: bool,
    /// (G1's Q13) ticks with a declared `# integrity:` shape off, over every visit
    integrity_breaks: usize,
    /// (the emulated DAY-1 latch) TIMEOUT releases, releases with the napper asleep, visits ending
    /// with the bed occupied; the release -> boarding ride per visit; latch lines shown
    latch_timeouts: usize,
    released_asleep: usize,
    bed_occupied_end: usize,
    release_to_board: Vec<i64>,
    latch_lines_shown: usize,
    /// (G1's Q11) the queue's release to R per visit (approach mode)
    qrel_to_r: Vec<i64>,
    /// (G1 13:02) the P-eye static printed (once)
    p_eye_shown: bool,
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
    // (G1's Q9: the two-visit bench fails on its collected per-row problems)
    if clean < n {
        anyhow::bail!("{} of {n} two-visit rows with problems (their lines above)", n - clean);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (G1 01:14b's planted control for ironspec's override rule: a bench stand-in placed on a
    /// design cell fails by name, never replaces it)
    #[test]
    fn a_standin_on_a_design_cell_fails_by_name() {
        let mut b = Builder::new("t");
        b.set(2, 0, 1, "redstone_wire[east=none,north=none,power=0,south=side,west=side]");
        let e = standin(&mut b, [2, 0, 1], "lever[face=floor,facing=north,powered=false]").unwrap_err().to_string();
        assert!(e.contains("[2, 0, 1]") && e.contains("redstone_wire") && e.contains("lever"), "{e}");
        assert!(standin(&mut b, [3, 0, 1], "lever[face=floor,facing=north,powered=false]").is_ok());
    }

    /// (G1 03:46 B2's control: `--plant-empty` takes only the siding and park rails; P and the
    /// removed run-up cell are refused by name)
    #[test]
    fn plant_empty_takes_only_siding_and_park() {
        assert_eq!(plant_empty_role([-1, 0, 3], [1, 0, 3]), Ok("the siding"));
        assert_eq!(plant_empty_role([0, 0, 3], [1, 0, 3]), Ok("the siding"));
        assert_eq!(plant_empty_role([1, 0, 3], [1, 0, 3]), Ok("the park"));
        for c in [[-2, 0, 4], [0, 0, 4]] {
            let e = plant_empty_role(c, [1, 0, 3]).unwrap_err();
            assert!(e.contains(&format!("{c:?}")) && e.contains("not a siding or park rail"), "{e}");
        }
    }

    /// (the re-laid stop's files of record generate with their stand-ins on air: the bench with
    /// day-1's latch, seal-P, and the plain bench)
    #[test]
    fn the_relaid_files_generate() {
        for (bench, latch) in [(true, true), (true, false), (false, true), (false, false)] {
            let r = design_a2(bench, false, true, true, true, true, true, true, true, true, Some(1), false, false, latch, true);
            assert!(r.is_ok(), "bench {bench} latch {latch}: {:?}", r.err());
        }
    }

    /// (the re-laid header's contact declarations and their speed sets parse with the judge's own
    /// readers: three declarations, each with its own `contact-speeds:` line)
    #[test]
    fn the_relaid_contacts_parse() {
        let text = design_a2(true, false, true, true, true, true, true, true, true, true, Some(1), false, false, false, true).unwrap();
        let decls = crate::contacts::declared_contacts_bounded(&text).unwrap();
        assert_eq!(decls.len(), 3, "{decls:?}");
        let speeds = parse_contact_speeds(&text, [0, 0, 0]).unwrap();
        assert_eq!(speeds.len(), 3);
        assert!(text.contains("# build-order: park=2,0,3=minecraft:slime_block;~2,1,3=minecraft:rail[shape=north_south,waterlogged=false];1,0,3=minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false];2,1,3=air"));
        // (vcompose 03:28: the keep-clear holds no designed block; the hook at (-3,1,4) is outside it)
        assert!(text.contains("\nkeep-clear: -3,2,4 -3,3,4"), "keep-clear");
        assert_eq!(parse_stand_up(&text).len(), 2);
    }
}
