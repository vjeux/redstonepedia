//! The hold bench (ironspec Nov 11 15:44, the metered hold ahead of R1's waiting queue): a straight
//! line along +x built in code: a descent of `descent` plain slopes, a brake slope (one descending
//! powered rail, never powered), `flats` flat cells, then the hold gate H_R over the first queue
//! slope (a fence gate over a descending rail, as G0 is). Carts are launched at the descent's top
//! at given stored speeds, ridden or empty, `gap` gt apart; each one's stored speed leaving the
//! brake, any contact with a cart ahead (the stored speed at contact, the brief's 0.2 bar), and
//! where it comes to rest (a cart left at rest on the brake never moves again: a failure).

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

/// One launched cart's record.
pub struct HoldCart {
    pub v_in: f64,
    pub v_brake_in: Option<f64>,
    pub v_brake_out: Option<f64>,
    pub contact: Option<f64>,
    /// The contact's place: the cart's x, the cart ahead and its x, the tick.
    pub contact_at: Option<(f64, usize, f64, i64)>,
    pub rest_x: f64,
    pub rest_v: f64,
    pub stuck_on_brake: bool,
}

/// The bench: returns each cart's record and the text.
#[allow(clippy::too_many_arguments)]
pub fn hold_bench(speeds: &[f64], riders: &[bool], descent: i32, post: i32, flats: i32, gap: i64, ticks: i64, brake: bool) -> Result<(Vec<HoldCart>, String), String> {
    let ridden_k = |k: usize| riders.get(k).or(riders.last()).copied().unwrap_or(true);
    let glass = Block::parse("minecraft:glass")?;
    let slope = Block::parse("minecraft:rail[shape=ascending_west,waterlogged=false]")?;
    let flat = Block::parse("minecraft:rail[shape=east_west,waterlogged=false]")?;
    let brake_b = Block::parse("minecraft:powered_rail[powered=false,shape=ascending_west,waterlogged=false]")?;
    let gate = Block::parse("minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]")?;
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    // (the descent: cell x at height descent - x, each rising toward the west; an ascending rail
    // needs a rigid block on its high side, the glass under the slope before it, or for the
    // first one a glass block of its own)
    blocks.push((BlockPos::new(-1, descent, 0), glass.clone()));
    let mut put = |x: i32, y: i32, b: &Block| {
        blocks.push((BlockPos::new(x, y, 0), b.clone()));
        blocks.push((BlockPos::new(x, y - 1, 0), glass.clone()));
    };
    for x in 0..descent {
        put(x, descent - x, &slope);
    }
    // (the brake slope, then `post` plain slopes down (a bounced cart can't climb back up them to
    // the brake), the flats at height -post, then the first queue slope one lower with H_R over it)
    let xb = descent;
    put(xb, 0, if brake { &brake_b } else { &slope });
    for k in 0..post {
        put(xb + 1 + k, -1 - k, &slope);
    }
    let h = -post;
    let xf = xb + 1 + post;
    for k in 0..flats {
        put(xf + k, h, &flat);
    }
    let xq = xf + flats;
    put(xq, h - 1, &slope);
    put(xq + 1, h - 2, &slope);
    blocks.push((BlockPos::new(xq, h, 0), gate));
    let hi = descent + 4;
    let spec = FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(xq + 6, 1, 3), lo: BlockPos::new(-4, -6, -4), hi: BlockPos::new(xq + 8, hi + 4, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&spec, &o)?;
    while run.settle_tick() {}
    let mut ids: Vec<Option<u32>> = vec![None; speeds.len()];
    let mut rec: Vec<HoldCart> = speeds.iter().map(|v| HoldCart { v_in: *v, v_brake_in: None, v_brake_out: None, contact: None, contact_at: None, rest_x: f64::NAN, rest_v: 0.0, stuck_on_brake: false }).collect();
    let mut prev: Vec<Option<(f64, f64)>> = vec![None; speeds.len()];
    let mut rest_on_brake: Vec<i64> = vec![0; speeds.len()];
    for t in 0..ticks {
        for (k, v) in speeds.iter().enumerate() {
            if t == k as i64 * gap {
                // (Motion in the spawn NBT, as the contact scenes launch their carts)
                let rider = if ridden_k(k) { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]" } else { "" };
                let nbt = format!("{{Motion:[{v:.4}d,0.0d,0.0d]{rider}}}");
                let y = descent as f64 + 0.9 + 0.0625;
                let id = redsim::entity::spawn(&mut run.world, "minecraft:minecart", [0.1, y, 0.5], Some(&nbt))?;
                ids[k] = Some(id);
            }
        }
        run.tick();
        if std::env::var("MCTAS_BRAKE_TRACE").is_ok() && t < 40 {
            for (k, id) in ids.iter().enumerate() {
                if let Some(i) = id.and_then(|id| redsim::entity::index_of(&run.world, id)) {
                    let e = &run.world.entities.list[i];
                    let below = run.world.get(BlockPos::new(e.pos.x.floor() as i32, e.pos.y.floor() as i32, 0));
                    eprintln!("t {t} cart {k}: {:.3},{:.3},{:.3} dm {:.3},{:.3},{:.3} passengers {} on {:?}", e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, e.passengers.len(), below);
                }
            }
        }
        // (each cart's x and stored speed; a contact: its stored speed dropping by 0.04 or more
        // with another cart's centre within 1.3 ahead of it)
        let now: Vec<Option<(f64, f64)>> = ids.iter().map(|id| id.and_then(|id| redsim::entity::index_of(&run.world, id)).map(|i| { let e = &run.world.entities.list[i]; (e.pos.x, (e.delta_movement.x.powi(2) + e.delta_movement.z.powi(2)).sqrt()) })).collect();
        for k in 0..speeds.len() {
            let (Some((x, v)), Some((px, pv))) = (now[k], prev[k]) else { continue };
            if px < xb as f64 && x >= xb as f64 && rec[k].v_brake_in.is_none() {
                rec[k].v_brake_in = Some(pv);
            }
            if px < (xb + 1) as f64 && x >= (xb + 1) as f64 && rec[k].v_brake_out.is_none() {
                rec[k].v_brake_out = Some(v);
            }
            if pv - v >= 0.04 && rec[k].contact.is_none() {
                let ahead = (0..speeds.len()).find(|&j| j != k && now[j].is_some_and(|(xj, _)| xj > x && xj - x < 1.3));
                if let Some(j) = ahead {
                    rec[k].contact = Some(pv);
                    rec[k].contact_at = Some((x, j, now[j].map_or(f64::NAN, |n| n.0), t));
                }
            }
            if (xb as f64..(xb + 1) as f64).contains(&x) && v < 0.001 {
                rest_on_brake[k] += 1;
                if rest_on_brake[k] >= 40 {
                    rec[k].stuck_on_brake = true;
                }
            } else {
                rest_on_brake[k] = 0;
            }
            rec[k].rest_x = x;
            rec[k].rest_v = v;
        }
        prev = now;
    }
    // (a cart left on the brake cell at the end, at rest or nearly: it never moves again; with no
    // brake the cell is a plain slope and a cart resting there is held by the gate, not stuck)
    for r in rec.iter_mut() {
        if brake && (xb as f64..(xb + 1) as f64).contains(&r.rest_x) && r.rest_v < 0.01 {
            r.stuck_on_brake = true;
        }
        if !brake {
            r.stuck_on_brake = false;
        }
    }
    let mut lines = vec![format!(
        "hold bench: {} carts ({}), a {}-slope descent, {}, {post} plain slope(s) down, {flats} flats to H_R (over the queue's first slope at x {xq}), {gap} gt apart, {ticks} gt",
        speeds.len(),
        (0..speeds.len()).map(|k| if ridden_k(k) { "R" } else { "E" }).collect::<Vec<_>>().join(""),
        descent,
        if brake { "one brake slope (a descending powered rail, never powered)" } else { "no brake (a plain slope)" }
    )];
    for (k, r) in rec.iter().enumerate() {
        lines.push(format!(
            "  cart {k}: launched at {:.2}; entering the brake {}; leaving it {}; contact with a cart ahead {}; at rest at x {:.2}{}",
            r.v_in,
            r.v_brake_in.map_or("-".to_string(), |v| format!("{v:.3}")),
            r.v_brake_out.map_or("-".to_string(), |v| format!("{v:.3}")),
            match (r.contact, r.contact_at) {
                (Some(v), Some((x, j, xj, t))) => format!("at a stored {v:.3}{} (t {t}: at x {x:.2}{}, cart {j} at {xj:.2})", if v > 0.2 { " (above 0.2)" } else { "" }, if (xb as f64..(xb + 1) as f64).contains(&x) { ", ON THE BRAKE" } else { "" }),
                _ => "none".to_string(),
            },
            r.rest_x,
            if r.stuck_on_brake { "  STOPPED ON THE BRAKE" } else { "" }
        ));
    }
    Ok((rec, lines.join("\n")))
}

