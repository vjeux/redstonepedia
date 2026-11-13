//! (tickexact 2026-11-13, ironspec 01:46 PT (a); farmopt's step lists 02:10) `mctas mobai rail-steps`:
//! official replays of the rail-shape builder recipes on the module of record. Per lane x (C =
//! (x,24,7) stored north_west, D = (x,24,8) the branch detector, c3 = (x-1,24,7)): the module
//! pasted (rails strict, as stated; fluids scheduled), then C, D and c3 removed without updates
//! (`setblock … air strict`), and a recipe replayed as plain `setblock`s, which re-run each rail's
//! placement (BaseRailBlock.onPlace -> updateState -> RailState.place, and the neighbours'
//! updates), 2 gt after each step. Every watched cell's state, read back from the saved region
//! files, is printed after every step; each recipe's expected end is checked:
//! - test (ironspec's recipe): c3; D; a temporary glass at (x,23,6) and a temporary rail on it;
//!   a temporary redstone block at the power cell; C; remove the temporary rail, the power, the
//!   glass. Expect C north_west, c3 and D as stored, nothing temporary left;
//! - control (i) (ironfarm's original order, the temporary rail removed before D): c3; the
//!   temporary glass + rail; C; remove the rail and the glass; D. Expect C south_west;
//! - control (ii) (D placed while the temporary rail stands): c3; the temporary glass + rail; C; D;
//!   remove the rail and the glass. Expect C south_west.

use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn read(srv: &mut VanillaServer, cells: &[[i32; 3]]) -> Result<Vec<String>> {
    let lo: [i32; 3] = std::array::from_fn(|k| cells.iter().map(|c| c[k]).min().unwrap_or(0));
    let hi: [i32; 3] = std::array::from_fn(|k| cells.iter().map(|c| c[k]).max().unwrap_or(0));
    let snap = srv.snapshot([lo[0], lo[1] + DY, lo[2]], [hi[0], hi[1] + DY, hi[2]])?;
    Ok(cells.iter().map(|c| snap.region.get([c[0], c[1] + DY, c[2]]).unwrap_or("?").to_string()).collect())
}

fn short(s: &str) -> String {
    s.trim_start_matches("minecraft:").to_string()
}

