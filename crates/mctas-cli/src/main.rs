mod bench;
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
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install Java, the server and its libraries, and build the bridge.
    Setup,
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
    let cli = Cli::parse();
    match cli.command {
        Cmd::Setup => {
            let build = bridge::build(&cli.version)?;
            println!("minecraft {}", build.install.version);
            println!("java      {}", build.install.java.display());
            println!("server    {}", build.install.server_jar.display());
            println!("libs      {}", build.install.libs_dir.display());
            println!("bridge    {}", build.classes.display());
            let mut w = Worker::spawn(&WorkerConfig { version: cli.version.clone(), ..Default::default() })?;
            println!("worker    ok ({})", w.info()?["version"]);
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
        Cmd::Mobfarm { command } => mobfarm::cli::run(&cli.version, command)?,
        Cmd::Farmopt { command } => farmopt_iron::cli::run(command)?,
        Cmd::Terrain { command } => terrain::cli::run(&cli.version, command)?,
        Cmd::Test(args) => door_judge::cmd_test(&cli.version, args)?,
        Cmd::Recorded { command } => door_judge::cmd_recorded(&cli.version, command)?,
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
