mod bench;
mod design_cli;
mod door;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use mctas::design::Design;
use mctas::sim::{PlaceMode, SandboxOptions, Worker, WorkerConfig};
use mctas::{bridge, setup, source};
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas", about = "Headless, tick-by-tick Minecraft for redstone and farm engineering")]
struct Cli {
    /// Minecraft version.
    #[arg(long, global = true, default_value = mctas::DEFAULT_VERSION)]
    version: String,
    /// (ironspec Nov 14 01:58 / 02:35, the binary rule) The commit an evidence run cites: the binary's
    /// stamped build tree must be that commit's build inputs and the worktree's, and the worktree that
    /// commit (HEAD by default, only when the worktree is clean).
    #[arg(long, global = true)]
    cite: Option<String>,
    /// (the binary rule) Run an evidence subcommand from a binary that isn't the cited commit's: it says
    /// NOT EVIDENCE at its head and foot and exits 3, so no verdict of it can be cited.
    #[arg(long, global = true)]
    not_evidence: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install Java, the server and its libraries, and build the bridge.
    Setup {
        /// Then start this many unmodified 26.3 servers at once and stop them: the bundler's first
        /// unpacking under concurrent starts (each start's outcome and time printed; on a fresh
        /// `MCTAS_HOME` the first start unpacks while the others wait on the repo's lock).
        #[arg(long, default_value_t = 0)]
        vanilla_starts: usize,
    },
    /// Read the game's source code (decompiled with Vineflower).
    Src {
        /// The client jar's source (models, renderers) instead of the server's.
        #[arg(long, global = true)]
        client: bool,
        #[command(subcommand)]
        command: SrcCmd,
    },
    /// Talk to a simulator.
    Sim {
        #[command(subcommand)]
        command: SimCmd,
    },
    /// Measure throughput and memory with many workers and sandboxes.
    Bench(bench::BenchArgs),
    /// 3x3 piston door project.
    Door {
        #[command(subcommand)]
        command: door::DoorCmd,
    },
    /// Redstone clock study (docs/clocks.md).
    Clock {
        #[command(subcommand)]
        command: clock_cli::ClockCmd,
    },
    /// Contraption studies: pulse circuits, T flip-flops, delay lines, gates, memory, piston
    /// extenders, flying machines (docs/circuits.md).
    Circuit {
        #[command(subcommand)]
        command: circuit_cli::CircuitCmd,
    },
    /// REDSTONEPEDIA, the website of every build (docs/site.md): build, deploy.
    Site {
        #[command(subcommand)]
        command: site::SiteCmd,
    },
    /// Litematica schematics (.litematic) of designs: export, read, check (docs/litematic.md).
    Litematic {
        #[command(subcommand)]
        command: litematic::cli::LitematicCmd,
    },
    /// The in-browser viewer (docs/webrender.md): offscreen PNG shots, checks, the wasm bundle.
    Web {
        #[command(subcommand)]
        command: web_cli::WebCmd,
    },
    /// Boat/lead physics fixtures and exact game calibration (designs/briefs/boat-leads.md).
    BoatLeads {
        #[command(subcommand)]
        command: boat_leads::cli::BoatLeadsCmd,
    },
    /// Bedrock breaking study (designs/bedrock/NOTES.md): per-window swap scanner.
    Bedrock {
        #[command(subcommand)]
        command: bedrock::cli::BedrockCmd,
    },
    /// Mob farms (designs/briefs/farm-iron.md): layout generators, the golem exit flood, runs.
    Mobfarm {
        #[command(subcommand)]
        command: mobfarm::cli::MobfarmCmd,
        /// (tickexact 2026-11-11, ironspec's sweep) Print each move a block shape's voxel grid
        /// stopped differently from a box test (the cart-pole fix: a cart starting a move inside a
        /// shape) with its cell and both distances; their count is printed after every command
        #[arg(long, global = true)]
        voxel_trace: bool,
    },
    /// The farm optimizer (designs/briefs/farm-optimizer.md): a stage spec in, the merged Pareto
    /// front per fork out, with the bounds and the gap.
    Farmopt {
        #[command(subcommand)]
        command: farmopt_iron::cli::FarmoptCmd,
    },
    /// Real terrain (designs/briefs/terrain-and-spawning.md): the official server generates a
    /// seed's area once; samples, crops, the game's /locate.
    Terrain {
        #[command(subcommand)]
        command: terrain::cli::TerrainCmd,
    },
    /// The tests of THIS change (AGENTS.md, docs/tools.md#mctas-test): the tree's diff against
    /// main picks the pieces (a simulator change: the recorded game traces replayed on redsim and
    /// the fuzzers of the areas it touched; a judge or study change: that study's calibration
    /// checks; a changed design: its own judge; docs and the site: nothing), prints the plan and
    /// why, runs them in parallel sized from the machine, PASS / FAIL per piece, and ends with
    /// the line for the commit message. `--all`: every piece, on demand.
    Test(door_judge::TestArgs),
    /// The recorded game answers the simulator pieces replay: `check` their manifests (a stale
    /// recording fails), `write` a manifest after recording again (crates/mctas/src/recorded.rs).
    Recorded {
        #[command(subcommand)]
        command: door_judge::RecordedCmd,
    },
    /// Mob AI in redsim (designs/briefs/mob-ai.md): the path probe, fuzzers, fixtures, benchmarks.
    Mobai {
        #[command(subcommand)]
        command: mobai::cli::MobaiCmd,
    },
    /// The design text format itself: `blank-rows` lists (and rewrites) blank lines inside a
    /// layer, which the parser refuses.
    Design {
        #[command(subcommand)]
        command: design_cli::DesignCmd,
    },
}

