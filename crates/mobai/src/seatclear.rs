//! (tickexact 2026-11-12, ironspec: every rider's bed claim rests on it) `mctas mobai seat-clear`:
//! on the official 26.3 server, when does a seated rider's CANT_REACH_WALK_TARGET_SINCE stamp
//! clear, and when is its HOME released? onechunk's assembled module (a9a418d03,
//! wip/onechunk/module/f1q2-module-first-assembly.txt, the overworld part placed whole, shifted up
//! by `DY` into cleared air: rails after the other blocks, portals last) with its bed; a villager
//! riding a cart at a seat with HOME claimed on that bed (more than 16 and less than 150 away), at
//! night (18,000, time held). The brain agent (bridge/agent BrainAgent.java) prints every change of
//! WALK_TARGET, the stamp and HOME with the behaviour that made it, and every activity switch; the
//! harness steps one tick at a time and logs each event at its tick. 26.3's code: the stamp is
//! set by MoveToTargetSink.tryComputePath (line 125) when no complete path reaches the walk target,
//! and erased there when the target is reached (61, 119) or a complete path is computed (123), and
//! by SleepInBed.start (98, the nap); HOME is released by SetWalkTargetFromBlockMemory (the REST
//! package: HOME, close enough 1, too far 150, too long unreachable 1,200) once the stamp is older
//! than 1,200, the stamp then reset to now. The PANIC package's walk targets: SetWalkTargetAwayFrom
//! (the nearest hostile, the hurt-by entity; LandRandomPos.getPosAway 16 x 7) and
//! VillageBoundRandomStroll (2, 2).
//!
//! Rows: (1) the station pocket P0 (-1,0,0) with the lead posts (the pillager at (-4.5,-1,-4.5),
//! the zombie at (-4.5,-4,-1.5), seated in boats): `--dwells` dwells of `--dwell` gt at rest in the
//! pocket, between them `--away` gt seated in an enclosed cart far from the module (dismounted,
//! teleported and mounted again: the brain is the same villager's); per dwell the panic ticks, the
//! stroll draws (null or not), the flee draws, each stamp set and erasure with its cause; the
//! per-dwell miss rate (no erasure in the dwell). (2) The calm control: a lane cell (-2,24,7) of
//! the merge row, no scarer within reach: the stamp arms and HOME goes at the first calm REST tick
//! past 1,200. (3) Continuous panic with a pre-aged stamp: the lane seat calm until its stamp is
//! `--preage` gt old, then a zombie seated in a boat 3 from it until the stamp is 1,700 old (HOME
//! kept while panicking?), then the zombie removed (HOME lost at the first calm REST tick?).

use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read, Seek, SeekFrom};

const DY: i32 = 80;
pub(crate) const MEMS: &str = "minecraft:walk_target,minecraft:cant_reach_walk_target_since,minecraft:home,minecraft:path,minecraft:nearest_hostile";

fn at(x: f64, y: f64, z: f64) -> String {
    format!("{} {} {}", x, y + DY as f64, z)
}

/// The brain agent's lines since the last call.
pub(crate) struct Agent {
    pub(crate) at: u64,
}

impl Agent {
    pub(crate) fn drain(&mut self, srv: &VanillaServer) -> Result<Vec<String>> {
        let mut f = std::fs::File::open(&srv.log).with_context(|| format!("open {}", srv.log.display()))?;
        f.seek(SeekFrom::Start(self.at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.at += whole as u64;
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).filter(|l| !l.starts_with("Brain reports")).collect())
    }
}

/// Places the module's overworld part (rails after the other blocks, portals last) in cleared air;
/// returns its bed head's cell.
fn place_module(srv: &mut VanillaServer, module: &std::path::Path) -> Result<[i32; 3]> {
    place_module_with(srv, module, true)
}

/// Places the module (rails after the other blocks, portals last). With `strict_rails` every rail
/// goes in as the file states it, without its onPlace (`setblock ... strict`: no `RailState.place`
/// re-shaping it from its neighbours of the moment), which is the built state the guide's
/// placement order produces (the top slope's helper order, C's powered placement); without it a
/// rail takes whatever shape its neighbours give it when placed (the top slope (x,22,6) then
/// settles flat). Returns the bed's head.
pub(crate) fn place_module_with(srv: &mut VanillaServer, module: &std::path::Path, strict_rails: bool) -> Result<[i32; 3]> {
    for y0 in (DY - 24..DY + 34).step_by(16) {
        srv.cmd_ok(&format!("fill -16 {y0} -16 19 {} 20 minecraft:air", (y0 + 15).min(DY + 33)))?;
    }
    let d = mctas::design::Design::load(module)?;
    let mut bed = None;
    for pass in 0..3 {
        for (p, s) in d.blocks.iter() {
            let kind = if s.contains("nether_portal") {
                2
            } else if s.contains("rail") {
                1
            } else {
                0
            };
            if kind != pass {
                continue;
            }
            if s.contains("_bed") && s.contains("part=head") {
                bed = Some(*p);
            }
            let mode = if kind == 1 && strict_rails { " strict" } else { "" };
            srv.cmd_ok(&format!("setblock {} {} {} {s}{mode}", p[0], p[1] + DY, p[2]))?;
        }
    }
    bed.context("the module has no bed")
}

/// Every block of the module that differs in the world from the file (its position and the
/// file's state).
pub(crate) fn module_mismatches(srv: &mut VanillaServer, module: &std::path::Path) -> Result<Vec<String>> {
    let d = mctas::design::Design::load(module)?;
    let mut out = Vec::new();
    for (p, s) in d.blocks.iter() {
        if !srv.cmd_long(&format!("execute if block {} {} {} {s}", p[0], p[1] + DY, p[2]))?.contains("passed") {
            out.push(format!("({},{},{}) {s}", p[0], p[1], p[2]));
        }
    }
    Ok(out)
}

/// `mctas mobai module-check`: the module placed both ways (rails with their onPlace, and strict),
/// 20 gt later every block that differs from the file; `with_air`: with the strict placement, also
/// every cell the file leaves empty inside its bounding box (a fluid's flow, a block's drop), each
/// differing one identified.
pub fn module_check(version: &str, module: &std::path::Path, with_air: bool) -> Result<()> {
    for strict in [false, true] {
        let (mut r, _bed) = start_placed(version, module, strict)?;
        r.srv.step(20)?;
        let m = module_mismatches(&mut r.srv, module)?;
        println!("== {} rails: {} blocks differ from the file 20 gt after placing: {}", if strict { "strict" } else { "onPlace" }, m.len(), m.join("; "));
        if strict && with_air {
            let d = mctas::design::Design::load(module)?;
            let keys: Vec<[i32; 3]> = d.blocks.iter().map(|(p, _)| [p[0], p[1], p[2]]).collect();
            let lo = (0..3).map(|k| keys.iter().map(|p| p[k]).min().unwrap_or(0)).collect::<Vec<_>>();
            let hi = (0..3).map(|k| keys.iter().map(|p| p[k]).max().unwrap_or(0)).collect::<Vec<_>>();
            let listed: std::collections::BTreeSet<[i32; 3]> = keys.into_iter().collect();
            let (mut n, mut bad) = (0u32, Vec::new());
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        if listed.contains(&[x, y, z]) {
                            continue;
                        }
                        n += 1;
                        if !r.srv.cmd_long(&format!("execute if block {x} {} {z} minecraft:air", y + DY))?.contains("passed") {
                            let found = crate::bridgehash::identify(&mut r.srv, [x, y, z])?;
                            bad.push(format!("({x},{y},{z}) {}", found.as_deref().unwrap_or("(not among the candidates)")));
                        }
                    }
                }
            }
            println!(
                "== strict rails, the file's empty cells inside x {}..{} y {}..{} z {}..{}: {} of {n} are not air: {}",
                lo[0],
                hi[0],
                lo[1],
                hi[1],
                lo[2],
                hi[2],
                bad.len(),
                bad.join("; ")
            );
        }
        let _ = std::fs::remove_dir_all(&r.srv.dir);
    }
    Ok(())
}

fn rider_summon(srv: &mut VanillaServer, seat: [f64; 3], bed: [i32; 3], tag: &str) -> Result<()> {
    srv.cmd_ok(&format!(
        "summon minecraft:minecart {} {{Tags:[\"{tag}\"],Passengers:[{{id:\"minecraft:villager\",Tags:[\"rider\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
        at(seat[0], seat[1], seat[2]),
        bed[0],
        bed[1] + DY,
        bed[2]
    ))?;
    Ok(())
}

