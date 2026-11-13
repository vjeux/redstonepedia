//! split9's napper hold release (day-1, Nov 12 2026; ironspec's ruling 5 + the launch width, 06:46; the
//! hold's drive from 7b90c3f1, accepted 07:34; ironspec 10:49 / 10:50: (b), an ungated 12-gt pulse and
//! the departure seen by a tripwire): a pulse every cycle, never a latch. ARM3 (the third cart's detector
//! rail, (5,0,7), up >= 20 gt a cycle) is cut to WIDTH gt by an edge comparator E (subtract: ARM3's dust in
//! its rear, ARM3 through a delay-2 and a delay-4 repeater on its side), then four 1-tick repeaters, the
//! last powering the drive's input cell (7,-1,8) from ARM3 + 10. 7b90c3f1's own repeater (8,-1,8) facing
//! west reads that cell and strongly powers the stone (9,-1,8) under the hold rail (9,0,8), so the rail
//! rises at ARM3 + 12: D is the rail's rise (7b90c3f1 08:30: its bench powers the rail at ARM3 + D, the
//! input block at ARM3 + D - 2), and D 12. (No repeater beside the rail: its input would be a face of
//! lane 7's merge T.)
//!
//! WIDTH 12 (ironspec 10:09) = max(6 to launch, 7 to clear) + 3 frozen (a login, <= 3 gt, stops the
//! entities while the redstone runs) + 2 margin. The departure is seen by 7b90c3f1's north-south string
//! (8,1,7) over the napper-only cell (8,0,7), its hooks (8,1,6) facing south and (8,1,8) facing north on
//! glass (G1: glass is a sturdy face but no conductor, so a powered hook's anchor powers nothing): a seated
//! rider presses a string over its cart's cell, an empty cart never does (tick-exact's recorded
//! `tripwire-rider` scene, eec7361f5); the resting napper (x 9.2..9.8) and lane 7's riders (x <= 7.8)
//! never reach the string's cell; the controller's state (the FIFO pop, the K / J(-1) timers) keys to the
//! string's rising edge only (a rolling press stays on until the hooks' 10-gt re-check).
//!
//! The bench drives ARM3 through a repeater for 20 gt every 100 gt (a repeater's output, like a detector
//! rail's, turns on inside a game tick), in one of two geometries: `Line` (the hold, then a straight line
//! east under a string) or `Real` (7b90c3f1's cells: the hold (9,0,8) against the slime stop (9,0,9), the T
//! (9,0,7), the booster (8,0,7) on its redstone block, lane 7's merge T (7,0,7) and track, the merge row
//! west, the string and hooks above, the 12-gt chain laid into (7,-1,8)). It prints: the input cell's and
//! the rail's pulses after ARM3 at every phase; a ridden cart at rest placed at every gt between two
//! pulses (it leaves at the next pulse, never before), the string's rising edge (matched within 20 gt of
//! the departure stamp: the rider's box enters the string's cell before the cart's box clears the hold
//! cell), presses while resting, and (Real) the merge row reached; a cart at rest frozen for `freeze` gt
//! (the part at level 32: redstone running, entities frozen) from every offset around the pulse; the launch
//! threshold under the freeze for widths 8, 10, 12 and 14; (Real) a lane-7 rider through its curve; and
//! the positive control, tick-exact's `tripwire-rider` scene cell for cell, against the game's ticks.

use redsim::block::Block;
use redsim::entity::EntityKind;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

type P = [i32; 3];

/// The design: the rail powered at ARM3 + D (the input cell at ARM3 + D - 2) for WIDTH gt, every CYCLE.
/// D 12 (day-1 08:33, on 7b90c3f1's K re-lay: behind = D + 17, ahead = 41 - D against the bars 21 + 2 /
/// 16 + 2; D 10's 27 / 31 left the tightest margin 4, D 12's 29 / 29 leaves 6 / 11; D 11 is off this
/// chain's 2-gt grid).
pub const D: i64 = 12;
pub const WIDTH: i64 = 12;
pub const CYCLE: i64 = 100;
/// E's extra side delay for WIDTH: a delay-2 repeater (redstone ticks) before the delay-4.
pub const EXTRA: i64 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Geo {
    Line,
    Real,
}

/// The cells a run reads: the drive block (placed and removed by the bench), ARM3's repeater (its output
/// = ARM3), the controller's last repeater (OUT, into the input cell), the hold rail, a hook of the
/// departure string, and where the ridden cart is placed (at rest on the hold).
struct Cells {
    drive: P,
    arm3: P,
    out: P,
    hold: P,
    hook: P,
    place: [f64; 3],
}

fn cells(g: Geo) -> Cells {
    match g {
        Geo::Line => Cells { drive: [0, 0, 6], arm3: [0, 0, 5], out: [0, 0, -1], hold: [2, 1, -2], hook: [3, 2, -1], place: [2.5, 1.0625, -1.5] },
        Geo::Real => Cells { drive: [-1, -1, 8], arm3: [0, -1, 8], out: [6, -1, 8], hold: [9, 0, 8], hook: [8, 1, 6], place: [9.5, 0.0625, 8.5] },
    }
}

const DUST: &str = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
const STRING_NS: &str = "minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]";

fn farm(blocks: Vec<(BlockPos, Block)>, output: BlockPos, lo: BlockPos, hi: BlockPos) -> FarmSpec {
    FarmSpec { blocks, entities: Vec::new(), output, lo, hi, block_entities: Vec::new(), rock: false, nether: None, land: false }
}

thread_local! {
    /// (ironspec 21:25 PT: every cart-motion bench prints its carts' block contacts per row; an undeclared
    /// contact, not a stop or buffer the design declares, FAILS the row) The contacts the benches' cart loops
    /// noted since the last row: cell (the bench's frame) -> (ticks, whether the cell is declared for the run's
    /// geometry at the time it was met).
    static CONTACTS: std::cell::RefCell<std::collections::BTreeMap<P, (u32, bool)>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    /// (G1's audit hole 15) The geometry of the run in progress, for its own declared list, and whether its cart has
    /// departed the hold (a contact declared "after the departure" is accepted only then).
    static CUR_GEO: std::cell::Cell<ContactGeo> = const { std::cell::Cell::new(ContactGeo::Line) };
    static DEPARTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// (the chain fix) The undeclared contacts the rows found, drained into the tool's verdict.
    static CONTACT_FAILS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// (G1's audit hole 15) Which geometry a run lays: each has its own declared contacts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContactGeo {
    Line,
    Real,
    /// the Real track with the napper lift laid below the exit rail
    Lift,
}

/// The declared contacts, each for one geometry (G1's audit hole 15: a declared list per geometry, plus the
/// departure-time check): the Line geometry's backstop (1,1,-2) west of its hold; the Real hold's stop (the slime
/// (9,0,9) the resting cart's box meets) and the bench's merge-row end (-3,0,7) after the departure only (the
/// bench's row stops at x -2; the module's runs on west to the lanes); the napper lift's containment at its foot
/// ((6,-28,3), (6,-28,5), (7,-28,4): the bubble column's glass beside and past its soul sand, 7b90c3f1's
/// return-bench cells, among the lift's 79 contact cells ironspec declared at 22:05 PT, official tick for tick),
/// with the Real cells too since the lift's runs end on the Real track.
const DECLARED: &[(P, &str, ContactGeo, bool)] = &[
    ([1, 1, -2], "the Line geometry's backstop", ContactGeo::Line, false),
    ([9, 0, 9], "the hold's stop", ContactGeo::Real, false),
    ([-3, 0, 7], "the bench's merge-row end (after the departure)", ContactGeo::Real, true),
    ([9, 0, 9], "the hold's stop", ContactGeo::Lift, false),
    ([-3, 0, 7], "the bench's merge-row end (after the departure)", ContactGeo::Lift, true),
    ([6, -28, 3], "the lift's containment", ContactGeo::Lift, false),
    ([6, -28, 5], "the lift's containment", ContactGeo::Lift, false),
    ([7, -28, 4], "the lift's containment", ContactGeo::Lift, false),
];

/// Whether a contact with `c` is declared for the run in progress (its geometry, and the departure for an
/// after-the-departure entry).
fn declared_now(c: P) -> Option<&'static str> {
    let geo = CUR_GEO.with(|g| g.get());
    let departed = DEPARTED.with(|d| d.get());
    DECLARED.iter().find(|(p, _, g, after)| *p == c && *g == geo && (!*after || departed)).map(|(_, w, _, _)| *w)
}

/// A run starts: its geometry, not yet departed.
fn start_contacts(g: ContactGeo) {
    CUR_GEO.with(|c| c.set(g));
    DEPARTED.with(|d| d.set(false));
}

/// Note the blocks cart `id` met in its last move: when the move collided horizontally, the collision boxes
/// its box meets grown 0.01 in x and z, the floor and the ceiling left out.
fn note_contacts(run: &Run, id: u32) {
    let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) else { return };
    if !e.horizontal_collision {
        return;
    }
    let bb = e.bb;
    let cells: Vec<P> = run
        .world
        .collision_shapes(bb.inflate(0.01, 0.0, 0.01))
        .into_iter()
        .filter(|a| a.max_y > bb.min_y + 1e-6 && a.min_y < bb.max_y - 1e-6)
        .map(|a| {
            let w = crate::mirror::at([((a.min_x + a.max_x) / 2.0).floor() as i32, ((a.min_y + a.max_y) / 2.0).floor() as i32, ((a.min_z + a.max_z) / 2.0).floor() as i32]);
            [w.x, w.y, w.z]
        })
        .collect();
    CONTACTS.with(|m| {
        let mut m = m.borrow_mut();
        for c in cells {
            let ok = declared_now(c).is_some();
            let ent = m.entry(c).or_insert((0, true));
            ent.0 += 1;
            ent.1 &= ok;
        }
    });
}

/// Append the contacts noted since the last row to the last row pushed, and clear them: the declared ones
/// named, any other a FAIL (recorded for the tool's verdict).
fn annotate(lines: &mut [String]) {
    let m = CONTACTS.with(|m| std::mem::take(&mut *m.borrow_mut()));
    let Some(last) = lines.last_mut() else { return };
    if m.is_empty() {
        last.push_str("; cart block contacts: none");
        return;
    }
    let (mut decl, mut bad) = (Vec::new(), Vec::new());
    for (c, (n, ok)) in &m {
        let what = DECLARED.iter().find(|(p, ..)| p == c).map(|(_, w, ..)| *w);
        match (ok, what) {
            (true, Some(what)) => decl.push(format!("({},{},{}) {what} ({n} gt)", c[0], c[1], c[2])),
            _ => bad.push(format!("({},{},{}) ({n} gt){}", c[0], c[1], c[2], if what.is_some() { ": declared for another geometry or only after the departure" } else { "" })),
        }
    }
    if !bad.is_empty() {
        let row: String = last.chars().take(80).collect();
        CONTACT_FAILS.with(|f| f.borrow_mut().push(format!("undeclared cart contacts {} (row: {row}...)", bad.join(", "))));
    }
    last.push_str(&format!(
        "; cart block contacts: declared {}; undeclared {}",
        if decl.is_empty() { "none".to_string() } else { decl.join(", ") },
        if bad.is_empty() { "none (pass)".to_string() } else { format!("{} (FAIL: an undeclared contact)", bad.join(", ")) }
    ));
}

/// Clear the contacts noted so far (before a bench's first row).
fn clear_contacts() {
    CONTACTS.with(|m| m.borrow_mut().clear());
    CONTACT_FAILS.with(|f| f.borrow_mut().clear());
    OVERRIDES.with(|m| m.borrow_mut().clear());
}

/// (the chain fix) The rows' undeclared contacts into the tool's verdict, one bar; the overrides; and (ironspec's
/// summon-order rule, item 9) the summoned villagers' scope: every run here has the mob AI off.
fn judge_contacts(v: &mut crate::verdict::Verdict) -> String {
    let fails = CONTACT_FAILS.with(|f| std::mem::take(&mut *f.borrow_mut()));
    let w = v.bar("every row's cart contacts declared for its geometry", fails.is_empty());
    let ov = judge_overrides(v);
    let ai = v.scope("the summoned villagers' brain activity", "every run has the mob AI off (RunOpts ai: false): no brain ticks, so the riders and the standing villagers are payload only, for cart physics and the string's press");
    if fails.is_empty() { format!("the cart contacts: every one declared ({w}); {ov}; {ai}") } else { format!("the cart contacts: {} ({w}); {ov}; {ai}", fails.join("; ")) }
}