#[derive(Subcommand)]
enum SrcCmd {
    /// Decompile the server jar (cached).
    Decompile {
        #[arg(long)]
        force: bool,
    },
    /// Print the source root.
    Path,
    /// Regex search; prints path:line: text.
    Grep {
        pattern: String,
        /// Only files whose path contains this.
        #[arg(long)]
        path: Option<String>,
        #[arg(short = 'i', long)]
        ignore_case: bool,
        #[arg(long, default_value_t = 200)]
        max: usize,
    },
    /// Find the file of a class (simple or qualified name).
    Find { class: String },
    /// Print a class with line numbers.
    Show {
        class: String,
        /// Line range, e.g. 100:180.
        #[arg(long)]
        lines: Option<String>,
        /// Only lines matching this regex (with --context lines around).
        #[arg(long)]
        grep: Option<String>,
        #[arg(short = 'C', long, default_value_t = 3)]
        context: usize,
    },
}

#[derive(Subcommand)]
enum SimCmd {
    /// JSON-lines requests on stdin, responses on stdout (one worker).
    Repl,
    /// Place a design in a fresh sandbox, settle it and print what is pending.
    Place {
        design: PathBuf,
        /// Placement mode: raw (exact states) or update (like a player).
        #[arg(long, default_value = "update")]
        mode: String,
        /// Max ticks to wait for the design to go idle.
        #[arg(long, default_value_t = 200)]
        settle: u32,
        /// Print the settled design.
        #[arg(long)]
        dump: bool,
    },
}

