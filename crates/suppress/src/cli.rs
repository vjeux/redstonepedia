//! `mctas suppress ...` (designs/briefs/update-suppressor.md).

use anyhow::Result;
use clap::{Args, Subcommand};
use redsim::pos::BlockPos;

#[derive(Subcommand)]
pub enum SuppressCmd {
    /// Build a flood-generator family in redsim, fire one trigger edge, report the
    /// neighbor-updater cascade counts (the search for a wave that reaches 1,000,000).
    Probe(ProbeArgs),
    /// The judge rig on redsim (brief: control reacts, suppressed target stays stale until
    /// the wake), over positions x arms x mirror.
    Judge(JudgeArgs),
    /// The judge rig in the real game (bridge sandbox, the game's own cap).
    Bridge(BridgeArgs),
    /// Write the judge rig as a design file (for the record / the site pipeline).
    Emit(EmitArgs),
    /// The judge rig on the official dedicated server (bot clicks the lever). The cap is a
    /// server.properties knob; the default 5000 is where this rig's field suppresses.
    Vanilla(VanillaArgs),
}

#[derive(Args)]
pub struct VanillaArgs {
    #[arg(long, default_value_t = 5000)]
    cap: u32,
    #[arg(long, default_value_t = 31)]
    field: i32,
}

#[derive(Args)]
pub struct JudgeArgs {
    /// Dust field size (odd).
    #[arg(long, default_value_t = 31)]
    field: i32,
    /// The cascade cap to judge against (redsim `nu_cap`; the game's is 1,000,000).
    #[arg(long, default_value_t = 1_000_000)]
    cap: u32,
    /// Also run the control (head wire only; the target must react).
    #[arg(long)]
    control: bool,
    /// World offsets to try, `x,y,z;...`.
    #[arg(long, default_value = "0,64,0;1,64,0;15,64,0;16,64,0;14,64,13;-16,64,-1")]
    positions: String,
    /// Target-arm directions to try (subset of ENSW letters), each also mirrored.
    #[arg(long, default_value = "ENSW")]
    arms: String,
}

#[derive(Args)]
pub struct BridgeArgs {
    #[arg(long, default_value_t = 31)]
    field: i32,
    #[arg(long, default_value = "ENSW")]
    arms: String,
}

#[derive(Args)]
pub struct EmitArgs {
    #[arg(long, default_value_t = 31)]
    field: i32,
    /// Without the flood field (the control rig).
    #[arg(long)]
    control: bool,
    #[arg(long)]
    out: std::path::PathBuf,
}

#[derive(Args)]
pub struct ProbeArgs {
    /// Families, comma separated: dust | rail:<pattern>:<kinds>:<geom> (short: rail, rail-comb, rail-t).
    #[arg(long, default_value = "rail-t")]
    family: String,
    /// Sizes to try, comma separated (the family's `n`).
    #[arg(long, default_value = "4,8,16,32")]
    sizes: String,
    /// Triggers, comma separated: lever | lever-off | place | break.
    #[arg(long, default_value = "lever")]
    trigger: String,
    /// World offset x,y,z (dust order and rail shapes are position dependent).
    #[arg(long, default_value = "0,64,0")]
    at: String,
    /// Print each of the trigger's cascades, not just the largest.
    #[arg(long)]
    cascades: bool,
    /// With family `rail-rand`: how many random patches to sample.
    #[arg(long, default_value_t = 200)]
    seeds: i64,
}

fn print_verdict(v: &crate::judge::Verdict) {
    println!(
        "{} {} counted={} dropped={} middle={} target_edge={} target_wake={}{}",
        if v.suppressed_ok { "PASS" } else { "FAIL" },
        v.name,
        v.counted,
        v.dropped,
        v.middle_flipped,
        v.target_on_edge,
        v.target_on_wake,
        if v.problems.is_empty() { String::new() } else { format!(" PROBLEMS: {}", v.problems.join("; ")) }
    );
}

fn parse_pos(s: &str) -> Result<BlockPos> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<std::result::Result<_, _>>()?;
    anyhow::ensure!(v.len() == 3, "position must be x,y,z");
    Ok(BlockPos::new(v[0], v[1], v[2]))
}

fn arms(s: &str) -> Vec<redsim::pos::Dir> {
    use redsim::pos::Dir;
    s.chars()
        .filter_map(|c| match c {
            'E' => Some(Dir::East),
            'N' => Some(Dir::North),
            'S' => Some(Dir::South),
            'W' => Some(Dir::West),
            _ => None,
        })
        .collect()
}

