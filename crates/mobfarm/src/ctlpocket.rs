//! The pocket drive (day-1, Nov 12 2026; ironspec 15:11 / 16:03: (b) M16bw, the side drive, final once its
//! power rise matches (a)'s at every pocket). Each lane's pocket rail P (x,0,0) (a powered rail, north-south,
//! the pocket's slime back (x,0,-1)) is driven by the station's switch:
//! - (a) the slime support (x,-1,0) under P, strongly powered by a delay-1 repeater (x,-1,-1) facing north
//!   (input (x,-1,-2), the switch's cell) on glass (x,-2,-1);
//! - (b) M16bw: the support (x,-1,0) glass; a delay-1 repeater (x-1,0,0) facing west into P, on glass
//!   (x-1,-1,0), fed by the riser dust (x-2,0,0) on glass (x-2,-1,0), the step dust (x-3,-1,0) on glass
//!   (x-3,-2,0) and the feed dust: lanes 3 and 7 from the north (x-3,-1,-1) on glass (x-3,-2,-1), the
//!   switch's cell (x-3,-1,-2); lane -1 from the west (-5,-1,0) on glass (-5,-2,0) (9fc3155f: S1's ray), the
//!   switch's cell (-6,-1,0).
//! (A diode's `facing` names its input side, as in the game.)
//!
//! The bench lays all three pockets of a variant at once, places a redstone block on one lane's switch
//! cell for 30 gt at 20 phases, and prints that lane's P rise and fall after the switch's (the block placed
//! before tick t0, P read after each tick: P powered after tick t0 + k is k + 1 gt after the switch) and
//! the other pockets' rails (any rise is a leak); then the same with a restart at every tick around both
//! edges in every reload order. The control: (b)'s cells with lane -1's support a redstone block (the
//! emulation dfb89499 found leaking into lane 3's step dust, which touches it) must power lane 3's rail.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

type P = [i32; 3];

const XS: [i32; 3] = [-1, 3, 7];
const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const GLASS: &str = "minecraft:glass";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    A,
    B,
    /// (b)'s cells with lane -1's support a redstone block: the leak's positive control.
    LeakControl,
}

/// The variant's blocks and each lane's switch cell.
fn cells(v: Variant) -> (Vec<(P, String)>, [P; 3]) {
    let mut b: Vec<(P, String)> = Vec::new();
    let mut put = |p: P, s: &str| b.push((p, s.to_string()));
    let mut sw = [[0; 3]; 3];
    for (i, &x) in XS.iter().enumerate() {
        put([x, 0, 0], "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]");
        put([x, 0, -1], "minecraft:slime_block");
        match v {
            Variant::A => {
                put([x, -1, 0], "minecraft:slime_block");
                put([x, -1, -1], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]");
                put([x, -2, -1], GLASS);
                sw[i] = [x, -1, -2];
            }
            Variant::B | Variant::LeakControl => {
                if v == Variant::LeakControl && x == -1 {
                    put([x, -1, 0], "minecraft:redstone_block");
                } else {
                    put([x, -1, 0], GLASS);
                }
                put([x - 1, 0, 0], "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]");
                put([x - 1, -1, 0], GLASS);
                put([x - 2, 0, 0], DUST);
                put([x - 2, -1, 0], GLASS);
                put([x - 3, -1, 0], DUST);
                put([x - 3, -2, 0], GLASS);
                if x == -1 {
                    put([-5, -1, 0], DUST);
                    put([-5, -2, 0], GLASS);
                    sw[i] = [-6, -1, 0];
                } else {
                    put([x - 3, -1, -1], DUST);
                    put([x - 3, -2, -1], GLASS);
                    sw[i] = [x - 3, -1, -2];
                }
            }
        }
    }
    (b, sw)
}

