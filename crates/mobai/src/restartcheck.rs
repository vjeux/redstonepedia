//! (tickexact 2026-11-12, G1 on 94eb02e76; ironspec 20:35 PT (2a), 23:12-23:27 PT) A save/stop/start
//! of a world, checked. A load is N whole-chunk idle ticks (load-count's 20-TPS rows: no block ticks,
//! block entities or entities run in them; game and day time advance); at a world open, pending
//! ticks keep their save-time due times, so redstone pending across the load fires up to N ahead of
//! entities. The stepped harness also lets a fresh server tick before its RCON freeze: a probe entity
//! that MOVED across the load means uncounted harness ticks. The probe proves something only if it
//! was moving at the save; a probe at rest gives no evidence, and the row falls back to the no-probe
//! rule (game time +1, or the row fails).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

/// What a load did.
#[derive(Clone, Copy, Debug, Default)]
pub struct Reload {
    /// The game time after the load minus before the save.
    pub time_jump: i64,
    /// Whether a moving probe kept its position across the load (None: no probe, the probe had no
    /// position, or it was at rest at the save).
    pub held: Option<bool>,
    /// The probe was found but at rest at the save: no evidence (ironspec 23:27 PT (2b)).
    pub probe_at_rest: bool,
}

impl Reload {
    /// A row is exact when a moving probe held, or (no probe evidence) the game time moved exactly 1.
    pub fn clean(&self) -> bool {
        match self.held {
            Some(h) => h,
            None => self.time_jump == 1,
        }
    }

    /// A held probe across a jump of N: N idle ticks; a moved probe: uncounted harness ticks; no
    /// probe evidence (none, or at rest): a jump other than 1 is uncounted and fails the row.
    pub fn describe(&self) -> String {
        let rest = if self.probe_at_rest { "probe at rest: no evidence; " } else { "" };
        match self.held {
            Some(true) => format!(
                "{} idle tick{} (game time +{}; entities held: the row's probe; blocks idle: load-count's measurement)",
                self.time_jump,
                if self.time_jump == 1 { "" } else { "s" },
                self.time_jump
            ),
            Some(false) => format!("UNCOUNTED TICKS: the probe moved across the load (game time +{}); not citable as is", self.time_jump),
            None if self.time_jump == 1 => format!("{rest}no probe evidence (game time +1)"),
            None => format!("{rest}uncounted: {} ticks (game time +{}, no probe evidence of what ran in them); the row fails", self.time_jump, self.time_jump),
        }
    }
}

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect()
}

/// Save, stop the old server, start the new one on its world (`opts` given the world's path), and
/// compare the game time and `probe`'s Pos (an entity selector) across the load; a probe whose
/// Motion is zero at the save gives no evidence.
pub fn reload(mut srv: VanillaServer, opts: impl FnOnce(std::path::PathBuf) -> VanillaOptions, probe: Option<&str>) -> Result<(VanillaServer, Reload)> {
    srv.cmd_ok("save-on")?;
    srv.cmd_ok("save-all flush")?;
    let before = srv.game_time()?;
    let (pos_before, moving) = match probe {
        Some(p) => {
            let m = nums(&srv.cmd_long(&format!("data get entity {p} Motion"))?);
            (Some(srv.cmd_long(&format!("data get entity {p} Pos"))?), m.iter().any(|v| v.abs() > 1e-9))
        }
        None => (None, false),
    };
    let old = srv.dir.clone();
    let world = old.join(&srv.level_name);
    drop(srv);
    let mut fresh = VanillaServer::start(&opts(world))?;
    let after = fresh.game_time()?;
    let found = pos_before.as_ref().is_some_and(|b| b.contains("entity data"));
    let held = match (probe, &pos_before) {
        (Some(p), Some(b)) if found && moving => Some(fresh.cmd_long(&format!("data get entity {p} Pos"))?.trim() == b.trim()),
        _ => None,
    };
    let _ = std::fs::remove_dir_all(&old);
    Ok((fresh, Reload { time_jump: after - before, held, probe_at_rest: found && !moving }))
}

#[cfg(test)]
mod tests {
    use super::Reload;

    #[test]
    fn clean_and_describe_the_three_cases() {
        // a moving probe that held across a one-tick load
        let held = Reload { time_jump: 1, held: Some(true), probe_at_rest: false };
        assert!(held.clean());
        assert!(held.describe().starts_with("1 idle tick (game time +1"));
        // a moving probe that moved: uncounted harness ticks
        let moved = Reload { time_jump: 2, held: Some(false), probe_at_rest: false };
        assert!(!moved.clean());
        assert!(moved.describe().starts_with("UNCOUNTED TICKS"));
        // a probe at rest: no evidence, the no-probe rule
        let rest1 = Reload { time_jump: 1, held: None, probe_at_rest: true };
        assert!(rest1.clean());
        assert_eq!(rest1.describe(), "probe at rest: no evidence; no probe evidence (game time +1)");
        let rest2 = Reload { time_jump: 2, held: None, probe_at_rest: true };
        assert!(!rest2.clean());
        assert!(rest2.describe().contains("uncounted: 2 ticks"));
        // no probe, jump 3: fails
        let none3 = Reload { time_jump: 3, held: None, probe_at_rest: false };
        assert!(!none3.clean());
        assert!(none3.describe().starts_with("uncounted: 3 ticks"));
    }
}
