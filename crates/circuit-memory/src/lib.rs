//! Contraption study 5, track memory (docs/circuits.md): `mctas circuit memory <args...>`. OWNED BY
//! THE TRACK (the tff explorer runs both).
//!
//! * [`specs`]: census specs for rs-latch, d-latch and counter N mirroring the judge's stimuli, run
//!   on track tff's census engine (`circuit_tff::census`).
//! * [`ring`]: dropper-ring counters around the input cell (`circuit memory ring --n N`).

pub mod items;
pub mod ring;
pub mod seqbench;
pub mod specs;
pub mod stamp;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit memory", about = "Memory circuits (track memory): census for RS / D latches and counters")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The item counter: two droppers facing each other hold N items, each pulse fires only the
    /// one the copper bulb phase does not hold, so each runs empty once every N pulses. 39 blocks
    /// for every N up to 576 - the only counter whose size does not grow with N.
    Items {
        /// Counter sizes, comma-separated (2..=576).
        #[arg(long, value_delimiter = ',')]
        n: Vec<usize>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Dropper-ring counters: the smallest cycles dropper -> hopper -> ... whose droppers the
    /// input cell fires (directly or through dust next to it), one design per ring (`--keep`).
    Ring {
        /// Counter sizes, comma-separated.
        #[arg(long, value_delimiter = ',')]
        n: Vec<usize>,
        /// Most dust cells next to the input.
        #[arg(long, default_value_t = 2)]
        max_dust: usize,
        /// Designs kept per N, smallest first.
        #[arg(long, default_value_t = 3)]
        keep: usize,
        /// Copper bulb T flip-flops chained after the comparator (counter N * 2^bulbs).
        #[arg(long, default_value_t = 0)]
        bulbs: usize,
        /// More rings, each around a stone the previous ring's comparator powers (comma-separated
        /// sizes: counter N * THEN1 * THEN2 ...).
        #[arg(long, value_delimiter = ',')]
        then: Vec<usize>,
        /// With --bulbs: judge as a divider (the last comparator toggles once every N * 2^(K-1)
        /// pulses) instead of a level counter N * 2^K.
        #[arg(long)]
        divider: bool,
        /// Write the smallest candidates without judging them (by default each is judged in
        /// redsim, smallest first, and only passing ones are written).
        #[arg(long)]
        unchecked: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run a design under this track's census spec for its kind (`circuit:` header) and print the
    /// output's rest state, change frames, samples and the spec's verdict (debugging the census).
    Check { design: PathBuf, #[arg(long, default_value_t = 40)] gap: u32 },
    /// The carousel counter's bench (designs/briefs/farm-iron-carousel.md, part 1): golem pulses
    /// on the 100-gt slot grid from a reactive farm (missed slots, early and late pulses), the 7
    /// lines read every tick (one advance per pulse within --tc, the cycle order kept, the fallback
    /// at 100 + m), then the same run with chunk reloads at random ticks, which must match it.
    CounterBench {
        design: PathBuf,
        #[arg(long, default_value_t = 100_000)]
        pulses: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 20)]
        restarts: usize,
        /// Golem pulse lengths `MIN..MAX` (gt).
        #[arg(long, default_value = "2..15")]
        width: String,
        #[arg(long, default_value_t = 5)]
        jitter: u32,
        #[arg(long, default_value_t = 0.05)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        #[arg(long, default_value_t = 40)]
        ready: u32,
        #[arg(long, default_value_t = 40)]
        tc: u32,
        /// The fallback's m `MIN..MAX` (gt past 100).
        #[arg(long, default_value = "6..10")]
        m: String,
        #[arg(long)]
        dump: Option<PathBuf>,
        #[arg(long, default_value_t = 20_000)]
        dump_ticks: u32,
        /// Write a copy of the design carrying the base run's input as its `stimulus:` line: `mctas
        /// circuit trace-compare <copy>` is the real game's check of this very run.
        #[arg(long)]
        stimulus_out: Option<PathBuf>,
        /// Quarter turns the same run is repeated on, comma-separated (each must give the same
        /// advances tick for tick); empty for none.
        #[arg(long, default_value = "1,2,3")]
        turns: String,
    },
    /// Stamp the tunable-group headers (`group:`, `group title:`, `group note:`) on counter
    /// designs by their name: bulb-chain, dropper-ring, ring-bulbs, ring-chain, ring-divider (the
    /// catalog shows each group as one card). The ring generator stamps its own designs.
    StampGroups {
        #[arg(long)]
        dry_run: bool,
        files: Vec<PathBuf>,
    },
    /// Census: every filling of k cells of a box (x 0..A, y 0..B, z 0..C) with palette states,
    /// the named inputs redstone blocks in air cells (`--inputs 'r=-1,0,0;s=3,0,0'`), judged on
    /// redsim with the kind's stimulus (`rs-latch`, `d-latch`, `counter N`); designs + summary.tsv.
    Census {
        /// `rs-latch`, `d-latch` or `counter N`.
        #[arg(long)]
        kind: String,
        #[arg(long = "box")]
        dims: String,
        /// `name=x,y,z;...` in the kind's input names (r, s / d, e / one input for counters).
        #[arg(long)]
        inputs: String,
        #[arg(long, default_value = "1..3")]
        k: String,
        #[arg(long, default_value = "stone,dust,torch,repeater1,comparator")]
        palette: String,
        #[arg(long, default_value_t = 40)]
        gap: u32,
        #[arg(long, default_value_t = 160)]
        jobs: usize,
        /// Stone under every placed dust, diode and standing torch (counted): search one layer
        /// of components with `--box Ax1xC` placed at y=1.
        #[arg(long)]
        auto_floor: bool,
        #[arg(long)]
        out: PathBuf,
    },
}