fn spec(v: Variant) -> Result<(FarmSpec, [P; 3]), String> {
    let (cs, sw) = cells(v);
    let mut blocks = Vec::new();
    for (p, s) in cs {
        blocks.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(&s).map_err(|e| e.to_string())?));
    }
    let sp = FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(9, 1, 3),
        lo: BlockPos::new(-8, -3, -3),
        hi: BlockPos::new(10, 2, 3),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    };
    Ok((sp, sw))
}

/// One rail's record: powered at settle (before any switch), every rise and fall (the ticks after which it reads
/// powered / unpowered).
#[derive(Clone, Default)]
struct RailRec {
    at_settle: bool,
    rises: Vec<i64>,
    falls: Vec<i64>,
}

/// One run: lane `lane`'s switch on from t0 for `width` gt (None: no switch driven, the control's idle run); a
/// restart between ticks; each pocket rail's every edge (G1's audit hole 17: every edge kept, so a double pulse
/// from a restart is seen); the switch cell's state before -> after its drive.
#[allow(clippy::type_complexity)]
fn run_once(v: Variant, lane: Option<usize>, t0: i64, width: i64, restart: Option<(i64, crate::liveness::Restart)>) -> Result<([RailRec; 3], Option<(String, String)>, Option<crate::liveness::RestartEvidence>), String> {
    let (sp, sw) = spec(v)?;
    let sp = crate::mirror::checked(&sp)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let mut ev: Option<crate::liveness::RestartEvidence> = None;
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: P| crate::mirror::at(p);
    let mut out: [RailRec; 3] = Default::default();
    let mut was = [false; 3];
    let mut drive: Option<(String, String)> = None;
    // (the rails as settled, before any switch: the control's lane 3 is already powered by the leak)
    for (i, &x) in XS.iter().enumerate() {
        was[i] = run.world.get(bp([x, 0, 0])).to_string().contains("powered=true");
        out[i].at_settle = was[i];
    }
    for t in 0..(t0 + width + 30) {
        if let Some((rt, how)) = restart
            && rt == t
        {
            ev = Some(crate::liveness::restart_ev(&mut run.world, how)?);
        }
        if let Some(ln) = lane {
            if t == t0 {
                let before = run.world.get(bp(sw[ln])).to_string();
                run.world.set_block(bp(sw[ln]), on.clone(), redsim::world::flags::ALL);
                drive = Some((before, run.world.get(bp(sw[ln])).to_string()));
            }
            if t == t0 + width {
                run.world.set_block(bp(sw[ln]), air.clone(), redsim::world::flags::ALL);
            }
        }
        run.tick();
        for (i, &x) in XS.iter().enumerate() {
            let now = run.world.get(bp([x, 0, 0])).to_string().contains("powered=true");
            if now && !was[i] {
                out[i].rises.push(t);
            }
            if !now && was[i] {
                out[i].falls.push(t);
            }
            was[i] = now;
        }
    }
    Ok((out, drive, ev))
}

type Hist = std::collections::BTreeMap<i64, u32>;

fn row(m: &Hist) -> String {
    if m.is_empty() {
        return "never".to_string();
    }
    m.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")
}

