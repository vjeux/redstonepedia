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
//! - the exit route: from the siding's exit east to x -13, a booster, south along x -13 (y 1) with
//!   a loop west (z 20..23, x -22) for drag, into T1 (-13, 1, 28), its stem north: unpowered
//!   north_east (east along z 28 to slot 14), powered north_west (west, round to T2 (-15, 1, 31),
//!   its stem north: unpowered east along z 31 to slot 13, powered west, round x -17, east along
//!   z 34 to slot 12);
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

/// The siding's paste offset (siding cell = loop cell - offset): ironspec and farmopt 22:56, the
/// fold: its box (siding x -7..6, z -3..9) inside one chunk (loop x -22..-9, z 10..22) with no
/// chunk line through it (was [-20, 2, 10]); 2 higher than before, so its east side's redstone
/// (siding y -3..-2) keeps a clear layer over the release controller's lines (y -2..-1 along
/// x -11..-9).
/// Column 3 (c) (ironspec 2026-11-11: the shared bed >= 16 + margin from every rider: the siding
/// past the loop's far end, chunk z 58..73, 58f770cc's offset (2, 4, 64)); false: the side-by-side
/// fold (the siding west of the loop at (-15, 3, 13))
pub(crate) const FAR: bool = true;
/// The siding's turn (column 3 (c): half a turn, so its entry faces the climb along x 11 and its
/// exit leaves west onto the tree's stem x -6: no crossing between them) and its origin (its local
/// (0, 0, 0), the entry): (c)'s box x -4..9, z 60..72 (chunk z 58..73), F at z 73.
pub(crate) const SIDING_TURNS: u8 = if FAR { 2 } else { 0 };
pub(crate) const SIDING_AT: P3 = if FAR { [2, 6, 69] } else { [-15, 3, 13] };

/// A siding cell's place in the loop frame (its local cell turned, then moved to SIDING_AT).
pub(crate) fn siding_at(q: P3) -> P3 {
    let r = clock::rotate_pos(q, SIDING_TURNS);
    [r[0] + SIDING_AT[0], r[1] + SIDING_AT[1], r[2] + SIDING_AT[2]]
}
pub(crate) const DIVERT: P3 = [9, -4, 0];
// (column 3 (c): the top-fed tree on x -6..-4 over the register: the switches S1 (k1's line, z 31)
// and S2 (k2's, z 34) on the stem x -6 at y 3, their supports at y 2; each line's hold a braked
// slope at (-5, 2, z), its support (-5, 1, z) the kick control)
pub(crate) const T1: P3 = if FAR { [-6, 2, 31] } else { [-15, 0, 28] };
pub(crate) const T2: P3 = if FAR { [-6, 2, 34] } else { [-16, 0, 31] };
/// The branch's boosters (x along z -2, y -3; B2).
const BRANCH_BOOSTERS: [i32; 3] = [6, 0, -6];
/// The siding's release input (its local (4, -2, 0), turned with it).
pub(crate) const RELEASE: P3 = if SIDING_TURNS == 2 { [SIDING_AT[0] - 4, SIDING_AT[1] - 2, SIDING_AT[2]] } else { [SIDING_AT[0] + 4, SIDING_AT[1] - 2, SIDING_AT[2]] };
pub(crate) const HOLDS: [P3; 3] = if FAR { [[-5, 1, 28], [-5, 1, 31], [-5, 1, 34]] } else { [[-12, 0, 28], [-11, 0, 31], [-12, 0, 34]] };

fn rail(shape: &str) -> String {
    format!("minecraft:rail[shape={shape},waterlogged=false]")
}

fn booster(shape: &str) -> String {
    format!("minecraft:powered_rail[powered=true,shape={shape},waterlogged=false]")
}

fn comp(facing: &str, mode: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode={mode},powered=false]")
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
    let at = |q: &P3| siding_at(*q);
    let srot = |s: &str| clock::canon::rotate_state(s, SIDING_TURNS);
    // (measurement only, ironspec 2026-11-11: the integration gate's second night, every napper
    // failing: the siding's bed occupied)
    let occupied = std::env::var("MCTAS_BED_OCCUPIED").is_ok();
    for (q, st) in s.blocks.iter().filter(|(q, _)| inbox(q)) {
        if occupied && st.contains("_bed[") {
            b.put(at(q), &srot(&st.replace("occupied=false", "occupied=true")))?;
            continue;
        }
        b.put(at(q), &srot(st))?;
    }
    for (q, n) in s.block_entities.iter().filter(|(q, _)| inbox(q)) {
        d.block_entities.insert(at(q), n.clone());
    }
    for e in &s.entities {
        let q = [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32];
        if inbox(&q) {
            // (a point turns with its cell: half a turn takes x to 1 - x within the grid)
            let (x, z) = if SIDING_TURNS == 2 { (1.0 - e.pos[0], 1.0 - e.pos[2]) } else { (e.pos[0], e.pos[2]) };
            let p = [x + SIDING_AT[0] as f64, e.pos[1] + SIDING_AT[1] as f64, z + SIDING_AT[2] as f64];
            d.entities.push(EntitySpec { kind: e.kind.clone(), pos: p, nbt: e.nbt.clone() });
        }
    }
    // (a siding by itself (main's 248e34c6f, `siding: v1 wired alone`: "no launcher, approach,
    // run-out") carries no approach: the one the bench siding carried (local x -7 a booster on a
    // redstone block, x -6..-1 rails on glass, into its entry (0, 0, 0)) is laid where the pasted
    // siding has nothing)
    for lx in -7..=-1 {
        let (r, sup) = (at(&[lx, 0, 0]), at(&[lx, -1, 0]));
        if !b.blocks.contains_key(&r) {
            b.put(r, &srot(&if lx == -7 { booster("east_west") } else { rail("east_west") }))?;
            b.put(sup, if lx == -7 { RBLOCK } else { GLASS })?;
        }
    }
    if b.blocks.get(&RELEASE).map(|s| s.as_str()) != Some(GLASS) {
        bail!("the siding's release input {RELEASE:?} is {:?}, not glass: the siding file changed", b.blocks.get(&RELEASE));
    }
    // the siding releases each napper as soon as it is back in its cart (farmopt's release-held
    // mode, benched by his child: every napper re-boards and leaves): its release input is a
    // redstone block; the timed release is the hold near T1 (ironfarm 14:35)
    b.blocks.insert(RELEASE, RBLOCK.into());
    // the bed's port (R1's home: the runner gives the bed to each napper from it; ironfarm 14:00)
    let home = s.ports.get("home").copied().ok_or_else(|| anyhow::anyhow!("the siding file has no `port home:`"))?;
    d.ports.insert("home".into(), at(&home));
    // the siding's force-re-board input (58f770cc's gated siding, `force=x,y,z` on its `siding:`
    // line): it must be where the module's F line ends (FORCE)
    if let Some(f) = s.meta.get("siding").and_then(|l| l.split_whitespace().find_map(|w| w.strip_prefix("force="))) {
        let v: Vec<i32> = f.split(',').map(|n| n.parse()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("the siding's force={f}: {e}"))?;
        if v.len() != 3 || at(&[v[0], v[1], v[2]]) != FORCE {
            bail!("the siding's force input {f} (local) is not at the module's F line's end {FORCE:?} (world)");
        }
    }
    // the siding's own build list (`build: last x,y,z ...`, its cells placed after all others;
    // `fills-last` may stand anywhere in it), moved into the loop's frame; joined with the drop
    // ends' below
    let mut last: Vec<P3> = Vec::new();
    let mut fills_last = false;
    if let Some(bl) = s.meta.get("build") {
        let toks: Vec<&str> = bl.split_whitespace().collect();
        if !toks.contains(&"last") {
            bail!("the siding's `build: {bl}` is not a `last` list");
        }
        fills_last |= toks.contains(&"fills-last");
        for c in toks.into_iter().filter(|t| *t != "last" && *t != "fills-last") {
            let v: Vec<i32> = c.split(',').map(|n| n.parse()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("the siding's build cell {c}: {e}"))?;
            if v.len() != 3 {
                bail!("the siding's build cell {c} is not x,y,z");
            }
            last.push(at(&[v[0], v[1], v[2]]));
        }
    }
    // ---- the diverter: the branch north of the corner (the corner's support stays the glass
    // control the bench switches)
    match b.blocks.get(&[9, -3, 0]).map(|s| s.as_str()) {
        Some(s) if s.contains("shape=south_west") => {}
        other => bail!("the departure corner (9, -3, 0) is {other:?}, not a south_west rail: the loop file changed"),
    }
    lay(&mut b, [9, -3, -1], &rail("north_south"), false)?;
    if FAR {
        far_entry(&mut b)?;
    } else {
        side_entry(&mut b)?;
    }
    if FAR {
        far_exit(&mut b)?;
    } else {
        side_exit(&mut b)?;
    }
    if FAR {
        far_tree(&mut b)?;
    } else {
    // T1: stem north, unpowered north_east (east: slot 14's line), powered north_west (T2)
    b.put([-15, 1, 28], &rail("north_east"))?;
    b.put(T1, GLASS)?;
    // T1 west to T2 (-16, 1, 31), stem north: unpowered east (slot 13's line), powered west (the
    // tight tree, B1: T1's west arm turns south at once, T2 one cell west of T1, slot 12's line
    // round x -17: k1's path 2 cells and k2's 4 cells shorter than the first tree's)
    lay(&mut b, [-16, 1, 28], &rail("south_east"), false)?;
    lay(&mut b, [-16, 1, 29], &rail("north_south"), false)?;
    lay(&mut b, [-16, 1, 30], &rail("north_south"), false)?;
    b.put([T2[0], 1, T2[2]], &rail("north_east"))?;
    b.put(T2, GLASS)?;
    // T2 west round to z 34 (slot 12's line)
    lay(&mut b, [-17, 1, 31], &rail("south_east"), false)?;
    lay(&mut b, [-17, 1, 32], &rail("north_south"), false)?;
    lay(&mut b, [-17, 1, 33], &rail("north_south"), false)?;
    lay(&mut b, [-17, 1, 34], &rail("north_east"), false)?;
    // ---- the three drop lines, each with its hold (ironfarm 14:42: one hold per line, a short
    // leg from the hold to the drop): flat east, a climb ascending east (hx - 2, 1, z), flat
    // (hx - 1, 2, z), the hold (hx, 1, z) (a braked powered rail ascending west: the cart rolls down
    // onto it and stops; its support (hx, 0, z) is the kick control), flat east (hx + 1..-4, 1, z)
    // (glass under the last three), the stone stop (-2, 1, z). Slot 13's line (k1) has its climb,
    // flat and hold one cell east (hx -11): with the tight tree's T2 at x -16 that gives it the first
    // tree's two flats before its climb, so it comes onto its hold at the first tree's speed and
    // rests where it did (ironfarm 18:25: slot 13 fails from a 0.37 roll-off, and the tight tree
    // with the hold at x -12 rolled k1 off at 0.36)
    let lines: [(i32, i32); 3] = [(28, -14), (31, -15), (34, -16)];
    for (i, (z, x0)) in lines.into_iter().enumerate() {
        let hx = HOLDS[i][0];
        for x in x0..=hx - 3 {
            lay(&mut b, [x, 1, z], &rail("east_west"), false)?;
        }
        lay(&mut b, [hx - 2, 1, z], &rail("ascending_east"), false)?;
        lay(&mut b, [hx - 1, 2, z], &rail("east_west"), false)?;
        b.put([hx, 1, z], "minecraft:powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
        b.put([hx, 0, z], GLASS)?;
        for x in hx + 1..=-4 {
            lay(&mut b, [x, 1, z], &rail("east_west"), x >= -6)?;
        }
        // (the stop glass: a cart stop all the same, and no golem can spawn on it; ironfarm 16:30)
        b.put([-2, 1, z], GLASS)?;
    }
    // (k1's entry tuning, measured: `MCTAS_K1_BOOST` = flat | climb | top, one of slot 13's
    // line's cells before its hold an always-on booster)
    if let Ok(v) = std::env::var("MCTAS_K1_BOOST") {
        let (c, shape) = match v.as_str() {
            "flat" => ([-15, 1, 31], "east_west"),
            "climb" => ([-14, 1, 31], "ascending_east"),
            "top" => ([-13, 2, 31], "east_west"),
            _ => bail!("MCTAS_K1_BOOST: flat, climb or top"),
        };
        b.blocks.insert(c, booster(shape));
        b.blocks.insert([c[0], c[1] - 1, c[2]], RBLOCK.into());
    }
    }
    // the always-on redstone blocks (the climbs', the boosters') must touch nothing that carries a
    // signal besides their own rail: every redstone block this part added
    let in_siding = |p: &P3| (-7..=6).contains(&(p[0] - SIDING_AT[0])) && (-4..=4).contains(&(p[1] - SIDING_AT[1])) && (-3..=9).contains(&(p[2] - SIDING_AT[2]));
    let always_on: Vec<P3> = b.blocks.iter().filter(|(p, s)| !before.contains(*p) && s.as_str() == RBLOCK && !in_siding(p)).map(|(p, _)| *p).collect();
    for rb in always_on {
        for dlt in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]] {
            let q = [rb[0] + dlt[0], rb[1] + dlt[1], rb[2] + dlt[2]];
            if let Some(s) = b.blocks.get(&q) {
                // (an always-on booster beside it, part of the same powered climb, is meant)
                if s.contains("powered_rail[powered=true") {
                    continue;
                }
                if ["redstone_wire", "repeater", "comparator", "torch", "rail", "piston", "dropper", "hopper", "observer", "lamp", "bulb", "door", "trapdoor", "fence_gate"].iter().any(|k| s.contains(k)) {
                    bail!("the always-on block {rb:?} touches {q:?} = {s}");
                }
            }
        }
    }
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    // the drop ends go in after every other rail (ironfarm 15:13): placed by (y, x, z) they would
    // come before the slots' peaks, and each slot's slope, with one link so far, would take its
    // drop end (seen built in order: the slopes turned ascending_west)
    let drop_ends: [P3; 3] = if FAR { [[-4, 2, 28], [-4, 2, 31], [-4, 2, 34]] } else { [[-4, 1, 28], [-4, 1, 31], [-4, 1, 34]] };
    let mut cells: Vec<String> = drop_ends.iter().chain(last.iter()).map(|q| format!("{},{},{}", q[0], q[1], q[2])).collect();
    // (a build list the loop file already has comes first)
    if let Some(prev) = d.meta.get("build") {
        let toks: Vec<&str> = prev.split_whitespace().collect();
        if !toks.contains(&"last") {
            bail!("the loop's `build: {prev}` is not a `last` list");
        }
        fills_last |= toks.contains(&"fills-last");
        cells.splice(0..0, toks.into_iter().filter(|t| *t != "last" && *t != "fills-last").map(|c| c.to_string()));
    }
    cells.dedup();
    d.meta.insert("build".into(), format!("last {}{}", cells.join(" "), if fills_last { " fills-last" } else { "" }));
    Ok(added)
}