fn main() -> Result<()> {
    use clap::{CommandFactory, FromArgMatches};
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches)?;
    // (the binary rule's ONE call site (G1 Nov 14 02:44b): every evidence subcommand, named once in
    // `mobfarm::provenance::is_evidence`, runs only from a binary stamped with the cited commit's build
    // tree, or under --not-evidence, which heads and foots its output NOT EVIDENCE and exits 3)
    let evidence = matches.subcommand().is_some_and(|(fam, sub)| sub.subcommand_name().is_some_and(|name| mobfarm::provenance::is_evidence(fam, name)));
    use mobfarm::provenance::{Exit, finish};
    let citable = if evidence {
        match mobfarm::provenance::check(&std::env::current_dir()?, cli.cite.as_deref(), cli.not_evidence) {
            Ok((header, citable)) => {
                println!("{header}");
                citable
            }
            Err(refusal) => {
                println!("{refusal}");
                finish(Exit::Refused);
            }
        }
    } else {
        true
    };
    if !evidence {
        return run(cli);
    }
    // (an evidence run ends on the table's last line: NOT EVIDENCE under --not-evidence whatever the
    // verdict, else PASS when the run returns Ok, FAIL (named above) when it errs; a lane whose runner
    // exits by itself prints its own line from the same table)
    match run(cli) {
        Ok(()) if !citable => finish(Exit::NotEvidence),
        Ok(()) => finish(Exit::Pass),
        Err(e) => {
            println!("{e:#}");
            finish(if citable { Exit::Fail } else { Exit::NotEvidence })
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::Setup { vanilla_starts } => {
            let build = bridge::build(&cli.version)?;
            println!("minecraft {}", build.install.version);
            println!("java      {}", build.install.java.display());
            println!("server    {}", build.install.server_jar.display());
            println!("libs      {}", build.install.libs_dir.display());
            println!("bridge    {}", build.classes.display());
            let mut w = Worker::spawn(&WorkerConfig { version: cli.version.clone(), ..Default::default() })?;
            println!("worker    ok ({})", w.info()?["version"]);
            if vanilla_starts > 0 {
                // (farmopt 2026-11-11: the bundler's first unpacking raced when servers started
                // together on a fresh box; `mctas::vanilla::bundler_extract_guard` serializes it)
                let t0 = std::time::Instant::now();
                let results: Vec<(usize, Result<(), String>, f64)> = std::thread::scope(|s| {
                    let hs: Vec<_> = (0..vanilla_starts)
                        .map(|k| {
                            let version = cli.version.clone();
                            s.spawn(move || {
                                let r = mctas::vanilla::VanillaServer::start(&mctas::vanilla::VanillaOptions { version, ..Default::default() }).map(drop).map_err(|e| format!("{e:#}"));
                                (k, r, t0.elapsed().as_secs_f64())
                            })
                        })
                        .collect();
                    hs.into_iter().map(|h| h.join().unwrap_or((usize::MAX, Err("panicked".into()), 0.0))).collect()
                });
                let ok = results.iter().filter(|r| r.1.is_ok()).count();
                for (k, r, at) in &results {
                    println!("vanilla start {k}: {} at {at:.1} s", match r { Ok(()) => "up".to_string(), Err(e) => format!("FAILED: {e}") });
                }
                println!("vanilla    {ok} of {vanilla_starts} servers started at once came up");
                anyhow::ensure!(ok == vanilla_starts, "{} of {vanilla_starts} vanilla servers did not start", vanilla_starts - ok);
            }
        }
        Cmd::Src { client, command } => src(&cli.version, client, command)?,
        Cmd::Sim { command } => sim(&cli.version, command)?,
        Cmd::Bench(args) => bench::run(&cli.version, args)?,
        Cmd::Door { command } => door::run(&cli.version, command)?,
        Cmd::Clock { command } => clock_cli::run(&cli.version, command)?,
        Cmd::Circuit { command } => circuit_cli::run(&cli.version, command)?,
        Cmd::Site { command } => site::run(command)?,
        Cmd::Litematic { command } => litematic::cli::run(command)?,
        Cmd::Web { command } => web_cli::run(&cli.version, command)?,
        Cmd::BoatLeads { command } => boat_leads::cli::run(&cli.version, command)?,
        Cmd::Bedrock { command } => bedrock::cli::run(&cli.version, command)?,
        Cmd::Mobfarm { command, voxel_trace } => {
            mobfarm::cli::set_voxel_trace(voxel_trace);
            mobfarm::cli::run(&cli.version, command)?;
            eprintln!(
                "voxel-stop moves: {} (moves starting inside a block's shape that its voxel grid stopped differently from a box test; 0 expected; --voxel-trace lists them)",
                mobfarm::cli::voxel_stops()
            );
        }
        Cmd::Farmopt { command } => farmopt_iron::cli::run(command)?,
        Cmd::Terrain { command } => terrain::cli::run(&cli.version, command)?,
        Cmd::Test(args) => door_judge::cmd_test(&cli.version, args)?,
        Cmd::Recorded { command } => door_judge::cmd_recorded(&cli.version, command)?,
        Cmd::Mobai { command } => mobai::cli::run(&cli.version, command)?,
        Cmd::Design { command } => design_cli::run(command)?,
    }
    Ok(())
}

