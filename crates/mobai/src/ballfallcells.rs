//! (cc767b78, ironspec 07:53a / 10:52a (2): the c19 fall into the bed's ball as a SEEDED recording, a normal night's
//! fall) the rev 3 record's cells for the `ball-fall` / `ball-fall-empty` scenes (fixture.rs): record7f13b rev 3
//! (md5 3d7ed77b4bf3, = scenes/f1q2-record7f13b-rev3-3d7ed77b4bf3.txt: the geometry of tick-exact's car-15 c19 runs), in
//! record frame. The way-in's front and its drop, x 7..9, y -26..-13, z -6..0, every non-air cell as written but the
//! controller's (R1's comparator (7,-14,-4), its repeater (8,-14,-4) and the side dust (7,-14,-5): the scene's release
//! is a redstone block where the repeater stands); the bed (-3,-27,3) (head) / (-3,-27,2) (foot) as written, on a glass
//! floor with a glass ring round it (the record's bed tier's stop rails and trapdoors left out: the sleeper starts on
//! the bed and lies down there).

pub(crate) const BALL_FALL_CELLS: &[([i32; 3], &str)] = &[
    // the way-in's front, the departure and the rail end, the drop to the landing, the landing's rails (as written)
    ([7, -26, -5], "minecraft:glass"),
    ([7, -25, -5], "minecraft:rail[shape=east_west,waterlogged=false]"),
    ([7, -15, -5], "minecraft:glass"),
    ([7, -15, -4], "minecraft:glass"),
    ([8, -26, -5], "minecraft:glass"),
    ([8, -25, -5], "minecraft:rail[shape=east_west,waterlogged=false]"),
    ([8, -15, -4], "minecraft:glass"),
    ([9, -26, -5], "minecraft:glass"),
    ([9, -26, -4], "minecraft:glass"),
    ([9, -26, -3], "minecraft:glass"),
    ([9, -25, -5], "minecraft:rail[shape=south_west,waterlogged=false]"),
    ([9, -25, -4], "minecraft:rail[shape=north_south,waterlogged=false]"),
    ([9, -25, -3], "minecraft:rail[shape=ascending_south,waterlogged=false]"),
    ([9, -25, -2], "minecraft:glass"),
    ([9, -25, -1], "minecraft:glass"),
    ([9, -24, -2], "minecraft:rail[shape=north_south,waterlogged=false]"),
    ([9, -24, -1], "minecraft:rail[shape=ascending_south,waterlogged=false]"),
    ([9, -24, 0], "minecraft:glass"),
    ([9, -16, -3], "minecraft:glass"),
    ([9, -16, -2], "minecraft:glass"),
    ([9, -15, -4], "minecraft:glowstone"),
    ([9, -15, -3], "minecraft:detector_rail[powered=false,shape=ascending_north,waterlogged=false]"),
    ([9, -15, -2], "minecraft:rail[shape=north_south,waterlogged=false]"),
    ([9, -15, 0], "minecraft:glass"),
    ([9, -14, -5], "minecraft:glass"),
    ([9, -14, -4], "minecraft:powered_rail[powered=false,shape=ascending_north,waterlogged=false]"),
    ([9, -13, -6], "minecraft:glass"),
    ([9, -13, -5], "minecraft:detector_rail[powered=false,shape=ascending_north,waterlogged=false]"),
    // the bed (as written), its glass floor x -4..-2 z 1..4 at y -28, the glass ring round it at y -27 and -26
    ([-3, -27, 2], "minecraft:red_bed[facing=south,occupied=false,part=foot]"),
    ([-3, -27, 3], "minecraft:red_bed[facing=south,occupied=false,part=head]"),
    ([-4, -28, 1], "minecraft:glass"),
    ([-4, -28, 2], "minecraft:glass"),
    ([-4, -28, 3], "minecraft:glass"),
    ([-4, -28, 4], "minecraft:glass"),
    ([-3, -28, 1], "minecraft:glass"),
    ([-3, -28, 2], "minecraft:glass"),
    ([-3, -28, 3], "minecraft:glass"),
    ([-3, -28, 4], "minecraft:glass"),
    ([-2, -28, 1], "minecraft:glass"),
    ([-2, -28, 2], "minecraft:glass"),
    ([-2, -28, 3], "minecraft:glass"),
    ([-2, -28, 4], "minecraft:glass"),
    ([-4, -27, 2], "minecraft:glass"),
    ([-4, -27, 3], "minecraft:glass"),
    ([-2, -27, 2], "minecraft:glass"),
    ([-2, -27, 3], "minecraft:glass"),
    ([-3, -27, 1], "minecraft:glass"),
    ([-3, -27, 4], "minecraft:glass"),
    ([-4, -26, 2], "minecraft:glass"),
    ([-4, -26, 3], "minecraft:glass"),
    ([-2, -26, 2], "minecraft:glass"),
    ([-2, -26, 3], "minecraft:glass"),
    ([-3, -26, 1], "minecraft:glass"),
    ([-3, -26, 4], "minecraft:glass"),
];

/// The scenes' offset from record frame (the stone floor at y 62 under it, chunks 0..1 x 0..1).
pub(crate) const BALL_FALL_OFFSET: [i32; 3] = [16, 100, 16];

#[cfg(test)]
mod tests {
    use redsim::entity::EntityKind;

    /// The faller's HOME on each tick of a redsim run of the scene (the cart's passenger), and the sleeper's lie-down
    /// tick: redsim's own prediction of the recording's known answers (the recording decides).
    fn run(name: &str, ticks: i64) -> (Option<i64>, Option<i64>) {
        let s = crate::fixture::scenes().into_iter().find(|s| s.name == name).expect("the scene");
        let mut w = crate::fixture::redsim_world(&s, 1).expect("the world");
        crate::fixture::redsim_summon(&mut w, &s).expect("the summon");
        let actions = crate::fixture::scene_actions(name);
        let mut erased = None;
        let mut slept = None;
        for t in 1..=ticks {
            for (at, cells) in &actions {
                if *at as i64 == t - 1 {
                    for (p, st) in cells {
                        w.set_block(redsim::pos::BlockPos::new(p[0], p[1], p[2]), redsim::block::Block::parse(st).unwrap(), redsim::world::flags::ALL);
                    }
                }
            }
            w.tick();
            for e in &w.entities.list {
                let EntityKind::Mob(m) = &e.kind else { continue };
                let home = m.brain.as_deref().and_then(|b| b.home);
                if e.vehicle.is_some() {
                    if home.is_none() && erased.is_none() {
                        erased = Some(t);
                    }
                } else if m.sleeping_pos.is_some() && slept.is_none() {
                    slept = Some(t);
                }
            }
        }
        (erased, slept)
    }

    /// (ironspec 07:53a) redsim's planted pair: the sleeper lies down before the release (t100); the faller's HOME is
    /// erased once it falls inside 16 of the occupied bed (c19: ON + 16); with the bed empty nothing is erased
    #[test]
    fn the_ball_fall_erases_home_only_with_the_bed_occupied() {
        let (erased, slept) = run("ball-fall", 200);
        let slept = slept.expect("the sleeper lies down");
        assert!(slept < 100, "the sleeper asleep before the release: t{slept}");
        let erased = erased.expect("the faller's HOME erased");
        assert!(erased > 100 && erased < 140, "the erase after the release: t{erased}");
        let (erased_empty, _) = run("ball-fall-empty", 200);
        assert_eq!(erased_empty, None, "the empty bed erases nothing");
    }
}