/// The side-by-side entry: west along z -2 to the climb's column ex, two cells west of the siding's
/// box, the climb south up to the siding's level, south along ex to the siding's z, east into its
/// entry.
fn side_entry(b: &mut B) -> Result<()> {
    let (ex, ez) = (SIDING_AT[0] - 9, SIDING_AT[2]);
    lay(b, [9, -3, -2], &rail("south_west"), false)?;
    for x in ex + 1..=8 {
        lay(b, [x, -3, -2], &rail("east_west"), false)?;
    }
    // (B2: boosters on the branch, held on by redstone blocks for supports: the stand's front cart
    // (k0) leaves the corner slowest and reached the siding last, +204 against k1 +182 and k2 +192;
    // boosted, all three arrive sooner and closer together, so the force-re-board F (one time for
    // every k: the latest arrival + 112) can come earlier)
    for &x in &BRANCH_BOOSTERS {
        b.blocks.insert([x, -3, -2], booster("east_west"));
        b.blocks.insert([x, -4, -2], RBLOCK.into());
    }
    lay(b, [ex, -3, -2], &rail("south_east"), false)?;
    lay(b, [ex, -3, -1], &rail("north_south"), false)?;
    // the climb: boosters ascending south from the branch (y -3) to the siding's entry level, the
    // first one's support a redstone block (it powers them all through the rail line, up to 8)
    let climb = SIDING_AT[1] + 3;
    for k in 0..climb {
        let p = [ex, -3 + k, k];
        b.put(p, &booster("ascending_south"))?;
        b.put([p[0], p[1] - 1, p[2]], if k == 0 { RBLOCK } else { STONE })?;
    }
    for z in climb..ez {
        lay(b, [ex, SIDING_AT[1], z], &rail("north_south"), false)?;
    }
    lay(b, [ex, SIDING_AT[1], ez], &rail("north_east"), false)?;
    lay(b, [ex + 1, SIDING_AT[1], ez], &rail("east_west"), false)?;
    Ok(())
}

/// The side-by-side exit to T1: the exit's corner, slopes down south to y 1 (the cart leaves the
/// exit slot at 0.28-0.44 and would crawl on the flat: ironfarm 13:57), south along the exit's
/// column (over the release controller's chain: its cells there are y -2..-1, the rails' supports
/// at y 0 touch none of its dust) to z 25, west along z 26 to x -15, south into T1's stem.
fn side_exit(b: &mut B) -> Result<()> {
    let (xx, xz) = (SIDING_AT[0] + 7, SIDING_AT[2]);
    let xy = SIDING_AT[1] - 1;
    lay(b, [xx, xy, xz], &rail("south_west"), false)?;
    // (slopes down to y 1, then flat rails to the tree. A booster on its redstone block after the
    // slopes, or before them, left k0 slower at its drop end (0.06 b/t at +555, 600 problems in 70
    // slots), so none)
    for k in 1..xy {
        lay(b, [xx, xy - k, xz + k], &rail("ascending_north"), false)?;
    }
    for z in xz + xy..=25 {
        lay(b, [xx, 1, z], &rail("north_south"), false)?;
    }
    lay(b, [xx, 1, 26], &rail("north_west"), false)?;
    for x in -14..xx {
        lay(b, [x, 1, 26], &rail("east_west"), false)?;
    }
    lay(b, [-15, 1, 26], &rail("south_east"), false)?;
    lay(b, [-15, 1, 27], &rail("north_south"), false)?;
    Ok(())
}

/// Column 3 (c)'s entry, from the divert branch (9, -3, -1) to the turned siding past the far end:
/// east at z -2 to x 11, a booster climb south along x 11 to the siding's level (the third
/// booster's support a redstone block: its neighbours are the climb's own boosters, not the divert
/// drive's dust (11, -4, 0) or repeater (10, -4, 0)), flat along x 11 (a booster on a redstone
/// block at z 34) to z 68, a corner west at z 69 into the bench approach (its corner cell: local
/// x -8, turned: x 10).
fn far_entry(b: &mut B) -> Result<()> {
    let y = SIDING_AT[1];
    lay(b, [9, -3, -2], &rail("south_east"), false)?;
    lay(b, [10, -3, -2], &rail("east_west"), false)?;
    lay(b, [11, -3, -2], &rail("south_west"), false)?;
    let n = y + 3;
    for k in 0..n {
        let p = [11, -3 + k, -1 + k];
        b.put(p, &booster("ascending_south"))?;
        b.put([p[0], p[1] - 1, p[2]], if k == 2 { RBLOCK } else { STONE })?;
    }
    for z in n - 1..SIDING_AT[2] {
        lay(b, [11, y, z], &rail("north_south"), false)?;
    }
    b.blocks.insert([11, y, 34], booster("north_south"));
    b.blocks.insert([11, y - 1, 34], RBLOCK.into());
    lay(b, [11, y, SIDING_AT[2]], &rail("north_west"), false)?;
    let corner = siding_at([-8, 0, 0]);
    for x in corner[0]..11 {
        lay(b, [x, y, SIDING_AT[2]], &rail("east_west"), false)?;
    }
    Ok(())
}

/// Column 3 (c)'s exit, from the turned siding's exit (its local (6, -1, 0): x -4, leaving west):
/// a corner north onto the tree's stem column x -6, north along it (3 clear of the register lane
/// x -3), two slopes down north to the tree's level (y 3).
fn far_exit(b: &mut B) -> Result<()> {
    let ex = siding_at([6, -1, 0]);
    let y = ex[1];
    lay(b, [ex[0] - 1, y, ex[2]], &rail("east_west"), false)?;
    lay(b, [-6, y, ex[2]], &rail("north_east"), false)?;
    for z in 41..ex[2] {
        lay(b, [-6, y, z], &rail("north_south"), false)?;
    }
    for k in 1..=y - 3 {
        lay(b, [-6, y - k, 41 - k], &rail("ascending_south"), false)?;
    }
    for z in 35..=40 - (y - 3) {
        lay(b, [-6, 3, z], &rail("north_south"), false)?;
    }
    Ok(())
}

