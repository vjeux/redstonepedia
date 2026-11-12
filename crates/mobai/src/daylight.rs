//! (tickexact 2026-11-12, ironspec's daylight bridge for the carousel's re-phase triggers)
//! `mctas mobai daylight`: on the official 26.3 server, a daylight detector under the open sky and
//! the bot beside a bed. Every reading is the detector's own update (it updates when the game time
//! is a multiple of 20): `sample_at(t)` sets the day time to t one tick before an update (time
//! frozen by `advance_time false`), so the update reads exactly t.
//! - The clear sky's edges to the tick: the first rise 0 -> 1 at dawn, 7 -> 8 at dawn, 8 -> 7 at
//!   dusk, 1 -> 0 at dusk.
//! - Rain and thunder readings at fixed times.
//! - Bed times: the earliest evening and latest morning day time the bot can lie down, in clear
//!   weather, rain and thunder (thunder: noon too).
//! - Real skips: the bot lies down at a day time in a weather, the server skips the night after its
//!   100 gt asleep; the detector's readings at each update after the skip, and the day time it
//!   first reads 8.
//! - Rain ending at dusk: the readings as the rain fades from about 11,700.

use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DETECTOR: [i32; 3] = [0, 64, 0];
const BED_HEAD: [i32; 3] = [4, 64, 0];

fn power(srv: &mut VanillaServer) -> Result<i32> {
    for n in 0..=15 {
        if srv.test_block(DETECTOR, &format!("minecraft:daylight_detector[power={n}]"))? {
            return Ok(n);
        }
    }
    bail!("the detector reads no power state")
}

fn day_time(srv: &mut VanillaServer) -> Result<i64> {
    // (26.3: the overworld clock's total ticks; `time query daytime` is gone)
    let out = srv.cmd("time query time")?;
    out.split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter(|s| !s.is_empty())
        .last()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("time query time: {out}"))
}

/// Steps to the tick before the detector's next update (a game time of 19 mod 20).
fn to_pre_update(srv: &mut VanillaServer) -> Result<()> {
    let g = srv.game_time()?;
    let k = (19 - g.rem_euclid(20)).rem_euclid(20);
    if k > 0 {
        srv.step(k as u32)?;
    }
    Ok(())
}

/// The detector's reading at day time `t` (time frozen).
fn sample_at(srv: &mut VanillaServer, t: i64) -> Result<i32> {
    to_pre_update(srv)?;
    srv.cmd_ok(&format!("time set {t}"))?;
    srv.step(1)?;
    power(srv)
}

/// The first day time in [lo, hi] where `pred(level)` holds, the level being monotone across it
/// (pred false at lo, true at hi).
fn first_where(srv: &mut VanillaServer, mut lo: i64, mut hi: i64, pred: impl Fn(i32) -> bool) -> Result<Option<i64>> {
    if pred(sample_at(srv, lo)?) || !pred(sample_at(srv, hi)?) {
        return Ok(None);
    }
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if pred(sample_at(srv, mid)?) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(Some(hi))
}

fn set_weather(srv: &mut VanillaServer, w: &str) -> Result<()> {
    srv.cmd_ok(&format!("weather {w} 1000000"))?;
    // rain and thunder levels move 0.01 a tick
    srv.step(120)?;
    Ok(())
}

/// Whether the bot can lie down at day time `t` (time frozen) in weather `w`; wakes it again (in
/// thunder a sleeper stays asleep at noon: the sky is cleared until it wakes, then `w` restored).
fn can_sleep(srv: &mut VanillaServer, t: i64, w: &str) -> Result<bool> {
    srv.cmd_ok(&format!("time set {t}"))?;
    srv.step(1)?;
    srv.bot_use(BED_HEAD, 1)?;
    srv.step(2)?;
    let asleep = srv.test_block(BED_HEAD, "#minecraft:beds[occupied=true]")?;
    if asleep {
        srv.cmd_ok("time set 6000")?;
        srv.step(3)?;
        if srv.test_block(BED_HEAD, "#minecraft:beds[occupied=true]")? {
            srv.cmd_ok("weather clear 1000000")?;
            let mut woke = false;
            for _ in 0..30 {
                srv.step(10)?;
                if !srv.test_block(BED_HEAD, "#minecraft:beds[occupied=true]")? {
                    woke = true;
                    break;
                }
            }
            if !woke {
                bail!("the bot didn't wake at noon under a clearing sky");
            }
            set_weather(srv, w)?;
        }
    }
    Ok(asleep)
}

