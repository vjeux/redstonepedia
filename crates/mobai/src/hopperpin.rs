//! (tickexact 2026-11-13, ironspec 19:03 (2) / G1 19:10: the hopper / dropper / comparator pins for day-1's
//! ctl-credit-bench) `mctas mobai hopper-pin`: on the official 26.3 server, six rows, every stepped tick printed
//! and each row's known answer ASSERTED (G1 on AD: the run exits 1 when the game disagrees):
//! - (1) a 1-gt pulse into a dropper (3 shears) facing an EMPTY hopper that faces a chest; a comparator reads
//!   the hopper: the dropper fires at step 4 with its signal gone; TransferCooldown 7 right after the entry; the
//!   first push at step 11 (cooldown 8 again); the comparator 3 from step 6 and 0 from step 13;
//! - (2) the same dropper into a hopper already holding one item (facing down into stone: it cannot eject): the
//!   insert sets no cooldown (it stays 0); the comparator 6 from step 6;
//! - (3) a FULL hopper (5 shears) under a chest of 2, facing an empty chest, unpowered at step 0: an eject AND a
//!   suck at step 1 (still 5: a full hopper sucks only after its eject) under one cooldown 8, then steps 9, 17;
//! - (4) hoppers holding 0..5 shears (stack size 1): comparators 0 / 3 / 6 / 9 / 12 / 15;
//! - (4b) the BUILD-ORDER HAZARD: a comparator set beside an already filled hopper reads 0 until a neighbour
//!   update;
//! - (5) a dry dropper pulsed: nothing moves.
//! Step k is the k-th stepped tick after the command that made the row's edge (the server is frozen between
//! steps; a tick scheduled at the command's game time + d runs in step d).

use anyhow::{Context, Result, ensure};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 101;

fn count(srv: &mut VanillaServer, p: [i32; 3]) -> Result<usize> {
    let s = srv.cmd_long(&format!("data get block {} {} {} Items", p[0], p[1], p[2]))?;
    Ok(s.matches("minecraft:shears").count())
}

fn nbt_int(srv: &mut VanillaServer, p: [i32; 3], path: &str) -> Result<i64> {
    let s = srv.cmd_long(&format!("data get block {} {} {} {path}", p[0], p[1], p[2]))?;
    let tail = s.rsplit(':').next().unwrap_or("").trim();
    let num: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    num.parse::<i64>().with_context(|| format!("no {path} at {p:?}: {s}"))
}

fn items(n: usize) -> String {
    let v: Vec<String> = (0..n).map(|i| format!("{{Slot:{i}b,id:\"minecraft:shears\",count:1}}")).collect();
    format!("{{Items:[{}]}}", v.join(","))
}

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
    Ok(())
}

/// One printed step of a dropper row: the dropper's triggered flag and items, the hopper's items and
/// TransferCooldown, the chest's items, the comparator's output.
#[derive(Clone, Copy)]
struct DropStep {
    trig: bool,
    dropper: usize,
    hopper: usize,
    cooldown: i64,
    chest: usize,
    comp: i64,
}

/// One dropper row: a dropper at `base` facing east holding `stock` shears, the hopper east of it (`hopper`
/// state with `pre` shears already in it), a chest east of the hopper, a comparator south of the hopper reading
/// it; a 1-gt pulse from a redstone block west of the dropper at step 0. Returns steps 1..=18 (index k - 1).
fn dropper_row(srv: &mut VanillaServer, label: &str, base: [i32; 3], stock: usize, hopper: &str, pre: usize) -> Result<Vec<DropStep>> {
    let [x, y, z] = base;
    let (dropper, hop, chest, comp, src) = ([x, y, z], [x + 1, y, z], [x + 2, y, z], [x + 1, y, z + 1], [x - 1, y, z]);
    set(srv, dropper, &format!("minecraft:dropper[facing=east]{}", items(stock)))?;
    set(srv, comp, "minecraft:comparator[facing=north]")?;
    set(srv, hop, &format!("{hopper}{}", items(pre)))?;
    set(srv, chest, "minecraft:chest[facing=north]")?;
    srv.step(10)?;
    println!("  ({label}) dropper {stock} shears at {dropper:?} facing east -> hopper {hop:?} `{hopper}` holding {pre} -> chest {chest:?}; comparator {comp:?} reads the hopper; a redstone block at {src:?} for step 1 only");
    println!("    before: hopper TransferCooldown {}, comparator {}", nbt_int(srv, hop, "TransferCooldown")?, nbt_int(srv, comp, "OutputSignal")?);
    set(srv, src, "minecraft:redstone_block")?;
    let mut rows = Vec::new();
    for k in 1..=18 {
        srv.step(1)?;
        if k == 1 {
            set(srv, src, "minecraft:air")?;
        }
        let s = DropStep {
            trig: srv.test_block(dropper, "minecraft:dropper[triggered=true]")?,
            dropper: count(srv, dropper)?,
            hopper: count(srv, hop)?,
            cooldown: nbt_int(srv, hop, "TransferCooldown")?,
            chest: count(srv, chest)?,
            comp: nbt_int(srv, comp, "OutputSignal")?,
        };
        println!(
            "    step {k:>2}: dropper triggered={} items {} | hopper items {} TransferCooldown {} | chest items {} | comparator {}",
            s.trig, s.dropper, s.hopper, s.cooldown, s.chest, s.comp
        );
        rows.push(s);
    }
    Ok(rows)
}