thread_local! {
    /// (ironspec 04:07 PT, the chain audit C8) Every override the runs applied (a stand-in drive, a forced input, an
    /// emulated line, a control's left-out cell, a restart): label -> (its target's state before, after), the first
    /// application of each label kept.
    static OVERRIDES: std::cell::RefCell<std::collections::BTreeMap<String, (String, String)>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Record an override's first application: its target's state before and after.
fn note_override(label: &str, before: String, after: String) {
    OVERRIDES.with(|m| {
        m.borrow_mut().entry(label.to_string()).or_insert((before, after));
    });
}

/// The overrides the runs applied, each judged by the override rule (an unchanged target fails the run).
fn judge_overrides(v: &mut crate::verdict::Verdict) -> String {
    let m = OVERRIDES.with(|m| std::mem::take(&mut *m.borrow_mut()));
    if m.is_empty() {
        return "the overrides: none applied".to_string();
    }
    let parts: Vec<String> = m.iter().map(|(l, (b, a))| v.over(l, b, a)).collect();
    format!("the overrides (ironspec 04:07 PT, each target before -> after): {}", parts.join("; "))
}

/// Push a bench row and annotate it with the cart contacts noted while it ran.
fn push_row(lines: &mut Vec<String>, s: String) {
    lines.push(s);
    annotate(lines);
}

/// The Line geometry: the chain north along x 0, the hold rail east-west at (2,1,-2) on its stone, plain
/// rails east, the string over the first rail after the hold between hooks on glass.
fn spec_line(extra: i64) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, s: &str| -> Result<(), String> {
        blocks.push((BlockPos::new(x, y, z), b(s)?));
        Ok(())
    };
    for x in -4..=24 {
        for z in -5..=7 {
            put(x, -1, z, "minecraft:stone")?;
        }
    }
    // (a diode's `facing` points at its input, as in the game)
    // ARM3: a 1-tick repeater from the drive block, into ARM3's dust (E's rear)
    put(0, 0, 5, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?;
    put(0, 0, 4, DUST)?;
    // ARM3's branch to E's west side: (with `extra`) a delay-`extra` repeater, then the delay-4 repeater
    if extra > 0 {
        for p in [[-1, 0, 4], [-2, 0, 4], [-3, 0, 4], [-3, 0, 3]] {
            put(p[0], p[1], p[2], DUST)?;
        }
        put(-2, 0, 3, &format!("minecraft:repeater[delay={extra},facing=west,locked=false,powered=false]"))?;
    } else {
        for p in [[-1, 0, 4], [-2, 0, 4], [-2, 0, 3]] {
            put(p[0], p[1], p[2], DUST)?;
        }
    }
    put(-1, 0, 3, "minecraft:repeater[delay=4,facing=west,locked=false,powered=false]")?;
    // E: subtract, input from the south (ARM3's dust), output north
    put(0, 0, 3, "minecraft:comparator[facing=south,mode=subtract,powered=false]")?;
    // four 1-tick repeaters north, the last powering the input cell (0,0,-2) = (7,-1,8): a stone block
    for z in [2, 1, 0, -1] {
        put(0, 0, z, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?;
    }
    put(0, 0, -2, "minecraft:stone")?;
    // 7b90c3f1's repeater (8,-1,8) facing west: input the cell west of it, output east into the stone
    // under the hold rail
    put(1, 0, -2, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    put(2, 0, -2, "minecraft:stone")?;
    // the hold rail (east-west) on that stone, a backstop west of it, plain rails east on stone
    put(2, 1, -2, "minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]")?;
    put(1, 1, -2, "minecraft:stone")?;
    for x in 3..=23 {
        put(x, 0, -2, "minecraft:stone")?;
        put(x, 1, -2, "minecraft:rail[shape=east_west,waterlogged=false]")?;
    }
    // the departure string over the first rail after the hold, its hooks on glass north and south
    put(3, 2, 0, "minecraft:glass")?;
    put(3, 2, -4, "minecraft:glass")?;
    put(3, 2, -1, "minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    put(3, 2, -3, "minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    put(3, 2, -2, STRING_NS)?;
    Ok(farm(blocks, BlockPos::new(24, 1, 7), BlockPos::new(-5, -2, -6), BlockPos::new(25, 4, 8)))
}

/// The Real geometry, in 7b90c3f1's frame: the hold, the stop, the T, the booster, lane 7's merge T and
/// track, the merge row west, the departure string; the 12-gt chain laid at y -1 along z 8 into the input
/// cell (7,-1,8) (the bench's route: the real chain's cells are the controller box's).
fn spec_real(extra: i64) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, s: &str| -> Result<(), String> {
        blocks.push((BlockPos::new(x, y, z), b(s)?));
        Ok(())
    };
    // the chain's floor
    for x in -2..=8 {
        for z in 8..=12 {
            put(x, -2, z, "minecraft:stone")?;
        }
    }
    // ARM3: a 1-tick repeater from the drive block (-1,-1,8), into ARM3's dust (1,-1,8) (E's rear)
    put(0, -1, 8, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    put(1, -1, 8, DUST)?;
    // E's south side: ARM3's dust south and east, (with `extra`) a delay-`extra` repeater, the delay-4
    if extra > 0 {
        for p in [[1, -1, 9], [1, -1, 10], [1, -1, 11], [2, -1, 11]] {
            put(p[0], p[1], p[2], DUST)?;
        }
        put(2, -1, 10, &format!("minecraft:repeater[delay={extra},facing=south,locked=false,powered=false]"))?;
    } else {
        for p in [[1, -1, 9], [1, -1, 10], [2, -1, 10]] {
            put(p[0], p[1], p[2], DUST)?;
        }
    }
    put(2, -1, 9, "minecraft:repeater[delay=4,facing=south,locked=false,powered=false]")?;
    let direct = SPLIT_DIRECT.with(|c| c.get()).is_some();
    // E: subtract, rear from the west (ARM3's dust), output east (left out when the input cell is driven
    // directly)
    if !direct {
        put(2, -1, 8, "minecraft:comparator[facing=west,mode=subtract,powered=false]")?;
    }
    // four 1-tick repeaters east, the last powering the input cell (7,-1,8) (directly driven: the input cell
    // itself a floor lever on the chain's stone floor (7,-2,8), record7's conforming stand-in for the hold
    // input, a direct neighbour of 7b90c3f1's repeater (8,-1,8): a lever switched by the bench updates only
    // its own neighbours)
    for x in 3..=6 {
        put(x, -1, 8, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    }
    put(7, -1, 8, if direct { "minecraft:lever[face=floor,facing=north,powered=false]" } else { "minecraft:stone" })?;
    // 7b90c3f1's repeater (8,-1,8) facing west into the stone (9,-1,8) under the hold rail (9,0,8)
    put(8, -1, 8, "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]")?;
    // (ironspec 13:56, V14) the napper arrival's block under the hold rail is slime (a conductor: the
    // repeater (8,-1,8) powers the rail through it; the fired-pulse read comes off it)
    put(9, -1, 8, "minecraft:slime_block")?;
    put(9, 0, 8, "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
    // the stop and the glass over it: slime (ironspec 15:45, as landed in 7b90c3f1's lift-split and composed
    // by onechunk; tick-exact's rail-kick row and this bench show it acts as stone for the launch and the
    // rest: a minecart is not bounced by a slime block it runs into)
    put(9, 0, 9, "minecraft:slime_block")?;
    put(9, 1, 9, "minecraft:glass")?;
    // the T (9,0,7): arms N, S, W at its placement-order shape south_west (a cart from the hold turns west)
    put(9, -1, 7, "minecraft:glass")?;
    put(9, 0, 7, "minecraft:rail[shape=south_west,waterlogged=false]")?;
    // the booster (8,0,7) on its redstone block
    put(8, -1, 7, "minecraft:redstone_block")?;
    put(8, 0, 7, "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]")?;
    // lane 7's merge T (7,0,7) north_west and lane 7's track north of it
    put(7, -1, 7, "minecraft:glass")?;
    put(7, 0, 7, "minecraft:rail[shape=north_west,waterlogged=false]")?;
    for z in 1..=6 {
        put(7, -1, z, "minecraft:glass")?;
        put(7, 0, z, "minecraft:rail[shape=north_south,waterlogged=false]")?;
    }
    // the merge row west, ending on stone
    for x in -2..=6 {
        put(x, -1, 7, "minecraft:glass")?;
        put(x, 0, 7, "minecraft:rail[shape=east_west,waterlogged=false]")?;
    }
    put(-3, 0, 7, "minecraft:stone")?;
    // the departure string over (8,0,7), its hooks on glass
    put(8, 1, 5, "minecraft:glass")?;
    put(8, 1, 9, "minecraft:glass")?;
    put(8, 1, 6, "minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    put(8, 1, 8, "minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    put(8, 1, 7, STRING_NS)?;
    // (G1 on 6ca670551 (2)) the T's north arm, the napper's approach as rev 3 lays it: the lift's exit
    // rail (7,-3,4) on glass, the ascending (8,-3,4) on its redstone block (leaning on the curve's glass),
    // the curve (9,-2,4) south_west on glass, the slopes (9,-2,5) and (9,-1,6) on their redstone blocks
    put(7, -4, 4, "minecraft:glass")?;
    put(7, -3, 4, "minecraft:rail[shape=east_west,waterlogged=false]")?;
    // (G1 on 7ccd7737c point 1, ironspec 20:55 PT (2)(2)) the north arm's ascending cell as record7 lays it,
    // cell for cell: the module of record (onechunk's explore/farmopt-onechunk2 18bcaa92e,
    // f1q2-module-stop9.txt) has a powered rail, powered, ascending east at (8,-3,4) on a redstone block
    // (8,-4,4) (its layer -3 row z 4 `U`, layer -4 `q`); tick-exact's t-snap scene, the simulator's control,
    // keeps its own recorded cells (a plain ascending rail on stone at its (8,61,4))
    put(8, -4, 4, "minecraft:redstone_block")?;
    put(8, -3, 4, "minecraft:powered_rail[powered=true,shape=ascending_east,waterlogged=false]")?;
    put(9, -3, 4, "minecraft:glass")?;
    put(9, -2, 4, "minecraft:rail[shape=south_west,waterlogged=false]")?;
    put(9, -3, 5, "minecraft:redstone_block")?;
    put(9, -2, 5, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
    put(9, -2, 6, "minecraft:redstone_block")?;
    put(9, -1, 6, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, -5, -1), BlockPos::new(12, 4, 14)))
}

/// (ironspec 12:59, the box item) A (c′) layout in real in-chunk cells, read from a text file: one cell a
/// line `x y z state` (`... live` marks a cell of the live term, left out by the control); `bit x y z` (H2's
/// input dust), `q x y z` (the latch's state dust), `arm3 x y z` (the block the bench swaps to a redstone
/// block for 20 gt at each cycle start: the batch's ARM3; the napper's own pass over the real detector
/// rail (5,0,7) gives its second rise), `out x y z` (the fired pulse's cell, k's input). Laid over the
/// Real track with rev 3's hazards: the lanes' redstone blocks (x -1, 3, 7 at y -1, z 4..6), the merge
/// row's glass, the detector rail (5,0,7).
pub struct GateFile {
    cells: Vec<([i32; 3], String, bool)>,
    bit: [i32; 3],
    q: [i32; 3],
    arm3: [i32; 3],
    out: [i32; 3],
}

pub fn read_gate_file(path: &str) -> Result<GateFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    // (G1 on 7c330044b point 1: the bit and q lines are required, never a default (0,0,0); ARM3 is the real
    // detector rail's own cell (5,0,7), pressed by a cart, point 2)
    let mut f = GateFile { cells: Vec::new(), bit: [0; 3], q: [0; 3], arm3: [5, 0, 7], out: [7, -1, 8] };
    let (mut has_bit, mut has_q, mut has_out) = (false, false, false);
    let p3 = |w: &[&str]| -> Result<[i32; 3], String> {
        let n = |s: &str| s.parse::<i32>().map_err(|e| format!("{s}: {e}"));
        Ok([n(w[0])?, n(w[1])?, n(w[2])?])
    };
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() || w[0].starts_with('#') {
            continue;
        }
        match w[0] {
            "bit" | "q" | "arm3" | "out" if w.len() == 4 => {
                let p = p3(&w[1..4])?;
                match w[0] {
                    "bit" => {
                        f.bit = p;
                        has_bit = true;
                    }
                    "q" => {
                        f.q = p;
                        has_q = true;
                    }
                    "arm3" => f.arm3 = p,
                    _ => {
                        f.out = p;
                        has_out = true;
                    }
                }
            }
            "bit" | "q" | "arm3" | "out" => return Err(format!("{path}: bad line {line:?}: `{} x y z`", w[0])),
            // (G1's audit hole 13: a cell line is exactly `x y z state` or `x y z state live`; anything else, a
            // typo for live included, is refused, never read as not-live or dropped)
            _ if w.len() == 4 => f.cells.push((p3(&w[0..3])?, w[3].to_string(), false)),
            _ if w.len() == 5 && w[4] == "live" => f.cells.push((p3(&w[0..3])?, w[3].to_string(), true)),
            _ => return Err(format!("{path}: bad line {line:?}: a cell is `x y z state` or `x y z state live`")),
        }
    }
    if !has_bit || !has_q || !has_out {
        return Err(format!("{path}: the `bit x y z`, `q x y z` and `out x y z` lines are required (bit {has_bit}, q {has_q}, out {has_out})"));
    }
    if f.arm3 != [5, 0, 7] {
        return Err(format!("{path}: arm3 {:?}: ARM3 is driven by pressing the real detector rail (5,0,7) with a cart (G1 on 7c330044b: no stand-in with a reach the build lacks)", f.arm3));
    }
    Ok(f)
}

fn spec_gate_file(f: &GateFile, live: bool) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut cells: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
    let real = spec_real(2)?;
    let chain_or_string = |p: &BlockPos| {
        (p.y == -2 && (8..=12).contains(&p.z) && (-2..=8).contains(&p.x)) || (p.y == -1 && (8..=11).contains(&p.z) && (-1..=6).contains(&p.x)) || (p.y == 1 && p.x == 8 && (5..=9).contains(&p.z))
    };
    for (p, blk) in &real.blocks {
        if !chain_or_string(p) {
            cells.insert([p.x, p.y, p.z], blk.to_string());
        }
    }
    for x in [-1, 3, 7] {
        for z in 4..=6 {
            cells.insert([x, -1, z], "minecraft:redstone_block".into());
        }
    }
    for x in -4..=7 {
        cells.entry([x, -1, 7]).or_insert_with(|| "minecraft:glass".into());
    }
    cells.insert([5, 0, 7], "minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]".into());
    for (p, s, is_live) in &f.cells {
        if *is_live && !live {
            // (the chain audit C8: the control's left-out live cell, its target in the layout -> in this run)
            note_override(&format!("the live term left out (the phantom control: the file's live cell ({},{},{}))", p[0], p[1], p[2]), s.clone(), cells.get(p).cloned().unwrap_or_else(|| "minecraft:air".to_string()));
            continue;
        }
        cells.insert(*p, s.clone());
    }
    let mut blocks = Vec::new();
    for (p, s) in cells {
        if s == "minecraft:air" {
            continue;
        }
        blocks.push((BlockPos::new(p[0], p[1], p[2]), b(&s)?));
    }
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, -9, 0), BlockPos::new(11, 4, 11)))
}

/// One run of a file layout (as `gate_run`, the ARM3 drive by swapping `f.arm3`).
fn gate_run_file(f: &GateFile, live: bool, cycles: i64, spawn: Option<(i64, f64, f64)>, freeze: Option<(i64, i64)>) -> Result<GateRec, String> {
    let c = cells(Geo::Real);
    start_contacts(ContactGeo::Real);
    let sp = crate::mirror::checked(&spec_gate_file(f, live)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let bp = |p: P| crate::mirror::at(p);
    let powered = |run: &Run, p: P| run.world.get(bp(p)).to_string().contains("powered=true");
    let lit_dust = |run: &Run, p: P| {
        let s = run.world.get(bp(p)).to_string();
        s.contains("redstone_wire") && !s.contains("power=0")
    };
    let mut rec = GateRec::default();
    let mut id: Option<u32> = None;
    let (mut arm3_on, mut fired_on) = (false, false);
    let mut frozen: Option<[i32; 4]> = None;
    let mut last_pos: Option<(f64, f64, f64)> = None;
    // (G1 on 7c330044b point 2) ARM3: an empty cart set on the real detector rail (5,0,7) at each cycle start
    // and removed a tick later: the detector itself powers its neighbours (weak) and the glass below for
    // its own 20-gt check, with no stand-in block anywhere
    let mut presser: Option<u32> = None;
    let mut arm3_before: Option<String> = None;
    for t in 0..(cycles * CYCLE + 40) {
        let in_cycle = t.rem_euclid(CYCLE);
        if t / CYCLE < cycles && in_cycle == 0 {
            presser = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [f.arm3[0] as f64 + 0.5, f.arm3[1] as f64 + 0.0625, crate::mirror::zf(f.arm3[2] as f64 + 0.5)], Some("{Motion:[0.0d,0.0d,0.0d]}"))?);
            arm3_before = Some(run.world.get(bp(f.arm3)).to_string());
        } else if let Some(pid) = presser.take() {
            redsim::entity::kill(&mut run.world, pid)?;
        }
        if let Some((ts, x0, v)) = spawn
            && t == ts
        {
            id = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x0, -2.9375, crate::mirror::zf(4.5)], Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
        }
        if let Some((fr, len)) = freeze {
            if t == fr {
                frozen = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == fr + len
                && let Some(pb) = frozen.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        run.tick();
        if let Some(b) = arm3_before.take() {
            note_override("ARM3 pressed by an empty cart set on the real detector rail (5,0,7) each cycle", b, run.world.get(bp(f.arm3)).to_string());
        }
        if let Some(i) = id {
            note_contacts(&run, i);
        }
        let a = powered(&run, [5, 0, 7]);
        if a && !arm3_on {
            rec.arm3.push(t);
        }
        arm3_on = a;
        let out_now = lit_dust(&run, f.out) || powered(&run, [8, -1, 8]);
        edge(&mut rec.fired, &mut fired_on, out_now, t);
        if rec.det.is_none() && powered(&run, GATE_DET) {
            rec.det = Some(t);
        }
        if rec.bit.is_none() && lit_dust(&run, f.bit) {
            rec.bit = Some(t);
        }
        if let Some(i) = id
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == i && !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
        {
            let (x, y, z) = (e.pos.x, e.pos.y, crate::mirror::zf(e.pos.z));
            let in_hold = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2] && (y - 0.0625).abs() < 0.2;
            let still = last_pos.is_some_and(|(px, _, pz)| (px - x).abs() < 0.001 && (pz - z).abs() < 0.001);
            // (tick-exact's official rest, napper-ride cd070f992: in the hold cell, horizontal speed under
            // 0.005; `still` (moved under 0.001) kept for the late placements' bookkeeping only)
            let hs = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
            let _ = still;
            if in_hold && hs < 0.005 && rec.rest.is_none() {
                rec.rest = Some(t);
            }
            if in_hold && rec.entered.is_none() {
                rec.entered = Some(t);
            }
            if rec.entered.is_some() && !in_hold && rec.left.is_none() {
                rec.left = Some(t);
            }
            if rec.rest.is_some() && !in_hold && rec.off.is_none() {
                rec.off = Some(t);
                rec.off_powered = powered(&run, c.hold);
                DEPARTED.with(|d| d.set(true));
            }
            last_pos = Some((x, y, z));
        }
    }
    rec.q_end = lit_dust(&run, f.q);
    Ok(rec)
}

/// (ironspec 04:07 PT) The gate file's overrides: each file cell's state in the base layout before it is laid and
/// after; an override that leaves its cell as it was is a silent no-op.
fn gate_file_overrides(f: &GateFile) -> Result<Vec<(P, String, String)>, String> {
    let real = spec_real(2)?;
    let chain_or_string = |p: &BlockPos| {
        (p.y == -2 && (8..=12).contains(&p.z) && (-2..=8).contains(&p.x)) || (p.y == -1 && (8..=11).contains(&p.z) && (-1..=6).contains(&p.x)) || (p.y == 1 && p.x == 8 && (5..=9).contains(&p.z))
    };
    let mut base: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
    for (p, blk) in &real.blocks {
        if !chain_or_string(p) {
            base.insert([p.x, p.y, p.z], blk.to_string());
        }
    }
    for x in [-1, 3, 7] {
        for z in 4..=6 {
            base.insert([x, -1, z], "minecraft:redstone_block".into());
        }
    }
    for x in -4..=7 {
        base.entry([x, -1, 7]).or_insert_with(|| "minecraft:glass".into());
    }
    base.insert([5, 0, 7], "minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]".into());
    let norm = |s: &str| Block::parse(s).map(|b| b.to_string()).unwrap_or_else(|_| s.to_string());
    Ok(f.cells.iter().map(|(p, s, _)| (*p, base.get(p).map_or("minecraft:air".to_string(), |b| norm(b)), norm(s))).collect())
}