/// The boundary day time in [lo, hi] where the bot's ability to lie down flips (monotone).
fn bed_edge(srv: &mut VanillaServer, mut lo: i64, mut hi: i64, w: &str) -> Result<Option<i64>> {
    let a = can_sleep(srv, lo, w)?;
    let b = can_sleep(srv, hi, w)?;
    if a == b {
        return Ok(None);
    }
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if can_sleep(srv, mid, w)? == b {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(Some(hi))
}

/// A real skip: the weather, the bot lies down at day time `t`, time runs; returns the readings
/// (day time, level) at each detector update for `after` gt once the day time jumps.
fn skip(srv: &mut VanillaServer, weather: &str, t: i64, after: u32) -> Result<(Option<i64>, Vec<(i64, i32)>)> {
    set_weather(srv, weather)?;
    srv.cmd_ok(&format!("time set {t}"))?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.step(1)?;
    srv.bot_use(BED_HEAD, 1)?;
    srv.step(2)?;
    if !srv.test_block(BED_HEAD, "#minecraft:beds[occupied=true]")? {
        srv.cmd_ok("gamerule advance_time false")?;
        return Ok((None, Vec::new()));
    }
    let mut prev = day_time(srv)?;
    let mut jumped = None;
    for _ in 0..200 {
        srv.step(1)?;
        let d = day_time(srv)?;
        if d < prev || d - prev > 1 {
            jumped = Some((prev, d));
            break;
        }
        prev = d;
    }
    let mut readings = Vec::new();
    if jumped.is_some() {
        let mut n = 0;
        while n < after {
            srv.step(1)?;
            n += 1;
            if srv.game_time()?.rem_euclid(20) == 0 {
                readings.push((day_time(srv)?, power(srv)?));
            }
        }
    }
    srv.cmd_ok("gamerule advance_time false")?;
    Ok((jumped.map(|j| j.0), readings))
}

pub fn daylight(version: &str) -> Result<()> {
    println!("== daylight on the official {version} server: a daylight detector under the open sky, the bot beside a bed");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: true,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    // (the weather cycle on, as in a world in play — the runner turns it off: a skip resets rain and
    // thunder only when it runs; each weather set below lasts 1,000,000 gt)
    srv.cmd_ok("gamerule advance_weather true")?;
    srv.cmd_ok("fill -4 63 -4 8 63 4 minecraft:stone")?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:daylight_detector", DETECTOR[0], DETECTOR[1], DETECTOR[2]))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=foot]", BED_HEAD[0], BED_HEAD[1], BED_HEAD[2] + 1))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=head]", BED_HEAD[0], BED_HEAD[1], BED_HEAD[2]))?;
    srv.teleport_bot(BED_HEAD[0] as f64 + 1.5, 64.0, BED_HEAD[2] as f64 + 0.5)?;
    srv.step(20)?;

    // the clear sky's edges
    set_weather(&mut srv, "clear")?;
    let rise = first_where(&mut srv, 21000, 23500, |l| l >= 1)?;
    let dawn8 = first_where(&mut srv, 23500, 24800, |l| l >= 8)?;
    let dusk7 = first_where(&mut srv, 11000, 12600, |l| l < 8)?;
    let fall0 = first_where(&mut srv, 12600, 14500, |l| l < 1)?;
    let show = |t: Option<i64>| t.map(|t| t.to_string()).unwrap_or("-".into());
    println!(
        "clear sky: first rise 0 -> 1 at day time {} ; 7 -> 8 at dawn {} ; 8 -> 7 at dusk {} ; 1 -> 0 at dusk {}",
        show(rise),
        show(dawn8),
        show(dusk7),
        show(fall0)
    );
    if let (Some(a), Some(b)) = (dawn8, dusk7) {
        println!("  the 8 band: from {} past midnight to {}; its mirror about noon: {} before, {} after", a - 24000, b, 6000 - (a - 24000), b - 6000);
    }

    // rain and thunder readings
    let times = [6000i64, 11000, 11500, 11800, 12000, 12200, 12500, 13000, 22000, 22500, 23000, 23500, 24000, 24160, 24500];
    for w in ["clear", "rain", "thunder"] {
        set_weather(&mut srv, w)?;
        let mut line = format!("{w}:");
        for &t in &times {
            line.push_str(&format!(" {t}:{}", sample_at(&mut srv, t)?));
        }
        println!("{line}");
    }

    // bed times
    for w in ["clear", "rain", "thunder"] {
        set_weather(&mut srv, w)?;
        let eve = bed_edge(&mut srv, 11000, 13500, w)?;
        let morn = bed_edge(&mut srv, 22500, 24500, w)?;
        let noon = can_sleep(&mut srv, 6000, w)?;
        println!(
            "bed in {w}: the bot can lie down from {} in the evening, until {} in the morning; at noon {}",
            show(eve),
            show(morn.map(|m| m - 1)),
            if noon { "yes" } else { "no" }
        );
    }

    // real skips
    let cases: [(&str, i64); 9] = [
        ("clear", 12560),
        ("rain", 12020),
        ("rain", 12540),
        ("rain", 14000),
        ("thunder", 6000),
        ("thunder", 12542),
        ("thunder", 14000),
        ("clear", 23200),
        ("rain", 23300),
    ];
    for (w, t) in cases {
        let (from, readings) = skip(&mut srv, w, t, 400)?;
        match from {
            None => println!("skip in {w}, lying down at {t}: no skip (the bot couldn't lie down or the night didn't skip)"),
            Some(f) => {
                let eight = readings.iter().find(|(_, l)| *l >= 8).map(|(d, _)| *d);
                let seq: Vec<String> = readings.iter().take(12).map(|(d, l)| format!("{}:{l}", d.rem_euclid(24000))).collect();
                println!(
                    "skip in {w}, lying down at {t}: the day time jumped from {f}; first reads 8 at day time {}; readings {}",
                    eight.map(|e| e.rem_euclid(24000).to_string()).unwrap_or("-".into()),
                    seq.join(" ")
                );
            }
        }
    }

    // rain ending at dusk
    set_weather(&mut srv, "rain")?;
    srv.cmd_ok("time set 11700")?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("weather clear 1000000")?;
    let mut seq = Vec::new();
    for _ in 0..400 {
        srv.step(1)?;
        if srv.game_time()?.rem_euclid(20) == 0 {
            seq.push(format!("{}:{}", day_time(&mut srv)?.rem_euclid(24000), power(&mut srv)?));
        }
    }
    srv.cmd_ok("gamerule advance_time false")?;
    println!("rain ending at 11,700: {}", seq.join(" "));
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