pub fn run(version: &str, cmd: SuppressCmd) -> Result<()> {
    match cmd {
        SuppressCmd::Judge(a) => {
            let mut pass = 0;
            let mut total = 0;
            for pos_s in a.positions.split(';') {
                let at = parse_pos(pos_s.trim())?;
                for arm in arms(&a.arms) {
                    for mirror in [false, true] {
                        if a.control {
                            total += 1;
                            let name = format!("control at={at} arm={} mirror={mirror}", arm.name());
                            let v = crate::judge::judge_one(&name, a.field, false, a.cap, at, arm, mirror, false);
                            pass += v.suppressed_ok as usize;
                            print_verdict(&v);
                        }
                        total += 1;
                        let name = format!("suppress at={at} arm={} mirror={mirror}", arm.name());
                        let v = crate::judge::judge_one(&name, a.field, true, a.cap, at, arm, mirror, true);
                        pass += v.suppressed_ok as usize;
                        print_verdict(&v);
                    }
                }
            }
            println!("judge: {pass}/{total} pass (cap={})", a.cap);
        }
        SuppressCmd::Bridge(a) => {
            for v in crate::bridge::run_bridge(version, a.field, &arms(&a.arms))? {
                println!(
                    "{}: middle_flipped={} target_on_edge={} target_on_wake={} lever_ms={}{}",
                    v.name,
                    v.middle_flipped,
                    v.target_on_edge,
                    v.target_on_wake,
                    v.lever_ms,
                    if v.problems.is_empty() { String::new() } else { format!(" PROBLEMS: {}", v.problems.join("; ")) }
                );
            }
        }
        SuppressCmd::Vanilla(a) => {
            for v in crate::vanilla::run_vanilla(version, a.cap, a.field)? {
                println!(
                    "vanilla cap={} {}: middle_flipped={} target_on_edge={} target_on_wake={}",
                    v.cap,
                    if v.control { "control" } else { "flood" },
                    v.middle_flipped,
                    v.target_on_edge,
                    v.target_on_wake
                );
            }
        }
        SuppressCmd::Emit(a) => {
            let r = crate::judge::rig(a.field, !a.control, redsim::pos::Dir::East, false);
            let mut d = mctas::design::Design { name: format!("update-suppressor judge rig (field {}, {})", a.field, if a.control { "control" } else { "flood" }), ..Default::default() };
            d.meta.insert("build".into(), "place the `redstone` port block LAST, after the rest settles (it quasi-powers the target piston without updating it)".into());
            let p3 = |p: BlockPos| [p.x, p.y, p.z];
            d.ports.insert("lever".into(), p3(r.lever));
            d.ports.insert("middle".into(), p3(r.middle));
            d.ports.insert("target".into(), p3(r.target));
            d.ports.insert("wake".into(), p3(r.wake));
            d.ports.insert("redstone".into(), p3(r.redstone));
            for (p, b) in &r.blocks {
                d.blocks.insert(p3(*p), b.to_string());
            }
            d.blocks.insert(p3(r.redstone), "minecraft:redstone_block".into());
            std::fs::write(&a.out, d.to_text())?;
            println!("wrote {} ({} blocks)", a.out.display(), d.blocks.len());
        }
        SuppressCmd::Probe(a) => {
            let at = parse_pos(&a.at)?;
            for fam in a.family.split(',') {
                if fam.trim() == "rail-rand" {
                    for tok in a.sizes.split(',') {
                        let n: i32 = tok.trim().parse()?;
                        let (lines, _) = crate::probe_rand(a.seeds, n, at)?;
                        for l in lines {
                            println!("{l}");
                        }
                    }
                    continue;
                }
                for trig in a.trigger.split(',') {
                    for tok in a.sizes.split(',') {
                        let n: i32 = tok.trim().parse()?;
                        match crate::probe(fam.trim(), n, at, trig.trim()) {
                            Ok(r) => {
                                println!("[{}] {}", trig.trim(), crate::report_line(&r));
                                if a.cascades {
                                    for (i, (c, e, d)) in r.cascades.iter().take(12).enumerate() {
                                        println!("    cascade{i}: counted={c} exec={e} dropped={d}");
                                    }
                                }
                                for e in r.errors.iter().take(3) {
                                    println!("    err: {e}");
                                }
                            }
                            Err(e) => println!("[{}] {} n={n}: ERROR {e:#}", trig.trim(), fam.trim()),
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
