//! The odd launch tap (day-1, Nov 12 2026; ironspec 05:29: can lane B launch at REL + 13, off the
//! chain's 2-gt grid?). A cart with a villager rider rests on a powered hold rail against a backstop
//! block; the rail is powered (R) by a repeater pointing into it, the chain's own stage, or (P) by a
//! sticky piston, fired by that same repeater, pushing a redstone block beside the rail. In the game's
//! tick order a repeater's output lands in the scheduled-tick phase, before entities move; a piston's
//! block lands in the block-entity phase two gt after it fires, after entities have moved, so the cart
//! gets its power one gt later: 3 gt where a repeater adds 2. G1 on 72d510183: a launch needs ~5 gt of
//! rail power from rest, and P powers the rail for only (pulse - 2) gt, so the bench drives the firing
//! repeater for `pulse` gt (or holds it until the cart leaves the hold cell), counts a departure as the
//! cart leaving the cell, and checks that P's block comes back with the rail unpowered once the drive
//! ends; a pulse under 3 gt spits P's block (the control).

use redsim::block::Block;
use redsim::entity::EntityKind;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// The two ways to power the hold rail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drive {
    /// The firing repeater points into the hold rail (the chain's 2-gt stage).
    Repeater,
    /// The firing repeater fires a sticky piston that pushes a redstone block beside the hold rail.
    Piston,
}

const HOLD: [i32; 3] = [1, 1, 0];
const BLOCK_HOME: [i32; 3] = [1, 1, 2];
const BLOCK_OUT: [i32; 3] = [1, 1, 1];

