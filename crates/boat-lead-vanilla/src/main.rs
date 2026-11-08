mod audit;
mod dock;
mod budget;
mod build;
mod report;
mod recording;
mod run;
mod scenarios;
mod summary;
mod trace;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("dock") if args.len() == 3 => {
            let path=std::path::Path::new("designs/boat-lead-vanilla/pairs/normal-dock-mixed.json");
            dock::generate(path)?;
            run::run(path,std::path::Path::new(&args[2]))?;
            report::report(std::path::Path::new(&args[2]))
        }
        Some("controls") if args.len() == 3 => {
            scenarios::generate(std::path::Path::new("designs/boat-lead-vanilla/pairs"))?;
            for name in ["pair-raft-no-lead", "pair-oak-ordinary", "pair-oak-mixed", "pair-raft-mixed-no-lead"] {
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
        Some("budget") if args.len() == 4 => budget::budget(std::path::Path::new(&args[2]), std::path::Path::new(&args[3])),
        Some("run") if args.len() == 4 => run::run(std::path::Path::new(&args[2]), std::path::Path::new(&args[3])),
        _ => bail!("usage: boat-lead-vanilla analyze TRACE.jsonl | run SCENARIO.json OUTPUT"),
    }
}