fn src(version: &str, client: bool, command: SrcCmd) -> Result<()> {
    let decompile = |force: bool| if client { source::decompile_client(version, force) } else { source::decompile(version, force) };
    match command {
        SrcCmd::Decompile { force } => {
            let root = decompile(force)?;
            println!("{}", root.display());
        }
        SrcCmd::Path => println!("{}", decompile(false)?.display()),
        SrcCmd::Grep { pattern, path, ignore_case, max } => {
            let root = decompile(false)?;
            let re = regex::RegexBuilder::new(&pattern).case_insensitive(ignore_case).build()?;
            for hit in source::grep(&root, &re, path.as_deref(), max)? {
                let rel = hit.path.strip_prefix(&root).unwrap_or(&hit.path);
                println!("{}:{}: {}", rel.display(), hit.line, hit.text.trim_end());
            }
        }
        SrcCmd::Find { class } => {
            let root = decompile(false)?;
            for p in source::find(&root, &class)? {
                println!("{}", p.display());
            }
        }
        SrcCmd::Show { class, lines, grep, context } => {
            let root = decompile(false)?;
            let files = source::find(&root, &class)?;
            let file = match files.as_slice() {
                [one] => one.clone(),
                [] => bail!("no class {class}"),
                many => bail!("{class} is ambiguous: {}", many.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")),
            };
            let text = std::fs::read_to_string(&file)?;
            let all: Vec<&str> = text.lines().collect();
            let (lo, hi) = match lines {
                Some(r) => {
                    let (a, b) = r.split_once(':').context("--lines A:B")?;
                    (a.parse::<usize>()?.max(1), b.parse::<usize>()?.min(all.len()))
                }
                None => (1, all.len()),
            };
            let keep: Vec<bool> = match grep {
                Some(g) => {
                    let re = regex::Regex::new(&g)?;
                    let mut keep = vec![false; all.len()];
                    for (i, l) in all.iter().enumerate() {
                        if re.is_match(l) {
                            for k in keep.iter_mut().take((i + context + 1).min(all.len())).skip(i.saturating_sub(context)) {
                                *k = true;
                            }
                        }
                    }
                    keep
                }
                None => vec![true; all.len()],
            };
            println!("// {}", file.display());
            let mut last = 0;
            for n in lo..=hi {
                if !keep[n - 1] {
                    continue;
                }
                if last != 0 && n != last + 1 {
                    println!("     ...");
                }
                println!("{n:5}  {}", all[n - 1]);
                last = n;
            }
        }
    }
    Ok(())
}

fn sim(version: &str, command: SimCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match command {
        SimCmd::Repl => {
            let mut w = Worker::spawn(&cfg)?;
            let stdin = std::io::stdin();
            let mut out = std::io::stdout().lock();
            for line in stdin.lock().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let req: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        writeln!(out, "{}", serde_json::json!({"ok": false, "error": e.to_string()}))?;
                        continue;
                    }
                };
                let resp = match w.call(req) {
                    Ok(v) => v,
                    Err(e) => serde_json::json!({"ok": false, "error": format!("{e:#}")}),
                };
                writeln!(out, "{resp}")?;
                out.flush()?;
            }
        }
        SimCmd::Place { design, mode, settle, dump } => {
            let d = Design::load(&design)?;
            let mode = match mode.as_str() {
                "raw" => PlaceMode::Raw,
                "update" => PlaceMode::Update,
                other => bail!("unknown mode {other}"),
            };
            let mut w = Worker::spawn(&cfg)?;
            let sb = w.create(&SandboxOptions::default())?;
            w.set_blocks(sb, &d.block_list(), mode)?;
            let r = w.step_until_idle(sb, settle)?;
            println!("{} blocks, settled after {} ticks (idle: {:?})", d.block_count(), r.ticked, r.idle);
            println!("{:?}", w.status(sb)?);
            if dump {
                let (lo, hi) = d.bounds().context("empty design")?;
                let region = w.get_region(sb, lo, hi)?;
                let mut settled = Design { name: d.name.clone(), meta: d.meta.clone(), ports: d.ports.clone(), ..Default::default() };
                for (p, s) in region.blocks() {
                    settled.blocks.insert(p, s);
                }
                print!("{}", settled.to_text());
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn require_setup(version: &str) -> Result<()> {
    setup::install_server(version).map(|_| ())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    /// G1 Nov 14 02:59 (the binary rule's default-deny): EVERY subcommand of the evidence families is
    /// either checked at the dispatch or on the explicit NON_EVIDENCE list, and every listed entry names a
    /// real subcommand (no stale exemption).
    #[test]
    fn every_evidence_family_subcommand_is_checked_or_listed() {
        // (the whole command tree is large: built on a thread with a big stack, as a debug test thread's
        // default overflows)
        std::thread::Builder::new().stack_size(1 << 30).spawn(check_every_subcommand).expect("thread").join().expect("the enumeration");
    }

    fn check_every_subcommand() {
        let cli = super::Cli::command();
        let mut checked = 0usize;
        for fam in ["mobfarm", "mobai"] {
            let f = cli.find_subcommand(fam).unwrap_or_else(|| panic!("the {fam} family"));
            let names: Vec<String> = f.get_subcommands().map(|s| s.get_name().to_string()).collect();
            assert!(!names.is_empty(), "{fam} has subcommands");
            for n in &names {
                let listed = mobfarm::provenance::NON_EVIDENCE.contains(&(fam, n.as_str()));
                assert!(mobfarm::provenance::is_evidence(fam, n) != listed, "{fam} {n}: checked xor listed");
                checked += usize::from(!listed);
            }
            for (lf, ln) in mobfarm::provenance::NON_EVIDENCE {
                if lf == fam {
                    assert!(names.iter().any(|n| n == ln), "NON_EVIDENCE names {lf} {ln}, which isn't a subcommand");
                }
            }
        }
        assert!(checked > 0);
    }
}
