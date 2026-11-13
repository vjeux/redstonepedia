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
//! - Real skips: the bot lies down at a day time in a weather, the server skips the night once it
//!   has slept long enough; the detector's readings at each update after the skip, and the day time
//!   it first reads 8. (On this frozen, stepped server the bot's own sleep counter runs on between
//!   steps, so the skip comes early, 45-49 stepped gt: `--latency` measures it on a running
//!   server, 100 gt, `isSleepingLongEnough`; the readings after the skip run on game time.)
//! - Rain ending at dusk: the readings as the rain fades from about 11,700.

use anyhow::{Context, Result, bail};
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

/// (tickexact 2026-11-13, ironspec 06:08 real (5) / cc767b78: the DAY line's off-edge per weather)
/// `mctas mobai daylight --edges`: an open detector; for clear, rain and thunder (each set and held
/// 120 + 200 gt so both levels sit at their end values before any reading), the first day time in
/// 10,000..13,000 whose detector update reads <= 5 (the DAY line's L >= 6 turning off), found by
/// bisection on single updates at frozen day times, with the readings from 40 before it to 40 after
pub fn edges(version: &str) -> Result<()> {
    println!("== daylight --edges on the official {version} server: the first dusk update reading <= 5 (DAY off at L = 6) per weather, each weather held 320 gt first");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    })?;
    // the detector's chunk held loaded with nobody online
    srv.cmd_ok(&format!("forceload add {} {}", DETECTOR[0], DETECTOR[2]))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule advance_weather true")?;
    srv.cmd_ok("fill -4 63 -4 8 63 4 minecraft:stone")?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:daylight_detector", DETECTOR[0], DETECTOR[1], DETECTOR[2]))?;
    srv.step(20)?;
    let mut found = 0;
    let mut bad: Vec<String> = Vec::new();
    // (G1 on G #4) the off-edges asserted: the values redsim's own model gives and the first run read
    let expected = [("clear", 12230i64), ("rain", 12002), ("thunder", 11556)];
    for w in ["clear", "rain", "thunder"] {
        set_weather(&mut srv, w)?;
        srv.cmd_ok("time set 6000")?;
        srv.step(200)?;
        let e = first_where(&mut srv, 10000, 13000, |p| p <= 5)?;
        // every update from 10,800 to 12,600 (cc767b78's per-update comparison)
        let mut every = Vec::new();
        let mut scan: Vec<(i64, i64)> = Vec::new();
        for d in (10800..=12600).step_by(20) {
            let l = sample_at(&mut srv, d)?;
            every.push(format!("{d}:{l}"));
            scan.push((d, l as i64));
        }
        println!("  {w}: every update 10,800..12,600 (day time:level): {}", every.join(" "));
        // the bisection against the scan: the edge must lie after the scan's last update reading >= 6
        // before its first reading <= 5, and at or before that first reading
        let first_low = scan.iter().find(|(_, l)| *l <= 5).map(|(d, _)| *d);
        let last_high = first_low.and_then(|f| scan.iter().filter(|(d, l)| *d < f && *l >= 6).map(|(d, _)| *d).last());
        match (e, first_low, last_high) {
            (Some(e), Some(f), Some(h)) if h < e && e <= f => {}
            (e, f, h) => bad.push(format!("{w}: the bisected edge {e:?} is not inside the scan's step ({h:?}, {f:?}]")),
        }
        if let Some(&(_, want)) = expected.iter().find(|(n, _)| *n == w)
            && e != Some(want)
        {
            bad.push(format!("{w}: the edge {e:?}, expected {want}"));
        }
        let around = match e {
            Some(e) => {
                found += 1;
                let mut seq = Vec::new();
                for d in (e - 40..=e + 40).step_by(10) {
                    seq.push(format!("{d}:{}", sample_at(&mut srv, d)?));
                }
                seq.join(" ")
            }
            None => "-".into(),
        };
        let noon = sample_at(&mut srv, 6000)?;
        println!("  {w}: noon reads {noon}; DAY off (first update <= 5) at day time {}; around it (day time:level) {around}", e.map(|x| x.to_string()).unwrap_or_else(|| "none in 10,000..13,000".into()));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    anyhow::ensure!(found == 3, "{} of 3 weathers found no edge: NOT JUDGED, the run fails", 3 - found);
    println!("  bars (each edge = its expected value, inside its 20-gt scan step): {}", if bad.is_empty() { "HELD".to_string() } else { format!("FAILED: {}", bad.join("; ")) });
    anyhow::ensure!(bad.is_empty(), "{}", bad.join("; "));
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

/// (G1 on 6960525a1: the frozen runner's stepped ticks let the bot's own sleep counter run on
/// between steps, so the skip latency is measured on a running server) `mctas mobai daylight
/// --latency`: the official server unfrozen at 20 tps, the bot lies down in a bed at night with
/// time running; the game time from its lying down to the day time's jump, 3 times.
pub fn skip_latency(version: &str) -> Result<()> {
    println!("== daylight --latency on the official {version} server, running unfrozen at 20 tps: the game ticks from the bot lying down to the night skip");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: true,
        frozen: false,
        biome: "minecraft:plains".into(),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("gamerule advance_weather true")?;
    srv.cmd_ok("weather clear 1000000")?;
    srv.cmd_ok("fill -4 63 -4 8 63 4 minecraft:stone")?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=foot]", BED_HEAD[0], BED_HEAD[1], BED_HEAD[2] + 1))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=head]", BED_HEAD[0], BED_HEAD[1], BED_HEAD[2]))?;
    srv.teleport_bot(BED_HEAD[0] as f64 + 1.5, 64.0, BED_HEAD[2] as f64 + 0.5)?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    for k in 0..3 {
        srv.cmd_ok("time set 14000")?;
        std::thread::sleep(std::time::Duration::from_millis(200));
        srv.bot_use(BED_HEAD, 1)?;
        let mut lie = None;
        let start = std::time::Instant::now();
        let mut prev_day = day_time(&mut srv)?;
        let mut jumped = None;
        while start.elapsed() < std::time::Duration::from_secs(20) {
            let g = srv.game_time()?;
            let d = day_time(&mut srv)?;
            if lie.is_none() && srv.test_block(BED_HEAD, "#minecraft:beds[occupied=true]")? {
                lie = Some(g);
            }
            if d < prev_day || d - prev_day > 40 {
                jumped = Some(g);
                break;
            }
            prev_day = d;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        match (lie, jumped) {
            (Some(l), Some(j)) => println!("  skip {k}: the bed occupied from game time <= {l}, the day time jumped by game time {j}: {} gt (bounded by the polling, about 1 gt each way)", j - l),
            _ => println!("  skip {k}: lay down {lie:?}, jumped {jumped:?}"),
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}

/// (tickexact 2026-11-13, ironspec 00:36 PT: cc767b78's weather / time order) The daylight
/// detector's DAWN EDGE on the official server at 20 tps (unfrozen): the model's first clear day
/// time with level >= 1 is D (this file's `level`, 26.3's code); per row the clock is set while
/// frozen so that the detector's next update tick U (game time % 20 == 0) has the post-increment
/// day time D - 1, D, D + 1 or D + 2, and a command-block chain (block-tick phase, before the
/// detector's block-entity update) stamps the game and day time of the first tick it sees power > 0
/// (so the update tick is that stamp - 1). Shows which day time the detector's update reads.
pub fn dawn_edge(version: &str) -> Result<()> {
    let d = (22000..23000).find(|&d| level(d, 0.0, 0.0) >= 1).context("the model never reaches level 1 at dawn")?;
    println!("== daylight --dawn on the official {version} server at 20 tps: the model's first clear day time with level >= 1: D = {d}; rows with the update tick's post-increment day time D - 1 .. D + 2");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, frozen: false, tick_rate: 20, command_blocks: true, ..Default::default() })?;
    srv.cmd_ok("tick freeze")?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("weather clear 1000000")?;
    srv.cmd_ok("gamerule command_block_output false").or_else(|_| srv.cmd_ok("gamerule commandBlockOutput false"))?;
    srv.cmd_ok("forceload add 0 0")?;
    srv.cmd_ok("scoreboard objectives add lc dummy")?;
    let cmds = [
        "execute store result score #g lc run time query gametime",
        "execute store result score #d lc run time query time",
        "execute unless block 0 100 0 minecraft:daylight_detector[power=0] if score #eg lc matches 0 run scoreboard players operation #eg lc = #g lc",
        "execute unless block 0 100 0 minecraft:daylight_detector[power=0] if score #ed lc matches 0 run scoreboard players operation #ed lc = #d lc",
    ];
    for (i, c) in cmds.iter().enumerate() {
        let kind = if i == 0 { "repeating_command_block" } else { "chain_command_block" };
        srv.cmd_ok(&format!("setblock {} 100 0 minecraft:{kind}[facing=east]{{auto:1b,Command:\"{c}\"}}", 2 + i))?;
    }
    let score = |srv: &mut VanillaServer, who: &str| -> Result<i64> {
        let s = srv.cmd(&format!("scoreboard players get {who} lc"))?;
        Ok(s.split(" has ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0))
    };
    // (ironspec 02:28 PT (f3)) a row with no sample fails the run, as in the other tools
    let mut not_judged: Vec<String> = Vec::new();
    for target in [d - 1, d, d + 1, d + 2] {
        srv.cmd_ok("tick freeze")?;
        srv.cmd("setblock 0 100 0 minecraft:air")?;
        srv.cmd_ok("setblock 0 100 0 minecraft:daylight_detector")?;
        srv.cmd_ok("scoreboard players set #eg lc 0")?;
        srv.cmd_ok("scoreboard players set #ed lc 0")?;
        let g0 = srv.game_time()?;
        let u = g0 + (20 - g0.rem_euclid(20));
        let x = target - (u - g0);
        srv.cmd_ok(&format!("time set {x}"))?;
        srv.cmd_ok("tick unfreeze")?;
        std::thread::sleep(std::time::Duration::from_millis(((u - g0 + 50) * 50) as u64));
        srv.cmd_ok("tick freeze")?;
        let (eg, ed) = (score(&mut srv, "#eg")?, score(&mut srv, "#ed")?);
        if eg == 0 {
            not_judged.push(format!("target D{:+}", target - d));
        }
        println!(
            "  target D{:+}: frozen at game time {g0}, the clock set to {x}, so the update tick U = {u} (U % 20 = 0) has the post-increment day time {target}: {}",
            target - d,
            if eg == 0 {
                "the detector never showed power > 0 within ~50 gt: NOT JUDGED (0 samples)".to_string()
            } else {
                format!("first seen on at game time {eg} (day time {ed}): the detector turned on at the update tick {} ({}; its post-increment day time {})", eg - 1, if eg - 1 == u { "= U" } else { "U + 20" }, ed - 1)
            }
        );
    }
    // the day times where the two readings of the update tick disagree: the sky darkening from the
    // pre-increment time (updateSkyBrightness before tickTime) vs from the post-increment time
    let split: Vec<i64> = (0..24000i64).filter(|&t| level_split(t - 1, t, 0.0, 0.0) != level_split(t, t, 0.0, 0.0)).collect();
    println!("  day times where the darkening's pre- and post-increment readings give different levels (clear): {} ({:?}…)", split.len(), split.iter().take(12).collect::<Vec<_>>());
    for &target in split.iter().take(6) {
        srv.cmd_ok("tick freeze")?;
        let g0 = srv.game_time()?;
        let u = g0 + (20 - g0.rem_euclid(20));
        srv.cmd_ok(&format!("time set {}", target - (u - g0)))?;
        srv.cmd_ok("tick unfreeze")?;
        std::thread::sleep(std::time::Duration::from_millis(((u - g0 + 10) * 50) as u64));
        srv.cmd_ok("tick freeze")?;
        let gt = srv.game_time()?;
        let mut power = None;
        for n in 0..16 {
            if srv.cmd(&format!("execute if block 0 100 0 minecraft:daylight_detector[power={n}]"))?.contains("passed") {
                power = Some(n);
                break;
            }
        }
        let inside = gt > u && gt < u + 20;
        if !inside || power.is_none() {
            not_judged.push(format!("split row {target}"));
        }
        println!(
            "  split row: the update tick U = {u} with the post-increment day time {target}, read at game time {gt} ({}): power {:?}; the model with the darkening at {} (pre) gives {}, at {target} (post) gives {}",
            if inside { "after U, before the next update" } else { "OUTSIDE (U, U + 20): NOT JUDGED (0 samples)" },
            power,
            target - 1,
            level_split(target - 1, target, 0.0, 0.0),
            level_split(target, target, 0.0, 0.0)
        );
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    anyhow::ensure!(not_judged.is_empty(), "{} row(s) NOT JUDGED (0 samples): {}: the run fails", not_judged.len(), not_judged.join(", "));
    Ok(())
}

/// (tickexact 2026-11-12, day-1's inhibit level, ironspec) An open daylight detector's level from
/// 26.3's code, no server: `DaylightDetectorBlock.updateSignalStrength` with sky light 15, the
/// sky darkening of the day timeline and the weather layer (`redsim::weather::sky_darken_at`, the
/// effective thunder being thunder x rain) and the sun's angle (`redsim::daylight::sun_angle`),
/// pulled toward noon by 0.2 and rounded (`Math.round`).
fn level(day: i64, rain: f32, thunder: f32) -> i32 {
    level_split(day, day, rain, thunder)
}

/// [`level`] with the sky darkening read at `dark_day` and the sun's angle at `sun_day` (the update
/// tick's pre-increment vs post-increment time: ServerLevel.tick's updateSkyBrightness runs before
/// tickTime, the detector's block-entity update after it).
fn level_split(dark_day: i64, sun_day: i64, rain: f32, thunder: f32) -> i32 {
    let target = 15 - redsim::weather::sky_darken_at(dark_day.rem_euclid(24000), rain, thunder * rain);
    if target <= 0 {
        return 0;
    }
    let mut sun = redsim::daylight::sun_angle(sun_day.rem_euclid(24000)) * (std::f64::consts::PI / 180.0) as f32;
    let offset = if sun < std::f64::consts::PI as f32 { 0.0f32 } else { (std::f64::consts::PI * 2.0) as f32 };
    sun += (offset - sun) * 0.2f32;
    let v = target as f32 * redsim::entity::mth::cos(sun as f64);
    ((v as f64 + 0.5).floor() as i32).clamp(0, 15)
}

/// `mctas mobai daylight --levels`: the table day-1's controller needs (ironspec 02:46).
pub fn levels() -> Result<()> {
    let weathers = [("clear", 0.0f32, 0.0f32), ("rain", 1.0, 0.0), ("thunder", 1.0, 1.0)];
    println!("== an open daylight detector's level from 26.3's code (sky light 15), per day time d");
    println!("1. every d, step 100 (clear / rain / thunder):");
    for d in (0..24000).step_by(100) {
        println!("  d {d:>5}: {:>2} / {:>2} / {:>2}", level(d, 0.0, 0.0), level(d, 1.0, 0.0), level(d, 1.0, 1.0));
    }
    println!("   the steps (every d where a weather's level changes, to the tick):");
    for (name, r, t) in weathers {
        let mut steps = Vec::new();
        let mut prev = level(0, r, t);
        for d in 1..24000 {
            let l = level(d, r, t);
            if l != prev {
                steps.push(format!("{d}:{prev}->{l}"));
                prev = l;
            }
        }
        println!("  {name}: {}", steps.join(" "));
    }
    // a skip resets the weather cycle (`ServerLevel.resetWeatherCycle`): both flags off, so the
    // rain and thunder levels fall 0.01 a tick from where they were, from the jump to day 0
    println!("2. a skip from a storm to dawn (day 0, the rain and thunder levels falling 0.01 a tick):");
    for (name, r0, t0) in [("rain", 1.0f32, 0.0f32), ("thunder", 1.0, 1.0)] {
        let mut seq = Vec::new();
        let mut prev = -1;
        for k in 0..=400i64 {
            let r = (r0 - 0.01 * k as f32).max(0.0);
            let t = (t0 - 0.01 * k as f32).max(0.0);
            let l = level(k, r, t);
            if l != prev {
                seq.push(format!("{k}:{l}"));
                prev = l;
            }
        }
        let night: Vec<String> = [12_500i64, 13_000, 18_000, 23_000, 23_900].iter().map(|d| format!("{d}:{}", level(*d, r0, t0))).collect();
        println!("  {name}: the level before the skip {}; after it (day time:level at each change) {}", night.join(" "), seq.join(" "));
    }
    // the largest change of the level between two readings 20 gt apart, over every day time
    println!("3. the largest change between two readings 20 gt apart (any day time):");
    let mut sun = (0, 0);
    for (_, r, t) in weathers {
        for d in 0..24000 {
            let c = (level(d + 20, r, t) - level(d, r, t)).abs();
            if c > sun.0 {
                sun = (c, d);
            }
        }
    }
    println!("  the sun alone (each weather held): {} (from d {})", sun.0, sun.1);
    // ramps: rain 0.01 a tick (0.2 per reading), thunder 0.01 a tick, effective thunder = thunder x rain
    let ramp = |dr: f32, dt: f32| {
        let mut best = (0, 0i64, 0.0f32, 0.0f32);
        for d in (0..24000).step_by(10) {
            for i in 0..=100 {
                let a = i as f32 / 100.0;
                for j in 0..=100 {
                    let b = j as f32 / 100.0;
                    let (r1, t1) = ((a + dr).clamp(0.0, 1.0), (b + dt).clamp(0.0, 1.0));
                    let c = (level(d + 20, r1, t1) - level(d, a, b)).abs();
                    if c > best.0 {
                        best = (c, d, a, b);
                    }
                }
            }
        }
        best
    };
    for (name, dr, dt) in [("rain rising", 0.2f32, 0.0f32), ("rain falling", -0.2, 0.0), ("thunder rising", 0.0, 0.2), ("thunder falling", 0.0, -0.2), ("both rising", 0.2, 0.2), ("both falling", -0.2, -0.2)] {
        let (c, d, a, b) = ramp(dr, dt);
        println!("  {name} (with the sun): {c} (at d {d}, rain {a:.2}, thunder {b:.2})");
    }
    Ok(())
}

/// (tickexact 2026-11-12, G1 on f451e78d4: the weather layer's official control) `mctas mobai
/// daylight --weather-row`: with the day time held (`advance_time` off) at a few day times, the
/// sky cleared, then `weather thunder` (rain and thunder both rising 0.01 a tick) and later
/// `weather clear` (both falling): the detector read at every update (game time a multiple of
/// 20) against [`level`] with the levels the ramps reach by that tick; then one real thunder
/// skip, its readings after the jump against the model with both levels falling from 1.
pub fn weather_row(version: &str) -> Result<()> {
    println!("== daylight --weather-row on the official {version} server: the detector through rain and thunder ramps against the code's model (daylight --levels)");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: true,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule advance_weather true")?;
    srv.cmd_ok("fill -4 63 -4 8 63 4 minecraft:stone")?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:daylight_detector", DETECTOR[0], DETECTOR[1], DETECTOR[2]))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=foot]", BED_HEAD[0], BED_HEAD[1], BED_HEAD[2] + 1))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=north,part=head]", BED_HEAD[0], BED_HEAD[1], BED_HEAD[2]))?;
    srv.teleport_bot(BED_HEAD[0] as f64 + 1.5, 64.0, BED_HEAD[2] as f64 + 0.5)?;
    srv.step(20)?;
    let (mut readings, mut differ) = (0, 0);
    for d in [1000i64, 6000, 11700, 12400, 23600] {
        set_weather(&mut srv, "clear")?;
        srv.cmd_ok(&format!("time set {d}"))?;
        srv.step(40)?;
        for (cmd, up) in [("thunder", true), ("clear", false)] {
            srv.cmd_ok(&format!("weather {cmd} 1000000"))?;
            let mut seq = Vec::new();
            for k in 1..=140i64 {
                srv.step(1)?;
                if srv.game_time()?.rem_euclid(20) == 0 {
                    let x = (0.01 * k as f32).min(1.0);
                    let lv = if up { x } else { 1.0 - x };
                    let (got, want) = (power(&mut srv)?, level(d, lv, lv));
                    readings += 1;
                    if got != want {
                        differ += 1;
                    }
                    seq.push(format!("{k}:{got}{}", if got == want { String::new() } else { format!("(model {want})") }));
                }
            }
            println!("d {d:>5}, weather {cmd}: (gt since the command:level) {}", seq.join(" "));
        }
    }
    // one real thunder skip: lie down at 23,000 in full thunder; the readings after the jump
    let (lay, after) = skip(&mut srv, "thunder", 23000, 300)?;
    let mut seq = Vec::new();
    for (day, got) in &after {
        let k = day.rem_euclid(24000);
        let x = (1.0 - 0.01 * k as f32).max(0.0);
        let want = level(k, x, x);
        readings += 1;
        if *got != want {
            differ += 1;
        }
        seq.push(format!("{k}:{got}{}", if *got == want { String::new() } else { format!("(model {want})") }));
    }
    println!("a thunder skip (lay down at {}): (day time:level) {}", lay.map(|t| t.to_string()).unwrap_or("-".into()), seq.join(" "));
    println!("{readings} readings, {differ} differ from the model");
    let _ = std::fs::remove_dir_all(&srv.dir);
    if differ > 0 {
        bail!("{differ} readings differ from the model");
    }
    Ok(())
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