fn home(srv: &mut VanillaServer) -> Result<bool> {
    Ok(srv.cmd_long("data get entity @e[tag=rider,limit=1] Brain.memories.\"minecraft:home\"")?.contains("entity data"))
}

/// What one tick's agent lines say, folded into counts.
#[derive(Default, Clone)]
struct Counts {
    panic_ticks: u32,
    stroll: u32,
    stroll_null: u32,
    flee: u32,
    flee_null: u32,
    stamp_set: u32,
    erased_reached: u32,
    erased_path: u32,
    erased_nap: u32,
    home_lost: u32,
    /// MoveToTargetSink's erase calls, the stamp there or not: each a walk target reached
    /// (reach_reached) or completely pathed (reach_path)
    reach_reached: u32,
    reach_path: u32,
    /// The rider's block (for each landed draw's target relative to it) and the dwell's draws in
    /// order: the landed target's cell relative to the rider's (None: a null draw), how the target
    /// ended ("reached", "complete path", or "" unreached), and whether it cleared the stamp
    origin: [i32; 3],
    seq: Vec<(Option<[i32; 3]>, &'static str, bool)>,
}

/// The cell of a walk target printed as "walk target (x, y, z) close enough n" (y less `DY`).
fn target_cell(l: &str) -> Option<[i32; 3]> {
    let s = l.split("walk target (").nth(1)?.split(')').next()?;
    let v: Vec<f64> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
    (v.len() == 3).then(|| [v[0].floor() as i32, v[1].floor() as i32 - DY, v[2].floor() as i32])
}

impl Counts {
    fn add(&mut self, l: &str) {
        let by = l.rsplit(" by ").next().unwrap_or("");
        if l.starts_with("set minecraft:walk_target") && by.starts_with("VillageBoundRandomStroll") {
            self.stroll += 1;
            let rel = target_cell(l).map(|c| [c[0] - self.origin[0], c[1] - self.origin[1], c[2] - self.origin[2]]);
            self.seq.push((Some(rel.unwrap_or([99, 99, 99])), "", false));
        } else if l.starts_with("erase minecraft:walk_target") && by.starts_with("VillageBoundRandomStroll") {
            self.stroll_null += 1;
            self.seq.push((None, "", false));
        } else if l.starts_with("set minecraft:walk_target") && by.starts_with("SetWalkTargetAwayFrom") {
            self.flee += 1;
        } else if l.starts_with("erase minecraft:walk_target") && by.starts_with("SetWalkTargetAwayFrom") {
            self.flee_null += 1;
        } else if l.starts_with("set minecraft:cant_reach_walk_target_since") {
            self.stamp_set += 1;
        } else if l.starts_with("erase minecraft:cant_reach_walk_target_since") {
            // 26.3's jar: MoveToTargetSink.tryComputePath line 138 erases on a reached target,
            // 142 on a complete path, 144 sets the stamp; checkExtraStartConditions erases only on
            // a reached target; SleepInBed.start on the nap
            let had = !l.contains("(absent)");
            // the outcome of the latest landed draw still open
            let mut mark = |how: &'static str, had: bool, seq: &mut Vec<(Option<[i32; 3]>, &'static str, bool)>| {
                if let Some(d) = seq.iter_mut().rev().find(|d| d.0.is_some()) {
                    if d.1.is_empty() {
                        d.1 = how;
                        d.2 = had;
                    }
                }
            };
            if by.starts_with("SleepInBed") {
                if had {
                    self.erased_nap += 1;
                }
            } else if by.starts_with("MoveToTargetSink.tryComputePath:142") {
                self.reach_path += 1;
                mark("complete path", had, &mut self.seq);
                if had {
                    self.erased_path += 1;
                }
            } else if by.starts_with("MoveToTargetSink") {
                self.reach_reached += 1;
                mark("reached", had, &mut self.seq);
                if had {
                    self.erased_reached += 1;
                }
            }
        } else if l.starts_with("erase minecraft:home") && !l.contains("(absent)") {
            self.home_lost += 1;
        }
    }

    fn erased(&self) -> u32 {
        self.erased_reached + self.erased_path + self.erased_nap
    }

    fn merge(&mut self, x: &Counts) {
        self.panic_ticks += x.panic_ticks;
        self.stroll += x.stroll;
        self.stroll_null += x.stroll_null;
        self.flee += x.flee;
        self.flee_null += x.flee_null;
        self.stamp_set += x.stamp_set;
        self.erased_reached += x.erased_reached;
        self.erased_path += x.erased_path;
        self.erased_nap += x.erased_nap;
        self.home_lost += x.home_lost;
        self.reach_reached += x.reach_reached;
        self.reach_path += x.reach_path;
    }
}

/// A two-sided 95% Wilson interval of a proportion `k` of `n`.
/// The two-sided 95% Wilson interval (for the lane run).
pub(crate) fn wilson95_pub(k: u32, n: u32) -> (f64, f64) {
    wilson95(k, n)
}

/// A cell whose stated block holds water: a fluid cell (rule B, ironspec 00:27 PT: every fluid
/// cell's tick is scheduled at the paste).
pub(crate) fn holds_water(s: &str) -> bool {
    s.starts_with("minecraft:water") || s.starts_with("minecraft:bubble_column") || s.contains("waterlogged=true")
}

/// A part file laid over the module (9fc3155f's F3 scarer part: its glass and glowstone, its seated
/// pillager): blocks as the file states them, entities summoned at their positions. A pillager
/// summoned as a passenger gets no `finalizeSpawn`, so it is given the crossbow a natural one
/// carries, and the tag f3 (its charge state is read). (ironspec 00:27 PT, rule B) every fluid cell
/// gets its fluid tick scheduled: a plain setblock of water there first (LiquidBlock.onPlace
/// schedules it; SetBlockCommand's strict mode skips onPlace, flag 512), then its stated block over
/// it (the scheduled tick stays keyed to the cell and runs on that block's water); the count is
/// printed, and a design with water but no scheduled cell fails.
pub(crate) fn place_part(srv: &mut VanillaServer, part: &std::path::Path) -> Result<()> {
    place_part_skipping(srv, part, &[])
}

/// [`place_part`], with the cells in `skip_fresh` laid as stated without their water first (no
/// onPlace, no scheduled tick) and left out of the guard: only water-settle's control uses it, to
/// plant a fluid cell with no tick of its own.
pub(crate) fn place_part_skipping(srv: &mut VanillaServer, part: &std::path::Path, skip_fresh: &[[i32; 3]]) -> Result<()> {
    let d = mctas::design::Design::load(part)?;
    // the other blocks first, then the rails as stated (strict, as the module's); a cell already in
    // that state is left as it is (`cmd`: the setblock then reports no change), and module-check
    // reports whatever differs afterwards
    let (mut water, mut logged) = (0u32, 0u32);
    let mut unscheduled: Vec<String> = Vec::new();
    for pass in 0..2 {
        for (p, s) in d.blocks.iter() {
            let rail = s.contains("rail");
            let at = format!("{} {} {}", p[0], p[1] + DY, p[2]);
            let skipped = skip_fresh.contains(&[p[0], p[1], p[2]]);
            // a fluid cell: water first, plainly, in the first pass (a rail's goes in before the rail).
            // (ironspec 02:28 PT (f1)) The cell is emptied without updates first, so the water
            // setblock always places fresh water and runs LiquidBlock.onPlace (its tick scheduled);
            // only a setblock the server reports as changed counts, any other reply fails the run
            if pass == 0 && holds_water(s) && !skipped {
                srv.cmd(&format!("setblock {at} minecraft:air strict"))?;
                let out = srv.cmd(&format!("setblock {at} minecraft:water"))?;
                if !out.contains("Changed the block") {
                    unscheduled.push(format!("({},{},{}) {}", p[0], p[1], p[2], out.trim()));
                } else if s.starts_with("minecraft:water") {
                    water += 1;
                } else {
                    logged += 1;
                }
            }
            if skipped && pass == 0 {
                // the control's planted cell: its stated state, strictly (no onPlace, no tick)
                srv.cmd(&format!("setblock {at} {s} strict"))?;
                continue;
            }
            if skipped {
                continue;
            }
            if rail != (pass == 1) {
                continue;
            }
            if s.as_str() == "minecraft:water" {
                continue;
            }
            srv.cmd(&format!("setblock {at} {s}{}", if rail { " strict" } else { "" }))?;
        }
    }
    let fluid = d.blocks.iter().filter(|(p, s)| holds_water(s) && !skip_fresh.contains(&[p[0], p[1], p[2]])).count() as u32;
    println!(
        "  place_part {}: fluid cells placed fresh (the server reported each water setblock changed: onPlace ran): {} (water blocks {water}, waterlogged / bubble cells {logged}) of {fluid} in the design{}",
        part.display(),
        water + logged,
        if skip_fresh.is_empty() { String::new() } else { format!(" (+{} planted without a tick: a control)", skip_fresh.len()) }
    );
    anyhow::ensure!(unscheduled.is_empty() && water + logged == fluid, "place_part: {} of {fluid} fluid cells not placed fresh (no onPlace, no scheduled tick): {}: the run fails", unscheduled.len(), unscheduled.join("; "));
    for e in &d.entities {
        let nbt = e.nbt.clone().unwrap_or_default().replace(
            "id:\"minecraft:pillager\"",
            "id:\"minecraft:pillager\",Tags:[\"f3\"],equipment:{mainhand:{id:\"minecraft:crossbow\",count:1}}",
        );
        srv.cmd_ok(&format!("summon {} {} {nbt}", e.kind, at(e.pos[0], e.pos[1], e.pos[2])))?;
    }
    Ok(())
}

