//! (tickexact 2026-11-13, G1 21:25 (2) / ironspec 21:28 (2): (g)'s w = 2 boundary) `mctas mobai abut-latch`: on the
//! official 26.3 server, the abutting handover in a fresh world: a source (a redstone block set at step 0 and removed at
//! step RLS, here 2: the RLS pulse) feeds (a) a dust line straight into B2 (a stone block) and (b) a dust branch into a
//! delay-1 repeater facing B2, which powers B2 from RLS + 2 (its own 2-gt pulse after the source's 2 gt); Q-bar is a
//! redstone torch on top of B2 (it reads B2: `hasSignal(below, DOWN)`). Row "branch on" and the control "branch off"
//! (the repeater removed: B2 powered by the direct dust alone, [0, RLS)). Every step: the direct dust's power at B2's
//! face, the repeater's powered state, B2 powered (either), the torch's lit state. The torch's first read is its
//! scheduled tick 2 gt after the rising update (step 2 = RLS), where the dust is already off (the source went at the
//! step's start) and the repeater turns on in that tick's block ticks (HIGH) before the torch's NORMAL read.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 101;

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
    Ok(())
}

fn dust_power(srv: &mut VanillaServer, p: [i32; 3]) -> Result<i32> {
    for n in 0..=15 {
        if srv.test_block(p, &format!("minecraft:redstone_wire[power={n}]"))? {
            return Ok(n);
        }
    }
    Ok(-1)
}

/// One row at z offset `z0`; `branch` lays the repeater. Returns the steps the torch read unlit.
fn row(srv: &mut VanillaServer, z0: i32, branch: bool, rls: u32) -> Result<Vec<u32>> {
    let src = [0, Y, z0];
    let b2 = [5, Y, z0];
    let torch = [5, Y + 1, z0];
    let rep = [5, Y, z0 + 1];
    for x in 0..=6 {
        for dz in 0..=3 {
            set(srv, [x, Y - 1, z0 + dz], "minecraft:stone")?;
        }
    }
    set(srv, b2, "minecraft:stone")?;
    for x in 1..=4 {
        set(srv, [x, Y, z0], "minecraft:redstone_wire")?;
    }
    // the branch: dust from beside the source round to the repeater's back (5, Y, z0 + 2)
    set(srv, [0, Y, z0 + 1], "minecraft:redstone_wire")?;
    for x in 0..=5 {
        set(srv, [x, Y, z0 + 2], "minecraft:redstone_wire")?;
    }
    if branch {
        // (DiodeBlock: the input is read from pos.relative(FACING), the output goes to FACING's opposite) facing south:
        // its input the branch dust at (5, Y, z0 + 2), its output north into B2
        set(srv, rep, "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]")?;
    } else {
        set(srv, rep, "minecraft:glass")?;
    }
    set(srv, torch, "minecraft:redstone_torch[lit=true]")?;
    srv.step(10)?;
    println!("  row {}: the source {:?} for {rls} gt, B2 {:?}, Q-bar the torch {:?}", if branch { "branch on (a delay-1 repeater into B2 from the south)" } else { "branch off (the control: the repeater's cell glass)" }, src, b2, torch);
    set(srv, src, "minecraft:redstone_block")?;
    let mut unlit = Vec::new();
    for step in 0..=10u32 {
        if step > 0 {
            if step == rls {
                set(srv, src, "minecraft:air")?;
            }
            srv.step(1)?;
        }
        let d = dust_power(srv, [4, Y, z0])?;
        let r = branch && srv.test_block(rep, "minecraft:repeater[powered=true]")?;
        let lit = srv.test_block(torch, "minecraft:redstone_torch[lit=true]")?;
        if !lit {
            unlit.push(step);
        }
        println!("    step {step:>2}: direct dust at B2's face power {d:>2} | repeater powered {r} | B2 powered {} | Q-bar torch lit {lit}", d > 0 || r);
    }
    println!("    => the torch unlit on steps {unlit:?}");
    Ok(unlit)
}

pub fn abut_latch(version: &str) -> Result<()> {
    println!("== abut-latch on the official {version} server (G1 21:25 (2), ironspec 21:28 (2)): RLS = 2, the direct dust over [0, 2) and a delay-1 branch from 2, Q-bar's torch reading B2");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 15 15")?;
    srv.step(20)?;
    let on = row(&mut srv, 0, true, 2)?;
    let off = row(&mut srv, 8, false, 2)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!("  summary: branch on: the torch unlit on {on:?}; branch off: {off:?}");
    // the known answers (G1's AE pattern; MCTAS_ABUT_LATCH_PLANT=k inverts assert k): with the branch, Q-bar reads B2
    // powered at its first scheduled read (step 2, the repeater's HIGH turn-on draining before the torch's NORMAL read
    // though the direct dust went at the step's start) and stays off until 2 after B2 drops (steps 2..5); without the
    // branch it reads B2 unpowered at step 2 and never turns off
    let plant = |k: u32| std::env::var("MCTAS_ABUT_LATCH_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!((on == vec![2, 3, 4, 5]) != plant(1), "[assert 1{}] branch on: Q-bar's torch unlit exactly on steps 2..5: {on:?}", if plant(1) { ", PLANTED: its expectation inverted" } else { "" });
    anyhow::ensure!(off.is_empty() != plant(2), "[assert 2{}] branch off (the control): Q-bar's torch never unlit: {off:?}", if plant(2) { ", PLANTED: its expectation inverted" } else { "" });
    println!("  every known answer held (asserts 1..2)");
    Ok(())
}
