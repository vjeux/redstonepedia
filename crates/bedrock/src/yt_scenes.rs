//! The shoot of `mctas bedrock yt` (see [`crate::yt`]): every scene, in recording order, with
//! the machine built block by block by the real client.

use crate::portal::Pos;
use crate::yt::{self, A, D1, D2, D3, HEAD, L, N1, N2, N3, O, O2, PLAYER, PRESS, Prod, RB, SP, TD, X, add, face_point};
use anyhow::{Context, Result, bail};

const SLOT_PICK: u32 = 0;
const SLOT_EMPTY: u32 = 1;
const SLOT_TRAPDOOR: u32 = 2;
const SLOT_NETHERRACK: u32 = 3;
const SLOT_DUST: u32 = 4;
const SLOT_STICKY: u32 = 5;
const SLOT_RBLOCK: u32 = 6;
const SLOT_OBSERVER: u32 = 7;
const SLOT_STONE: u32 = 8;
/// The one key bound to Attack AND Use (set in the key binds screen on camera).
pub const PRESS_KEY: &str = "key.keyboard.r";
const KEY_R_CODE: u32 = 21;

fn r(t: Pos, d: Pos) -> Pos {
    add(t, d)
}

fn fp(p: Pos, face: &str) -> [f64; 3] {
    face_point(p, face, [0.0, 0.0, 0.0])
}

fn kit_machine(p: &mut Prod, pistons_in_hand: bool) -> Result<()> {
    let give = [
        (SLOT_TRAPDOOR, "minecraft:oak_trapdoor 1"),
        (SLOT_NETHERRACK, "minecraft:netherrack 8"),
        (SLOT_DUST, "minecraft:redstone 8"),
        (SLOT_STICKY, "minecraft:sticky_piston 1"),
        (SLOT_RBLOCK, "minecraft:redstone_block 1"),
        (SLOT_OBSERVER, "minecraft:observer 2"),
        (SLOT_STONE, "minecraft:stone 1"),
    ];
    for (slot, item) in give {
        p.cmd(&format!("item replace entity {PLAYER} hotbar.{slot} with {item}"))?;
    }
    if pistons_in_hand {
        p.cmd(&format!("item replace entity {PLAYER} hotbar.{SLOT_EMPTY} with minecraft:piston 64"))?;
    }
    p.cmd(&format!("item replace entity {PLAYER} inventory.0 with minecraft:redstone_lamp 1"))?;
    Ok(())
}

fn pickaxe(p: &mut Prod) -> Result<()> {
    p.cmd(&format!("item replace entity {PLAYER} hotbar.{SLOT_PICK} with minecraft:netherite_pickaxe[minecraft:enchantments={{\"minecraft:efficiency\":5}}]"))?;
    Ok(())
}

/// Put the player on the roof at T-relative cell `d` of machine `t`, facing `yaw`.
fn stand(p: &mut Prod, t: Pos, d: Pos, yaw: f32, pitch: f32) -> Result<()> {
    let c = r(t, d);
    p.teleport(c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5, yaw, pitch)
}