/// The module placed with F3's scarers (the lead pair, the napper post with `post`) and `glow`
/// glowstone floors, the brain agent's lines drained: (the server, its agent reader, the bed).
pub(crate) fn start_world(version: &str, module: &std::path::Path, post: bool, glow: &[[i32; 3]]) -> Result<(VanillaServer, Agent, [i32; 3])> {
    start_world_part(version, module, post, glow, None)
}

/// As [`start_world`], with a part file laid over the module too.
pub(crate) fn start_world_part(version: &str, module: &std::path::Path, post: bool, glow: &[[i32; 3]], part: Option<&std::path::Path>) -> Result<(VanillaServer, Agent, [i32; 3])> {
    let (r, bed) = start(version, module)?;
    let Run { mut srv, agent, .. } = r;
    posts(&mut srv)?;
    if post {
        f3_post(&mut srv)?;
    }
    for f in glow {
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:glowstone", f[0], f[1] + DY, f[2]))?;
    }
    if let Some(p) = part {
        place_part(&mut srv, p)?;
    }
    Ok((srv, agent, bed))
}

fn wilson95(k: u32, n: u32) -> (f64, f64) {
    let (n, p, z) = (n.max(1) as f64, k as f64 / n.max(1) as f64, 1.96f64);
    let c = (p + z * z / (2.0 * n)) / (1.0 + z * z / n);
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / (1.0 + z * z / n);
    ((c - h).max(0.0), (c + h).min(1.0))
}

struct Run {
    srv: VanillaServer,
    agent: Agent,
    t: u32,
    panicking: bool,
    stamp: Option<u32>,
    verbose: bool,
    /// Every agent line with its tick, while collecting (a dwell's events)
    events: Option<Vec<(u32, String)>>,
}

impl Run {
    /// One tick; its agent lines logged (`verbose`), collected and folded into `c`.
    fn tick(&mut self, c: &mut Counts) -> Result<()> {
        self.srv.step(1)?;
        self.t += 1;
        for l in self.agent.drain(&self.srv)? {
            if l.starts_with("activity ") {
                self.panicking = l.contains("minecraft:panic") || l.starts_with("activity panic");
            }
            if l.starts_with("set minecraft:cant_reach_walk_target_since") {
                self.stamp = Some(self.t);
            } else if l.starts_with("erase minecraft:cant_reach_walk_target_since") && !l.contains("(absent)") {
                self.stamp = None;
            }
            c.add(&l);
            if self.verbose {
                println!("    t{} {l}", self.t);
            }
            if let Some(ev) = self.events.as_mut() {
                ev.push((self.t, l));
            }
        }
        if self.panicking {
            c.panic_ticks += 1;
        }
        Ok(())
    }

    fn ticks(&mut self, n: u32, c: &mut Counts) -> Result<()> {
        for _ in 0..n {
            self.tick(c)?;
        }
        Ok(())
    }
}

fn start(version: &str, module: &std::path::Path) -> Result<(Run, [i32; 3])> {
    start_placed(version, module, true)
}

fn start_placed(version: &str, module: &std::path::Path, strict_rails: bool) -> Result<(Run, [i32; 3])> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 18000")?;
    srv.cmd_ok("forceload add -16 -16 79 79")?;
    srv.step(20)?;
    let bed = place_module_with(&mut srv, module, strict_rails)?;
    // the away seat: an enclosed cart at (60, 0, 60), glass all round
    srv.cmd_ok(&format!("fill 58 {} 58 62 {} 62 minecraft:glass hollow", DY - 1, DY + 3))?;
    srv.cmd_ok(&format!("setblock 60 {DY} 60 minecraft:rail"))?;
    srv.cmd_ok(&format!("summon minecraft:minecart {} {{Tags:[\"away\"]}}", at(60.5, 0.0625, 60.5)))?;
    srv.step(5)?;
    let mut agent = Agent { at: 0 };
    agent.drain(&srv)?;
    Ok((Run { srv, agent, t: 0, panicking: false, stamp: None, verbose: true, events: None }, bed))
}

/// The module's scarers, seated in boats at their cells' centres: S1 the pillager (-4.5,-1,-4.5),
/// S2 the zombie (-4.5,-4,-1.5) (the lead posts) and the bed stop's zombie (-4.5,-18,3.5) (the
/// module file's -5.0 was the dropped +0.5; 58f770cc's fix). Held night: S2 burns by day under its
/// glass roof.
fn posts(srv: &mut VanillaServer) -> Result<()> {
    // S2's ruled glowstone roof (9fc3155f's sky check; night is held anyway). (ironspec 04:07: an
    // override must change its cell) A module that already has it (rev 3) isn't overridden: the
    // cell is tested first and laid only when absent, and a lay must change it
    let roof = format!("-5 {} -2", DY - 2);
    if srv.cmd(&format!("execute if block {roof} minecraft:glowstone"))?.contains("passed") {
        println!("  S2's glowstone roof (-5,-2,-2): already in the module (no override)");
    } else {
        let out = srv.cmd(&format!("setblock {roof} minecraft:glowstone"))?;
        anyhow::ensure!(out.contains("Changed the block"), "S2's glowstone roof override (-5,-2,-2) changed nothing ({}): the run fails", out.trim());
        println!("  S2's glowstone roof (-5,-2,-2): laid (override)");
    }
    srv.cmd_ok(&format!("summon minecraft:oak_boat {} {{Passengers:[{{id:\"minecraft:pillager\",PersistenceRequired:1b,Tags:[\"s1\"],equipment:{{mainhand:{{id:\"minecraft:crossbow\",count:1}}}}}}]}}", at(-4.5, -1.0, -4.5)))?;
    srv.cmd_ok(&format!("summon minecraft:oak_boat {} {{Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b,Tags:[\"s2\"]}}]}}", at(-4.5, -4.0, -1.5)))?;
    srv.cmd_ok(&format!("summon minecraft:oak_boat {} {{Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b,Tags:[\"bz\"]}}]}}", at(-4.5, -18.0, 3.5)))?;
    Ok(())
}

/// The scarers alive: (S1, S2, the bed stop's zombie).
fn scarers(srv: &mut VanillaServer) -> Result<(bool, bool, bool)> {
    Ok((srv.count_entities("@e[tag=s1]")? == 1, srv.count_entities("@e[tag=s2]")? == 1, srv.count_entities("@e[tag=bz]")? == 1))
}

fn move_rider(srv: &mut VanillaServer, to: [f64; 3], cart: &str) -> Result<()> {
    srv.cmd_ok("ride @e[tag=rider,limit=1] dismount")?;
    srv.cmd_ok(&format!("tp @e[tag=rider,limit=1] {}", at(to[0], to[1] + 0.2, to[2])))?;
    srv.cmd_ok(&format!("ride @e[tag=rider,limit=1] mount @e[tag={cart},limit=1]"))?;
    Ok(())
}

