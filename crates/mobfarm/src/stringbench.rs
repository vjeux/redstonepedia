//! The rider-height string bench (ironspec Nov 11 21:27, the per-cell occupancy evidence on a
//! generic pocket cell): a straight line along +x built in code: a descent of `descent` plain
//! slopes, the pocket's rest slope with a tripwire string in the cell above it (two hooks across
//! the track facing it, stone supports one cell further out), then the pocket's gate over the next
//! slope and a run-out. One ridden cart is launched at the descent's top. With the gate shut the
//! cart comes to rest at the gate's face under the string and the gate opens `hold` gt after the
//! cart stops (a resting rider); with `--pass` the gate is open from the start (a passing rider).
//! Prints the hook's powered ticks against the cart's arrival under the string, its rest and its
//! leaving: the string as a level sensor for a resting rider, a blip for a passer.

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// The bench: returns the text.
pub fn string_bench(descent: i32, v0: f64, pass: bool, hold: i64, ticks: i64, riders: bool) -> Result<String, String> {
    let glass = Block::parse("minecraft:glass")?;
    let stone = Block::parse("minecraft:stone")?;
    let slope = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let gate_shut = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let gate_open = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=true,powered=false]")?;
    let string = Block::parse("minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,south=true,west=false,powered=false]")?;
    let hook_s = Block::parse("minecraft:tripwire_hook[attached=true,facing=south,powered=false]")?;
    let hook_n = Block::parse("minecraft:tripwire_hook[attached=true,facing=north,powered=false]")?;
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    blocks.push((BlockPos::new(-1, descent, 0), glass.clone()));
    let mut put = |blocks: &mut Vec<(BlockPos, Block)>, x: i32, y: i32, b: &Block| {
        blocks.push((BlockPos::new(x, y, 0), b.clone()));
        blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
    };
    // (the descent: cell x at height descent - x; then the rest slope at x = descent, height 0)
    for x in 0..descent {
        put(&mut blocks, x, descent - x, &slope);
    }
    let xr = descent;
    put(&mut blocks, xr, 0, &slope);
    // (the string in the cell above the rest rail, across the track; hooks at z -1 and +1 facing it;
    // stone supports at z -2 and +2)
    blocks.push((BlockPos::new(xr, 1, 0), string));
    blocks.push((BlockPos::new(xr, 1, -1), hook_s));
    blocks.push((BlockPos::new(xr, 1, 1), hook_n));
    blocks.push((BlockPos::new(xr, 1, -2), stone.clone()));
    blocks.push((BlockPos::new(xr, 1, 2), stone));
    // (the gate over the next slope (its block one above that rail, level with the rest rail), then
    // a run-out: two slopes and four flats)
    put(&mut blocks, xr + 1, -1, &slope);
    blocks.push((BlockPos::new(xr + 1, 0, 0), if pass { gate_open.clone() } else { gate_shut.clone() }));
    put(&mut blocks, xr + 2, -2, &slope);
    put(&mut blocks, xr + 3, -3, &slope);
    for k in 0..4 {
        put(&mut blocks, xr + 4 + k, -3, &flat);
    }
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(xr + 6, 1, 3), lo: BlockPos::new(-4, -8, -4), hi: BlockPos::new(xr + 10, descent + 4, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &o)?;
    while run.settle_tick() {}
    let rider = if riders { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
    let nbt = format!("{{Motion:[{v0:.4}d,0.0d,0.0d]{rider}}}");
    let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [0.1, descent as f64 + 0.9 + 0.0625, 0.5], Some(&nbt))?;
    let hook = BlockPos::new(xr, 1, -1);
    let gate_pos = BlockPos::new(xr + 1, 0, 0);
    let mut lines = vec![format!(
        "string bench: a {}-slope descent, launch {v0:.3} b/t, {} cart, the rest slope at x {xr} with a string in the cell above it, the gate over x {} {}",
        descent,
        if riders { "a ridden" } else { "an empty" },
        xr + 1,
        if pass { "open (a passing rider)" } else { "shut, opened when the cart has rested `hold` gt" }
    )];
    let mut was_on = false;
    let mut on_at: Option<i64> = None;
    let mut pulses: Vec<(i64, i64)> = Vec::new();
    let mut in_cell: Option<(i64, i64)> = None;
    let mut rest_since: Option<i64> = None;
    let mut opened_at: Option<i64> = None;
    let mut left_at: Option<i64> = None;
    let mut first_in: Option<i64> = None;
    let mut last_in: Option<i64> = None;
    for t in 0..ticks {
        run.tick();
        let st = redsim::entity::index_of(&run.world, id).map(|i| {
            let e = &run.world.entities.list[i];
            (e.pos.x, e.pos.y, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt(), e.passengers.len())
        });
        let on = matches!(run.world.get(hook), Block::TripwireHook { powered: true, .. });
        if on && !was_on {
            on_at = Some(t);
        }
        if !on && was_on
            && let Some(a) = on_at.take()
        {
            pulses.push((a, t));
        }
        was_on = on;
        if let Some((x, _, v, _)) = st {
            // (the rider's box, 0.6 wide, overlapping the string's cell in x)
            let over = x + 0.3 > xr as f64 && x - 0.3 < (xr + 1) as f64;
            if over {
                first_in.get_or_insert(t);
                last_in = Some(t);
            }
            if !pass && over && v < 0.001 {
                rest_since.get_or_insert(t);
            }
            if !pass
                && opened_at.is_none()
                && let Some(r) = rest_since
                && t - r >= hold
            {
                run.world.set_block(gate_pos, gate_open.clone(), redsim::world::flags::ALL);
                opened_at = Some(t);
            }
            if opened_at.is_some() && left_at.is_none() && x - 0.3 >= (xr + 1) as f64 {
                left_at = Some(t);
            }
        }
        if in_cell.is_none()
            && let (Some(a), Some(b)) = (first_in, last_in)
            && b < t
        {
            in_cell = Some((a, b));
        }
    }
    if let Some(a) = on_at {
        pulses.push((a, ticks));
    }
    lines.push(format!(
        "the rider's box over the string's cell: {}; at rest under it from {}; the gate opened {}; the cart's box clear of the cell {}",
        match (first_in, last_in) { (Some(a), Some(b)) => format!("t {a}..{b} ({} gt)", b - a + 1), _ => "never".to_string() },
        rest_since.map_or("-".to_string(), |r| format!("t {r}")),
        opened_at.map_or("-".to_string(), |o| format!("t {o}")),
        left_at.map_or("-".to_string(), |l| format!("t {l}"))
    ));
    lines.push(format!(
        "the hook powered: {}",
        if pulses.is_empty() { "never".to_string() } else { pulses.iter().map(|(a, b)| format!("t {a}..{} ({} gt)", b - 1, b - a)).collect::<Vec<_>>().join(", ") }
    ));
    let _ = in_cell;
    Ok(lines.join("\n"))
}