fn spec(drive: Drive) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    // (y 0 stone; the track along +x at y 1 from the backstop at x 0: the hold rail at x 1, plain rails
    // on)
    for x in -1..=24 {
        for z in -1..=6 {
            blocks.push((BlockPos::new(x, 0, z), b("minecraft:stone")?));
        }
    }
    blocks.push((BlockPos::new(0, 1, 0), b("minecraft:stone")?));
    blocks.push((BlockPos::new(HOLD[0], HOLD[1], HOLD[2]), b("minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]")?));
    for x in 2..=23 {
        blocks.push((BlockPos::new(x, 1, 0), b("minecraft:rail[shape=east_west,waterlogged=false]")?));
    }
    // (a diode's `facing` points at its input, as in the game)
    match drive {
        Drive::Repeater => {
            // the firing repeater at (1,1,1), input from (1,1,2), output north into the hold rail
            blocks.push((BlockPos::new(1, 1, 1), b("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?));
        }
        Drive::Piston => {
            // the redstone block at (1,1,2), the sticky piston behind it at (1,1,3) facing north (it pushes
            // the block to (1,1,1), beside the rail), the firing repeater at (1,1,4), input from (1,1,5)
            blocks.push((BlockPos::new(BLOCK_HOME[0], 1, BLOCK_HOME[2]), b("minecraft:redstone_block")?));
            blocks.push((BlockPos::new(1, 1, 3), b("minecraft:sticky_piston[extended=false,facing=north]")?));
            blocks.push((BlockPos::new(1, 1, 4), b("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?));
        }
    }
    Ok(FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(24, 2, 0),
        lo: BlockPos::new(-2, -1, -2),
        hi: BlockPos::new(25, 4, 7),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    })
}

/// One trial's outcome, times from the firing repeater's output rise.
#[derive(Debug, Default)]
struct Out {
    /// The cart clear of the hold cell (its rear past x 2.0).
    left: Option<i64>,
    /// (P) gt after the drive ended until the block was home, the beside cell empty and the rail
    /// unpowered.
    restored: Option<i64>,
    fired: bool,
}

/// One trial: the firing repeater's input driven from t = `at` for `pulse` gt (None: until the cart
/// leaves the hold cell).
fn trial(drive: Drive, at: i64, pulse: Option<i64>) -> Result<Out, String> {
    let sp = spec(drive)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [1.5, 1.0625, 0.5], Some("{Motion:[0.0d,0.0d,0.0d],Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
    let pos = |run: &Run| run.world.entities.list.iter().find(|e| e.id == id && !e.removed && matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.pos);
    for _ in 0..20 {
        run.tick();
    }
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let bp = |p: [i32; 3]| BlockPos::new(p[0], p[1], p[2]);
    let (fire, input) = match drive {
        Drive::Repeater => (BlockPos::new(1, 1, 1), BlockPos::new(1, 1, 2)),
        Drive::Piston => (BlockPos::new(1, 1, 4), BlockPos::new(1, 1, 5)),
    };
    let mut out = Out::default();
    let mut fired_at: Option<i64> = None;
    let mut drive_end: Option<i64> = None;
    let mut driving = false;
    for t in 0..(at + 160) {
        if t == at {
            run.world.set_block(input, on.clone(), redsim::world::flags::ALL);
            driving = true;
        }
        let end_now = driving
            && match pulse {
                Some(w) => t == at + w,
                None => out.left.is_some(),
            };
        if end_now {
            run.world.set_block(input, air.clone(), redsim::world::flags::ALL);
            driving = false;
            drive_end = Some(t);
        }
        run.tick();
        if fired_at.is_none() && run.world.get(fire).to_string().contains("powered=true") {
            fired_at = Some(t);
        }
        if let (Some(f), None) = (fired_at, out.left)
            && pos(&run).is_some_and(|p| p.x - 0.49 >= 2.0)
        {
            out.left = Some(t - f);
        }
        if drive == Drive::Piston
            && let (Some(e), None) = (drive_end, out.restored)
        {
            let home = run.world.get(bp(BLOCK_HOME)).to_string().contains("redstone_block");
            let beside_empty = run.world.get(bp(BLOCK_OUT)).to_string().contains("air");
            let unpowered = run.world.get(bp(HOLD)).to_string().contains("powered=false");
            if home && beside_empty && unpowered {
                out.restored = Some(t - e);
            }
        }
    }
    out.fired = fired_at.is_some();
    Ok(out)
}

/// The bench: both drives at every phase 0..19 for each pulse width and held until the cart leaves;
/// prints the departures (the cart leaving the hold cell after the firing stage's output), P's
/// restoration after the drive ends, and the held drives' difference.
pub fn bench(pulses: &[i64]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut held_left: std::collections::BTreeMap<&str, std::collections::BTreeMap<i64, u32>> = Default::default();
    let modes: Vec<Option<i64>> = pulses.iter().map(|w| Some(*w)).chain(std::iter::once(None)).collect();
    for (name, drive) in [("R", Drive::Repeater), ("P", Drive::Piston)] {
        for &pulse in &modes {
            let mut left: std::collections::BTreeMap<i64, u32> = Default::default();
            let (mut stayed, mut restored, mut not_restored) = (0u32, Vec::new(), 0u32);
            for at in 0..20 {
                let o = trial(drive, at, pulse)?;
                match o.left {
                    Some(l) => *left.entry(l).or_default() += 1,
                    None => stayed += 1,
                }
                if drive == Drive::Piston {
                    match o.restored {
                        Some(r) => restored.push(r),
                        None => not_restored += 1,
                    }
                }
            }
            if pulse.is_none() {
                held_left.insert(name, left.clone());
            }
            let lrow: Vec<String> = left.iter().map(|(l, n)| format!("+{l}:{n}")).collect();
            let rest = if drive == Drive::Piston {
                format!(
                    "; the block home, the beside cell empty and the rail unpowered {} of 20 ({} gt after the drive ended){}",
                    20 - not_restored,
                    match (restored.iter().min(), restored.iter().max()) {
                        (Some(a), Some(b)) if a == b => a.to_string(),
                        (Some(a), Some(b)) => format!("{a}..{b}"),
                        _ => "-".into(),
                    },
                    if not_restored > 0 { " (the rail left powered: a spat block)" } else { "" }
                )
            } else {
                String::new()
            };
            out.push(format!(
                "  {name}, the drive {}: the cart (a villager riding) left the hold cell {} of 20 (gt after the firing stage:count {}){}{rest}",
                match pulse {
                    Some(w) => format!("a {w}-gt pulse"),
                    None => "held until the cart leaves".to_string(),
                },
                20 - stayed,
                if lrow.is_empty() { "-".into() } else { lrow.join(" ") },
                if stayed > 0 { format!("; stayed on the hold cell {stayed}") } else { String::new() }
            ));
        }
    }
    let one = |name: &str| held_left.get(name).and_then(|m| (m.len() == 1).then(|| *m.keys().next().unwrap()));
    let (r, p) = (one("R"), one("P"));
    out.insert(
        0,
        format!(
            "odd launch tap (a cart with a villager rider on a powered hold rail, a backstop behind it; departure = leaving the hold cell): {}",
            match (r, p) {
                (Some(r), Some(p)) => format!("held drives: R leaves +{r}, P +{p} after the firing stage's output: the piston adds {} gt where the next repeater stage would add 2 ({})", p - r, if (p - r) % 2 == 1 { "an odd offset: B at REL + 13 from the stage at REL + 10" } else { "no odd offset" }),
                _ => "the held drives' departures are not one value each (see the rows)".to_string(),
            }
        ),
    );
    Ok(out)
}