/// One recipe on one lane: (C's end state, whether the expectation held).
fn replay(version: &str, file: &std::path::Path, x: i32, case: &str, power: [i32; 3]) -> Result<(String, bool)> {
    let d = mctas::design::Design::load(file)?;
    let (c, dd, c3, tr, tb) = ([x, 24, 7], [x, 24, 8], [x - 1, 24, 7], [x, 24, 6], [x, 23, 6]);
    // (ironfarm 02:13) the top slope (x,22,6), its high-side glass (x,22,7) and C's support glass
    // (x,23,7): the `full` order replays them first
    let (slope, sg, cg) = ([x, 22, 6], [x, 22, 7], [x, 23, 7]);
    let stated = |p: [i32; 3]| -> Result<String> { d.blocks.get(&p).cloned().ok_or_else(|| anyhow::anyhow!("({},{},{}) is empty in the file", p[0], p[1], p[2])) };
    let (s_c, s_d, s_c3) = (stated(c)?, stated(dd)?, stated(c3)?);
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), with_bot: false, tick_rate: 1000, keep_dir: true, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, file)?;
    srv.step(5)?;
    let mut watched = vec![c, c3, dd, tr, tb, power, slope, cg];
    for o in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
        let n = [power[0] + o[0], power[1] + o[1], power[2] + o[2]];
        if !watched.contains(&n) {
            watched.push(n);
        }
    }
    let names: Vec<String> = watched
        .iter()
        .map(|p| {
            let tag = if *p == c {
                "C"
            } else if *p == c3 {
                "c3"
            } else if *p == dd {
                "D"
            } else if *p == tr {
                "temp rail"
            } else if *p == tb {
                "temp block"
            } else if *p == power {
                "power"
            } else if *p == slope {
                "slope"
            } else if *p == cg {
                "C's support"
            } else {
                "power's neighbour"
            };
            format!("{tag} ({},{},{})", p[0], p[1], p[2])
        })
        .collect();
    let before = read(&mut srv, &watched)?;
    // the cells under replay emptied without updates; the temporary cells must be free
    for (p, s) in [(tr, &before[3]), (tb, &before[4]), (power, &before[5])] {
        if s != "minecraft:air" {
            bail!("lane {x}: ({},{},{}) holds {s} in the module: not free for the recipe", p[0], p[1], p[2]);
        }
    }
    let full = case == "full";
    let (s_slope, s_sg, s_cg) = (stated(slope)?, stated(sg)?, stated(cg)?);
    let mut emptied = vec![c, dd, c3];
    if full {
        emptied.extend([cg, slope, sg]);
    }
    for p in emptied {
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:air strict", p[0], p[1] + DY, p[2]))?;
    }
    let at = |p: [i32; 3]| format!("{} {} {}", p[0], p[1] + DY, p[2]);
    let test: Vec<(String, String)> = vec![
        ("c3 as stored".into(), format!("setblock {} {s_c3}", at(c3))),
        ("D as stored".into(), format!("setblock {} {s_d}", at(dd))),
        ("the temporary glass".into(), format!("setblock {} minecraft:glass", at(tb))),
        ("the temporary rail".into(), format!("setblock {} minecraft:rail", at(tr))),
        ("the temporary redstone block".into(), format!("setblock {} minecraft:redstone_block", at(power))),
        ("C as stored".into(), format!("setblock {} {s_c}", at(c))),
        ("remove the temporary rail".into(), format!("setblock {} minecraft:air", at(tr))),
        ("remove the power".into(), format!("setblock {} minecraft:air", at(power))),
        ("remove the temporary glass".into(), format!("setblock {} minecraft:air", at(tb))),
    ];
    let steps: Vec<(String, String)> = match case {
        "test" => test,
        // (ironfarm 02:13) the slope's glass, a temporary rail on it, the slope, the rail removed,
        // C's support glass, then the test
        "full" => {
            let mut v: Vec<(String, String)> = vec![
                ("the slope's high-side block as stored".into(), format!("setblock {} {s_sg}", at(sg))),
                ("a temporary rail at C's support cell".into(), format!("setblock {} minecraft:rail", at(cg))),
                ("the slope as stored".into(), format!("setblock {} {s_slope}", at(slope))),
                ("remove the temporary rail".into(), format!("setblock {} minecraft:air", at(cg))),
                ("C's support as stored".into(), format!("setblock {} {s_cg}", at(cg))),
            ];
            v.extend(test);
            v
        }
        "ctl-i" => vec![
            ("c3 as stored".into(), format!("setblock {} {s_c3}", at(c3))),
            ("the temporary glass".into(), format!("setblock {} minecraft:glass", at(tb))),
            ("the temporary rail".into(), format!("setblock {} minecraft:rail", at(tr))),
            ("C as stored".into(), format!("setblock {} {s_c}", at(c))),
            ("remove the temporary rail".into(), format!("setblock {} minecraft:air", at(tr))),
            ("remove the temporary glass".into(), format!("setblock {} minecraft:air", at(tb))),
            ("D as stored".into(), format!("setblock {} {s_d}", at(dd))),
        ],
        "ctl-ii" => vec![
            ("c3 as stored".into(), format!("setblock {} {s_c3}", at(c3))),
            ("the temporary glass".into(), format!("setblock {} minecraft:glass", at(tb))),
            ("the temporary rail".into(), format!("setblock {} minecraft:rail", at(tr))),
            ("C as stored".into(), format!("setblock {} {s_c}", at(c))),
            ("D as stored".into(), format!("setblock {} {s_d}", at(dd))),
            ("remove the temporary rail".into(), format!("setblock {} minecraft:air", at(tr))),
            ("remove the temporary glass".into(), format!("setblock {} minecraft:air", at(tb))),
        ],
        other => bail!("--case test, full, ctl-i, ctl-ii or all, got {other}"),
    };
    println!("  lane {x}, {case}: before (as pasted): {}", names.iter().zip(&before).map(|(n, s)| format!("{n} {}", short(s))).collect::<Vec<_>>().join("; "));
    for (k, (label, cmd)) in steps.iter().enumerate() {
        let out = srv.cmd(cmd)?;
        anyhow::ensure!(out.contains("Changed the block"), "lane {x}, {case}, step {} ({label}): {out}", k + 1);
        srv.step(2)?;
        let now = read(&mut srv, &watched)?;
        println!(
            "    step {} {label}: C {}; c3 {}; D {}; slope {}; temp rail {}; temp block {}; power {}",
            k + 1,
            short(&now[0]),
            short(&now[1]),
            short(&now[2]),
            short(&now[6]),
            short(&now[3]),
            short(&now[4]),
            short(&now[5])
        );
    }
    let end = read(&mut srv, &watched)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    let shape = |s: &str| s.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("?").to_string();
    let c_end = shape(&end[0]);
    let temps_gone = end[3] == "minecraft:air" && end[4] == "minecraft:air" && end[5] == "minecraft:air";
    let slope_ok = shape(&end[6]) == shape(&s_slope);
    let ok = match case {
        "test" => c_end == "north_west" && temps_gone && shape(&end[1]) == shape(&s_c3) && shape(&end[2]) == shape(&s_d),
        "full" => c_end == "north_west" && temps_gone && shape(&end[1]) == shape(&s_c3) && shape(&end[2]) == shape(&s_d) && slope_ok && end[7] == s_cg,
        _ => c_end == "south_west" && temps_gone,
    };
    let want = match case {
        "test" => "C north_west, c3 and D as stored, nothing temporary left",
        "full" => "the slope as stored, C's support as stored, C north_west, c3 and D as stored, nothing temporary left",
        _ => "C south_west",
    };
    println!(
        "  lane {x}, {case}: END C {} (c3 {}, D {}, slope {}; temporary cells {}): expected {want}: {}",
        c_end,
        shape(&end[1]),
        shape(&end[2]),
        shape(&end[6]),
        if temps_gone { "gone" } else { "LEFT" },
        if ok { "HELD" } else { "NOT HELD" }
    );
    // the other power neighbours as they ended (nothing must have changed there)
    let changed: Vec<String> = watched.iter().enumerate().skip(8).filter(|(i, _)| end[*i] != before[*i]).map(|(i, p)| format!("({},{},{}) {} -> {}", p[0], p[1], p[2], short(&before[i]), short(&end[i]))).collect();
    if !changed.is_empty() {
        println!("  lane {x}, {case}: the power cell's neighbours CHANGED: {}", changed.join("; "));
    }
    Ok((c_end, ok && changed.is_empty()))
}