/// Column 3 (c)'s top-fed tree on x -6..-4 over the register (ironspec 2026-11-11: drop height 2,
/// holds stay): the stem north along x -6 at y 3, S2 (-6, 3, 34) (k2's line east), S1 (-6, 3, 31)
/// (k1's), the end curve (-6, 3, 28) (k0's); each line a hold (-5, 2, z) (a braked powered rail
/// ascending west: the cart comes off the stem down onto it and stops; its support (-5, 1, z) the
/// kick control) and a plain ledge end (-4, 2, z) on glass, off which the cart drops 2 onto the
/// slot's brake (-3, 0, z). (No floor booster next to the hold: powered rails pass power along a
/// line, so the hold would never brake; the kick alone sets the ledge speed, to be measured
/// against ironfarm's 0.20..0.40 b/t.)
fn far_tree(b: &mut B) -> Result<()> {
    b.put([-6, 3, 34], &rail("north_south"))?;
    b.put(T2, GLASS)?;
    lay(b, [-6, 3, 33], &rail("north_south"), false)?;
    lay(b, [-6, 3, 32], &rail("north_south"), false)?;
    b.put([-6, 3, 31], &rail("north_south"))?;
    b.put(T1, GLASS)?;
    lay(b, [-6, 3, 30], &rail("north_south"), false)?;
    lay(b, [-6, 3, 29], &rail("north_south"), false)?;
    lay(b, [-6, 3, 28], &rail("south_east"), false)?;
    for (i, z) in [28, 31, 34].into_iter().enumerate() {
        b.put([-5, 2, z], "minecraft:powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
        b.put(HOLDS[i], GLASS)?;
        lay(b, [-4, 2, z], &rail("east_west"), true)?;
        // (no backstop beyond the slot: a cart off the ledge at 0.22 b/t hit glass at x -2 before
        // it fell and rolled back onto the hold; ironfarm's slot_drop has none: from this ledge
        // (its h 3: the slot cell at y -1) 0.20..0.30 b/t lands on the slot)
    }
    Ok(())
}

/// Column 3 (c)'s switch drive at a switch's support `sup` (S1 (-6, 2, 31), S2 (-6, 2, 34)), its line
/// arriving at (sup.x - 5, sup.y, sup.z): an inverter (a block (x - 4), a wall torch on its east
/// face (x - 3)), a dust (x - 2), a repeater (x - 1) east into the support. (A plain rail at a
/// three-way junction rests on the south-east curve: unpowered, the cart turns east into the line;
/// the line (T1 = not k0, T2 = k2) inverted powers the straight way for every other cart. Stone
/// over the holds' kick rows beside the line's dusts (z - 1) keeps them from joining as a stair.)
fn far_t_drive(b: &mut B, sup: P3) -> Result<()> {
    let [x, y, z] = sup;
    b.put([x - 4, y, z], STONE)?;
    b.put([x - 3, y, z], "minecraft:redstone_wall_torch[facing=east,lit=true]")?;
    b.on([x - 2, y, z], crate::schedgen::DUST)?;
    b.on([x - 1, y, z], &crate::schedgen::rep("west", 1))?;
    b.blocks.insert(sup, STONE.into());
    for dx in [5, 2] {
        b.put([x - dx, y, z - 1], STONE)?;
    }
    Ok(())
}

/// `mctas circuit memory siding-ctl-gen --out F`: the siding's control module, in the nightly
/// scheduler's own frame (the scheduler stamped at 0 0 0; additions east of it, x >= 2, and under
/// it, y -2), driven by REL alone (WIP, part 1: the scheduler's REL and night inputs, the visit
/// pulse VP out).
/// - REL: `input.rel` (12, -1, -4), a bus west along z -4 at y -1 under the scheduler's input row,
///   up into a dust (-5, 0, -4) and a repeater on the scheduler's input cell (-5, 0, -3).
/// - Night: the scheduler's window cell (-2, 0, 1) powered from below by a torch (-2, -1, 1) on a
///   block (-2, -2, 1) that the day line powers through a repeater (-1, -2, 1) from `input.day`
///   (0, -2, 1) (in the loop: a normal daylight detector, > 0 by day).
/// - VP, the visit pulse (the scheduler's gate output, high from the release's start while the
///   release is a visit): two repeaters off V's stone (-5, 0, 4) east into a block (-2, 0, 4), a
///   dust under it, down to y -2, south along x -1 and east along z 11 to `output.vp`.
pub fn control_blocks(sched: &Path) -> Result<B> {
    use crate::schedgen::{DUST, rep};
    let mut b = B { blocks: Default::default(), ports: Default::default(), nbt: Default::default() };
    b.stamp(sched, [0, 0, 0], 0, None)?;
    // (night 1, ironspec 18:26: the D ring's token is built in the hopper h13 (1, 0, 0) that feeds
    // d0, not in d0: the day line locks h13 (`wire_control`), so the token waits there all day and
    // drops into d0 as the night window opens: the night's first release finds D = 0 and visits,
    // every night from the first, and from a power-on at any time of day)
    if let Some(tok) = b.nbt.remove(&[0, 0, 0]) {
        b.nbt.insert([1, 0, 0], tok);
    } else {
        bail!("the scheduler's D ring has no token in d0 (0, 0, 0)");
    }
    // (the scheduler's slot-out stage does nothing in the farm: its visit latch C, the reset path
    // into it from the 3rd stage's end, the step line along x -7, and the slot-out module with its
    // connections (every cell at z >= 9); removed, ironspec 19:52: faults there changed nothing)
    let slot_out: Vec<P3> = b
        .blocks
        .keys()
        .copied()
        .filter(|&[x, y, z]| {
            (z >= 9 && x <= 0)
                || ((-5..=-3).contains(&x) && (5..=8).contains(&z) && (-1..=0).contains(&y))
                || (z == 6 && (-13..=-6).contains(&x) && (-1..=0).contains(&y))
                || (x == -13 && z == 5 && (-1..=0).contains(&y))
                || (x == -7 && (6..=17).contains(&z) && (0..=1).contains(&y))
        })
        .collect();
    for p in &slot_out {
        b.blocks.remove(p);
        b.nbt.remove(p);
    }
    for k in ["output.s0", "output.s1", "output.s2", "output.v"] {
        b.ports.remove(k);
    }
    // ---- REL into the scheduler (its input cell becomes a repeater); the input block (14, -2, -4)
    // feeds the bus (up to y -1 at x 12) and REL2 south along x 14 for the siding logic
    b.blocks.remove(&[-5, 0, -3]);
    b.on([-5, 0, -3], &rep("north", 1))?;
    b.on([-5, 0, -4], DUST)?;
    b.put([14, -2, -4], STONE)?;
    b.ports.insert("input.rel".into(), [14, -2, -4]);
    b.on([13, -2, -4], DUST)?;
    b.on([12, -1, -4], DUST)?;
    let bus: Vec<P3> = (-4..=11).rev().map(|x| [x, -1, -4]).collect();
    b.wire([12, -1, -4], &bus, 12)?;
    // ---- night: the torch under the window cell, off while the day line powers its block
    b.put([0, -2, 1], STONE)?;
    b.ports.insert("input.day".into(), [0, -2, 1]);
    b.on([-1, -2, 1], &rep("east", 1))?;
    b.put([-2, -2, 1], STONE)?;
    b.put([-2, -1, 1], "minecraft:redstone_torch[lit=true]")?;
    // ---- NV = NOT (D = 0 and night), steady between releases: the gate's side dust (-4, 0, 1)
    // read by a repeater south into a block (-4, 0, 3), a dust under it, down to y -2 and east along
    // z 3 to `output.nv` (the visit gate itself, V's stone, comes ~13 gt after REL: too late for
    // the diverter, so the release is known a visit from NV sampled at its start)
    b.on([-4, 0, 2], &rep("north", 1))?;
    b.put([-4, 0, 3], STONE)?;
    b.on([-4, -1, 3], DUST)?;
    let nv: Vec<P3> = (-3..=3).map(|x| [x, -2, 3]).collect();
    b.wire([-4, -1, 3], &nv, 0)?;
    b.ports.insert("output.nv".into(), [3, -2, 3]);
    // (v itself is not used: the visit is G2 below, the windows hang off it)
    // ---- G2 = REL and not NV (NV is still the release's own value while REL is high: the D ring
    // steps ~19 gt after REL), a 12-gt pulse at the start of every visit release; the windows W0,
    // W1 = W0 + 10, W2 = W0 + 20 down a repeater chain along x 14, each tapped by a block T_k
    // (14, -2, z_k), z_k = 6, 10, 14
    for z in -3..=2 {
        b.on([14, -2, z], DUST)?;
    }
    b.on([4, -2, 3], &rep("west", 1))?;
    for x in 5..=12 {
        b.on([x, -2, 3], DUST)?;
    }
    b.on([13, -2, 3], &rep("west", 1))?;
    b.on([14, -2, 3], &comp("north", "subtract"))?;
    // the chain: four repeaters from G2 to T0 (the windows' offset, set by `w0`), then 10 gt between
    // taps (repeaters 3 + 1 + 1: no dust in the chain, so nothing beside it leaks in)
    let w0 = [1u8, 1, 1, 1];
    for (i, d) in w0.iter().enumerate() {
        b.on([14, -2, 4 + i as i32], &rep("north", *d))?;
    }
    // T0 (14, -2, 8): W0's tap, which fires the visit-delay register's INJ (below). (The module's
    // own divert windows W1, W2 and their P_k gates, the OR column to `output.divert`, the visit
    // counter R' with its taps and its step line, and the router lines `output.t1`/`output.t2`
    // were removed, ironspec 19:52: the local divert unit (`divert_unit`) has its own windows,
    // ring R'' and router, and fault-survey-v1 showed faults in R' changing nothing.)
    b.put([14, -2, 8], STONE)?;
    // ---- the visit-delay register: a ring of droppers D0 (INJ) .. D5 and hoppers, D0 (18, -2, -2)
    // east along z -2 to H2 (23), back west along z -1 to H5 (18) into D0. The REL clock is a dust
    // patch on the tops of D1..D5 and the hoppers between them (hoppers locked while it is high, so
    // every item moves exactly one stage a release); INJ fires on W0 (a visit) through the line
    // along x 16 from T0's east repeater. 3 items in INJ. D5 holds a visit's item during the 4th
    // release after it: SREL = D5 and RELd.
    let reg: [(P3, &str); 12] = [
        ([18, -2, -2], "minecraft:dropper[facing=east,triggered=false]"),
        ([19, -2, -2], "minecraft:hopper[enabled=true,facing=east]"),
        ([20, -2, -2], "minecraft:dropper[facing=east,triggered=false]"),
        ([21, -2, -2], "minecraft:hopper[enabled=true,facing=east]"),
        ([22, -2, -2], "minecraft:dropper[facing=east,triggered=false]"),
        ([23, -2, -2], "minecraft:hopper[enabled=true,facing=south]"),
        ([23, -2, -1], "minecraft:dropper[facing=west,triggered=false]"),
        ([22, -2, -1], "minecraft:hopper[enabled=true,facing=west]"),
        ([21, -2, -1], "minecraft:dropper[facing=west,triggered=false]"),
        ([20, -2, -1], "minecraft:hopper[enabled=true,facing=west]"),
        ([19, -2, -1], "minecraft:dropper[facing=west,triggered=false]"),
        ([18, -2, -1], "minecraft:hopper[enabled=true,facing=north]"),
    ];
    for (p, s) in reg {
        b.put(p, s)?;
    }
    b.nbt.insert([18, -2, -2], "{Items:[{Slot:0b,id:\"minecraft:stone\",count:3}]}".into());
    for p in [[20, -1, -2], [21, -1, -2], [22, -1, -2], [23, -1, -2], [23, -1, -1], [22, -1, -1], [21, -1, -1], [20, -1, -1], [19, -1, -1]] {
        b.put(p, DUST)?;
    }
    b.on([24, -1, -1], &rep("east", 1))?;
    // INJ's trigger: T0 -> a repeater east into a block (16, -2, 8), dust north along x 16, a
    // repeater east into INJ
    b.on([15, -2, 8], &rep("west", 1))?;
    b.put([16, -2, 8], STONE)?;
    let inj: Vec<P3> = (-2..=7).rev().map(|z| [16, -2, z]).collect();
    b.wire([16, -2, 8], &inj, 0)?;
    b.on([17, -2, -2], &rep("west", 1))?;
    // the REL branch: east along z -6, the clock south along x 25 and up into the patch's repeater;
    // RELd on along z -6 to x 27 and south along x 27 (its delay), west along z 2
    // (the branch keeps off (15, -2, -5..-6): the loop's REL line feeds the input block from the
    // north through (14, -2, -6) and a repeater, and must not touch it, or it latches)
    b.on([15, -2, -4], DUST)?;
    b.on([16, -2, -4], DUST)?;
    b.on([16, -2, -5], DUST)?;
    let relb: Vec<P3> = (16..=25).map(|x| [x, -2, -6]).collect();
    b.wire([16, -2, -5], &relb, 8)?;
    for z in -5..=-2 {
        b.on([25, -2, z], DUST)?;
    }
    b.on([25, -1, -1], DUST)?;
    // (RELd and SA moved out: the hold's kick unit near T1 takes REL from the core; the module
    // only says which release is the drop's, steadily)
    // ---- NS5 = NOT (a visit's item in D5): D5's comparator (south), a repeater into a block, a
    // torch on it, south to z 5 and out east along z 5 (`output.ns5`); high except from the clock
    // of the 4th release after a visit to the clock of the 5th
    b.on([19, -2, 0], &comp("north", "compare"))?;
    b.put([19, -2, 1], STONE)?;
    b.put([19, -2, 2], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    for z in 3..=5 {
        b.on([19, -2, z], DUST)?;
    }
    for x in 20..=27 {
        b.on([x, -2, 5], DUST)?;
    }
    b.ports.insert("output.ns5".into(), [27, -2, 5]);
    // ---- NS3 = NOT (a visit's item in D3): the same off D3 (23, -2, -1), out east along z 3
    // (`output.ns3`): low from the clock of the 2nd release after a visit to the clock of the 3rd,
    // which holds the 3rd release's +20 (the siding's force-re-board time, F)
    b.on([23, -2, 0], &comp("north", "compare"))?;
    b.put([23, -2, 1], STONE)?;
    b.put([23, -2, 2], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    b.on([23, -2, 3], DUST)?;
    for x in 24..=28 {
        b.on([x, -2, 3], DUST)?;
    }
    b.ports.insert("output.ns3".into(), [28, -2, 3]);
    // ---- NV out: a dust on NV's block (-4, 0, 3), up two stairs to y 3 (over the D ring), east
    // along z 3 to `output.nv` (28, 4, 3): at y 4, two above the D ring's loop dust (y 1): a line
    // at y 3 strongly powers its support stones at y 2, which sit right over the loop dust and so
    // count for the ring's droppers' quasi-connectivity (they jammed the ring)
    b.on([-4, 1, 3], DUST)?;
    b.on([-3, 2, 3], DUST)?;
    b.on([-2, 3, 3], DUST)?;
    let nvo: Vec<P3> = (-1..=28).map(|x| [x, 4, 3]).collect();
    b.wire([-2, 3, 3], &nvo, 10)?;
    b.ports.insert("output.nv".into(), [28, 4, 3]);
    Ok(b)
}

/// The module written alone (its ports driven by the bench: `door drive`).
pub fn control_gen(out: &Path, sched: &Path) -> Result<()> {
    let b = control_blocks(sched)?;
    let mut d = Design::parse("name: carousel-siding-control\n")?;
    d.blocks = b.blocks;
    d.block_entities = b.nbt;
    d.ports = b.ports;
    d.save(out)?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}

/// A wire through `cells` (each the next horizontally, or one up/down and one over: a stair), a
/// repeater every `every` cells on straight flat runs pointing along it; refuses a cell whose
/// neighbours (same level, or a stair's reach) hold dust that is not this wire's, and a stair
/// whose upper landing has something solid over the lower cell.
pub(crate) fn route(b: &mut B, from: P3, cells: &[P3], every: usize) -> Result<()> {
    route_d(b, from, cells, every, every, 1)
}

/// `route` with the dust count since the source's last repeater (`run0`: 0 right after a repeater,
/// `every` for "take one on the first flat cell") and the repeaters' delay.
pub(crate) fn route_d(b: &mut B, from: P3, cells: &[P3], every: usize, run0: usize, delay: u8) -> Result<()> {
    use crate::schedgen::{DUST, rep};
    let mine: std::collections::BTreeSet<P3> = cells.iter().copied().chain(std::iter::once(from)).collect();
    let mut prev = from;
    // dust cells since the last repeater (starting full: the first flat cell takes one, whatever the
    // source's strength): a repeater goes on the first flat cell (with a flat cell after it) once
    // `every` dust cells have passed, stairs included
    let mut run = run0;
    for (i, &c) in cells.iter().enumerate() {
        let (dx, dy, dz) = (c[0] - prev[0], c[1] - prev[1], c[2] - prev[2]);
        if dx.abs() + dz.abs() != 1 || dy.abs() > 1 {
            bail!("route: {prev:?} -> {c:?} is not a step");
        }
        if dy != 0 {
            let lower = if dy > 0 { prev } else { c };
            let over = [lower[0], lower[1] + 1, lower[2]];
            if b.blocks.get(&over).is_some_and(|s| !s.contains("air")) && !mine.contains(&over) {
                bail!("route: the stair {prev:?} -> {c:?} has {:?} over its lower cell", b.blocks.get(&over));
            }
        }
        for n in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1], [1, -1, 0], [-1, -1, 0], [0, -1, 1], [0, -1, -1]] {
            let q = [c[0] + n[0], c[1] + n[1], c[2] + n[2]];
            // (a diagonal is no connection when a stone cuts it: for a dust one lower, the cell
            // beside this dust above it; for one higher, the cell over this dust)
            let cut = match n[1] {
                -1 => Some([c[0] + n[0], c[1], c[2] + n[2]]),
                1 => Some([c[0], c[1] + 1, c[2]]),
                _ => None,
            };
            if cut.is_some_and(|k| b.blocks.get(&k).is_some_and(|s| s == STONE)) {
                continue;
            }
            if !mine.contains(&q) && b.blocks.get(&q).is_some_and(|s| s.contains("redstone_wire")) {
                bail!("route: {c:?} would touch foreign dust at {q:?} ({})", b.blocks[&q]);
            }
        }
        // (a repeater only on a straight flat cell: the next cell one more step the same way)
        let next_straight = cells.get(i + 1).is_some_and(|n| n[1] == c[1] && n[0] - c[0] == dx && n[2] - c[2] == dz);
        let prev_flat = dy == 0;
        if every > 0 && run >= every && prev_flat && next_straight {
            let facing = match (dx, dz) {
                (1, 0) => "west",
                (-1, 0) => "east",
                (0, 1) => "north",
                _ => "south",
            };
            b.on(c, &rep(facing, delay))?;
            run = 0;
        } else {
            b.on(c, DUST)?;
            run += 1;
            if run > 14 {
                bail!("route: dust {c:?} is {run} cells past its last repeater (dead)");
            }
        }
        prev = c;
    }
    Ok(())
}

/// A repeater's `rep` side for a signal stepping by `d` into it (route_d's convention).
pub(crate) fn facing_in(d: P3) -> &'static str {
    match (d[0], d[2]) {
        (1, 0) => "west",
        (-1, 0) => "east",
        (0, 1) => "north",
        _ => "south",
    }
}

