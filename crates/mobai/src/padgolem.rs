//! (tickexact 2026-11-12, ironspec: decision-critical, the lead scarer posts against a fresh pad
//! golem) `mctas mobai pad-golems`: on the official 26.3 server, the stand's west end in its real
//! geometry from dfb89499's station file (explore/farmopt-m1-restart d1a5f2985,
//! wip/station/f1q2-station.txt, layers y -9..-4: the pad's two nether portal rows x -2..7 at z -5
//! and -2, y -8..-6, their obsidian frames, the cobblestone floor rows at y -7, z -6/-4/-3/-1, the
//! glass over the frames), shifted up by `DY` into open air; 9fc3155f's two glass scarer posts
//! (explore/farmopt-c3-scarers2 4749723f2, the budget's cells): the pillager in a boat at
//! (-5,-1,-5) and the zombie in a boat at (-5,-4,-2), both open east, each boat at its cell's centre
//! as a number (checked at the start: the run is invalid otherwise); the stand's three summoners
//! riding carts at their pockets' rest points (x+0.5, 0.0625, 0.5), x -1/3/7 (the pillager's
//! nearest 9.35 away); at night. The target agent (bridge/agent TargetAgent.java) prints every
//! target the pillager and the golems take (a mob's target isn't in its saved data).
//!
//! Rows: golems summoned one at a time on the pad's floor cells (feet y -6), each after a random
//! 1..20 gt, every cell within 16 of a post, 60+ spawns; again with zombified piglins standing in
//! the portal cells beside each spawn as push sources (their portal cooldown kept from running out
//! while inside); the worst case, a villager within 8 that the pillager sees (invulnerable), each
//! golem summoned in a tick the pillager's crossbow is charged. Per golem: the pillager's target,
//! crossbow and ticks since its last shot at the summon; the tick it leaves the overworld; its
//! positions and health; the targets it and the pillager take; every arrow. The controls: the z -5
//! portal row blocked (obsidian), so a golem on the z -4 floor row touches no portal and stays:
//! with the summoners (the pillager keeps its villager) and without any villager (the pillager
//! takes the golem: its shots and the golem's damage show the scene sees the effect).

use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read, Seek, SeekFrom};

const DY: i32 = 80;

#[derive(Clone, Copy, PartialEq)]
enum Riders {
    Summoners,
    SummonersAndNear,
    None,
}

fn at(x: f64, y: f64, z: f64) -> String {
    format!("{} {} {}", x, y + DY as f64, z)
}

fn num_after(out: &str, key: &str) -> Option<f64> {
    out.split(key).nth(1).and_then(|s| s.trim_start_matches([':', ' ']).chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect::<String>().parse().ok())
}

/// An entity's position (y less `DY`), its selector run in `dim` when given (a selector with
/// `distance=0..` then sees that dimension only: a bare `@e` sees every dimension).
fn pos_in(srv: &mut VanillaServer, dim: Option<&str>, sel: &str) -> Result<Option<[f64; 3]>> {
    let cmd = match dim {
        Some(d) => format!("execute in minecraft:{d} run data get entity {sel} Pos"),
        None => format!("data get entity {sel} Pos"),
    };
    let out = srv.cmd_long(&cmd)?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1] - DY as f64, v[2]]))
}

/// A glass post open east around (x, y, z) (two cells high), with `mob` in a boat inside: the
/// cells of 9fc3155f's budget (floor (x,y-1,z), roof (x,y+2,z), walls (x,y..y+1,z-1),
/// (x,y..y+1,z+1), (x-1,y..y+1,z)); the boat at the cell's centre as a number (the budget's
/// emitted "{x}.5" boats sit a cell off).
fn post(srv: &mut VanillaServer, x: i32, y: i32, z: i32, mob: &str) -> Result<()> {
    let y0 = y + DY;
    srv.cmd_ok(&format!("setblock {x} {} {z} minecraft:glass", y0 - 1))?;
    srv.cmd_ok(&format!("setblock {x} {} {z} minecraft:glass", y0 + 2))?;
    for dy in 0..2 {
        srv.cmd_ok(&format!("setblock {} {} {z} minecraft:glass", x - 1, y0 + dy))?;
        srv.cmd_ok(&format!("setblock {x} {} {} minecraft:glass", y0 + dy, z - 1))?;
        srv.cmd_ok(&format!("setblock {x} {} {} minecraft:glass", y0 + dy, z + 1))?;
    }
    // a passenger loaded from summon data gets no finalizeSpawn: the pillager's crossbow is given
    let gear = if mob == "pillager" { ",equipment:{mainhand:{id:\"minecraft:crossbow\",count:1}}" } else { "" };
    srv.cmd_ok(&format!("summon minecraft:oak_boat {} {{Passengers:[{{id:\"minecraft:{mob}\",PersistenceRequired:1b,Tags:[\"post\"]{gear}}}]}}", at(x as f64 + 0.5, y as f64, z as f64 + 0.5)))?;
    Ok(())
}

