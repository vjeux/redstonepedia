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
    },
    /// The nether-portal bedrock tutorial on video: scout a real random world for the best
    /// drill column, film the whole survival recipe with the real client (the actor mod
    /// playing), and cut it with captions (see `bedrock::video`).
    Video {
        /// World seed (scout only; default: from the clock — a random world).
        #[arg(long)]
        seed: Option<i64>,
        /// Production folder (plan, world, checkpoints, segments, the final webm).
        #[arg(long, default_value = "bedrock-video-production")]
        out: PathBuf,
        /// smoke | scout | record | compose | all.
        #[arg(long, default_value = "all")]
        phase: String,
        /// How the real client is driven: fabric (actor mod under Fabric Loader) | launcher.
        #[arg(long, default_value = "fabric")]
        driver: String,
        /// Resume the shoot from this checkpoint tag (setup, intro, digdown, patch, return,
        /// overworld, layer4..layer0, finale).
        #[arg(long)]
        from: Option<String>,
        #[arg(long, default_value_t = 1280)]
        width: u32,
        #[arg(long, default_value_t = 720)]
        height: u32,
        #[arg(long, default_value_t = 30)]
        fps: u32,
        /// The v2 remake: batch sweep + stay-in-the-Nether remote trigger, tutorial cut.
        #[arg(long)]
        v2: bool,
    },
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
        BedrockCmd::Vanilla { scenario, windows, split } => {
            let sc = Scenario::load(&scenario)?;
            let stance = sc.stance.clone().ok_or_else(|| anyhow::anyhow!("{}: no stance (where the bot stands)", sc.name))?;
            let res = crate::vanilla::confirm(version, &sc, &stance, &windows, split)?;
            for (w, o) in &res {
                println!("{}: vanilla, chord in window {w}{}: {}", sc.name, if split { " (split over two windows)" } else { "" }, format!("{o:?}").to_lowercase());
            }
        }
        BedrockCmd::Video { seed, out, phase, driver, from, width, height, fps, v2 } => {
            crate::video::run(version, crate::video::VideoOptions { seed, out, phase, driver, from, width, height, fps, v2 })?;
        }
    }
    Ok(())
}
