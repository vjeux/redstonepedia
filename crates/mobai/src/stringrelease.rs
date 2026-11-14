//! (tickexact 2026-11-13, ironspec 21:21 (3) / 21:42: a real P_STRING's release on a test rig) `mctas mobai
//! string-release`: on the official 26.3 server, the stop's essentials in a fresh world: P a straight north-south rail
//! (P straight, ironspec 21:39) between rails north and south of it, the bed beside P (its head west of P, facing
//! east), P_STRING one cell above P between hooks north and south on glass one further out (58f770cc's N-S form), and
//! an observer under the bed head facing up (the wake sensor: OCCUPIED set at the lie-down / cleared at the wake gives
//! it a 2-gt pulse). Each trial in the same rig, the string released and the cell empty first:
//! - (a) the napper: a villager summoned standing on P with HOME the bed head at day time 12000 - k (k = 0..9: the
//!   string's 10-gt re-check grid starts at its press, so k samples every grid phase against the lie-down);
//! - (b) an early wake: the same at k = 0, the sleeper hurt at lie-down + w (w = 0, 1, 2, 3, 4, 8): it wakes, stands up;
//! - (c) a wanderer: a villager with no HOME standing on P by day (IDLE) for 100 gt, then removed.
//! Every tick: the string's and both hooks' powered states, the observer's, the villager's position and sleep.
//! Printed per trial: the string's press tick(s), the lie-down tick L (sleeping_pos first set), the observer's powered
//! rows, the string's drop after L, drop - pulse rise against RLS's read at pulse + 12 (ctl-shutter's model: by pulse
//! + 8). The 26.3 bounds are asserted: (1) pressed while standing on P; (2) after the lie-down the string drops at
//! L + 1 .. L + 10 (its own 10-gt re-check, TripWireBlock.tick 154-162; entityInside never releases); (3) the
//! observer's pulse rises at L + 2 (2 gt); (4) drop - pulse rise <= + 8; (5) the wanderer holds it while on P and
//! no pulse; (6) an empty rig never presses. MCTAS_STRING_RELEASE_PLANT=<k> inverts assert k.

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 101;
const P: [i32; 3] = [0, Y, 0];
const HEAD: [i32; 3] = [-1, Y, 0];
const FOOT: [i32; 3] = [-2, Y, 0];
const STRING: [i32; 3] = [0, Y + 1, 0];
const OBS: [i32; 3] = [-1, Y - 1, 0];

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
    Ok(())
}

fn pos(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let v: Vec<f64> = out.split('[').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None })
}

fn sleeping(srv: &mut VanillaServer, sel: &str) -> Result<bool> {
    Ok(srv.cmd_long(&format!("data get entity {sel} sleeping_pos"))?.contains("entity data"))
}

fn build(srv: &mut VanillaServer) -> Result<()> {
    srv.cmd_ok(&format!("fill -6 {} -6 6 {} 6 minecraft:stone", Y - 2, Y - 1))?;
    srv.cmd_ok(&format!("fill -6 {Y} -6 6 {} 6 minecraft:air", Y + 4))?;
    set(srv, P, "minecraft:rail[shape=north_south]")?;
    set(srv, OBS, "minecraft:observer[facing=up,powered=false]")?;
    set(srv, [-1, Y - 2, 0], "minecraft:stone")?;
    set(srv, FOOT, "minecraft:red_bed[facing=east,occupied=false,part=foot]")?;
    set(srv, HEAD, "minecraft:red_bed[facing=east,occupied=false,part=head]")?;
    set(srv, [0, Y + 1, -2], "minecraft:glass")?;
    set(srv, [0, Y + 1, 2], "minecraft:glass")?;
    set(srv, [0, Y + 1, -1], "minecraft:tripwire_hook[attached=false,facing=south,powered=false]")?;
    set(srv, [0, Y + 1, 1], "minecraft:tripwire_hook[attached=false,facing=north,powered=false]")?;
    set(srv, STRING, "minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]")?;
    srv.step(5)?;
    anyhow::ensure!(srv.test_block(STRING, "minecraft:tripwire[attached=true]")?, "the string did not attach");
    // the enclosure (as the stop's): P the bed's only free neighbour (findStandUpPosition's only cell), glass round
    // the bed and round P two high, the cells north and south of P under the hooks glass with glass over the hooks
    for c in [[-1, Y, -1], [-1, Y, 1], [-2, Y, -1], [-2, Y, 1], [-3, Y, -1], [-3, Y, 0], [-3, Y, 1], [0, Y, -1], [0, Y, 1], [1, Y, 0]] {
        set(srv, c, "minecraft:glass")?;
    }
    for c in [[-1, Y + 1, -1], [-1, Y + 1, 1], [-2, Y + 1, -1], [-2, Y + 1, 1], [-3, Y + 1, -1], [-3, Y + 1, 0], [-3, Y + 1, 1], [1, Y + 1, 0], [0, Y + 2, -1], [0, Y + 2, 1]] {
        set(srv, c, "minecraft:glass")?;
    }
    Ok(())
}

