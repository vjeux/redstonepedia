//! (tickexact 2026-11-12, ironspec 16:29 (c): A6's one-way route over restarts) `mctas mobai
//! block-watch`: on the official 26.3 server, the module (`--module`, its entities summoned,
//! force-loaded) with `--set` cells laid over it, `--ticks` gt with one save/stop/start at each of
//! `--restarts` (each its own row, plus a row without), every tick each `--watch` cell tested
//! against its state (`execute if block` of the full state); printed per row: the first tick a
//! watched cell differs (and which), or that every cell held at every tick.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() }
}

pub fn block_watch(version: &str, module: &std::path::Path, set: &[([i32; 3], String)], watch: &[([i32; 3], String)], restarts: &[u32], ticks: u32) -> Result<()> {
    println!(
        "== block-watch on the official {version} server: {} with {} cells set over it; {} cells watched every tick for {ticks} gt; restarts at {restarts:?}",
        module.display(),
        set.len(),
        watch.len()
    );
    let mut rows: Vec<Option<u32>> = vec![None];
    rows.extend(restarts.iter().map(|&r| Some(r)));
    for restart in rows {
        let mut srv = VanillaServer::start(&opts(version, None))?;
        srv.cmd_ok("gamerule spawn_mobs false")?;
        srv.cmd_ok("forceload add -32 -32 31 31")?;
        srv.step(40)?;
        crate::seatclear::place_part(&mut srv, module)?;
        for (p, s) in set {
            srv.cmd(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
        }
        srv.step(5)?;
        let mut first_bad: Option<String> = None;
        let mut reload_note: Option<crate::restartcheck::Reload> = None;
        for t in 1..=ticks {
            srv.step(1)?;
            for (p, s) in watch {
                if first_bad.is_none() && !srv.cmd_long(&format!("execute if block {} {} {} {s}", p[0], p[1] + DY, p[2]))?.contains("passed") {
                    first_bad = Some(format!("t{t}: ({},{},{}) no longer {s}", p[0], p[1], p[2]));
                }
            }
            if restart == Some(t) {
                let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, Some(w)), None)?;
                srv = s;
                srv.cmd_ok("forceload add -32 -32 31 31")?;
                reload_note = Some(rl);
            }
        }
        println!(
            "  {}{}{}: {}",
            if reload_note.is_some_and(|x| !x.clean()) { "FAILED (uncounted ticks across the load): " } else { "" },
            restart.map(|r| format!("a restart at t{r}")).unwrap_or_else(|| "no restart".into()),
            reload_note.map(|x| format!(" ({})", x.describe())).unwrap_or_default(),
            first_bad.unwrap_or_else(|| format!("every watched cell held its state at every tick of {ticks}"))
        );
        let _ = std::fs::remove_dir_all(&srv.dir);
    }
    Ok(())
}
