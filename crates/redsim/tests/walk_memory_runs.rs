//! (b8949c96, ironspec 08:13 / 08:20 / 08:22) The record of `SetWalkTargetFromBlockMemory`'s runs
//! (`AiWorld::walk_memory_runs`): one test per branch against the behaviour's own conditions (Release:
//! the stamp older than 1,200; ReleaseAfterTries: too far, 1,000 tries; Toward: beyond 150; Target:
//! beyond the close distance 1; Nothing: within it), no run while WALK_TARGET is held, a seated rider's
//! HOME-target run, and the default-off path changing nothing (the same world with the record on and
//! off gives the same trace, tick by tick).

use redsim::block::Block;
use redsim::entity::mob::ai::brain;
use redsim::entity::mob::ai::{WalkMemoryBranch, WalkMemoryRun};
use redsim::pos::BlockPos;
use redsim::world::{World, flags};

fn put(w: &mut World, x: i32, y: i32, z: i32, s: &str) {
    w.set_block(BlockPos::new(x, y, z), Block::parse(s).unwrap(), flags::RAW);
}

const HOME_NBT: &str = "Brain:{memories:{\"minecraft:home\":{value:{dimension:\"minecraft:overworld\",pos:[I;0,0,0]}}}}";

/// A night world (day time 18,000: the schedule's REST) with the mob AI on, a stone floor at y -1 over
/// x `floor.0..=floor.1`, z -3..3, a bed (head (0,0,0), foot (0,0,1)) and, when `record`, the record on.
fn night(x_max: i32, floor: (i32, i32), record: bool) -> World {
    let mut w = World::new(BlockPos::new(-8, -8, -8), [x_max + 16, 24, 16]);
    w.mob.seed(1, 18000);
    w.mob.ai.on = true;
    for x in floor.0..=floor.1 {
        for z in -3..=3 {
            put(&mut w, x, -1, z, "minecraft:stone");
        }
    }
    put(&mut w, 0, 0, 0, "minecraft:red_bed[facing=north,occupied=false,part=head]");
    put(&mut w, 0, 0, 1, "minecraft:red_bed[facing=north,occupied=false,part=foot]");
    if record {
        w.mob.ai.walk_memory_runs = Some(Vec::new());
    }
    w
}

/// A villager standing at `(x, 0, 0.5)` whose HOME is the bed's head.
fn villager(w: &mut World, x: f64) -> u32 {
    redsim::entity::spawn(w, "minecraft:villager", [x, 0.0, 0.5], Some(&format!("{{PersistenceRequired:1b,{HOME_NBT}}}"))).unwrap()
}

fn idx(w: &World, id: u32) -> usize {
    w.entities.list.iter().position(|e| !e.removed && e.id == id).expect("the villager")
}

/// The HOME runs recorded since the last call.
fn drain(w: &mut World, id: u32) -> Vec<WalkMemoryRun> {
    let log = w.mob.ai.walk_memory_runs.as_mut().expect("the record on");
    log.drain(..).filter(|r| r.id == id && r.mem == brain::M::Home).collect()
}

/// Ticks until the villager's first HOME run (at most `ticks`).
fn first_run(w: &mut World, id: u32, ticks: u32) -> WalkMemoryRun {
    for _ in 0..ticks {
        w.tick();
        if let Some(r) = drain(w, id).into_iter().next() {
            return r;
        }
    }
    panic!("no run of SetWalkTargetFromBlockMemory toward HOME in {ticks} ticks");
}

#[test]
fn beyond_the_close_distance_records_a_target_run() {
    let mut w = night(16, (-3, 12), true);
    let id = villager(&mut w, 5.5);
    let r = first_run(&mut w, id, 40);
    assert_eq!(r.branch, WalkMemoryBranch::Target, "{r:?}");
    assert_eq!(r.stamp_age_seen, None, "{r:?}");
}

#[test]
fn within_the_close_distance_records_the_nothing_branch() {
    let mut w = night(16, (-3, 12), true);
    let id = villager(&mut w, 1.5);
    let r = first_run(&mut w, id, 40);
    assert_eq!(r.branch, WalkMemoryBranch::Nothing, "{r:?}");
}

#[test]
fn a_stamp_older_than_1200_records_a_release_with_the_age_it_saw() {
    let mut w = night(16, (-3, 12), true);
    let id = villager(&mut w, 5.5);
    let i = idx(&w, id);
    let planted = w.time - 1300;
    brain::ex_mut(&mut w, i).mem.cant_reach_since = Some(planted);
    let r = first_run(&mut w, id, 40);
    assert_eq!(r.branch, WalkMemoryBranch::Release, "{r:?}");
    assert_eq!(r.stamp_age_seen, Some(r.t - planted), "the age the behaviour saw: {r:?}");
    assert!(r.stamp_age_seen.unwrap() > 1200, "{r:?}");
    let i = idx(&w, id);
    assert_eq!(brain::ex(&w, i).mem.cant_reach_since, Some(r.t), "the release sets the stamp to its tick (age 0 after it)");
    assert_eq!(brain::brain(&w, i).home, None, "HOME erased by the release");
}

#[test]
fn beyond_150_records_a_toward_run() {
    let mut w = night(176, (-3, 172), true);
    let id = villager(&mut w, 160.5);
    let r = first_run(&mut w, id, 40);
    assert_eq!(r.branch, WalkMemoryBranch::Toward, "{r:?}");
}

