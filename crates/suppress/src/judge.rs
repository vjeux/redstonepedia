//! The judge rig (designs/briefs/update-suppressor.md, "The judge").
//!
//! Geometry (canonical, target arm = +x): a lever on a note block N0. The lever's first
//! cascade delivers DOWN to N0 before UP to the middle block B (a note block over the
//! lever): N0 flips first, its `updateNeighborsAt` steps on the head wire under it and the
//! flood field (a flat dust field one layer down) runs depth-first *above* the lever's own
//! multi-neighbour entry in the updater stack, so B is only delivered after the whole
//! flood drained - at the cascade's final counted-call total. B then flips, and B's own
//! `updateNeighborsAt(B)` is the specified notification: dropped iff the total reached the
//! cap. The target T is a quasi-powered piston beside B (a redstone block diagonally above
//! it, placed after settling, so at rest it is a BUD): any neighbour update extends it.
//! DOWN/UP are used for starter/middle because a horizontal rotation permutes the
//! W,E,N,S update order but never D before U (`NeighborUpdater.UPDATE_ORDER`): the rig is
//! rotation- and mirror-proof by construction, which the matrix verifies.
//!
//! Moat: B and T sit three face-steps above the nearest flood wire, outside dust's
//! two-step reach (`DefaultRedstoneWireEvaluator` updates neighbours of neighbours).

use anyhow::Result;
use redsim::block::{Block, Face, Instrument};
use redsim::pos::{BlockPos, Dir};
use redsim::world::World;

fn bp(x: i32, y: i32, z: i32) -> BlockPos {
    BlockPos::new(x, y, z)
}

fn note(powered: bool) -> Block {
    Block::Note { instrument: Instrument::Harp, note: 0, powered }
}

fn shifted(p: BlockPos, d: Dir) -> BlockPos {
    let s = d.step();
    bp(p.x + s[0], p.y + s[1], p.z + s[2])
}

fn perp(arm: Dir, mirror: bool) -> Dir {
    match (arm, mirror) {
        (Dir::East, false) | (Dir::West, true) => Dir::North,
        (Dir::East, true) | (Dir::West, false) => Dir::South,
        (Dir::North, false) | (Dir::South, true) => Dir::West,
        _ => Dir::East,
    }
}

/// The rig at relative coordinates (field centered on x/z 0). `field` false = the control
/// (head wire only). The redstone block is returned separately: it is placed last, after
/// settling, exactly as in survival (its placement updates cannot reach the piston).
pub struct Rig {
    pub blocks: Vec<(BlockPos, Block)>,
    pub lever: BlockPos,
    pub middle: BlockPos,
    pub target: BlockPos,
    pub qc: BlockPos,
    pub redstone: BlockPos,
    pub wake: BlockPos,
}

pub fn rig(field_n: i32, field: bool, arm: Dir, mirror: bool) -> Rig {
    let mut v: Vec<(BlockPos, Block)> = Vec::new();
    let h = field_n / 2;
    let (x0, x1) = if field { (-h, h) } else { (0, 0) };
    for x in x0..=x1 {
        for z in x0..=x1 {
            v.push((bp(x, 0, z), crate::stone()));
            v.push((bp(x, 1, z), redsim::redstone::wire::CROSS));
        }
    }
    let lever = bp(0, 3, 0);
    let middle = bp(0, 4, 0);
    v.push((bp(0, 2, 0), note(false))); // N0: lever support + flood starter
    v.push((lever, Block::Lever { face: Face::Floor, facing: arm, powered: false }));
    v.push((middle, note(false)));
    let target = shifted(middle, arm);
    v.push((target, Block::Piston { sticky: false, facing: Dir::Up, extended: false }));
    let qc = target.above();
    let redstone = shifted(qc, arm);
    let wake = shifted(target, perp(arm, mirror));
    Rig { blocks: v, lever, middle, target, qc, redstone, wake }
}

/// `PistonBaseBlock.getNeighborSignal` for a piston facing up (direct + quasi-connectivity).
fn piston_powered(w: &World, t: BlockPos) -> bool {
    for d in Dir::ALL {
        if d != Dir::Up && w.signal(t.rel(d), d) > 0 {
            return true;
        }
    }
    let a = t.above();
    Dir::ALL.iter().any(|&d| a.rel(d) != t && w.signal(a.rel(d), d) > 0)
}