/// Lays `cells` from `from` (a source whose signal is `run0` dust old at `from`) with repeaters
/// on straight flat cells, at least one every 14 dust, their delays adding up to exactly
/// `total_rt` (spread evenly, 1..4 each; more repeaters where the total needs them). Returns the
/// repeaters' cells.
pub(crate) fn lay_exact(b: &mut B, from: P3, cells: &[P3], run0: usize, total_rt: u8) -> Result<Vec<P3>> {
    use crate::schedgen::rep;
    let straight = |i: usize| -> bool {
        let p = if i == 0 { from } else { cells[i - 1] };
        let c = cells[i];
        let Some(n) = cells.get(i + 1) else { return false };
        c[1] == p[1] && n[1] == c[1] && [n[0] - c[0], n[2] - c[2]] == [c[0] - p[0], c[2] - p[2]] && (c[0] - p[0]).abs() + (c[2] - p[2]).abs() == 1
    };
    let mut reps: Vec<usize> = Vec::new();
    let mut cand: Option<usize> = None;
    for i in 0..cells.len() {
        if straight(i) {
            cand = Some(i);
        }
        let since = match reps.last() {
            Some(&r) => i - r,
            None => i + 1 + run0,
        };
        if since > 14 {
            let c = cand.filter(|&c| reps.last().is_none_or(|&r| c > r)).ok_or_else(|| anyhow::anyhow!("lay_exact: no straight flat cell for a repeater before {:?}", cells[i]))?;
            reps.push(c);
            cand = None;
        }
    }
    let mut k = reps.len();
    if k == 0 && total_rt == 0 {
        route_d(b, from, cells, 0, run0, 1)?;
        return Ok(Vec::new());
    }
    if k == 0 && total_rt > 0 {
        let c = (0..cells.len()).find(|&i| straight(i)).ok_or_else(|| anyhow::anyhow!("lay_exact: no straight flat cell at all on the {}-cell path {from:?} -> {:?}: {cells:?}", cells.len(), cells.last()))?;
        reps.push(c);
        k = 1;
    }
    if (k as u8) > total_rt {
        bail!("lay_exact: the path needs {k} repeaters, more than {total_rt} rt");
    }
    while (k as u8) * 4 < total_rt {
        let c = (0..cells.len()).find(|&i| straight(i) && !reps.contains(&i)).ok_or_else(|| anyhow::anyhow!("lay_exact: no room for {total_rt} rt"))?;
        reps.push(c);
        reps.sort();
        k += 1;
    }
    let base = total_rt / k as u8;
    let rem = total_rt as usize % k;
    let mut start = 0;
    let mut src = from;
    let mut placed = Vec::new();
    for (j, &r) in reps.iter().enumerate() {
        if r > start {
            route_d(b, src, &cells[start..r], 0, 0, 1)?;
        }
        let p = if r == 0 { from } else { cells[r - 1] };
        let c = cells[r];
        b.on(c, &rep(facing_in([c[0] - p[0], 0, c[2] - p[2]]), base + u8::from(j < rem)))?;
        placed.push(c);
        src = c;
        start = r + 1;
    }
    if start < cells.len() {
        route_d(b, src, &cells[start..], 0, 0, 1)?;
    }
    Ok(placed)
}

pub(crate) fn seg_to(cells: &mut Vec<P3>, to: P3) {
    let mut c = *cells.last().expect("seg_to after a start");
    while c != to {
        let s = |a: i32, b: i32| (b - a).signum();
        if c[1] != to[1] {
            c = [c[0] + s(c[0], to[0]), c[1] + s(c[1], to[1]), c[2] + if c[0] == to[0] { s(c[2], to[2]) } else { 0 }];
        } else if c[0] != to[0] {
            c = [c[0] + s(c[0], to[0]), c[1], c[2]];
        } else {
            c = [c[0], c[1], c[2] + s(c[2], to[2])];
        }
        cells.push(c);
    }
}

/// The wired pocket stand's REL (farmopt's gates, `queue-loop --stand-wired`: its input cell
/// (8, -2, 2), on from the slot's +10 for 40 gt opens the three gates and resets the latch),
/// driven from the release controller's REL (the core's dust (0, -1, 11), REL at golem +4..+16): a
/// tap north (0, -1, 10) into block B0 (0, -1, 9), a delay line east along z 9 (B1 = B0 + 10 gt,
/// B2 = + 20, B3 = + 28), each block tapped south into an OR line along z 7 (four 12-gt copies:
/// golem +10..+50), down to (8, -2, 4) and a repeater north into the input cell (glass -> stone).
pub(crate) fn stand_rel(d: &mut Design) -> Result<Vec<P3>> {
    use crate::schedgen::{DUST, rep};
    let cell: P3 = [8, -2, 2];
    if d.blocks.get(&cell).map(|s| s.as_str()) != Some(GLASS) {
        bail!("the pocket stand's REL cell {cell:?} is {:?}, not glass: the loop file changed", d.blocks.get(&cell));
    }
    let before: std::collections::BTreeSet<P3> = d.blocks.keys().copied().collect();
    let mut b = B { blocks: d.blocks.clone(), ports: Default::default(), nbt: Default::default() };
    // (the tap 6 gt: REL at the cell golem +14..+54 in release-trace, about the bench's +13 in a
    // run: the wired stand passes REL starts +10..+15 with the stagger, the bench's sweep)
    let tap_delay: u8 = std::env::var("MCTAS_STAND_REL_TAP").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    // (column 3 (c): the core under the lanes; REL from its refill dust (core (-2, 1, -1)) through
    // a 1-rt tap beside it and an auto-routed line to the old tap's input (0, -1, 11); the old
    // tap's delay less those repeaters, so the network sees REL at the same time)
    let tap_delay = if FAR {
        let src = crate::releasegen::core_place([-2, 1, -1]);
        let tap = crate::releasegen::core_place([-3, 1, -1]);
        let d = [tap[0] - src[0], tap[1] - src[1], tap[2] - src[2]];
        b.on(tap, &rep(facing_in(d), 1))?;
        let start = [tap[0] + d[0], tap[1] + d[1], tap[2] + d[2]];
        // (kept off the module's REL lead, off the core's REL stone's free side: core (0, 1, -4..-6)
        // and every cell next to them; and out of the divert unit's box (placed later: its first
        // frame x 16..34 shifted by UNIT_SHIFT, with its corner route, x -10..11, y -9..-3,
        // z -3..18))
        let mut avoid: std::collections::BTreeSet<P3> = Default::default();
        for x in -10..=11 {
            for y in -9..=-3 {
                for z in -3..=18 {
                    avoid.insert([x, y, z]);
                }
            }
        }
        for z in [-4, -5, -6, -7] {
            let c = crate::releasegen::core_place([0, 1, z]);
            for dc in [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                avoid.insert([c[0] + dc[0], c[1] + dc[1], c[2] + dc[2]]);
            }
        }
        let lim = crate::autoroute::Limits { lo: [-6, -9, -6], hi: [11, -1, 60], avoid, near_ok: [tap, src].into_iter().collect() };
        let path = crate::autoroute::find(&b, start, [0, -1, 11], &lim)?;
        route_d(&mut b, tap, &path, 14, 0, 1)?;
        let n = path.iter().filter(|p| b.blocks.get(*p).is_some_and(|s| s.contains("repeater"))).count() as u8;
        let total = tap_delay + crate::releasegen::FAR_SHIFT_RT;
        if 1 + n >= total || total - 1 - n > 4 {
            bail!("the stand's REL line has {n} repeaters: the old tap cannot make up {total} rt");
        }
        total - 1 - n
    } else {
        tap_delay
    };
    b.on([0, -1, 10], &rep("south", tap_delay))?;
    b.put([0, -1, 9], STONE)?;
    b.on([1, -1, 9], &rep("west", 4))?;
    b.on([2, -1, 9], &rep("west", 1))?;
    b.put([3, -1, 9], STONE)?;
    b.on([4, -1, 9], &rep("west", 4))?;
    b.on([5, -1, 9], &rep("west", 1))?;
    b.put([6, -1, 9], STONE)?;
    b.on([7, -1, 9], &rep("west", 4))?;
    b.put([8, -1, 9], STONE)?;
    for x in [0, 3, 6, 8] {
        b.on([x, -1, 8], &rep("south", 1))?;
    }
    for x in 0..=8 {
        b.on([x, -1, 7], DUST)?;
    }
    b.on([8, -1, 6], DUST)?;
    b.on([8, -1, 5], DUST)?;
    b.on([8, -2, 4], DUST)?;
    b.on([8, -2, 3], &rep("south", 1))?;
    b.blocks.insert(cell, STONE.into());
    // (a conductor over the gate's dust (8, -2, 1): the stone cell under the stand's own dust
    // (8, -1, 2) would otherwise hand that dust's signal down to the gate's dust (a wire takes its
    // upper diagonal neighbour's signal when the block between conducts and the one over it does
    // not), and a build in order (the game's shapes) then ties the gate to the stand's latch line:
    // the stand left 9 gt late and one visit's window took two carts. The file's settled shapes
    // hid it.)
    b.put([8, -1, 1], STONE)?;
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    Ok(added)
}

