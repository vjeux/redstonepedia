//! `mctas bedrock ...`: the bedrock breaking study's commands (designs/bedrock/NOTES.md).

use crate::{Outcome, Scenario, scan};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

/// `x,y,z` for `--cells`.
fn parse_cell(s: &str) -> Result<[i32; 3], String> {
    let n: Vec<&str> = s.split(',').collect();
    if n.len() != 3 {
        return Err(format!("expected x,y,z, got `{s}`"));
    }
    let mut out = [0i32; 3];
    for (i, part) in n.iter().enumerate() {
        out[i] = part.trim().parse().map_err(|_| format!("`{part}` is not a number"))?;
    }
    Ok(out)
}

#[derive(Subcommand)]
pub enum BedrockCmd {
    /// Try the scenario's chord in every inter-tick window of a range (a fresh sandbox each) and
    /// print, per window, whether the bedrock broke (`broken`), broke after the release actions
    /// (`headless`) or not (`fail`), plus the fraction of windows that work: the odds of a player
    /// pressing the chord at a random moment in that range.
    Scan {
        scenario: PathBuf,
        #[arg(long, default_value_t = 0)]
        from: u32,
        #[arg(long, default_value_t = 12)]
        to: u32,
        /// Print the bridge's answer for each chord action (the click it used).
        #[arg(long)]
        verbose: bool,
    },
    /// One run without the chord: what the swap cell holds in each window, one letter per window
    /// (A armed: extended piston with its retract pending; O empty with a retract pending;
    /// m moving piston, a chord does nothing; . empty; x a piston with nothing pending, a chord
    /// wastes it; ? other), then the counts.
    Watch {
        scenario: PathBuf,
        #[arg(long, default_value_t = 100)]
        to: u32,
        /// Per window: the swap cell's block, every pending block event, and the blocks of the
        /// cells named by `--cells` (each `x,y,z`), to see a machine's phase by phase.
        #[arg(long)]
        verbose: bool,
        /// Extra cells to print with `--verbose`, e.g. `--cells 1,3,0 --cells 0,2,1`.
        #[arg(long, value_parser = parse_cell)]
        cells: Vec<[i32; 3]>,
    },
    /// The scenario on the UNMODIFIED official server: the chord done by a network bot (real
    /// break and place packets) in each given window, the server frozen and stepped one tick at a
    /// time. `--split`: the chord's first action in the window, the rest one tick later.
    Vanilla {
        scenario: PathBuf,
        #[arg(long, value_delimiter = ',', default_value = "0")]
        windows: Vec<u32>,
        #[arg(long)]
        split: bool,
        /// Print, in every window up to `--probe-to`, whether a cell matches a block predicate
        /// (`x,y,z=minecraft:piston[extended=true]`); repeatable.
        #[arg(long)]
        probe: Vec<String>,
        #[arg(long, default_value_t = 40)]
        probe_to: u32,
    },
    /// A machine design (layer format, ports target/swap/press/start) from a simulator scenario:
    /// its blocks, a bedrock roof layer under them, the start kick (`--start x,y,z=state`, the
    /// block placed last) and the press cell.
    Design {
        scenario: PathBuf,
        #[arg(long, value_parser = parse_cell)]
        press: [i32; 3],
        #[arg(long)]
        start: String,
        #[arg(long)]
        out: PathBuf,
        /// More swap modules on the same clock (a batch machine): `target/swap/press` cells, each
        /// `x,y,z`; written as ports `target2`, `swap2`, `press2`, ... (repeatable).
        #[arg(long)]
        module: Vec<String>,
    },
    /// Judge a machine design on redsim (the exact model): the press tried in every window, a miss
    /// then the one-press repair; at the first `--offsets` positions of the door judge, in 4
    /// quarter turns with `--rotations`, mirrored too with `--mirror`.
    Judge {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        offsets: usize,
        #[arg(long)]
        rotations: bool,
        #[arg(long)]
        mirror: bool,
        #[arg(long, default_value_t = 160)]
        ticks: u32,
        #[arg(long, default_value_t = 60)]
        steady: u32,
    },
    /// Shrink a machine design with the door search's edit neighbourhood (removals of 1-3 blocks,
    /// delays, substitutions, moves, swaps, relocations), keeping only edits that pass the redsim
    /// judge at every test position (and orientation): the longest wait for an armed window at most
    /// `--max-gap` ticks, and the repair working.
    Minimize {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Edit families (the door search's names) or `all`.
        #[arg(long, default_value = "all")]
        moves: String,
        #[arg(long, default_value_t = 2)]
        radius: i32,
        #[arg(long, default_value_t = 4)]
        offsets: usize,
        #[arg(long)]
        rotations: bool,
        #[arg(long)]
        mirror: bool,
        #[arg(long, default_value_t = 6)]
        max_gap: u32,
        #[arg(long, default_value_t = 24)]
        threads: usize,
        #[arg(long, default_value_t = 30)]
        plateau: usize,
    },
    /// A scenario from a machine design, its target moved to `--at`: for `bedrock scan|watch` on
    /// the game (no `--dimension`) or `bedrock vanilla` on the official server (`--dimension
    /// the_nether`: the real Nether roof).
    Scenario {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_parser = parse_cell, default_value = "0,5,0")]
        at: [i32; 3],
        #[arg(long)]
        dimension: Option<String>,
        /// Quarter turns (clockwise from above) applied to the design first.
        #[arg(long, default_value_t = 0)]
        turns: i32,
        /// Mirror the design (x -> -x) first.
        #[arg(long)]
        mirror: bool,
        /// Vanilla: the bot plays in survival (Haste II, Efficiency V netherite pickaxe).
        #[arg(long)]
        survival: bool,
        /// Which swap module of a batch machine the chord and the stance are for (1, 2, ...).
        #[arg(long, default_value_t = 1)]
        module: usize,
        /// Build the batch machine with every OTHER module already used (target gone, a piston
        /// facing down left on the hole).
        #[arg(long)]
        others_used: bool,
        /// Add a MISS in window A (the press in a window that is not armed: the piston now faces
        /// the target) and the one-press REPAIR in window B (`--miss-repair A,B`): the same spot,
        /// looking along the swap piston's facing reversed and 10 degrees down, the original piston
        /// placed back.
        #[arg(long, value_delimiter = ',')]
        miss_repair: Vec<u32>,
    },
    /// The machine design on the OFFICIAL server's real Nether roof in every orientation (4 turns,
    /// mirrored too) at every `--at` target position, all on one server: the bot presses in each of
    /// `--windows` (each trial rebuilt from scratch). A directional / positional check.
    VanillaMatrix {
        design: PathBuf,
        #[arg(long, value_parser = parse_cell, required = true)]
        at: Vec<[i32; 3]>,
        #[arg(long, value_delimiter = ',')]
        windows: Vec<u32>,
        #[arg(long)]
        mirror: bool,
    },
    /// Replace a design's clock (the cells inside `--remove`) by every clock of the clock study's
    /// archive whose period is in `--periods`, in every orientation and position inside `--region`,
    /// and judge each on redsim (`bedrock judge`'s rules): the passing grafts, fewest blocks first.
    Graft {
        design: PathBuf,
        #[arg(long, value_parser = parse_box)]
        remove: ([i32; 3], [i32; 3]),
        #[arg(long, value_parser = parse_box)]
        region: ([i32; 3], [i32; 3]),
        #[arg(long, value_delimiter = ',', default_value = "12")]
        periods: Vec<u32>,
        #[arg(long, default_value = "designs/clocks/archive")]
        clocks: PathBuf,
        #[arg(long)]
        mirror: bool,
        #[arg(long, default_value_t = 2)]
        offsets: usize,
        #[arg(long)]
        rotations: bool,
        #[arg(long, default_value_t = 6)]
        max_gap: u32,
        #[arg(long, default_value_t = 32)]
        threads: usize,
        /// Write the best N grafts here (`<n>-<blocks>b.txt`).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 5)]
        keep: usize,
    },
}