fn build(srv: &mut VanillaServer, station: &std::path::Path, block_row: bool, riders: Riders) -> Result<()> {
    srv.cmd_ok(&format!("fill -12 {} -12 16 {} 8 minecraft:air", DY - 12, DY + 6))?;
    let d = mctas::design::Design::load(station)?;
    for pass in 0..2 {
        for (p, s) in d.blocks.iter() {
            if !(-9..=-4).contains(&p[1]) {
                continue;
            }
            let portal = s.contains("nether_portal");
            if (pass == 0) == portal {
                continue;
            }
            let s = if block_row && portal && p[2] == -5 { "minecraft:obsidian" } else { s.as_str() };
            srv.cmd_ok(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
        }
    }
    post(srv, -5, -1, -5, "pillager")?;
    post(srv, -5, -4, -2, "zombie")?;
    if riders != Riders::None {
        for x in [-1, 3, 7] {
            srv.cmd_ok(&format!("setblock {x} {} 0 minecraft:stone", DY - 1))?;
            srv.cmd_ok(&format!("setblock {x} {DY} 0 minecraft:rail"))?;
            // (not Invulnerable: an invulnerable entity can't be seen as an enemy, so no mob targets it)
            srv.cmd_ok(&format!("summon minecraft:minecart {} {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b,Tags:[\"summoner\"]}}]}}", at(x as f64 + 0.5, 0.0625, 0.5)))?;
        }
    }
    if riders == Riders::SummonersAndNear {
        // 7.0 from the pillager, in its sight through the post's open east side; healed every tick
        // (the pillager shoots it)
        srv.cmd_ok(&format!("setblock 2 {} -5 minecraft:glass", DY - 2))?;
        srv.cmd_ok(&format!("summon minecraft:oak_boat {} {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b,Tags:[\"near\"]}}]}}", at(2.5, -1.0, -4.5)))?;
    }
    Ok(())
}

/// At the scene's start: each scarer's boat and rider inside its glass box (the post cell), the
/// cell east of it open (ironspec: the run is invalid otherwise).
fn check_posts(srv: &mut VanillaServer) -> Result<()> {
    for (mob, x, y, z) in [("pillager", -5, -1, -5), ("zombie", -5, -4, -2)] {
        let rider = pos_in(srv, None, &format!("@e[type=minecraft:{mob},limit=1]"))?;
        let boat = pos_in(srv, None, &format!("@e[type=minecraft:oak_boat,limit=1,sort=nearest,x={},y={},z={}]", x as f64 + 0.5, y + DY, z as f64 + 0.5))?;
        let open = srv.test_block([x + 1, y + DY, z], "minecraft:air")? && srv.test_block([x + 1, y + 1 + DY, z], "minecraft:air")?;
        // horizontally in the post cell; the boat on the post's floor, its rider seated in it (a
        // seated rider's feet sit below the boat's: the pillager's 0.4125, the zombie's 0.5125)
        let cell = |p: [f64; 3]| p[0] >= x as f64 && p[0] < (x + 1) as f64 && p[2] >= z as f64 && p[2] < (z + 1) as f64;
        let boat_in = boat.is_some_and(|p| cell(p) && p[1] >= y as f64 && p[1] < (y + 1) as f64);
        let rider_in = rider.is_some_and(|p| cell(p) && p[1] >= (y - 1) as f64 && p[1] < (y + 2) as f64);
        let seated = srv
            .cmd_long(&format!("data get entity @e[type=minecraft:oak_boat,limit=1,sort=nearest,x={},y={},z={}] Passengers[0].id", x as f64 + 0.5, y + DY, z as f64 + 0.5))?
            .contains(&format!("minecraft:{mob}"));
        println!(
            "  post {mob} ({x},{y},{z}): boat at {:?}, rider at {:?}; boat in its glass box: {boat_in}; rider in it: {rider_in}, seated in the boat: {seated}; open east: {open}",
            boat.map(|p| format!("({:.4}, {:.4}, {:.4})", p[0], p[1], p[2])),
            rider.map(|p| format!("({:.4}, {:.4}, {:.4})", p[0], p[1], p[2])),
        );
        if !(boat_in && rider_in && seated && open) {
            bail!("pad-golems: the {mob}'s boat or rider is outside its post, or its east side is closed: the run is invalid");
        }
    }
    Ok(())
}

/// The pillager's crossbow: charged or not.
fn crossbow(srv: &mut VanillaServer) -> Result<&'static str> {
    let out = srv.cmd_long("data get entity @e[type=minecraft:pillager,limit=1] equipment.mainhand")?;
    Ok(if out.contains("charged_projectiles") { "charged" } else if out.contains("crossbow") { "uncharged" } else { "no crossbow" })
}

