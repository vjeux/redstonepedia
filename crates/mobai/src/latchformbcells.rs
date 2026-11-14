//! (tickexact 2026-11-13, day-1 23:37 / ironspec 23:58 / cc767b78 23:50) day-1's P_HOLD latch, FORM (b), as a standalone
//! rig (a/1902620467569082, md5 c1e07b1b812b): 80 cells (RLS_IN (-3,0,0) and ARM_IN (4,0,-3) are air at rest), in the
//! rig's local frame (y 0 = the latch level, y -1 the glass under every wire). `LATCH_FORMB_SETTLED` overrides the
//! listed states with the settled RESET states (wire shapes and powers, Q unlit), so a scene that places them raw
//! is already a fixed point (`mctas mobai scene-lint`).

pub(crate) const LATCH_FORMB_CELLS: &[([i32; 3], &str)] = &[
    ([-3, -1, 0], "minecraft:glass"),
    ([-3, -1, 1], "minecraft:glass"),
    ([-3, -1, 2], "minecraft:glass"),
    ([-3, -1, -1], "minecraft:glass"),
    ([-3, -1, -2], "minecraft:glass"),
    ([-3, -1, -3], "minecraft:glass"),
    ([-3, -1, -4], "minecraft:glass"),
    ([-2, -1, 0], "minecraft:glass"),
    ([-2, -1, 2], "minecraft:glass"),
    ([-2, -1, -2], "minecraft:glass"),
    ([-2, -1, -4], "minecraft:glass"),
    ([-1, -1, 0], "minecraft:glass"),
    ([-1, -1, 2], "minecraft:glass"),
    ([-1, -1, 3], "minecraft:glass"),
    ([-1, -1, 4], "minecraft:glass"),
    ([-1, -1, -2], "minecraft:glass"),
    ([-1, -1, -4], "minecraft:glass"),
    ([0, -1, 0], "minecraft:glass"),
    ([0, -1, 1], "minecraft:glass"),
    ([0, -1, 2], "minecraft:glass"),
    ([0, -1, -1], "minecraft:glass"),
    ([0, -1, -2], "minecraft:glass"),
    ([0, -1, -4], "minecraft:glass"),
    ([1, -1, 0], "minecraft:glass"),
    ([1, -1, 2], "minecraft:glass"),
    ([1, -1, -4], "minecraft:glass"),
    ([2, -1, 0], "minecraft:glass"),
    ([2, -1, 2], "minecraft:glass"),
    ([2, -1, -4], "minecraft:glass"),
    ([3, -1, 0], "minecraft:glass"),
    ([3, -1, 2], "minecraft:glass"),
    ([3, -1, -1], "minecraft:glass"),
    ([3, -1, -2], "minecraft:glass"),
    ([3, -1, -3], "minecraft:glass"),
    ([3, -1, -4], "minecraft:glass"),
    ([4, -1, 0], "minecraft:glass"),
    ([4, -1, 1], "minecraft:glass"),
    ([4, -1, 2], "minecraft:glass"),
    ([4, -1, -3], "minecraft:glass"),
    ([5, -1, 0], "minecraft:glass"),
    ([6, -1, 0], "minecraft:glass"),
    ([-3, 0, -1], "minecraft:redstone_wire"),
    ([-3, 0, -2], "minecraft:redstone_wire"),
    ([-3, 0, -3], "minecraft:redstone_wire"),
    ([-3, 0, -4], "minecraft:redstone_wire"),
    ([-2, 0, -2], "minecraft:redstone_wire"),
    ([-1, 0, -2], "minecraft:redstone_wire"),
    ([0, 0, -2], "minecraft:redstone_wire"),
    ([0, 0, -1], "minecraft:repeater[delay=1,facing=north]"),
    ([-2, 0, -4], "minecraft:repeater[delay=2,facing=west]"),
    ([-1, 0, -4], "minecraft:repeater[delay=2,facing=west]"),
    ([0, 0, -4], "minecraft:repeater[delay=2,facing=west]"),
    ([1, 0, -4], "minecraft:repeater[delay=2,facing=west]"),
    ([2, 0, -4], "minecraft:repeater[delay=2,facing=west]"),
    ([3, 0, -4], "minecraft:redstone_wire"),
    ([3, 0, -3], "minecraft:redstone_wire"),
    ([3, 0, -2], "minecraft:redstone_wire"),
    ([3, 0, -1], "minecraft:repeater[delay=1,facing=north]"),
    ([-2, 0, 0], "minecraft:redstone_wire"),
    ([-1, 0, 0], "minecraft:redstone_wire"),
    ([0, 0, 0], "minecraft:stone"),
    ([1, 0, 0], "minecraft:redstone_wall_torch[facing=east]"),
    ([2, 0, 0], "minecraft:redstone_wire"),
    ([3, 0, 0], "minecraft:stone"),
    ([4, 0, 0], "minecraft:redstone_wall_torch[facing=east]"),
    ([4, 0, 1], "minecraft:redstone_wire"),
    ([4, 0, 2], "minecraft:redstone_wire"),
    ([3, 0, 2], "minecraft:redstone_wire"),
    ([2, 0, 2], "minecraft:redstone_wire"),
    ([1, 0, 2], "minecraft:redstone_wire"),
    ([0, 0, 2], "minecraft:redstone_wire"),
    ([0, 0, 1], "minecraft:redstone_wire"),
    ([-3, 0, 1], "minecraft:redstone_wire"),
    ([-3, 0, 2], "minecraft:redstone_wire"),
    ([-2, 0, 2], "minecraft:redstone_wire"),
    ([-1, 0, 2], "minecraft:redstone_wire"),
    ([-1, 0, 3], "minecraft:repeater[delay=1,facing=north]"),
    ([-1, 0, 4], "minecraft:redstone_lamp"),
    ([5, 0, 0], "minecraft:repeater[delay=1,facing=west]"),
    ([6, 0, 0], "minecraft:redstone_lamp"),
];