pub fn seat_clear(version: &str, module: &std::path::Path, rows: &str, lanes: &[(i32, u32, u32)], dwells: u32, preage: u32, depart: u32, drift: u32, floors: &[[i32; 3]], glass: &[[i32; 3]]) -> Result<()> {
    println!("== seat-clear on the official {version} server: onechunk's module ({}), a seated rider with HOME on its bed, night held (18,000)", module.display());
    let pocket = [-0.5, 0.0625, 0.5];
    let lane = [-1.5, 24.0625, 7.5];

    // (2) the calm control first: the lane seat, 2,400 gt
    if rows.contains('2') {
    println!("== (2) the calm control: the lane cell (-2,24,7), no scarer within reach, 2,400 gt");
    let (mut r, bed) = start(version, module)?;
    let dist = |s: [f64; 3]| ((s[0] - (bed[0] as f64 + 0.5)).powi(2) + (s[1] - bed[1] as f64).powi(2) + (s[2] - (bed[2] as f64 + 0.5)).powi(2)).sqrt();
    println!("  the bed head at ({},{},{}); from the pocket seat {:.1}, from the lane seat {:.1}", bed[0], bed[1], bed[2], dist(pocket), dist(lane));
    posts(&mut r.srv)?;
    println!("  scarers alive at the start (S1, S2, the bed stop's zombie): {:?}", scarers(&mut r.srv)?);
    rider_summon(&mut r.srv, lane, bed, "seat")?;
    let mut c2 = Counts::default();
    let mut lost = None;
    for _ in 0..2400 {
        r.tick(&mut c2)?;
        if lost.is_none() && c2.home_lost > 0 {
            lost = Some(r.t);
        }
    }
    println!(
        "  (2): stamp sets {}, erasures {} (reached {}, complete path {}, nap {}); HOME lost at {}; HOME at the end: {}; scarers alive at the end: {:?}",
        c2.stamp_set,
        c2.erased(),
        c2.erased_reached,
        c2.erased_path,
        c2.erased_nap,
        lost.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()),
        home(&mut r.srv)?,
        scarers(&mut r.srv)?
    );
    let _ = std::fs::remove_dir_all(&r.srv.dir);
    drop(r);
    }

    // (3) continuous panic with a pre-aged stamp
    if rows.contains('3') {
    println!(
        "== (3) continuous panic with a pre-aged stamp: a seat on glass over the void at (40,0,40) (no stable cell within reach: a cell over glass isn't a stable destination, so no stroll or flee draw lands), calm until its stamp is {preage} gt old, then a zombie seated in a boat 3 east of it until the stamp is 1,700 old, then the zombie removed"
    );
    let (mut r, bed) = start(version, module)?;
    r.srv.cmd_ok(&format!("setblock 40 {} 40 minecraft:glass", DY - 1))?;
    r.srv.cmd_ok(&format!("setblock 40 {DY} 40 minecraft:rail"))?;
    rider_summon(&mut r.srv, [40.5, 0.0625, 40.5], bed, "seat")?;
    let mut c3 = Counts::default();
    let mut guard = 0;
    while r.stamp.is_none_or(|s| r.t - s < preage) {
        r.tick(&mut c3)?;
        guard += 1;
        if guard > 3000 || c3.home_lost > 0 {
            bail!("seat-clear (3): the stamp never reached {preage} gt (HOME lost {})", c3.home_lost);
        }
    }
    let s0 = r.stamp.unwrap_or(0);
    println!("  t{}: the stamp is {} gt old; a zombie seated in a boat at (43.5,0,40.5), 3 east of the rider", r.t, r.t - s0);
    r.srv.cmd_ok(&format!("setblock 43 {} 40 minecraft:glass", DY - 1))?;
    r.srv.cmd_ok(&format!("summon minecraft:oak_boat {} {{Tags:[\"z3\"],Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b,Tags:[\"z3\"]}}]}}", at(43.5, 0.0, 40.5)))?;
    let mut panicked = 0u32;
    while r.stamp.is_some_and(|s| r.t - s < 1700) && c3.home_lost == 0 {
        r.tick(&mut c3)?;
        if r.panicking {
            panicked += 1;
        }
    }
    let kept = home(&mut r.srv)?;
    println!(
        "  t{}: stamp {}; panicking {} ({panicked} panicking ticks since the zombie came); HOME present {kept}",
        r.t,
        r.stamp.map(|s| format!("{} gt old", r.t - s)).unwrap_or_else(|| "erased".into()),
        r.panicking
    );
    r.srv.cmd_ok("kill @e[tag=z3]")?;
    let t_removed = r.t;
    let mut lost3 = None;
    for _ in 0..400 {
        r.tick(&mut c3)?;
        if c3.home_lost > 0 {
            lost3 = Some(r.t);
            break;
        }
    }
    println!(
        "  (3): HOME kept through the panic {kept}; the zombie removed at t{t_removed}; HOME lost {}",
        lost3.map(|t| format!("at t{t}, {} gt after", t - t_removed)).unwrap_or_else(|| "not within 400 gt".into())
    );
    let _ = std::fs::remove_dir_all(&r.srv.dir);
    drop(r);
    }

    // (1) pocket dwells with the lead posts, per lane
    for &(x, dwell, away) in lanes {
        if !rows.contains('1') {
            break;
        }
        pocket_lane(version, module, x, dwells, dwell, away, depart, drift, floors, glass)?;
    }
    Ok(())
}

/// The one-sided 95% Wilson upper bound of a proportion `k` of `n`.
fn upper95(k: u32, n: u32) -> f64 {
    let (n, p, z) = (n as f64, k as f64 / n.max(1) as f64, 1.645f64);
    (p + z * z / (2.0 * n) + z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / (1.0 + z * z / n)
}

/// The pocket cart's z (loop frame), while it departs.
fn pos_in_cart(srv: &mut VanillaServer) -> Result<Option<f64>> {
    let out = srv.cmd_long("data get entity @e[tag=pocket,limit=1] Pos[2]")?;
    Ok(out.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('d').parse().ok()))
}

/// One panic-stroll draw of a dwell: its tick (from the dwell's start), its branch ("land":
/// LandRandomPos; "bias": DefaultRandomPos.getPosTowards the village section), its landed cell
/// relative to the rider's block (None: a null draw), its outcome ("null", "reached", "complete",
/// "partial", "no path", "open"), how long its WALK_TARGET stayed set, whether it cleared the stamp.
pub(crate) struct Draw {
    pub(crate) t: u32,
    pub(crate) branch: &'static str,
    pub(crate) cell: Option<[i32; 3]>,
    pub(crate) outcome: &'static str,
    pub(crate) hold: Option<u32>,
    pub(crate) cleared: bool,
}

/// A dwell's draws from its agent lines (`t0`: the dwell's first tick less one).
fn draw_seq(ev: &[(u32, String)], t0: u32, origin: [i32; 3]) -> Vec<Draw> {
    draw_seq_moving(ev, t0, origin, &std::collections::BTreeMap::new())
}

/// As [`draw_seq`], each landed cell relative to the rider's block at its draw tick where `moving`
/// has one (the departure: the rider rolling out), else to `origin`.
pub(crate) fn draw_seq_moving(ev: &[(u32, String)], t0: u32, origin: [i32; 3], moving: &std::collections::BTreeMap<u32, [i32; 3]>) -> Vec<Draw> {
    let mut out: Vec<Draw> = Vec::new();
    let mut branch = "";
    let mut open: Option<usize> = None;
    // the draw closed this tick by MoveToTargetSink's WALK_TARGET erase: a reached target's stamp
    // erase comes right after it (checkExtraStartConditions erases WALK_TARGET first)
    let mut closed: Option<(usize, u32)> = None;
    for (t, l) in ev {
        let by = l.rsplit(" by ").next().unwrap_or("");
        if l.starts_with("erase minecraft:cant_reach_walk_target_since") && by.starts_with("MoveToTargetSink") && open.is_none() {
            if let Some((i, ct)) = closed {
                if ct == *t {
                    out[i].outcome = if by.starts_with("MoveToTargetSink.tryComputePath:142") { "complete" } else { "reached" };
                    out[i].cleared = !l.contains("(absent)");
                }
            }
            continue;
        }
        if l.starts_with("random pos ") {
            if l.contains("DefaultRandomPos.getPosTowards") {
                branch = "bias";
            } else if branch.is_empty() && l.contains("LandRandomPos") {
                branch = "land";
            }
        } else if l.starts_with("set minecraft:walk_target") && by.starts_with("VillageBoundRandomStroll") {
            let o = moving.get(t).copied().unwrap_or(origin);
            let cell = target_cell(l).map(|c| [c[0] - o[0], c[1] - o[1], c[2] - o[2]]);
            out.push(Draw { t: t - t0, branch: if branch.is_empty() { "?" } else { branch }, cell, outcome: "open", hold: None, cleared: false });
            open = Some(out.len() - 1);
            branch = "";
        } else if l.starts_with("erase minecraft:walk_target") && by.starts_with("VillageBoundRandomStroll") {
            out.push(Draw { t: t - t0, branch: if branch.is_empty() { "?" } else { branch }, cell: None, outcome: "null", hold: None, cleared: false });
            branch = "";
        } else if let Some(i) = open {
            let d = &mut out[i];
            if l.starts_with("erase minecraft:cant_reach_walk_target_since") && by.starts_with("MoveToTargetSink") {
                d.outcome = if by.starts_with("MoveToTargetSink.tryComputePath:142") { "complete" } else { "reached" };
                d.cleared = !l.contains("(absent)");
            } else if l.starts_with("set minecraft:path") && l.contains("reach false") && d.outcome == "open" {
                d.outcome = "partial";
            } else if l.starts_with("erase minecraft:walk_target") && by.starts_with("MoveToTargetSink") && !l.contains("(absent)") {
                d.hold = Some(t - t0 - d.t);
                if d.outcome == "open" {
                    d.outcome = "no path";
                }
                closed = Some((i, *t));
                open = None;
            }
        }
    }
    out
}