/// The file layout's rows (ironspec 12:59: the 300 arrivals, the freeze offsets, the ARM3 control, on
/// the real cells); `quick` runs one speed. (The chain fix) every bar, control and override through `v`.
pub fn gate_file_bench(path: &str, freeze: i64, quick: bool, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    if freeze < 1 {
        return Err(format!("--freeze {freeze}: at least 1 gt (0 thaws in the tick it freezes, so the freeze rows would test nothing)"));
    }
    let f = read_gate_file(path)?;
    clear_contacts();
    let mut lines = Vec::new();
    // (ironspec 04:07 PT: every override printed with its cell's state before and after; a no-op fails the run)
    let ov = gate_file_overrides(&f)?;
    let ov_w: Vec<String> = ov.iter().map(|(p, a, b)| v.over(&format!("({},{},{})", p[0], p[1], p[2]), a, b)).collect();
    lines.push(format!("the gate file's overrides ({}; ironspec 04:07 PT, each cell's state before -> after): {}", ov.len(), ov_w.join("; ")));
    let base = gate_run_file(&f, true, 4, Some((5, 7.3, 0.3)), None)?;
    let blocks = f.cells.len();
    push_row(&mut lines, format!(
        "(c′) real cells from {path} ({blocks} cells, {} of the live term): detector -> H2 {} gt; one arrival: rest {:?}, bit {:?}, fired {:?}, departure {:?} (rail powered then {})",
        f.cells.iter().filter(|c| c.2).count(),
        match (base.det, base.bit) {
            (Some(d), Some(b)) => (b - d).to_string(),
            _ => "-".into(),
        },
        base.rest,
        base.bit,
        base.fired,
        base.off,
        base.off_powered
    ));
    let speeds: &[f64] = if quick { &[0.25] } else { &[0.1, 0.25, 0.4] };
    let (mut n, mut one, mut bad, mut late, mut q_left) = (0u32, 0u32, Vec::new(), 0u32, 0u32);
    let (mut mmin, mut mmax, mut dmin, mut dmax) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for &sp in speeds {
        for ph in 0..CYCLE {
            let r = gate_run_file(&f, true, 5, Some((110 + ph, 7.3, sp)), None)?;
            n += 1;
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                mmin = mmin.min(bit - rest);
                mmax = mmax.max(bit - rest);
            } else {
                // (G1 on 7c330044b point 1: missing data is a failure, never a drop)
                bad.push(format!("v {sp} spawn {}: rest {:?}, bit {:?}", 110 + ph, r.rest, r.bit));
            }
            let fires: Vec<i64> = r.fired.iter().filter(|&&(f0, _)| r.det.is_some_and(|d| f0 > d)).map(|x| x.0).collect();
            if fires.len() == 1 {
                one += 1;
            } else {
                bad.push(format!("v {sp} spawn {}: {} fired", 110 + ph, fires.len()));
            }
            if let Some(&f0) = fires.first() {
                // (E measured: the last ARM3 rise at or before the fired pulse)
                match (r.arm3.iter().copied().filter(|&a| a <= f0).max(), r.off) {
                    (Some(e), Some(o)) => {
                        dmin = dmin.min(o - e);
                        dmax = dmax.max(o - e);
                        late += u32::from(!r.off_powered);
                    }
                    (e, o) => bad.push(format!("v {sp} spawn {}: ARM3 before the fired pulse {e:?}, departure {o:?}", 110 + ph)),
                }
            }
            q_left += u32::from(r.q_end);
        }
    }
    let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
    let w_margin = v.bar_n("the gate file's arrivals: the bit at H2 >= 2 gt after rest", usize::from(mmin != i64::MAX) * n as usize, mmin != i64::MAX && mmin >= 2);
    let w_runs = v.bar_n("the gate file's arrivals: every run one fired pulse with its rest, bit, ARM3 and departure", n as usize, bad.is_empty());
    let w_late = v.bar_n("the gate file's arrivals: no departure late (the rail unpowered)", n as usize, late == 0);
    let w_q = v.bar_n("the gate file's arrivals: the latch never left set", n as usize, q_left == 0);
    push_row(&mut lines, format!(
        "  arrivals ({n} runs: v {:?} x every gt of a cycle; ARM3 by a cart pressing the real detector rail (5,0,7), the napper's own from its pass over it): the bit at H2 after rest {}..{} gt (bar >= 2: {w_margin}); exactly one fired pulse {one} ({w_runs}{}); the departure at E's measured rise + {}..{}, late {late} ({w_late}); the latch left set {q_left} ({w_q})",
        speeds,
        m(mmin, i64::MAX),
        m(mmax, i64::MIN),
        if bad.is_empty() { String::new() } else { format!("; the misses {}: {}", bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        m(dmin, i64::MAX),
        m(dmax, i64::MIN)
    ));
    let r0 = gate_run_file(&f, true, 5, Some((110, 7.3, 0.1)), None)?;
    if let (Some(d1), Some(rs1), Some(&(f0, _))) = (r0.det, r0.rest, r0.fired.iter().find(|&&(x, _)| r0.det.is_some_and(|d| x > d))) {
        let (mut ok, mut nbad, mut least, mut runs) = (0u32, 0u32, i64::MAX, 0usize);
        for t in (d1 - 3..=rs1).chain(f0 - 5..=f0 + 16) {
            let r = gate_run_file(&f, true, 5, Some((110, 7.3, 0.1)), Some((t, freeze)))?;
            runs += 1;
            let fires = r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                least = least.min(bit - rest);
            } else {
                nbad += 1;
            }
            if fires == 1 && r.off.is_some() && r.off_powered {
                ok += 1;
            } else {
                nbad += 1;
            }
        }
        let w = v.bar_n(&format!("the gate file's {freeze}-gt freeze rows: one fired launch each, the bit >= 2 gt after rest"), runs, nbad == 0 && least != i64::MAX && least >= 2);
        push_row(&mut lines, format!(
            "  a {freeze}-gt freeze at every offset of the approach (detector - 3 .. rest) and around the fired pulse (- 5 .. + 16): launched on one fired pulse {ok}, failures (no single fired launch, or a run missing its rest or bit) {nbad}; the bit after rest at least {} gt ({w})",
            m(least, i64::MAX)
        ));
    } else {
        v.fail("the gate file's freeze rows: NOT JUDGED (the reference run lacks a detector rise, a rest or a fired pulse)");
        push_row(&mut lines, format!("  the freeze row: FAIL, the reference run (v 0.1) lacks a detector rise, a rest or a fired pulse (det {:?}, rest {:?})", r0.det, r0.rest));
    }
    let ctl = gate_run_file(&f, false, 5, Some((110, 7.3, 0.3)), None)?;
    let lv = gate_run_file(&f, true, 5, Some((110, 7.3, 0.3)), None)?;
    let nf = |r: &GateRec| r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
    let w_live = v.bar("the gate file: one fired pulse for one rest with the live term", nf(&lv) == 1);
    let w_ctl = v.control("the gate file without the live term fires the phantom", 1, nf(&ctl) > 1);
    push_row(&mut lines, format!(
        "  the napper's own second ARM3 rise (its pass over the detector rail (5,0,7)): with the live term {} fired pulse(s) ({w_live}); the control without it {} ({w_ctl})",
        nf(&lv),
        nf(&ctl)
    ));
    lines.push(format!("  {}", judge_contacts(v)));
    Ok(lines)
}

/// (ironspec 15:23 (2)) The napper's ride from R to rest with the lift laid, every tick: t (ticks after the
/// summon, as tick-exact's napper-ride counts), the cart's position and motion, the approach detector's
/// state; the rise, the T entry (its cell becoming the T's) and the rest (the hold cell, horizontal speed
/// under 0.005). The summon as tick-exact's: (4.5, -18.9375 - depth, 4.5), Motion (v, 0, 0), a villager
/// riding. (G1's audit hole 12) The match with tick-exact's official trace (artifact 28968963999451103: the
/// official summon (4.5, -27.9375, 4.5) at 0.2 east, the lift 9 down) is machine-checked: the rise at t57, the T
/// entry at t85 at (9.5, 0.0625, 7.2316) and 0.5929, the rest at t89 (z 8.5100); any other summon has no official
/// trace and is refused.
pub fn lift_trace(k: i64, depth: i32, v: f64, ticks: i64, verdict: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    if depth != 9 || (v - 0.2).abs() > 1e-9 {
        return Err(format!("--lift-trace {v} (the lift {depth} down): tick-exact's official trace is at 0.2 with the lift 9 down; no other summon has one to compare against"));
    }
    GATE_LIFT.with(|c| c.set(depth));
    clear_contacts();
    start_contacts(ContactGeo::Lift);
    let res = (|| -> Result<Vec<String>, String> {
        let c = cells(Geo::Real);
        let sp = crate::mirror::checked(&spec_gate(2, true, k)?)?;
        let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
        let mut run = Run::start(&sp, &ro)?;
        while run.settle_tick() {}
        let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [4.5, -18.9375 - depth as f64, 4.5], Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?;
        let mut lines = vec![format!("the napper's ride, every tick (redsim; the summon at (4.5, {:.4}, 4.5) Motion ({v}, 0, 0), the lift {depth} down; t = ticks after the summon, as tick-exact's napper-ride)", -18.9375 - depth as f64)];
        let (mut rise, mut t_entry, mut rest) = (None::<i64>, None::<(i64, [f64; 3], f64)>, None::<(i64, [f64; 3])>);
        for t in 1..=ticks {
            run.tick();
            note_contacts(&run, id);
            let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) else {
                lines.push(format!("t{t}: no cart"));
                break;
            };
            let p = [e.pos.x, e.pos.y, e.pos.z];
            let m = [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z];
            let hs = (m[0] * m[0] + m[2] * m[2]).sqrt();
            let det = run.world.get(crate::mirror::at(GATE_DET)).to_string().contains("powered=true");
            let cell = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
            if rise.is_none() && det {
                rise = Some(t);
            }
            if t_entry.is_none() && cell == [9, 0, 7] {
                t_entry = Some((t, p, hs));
            }
            if rest.is_none() && cell == c.hold && hs < 0.005 {
                rest = Some((t, p));
            }
            lines.push(format!("t{t}: ({:.4}, {:.4}, {:.4}) motion ({:+.4}, {:+.4}, {:+.4}) horizontal {hs:.4}{}", p[0], p[1], p[2], m[0], m[1], m[2], if det { " DETECTOR" } else { "" }));
            if rest.is_some_and(|(r, _)| t > r + 2) {
                break;
            }
        }
        push_row(&mut lines, format!(
            "summary: the rise at {}; the T entry at {}; rest at {}; detector -> rest {}",
            rise.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
            t_entry.map(|(t, p, s)| format!("t{t} ({:.4}, {:.4}, {:.4}) {s:.4}", p[0], p[1], p[2])).unwrap_or_else(|| "-".into()),
            rest.map(|(t, p)| format!("t{t} ({:.4}, {:.4}, {:.4})", p[0], p[1], p[2])).unwrap_or_else(|| "-".into()),
            match (rise, rest) {
                (Some(a), Some((b, _))) => format!("{} gt", b - a),
                _ => "-".into(),
            }
        ));
        // (G1's audit hole 12: tick-exact's official trace, machine-checked)
        let near = |a: f64, b: f64| (a - b).abs() < 5e-5;
        let rise_ok = rise == Some(57);
        let entry_ok = t_entry.is_some_and(|(t, p, s)| t == 85 && near(p[0], 9.5) && near(p[1], 0.0625) && near(p[2], 7.2316) && near(s, 0.5929));
        let rest_ok = rest.is_some_and(|(t, p)| t == 89 && near(p[2], 8.51));
        let w_rise = verdict.bar("the rise at t57 as the official trace", rise_ok);
        let w_entry = verdict.bar("the T entry at t85, (9.5, 0.0625, 7.2316) at 0.5929, as the official trace", entry_ok);
        let w_rest = verdict.bar("rest at t89 (z 8.5100) as the official trace", rest_ok);
        lines.push(format!("  against tick-exact's official trace (artifact 28968963999451103): the rise {w_rise}; the T entry {w_entry}; the rest {w_rest}; {}", judge_contacts(verdict)));
        Ok(lines)
    })();
    GATE_LIFT.with(|c| c.set(0));
    res
}

fn spec(g: Geo, extra: i64) -> Result<FarmSpec, String> {
    match g {
        Geo::Line => spec_line(extra),
        Geo::Real => spec_real(extra),
    }
}

/// (ironspec 11:43 / 11:49, (c′)) The presence-gated hold release, as blocks in the Real geometry (the
/// string dropped): the lift's exit rail (7,-3,4) becomes the approach detector; its read repeater (7,-3,3)
/// sets a loop latch (a repeater R1 into a subtract comparator C whose output dust loops back to R1; the
/// fired pulse, tapped off the hold's stone (9,-1,8), resets it through C's side); the latch's Q runs
/// through the settle delay (the chain sized for k gt from the detector to H2) into the rear of a
/// subtract comparator P whose side is NOT Q (a torch on a block the live Q powers through a repeater):
/// P = the delayed bit AND the live bit (`live` false: the torch left out, the control); H2 samples P,
/// locked by L2 from E's line at E + 2; the AND (7,-1,9) (subtract: rear E through E_d, side NOT H2)
/// fires into the input cell (7,-1,8) at ARM3 + 10 (D 12 unchanged). ARM3's drive and E (with its 12-gt
/// side) sit east of the gate (the bench's own route; the real cells are the controller box's).
pub const GATE_DRIVE: [i32; 3] = [20, -1, 13];
thread_local! {
    /// (ironspec 14:15) A restart (every chunk saved and loaded) before this tick in this order, for the
    /// next `gate_run`.
    static GATE_RESTART: std::cell::Cell<Option<(i64, crate::liveness::Restart)>> = const { std::cell::Cell::new(None) };
    /// (G1 on ed3a09628) The napper lift laid below the exit rail at this extra depth (0: none, the cart
    /// starts on the exit rail): 7b90c3f1's return-bench napper lift (R (4,-19-d,4) and its entry on glass,
    /// the soul sand under the column (6,-19-d..-4,4) with its glass walls, the top's water source (5,-3,4)
    /// pushing the cart east onto the exit rail (7,-3,4)), the napper summoned at R heading east.
    static GATE_LIFT: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    /// (ironspec 14:41) The ungated control: the AND's side (NOT H2) left out, so the pulse fires every
    /// cycle whatever the hold holds.
    static GATE_UNGATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// (G1 on 652881d4f point 4) The E-within-k control: drive E once more this many gt after the run's
    /// first fired pulse (a 20-gt drive), so the E-within-k row must count it.
    static GATE_EXTRA_E: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
    /// (ironspec 20:55 PT (2)(6), G1 on 7ccd7737c point 6) split9's input cell driven directly for this many
    /// gt (odd widths included), not by the chain: E's comparator left out and the input cell (7,-1,8) a floor
    /// lever on the chain's stone floor (record7's conforming stand-in for the hold input), switched by the
    /// bench so the rail rises at ARM3 + D as the chain's does.
    static SPLIT_DIRECT: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}
const GATE_ARM3: [i32; 3] = [19, -1, 13];
const GATE_BIT: [i32; 3] = [10, -1, 11];
const GATE_Q: [i32; 3] = [15, -1, 5];
const GATE_DET: [i32; 3] = [7, -3, 4];

fn spec_gate(extra: i64, live: bool, k: i64) -> Result<FarmSpec, String> {
    // (G1 on 652881d4f point 2: the settle chain lays only even k in 36..=48; refuse any other here, so no
    // caller can print a k that was not laid)
    if !(36..=48).contains(&k) || k % 2 != 0 {
        return Err(format!("spec_gate: k {k} is not on the settle chain (even, 36..=48)"));
    }
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut cells: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
    // the Real geometry's track (its cells as spec_real lays them, without the ungated chain and the string)
    let real = spec_real(extra)?;
    let chain_or_string = |p: &BlockPos| {
        (p.y == -2 && (8..=12).contains(&p.z) && (-2..=8).contains(&p.x)) || (p.y == -1 && (8..=11).contains(&p.z) && (-1..=6).contains(&p.x)) || (p.y == 1 && p.x == 8 && (5..=9).contains(&p.z))
    };
    for (p, blk) in &real.blocks {
        if !chain_or_string(p) {
            cells.insert([p.x, p.y, p.z], blk.to_string());
        }
    }
    let mut put = |x: i32, y: i32, z: i32, s: &str| {
        cells.insert([x, y, z], s.to_string());
    };
    let rep = |delay: i64, facing: &str| format!("minecraft:repeater[delay={delay},facing={facing},locked=false,powered=false]");
    let cmp = |facing: &str| format!("minecraft:comparator[facing={facing},mode=subtract,powered=false]");
    // the floors: y -2 under the gate (x 7..20, z 1..16), except the track's own y -2 cells and the
    // approach's clearance (the detector rail (7,-3,4), the ascending (8,-3,4), the curve and slopes: no
    // block over a rider's path; and none at x 10 beside the curve (9,-2,4) and the slope (9,-2,5): the
    // cart's box swings out to x 10.16 there (tick-exact's official ride, t79), and x 10 is outside the
    // chunk, empty in the module: a floor there pushed the cart back and cost it 4 gt to rest)
    for x in 7..=20 {
        for z in 1..=16 {
            if !((7..=10).contains(&x) && (2..=7).contains(&z)) {
                put(x, -2, z, "minecraft:stone");
            }
        }
    }
    // (G1 on ed3a09628) the napper lift below the exit rail, as 7b90c3f1's return-bench lays it
    let lift = GATE_LIFT.with(|c| c.get());
    if lift > 0 {
        let rb_y = -19 - lift;
        put(4, rb_y, 4, "minecraft:rail[shape=east_west,waterlogged=false]");
        put(4, rb_y - 1, 4, "minecraft:glass");
        put(5, rb_y, 4, "minecraft:rail[shape=east_west,waterlogged=false]");
        put(5, rb_y - 1, 4, "minecraft:glass");
        put(6, rb_y - 1, 4, "minecraft:soul_sand");
        for y in rb_y..=-4 {
            put(6, y, 4, "minecraft:water[level=0]");
            put(7, y, 4, "minecraft:glass");
            put(6, y, 3, "minecraft:glass");
            put(6, y, 5, "minecraft:glass");
            if y > rb_y {
                put(5, y, 4, "minecraft:glass");
            }
        }
        put(7, rb_y - 1, 4, "minecraft:glass");
        put(6, rb_y - 1, 3, "minecraft:glass");
        put(6, rb_y - 1, 5, "minecraft:glass");
        put(6, -3, 3, "minecraft:glass");
        put(6, -3, 5, "minecraft:glass");
        put(6, -2, 4, "minecraft:glass");
        put(5, -3, 4, "minecraft:water[level=0]");
        put(4, -3, 4, "minecraft:glass");
        put(5, -3, 3, "minecraft:glass");
        put(5, -3, 5, "minecraft:glass");
        put(5, -2, 4, "minecraft:glass");
    }
    // the approach detector and its read repeater (north), the SET line along z 1 at y -3, climbing to y -1
    put(GATE_DET[0], GATE_DET[1], GATE_DET[2], "minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]");
    put(7, -4, 3, "minecraft:glass");
    put(7, -3, 3, &rep(1, "south"));
    for (x, z) in [(7, 2), (7, 1), (8, 1), (9, 1), (10, 1), (11, 1)] {
        put(x, -4, z, "minecraft:stone");
        put(x, -3, z, DUST);
    }
    // (the climb: (11,-3,1) -> (12,-2,1) on the step (12,-3,1) -> (13,-1,1) on the step (13,-2,1))
    put(11, -2, 1, "minecraft:air");
    put(12, -3, 1, "minecraft:stone");
    put(12, -2, 1, DUST);
    put(12, -1, 1, "minecraft:air");
    put(13, -1, 1, DUST);
    put(14, -1, 1, &rep(1, "west"));
    for (x, z) in [(15, 1), (16, 1), (17, 1), (18, 1), (19, 1), (20, 1), (20, 2), (20, 3), (20, 4)] {
        put(x, -1, z, DUST);
    }
    // the loop latch: R1 (17,-1,5) into C (16,-1,5), C's output dust round to R1's input; RESET into C's
    // south side
    put(17, -1, 5, &rep(1, "east"));
    put(16, -1, 5, &cmp("east"));
    for (x, z) in [(15, 5), (14, 5), (14, 4), (14, 3), (15, 3), (16, 3), (17, 3), (18, 3), (19, 3), (19, 4), (19, 5), (18, 5)] {
        put(x, -1, z, DUST);
    }
    put(16, -1, 6, &rep(1, "south"));
    // the reset line: the hold's stone (9,-1,8) read by (10,-1,8), along z 8 to (16,-1,7)
    put(10, -1, 8, &rep(1, "west"));
    for (x, z) in [(11, 8), (12, 8), (13, 8), (14, 8), (15, 8), (16, 8), (16, 7)] {
        put(x, -1, z, DUST);
    }
    // the settle delay: the tap (19,-1,6), dust to (18,-1,10), the chain (17..13,-1,10) into P's rear: a
    // delay-a repeater, 3 delay-4, a delay-b (with the read, SET, tap and P repeaters, the detector to H2
    // in 32 + 2 (a + b) gt; ironspec 14:36: k = R_max + F + 2 at even widths, a repeater delay; k 36..48)
    let n = ((k - 32) / 2).max(2);
    let (ra, rb) = ((n - 1).clamp(1, 4), (n - (n - 1).clamp(1, 4)).clamp(1, 4));
    put(19, -1, 6, &rep(1, "north"));
    for (x, z) in [(19, 7), (19, 8), (19, 9), (19, 10), (18, 10)] {
        put(x, -1, z, DUST);
    }
    put(17, -1, 10, &rep(ra, "east"));
    for x in 14..=16 {
        put(x, -1, 10, &rep(4, "east"));
    }
    put(13, -1, 10, &rep(rb, "east"));
    // P (12,-1,10): rear the delay, side NOT Q (the live term), output west into H2's input dust
    // (12-1,10)->(11,-1,10)->(11,-1,11)->(10,-1,11). (A comparator's side reads only dust, diodes and
    // redstone blocks: a torch beside it gives nothing (its direct signal is upward only); each NOT
    // reaches its comparator's side through a repeater or a dust.)
    put(12, -1, 10, &cmp("east"));
    put(11, -1, 10, DUST);
    put(11, -1, 11, DUST);
    put(10, -1, 11, DUST);
    if live {
        // NOT Q: the live Q from (19,-1,8) down to (20,-2,8), along y -2 to (15,-2,11), up to (15,-1,12),
        // a repeater into B3 (13,-1,12), a wall torch on B3's west face (12,-1,12), a repeater (12,-1,11)
        // into P's south side
        for (x, z) in [(20, 8), (20, 9), (20, 10), (20, 11), (19, 11), (18, 11), (17, 11), (16, 11), (15, 11)] {
            put(x, -3, z, "minecraft:stone");
            put(x, -2, z, DUST);
        }
        put(20, -1, 8, "minecraft:air");
        put(15, -1, 12, DUST);
        put(14, -1, 12, &rep(1, "east"));
        put(13, -1, 12, "minecraft:stone");
        put(12, -1, 12, "minecraft:redstone_wall_torch[facing=west,lit=true]");
        put(12, -1, 11, &rep(1, "south"));
    }
    // H2 (9,-1,11) into B2 (8,-1,11); L2 (9,-1,12) locks it from E's line at (9,-1,13); NOT H2: a wall
    // torch on B2's north face (8,-1,10) into the dust (8,-1,9) on the AND's east side
    put(9, -1, 11, &rep(1, "east"));
    put(8, -1, 11, "minecraft:stone");
    put(9, -1, 12, &rep(1, "south"));
    // (ironspec 14:41, the ungated control: no NOT H2, the AND passes E_d every cycle)
    if !GATE_UNGATED.with(|c| c.get()) {
        put(8, -1, 10, "minecraft:redstone_wall_torch[facing=north,lit=true]");
    }
    put(8, -1, 9, DUST);
    // the AND (7,-1,9) into the input cell; E_d (7,-1,10) delay 2 behind a 1-tick repeater (7,-1,11) (a
    // repeater, not dust, beside B2)
    put(7, -1, 9, &cmp("south"));
    put(7, -1, 10, &rep(2, "south"));
    put(7, -1, 11, &rep(1, "south"));
    // E's line: E's output (16,-1,13), (16,-1,14), west along z 14 to (7,-1,14), north to (7,-1,12); L2's
    // branch (9,-1,13)
    for (x, z) in [(16, 13), (16, 14), (15, 14), (14, 14), (13, 14), (12, 14), (11, 14), (10, 14), (9, 14), (8, 14), (7, 14), (7, 13), (7, 12), (9, 13)] {
        put(x, -1, z, DUST);
    }
    // ARM3 (the bench's drive at (20,-1,13) through a repeater), E (17,-1,13) with its 12-gt side
    put(GATE_ARM3[0], GATE_ARM3[1], GATE_ARM3[2], &rep(1, "east"));
    put(18, -1, 13, DUST);
    put(17, -1, 13, &cmp("east"));
    for (x, z) in [(18, 14), (18, 15), (18, 16), (17, 16)] {
        put(x, -1, z, DUST);
    }
    put(17, -1, 15, &rep(extra.max(1), "south"));
    put(17, -1, 14, &rep(4, "south"));
    // (the chain audit C8: each control's left-out cell and the lift's laid cells, as overrides with their target's
    // state in the design -> in this run)
    let at = |p: [i32; 3]| cells.get(&p).cloned().unwrap_or_else(|| "minecraft:air".to_string());
    if !live {
        note_override("the live term left out (the phantom control: P's NOT Q torch (12,-1,12))", "minecraft:redstone_wall_torch[facing=west,lit=true]".into(), at([12, -1, 12]));
    }
    if GATE_UNGATED.with(|c| c.get()) {
        note_override("the ungated control (GATE_UNGATED: the AND's NOT H2 torch (8,-1,10))", "minecraft:redstone_wall_torch[facing=north,lit=true]".into(), at([8, -1, 10]));
    }
    if lift > 0 {
        note_override(&format!("the napper lift laid {lift} down (GATE_LIFT: the soul sand under its column (6,{},4))", -20 - lift), "minecraft:air".into(), at([6, -20 - lift, 4]));
    }
    let mut blocks = Vec::new();
    for (p, s) in cells {
        if s == "minecraft:air" {
            continue;
        }
        blocks.push((BlockPos::new(p[0], p[1], p[2]), b(&s)?));
    }
    Ok(farm(blocks, BlockPos::new(-4, 0, 7), BlockPos::new(-5, if GATE_LIFT.with(|c| c.get()) > 0 { -23 - GATE_LIFT.with(|c| c.get()) } else { -5 }, 0), BlockPos::new(22, 4, 18)))
}