/// The settled RESET states that differ from the listed ones (local frame), from `mctas mobai scene-lint latch-formb
/// --all` with Q written unlit (RESET: QBAR lit powers BASE, so Q is dark and the feedback line carries nothing).
pub(crate) const LATCH_FORMB_SETTLED: &[([i32; 3], &str)] = &[
    ([4, 0, 0], "minecraft:redstone_wall_torch[facing=east,lit=false]"),
    ([-3, 0, -4], "minecraft:redstone_wire[east=side,north=none,power=0,south=side,west=none]"),
    ([-3, 0, -3], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([-3, 0, -2], "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=none]"),
    ([-3, 0, -1], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([-3, 0, 1], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([-3, 0, 2], "minecraft:redstone_wire[east=side,north=side,power=0,south=none,west=none]"),
    ([-2, 0, -2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([-2, 0, 0], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([-2, 0, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([-1, 0, -2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([-1, 0, 0], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([-1, 0, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=side,west=side]"),
    ([0, 0, -2], "minecraft:redstone_wire[east=none,north=none,power=0,south=side,west=side]"),
    ([0, 0, 1], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([0, 0, 2], "minecraft:redstone_wire[east=side,north=side,power=0,south=none,west=side]"),
    ([1, 0, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([2, 0, 0], "minecraft:redstone_wire[east=side,north=none,power=15,south=none,west=side]"),
    ([2, 0, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([3, 0, -4], "minecraft:redstone_wire[east=none,north=none,power=0,south=side,west=side]"),
    ([3, 0, -3], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([3, 0, -2], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([3, 0, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([4, 0, 1], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([4, 0, 2], "minecraft:redstone_wire[east=none,north=side,power=0,south=none,west=side]"),
];

/// The no-branch copy's one other difference: the wire before the branch's cell (glass there) runs east-west only.
pub(crate) const LATCH_FORMB_NOBRANCH_WIRE: ([i32; 3], &str) = ([0, 0, -2], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]");

/// The rows of scene `latch-formb`: (label, RLS width, branch kept, ARM width at RLS + 8 (0: none)).
pub(crate) const LATCH_FORMB_ROWS: &[(&str, u32, bool, u32)] = &[
    ("RLS 1", 1, true, 0),
    ("RLS 2", 2, true, 0),
    ("RLS 3", 3, true, 0),
    ("RLS 20", 20, true, 0),
    ("RLS 2, no branch", 2, false, 0),
    ("RLS 2 + ARM 1", 2, true, 1),
    ("RLS 2 + ARM 2", 2, true, 2),
    ("RLS 2 + ARM 3", 2, true, 3),
    ("RLS 2 + ARM 4", 2, true, 4),
    ("RLS 2 + ARM 5", 2, true, 5),
];

/// Copy k's offset: world = local + (16 k + 8, 70, 8).
pub(crate) fn latch_formb_off(k: usize) -> [i32; 3] {
    [16 * k as i32 + 8, 70, 8]
}

/// The RLS command's tick (the scene's actions apply before the tick named).
pub(crate) const LATCH_FORMB_RLS_AT: u32 = 20;
