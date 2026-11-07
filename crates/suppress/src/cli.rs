//! `mctas suppress ...` (designs/briefs/update-suppressor.md).

use anyhow::Result;
use clap::{Args, Subcommand};
use redsim::pos::BlockPos;

#[derive(Subcommand)]
pub enum SuppressCmd {
    /// Build a flood-generator family in redsim, fire one trigger edge, report the
    /// neighbor-updater cascade counts (the search for a wave that reaches 1,000,000).
    Probe(ProbeArgs),
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

fn parse_pos(s: &str) -> Result<BlockPos> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<std::result::Result<_, _>>()?;
    anyhow::ensure!(v.len() == 3, "position must be x,y,z");
    Ok(BlockPos::new(v[0], v[1], v[2]))
}

pub fn run(_version: &str, cmd: SuppressCmd) -> Result<()> {
    match cmd {
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