/// The brake window (ironspec Nov 11 15:56, bar 1): each launch speed alone in its own world (no
/// other cart), its entry and exit speeds at the brake and whether it stops on it; the least
/// entry speed that never stops on the brake and the greatest whose exit keeps 0.2 (and 0.15, the
/// margin) or less.
pub fn brake_window(speeds: &[f64], ridden: bool, descent: i32, post: i32, flats: i32, ticks: i64) -> Result<String, String> {
    let mut rows: Vec<(f64, Option<f64>, Option<f64>, bool)> = Vec::new();
    for v in speeds {
        let (rec, _) = hold_bench(&[*v], &[ridden], descent, post, flats, 1, ticks, true)?;
        let r = &rec[0];
        rows.push((r.v_in, r.v_brake_in, r.v_brake_out, r.stuck_on_brake));
    }
    let mut lines = vec![format!("brake window ({}, one cart per world, a {descent}-slope descent before the brake): launch, entry, exit, stopped", if ridden { "ridden" } else { "empty" })];
    for (vi, ein, eout, stuck) in &rows {
        lines.push(format!(
            "  launched {vi:.2}: entry {}, exit {}{}",
            ein.map_or("-".to_string(), |v| format!("{v:.3}")),
            eout.map_or("-".to_string(), |v| format!("{v:.3}")),
            if *stuck { "  STOPPED ON THE BRAKE" } else { "" }
        ));
    }
    // (the floor: the least entry above which no tried entry stopped; the ceiling: the greatest
    // entry below which every tried exit kept 0.2, and 0.15)
    let mut sorted: Vec<&(f64, Option<f64>, Option<f64>, bool)> = rows.iter().filter(|r| r.1.is_some()).collect();
    sorted.sort_by(|a, b| a.1.unwrap().total_cmp(&b.1.unwrap()));
    let last_stuck = sorted.iter().rposition(|r| r.3);
    let floor = match last_stuck {
        Some(i) => sorted.get(i + 1).map(|r| r.1.unwrap()),
        None => sorted.first().map(|r| r.1.unwrap()),
    };
    let ceil = |bar: f64| -> Option<f64> {
        let first_bad = sorted.iter().position(|r| !r.3 && r.2.is_some_and(|e| e > bar));
        match first_bad {
            Some(0) => None,
            Some(i) => Some(sorted[i - 1].1.unwrap()),
            None => sorted.last().map(|r| r.1.unwrap()),
        }
    };
    lines.push(format!(
        "  the window: never stops on the brake from an entry of {} (every tried entry at or above it passed); its exit keeps 0.2 or less up to an entry of {}, 0.15 (the margin) up to {}",
        floor.map_or("- (every tried entry stopped)".to_string(), |v| format!("{v:.3}")),
        ceil(0.2).map_or("-".to_string(), |v| format!("{v:.3}")),
        ceil(0.15).map_or("-".to_string(), |v| format!("{v:.3}"))
    ));
    Ok(lines.join("\n"))
}
