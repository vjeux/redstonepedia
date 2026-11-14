//! (tickexact 2026-11-13, cc767b78 / ironspec 22:30 / dfb89499 22:32: the slot-1 push) rev 3's cells round lane 3's hold
//! and slot 1 (record7f13b rev 3, md5 3d7ed77b4bf3: x 1..5, y 14..21, z -2..4; the hold cart's trapdoor (3,16,0) against
//! the glass (3,16,-1), the hold string (3,17,0) between its hooks, R_1 (3,17,1) with its repeater (4,17,1), the slope
//! rails (3,18,2) / (3,19,3), their glass and glowstone; the next rail (3,20,4) left out: its support (3,20,5) lies
//! outside the box), in the record's frame.

pub(crate) const SLOT_HOLD_CELLS: &[([i32; 3], &str)] = &[
    ([3, 16, -1], "minecraft:glass"),
    ([2, 16, 0], "minecraft:glass"),
    ([3, 16, 0], "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]"),
    ([4, 16, 0], "minecraft:glass"),
    ([1, 16, 1], "minecraft:glowstone"),
    ([3, 16, 1], "minecraft:glass"),
    ([4, 16, 1], "minecraft:glass"),
    ([5, 16, 1], "minecraft:glowstone"),
    ([3, 17, -1], "minecraft:smooth_stone_slab[type=bottom,waterlogged=false]"),
    ([1, 17, 0], "minecraft:glowstone"),
    ([2, 17, 0], "minecraft:tripwire_hook[attached=true,facing=east,powered=false]"),
    ([3, 17, 0], "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]"),
    ([4, 17, 0], "minecraft:tripwire_hook[attached=true,facing=west,powered=false]"),
    ([5, 17, 0], "minecraft:glowstone"),
    ([3, 17, 1], "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]"),
    ([4, 17, 1], "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]"),
    ([1, 17, 2], "minecraft:glowstone"),
    ([3, 17, 2], "minecraft:glass"),
    ([5, 17, 2], "minecraft:glowstone"),
    ([4, 18, 1], "minecraft:glass"),
    ([3, 18, 2], "minecraft:rail[shape=ascending_south,waterlogged=false]"),
    ([2, 18, 3], "minecraft:glowstone"),
    ([3, 18, 3], "minecraft:glass"),
    ([3, 19, -1], "minecraft:glass"),
    ([3, 19, 3], "minecraft:rail[shape=ascending_south,waterlogged=false]"),
    ([2, 19, 4], "minecraft:glowstone"),
    ([3, 19, 4], "minecraft:glass"),
];
