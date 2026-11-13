//! (tickexact 2026-11-13, ironspec 00:27 / 01:49 PT: rules B, C, E; G1 on 6cfa4dd72) `mctas mobai
//! water-settle`: on the official 26.3 server, a design pasted with every fluid cell's tick scheduled
//! (`seatclear::place_part`, rule B), the bench's `--set` cells laid, the world saved and read back
//! right away (the region files' scheduled fluid ticks against the stated fluid cells: a shortfall
//! fails), then run `--ticks` gt, saved and read back again: every cell of the design's box (+1 on
//! each side) whose fluid-bearing state differs from the stored file is listed: NEW water (a cell
//! the design leaves dry), LOST water (a stored fluid cell come out dry: a sourceless current that
//! drained), CHANGED (a fluid-bearing cell in another state: a water level, a bubble column's drag,
//! a waterlogged block's other properties). Any of the three fails the run (rule C: water moving
//! from its stored state); a `# water: x,y,z=purpose (evidence)` line only annotates a current the
//! file already holds as water and exempts no difference (G1 on b00a076b0; the brief 38a136e2f).
//! `--controls`: three designs built here through the same code path must fail: a sourceless
//! flowing cell (LOST), a bubble column stored with the wrong drag (CHANGED) and a `# water:`-
//! declared cell stored dry that the settle wets (NEW); C7 as laid (its leak at (7,-23,-1)) is the
//! real-data control.

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn opts(version: &str) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), with_bot: false, tick_rate: 1000, keep_dir: true, ..Default::default() }
}

fn snapshot(srv: &mut VanillaServer, lo: [i32; 3], hi: [i32; 3]) -> Result<mctas::vanilla::Snapshot> {
    // VanillaServer::snapshot: a marker flipped in every chunk (so each is saved fresh, its
    // scheduled-tick lists too), `save-all flush`, the overworld's region files read back
    srv.snapshot([lo[0], lo[1] + DY, lo[2]], [hi[0], hi[1] + DY, hi[2]]).context("reading the saved box back")
}

/// A fluid-bearing state normalized for comparison (a water source written with or without level=0).
fn norm(s: &str) -> String {
    s.replace("[level=0]", "")
}

/// What one settle found: (new, lost, changed, scheduled-tick shortfall); a `# water:` cell counts too.
struct Outcome {
    new: usize,
    lost: usize,
    changed: usize,
    shortfall: usize,
    /// The region files' scheduled fluid ticks right after the paste (all positions), and the
    /// stated fluid cells: a control shows the total can match while a cell has no tick of its own.
    total_ticks: usize,
    fluid: usize,
}

fn settle(version: &str, file: &std::path::Path, set: &[([i32; 3], String)], ticks: u32) -> Result<Outcome> {
    settle_skipping(version, file, set, ticks, &[])
}

