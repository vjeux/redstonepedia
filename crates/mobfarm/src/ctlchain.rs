//! The cycle start's REL-to-REL interval and phase lock (day-1, Nov 12 2026; ironspec 15:23 (3), 18:06 (3),
//! 18:40 (4)/(5)). The brief's "one trigger, one chain": Pending (P) is set by the pad's golem pulse (1 gt
//! after a summon tick; summon ticks every 100 gt of game time) or by the watchdog's expiry (W after the LAST
//! start, retriggerable); stage 1 fires on the level P AND hold-settled AND NOT B; REL is stage 1 + 2; the
//! wave entering stage 3 (+4) clears P. Three B rules:
//! - AsBuilt: B from stage 3 to the sequence's end (`b_end` after stage 1, the chain's 15 stages): a golem
//!   pulse inside B is latched and starts the chain again when B clears (the ~30-gt double start);
//! - Held: B held through the cycle (100 gt after stage 1), a pulse inside it latched to its end: the
//!   interval is >= 100, but an off-phase cycle keeps its phase (ironspec 18:40's control);
//! - ReArm (ironspec 18:40 (5)): B held 100 gt after stage 1; a golem pulse while B is active re-arms B to
//!   the pulse + 100, so the latched start fires at the next golem-locked slot; a pulse at or after B's
//!   expiry starts at once (the tie breaks toward the start). Optionally the flywheel: W = 100, retriggered
//!   by each start.
//! A freeze (the pearl loader) stops the chain and the summons for its length while game time runs on, so the
//! chain's phase against the summon ticks shifts by it. The model prints, per rule: the least REL-to-REL
//! interval; per disturbance, the cycles from the first golem after it until the cycle is golem-locked (REL
//! at the summon tick + 3) for good, and the off-phase RELs in that span (each a batch released off-phase,
//! counted as a summon lost); the scenarios not locked at their end; the off-phase RELs a day in steady
//! running.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    AsBuilt,
    Held,
    ReArm,
}

/// One run: golem pulses at the given game ticks (lost while frozen), the watchdog's W, the B rule, the
/// hold's settle after a start, an optional freeze (from, length); the REL game ticks over `t_end`.
fn run(pulses: &[i64], w: i64, mode: Mode, b_end: i64, settle: i64, t_end: i64, freeze: Option<(i64, i64)>) -> Vec<i64> {
    let mut rels = Vec::new();
    let (mut p, mut b_until, mut last_start, mut settled_at) = (false, i64::MIN, 0i64, 0i64);
    let mut stage3_at: Option<i64> = None;
    let mut pi = 0usize;
    // (the chain's local time: it stands still while frozen)
    let mut lt = 0i64;
    for t in 0..t_end {
        let frozen = freeze.is_some_and(|(a, f)| t >= a && t < a + f);
        let mut pulse = false;
        while pi < pulses.len() && pulses[pi] <= t {
            if pulses[pi] == t && !frozen {
                pulse = true;
            }
            pi += 1;
        }
        if frozen {
            continue;
        }
        if pulse {
            p = true;
            if mode == Mode::ReArm && lt < b_until {
                b_until = lt + 100;
            }
        }
        if lt == last_start + w {
            p = true;
        }
        if stage3_at == Some(lt) {
            p = false;
            stage3_at = None;
        }
        let busy = lt < b_until || stage3_at.is_some();
        if p && !busy && lt >= settled_at {
            last_start = lt;
            rels.push(t + 2);
            stage3_at = Some(lt + 4);
            b_until = lt + if mode == Mode::AsBuilt { b_end } else { 100 };
            settled_at = lt + settle;
        }
        lt += 1;
    }
    rels
}

/// (G1's checklist (2)) The least REL-to-REL interval; None with fewer than two RELs (never a pass).
fn least(rels: &[i64]) -> Option<i64> {
    rels.windows(2).map(|w| w[1] - w[0]).min()
}

/// The lock after a disturbance: (the RELs from the first golem pulse after `td` until the cycle is
/// golem-locked for good, the off-phase RELs among them, the skipped slots (a pulse with summons present
/// and no REL in its cycle, ironspec 19:53 PT (2)(a)); locked at the end). (G1's checklist (2)) no golem pulse
/// after the disturbance, or no REL after it, is not locked (never a vacuous pass).
fn lock_after(rels: &[i64], pulses: &[i64], sp: i64, td: i64) -> (usize, usize, usize, bool) {
    let locked = |r: i64| (r - sp).rem_euclid(100) == 3;
    let Some(&tg) = pulses.iter().find(|&&p| p >= td) else { return (0, 0, 0, false) };
    let after: Vec<i64> = rels.iter().copied().filter(|&r| r >= tg).collect();
    // (a slot is skipped when no REL falls in [p, p + 100) for a pulse p after tg, the last pulse aside
    // (the run ends 50 gt after it))
    let last_p = pulses.last().copied().unwrap_or(0);
    let skipped = pulses.iter().filter(|&&p| p >= tg && p < last_p && !rels.iter().any(|&r| r >= p && r < p + 100)).count();
    // the last off-phase REL after tg: the lock holds from the REL after it
    let last_off = after.iter().rposition(|&r| !locked(r));
    let at_end = after.last().is_some_and(|&r| locked(r));
    match last_off {
        None => (0, 0, skipped, at_end),
        Some(i) => (i + 1, after[..=i].iter().filter(|&&r| !locked(r)).count(), skipped, at_end),
    }
}