/// The 11-block build around target `t`: `show` films one segment per block (the tutorial),
/// otherwise one segment for the whole build (`seg_name`). `a_from_offhand`: the swap piston comes
/// from the offhand stack (the pickaxe selected).
pub fn build_machine(p: &mut Prod, t: Pos, show: bool, a_from_offhand: bool, seg_name: &str) -> Result<()> {
    let s = |p: &mut Prod, name: &str| -> Result<()> {
        if show {
            p.segment(name, "build")?;
        }
        Ok(())
    };
    if !show {
        stand(p, t, [-3, 1, 0], -90.0, 25.0)?;
        p.ticks(6)?;
        p.segment(seg_name, "build2")?;
    }
    // 1. The swap piston on T, facing west (look east at T's top, from 3 blocks west).
    if show {
        stand(p, t, [-3, 1, 0], -90.0, 25.0)?;
    }
    s(p, "b01-piston")?;
    p.ticks(10)?;
    let a_slot = if a_from_offhand { SLOT_PICK } else { SLOT_EMPTY };
    p.put(a_slot, t, "up", face_point(t, "up", [-0.2, 0.0, 0.0]), false, r(t, A), "minecraft:piston[facing=west]", "1 swap piston")?;
    p.ticks(14)?;
    // 2. The trapdoor over the press spot: on a temporary netherrack, then the netherrack mined.
    p.go_route(t, &[(-3, -3), (0, -3)])?;
    s(p, "b02-trapdoor")?;
    p.ticks(6)?;
    p.put(SLOT_NETHERRACK, r(t, [0, 0, -1]), "up", fp(r(t, [0, 0, -1]), "up"), false, r(t, N1), "minecraft:netherrack", "temporary netherrack")?;
    p.ticks(6)?;
    p.put(SLOT_TRAPDOOR, r(t, N1), "up", fp(r(t, N1), "up"), false, r(t, TD), "minecraft:oak_trapdoor[half=bottom,open=false,facing=north]", "2 trapdoor")?;
    p.ticks(6)?;
    p.dig(r(t, N1), "north", fp(r(t, N1), "north"), "temporary netherrack")?;
    p.ticks(12)?;
    // 3-5. Three redstone dust on the roof, from the east.
    p.go_route(t, &[(4, -3), (4, 1)])?;
    s(p, "b03-dust")?;
    p.ticks(6)?;
    for (d, what) in [(D1, "3 dust"), (D2, "4 dust"), (D3, "5 dust")] {
        let below = add(r(t, d), [0, -1, 0]);
        p.put(SLOT_DUST, below, "up", fp(below, "up"), false, r(t, d), "minecraft:redstone_wire", what)?;
        p.ticks(6)?;
    }
    p.ticks(6)?;
    // 6. The sticky piston facing south: on a temporary netherrack, looking north.
    let below_n2 = add(r(t, N2), [0, -1, 0]);
    p.put(SLOT_NETHERRACK, below_n2, "up", fp(below_n2, "up"), false, r(t, N2), "minecraft:netherrack", "temporary netherrack")?;
    p.go_route(t, &[(4, 3), (2, 3)])?;
    s(p, "b06-sticky")?;
    p.ticks(6)?;
    p.put(SLOT_STICKY, r(t, N2), "up", face_point(r(t, N2), "up", [0.0, 0.0, 0.2]), false, r(t, SP), "minecraft:sticky_piston[facing=south]", "6 sticky piston")?;
    p.ticks(6)?;
    p.dig(r(t, N2), "south", fp(r(t, N2), "south"), "temporary netherrack")?;
    p.ticks(8)?;
    // 7. The redstone block on the sticky piston's face.
    s(p, "b07-rblock")?;
    p.ticks(6)?;
    p.put(SLOT_RBLOCK, r(t, SP), "south", fp(r(t, SP), "south"), false, r(t, RB), "minecraft:redstone_block", "7 redstone block")?;
    p.ticks(14)?;
    // 8. The observer facing east, on the first dust (sneaking: no click on the dust itself).
    p.go_route(t, &[(-2, 3), (-2, 1)])?;
    s(p, "b08-observer")?;
    p.ticks(6)?;
    let d1 = r(t, D1);
    let d1_top = [d1[0] as f64 + 0.5, d1[1] as f64 + 0.0625, d1[2] as f64 + 0.5];
    p.put(SLOT_OBSERVER, d1, "up", d1_top, true, r(t, O), "minecraft:observer[facing=east]", "8 observer")?;
    p.ticks(10)?;
    // 9. The stone on the observer's back.
    s(p, "b09-stone")?;
    p.ticks(6)?;
    p.put(SLOT_STONE, r(t, O), "west", fp(r(t, O), "west"), false, r(t, X), "minecraft:stone", "9 stone")?;
    p.ticks(14)?;
    // 10. The top observer facing south, on the redstone block: from a one-block step (temporary).
    p.go_route(t, &[(-2, -2), (0, -2)])?;
    s(p, "b10-observer2")?;
    p.ticks(6)?;
    let below_n3 = add(r(t, N3), [0, -1, 0]);
    p.put(SLOT_NETHERRACK, below_n3, "up", fp(below_n3, "up"), false, r(t, N3), "minecraft:netherrack", "temporary step")?;
    p.ticks(4)?;
    let n3 = r(t, N3);
    p.go_jump(n3[0], n3[2])?;
    let feet = p.pos()?;
    if feet[1] < (n3[1] + 1) as f64 - 0.01 {
        bail!("not on the step: feet {feet:?}");
    }
    p.ticks(6)?;
    p.put(SLOT_OBSERVER, r(t, RB), "up", fp(r(t, RB), "up"), false, r(t, O2), "minecraft:observer[facing=south]", "10 top observer")?;
    p.ticks(8)?;
    p.go(n3[0], n3[2] - 2)?;
    p.ticks(4)?;
    p.dig(n3, "north", fp(n3, "north"), "temporary step")?;
    p.ticks(10)?;
    // 11. The lamp, LAST: the top observer sees it and the clock starts.
    p.go_route(t, &[(4, -4), (4, 4), (2, 4)])?;
    s(p, "b11-lamp")?;
    p.act(&format!("hotbar {SLOT_TRAPDOOR} redstone_lamp"))?;
    p.ticks(6)?;
    p.put(SLOT_TRAPDOOR, r(t, O2), "south", fp(r(t, O2), "south"), false, r(t, L), "minecraft:redstone_lamp", "11 lamp")?;
    // The clock runs: the redstone block leaves its home cell within a few ticks.
    let mut moved = false;
    for _ in 0..40 {
        if !p.is(r(t, RB), "minecraft:redstone_block")? {
            moved = true;
            break;
        }
        p.ticks(1)?;
    }
    if !moved {
        bail!("the clock did not start");
    }
    p.event("clock running");
    p.ticks(40)?;
    Ok(())
}

