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
//! the odd-odd gap chunks; ironspec Nov 13 10:26: TRANSLATE ONLY, a mirrored copy is a different
//! machine, `--mirror` an exploration whose output never counts as design evidence): the loader at
//! its frame + (6, 0, 6) like the modules (the column frame to the world's: since 4f6bf5812 the loader
//! file carries its gap-chunk place in the column frame, origin 14 0 17, so its overworld part sits
//! at chunk (1,1); the old + (17, 0, 17) assumed a loader at its file's origin and double-shifted the
//! moved file into (1,2) / (2,2)); N = 1 a module at chunk (0,0) (its frame + (6, 0, 6)); N = 4 also
//! (2,0) (+ (38, 0, 6)) and the southern two (+ (6 / 38, 0, 38)), or mirrored in z under `--mirror`
//! (world z = 41 - frame z).

/// (the loader's move from its file's frame to the world's: the column frame's (6, 0, 6))
const LOADER_DX: i32 = 6;
const LOADER_DZ: i32 = 6;

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

/// (ironspec Nov 12 10:50, the east re-lay's tiling check) Every copy's cells at the budget's
/// pitch, with the loader's: any cell two of them claim, any copy's cell inside another copy's
/// keep-out box (`keep_out`: boxes in the module's frame, carried with each copy), and the chunks
/// each copy uses (a gap chunk is free when only its own copy's re-lay uses it); the least distance
/// from each copy's cells outside its own chunk to any other copy's cells.
pub fn overlap_report(module: &Design, loader: &Design, n: usize, mirror: bool, keep_out: &[([i32; 3], [i32; 3])]) -> Result<(String, bool), String> {
    let chunk = |v: i32| v.div_euclid(16);
    let place = |p: [i32; 3], dx: i32, dz: i32, m: bool| [p[0] + dx, p[1], if m { dz - p[2] } else { p[2] + dz }];
    let mut owner: std::collections::HashMap<[i32; 3], String> = std::collections::HashMap::new();
    let mut clashes: Vec<String> = Vec::new();
    let mut chunks: std::collections::BTreeMap<(i32, i32), std::collections::BTreeSet<String>> = std::collections::BTreeMap::new();
    let copies = placements(n, mirror)?;
    for (k, (dx, dz, m)) in copies.iter().enumerate() {
        let name = format!("module {}", k + 1);
        for p in module.blocks.keys() {
            let w = place(*p, *dx, *dz, *m);
            chunks.entry((chunk(w[0]), chunk(w[2]))).or_default().insert(name.clone());
            if let Some(o) = owner.insert(w, name.clone())
                && o != name
            {
                clashes.push(format!("({},{},{}) {o} and {name}", w[0], w[1], w[2]));
            }
        }
    }
    for p in loader.blocks.keys() {
        let w = [p[0] + LOADER_DX, p[1], p[2] + LOADER_DZ];
        chunks.entry((chunk(w[0]), chunk(w[2]))).or_default().insert("the loader".to_string());
        if let Some(o) = owner.insert(w, "the loader".to_string()) {
            clashes.push(format!("({},{},{}) {o} and the loader", w[0], w[1], w[2]));
        }
    }
    // (each copy's keep-out boxes against every other copy's and the loader's cells)
    let mut inside: Vec<String> = Vec::new();
    for (k, (dx, dz, m)) in copies.iter().enumerate() {
        let name = format!("module {}", k + 1);
        for (lo, hi) in keep_out {
            let (a, b) = (place(*lo, *dx, *dz, *m), place(*hi, *dx, *dz, *m));
            let (l, h) = ([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]);
            for (c, o) in &owner {
                if *o != name && (0..3).all(|i| c[i] >= l[i] && c[i] <= h[i]) {
                    inside.push(format!("({},{},{}) of {o} inside {name}'s keep-out {:?}..{:?}", c[0], c[1], c[2], lo, hi));
                }
            }
        }
    }
    // (the least distance from a copy's cells outside its home chunk to another copy's cells)
    let mut least: Vec<String> = Vec::new();
    for (k, (dx, dz, m)) in copies.iter().enumerate() {
        let name = format!("module {}", k + 1);
        let home = (chunk(dx - 6 + 6), chunk(if *m { dz - 9 } else { dz - 6 + 6 }));
        let out: Vec<[i32; 3]> = module.blocks.keys().map(|p| place(*p, *dx, *dz, *m)).filter(|w| (chunk(w[0]), chunk(w[2])) != home).collect();
        if out.is_empty() {
            continue;
        }
        let mut best: Option<(f64, [i32; 3], [i32; 3], String)> = None;
        for w in &out {
            for (c, o) in &owner {
                if *o == name {
                    continue;
                }
                let d = (((w[0] - c[0]).pow(2) + (w[1] - c[1]).pow(2) + (w[2] - c[2]).pow(2)) as f64).sqrt();
                if best.as_ref().is_none_or(|b| d < b.0) {
                    best = Some((d, *w, *c, o.clone()));
                }
            }
        }
        if let Some((d, w, c, o)) = best {
            least.push(format!("{name}: {} cells outside its chunk {:?}; the nearest other cell {d:.1} away ({},{},{} to {o}'s {},{},{})", out.len(), home, w[0], w[1], w[2], c[0], c[1], c[2]));
        }
    }
    let shared: Vec<String> = chunks.iter().filter(|(_, v)| v.len() > 1).map(|((x, z), v)| format!("({x},{z}): {}", v.iter().cloned().collect::<Vec<_>>().join(" + "))).collect();
    let ok = clashes.is_empty() && inside.is_empty();
    let s = format!(
        "the tiling's cells (N = {n}{}): {} cells claimed twice{}; {} cells inside another copy's keep-out{}; chunks used: {}; chunks used by more than one: {}; {}; {}\n",
        if mirror && n == 4 { ", the southern copies mirrored" } else { "" },
        clashes.len(),
        if clashes.is_empty() { String::new() } else { format!(" (FAIL): {}", clashes.iter().take(20).cloned().collect::<Vec<_>>().join("; ")) },
        inside.len(),
        if inside.is_empty() { String::new() } else { format!(" (FAIL): {}", inside.iter().take(20).cloned().collect::<Vec<_>>().join("; ")) },
        chunks.iter().map(|((x, z), v)| format!("({x},{z}) {}", v.iter().cloned().collect::<Vec<_>>().join("+"))).collect::<Vec<_>>().join(", "),
        if shared.is_empty() { "none".to_string() } else { shared.join("; ") },
        if least.is_empty() { "every copy inside its own chunk".to_string() } else { least.join("; ") },
        if ok { "PASS" } else { "FAIL" }
    );
    Ok((s, ok))
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

/// (G1 on f1507f5dc, ironspec 08:17) The Nether side: every chunk of the Nether group (the kill room,
/// its storage and shield, the twin) against the twin's portal ticket alone (the loop's
/// overworld-to-Nether crossings: level 30 at the twin's exit block's chunk; the kill room's own
/// ticket comes and goes with golem arrivals, so ticking can't depend on it): best level 30 + the
/// Chebyshev distance, >= 32 a violation. The twin's exit blocks are the twin's portal blocks (the
/// group's portal blocks at or above `twin_y`); the worst of their chunks is taken.
pub fn nether_report(group: &Design, twin_y: i32) -> Result<(String, bool), String> {
    let chunk = |v: i32| v.div_euclid(16);
    let mut tickets: Vec<(i32, i32)> = Vec::new();
    for (p, s) in &group.blocks {
        if s.contains("nether_portal") && p[1] >= twin_y {
            let c = (chunk(p[0]), chunk(p[2]));
            if !tickets.contains(&c) {
                tickets.push(c);
            }
        }
    }
    if tickets.is_empty() {
        return Err(format!("no portal block at or above y {twin_y} (the twin)"));
    }
    let mut per: std::collections::BTreeMap<(i32, i32), usize> = std::collections::BTreeMap::new();
    for p in group.blocks.keys() {
        *per.entry((chunk(p[0]), chunk(p[2]))).or_insert(0) += 1;
    }
    let mut ok = true;
    let mut s = format!(
        "the Nether group against the twin's ticket alone (level 30 at the chunk of the twin's exit block: its portal blocks at y >= {twin_y} sit in chunk(s) {}); the same group serves N = 1 and N = 4:\n",
        tickets.iter().map(|(x, z)| format!("({x},{z})")).collect::<Vec<_>>().join(", ")
    );
    for ((cx, cz), n) in &per {
        let d = tickets.iter().map(|(tx, tz)| (cx - tx).abs().max((cz - tz).abs())).max().unwrap_or(99);
        let level = 30 + d;
        if level >= 32 {
            ok = false;
        }
        s.push_str(&format!(
            "  chunk ({cx},{cz}): {n} blocks; distance {d}, best level {level}: {}\n",
            match level {
                ..=31 => "entity-ticking",
                32 => "BLOCK-TICKING ONLY (a violation)",
                _ => "NOT TICKING (a violation)",
            }
        ));
    }
    s.push_str(&format!("the Nether group: {}\n", if ok { "every chunk entity-ticking from the twin's ticket alone (PASS)" } else { "FAIL" }));
    Ok((s, ok))
}

/// The report and whether every cell is entity-ticking.
pub fn report(module: &Design, loader: &Design, n: usize, mirror: bool) -> Result<(String, bool), String> {
    let chunk = |v: i32| v.div_euclid(16);
    // the loader's overworld portal blocks, placed: the ticket's possible chunks
    let mut tickets: Vec<(i32, i32)> = Vec::new();
    for (p, s) in &loader.blocks {
        if s.contains("nether_portal") {
            let c = (chunk(p[0] + LOADER_DX), chunk(p[2] + LOADER_DZ));
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