/// The first step (1-based) at which `f` holds, if any.
fn first(rows: &[DropStep], f: impl Fn(&DropStep) -> bool) -> Option<usize> {
    rows.iter().position(f).map(|i| i + 1)
}

/// Row (1)'s known answers. (G1's control) MCTAS_HOPPER_PIN_PLANT=1 plants a wrong expectation (cooldown 8 right
/// after the entry): the run must then fail by name.
fn assert_row1(r: &[DropStep]) -> Result<()> {
    let want_cd = if std::env::var("MCTAS_HOPPER_PIN_PLANT").is_ok() { 8 } else { 7 };
    ensure!(r.iter().all(|s| !s.trig), "(1) the dropper reads triggered after its signal was removed");
    ensure!(first(r, |s| s.dropper == 2) == Some(4) && r[..3].iter().all(|s| s.dropper == 3), "(1) the dropper must fire at step 4 (its signal gone): items {:?}", r.iter().map(|s| s.dropper).collect::<Vec<_>>());
    ensure!(r[3].hopper == 1 && r[3].cooldown == want_cd, "(1) right after the entry (step 4) the hopper must hold 1 with TransferCooldown {want_cd}: {} / {}", r[3].hopper, r[3].cooldown);
    ensure!(first(r, |s| s.chest == 1) == Some(11) && r[10].hopper == 0 && r[10].cooldown == 8, "(1) the first push must land at step 11 (7 after the entry) with the cooldown 8 again");
    ensure!(first(r, |s| s.comp == 3) == Some(6) && r[..5].iter().all(|s| s.comp == 0), "(1) the comparator must read 3 from step 6 (2 gt after the entry)");
    ensure!(r[10..].iter().position(|s| s.comp == 0).map(|i| i + 11) == Some(13), "(1) the comparator must read 0 from step 13 (2 gt after the push)");
    Ok(())
}

/// Row (2)'s known answers.
fn assert_row2(r: &[DropStep]) -> Result<()> {
    ensure!(first(r, |s| s.hopper == 2) == Some(4), "(2) the insert into the holding hopper must land at step 4");
    ensure!(r.iter().all(|s| s.cooldown <= 0), "(2) an insert into a hopper already holding an item must set no cooldown: {:?}", r.iter().map(|s| s.cooldown).collect::<Vec<_>>());
    ensure!(first(r, |s| s.comp == 6) == Some(6), "(2) the comparator must read 6 (2 items) from step 6");
    Ok(())
}

fn full_hopper_row(srv: &mut VanillaServer, base: [i32; 3]) -> Result<()> {
    let [x, y, z] = base;
    let (hop, top, front, power) = ([x, y, z], [x, y + 1, z], [x + 1, y, z], [x, y, z - 1]);
    set(srv, power, "minecraft:redstone_block")?;
    set(srv, hop, &format!("minecraft:hopper[facing=east,enabled=false]{}", items(5)))?;
    set(srv, top, &format!("minecraft:chest[facing=north]{}", items(2)))?;
    set(srv, front, "minecraft:chest[facing=north]")?;
    srv.step(10)?;
    println!("  (3) a FULL hopper {hop:?} (5 shears) facing east into an empty chest {front:?}, a chest of 2 above it {top:?}; held disabled by a redstone block {power:?}, removed at step 0");
    println!("    before: hopper {} top {} front {} TransferCooldown {}", count(srv, hop)?, count(srv, top)?, count(srv, front)?, nbt_int(srv, hop, "TransferCooldown")?);
    set(srv, power, "minecraft:air")?;
    let mut rows: Vec<(usize, i64, usize, usize)> = Vec::new();
    for k in 1..=20 {
        srv.step(1)?;
        let row = (count(srv, hop)?, nbt_int(srv, hop, "TransferCooldown")?, count(srv, top)?, count(srv, front)?);
        println!("    step {k:>2}: hopper items {} TransferCooldown {} | top chest {} | front chest {}", row.0, row.1, row.2, row.3);
        rows.push(row);
    }
    // (the known answer) one eject and one suck at step 1 under one cooldown 8; the next transfers at 9 and 17
    ensure!(rows[0] == (5, 8, 1, 1), "(3) step 1 must show an eject AND a suck under one cooldown 8 (hopper 5, cooldown 8, top 1, front 1): {:?}", rows[0]);
    ensure!(rows[8] == (5, 8, 0, 2), "(3) step 9 must show the second eject + suck: {:?}", rows[8]);
    ensure!(rows[16] == (4, 8, 0, 3), "(3) step 17 must show the third eject with nothing to suck: {:?}", rows[16]);
    let fronts: Vec<usize> = rows.iter().map(|r| r.3).collect();
    let moves: Vec<usize> = (1..fronts.len()).filter(|&i| fronts[i] != fronts[i - 1]).map(|i| i + 1).collect();
    ensure!(moves == vec![9, 17], "(3) the front chest must change only at steps 1, 9 and 17 (its 8-gt cycle): changes after step 1 at {moves:?}");
    Ok(())
}