/// Row (1) for the pocket lane at `x` (its rest point (x+0.5, 0.0625, 0.5)): `dwells` dwells of
/// `dwell` gt at rest with the lead posts in place; then, with `depart` > 0, the pocket's support
/// powered so the cart rolls out on the module's rails for `depart` gt with the scarers present
/// (the real departure, pocket -> (x,0,1..6)); the rest of `away` gt seated in the enclosed away
/// cart, plus `drift` gt (1: the cycle advances against the 20-gt sensors by 1 gt a dwell, every
/// offset sampled); `floors`: glowstone floors laid first (9fc3155f's extra stroll cells).
fn pocket_lane(version: &str, module: &std::path::Path, x: i32, dwells: u32, dwell: u32, away: u32, depart: u32, drift: u32, floors: &[[i32; 3]], glass: &[[i32; 3]]) -> Result<()> {
    let pocket = [x as f64 + 0.5, 0.0625, 0.5];
    println!(
        "== (1) pocket lane x {x}: rest point ({}, 0.0625, 0.5), the lead posts; {dwells} dwells of {dwell} gt, {} {away} gt away between them, drift {drift}; extra glowstone floors {floors:?}",
        x as f64 + 0.5,
        if depart > 0 { format!("the real departure {depart} gt (within the turn),") } else { String::new() }
    );
    let (mut r, bed) = start(version, module)?;
    posts(&mut r.srv)?;
    for f in floors {
        r.srv.cmd_ok(&format!("setblock {} {} {} minecraft:glowstone", f[0], f[1] + DY, f[2]))?;
    }
    for f in glass {
        r.srv.cmd_ok(&format!("setblock {} {} {} minecraft:glass", f[0], f[1] + DY, f[2]))?;
    }
    r.srv.cmd_ok(&format!("summon minecraft:minecart {} {{Tags:[\"pocket\"]}}", at(pocket[0], pocket[1], pocket[2])))?;
    rider_summon(&mut r.srv, [60.5, 0.0625, 60.5], bed, "seat0")?;
    r.srv.cmd_ok("kill @e[tag=seat0]")?;
    r.srv.cmd_ok("ride @e[tag=rider,limit=1] mount @e[tag=away,limit=1]")?;
    r.verbose = false;
    // the lane's scarer: S2 (the zombie) for x -1, S1 (the pillager) for x 3 and 7 (9fc3155f's
    // cover); a dwell with it dead is invalid
    let s0 = scarers(&mut r.srv)?;
    println!("  scarers alive at the start (S1, S2, the bed stop's zombie): {s0:?}");
    let mut invalid = 0u32;
    let mut total = Counts::default();
    let (mut missed, mut lost1) = (0u32, None);
    let mut dist: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
    let (mut pmin, mut pmax) = (u32::MAX, 0u32);
    let mut first_erasure: Vec<u32> = Vec::new();
    let mut draws_min = u32::MAX;
    // by draw index before the first clear: (dwells reaching that draw, of which it cleared)
    let mut by_index: Vec<(u32, u32)> = Vec::new();
    // each landed target cell relative to the rider's block: (landed, reachable, cleared)
    let mut targets: std::collections::BTreeMap<[i32; 3], (u32, u32, u32)> = std::collections::BTreeMap::new();
    // the unreachable draws' WALK_TARGET holds: (outcome, gt) -> draws
    let mut holds: std::collections::BTreeMap<(&'static str, u32), u32> = std::collections::BTreeMap::new();
    // by the dwell's offset against the 20-gt sensors: (dwells, panic min, panic max, null draws,
    // draws, dwells without a clear)
    let mut by_offset: std::collections::BTreeMap<usize, (u32, u32, u32, u32, u32, u32)> = std::collections::BTreeMap::new();
    // each landed cell by branch and outcome
    let mut branches: std::collections::BTreeMap<([i32; 3], &'static str, &'static str), u32> = std::collections::BTreeMap::new();
    println!("  per dwell: k, panic ticks, stroll draws (all), landed, reachable (reached / complete path), the first landed draw (+gt), erased (cause, +gt), the first clearing draw's index, the landed targets relative to the rider's block (- unreached, + reachable, * cleared the stamp)");
    for k in 0..dwells {
        let (s1, s2, _) = scarers(&mut r.srv)?;
        if !(if x == -1 { s2 } else { s1 }) {
            invalid += 1;
        }
        move_rider(&mut r.srv, pocket, "pocket")?;
        let mut c = Counts { origin: [x, 0, 0], ..Counts::default() };
        let t0 = r.t;
        r.events = Some(Vec::new());
        let (mut first, mut first_landed, mut cause) = (None, None, "");
        let mut departed_clear = None;
        let mut panic_in_pocket = 0u32;
        let mut moving: std::collections::BTreeMap<u32, [i32; 3]> = std::collections::BTreeMap::new();
        for i in 0..dwell + depart {
            if depart > 0 && i == dwell {
                // the real departure: the pocket's support powered, the cart launched south
                r.srv.cmd_ok(&format!("setblock {x} {} 0 minecraft:redstone_block", DY - 1))?;
            }
            let (e0, l0, p0) = (c.erased(), c.stroll, c.erased_path);
            r.tick(&mut c)?;
            if r.panicking && i < dwell {
                panic_in_pocket += 1;
            }
            if i >= dwell {
                // the departure: the rider's block each tick, for its draws' cells
                if let Some(p) = rider_pos(&mut r.srv)? {
                    moving.insert(r.t, [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
                }
            }
            if first.is_none() && c.erased() > e0 {
                first = Some(r.t - t0);
                cause = if c.erased_path > p0 { "complete path" } else if c.erased_nap > 0 { "nap" } else { "reached" };
                if i >= dwell {
                    departed_clear = first;
                }
            }
            if first_landed.is_none() && c.stroll > l0 {
                first_landed = Some(r.t - t0);
            }
        }
        let ev = r.events.take().unwrap_or_default();
        let seq = draw_seq_moving(&ev, t0, [x, 0, 0], &moving);
        // the panic onset, and what held WALK_TARGET then (a REST bed walk on a partial path keeps
        // it while MoveToTargetSink runs, so no stroll can draw until it's released)
        let onset = ev.iter().find(|(_, l)| l.starts_with("activity panic")).map(|(t, _)| *t);
        let onset_note = match onset {
            None => "no panic onset in the dwell".to_string(),
            Some(o) => {
                let mut held: Option<(u32, String)> = None;
                let mut path: Option<String> = None;
                for (t, l) in ev.iter().filter(|(t, _)| *t < o) {
                    if l.starts_with("set minecraft:walk_target") {
                        held = Some((*t, l.trim_start_matches("set minecraft:walk_target = ").to_string()));
                    } else if l.starts_with("erase minecraft:walk_target") && !l.contains("(absent)") {
                        held = None;
                    } else if l.starts_with("set minecraft:path") {
                        path = Some(l.split(" = ").nth(1).unwrap_or("").split(" by ").next().unwrap_or("").to_string());
                    } else if l.starts_with("erase minecraft:path") && !l.contains("(absent)") {
                        path = None;
                    }
                }
                let release = ev.iter().find(|(t, l)| *t >= o && l.starts_with("erase minecraft:walk_target") && !l.contains("(absent)"));
                let first_draw = seq.first().map(|d| d.t);
                match held {
                    None => format!("onset +{}; WALK_TARGET free at onset; the first stroll draw {}", o - t0, first_draw.map(|d| format!("+{d}")).unwrap_or_else(|| "none".into())),
                    Some((ht, what)) => format!(
                        "onset +{}; WALK_TARGET held at onset since +{} ({}; path {}); released {}; the first stroll draw {}",
                        o - t0,
                        ht - t0,
                        what,
                        path.unwrap_or_else(|| "none".into()),
                        release.map(|(t, l)| format!("+{} ({} gt after onset, by {})", t - t0, t - o, l.rsplit(" by ").next().unwrap_or(""))).unwrap_or_else(|| "not in the dwell".into()),
                        first_draw.map(|d| format!("+{d}")).unwrap_or_else(|| "none".into())
                    ),
                }
            }
        };
        let cart_z = if depart > 0 { pos_in_cart(&mut r.srv)? } else { None };
        if let Some(f) = first {
            first_erasure.push(f);
        }
        if c.erased() == 0 {
            missed += 1;
        }
        *dist.entry(c.erased()).or_default() += 1;
        pmin = pmin.min(c.panic_ticks);
        pmax = pmax.max(c.panic_ticks);
        draws_min = draws_min.min(c.stroll + c.stroll_null);
        // the panic window and the draws by the dwell's offset against the 20-gt sensors
        let off = ((k * (dwell + away + drift)) % 20) as usize;
        let e = by_offset.entry(off).or_insert((0u32, u32::MAX, 0u32, 0u32, 0u32, 0u32));
        e.0 += 1;
        e.1 = e.1.min(panic_in_pocket);
        e.2 = e.2.max(panic_in_pocket);
        e.3 += c.stroll_null;
        e.4 += c.stroll + c.stroll_null;
        if c.erased() == 0 {
            e.5 += 1;
        }
        // the draws before the first clearing one, and the pooled clear rate by draw index
        let clear_at = c.seq.iter().position(|d| d.2);
        let before = &c.seq[..clear_at.unwrap_or(c.seq.len())];
        let (nulls, unreached) = (before.iter().filter(|d| d.0.is_none()).count(), before.iter().filter(|d| d.0.is_some()).count());
        for (i, d) in c.seq.iter().enumerate().take(clear_at.map(|i| i + 1).unwrap_or(c.seq.len())) {
            if by_index.len() <= i {
                by_index.resize(i + 1, (0, 0));
            }
            by_index[i].0 += 1;
            if d.2 {
                by_index[i].1 += 1;
            }
        }
        for d in &c.seq {
            if let Some(rel) = d.0 {
                let e = targets.entry(rel).or_insert((0, 0, 0));
                e.0 += 1;
                if !d.1.is_empty() {
                    e.1 += 1;
                }
                if d.2 {
                    e.2 += 1;
                }
            }
        }
        println!(
            "  dwell {k}: {} panic ({panic_in_pocket} in the pocket), {} draws, {} landed, {} reachable ({} / {}), first landed {}, erased {}; the first clearing draw {} (before it {nulls} null, {unreached} landed unreached){}",
            c.panic_ticks,
            c.stroll + c.stroll_null,
            c.stroll,
            c.reach_reached + c.reach_path,
            c.reach_reached,
            c.reach_path,
            first_landed.map(|f| format!("+{f}")).unwrap_or_else(|| "none".into()),
            first.map(|f| format!("yes ({cause}, +{f}{})", if departed_clear.is_some() { ", on the departure" } else { "" })).unwrap_or_else(|| "NO".into()),
            clear_at.map(|i| format!("#{}", i + 1)).unwrap_or_else(|| "none".into()),
            cart_z.map(|z| format!("; the departing cart at z {z:.2} after {depart} gt")).unwrap_or_default()
        );
        // the draw sequence: +tick, n (null) or branch(cell)outcome[hold], * cleared the stamp
        println!("    onset {k}: {onset_note}");
        println!(
            "    seq {k}: {}",
            seq.iter()
                .map(|d| match d.cell {
                    None => format!("+{} n", d.t),
                    Some(c) => format!(
                        "+{} {}({},{},{}){}{}{}",
                        d.t,
                        if d.branch == "bias" { "b" } else if d.branch == "land" { "l" } else { "?" },
                        c[0],
                        c[1],
                        c[2],
                        match d.outcome {
                            "reached" => "R",
                            "complete" => "C",
                            "partial" => "P",
                            "no path" => "N",
                            _ => "O",
                        },
                        d.hold.map(|h| h.to_string()).unwrap_or_default(),
                        if d.cleared { "*" } else { "" }
                    ),
                })
                .collect::<Vec<_>>()
                .join(" ")
        );
        for d in &seq {
            if let (Some(c), Some(h)) = (d.cell, d.hold) {
                if d.outcome == "partial" || d.outcome == "no path" {
                    *holds.entry((d.outcome, h)).or_default() += 1;
                }
                *branches.entry((c, d.branch, d.outcome)).or_default() += 1;
            } else if let Some(c) = d.cell {
                *branches.entry((c, d.branch, d.outcome)).or_default() += 1;
            }
        }
        if depart > 0 {
            // the departed cart and its support reset; a fresh cart at rest in the pocket
            r.srv.cmd_ok("ride @e[tag=rider,limit=1] dismount")?;
            r.srv.cmd_ok("kill @e[tag=pocket]")?;
            r.srv.cmd_ok(&format!("setblock {x} {} 0 minecraft:stone", DY - 1))?;
            r.srv.cmd_ok(&format!("summon minecraft:minecart {} {{Tags:[\"pocket\"]}}", at(pocket[0], pocket[1], pocket[2])))?;
        }
        let mut a = Counts::default();
        move_rider(&mut r.srv, [60.5, 0.0625, 60.5], "away")?;
        r.ticks(away.saturating_sub(depart) + drift, &mut a)?;
        total.merge(&c);
        if lost1.is_none() && (c.home_lost > 0 || a.home_lost > 0) {
            lost1 = Some(k);
        }
        total.home_lost += a.home_lost;
    }
    let draws = total.stroll + total.stroll_null;
    let reach = total.reach_reached + total.reach_path;
    let (ql, qh) = wilson95(reach, draws);
    let (ll, lh) = wilson95(total.stroll, draws);
    println!(
        "  lane x {x} totals: draws {draws}, landed {} ({:.4}, 95% {:.4}..{:.4}), reachable {reach} (q = {:.4}, 95% {ql:.4}..{qh:.4}: reached {}, complete path {}); the non-clear rate (null and landed unreached) {:.4}; the fewest draws in a dwell n_min = {draws_min}; (1 - q_low)^n_min = {:.3e}",
        total.stroll,
        total.stroll as f64 / draws.max(1) as f64,
        ll,
        lh,
        reach as f64 / draws.max(1) as f64,
        total.reach_reached,
        total.reach_path,
        (draws - reach) as f64 / draws.max(1) as f64,
        (1.0 - ql).powi(draws_min as i32)
    );
    println!(
        "  lane x {x}, the clear rate by draw index before the first clear (dwells that reached draw i: cleared there): {}",
        by_index.iter().enumerate().map(|(i, (n, c))| format!("#{}: {c}/{n}", i + 1)).collect::<Vec<_>>().join(", ")
    );
    println!(
        "  lane x {x}, the landed targets relative to the rider's block (landed / reachable / cleared): {}",
        targets.iter().map(|(r, (l, re, c))| format!("({},{},{}) {l}/{re}/{c}", r[0], r[1], r[2])).collect::<Vec<_>>().join(", ")
    );
    println!(
        "  lane x {x}, the landed cells by branch and outcome (b: DefaultRandomPos.getPosTowards the village section, l: LandRandomPos): {}",
        branches.iter().map(|((c, b, o), n)| format!("({},{},{}) {b} {o}: {n}", c[0], c[1], c[2])).collect::<Vec<_>>().join(", ")
    );
    println!(
        "  lane x {x}, the unreachable draws' WALK_TARGET holds (outcome, gt: draws): {}",
        holds.iter().map(|((o, h), n)| format!("{o} {h}: {n}")).collect::<Vec<_>>().join(", ")
    );
    first_erasure.sort();
    let med = first_erasure.get(first_erasure.len() / 2).copied().unwrap_or(0);
    println!(
        "  lane x {x}: {dwells} dwells of {dwell} gt; erasures per dwell {}; the per-dwell miss rate {missed}/{dwells} = {:.3} (one-sided 95% upper bound {:.4}); the first erasure in a dwell at +{}..+{} gt (median +{med}); panic ticks per dwell {:.1} ({pmin}..{pmax}); stroll draws in the dwells: {} landed, {} null ({:.3} a panicking tick); flee draws {} (null {}); erasures reached {} / complete path {} / nap {}; HOME lost {}",
        dist.iter().map(|(e, n)| format!("{e}: {n}")).collect::<Vec<_>>().join(", "),
        missed as f64 / dwells as f64,
        upper95(missed, dwells),
        first_erasure.first().copied().unwrap_or(0),
        first_erasure.last().copied().unwrap_or(0),
        total.panic_ticks as f64 / dwells as f64,
        total.stroll,
        total.stroll_null,
        (total.stroll + total.stroll_null) as f64 / total.panic_ticks.max(1) as f64,
        total.flee,
        total.flee_null,
        total.erased_reached,
        total.erased_path,
        total.erased_nap,
        lost1.map(|k| format!("in dwell {k}'s turn")).unwrap_or_else(|| "never".into())
    );
    let nulls_all = total.stroll_null;
    let draws_all = total.stroll + total.stroll_null;
    let (pl, ph) = wilson95(nulls_all, draws_all);
    println!(
        "  lane x {x}: P_null per draw (a tick's stroll draw, no walk target held, finding no candidate) = {nulls_all}/{draws_all} = {:.4} (95% {pl:.4}..{ph:.4}); per offset against the 20-gt sensors (offset: dwells, panic window min..max, P_null, dwells without a clear): {}",
        nulls_all as f64 / draws_all.max(1) as f64,
        by_offset
            .iter()
            .map(|(o, (n, mn, mx, nu, dr, miss))| format!("{o}: {n}, {mn}..{mx}, {:.3}, {miss}", *nu as f64 / (*dr).max(1) as f64))
            .collect::<Vec<_>>()
            .join("; ")
    );
    println!("  lane x {x}: scarers alive at the end (S1, S2, the bed stop's zombie): {:?}; dwells with the lane's scarer dead (invalid): {invalid}", scarers(&mut r.srv)?);
    let _ = std::fs::remove_dir_all(&r.srv.dir);
    Ok(())
}

/// The rider's position (loop frame).
fn rider_pos(srv: &mut VanillaServer) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long("data get entity @e[tag=rider,limit=1] Pos")?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1] - DY as f64, v[2]]))
}