/// What the harness follows every tick: the arrows in the world (a new one is a shot), the last
/// shot's game time, the target agent's lines (read from the server log as it grows): the
/// pillager's target ("type uuid") and the current golem's own targets.
struct Watch {
    arrows: u32,
    last_shot: Option<i64>,
    shots_at_golem: u32,
    log_at: u64,
    pillager_target: String,
    golem: Option<String>,
    golem_targets: Vec<String>,
    heal_near: bool,
}

impl Watch {
    fn new(heal_near: bool) -> Watch {
        Watch { arrows: 0, last_shot: None, shots_at_golem: 0, log_at: 0, pillager_target: "none".into(), golem: None, golem_targets: Vec::new(), heal_near }
    }

    /// The pillager's target in words: this golem, an earlier golem (gone through the portal), a
    /// villager, or none.
    fn target(&self) -> String {
        let mut it = self.pillager_target.split_whitespace();
        let (ty, uuid) = (it.next().unwrap_or("none"), it.next().unwrap_or(""));
        match ty {
            "minecraft:iron_golem" if self.golem.as_deref() == Some(uuid) => "this golem".into(),
            "minecraft:iron_golem" => "an earlier golem (gone through the portal)".into(),
            "none" => "none".into(),
            t => t.to_string(),
        }
    }

