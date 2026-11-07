//! `mctas bedrock video`: the nether-portal bedrock tutorial, filmed in a real random world.
//!
//! Phases (`--phase scout|record|compose|all|smoke`):
//!
//! * `scout`: an official server with a default-generated world of the seed; the Nether floor
//!   around the origin is force-loaded, saved and read back from its region files, and the
//!   column needing the fewest portal trips (NOTES.md's stepped pit) with a diggable entry
//!   shaft and a dry overworld counterpart is chosen. Writes `plan.json` and the pristine
//!   generated world (`world-src/`, chunks pre-generated, never edited).
//! * `record`: the same world, real time, survival. The player is the real client (Fabric +
//!   the actor mod of `bridge/actor`, or the actor launcher) driven through the whole recipe:
//!   dig to the floor, prepare the patch, build a return portal, dig the overworld drill room,
//!   then convert row after row (blockers chosen with the [`crate::portal`] predictor, every
//!   conversion asserted over RCON) down to a hole at y 0. Commands give the starting kit and
//!   skip the long overworld walk between the return twin and the drill portal (captioned);
//!   they never place or break a block at the patch or the portals. Raw segments + a caption
//!   timeline land in `segments/`; world checkpoints (`checkpoints/<tag>/`) + `state.json`
//!   make the shoot resumable with `--from <tag>`.
//! * `compose`: cards and caption overlays drawn with `mctas::mcfont`, segments cut at their
//!   speeds, VP9 WebM out (`bedrock-portal-method.webm`), stills in `stills/` for checking.
//! * `smoke`: a throwaway world where the actor just mines, places and walks on the surface.

use crate::portal::{self, Blocks, Kind, Pos};
use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const PLAYER: &str = "mctas";
const SLOT_PICK: u32 = 0;
const SLOT_OBSIDIAN: u32 = 1;
const SLOT_FLINT: u32 = 2;
const SLOT_NETHERRACK: u32 = 3;
const SLOT_COBBLE: u32 = 4;
const SLOT_TORCH: u32 = 5;