/// Into the press spot: open the trapdoor, step in, close it (crawling), crawl against the swap
/// piston, look up at its side.
fn lie_down(p: &mut Prod, t: Pos) -> Result<()> {
    p.slot(SLOT_PICK)?;
    let td = r(t, TD);
    // From the cell north of the press spot.
    let north = r(t, [0, 1, -2]);
    p.go(north[0], north[2])?;
    p.ticks(4)?;
    // A closed bottom trapdoor is 3/16 tall: aim at its real top.
    p.look([td[0] as f64 + 0.5, td[1] as f64 + 0.1875, td[2] as f64 + 0.5], 10)?;
    p.ticks(4)?;
    p.hit_on(td, "up")?;
    p.act("click use")?;
    p.ticks(4)?;
    p.must(td, "minecraft:oak_trapdoor[open=true]", "trapdoor opened")?;
    p.event("trapdoor open");
    let ps = r(t, PRESS);
    p.go(ps[0], ps[2])?;
    p.ticks(4)?;
    // The open trapdoor is a panel on the south side of its cell: look at it and close it.
    p.look([td[0] as f64 + 0.5, td[1] as f64 + 0.55, td[2] as f64 + 0.8125], 10)?;
    p.ticks(4)?;
    p.hit_on(td, "north")?;
    p.act("click use")?;
    p.ticks(6)?;
    p.must(td, "minecraft:oak_trapdoor[open=false]", "trapdoor closed")?;
    let pose = p.act("pose")?;
    if !pose.contains("pose=SWIMMING") {
        bail!("not crawling after closing the trapdoor: {pose}");
    }
    p.event("crawling");
    // Crawl south against the swap piston.
    p.turn(0.0, 10.0, 8)?;
    p.act("hold forward 8")?;
    p.ticks(4)?;
    let feet = p.pos()?;
    p.loga(&format!("lying at {feet:?}"));
    p.turn(0.0, -55.0, 12)?;
    p.ticks(4)?;
    hit_soon(p, r(t, A), "north")?;
    Ok(())
}

/// The crosshair check, tolerant of the swap piston's own animation (a moving piston has no
/// outline: the ray passes through it for a tick or two).
fn hit_soon(p: &mut Prod, block: Pos, face: &str) -> Result<String> {
    let mut last = None;
    for _ in 0..20 {
        match p.hit_on(block, face) {
            Ok(h) => return Ok(h),
            Err(e) => last = Some(e),
        }
        p.ticks(1)?;
    }
    Err(last.unwrap())
}