pub fn rail_steps(version: &str, file: &std::path::Path, lanes: &[i32], case: &str, power: Option<[i32; 3]>) -> Result<()> {
    let cases: Vec<&str> = if case == "all" { vec!["full", "ctl-i", "ctl-ii"] } else { vec![case] };
    println!(
        "== rail-steps on the official {version} server: {} pasted (rails strict, fluids scheduled), per lane x in {lanes:?}: C = (x,24,7), D = (x,24,8), c3 = (x-1,24,7) emptied without updates, then each recipe as plain setblocks (2 gt apart); the power cell {}",
        file.display(),
        power.map(|p| format!("at ({},{},{}) (relative to the lane: x + {})", p[0], p[1], p[2], p[0])).unwrap_or_else(|| "(x,25,7), above C".to_string())
    );
    let mut failed = Vec::new();
    for &x in lanes {
        let p = power.map(|p| [x + p[0], p[1], p[2]]).unwrap_or([x, 25, 7]);
        for c in &cases {
            let (_, ok) = replay(version, file, x, c, p)?;
            if !ok {
                failed.push(format!("lane {x} {c}"));
            }
        }
    }
    println!("  every expectation: {}", if failed.is_empty() { "HELD".to_string() } else { format!("NOT HELD in {}", failed.join(", ")) });
    anyhow::ensure!(failed.is_empty(), "rail-steps: {} expectation(s) not held", failed.len());
    Ok(())
}

fn cell(s: &str) -> Result<[i32; 3]> {
    let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
    anyhow::ensure!(v.len() == 3, "x,y,z, got {s}");
    Ok([v[0], v[1], v[2]])
}

/// The rails among a cell's twelve neighbours a rail can connect through (the four horizontal sides
/// at its own height, one up and one down): RailState's potential connections, counted on the read-
/// back states (any rail block there counts).
fn rail_neighbours(srv: &mut VanillaServer, p: [i32; 3]) -> Result<usize> {
    let mut cells = Vec::new();
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for dy in [-1, 0, 1] {
            cells.push([p[0] + dx, p[1] + dy, p[2] + dz]);
        }
    }
    Ok(read(srv, &cells)?.iter().filter(|s| s.contains("rail")).count())
}

