//! The bed siding's two junctions on farmopt's pocket-stand loop (day-1 lane 2026-11-10; parent
//! 12:11: the chosen napper's cart diverts once a night and rejoins its batch; agreed with ironfarm
//! and farmopt 12:51-13:15).
//!
//! `release-gen --loop L --siding S` adds, to the loop file it writes:
//! - farmopt's wired siding (S's box, local x -7..6, y -4..4, z -3..9, with its dorm zombie) pasted
//!   unturned at (-20, 2, 10): its bed 17+ blocks from every rider (a calm villager within 16 took
//!   the bed in farmopt's bench), its entry booster at (-27, 2, 10), its exit (-14, 1, 10);
//! - the diverter: a branch rail north of the departure corner (9, -3, 0), which makes the corner a
//!   T (west, south, north): unpowered it stays south_west (the loop), powered it turns north_west
//!   (the branch);
//! - the entry route: north to z -2, west along z -2 (y -3) to x -29, south along x -29 climbing to
//!   y 2 on five boosters, east into the siding's booster;
//! - the exit route: from the siding's exit east to x -13, south along x -13 (y 1) to z 26, east to
//!   x -8, south into T1 (-8, 1, 28), its stem north: unpowered north_east (east along z 28 to slot
//!   14), powered north_west (west, round to T2 (-10, 1, 31), its stem north: unpowered east along
//!   z 31 to slot 13, powered west, round x -12, east along z 34 to slot 12);
//! - each drop line ends as ironfarm's side drop: three plain rails at y 1 on glass west of the
//!   slot's slope (x -6..-4), a stone stop east of it at (-2, 1, z); the cart rolls off the end into
//!   the empty slot during the long rest.
//!
//! Four controls, bench cells for now (glass = off, a redstone block = on): the corner T's support
//! (9, -4, 0), T1's (-8, 0, 28), T2's (-10, 0, 31), the siding's release input (-16, 0, 10).

use crate::schedgen::{B, P3};
use anyhow::{Result, bail};
use mctas::design::{Design, EntitySpec};
use std::path::Path;

const STONE: &str = "minecraft:stone";
const GLASS: &str = "minecraft:glass";
const RBLOCK: &str = "minecraft:redstone_block";

pub(crate) const SIDING_AT: P3 = [-20, 2, 10];
pub(crate) const DIVERT: P3 = [9, -4, 0];
pub(crate) const T1: P3 = [-8, 0, 28];
pub(crate) const T2: P3 = [-10, 0, 31];
pub(crate) const RELEASE: P3 = [-16, 0, 10];

fn rail(shape: &str) -> String {
    format!("minecraft:rail[shape={shape},waterlogged=false]")
}

fn booster(shape: &str) -> String {
    format!("minecraft:powered_rail[powered=true,shape={shape},waterlogged=false]")
}

/// A rail at `p` on a support (stone unless `glass`).
fn lay(b: &mut B, p: P3, s: &str, glass: bool) -> Result<()> {
    b.put(p, s)?;
    b.put([p[0], p[1] - 1, p[2]], if glass { GLASS } else { STONE })
}

