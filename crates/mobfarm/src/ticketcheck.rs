//! `mctas mobfarm ticket-check` (onechunk, Nov 12; ironspec's freeze ruling): whether every farm
//! chunk sits at entity-ticking level from the loader loop's portal ticket whenever the loop runs.
//!
//! 26.3's portal ticket, read from the source (`mctas src show`): `NetherPortalBlock.getExitPortal`
//! places `placePortalTicket(pos)` in the DESTINATION level only, at the exit portal block
//! `findClosestPortalPosition` found; `Entity.placePortalTicket` adds
//! `addTicketWithRadius(TicketType.PORTAL, ChunkPos.containing(pos), 3)`, a ticket of level
//! `ChunkLevel.byStatus(FULL) - 3` = 33 - 3 = 30 (`TicketType.PORTAL = register("portal", 300L, 15)`:
//! it persists, loads, simulates and keeps the dimension active; a 300-gt timeout reset on every
//! re-add). A chunk at Chebyshev distance d from the ticket's chunk gets level 30 + d, and
//! `ChunkLevel.fullStatus` reads <= 31 entity-ticking, 32 block-ticking, 33 loaded (not ticking).
//! The overworld ticket comes from the items' Nether-to-overworld crossings, so it stands while one
//! crosses at least every 300 gt.
//!
//! The placements (the budget's tiling, wip/c3/f1q2-budget.txt: modules on even chunks, loops at
//! the odd-odd gap chunks; flag (a)'s fallback mirrors the southern modules in z): the loader at its
//! frame + (17, 0, 17) (gap chunk (1,1)); N = 1 a module at chunk (0,0) (its frame + (6, 0, 6));
//! N = 4 also (2,0) (+ (38, 0, 6)) and the southern two, mirrored in z (world z = 41 - frame z) or
//! not (+ (6 / 38, 0, 38)).

use mctas::design::Design;

/// One module copy's placement: (x offset, z offset, mirrored in z).
fn placements(n: usize, mirror: bool) -> Result<Vec<(i32, i32, bool)>, String> {
    let south = |x: i32| if mirror { (x, 41, true) } else { (x, 38, false) };
    match n {
        1 => Ok(vec![(6, 6, false)]),
        2 => Ok(vec![(6, 6, false), (38, 6, false)]),
        4 => Ok(vec![(6, 6, false), (38, 6, false), south(6), south(38)]),
        _ => Err(format!("--tiling {n}: 1, 2 or 4 (N = 9 runs four loops)")),
    }
}

/// (ironspec's pad-row piglins, Nov 12) The pad portals' random-tick zombified piglins per copy:
/// 26.3's `NetherPortalBlock.randomTick` spawns one at the bottom of the ticked block's portal column
/// (inside the bottom row, its portal cooldown set) when `random.nextInt(2000) < difficulty id` and a
/// player is close enough for spawning (`ChunkMap.anyPlayerCloseEnoughForSpawning`: the chunk within
/// 5 chunks of a player's, or its centre within 128 blocks); every block of a ticking section is
/// random-ticked with chance `random_tick_speed` (3) / 4096 a tick. With tick-exact's official
/// behaviour (ironspec 06:54: inside the rows they stay while the player is within 32 or in the
/// Nether; beyond 128 they despawn at once; at 32..128 a portal-spawned one despawned at ~t 211,
/// a crossed-back one is assumed to stay), the accumulation per copy over `days` by the player's
/// position.
pub fn piglin_season(module: &Design, difficulty: i32, days: f64, tick_speed: f64, despawn_32_128: f64) -> String {
    let portals = module.blocks.values().filter(|s| s.contains("nether_portal")).count() as f64;
    let per_tick = portals * tick_speed / 4096.0 * difficulty as f64 / 2000.0;
    let per_day = per_tick * 24000.0;
    format!(
        "pad portal blocks a copy: {portals}; a portal block's random tick {tick_speed}/4096 a tick, a spawn on {difficulty}/2000 of them (difficulty id {difficulty}): {per_day:.3} zombified piglins a copy-day while a player is within 128 (none otherwise)\n  the player within 32 (or arriving from the Nether): they stay, {:.1} a copy over {days} days (accumulating)\n  32..128: each portal-spawned one despawns in ~{despawn_32_128} gt: {:.4} present on average, ~0 accumulated (crossed-back ones, assumed to stay, are b8949c96's count)\n  beyond 128: 0 spawned, those present despawn at once\n  in another dimension: 0 spawned (no overworld player), those present stay\n",
        per_day * days,
        per_tick * despawn_32_128
    )
}