#[test]
fn beyond_150_with_no_spot_in_1000_tries_records_a_release_after_tries() {
    // the villager on one block in the air, walled in glass: no random target can be drawn toward HOME
    let mut w = night(176, (-3, 3), true);
    put(&mut w, 160, -1, 0, "minecraft:stone");
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for y in 0..=2 {
            put(&mut w, 160 + dx, y, dz, "minecraft:glass");
        }
    }
    put(&mut w, 160, 2, 0, "minecraft:glass");
    let id = villager(&mut w, 160.5);
    let r = first_run(&mut w, id, 40);
    assert_eq!(r.branch, WalkMemoryBranch::ReleaseAfterTries, "{r:?}");
}

#[test]
fn no_run_while_the_walk_target_is_held() {
    // a tick that MoveToTargetSink runs through unbroken (running before and after, the same end: its
    // WALK_TARGET held the whole tick) records no run; the same trace records runs on the other ticks
    // (the control: the behaviour runs whenever the sink's stop leaves WALK_TARGET absent)
    let mut w = night(16, (-3, 12), true);
    let id = villager(&mut w, 5.5);
    let sink = |w: &World| -> (bool, i64) {
        let x = brain::ex(w, idx(w, id));
        x.nodes
            .iter()
            .find(|n| matches!(n.kind, brain::tree::Kind::MoveToTargetSink { .. }))
            .map_or((false, 0), |n| (n.running, n.end))
    };
    let (mut held_ticks, mut runs) = (0, 0);
    for _ in 0..600 {
        let before = sink(&w);
        let held = brain::ex(&w, idx(&w, id)).mem.walk_target.is_some();
        w.tick();
        let after = sink(&w);
        let n = drain(&mut w, id).len();
        runs += n;
        if held && before.0 && after.0 && before.1 == after.1 {
            held_ticks += 1;
            assert_eq!(n, 0, "a run recorded on a tick the sink ran through with WALK_TARGET held");
        }
    }
    assert!(held_ticks > 0 && runs > 0, "the trace saw both: {held_ticks} held ticks, {runs} runs");
}

#[test]
fn a_seated_rider_with_home_out_of_reach_records_a_home_target_run() {
    let mut w = night(16, (-3, 12), true);
    let cart = redsim::entity::spawn(&mut w, "minecraft:minecart", [5.5, 0.0, 0.5], Some(&format!("{{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,{HOME_NBT}}}]}}"))).unwrap();
    let id = w.entities.list.iter().find(|e| !e.removed && e.vehicle == Some(cart)).map(|e| e.id).expect("the rider");
    let r = first_run(&mut w, id, 40);
    assert_eq!(r.branch, WalkMemoryBranch::Target, "{r:?}");
}

/// Every entity's id, position and motion bits, and the villager's brain memories and HOME, per tick.
/// With `with_uid`, the path's uid is printed as it is (the world's own: `World::path_uid`, so two fresh worlds
/// hand out the same uids whatever runs beside them); without, it is left out.
fn trace_with(record: bool, ticks: u32, with_uid: bool) -> Vec<String> {
    let mut w = night(16, (-3, 12), record);
    let id = villager(&mut w, 5.5);
    let i = idx(&w, id);
    let planted = w.time - 1300;
    brain::ex_mut(&mut w, i).mem.cant_reach_since = Some(planted);
    let mut out = Vec::new();
    for t in 0..ticks {
        w.tick();
        if let Some(log) = w.mob.ai.walk_memory_runs.as_mut() {
            log.clear();
        }
        for e in w.entities.list.iter().filter(|e| !e.removed) {
            out.push(format!("t {t} #{} {:x} {:x} {:x} {:x} {:x} {:x}", e.id, e.pos.x.to_bits(), e.pos.y.to_bits(), e.pos.z.to_bits(), e.delta_movement.x.to_bits(), e.delta_movement.y.to_bits(), e.delta_movement.z.to_bits()));
        }
        let i = idx(&w, id);
        let mem = format!("{:?}", brain::ex(&w, i).mem);
        let mem: String = if with_uid {
            mem
        } else {
            mem.split("uid: ")
                .enumerate()
                .map(|(k, s)| {
                    if k == 0 {
                        return s.to_string();
                    }
                    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
                    format!("uid: _{}", &s[digits.len()..])
                })
                .collect()
        };
        out.push(format!("t {t} mem {mem} home {:?}", brain::brain(&w, i).home));
    }
    out
}

fn trace(record: bool, ticks: u32) -> Vec<String> {
    trace_with(record, ticks, false)
}

#[test]
fn the_record_off_and_on_give_the_same_trace() {
    // (the path's uid left out here; the uid-included form is the next test)
    let (off, on) = (trace(false, 400), trace(true, 400));
    assert_eq!(off.len(), on.len());
    for (a, b) in off.iter().zip(&on) {
        assert_eq!(a, b, "the record changed the world");
    }
}

/// (G1 08:32 (a); un-ignored with the per-world path ids, G1 / ironspec 2026-11-13 10:57 / 11:01) the same
/// comparison with the path's uid INCLUDED, as it is: each world hands out its own uids (`World::path_uid`), so
/// two OFF runs and an ON run print the same absolute uids however many tests run beside them (the old
/// process-wide counter made this test single-threaded and relative).
#[test]
fn the_record_off_and_on_give_the_same_trace_uid_included() {
    let (off, on, off2) = (trace_with(false, 400, true), trace_with(true, 400, true), trace_with(false, 400, true));
    assert!(off.iter().any(|l| l.contains("uid: ")), "the trace carries a path uid");
    assert_eq!(off, off2, "two OFF runs, absolute uids: identical (the control)");
    assert_eq!(off.len(), on.len());
    for (a, b) in off.iter().zip(&on) {
        assert_eq!(a, b, "the record changed the world (uid included)");
    }
}