fn settle_skipping(version: &str, file: &std::path::Path, set: &[([i32; 3], String)], ticks: u32, skip_fresh: &[[i32; 3]]) -> Result<Outcome> {
    let d = mctas::design::Design::load(file)?;
    let keys: Vec<[i32; 3]> = d.blocks.iter().map(|(p, _)| [p[0], p[1], p[2]]).collect();
    anyhow::ensure!(!keys.is_empty(), "{}: no blocks", file.display());
    let lo: [i32; 3] = std::array::from_fn(|k| keys.iter().map(|p| p[k]).min().unwrap_or(0) - 1);
    let hi: [i32; 3] = std::array::from_fn(|k| keys.iter().map(|p| p[k]).max().unwrap_or(0) + 1);
    let stated: std::collections::HashMap<[i32; 3], String> = d.blocks.iter().map(|(p, s)| ([p[0], p[1], p[2]], s.clone())).collect();
    // (ironspec 01:07 PT) the design's `# water: x,y,z=purpose (evidence); …` lines: designed currents
    let text = std::fs::read_to_string(file)?;
    let mut designed: std::collections::BTreeMap<[i32; 3], String> = Default::default();
    for l in text.lines().filter_map(|l| l.trim().strip_prefix("# water:")) {
        for e in l.split(';').filter(|e| !e.trim().is_empty()) {
            let (p, why) = e.split_once('=').unwrap_or((e, ""));
            let v: Vec<i32> = p.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            if v.len() == 3 {
                designed.insert([v[0], v[1], v[2]], why.trim().to_string());
            }
        }
    }
    println!(
        "== water-settle on the official {version} server: {} (+{DY} y) pasted with every fluid cell's tick scheduled, {} --set cell(s) laid, saved and read back, then {ticks} gt, saved and read back: every cell of x {}..{} y {}..{} z {}..{} whose fluid-bearing state differs from the stored file",
        file.display(),
        set.len(),
        lo[0],
        hi[0],
        lo[1],
        hi[1],
        lo[2],
        hi[2]
    );
    let mut srv = VanillaServer::start(&opts(version))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok(&format!("forceload add {} {} {} {}", lo[0] - 16, lo[2] - 16, hi[0] + 16, hi[2] + 16))?;
    srv.step(40)?;
    crate::seatclear::place_part_skipping(&mut srv, file, skip_fresh)?;
    for (p, s) in set {
        // (G1 on 6ec22ffd8 #9, ironspec 04:07) an override must change its cell
        let out = srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
        anyhow::ensure!(out.contains("Changed the block"), "--set ({},{},{}) = {s} changed nothing ({}): the run fails", p[0], p[1], p[2], out.trim());
    }
    let s0 = snapshot(&mut srv, lo, hi)?;
    let fluid = stated.values().filter(|s| crate::seatclear::holds_water(s)).count();
    // (ironspec 02:28 PT (f2)) every stated fluid cell's OWN position must hold a scheduled fluid
    // tick in the region files (a total over the chunks lets a spreading neighbour's tick mask a
    // missing one)
    let ticked: std::collections::HashSet<[i32; 3]> = s0.fluid_tick_cells.iter().copied().collect();
    let mut missing: Vec<String> = stated
        .iter()
        .filter(|(p, s)| crate::seatclear::holds_water(s) && !ticked.contains(&[p[0], p[1] + DY, p[2]]))
        .map(|(p, s)| format!("({},{},{}) {s}", p[0], p[1], p[2]))
        .collect();
    missing.sort();
    let shortfall = missing.len();
    println!(
        "  right after the paste: the region files hold {} scheduled fluid tick(s) in the box's chunks; {} of the {fluid} stated fluid cells hold one at their own position{}",
        s0.fluid_ticks,
        fluid - shortfall,
        if shortfall > 0 { format!(": MISSING at {}", missing.join("; ")) } else { String::new() }
    );
    srv.step(ticks)?;
    let s1 = snapshot(&mut srv, lo, hi)?;
    let (mut new, mut lost, mut changed) = (Vec::new(), Vec::new(), Vec::new());
    let mut counts = (0usize, 0usize, 0usize);
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let want = stated.get(&[x, y, z]).map(|s| s.as_str()).unwrap_or("minecraft:air");
                // (G1 on 6ec22ffd8 #11) an unreadable cell fails, never reads as dry
                let got = s1.region.get([x, y + DY, z]).ok_or_else(|| anyhow::anyhow!("({x},{y},{z}): unreadable in the saved region files"))?;
                let (w, g) = (crate::seatclear::holds_water(want), crate::seatclear::holds_water(got));
                // (G1 on b00a076b0, the brief 38a136e2f) `# water:` only annotates a current the file
                // already holds as water: it exempts no difference
                let label = designed.get(&[x, y, z]).map(|why| format!(" (declared `# water:` {why}: no exemption)")).unwrap_or_default();
                let line = format!("({x},{y},{z}) {want} -> {got}{label}");
                if g && !w {
                    counts.0 += 1;
                    new.push(line);
                } else if w && !g {
                    counts.1 += 1;
                    lost.push(line);
                } else if w && g && norm(want) != norm(got) {
                    counts.2 += 1;
                    changed.push(line);
                }
            }
        }
    }
    let show = |v: &Vec<String>| if v.is_empty() { String::new() } else { format!(": {}", v.join("; ")) };
    println!("  after {ticks} gt: NEW water {} cell(s){}", new.len(), show(&new));
    println!("  LOST water {} cell(s){}", lost.len(), show(&lost));
    println!("  CHANGED fluid-bearing state {} cell(s){}", changed.len(), show(&changed));
    println!("  scheduled fluid ticks left in the box's chunks: {}", s1.fluid_ticks);
    let saved_currents: Vec<String> = designed.iter().filter(|(p, _)| stated.get(*p).is_some_and(|s| crate::seatclear::holds_water(s))).map(|(p, why)| format!("({},{},{}) {why}", p[0], p[1], p[2])).collect();
    let dry_declared: Vec<String> = designed.keys().filter(|p| !stated.get(*p).is_some_and(|s| crate::seatclear::holds_water(s))).map(|p| format!("({},{},{})", p[0], p[1], p[2])).collect();
    println!(
        "  `# water:` annotations: {} declared, {} held as water in the file{}{}",
        designed.len(),
        saved_currents.len(),
        show(&saved_currents),
        if dry_declared.is_empty() { String::new() } else { format!("; declared but stored dry (annotates nothing): {}", dry_declared.join(", ")) }
    );
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(Outcome { new: counts.0, lost: counts.1, changed: counts.2, shortfall, total_ticks: s0.fluid_ticks, fluid })
}