/// (tickexact 2026-11-12, ironspec's F3) F3's napper post: a pillager seated in a boat at
/// (1.5,24,-4.5) in a glass box open south (9fc3155f's cells: floor (1,23,-5), roof (1,26,-5),
/// walls (1,24..25,-6), (2,24..25,-5), (0,24..25,-5)).
fn f3_post(srv: &mut VanillaServer) -> Result<()> {
    let g = |srv: &mut VanillaServer, x: i32, y: i32, z: i32| srv.cmd_ok(&format!("setblock {x} {} {z} minecraft:glass", y + DY)).map(|_| ());
    g(srv, 1, 23, -5)?;
    g(srv, 1, 26, -5)?;
    for y in 24..=25 {
        g(srv, 1, y, -6)?;
        g(srv, 2, y, -5)?;
        g(srv, 0, y, -5)?;
    }
    srv.cmd_ok(&format!("summon minecraft:oak_boat {} {{Passengers:[{{id:\"minecraft:pillager\",PersistenceRequired:1b,Tags:[\"f3\"],equipment:{{mainhand:{{id:\"minecraft:crossbow\",count:1}}}}}}]}}", at(1.5, 24.0, -4.5)))?;
    Ok(())
}

/// (tickexact 2026-11-12, ironspec's F3 inputs) A seat measured with the design's scarers (the
/// lead pair and, with `post`, F3's napper post) and `glow` glowstone floors: a rider at rest in a
/// cart at `seat` (the lane's pressed queue filled first by empty carts at `fill`, from the front),
/// `rail_glow`: the seat rail's floor glowstone. Until `draws` stroll draws: P_null, q, the landed
/// cells by branch and outcome, MoveToTargetSink's state at the panic onset, the pillagers'
/// charged ticks.
pub fn f3_seat(version: &str, module: &std::path::Path, name: &str, seat: [i32; 3], fill: &[[i32; 3]], post: bool, glow: &[[i32; 3]], rail_glow: bool, draws: u32) -> Result<()> {
    f3_seat_parts(version, module, name, seat, fill, post, glow, rail_glow, draws, &[], None, &[], &SeatExtra::default())
}