fn extended(w: &World, t: BlockPos) -> bool {
    matches!(w.get(t), Block::Piston { extended: true, .. })
}

fn note_powered(w: &World, p: BlockPos) -> bool {
    matches!(w.get(p), Block::Note { powered: true, .. })
}

pub struct Verdict {
    pub name: String,
    pub suppressed_ok: bool,
    pub counted: u64,
    pub dropped: u64,
    pub middle_flipped: bool,
    pub target_on_edge: bool,
    pub target_on_wake: bool,
    pub problems: Vec<String>,
}

/// One judged run on redsim. `expect_suppressed`: judge against the suppressor contract
/// (otherwise the control contract: the target must react on the edge).
pub fn judge_one(name: &str, field_n: i32, field: bool, cap: u32, at: BlockPos, arm: Dir, mirror: bool, expect_suppressed: bool) -> Verdict {
    let r = rig(field_n, field, arm, mirror);
    let off = |p: BlockPos| bp(p.x + at.x, p.y + at.y, p.z + at.z);
    let mut all: Vec<(BlockPos, Block)> = r.blocks.iter().map(|(p, b)| (off(*p), *b)).collect();
    let rbp = off(r.redstone);
    // Size the world from the blocks (with the redstone block for the bounds).
    let mut bounds = all.clone();
    bounds.push((rbp, Block::RedstoneBlock));
    let mut w = crate::world_for(&bounds, bp(0, 0, 0));
    let mut problems: Vec<String> = Vec::new();
    w.place_like_judge(&all);
    w.run_until_idle(100);
    all.clear();
    let (lever, middle, target, wake) = (off(r.lever), off(r.middle), off(r.target), off(r.wake));
    if extended(&w, target) || note_powered(&w, middle) {
        problems.push("rest state wrong (target extended or middle powered after placement)".into());
    }
    if !w.idle() {
        problems.push("not idle after placement".into());
    }
    w.set_block_and_update(rbp, Block::RedstoneBlock);
    for _ in 0..3 {
        w.tick();
    }
    if extended(&w, target) {
        problems.push("placing the quasi source woke the target".into());
    }
    if !piston_powered(&w, target) {
        problems.push("target is not quasi-powered at rest (the control could never react)".into());
    }
    w.nu_cap = cap;
    w.nu_stats_reset();
    let errors_before = w.errors.len();
    let hist_before = w.nu_history.len();
    if !w.use_block(lever) {
        problems.push("lever use failed".into());
    }
    for _ in 0..10 {
        w.tick();
    }
    w.run_until_idle(100);
    let edge = &w.nu_history[hist_before..];
    let counted = edge.iter().map(|s| s.counted).max().unwrap_or(0);
    let dropped: u64 = edge.iter().map(|s| s.dropped).sum();
    let middle_flipped = note_powered(&w, middle);
    let target_on_edge = extended(&w, target);
    if !middle_flipped {
        problems.push("middle block did not flip on the edge".into());
    }
    // Wake with one ordinary update.
    w.set_block_and_update(wake, crate::stone());
    for _ in 0..10 {
        w.tick();
    }
    w.run_until_idle(100);
    let target_on_wake = extended(&w, target);
    if !target_on_wake {
        problems.push("target did not react to the wake update".into());
    }
    w.remove_block(wake, false);
    let _ = w.use_block(lever);
    for _ in 0..10 {
        w.tick();
    }
    w.run_until_idle(100);
    if note_powered(&w, middle) {
        problems.push("middle block did not flip back on reset".into());
    }
    if !w.idle() {
        problems.push("not idle at the end".into());
    }
    for e in &w.errors[errors_before..] {
        if !(dropped > 0 && e.contains("too many chained")) {
            problems.push(format!("model error: {e}"));
        }
    }
    let suppressed_ok = if expect_suppressed {
        problems.is_empty() && !target_on_edge && target_on_wake && counted >= cap as u64 && dropped > 0
    } else {
        problems.is_empty() && target_on_edge && target_on_wake
    };
    Verdict { name: name.to_string(), suppressed_ok, counted, dropped, middle_flipped, target_on_edge, target_on_wake, problems }
}