/// `mctas circuit memory <args...>`.
pub fn run(args: Vec<String>) -> Result<()> {
    let a = match Args::try_parse_from(std::iter::once("mctas circuit memory".to_string()).chain(args)) {
        Ok(a) => a,
        Err(e) if matches!(e.kind(), clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion) => {
            print!("{e}");
            return Ok(());
        }
        Err(e) => bail!("{e}"),
    };
    match a.cmd {
        Cmd::Items { n, out } => items::run(&n, &out)?,
        Cmd::Ring { n, max_dust, keep, bulbs, then, divider, unchecked, out } => ring::run(&n, max_dust, keep, bulbs, &then, divider, !unchecked, &out)?,
        Cmd::StampGroups { dry_run, files } => stamp::run(&files, dry_run)?,
        Cmd::CounterBench { design, pulses, seed, restarts, width, jitter, miss, burst, ready, tc, m, dump, dump_ticks, stimulus_out, turns } => {
            let turns: Vec<u8> = turns.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.parse::<u8>().with_context(|| format!("--turns {s:?}"))).collect::<Result<_>>()?;
            let range = |s: &str| -> Result<(i64, i64)> {
                let (a, b) = s.split_once("..").ok_or_else(|| anyhow!("{s:?}: MIN..MAX"))?;
                Ok((a.trim().parse()?, b.trim().parse()?))
            };
            let (w0, w1) = range(&width)?;
            let o = seqbench::Opts { pulses, seed, restarts, width: (w0 as u32, w1 as u32), jitter, miss, burst, ready, tc, m: range(&m)?, dump, dump_ticks, stimulus_out, turns, ..Default::default() };
            if !seqbench::bench(&design, &o)? {
                bail!("the counter fails its bench");
            }
        }
        Cmd::Check { design, gap } => {
            let c = circuit_tff::model::Circuit::load(&design)?;
            let words: Vec<String> = std::iter::once(c.kind.clone()).chain(c.args.iter().cloned()).collect();
            let words: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
            let spec: Box<dyn circuit_tff::census::Spec> = match words.as_slice() {
                ["rs-latch"] => Box::new(specs::RsSpec { gap }),
                ["d-latch"] => Box::new(specs::DSpec { gap }),
                ["counter", n] => Box::new(specs::CounterSpec { n: n.parse().context("counter N")?, gap }),
                ["dff"] => Box::new(specs::DffSpec { gap }),
                ["tff-reset"] => Box::new(specs::TffResetSpec { gap }),
                _ => bail!("kind {words:?}: rs-latch | d-latch | counter N | dff | tff-reset"),
            };
            let mut inputs = c.inputs.clone();
            inputs.sort_by(|a, b| a.name.cmp(&b.name));
            let census = circuit_tff::census::Census {
                inputs: inputs.iter().map(|i| redsim::BlockPos::new(i.pos[0], i.pos[1], i.pos[2])).collect(),
                fixed: Vec::new(),
                fixed_be: Vec::new(),
                free: Vec::new(),
                palette: Vec::new(),
                palette_be: Vec::new(),
                k_min: 0,
                k_max: 0,
                outputs: Some(c.outputs.iter().map(|(_, p)| redsim::BlockPos::new(p[0], p[1], p[2])).collect()),
                jobs: 1,
                margin: 3,
                auto_floor: false,
            };
            let mut blocks = Vec::new();
            for (p, s) in &c.design.blocks {
                blocks.push((redsim::BlockPos::new(p[0], p[1], p[2]), redsim::Block::parse(s).map_err(|e| anyhow!(e))?));
            }
            let be: Vec<(redsim::BlockPos, String)> = c.design.block_entities.iter().map(|(p, s)| (redsim::BlockPos::new(p[0], p[1], p[2]), s.clone())).collect();
            match census.simulate_debug(&blocks, &be, spec.as_ref()) {
                Ok(lines) => {
                    for l in lines {
                        println!("{l}");
                    }
                }
                Err(e) => println!("rejected: {e}"),
            }
        }
        Cmd::Census { kind, dims, inputs, k, palette, gap, jobs, auto_floor, out } => {
            let d: Vec<i32> = dims.split('x').map(|t| t.parse::<i32>()).collect::<Result<_, _>>().context("--box AxBxC")?;
            if d.len() != 3 {
                bail!("--box AxBxC");
            }
            let mut free = Vec::new();
            for y in 0..d[1] {
                for z in 0..d[2] {
                    for x in 0..d[0] {
                        free.push(redsim::BlockPos::new(x, y, z));
                    }
                }
            }
            let mut named: Vec<(String, redsim::BlockPos)> = Vec::new();
            for part in inputs.split(';').filter(|s| !s.trim().is_empty()) {
                let (n, p) = part.split_once('=').ok_or_else(|| anyhow!("--inputs name=x,y,z;..."))?;
                let v: Vec<i32> = p.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().context("--inputs")?;
                named.push((n.trim().to_string(), redsim::BlockPos::new(v[0], v[1], v[2])));
            }
            // the judge orders inputs by name
            named.sort_by(|a, b| a.0.cmp(&b.0));
            let words: Vec<&str> = kind.split_whitespace().collect();
            let spec: Box<dyn circuit_tff::census::Spec> = match words.as_slice() {
                ["rs-latch"] => Box::new(specs::RsSpec { gap }),
                ["d-latch"] => Box::new(specs::DSpec { gap }),
                ["counter", n] => Box::new(specs::CounterSpec { n: n.parse().context("counter N")?, gap }),
                ["dff"] => Box::new(specs::DffSpec { gap }),
                ["tff-reset"] => Box::new(specs::TffResetSpec { gap }),
                _ => bail!("--kind rs-latch | d-latch | counter N | dff | tff-reset"),
            };
            let want_inputs = match words[0] {
                "rs-latch" => vec!["r", "s"],
                "d-latch" => vec!["d", "e"],
                "dff" => vec!["clk", "d"],
                "tff-reset" => vec!["r", "t"],
                _ => vec![""],
            };
            let names: Vec<&str> = named.iter().map(|(n, _)| n.as_str()).collect();
            if names != want_inputs && !(want_inputs == vec![""] && names.len() == 1) {
                bail!("--inputs for {kind}: {want_inputs:?}, got {names:?}");
            }
            let (pal, pal_be) = circuit_tff::cli::parse_palette(&palette)?;
            let (k_min, k_max) = circuit_tff::cli::parse_k(&k)?;
            let free: Vec<redsim::BlockPos> = free.into_iter().filter(|p| !named.iter().any(|(_, q)| q == p)).collect();
            eprintln!("{kind}: {} free cells, {} palette states, k {k_min}..={k_max}", free.len(), pal.len());
            let c = circuit_tff::census::Census {
                inputs: named.iter().map(|(_, p)| *p).collect(),
                fixed: Vec::new(),
                fixed_be: Vec::new(),
                free,
                palette: pal,
                palette_be: pal_be,
                k_min,
                k_max,
                outputs: None,
                jobs,
                margin: 3,
                auto_floor,
            };
            let found = c.run(spec.as_ref(), true);
            std::fs::create_dir_all(&out)?;
            let mut rows: Vec<(String, usize, u32, String)> = Vec::new();
            let short = words.join("");
            for f in &found {
                let fam = circuit_tff::cli::family_of(&f.blocks);
                let h = circuit_tff::cli::fnv(&f.key);
                let name = format!("{short}-{}-{}b-{}", if fam.is_empty() { "none".to_string() } else { fam.replace('+', "-") }, f.blocks.len(), &h[..8]);
                let named_in: Vec<(String, redsim::BlockPos)> = named.iter().map(|(n, p)| (if want_inputs == vec![""] { String::new() } else { n.clone() }, *p)).collect();
                let des = circuit_tff::cli::found_design(&kind, &name, f, &named_in);
                des.save(&out.join(format!("{name}.txt")))?;
                rows.push((fam, f.blocks.len(), f.score, name));
            }
            rows.sort();
            let mut tsv = String::from("family\tblocks\tscore\tdesign\n");
            for (fam, b, s, n) in &rows {
                tsv.push_str(&format!("{fam}\t{b}\t{s}\t{n}\n"));
            }
            std::fs::write(out.join("summary.tsv"), &tsv)?;
            println!("{} {kind} types found (summary.tsv in {})", rows.len(), out.display());
            for (fam, b, s, n) in rows.iter().take(30) {
                println!("  {b} blocks, {fam}, score {s}: {n}");
            }
        }
    }
    Ok(())
}