/// One trial's record.
struct Trial {
    label: String,
    presses: Vec<u32>,
    releases: Vec<u32>,
    lie: Option<u32>,
    wake: Option<u32>,
    obs_rows: Vec<u32>,
    /// whether the villager stood on P on the row before its lie-down
    on_p_before_lie: bool,
    /// the cell it stood on at the first row after its wake
    stood_up: Option<[i32; 3]>,
    /// the first row (after the summon) it was not on P, if any
    left_p: Option<u32>,
}

/// How a trial starts: where the villager is summoned, and when (if) it is moved onto P.
struct Start {
    /// summon position (world)
    at: [f64; 3],
    /// teleported onto P before this tick
    tp_at: Option<u32>,
}

/// One trial: the villager summoned (`start`; HOME the bed head when `home`) at day time `day`, the sleeper hurt at
/// lie-down + `hurt_after` when given, the villager teleported far away at `away_at`; `ticks` gt watched.
#[allow(clippy::too_many_arguments)]
fn trial(srv: &mut VanillaServer, label: &str, day: i64, home: bool, start: Start, hurt_after: Option<u32>, ticks: u32, away_at: Option<u32>) -> Result<Trial> {
    srv.cmd(&format!("tp @e[type=minecraft:villager] 0.5 {Y} 12.5"))?;
    srv.cmd("kill @e[type=minecraft:villager]")?;
    srv.cmd_ok(&format!("time set {day}"))?;
    srv.step(25)?;
    anyhow::ensure!(!srv.test_block(STRING, "minecraft:tripwire[powered=true]")?, "{label}: the string still powered before the trial");
    srv.cmd_ok(&format!("time set {day}"))?;
    let brain = if home { format!(",Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}", HEAD[0], HEAD[1], HEAD[2]) } else { String::new() };
    srv.cmd_ok(&format!("summon minecraft:villager {} {} {} {{PersistenceRequired:1b,Tags:[\"rlsv\"]{brain}}}", start.at[0], start.at[1], start.at[2]))?;
    let sel = "@e[tag=rlsv,limit=1]";
    let mut t = Trial { label: label.to_string(), presses: Vec::new(), releases: Vec::new(), lie: None, wake: None, obs_rows: Vec::new(), on_p_before_lie: false, stood_up: None, left_p: None };
    let mut was = false;
    let mut on_p_prev = false;
    println!(
        "  trial {label}: day time {day}, HOME {}, summoned at ({:.3},{:.4},{:.3}){}",
        if home { "the bed head" } else { "none" },
        start.at[0],
        start.at[1] - Y as f64,
        start.at[2],
        start.tp_at.map(|d| format!(", teleported onto P before t{d}")).unwrap_or_default()
    );
    for tick in 1..=ticks {
        if start.tp_at == Some(tick) {
            srv.cmd_ok(&format!("tp {sel} 0.5 {Y} 0.5"))?;
        }
        srv.step(1)?;
        let s = srv.test_block(STRING, "minecraft:tripwire[powered=true]")?;
        let o = srv.test_block(OBS, "minecraft:observer[powered=true]")?;
        let p = pos(srv, sel)?;
        let zz = p.is_some() && sleeping(srv, sel)?;
        if s && !was {
            t.presses.push(tick);
        }
        if !s && was {
            t.releases.push(tick);
        }
        was = s;
        if o {
            t.obs_rows.push(tick);
        }
        let on_p = p.is_some_and(|p| p[0].floor() as i32 == P[0] && p[2].floor() as i32 == P[2] && (p[1] - Y as f64).abs() < 0.5);
        if zz && t.lie.is_none() {
            t.lie = Some(tick);
            t.on_p_before_lie = on_p_prev;
        }
        if !zz && t.lie.is_some() && t.wake.is_none() && p.is_some() {
            t.wake = Some(tick);
            t.stood_up = p.map(|p| [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
        }
        on_p_prev = on_p;
        if !on_p && t.left_p.is_none() && (start.tp_at.is_none_or(|d| tick > d)) {
            t.left_p = Some(tick);
        }
        if let (Some(l), Some(h)) = (t.lie, hurt_after)
            && tick == l + h
        {
            srv.cmd_ok(&format!("damage {sel} 1 minecraft:generic"))?;
        }
        if away_at == Some(tick) {
            srv.cmd_ok(&format!("tp {sel} 0.5 {Y} 12.5"))?;
        }
        println!(
            "    t{tick:>3}: string {s} | observer {o} | villager {}{}",
            p.map(|p| format!("({:.3},{:.4},{:.3})", p[0], p[1] - Y as f64, p[2])).unwrap_or_else(|| "-".into()),
            if zz { " ASLEEP" } else { "" }
        );
    }
    // (no corpse left on P: a killed villager lies 20 gt in its death animation, its box still in the string's band)
    srv.cmd(&format!("tp @e[tag=rlsv] 0.5 {Y} 12.5"))?;
    srv.cmd("kill @e[tag=rlsv]")?;
    let rise = t.lie.and_then(|l| t.obs_rows.iter().copied().find(|r| *r >= l));
    let drop = t.lie.and_then(|l| t.releases.iter().copied().find(|r| *r > l));
    println!(
        "    => presses {:?}, releases {:?}, lie-down {:?}, wake {:?}, observer powered rows {:?}; drop - L {} | drop - pulse rise {}",
        t.presses,
        t.releases,
        t.lie,
        t.wake,
        t.obs_rows,
        match (drop, t.lie) {
            (Some(d), Some(l)) => format!("{:+}", d as i64 - l as i64),
            _ => "-".into(),
        },
        match (drop, rise) {
            (Some(d), Some(r)) => format!("{:+} (RLS reads at + 12)", d as i64 - r as i64),
            _ => "-".into(),
        }
    );
    Ok(t)
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_STRING_RELEASE_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

pub fn string_release(version: &str) -> Result<()> {
    println!("== string-release on the official {version} server (ironspec 21:21 (3), 21:42): P a straight rail {:?}, the bed head {:?} (facing east), P_STRING {:?} (hooks N / S on glass), the observer {:?} facing up at the head", P, HEAD, STRING, OBS);
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("forceload add -16 -16 15 15")?;
    srv.step(20)?;
    build(&mut srv)?;
    // (6) the empty rig
    srv.cmd("kill @e[type=minecraft:villager]")?;
    let mut empty_presses = 0;
    for _ in 0..40 {
        srv.step(1)?;
        if srv.test_block(STRING, "minecraft:tripwire[powered=true]")? {
            empty_presses += 1;
        }
    }
    println!("  the empty rig: the string powered on {empty_presses} of 40 ticks");
    // the bed top (off P: its box misses the string's band) and P
    let bed_top = [-0.5, Y as f64 + 0.5625, 0.5];
    let on_p = [0.5, Y as f64, 0.5];
    // (a) every grid phase: summoned on the bed top at 11999 (its brain in IDLE: its first SleepInBed try comes about 20
    // gt after the summon), moved onto P before tick d (the press the tick after), so the lie-down falls at every phase
    // of the string's 10-gt re-check grid
    let mut naps = Vec::new();
    for d in 1..=12u32 {
        naps.push(trial(&mut srv, &format!("(a) napper onto P before t{d}"), 11999, true, Start { at: bed_top, tp_at: Some(d) }, None, 70, None).with_context(|| format!("napper d {d}"))?);
    }
    // (d) the lie-down at once: summoned on P at 12000 (its brain built in REST): it lies down on its first tick
    let at_once = trial(&mut srv, "(d) on P and asleep at once", 12000, true, Start { at: on_p, tp_at: None }, None, 40, None)?;
    // (b) early wakes: onto P before t5, the sleeper hurt at L + w
    let mut wakes = Vec::new();
    for w in [0u32, 1, 2, 3, 4, 8] {
        wakes.push(trial(&mut srv, &format!("(b) early wake at L + {w}"), 11999, true, Start { at: bed_top, tp_at: Some(5) }, Some(w), 90, None).with_context(|| format!("wake w {w}"))?);
    }
    // (c) a wanderer: no HOME, on P by day, teleported away at t100
    let wanderer = trial(&mut srv, "(c) wanderer", 6000, false, Start { at: on_p, tp_at: None }, None, 130, Some(100))?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    // the asserts (the naps judged only where the villager stood on P on the row before its lie-down)
    let judged: Vec<&Trial> = naps.iter().filter(|t| t.on_p_before_lie).collect();
    println!("  naps judged (on P on the row before the lie-down): {} of {}", judged.len(), naps.len());
    pin(1, judged.len() >= 10 && judged.iter().all(|t| !t.presses.is_empty()), format!("(1) at least 10 of the 12 naps lie down from P, each having pressed the string: {} judged, presses {:?}", judged.len(), judged.iter().map(|t| t.presses.first().copied()).collect::<Vec<_>>()))?;
    let lags: Vec<Option<i64>> = judged.iter().map(|t| t.lie.and_then(|l| t.releases.iter().copied().find(|r| *r > l).map(|d| d as i64 - l as i64))).collect();
    pin(2, lags.iter().all(|g| g.is_some_and(|g| (1..=10).contains(&g))), format!("(2) after the lie-down the string drops at L + 1 .. L + 10: {lags:?}"))?;
    let rises: Vec<Option<i64>> = judged.iter().map(|t| t.lie.and_then(|l| t.obs_rows.iter().copied().find(|r| *r >= l).map(|r| r as i64 - l as i64))).collect();
    pin(3, rises.iter().all(|r| *r == Some(2)), format!("(3) the observer's pulse rises at L + 2: {rises:?}"))?;
    let vs_pulse: Vec<Option<i64>> = lags.iter().zip(rises.iter()).map(|(g, r)| g.zip(*r).map(|(g, r)| g - r)).collect();
    pin(4, vs_pulse.iter().all(|v| v.is_some_and(|v| v <= 8)), format!("(4) drop - pulse rise <= + 8 (RLS reads at + 12): {vs_pulse:?}"))?;
    // (5) the wanderer: held while on P, released at the first re-check after it leaves P (its own stroll, e.g. up onto
    // the bed top, or the teleport at t100), no observer pulse
    let left = wanderer.left_p;
    pin(
        5,
        wanderer.presses.first().is_some_and(|p| *p <= 2) && wanderer.obs_rows.is_empty() && left.is_some_and(|l| wanderer.releases.first().is_some_and(|r| *r >= l && *r <= l + 10)),
        format!("(5) the wanderer holds the string while on P, released within 10 gt of leaving P (left at {left:?}), no observer pulse: presses {:?} releases {:?} observer {:?}", wanderer.presses, wanderer.releases, wanderer.obs_rows),
    )?;
    println!("  the wanderer left P at row {left:?} ({})", if left.is_some_and(|l| l < 100) { "by its own stroll, before the teleport" } else { "by the teleport" });
    pin(6, empty_presses == 0, format!("(6) the empty rig never presses: {empty_presses}"))?;
    pin(7, at_once.lie == Some(1) && at_once.presses.is_empty(), format!("(7) on P and asleep at once: lies down at t1 and never presses the string: lie {:?}, presses {:?}", at_once.lie, at_once.presses))?;
    pin(8, wakes.iter().all(|w| w.stood_up == Some(P)), format!("(8) every woken sleeper stands up on P (the bed's only free neighbour): {:?}", wakes.iter().map(|w| w.stood_up).collect::<Vec<_>>()))?;
    println!("  napper lags (drop - L, judged naps): {lags:?}; drop - pulse rise: {vs_pulse:?}");
    for w in &wakes {
        println!("  {}: lie-down {:?}, wake {:?}, stood up {:?}, observer rows {:?}, string presses {:?} releases {:?}", w.label, w.lie, w.wake, w.stood_up, w.obs_rows, w.presses, w.releases);
    }
    println!("  every known answer held (asserts 1..8)");
    Ok(())
}