/// (farmopt 02:19: b8949c96's two cells) A free-form replay: `--script "STEP;STEP;…"` with
/// `empty x,y,z` (removed without updates before the replay, in order), `set x,y,z=STATE` and
/// `air x,y,z` (plain setblocks), `wait N` (gt), `expect x,y,z=SHAPE` (checked at the end) and
/// `count x,y,z` (that cell's rail neighbours, the RailState potential connections, printed at the
/// end); every cell named is watched and printed after every step, plus `--watch` cells.
pub fn rail_script(version: &str, file: &std::path::Path, script: &str, extra: &[[i32; 3]]) -> Result<()> {
    println!("== rail-steps --script on the official {version} server: {} pasted (rails strict, fluids scheduled); {script}", file.display());
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), with_bot: false, tick_rate: 1000, keep_dir: true, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, file)?;
    srv.step(5)?;
    let mut watched: Vec<[i32; 3]> = extra.to_vec();
    let mut steps: Vec<(String, String)> = Vec::new();
    let mut expects: Vec<([i32; 3], String)> = Vec::new();
    let mut counts: Vec<[i32; 3]> = Vec::new();
    let mut empties: Vec<[i32; 3]> = Vec::new();
    let mut requires: Vec<([i32; 3], String)> = Vec::new();
    let mut watch = |p: [i32; 3], w: &mut Vec<[i32; 3]>| {
        if !w.contains(&p) {
            w.push(p);
        }
    };
    for s in script.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (verb, rest) = s.split_once(' ').unwrap_or((s, ""));
        match verb {
            "empty" => {
                let p = cell(rest)?;
                watch(p, &mut watched);
                empties.push(p);
            }
            "require" => {
                // (G1 on 7354e3025) a precondition, not an override: the cell's state as pasted
                let (p, st) = rest.split_once('=').ok_or_else(|| anyhow::anyhow!("require x,y,z=STATE, got {s}"))?;
                let p = cell(p)?;
                watch(p, &mut watched);
                requires.push((p, st.trim().to_string()));
            }
            "set" => {
                let (p, st) = rest.split_once('=').ok_or_else(|| anyhow::anyhow!("set x,y,z=STATE, got {s}"))?;
                let p = cell(p)?;
                watch(p, &mut watched);
                steps.push((s.to_string(), format!("setblock {} {} {} {}", p[0], p[1] + DY, p[2], st.trim())));
            }
            "air" => {
                let p = cell(rest)?;
                watch(p, &mut watched);
                steps.push((s.to_string(), format!("setblock {} {} {} minecraft:air", p[0], p[1] + DY, p[2])));
            }
            "wait" => steps.push((s.to_string(), String::new())),
            "expect" => {
                let (p, sh) = rest.split_once('=').ok_or_else(|| anyhow::anyhow!("expect x,y,z=SHAPE, got {s}"))?;
                let p = cell(p)?;
                watch(p, &mut watched);
                expects.push((p, sh.trim().to_string()));
            }
            "count" => {
                let p = cell(rest)?;
                watch(p, &mut watched);
                counts.push(p);
            }
            other => bail!("--script: unknown step {other:?} (empty, require, set, air, wait, expect, count)"),
        }
    }
    let label = |p: &[i32; 3]| format!("({},{},{})", p[0], p[1], p[2]);
    let before = read(&mut srv, &watched)?;
    println!("  as pasted: {}", watched.iter().zip(&before).map(|(p, s)| format!("{} {}", label(p), short(s))).collect::<Vec<_>>().join("; "));
    for (p, want) in &requires {
        let i = watched.iter().position(|w| w == p).unwrap_or(0);
        anyhow::ensure!(&before[i] == want, "require {}: the module holds {} there, not {want}: the run fails", label(p), before[i]);
        println!("  require {}: {want}: as pasted", label(p));
    }
    for p in &empties {
        // (ironspec 04:07) an override must change its cell: emptying an air cell fails (state the
        // precondition with `require x,y,z=minecraft:air` instead)
        let out = srv.cmd(&format!("setblock {} {} {} minecraft:air strict", p[0], p[1] + DY, p[2]))?;
        anyhow::ensure!(out.contains("Changed the block"), "empty {}: changed nothing ({}): a bench override must change its cell; use `require` for a precondition: the run fails", label(p), out.trim());
    }
    for (k, (s, cmd)) in steps.iter().enumerate() {
        if cmd.is_empty() {
            let n: u32 = s.trim_start_matches("wait").trim().parse()?;
            srv.step(n)?;
        } else {
            let out = srv.cmd(cmd)?;
            anyhow::ensure!(out.contains("Changed the block"), "step {} ({s}): {out}", k + 1);
            srv.step(2)?;
        }
        let now = read(&mut srv, &watched)?;
        println!("    step {} {s}: {}", k + 1, watched.iter().zip(&now).map(|(p, st)| format!("{} {}", label(p), short(st))).collect::<Vec<_>>().join("; "));
    }
    let end = read(&mut srv, &watched)?;
    let shape = |s: &str| s.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("?").to_string();
    let mut failed = Vec::new();
    for (p, want) in &expects {
        let i = watched.iter().position(|w| w == p).unwrap_or(0);
        let got = if want.contains(':') { end[i].clone() } else { shape(&end[i]) };
        let ok = &got == want;
        println!("  expect {} {want}: {} ({})", label(p), if ok { "HELD" } else { "NOT HELD" }, short(&end[i]));
        if !ok {
            failed.push(label(p));
        }
    }
    for p in &counts {
        println!("  {}: {} rail neighbour(s) (the four sides at y-1..y+1)", label(p), rail_neighbours(&mut srv, *p)?);
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    anyhow::ensure!(failed.is_empty(), "rail-steps --script: {} expectation(s) not held", failed.len());
    Ok(())
}