/// Pastes the siding and lays the routes and junctions into `d`; returns the cells it added.
pub(crate) fn physical(d: &mut Design, siding: &Path) -> Result<Vec<P3>> {
    let mut b = B { blocks: d.blocks.clone(), ports: Default::default(), nbt: Default::default() };
    let before: std::collections::BTreeSet<P3> = d.blocks.keys().copied().collect();
    // ---- the siding
    let s = Design::load(siding)?;
    let inbox = |q: &P3| (-7..=6).contains(&q[0]) && (-4..=4).contains(&q[1]) && (-3..=9).contains(&q[2]);
    let at = |q: &P3| [q[0] + SIDING_AT[0], q[1] + SIDING_AT[1], q[2] + SIDING_AT[2]];
    for (q, st) in s.blocks.iter().filter(|(q, _)| inbox(q)) {
        b.put(at(q), st)?;
    }
    for (q, n) in s.block_entities.iter().filter(|(q, _)| inbox(q)) {
        d.block_entities.insert(at(q), n.clone());
    }
    for e in &s.entities {
        let q = [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32];
        if inbox(&q) {
            let p = [e.pos[0] + SIDING_AT[0] as f64, e.pos[1] + SIDING_AT[1] as f64, e.pos[2] + SIDING_AT[2] as f64];
            d.entities.push(EntitySpec { kind: e.kind.clone(), pos: p, nbt: e.nbt.clone() });
        }
    }
    if b.blocks.get(&RELEASE).map(|s| s.as_str()) != Some(GLASS) {
        bail!("the siding's release input {RELEASE:?} is {:?}, not glass: the siding file changed", b.blocks.get(&RELEASE));
    }
    // ---- the diverter: the branch north of the corner (the corner's support stays the glass
    // control the bench switches)
    match b.blocks.get(&[9, -3, 0]).map(|s| s.as_str()) {
        Some(s) if s.contains("shape=south_west") => {}
        other => bail!("the departure corner (9, -3, 0) is {other:?}, not a south_west rail: the loop file changed"),
    }
    lay(&mut b, [9, -3, -1], &rail("north_south"), false)?;
    // ---- the entry route
    lay(&mut b, [9, -3, -2], &rail("south_west"), false)?;
    for x in -28..=8 {
        lay(&mut b, [x, -3, -2], &rail("east_west"), false)?;
    }
    lay(&mut b, [-29, -3, -2], &rail("south_east"), false)?;
    lay(&mut b, [-29, -3, -1], &rail("north_south"), false)?;
    // the climb: five boosters ascending south, the first one's support a redstone block (it powers
    // all five through the rail line)
    for k in 0..5 {
        let p = [-29, -3 + k, k];
        b.put(p, &booster("ascending_south"))?;
        b.put([p[0], p[1] - 1, p[2]], if k == 0 { RBLOCK } else { STONE })?;
    }
    for z in 5..=9 {
        lay(&mut b, [-29, 2, z], &rail("north_south"), false)?;
    }
    lay(&mut b, [-29, 2, 10], &rail("north_east"), false)?;
    lay(&mut b, [-28, 2, 10], &rail("east_west"), false)?;
    // ---- the exit route to T1
    lay(&mut b, [-13, 1, 10], &rail("south_west"), false)?;
    for z in 11..=25 {
        lay(&mut b, [-13, 1, z], &rail("north_south"), false)?;
    }
    lay(&mut b, [-13, 1, 26], &rail("north_east"), false)?;
    for x in -12..=-9 {
        lay(&mut b, [x, 1, 26], &rail("east_west"), false)?;
    }
    lay(&mut b, [-8, 1, 26], &rail("south_west"), false)?;
    lay(&mut b, [-8, 1, 27], &rail("north_south"), false)?;
    // T1: stem north, unpowered north_east (slot 14), powered north_west (T2); its support glass
    b.put([-8, 1, 28], &rail("north_east"))?;
    b.put(T1, GLASS)?;
    // T1 west to T2
    lay(&mut b, [-9, 1, 28], &rail("east_west"), false)?;
    lay(&mut b, [-10, 1, 28], &rail("south_east"), false)?;
    lay(&mut b, [-10, 1, 29], &rail("north_south"), false)?;
    lay(&mut b, [-10, 1, 30], &rail("north_south"), false)?;
    b.put([-10, 1, 31], &rail("north_east"))?;
    b.put(T2, GLASS)?;
    // T2 west round to z 34
    lay(&mut b, [-11, 1, 31], &rail("east_west"), false)?;
    lay(&mut b, [-12, 1, 31], &rail("south_east"), false)?;
    lay(&mut b, [-12, 1, 32], &rail("north_south"), false)?;
    lay(&mut b, [-12, 1, 33], &rail("north_south"), false)?;
    lay(&mut b, [-12, 1, 34], &rail("north_east"), false)?;
    // ---- the three drop lines (east ends over the slots' slopes at x -3), glass under the last three
    let lines: [(i32, i32); 3] = [(28, -7), (31, -9), (34, -11)];
    for (z, x0) in lines {
        for x in x0..=-4 {
            lay(&mut b, [x, 1, z], &rail("east_west"), x >= -6)?;
        }
        b.put([-2, 1, z], STONE)?;
    }
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    Ok(added)
}