/// The report and whether every cell is entity-ticking.
pub fn report(module: &Design, loader: &Design, n: usize, mirror: bool) -> Result<(String, bool), String> {
    let chunk = |v: i32| v.div_euclid(16);
    // the loader's overworld portal blocks, placed: the ticket's possible chunks
    let mut tickets: Vec<(i32, i32)> = Vec::new();
    for (p, s) in &loader.blocks {
        if s.contains("nether_portal") {
            let c = (chunk(p[0] + 17), chunk(p[2] + 17));
            if !tickets.contains(&c) {
                tickets.push(c);
            }
        }
    }
    if tickets.is_empty() {
        return Err("the loader holds no overworld portal block".to_string());
    }
    // every cell an entity occupies or passes, or a block the farm needs: the module's blocks and
    // entities (its Nether part aside)
    let mut cells: Vec<([i32; 3], &str)> = module.blocks.keys().map(|p| (*p, "block")).collect();
    for e in &module.entities {
        cells.push(([e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32], "entity"));
    }
    let mut s = format!(
        "the portal ticket (26.3): level 30 at the chunk of the exit portal block found in the destination level (radius 3), entity-ticking at Chebyshev distance <= 1, block-ticking at 2, loaded at 3; the loader's overworld portal placed in chunk(s) {}\n",
        tickets.iter().map(|(x, z)| format!("({x},{z})")).collect::<Vec<_>>().join(", ")
    );
    let mut ok = true;
    for (k, (dx, dz, mirrored)) in placements(n, mirror)?.into_iter().enumerate() {
        let mut per: std::collections::BTreeMap<(i32, i32), (usize, usize)> = std::collections::BTreeMap::new();
        for (p, kind) in &cells {
            let wx = p[0] + dx;
            let wz = if mirrored { dz - p[2] } else { p[2] + dz };
            let e = per.entry((chunk(wx), chunk(wz))).or_insert((0, 0));
            if *kind == "block" {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
        s.push_str(&format!("  module {} (frame + ({dx}, 0, {}){}):\n", k + 1, if mirrored { format!("{dz} - z") } else { dz.to_string() }, if mirrored { ", mirrored in z" } else { "" }));
        for ((cx, cz), (b, e)) in &per {
            // (G1: a chunk's best level is the least over the live tickets of L + d; with the
            // loader's portal in one chunk there is one ticket chunk. Should the portal span
            // chunks, the exit block found decides which one is live: the worst of them is shown)
            let d = tickets.iter().map(|(tx, tz)| (cx - tx).abs().max((cz - tz).abs())).max().unwrap_or(99);
            let level = 30 + d;
            let status = match level {
                ..=31 => "entity-ticking",
                32 => "BLOCK-TICKING ONLY (a violation: the controller would run, the carts and riders frozen)",
                33 => "LOADED, NOT TICKING (a violation)",
                _ => "NOT LOADED (a violation)",
            };
            if level >= 32 {
                ok = false;
            }
            s.push_str(&format!("    chunk ({cx},{cz}): {b} blocks, {e} entities; distance {d}, best level {level}: {status}\n"));
        }
    }
    s.push_str(&format!("N = {n}{}: {}\n", if n == 4 { if mirror { " (the southern modules mirrored)" } else { " (none mirrored)" } } else { "" }, if ok { "every farm chunk entity-ticking (PASS)" } else { "FAIL" }));
    Ok((s, ok))
}