/// The bench (see the module's doc). (The chain fix) every bar through `v`: each driven lane one rise and one fall
/// at every phase, no other pocket's rail moving (a leak), every restart row the same, (b) the same as (a) phase by
/// phase; the control (no switch driven) must show lane 3 powered at settle by the leak.
pub fn bench(v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    let width = 30i64;
    let mut lines = vec![format!(
        "the pocket drive (ironspec 16:03: (b) M16bw final once its power rise matches (a)'s at every pocket): all three pockets of a variant laid at once, one lane's switch cell driven {width} gt at 20 phases; P = the pocket rail (x,0,0); every run's spec checked: 0 cells outside its world's box{}",
        crate::mirror::tag()
    )];
    let mut rises: std::collections::BTreeMap<(usize, &str), Vec<Option<(i64, i64)>>> = Default::default();
    for (var, vname) in [(Variant::A, "(a) the slime support (x,-1,0) under P, its delay-1 repeater (x,-1,-1)"), (Variant::B, "(b) M16bw: the side repeater (x-1,0,0) into P, the riser (x-2,0,0), the step (x-3,-1,0), the feed (lane -1 from the west)")] {
        lines.push(format!("  {vname}:"));
        let tag = if var == Variant::A { "a" } else { "b" };
        for (i, &x) in XS.iter().enumerate() {
            let (mut rise, mut fall, mut leaks) = (Hist::new(), Hist::new(), 0u32);
            let mut base: Option<(i64, i64)> = None;
            // (G1 on dd1c4270f point 2: each phase's (rise, fall) kept, an edge-less phase a failure, and (a)
            // against (b) compared phase by phase; G1's audit hole 17: exactly one rise and one fall)
            let mut per_phase: Vec<Option<(i64, i64)>> = Vec::new();
            let mut drive_over = String::new();
            let (mut drive_n, mut drive_unchanged) = (0usize, 0usize);
            for t0 in 40..60 {
                let (o, drive, _) = run_once(var, Some(i), t0, width, None)?;
                // (G1's Y3: every application of the switch override judged)
                if let Some((b, a)) = &drive {
                    drive_n += 1;
                    drive_unchanged += usize::from(b == a);
                }
                if drive_over.is_empty()
                    && let Some((b, a)) = &drive
                {
                    drive_over = v.over(&format!("({tag}) lane {x}'s switch cell"), b, a);
                }
                if o[i].rises.len() == 1 && o[i].falls.len() == 1 && !o[i].at_settle {
                    let (r, f) = (o[i].rises[0], o[i].falls[0]);
                    *rise.entry(r - t0 + 1).or_default() += 1;
                    *fall.entry(f - (t0 + width) + 1).or_default() += 1;
                    base.get_or_insert((r - t0, f - (t0 + width)));
                    per_phase.push(Some((r - t0, f - (t0 + width))));
                } else {
                    per_phase.push(None);
                }
                leaks += o.iter().enumerate().filter(|(j, s)| *j != i && (s.at_settle || !s.rises.is_empty())).count() as u32;
            }
            let edgeless = per_phase.iter().filter(|p| p.is_none()).count();
            if drive_unchanged > 0 || drive_n != per_phase.len() {
                v.fail(&format!("({tag}) lane {x}'s switch override: {drive_unchanged} of its {drive_n} applications unchanged, {} phases without one", per_phase.len() - drive_n.min(per_phase.len())));
                drive_over += " (an application UNCHANGED or missing: FAIL)";
            } else {
                drive_over += &format!(" (all {drive_n} applications changed it)");
            }
            let edges = v.bar_n(&format!("({tag}) lane {x}: one rise and one fall at every phase"), per_phase.len(), edgeless == 0);
            let leak_w = v.bar_n(&format!("({tag}) lane {x}: no other pocket's rail moves"), per_phase.len(), leaks == 0);
            rises.insert((i, tag), per_phase);
            let t0 = 40;
            let (mut n, mut same, mut diff) = (0u32, 0u32, Vec::<String>::new());
            let (mut ev_bad, mut ev_first) = (Vec::<String>::new(), String::new());
            for rt in ((t0 - 1)..=(t0 + 6)).chain((t0 + width - 1)..=(t0 + width + 6)) {
                for how in crate::liveness::Restart::rows() {
                    let (o, _, ev) = run_once(var, Some(i), t0, width, Some((rt, how)))?;
                    n += 1;
                    // (G1's Y6: the restart shown to have happened, from its own evidence)
                    match &ev {
                        Some(e) if e.happened() => {
                            if ev_first.is_empty() {
                                ev_first = e.line();
                            }
                        }
                        Some(e) => ev_bad.push(format!("restart at t0 + {} ({}): {}", rt - t0, how.label(), e.line())),
                        None => ev_bad.push(format!("restart at t0 + {} ({}): no restart ran", rt - t0, how.label())),
                    }
                    let got_r: Vec<i64> = o[i].rises.iter().map(|r| r - t0).collect();
                    let got_f: Vec<i64> = o[i].falls.iter().map(|f| f - (t0 + width)).collect();
                    let leak = o.iter().enumerate().any(|(j, s)| j != i && (s.at_settle || !s.rises.is_empty()));
                    if base.is_some_and(|(br, bf)| got_r == vec![br] && got_f == vec![bf]) && !leak {
                        same += 1;
                    } else {
                        diff.push(format!("restart at t0 + {} ({}): P rises {got_r:?}, falls {got_f:?}, leak {leak}", rt - t0, how.label()));
                    }
                }
            }
            let restarts = v.bar_n(&format!("({tag}) lane {x}: every restart row the same as without one"), n as usize, diff.is_empty() && base.is_some());
            let ev_w = v.bar_n(&format!("({tag}) lane {x}: every restart happened (its chunks unloaded and loaded again, its scheduled ticks back)"), n as usize, ev_bad.is_empty());
            // (G1's Y6, the control through the REAL run path: the same restart row with the restart a no-op must fail the
            // same predicate)
            let (_, _, stub) = crate::liveness::with_noop_restart(|| run_once(var, Some(i), t0, width, Some((t0 + 3, crate::liveness::Restart::rows()[0]))))?;
            let ev_c = v.control(&format!("({tag}) lane {x}: a no-op restart through the real run fails the restart bar"), usize::from(stub.is_some()), !stub.as_ref().is_some_and(|e| e.happened()));
            let restarts = format!("{restarts}; the restarts' own evidence: {} ({ev_w}; the stub control {ev_c}){}", if ev_first.is_empty() { "none".to_string() } else { format!("e.g. {ev_first}") }, if ev_bad.is_empty() { String::new() } else { format!(": {}", ev_bad.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) });
            lines.push(format!(
                "    lane {x} ({drive_over}): P powered (gt after the switch, gt:count) {}, unpowered after the switch's removal {}; phases without exactly one rise and one fall {edgeless} ({edges}); the other pockets' rails moved {leaks} times (a leak; {leak_w}); with a restart at every tick around both edges in every reload order: {same} of {n} the same ({restarts}){}",
                row(&rise),
                row(&fall),
                if diff.is_empty() { String::new() } else { format!(" (the differing rows: {})", diff.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) }
            ));
        }
    }
    let mut verdict: Vec<String> = Vec::new();
    for (i, x) in XS.iter().enumerate() {
        let (a, b) = (rises.get(&(i, "a")), rises.get(&(i, "b")));
        let same = matches!((a, b), (Some(a), Some(b)) if !a.is_empty() && a == b && a.iter().all(|p| p.is_some()));
        let w = v.bar(&format!("lane {x}: (b)'s rise and fall the same as (a)'s at every phase"), same);
        verdict.push(format!("lane {x} {}", w));
    }
    lines.push(format!("  (b)'s rise against (a)'s, phase by phase: {}", verdict.join(", ")));
    // the control: lane -1's support a redstone block, NO switch driven (G1's audit defect 3): lane 3's rail must be
    // powered at settle by the leak, through the same run
    let (o, drive, _) = run_once(Variant::LeakControl, None, 40, width, None)?;
    let ctl = v.control("lane -1's support a redstone block powers lane 3's rail at settle (no switch driven)", 1, drive.is_none() && o[1].at_settle);
    lines.push(format!(
        "  the control ((b)'s cells, lane -1's support (-1,-1,0) a redstone block, no switch driven{}): lane 3's rail at settle {} (lane 3's step dust (0,-1,0) touches that support), lane 7's {}: {ctl}",
        if drive.is_none() { "" } else { ": A SWITCH WAS DRIVEN" },
        if o[1].at_settle { "powered" } else { "unpowered" },
        if o[2].at_settle { "powered" } else { "unpowered" }
    ));
    Ok(lines)
}