/// Is the clock running? Samples the redstone block's home cell for ~1.5 s (it changes every
/// 6 ticks while the clock runs) and logs the pattern.
fn machine_alive(p: &mut Prod, t: Pos, label: &str) -> Result<bool> {
    let mut pat = String::new();
    let mut changes = 0;
    let mut last = None;
    for _ in 0..24 {
        let home = p.is(r(t, RB), "minecraft:redstone_block")?;
        let head = p.is(r(t, HEAD), "minecraft:piston_head")?;
        pat.push(match (home, head) {
            (true, true) => 'R',
            (true, false) => 'r',
            (false, true) => 'H',
            (false, false) => '.',
        });
        if last.is_some_and(|l| l != home) {
            changes += 1;
        }
        last = Some(home);
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    p.loga(&format!("machine {label}: {pat} ({changes} changes)"));
    Ok(changes > 0)
}

/// The client-side rises of the swap piston (`extended=true` seen) from the track log, in wall
/// milliseconds.
fn rises(p: &Prod, file: &str) -> Result<Vec<f64>> {
    let log = std::fs::read_to_string(p.out_dir().join("client").join("mctas-actor").join(file)).unwrap_or_default();
    // The first sample logs the starting states: not a rise.
    let first = log.lines().next().and_then(|l| l.split(' ').next()).unwrap_or("").to_string();
    let mut v = Vec::new();
    for l in log.lines() {
        let f: Vec<&str> = l.splitn(4, ' ').collect();
        if f.len() == 4 && f[0] != first && f[2] == "0" && f[3].contains("extended=true") {
            if let Ok(ms) = f[0].parse::<f64>() {
                v.push(ms);
            }
        }
    }
    Ok(v)
}

/// The machine's beat as the player sees it: the earliest phase (mod 300 ms) of the arm's rises
/// over the last few seconds (a frame only ever delays what the client sees).
fn beat(rs: &[f64]) -> Option<f64> {
    let last = *rs.last()?;
    let recent: Vec<f64> = rs.iter().copied().filter(|r| last - r < 3000.0).collect();
    if recent.len() < 4 {
        return None;
    }
    let base = recent[0];
    let offs: Vec<f64> = recent.iter().map(|r| (r - base).rem_euclid(300.0)).collect();
    // The densest 45 ms window of the offsets (an odd rise, e.g. a repaired piston's, is left
    // out), and its earliest member.
    let mut best = (0usize, 0.0f64);
    for &o in &offs {
        let n = offs.iter().filter(|&&q| (q - o).rem_euclid(300.0) < 45.0).count();
        if n > best.0 {
            best = (n, o);
        }
    }
    Some(base + best.1)
}

/// One press in rhythm: watch the arm (the track log), press `offset` ms after a rise, the way
/// a player times it from the arm popping out. Returns true when the bedrock is gone.
fn press_rhythm(p: &mut Prod, t: Pos, label: &str, log: &str, offset: f64) -> Result<bool> {
    hit_soon(p, r(t, A), "north")?;
    let mut b = None;
    for _ in 0..20 {
        b = beat(&rises(p, log)?);
        if b.is_some() {
            break;
        }
        p.ticks(10)?;
    }
    let b = b.context("no beat seen")?;
    let now = yt::now() * 1000.0;
    let k = ((now + 250.0 - b - offset) / 300.0).ceil();
    let at = b + k * 300.0 + offset;
    let res = p.act(&format!("pressat {} {PRESS_KEY}", at as i64))?;
    p.event(&format!("press {label}: rise at {:.0} {res}", b + k * 300.0));
    p.ticks(10)?;
    Ok(!p.is(t, "minecraft:bedrock")?)
}

/// After a miss (the swap piston faces down): the one-press repair from the same spot.
fn repair(p: &mut Prod, t: Pos) -> Result<()> {
    p.turn(-52.0, 10.0, 10)?;
    p.ticks(4)?;
    let h = hit_soon(p, r(t, A), "north")?;
    p.loga(&format!("repair aim: {h}"));
    p.act(&format!("tap {PRESS_KEY}"))?;
    p.ticks(6)?;
    p.event("repair");
    p.must(r(t, A), "minecraft:piston[facing=west]", "repaired: the swap piston faces west again")?;
    p.turn(0.0, -55.0, 10)?;
    p.ticks(2)?;
    Ok(())
}

/// Pistons out of the offhand (empty slot + F), pickaxe in hand, then break the swap piston and
/// look into the hole.
fn reveal(p: &mut Prod, t: Pos) -> Result<()> {
    p.slot(SLOT_EMPTY)?;
    p.act("tap key.keyboard.f")?;
    p.ticks(4)?;
    p.slot(SLOT_PICK)?;
    p.ticks(4)?;
    p.turn(0.0, -55.0, 6)?;
    hit_soon(p, r(t, A), "north")?;
    // The machine keeps pumping the swapped piston: tap until it is gone (a moving piston
    // can't be broken). Uneven spacing: taps every 6 ticks would all land in the same phase of
    // the machine's 6-tick cycle.
    for k in 0..8u32 {
        p.act(&format!("tap {PRESS_KEY}"))?;
        p.ticks(4 + k % 3)?;
        if p.is(r(t, A), "minecraft:air")? {
            break;
        }
    }
    p.must(r(t, A), "minecraft:air", "the swap piston broken")?;
    p.event("piston broken");
    // Look down into the hole where T was.
    p.look([t[0] as f64 + 0.5, t[1] as f64 + 0.05, t[2] as f64 + 0.6], 16)?;
    p.ticks(40)?;
    p.event("hole visible");
    Ok(())
}

pub fn scenes(p: &mut Prod) -> Result<()> {
    let plan = p.plan.clone();
    let t1 = plan.t1;
    let t2 = plan.t2;
    let b = plan.beacon;
    // Every client session (a resume starts a new one): no toasts, the HUD, first person, and the
    // key binds of the press once they were set on camera (options.txt is rewritten at launch).
    p.act("toasts off")?;
    p.act("hud 1")?;
    p.act("cam first")?;
    if p.is_done("setup-press") {
        for c in ["gui options", "gui press Controls", "gui press Key Binds", "gui select key.attack"] {
            p.act(c)?;
            p.ticks(4)?;
        }
        p.act(&format!("gui key {KEY_R_CODE}"))?;
        p.act("gui select key.use")?;
        p.act(&format!("gui key {KEY_R_CODE}"))?;
        p.act("gui close")?;
        let binds = p.act("gui binds")?;
        p.loga(&format!("binds restored: {binds}"));
    }
    if !p.is_done("setup") {
        for c in [
            "gamerule sendCommandFeedback false",
            "gamerule logAdminCommands false",
            "gamerule announceAdvancements false",
            "gamerule doMobSpawning false",
        ] {
            let _ = p.cmd_raw(c);
        }
        p.cmd(&format!("gamemode survival {PLAYER}"))?;
        p.cmd(&format!("clear {PLAYER}"))?;
        p.in_nether(&format!("forceload add {} {} {} {}", t1[0] - 24, t1[2] - 12, t2[0] + 12, t2[2] + 12))?;
        // The beacon's pyramid (164 iron blocks; placed by command, said so on screen).
        for (k, y) in (b[1] - 4..b[1]).enumerate() {
            let rr = 4 - k as i32;
            p.in_nether(&format!("fill {} {y} {} {} {y} {} minecraft:iron_block", b[0] - rr, b[2] - rr, b[0] + rr, b[2] + rr))?;
        }
        pickaxe(p)?;
        p.cmd(&format!("item replace entity {PLAYER} hotbar.1 with minecraft:beacon 1"))?;
        p.cmd(&format!("item replace entity {PLAYER} hotbar.2 with minecraft:iron_ingot 1"))?;
        p.act("toasts off")?;
        p.act("hud 1")?;
        p.act("cam first")?;
        stand(p, t1, [-4, 1, 0], -90.0, 10.0)?;
        p.ticks(60)?;
        p.mark("setup")?;
    }
    if !p.is_done("intro") {
        // The roof: a slow look across it.
        p.act("hud 0")?;
        stand(p, t1, [-4, 1, -6], 160.0, 8.0)?;
        p.ticks(40)?;
        p.segment("intro-roof", "intro")?;
        p.turn(60.0, 12.0, 120)?;
        p.ticks(10)?;
        p.act("hud 1")?;
        p.mark("intro")?;
    }
    if !p.is_done("beacon") {
        // Walk to the pyramid and climb it from the east.
        stand(p, t1, [-4, 1, 1], 90.0, 5.0)?;
        p.ticks(20)?;
        p.segment("beacon-walk", "beacon")?;
        p.go(b[0] + 6, b[2])?;
        p.go_jump(b[0] + 1, b[2])?;
        let feet = p.pos()?;
        if feet[1] < b[1] as f64 - 0.01 {
            bail!("not on top of the pyramid: {feet:?}");
        }
        p.segment("beacon-place", "beacon")?;
        let center = [b[0], b[1] - 1, b[2]];
        p.put(1, center, "up", fp(center, "up"), false, b, "minecraft:beacon", "beacon")?;
        // The pyramid is counted every 4 seconds.
        p.ticks(90)?;
        p.segment("beacon-gui", "beacon")?;
        p.slot(2)?;
        p.look([b[0] as f64 + 0.5, b[1] as f64 + 1.0, b[2] as f64 + 0.5], 8)?;
        p.ticks(4)?;
        p.hit_on(b, "up")?;
        p.act("click use")?;
        p.ticks(20)?;
        let name = p.act("gui name")?;
        if !name.contains("BeaconScreen") {
            bail!("the beacon screen did not open: {name}");
        }
        let res = p.act("beacon iron_ingot")?;
        p.event(&format!("beacon {res}"));
        // The beacon hands out its effects every 4 seconds.
        let mut fx = String::new();
        for _ in 0..12 {
            p.ticks(10)?;
            fx = p.cmd_raw(&format!("data get entity {PLAYER} active_effects"))?;
            if fx.contains("minecraft:haste") {
                break;
            }
        }
        p.loga(&format!("effects: {fx}"));
        if !fx.contains("minecraft:haste") || !fx.contains("amplifier: 1b") {
            bail!("no Haste II after the beacon: {fx}");
        }
        p.event("haste II on");
        p.ticks(10)?;
        p.segment("beacon-beam", "beacon")?;
        p.act("hud 0")?;
        p.turn(-120.0, -62.0, 30)?;
        p.ticks(50)?;
        p.act("hud 1")?;
        p.cut()?;
        p.mark("beacon")?;
    }
    if !p.is_done("build") {
        p.cmd(&format!("clear {PLAYER}"))?;
        pickaxe(p)?;
        kit_machine(p, true)?;
        stand(p, t1, [-3, 1, 0], -90.0, 15.0)?;
        p.ticks(10)?;
        p.segment("kit-hotbar", "kit")?;
        for s in [1, 2, 3, 4, 5, 6, 7, 8, 0] {
            p.slot(s)?;
            p.ticks(9)?;
        }
        p.ticks(10)?;
        build_machine(p, t1, true, false, "")?;
        p.cut()?;
        machine_alive(p, t1, "after the build")?;
        p.mark("build")?;
        machine_alive(p, t1, "after the build checkpoint")?;
    }
    if !p.is_done("overview") {
        p.act("hud 0")?;
        for (i, (d, look)) in [([-3, 1, -4], [1.0, 1.6, 0.8]), ([5, 1, -3], [1.0, 1.7, 0.9]), ([4, 1, 5], [1.0, 1.7, 0.6]), ([-3, 1, 3], [1.0, 1.5, 0.6])].iter().enumerate() {
            stand(p, t1, *d, 0.0, 0.0)?;
            let lp = [t1[0] as f64 + look[0], t1[1] as f64 + look[1], t1[2] as f64 + look[2]];
            p.look(lp, 2)?;
            p.ticks(10)?;
            p.segment(&format!("overview-{}", i + 1), "overview")?;
            let pose = p.act("pose")?;
            p.event(&format!("pose {pose}"));
            p.ticks(70)?;
        }
        p.act("hud 1")?;
        p.cut()?;
        machine_alive(p, t1, "after the overview")?;
        p.mark("overview")?;
    }
    if !p.is_done("setup-press") {
        // The key binds screen: Attack and Use on the same key; then pistons to the offhand.
        stand(p, t1, [0, 1, -3], 0.0, 20.0)?;
        p.ticks(10)?;
        p.segment("keybinds", "keys")?;
        p.ticks(10)?;
        p.act("gui options")?;
        p.ticks(24)?;
        p.act("gui press Controls")?;
        p.ticks(24)?;
        p.act("gui press Key Binds")?;
        p.ticks(24)?;
        p.act("gui scrollto key.attack")?;
        p.ticks(30)?;
        p.act("gui select key.attack")?;
        p.event("select attack");
        p.ticks(20)?;
        p.act(&format!("gui key {KEY_R_CODE}"))?;
        p.event("attack = R");
        p.ticks(24)?;
        p.act("gui select key.use")?;
        p.event("select use");
        p.ticks(20)?;
        p.act(&format!("gui key {KEY_R_CODE}"))?;
        p.event("use = R");
        p.ticks(70)?;
        p.act("gui close")?;
        p.ticks(4)?;
        let binds = p.act("gui binds")?;
        p.loga(&format!("binds: {binds}"));
        if !binds.contains("key.attack=key.keyboard.r") || !binds.contains("key.use=key.keyboard.r") {
            bail!("the key binds did not take: {binds}");
        }
        // The pistons on the hotbar (slot 1), selected, then F.
        let hb = p.act(&format!("hotbar {SLOT_EMPTY} piston"))?;
        let slot: u32 = hb.split("slot=").nth(1).and_then(|v| v.split(' ').next()).and_then(|v| v.parse().ok()).context("hotbar reply")?;
        p.ticks(4)?;
        p.segment("offhand", "keys")?;
        p.ticks(6)?;
        p.slot(slot)?;
        p.ticks(14)?;
        p.act("tap key.keyboard.f")?;
        p.event("F: pistons to the offhand");
        p.ticks(14)?;
        p.slot(SLOT_PICK)?;
        p.ticks(24)?;
        let off = p.cmd_raw(&format!("execute if items entity {PLAYER} weapon.offhand minecraft:piston"))?;
        if !off.contains("passed") {
            bail!("no pistons in the offhand: {off}");
        }
        p.cut()?;
        machine_alive(p, t1, "after the key binds")?;
        p.mark("setup-press")?;
    }
    if !p.is_done("press") {
        machine_alive(p, t1, "before lying down")?;
        p.segment("liedown", "press")?;
        lie_down(p, t1)?;
        machine_alive(p, t1, "lying down")?;
        p.ticks(10)?;
        p.act("cam third")?;
        p.turn(0.0, 0.0, 8)?;
        p.ticks(40)?;
        p.act("cam first")?;
        p.turn(0.0, -55.0, 10)?;
        p.ticks(6)?;
        let mut won = false;
        let a1 = r(t1, A);
        let h1 = r(t1, HEAD);
        p.act(&format!("track start press.log {},{},{} {},{},{}", a1[0], a1[1], a1[2], h1[0], h1[1], h1[2]))?;
        p.ticks(40)?;
        // Try 1 is pressed OFF the beat on purpose, right as the arm starts out (the miss chapter
        // shows what a miss does and the one-press repair); then in rhythm: ~60 ms after the arm
        // pops out (the press timing lab: the first press at +60 ms removed the bedrock).
        let offsets = [-30.0, 60.0, 60.0, 70.0, 50.0, 80.0, 60.0, 90.0, 40.0, 100.0, 60.0, 60.0];
        for (k, off) in offsets.iter().enumerate() {
            let k = k + 1;
            p.segment(&format!("try-{k:02}"), "press")?;
            p.ticks(8)?;
            if press_rhythm(p, t1, &format!("try {k} offset {off}"), "press.log", *off)? {
                p.event("HIT: the bedrock is gone");
                won = true;
                p.ticks(20)?;
                break;
            }
            if p.is(r(t1, A), "minecraft:piston[facing=down]")? {
                p.event("MISS: the swap piston faces down");
                p.ticks(12)?;
                repair(p, t1)?;
                p.ticks(20)?;
            } else {
                p.event("no change");
            }
        }
        if !won {
            bail!("no hit in 12 presses");
        }
        p.act("track stop")?;
        p.segment("reveal", "press")?;
        reveal(p, t1)?;
        // A clean look at the hole from the roof (the thumbnail's frame).
        p.act("hold back 24")?;
        p.go_route(t1, &[(-2, -2), (-2, 0)])?;
        p.act("hud 0")?;
        p.look([t1[0] as f64 + 0.5, t1[1] as f64 + 0.9, t1[2] as f64 + 0.5], 10)?;
        p.ticks(6)?;
        p.segment("hole-shot", "press")?;
        p.ticks(50)?;
        p.look([t1[0] as f64 + 0.9, t1[1] as f64 + 1.2, t1[2] as f64 + 0.6], 40)?;
        p.ticks(10)?;
        p.act("hud 1")?;
        p.cut()?;
        p.mark("press")?;
    }
    if !p.is_done("m2") {
        // The second machine (same build, shown sped up) for the slow motion. Its kit by command.
        kit_machine(p, false)?;
        // The pistons go back to the offhand: the swap piston comes from there.
        p.slot(SLOT_EMPTY)?;
        p.act("tap key.keyboard.f")?;
        p.slot(SLOT_PICK)?;
        build_machine(p, t2, false, true, "m2-build")?;
        p.cut()?;
        p.mark("m2")?;
    }
    if !p.is_done("slowmo") {
        // From outside: one cycle frozen and stepped tick by tick.
        p.act("hud 0")?;
        stand(p, t2, [-3, 1, -3], 0.0, 0.0)?;
        p.look([t2[0] as f64 + 0.2, t2[1] as f64 + 1.7, t2[2] as f64 + 0.8], 2)?;
        p.ticks(20)?;
        p.segment("slowmo-cycle", "slowmo")?;
        let pose = p.act("pose")?;
        p.event(&format!("pose {pose}"));
        p.ticks(16)?;
        p.cmd("tick freeze")?;
        p.event("freeze");
        // Start the count where the arm has just retracted (head cell empty, piston not extended).
        for _ in 0..12 {
            if p.is(r(t2, HEAD), "minecraft:air")? && p.is(r(t2, A), "minecraft:piston[extended=false]")? {
                break;
            }
            p.cmd("tick step 1")?;
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
        for k in 0..13 {
            std::thread::sleep(std::time::Duration::from_millis(900));
            p.cmd("tick step 1")?;
            std::thread::sleep(std::time::Duration::from_millis(150));
            let ext = p.is(r(t2, A), "minecraft:piston[extended=true]")?;
            let head = p.is(r(t2, HEAD), "minecraft:piston_head")?;
            let moving = p.is(r(t2, HEAD), "minecraft:moving_piston")?;
            let rb_home = p.is(r(t2, RB), "minecraft:redstone_block")?;
            p.event(&format!("step {k} ext={ext} head={head} moving={moving} rb_home={rb_home}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(900));
        p.cmd("tick unfreeze")?;
        p.event("unfreeze");
        p.ticks(10)?;
        p.act("hud 1")?;
        p.cut()?;
        p.mark("slowmo")?;
    }
    if !p.is_done("slowpress") {
        // The press itself, frozen at the armed tick.
        p.segment("m2-liedown", "slowmo")?;
        lie_down(p, t2)?;
        p.ticks(10)?;
        p.segment("slowmo-press", "slowmo")?;
        p.ticks(10)?;
        p.cmd("tick freeze")?;
        p.event("freeze");
        let mut armed = false;
        for _ in 0..16 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            p.cmd("tick step 1")?;
            std::thread::sleep(std::time::Duration::from_millis(150));
            let head = p.is(r(t2, HEAD), "minecraft:piston_head")?;
            p.event(&format!("step head={head}"));
            if head {
                armed = true;
                break;
            }
        }
        if !armed {
            bail!("never armed while stepping");
        }
        p.event("ARMED");
        std::thread::sleep(std::time::Duration::from_millis(1200));
        hit_soon(p, r(t2, A), "north")?;
        p.act(&format!("tap {PRESS_KEY}"))?;
        p.event("press (frozen)");
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let down = p.is(r(t2, A), "minecraft:piston[facing=down]")?;
        p.event(&format!("swapped: facing down = {down}"));
        p.cmd("tick step 1")?;
        std::thread::sleep(std::time::Duration::from_millis(200));
        let gone = !p.is(t2, "minecraft:bedrock")?;
        p.event(&format!("next tick: bedrock gone = {gone}"));
        std::thread::sleep(std::time::Duration::from_millis(1500));
        p.cmd("tick unfreeze")?;
        p.event("unfreeze");
        if !gone {
            bail!("the frozen press did not remove the bedrock");
        }
        p.ticks(10)?;
        p.segment("slowmo-reveal", "slowmo")?;
        reveal(p, t2)?;
        p.cut()?;
        p.mark("slowpress")?;
    }
    if !p.is_done("drop") {
        // The payoff: back to the first hole, a Slow Falling potion, in, dig down, drift down
        // into the Nether.
        p.act("hold back 24")?;
        p.cmd(&format!("item replace entity {PLAYER} hotbar.{SLOT_TRAPDOOR} with minecraft:potion[minecraft:potion_contents={{potion:\"minecraft:slow_falling\"}}]"))?;
        stand(p, t1, [-1, 1, -3], 0.0, 30.0)?;
        p.ticks(10)?;
        p.go(t1[0] - 1, t1[2])?;
        p.turn(-90.0, 35.0, 4)?;
        p.ticks(6)?;
        p.segment("drink", "drop")?;
        p.slot(SLOT_TRAPDOOR)?;
        p.ticks(6)?;
        p.turn(-90.0, -30.0, 8)?;
        p.act("holduse 40")?;
        p.event("drank slow falling");
        p.slot(SLOT_PICK)?;
        p.ticks(6)?;
        p.segment("drop", "drop")?;
        p.act("hud 0")?;
        p.look([t1[0] as f64 + 0.5, t1[1] as f64 + 0.2, t1[2] as f64 + 0.5], 10)?;
        p.ticks(10)?;
        p.go(t1[0], t1[2])?;
        p.ticks(10)?;
        p.turn(0.0, 90.0, 8)?;
        let mut falling = false;
        for _ in 0..80 {
            let feet = p.pos()?;
            if feet[1] < plan.floor as f64 + 1.5 {
                break;
            }
            let below = [t1[0], feet[1].floor() as i32 - 1, t1[2]];
            if p.is(below, "minecraft:air")? {
                if !falling && feet[1] < plan.shaft_bottom as f64 + 0.5 {
                    p.event("falling");
                    falling = true;
                }
                p.ticks(10)?;
                continue;
            }
            p.act("click attack")?;
            p.ticks(5)?;
        }
        p.ticks(30)?;
        let feet = p.pos()?;
        p.event(&format!("landed at {feet:?}"));
        p.turn(60.0, -20.0, 40)?;
        p.turn(0.0, -80.0, 40)?;
        p.ticks(30)?;
        p.cut()?;
        p.mark("drop")?;
    }
    Ok(())
}

/// The actor's verbs on the real roof (no filming): hit, click, keys, screens, cue.
pub fn smoke(p: &mut Prod) -> Result<()> {
    let t = p.plan.t1;
    p.cmd(&format!("gamemode survival {PLAYER}"))?;
    p.in_nether(&format!("forceload add {} {} {} {}", t[0] - 8, t[2] - 8, t[0] + 8, t[2] + 8))?;
    pickaxe(p)?;
    kit_machine(p, true)?;
    p.cmd(&format!("effect give {PLAYER} minecraft:haste infinite 1 true"))?;
    p.act("toasts off")?;
    stand(p, t, [-3, 1, 0], -90.0, 25.0)?;
    p.ticks(20)?;
    let pose = p.act("pose")?;
    p.loga(&format!("pose: {pose}"));
    p.put(SLOT_EMPTY, t, "up", face_point(t, "up", [-0.2, 0.0, 0.0]), false, r(t, A), "minecraft:piston[facing=west]", "smoke piston")?;
    let st = p.act(&format!("state {} {} {}", t[0], t[1] + 1, t[2]))?;
    p.loga(&format!("client sees: {st}"));
    p.dig(r(t, A), "west", fp(r(t, A), "west"), "smoke piston mined")?;
    p.act("gui options")?;
    p.ticks(10)?;
    p.act("gui press Controls")?;
    p.ticks(10)?;
    p.act("gui press Key Binds")?;
    p.ticks(10)?;
    p.act("gui scrollto key.attack")?;
    p.act("gui select key.attack")?;
    p.act(&format!("gui key {KEY_R_CODE}"))?;
    p.act("gui select key.use")?;
    p.act(&format!("gui key {KEY_R_CODE}"))?;
    p.ticks(10)?;
    p.act("gui close")?;
    let binds = p.act("gui binds")?;
    p.loga(&format!("binds: {binds}"));
    let fx = p.cmd_raw(&format!("data get entity {PLAYER} active_effects")).context("effects")?;
    p.loga(&format!("effects: {fx}"));
    println!("smoke ok: {binds}");
    Ok(())
}

/// The press timing lab (no filming, from a checkpoint after the key binds): what the client
/// sees of the machine's rhythm, the frame rate, and the hit rate of presses at each offset from
/// the arm's rise.
pub fn presslab(p: &mut Prod) -> Result<()> {
    let t = p.plan.t1;
    p.act("toasts off")?;
    if !p.is(r(t, L), "minecraft:redstone_lamp")? {
        // From the beacon checkpoint: build the machine first (its clock runs in this session).
        p.cmd(&format!("clear {PLAYER}"))?;
        pickaxe(p)?;
        kit_machine(p, true)?;
        build_machine(p, t, false, false, "lab-build")?;
        p.cut()?;
        let hb = p.act(&format!("hotbar {SLOT_EMPTY} piston"))?;
        let slot: u32 = hb.split("slot=").nth(1).and_then(|v| v.split(' ').next()).and_then(|v| v.parse().ok()).context("hotbar reply")?;
        p.slot(slot)?;
        p.act("tap key.keyboard.f")?;
        p.slot(SLOT_PICK)?;
    }
    for c in ["gui options", "gui press Controls", "gui press Key Binds", "gui select key.attack"] {
        p.act(c)?;
        p.ticks(4)?;
    }
    p.act(&format!("gui key {KEY_R_CODE}"))?;
    p.act("gui select key.use")?;
    p.act(&format!("gui key {KEY_R_CODE}"))?;
    p.act("gui close")?;
    stand(p, t, [0, 1, -3], 0.0, 20.0)?;
    p.ticks(10)?;
    lie_down(p, t)?;
    machine_alive(p, t, "lab")?;
    let pos = p.act("pos")?;
    println!("pos: {pos}");
    let a = r(t, A);
    let h = r(t, HEAD);
    p.act(&format!("track start lab.log {},{},{} {},{},{}", a[0], a[1], a[2], h[0], h[1], h[2]))?;
    std::thread::sleep(std::time::Duration::from_secs(4));
    p.act("track stop")?;
    let log = std::fs::read_to_string(p.out_dir().join("client").join("mctas-actor").join("lab.log"))?;
    let mut rises = Vec::new();
    for l in log.lines() {
        let f: Vec<&str> = l.splitn(4, ' ').collect();
        if f.len() == 4 && f[2] == "0" && f[3].contains("extended=true") {
            rises.push(f[0].parse::<f64>().unwrap_or(0.0));
        }
    }
    println!("track lines {}, rises {}", log.lines().count(), rises.len());
    for l in log.lines().take(60) {
        println!("  {l}");
    }
    for k in 0..3 {
        let res = p.act_long(&format!("presscue {} {} {} {PRESS_KEY} 0 3000", h[0], h[1], h[2]), 10);
        p.ticks(8)?;
        let gone = !p.is(t, "minecraft:bedrock")?;
        let down = p.is(a, "minecraft:piston[facing=down]")?;
        println!("presscue {k}: {res:?} -> gone={gone} down={down}");
        if gone {
            return Ok(());
        }
        if down {
            repair(p, t)?;
            p.ticks(10)?;
        }
    }
    if rises.len() < 3 {
        bail!("too few rises");
    }
    // Period and the earliest phase (frames only delay what the client sees).
    let period = (rises[rises.len() - 1] - rises[0]) / (rises.len() - 1) as f64;
    let base = rises[0];
    let phase = rises.iter().map(|r| (r - base).rem_euclid(300.0)).fold(f64::MAX, |m, v| if v > 150.0 { m.min(v - 300.0) } else { m.min(v) });
    println!("period {period:.1} ms (expect 300), phase {phase:.1}");
    let mut results = Vec::new();
    for round in 0..2 {
        for off in [60.0, 100.0, 140.0, 180.0, 220.0, 260.0] {
            let now = yt::now() * 1000.0;
            let k = ((now - base - phase) / 300.0).ceil() + 2.0;
            let at = base + phase + k * 300.0 + off;
            let res = p.act(&format!("pressat {} {PRESS_KEY}", at as i64))?;
            p.ticks(8)?;
            let gone = !p.is(t, "minecraft:bedrock")?;
            let down = p.is(a, "minecraft:piston[facing=down]")?;
            println!("round {round} offset {off:.0}: {res} -> gone={gone} down={down}");
            results.push((off, gone));
            if gone {
                println!("HIT at offset {off}");
                return Ok(());
            }
            if down {
                repair(p, t)?;
                p.ticks(10)?;
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn unused() -> Result<()> {
    let _ = yt::ROOF_Y;
    Ok(())
}