/// (tickexact 2026-11-12, b8949c96's queue post) More for a seat run: entities summoned after the
/// parts ("kind x y z {nbt}", y in the design's frame), a point whose nearest zombie's distance to
/// the rider is printed (the post), and the run's tick cap.
pub struct SeatExtra {
    pub summon: Vec<String>,
    pub watch: Option<[f64; 3]>,
    pub max_ticks: u32,
}

impl Default for SeatExtra {
    fn default() -> Self {
        SeatExtra { summon: Vec::new(), watch: None, max_ticks: 6000 }
    }
}

/// The zombie nearest `p` and the seat rider: both positions and their distance (feet to feet).
fn post_distance(srv: &mut VanillaServer, p: [f64; 3]) -> Result<String> {
    let z = ent_pos(srv, &format!("@e[type=minecraft:zombie,sort=nearest,limit=1,x={},y={},z={}]", p[0], p[1] + DY as f64, p[2]))?;
    let r = rider_pos(srv)?;
    Ok(match (r, z) {
        (Some(r), Some(z)) => {
            let d = ((r[0] - z[0]).powi(2) + (r[1] - z[1]).powi(2) + (r[2] - z[2]).powi(2)).sqrt();
            format!("the rider ({:.4},{:.4},{:.4}), the zombie ({:.4},{:.4},{:.4}): distance {d:.4}", r[0], r[1], r[2], z[0], z[1], z[2])
        }
        _ => format!("the rider {r:?}, the zombie {z:?}"),
    })
}