    fn poll(&mut self, srv: &mut VanillaServer) -> Result<()> {
        let mut f = std::fs::File::open(&srv.log).with_context(|| format!("open {}", srv.log.display()))?;
        f.seek(SeekFrom::Start(self.log_at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        // only whole lines
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.log_at += whole as u64;
        for l in s[..whole].lines() {
            let Some(rest) = l.split("mctas target agent: t=").nth(1) else { continue };
            let Some((who, target)) = rest.split_once(" -> ") else { continue };
            let target = target.trim().to_string();
            if who.contains("minecraft:pillager") {
                self.pillager_target = target;
            } else if who.contains("minecraft:iron_golem") && self.golem.as_deref().is_some_and(|g| who.contains(g)) {
                let t = who.split_whitespace().next().unwrap_or("");
                self.golem_targets.push(format!("t={t} {}", target.split_whitespace().next().unwrap_or("none")));
            }
        }
        let n = srv.count_entities("@e[type=minecraft:arrow]")?;
        if n > self.arrows {
            self.last_shot = Some(srv.game_time()?);
            if self.target() == "this golem" {
                self.shots_at_golem += n - self.arrows;
            }
        }
        self.arrows = n;
        if self.heal_near {
            srv.cmd("data merge entity @e[tag=near,limit=1] {Health:20.0f}")?;
        }
        Ok(())
    }

    fn step(&mut self, srv: &mut VanillaServer, n: u32) -> Result<()> {
        for _ in 0..n {
            srv.step(1)?;
            self.poll(srv)?;
        }
        Ok(())
    }
}

struct Golem {
    gone: Option<u32>,
    shots_at_it: u32,
    damage: f64,
    moved_west: f64,
    line: String,
}

/// One golem summoned at the floor cell (x, z) (feet y -6): until it leaves the overworld or `limit`
/// gt.
fn golem(srv: &mut VanillaServer, w: &mut Watch, x: i32, z: i32, k: usize, limit: u32) -> Result<Golem> {
    let tag = format!("pg{k}");
    let t0 = srv.game_time()?;
    let cb0 = crossbow(srv)?;
    w.golem = None;
    let at_spawn = format!(
        "pillager target {}, crossbow {cb0}, last shot {}",
        w.target(),
        w.last_shot.map(|t| format!("{} gt before", t0 - t)).unwrap_or_else(|| "none yet".into())
    );
    w.golem_targets.clear();
    let shots0 = w.shots_at_golem;
    srv.cmd_ok(&format!("summon minecraft:iron_golem {} {{Tags:[\"{tag}\"],PersistenceRequired:1b}}", at(x as f64 + 0.5, -6.0, z as f64 + 0.5)))?;
    let sel = format!("@e[type=minecraft:iron_golem,tag={tag},limit=1,distance=0..]");
    // its UUID as the agent prints it (getStringUUID: 8-4-4-4-12 hex of the four ints)
    let u = srv.cmd_long(&format!("execute in minecraft:overworld run data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    if ints.len() != 4 {
        bail!("pad-golems: golem {k}'s UUID: {u}");
    }
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    w.golem = Some(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]));
    let mut path = Vec::new();
    let (mut gone, mut hp_min) = (None, 100.0f64);
    let (mut first, mut last) = (None, None);
    for t in 1..=limit {
        w.step(srv, 1)?;
        match pos_in(srv, Some("overworld"), &sel)? {
            Some(q) => {
                let h = srv.cmd_long(&format!("execute in minecraft:overworld run data get entity {sel} Health"))?;
                let hp = num_after(&h, "entity data").context("a golem's Health")?;
                hp_min = hp_min.min(hp);
                if t <= 2 || t % 20 == 0 {
                    path.push(format!("t{t} ({:.2},{:.2},{:.2}) hp {hp}", q[0], q[1], q[2]));
                }
                first.get_or_insert(q);
                last = Some(q);
            }
            None => {
                gone = Some(t);
                let nether = pos_in(srv, Some("the_nether"), &sel)?;
                path.push(format!(
                    "last in the overworld {}; in the Nether at +{t}: {}",
                    last.map(|q| format!("({:.3},{:.3},{:.3})", q[0], q[1], q[2])).unwrap_or_else(|| "(its summon tick only)".into()),
                    nether.map(|q| format!("({:.2},{:.2},{:.2})", q[0], q[1] + DY as f64, q[2])).unwrap_or_else(|| "not seen (hidden in a generating chunk)".into())
                ));
                break;
            }
        }
    }
    let cb1 = crossbow(srv)?;
    let shots_at_it = w.shots_at_golem - shots0;
    let moved_west = match (first, last) {
        (Some(a), Some(b)) => a[0] - b[0],
        _ => 0.0,
    };
    let line = format!(
        "golem {k} at ({x},-6,{z}): at the summon {at_spawn}; {}; its targets [{}]; the pillager's target after: {}; crossbow {cb1}; shots while the pillager targeted this golem {shots_at_it}; least health {hp_min}; {}",
        gone.map(|t| format!("left the overworld at +{t} gt")).unwrap_or_else(|| format!("STAYED {limit} gt")),
        w.golem_targets.join(", "),
        w.target(),
        path.join(" ")
    );
    srv.cmd_ok(&format!("kill @e[type=minecraft:iron_golem,tag={tag}]"))?;
    Ok(Golem { gone, shots_at_it, damage: 100.0 - hp_min, moved_west, line })
}

#[derive(Default, Debug)]
struct Tally {
    crossed: u32,
    stayed: u32,
    shots_at_golems: u32,
    hurt: u32,
    went_west: u32,
}

struct Row<'a> {
    label: &'a str,
    block_row: bool,
    push: bool,
    riders: Riders,
    cells: &'a [(i32, i32)],
    limit: u32,
}