/// The module REL route's repeater spacing (B3; `MCTAS_REL_EVERY` to measure others).
fn rel_every() -> usize {
    std::env::var("MCTAS_REL_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(13)
}

/// The station's portal pad (farmopt's carousel-station-pad.txt, a part in the loop's own frame)
/// pasted at the loop's origin, its golem pulse wired into the release controller in place of the
/// bench's block at the golem input (-7, -1, 5): a repeater reads the pad's last dust (9, 6, -2)
/// from the south, a line west along z 0 at y 6 (over the stand: its riders' heads are below y 3;
/// the pad's cells are at z <= -1), down a stair west to (-7, 3, 0) and south onto (-7, 0, 3); that
/// dust powers its block (-7, -1, 3), which a repeater (-7, -1, 4) reads into a stone at the golem
/// input: a powered block there feeds X (-7, -1, 6) and ARMED's set line (-8, -1, 5) as the
/// bench's block does, one way (the set line is held on by the ARMED loop once armed, so no dust
/// may join it to X or to the pad's line: the stone (-8, 0, 3) and the stair's support (-7, 0, 2)
/// cut the stair's last dust off the set line's (-8, -1, 3) and (-7, -1, 2)). Refuses any cell
/// both designs hold, as `mobfarm paste` does. Returns the cells added.
pub(crate) fn wire_pad(d: &mut Design, pad: &Path) -> Result<Vec<P3>> {
    use crate::schedgen::rep;
    let p = Design::load(pad)?;
    let before: std::collections::BTreeSet<P3> = d.blocks.keys().copied().collect();
    let mut clashes = Vec::new();
    for (c, s) in &p.blocks {
        match d.blocks.get(c) {
            Some(t) if t != s => clashes.push(format!("{c:?}: the loop's {t}, the pad's {s}")),
            _ => {
                d.blocks.insert(*c, s.clone());
            }
        }
    }
    if !clashes.is_empty() {
        bail!("{} cells both the loop and the pad hold: {}", clashes.len(), clashes.iter().take(8).cloned().collect::<Vec<_>>().join("; "));
    }
    for (c, s) in &p.block_entities {
        d.block_entities.insert(*c, s.clone());
    }
    d.entities.extend(p.entities.iter().cloned());
    for (k, c) in &p.ports {
        if d.ports.contains_key(k) {
            bail!("the port {k} is in both the loop and the pad");
        }
        d.ports.insert(k.clone(), *c);
    }
    if !p.nether_blocks.is_empty() {
        if !d.nether_blocks.is_empty() {
            bail!("both the loop and the pad have a Nether part");
        }
        d.nether_blocks = p.nether_blocks.clone();
        d.nether_block_entities = p.nether_block_entities.clone();
        d.nether_ports = p.nether_ports.clone();
    }
    for k in ["station", "nether-origin"] {
        if let Some(v) = p.meta.get(k) {
            d.meta.insert(k.into(), v.clone());
        }
    }
    // (the build steps: both designs' tokens, each once)
    if let Some(bl) = p.meta.get("build") {
        let mut toks: Vec<String> = d.meta.get("build").map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default();
        for t in bl.split_whitespace() {
            if !toks.iter().any(|x| x == t) {
                toks.push(t.to_string());
            }
        }
        d.meta.insert("build".into(), toks.join(" "));
    }
    // ---- the pulse into X
    let pulse = *p.ports.get("output.pulse").ok_or_else(|| anyhow::anyhow!("the pad has no `port output.pulse`"))?;
    if pulse != [9, 6, -2] {
        bail!("the pad's pulse is at {pulse:?}, not (9, 6, -2): the route below assumes it");
    }
    let golem = [-7, -1, 5];
    if d.blocks.get(&golem).is_some_and(|s| !s.ends_with(":air")) {
        bail!("the golem input {golem:?} holds {:?}: it must be the bench's empty cell", d.blocks.get(&golem));
    }
    let mut b = B { blocks: d.blocks.clone(), ports: Default::default(), nbt: Default::default() };
    b.on([9, 6, -1], &rep("north", 1))?;
    let mut pr = vec![[9, 6, 0]];
    seg_to(&mut pr, [-4, 6, 0]);
    for k in 1..=3 {
        pr.push([-4 - k, 6 - k, 0]);
    }
    for k in 1..=3 {
        pr.push([-7, 3 - k, k]);
    }
    b.put([-8, 0, 3], STONE)?;
    route_d(&mut b, [9, 6, -1], &pr, 12, 0, 1)?;
    b.on([-7, -1, 4], &rep("north", 1))?;
    b.put(golem, STONE)?;
    let added: Vec<P3> = b.blocks.keys().copied().filter(|q| !before.contains(q)).collect();
    d.blocks = b.blocks;
    Ok(added)
}

/// The R'' step line's repeater delay (the line has ~25 repeaters).
fn step_delay() -> u8 {
    3
}

/// The module's place in the loop (its frame's 0 0 0), under the ground west of the siding.
/// The control module's placement (the fold, the parent and ironspec 22:33: vjeux's compact rule):
/// module cell q at `rotate_pos(q, MOD_TURNS) + MODULE_AT` ((x, z) -> (z, -x)): its long axis along
/// z under the strip between the siding and the queue (x -20..-7, z 14..55), below every other
/// part (y -17..-9), its REL input on the west side.
pub(crate) const MOD_TURNS: u8 = 3;
/// (column 3 (c): under the loop at x -6..7, below the divert unit (y -7..-4) and the release core
/// (y -2..-1); the side-by-side fold: west of the loop at x -20..-7)
pub(crate) const MODULE_AT: P3 = if FAR { [0, -14, 42] } else { [-15, -13, 42] };

/// A module cell's place in the loop frame.
pub(crate) fn module_at(q: P3) -> P3 {
    let r = clock::rotate_pos(q, MOD_TURNS);
    [r[0] + MODULE_AT[0], r[1] + MODULE_AT[1], r[2] + MODULE_AT[2]]
}

/// A module block's state turned with it.
fn mrot(s: &str) -> String {
    clock::canon::rotate_state(s, MOD_TURNS)
}

/// The night window's inverted daylight detector: west of the module at its depth, nothing of the
/// farm over it (open sky), its 11-dust strength filter south along x -26 and the night latch at
/// z 36.
pub(crate) const DETECTOR: P3 = if FAR { [9, -9, 55] } else { [-27, -9, 24] };
/// The siding's force-re-board input F (farmopt's child 58f770cc: (-5, -3, -4) in the siding's
/// frame), a dust the siding's repeater reads from the south.
pub(crate) const FORCE: P3 = if SIDING_TURNS == 2 { [SIDING_AT[0] + 5, SIDING_AT[1] - 3, SIDING_AT[2] + 4] } else { [SIDING_AT[0] - 5, SIDING_AT[1] - 3, SIDING_AT[2] - 4] };

/// The divert unit, next to the departure corner (REL from the core reaches it in ~6 gt; the far
/// module's own visit pulse comes ~30 gt after REL, too late for the corner): G2' = REL and not
/// (NV or REL 10 gt late) at (20, -6, 10), a 10-gt pulse on a visit release; T0 its block, the
/// chain north along x 20 (T1 = T0 + 10 gt, T2 = T0 + 20 gt, 4 cells apart... 6 apart here), P_k
/// east of each tap (comparators, rear the tap, side NOT l_k from R''); their outputs up two
/// stairs east to an OR column along x 24 at y -4, west along z 0 into the corner's repeater.
/// R'' is a copy of the module's k counter (2 items in 3 droppers), stepped ~120 gt after each
/// visit pulse (in sync with the module's R'); the router (T1 = NOT l1, T2 = l0) hangs off it.
fn divert_unit(b: &mut B) -> Result<(P3, P3)> {
    use crate::schedgen::{DUST, rep};
    // (the fold: the unit is laid out in its first frame (x 16..34, east of the loop) in a builder of
    // its own, then shifted in by UNIT_SHIFT, under the loop; its outer routes (REL in, the corner
    // output) are laid in the loop's frame after it)
    let mut t = B { blocks: Default::default(), ports: Default::default(), nbt: Default::default() };
    {
    let b = &mut t;
    b.on([20, -6, 11], &rep("south", 1))?;
    // REL 10 gt late into G2''s west side with NV: a junction dust (18, -6, 10) fed by two
    // repeaters, REL's from the south (18, -6, 11) off a branch (18, -6, 12), NV's from the west
    // (17, -6, 10) (NV comes along z 10 into (16, -6, 10))
    b.on([18, -6, 12], DUST)?;
    b.on([18, -6, 11], &rep("south", 4))?;
    b.on([18, -6, 10], DUST)?;
    b.on([19, -6, 10], &rep("west", 2))?;
    b.on([20, -6, 10], &comp("south", "subtract"))?;
    b.on([17, -6, 10], &rep("west", 1))?;
    // the chain north: T0 (20, -6, 9), five repeaters, T1 (20, -6, 3), five, T2 (20, -6, -3). The
    // first after T0 at 2 rt: cart k's window starts 0 / +12 / +22 after cart 0's, not 0 / +10 /
    // +20 (51b2aacd 23:58: the stand's guard sends its carts 13 and 10 gt apart; ironspec's
    // contract: >= 3 gt each side of every cart's occupancy of the corner cell)
    let tz = [9, 3, -3];
    for (k, &z) in tz.iter().enumerate() {
        b.put([20, -6, z], STONE)?;
        if k < 2 {
            for dz in 1..=5 {
                b.on([20, -6, z - dz], &rep("south", if k == 0 && dz == 1 { 2 } else { 1 }))?;
            }
        }
        // P_k east of the tap, its side from the north: a repeater fed by the row of NOT l_k
        b.on([21, -6, z], &comp("west", "subtract"))?;
        b.on([21, -6, z - 1], &rep("north", 1))?;
        // (its feed dust (21, -6, z - 2) is the end of R'''s row)
        // its output east, up two stairs to the OR column (24, -4, z)
        b.on([22, -6, z], DUST)?;
        b.on([23, -5, z], DUST)?;
        b.on([24, -4, z], DUST)?;
    }
    for z in -2..=8 {
        b.on([24, -4, z], DUST)?;
    }
    // (the corner's route and its repeater: the fold's, under the stand's guard, below; +4 gt net
    // as main's 3 rt repeater)
    // R'': droppers a (26, -6, 4), b (28, -6, 4), c (27, -6, 5), hoppers h (27, -6, 4), H (28, -6, 5),
    // j (26, -6, 5); a -> b -> c -> a; state 0 = c empty at power-on. Rows of NOT l_k west to the
    // P sides (21, -6, z_k - 2): l0 (c, tap south) -> row 7; l1 (a, tap north) -> row 1; l2 (b, tap
    // east) -> row -5.
    let item = "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}";
    let ring: [(P3, &str); 6] = [
        ([26, -6, 4], "minecraft:dropper[facing=east,triggered=false]"),
        ([27, -6, 4], "minecraft:hopper[enabled=true,facing=east]"),
        ([28, -6, 4], "minecraft:dropper[facing=south,triggered=false]"),
        ([28, -6, 5], "minecraft:hopper[enabled=true,facing=west]"),
        ([27, -6, 5], "minecraft:dropper[facing=west,triggered=false]"),
        ([26, -6, 5], "minecraft:hopper[enabled=true,facing=north]"),
    ];
    for (p, s) in ring {
        b.put(p, s)?;
        b.put([p[0], -5, p[2]], DUST)?;
    }
    b.nbt.insert([26, -6, 4], item.into());
    b.nbt.insert([28, -6, 4], item.into());
    // c's tap (state 0) south -> row 7
    b.on([27, -6, 6], &comp("north", "compare"))?;
    b.on([27, -6, 7], &rep("north", 1))?;
    // (along z 8 only to x 25, then z 7: clear of P0's output stair at z 9)
    let mut r0 = vec![[27, -6, 8], [26, -6, 8], [25, -6, 8], [25, -6, 7]];
    seg_to(&mut r0, [21, -6, 7]);
    route(b, [27, -6, 7], &r0, 0)?;
    // a's tap (state 1) north -> row 1
    b.on([26, -6, 3], &comp("south", "compare"))?;
    b.on([26, -6, 2], &rep("south", 1))?;
    let mut r1 = vec![[26, -6, 1]];
    seg_to(&mut r1, [21, -6, 1]);
    route(b, [26, -6, 2], &r1, 0)?;
    // b's tap (state 2) east -> north along x 31 -> row -5
    b.on([29, -6, 4], &comp("west", "compare"))?;
    b.on([30, -6, 4], &rep("west", 1))?;
    let mut r2 = vec![[31, -6, 4]];
    seg_to(&mut r2, [31, -6, -5]);
    seg_to(&mut r2, [21, -6, -5]);
    route(b, [30, -6, 4], &r2, 8)?;
    // R'''s step: off T0's top, a delay line east and back into j's top, ~140 gt after the visit
    // pulse: after this visit's own windows (+0..+30), and after the cart of the visit 4 releases
    // back has gone through T1/T2 (the router reads R'' as "the next visit's k" until then)
    b.on([20, -5, 9], DUST)?;
    let mut st = vec![[20, -4, 10]];
    seg_to(&mut st, [20, -4, 14]);
    seg_to(&mut st, [34, -4, 14]);
    seg_to(&mut st, [34, -4, 6]);
    seg_to(&mut st, [30, -4, 6]);
    seg_to(&mut st, [29, -5, 6]);
    route_d(b, [20, -5, 9], &st, 1, 1, step_delay())?;
    b.on([28, -5, 6], DUST)?;
    }
    // ---- shift the unit in, every cell checked: no clash, and no dust of it touching another
    // part's dust (the routes' rule)
    let u = unit_at;
    let mine: std::collections::BTreeSet<P3> = t.blocks.keys().map(|p| u(*p)).collect();
    for (p, s) in &t.blocks {
        b.put(u(*p), s)?;
    }
    for (p, n) in &t.nbt {
        b.nbt.insert(u(*p), n.clone());
    }
    for (p, s) in &t.blocks {
        if !s.contains("redstone_wire") {
            continue;
        }
        let c = u(*p);
        for n in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1], [1, -1, 0], [-1, -1, 0], [0, -1, 1], [0, -1, -1]] {
            let q = [c[0] + n[0], c[1] + n[1], c[2] + n[2]];
            let cut = match n[1] {
                -1 => Some([c[0] + n[0], c[1], c[2] + n[2]]),
                1 => Some([c[0], c[1] + 1, c[2]]),
                _ => None,
            };
            if cut.is_some_and(|k| b.blocks.get(&k).is_some_and(|s| s == STONE)) {
                continue;
            }
            if !mine.contains(&q) && b.blocks.get(&q).is_some_and(|s| s.contains("redstone_wire")) {
                bail!("the divert unit's dust {c:?} would touch foreign dust at {q:?}");
            }
        }
    }
    // ---- REL in: a repeater off the core's REL dust (0, -1, 13) south into (0, -1, 15) (the old
    // tap's time: one repeater, then one on the route), down a stair south along x 0 to y -6 at
    // z 20, west along z 20 to x -10, north to the feed's row + 1, east along it past the REL-late
    // branch's source (unit (18, -6, 13)) to G2''s feed column, north into its feed (unit
    // (20, -6, 12))
    let feed = u([20, -6, 12]);
    // (the REL-late branch's dust (unit (18, -6, 12)) touches the route on purpose: it reads REL
    // from it; out of the way while the route checks its neighbours)
    let lifted: Vec<(P3, String)> = [u([18, -6, 12])].iter().filter_map(|p| b.blocks.remove(p).map(|s| (*p, s))).collect();
    if FAR {
        // (column 3 (c), milestone B: the core under the lanes; a 1-rt tap beside its REL dust
        // (core (-2, 1, -3)) at core (-2, 1, -4), an auto-routed line to the old approach's last
        // cells (along the feed's row + 1 past the REL-late dust, north into the feed), the
        // repeaters adding up to the old tap and route's 2 rt + FAR_SHIFT_RT)
        let src = crate::releasegen::core_place([-2, 1, -3]);
        let tap = crate::releasegen::core_place([-3, 1, -3]);
        let dv = [tap[0] - src[0], 0, tap[2] - src[2]];
        let start = [tap[0] + dv[0], tap[1], tap[2] + dv[2]];
        let late = u([18, -6, 12]);
        let tail = vec![[late[0], -6, feed[2] + 1], [late[0] + 1, -6, feed[2] + 1], [feed[0], -6, feed[2] + 1], feed];
        // (the tap is the first of the line's repeaters, placed before the search so the line
        // keeps off it)
        b.on(tap, &rep(facing_in(dv), 1))?;
        // (the search ends at the approach's first cell and keeps off the rest of it and every cell
        // next to it)
        let mut avoid: std::collections::BTreeSet<P3> = Default::default();
        for t in &tail[1..] {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        avoid.insert([t[0] + dx, t[1] + dy, t[2] + dz]);
                    }
                }
            }
        }
        avoid.remove(&tail[0]);
        for t in &tail {
            avoid.remove(&[t[0], t[1] - 1, t[2]]);
        }
        let lim = crate::autoroute::Limits { lo: [-10, -9, 13], hi: [11, -3, 55], avoid, near_ok: [tap].into_iter().collect() };
        let mut rel = crate::autoroute::find(b, start, tail[0], &lim)?;
        rel.extend(tail[1..].iter().copied());
        let total = 2 + crate::releasegen::FAR_SHIFT_RT;
        lay_exact(b, tap, &rel, 0, total - 1)?;
    } else {
    b.on([0, -1, 14], &rep("north", 1))?;
    let mut rel = vec![[0, -1, 15]];
    seg_to(&mut rel, [0, -6, 20]);
    seg_to(&mut rel, [-10, -6, 20]);
    seg_to(&mut rel, [-10, -6, feed[2] + 1]);
    seg_to(&mut rel, [feed[0], -6, feed[2] + 1]);
    rel.push(feed);
    route_d(b, [0, -1, 14], &rel, 14, 0, 1)?;
    }
    for (p, s) in lifted {
        b.blocks.insert(p, s);
    }
    // ---- the corner: a repeater east off the OR column (unit (24, -4, 0)), down four stairs east
    // to y -8 (under the unit and the stand's guard circuit, x 0..8 y -6..1 z 0..5), south to the
    // unit's row 2, east along it to x 11, up four stairs north into (11, -4, 0), the repeater
    // (10, -4, 0) west into the corner's support
    let r0 = u([25, -4, 0]);
    b.on(r0, &rep("west", 1))?;
    let mut out = vec![[r0[0] + 1, -4, r0[2]]];
    for k in 1..=4 {
        out.push([r0[0] + 1 + k, -4 - k, r0[2]]);
    }
    let d = *out.last().unwrap();
    seg_to(&mut out, [d[0], -8, 4]);
    seg_to(&mut out, [11, -8, 4]);
    for k in 1..=4 {
        out.push([11, -8 + k, 4 - k]);
    }
    route_d(b, r0, &out, 8, 0, 1)?;
    // (2 rt: the windows +4 gt in all (this route's own repeater +2, this +2), for the stand's
    // guard, which brings carts 1 and 2 to the corner 3 gt later (51b2aacd 23:46): cart k's window
    // +21 + 10k .. +30 + 10k after the golem, 3 gt open before cart k and 4..7 after with either
    // stand)
    b.on([10, -4, 0], &rep("east", 2))?;
    // (the router: T1 = NOT l1 = row 1's signal, T2 = l0 = NOT row 7's: returned as their cells)
    Ok((u([24, -6, 1]), u([25, -6, 8])))
}

