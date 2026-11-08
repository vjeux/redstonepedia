mod audit;
mod bank;
mod budget;
mod build;
mod checks;
mod dock;
mod graph;
mod orientation;
mod pack;
mod recording;
mod regress;
mod report;
mod run;
mod scenarios;
mod summary;
mod trace;
mod verify;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("regress") if args.len() == 3 => regress::regress(std::path::Path::new(&args[2])),
        Some("pack") if args.len() == 3 => pack::pack(std::path::Path::new(&args[2])),
        Some("orientation") if args.len() == 3 => {
            let dir = std::path::Path::new("designs/boat-lead-vanilla/pairs");
            orientation::generate(dir)?;
            for name in ["opposite-yaw", "opposite-yaw-no-lead"] {
                let output = std::path::Path::new(&args[2]).join(name);
                run::run(&dir.join(format!("{name}.json")), &output)?;
                report::report(&output)?;
            }
            Ok(())
        }
        Some("bank") if args.len() == 4 => bank::bank(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        ),
        Some("verify") if args.len() == 3 => verify::verify(std::path::Path::new(&args[2])),
        Some("dock") if args.len() == 3 => {
            let path =
                std::path::Path::new("designs/boat-lead-vanilla/pairs/normal-dock-mixed.json");
            dock::generate(path)?;
            run::run(path, std::path::Path::new(&args[2]))?;
            report::report(std::path::Path::new(&args[2]))
        }
        Some("controls") if args.len() == 3 => {
            scenarios::generate(std::path::Path::new("designs/boat-lead-vanilla/pairs"))?;
            for name in [
                "pair-raft-no-lead",
                "pair-oak-ordinary",
                "pair-oak-mixed",
                "pair-raft-mixed-no-lead",
            ] {
                let scenario = format!("designs/boat-lead-vanilla/pairs/{name}.json");
                let output = std::path::Path::new(&args[2]).join(name);
                run::run(std::path::Path::new(&scenario), &output)?;
                report::report(&output)?;
            }
            Ok(())
        }
        Some("report") if args.len() == 3 => report::report(std::path::Path::new(&args[2])),
        Some("audit") => audit::audit(std::path::Path::new(".")),
        Some("generate") if args.len() == 3 => scenarios::generate(std::path::Path::new(&args[2])),
        Some("summary") if args.len() == 3 => summary::summarize(std::path::Path::new(&args[2])),
        Some("analyze") if args.len() == 3 => trace::analyze(std::path::Path::new(&args[2])),
        Some("budget") if args.len() == 4 => budget::budget(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        ),
        Some("run") if args.len() == 4 => run::run(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        ),
        _ => bail!("usage: boat-lead-vanilla analyze TRACE.jsonl | run SCENARIO.json OUTPUT"),
    }
}