fn run(version: &str, station: &std::path::Path, row: &Row) -> Result<Tally> {
    println!("== {}", row.label);
    let jar = mctas::bridge::target_agent(version)?;
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}=minecraft:pillager,minecraft:iron_golem", jar.display()))),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 18000")?;
    srv.cmd_ok("forceload add -16 -16 31 15")?;
    srv.step(20)?;
    build(&mut srv, station, row.block_row, row.riders)?;
    let mut w = Watch::new(row.riders == Riders::SummonersAndNear);
    w.step(&mut srv, 5)?;
    check_posts(&mut srv)?;
    // (fresh chunks: a crossing in their first ~340 gt waits hidden; build, then wait 400 gt)
    w.step(&mut srv, 400)?;
    check_posts(&mut srv)?;
    println!("  after 400 gt: the pillager's target {}, its crossbow {}, arrows so far {}", w.target(), crossbow(&mut srv)?, w.arrows);
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut rnd = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut tally = Tally::default();
    for (k, &(x, z)) in row.cells.iter().enumerate() {
        if row.push {
            // zombified piglins in the portal cells beside the spawn cell, their cooldown full
            for zp in [z - 1, z + 1] {
                if zp == -5 || zp == -2 {
                    srv.cmd_ok(&format!("summon minecraft:zombified_piglin {} {{PortalCooldown:300,PersistenceRequired:1b,Tags:[\"push\"]}}", at(x as f64 + 0.5, -8.0, zp as f64 + 0.5)))?;
                }
            }
        }
        w.step(&mut srv, rnd(20) as u32 + 1)?;
        if row.riders == Riders::SummonersAndNear {
            // the worst case: summon in a tick the pillager's crossbow is charged
            let mut waited = 0;
            while crossbow(&mut srv)? != "charged" {
                w.step(&mut srv, 1)?;
                waited += 1;
                if waited > 600 {
                    bail!("pad-golems: the pillager never charged its crossbow at the near villager in 600 gt");
                }
            }
        }
        let g = golem(&mut srv, &mut w, x, z, k, row.limit)?;
        println!("  {}", g.line);
        if g.gone.is_some() {
            tally.crossed += 1;
        } else {
            tally.stayed += 1;
        }
        tally.shots_at_golems += g.shots_at_it;
        if g.damage > 0.0 {
            tally.hurt += 1;
        }
        if g.gone.is_none() && g.moved_west > 1.0 {
            tally.went_west += 1;
        }
        if row.push {
            srv.cmd_ok("kill @e[tag=push]")?;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!(
        "  {}: {} crossed, {} stayed ({} of them went west toward the posts), {} shots while the pillager targeted a golem, {} golems hurt",
        row.label, tally.crossed, tally.stayed, tally.went_west, tally.shots_at_golems, tally.hurt
    );
    Ok(tally)
}

pub fn pad_golems(version: &str, station: &std::path::Path) -> Result<()> {
    println!("== pad-golems on the official {version} server: the stand's west end (dfb89499's station file), a pillager and a zombie in their posts, golems summoned on the pad");
    // every floor cell (z -6/-4/-3/-1, x -3..8) within 16 of a post, in order, 60+ spawns
    let mut cells = Vec::new();
    for x in -3..=8 {
        for z in [-6, -4, -3, -1] {
            let d = |px: f64, py: f64, pz: f64| ((x as f64 + 0.5 - px).powi(2) + (-6.0 - py).powi(2) + (z as f64 + 0.5 - pz).powi(2)).sqrt();
            if d(-4.5, -1.0, -4.5) <= 16.0 || d(-4.5, -4.0, -1.5) <= 16.0 {
                cells.push((x, z));
            }
        }
    }
    let mut spawns = cells.clone();
    spawns.extend(cells.iter().take(16).copied());
    let worst: Vec<(i32, i32)> = cells.iter().copied().step_by(2).collect();
    let control = [(0, -4), (3, -4), (6, -4)];
    let rows = [
        Row { label: "golems on the pad, the summoners at their rest points (60+ spawns)", block_row: false, push: false, riders: Riders::Summoners, cells: &spawns, limit: 40 },
        Row { label: "the same, zombified piglins in the portal cells beside each spawn", block_row: false, push: true, riders: Riders::Summoners, cells: &cells, limit: 40 },
        Row { label: "the worst case: a villager 7 from the pillager in its sight, each golem summoned while its crossbow is charged", block_row: false, push: false, riders: Riders::SummonersAndNear, cells: &worst, limit: 40 },
        Row { label: "control: the z -5 portal row blocked, golems on the z -4 floor row, the summoners present", block_row: true, push: false, riders: Riders::Summoners, cells: &control, limit: 200 },
        Row { label: "control: the z -5 portal row blocked, no villager anywhere", block_row: true, push: false, riders: Riders::None, cells: &control, limit: 200 },
    ];
    let mut t = Vec::new();
    for row in &rows {
        t.push(run(version, station, row)?);
    }
    println!("verdict:");
    for (row, t) in rows.iter().zip(&t) {
        println!("  {}: {:?}", row.label, t);
    }
    let bar = t[..3].iter().all(|t| t.stayed == 0 && t.shots_at_golems == 0 && t.hurt == 0);
    println!("  the bar (every golem crosses, 0 shots at a golem before its crossing, 0 lingering) on the three pad rows: {}", if bar { "MET" } else { "NOT MET" });
    if t[3].stayed == 0 || t[4].stayed == 0 || (t[4].shots_at_golems == 0 && t[4].hurt == 0 && t[4].went_west == 0) {
        bail!("pad-golems: a control's golems crossed or showed nothing (no shot, no damage, no move toward a post): the scene can't see the effect");
    }
    Ok(())
}