/// As [`f3_seat`], with part files laid over the module in order (blocks as stated, entities
/// summoned: [`place_part`]), `air` cells cleared after them, and the rider's cart at `cart` (else the
/// seat cell's centre 0.5 up); each file's cells checked against the world after everything is
/// placed (module-check).
#[allow(clippy::too_many_arguments)]
pub fn f3_seat_parts(version: &str, module: &std::path::Path, name: &str, seat: [i32; 3], fill: &[[i32; 3]], post: bool, glow: &[[i32; 3]], rail_glow: bool, draws: u32, parts: &[std::path::PathBuf], cart: Option<[f64; 3]>, air: &[[i32; 3]], extra: &SeatExtra) -> Result<()> {
    println!(
        "== seat {name} ({},{},{}){}: the lead pair{}, {} glowstone floors{}; the queue filled at {fill:?}",
        seat[0],
        seat[1],
        seat[2],
        if rail_glow { ", its rail's floor glowstone" } else { "" },
        if post { " and F3's napper post (1,24,-5)" } else { "" },
        glow.len(),
        if parts.is_empty() { String::new() } else { format!("; parts laid over the module: {}", parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")) }
    );
    let (mut r, bed) = start(version, module)?;
    posts(&mut r.srv)?;
    if post {
        f3_post(&mut r.srv)?;
    }
    for p in parts {
        place_part(&mut r.srv, p)?;
    }
    for a in air {
        r.srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", a[0], a[1] + DY, a[2]))?;
    }
    if !air.is_empty() {
        println!("  cleared to air after the parts: {air:?}");
    }
    for f in glow {
        r.srv.cmd_ok(&format!("setblock {} {} {} minecraft:glowstone", f[0], f[1] + DY, f[2]))?;
    }
    for s in &extra.summon {
        let w: Vec<&str> = s.trim().splitn(5, ' ').collect();
        anyhow::ensure!(w.len() >= 4, "--summon \"kind x y z [nbt]\", got {s}");
        let (x, y, z): (f64, f64, f64) = (w[1].parse()?, w[2].parse()?, w[3].parse()?);
        r.srv.cmd_ok(&format!("summon {} {} {}", w[0], at(x, y, z), w.get(4).copied().unwrap_or("")))?;
        println!("  summoned: {s}");
    }
    if !parts.is_empty() {
        r.srv.step(20)?;
        // module-check, file by file: a cell a later file re-lays is the later file's
        let files: Vec<&std::path::Path> = std::iter::once(module).chain(parts.iter().map(|p| p.as_path())).collect();
        let designs: Vec<mctas::design::Design> = files.iter().map(|f| mctas::design::Design::load(f)).collect::<Result<_>>()?;
        for (i, f) in files.iter().enumerate() {
            let m = module_mismatches(&mut r.srv, f)?;
            let (later, own): (Vec<&String>, Vec<&String>) = m.iter().partition(|s| {
                let cell: Vec<i32> = s.trim_start_matches('(').split(')').next().unwrap_or("").split(',').filter_map(|v| v.parse().ok()).collect();
                cell.len() == 3 && designs[i + 1..].iter().any(|d| d.blocks.contains_key(&[cell[0], cell[1], cell[2]]))
            });
            println!(
                "  module-check {}: {} cells differ from the file, {} of them re-laid by a later file; the rest: {}",
                f.display(),
                m.len(),
                later.len(),
                if own.is_empty() { "none".to_string() } else { own.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ") }
            );
        }
    }
    if rail_glow {
        r.srv.cmd_ok(&format!("setblock {} {} {} minecraft:glowstone", seat[0], seat[1] - 1 + DY, seat[2]))?;
    }
    let floor = ["minecraft:glass", "minecraft:glowstone", "minecraft:stone", "minecraft:cobblestone"]
        .into_iter()
        .find(|b| r.srv.test_block([seat[0], seat[1] - 1 + DY, seat[2]], b).unwrap_or(false))
        .unwrap_or("other");
    println!("  the seat rail's floor: {floor}");
    for f in fill {
        r.srv.cmd_ok(&format!("summon minecraft:minecart {} {{Tags:[\"queue\"]}}", at(f[0] as f64 + 0.5, f[1] as f64 + 0.5, f[2] as f64 + 0.5)))?;
        r.srv.step(10)?;
    }
    rider_summon(&mut r.srv, cart.unwrap_or([seat[0] as f64 + 0.5, seat[1] as f64 + 0.5, seat[2] as f64 + 0.5]), bed, "seat")?;
    r.srv.step(40)?;
    // every cart of the queue after settling (where a pressed queue really rests)
    let carts = r.srv.cmd_long(&format!("execute as @e[type=minecraft:minecart,x={},y={},z=-2,dx=1,dy=10,dz=12] run data get entity @s Pos", seat[0], 15 + DY))?;
    let mut ps: Vec<String> = carts
        .split("entity data: ")
        .skip(1)
        .filter_map(|s| {
            let v: Vec<f64> = s.split(']').next()?.trim_start_matches('[').split(',').filter_map(|t| t.trim().trim_end_matches('d').parse().ok()).collect();
            (v.len() == 3).then(|| format!("({:.2},{:.2},{:.2})", v[0], v[1] - DY as f64, v[2]))
        })
        .collect();
    ps.sort();
    println!("  the queue's carts at rest (the rider's included): {}", ps.join(" "));
    if let Some(p) = extra.watch {
        println!("  {name} at the start: {}", post_distance(&mut r.srv, p)?);
    }
    r.verbose = false;
    let mut c = Counts { origin: seat, ..Counts::default() };
    r.events = Some(Vec::new());
    let t0 = r.t;
    let (mut guard, mut charged) = (0u32, 0u32);
    while c.stroll + c.stroll_null < draws && guard < extra.max_ticks {
        r.tick(&mut c)?;
        guard += 1;
        for tag in ["s1", "f3"] {
            if r.srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] equipment.mainhand"))?.contains("charged_projectiles") {
                charged += 1;
            }
        }
    }
    let ev = r.events.take().unwrap_or_default();
    let seq = draw_seq(&ev, t0, seat);
    let onsets: Vec<String> = ev.iter().filter(|(_, l)| l.starts_with("activity panic")).map(|(t, l)| format!("+{} {}", t - t0, l.split(" (").nth(1).map(|s| format!("({s}")).unwrap_or_default())).collect();
    let n = c.stroll + c.stroll_null;
    let reach = c.reach_reached + c.reach_path;
    let (pl, ph) = wilson95(c.stroll_null, n);
    let (ql, qh) = wilson95(reach, n);
    let mut cells: std::collections::BTreeMap<([i32; 3], &'static str, &'static str), u32> = std::collections::BTreeMap::new();
    for d in &seq {
        if let Some(cl) = d.cell {
            *cells.entry((cl, d.branch, d.outcome)).or_default() += 1;
        }
    }
    println!("  {name}: the rider at {:?} after {guard} gt; panic onsets {}", rider_pos(&mut r.srv)?, onsets.join("; "));
    if let Some(p) = extra.watch {
        println!("  {name} at the end: {}; flee targets (SetWalkTargetAwayFrom: set {}, erased {})", post_distance(&mut r.srv, p)?, c.flee, c.flee_null);
    }
    println!(
        "  {name}: {n} draws over {guard} gt ({} panic ticks): P_null {}/{n} = {:.4} (95% {pl:.4}..{ph:.4}); q = {reach}/{n} = {:.4} (95% {ql:.4}..{qh:.4}: reached {}, complete path {}); landed cells relative to the seat (cell, branch, outcome: draws): {}; pillager ticks charged (S1, F3's): {charged}",
        c.panic_ticks,
        c.stroll_null,
        c.stroll_null as f64 / n.max(1) as f64,
        reach as f64 / n.max(1) as f64,
        c.reach_reached,
        c.reach_path,
        cells.iter().map(|((cl, b, o), k)| format!("({},{},{}) {b} {o}: {k}", cl[0], cl[1], cl[2])).collect::<Vec<_>>().join(", ")
    );
    let _ = std::fs::remove_dir_all(&r.srv.dir);
    Ok(())
}

/// An entity's raw position (y less `DY`) by selector.
fn ent_pos(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1] - DY as f64, v[2]]))
}

/// (tickexact 2026-11-12, ironspec: the queue geometry the statics use) `mctas mobai queue-fill`:
/// df472311b's fill procedure on the strict-placed module (the top slope as built): carts summoned
/// at each fill cell's centre 0.5 up, (3,17,1) to (3,21,5), 10 gt apart (empty, or each with a
/// villager when `ridden`), then a ridden cart at the top cell (3,22,6), then 40 gt. Every tick: R_1's
/// powered state, H's trapdoor, every cart's position and its rider's; at the end the resting
/// pitch between neighbours.
pub fn queue_fill(version: &str, module: &std::path::Path, ridden: bool) -> Result<()> {
    let (mut r, _bed) = start(version, module)?;
    let m = module_mismatches(&mut r.srv, module)?;
    println!(
        "== queue-fill on the official {version} server ({}; module-check: {} blocks differ from the file{})",
        if ridden { "every cart ridden" } else { "empty carts, the last ridden (df472311b's procedure)" },
        m.len(),
        if m.iter().any(|s| s.contains("rail")) { ", rails among them" } else { ", no rail" }
    );
    let fill: [[i32; 3]; 6] = [[3, 17, 1], [3, 18, 2], [3, 19, 3], [3, 20, 4], [3, 21, 5], [3, 22, 6]];
    let mut t = 0u32;
    for (i, f) in fill.iter().enumerate() {
        let last = i == fill.len() - 1;
        let pass = if ridden || last { format!(",Passengers:[{{id:\"minecraft:villager\",Tags:[\"qv{i}\"],NoAI:1b}}]") } else { String::new() };
        r.srv.cmd_ok(&format!("summon minecraft:minecart {} {{Tags:[\"q{i}\"]{pass}}}", at(f[0] as f64 + 0.5, f[1] as f64 + 0.5, f[2] as f64 + 0.5)))?;
        for _ in 0..(if last { 40 } else { 10 }) {
            r.srv.step(1)?;
            t += 1;
            let r1 = r.srv.cmd_long(&format!("execute if block 3 {} 1 minecraft:powered_rail[powered=true]", 17 + DY))?.contains("passed");
            let shut = r.srv.cmd_long(&format!("execute if block 3 {} 0 minecraft:oak_trapdoor[open=false]", 16 + DY))?.contains("passed");
            let mut row = Vec::new();
            for k in 0..=i {
                let c = ent_pos(&mut r.srv, &format!("@e[tag=q{k},limit=1]"))?;
                let v = ent_pos(&mut r.srv, &format!("@e[tag=qv{k},limit=1]"))?;
                row.push(match (c, v) {
                    (Some(c), Some(v)) => format!("c{k} ({:.4},{:.4},{:.4}) rider {:+.4}", c[0], c[1], c[2], v[1] - c[1]),
                    (Some(c), None) => format!("c{k} ({:.4},{:.4},{:.4})", c[0], c[1], c[2]),
                    _ => format!("c{k} gone"),
                });
            }
            println!("  t{t} R_1 {} H {}: {}", if r1 { "POWERED" } else { "unpowered" }, if shut { "shut" } else { "OPEN" }, row.join("; "));
        }
    }
    let mut ps: Vec<[f64; 3]> = Vec::new();
    for k in 0..fill.len() {
        if let Some(c) = ent_pos(&mut r.srv, &format!("@e[tag=q{k},limit=1]"))? {
            ps.push(c);
        }
    }
    ps.sort_by(|a, b| a[2].total_cmp(&b[2]));
    let pitch: Vec<String> = ps.windows(2).map(|w| format!("({:+.3},{:+.3})", w[1][1] - w[0][1], w[1][2] - w[0][2])).collect();
    println!("  at rest, by z: {}; the pitch (dy, dz) between neighbours: {}", ps.iter().map(|p| format!("({:.3},{:.3})", p[1], p[2])).collect::<Vec<_>>().join(" "), pitch.join(" "));
    let _ = std::fs::remove_dir_all(&r.srv.dir);
    Ok(())
}