/// One (c′) run's record.
#[derive(Default)]
struct GateRec {
    /// ARM3's rises (the drive's repeater).
    arm3: Vec<i64>,
    /// the fired pulses at the input cell (rise, first unpowered tick).
    fired: Vec<(i64, i64)>,
    /// the approach detector's first rise; the cart's first tick at rest on the hold (its centre in the
    /// hold cell and moving < 0.001 a tick); the bit's first rise at H2's input.
    det: Option<i64>,
    rest: Option<i64>,
    bit: Option<i64>,
    /// the departure (its centre off the hold cell after resting) and the hold rail's state then.
    off: Option<i64>,
    off_powered: bool,
    /// the latch's Q at the end of the run.
    q_end: bool,
    /// (ironspec 14:41, the ungated control) the cart's first tick in the hold cell and its first tick out
    /// of it after (rest or not).
    entered: Option<i64>,
    left: Option<i64>,
}

/// One (c′) run: the drive at every cycle start (phase 0) for 20 gt over `cycles` cycles; a ridden cart
/// spawned on the exit rail (7,-3,4) at `x0` moving east at `v` at `t_spawn` (None: no cart); the part
/// frozen over `freeze`; with `napper_arm3` the departing cart's contact with ARM3's cell (5,0,7) drives
/// ARM3 for 20 gt (the second rise).
#[allow(clippy::too_many_arguments)]
fn gate_run(extra: i64, live: bool, k: i64, cycles: i64, spawn: Option<(i64, f64, f64)>, freeze: Option<(i64, i64)>, napper_arm3: bool) -> Result<GateRec, String> {
    let c = cells(Geo::Real);
    start_contacts(if GATE_LIFT.with(|c| c.get()) > 0 { ContactGeo::Lift } else { ContactGeo::Real });
    let sp = crate::mirror::checked(&spec_gate(extra, live, k)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let powered = |run: &Run, p: P| run.world.get(bp(p)).to_string().contains("powered=true");
    let lit_dust = |run: &Run, p: P| {
        let s = run.world.get(bp(p)).to_string();
        s.contains("redstone_wire") && !s.contains("power=0")
    };
    let mut rec = GateRec::default();
    let mut id: Option<u32> = None;
    let (mut arm3_on, mut fired_on) = (false, false);
    let mut frozen: Option<[i32; 4]> = None;
    let mut contact: Option<i64> = None;
    let mut last_pos: Option<(f64, f64, f64)> = None;
    for t in 0..(cycles * CYCLE + 40) {
        if let Some((rt, how)) = GATE_RESTART.with(|c| c.get())
            && rt == t
        {
            let remap = crate::liveness::restart(&mut run.world, how)?;
            let renumbered = remap.iter().filter(|(a, b)| a != b).count();
            // (the override's print: the entities the restart carried (liveness::restart maps every live entity's id
            // before the restart to its id after), the first restart of this kind with one in the world (a restart at the
            // summon tick comes before the summon and carries none))
            if !remap.is_empty() {
                note_override(
                    &format!("a restart (every chunk saved and loaded, {})", how.label()),
                    format!("{} live entities before it", remap.len()),
                    format!("every chunk reloaded, {} carried across ({renumbered} with new ids)", remap.len()),
                );
            }
            // (G1's audit hole 14: a cart missing from the remap is an error, never a stale id kept)
            if let Some(i) = id {
                id = Some(*remap.get(&i).ok_or_else(|| format!("the restart before t{t} ({}) lost the cart {i} (no remap entry)", how.label()))?);
            }
        }
        let in_cycle = t.rem_euclid(CYCLE);
        if t / CYCLE < cycles && in_cycle == 0 {
            let before = run.world.get(bp(GATE_DRIVE)).to_string();
            run.world.set_block(bp(GATE_DRIVE), on.clone(), redsim::world::flags::ALL);
            note_override("ARM3's drive (20,-1,13) (the stand-in for ARM3's detector rail, every cycle)", before, run.world.get(bp(GATE_DRIVE)).to_string());
        }
        if in_cycle == 20 && contact.is_none_or(|tc| t >= tc + 20) {
            run.world.set_block(bp(GATE_DRIVE), air.clone(), redsim::world::flags::ALL);
        }
        if let Some(off) = GATE_EXTRA_E.with(|c| c.get())
            && let Some(&(f0, _)) = rec.fired.first()
        {
            if t == f0 + off {
                let before = run.world.get(bp(GATE_DRIVE)).to_string();
                run.world.set_block(bp(GATE_DRIVE), on.clone(), redsim::world::flags::ALL);
                note_override(&format!("E driven again {off} gt after the first fired pulse (GATE_EXTRA_E)"), before, run.world.get(bp(GATE_DRIVE)).to_string());
            }
            if t == f0 + off + 20 {
                run.world.set_block(bp(GATE_DRIVE), air.clone(), redsim::world::flags::ALL);
            }
        }
        if let Some((ts, x0, v)) = spawn
            && t == ts
        {
            let lift = GATE_LIFT.with(|c| c.get());
            // (with the lift: the napper summoned at R heading east at v, as the return-bench and
            // tick-exact's napper-ride do; else on the exit rail at x0)
            let at = if lift > 0 { [4.5, -18.9375 - lift as f64, crate::mirror::zf(4.5)] } else { [x0, -2.9375, crate::mirror::zf(4.5)] };
            id = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", at, Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
        }
        if let Some((f, len)) = freeze {
            if t == f {
                frozen = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == f + len
                && let Some(pb) = frozen.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        run.tick();
        if let Some(i) = id {
            note_contacts(&run, i);
        }
        let a = powered(&run, GATE_ARM3);
        if a && !arm3_on {
            rec.arm3.push(t);
        }
        arm3_on = a;
        edge(&mut rec.fired, &mut fired_on, run.world.best_neighbor_signal(bp([7, -1, 8])) > 0 || powered(&run, [8, -1, 8]), t);
        if rec.det.is_none() && powered(&run, GATE_DET) {
            rec.det = Some(t);
        }
        if rec.bit.is_none() && lit_dust(&run, GATE_BIT) {
            rec.bit = Some(t);
        }
        if let Some(i) = id
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == i && !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
        {
            let (x, y, z) = (e.pos.x, e.pos.y, crate::mirror::zf(e.pos.z));
            let in_hold = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2] && (y - 0.0625).abs() < 0.2;
            let still = last_pos.is_some_and(|(px, _, pz)| (px - x).abs() < 0.001 && (pz - z).abs() < 0.001);
            // (tick-exact's official rest, napper-ride cd070f992: in the hold cell, horizontal speed under
            // 0.005; `still` (moved under 0.001) kept for the late placements' bookkeeping only)
            let hs = (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt();
            let _ = still;
            if in_hold && hs < 0.005 && rec.rest.is_none() {
                rec.rest = Some(t);
            }
            if in_hold && rec.entered.is_none() {
                rec.entered = Some(t);
            }
            if rec.entered.is_some() && !in_hold && rec.left.is_none() {
                rec.left = Some(t);
            }
            if rec.rest.is_some() && !in_hold && rec.off.is_none() {
                rec.off = Some(t);
                rec.off_powered = powered(&run, c.hold);
                DEPARTED.with(|d| d.set(true));
            }
            if napper_arm3 && rec.off.is_some() && contact.is_none() && x - 0.49 < 6.0 && (7.0..8.0).contains(&z) {
                contact = Some(t);
                run.world.set_block(bp(GATE_DRIVE), on.clone(), redsim::world::flags::ALL);
            }
            if let Some(tc) = contact
                && t == tc + 20
            {
                run.world.set_block(bp(GATE_DRIVE), air.clone(), redsim::world::flags::ALL);
            }
            last_pos = Some((x, y, z));
        }
    }
    rec.q_end = lit_dust(&run, GATE_Q);
    Ok(rec)
}

/// (the chain fix, G1 / ironspec: the controls run the rows' own judgement) One arrival run's misses by the arrival
/// rows' criteria: the detector -> H2 delay equal to k_lock, the rest and the bit present with the bit >= 2 gt after
/// the rest, exactly one fired pulse after the detector, ARM3 measured before it, the departure on the powered rail,
/// the latch not left set.
fn arrival_misses(r: &GateRec, k: i64) -> Vec<String> {
    let mut m = Vec::new();
    match (r.det, r.bit) {
        (Some(d), Some(b)) if b - d != k => m.push(format!("detector -> H2 {} gt, not k_lock {k}", b - d)),
        (None, _) => m.push("no detector rise".into()),
        _ => {}
    }
    match (r.rest, r.bit) {
        (Some(rest), Some(bit)) if bit - rest < 2 => m.push(format!("the bit {} gt after rest (bar >= 2)", bit - rest)),
        (Some(_), Some(_)) => {}
        (rest, bit) => m.push(format!("rest {rest:?}, bit {bit:?}")),
    }
    let fires: Vec<i64> = r.fired.iter().filter(|&&(f, _)| r.det.is_some_and(|d| f > d)).map(|x| x.0).collect();
    if fires.len() != 1 {
        m.push(format!("{} fired pulses", fires.len()));
    }
    if let Some(&f) = fires.first() {
        if r.arm3.iter().all(|&a| a > f) {
            m.push(format!("no ARM3 rise before the fired pulse t{f}"));
        }
        match r.off {
            Some(_) if !r.off_powered => m.push("the departure late (the rail unpowered)".into()),
            Some(_) => {}
            None => m.push(format!("fired at t{f} and never left the hold")),
        }
    }
    if r.q_end {
        m.push("the latch left set".into());
    }
    m
}

/// (the chain fix) The E-within-k measure of one run, shared by the row and its control: (the fired pulses, the E
/// rises within k_lock after one, the least gap from a fired pulse to the next E rise).
fn e_within(r: &GateRec, k: i64) -> (u32, u32, i64) {
    let (mut fires, mut within, mut least) = (0u32, 0u32, i64::MAX);
    for &(f, _) in &r.fired {
        fires += 1;
        for &a in &r.arm3 {
            if a > f {
                least = least.min(a - f);
                if a - f <= k {
                    within += 1;
                }
            }
        }
    }
    (fires, within, least)
}

/// The (c′) bench: arrivals at every phase of the detector's contact relative to E's rise (the spawn
/// time stepped by 1 gt over a cycle) at three exit speeds; per run the bit's arrival at H2 against the
/// cart's rest, the fired pulses (exactly one per rest), the departure on the fired pulse (relative to
/// E's rise, the brief's criterion), the napper's own second ARM3 rise firing nothing; a 3-gt freeze at
/// every offset around the fired pulse; the control with the live term removed (the phantom pulse). (The chain
/// fix) every bar and control through `v`; the controls run the rows' own judgement.
pub fn gate_bench(extra: i64, k: i64, freeze: i64, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    if freeze < 1 {
        return Err(format!("--freeze {freeze}: at least 1 gt (0 thaws in the tick it freezes, so the freeze rows would test nothing)"));
    }
    if extra < 1 {
        return Err(format!("--extra {extra}: the gate's E side is a delay-extra repeater, at least 1 (G1's audit hole 8: never silently raised)"));
    }
    clear_contacts();
    let mut lines = Vec::new();
    // (the pulse and the delay, from a run with one arrival)
    let base = gate_run(extra, true, k, 4, Some((5, 7.3, 0.3)), None, true)?;
    let delay = match (base.det, base.bit) {
        (Some(d), Some(b)) => (b - d).to_string(),
        _ => "-".into(),
    };
    push_row(&mut lines, format!(
        "(c′) the presence-gated hold release as blocks (ironspec 11:43 / 11:49: the approach detector (7,-3,4) -> a loop latch -> the settle delay -> P = delayed AND live -> H2 sampled at E + 2 (L2) -> the AND (7,-1,9) into the input cell; the fired pulse resets the latch) in the Real geometry with the T's north arm: one arrival's detector-to-H2 delay {delay} gt (k_lock {k}, the (c′) settle lock: ironspec 14:36, k_lock = R_max + F + 2 at even widths, a repeater delay; ironspec 22:03 PT's names: k_lock, k_sw, K(x)); every run's spec checked: 0 cells outside its world's box"
    ));
    lines.push(tsnap_line(v)?);
    let (mut n, mut margin_min, mut margin_max) = (0u32, i64::MAX, i64::MIN);
    let (mut one_fire, mut late, mut q_left) = (0u32, 0u32, 0u32);
    let mut dep_e = (i64::MAX, i64::MIN);
    let mut dly = (i64::MAX, i64::MIN);
    let mut e_to_fire = (i64::MAX, i64::MIN);
    let mut bad = Vec::new();
    for sp in [0.1, 0.25, 0.4] {
        for ph in 0..CYCLE {
            let ts = 110 + ph;
            let r = gate_run(extra, true, k, 5, Some((ts, 7.3, sp)), None, true)?;
            n += 1;
            for miss in arrival_misses(&r, k) {
                bad.push(format!("v {sp} spawn {ts}: {miss}"));
            }
            if let (Some(d), Some(b)) = (r.det, r.bit) {
                dly = (dly.0.min(b - d), dly.1.max(b - d));
            }
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                margin_min = margin_min.min(bit - rest);
                margin_max = margin_max.max(bit - rest);
            }
            let fires: Vec<i64> = r.fired.iter().filter(|&&(f, _)| r.det.is_some_and(|d| f > d)).map(|x| x.0).collect();
            one_fire += u32::from(fires.len() == 1);
            if let Some(&f) = fires.first()
                && let Some(e_rise) = r.arm3.iter().copied().filter(|&a| a <= f).max()
            {
                // (G1 on 7ccd7737c point 4: E's rise measured, the last ARM3 rise at or before the fired pulse)
                e_to_fire = (e_to_fire.0.min(f - e_rise), e_to_fire.1.max(f - e_rise));
                if let Some(o) = r.off {
                    dep_e = (dep_e.0.min(o - e_rise), dep_e.1.max(o - e_rise));
                    late += u32::from(!r.off_powered);
                }
            }
            q_left += u32::from(r.q_end);
        }
    }
    let span = |a: (i64, i64)| if a.0 == i64::MAX { "-".to_string() } else if a.0 == a.1 { a.0.to_string() } else { format!("{}..{}", a.0, a.1) };
    let w_arr = v.bar_n("(c′) arrivals: every run meets the arrival criteria (k_lock, rest and bit, the bit >= 2 after rest, one fired pulse, ARM3 before it, the departure on the powered rail, the latch cleared)", n as usize, bad.is_empty());
    push_row(&mut lines, format!(
        "  arrivals (a ridden cart from the exit rail at v 0.1 / 0.25 / 0.4, spawned at every gt of a cycle: {n} runs, the napper's own second ARM3 rise driven on its departure): each run's detector -> H2 delay {} gt against k_lock {k}; the bit at H2 after the cart's rest by {}..{} gt (ironspec: >= 2); exactly one fired pulse {one_fire} of {n}; E (the ARM3 rise, measured) -> the fired pulse {} gt; the departure (its centre off the hold cell) at E's measured rise + {} gt, late (the rail unpowered) {late}; the latch left set {q_left}: {w_arr}{}",
        span(dly),
        if margin_min == i64::MAX { "-".to_string() } else { margin_min.to_string() },
        if margin_max == i64::MIN { "-".to_string() } else { margin_max.to_string() },
        span(e_to_fire),
        span(dep_e),
        if bad.is_empty() { String::new() } else { format!(" (the misses, {}: {})", bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    // a freeze at every offset around the fired pulse (a cart that rested)
    let r0 = gate_run(extra, true, k, 5, Some((110, 7.3, 0.3)), None, false)?;
    if let Some(&(f0, _)) = r0.fired.iter().find(|&&(f, _)| r0.det.is_some_and(|d| f > d)) {
        let (mut fl, mut fok, mut fmiss, mut runs) = (0u32, 0u32, 0u32, 0usize);
        let mut fdep = (i64::MAX, i64::MIN);
        for off in -5..=16 {
            let r = gate_run(extra, true, k, 5, Some((110, 7.3, 0.3)), Some((f0 + off, freeze)), false)?;
            runs += 1;
            match (r.off, r.fired.iter().find(|&&(f, _)| r.det.is_some_and(|d| f > d))) {
                (Some(o), Some(&(f, _))) if r.off_powered => {
                    // (E measured: the last ARM3 rise at or before the fired pulse)
                    match r.arm3.iter().copied().filter(|&a| a <= f).max() {
                        Some(e) => {
                            fok += 1;
                            fdep = (fdep.0.min(o - e), fdep.1.max(o - e));
                        }
                        None => fmiss += 1,
                    }
                }
                (Some(_), Some(_)) => fl += 1,
                _ => fmiss += 1,
            }
        }
        let w = v.bar_n(&format!("(c′) a {freeze}-gt freeze around the fired pulse: every run launched on it"), runs, fl == 0 && fmiss == 0);
        push_row(&mut lines, format!(
            "  a {freeze}-gt freeze at every offset from the fired pulse's rise - 5 to + 16 (a rested napper): launched on the fired pulse {fok}, late {fl}, missed {fmiss} ({w}); the departure at E's measured rise + {} gt",
            span(fdep)
        ));
    } else {
        // (G1 on 7c330044b / c22def814: the row never vanishes; the rest of the bench still runs)
        v.fail("(c′) the freeze sweep around the fired pulse: NOT JUDGED (the reference arrival fired nothing)");
        lines.push("  the freeze sweep around the fired pulse: FAIL, the reference arrival fired nothing".into());
    }
    // (G1's note 4 on 9c30d6dc3) a freeze during the approach (redstone running, the cart frozen: ironspec's
    // + 3 in 38): at every offset from the detector's rise - 3 to the cart's rest, at the slowest exit speed
    let r1 = gate_run(extra, true, k, 5, Some((110, 7.3, 0.1)), None, false)?;
    if let (Some(d1), Some(rs1)) = (r1.det, r1.rest) {
        let (mut ok, mut bad_n, mut least, mut most, mut runs) = (0u32, Vec::new(), i64::MAX, i64::MIN, 0usize);
        for off in -3..=(rs1 - d1) {
            let r = gate_run(extra, true, k, 5, Some((110, 7.3, 0.1)), Some((d1 + off, freeze)), false)?;
            runs += 1;
            let fires = r.fired.iter().filter(|&&(f, _)| r.det.is_some_and(|d| f > d)).count();
            if let (Some(rest), Some(bit)) = (r.rest, r.bit) {
                least = least.min(bit - rest);
                most = most.max(bit - rest);
            } else {
                // (G1 on 652881d4f / c22def814: a run without a detector rise, a rest or a bit is a failure, never a drop)
                bad_n.push(format!("offset {off}: det {:?}, rest {:?}, bit {:?}", r.det, r.rest, r.bit));
            }
            if fires == 1 && r.off.is_some() && r.off_powered {
                ok += 1;
            } else {
                bad_n.push(format!("offset {off}: {fires} fired, departure {:?}, rail powered then {}", r.off, r.off_powered));
            }
        }
        let w = v.bar_n(&format!("(c′) a {freeze}-gt freeze during the approach: one fired launch each, the bit at or after rest (the functional bar 0)"), runs, bad_n.is_empty() && least != i64::MAX && least >= 0);
        push_row(&mut lines, format!(
            "  a {freeze}-gt freeze during the approach (G1's note 4: redstone running, the cart frozen; v 0.1, the slowest exit) at every offset from the detector's rise - 3 to the cart's rest (+{}): launched on one fired pulse {ok}, otherwise {}{}; the bit at H2 after the cart's rest by {}..{} gt (the functional bar 0: presence sampled at rest; the design margin k - approach max - 3): {w}",
            rs1 - d1,
            bad_n.len(),
            if bad_n.is_empty() { String::new() } else { format!(" ({})", bad_n.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) },
            if least == i64::MAX { "-".to_string() } else { least.to_string() },
            if most == i64::MIN { "-".to_string() } else { most.to_string() }
        ));
        // (ironspec 14:15) a restart at every tick of the approach (the detector's rise - 3 .. the rest), in
        // every reload order (tick-exact's top-exit restart case: detector -> rest 36)
        let (mut rok, mut rbad, mut rleast, mut n) = (0u32, Vec::new(), i64::MAX, 0u32);
        let mut dr = (i64::MAX, i64::MIN);
        for rt in (d1 - 3)..=rs1 {
            for how in crate::liveness::Restart::rows() {
                GATE_RESTART.with(|c| c.set(Some((rt, how))));
                let r = gate_run(extra, true, k, 5, Some((110, 7.3, 0.1)), None, false);
                GATE_RESTART.with(|c| c.set(None));
                let r = r?;
                n += 1;
                let fires = r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
                if let (Some(rest), Some(bit), Some(d)) = (r.rest, r.bit, r.det) {
                    rleast = rleast.min(bit - rest);
                    dr = (dr.0.min(rest - d), dr.1.max(rest - d));
                } else {
                    rbad.push(format!("restart before t{} ({}): det {:?}, rest {:?}, bit {:?}", rt, how.label(), r.det, r.rest, r.bit));
                }
                if fires == 1 && r.off.is_some() && r.off_powered {
                    rok += 1;
                } else {
                    rbad.push(format!("restart before t{} ({}): {fires} fired, departure {:?}", rt, how.label(), r.off));
                }
            }
        }
        let w = v.bar_n("(c′) a restart at every tick of the approach: one fired launch each, the bit at or after rest", n as usize, rbad.is_empty() && rleast != i64::MAX && rleast >= 0);
        push_row(&mut lines, format!(
            "  a restart (every chunk saved and loaded) at every tick of the approach (detector - 3 .. rest) in every reload order ({n} runs, v 0.1): detector -> rest {} gt (the run without a restart: {}); launched on one fired pulse {rok}, otherwise {}{}; the bit at H2 after the cart's rest at least {} gt (the functional bar 0): {w}",
            span(dr),
            rs1 - d1,
            rbad.len(),
            if rbad.is_empty() { String::new() } else { format!(" ({})", rbad.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) },
            if rleast == i64::MAX { "-".to_string() } else { rleast.to_string() }
        ));
    } else {
        // (G1 on c22def814: the approach freeze and restart rows never vanish silently)
        v.fail("(c′) the approach freeze and restart rows: NOT JUDGED (the reference run has no detector rise or rest)");
        push_row(&mut lines, format!(
            "  the approach freeze and restart rows: FAIL, the reference run (v 0.1) has no detector rise or rest (det {:?}, rest {:?})",
            r1.det, r1.rest
        ));
    }
    // the control: the live term removed (P = the delayed bit alone): the napper's second ARM3 rise
    let ctl = gate_run(extra, false, k, 5, Some((110, 7.3, 0.3)), None, true)?;
    let ctl_fires = ctl.fired.iter().filter(|&&(f, _)| ctl.det.is_some_and(|d| f > d)).count();
    let live = gate_run(extra, true, k, 5, Some((110, 7.3, 0.3)), None, true)?;
    let live_fires = live.fired.iter().filter(|&&(f, _)| live.det.is_some_and(|d| f > d)).count();
    let w_live = v.bar("(c′) one fired pulse for one rest with the live term", live_fires == 1);
    let w_ctl = v.control("(c′) without the live term the napper's second ARM3 rise fires the phantom", 1, ctl_fires > 1);
    push_row(&mut lines, format!("  the napper's own second ARM3 rise: with the live term {live_fires} fired pulse(s) for one rest ({w_live}); the control without it {ctl_fires} ({w_ctl})"));
    // (G1 on ed3a09628, ironspec 14:36) the lift laid: the arrivals from R, and a restart at every tick of
    // the ride, alone and with a login's freeze at the restart
    GATE_LIFT.with(|c| c.set(9));
    let lift = gate_lift_rows(extra, k, v);
    GATE_LIFT.with(|c| c.set(0));
    lines.extend(lift?);
    // (ironspec 14:41/14:42) the positive control: the ungated pulse (the AND passes E_d every cycle) must fail
    // the arrival rows' own judgement (`arrival_misses`, the same function the arrivals row runs) in every run; the
    // gate's own kind of failure (a pulse onto the empty hold while the napper is on its way, a launch before rest)
    // printed beside it. (Its contacts are part of the planted defect, a cart launched before its rest: printed with
    // the control, kept out of the rows' contact bar.)
    let rows_contact_fails = CONTACT_FAILS.with(|f| std::mem::take(&mut *f.borrow_mut()));
    GATE_UNGATED.with(|c| c.set(true));
    let ung = (|| -> Result<(u32, u32, u32, u32, u32, Vec<i64>), String> {
        let (mut runs, mut on_way, mut before, mut bite, mut judged) = (0u32, 0u32, 0u32, 0u32, 0u32);
        let mut quiet = Vec::new();
        for &sp in &[0.1, 0.25, 0.4] {
            for ph in 0..CYCLE {
                let r = gate_run(extra, true, k, 5, Some((110 + ph, 7.3, sp)), None, false)?;
                runs += 1;
                let w = r.fired.iter().filter(|&&(f, _)| r.det.is_some_and(|d| f > d) && r.rest.is_none_or(|rs| f < rs)).count() as u32;
                let b = u32::from(r.left.is_some() && r.rest.is_none_or(|rs| r.left.is_some_and(|l| l <= rs)));
                on_way += w;
                before += b;
                bite += u32::from(w > 0 || b > 0);
                if w == 0 && b == 0 {
                    quiet.push(ph);
                }
                judged += u32::from(!arrival_misses(&r, k).is_empty());
            }
        }
        Ok((runs, on_way, before, bite, judged, quiet))
    })();
    GATE_UNGATED.with(|c| c.set(false));
    let (runs, on_way, before, bite, judged, mut quiet) = ung?;
    quiet.sort();
    quiet.dedup();
    let w_ung = v.control("(c′) the ungated pulse fails the arrival criteria in every run", runs as usize, judged == runs);
    // (the control's row annotated with its own contacts first, then its undeclared ones taken out of the rows' contact
    // bar: the rows' failures restored as they were before the control ran)
    push_row(&mut lines, format!(
        "  the positive control (ironspec 14:41: an ungated pulse, the AND's NOT H2 left out), over the {runs} arrivals: failures of the gate's own kind after the approach: pulses onto the empty hold while the napper is on its way {on_way}, launches before rest {before}; runs bitten that way {bite} of {runs}{}; runs failing the arrival rows' own judgement (the same function) {judged} of {runs} ({w_ung})",
        if quiet.is_empty() { String::new() } else { format!(" (arrival phases where it does not bite that way: {})", quiet.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",")) }
    ));
    let ctl_contacts = CONTACT_FAILS.with(|f| std::mem::replace(&mut *f.borrow_mut(), rows_contact_fails));
    lines.push(format!(
        "  the positive control's undeclared contacts (its planted defect's: a cart launched before its rest; not judged in the rows' contact bar): {}",
        if ctl_contacts.is_empty() { "none".to_string() } else { ctl_contacts.join("; ") }
    ));
    let (mut within, mut least, mut fires) = (0u32, i64::MAX, 0u32);
    for &sp in &[0.1, 0.25, 0.4] {
        for ph in 0..CYCLE {
            let r = gate_run(extra, true, k, 5, Some((110 + ph, 7.3, sp)), None, false)?;
            let (f, w, l) = e_within(&r, k);
            fires += f;
            within += w;
            least = least.min(l);
        }
    }
    // (G1's audit hole 11: a verdict word, and no fired pulse is NOT JUDGED)
    let w_e = v.bar_n(&format!("(c′) no E rise within k_lock {k} after a fired pulse"), fires as usize, within == 0);
    push_row(&mut lines, format!(
        "  E rises within k_lock {k} gt after a fired pulse over the 300 arrivals ({fires} fired pulses): {within} (the permanent bar 0: {w_e}; the least gap from a fired pulse to the next E rise {} gt; the bench drives E once per 100-gt cycle: the release chain's own interval under restarts and logins is the controller's row)",
        if least == i64::MAX { "-".to_string() } else { least.to_string() }
    ));
    // (G1 on 652881d4f point 4) the row's control: E driven again 20 gt after the first fired pulse (inside k), counted
    // by the row's own measure (`e_within`)
    GATE_EXTRA_E.with(|c| c.set(Some(20)));
    let ctl = gate_run(extra, true, k, 5, Some((110, 7.3, 0.25)), None, false);
    GATE_EXTRA_E.with(|c| c.set(None));
    let ctl = ctl?;
    let (cf, ctl_within, _) = e_within(&ctl, k);
    let w_ec = v.control("(c′) E driven again 20 gt after the first fired pulse is counted within k_lock", cf as usize, ctl_within > 0);
    push_row(&mut lines, format!("  the E-within-k control (E driven again 20 gt after the first fired pulse): counted {ctl_within} ({w_ec})"));
    lines.push(format!("  {}", judge_contacts(v)));
    Ok(lines)
}

/// (G1 on ed3a09628; ironspec 14:36: k = R_max + F + 2) The lift-top rows with the napper lift laid (the
/// stop 9 down, `GATE_LIFT`): the napper summoned at R east at 0.2 at every gt of a cycle; then a restart
/// (every chunk saved and loaded) before every tick from the summon to the rest, in every reload order,
/// alone and with a 1..3-gt freeze starting at the restart (a login coinciding with it); detector -> rest,
/// the bit at H2 after the cart's rest, one fired pulse, the departure on the powered rail.
fn gate_lift_rows(extra: i64, k: i64, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    let depth = GATE_LIFT.with(|c| c.get());
    let nf = |r: &GateRec| r.fired.iter().filter(|&&(x, _)| r.det.is_some_and(|d| x > d)).count();
    let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
    let ts = 110;
    let r0 = gate_run(extra, true, k, 5, Some((ts, 0.0, 0.2)), None, false)?;
    let (Some(d0), Some(rs0)) = (r0.det, r0.rest) else {
        v.fail("(c′) the lift-top rows: NOT JUDGED (the reference ride from R did not come to rest on the hold)");
        return Ok(vec![format!("  the lift-top rows (the napper lift laid, the stop {depth} down): the reference ride from R did not come to rest on the hold (detector {:?}, rest {:?}): FAIL", r0.det, r0.rest)]);
    };
    let mut lines = vec![format!(
        "  (G1 on ed3a09628; ironspec 14:36) the napper lift laid (the stop {depth} down: R (4,{},4) and its entry on glass, the soul sand under the bubble column (6,{}..-4,4), the top's water source (5,-3,4); 7b90c3f1's return-bench cells), the napper summoned at R east at 0.2: detector -> rest {} gt with no restart (tick-exact's official ride: 32); the bit at H2 after rest {} gt; fired {}",
        -19 - depth,
        -19 - depth,
        rs0 - d0,
        r0.bit.map(|b| (b - rs0).to_string()).unwrap_or_else(|| "-".into()),
        nf(&r0)
    )];
    // every summon phase across a cycle
    let (mut one, mut late, mut dr, mut br) = (0u32, 0u32, (i64::MAX, i64::MIN), (i64::MAX, i64::MIN));
    // (G1 on 652881d4f point 4: a run without a detector rise, a rest or a bit is a failure, never a drop)
    let mut missing = 0u32;
    for ph in 0..CYCLE {
        let r = gate_run(extra, true, k, 5, Some((ts + ph, 0.0, 0.2)), None, false)?;
        if let (Some(d), Some(rs), Some(b)) = (r.det, r.rest, r.bit) {
            dr = (dr.0.min(rs - d), dr.1.max(rs - d));
            br = (br.0.min(b - rs), br.1.max(b - rs));
        } else {
            missing += 1;
        }
        one += u32::from(nf(&r) == 1);
        late += u32::from(r.off.is_some() && !r.off_powered);
        missing += u32::from(r.off.is_none());
    }
    let w = v.bar_n("(c′) the lift's arrivals at every summon phase: one fired pulse, none late, none missing data", CYCLE as usize, missing == 0 && one == CYCLE as u32 && late == 0);
    push_row(&mut lines, format!(
        "  the lift's arrivals at every summon phase of a cycle ({CYCLE} runs): detector -> rest {}..{} gt; the bit at H2 after rest {}..{} gt; exactly one fired pulse {one}; late {late}; runs missing a detector rise, a rest, a bit or a departure {missing} ({w})",
        m(dr.0, i64::MAX),
        m(dr.1, i64::MIN),
        m(br.0, i64::MAX),
        m(br.1, i64::MIN)
    ));
    // a restart at every tick of the ride, alone and with a login's 1..3-gt freeze at it
    for f in [0i64, 1, 2, 3] {
        let (mut n, mut one, mut dr, mut least, mut worst) = (0u32, 0u32, (i64::MAX, i64::MIN), i64::MAX, String::new());
        let mut missing = 0u32;
        for rt in ts..=(rs0 + 1) {
            for how in crate::liveness::Restart::rows() {
                GATE_RESTART.with(|c| c.set(Some((rt, how))));
                let r = gate_run(extra, true, k, 5, Some((ts, 0.0, 0.2)), if f > 0 { Some((rt, f)) } else { None }, false);
                GATE_RESTART.with(|c| c.set(None));
                let r = r?;
                n += 1;
                if let (Some(d), Some(rs), Some(b)) = (r.det, r.rest, r.bit) {
                    if rs - d > dr.1 {
                        worst = format!("restart before t{rt} ({} after the summon), {}", rt - ts, how.label());
                    }
                    dr = (dr.0.min(rs - d), dr.1.max(rs - d));
                    least = least.min(b - rs);
                } else {
                    missing += 1;
                }
                one += u32::from(nf(&r) == 1 && r.off.is_some() && r.off_powered);
            }
        }
        let w = v.bar_n(&format!("(c′) the lift's restart rows{}: one fired launch each, no missing data, the bit >= 2 gt after rest", if f > 0 { format!(" with a {f}-gt freeze") } else { String::new() }), n as usize, missing == 0 && one == n && least != i64::MAX && least >= 2);
        push_row(&mut lines, format!(
            "  a restart before every tick from the summon to the rest, every reload order{} ({n} runs): detector -> rest {}..{} gt (redsim's worst {}, at {worst}; tick-exact's official: 36 with a restart at the lift's top; rest as tick-exact's: in the hold cell under 0.005 b/gt); launched on one fired pulse {one} of {n}; runs missing a detector rise, a rest or a bit {missing}; the bit at H2 after rest at least {} gt (k_lock {k} - the measured worst = {}) ({w})",
            if f > 0 { format!(", with a {f}-gt login freeze starting at the restart") } else { String::new() },
            m(dr.0, i64::MAX),
            m(dr.1, i64::MIN),
            m(dr.1, i64::MIN),
            m(least, i64::MAX),
            if dr.1 == i64::MIN { "-".to_string() } else { (k - dr.1).to_string() }
        ));
    }
    // (ironspec 15:23 (1)) a restart and a later login stacking: a restart before each tick from the summon
    // to the rise (the save order) x a 1..3-gt freeze starting at every tick from the rise to the rest
    let save = crate::liveness::Restart::rows().into_iter().next().ok_or("no restart order")?;
    let (mut n, mut one, mut dr, mut least, mut worst) = (0u32, 0u32, (i64::MAX, i64::MIN), i64::MAX, String::new());
    let mut missing = 0u32;
    for rt in ts..d0 {
        for f in 1..=3i64 {
            for ft in d0..=rs0 {
                GATE_RESTART.with(|c| c.set(Some((rt, save))));
                let r = gate_run(extra, true, k, 5, Some((ts, 0.0, 0.2)), Some((ft, f)), false);
                GATE_RESTART.with(|c| c.set(None));
                let r = r?;
                n += 1;
                if let (Some(d), Some(rs), Some(b)) = (r.det, r.rest, r.bit) {
                    if rs - d > dr.1 {
                        worst = format!("restart before t{rt} ({} after the summon), a {f}-gt freeze from t{ft}", rt - ts);
                    }
                    dr = (dr.0.min(rs - d), dr.1.max(rs - d));
                    least = least.min(b - rs);
                } else {
                    missing += 1;
                }
                one += u32::from(nf(&r) == 1 && r.off.is_some() && r.off_powered);
            }
        }
    }
    let w = v.bar_n("(c′) the restart x later-freeze rows: one fired launch each, no missing data, the bit >= 2 gt after rest", n as usize, missing == 0 && one == n && least != i64::MAX && least >= 2);
    push_row(&mut lines, format!(
        "  (ironspec 15:23 (1)) a restart before each tick from the summon to the rise (the save order) x a 1..3-gt login freeze starting at every tick from the rise to the rest ({n} runs): detector -> rest {}..{} gt (the worst at {worst}); launched on one fired pulse {one} of {n}; runs missing a detector rise, a rest or a bit {missing}; the bit at H2 after rest at least {} gt (bar >= 2: {w})",
        m(dr.0, i64::MAX),
        m(dr.1, i64::MIN),
        m(least, i64::MAX)
    ));
    Ok(lines)
}

/// One run's record: ARM3's rises, the controller output's and the rail's pulses (rise, first unpowered
/// tick), the cart's departure from the hold cell, the string's rises, whether (Real) the cart reached
/// the merge row west of lane 7's T.
#[derive(Default)]
struct Rec {
    arm3: Vec<i64>,
    out: Vec<(i64, i64)>,
    rail: Vec<(i64, i64)>,
    left: Option<i64>,
    trip: Vec<i64>,
    merged: bool,
    /// (ironspec 11:05 (2)) the departing napper's contact with ARM3's cell (5,0,7) (Real, emulated).
    arm3_contact: Option<i64>,
    /// the ticks a cart's centre was on the hold cell.
    hold_busy: Vec<i64>,
    /// the departures from the hold (a cart leaving the hold cell).
    departures: u32,
    /// (G1 on 6ca670551, the brief's criterion) the first tick the cart's centre is off the hold cell
    /// after it was on it, and whether the hold rail was powered at that tick (unpowered: late).
    off_rail: Option<i64>,
    off_powered: bool,
}

fn edge(v: &mut Vec<(i64, i64)>, was: &mut bool, now: bool, t: i64) {
    if now && !*was {
        v.push((t, i64::MAX));
    }
    if !now && *was
        && let Some(last) = v.last_mut()
    {
        last.1 = t;
    }
    *was = now;
}

/// One run: the drive block placed at every cycle start (t = phase + k * 100, k < `cycles`) for 20 gt; a
/// ridden cart placed at rest on the hold rail at t = `place` (None: no cart); the part frozen at level 32
/// over `freeze` = (from, gt) (redstone running, entities frozen); with `napper_arm3` (Real) the departing
/// cart's contact with ARM3's cell (5,0,7) powers the drive for 20 gt, as the detector rail there would.
/// States are read after each tick.
fn run_once(g: Geo, extra: i64, phase: i64, cycles: i64, place: Option<i64>, freeze: Option<(i64, i64)>) -> Result<Rec, String> {
    run_full(g, extra, phase, cycles, place, freeze, false)
}

fn run_full(g: Geo, extra: i64, phase: i64, cycles: i64, place: Option<i64>, freeze: Option<(i64, i64)>, napper_arm3: bool) -> Result<Rec, String> {
    let c = cells(g);
    start_contacts(if g == Geo::Line { ContactGeo::Line } else { ContactGeo::Real });
    let sp = crate::mirror::checked(&spec(g, extra)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let powered = |run: &Run, p: P| run.world.get(bp(p)).to_string().contains("powered=true");
    let mut rec = Rec::default();
    let mut cart: Option<u32> = None;
    let (mut arm3_on, mut out_on, mut rail_on, mut trip_on) = (false, false, false, false);
    let mut frozen: Option<[i32; 4]> = None;
    let horizon = phase + cycles * CYCLE + 40;
    let direct = if g == Geo::Real { SPLIT_DIRECT.with(|c| c.get()) } else { None };
    let lever_on = Block::parse("minecraft:lever[face=floor,facing=north,powered=true]").map_err(|e| e.to_string())?;
    let lever_off = Block::parse("minecraft:lever[face=floor,facing=north,powered=false]").map_err(|e| e.to_string())?;
    for t in 0..horizon {
        let in_cycle = (t - phase).rem_euclid(CYCLE);
        if t >= phase && (t - phase) / CYCLE < cycles && in_cycle == 0 {
            let before = run.world.get(bp(c.drive)).to_string();
            run.world.set_block(bp(c.drive), on.clone(), redsim::world::flags::ALL);
            note_override(&format!("ARM3's drive cell ({},{},{}) (the stand-in for ARM3's detector rail, every cycle)", c.drive[0], c.drive[1], c.drive[2]), before, run.world.get(bp(c.drive)).to_string());
        }
        if t >= phase && in_cycle == 20 {
            run.world.set_block(bp(c.drive), air.clone(), redsim::world::flags::ALL);
        }
        // (the direct drive: the lever on at the cycle's in_cycle D (ARM3 rises 2 gt after the drive and the
        // chain's input cell at ARM3 + D - 2), off w gt later)
        if let Some(w) = direct
            && t >= phase
            && (t - phase) / CYCLE < cycles
        {
            if in_cycle == D {
                let before = run.world.get(bp([7, -1, 8])).to_string();
                run.world.set_block(bp([7, -1, 8]), lever_on.clone(), redsim::world::flags::ALL);
                note_override(&format!("the input cell (7,-1,8) driven directly (SPLIT_DIRECT {w} gt: the floor lever on)"), before, run.world.get(bp([7, -1, 8])).to_string());
            }
            if in_cycle == D + w {
                run.world.set_block(bp([7, -1, 8]), lever_off.clone(), redsim::world::flags::ALL);
            }
        }
        if place == Some(t) {
            cart = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [c.place[0], c.place[1], crate::mirror::zf(c.place[2])], Some("{Motion:[0.0d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?);
        }
        if let Some((f, len)) = freeze {
            if t == f {
                frozen = Some(crate::liveness::freeze32(&mut run.world));
            }
            if t == f + len
                && let Some(pb) = frozen.take()
            {
                crate::liveness::thaw32(&mut run.world, pb);
            }
        }
        run.tick();
        if let Some(i) = cart {
            note_contacts(&run, i);
        }
        let a = powered(&run, c.arm3);
        if a && !arm3_on {
            rec.arm3.push(t);
        }
        arm3_on = a;
        edge(&mut rec.out, &mut out_on, powered(&run, c.out), t);
        edge(&mut rec.rail, &mut rail_on, powered(&run, c.hold), t);
        let tr = powered(&run, c.hook);
        if tr && !trip_on {
            rec.trip.push(t);
        }
        trip_on = tr;
        if let Some(id) = cart
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed && matches!(e.kind, EntityKind::Minecart(_)))
        {
            // (the cart's position in the design's frame)
            let (x, z) = (e.pos.x, crate::mirror::zf(e.pos.z));
            let gone = match g {
                Geo::Line => x - 0.49 >= c.hold[0] as f64 + 1.0,
                Geo::Real => z + 0.49 < c.hold[2] as f64,
            };
            if gone && rec.left.is_none() {
                rec.left = Some(t);
                rec.departures += 1;
            }
            let in_hold = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2];
            if in_hold {
                rec.hold_busy.push(t);
            } else if !rec.hold_busy.is_empty() && rec.off_rail.is_none() {
                rec.off_rail = Some(t);
                rec.off_powered = rail_on;
                DEPARTED.with(|d| d.set(true));
            }
            if g == Geo::Real && x <= 4.5 && (7.0..8.0).contains(&z) {
                rec.merged = true;
            }
            // (the napper's own ARM3 rise: its box reaches the detector rail's cell (5,0,7))
            if napper_arm3 && g == Geo::Real && rec.left.is_some() && rec.arm3_contact.is_none() && x - 0.49 < 6.0 && (7.0..8.0).contains(&z) {
                rec.arm3_contact = Some(t);
                run.world.set_block(bp(c.drive), on.clone(), redsim::world::flags::ALL);
            }
        }
        if let Some(tc) = rec.arm3_contact
            && t == tc + 20
        {
            run.world.set_block(bp(c.drive), air.clone(), redsim::world::flags::ALL);
        }
    }
    Ok(rec)
}

type Hist = std::collections::BTreeMap<i64, u32>;

fn row(m: &Hist) -> String {
    m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")
}

/// (G1's audit hole 7) The departure string's press credited to a departure at `o`: the first string edge after the
/// cart's last tick at rest (`rest_end`, its last tick on the hold before the rail's rise) and within 20 gt after the
/// departure (the rider's box enters the string's cell before the cart's box clears the hold cell); a press while
/// the cart rests, or before it, is never credited.
fn departure_press(trip: &[i64], rest_end: Option<i64>, o: i64) -> Option<i64> {
    let re = rest_end?;
    trip.iter().copied().find(|&x| x > re && x <= o + 20)
}

/// One freeze sweep's results.
struct FreezeOut {
    ok: u32,
    late: u32,
    miss: u32,
    dep: Hist,
    spare: i64,
    trip: Hist,
    bad: Vec<String>,
    /// the reference run's measured rail pulse: its width and its rise after ARM3 (G1's audit hole 9)
    width: i64,
    rise_after_arm3: i64,
    runs: usize,
}

/// The freeze sweep for one width (`extra`): a cart at rest on the hold from before the pulse, the part
/// frozen `len` gt from every offset (relative to the rail's rise) -3..width+1; per offset, by the brief's
/// criterion (G1 on 6ca670551): launched (its centre off the hold cell while the hold rail is still
/// powered, with the departure string's press credited by `departure_press`), late (off the cell with the rail
/// unpowered at that tick: a braked roll-off or the slow creep) or missed (not off on this cycle).
fn freeze_sweep(g: Geo, extra: i64, len: i64) -> Result<FreezeOut, String> {
    let reference = run_once(g, extra, 0, 3, None, None)?;
    let (Some(&(rise, fall)), Some(&arm3)) = (reference.rail.get(1), reference.arm3.get(1)) else {
        return Err("the reference run saw fewer than two pulses".into());
    };
    let width = fall - rise;
    let (mut ok, mut late, mut miss) = (0u32, 0u32, 0u32);
    let mut spare = i64::MAX;
    let (mut dep, mut trip) = (Hist::new(), Hist::new());
    let mut bad = Vec::new();
    let mut runs = 0usize;
    for off in -3..=(width + 1) {
        // (the cart placed 40 gt before the second pulse: at rest on the hold by its rise)
        let rec = run_once(g, extra, 0, 3, Some(rise - 40), Some((rise + off, len)))?;
        runs += 1;
        let rest_end = rec.hold_busy.iter().copied().filter(|&b| b < rise).max();
        match rec.off_rail {
            Some(o) if o >= rise && o < rise + width && rec.off_powered => {
                *dep.entry(o - arm3).or_default() += 1;
                spare = spare.min(fall - 1 - o);
                if let Some(tr) = departure_press(&rec.trip, rest_end, o) {
                    ok += 1;
                    *trip.entry(tr - arm3).or_default() += 1;
                } else {
                    // (G1 on 6b9be93ea point 5: the departure string's edge is what the FIFO pop reads; a
                    // launch without it is a failure, never a drop)
                    miss += 1;
                    bad.push(format!("freeze from rise + {off}: launched at ARM3 + {}, but the departure string never rose after the rest and within 20 gt", o - arm3));
                }
            }
            Some(o) if o >= rise && o < rise + CYCLE - 20 => {
                late += 1;
                bad.push(format!("freeze from rise + {off}: off the rail at ARM3 + {}, {} (late)", o - arm3, if rec.off_powered { "after the pulse's end" } else { "the rail unpowered" }));
            }
            Some(o) => {
                miss += 1;
                bad.push(format!("freeze from rise + {off}: off the rail at ARM3 + {}", o - arm3));
            }
            None => {
                miss += 1;
                bad.push(format!("freeze from rise + {off}: never left"));
            }
        }
    }
    Ok(FreezeOut { ok, late, miss, dep, spare, trip, bad, width, rise_after_arm3: rise - arm3, runs })
}

/// The Line geometry's control: an unridden villager standing on the rail under the string; whether a
/// hook powers within 40 gt.
fn tripwire_control() -> Result<bool, String> {
    let sp = crate::mirror::checked(&spec_line(EXTRA)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    redsim::entity::spawn(&mut run.world, "minecraft:villager", [3.5, 1.0625, crate::mirror::zf(-1.5)], Some("{PersistenceRequired:1b}"))?;
    for _ in 0..40 {
        run.tick();
        if run.world.get(crate::mirror::at([3, 2, -1])).to_string().contains("powered=true") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The Real geometry's lane-7 check (G1 10:57 (1)): a ridden cart rolling south down lane 7 (from z 2.5 at
/// 0.35) through its merge T (7,0,7) and west along the merge row; the string over (8,0,7) must stay off.
/// Returns (the string's rises, the cart reached the merge row).
fn lane7_rider(extra: i64) -> Result<(usize, bool), String> {
    start_contacts(ContactGeo::Real);
    let sp = crate::mirror::checked(&spec_real(extra)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let v = if crate::mirror::on() { -0.35 } else { 0.35 };
    let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [7.5, 0.0625, crate::mirror::zf(2.5)], Some(&format!("{{Motion:[0.0d,0.0d,{v}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?;
    let (mut rises, mut was, mut merged) = (0usize, false, false);
    for _ in 0..100 {
        run.tick();
        note_contacts(&run, id);
        let now = run.world.get(crate::mirror::at([8, 1, 6])).to_string().contains("powered=true");
        if now && !was {
            rises += 1;
        }
        was = now;
        if let Some(e) = run.world.entities.list.iter().find(|e| e.id == id && !e.removed) {
            let z = crate::mirror::zf(e.pos.z);
            if e.pos.x <= 4.5 && (7.0..8.0).contains(&z) {
                merged = true;
                // (the rider has merged west: the merge row's end is then a declared contact, as after a departure)
                DEPARTED.with(|d| d.set(true));
            }
        }
    }
    Ok((rises, merged))
}

/// The positive control (ironspec 10:50): tick-exact's recorded `tripwire-rider` scene (explore/tickexact
/// eec7361f5, crates/mobai/src/fixture.rs), cell for cell and unmirrored: per row x = 2, 6, 10, 14, 18 a
/// north-south rail on glass at y 64 for z 0..12, the string (x,65,6) between hooks (x,65,5) facing south
/// and (x,65,7) facing north on glass at z 4 / 8; x 2 a ridden cart at rest under it, x 6 a ridden cart
/// from z 1.5 at 0.35, x 10 an empty cart at rest, x 14 an empty cart from z 1.5 at 0.35, x 18 a villager
/// standing under it. Per row the hook's on / off ticks (t1 = after the first tick) over 60 ticks, against
/// the game's (tick-exact 10:57: x 2 and x 18 on at t1; x 6 on t18, off t28; x 10 and x 14 never).
fn rider_scene() -> Result<(Vec<String>, bool), String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for x in 0..=32 {
        for z in 0..=16 {
            blocks.push((BlockPos::new(x, 62, z), b("minecraft:stone")?));
        }
    }
    let rows = [2, 6, 10, 14, 18];
    for x in rows {
        for z in 0..=12 {
            blocks.push((BlockPos::new(x, 63, z), b("minecraft:glass")?));
            blocks.push((BlockPos::new(x, 64, z), b("minecraft:rail[shape=north_south,waterlogged=false]")?));
        }
        blocks.push((BlockPos::new(x, 65, 4), b("minecraft:glass")?));
        blocks.push((BlockPos::new(x, 65, 8), b("minecraft:glass")?));
        blocks.push((BlockPos::new(x, 65, 5), b("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?));
        blocks.push((BlockPos::new(x, 65, 7), b("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?));
        blocks.push((BlockPos::new(x, 65, 6), b(STRING_NS)?));
    }
    let sp = farm(blocks, BlockPos::new(30, 63, 15), BlockPos::new(-1, 61, -1), BlockPos::new(33, 68, 17));
    // (unmirrored: the recorded scene's own cells; refused if any lies outside the world's box)
    let off = crate::mirror::out_of_box(&sp);
    if off > 0 {
        return Err(format!("the tripwire-rider scene: {off} cell(s) outside the world's box"));
    }
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let ridden = |v: f64| format!("{{Motion:[0.0d,0.0d,{v}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}");
    let empty = |v: f64| format!("{{Motion:[0.0d,0.0d,{v}d]}}");
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [2.5, 64.0625, 6.5], Some(&ridden(0.0)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [6.5, 64.0625, 1.5], Some(&ridden(0.35)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [10.5, 64.0625, 6.5], Some(&empty(0.0)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:minecart", [14.5, 64.0625, 1.5], Some(&empty(0.35)))?;
    redsim::entity::spawn(&mut run.world, "minecraft:villager", [18.5, 64.0, 6.5], Some("{PersistenceRequired:1b}"))?;
    let mut spans: Vec<Vec<(i64, i64)>> = vec![Vec::new(); rows.len()];
    let mut was = vec![false; rows.len()];
    for t in 1..=60i64 {
        run.tick();
        for (i, &x) in rows.iter().enumerate() {
            let now = run.world.get(BlockPos::new(x, 65, 5)).to_string().contains("powered=true");
            edge(&mut spans[i], &mut was[i], now, t);
        }
    }
    // the game's rows (tick-exact 10:57): (on, off) per row; i64::MAX = still on at t60
    let game: [&[(i64, i64)]; 5] = [&[(1, i64::MAX)], &[(18, 28)], &[], &[], &[(1, i64::MAX)]];
    let label = ["ridden at rest", "ridden rolling from z 1.5 at 0.35", "empty at rest", "empty rolling", "a villager standing"];
    let fmt = |v: &[(i64, i64)]| if v.is_empty() { "never".to_string() } else { v.iter().map(|&(a, b)| if b == i64::MAX { format!("on t{a}") } else { format!("on t{a}, off t{b}") }).collect::<Vec<_>>().join("; ") };
    let mut all = true;
    let mut out = Vec::new();
    for i in 0..rows.len() {
        let ok = spans[i].as_slice() == game[i];
        all &= ok;
        out.push(format!("x {} ({}): {} (game {}{})", rows[i], label[i], fmt(&spans[i]), fmt(game[i]), if ok { ", same" } else { ", DIFFERENT" }));
    }
    Ok((out, all))
}

/// (G1 on 6ca670551 (2)) An arriving napper in the Real geometry: a ridden cart on the lift's exit rail
/// (7,-3,4) at `x0` (its entry phase) moving east at `v`, up the ascending (8,-3,4), the curve (9,-2,4),
/// the slopes (9,-2,5), (9,-1,6), through the T (9,0,7) onto the hold, spawned at `t_spawn` (after a
/// pulse), the cycle drive at phase 0. Returns (the string's rises before the next pulse's rise at
/// `next_rise`, its centre on the hold cell the tick before that rise, the string's rises in the 40 gt
/// from that rise).
fn arrive_once(extra: i64, x0: f64, v: f64, t_spawn: i64, next_rise: i64) -> Result<(u32, bool, u32), String> {
    let c = cells(Geo::Real);
    start_contacts(ContactGeo::Real);
    let sp = crate::mirror::checked(&spec_real(extra)?)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let mut id: Option<u32> = None;
    let (mut before, mut after, mut was, mut rested) = (0u32, 0u32, false, false);
    for t in 0..(next_rise + 40) {
        let in_cycle = t.rem_euclid(CYCLE);
        if t / CYCLE < 3 && in_cycle == 0 {
            run.world.set_block(bp(c.drive), on.clone(), redsim::world::flags::ALL);
        }
        if in_cycle == 20 {
            run.world.set_block(bp(c.drive), air.clone(), redsim::world::flags::ALL);
        }
        if t == t_spawn {
            id = Some(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x0, -2.9375, crate::mirror::zf(4.5)], Some(&format!("{{Motion:[{v}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
        }
        run.tick();
        if let Some(i) = id {
            note_contacts(&run, i);
        }
        let now = run.world.get(bp(c.hook)).to_string().contains("powered=true");
        if now && !was {
            if t < next_rise {
                before += 1;
            } else {
                after += 1;
            }
        }
        was = now;
        if t == next_rise - 1
            && let Some(i) = id
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == i && !e.removed)
        {
            let (x, z) = (e.pos.x, crate::mirror::zf(e.pos.z));
            rested = x.floor() as i32 == c.hold[0] && z.floor() as i32 == c.hold[2];
        }
        // (the departure: after the pulse's rise, the cart's centre off the hold cell)
        if t >= next_rise
            && let Some(i) = id
            && let Some(e) = run.world.entities.list.iter().find(|e| e.id == i && !e.removed)
            && !(e.pos.x.floor() as i32 == c.hold[0] && crate::mirror::zf(e.pos.z).floor() as i32 == c.hold[2])
        {
            DEPARTED.with(|d| d.set(true));
        }
    }
    Ok((before, rested, after))
}

/// The bench (WIDTH from `extra`: 8 + 2 x extra gt) in geometry `g`: the pulse at every ARM3 phase 0..19
/// over 5 cycles; a ridden cart at rest placed at every `step` gt between two pulses (it leaves at the
/// next pulse), with the string's edge and presses while resting; the freeze sweep at this width; the
/// launch threshold under the freeze for widths 8, 10, 12, 14; the controls. (The chain fix) every bar and
/// control through `v`.
pub fn bench(step: i64, extra: i64, freeze: i64, g: Geo, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    // (G1's audit hole 8: no argument may make a bar vacuous or silently differ from what it prints)
    if step < 1 {
        return Err(format!("--step {step}: at least 1"));
    }
    if extra < 0 {
        return Err(format!("--extra {extra}: at least 0 (the pulse is 8 + 2 x extra gt)"));
    }
    if freeze < 1 {
        return Err(format!("--freeze {freeze}: at least 1 gt (0 thaws in the tick it freezes, so the freeze rows would test nothing)"));
    }
    clear_contacts();
    // (the 20 phases' pulses, against the design: the input cell at ARM3 + D - 2 and the rail at ARM3 + D, both
    // 8 + 2 x extra wide, one a cycle)
    let pulses = |ex: i64| -> Result<(Hist, Hist, Hist, Hist, bool), String> {
        let (mut out_rise, mut out_w, mut rail_rise, mut rail_w) = (Hist::new(), Hist::new(), Hist::new(), Hist::new());
        let mut one_a_cycle = true;
        for phase in 0..20 {
            let rec = run_once(g, ex, phase, 5, None, None)?;
            one_a_cycle &= rec.arm3.len() == 5 && rec.out.len() == 5 && rec.rail.len() == 5;
            for (i, &a) in rec.arm3.iter().enumerate() {
                if let Some(&(r, f)) = rec.out.get(i) {
                    *out_rise.entry(r - a).or_default() += 1;
                    *out_w.entry(f - r).or_default() += 1;
                }
                if let Some(&(r, f)) = rec.rail.get(i) {
                    *rail_rise.entry(r - a).or_default() += 1;
                    *rail_w.entry(f - r).or_default() += 1;
                }
            }
        }
        Ok((out_rise, out_w, rail_rise, rail_w, one_a_cycle))
    };
    let width = 8 + 2 * extra;
    let design_ok = |h: &(Hist, Hist, Hist, Hist, bool), w: i64| {
        let only = |m: &Hist, k: i64| !m.is_empty() && m.keys().all(|&x| x == k);
        h.4 && only(&h.0, D - 2) && only(&h.1, w) && only(&h.2, D) && only(&h.3, w)
    };
    let ph = pulses(extra)?;
    let w_pulse = v.bar_n(&format!("the pulse at every ARM3 phase as designed (input at ARM3 + {}, rail at ARM3 + {D}, both {width} gt, one a cycle)", D - 2), ph.0.values().sum::<u32>() as usize, design_ok(&ph, width));
    // (the control, G1's audit hole 6: the chain built at extra 0 (8 gt) must fail the design-pulse bar for the design's
    // width, through the same measurement)
    let pc = pulses(0)?;
    let w_pctl = if extra != 0 { v.control(&format!("the chain at extra 0 (8 gt) fails the {width}-gt design-pulse bar"), pc.0.values().sum::<u32>() as usize, !design_ok(&pc, width)).to_string() } else { v.scope("the extra-0 control", "this run is itself at extra 0") };
    // the late arrivals, against phase 0's measured rail pulses
    let reference = run_once(g, extra, 0, 3, None, None)?;
    let (Some(&(_, first_end)), Some(&(next_rise, next_fall)), Some(&next_arm3)) = (reference.rail.first(), reference.rail.get(1), reference.arm3.get(1)) else {
        return Err("the reference run saw fewer than two pulses".into());
    };
    let (mut dep, mut dep_arm3, mut clear_arm3, mut trip_arm3, mut trip_dep) = (Hist::new(), Hist::new(), Hist::new(), Hist::new(), Hist::new());
    let mut bad = Vec::new();
    let (mut n, mut tripped, mut resting, mut merged, mut late) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut a = 0i64;
    while first_end + a < next_rise {
        let place = first_end + a;
        let rec = run_once(g, extra, 0, 3, Some(place), None)?;
        n += 1;
        // (G1 (1): a press while the napper rests on the hold, before the pulse launches it)
        let rp = rec.trip.iter().filter(|&&x| x >= place && x < next_rise).count() as u32;
        resting += rp;
        if rp > 0 {
            bad.push(format!("placed {a} gt after the pulse's end: {rp} press(es) while resting"));
        }
        merged += u32::from(rec.merged);
        if g == Geo::Real && !rec.merged {
            bad.push(format!("placed {a} gt after the pulse's end: never reached the merge row"));
        }
        // (the departure: the cart's centre off the hold cell, the brief's criterion; G1's audit hole 6: inside the next
        // pulse only (a departure on a later pulse, or after the pulse's end, is a miss), on the powered rail, with the
        // string's press credited by `departure_press`)
        let rest_end = rec.hold_busy.iter().copied().filter(|&b| b < next_rise).max();
        match rec.off_rail {
            Some(o) if o >= next_rise && o < next_fall => {
                *dep.entry(o - next_rise).or_default() += 1;
                *dep_arm3.entry(o - next_arm3).or_default() += 1;
                if !rec.off_powered {
                    late += 1;
                    bad.push(format!("placed {a} gt after the pulse's end: departed with the rail unpowered (late)"));
                }
                if let Some(l) = rec.left {
                    *clear_arm3.entry(l - next_arm3).or_default() += 1;
                }
                match departure_press(&rec.trip, rest_end, o) {
                    Some(tr) => {
                        tripped += 1;
                        *trip_arm3.entry(tr - next_arm3).or_default() += 1;
                        *trip_dep.entry(tr - o).or_default() += 1;
                    }
                    None => bad.push(format!("placed {a} gt after the pulse's end: the departure string never rose after the rest within 20 gt of the departure")),
                }
            }
            Some(o) if o >= next_fall => bad.push(format!("placed {a} gt after the pulse's end: off the rail at {o}, after the next pulse's end ({next_fall})")),
            Some(o) => bad.push(format!("placed {a} gt after the pulse's end: off the rail at {o}, before the next pulse ({next_rise})")),
            None => bad.push(format!("placed {a} gt after the pulse's end: never left")),
        }
        a += step;
    }
    let w_place = v.bar_n("the placed carts: each leaves on the next pulse on the powered rail, its string press credited, no press while resting, (Real) the merge row reached", n as usize, bad.is_empty());
    let geo = match g {
        Geo::Line => "the Line geometry (the hold, a straight line east under the string)",
        Geo::Real => "the Real geometry (7b90c3f1's hold (9,0,8), the slime stop (9,0,9), T (9,0,7), booster (8,0,7), lane 7's T (7,0,7); the string (8,1,7), hooks (8,1,6) / (8,1,8) on glass)",
    };
    let mut lines = vec![
        format!(
            "split9's hold release in {geo} (ARM3's edge cut to {width} gt by a subtract comparator with a delay-{} side, four 1-tick repeaters into the input cell (7,-1,8); 7b90c3f1's repeater (8,-1,8) into the stone under the hold rail; ARM3 a repeater's output, inside the tick as a detector rail's): over 20 ARM3 phases x 5 cycles the input cell is powered at ARM3 + (gt:count) {} (design {}) for (gt:count) {} (design {width}); the hold rail at ARM3 + {} (design D {D}: the rail's rise) for {}; {} ({w_pulse}); CONTROL (the chain at extra 0): input at ARM3 + {} for {}, the rail for {} ({w_pctl}); every run's spec checked: 0 cells outside its world's box",
            if extra > 0 { format!("{extra} + delay-4") } else { "4".to_string() },
            row(&ph.0),
            D - 2,
            row(&ph.1),
            row(&ph.2),
            row(&ph.3),
            if ph.4 { "one pulse a cycle" } else { "NOT one pulse a cycle" },
            row(&pc.0),
            row(&pc.1),
            row(&pc.3)
        ),
        format!(
            "  a ridden cart at rest placed {n} times between two pulses (0..{} gt after the first pulse's end, every {step}): off the hold rail (its centre off the cell: the brief's departure) at the next rail pulse's rise + (gt:count) {} = that cycle's ARM3 + {}, late (the hold rail unpowered at that tick) {late}; its box clear of the cell at ARM3 + {}; the departure string rose (after the rest, within 20 gt of the departure) for {tripped} of {n} at ARM3 + {} (string - departure, gt:count: {}); presses while the napper rested {resting}{}: {w_place}{}",
            next_rise - first_end - 1,
            row(&dep),
            row(&dep_arm3),
            row(&clear_arm3),
            if trip_arm3.is_empty() { "-".to_string() } else { row(&trip_arm3) },
            if trip_dep.is_empty() { "-".to_string() } else { row(&trip_dep) },
            if g == Geo::Real { format!("; reached the merge row west of lane 7's T {merged} of {n}") } else { String::new() },
            if bad.is_empty() { String::new() } else { format!(" (the misses: {})", bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }
        ),
    ];
    // (the contacts of the 20 phases and the placed carts above go on the placed carts' row)
    annotate(&mut lines);
    let fo = freeze_sweep(g, extra, freeze)?;
    let w_fr = v.bar_n(&format!("a {freeze}-gt freeze at every offset around the pulse: every run launched with its press"), fo.runs, fo.late == 0 && fo.miss == 0);
    push_row(&mut lines, format!(
        "  a cart at rest on the hold, the part frozen {freeze} gt (level 32: redstone running, entities frozen; a login) from every offset around the pulse (rail rise - 3 .. its end + 1), by the brief's criterion (the departure = its centre off the hold cell inside the pulse; late = the hold rail unpowered at that tick): launched {}, late {}, missed {} ({w_fr}){}; departures at ARM3 + {}; the least spare (the rail's powered ticks left after a departure) {}; the string's rising edge at ARM3 + {} (the state follows the departure that happens)",
        fo.ok,
        fo.late,
        fo.miss,
        if fo.bad.is_empty() { String::new() } else { format!(" ({})", fo.bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) },
        row(&fo.dep),
        if fo.spare == i64::MAX { "-".to_string() } else { fo.spare.to_string() },
        if fo.trip.is_empty() { "- (never)".to_string() } else { row(&fo.trip) }
    ));
    // (ironspec 10:05, G1 on 6ca670551: the launch threshold under the freeze by the brief's criterion,
    // every departure before the pulse ends; the chain makes even widths, so the exact least width is
    // read from the widest run: the latest departure after the rail's rise, + 1. G1's audit hole 9: each run's measured
    // rail width must be the width it is labelled; the design width (ironspec 10:09: 12 = max(6, 7) + 3 frozen + 2
    // margin) must launch every run, and the least width with none late or missed must leave the 2-gt margin)
    let mut thr: Vec<String> = Vec::new();
    let mut least: Option<i64> = None;
    let mut latest_after_rise = i64::MIN;
    let mut widths_ok = true;
    let mut design_clean = false;
    for x in 0..=3 {
        let f = freeze_sweep(g, x, freeze)?;
        let w = 8 + 2 * x;
        widths_ok &= f.width == w && f.rise_after_arm3 == D;
        thr.push(format!("{w} gt (measured {} gt at ARM3 + {}): {} launched, {} late, {} missed{}", f.width, f.rise_after_arm3, f.ok, f.late, f.miss, if f.spare == i64::MAX { String::new() } else { format!(" (least spare {})", f.spare) }));
        if f.late + f.miss == 0 && least.is_none() {
            least = Some(w);
        }
        if w == WIDTH {
            design_clean = f.late + f.miss == 0;
        }
        if x == 3 {
            // (the departures vs ARM3; the rail rises at ARM3 + D)
            latest_after_rise = f.dep.keys().copied().max().map_or(i64::MIN, |m| m - D);
        }
    }
    let w_widths = v.bar("each threshold run's measured rail width and rise as labelled", widths_ok);
    let w_design = v.bar(&format!("the design width {WIDTH} launches every run under the {freeze}-gt freeze"), design_clean);
    let w_margin = v.bar("the least clean width leaves the design's 2-gt margin (least <= WIDTH - 2)", least.is_some_and(|l| l <= WIDTH - 2));
    push_row(&mut lines, format!(
        "  the launch threshold under a {freeze}-gt freeze at every offset, by the brief's criterion: {} ({w_widths}); the design width {WIDTH}: {w_design}; the least even width with none late or missed {} (<= {}: {w_margin}); the exact least width DERIVED (not run: the latest departure in the 14-gt run at the rail's rise + {}, + 1) {} gt",
        thr.join("; "),
        least.map_or("none up to 14".to_string(), |w| format!("{w} gt")),
        WIDTH - 2,
        if latest_after_rise == i64::MIN { "-".to_string() } else { latest_after_rise.to_string() },
        if latest_after_rise == i64::MIN { "-".to_string() } else { (latest_after_rise + 1).to_string() }
    ));
    // (ironspec 20:55 PT (2)(6), G1 on 7ccd7737c point 6) the threshold run directly at 8, 9 and 10 gt: the
    // input cell a floor lever on for exactly that long (Real geometry; record7's conforming stand-in). (G1's audit
    // hole 9) errors propagate; each run's rail must be exactly w wide and rise at ARM3 + D; 10 gt (the design's 12 less
    // its 2-gt margin) must launch every run; 8 and 9 locate the threshold; the control, w 0, must fail
    if g == Geo::Real {
        let mut direct = Vec::new();
        let mut shape_ok = true;
        let mut ten_clean = false;
        for w in [8i64, 9, 10] {
            SPLIT_DIRECT.with(|c| c.set(Some(w)));
            let r = freeze_sweep(g, 0, freeze);
            SPLIT_DIRECT.with(|c| c.set(None));
            let f = r?;
            shape_ok &= f.width == w && f.rise_after_arm3 == D;
            if w == 10 {
                ten_clean = f.late + f.miss == 0;
            }
            direct.push(format!(
                "{w} gt (measured {} gt at ARM3 + {}): {} launched, {} late, {} missed{}{}",
                f.width,
                f.rise_after_arm3,
                f.ok,
                f.late,
                f.miss,
                if f.spare == i64::MAX { String::new() } else { format!(" (least spare {})", f.spare) },
                if f.bad.is_empty() { String::new() } else { format!(" [{}]", f.bad.iter().take(2).cloned().collect::<Vec<_>>().join("; ")) }
            ));
        }
        let w_shape = v.bar("each direct run's rail exactly w wide at ARM3 + D", shape_ok);
        let w_ten = v.bar("the direct 10-gt run (the design's 12 less its 2-gt margin) launches every run", ten_clean);
        let w_scope = v.scope("the direct 8- and 9-gt runs' launch counts", "they locate the threshold below the judged 10");
        SPLIT_DIRECT.with(|c| c.set(Some(0)));
        let c0 = freeze_sweep(g, 0, freeze);
        SPLIT_DIRECT.with(|c| c.set(None));
        let (c0_text, c0_bites) = match &c0 {
            Ok(f) => (format!("{} launched, {} late, {} missed", f.ok, f.late, f.miss), f.late + f.miss > 0),
            Err(e) => (format!("no pulse ({e})"), true),
        };
        let w_c0 = v.control("the direct run at 0 gt launches nothing", 1, c0_bites);
        push_row(&mut lines, format!(
            "  the launch threshold run directly (the input cell (7,-1,8) a floor lever on the stone floor, on for exactly w gt, the rail rising at ARM3 + D as the chain's does; a {freeze}-gt freeze at every offset): {} ({w_shape}; 10 gt: {w_ten}; {w_scope}); CONTROL w 0: {c0_text} ({w_c0})",
            direct.join("; ")
        ));
    }
    match g {
        Geo::Line => {
            let hit = tripwire_control()?;
            let w = v.control("an unridden villager standing under the string powers a hook", 1, hit);
            push_row(&mut lines, format!("  the string's control (an unridden villager standing on the rail under it): a hook powered {} ({w})", if hit { "yes" } else { "NO" }));
        }
        Geo::Real => {
            let (rises, merged7) = lane7_rider(extra)?;
            // (G1 on 6b9be93ea point 5: a rider that never merges fails the row too)
            let w7 = v.bar("a lane-7 rider never presses the string and reaches the merge row", rises == 0 && merged7);
            push_row(&mut lines, format!(
                "  a lane-7 rider through its merge T (7,0,7) and west (G1: its box stays at x <= 7.8): the string rose {rises} times; the rider reached the merge row {} ({w7})",
                if merged7 { "yes" } else { "NO" }
            ));
            // (ironspec 11:05 (2): ARM3's only consumer is E; the napper's own rise gives a second pulse on
            // an empty hold)
            let rec = run_full(g, extra, 0, 3, Some(first_end + 10), None, true)?;
            let dep = rec.left.unwrap_or(i64::MAX);
            let after: Vec<(i64, i64)> = rec.rail.iter().copied().filter(|&(r, _)| r > dep && r < next_arm3 + CYCLE).collect();
            let busy = after.iter().any(|&(r, f)| rec.hold_busy.iter().any(|&b| b >= r && b < f));
            let w_own = v.bar("the napper's own ARM3 rise: one departure, no cart on the hold during the extra pulse", rec.arm3_contact.is_some() && rec.departures == 1 && !busy);
            push_row(&mut lines, format!(
                "  the napper's own ARM3 rise (its box reaching ARM3's cell (5,0,7) after it leaves; the detector rail emulated for 20 gt): contact at ARM3 + {}; the rail's pulses after its departure, before the next batch's ARM3 (+ {CYCLE}): {}; a cart on the hold during them: {}; departures {} ({w_own})",
                rec.arm3_contact.map_or("- (no contact)".to_string(), |t| (t - next_arm3).to_string()),
                if after.is_empty() { "none".to_string() } else { after.iter().map(|&(r, f)| format!("ARM3 + {} for {}", r - next_arm3, f - r)).collect::<Vec<_>>().join(", ") },
                if busy { "YES" } else { "none" },
                rec.departures
            ));
            // (G1 on 6ca670551 (2): the arriving napper through the T's north arm, at every entry phase)
            let mut per_v: Vec<String> = Vec::new();
            let (mut n_all, mut pressed_all, mut rested_all, mut dep_all) = (0u32, 0u32, 0u32, 0u32);
            for sp in [0.1, 0.2, 0.3, 0.4] {
                let (mut pressed, mut rested, mut dep_press) = (0u32, 0u32, 0u32);
                let k_n = 16;
                for k in 0..k_n {
                    let x0 = 7.10 + 0.05 * k as f64;
                    let (b, r, a) = arrive_once(extra, x0, sp, first_end + 2, next_rise)?;
                    pressed += u32::from(b > 0);
                    rested += u32::from(r);
                    dep_press += u32::from(a > 0);
                }
                per_v.push(format!("v {sp}: the string pressed before the release in {pressed} of {k_n}, at rest on the hold at the pulse {rested} of {k_n}, the departure's press {dep_press} of {k_n}"));
                n_all += k_n;
                pressed_all += pressed;
                rested_all += rested;
                dep_all += dep_press;
            }
            // (G1 on 7ccd7737c point 2: a cart that derails or stalls before the T presses nothing; every arrival must
            // also come to rest on the hold, and its departure press the string)
            let w_arr = v.bar_n("arriving nappers: none presses the string before the release, every one rests on the hold and presses it departing", n_all as usize, pressed_all == 0 && rested_all == n_all && dep_all == n_all);
            push_row(&mut lines, format!(
                "  arriving nappers (G1: the T's north arm laid; a ridden cart from the lift's exit rail (7,-3,4) at v 0.1..0.4 east x 16 entry offsets, up the slopes through the T (9,0,7) onto the hold, between two pulses): {}; in all, the string pressed before the release in {pressed_all} of {n_all} (bar 0), at rest on the hold at the pulse {rested_all} of {n_all}, the departure's press {dep_all} of {n_all} ({w_arr})",
                per_v.join("; ")
            ));
        }
    }
    let (scene, same) = rider_scene()?;
    let w_scene = v.control("tick-exact's recorded tripwire-rider scene, every row as the game", 5, same);
    push_row(&mut lines, format!("  the positive control, tick-exact's recorded tripwire-rider scene (explore/tickexact eec7361f5) cell for cell: {}; {w_scene}", scene.join("; ")));
    lines.push(tsnap_line(v)?);
    lines.push(format!("  {}", judge_contacts(v)));
    Ok(lines)
}

/// (ironspec 12:23) The arrivals' positive control: tick-exact's recorded t-snap scene
/// (explore/tickexact ff1ca056a, crates/mobai/src/fixture.rs; t-snap.rec hash a7e7d4d7402d132f) cell
/// for cell: the T (9,64,7) south_west with its north arm (the exit rail (7,61,4), a plain ascending
/// (8,61,4), the curve (9,62,4), the powered slopes (9,62,5) and (9,63,6) on redstone blocks), the hold
/// (9,64,8) unpowered with a stone stop (9,64,9), the string (8,65,7) on hooks (8,65,6) / (8,65,8) over
/// stone; 8 copies 16 apart in x, each a cart with a seated villager on the upper slope at (z, vz).
/// The game's first presses (game frames): copies 0, 1, 3, 4, 5 at t7, t5, t4, t3, t3; copies 2 (0.285
/// into the T), 6 and 7 (0.4) never.
fn tsnap_scene() -> Result<(Vec<String>, bool), String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let starts: [(f64, f64); 8] = [(6.6, 0.05), (6.6, 0.15), (6.6, 0.35), (6.95, 0.048), (6.9, 0.148), (6.8, 0.348), (6.6, 0.6), (6.8, 0.8)];
    let game: [Option<i64>; 8] = [Some(7), Some(5), None, Some(4), Some(3), Some(3), None, None];
    let game_v = [0.174, 0.175, 0.285, 0.068, 0.105, 0.255, 0.400, 0.400];
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for x in 0..128 {
        for z in 0..16 {
            blocks.push((BlockPos::new(x, 59, z), b("minecraft:stone")?));
        }
    }
    for k in 0..starts.len() as i32 {
        let dx = 16 * k;
        let mut put = |x: i32, y: i32, z: i32, s: &str| -> Result<(), String> {
            blocks.push((BlockPos::new(x + dx, y, z), b(s)?));
            Ok(())
        };
        put(7, 60, 4, "minecraft:stone")?;
        put(7, 61, 4, "minecraft:rail[shape=east_west,waterlogged=false]")?;
        put(8, 60, 4, "minecraft:stone")?;
        put(8, 61, 4, "minecraft:rail[shape=ascending_east,waterlogged=false]")?;
        put(9, 61, 4, "minecraft:stone")?;
        put(9, 62, 4, "minecraft:rail[shape=south_west,waterlogged=false]")?;
        put(9, 61, 5, "minecraft:redstone_block")?;
        put(9, 62, 5, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
        put(9, 62, 6, "minecraft:redstone_block")?;
        put(9, 63, 6, "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]")?;
        put(9, 63, 7, "minecraft:stone")?;
        put(9, 64, 7, "minecraft:rail[shape=south_west,waterlogged=false]")?;
        put(9, 63, 8, "minecraft:stone")?;
        put(9, 64, 8, "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]")?;
        put(9, 64, 9, "minecraft:stone")?;
        put(8, 65, 5, "minecraft:stone")?;
        put(8, 65, 9, "minecraft:stone")?;
        put(8, 65, 6, "minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
        put(8, 65, 7, STRING_NS)?;
        put(8, 65, 8, "minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    }
    let sp = farm(blocks, BlockPos::new(1, 60, 14), BlockPos::new(-1, 58, -1), BlockPos::new(129, 68, 17));
    let off = crate::mirror::out_of_box(&sp);
    if off > 0 {
        return Err(format!("the t-snap scene: {off} cell(s) outside the world's box"));
    }
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let mut ids = Vec::new();
    for (k, (z, vz)) in starts.iter().enumerate() {
        let x = 9.5 + 16.0 * k as f64;
        ids.push(redsim::entity::spawn(&mut run.world, "minecraft:minecart", [x, 63.0625 + (z - 6.0), *z], Some(&format!("{{Motion:[0.0d,0.0d,{vz}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}")))?);
    }
    let mut first: Vec<Option<i64>> = vec![None; starts.len()];
    let mut v_in: Vec<Option<f64>> = vec![None; starts.len()];
    for t in 1..=60i64 {
        run.tick();
        for k in 0..starts.len() {
            let dx = 16 * k as i32;
            if first[k].is_none() && run.world.get(BlockPos::new(8 + dx, 65, 6)).to_string().contains("powered=true") {
                first[k] = Some(t);
            }
            if v_in[k].is_none()
                && let Some(e) = run.world.entities.list.iter().find(|e| e.id == ids[k] && !e.removed)
                && e.pos.z >= 7.0
            {
                v_in[k] = Some((e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt());
            }
        }
    }
    let fmt = |f: Option<i64>| f.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into());
    let mut all = true;
    let mut out = Vec::new();
    for k in 0..starts.len() {
        let ok = first[k] == game[k];
        all &= ok;
        out.push(format!(
            "copy {k} (z {}, vz {}; into the T at {} (game {:.3})): first press {} (game {}{})",
            starts[k].0,
            starts[k].1,
            v_in[k].map(|v| format!("{v:.3}")).unwrap_or_else(|| "-".into()),
            game_v[k],
            fmt(first[k]),
            fmt(game[k]),
            if ok { ", same" } else { ", DIFFERENT" }
        ));
    }
    Ok((out, all))
}

/// The t-snap control's line (the arrival rows count only after it): (the chain fix) a control in the tool's verdict.
fn tsnap_line(v: &mut crate::verdict::Verdict) -> Result<String, String> {
    let (scene, same) = tsnap_scene()?;
    let w = v.control("tick-exact's recorded t-snap scene, every copy as the game (the arrival rows count only after it)", 8, same);
    Ok(format!(
        "  the arrivals' positive control, tick-exact's recorded t-snap scene (explore/tickexact ff1ca056a: arrivals at the T from the north at <= ~0.26 swing the rider's box into (8,.,7)), the simulator's control with its own recorded cells (its north arm's ascending cell a plain rail on stone; the bench's arrivals ride record7's powered one): {}; {w}",
        scene.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (G1's audit hole 7, the matcher's control) a press while the cart rests (at or before its last tick at rest) is
    /// never credited to the departure; a press after it within 20 gt of the departure is; one later is not.
    #[test]
    fn departure_press_only_after_the_rest() {
        // the cart rests until t 100, departs at t 106
        assert_eq!(departure_press(&[95, 100], Some(100), 106), None);
        assert_eq!(departure_press(&[95, 104], Some(100), 106), Some(104));
        assert_eq!(departure_press(&[127], Some(100), 106), None);
        assert_eq!(departure_press(&[126], Some(100), 106), Some(126));
        // no tick at rest: nothing to credit after
        assert_eq!(departure_press(&[104], None, 106), None);
    }

    /// (the chain fix) one arrival run's misses: a clean run has none; each criterion, broken alone, is a miss.
    #[test]
    fn arrival_misses_each_criterion() {
        let clean = GateRec { arm3: vec![200], fired: vec![(210, 222)], det: Some(150), rest: Some(180), bit: Some(192), off: Some(214), off_powered: true, q_end: false, entered: Some(170), left: Some(214) };
        assert!(arrival_misses(&clean, 42).is_empty());
        let mut r = GateRec { bit: Some(191), ..clean_copy(&clean) };
        assert!(!arrival_misses(&r, 42).is_empty(), "the delay off k_lock");
        r = GateRec { rest: Some(191), ..clean_copy(&clean) };
        assert!(!arrival_misses(&r, 42).is_empty(), "the bit 1 gt after rest");
        r = GateRec { fired: vec![(210, 222), (310, 322)], ..clean_copy(&clean) };
        assert!(!arrival_misses(&r, 42).is_empty(), "two fired pulses");
        r = GateRec { off_powered: false, ..clean_copy(&clean) };
        assert!(!arrival_misses(&r, 42).is_empty(), "the departure late");
        r = GateRec { q_end: true, ..clean_copy(&clean) };
        assert!(!arrival_misses(&r, 42).is_empty(), "the latch left set");
        r = GateRec { arm3: vec![300], ..clean_copy(&clean) };
        assert!(!arrival_misses(&r, 42).is_empty(), "no ARM3 before the fired pulse");
    }

    fn clean_copy(r: &GateRec) -> GateRec {
        GateRec { arm3: r.arm3.clone(), fired: r.fired.clone(), det: r.det, rest: r.rest, bit: r.bit, off: r.off, off_powered: r.off_powered, q_end: r.q_end, entered: r.entered, left: r.left }
    }

    /// (G1's audit hole 13) the gate file's parser: a typo for `live` or a sixth token is refused; `out` is required.
    #[test]
    fn gate_file_parser_is_strict() {
        let dir = std::env::temp_dir();
        let write = |name: &str, body: &str| {
            let p = dir.join(name);
            std::fs::write(&p, body).unwrap();
            p.to_string_lossy().to_string()
        };
        let ok = write("gf-ok.txt", "bit 10 -1 11\nq 15 -1 5\nout 7 -1 8\n1 2 3 minecraft:stone\n1 2 4 minecraft:stone live\n");
        assert_eq!(read_gate_file(&ok).unwrap().cells.len(), 2);
        let typo = write("gf-typo.txt", "bit 10 -1 11\nq 15 -1 5\nout 7 -1 8\n1 2 4 minecraft:stone liev\n");
        assert!(read_gate_file(&typo).is_err());
        let extra = write("gf-extra.txt", "bit 10 -1 11\nq 15 -1 5\nout 7 -1 8\n1 2 4 minecraft:stone live x\n");
        assert!(read_gate_file(&extra).is_err());
        let no_out = write("gf-noout.txt", "bit 10 -1 11\nq 15 -1 5\n1 2 3 minecraft:stone\n");
        assert!(read_gate_file(&no_out).is_err());
    }
}