/// `x,y,z:x,y,z`.
fn parse_box(s: &str) -> Result<([i32; 3], [i32; 3]), String> {
    let (a, b) = s.split_once(':').ok_or_else(|| format!("expected x,y,z:x,y,z, got `{s}`"))?;
    Ok((parse_cell(a)?, parse_cell(b)?))
}

pub fn run(version: &str, cmd: BedrockCmd) -> Result<()> {
    match cmd {
        BedrockCmd::Scan { scenario, from, to, verbose } => {
            let sc = Scenario::load(&scenario)?;
            let rep = scan(version, &sc, from, to)?;
            println!("{}: baseline without the chord: {:?}", sc.name, rep.baseline);
            let mut good = 0;
            for r in &rep.windows {
                if r.outcome != Outcome::Fail {
                    good += 1;
                }
                println!("window {:>3}  {:<8}  left at chord cell: {}", r.window, format!("{:?}", r.outcome).to_lowercase(), r.left);
                if verbose {
                    for c in &r.chord {
                        println!("      {c}");
                    }
                }
            }
            println!("{good}/{} windows work", rep.windows.len());
        }
        BedrockCmd::Watch { scenario, to, verbose, cells } => {
            let sc = Scenario::load(&scenario)?;
            let w = crate::watch(version, &sc, to, &cells)?;
            let line: String = w.cells.iter().map(|c| c.letter()).collect();
            println!("{}: {line}", sc.name);
            if verbose {
                for row in &w.rows {
                    println!("window {:>3}  {}  {}", row.window, row.state, row.extra.join("  "));
                }
            }
            let count = |c: crate::Cell| w.cells.iter().filter(|x| **x == c).count();
            println!(
                "armed {}  open-armed {}  moving {}  open {}  harmful {}  other {}  (of {})",
                count(crate::Cell::Armed),
                count(crate::Cell::OpenArmed),
                count(crate::Cell::Moving),
                count(crate::Cell::Open),
                count(crate::Cell::Harmful),
                count(crate::Cell::Other),
                w.cells.len()
            );
        }
        BedrockCmd::Vanilla { scenario, windows, split, probe, probe_to } => {
            let mut sc = Scenario::load(&scenario)?;
            for p in &probe {
                let (cell, pred) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("--probe x,y,z=predicate"))?;
                let pos = parse_cell(cell).map_err(|e| anyhow::anyhow!(e))?;
                for k in 0..=probe_to {
                    sc.script.push(crate::Action { what: "probe".into(), pos, state: Some(pred.to_string()), at: k, click: None, face: None, look: None, every: 0, times: 0 });
                }
            }
            let stance = sc.stance.clone().ok_or_else(|| anyhow::anyhow!("{}: no stance (where the bot stands)", sc.name))?;
            let res = crate::vanilla::confirm(version, &sc, &stance, &windows, split)?;
            for (w, o) in &res {
                println!("{}: vanilla, chord in window {w}{}: {}", sc.name, if split { " (split over two windows)" } else { "" }, format!("{o:?}").to_lowercase());
            }
        }
        BedrockCmd::Design { scenario, press, start, out, module } => {
            let sc = Scenario::load(&scenario)?;
            let (pos, state) = start.split_once('=').ok_or_else(|| anyhow::anyhow!("--start x,y,z=state"))?;
            let pos = parse_cell(pos).map_err(|e| anyhow::anyhow!(e))?;
            let name = out.file_stem().and_then(|s| s.to_str()).unwrap_or("machine").to_string();
            let mut d = crate::machine::design_from_scenario(&sc, &name, press, (pos, state.to_string()))?;
            for (i, m) in module.iter().enumerate() {
                let cells: Vec<[i32; 3]> = m.split('/').map(parse_cell).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!(e))?;
                let [t, s, p] = cells[..] else { anyhow::bail!("--module target/swap/press, got `{m}`") };
                for (n, c) in [("target", t), ("swap", s), ("press", p)] {
                    d.ports.insert(format!("{n}{}", i + 2), c);
                }
            }
            d.save(&out)?;
            println!("{}: {} blocks (roof not counted), {} modules", out.display(), crate::machine::Machine::new(d.clone())?.blocks(), crate::machine::module_suffixes(&d).len());
        }
        BedrockCmd::Judge { design, offsets, rotations, mirror, ticks, steady } => judge_cmd(&design, offsets, rotations, mirror, ticks, steady)?,
        BedrockCmd::Minimize { design, out, moves, radius, offsets, rotations, mirror, max_gap, threads, plateau } => {
            let base = mctas::design::Design::load(&design)?;
            let offs: Vec<[i32; 3]> = piston_door::search::judge_offsets(28).into_iter().take(offsets.max(1)).collect();
            let o = crate::machine::MinimizeOptions {
                families: piston_door::search::parse_families(&moves)?,
                radius,
                variants: crate::machine::variants(&offs, rotations, mirror),
                screen: 1,
                max_gap,
                judge: crate::machine::JudgeOptions::default(),
                threads,
                plateau,
            };
            let t0 = std::time::Instant::now();
            let best = crate::machine::minimize(base, &o, &|s| println!("[{:>6.1}s] {s}", t0.elapsed().as_secs_f64()))?;
            let mut best = best;
            best.name = out.file_stem().and_then(|s| s.to_str()).unwrap_or("machine").to_string();
            best.save(&out)?;
            println!("{}: {} blocks", out.display(), crate::machine::Machine::new(best)?.blocks());
        }
        BedrockCmd::Scenario { design, out, at, dimension, turns, mirror, survival, module, others_used, miss_repair } => {
            let d = crate::machine::transformed(&mctas::design::Design::load(&design)?, turns, mirror, [0, 0, 0])?;
            let suffix = if module <= 1 { String::new() } else { module.to_string() };
            let d = if others_used { crate::machine::others_used(&d, &suffix)? } else { d };
            let d = crate::machine::module_view(&d, &suffix)?;
            let mut sc = crate::machine::scenario_from_design(&d, at, dimension.as_deref())?;
            sc.survival = survival;
            if let [miss, repair] = miss_repair[..] {
                crate::machine::add_miss_repair(&mut sc, miss, repair)?;
            }
            std::fs::write(&out, serde_json::to_string_pretty(&sc)?)?;
            println!("{}: {} blocks, target {:?}", out.display(), sc.blocks.len(), sc.target);
        }
        BedrockCmd::Graft { design, remove, region, periods, clocks, mirror, offsets, rotations, max_gap, threads, out_dir, keep } => {
            graft_cmd(&design, remove, region, &periods, &clocks, mirror, offsets, rotations, max_gap, threads, out_dir.as_deref(), keep)?
        }
        BedrockCmd::VanillaMatrix { design, at, windows, mirror } => {
            let d = mctas::design::Design::load(&design)?;
            let mut scs = Vec::new();
            let mut labels = Vec::new();
            for a in &at {
                for t in 0..4 {
                    for mi in if mirror { vec![false, true] } else { vec![false] } {
                        let dv = crate::machine::transformed(&d, t, mi, [0, 0, 0])?;
                        for s in crate::machine::module_suffixes(&dv) {
                            let dm = crate::machine::module_view(&dv, &s)?;
                            scs.push((crate::machine::scenario_from_design(&dm, *a, Some("the_nether"))?, windows.clone()));
                            let which = if s.is_empty() { String::new() } else { format!(" module {s}") };
                            labels.push(format!("at {a:?} turns {t} mirror {mi}{which}"));
                        }
                    }
                }
            }
            let res = crate::vanilla::confirm_many(version, &scs)?;
            let mut last = usize::MAX;
            let mut line = String::new();
            for (i, w, o) in res {
                if i != last {
                    if !line.is_empty() {
                        println!("{line}");
                    }
                    line = format!("{:<40}", labels[i]);
                    last = i;
                }
                line.push_str(&format!(" {w}:{}", format!("{o:?}").to_lowercase()));
            }
            println!("{line}");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn graft_cmd(
    design: &std::path::Path,
    remove: ([i32; 3], [i32; 3]),
    region: ([i32; 3], [i32; 3]),
    periods: &[u32],
    clocks: &std::path::Path,
    mirror: bool,
    offsets: usize,
    rotations: bool,
    max_gap: u32,
    threads: usize,
    out_dir: Option<&std::path::Path>,
    keep: usize,
) -> Result<()> {
    let base = mctas::design::Design::load(design)?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(clocks)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt") && crate::machine::clock_period(p).is_some_and(|n| periods.contains(&n)))
        .collect();
    files.sort();
    let t0 = std::time::Instant::now();
    let cands = crate::machine::grafts(&base, remove, &files, region, mirror)?;
    println!("{} clocks with period in {periods:?}: {} grafts to judge", files.len(), cands.len());
    let offs: Vec<[i32; 3]> = piston_door::search::judge_offsets(28).into_iter().take(offsets.max(1)).collect();
    let vs = crate::machine::variants(&offs, rotations, false);
    let ok = crate::machine::judge_designs(cands, &vs, &crate::machine::JudgeOptions::default(), max_gap, threads);
    println!("{} grafts pass at {} variants in {:.1}s", ok.len(), vs.len(), t0.elapsed().as_secs_f64());
    for (i, (d, g, n)) in ok.iter().take(keep.max(1) * 4).enumerate() {
        println!("{:>3}. {n} blocks, worst gap {g}: {}", i + 1, d.name);
        if let Some(dir) = out_dir
            && i < keep
        {
            std::fs::create_dir_all(dir)?;
            d.save(&dir.join(format!("{:02}-{n}b.txt", i + 1)))?;
        }
    }
    Ok(())
}

fn judge_cmd(design: &std::path::Path, offsets: usize, rotations: bool, mirror: bool, ticks: u32, steady: u32) -> Result<()> {
    let d = mctas::design::Design::load(design)?;
    let offs: Vec<[i32; 3]> = piston_door::search::judge_offsets(28).into_iter().take(offsets.max(1)).collect();
    let o = crate::machine::JudgeOptions { ticks, steady, repair: true };
    let t0 = std::time::Instant::now();
    let mut ok = 0;
    let vs = crate::machine::variants(&offs, rotations, mirror);
    let modules = crate::machine::module_suffixes(&d);
    for (t, mi, off) in &vs {
        let dv = crate::machine::transformed(&d, *t, *mi, *off)?;
        let mut all = true;
        let mut line = String::new();
        for s in &modules {
            let mut views = vec![(crate::machine::module_view(&dv, s)?, "")];
            if modules.len() > 1 {
                views.push((crate::machine::module_view(&crate::machine::others_used(&dv, s)?, s)?, " others used"));
            }
            for (view, how) in views {
                let m = crate::machine::Machine::new(view)?;
                let v = crate::machine::judge(&m, &o);
                all &= v.ok(6);
                if modules.len() > 1 {
                    line.push_str(&format!(" [module {}{how}] ", if s.is_empty() { "1" } else { s }));
                } else {
                    line.push_str(&format!(" {} blocks, ", m.blocks()));
                }
                line.push_str(&v.summary(steady));
            }
        }
        if all {
            ok += 1;
        }
        println!("turns {t} mirror {mi:<5} offset {off:?}:{line}");
    }
    println!("{ok}/{} variants pass (gap <= 6, repair ok{}) in {:.1}s", vs.len(), if modules.len() > 1 { format!(", all {} modules", modules.len()) } else { String::new() }, t0.elapsed().as_secs_f64());
    Ok(())
}