/// Rows (4) and (4b): the comparator values for 0..5 stack-size-1 items, and the build-order hazard.
fn comparator_rows(srv: &mut VanillaServer) -> Result<()> {
    println!("  (4) hoppers facing down into stone holding n shears (stack size 1), each read by a comparator south of it (each comparator laid first, its hopper set after it)");
    for n in 0..=5usize {
        set(srv, [2 * n as i32, Y, 31], "minecraft:comparator[facing=north]")?;
    }
    srv.step(2)?;
    for n in 0..=5usize {
        set(srv, [2 * n as i32, Y, 30], &format!("minecraft:hopper[facing=down]{}", items(n)))?;
    }
    srv.step(6)?;
    let mut vals = Vec::new();
    for n in 0..=5usize {
        vals.push(nbt_int(srv, [2 * n as i32, Y, 31], "OutputSignal")?);
    }
    println!("    comparator output by item count 0..5: {vals:?}");
    ensure!(vals == vec![0, 3, 6, 9, 12, 15], "(4) 0..5 stack-size-1 items must read 0 / 3 / 6 / 9 / 12 / 15, read {vals:?}");
    // (4b) the hazard: the hopper filled first, the comparator set beside it afterwards
    set(srv, [20, Y, 30], &format!("minecraft:hopper[facing=down]{}", items(3)))?;
    srv.step(2)?;
    set(srv, [20, Y, 31], "minecraft:comparator[facing=north]")?;
    srv.step(6)?;
    let stale = nbt_int(srv, [20, Y, 31], "OutputSignal")?;
    println!("  (4b) BUILD-ORDER HAZARD: a hopper holding 3 shears set first, a comparator set beside it after: the comparator reads {stale} (not 9) until a neighbour update");
    ensure!(stale == 0, "(4b) the comparator set beside an already filled hopper must read 0 (the build-order hazard), read {stale}");
    Ok(())
}

pub fn hopper_pin(version: &str) -> Result<()> {
    println!("== hopper-pin on the official {version} server (ironspec 19:03 (2), G1 19:10): stepped ticks, each row's edge made by a command at step 0; every known answer asserted");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        tick_rate: 1000,
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 47 63")?;
    srv.cmd_ok(&format!("fill -16 {} -16 47 {} 63 minecraft:stone", Y - 1, Y - 1))?;
    srv.step(20)?;
    let r1 = dropper_row(&mut srv, "1", [0, Y, 0], 3, "minecraft:hopper[facing=east]", 0)?;
    assert_row1(&r1)?;
    let r2 = dropper_row(&mut srv, "2", [0, Y, 10], 3, "minecraft:hopper[facing=down]", 1)?;
    assert_row2(&r2)?;
    full_hopper_row(&mut srv, [0, Y, 20])?;
    comparator_rows(&mut srv)?;
    let r5 = dropper_row(&mut srv, "5 (dry)", [0, Y, 40], 0, "minecraft:hopper[facing=east]", 0)?;
    ensure!(r5.iter().all(|s| s.dropper == 0 && s.hopper == 0 && s.chest == 0 && s.comp == 0), "(5) a dry dropper must move nothing");
    println!("  every known answer held: (1) fire at 4, cooldown 7 after the entry, push at 11, comparator 3 at 6 / 0 at 13; (2) no cooldown into a holding hopper; (3) eject + suck at 1 under one cooldown 8, then 9, 17; (4) 0/3/6/9/12/15; (4b) the build-order hazard reads 0; (5) a dry dropper moves nothing");
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