#[derive(Clone, Debug)]
pub struct VideoOptions {
    pub seed: Option<i64>,
    pub out: PathBuf,
    pub phase: String,
    /// `fabric` (the actor mod under Fabric Loader) or `launcher` (the same driver, no loader).
    pub driver: String,
    pub from: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// The scouted geometry (see `scout`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub seed: i64,
    /// Nether: the frame's west x and center row z; frames occupy x fx..fx+3.
    pub fx: i32,
    pub rz: i32,
    /// Rows (z) holding bedrock per layer, layers[0] is y 0.
    pub layers: Vec<Vec<i32>>,
    pub trips: usize,
    /// The entry shaft column and the standable feet y the player is put on.
    pub shaft_x: i32,
    pub shaft_z: i32,
    pub shaft_stand_y: i32,
    /// Overworld: drill portal frame west x / plane z, and the dig-down surface stand.
    pub ox: i32,
    pub oz: i32,
    pub ow_stand_y: i32,
    /// Nether return portal frame west x (plane z = rz).
    pub rx: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Segment {
    file: String,
    /// The checkpoint tag that owns this segment (dropped on resume when its scene is redone).
    scene: String,
    caption: Vec<String>,
    speed: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Timeline {
    segments: Vec<Segment>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct State {
    done: Vec<String>,
    /// Which bedrock layer the overworld drill portal's height currently serves (4 = as built).
    drill_serves: Option<i32>,
    /// Cobblestone blockers currently placed at the patch.
    blockers: Vec<Pos>,
    /// Feet position inside the return portal's overworld twin (to walk back through it).
    twin: Option<[f64; 3]>,
}

pub fn run(version: &str, o: VideoOptions) -> Result<()> {
    std::fs::create_dir_all(&o.out)?;
    match o.phase.as_str() {
        "smoke" => smoke(version, &o),
        "inspect" => {
            // Print the saved blocks around the patch floor of a world folder (no server).
            let plan = load_plan(&o)?;
            let world = match &o.from {
                Some(tag) => o.out.join("checkpoints").join(tag),
                None => o.out.join("world-src"),
            };
            let dim = world.join("dimensions").join("minecraft").join("the_nether");
            let dim = if dim.exists() { dim } else { world.clone() };
            // The drill room too: every non-air cell (what a clear left behind).
            let ow = world.join("dimensions").join("minecraft").join("overworld");
            let ow = if ow.exists() { ow } else { world.clone() };
            if let Ok(snap) = mctas::vanilla::read_box(&ow, [plan.ox - 2, 3, plan.oz - 3], [plan.ox + 5, 10, plan.oz + 3]) {
                let b = Blocks::new(snap.region);
                println!("overworld drill room non-air:");
                for y in 3..=10 {
                    for z in (plan.oz - 3)..=(plan.oz + 3) {
                        for x in (plan.ox - 2)..=(plan.ox + 5) {
                            let s = b.get([x, y, z]);
                            if b.kind([x, y, z]) != Kind::Replaceable {
                                println!("  [{x},{y},{z}] {s}");
                            }
                        }
                    }
                }
            }
            let (ix0, ix1) = (plan.fx - 36, plan.fx + 12);
            let snap = mctas::vanilla::read_box(&dim, [ix0, 0, plan.rz - 6], [ix1, 9, plan.rz + 6])?;
            let b = Blocks::new(snap.region);
            for y in [0, 4, 5] {
                println!("y={y} (rows z, cols x {ix0}..{ix1}; # solid, . replaceable, ? other, L lava, B bedrock)");
                for z in (plan.rz - 6)..=(plan.rz + 6) {
                    let mut line = String::new();
                    for x in ix0..=ix1 {
                        let s = b.get([x, y, z]);
                        line.push(if s.starts_with("minecraft:bedrock") {
                            'B'
                        } else if s.starts_with("minecraft:lava") {
                            'L'
                        } else {
                            match b.kind([x, y, z]) {
                                Kind::Solid => '#',
                                Kind::Replaceable => '.',
                                Kind::Other => '?',
                            }
                        });
                    }
                    println!("z={z:3} {line}");
                }
            }
            Ok(())
        }
        "scout" => {
            let plan = scout(version, &o)?;
            println!("plan: seed {} patch x {}..{} rows around z {}, {} trips", plan.seed, plan.fx, plan.fx + 3, plan.rz, plan.trips);
            Ok(())
        }
        "record" => {
            let plan = load_plan(&o)?;
            record(version, &plan, &o)
        }
        "compose" => {
            let plan = load_plan(&o)?;
            compose(version, &plan, &o)
        }
        "all" => {
            let plan = if o.out.join("plan.json").exists() { load_plan(&o)? } else { scout(version, &o)? };
            record(version, &plan, &o)?;
            compose(version, &plan, &o)
        }
        other => bail!("unknown --phase {other} (smoke|scout|record|compose|all)"),
    }
}

fn load_plan(o: &VideoOptions) -> Result<Plan> {
    let text = std::fs::read_to_string(o.out.join("plan.json")).context("no plan.json (run --phase scout first)")?;
    Ok(serde_json::from_str(&text)?)
}

fn server_opts(version: &str, seed: i64, world: Option<PathBuf>) -> VanillaOptions {
    VanillaOptions {
        version: version.to_string(),
        heap_mb: 4096,
        keep_dir: true,
        seed,
        default_world: true,
        world,
        gamemode: "survival".into(),
        force_gamemode: false,
        frozen: false,
        with_bot: false,
        view_distance: 6,
        simulation_distance: 5,
        ..Default::default()
    }
}

// ---- scout ---------------------------------------------------------------------------------

fn scout(version: &str, o: &VideoOptions) -> Result<Plan> {
    let seed = o.seed.unwrap_or_else(|| {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        (t ^ (std::process::id() as u128) << 32) as i64
    });
    eprintln!("scout: seed {seed}");
    let mut srv = VanillaServer::start(&server_opts(version, seed, None))?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -80 -80 79 79")?;
    // Wait for the floor chunks to exist on disk.
    let t0 = Instant::now();
    let floor = loop {
        match srv.read_dim_box("the_nether", [-64, 0, -64], [63, 15, 63]) {
            Ok(s) => break Blocks::new(s.region),
            Err(e) => {
                if t0.elapsed() > Duration::from_secs(240) {
                    return Err(e.context("the nether floor never generated"));
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    };
    eprintln!("scout: floor read after {:.0}s", t0.elapsed().as_secs_f64());
    // Candidate frames: fewest trips, no air/fluid in the floor stack, no lava overhead.
    let mut cands: Vec<(usize, i32, i32, i32, Vec<Vec<i32>>)> = Vec::new();
    for fx in -48..=44 {
        for rz in -44..=44 {
            if let Some((trips, layers)) = candidate(&floor, fx, rz) {
                cands.push((trips, fx.abs() + rz.abs(), fx, rz, layers));
            }
        }
    }
    cands.sort();
    if cands.is_empty() {
        bail!("scout: no usable floor column in the searched area");
    }
    for c in cands.iter().take(5) {
        eprintln!("scout: candidate fx={} rz={} trips={}", c.2, c.3, c.0);
    }
    // The first candidate whose shaft digs down solid rock and whose overworld spot is dry.
    let mut chosen: Option<Plan> = None;
    for (_, _, fx, rz, layers) in cands.iter().take(24) {
        let (fx, rz) = (*fx, *rz);
        let shaft = (fx + 1, rz + 7);
        let col = srv.read_dim_box("the_nether", [shaft.0, 0, shaft.1], [shaft.0, 127, shaft.1])?;
        let col = Blocks::new(col.region);
        let Some(stand_y) = diggable_stand(&col, shaft.0, shaft.1) else { continue };
        let (ox, oz) = (8 * (fx + 1) + 2, 8 * rz + 4);
        srv.cmd_ok(&format!("forceload add {} {} {} {}", ox - 16, oz - 16, ox + 24, oz + 16))?;
        std::thread::sleep(Duration::from_secs(2));
        let ow = srv.read_dim_box("overworld", [ox, 0, oz - 2], [ox + 3, 12, oz + 4])?;
        let owb = Blocks::new(ow.region);
        let bad = (ox..=ox + 3).any(|x| (0..=12).any(|y| (oz - 2..=oz + 4).any(|z| matches!(owb.get([x, y, z]), s if s.starts_with("minecraft:water") || s.starts_with("minecraft:lava")))));
        if bad {
            srv.cmd_ok(&format!("forceload remove {} {} {} {}", ox - 16, oz - 16, ox + 24, oz + 16))?;
            continue;
        }
        let surf = srv.read_dim_box("overworld", [ox + 1, 0, oz + 3], [ox + 1, 200, oz + 3])?;
        let surfb = Blocks::new(surf.region);
        let Some(ow_stand_y) = surface_stand(&surfb, ox + 1, oz + 3) else {
            srv.cmd_ok(&format!("forceload remove {} {} {} {}", ox - 16, oz - 16, ox + 24, oz + 16))?;
            continue;
        };
        // The return portal's corridor must not cross floor-level lava (east first, else west).
        let strip_lava = |rx: i32| -> bool {
            let (a, b) = if rx > fx { (fx + 4, rx + 4) } else { (rx, fx - 1) };
            (a..=b).any(|x| (0..=6).any(|y| (rz..=rz + 1).any(|z| floor.get([x, y, z]).starts_with("minecraft:lava"))))
        };
        let rx = if !strip_lava(fx + 24) {
            fx + 24
        } else if !strip_lava(fx - 28) {
            fx - 28
        } else {
            srv.cmd_ok(&format!("forceload remove {} {} {} {}", ox - 16, oz - 16, ox + 24, oz + 16))?;
            continue;
        };
        let trips: usize = layers.iter().map(|r| r.len()).sum();
        chosen = Some(Plan { seed, fx, rz, layers: layers.clone(), trips, shaft_x: shaft.0, shaft_z: shaft.1, shaft_stand_y: stand_y, ox, oz, ow_stand_y, rx });
        break;
    }
    let plan = chosen.context("scout: every candidate failed the shaft/overworld checks")?;
    eprintln!("scout: patch x {}..{} z {} ({} trips), shaft ({},{}) stand y{}, overworld frame ({},{}) stand y{}",
        plan.fx, plan.fx + 3, plan.rz, plan.trips, plan.shaft_x, plan.shaft_z, plan.shaft_stand_y, plan.ox, plan.oz, plan.ow_stand_y);
    srv.cmd_ok("save-all flush")?;
    let world_src = o.out.join("world-src");
    let _ = std::fs::remove_dir_all(&world_src);
    let src = srv.dir.join(&srv.level_name);
    drop(srv); // stopped (keep_dir): copy a quiescent world
    copy_tree(&src, &world_src)?;
    std::fs::write(o.out.join("plan.json"), serde_json::to_string_pretty(&plan)?)?;
    Ok(plan)
}

/// Rows holding bedrock per layer at (fx, rz), if the column is usable: a solid floor stack
/// (no air, no fluid in y 0..4 over the frame) and no lava in/above the pit.
fn candidate(floor: &Blocks, fx: i32, rz: i32) -> Option<(usize, Vec<Vec<i32>>)> {
    for x in fx - 1..=fx + 4 {
        for z in rz - 5..=rz + 5 {
            for y in 0..=12 {
                let s = floor.get([x, y, z]);
                if s.starts_with("minecraft:lava") {
                    return None;
                }
                if (fx..=fx + 3).contains(&x) && y <= 4 && floor.kind([x, y, z]) != Kind::Solid {
                    return None;
                }
            }
        }
    }
    let mut layers = vec![Vec::new(); 5];
    let mut trips = 0;
    for l in 0..5i32 {
        for rz2 in rz - l..=rz + l {
            if (fx..=fx + 3).any(|x| floor.get([x, l, rz2]).starts_with("minecraft:bedrock")) {
                layers[l as usize].push(rz2);
                trips += 1;
            }
        }
    }
    Some((trips, layers))
}

/// A feet-y to stand on with solid, lava-free rock from there down to the floor (dig straight down).
fn diggable_stand(col: &Blocks, x: i32, z: i32) -> Option<i32> {
    (6..=110).rev().find(|&y| {
        col.kind([x, y, z]) == Kind::Replaceable
            && col.kind([x, y + 1, z]) == Kind::Replaceable
            && col.kind([x, y - 1, z]) == Kind::Solid
            && (5..y).all(|yy| col.kind([x, yy, z]) == Kind::Solid)
    })
}

/// The overworld surface feet-y (solid top, air above, not water/lava/leaves-as-sea).
fn surface_stand(col: &Blocks, x: i32, z: i32) -> Option<i32> {
    (40..=180).rev().find(|&y| {
        let below = col.get([x, y - 1, z]);
        col.kind([x, y, z]) == Kind::Replaceable
            && col.kind([x, y + 1, z]) == Kind::Replaceable
            && col.kind([x, y - 1, z]) == Kind::Solid
            && !below.contains("leaves") && !below.contains("log")
    })
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (p, q) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_tree(&p, &q)?;
        } else if e.file_name() != "session.lock" {
            std::fs::copy(&p, &q)?;
        }
    }
    Ok(())
}

// ---- the actor (file-command client in the real game client) --------------------------------

struct Actor {
    dir: PathBuf,
    seq: u64,
}

impl Actor {
    fn new(game_dir: &Path) -> Result<Actor> {
        let dir = game_dir.join("mctas-actor");
        std::fs::create_dir_all(&dir)?;
        for e in std::fs::read_dir(&dir)?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("cmd-") || n.starts_with("done-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
        Ok(Actor { dir, seq: 0 })
    }

    fn cmd(&mut self, line: &str) -> Result<String> {
        self.cmd_timeout(line, Duration::from_secs(120))
    }

    fn cmd_timeout(&mut self, line: &str, timeout: Duration) -> Result<String> {
        self.seq += 1;
        let seq = self.seq;
        let tmp = self.dir.join(format!("cmd-{seq:06}.part"));
        std::fs::write(&tmp, format!("{line}\n"))?;
        std::fs::rename(&tmp, self.dir.join(format!("cmd-{seq:06}.txt")))?;
        let done = self.dir.join(format!("done-{seq:06}.txt"));
        let t0 = Instant::now();
        loop {
            if let Ok(text) = std::fs::read_to_string(&done) {
                let _ = std::fs::remove_file(&done);
                let text = text.trim().to_string();
                if let Some(err) = text.strip_prefix("error ") {
                    bail!("actor `{line}`: {err}");
                }
                return Ok(text);
            }
            if t0.elapsed() > timeout {
                bail!("actor `{line}` timed out after {timeout:?} (see {})", self.dir.join("actor.log").display());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

// ---- production -----------------------------------------------------------------------------

struct Prod {
    srv: VanillaServer,
    actor: Actor,
    display: mctas::video::Display,
    client: mctas::video::Client,
    rec: Option<mctas::video::Recorder>,
    out: PathBuf,
    plan: Plan,
    timeline: Timeline,
    state: State,
    log: PathBuf,
    seg_idx: usize,
    blockers: Vec<Pos>,
    width: u32,
    height: u32,
    fps: u32,
}

impl Prod {
    fn loga(&self, line: &str) {
        eprintln!("[video] {line}");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.log) {
            use std::io::Write;
            let _ = writeln!(f, "{line}");
        }
    }

    fn rcon(&mut self, cmd: &str) -> Result<String> {
        self.srv.cmd(cmd)
    }

    fn rcon_ok(&mut self, cmd: &str) -> Result<String> {
        self.srv.cmd_ok(cmd)
    }

    fn test_block(&mut self, dim: &str, p: Pos, pred: &str) -> Result<bool> {
        let out = self.rcon(&format!("execute in minecraft:{dim} if block {} {} {} {pred}", p[0], p[1], p[2]))?;
        if out.contains("Test passed") {
            Ok(true)
        } else if out.contains("Test failed") {
            Ok(false)
        } else {
            bail!("execute if block: {out}")
        }
    }

    fn assert_block(&mut self, dim: &str, p: Pos, pred: &str, what: &str) -> Result<()> {
        if self.test_block(dim, p, pred)? {
            self.loga(&format!("ASSERT ok {dim} {p:?} {pred} ({what})"));
            Ok(())
        } else {
            bail!("ASSERT failed {dim} {p:?} is not {pred} ({what})")
        }
    }

    fn tp(&mut self, dim: &str, x: f64, y: f64, z: f64, yaw: f32, pitch: f32) -> Result<()> {
        self.rcon_ok(&format!("execute in minecraft:{dim} run tp {PLAYER} {x:.2} {y:.2} {z:.2} {yaw} {pitch}"))?;
        self.actor.cmd_timeout("wait 10", Duration::from_secs(40))?;
        Ok(())
    }

    fn pos(&mut self) -> Result<String> {
        self.actor.cmd("pos")
    }

    fn feet(&mut self) -> Result<[f64; 3]> {
        let s = self.pos()?;
        let p = s.split("pos=").nth(1).and_then(|r| r.split(' ').next()).context("pos line")?;
        let v: Vec<f64> = p.split(',').map(|x| x.parse()).collect::<std::result::Result<_, _>>()?;
        Ok([v[0], v[1], v[2]])
    }

    fn select(&mut self, slot: u32) -> Result<()> {
        self.actor.cmd(&format!("select {slot}"))?;
        Ok(())
    }

    fn mine(&mut self, p: Pos) -> Result<()> {
        self.actor.cmd_timeout(&format!("mine {} {} {}", p[0], p[1], p[2]), Duration::from_secs(60))?;
        Ok(())
    }

    /// Bulk mining (held attack key, sweep): clearing and digging, never near blocks that matter.
    fn mine_sweep(&mut self, p: Pos) -> Result<()> {
        self.actor.cmd_timeout(&format!("mine {} {} {} sweep", p[0], p[1], p[2]), Duration::from_secs(60))?;
        Ok(())
    }

    fn place(&mut self, click: Pos, face: &str) -> Result<String> {
        self.actor.cmd(&format!("place {} {} {} {}", click[0], click[1], click[2], face))
    }

    fn walk(&mut self, x: i32, z: i32) -> Result<()> {
        self.actor.cmd_timeout(&format!("walk {x} {z}"), Duration::from_secs(90))?;
        Ok(())
    }

    /// Walk waypoints in order (the actor steers greedily: corridors need their corners named).
    fn walk_path(&mut self, pts: &[(i32, i32)]) -> Result<()> {
        for &(x, z) in pts {
            self.walk(x, z)?;
        }
        Ok(())
    }

    fn look(&mut self, x: f64, y: f64, z: f64, ticks: u32) -> Result<()> {
        self.actor.cmd(&format!("look {x:.2} {y:.2} {z:.2} {ticks}"))?;
        Ok(())
    }

    fn lookdir(&mut self, yaw: f32, pitch: f32, ticks: u32) -> Result<()> {
        self.actor.cmd(&format!("lookdir {yaw} {pitch} {ticks}"))?;
        Ok(())
    }

    fn wait(&mut self, ticks: u32) -> Result<()> {
        self.actor.cmd_timeout(&format!("wait {ticks}"), Duration::from_secs(120))?;
        Ok(())
    }

    fn seg(&mut self, name: &str, scene: &str, caption: &[&str], speed: f64) -> Result<()> {
        if let Some(r) = self.rec.take() {
            r.stop()?;
        }
        self.seg_idx += 1;
        let file = format!("{:02}-{name}.mkv", self.seg_idx);
        let path = self.out.join("segments").join(&file);
        self.timeline.segments.push(Segment { file: file.clone(), scene: scene.to_string(), caption: caption.iter().map(|s| s.to_string()).collect(), speed });
        std::fs::write(self.out.join("timeline.json"), serde_json::to_string_pretty(&self.timeline)?)?;
        self.rec = Some(mctas::video::Recorder::start_crf(&self.display, self.width, self.height, self.fps, &path, Some(16))?);
        self.loga(&format!("segment {file} ({caption:?} @ {speed}x)"));
        Ok(())
    }

    fn stop_rec(&mut self) -> Result<()> {
        if let Some(r) = self.rec.take() {
            r.stop()?;
        }
        Ok(())
    }

    fn save_state(&self) -> Result<()> {
        std::fs::write(self.out.join("state.json"), serde_json::to_string_pretty(&self.state)?)?;
        Ok(())
    }

    fn mark_done(&mut self, tag: &str) -> Result<()> {
        if !self.state.done.contains(&tag.to_string()) {
            self.state.done.push(tag.to_string());
        }
        self.save_state()
    }

    fn checkpoint(&mut self, tag: &str) -> Result<()> {
        self.rcon_ok("save-off")?;
        self.rcon_ok("save-all flush")?;
        let dst = self.out.join("checkpoints").join(tag);
        let _ = std::fs::remove_dir_all(&dst);
        let r = copy_tree(&self.srv.dir.join(&self.srv.level_name), &dst);
        let _ = self.rcon_ok("save-on");
        r?;
        self.mark_done(tag)?;
        // The checkpoint carries the state it was taken with (resume loads it back).
        std::fs::write(dst.join("mctas-state.json"), serde_json::to_string_pretty(&self.state)?)?;
        self.loga(&format!("checkpoint {tag}"));
        Ok(())
    }

    fn done(&self, tag: &str) -> bool {
        self.state.done.iter().any(|t| t == tag)
    }

    fn nether_box(&mut self, lo: Pos, hi: Pos) -> Result<Blocks> {
        Ok(Blocks::new(self.srv.read_dim_box("the_nether", lo, hi)?.region))
    }

    fn overworld_box(&mut self, lo: Pos, hi: Pos) -> Result<Blocks> {
        Ok(Blocks::new(self.srv.read_dim_box("overworld", lo, hi)?.region))
    }
}

fn launch_client(version: &str, o: &VideoOptions, display: &mctas::video::Display, port: u16, game_dir: &Path) -> Result<mctas::video::Client> {
    match o.driver.as_str() {
        "fabric" => {
            let build = mctas::video::build_client(version)?;
            let mut fabric = mctas::fabric::install_loader(version)?;
            let jar = mctas::fabric::build_actor(version, &build, &fabric.libs)?;
            fabric.mods = vec![jar];
            eprintln!("client: Fabric loader {} + actor mod", fabric.loader);
            mctas::fabric::launch(&build, &fabric, display, Some(port), o.width, o.height, game_dir)
        }
        "launcher" => {
            let build = mctas::video::build_actor_client(version)?;
            eprintln!("client: actor launcher (no Fabric)");
            mctas::video::Client::launch_main(&build, display, Some(port), o.width, o.height, game_dir, "dev.vjeux.mctas.client.ActorMain", &[], &[])
        }
        other => bail!("unknown --driver {other} (fabric|launcher)"),
    }
}

fn start_prod(version: &str, plan: &Plan, o: &VideoOptions, world: PathBuf) -> Result<Prod> {
    // The Nether at render distance 6 on llvmpipe runs at a few fps; 4 is plenty inside the fog.
    // SAFETY: no other thread of this process is reading the environment yet (the server and
    // client are child processes, spawned below).
    if std::env::var_os("MCTAS_CLIENT_RENDER_DISTANCE").is_none() {
        unsafe { std::env::set_var("MCTAS_CLIENT_RENDER_DISTANCE", "4") };
    }
    let mut srv = VanillaServer::start(&server_opts(version, plan.seed, Some(world)))?;
    let port = srv.game_port();
    let display = mctas::video::Display::start(o.width, o.height)?;
    let game_dir = o.out.join("client");
    let mut actor = Actor::new(&game_dir)?;
    let client = launch_client(version, o, &display, port, &game_dir)?;
    // Wait for the actor to answer inside the world.
    let t0 = Instant::now();
    loop {
        match actor.cmd_timeout("ping", Duration::from_secs(20)) {
            Ok(s) if s.contains("pos=") => break,
            Ok(_) => {}
            Err(e) => {
                if t0.elapsed() > Duration::from_secs(300) {
                    return Err(e.context(format!("the client never joined; see {}", client.log.display())));
                }
            }
        }
        if t0.elapsed() > Duration::from_secs(300) {
            bail!("the client never joined; see {}", client.log.display());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    std::fs::create_dir_all(o.out.join("segments"))?;
    let timeline: Timeline = std::fs::read_to_string(o.out.join("timeline.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let state: State = std::fs::read_to_string(o.out.join("state.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let seg_idx = timeline.segments.len();
    let blockers = state.blockers.clone();
    Ok(Prod {
        srv,
        actor,
        display,
        client,
        rec: None,
        out: o.out.clone(),
        plan: plan.clone(),
        timeline,
        state,
        log: o.out.join("production.log"),
        seg_idx,
        blockers,
        width: o.width,
        height: o.height,
        fps: o.fps,
    })
}

// ---- smoke ----------------------------------------------------------------------------------

fn smoke(version: &str, o: &VideoOptions) -> Result<()> {
    let plan = Plan { seed: 1, fx: 0, rz: 0, layers: vec![], trips: 0, shaft_x: 0, shaft_z: 0, shaft_stand_y: 70, ox: 0, oz: 0, ow_stand_y: 70, rx: 0 };
    let world_dir = o.out.join("smoke-world");
    let _ = std::fs::remove_dir_all(&world_dir);
    // A fresh world: start once without a world to generate, then it is kept (keep_dir).
    let mut p = {
        let mut srv = VanillaServer::start(&server_opts(version, 1, None))?;
        srv.cmd_ok("save-all flush")?;
        copy_tree(&srv.dir.join(&srv.level_name), &world_dir)?;
        drop(srv);
        start_prod(version, &plan, o, world_dir.clone())?
    };
    p.loga("smoke: client in");
    p.rcon_ok(&format!("gamemode survival {PLAYER}"))?;
    p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.0 with minecraft:netherite_pickaxe"))?;
    p.rcon_ok(&format!("enchant {PLAYER} minecraft:efficiency 5"))?;
    p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.1 with minecraft:obsidian 64"))?;
    p.rcon_ok(&format!("effect give {PLAYER} minecraft:haste infinite 1 true"))?;
    // A flat test pad (the smoke world is throwaway; commands are fine here).
    p.rcon_ok("forceload add -16 -16 16 16")?;
    std::thread::sleep(Duration::from_secs(3));
    p.rcon_ok("fill -4 68 -4 4 69 4 minecraft:stone")?;
    p.rcon_ok("fill -4 70 -4 4 74 4 minecraft:air")?;
    p.tp("overworld", 0.5, 70.0, 0.5, 0.0, 20.0)?;
    std::thread::sleep(Duration::from_secs(2));
    let feet = p.feet()?;
    p.loga(&format!("smoke: feet {feet:?}"));
    let b = [feet[0].floor() as i32, feet[1].floor() as i32 - 1, feet[2].floor() as i32 + 2];
    p.walk(b[0], b[2] - 1)?;
    p.mine(b)?;
    p.assert_block("overworld", b, "minecraft:air", "smoke mined the block ahead")?;
    p.select(SLOT_OBSIDIAN)?;
    let floor = [b[0], b[1] - 1, b[2]];
    let r = p.place(floor, "UP")?;
    p.loga(&format!("smoke place: {r}"));
    p.assert_block("overworld", b, "minecraft:obsidian", "smoke placed obsidian")?;
    p.select(SLOT_PICK)?;
    let t0 = Instant::now();
    p.mine(b)?;
    p.loga(&format!("smoke: obsidian mined in {:.1}s (survival timing)", t0.elapsed().as_secs_f64()));
    p.assert_block("overworld", b, "minecraft:air", "smoke mined the obsidian back")?;
    println!("smoke ok: mine/place/walk/select all real; {}", p.pos()?);
    Ok(())
}

// ---- record: shared player work -------------------------------------------------------------

impl Prod {
    /// Mine straight down the player's column until their feet are at `target_y`.
    fn dig_down(&mut self, target_y: f64) -> Result<()> {
        for _ in 0..200 {
            let f = self.feet()?;
            if f[1] <= target_y + 0.05 {
                return Ok(());
            }
            let below = [f[0].floor() as i32, f[1].floor() as i32 - 1, f[2].floor() as i32];
            self.mine_sweep(below)?;
            self.wait(4)?;
        }
        bail!("dig_down never reached y {target_y}")
    }

    /// Mine every solid, minable block in the box (bedrock and portals never; obsidian only if
    /// asked), re-reading until it is air. Player stands are taken from the reads: a cell with a
    /// solid block under it and air where feet/head go, near each target.
    fn clear_box(&mut self, dim: &str, lo: Pos, hi: Pos, obsidian: bool) -> Result<()> {
        // Read a margin around the box too: stand-finding needs the floor under a stand and
        // the headroom over it.
        let rlo = [lo[0] - 3, lo[1] - 1, lo[2] - 3];
        let rhi = [hi[0] + 3, hi[1] + 2, hi[2] + 3];
        let mut rescued = false;
        for pass in 0..60 {
            let blocks = if dim == "the_nether" { self.nether_box(rlo, rhi)? } else { self.overworld_box(rlo, rhi)? };
            let mut todo: Vec<Pos> = Vec::new();
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    for x in lo[0]..=hi[0] {
                        let s = blocks.get([x, y, z]);
                        if blocks.kind([x, y, z]) == Kind::Solid
                            && !s.starts_with("minecraft:bedrock")
                            && (obsidian || !s.starts_with("minecraft:obsidian"))
                        {
                            todo.push([x, y, z]);
                        }
                    }
                }
            }
            if todo.is_empty() {
                return Ok(());
            }
            // Column by column from the south (where the player always enters), BOTTOM-UP
            // within a column: from solid rock the only stands are mined-out cells, so each
            // column is hollowed from its base standing in the previous one (top-down would
            // need stands floating in unmined rock). Small batches, because stands come from
            // the pass-start read; blocks with no stand yet wait for the next read.
            todo.sort_by_key(|p| (std::cmp::Reverse(p[2]), p[0], p[1]));
            self.loga(&format!("clear_box {dim} pass {pass}: {} blocks", todo.len()));
            self.select(SLOT_PICK)?;
            let first: Vec<Pos> = todo.iter().take(8).copied().collect();
            let mut mined = 0;
            for p in todo {
                if mined >= 40 {
                    break;
                }
                match self.stand_near(&blocks, p) {
                    Ok(()) => {
                        self.mine_sweep(p)?;
                        mined += 1;
                    }
                    Err(_) => continue,
                }
            }
            if mined == 0 {
                self.loga(&format!("clear_box stuck; first remaining: {first:?}"));
                // The player can end up walled into a side pocket (a geode shell, say) with
                // every walk failing: put them back in the middle of the room (travel, not a
                // block edit) once; a second stuck pass in a row really is stuck.
                if !rescued {
                    rescued = true;
                    let (cx, cz) = ((lo[0] + hi[0]) / 2, (lo[2] + hi[2]) / 2);
                    self.loga(&format!("clear_box rescue: tp to room center ({cx}, {}, {cz})", lo[1]));
                    self.tp(dim, cx as f64 + 0.5, lo[1] as f64, cz as f64 + 0.5, 0.0, 30.0)?;
                    continue;
                }
                bail!("clear_box {dim} {lo:?}..{hi:?}: no reachable stand in a whole pass");
            }
            rescued = false;
        }
        bail!("clear_box {dim} {lo:?}..{hi:?} still not clear after 60 passes")
    }

    /// Walk to a standable cell within reach of `target` (from the last box read).
    fn stand_near(&mut self, blocks: &Blocks, target: Pos) -> Result<()> {
        let f = self.feet()?;
        let cur = [f[0].floor() as i32, f[1].floor() as i32, f[2].floor() as i32];
        let reach = |stand: Pos| -> bool {
            let eye = [stand[0] as f64 + 0.5, stand[1] as f64 + 1.62, stand[2] as f64 + 0.5];
            let c = [target[0] as f64 + 0.5, target[1] as f64 + 0.5, target[2] as f64 + 0.5];
            ((eye[0] - c[0]).powi(2) + (eye[1] - c[1]).powi(2) + (eye[2] - c[2]).powi(2)).sqrt() <= 4.3
        };
        if reach(cur) {
            return Ok(());
        }
        let mut cands: Vec<(i32, Pos)> = Vec::new();
        for dx in -3..=3 {
            for dz in -3..=3 {
                // A stand can be up to 5 below the target (eye height + reach), e.g. the last
                // ceiling-corner block of a room mined from its floor.
                for dy in -5..=2 {
                    let s = [target[0] + dx, target[1] + dy, target[2] + dz];
                    // Never the target itself, and never standing ON it (mining the y=0 row
                    // while standing on it would drop the player through the new hole).
                    if s == target || (s[0] == target[0] && s[2] == target[2] && s[1] == target[1] + 1) {
                        continue;
                    }
                    if blocks.kind([s[0], s[1] - 1, s[2]]) == Kind::Solid
                        && blocks.kind(s) == Kind::Replaceable
                        && blocks.kind([s[0], s[1] + 1, s[2]]) == Kind::Replaceable
                        && reach(s)
                    {
                        // Same level as the player first: a same-height map stand on top of
                        // unmined rock outside the room is usually unreachable on foot, while
                        // the floor below the target is a short walk.
                        let d = (s[0] - cur[0]).abs() + (s[2] - cur[2]).abs() + (s[1] - cur[1]).abs() * 4;
                        cands.push((d, s));
                    }
                }
            }
        }
        cands.sort_by_key(|(d, _)| *d);
        cands.dedup_by_key(|(_, s)| *s);
        match cands.first().copied() {
            Some(_) => {
                let mut last_err = None;
                for (_, s) in cands.into_iter().take(4) {
                    match self.walk(s[0], s[2]) {
                        Ok(()) => return Ok(()),
                        Err(e) => {
                            self.loga(&format!("stand_near {target:?}: walk to {s:?} failed: {e:#}"));
                            last_err = Some(e);
                        }
                    }
                }
                Err(last_err.unwrap())
            }
            None => {
                // The pass-start read may lag the real cavity (sweep overshoot, earlier
                // blocks): walk at the target anyway and re-check reach — but only at the
                // nearby frontier (a far target inside solid rock would burn the walk timeout).
                if (target[0] - cur[0]).abs() + (target[2] - cur[2]).abs() > 6 {
                    bail!("no stand within reach of {target:?} (player at {cur:?})");
                }
                self.walk(target[0], target[2]).ok();
                let f = self.feet()?;
                let cur = [f[0].floor() as i32, f[1].floor() as i32, f[2].floor() as i32];
                if reach(cur) {
                    Ok(())
                } else {
                    bail!("no stand within reach of {target:?} (player at {cur:?})")
                }
            }
        }
    }

    /// A 4x5 nether portal frame with its bottom row at `ybase`, plane z = z0, west x = x0,
    /// placed block by block and lit with flint and steel.
    fn build_portal(&mut self, dim: &str, x0: i32, ybase: i32, z0: i32) -> Result<()> {
        self.select(SLOT_OBSIDIAN)?;
        // bottom row on the floor blocks below
        for x in x0..=x0 + 3 {
            self.walk(x, z0 + 2)?;
            self.place([x, ybase - 1, z0], "UP")?;
        }
        // pillars
        for x in [x0, x0 + 3] {
            self.walk(x, z0 + 2)?;
            for y in ybase + 1..=ybase + 3 {
                self.place([x, y - 1, z0], "UP")?;
            }
        }
        // top row: corners on the pillars, middles against a corner's side face
        self.walk(x0, z0 + 2)?;
        self.place([x0, ybase + 3, z0], "UP")?;
        self.walk(x0 + 3, z0 + 2)?;
        self.place([x0 + 3, ybase + 3, z0], "UP")?;
        self.place([x0 + 3, ybase + 4, z0], "WEST")?;
        self.place([x0 + 2, ybase + 4, z0], "WEST")?;
        for x in x0..=x0 + 3 {
            self.assert_block(dim, [x, ybase, z0], "minecraft:obsidian", "portal bottom row")?;
            self.assert_block(dim, [x, ybase + 4, z0], "minecraft:obsidian", "portal top row")?;
        }
        self.select(SLOT_FLINT)?;
        self.walk(x0 + 1, z0 + 1)?;
        self.place([x0 + 1, ybase, z0], "UP")?;
        self.assert_block(dim, [x0 + 1, ybase + 1, z0], "minecraft:nether_portal", "portal lit")?;
        self.select(SLOT_PICK)?;
        Ok(())
    }

    /// Mine the 10 frame obsidian of the generated portal on the patch (bottom row stays).
    fn mine_frame(&mut self, fx: i32, l: i32, rz: i32) -> Result<()> {
        self.select(SLOT_PICK)?;
        let mut cells: Vec<Pos> = Vec::new();
        for x in [fx, fx + 3] {
            for y in l + 1..=l + 3 {
                cells.push([x, y, rz]);
            }
        }
        for x in fx..=fx + 3 {
            cells.push([x, l + 4, rz]);
        }
        for c in cells {
            self.mine(c)?;
        }
        self.assert_block("the_nether", [fx + 1, l + 1, rz], "minecraft:air", "generated portal gone after mining its frame")?;
        Ok(())
    }

    fn torch(&mut self, floor: Pos) -> Result<()> {
        self.walk(floor[0], floor[2] + 1)?;
        self.select(SLOT_TORCH)?;
        self.place(floor, "UP")?;
        self.select(SLOT_PICK)?;
        Ok(())
    }

    /// Make every cell of the sheet (x0..=x1, y, z0..=z1) solid, filling holes with netherrack
    /// (clicked on whatever solid face is available). Fails if a cell of the inner patch rect
    /// `must` stays non-solid.
    fn fill_floor(&mut self, dim: &str, y: i32, x0: i32, x1: i32, z0: i32, z1: i32, must: Option<(i32, i32, i32, i32)>) -> Result<()> {
        let blocks = if dim == "the_nether" { self.nether_box([x0, y - 1, z0], [x1, y, z1])? } else { self.overworld_box([x0, y - 1, z0], [x1, y, z1])? };
        self.select(SLOT_NETHERRACK)?;
        for z in z0..=z1 {
            for x in x0..=x1 {
                if blocks.kind([x, y, z]) == Kind::Solid {
                    continue;
                }
                let clicks: [(Pos, &str); 5] = [
                    ([x, y - 1, z], "UP"),
                    ([x, y, z + 1], "NORTH"),
                    ([x, y, z - 1], "SOUTH"),
                    ([x + 1, y, z], "WEST"),
                    ([x - 1, y, z], "EAST"),
                ];
                for (click, face) in clicks {
                    if blocks.kind(click) == Kind::Solid {
                        self.stand_near(&blocks, click).or_else(|_| self.walk(x, z + 1))?;
                        self.place(click, face)?;
                        break;
                    }
                }
            }
        }
        self.select(SLOT_PICK)?;
        if let Some((mx0, mx1, mz0, mz1)) = must {
            let check = if dim == "the_nether" { self.nether_box([mx0, y, mz0], [mx1, y, mz1])? } else { self.overworld_box([mx0, y, mz0], [mx1, y, mz1])? };
            for z in mz0..=mz1 {
                for x in mx0..=mx1 {
                    if check.kind([x, y, z]) != Kind::Solid {
                        bail!("fill_floor: {dim} [{x},{y},{z}] still not solid ({})", check.get([x, y, z]));
                    }
                }
            }
        }
        Ok(())
    }
}

// ---- record: scenes ---------------------------------------------------------------------------

fn record(version: &str, plan: &Plan, o: &VideoOptions) -> Result<()> {
    let world = match &o.from {
        Some(tag) => o.out.join("checkpoints").join(tag),
        None => o.out.join("world-src"),
    };
    if !world.exists() {
        bail!("world {} missing (run --phase scout first)", world.display());
    }
    if o.from.is_some() {
        // Roll state + timeline back to the checkpoint's own snapshot.
        let state: State = serde_json::from_str(&std::fs::read_to_string(world.join("mctas-state.json")).context("checkpoint has no mctas-state.json")?)?;
        let mut timeline: Timeline = std::fs::read_to_string(o.out.join("timeline.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        timeline.segments.retain(|s| state.done.contains(&s.scene));
        std::fs::write(o.out.join("state.json"), serde_json::to_string_pretty(&state)?)?;
        std::fs::write(o.out.join("timeline.json"), serde_json::to_string_pretty(&timeline)?)?;
        eprintln!("record: resuming from checkpoint {:?} (done: {:?})", o.from, state.done);
    }
    let mut p = start_prod(version, plan, o, world)?;
    p.loga(&format!("record: seed {} patch x {}..{} z {} trips {}", plan.seed, plan.fx, plan.fx + 3, plan.rz, plan.trips));
    scenes(&mut p)?;
    p.stop_rec()?;
    p.checkpoint("end")?;
    p.loga("record done");
    Ok(())
}

fn scenes(p: &mut Prod) -> Result<()> {
    let fx = p.plan.fx;
    let rz = p.plan.rz;
    if !p.done("setup") {
        let seed_out = p.rcon("seed")?;
        p.loga(&format!("server says: {}", seed_out.trim()));
        p.rcon_ok(&format!("gamemode survival {PLAYER}"))?;
        p.rcon_ok(&format!("clear {PLAYER}"))?;
        p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.0 with minecraft:netherite_pickaxe"))?;
        p.rcon_ok(&format!("enchant {PLAYER} minecraft:efficiency 5"))?;
        p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.1 with minecraft:obsidian 64"))?;
        p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.2 with minecraft:flint_and_steel"))?;
        p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.3 with minecraft:netherrack 64"))?;
        p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.4 with minecraft:cobblestone 64"))?;
        p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.5 with minecraft:torch 64"))?;
        p.rcon_ok(&format!("give {PLAYER} minecraft:obsidian 128"))?;
        p.rcon_ok(&format!("give {PLAYER} minecraft:netherrack 128"))?;
        p.rcon_ok(&format!("effect give {PLAYER} minecraft:haste infinite 1 true"))?;
        p.rcon_ok("time set noon")?;
        p.loga("kit given (captioned): netherite pickaxe Efficiency V, obsidian, flint and steel, netherrack, cobblestone, torches, Haste II");
        p.tp("the_nether", p.plan.shaft_x as f64 + 0.5, p.plan.shaft_stand_y as f64, p.plan.shaft_z as f64 + 0.5, 180.0, 10.0)?;
        p.checkpoint("setup")?;
    }
    if !p.done("intro") {
        p.seg("intro", "intro", &[
            &format!("Survival — a real random world, seed {}", p.plan.seed),
            &format!("Nether floor near {}, {} · kit given (Efficiency V pickaxe, obsidian, Haste II) · travel here skipped", fx + 1, rz),
        ], 1.0)?;
        p.lookdir(180.0, 5.0, 10)?;
        p.wait(30)?;
        p.lookdir(120.0, 0.0, 30)?;
        p.lookdir(240.0, 0.0, 40)?;
        p.lookdir(180.0, 20.0, 20)?;
        p.wait(20)?;
        p.checkpoint("intro")?;
    }
    if !p.done("digdown") {
        p.seg("dig-down", "digdown", &["Dig down to the bedrock floor (y 0–4)"], 6.0)?;
        p.walk(p.plan.shaft_x, p.plan.shaft_z)?;
        p.dig_down(5.0)?;
        // step north into the patch room's south strip
        p.select(SLOT_PICK)?;
        for z in (rz + 5..p.plan.shaft_z).rev() {
            p.mine([p.plan.shaft_x, 5, z])?;
            p.mine([p.plan.shaft_x, 6, z])?;
            p.walk(p.plan.shaft_x, z)?;
        }
        p.checkpoint("digdown")?;
    }
    if !p.done("patch") {
        p.seg("patch-prep", "patch", &[
            "Prepare a 4×3 patch on the top bedrock layer (y=4):",
            "the 12 bottom cells solid, the 4×4×3 box above them air",
        ], 6.0)?;
        p.clear_box("the_nether", [fx - 1, 5, rz - 5], [fx + 4, 8, rz + 5], false)?;
        // a walkable, fully solid floor sheet at y=4 (the patch rows must end solid)
        p.fill_floor("the_nether", 4, fx - 1, fx + 4, rz - 5, rz + 5, Some((fx, fx + 3, rz - 4, rz + 4)))?;
        p.walk(fx + 1, rz + 4)?;
        p.torch([fx - 1, 4, rz - 3])?;
        p.torch([fx + 4, 4, rz + 3])?;
        // the "before": look closely at the top bedrock layer
        p.seg("bedrock-before", "patch", &["Bedrock — unbreakable by hand. Watch this row…"], 1.0)?;
        p.walk(fx + 1, rz + 3)?;
        p.look(fx as f64 + 2.0, 4.4, rz as f64 + 0.5, 16)?;
        p.wait(50)?;
        // Aim the first trip while still here (it is made from the overworld much later).
        if let Some(&first_row) = row_order(&p.plan.layers[4], rz).first() {
            prep_trip(p, 4, first_row)?;
        }
        p.checkpoint("patch")?;
    }
    if !p.done("return") {
        let rx = p.plan.rx;
        p.seg("return-portal", "return", &[
            &format!("A permanent return portal, {} blocks {}", (rx - fx).abs(), if rx > fx { "east" } else { "west" }),
            "(17+ away: portal searches are 16 in the Nether, 128 in the Overworld — exactly 8×)",
        ], 6.0)?;
        let mouth = if rx > fx { fx + 4 } else { fx - 1 };
        p.clear_box("the_nether", [mouth.min(rx + 5), 5, rz], [mouth.max(rx + 5), 6, rz + 1], false)?;
        p.fill_floor("the_nether", 4, mouth.min(rx + 5), mouth.max(rx + 5), rz, rz + 1, None)?;
        p.clear_box("the_nether", [rx - 2, 5, rz - 3], [rx + 5, 9, rz + 3], false)?;
        p.fill_floor("the_nether", 4, rx - 1, rx + 4, rz - 1, rz + 1, Some((rx, rx + 3, rz, rz)))?;
        p.walk(rx + 1, rz + 2)?;
        p.build_portal("the_nether", rx, 5, rz)?;
        p.torch([rx - 1, 4, rz + 2])?;
        // East out of the room AROUND the lit frame (its interior would teleport us).
        let around = if rx > fx { rx - 1 } else { rx + 4 };
        p.walk_path(&[(around, rz + 2), (around, rz), (mouth, rz), (fx + 1, rz + 3)])?;
        p.checkpoint("return")?;
    }
    if !p.done("overworld") {
        p.seg("to-overworld", "overworld", &["Back through the return portal…"], 1.5)?;
        go_return(p)?;
        p.seg("overworld-dig", "overworld", &[
            "— travel skipped — the drill site: 8× the patch coordinates",
            "Dig down: the drill portal's inside floor must be at y = layer + 1 (here y=5)",
        ], 8.0)?;
        p.tp("overworld", (p.plan.ox + 1) as f64 + 0.5, p.plan.ow_stand_y as f64, (p.plan.oz + 3) as f64 + 0.5, 180.0, 20.0)?;
        p.walk(p.plan.ox + 1, p.plan.oz + 3)?;
        p.dig_down(4.0)?;
        p.clear_box("overworld", [p.plan.ox - 2, 4, p.plan.oz - 3], [p.plan.ox + 5, 9, p.plan.oz + 3], false)?;
        p.seg("drill-build", "overworld", &[
            "The drill portal: interior columns at x mod 8 = 3–4,",
            "so both columns land on the same Nether block. Light it.",
        ], 2.0)?;
        p.build_portal("overworld", p.plan.ox, 4, p.plan.oz)?;
        p.state.drill_serves = Some(4);
        p.torch([p.plan.ox - 1, 3, p.plan.oz + 2])?;
        p.torch([p.plan.ox + 4, 3, p.plan.oz + 2])?;
        p.checkpoint("overworld")?;
    }
    // The drilling: layer by layer from y=4 down. Every trip is prep in the Nether (blockers),
    // back through the return portal, a captioned teleport across the Overworld, and through
    // the drill portal — except the very first, whose blockers were placed during `patch`
    // (the player then builds both portals and comes straight through).
    let mut trip_no = 0usize;
    let mut first_done = false;
    for l in (0..=4i32).rev() {
        let ltag = format!("layer{l}");
        if p.done(&ltag) {
            trip_no += p.plan.layers[l as usize].len();
            if !p.plan.layers[l as usize].is_empty() {
                first_done = true;
            }
            continue;
        }
        let rows = row_order(&p.plan.layers[l as usize], rz);
        p.loga(&format!("layer {l}: rows {rows:?}"));
        if rows.is_empty() {
            ensure_nether(p)?;
            mine_layer(p, l)?;
            p.checkpoint(&ltag)?;
            continue;
        }
        for &row in &rows {
            trip_no += 1;
            let ttag = format!("trip-{l}-{row}");
            if p.done(&ttag) {
                first_done = true;
                continue;
            }
            let is_first = !first_done;
            let is_second = trip_no == 2;
            if !is_first {
                ensure_nether(p)?;
                if is_second {
                    p.seg("second-trip", &ttag, &[
                        "Aim each trip: one cobblestone blocks the rows you don't want",
                        &format!("(trip {trip_no} of {} · travel between portals skipped)", p.plan.trips),
                    ], 2.0)?;
                } else {
                    p.seg(&format!("trip-l{l}-r{row}"), &ttag, &[
                        &format!("Layer y={l} — each deeper layer is one row narrower (trip {trip_no} of {})", p.plan.trips),
                    ], 12.0)?;
                }
                prep_trip(p, l, row)?;
                go_return(p)?;
                p.tp("overworld", (p.plan.ox + 1) as f64 + 0.5, 4.0, (p.plan.oz + 2) as f64 + 0.5, 180.0, 10.0)?;
            } else {
                p.seg("first-trip", &ttag, &[
                    "No portal near the exit → the game builds one…",
                    "…and writes its bottom row over the bedrock",
                ], 1.0)?;
            }
            lower_drill_portal(p, l)?;
            do_trip(p, l, row)?;
            if is_first {
                p.seg("first-mine", &ttag, &[
                    "Mine the new frame — but keep the converted row:",
                    "it stays as the solid floor for the next trips",
                ], 1.0)?;
            }
            p.mine_frame(fx, l, row)?;
            p.mark_done(&ttag)?;
            first_done = true;
        }
        // Layer finished: mine everything solid left in this layer's rows (converted obsidian
        // and any plain rock), opening the step for the layer below.
        p.seg(&format!("mine-layer-{l}"), &ltag, &[
            &format!("Layer y={l} converted — now mine the obsidian out"),
        ], 8.0)?;
        p.loga(&format!("layer {l}: mining the converted rows out"));
        mine_layer(p, l)?;
        p.checkpoint(&ltag)?;
    }
    if !p.done("finale") {
        for x in fx..=fx + 3 {
            p.assert_block("the_nether", [x, 0, rz], "minecraft:air", "hole through y=0")?;
        }
        p.seg("finale", "finale", &["Through. A 1×4 hole in the Nether floor, mined in survival."], 1.0)?;
        p.walk(fx + 1, rz - 2)?;
        p.walk(fx + 1, rz - 1)?;
        p.actor.cmd("sneak 1")?;
        p.look(fx as f64 + 2.0, -2.0, rz as f64 + 0.5, 20)?;
        p.wait(60)?;
        p.lookdir(180.0, -10.0, 20)?;
        p.actor.cmd("sneak 0")?;
        p.wait(30)?;
        p.checkpoint("finale")?;
    }
    Ok(())
}

/// Center row first, then alternating outward (converted rows stay as support).
fn row_order(rows: &[i32], rz: i32) -> Vec<i32> {
    let mut v: Vec<i32> = rows.to_vec();
    v.sort_by_key(|r| (r - rz).abs());
    v
}

/// In the Nether at the patch: walk the corridor, enter the return portal, land at its twin.
fn go_return(p: &mut Prod) -> Result<()> {
    let fx = p.plan.fx;
    let rx = p.plan.rx;
    let rz = p.plan.rz;
    let mouth = if rx > fx { fx + 4 } else { fx - 1 };
    p.walk_path(&[(mouth, rz), (rx + 1, rz + 1), (rx + 1, rz)])?;
    p.actor.cmd_timeout("waitdim overworld", Duration::from_secs(90))?;
    p.wait(10)?;
    if p.state.twin.is_none() {
        p.state.twin = Some(p.feet()?);
        p.save_state()?;
        p.loga(&format!("return twin at {:?}", p.state.twin));
    }
    Ok(())
}

/// The player is in the Nether afterwards (via the return twin when coming from the Overworld;
/// the teleport puts them at the twin — the trip through it is real).
fn ensure_nether(p: &mut Prod) -> Result<()> {
    if p.pos()?.contains("the_nether") {
        return Ok(());
    }
    let twin = p.state.twin.context("return twin not known yet")?;
    p.tp("overworld", twin[0], twin[1], twin[2], 0.0, 0.0)?;
    p.actor.cmd_timeout("waitdim the_nether", Duration::from_secs(90))?;
    p.wait(10)?;
    // leave the return portal towards the patch (around the frame, never back through it)
    let (fx, rx, rz) = (p.plan.fx, p.plan.rx, p.plan.rz);
    let mouth = if rx > fx { fx + 4 } else { fx - 1 };
    let around = if rx > fx { rx - 1 } else { rx + 4 };
    p.walk_path(&[(rx + 1, rz + 1), (around, rz + 1), (around, rz), (mouth, rz), (fx + 1, rz + 3)])?;
    Ok(())
}

/// Aim one trip from the Nether side: mine the previous blockers, then place cobblestone until
/// the predictor's unique winner is the target row's site.
fn prep_trip(p: &mut Prod, l: i32, row: i32) -> Result<()> {
    let fx = p.plan.fx;
    let rz = p.plan.rz;
    let exit: Pos = [fx + 1, l + 1, rz];
    let target: Pos = [fx + 1, l + 1, row];
    let mut blocks = p.nether_box([exit[0] - 19, 0, exit[2] - 19], [exit[0] + 19, 64, exit[2] + 19])?;
    for old in std::mem::take(&mut p.blockers) {
        if blocks.get(old).starts_with("minecraft:cobblestone") {
            p.walk(old[0], old[2] + 1)?;
            p.mine(old)?;
            blocks.set(old, "minecraft:air");
        }
    }
    let mut want: Vec<Pos> = Vec::new();
    for _ in 0..10 {
        let list = portal::sites(&blocks, exit, 0, 127);
        match list.first().copied() {
            Some(s) if s.origin == target && s.full => {
                match list.iter().find(|x| x.full && x.dist_sq == s.dist_sq && x.origin != target).copied() {
                    None => break,
                    Some(other) => place_blocker(p, &mut blocks, other, target, &mut want)?,
                }
            }
            Some(s) if s.origin == target => bail!("target site {target:?} is only partial (side rows not solid+air)"),
            Some(s) => place_blocker(p, &mut blocks, s, target, &mut want)?,
            None => bail!("no portal site at all around {exit:?} (target {target:?} not hostable)"),
        }
    }
    let win = portal::winner(&blocks, exit, 0, 127).context("predictor: no site")?;
    if win.origin != target || !win.full {
        bail!("predictor: winner {:?} full={} instead of target {target:?}", win.origin, win.full);
    }
    p.blockers.clone_from(&want);
    p.state.blockers = want;
    p.save_state()?;
    p.loga(&format!("layer {l} row {row}: blockers {:?}", p.blockers));
    Ok(())
}

/// The trip itself, from the drill room: walk in, arrive in the generated portal, step out,
/// verify over RCON that the row turned to obsidian.
fn do_trip(p: &mut Prod, l: i32, row: i32) -> Result<()> {
    let fx = p.plan.fx;
    let rz = p.plan.rz;
    p.walk(p.plan.ox + 1, p.plan.oz + 1)?;
    p.walk(p.plan.ox + 1, p.plan.oz)?;
    p.actor.cmd_timeout("waitdim the_nether", Duration::from_secs(90))?;
    // Out of the new portal first (so we don't bounce back), then verify.
    let side = if row >= rz { row + 1 } else { row - 1 };
    p.walk(fx + 1, side)?;
    p.assert_block("the_nether", [fx + 1, l + 1, row], "minecraft:nether_portal", "generated portal interior")?;
    for x in fx..=fx + 3 {
        p.assert_block("the_nether", [x, l, row], "minecraft:obsidian", "converted bedrock -> obsidian")?;
    }
    p.look(fx as f64 + 2.0, l as f64 + 0.4, row as f64 + 0.5, 10)?;
    p.wait(20)?;
    Ok(())
}

/// Pick a cell that kills `win` without touching `target`'s site, place cobblestone there.
fn place_blocker(p: &mut Prod, blocks: &mut Blocks, win: portal::Site, target: Pos, want: &mut Vec<Pos>) -> Result<()> {
    let target_cells: HashSet<Pos> = portal::air_cells(&portal::Site { origin: target, full: true, dist_sq: 0 }).into_iter().collect();
    let fx = p.plan.fx;
    let l = target[1] - 1;
    let cand = portal::air_cells(&win)
        .into_iter()
        .filter(|c| !target_cells.contains(c))
        .filter(|c| blocks.kind(*c) == Kind::Replaceable)
        .filter(|c| c[1] == l + 1 && (fx..=fx + 3).contains(&c[0]))
        .find(|c| blocks.kind([c[0], c[1] - 1, c[2]]) == Kind::Solid)
        .context("no placeable blocker cell")?;
    let feet_z = if cand[2] >= p.plan.rz { cand[2] + 1 } else { cand[2] - 1 };
    p.walk(cand[0], feet_z)?;
    p.select(SLOT_COBBLE)?;
    p.place([cand[0], cand[1] - 1, cand[2]], "UP")?;
    p.assert_block("the_nether", cand, "minecraft:cobblestone", "blocker placed")?;
    p.select(SLOT_PICK)?;
    blocks.set(cand, "minecraft:cobblestone");
    want.push(cand);
    Ok(())
}

/// Lower the overworld drill portal until its interior floor serves layer `l` (no-op when it
/// already does): mine its bottom row, dig the floor under it, rebuild the row one lower,
/// relight — once per layer.
fn lower_drill_portal(p: &mut Prod, l: i32) -> Result<()> {
    let (ox, oz) = (p.plan.ox, p.plan.oz);
    let mut served = p.state.drill_serves.unwrap_or(4);
    while served > l {
        let old_bottom = served; // the bottom row's y while serving `served` (interior at +1)
        p.loga(&format!("lowering the drill portal (bottom y {old_bottom} -> {})", old_bottom - 1));
        p.select(SLOT_PICK)?;
        p.walk(ox + 1, oz + 2)?;
        for x in ox..=ox + 3 {
            p.mine([x, old_bottom, oz])?;
        }
        for x in ox..=ox + 3 {
            p.mine([x, old_bottom - 1, oz])?;
        }
        p.select(SLOT_OBSIDIAN)?;
        for x in ox..=ox + 3 {
            p.walk(x, oz + 2)?;
            p.place([x, old_bottom - 2, oz], "UP")?;
        }
        for x in [ox, ox + 3] {
            p.place([x, old_bottom - 1, oz], "UP")?;
        }
        p.select(SLOT_FLINT)?;
        p.place([ox + 1, old_bottom - 1, oz], "UP")?;
        p.assert_block("overworld", [ox + 1, old_bottom, oz], "minecraft:nether_portal", "lowered drill portal lit")?;
        p.select(SLOT_PICK)?;
        served -= 1;
        p.state.drill_serves = Some(served);
        p.save_state()?;
    }
    Ok(())
}

/// Mine out everything solid in a finished layer's rows (converted obsidian + plain rock),
/// outside rows first so the player always stands on something.
fn mine_layer(p: &mut Prod, l: i32) -> Result<()> {
    let fx = p.plan.fx;
    let rz = p.plan.rz;
    for _ in 0..10 {
        let blocks = p.nether_box([fx, l, rz - l], [fx + 3, l, rz + l])?;
        let mut todo: Vec<Pos> = Vec::new();
        for z in rz - l..=rz + l {
            for x in fx..=fx + 3 {
                if blocks.kind([x, l, z]) == Kind::Solid {
                    todo.push([x, l, z]);
                }
            }
        }
        if todo.is_empty() {
            return Ok(());
        }
        todo.sort_by_key(|c| std::cmp::Reverse((c[2] - rz).abs()));
        p.select(SLOT_PICK)?;
        let wide = p.nether_box([fx - 3, l, rz - l - 3], [fx + 6, l + 2, rz + l + 3])?;
        let mut mined = 0;
        for c in todo {
            if p.stand_near(&wide, c).is_ok() {
                p.mine(c)?;
                mined += 1;
            }
        }
        if mined == 0 {
            bail!("layer {l}: no reachable stand in a whole pass");
        }
    }
    bail!("layer {l} still not mined out")
}

// ---- compose ----------------------------------------------------------------------------------

const YELLOW: [u8; 3] = [255, 255, 85];
const WHITE: [u8; 3] = [255, 255, 255];
const GRAY: [u8; 3] = [170, 170, 170];
const AQUA: [u8; 3] = [85, 255, 255];
const GREEN: [u8; 3] = [85, 255, 85];

fn ffmpeg_run(args: &[String]) -> Result<()> {
    let out = std::process::Command::new(mctas::video::ffmpeg()?).args(["-loglevel", "error", "-y"]).args(args).output()?;
    if !out.status.success() {
        bail!("ffmpeg {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

fn duration(path: &Path) -> Result<f64> {
    let probe = mctas::video::ffmpeg()?.with_file_name("ffprobe");
    let out = std::process::Command::new(probe)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()?;
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().with_context(|| format!("duration of {}", path.display()))
}

fn card(font: &mctas::mcfont::Font, bg: &mctas::mcfont::Image, w: u32, h: u32, lines: &[(String, u32, [u8; 3])]) -> mctas::mcfont::Image {
    let mut img = mctas::mcfont::Image::new(w, h, [0, 0, 0, 255]);
    img.tile(bg, 5, 0.25);
    let gap = 8i64;
    let total: i64 = lines.iter().map(|(_, s, _)| (mctas::mcfont::LINE_HEIGHT * s) as i64 + gap).sum::<i64>() - gap;
    let mut y = (h as i64 - total) / 2;
    for (text, s, color) in lines {
        for piece in font.wrap(text, *s, w - 80) {
            if !piece.is_empty() {
                font.draw_centered(&mut img, w as i64 / 2, y, &piece, *s, *color, true);
            }
            y += (mctas::mcfont::LINE_HEIGHT * s) as i64 + gap;
        }
    }
    img
}

fn label(font: &mctas::mcfont::Font, w: u32, h: u32, lines: &[String]) -> mctas::mcfont::Image {
    let mut img = mctas::mcfont::Image::new(w, h, [0, 0, 0, 0]);
    let s = 2u32;
    let pad = 10i64;
    let margin = 18i64;
    let max_w = (w as f64 * 0.92) as u32;
    let mut laid: Vec<String> = Vec::new();
    for t in lines {
        laid.extend(font.wrap(t, s, max_w - 2 * pad as u32));
    }
    let box_w = laid.iter().map(|t| font.width(t, s) as i64).max().unwrap_or(0) + 2 * pad;
    let box_h = (mctas::mcfont::LINE_HEIGHT * s) as i64 * laid.len() as i64 + 2 * pad - 2;
    let (x, y) = (margin, h as i64 - margin - box_h);
    img.fill_rect(x, y, box_w, box_h, [0, 0, 0, 160]);
    let mut ty = y + pad;
    for t in &laid {
        font.draw(&mut img, x + pad, ty, t, s, WHITE, true);
        ty += (mctas::mcfont::LINE_HEIGHT * s) as i64;
    }
    img
}

fn compose(version: &str, plan: &Plan, o: &VideoOptions) -> Result<()> {
    let timeline: Timeline = serde_json::from_str(&std::fs::read_to_string(o.out.join("timeline.json"))?)?;
    let font = mctas::mcfont::Font::minecraft(version)?;
    let netherrack = mctas::mcfont::texture(version, "block/netherrack")?;
    let work = o.out.join("work");
    std::fs::create_dir_all(&work)?;
    let (w, h, fps) = (o.width, o.height, o.fps);
    // Parts in order; `inp` counts ffmpeg inputs, each finished part is [p<its first input>].
    let mut inputs: Vec<String> = Vec::new();
    let mut graphs: Vec<String> = Vec::new();
    let mut outs: Vec<String> = Vec::new();
    let mut inp = 0usize;
    let mut total_secs = 0.0f64;
    let mut card_marks: Vec<(f64, String)> = Vec::new();
    macro_rules! add_card {
        ($name:expr, $secs:expr, $img:expr) => {{
            let png = work.join(format!("{}.png", $name));
            $img.save_png(&png)?;
            inputs.extend(["-loop".into(), "1".into(), "-framerate".into(), fps.to_string(), "-t".into(), format!("{:.2}", $secs), "-i".into(), png.display().to_string()]);
            graphs.push(format!("[{inp}:v]setsar=1,format=yuv420p,fade=t=in:st=0:d=0.3,fade=t=out:st={:.2}:d=0.3[p{inp}]", $secs - 0.3));
            outs.push(format!("[p{inp}]"));
            card_marks.push((total_secs + $secs / 2.0, $name.to_string()));
            total_secs += $secs;
            inp += 1;
        }};
    }
    let title = card(&font, &netherrack, w, h, &[
        ("Removing bedrock in survival:".into(), 4, WHITE),
        ("the nether portal method".into(), 6, YELLOW),
        ("".into(), 2, WHITE),
        ("Minecraft Java 26.3 · a real random world".into(), 3, AQUA),
        (format!("seed {} · Nether floor {}, {}", plan.seed, plan.fx + 1, plan.rz), 2, GRAY),
        ("survival mode · peaceful · no block is ever placed or broken by a command".into(), 2, GRAY),
    ]);
    add_card!("card-title", 5.0, &title);
    let why = card(&font, &netherrack, w, h, &[
        ("Why it works".into(), 6, YELLOW),
        ("".into(), 2, WHITE),
        ("Enter a portal with no portal near the exit (16 blocks in the Nether)".into(), 2, WHITE),
        ("and the game builds one: PortalForcer looks for a 4x3 patch —".into(), 2, WHITE),
        ("12 solid bottom cells (bedrock counts), air above —".into(), 2, WHITE),
        ("then writes the obsidian frame without checking what it overwrites.".into(), 2, WHITE),
        ("".into(), 2, WHITE),
        ("The middle row's 4 bedrock become obsidian. One 1x4 line per trip.".into(), 3, GREEN),
        ("Mine the obsidian, repeat, and drill layer by layer to the void.".into(), 2, WHITE),
    ]);
    add_card!("card-why", 8.0, &why);
    // Screen-time budgets (seconds) per segment kind: bulk mining is sped up as much as it
    // takes to fit (the recorded speed is the floor), so the cut lands at ~5-6 minutes.
    let budget = |file: &str| -> Option<f64> {
        let stem = file.splitn(2, '-').nth(1).unwrap_or(file).trim_end_matches(".mkv");
        Some(match stem {
            "intro" => 16.0,
            "dig-down" => 20.0,
            "patch-prep" => 32.0,
            "bedrock-before" => 12.0,
            "return-portal" => 32.0,
            "to-overworld" => 14.0,
            "overworld-dig" => 24.0,
            "drill-build" => 30.0,
            "first-trip" => 45.0,
            "first-mine" => 30.0,
            "second-trip" => 35.0,
            "finale" => 20.0,
            s if s.starts_with("mine-layer") => 10.0,
            s if s.starts_with("trip-") => 14.0,
            _ => return None,
        })
    };
    for seg in &timeline.segments {
        let path = o.out.join("segments").join(&seg.file);
        let raw = duration(&path)?;
        let speed = match budget(&seg.file) {
            Some(b) => seg.speed.max(raw / b),
            None => seg.speed,
        };
        let secs = raw / speed;
        let label_png = work.join(format!("{}.label.png", seg.file));
        label(&font, w, h, &seg.caption).save_png(&label_png)?;
        inputs.extend(["-i".into(), path.display().to_string(), "-i".into(), label_png.display().to_string()]);
        graphs.push(format!(
            "[{inp}:v]setpts=(PTS-STARTPTS)/{sp:.3},fps={fps},scale={w}:{h},setsar=1[s{inp}];[s{inp}][{lp}:v]overlay=0:0,format=yuv420p[p{inp}]",
            sp = speed, lp = inp + 1
        ));
        outs.push(format!("[p{inp}]"));
        total_secs += secs;
        inp += 2;
    }
    let limits = card(&font, &netherrack, w, h, &[
        ("Rules and limits".into(), 6, YELLOW),
        ("".into(), 2, WHITE),
        ("One 1x4 line per trip; a 1x4 hole through all 5 layers is about 20 trips".into(), 2, WHITE),
        (format!("(this spot needs {}). Mine the generated portal before every trip,", plan.trips), 2, WHITE),
        ("or the next trip just links to it.".into(), 2, WHITE),
        ("".into(), 2, WHITE),
        ("Cannot reach the Nether roof: the frame bottom is at most y=122,".into(), 2, WHITE),
        ("the roof starts at y=123.".into(), 2, WHITE),
        ("On the Overworld floor, nearer cave or surface sites usually win —".into(), 2, WHITE),
        ("the exit height is the Nether height, ~60 blocks above the floor.".into(), 2, WHITE),
        ("".into(), 2, WHITE),
        (format!("seed {} · patch x {}..{}, z {}..{} · github.com/vjeux/redstonepedia", plan.seed, plan.fx, plan.fx + 3, plan.rz - 4, plan.rz + 4), 2, GRAY),
    ]);
    add_card!("card-limits", 9.0, &limits);
    let n_parts = outs.len();
    let graph = format!("{};{}concat=n={n_parts}:v=1:a=0,scale=out_range=tv,format=yuv420p[o]", graphs.join(";"), outs.concat());
    let out_file = o.out.join("bedrock-portal-method.webm");
    let mut args = inputs;
    args.extend(["-filter_complex".into(), graph, "-map".into(), "[o]".into()]);
    args.extend([
        "-c:v".into(), "libvpx-vp9".into(), "-b:v".into(), "0".into(), "-crf".into(), "30".into(), "-row-mt".into(), "1".into(),
        "-deadline".into(), "good".into(), "-cpu-used".into(), "4".into(), "-pix_fmt".into(), "yuv420p".into(), "-color_range".into(), "tv".into(), out_file.display().to_string(),
    ]);
    ffmpeg_run(&args)?;
    let final_secs = duration(&out_file)?;
    // Verification stills: every card middle + a spread across the video.
    let stills = o.out.join("stills");
    std::fs::create_dir_all(&stills)?;
    let mut times: Vec<(f64, String)> = card_marks;
    for i in 0..12 {
        times.push((final_secs * (i as f64 + 0.5) / 12.0, format!("t{i:02}")));
    }
    for (t, name) in &times {
        ffmpeg_run(&["-ss".into(), format!("{t:.2}"), "-i".into(), out_file.display().to_string(), "-frames:v".into(), "1".into(), "-vf".into(), "scale=960:-2".into(), "-q:v".into(), "3".into(), stills.join(format!("{name}.jpg")).display().to_string()])?;
    }
    let probe = mctas::video::ffmpeg()?.with_file_name("ffprobe");
    let meta = std::process::Command::new(probe).args(["-v", "error", "-show_entries", "stream=codec_name,color_range,pix_fmt,width,height", "-of", "default=nw=1"]).arg(&out_file).output()?;
    println!("composed {} ({:.1}s, {} KiB)\n{}", out_file.display(), final_secs, std::fs::metadata(&out_file)?.len() / 1024, String::from_utf8_lossy(&meta.stdout).trim());
    println!("stills in {}", stills.display());
    Ok(())
}