/// The divert unit's shift from its first frame (x 16..34 east of the loop) to under the loop
/// (x -9..9, z -3..16, y -7..-4): the fold.
pub(crate) const UNIT_SHIFT: P3 = [-25, 0, 2];

/// A divert-unit cell (first frame) in the loop frame.
pub(crate) fn unit_at(p: P3) -> P3 {
    [p[0] + UNIT_SHIFT[0], p[1] + UNIT_SHIFT[1], p[2] + UNIT_SHIFT[2]]
}

/// Stamps the control module into the loop at `MODULE_AT` and wires it: REL from the release
/// controller's REL stone (-2, -1, 13), a normal daylight detector for the night window, and the
/// four controls (the corner T's support, T1's, T2's through repeaters, the siding's exit slot
/// through a repeater into its rail). Returns the cells added.
pub(crate) fn wire_control(d: &mut Design, sched: &Path) -> Result<Vec<P3>> {
    use crate::schedgen::{DUST, rep};
    let m = control_blocks(sched)?;
    let at = module_at;
    let before: std::collections::BTreeSet<P3> = d.blocks.keys().copied().collect();
    let mut b = B { blocks: d.blocks.clone(), ports: Default::default(), nbt: Default::default() };
    for (q, s) in &m.blocks {
        b.put(at(*q), &mrot(s))?;
    }
    for (q, n) in &m.nbt {
        d.block_entities.insert(at(*q), n.clone());
    }
    // night: an INVERTED daylight detector under the open sky north of the module (DETECTOR, y 1:
    // nothing of the farm over it; its output is 15 - (sky light - darkening): 11 exactly in the
    // deep night 13,670..22,330 in clear weather, rain and thunder, less outside it: the scheduler's
    // weather-proof window, ironspec's ruling), eleven dusts south (so the repeater after them is
    // on only at 11), a repeater into a block, a torch on the block's south face: the line is ON
    // by day and also while the detector is missing (a runner places detectors after its settle:
    // the module must not start a visit then), down a stair south to y -7, south along x -56, up
    // into the day line's dust (module (2, -2, 1)) and its repeater into the day block. (The 3+
    // threshold of 1152b0e38 opened at about 12,500 and closed at 23,640: 27 visits a night, 6 of
    // them second naps that did not come back; the window of 13,670..22,330 holds 21 or 22.)
    b.put(DETECTOR, "minecraft:daylight_detector[inverted=true,power=0]")?;
    // (glass over it: a golem's legacy spawn takes a daylight detector for a floor; glass keeps
    // the sky light)
    b.put([DETECTOR[0], DETECTOR[1] + 1, DETECTOR[2]], "minecraft:glass")?;
    // (column 3 (c): the detector at (8, -9, 55) under an open column (nothing of the farm over
    // x 8 at z 55, inside the loop's z <= 55), clear of the module, which fills x -5..4 down to
    // z 55; its eleven dusts east, down a stair north along x 9 to y -17, on two cells; thr
    // (9, -17, 44) facing north, the latch on its west side (x 8..5); thr's output auto-routed
    // (a day/night line: its delay is moot) into the inverter block (-5, -17, 39) from the north,
    // a wall torch on its east face (lit = day), the day line east along z 39 through the
    // module's corridor (its (3, -3, -5..-1)) into its day input)
    let far_thr = [DETECTOR[0] + 1, -17, DETECTOR[2] - 11];
    if FAR {
        let x = DETECTOR[0] + 1;
        let mut c = vec![[x, DETECTOR[1], DETECTOR[2]]];
        for k in 1..=8 {
            c.push([x, DETECTOR[1] - k, DETECTOR[2] - k]);
        }
        c.push([x, -17, DETECTOR[2] - 9]);
        c.push([x, -17, DETECTOR[2] - 10]);
        if c.len() != 11 || c[10] != [far_thr[0], far_thr[1], far_thr[2] + 1] {
            bail!("the detector's filter has {} dusts ending at {:?}, not 11 into thr {far_thr:?}", c.len(), c.last());
        }
        for p in &c {
            b.on(*p, DUST)?;
        }
        b.on(far_thr, &rep("south", 1))?;
        // (the inverter block at (-6, -17, 39), its torch at (-5, -17, 39): a torch powers the block
        // over it, and (-4, -16, 39) is the module's REL bus support, so the torch stays off x -4)
        let blk = [-6, -17, 39];
        b.put(blk, STONE)?;
        b.put([blk[0] + 1, -17, 39], "minecraft:redstone_wall_torch[facing=east,lit=true]")?;
        let dl: Vec<P3> = (blk[0] + 2..=-2).map(|xx| [xx, -17, 39]).chain([at([3, -3, -1]), at([3, -2, 0]), at([3, -2, 1])]).collect();
        route_d(&mut b, [blk[0] + 1, -17, 39], &dl, 6, 0, 1)?;
        // (thr's output north into the inverter: auto-routed to the block's south neighbour's
        // south neighbour, then north into it; the latch's cells kept clear)
        let out0 = [far_thr[0], -17, far_thr[2] - 1];
        let last = [blk[0], -17, blk[2] + 1];
        let pre = [blk[0], -17, blk[2] + 2];
        let mut avoid: std::collections::BTreeSet<P3> = Default::default();
        for dx in 1..=5 {
            avoid.insert([far_thr[0] - dx, -17, far_thr[2]]);
        }
        avoid.insert(last);
        let lim = crate::autoroute::Limits { lo: [-6, -19, 30], hi: [11, -10, 55], avoid, near_ok: [far_thr, blk].into_iter().collect() };
        let mut tl = crate::autoroute::find(&b, out0, pre, &lim)?;
        tl.push(last);
        route_d(&mut b, far_thr, &tl, 14, 0, 1)?;
    } else {
    for dz in 1..=11 {
        b.on([DETECTOR[0], DETECTOR[1], DETECTOR[2] + dz], DUST)?;
    }
    b.on([DETECTOR[0], DETECTOR[1], DETECTOR[2] + 12], &rep("north", 1))?;
    b.put([DETECTOR[0], DETECTOR[1], DETECTOR[2] + 13], STONE)?;
    b.put([DETECTOR[0], DETECTOR[1], DETECTOR[2] + 14], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    let mut dl = vec![[DETECTOR[0], DETECTOR[1], DETECTOR[2] + 15]];
    // (down a stair east along z 39 into the module's day line, its last two cells the module's)
    seg_to(&mut dl, at([3, -3, -1]));
    dl.push(at([3, -2, 0]));
    dl.push(at([3, -2, 1]));
    route_d(&mut b, [DETECTOR[0], DETECTOR[1], DETECTOR[2] + 14], &dl, 6, 0, 1)?;
    }
    // (the night latch on thr comes after the module's REL route, below)
    b.on(at([2, -2, 1]), DUST)?;
    b.on(at([1, -2, 1]), &mrot(&rep("east", 1)))?;
    // (night 1: the day line also locks the D ring's hopper h13 (module (1, 0, 0), the one that
    // feeds d0), where the ring's token is built: by day the token waits in h13, and as the line
    // goes off with the night window it drops into d0, so the night's first release visits. A
    // branch up off the line's dust (3, -3, -2): dusts (2, -2, -2) and (1, -1, -2), a repeater
    // (1, -1, -1) south into a block (1, -1, 0) under h13)
    if !b.blocks.get(&at([3, -3, -2])).is_some_and(|s| s.contains("redstone_wire")) {
        bail!("the day line has no dust at module (3, -3, -2) to lock h13 from: {:?}", b.blocks.get(&at([3, -3, -2])));
    }
    b.on(at([2, -2, -2]), DUST)?;
    b.on(at([1, -1, -2]), DUST)?;
    b.on(at([1, -1, -1]), &mrot(&rep("north", 1)))?;
    b.put(at([1, -1, 0]), STONE)?;
    // ---- the holds' kick route first (REL from the core's REL stone (-2, -1, 13) through
    // (-2, -1, 14), down a stair south to y -4, west along z 18 to x -21, the delay south along
    // x -21 into the gate KA (-21, -4, 30)): the module's REL and F's rear branch off it (the fold)
    // (column 3 (c), milestone B: the core under the lanes; the module's REL straight off the
    // core's REL stone (core (0, 1, -3)) through its one free side, an auto-routed line to the
    // feed with main's delay + FAR_SHIFT_RT: main brings REL to the module's input block 14 gt
    // after the stone (release-trace: rel +4, rel_in +18), 7 rt with the feed's repeater; the
    // layout's own parts (the kick's delay and KA, F, the holds, the T drives) are left out under
    // `MCTAS_FAR_B_ONLY`, B's check)
    let b_only = FAR && std::env::var("MCTAS_FAR_B_ONLY").is_ok();
    let mut rel: Vec<P3> = Vec::new();
    let mut unlock_src: Option<P3> = None;
    let mut tap_x = 0;
    if FAR {
        // (a straight lead off the stone's free side along core x 0 (core (0, 1, -4..-7), all
        // free): a 1-rt repeater, a dust (the night latch's unlock taps it, REL + 2 gt), a 4-rt and
        // a 3-rt repeater (8 rt in all), then an auto-routed stair down to the feed that holds none)
        let stone = crate::releasegen::core_place([0, 1, -3]);
        let lead: Vec<P3> = (4..=8).map(|z| crate::releasegen::core_place([0, 1, -z])).collect();
        let dv = [lead[0][0] - stone[0], 0, lead[0][2] - stone[2]];
        let total = 7 - 1 + crate::releasegen::FAR_SHIFT_RT;
        b.on(lead[0], &rep(facing_in(dv), 1))?;
        b.on(lead[1], DUST)?;
        b.on(lead[2], &rep(facing_in(dv), 4))?;
        b.on(lead[3], &rep(facing_in(dv), total - 5))?;
        let lim = crate::autoroute::Limits { lo: [-6, -17, 14], hi: [11, -3, 55], avoid: Default::default(), near_ok: [lead[3]].into_iter().collect() };
        rel = crate::autoroute::find(&b, lead[4], [-6, -16, 29], &lim)?;
        lay_exact(&mut b, lead[3], &rel, 0, 0)?;
        unlock_src = Some(lead[1]);
    }
    if !FAR {
    let mut kr = vec![[-2, -1, 14], [-2, -1, 15]];
    for k in 1..=3 {
        kr.push([-2, -1 - k, 15 + k]);
    }
    let kx = -21;
    seg_to(&mut kr, [kx, -4, 18]);
    route(&mut b, [-2, -1, 13], &kr, 6)?;
    let kick_delay = 6;
    for k in 0..kick_delay {
        b.on([kx, -4, 19 + k], &rep("north", 4))?;
    }
    for z in 19 + kick_delay..=29 {
        b.on([kx, -4, z], DUST)?;
    }
    b.on([kx, -4, 30], &comp("north", "subtract"))?;
    }
    // (a dust of the kick route along z 18 near x0, to branch from: a repeater there would not
    // feed a repeater beside it)
    let kick_dust_near = |b: &B, x0: i32| -> Result<i32> {
        for dx in [0, 1, -1, 2, -2] {
            let x = x0 + dx;
            if (-20..=-3).contains(&x) && b.blocks.get(&[x, -4, 18]).is_some_and(|s| s.contains("redstone_wire")) {
                return Ok(x);
            }
        }
        bail!("the kick route has no dust near x {x0} on z 18")
    };
    // REL into the module: a repeater on the kick route's dust near x -16 into (x, -4, 20), down a
    // stair west along z 20 to y -15, west to x -27, south along x -27 to z 28, east into the
    // module's feed dust and its repeater into the input block (module (14, -2, -4))
    if !FAR {
    tap_x = kick_dust_near(&b, -16)?;
    // (2 rt: the module's REL at the time the old route brought it, so its outputs keep their times)
    b.on([tap_x, -4, 19], &rep("north", 2))?;
    rel = vec![[tap_x, -4, 20]];
    seg_to(&mut rel, [tap_x - 11, -15, 20]);
    seg_to(&mut rel, [-27, -15, 20]);
    seg_to(&mut rel, [-27, -15, 28]);
    seg_to(&mut rel, at([14, -2, -7]));
    route_d(&mut b, [tap_x, -4, 19], &rel, rel_every(), 0, 1)?;
    }
    // (the feed dust touches the module's REL branch on purpose: the same signal)
    b.on(at([14, -2, -6]), DUST)?;
    b.on(at([14, -2, -5]), &mrot(&rep("north", 1)))?;
    // ---- the night latch: the threshold repeater (thr) locked except once a release, so NV (the
    // scheduler's "the next release is a visit") changes only between the divert unit's read of
    // it (golem +12..22, NV ~25 gt late there) and the module's (golem +32..44): a night edge
    // between the two reads diverted a cart the module never registered, so no kick came (old
    // 7d947146b file, seed 2, slot 459 at day time 22,369). The unlock: REL tapped off the
    // module's REL route (z -10, y -6) at x -44, west along z -12, up a stair north along x -50 to
    // y 1, west along z -20 into a repeater, a block, its wall torch (lit = locked) and the lock
    // repeater pointing into thr's east side.
    if FAR {
        // (column 3 (c): the latch on thr's east side at z 45, south of the module's last row
        // (world z 44): thr + 1 a repeater into thr, thr + 2 the wall torch (lit = locked), thr + 3
        // its block, thr + 4 the unlock's repeater into that block, fed from (0, -17, 45) by a line
        // auto-routed off the REL branch's last flat run)
        let thr = far_thr;
        b.on([thr[0] - 4, thr[1], thr[2]], &rep("west", 4))?;
        b.put([thr[0] - 3, thr[1], thr[2]], STONE)?;
        b.put([thr[0] - 2, thr[1], thr[2]], "minecraft:redstone_wall_torch[facing=east,lit=true]")?;
        b.on([thr[0] - 1, thr[1], thr[2]], &rep("west", 4))?;
        let sink = [thr[0] - 5, thr[1], thr[2]];
        // (the unlock off the REL lead's dust: a 1-rt tap on its east side, an auto-routed line
        // with 8 rt more (measured: 4 rt dropped the lock at golem + 26), so the lock drops at
        // + 34: main's + 30 and the controller's FAR_SHIFT)
        let src = unlock_src.ok_or_else(|| anyhow::anyhow!("no REL lead to tap for the unlock"))?;
        let d = [1, 0, 0];
        let tr = [src[0] + d[0], src[1], src[2] + d[2]];
        let start = [tr[0] + d[0], tr[1], tr[2] + d[2]];
        b.on(tr, &rep(facing_in(d), 1))?;
        let lim = crate::autoroute::Limits { lo: [-6, -18, 14], hi: [11, -3, 55], avoid: Default::default(), near_ok: [tr, [thr[0] - 4, thr[1], thr[2]]].into_iter().collect() };
        let lk = crate::autoroute::find(&b, start, sink, &lim)?;
        lay_exact(&mut b, tr, &lk, 0, 8)?;
        let laid = true;
        let why = String::new();
        if !laid {
            bail!("no tap on the REL branch reaches the latch's unlock input {sink:?} (last: {why})");
        }
    } else {
    let thr = [DETECTOR[0], DETECTOR[1], DETECTOR[2] + 12];
    // (the unlock: a repeater off the module's REL branch where its stair crosses x thr + 5, south
    // along that x into the lock)
    let lx = thr[0] + 5;
    let tap = [lx, -4 - (tap_x - lx), 20];
    if !b.blocks.get(&tap).is_some_and(|s| s.contains("redstone_wire")) {
        bail!("the module's REL branch at {tap:?} is {:?}, not a dust to branch from", b.blocks.get(&tap));
    }
    b.on([tap[0], tap[1], 21], &rep("north", 1))?;
    let mut lk = vec![[tap[0], tap[1], 22]];
    seg_to(&mut lk, [lx, thr[1], thr[2]]);
    route_d(&mut b, [tap[0], tap[1], 21], &lk, 3, 0, 1)?;
    // (both repeaters at 4 rt: the unlock at golem +~48..60, the night edge at the module's NV at
    // ~+64, mid-way between its read (+32..44) and the divert unit's next (+112, ~25 gt behind))
    b.on([thr[0] + 4, thr[1], thr[2]], &rep("east", 4))?;
    b.put([thr[0] + 3, thr[1], thr[2]], STONE)?;
    b.put([thr[0] + 2, thr[1], thr[2]], "minecraft:redstone_wall_torch[facing=west,lit=true]")?;
    b.on([thr[0] + 1, thr[1], thr[2]], &rep("east", 4))?;
    }
    // ---- NV, the module's steady "the next release is a visit", to the divert unit: from its port
    // (28, 4, 3) down a stair south to y -9, east along z 12, north to z 10, up a stair east into
    // (16, -6, 10)
    let nv_port = at([28, 4, 3]);
    // (from the port's next cell up two stairs north, a stair up east, east into the divert unit's
    // NV feed (unit (16, -6, 10)))
    let n0 = at([28, 4, 4]);
    let mut nv = if FAR {
        // (column 3 (c): the module under the unit: from the port's next cell north along y -9 (the
        // module's top layer, north of it) to z 9, west along z 9 to the feed's x, up a stair south
        // into the feed)
        let feed = unit_at([16, -6, 10]);
        let rise = feed[1] - n0[1];
        let mut v = vec![n0];
        seg_to(&mut v, [n0[0], n0[1], feed[2] - rise]);
        seg_to(&mut v, [feed[0], n0[1], feed[2] - rise]);
        seg_to(&mut v, feed);
        v
    } else {
        vec![n0, [n0[0], n0[1] + 1, n0[2] - 1], [n0[0], n0[1] + 2, n0[2] - 2], [n0[0] + 1, n0[1] + 3, n0[2] - 2]]
    };
    if !FAR {
        seg_to(&mut nv, unit_at([16, -6, 10]));
    }
    if FAR {
        // (column 3 (c): the route is shorter than main's (12 gt from the module's NV to the
        // unit's junction against main's 22): its repeaters (every 6) are given delays adding up
        // to main's 11 rt, so NV reaches the unit main's + 4 gt like every controller line)
        route(&mut b, nv_port, &nv, 6)?;
        let reps: Vec<P3> = nv.iter().copied().filter(|p| b.blocks.get(p).is_some_and(|s| s.contains("repeater"))).collect();
        let k = reps.len() as u8;
        // (measured: 12 gt module NV -> unit junction with these at 1 rt; main 22: 5 rt more)
        let want: u8 = k + 5;
        if k == 0 || want < k || want > 4 * k {
            bail!("the NV route has {k} repeaters: cannot spread {want} rt over them");
        }
        for (j, p) in reps.iter().enumerate() {
            let d = want / k + u8::from((j as u8) < want % k);
            let s = b.blocks[p].clone();
            let s = s.split("delay=").enumerate().map(|(i, part)| if i == 1 { format!("{d}{}", &part[1..]) } else { part.to_string() }).collect::<Vec<_>>().join("delay=");
            b.blocks.insert(*p, s);
        }
    } else {
    route(&mut b, nv_port, &nv, 6)?;
    }
    let (t1_row, t2_row) = divert_unit(&mut b)?;
    b.blocks.insert(DIVERT, STONE.into());
    // ---- T1 and T2 from the unit's ring: each T's support driven from below by two torches (a
    // block, a torch on it, a block, a torch under the support: the same sense as the line), the
    // line into the lowest block through a repeater from the west
    let tower = |b: &mut B, sup: P3| -> Result<()> {
        b.put([sup[0], sup[1] - 4, sup[2]], STONE)?;
        b.put([sup[0], sup[1] - 3, sup[2]], "minecraft:redstone_torch[lit=true]")?;
        b.put([sup[0], sup[1] - 2, sup[2]], STONE)?;
        b.put([sup[0], sup[1] - 1, sup[2]], "minecraft:redstone_torch[lit=false]")?;
        b.blocks.insert(sup, STONE.into());
        b.on([sup[0] - 1, sup[1] - 4, sup[2]], &rep("west", 1))
    };
    // T1 = NOT l1: off row 1 at (24, -6, 1), north, down a stair to y -11, west along z -5, south
    // along x -17, up a stair south to (-17, -4, 27), into (-17, -4, 28)
    // (WIP, column 3 (c): `MCTAS_FAR_NO_T` leaves T1 and T2 unwired, to lay their routes out)
    let skip_t = FAR && (std::env::var("MCTAS_FAR_NO_T").is_ok() || b_only);
    if !skip_t {
    let mut t1 = vec![[t1_row[0], -6, t1_row[2] - 1]];
    for k in 1..=5 {
        t1.push([t1_row[0], -6 - k, t1_row[2] - 1 - k]);
    }
    seg_to(&mut t1, [-5, -11, t1_row[2] - 6]);
    // (south along x -5, east of the module, up a stair south to y -4 at z 26, west along z 26 (a
    // row clear of T1's tower stone, which its own repeater powers), south into (-17, -4, 28))
    seg_to(&mut t1, [-5, -11, 19]);
    seg_to(&mut t1, [-5, -4, 26]);
    if FAR {
        // (column 3 (c), the first prototype: S1's support (-6, 2, 31) driven from the west at its
        // level by a repeater (-7, 2, 31); the route west along z 26 at y -4 to x -16 (west of the
        // release core, x -13..-2), up a stair south to y 2 at z 31, east along z 31: wiring at
        // x -16..-7 until the core folds in)
        seg_to(&mut t1, [-16, -4, 26]);
        for k in 1..=7 {
            t1.push([-16, -4 + k, 26 - k]);
        }
        // (at y 3, two over the holds' kick rows at y 1 (one over would join them as a dust stair),
        // back south to z 31 (clear of T2's row along z 34), east along it, down a stair to the
        // repeater's input (-8, 2, 31))
        seg_to(&mut t1, [-16, 3, 31]);
        seg_to(&mut t1, [T1[0] - 6, T1[1] + 1, T1[2]]);
        t1.push([T1[0] - 5, T1[1], T1[2]]);
        route(&mut b, t1_row, &t1, 6)?;
        far_t_drive(&mut b, T1)?;
    } else {
    seg_to(&mut t1, [-17, -4, 26]);
    seg_to(&mut t1, [-17, -4, 28]);
    route(&mut b, t1_row, &t1, 6)?;
    tower(&mut b, T1)?;
    }
    // T2 = l0 = NOT (row 7's NOT l0): off row 7 at (24, -6, 8), a repeater south into a block
    // (24, -6, 10), a torch on it, down a stair south to y -13, west along z 19, south along x -19,
    // up a stair south to (-19, -4, 30), into (-19, -4, 31)
    b.on([t2_row[0], -6, t2_row[2] + 1], &rep("north", 1))?;
    b.put([t2_row[0], -6, t2_row[2] + 2], STONE)?;
    b.put([t2_row[0], -6, t2_row[2] + 3], "minecraft:redstone_wall_torch[facing=south,lit=true]")?;
    let mut t2 = vec![[t2_row[0], -6, t2_row[2] + 4]];
    for k in 1..=7 {
        t2.push([t2_row[0], -6 - k, t2_row[2] + 4 + k]);
    }
    // (two cells west of T2, where the tower's input repeater reads it: west to x -3, up a stair
    // south along x -3 to y -4 at z 31, south to z 33 (clear of T2's tower stone), west along
    // z 33, north into it)
    let x2 = T2[0] - 2;
    let z2 = t2_row[2] + 11;
    seg_to(&mut t2, [-3, -13, z2]);
    seg_to(&mut t2, [-3, -13, z2 + 1]);
    if FAR {
        // (column 3 (c), the first prototype: S2's support (-6, 2, 34) driven from the west by a
        // repeater (-7, 2, 34); the route up a stair south along x -3 to y -4 (under the register,
        // below the core), west along z 33 at y -4 to x -17, up a stair south to y 2 at z 39, back
        // north along x -17 to z 34, east along z 34 at y 2)
        seg_to(&mut t2, [-3, -4, 31]);
        seg_to(&mut t2, [-3, -4, 33]);
        seg_to(&mut t2, [-17, -4, 33]);
        for k in 1..=7 {
            t2.push([-17, -4 + k, 33 + k]);
        }
        t2.push([-18, 3, 40]);
        seg_to(&mut t2, [-18, 3, 34]);
        seg_to(&mut t2, [T2[0] - 6, T2[1] + 1, T2[2]]);
        t2.push([T2[0] - 5, T2[1], T2[2]]);
        route(&mut b, [t2_row[0], -6, t2_row[2] + 3], &t2, 6)?;
        far_t_drive(&mut b, T2)?;
    } else {
    seg_to(&mut t2, [-3, -4, 31]);
    seg_to(&mut t2, [-3, -4, 33]);
    seg_to(&mut t2, [x2, -4, 33]);
    seg_to(&mut t2, [x2, -4, 31]);
    route(&mut b, [t2_row[0], -6, t2_row[2] + 3], &t2, 6)?;
    tower(&mut b, T2)?;
    }
    }
    if !b_only {
    // ---- the holds' kick: REL from the core's REL stone (-2, -1, 13) through (-2, -1, 14), down a
    // stair south to y -4, west along z 18 to x -21, a delay (`kick_delay` repeaters of 4 gt)
    // south along x -21, the gate KA (-21, -4, 30): REL late minus NS5 (the module's NOT "a
    // visit's item in D5": high except from the 4th release after a visit to the 5th), its output
    // south, east along z 37 up to y -1, into the bus along x -13 (y -1, z 30..36) that feeds the
    // three holds' drivers: a dust up a stair onto (-12, 0, z + 2), a repeater north into the
    // hold's support (-12, 0, z)
    // (the route and its delay chain are built above, before the module's REL branch)
    // NS5 into KA's west side through a repeater (-22, -4, 30), from the module's port: up a stair
    // north to y -11 at z 9, west along z 9 (over NS3's run along x -12), up a stair south along
    // x -24 to y -4 at z 16, south along x -24 to z 30, east into (-23, -4, 30)
    let ns5_port = at([27, -2, 5]);
    let s5 = at([28, -2, 5]);
    let mut ns = vec![s5, [s5[0], s5[1], s5[2] - 1]];
    seg_to(&mut ns, [s5[0], s5[1] + 4, s5[2] - 5]);
    seg_to(&mut ns, [-24, s5[1] + 4, s5[2] - 5]);
    seg_to(&mut ns, [-24, -4, s5[2] - 5 + 7]);
    seg_to(&mut ns, [-24, -4, 30]);
    seg_to(&mut ns, [-23, -4, 30]);
    route(&mut b, ns5_port, &ns, 14)?;
    b.on([-22, -4, 30], &rep("west", 1))?;
    // ---- the siding's force-re-board F at the 3rd release after a visit, +20 (58f770cc: the cart
    // then reaches the siding's slot ~95 gt later, its hold before the kick): F's gate FA, a
    // comparator facing south at (-25, -1, 5), its output into F's dust; rear REL from the module's
    // REL route (a repeater off it at x -25, south under the entry route along x -25 at y -6, up
    // five stairs south to (-25, -1, 4)); side NS3 from the module (east along z 3 at y -6, up
    // four stairs east to (-27, -2, 3), south to (-27, -2, 4), a stair up to (-27, -1, 5) and a
    // repeater east into FA)
    if std::env::var("MCTAS_F_FROM_X").is_ok() {
        // (B3b was dropped (ironspec 18:47) and is not laid out for the folded siding)
        if FORCE != [-25, -1, 6] {
            bail!("MCTAS_F_FROM_X (B3b, dropped) is laid out for the siding at [-20, 2, 10] only");
        }
        // B3b: F's rear off the release controller's X line (a golem's pulse, the REQ it left, or
        // EXPIRE), not the module's REL route: the tap (-8, -1, 6) reads X's dust (-7, -1, 6),
        // (-9, -1, 6) a dust, repeaters (-10, -1, 6) and (-11, -1, 6) (no dust there: the REQ line
        // (-10..-11, -1, 7) is beside them; the second's delay, `MCTAS_F_X_DELAY`, sets F's time),
        // then along x -12 to z 4 and west along z 4 into F's rear (-25, -1, 4)
        let dl: u8 = std::env::var("MCTAS_F_X_DELAY").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
        b.on([-8, -1, 6], &rep("east", 1))?;
        b.on([-9, -1, 6], DUST)?;
        b.on([-10, -1, 6], &rep("east", 1))?;
        b.on([-11, -1, 6], &rep("east", dl))?;
        let mut xr = vec![[-12, -1, 6]];
        seg_to(&mut xr, [-12, -1, 4]);
        seg_to(&mut xr, [FORCE[0], -1, 4]);
        route_d(&mut b, [-11, -1, 6], &xr, 13, 0, 1)?;
    } else {
        // (F's rear: a repeater off the kick route's dust near x -19 into (x, -4, 16), along z 16 to
        // F's column + 1, north under the siding to F's row + 2, up a stair north to F's row - 2 one
        // below F, a stair up west into the rear dust (FORCE - (0, 0, 2)))
        let fx = kick_dust_near(&b, -19)?;
        // (4 rt: F at the time the old route set it, +320 after the visit: ironspec's B3a)
        b.on([fx, -4, 17], &rep("south", 4))?;
        let mut fr = vec![[fx, -4, 16]];
        // (the route's first cell after its source: the tap repeater, or column 3 (c)'s FA)
        let mut from = [fx, -4, 17];
        if FAR {
            // (column 3 (c): F's gate FA right after the tap, a comparator at (fx, -4, 16) facing
            // south (rear: the tap), NS3 into its east side (a repeater (fx + 1, -4, 16)); its
            // gated output north and down a stair to y -6 (under the kick route's y -4, over the
            // module's top), west, south along x fx - 3 to z 56, east along z 56 under the far end
            // to x 11, up a stair south to F's support level, south along x 11 (outside the siding's
            // box) to F's z, west through three repeaters into F's support (F powered from below:
            // the turned siding's comparator reads it from the north, its east and west neighbours
            // at F's level are the siding's, z 74 is the next chunk))
            b.on([fx, -4, 16], &comp("south", "subtract"))?;
            b.on([fx + 1, -4, 16], &rep("east", 1))?;
            from = [fx, -4, 16];
            fr = vec![[fx, -4, 15], [fx, -5, 14], [fx, -6, 13]];
            seg_to(&mut fr, [fx - 3, -6, 13]);
            seg_to(&mut fr, [fx - 3, -6, 56]);
            seg_to(&mut fr, [FORCE[0] + 4, -6, 56]);
            seg_to(&mut fr, [FORCE[0] + 4, FORCE[1] - 1, 56 + FORCE[1] + 5]);
            seg_to(&mut fr, [FORCE[0] + 4, FORCE[1] - 1, FORCE[2]]);
        } else {
        seg_to(&mut fr, [FORCE[0] + 1, -4, 16]);
        seg_to(&mut fr, [FORCE[0] + 1, -4, FORCE[2] + 2]);
        seg_to(&mut fr, [FORCE[0] + 1, FORCE[1] - 1, FORCE[2] - 2]);
        fr.push([FORCE[0], FORCE[1], FORCE[2] - 2]);
        }
        route_d(&mut b, from, &fr, 6, 0, 1)?;
        // (measurement only, ironspec 2026-11-11: the integration gate's F +6 corner (the
        // missed-golem +6): the route's first straight dust a 3 rt repeater along it)
        if std::env::var("MCTAS_F_PLUS6").is_ok() {
            let mut all = vec![from];
            all.extend(fr.iter().copied());
            let i = (1..all.len() - 1)
                .find(|&i| {
                    let (a, c, n) = (all[i - 1], all[i], all[i + 1]);
                    a[1] == c[1] && c[1] == n[1] && [c[0] - a[0], c[2] - a[2]] == [n[0] - c[0], n[2] - c[2]] && b.blocks.get(&c).is_some_and(|s| s.contains("redstone_wire"))
                })
                .ok_or_else(|| anyhow::anyhow!("F's rear route has no straight dust for the +6 corner"))?;
            let (a, c) = (all[i - 1], all[i]);
            let input = match (c[0] - a[0], c[2] - a[2]) {
                (1, 0) => "west",
                (-1, 0) => "east",
                (0, 1) => "north",
                _ => "south",
            };
            b.blocks.insert(c, rep(input, 3));
        }
    }
    if FAR {
        // (column 3 (c): F's route ends at (F.x + 4, F.y - 1, F.z); three repeaters west into F's
        // support, which powers F from below)
        for dx in [3, 2, 1] {
            b.on([FORCE[0] + dx, FORCE[1] - 1, FORCE[2]], &rep("east", 1))?;
        }
        b.put([FORCE[0], FORCE[1] - 1, FORCE[2]], STONE)?;
    } else {
        b.on([FORCE[0], FORCE[1], FORCE[2] - 1], &comp("north", "subtract"))?;
    }
    b.on(FORCE, DUST)?;
    if FAR {
        // (column 3 (c): NS3 into FA's east side at the tap: from its port north along x -12 at
        // y -15 to z 8 (under NS5's row at z 9), up a stair north to y -9 at z 2, up a stair north
        // to y -4 at z -3, west along z -3 to x fx + 2, south along it into the repeater (fx + 1,
        // -4, 16))
        let fx = kick_dust_near(&b, -19)?;
        let s3 = at([29, -2, 3]);
        let mut n3 = vec![s3];
        seg_to(&mut n3, [s3[0], s3[1], s3[2] - 5]);
        seg_to(&mut n3, [s3[0], s3[1] + 6, s3[2] - 11]);
        // (a flat run of 3 between the climbs, for a repeater: a stair holds none)
        seg_to(&mut n3, [s3[0], s3[1] + 6, s3[2] - 14]);
        let zt = s3[2] - 14 - (-4 - (s3[1] + 6));
        seg_to(&mut n3, [s3[0], -4, zt]);
        seg_to(&mut n3, [fx + 2, -4, zt]);
        seg_to(&mut n3, [fx + 2, -4, 16]);
        route_d(&mut b, at([28, -2, 3]), &n3, 6, 6, 1)?;
    }
    if FAR {
    } else {
    let ns3_port = at([28, -2, 3]);
    // (north along x -12 at y -15 to z 8 (under NS5's crossing at z 9), up a stair north to y -9 at
    // z 2, north to z -7, west, up a stair west along z -7 to y -2, west to x -25 (clear of the
    // siding's entry climb on x -24 and the arming detector), south along x -25 to F's row - 1,
    // east, a stair up east onto (FORCE - (2, 0, 1)))
    let s3 = at([29, -2, 3]);
    let mut n3 = vec![s3];
    seg_to(&mut n3, [s3[0], s3[1], s3[2] - 5]);
    seg_to(&mut n3, [s3[0], s3[1] + 6, s3[2] - 11]);
    seg_to(&mut n3, [s3[0], s3[1] + 6, -7]);
    seg_to(&mut n3, [s3[0] - 2, s3[1] + 6, -7]);
    seg_to(&mut n3, [s3[0] - 2 - (-2 - (s3[1] + 6)), -2, -7]);
    seg_to(&mut n3, [FORCE[0] - 5, -2, -7]);
    seg_to(&mut n3, [FORCE[0] - 5, -2, FORCE[2] - 1]);
    seg_to(&mut n3, [FORCE[0] - 2, FORCE[1], FORCE[2] - 1]);
    // (the port is the module's 6th dust from NS3's torch; repeaters every 6: the climbs)
    route_d(&mut b, ns3_port, &n3, 6, 6, 1)?;
    b.on([FORCE[0] - 1, FORCE[1], FORCE[2] - 1], &rep("west", 1))?;
    }
    let mut ko = vec![[-21, -4, 31], [-21, -4, 32], [-21, -4, 33], [-21, -4, 34], [-21, -4, 35], [-21, -4, 36], [-21, -4, 37]];
    seg_to(&mut ko, [-18, -4, 37]);
    for k in 1..=3 {
        ko.push([-18 + k, -4 + k, 37]);
    }
    seg_to(&mut ko, [-15, -1, 30]);
    route(&mut b, [-21, -4, 30], &ko, 6)?;
    // each hold's driver: the bus dust (-15, -1, z + 2) up a stair onto (-14, 0, z + 2), a repeater
    // east into a block (-12, 0, z + 2), a repeater north into the hold's support (-12, 0, z) (a
    // block, not a dust, next to the other holds' supports: their kicks stay apart)
    for &h in &HOLDS {
        if FAR {
            // (column 3 (c): the hold's support (-5, 1, z) kicked like the others, one level up:
            // a repeater north into it from (-5, 1, z + 2)'s stone, a repeater east into that from a
            // dust row west along z + 2 at y 1 to x -13, down a stair onto the bus (-15, -1, z + 2))
            b.blocks.insert(h, STONE.into());
            let k2 = std::env::var("MCTAS_K2_KICK").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<u8>().ok()?, b.trim().parse::<u8>().ok()?)))).unwrap_or((3, 1));
            let (ra, rb) = if h[2] == 34 { k2 } else { (1, 1) };
            b.on([h[0], h[1], h[2] + 1], &rep("south", rb))?;
            b.put([h[0], h[1], h[2] + 2], STONE)?;
            b.on([h[0] - 1, h[1], h[2] + 2], &rep("west", ra))?;
            for x in -13..=h[0] - 2 {
                b.on([x, h[1], h[2] + 2], DUST)?;
            }
            b.on([-14, h[1] - 1, h[2] + 2], DUST)?;
            continue;
        }
        if h[0] != -12 {
            // slot 13's hold at x -11 sits over the controller's chain bus (-11, -1, 28..33): its
            // support stays the line's glass (a powered block there would power that dust), and a
            // repeater at the rail's level powers the hold rail itself: the bus stair's top
            // (-14, 0, z + 2), (-13, 0, z + 2), up a stair onto (-12, 1, z + 2), (-11, 1, z + 2), a
            // repeater (-11, 1, z + 1) north into the hold (-11, 1, z)
            b.on([-14, 0, h[2] + 2], DUST)?;
            b.on([-13, 0, h[2] + 2], DUST)?;
            b.on([-12, 1, h[2] + 2], DUST)?;
            b.on([-11, 1, h[2] + 2], DUST)?;
            b.on([-11, 1, h[2] + 1], &rep("south", 1))?;
            continue;
        }
        b.blocks.insert(h, STONE.into());
        // (slot 12's hold (k2, z 34) kicks 4 gt after the others: its repeater 3 rt, for no block
        // (ironspec 21:04: the siding's last corner, a stuck napper with a missed golem at k2, missed
        // its hold by 0.5-1.5 gt with the new fast kick; its cart lands 4 gt later, ~12 gt before
        // the next release's first advance). `MCTAS_K2_KICK=A,B` (the two repeaters' rt, default
        // 3,1) measures its margins only.
        let k2 = std::env::var("MCTAS_K2_KICK").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<u8>().ok()?, b.trim().parse::<u8>().ok()?)))).unwrap_or((3, 1));
        let (ra, rb) = if h[2] == 34 { k2 } else { (1, 1) };
        b.on([h[0], 0, h[2] + 1], &rep("south", rb))?;
        b.put([h[0], 0, h[2] + 2], STONE)?;
        b.on([h[0] - 1, 0, h[2] + 2], &rep("west", ra))?;
        b.on([h[0] - 2, 0, h[2] + 2], DUST)?;
    }
    }
    // (the divert unit's ring items)
    for (q, n) in &b.nbt {
        d.block_entities.insert(*q, n.clone());
    }
    let added: Vec<P3> = b.blocks.keys().copied().filter(|p| !before.contains(p)).collect();
    d.blocks = b.blocks;
    Ok(added)
}