pub fn water_settle(version: &str, file: &std::path::Path, set: &[([i32; 3], String)], ticks: u32) -> Result<()> {
    // (G1 on 6ec22ffd8 #3 / #11) no 0-tick run; a design run runs the controls first through the same
    // code path (each must be caught), so a design is never judged without them
    anyhow::ensure!(ticks >= 1, "--ticks 0: no settling judged (NOT JUDGED)");
    controls(version, ticks)?;
    let o = settle(version, file, set, ticks)?;
    anyhow::ensure!(o.shortfall == 0, "rule B: {} stated fluid cell(s) without a scheduled tick: the run fails", o.shortfall);
    anyhow::ensure!(o.new + o.lost + o.changed == 0, "rule C: NEW {} / LOST {} / CHANGED {}: the water moved from its stored state, the run fails", o.new, o.lost, o.changed);
    Ok(())
}

/// The three controls, through `settle` itself: each must be caught in its own category.
pub fn controls(version: &str, ticks: u32) -> Result<()> {
    let dir = std::env::temp_dir().join(format!("water-settle-controls-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    // (i) a flowing cell with no source, on a stone floor: it drains (LOST)
    let mut lost = mctas::design::Design::default();
    lost.name = "water-settle-control-lost".into();
    for x in -1..=1 {
        for z in -1..=1 {
            lost.blocks.insert([x, 0, z], "minecraft:stone".into());
        }
    }
    lost.blocks.insert([0, 1, 0], "minecraft:water[level=3]".into());
    let lost_path = dir.join("lost.txt");
    lost.save(&lost_path)?;
    // (ii) a bubble column stored with drag=true (downward) over soul sand (upward), walled in
    // glass with a glass cap: its state changes (CHANGED)
    let mut drag = mctas::design::Design::default();
    drag.name = "water-settle-control-drag".into();
    drag.blocks.insert([0, 0, 0], "minecraft:soul_sand".into());
    drag.blocks.insert([0, 1, 0], "minecraft:bubble_column[drag=true]".into());
    for p in [[1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1], [0, 2, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]] {
        drag.blocks.insert(p, "minecraft:glass".into());
    }
    let drag_path = dir.join("drag.txt");
    drag.save(&drag_path)?;
    // (iii) (G1 on b00a076b0) a source in a glass trough with its east neighbour stored dry but
    // declared `# water:`: the settle wets it, and the declaration exempts nothing (NEW)
    let mut wet = mctas::design::Design::default();
    wet.name = "water-settle-control-declared".into();
    for x in -1..=2 {
        for z in -1..=1 {
            wet.blocks.insert([x, 0, z], "minecraft:glass".into());
            if z != 0 || x == -1 || x == 2 {
                wet.blocks.insert([x, 1, z], "minecraft:glass".into());
            }
        }
    }
    wet.blocks.insert([0, 1, 0], "minecraft:water".into());
    let wet_path = dir.join("declared.txt");
    wet.save(&wet_path)?;
    let text = std::fs::read_to_string(&wet_path)?;
    std::fs::write(&wet_path, format!("# water: 1,1,0=a current the file stores dry (control: the declaration must not exempt it)\n{text}"))?;
    // (iv) (ironspec 02:28 PT (f1)) place_part's guard bites: a water cell the server can't set
    // (y 250 + 80, above the build limit) must fail the paste, not pass on a count of attempts
    let mut high = mctas::design::Design::default();
    high.name = "water-settle-control-unplaceable".into();
    high.blocks.insert([0, 0, 0], "minecraft:glass".into());
    high.blocks.insert([0, 250, 0], "minecraft:water".into());
    let high_path = dir.join("unplaceable.txt");
    high.save(&high_path)?;
    // (v) (f2) a cell with no tick of its own while the total matches: a source placed fresh at
    // (0,1,0), a waterlogged slab at (2,1,0) planted strictly (no tick), and a --set source at
    // (0,1,1) the design doesn't state (its tick makes the total equal the stated cells)
    let mut mask = mctas::design::Design::default();
    mask.name = "water-settle-control-masked".into();
    for x in -1..=3 {
        for z in -1..=2 {
            mask.blocks.insert([x, 0, z], "minecraft:glass".into());
        }
    }
    mask.blocks.insert([0, 1, 0], "minecraft:water".into());
    mask.blocks.insert([2, 1, 0], "minecraft:oak_slab[type=bottom,waterlogged=true]".into());
    let mask_path = dir.join("masked.txt");
    mask.save(&mask_path)?;
    let a = settle(version, &lost_path, &[], ticks)?;
    let b = settle(version, &drag_path, &[], ticks)?;
    let c = settle(version, &wet_path, &[], ticks)?;
    let d = settle(version, &high_path, &[], ticks);
    let e = settle_skipping(version, &mask_path, &[([0, 1, 1], "minecraft:water".to_string())], 1, &[[2, 1, 0]])?;
    let _ = std::fs::remove_dir_all(&dir);
    let d_caught = matches!(&d, Err(err) if format!("{err:#}").contains("not placed fresh"));
    println!("  control (i) sourceless flowing cell: LOST {} -> {}", a.lost, if a.lost > 0 { "caught" } else { "NOT CAUGHT" });
    println!("  control (ii) bubble column stored with the wrong drag: CHANGED {} -> {}", b.changed, if b.changed > 0 { "caught" } else { "NOT CAUGHT" });
    println!("  control (iii) a `# water:`-declared cell stored dry that the settle wets: NEW {} -> {}", c.new, if c.new > 0 { "caught" } else { "NOT CAUGHT" });
    println!(
        "  control (iv) a water cell the server can't set (above the build limit): {} -> {}",
        match &d {
            Ok(_) => "the paste passed".to_string(),
            Err(err) => format!("{err:#}").chars().take(160).collect::<String>(),
        },
        if d_caught { "caught" } else { "NOT CAUGHT" }
    );
    println!(
        "  control (v) a waterlogged cell with no tick of its own while the region files' total ({}) covers the {} stated fluid cells: missing at its own position {} -> {}",
        e.total_ticks,
        e.fluid,
        e.shortfall,
        if e.shortfall > 0 && e.total_ticks >= e.fluid { "caught (a total would have passed)" } else if e.shortfall > 0 { "caught (the total short too: no masking shown)" } else { "NOT CAUGHT" }
    );
    anyhow::ensure!(a.lost > 0 && b.changed > 0 && c.new > 0 && d_caught && e.shortfall > 0, "a water-settle control was not caught: the run fails");
    Ok(())
}
