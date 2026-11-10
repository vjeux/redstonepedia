//! Contraption study 5, track memory (docs/circuits.md): `mctas circuit memory <args...>`. OWNED BY
//! THE TRACK (the tff explorer runs both).
//!
//! * [`specs`]: census specs for rs-latch, d-latch and counter N mirroring the judge's stimuli, run
//!   on track tff's census engine (`circuit_tff::census`).
//! * [`ring`]: dropper-ring counters around the input cell (`circuit memory ring --n N`).

pub mod compose;
pub mod items;
pub mod ring;
pub mod counter8;
pub mod loopbench;
pub mod loopgen;
pub mod loopmodel;
pub mod schedbench;
pub mod schedgen;
pub mod schedmodel;
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
    /// One design from several placed at offsets (and turned): `PATH@DX,DY,DZ[,TURNS][=PREFIX]`
    /// each; a cell two modules fill must hold the same block; ports renamed with the prefix
    /// (`input` -> `input.PREFIX`, `output.n` -> `output.PREFIX-n`) or dropped (`=-`).
    Compose {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "composed")]
        name: String,
        parts: Vec<String>,
    },
    /// The carousel's nightly scheduler core, generated (crates/circuit-memory/src/schedgen.rs):
    /// the head-pointer ring, the 7 lane cells and their OR bus, the visit edge, the hump window,
    /// STEP, the level ring.
    SchedGen {
        #[arg(long)]
        out: PathBuf,
        /// The module designs (vcell.txt, vedge-c2.txt, fedge.txt, l-ring.txt).
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-scheduler")]
        modules: PathBuf,
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-scheduler/offdelay-k3.txt")]
        offdelay: PathBuf,
    },
    /// The single-lane loop scheduler's bench on redsim (crates/circuit-memory/src/loopbench.rs).
    LoopBench {
        design: PathBuf,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.01)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        #[arg(long, default_value_t = 13_670)]
        dusk: u64,
        #[arg(long, default_value_t = 22_330)]
        end: u64,
        #[arg(long, default_value_t = 0)]
        game: u32,
    },
    /// The single-lane loop's nightly scheduler, generated (crates/circuit-memory/src/loopgen.rs).
    LoopGen {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-scheduler")]
        modules: PathBuf,
        #[arg(long, default_value = "designs/circuits/pulsemult/archive/off-delay-2stage-k1.txt")]
        offdelay: PathBuf,
    },
    /// The single-lane loop's sleep scheduling, modeled (crates/circuit-memory/src/loopmodel.rs).
    LoopModel {
        #[arg(long, default_value = "every")]
        policy: String,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.01)]
        miss: f64,
        #[arg(long, default_value_t = 4)]
        every: u32,
        #[arg(long, default_value_t = 13_670)]
        dusk: u64,
        #[arg(long, default_value_t = 22_330)]
        end: u64,
        /// The visit's length (the cart leaves the coming-back batch until it rejoins), `A..B` gt.
        #[arg(long, default_value = "400..430")]
        visit: String,
        /// The nap after the visit starts, `A..B` gt.
        #[arg(long, default_value = "115..135")]
        nap: String,
        /// `tail` (rejoins at the queue's tail) or `place` (in its batch's place).
        #[arg(long, default_value = "tail")]
        rejoin: String,
        /// A batch's way back from the station to the tail, gt.
        #[arg(long, default_value_t = 60)]
        out: u64,
        #[arg(long)]
        verbose: bool,
    },
    /// Carousel counter v8 generated from v7 (crates/circuit-memory/src/counter8.rs): all 7 lines
    /// out of the south face at the paired-lane frame's wiring columns, two heights.
    CounterV8 {
        #[arg(long, default_value = "designs/farms/iron/parts/carousel-counter/carousel-counter-v7.txt")]
        v7: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// The nightly scheduler's bench on redsim (crates/circuit-memory/src/schedbench.rs): the
    /// counter's lines driven ideal through days and nights with missed slots, the night window,
    /// visits read from the design, every slot's willingness; per villager its naps, nap
    /// day-time range and worst gap window.
    SchedBench {
        design: PathBuf,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.01)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        #[arg(long, default_value_t = 13_670)]
        dusk: u64,
        #[arg(long, default_value_t = 22_330)]
        end: u64,
        /// Replay the run's first N days in the real game, every output probe compared every tick.
        #[arg(long, default_value_t = 0)]
        game: u32,
        /// Print every change of the lines and outputs over `FROM..TO` (ticks of the run).
        #[arg(long)]
        watch: Option<String>,
        /// Extra cells `x,y,z x,y,z ...` whose signal --watch prints.
        #[arg(long, default_value = "")]
        probes: String,
    },
    /// The sleep routine's nightly scheduler, its policy judged on timing alone (no circuit): the
    /// counter's advances with misses, a visit per dispatch (nap, length, one bed), every slot's
    /// willingness (gameTime - LAST_SLEPT < 24,000); per villager its naps a night, nap day-time
    /// range, worst gap window and slots lost (crates/circuit-memory/src/schedmodel.rs).
    SchedModel {
        /// dusk21, crt, round or urgent.
        #[arg(long, default_value = "round")]
        policy: String,
        #[arg(long, default_value_t = 60)]
        days: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.05)]
        miss: f64,
        #[arg(long, default_value_t = 0.3)]
        burst: f64,
        /// A dispatch every this many advances.
        #[arg(long, default_value_t = 5)]
        every: u32,
        #[arg(long, default_value_t = 12_000)]
        dusk: u64,
        #[arg(long, default_value_t = 24_010)]
        dawn: u64,
        /// The nap's delay after the dispatch, `MIN..MAX` gt.
        #[arg(long, default_value = "135..185")]
        nap: String,
        /// The visit's length, `MIN..MAX` gt.
        #[arg(long, default_value = "291..486")]
        visit: String,
        #[arg(long, default_value_t = 40)]
        ready: u64,
        /// Step the dispatch phase on every advance, day and night.
        #[arg(long)]
        phase_always: bool,
        /// `block`: a block starting within this many gt of dusk runs 22 visits and moves the head.
        #[arg(long, default_value_t = 200)]
        rotate: u64,
        /// `pring`: the window's last day time for a dispatch.
        #[arg(long, default_value_t = 22_965)]
        pring_end: u64,
        /// The yard queues a napper dispatched while the bed is busy.
        #[arg(long)]
        queue: bool,
        /// `pring`: hold a dispatch while the yard is busy (the pointer stays).
        #[arg(long)]
        hold: bool,
        /// The share of visits whose nap (and the yard's idle) slips 40..200 gt (the HOME re-take).
        #[arg(long, default_value_t = 0.0)]
        retake: f64,
        /// The share of visits drawn from the tail [--visit-body, the longest] (0: uniform).
        #[arg(long, default_value_t = 0.0)]
        visit_tail: f64,
        #[arg(long, default_value_t = 330)]
        visit_body: u32,
        #[arg(long)]
        verbose: bool,
    },
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
        Cmd::Compose { out, name, parts } => compose::compose(&out, &name, &parts)?,
        Cmd::SchedGen { out, modules, offdelay } => schedgen::generate(&out, &modules, &offdelay)?,
        Cmd::CounterV8 { v7, out } => counter8::generate(&v7, &out)?,
        Cmd::LoopGen { out, modules, offdelay } => loopgen::generate(&out, &modules, &offdelay)?,
        Cmd::LoopBench { design, days, seed, miss, burst, dusk, end, game } => {
            let o = loopbench::Opts { days, seed, miss, burst, dusk, end, game_days: game, ..Default::default() };
            if !loopbench::bench(&design, &o)? {
                bail!("the loop scheduler fails its bench");
            }
        }
        Cmd::LoopModel { policy, days, seed, miss, every, dusk, end, visit, nap, rejoin, out, verbose } => {
            let pair = |s: &str| -> Result<(u32, u32)> {
                let (a, b) = s.split_once("..").unwrap_or((s, s));
                Ok((a.parse()?, b.parse()?))
            };
            let o = loopmodel::Opts { policy, days, seed, miss, every, dusk, end, visit: pair(&visit)?, nap: pair(&nap)?, rejoin, out, verbose };
            if !loopmodel::run(&o) {
                bail!("the loop schedule loses slots or naps");
            }
        }
        Cmd::SchedBench { design, days, seed, miss, burst, dusk, end, game, watch, probes } => {
            let watch = match watch {
                Some(s) => {
                    let (a, z) = s.split_once("..").ok_or_else(|| anyhow::anyhow!("--watch FROM..TO"))?;
                    Some((a.parse()?, z.parse()?))
                }
                None => None,
            };
            let probes: Vec<[i32; 3]> = probes
                .split_whitespace()
                .map(|s| {
                    let v: Vec<i32> = s.split(',').filter_map(|n| n.parse().ok()).collect();
                    [v.first().copied().unwrap_or(0), v.get(1).copied().unwrap_or(0), v.get(2).copied().unwrap_or(0)]
                })
                .collect();
            let o = schedbench::Opts { days, seed, miss, burst, dusk, end, game_days: game, watch, probes, ..Default::default() };
            if !schedbench::bench(&design, &o)? {
                bail!("the scheduler fails its bench");
            }
        }
        Cmd::SchedModel { policy, days, seed, miss, burst, every, dusk, dawn, nap, visit, ready, phase_always, rotate, pring_end, queue, hold, retake, visit_tail, visit_body, verbose } => {
            let range = |s: &str| -> Result<(u32, u32)> {
                let (a, b) = s.split_once("..").ok_or_else(|| anyhow!("{s:?}: MIN..MAX"))?;
                Ok((a.trim().parse()?, b.trim().parse()?))
            };
            let o = schedmodel::Opts { policy, days, seed, miss, burst, every, dusk, dawn, nap: range(&nap)?, visit: range(&visit)?, ready, phase_always, verbose, rotate, pring_end, queue, hold, retake, visit_tail, visit_body };
            if !schedmodel::run(&o)? {
                bail!("the policy loses slots or leaves a villager a night without a nap");
            }
        }
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
