//! The odd launch tap (day-1, Nov 12 2026; ironspec 05:29: can lane B launch at REL + 13, off the
//! chain's 2-gt grid?). A cart rests on a powered hold rail against a backstop block; the rail is powered
//! (R) by a repeater pointing into it, the chain's own stage, or (P) by a sticky piston, fired by that
//! same repeater, pushing a redstone block beside the rail. In the game's tick order a repeater's output
//! lands in the scheduled-tick phase, before entities move, so the cart leaves in the same gt; a piston's
//! block lands in the block-entity phase two gt after it fires, after entities have moved, so the cart
//! leaves one gt later: 3 gt where a repeater adds 2. The bench measures the cart's departure against
//! the firing repeater's output at every phase of a 20-gt grid.

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

fn spec(drive: Drive) -> Result<FarmSpec, String> {
    let b = |s: &str| Block::parse(s).map_err(|e| e.to_string());
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    // (y 0 stone; the track along +x at y 1 from the backstop at x 0: the hold rail at x 1, plain rails
    // on)
    for x in -1..=12 {
        for z in -1..=6 {
            blocks.push((BlockPos::new(x, 0, z), b("minecraft:stone")?));
        }
    }
    blocks.push((BlockPos::new(0, 1, 0), b("minecraft:stone")?));
    blocks.push((BlockPos::new(1, 1, 0), b("minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]")?));
    for x in 2..=11 {
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
            blocks.push((BlockPos::new(1, 1, 2), b("minecraft:redstone_block")?));
            blocks.push((BlockPos::new(1, 1, 3), b("minecraft:sticky_piston[extended=false,facing=north]")?));
            blocks.push((BlockPos::new(1, 1, 4), b("minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?));
        }
    }
    Ok(FarmSpec {
        blocks,
        entities: Vec::new(),
        output: BlockPos::new(12, 2, 0),
        lo: BlockPos::new(-2, -1, -2),
        hi: BlockPos::new(13, 4, 7),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    })
}

/// One trial: the firing repeater's input driven at t = `at`; returns (the tick the firing repeater's
/// output rose, the tick the cart first moved), both from the run's start.
fn trial(drive: Drive, at: i64) -> Result<(Option<i64>, Option<i64>), String> {
    let sp = spec(drive)?;
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&sp, &ro)?;
    while run.settle_tick() {}
    let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [1.5, 1.0625, 0.5], Some("{Motion:[0.0d,0.0d,0.0d]}"))?;
    let pos = |run: &Run| run.world.entities.list.iter().find(|e| e.id == id && !e.removed && matches!(e.kind, EntityKind::Minecart(_))).map(|e| e.pos);
    // (let the cart settle on the rail first)
    for _ in 0..20 {
        run.tick();
    }
    let x0 = pos(&run).map(|p| p.x).ok_or("the cart is gone")?;
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let (fire, input) = match drive {
        Drive::Repeater => (BlockPos::new(1, 1, 1), BlockPos::new(1, 1, 2)),
        Drive::Piston => (BlockPos::new(1, 1, 4), BlockPos::new(1, 1, 5)),
    };
    let (mut fired, mut moved) = (None, None);
    for t in 0..(at + 40) {
        if t == at {
            run.world.set_block(input, on.clone(), redsim::world::flags::ALL);
        }
        run.tick();
        if fired.is_none() && run.world.get(fire).to_string().contains("powered=true") {
            fired = Some(t);
        }
        if moved.is_none() && pos(&run).is_some_and(|p| (p.x - x0).abs() > 1e-6) {
            moved = Some(t);
        }
    }
    Ok((fired, moved))
}

/// The bench: both drives at every phase 0..19; prints the cart's departure after the firing repeater's
/// output (R: the chain's stage powering the rail; P: the piston's), and the difference.
pub fn bench() -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut lags: std::collections::BTreeMap<&str, std::collections::BTreeMap<i64, u32>> = Default::default();
    for (name, drive) in [("R (a repeater into the hold rail)", Drive::Repeater), ("P (a sticky piston pushing a redstone block beside it)", Drive::Piston)] {
        let mut bad = Vec::new();
        for at in 0..20 {
            match trial(drive, at)? {
                (Some(f), Some(m)) => {
                    *lags.entry(name).or_default().entry(m - f).or_default() += 1;
                }
                other => bad.push(format!("at {at}: fired/moved {other:?}")),
            }
        }
        let row: Vec<String> = lags.get(name).map(|m| m.iter().map(|(l, n)| format!("+{l}:{n}")).collect()).unwrap_or_default();
        out.push(format!(
            "  {name}: the cart's first move after the firing repeater's output rose (gt:count over 20 phases) {}{}",
            row.join(" "),
            if bad.is_empty() { String::new() } else { format!(" (FAIL: {})", bad.join("; ")) }
        ));
    }
    let one = |name: &str| lags.get(name).and_then(|m| (m.len() == 1).then(|| *m.keys().next().unwrap()));
    let (r, p) = (one("R (a repeater into the hold rail)"), one("P (a sticky piston pushing a redstone block beside it)"));
    out.insert(
        0,
        format!(
            "odd launch tap: a cart on a powered hold rail (backstop behind it), powered by R or P: {}",
            match (r, p) {
                (Some(r), Some(p)) => format!("R departs +{r}, P +{p} after the firing stage's output: the piston adds {} gt where the next repeater stage would add 2 ({})", p - r, if (p - r) % 2 == 1 { "an odd offset: B at REL + 13 from the stage at REL + 10" } else { "no odd offset" }),
                _ => "the departure is not one value per drive (see the rows)".to_string(),
            }
        ),
    );
    Ok(out)
}