/// The bench: per B rule (and the flywheel), the least REL-to-REL interval and the phase lock over the
/// scenarios, at each W. (The chain fix) the (5) rows judged through `v` on every bar ((a) with at least two RELs,
/// (b) including the failed rolls, every scenario locked at its end, (c)); the two CONTROL (d) rows must fail their
/// own bar (the chain as built (a), B held without the re-arm (b)).
pub fn bench(b_end: i64, settle: i64, bar: i64, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    // (G1's audit hole 18: the bar is ironspec 18:06's >= 100; a smaller one is refused)
    if bar < 100 {
        return Err(format!("--bar {bar}: ironspec 18:06's bar is >= 100; a smaller bar is refused"));
    }
    if settle < 0 || b_end < 1 {
        return Err(format!("--settle {settle} / --b-end {b_end}: settle >= 0, b-end >= 1"));
    }
    let mut lines = vec![format!(
        "the cycle start (P set by the golem pulse at the summon tick + 1 or by the watchdog W after the last start; stage 1 on P AND hold-settled ({settle} gt after a start) AND NOT B; REL at stage 1 + 2; summon ticks every 100 gt of game time; golem-locked = REL at the summon tick + 3): the bars (ironspec 18:40 (4)) (a) the least REL-to-REL >= {bar}, (b) after any disturbance (a failed roll included) locked within one cycle of the first golem after it, <= 1 extra summon lost, (c) 0 off-phase cycles in steady running"
    )];
    // (label, mode, flywheel, the row's role: None judged, Some('a') / Some('b') a control that must fail that bar)
    let rows: [(&str, Mode, Option<i64>, Option<char>); 4] = [
        ("CONTROL (d), the chain as built (B to the sequence's end)", Mode::AsBuilt, None, Some('a')),
        ("CONTROL (d), B held 100 gt, a pulse inside it latched to its end (no re-phase)", Mode::Held, None, Some('b')),
        ("(5) B held 100 gt, re-armed to the pulse + 100 by a golem pulse inside it (the tie to the start)", Mode::ReArm, None, None),
        ("(5) the re-arm with the flywheel (W = 100, retriggered by each start)", Mode::ReArm, Some(100), None),
    ];
    for (label, mode, fly, role) in rows {
        let ws: Vec<i64> = match fly {
            Some(w) => vec![w],
            None => vec![105, 110, 115, 120, 125],
        };
        let (mut worst_int, mut worst_int_at) = (i64::MAX, String::new());
        let mut short_runs = 0u32;
        let (mut max_cycles, mut max_lost, mut max_at) = (0usize, 0usize, String::new());
        // (ironspec 19:53 PT (2)(a): the failed rolls' own count, so "a failed roll restarts on phase with
        // no skip" prints from it)
        let mut max_lost_failed = 0usize;
        let mut n_failed = 0u32;
        let (mut n, mut unlocked, mut unlocked_at) = (0u32, 0u32, String::new());
        let mut steady_off = 0usize;
        let mut steady_rels = 0usize;
        for &w in &ws {
            let mut check = |name: String, pulses: Vec<i64>, sp: i64, td: i64, freeze: Option<(i64, i64)>| {
                let t_end = pulses.last().copied().unwrap_or(0) + 50;
                let rels = run(&pulses, w, mode, b_end, settle, t_end, freeze);
                // (G1's checklist (2): fewer than two RELs gives no interval: counted, never a pass)
                match least(&rels) {
                    Some(l) if l < worst_int => {
                        worst_int = l;
                        worst_int_at = format!("W {w}, {name}");
                    }
                    Some(_) => {}
                    None => short_runs += 1,
                }
                let (c, off, skipped, at_end) = lock_after(&rels, &pulses, sp, td);
                let lost = off + skipped;
                if name.starts_with("a failed roll") {
                    max_lost_failed = max_lost_failed.max(lost);
                    n_failed += 1;
                }
                n += 1;
                if !at_end {
                    unlocked += 1;
                    if unlocked_at.is_empty() {
                        unlocked_at = format!("W {w}, {name}");
                    }
                }
                if (c, lost) > (max_cycles, max_lost) {
                    max_cycles = c;
                    max_lost = lost;
                    max_at = format!("W {w}, {name}");
                }
            };
            for sp in [0i64, 37, 99] {
                let tick = |k: i64| 100 * k + sp + 1;
                // normal running from a golem start (the steady state)
                let normal: Vec<i64> = (0..300).map(tick).collect();
                check(format!("normal (summon phase {sp})"), normal, sp, 0, None);
                // one failed pad roll at each of cycles 5..15 (the disturbance: the failed summon tick)
                for f in 5..15 {
                    let pulses: Vec<i64> = (0..60).filter(|&k| k != f).map(tick).collect();
                    check(format!("a failed roll at cycle {f} (phase {sp})"), pulses, sp, tick(f), None);
                }
                // summons absent for n cycles (the watchdog cycling meanwhile), resuming at the same phase
                for nn in 2..40 {
                    let pulses: Vec<i64> = (0..100).filter(|&k| k <= 5 || k >= 5 + nn).map(tick).collect();
                    check(format!("summons absent {nn} cycles (phase {sp})"), pulses, sp, 100 * (5 + nn), None);
                }
                // a freeze of f gt at every 7th tick of a cycle (the chain and the summons stopped, game time on)
                for f in [3i64, 4, 40, 150, 333] {
                    for at in (0..100).step_by(7) {
                        let tf = 2_000 + sp + at;
                        let pulses: Vec<i64> = (0..80).map(tick).collect();
                        check(format!("a {f}-gt freeze at cycle 20 + {at} (phase {sp})"), pulses, sp, tf + f, Some((tf, f)));
                    }
                }
            }
            // a cold start (the chain from game time 0, no summons until cycle 30) at every summon phase
            for sp in 0..100 {
                let pulses: Vec<i64> = (30..90).map(|k| 100 * k + sp + 1).collect();
                check(format!("a cold start, summons from cycle 30 at phase {sp}"), pulses, sp, 3_000, None);
            }
            // steady state: off-phase RELs a day in normal running (24,000 gt, the run ending 50 gt after the
            // last pulse so no watchdog fires past the summons)
            let normal: Vec<i64> = (0..240).map(|k| 100 * k + 1).collect();
            let rels = run(&normal, w, mode, b_end, settle, 23_901 + 50, None);
            steady_rels += rels.len();
            steady_off = steady_off.max(rels.iter().filter(|&&r| r.rem_euclid(100) != 3).count());
        }
        let a_ok = worst_int != i64::MAX && worst_int >= bar && short_runs == 0;
        let b_ok = max_cycles <= 1 && max_lost <= 1 && max_lost_failed <= 1;
        let (a_w, b_w, lock_w, c_w) = match role {
            None => (
                v.bar_n(&format!("{label}: (a) the least REL-to-REL >= {bar}"), n as usize, a_ok),
                v.bar_n(&format!("{label}: (b) locked within one cycle, <= 1 extra summon lost (failed rolls included)"), n as usize, b_ok && n_failed > 0),
                v.bar_n(&format!("{label}: every scenario locked at its end"), n as usize, unlocked == 0),
                v.bar_n(&format!("{label}: (c) 0 off-phase RELs in a day of steady running"), steady_rels, steady_off == 0),
            ),
            // (G1's Y4: each control bites on ITS bar's own predicate over samples it has: (a) a REL-to-REL measured under
            // the bar, never an empty run; (b) the recovery predicate over failed-roll scenarios, not the lock bar's)
            Some('a') => (v.control(&format!("{label} fails (a)"), if worst_int == i64::MAX || short_runs > 0 { 0 } else { n as usize }, worst_int != i64::MAX && short_runs == 0 && worst_int < bar).to_string(), "printed".to_string(), "printed".to_string(), "printed".to_string()),
            Some(_) => ("printed".to_string(), v.control(&format!("{label} fails (b)"), n_failed as usize, !b_ok).to_string(), "printed".to_string(), "printed".to_string()),
        };
        lines.push(format!(
            "  {label}: (a) the least REL-to-REL {} ({a_w}; at {}{}); (b) from the first golem after a disturbance to the lock, at most {max_cycles} cycles with {max_lost} summons lost (off-phase RELs + skipped slots, a pulse with no REL in its cycle){}; after a failed roll ({n_failed} scenarios) at most {max_lost_failed} summons lost beyond the roll's own ({b_w}); not locked at the scenario's end {unlocked} of {n}{} ({lock_w}); (c) off-phase RELs in a day of steady running {steady_off} ({c_w})",
            if worst_int == i64::MAX { "NONE (fewer than two RELs)".to_string() } else { worst_int.to_string() },
            worst_int_at,
            if short_runs > 0 { format!("; {short_runs} scenario(s) with fewer than two RELs: NOT JUDGED") } else { String::new() },
            if max_at.is_empty() { String::new() } else { format!(", the worst at {max_at}") },
            if unlocked_at.is_empty() { String::new() } else { format!(" (first: {unlocked_at})") }
        ));
    }
    Ok(lines)
}
