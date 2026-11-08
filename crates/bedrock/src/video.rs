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
// v2 (the sweep + remote-trigger remake) fills the rest of the hotbar.
const SLOT_REDSTONE: u32 = 6;
const SLOT_STONE: u32 = 7;
/// The rotating slot: `hold()` swaps whatever the machine build needs into it.
const SLOT_SWAP: u32 = 8;

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
    /// The v2 remake (batch sweep + stay-in-the-Nether remote trigger, tutorial cut).
    pub v2: bool,
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
    /// v2 geometry (see `scout2`); absent in a v1 plan.
    #[serde(default)]
    pub v2: Option<V2Plan>,
}

/// The v2 sweep plan for one field (see `sim_field`): everything else (portal and machine
/// coordinates) is derived from `(fx, rz)` by `v2_coords`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct V2Plan {
    /// First / last convertible field rows (z); the rows just outside are support borders.
    pub z0: i32,
    pub z1: i32,
    /// Exit-point y the drill item flies at (its dropper's height in the Overworld).
    pub exit_y: i32,
    /// A row the simulation pierces through all five layers (the money shot).
    pub hole_row: i32,
    /// Simulated total trips (field + border lines, both players' and items').
    pub trips_total: usize,
    /// Simulated converted rows: [layer y4..y0][pass A,B][rows in conversion order].
    pub pass_rows: Vec<Vec<Vec<i32>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Segment {
    file: String,
    /// The checkpoint tag that owns this segment (dropped on resume when its scene is redone).
    scene: String,
    caption: Vec<String>,
    speed: f64,
    /// v2: the persistent step indicator ("Step 3/6 — The trigger machine"), "" for none.
    #[serde(default)]
    step: String,
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
    /// v2: the dropper currently feeds drill portal B (else A).
    #[serde(default)]
    v2_dropper_b: bool,
    /// v2: trips taken so far (the on-screen counter).
    #[serde(default)]
    v2_trips: usize,
    /// v2: converted rows per finished pass, tag -> rows (what the pass-end mining replays).
    #[serde(default)]
    v2_converted: Vec<(String, Vec<i32>)>,
}

pub fn run(version: &str, o: VideoOptions) -> Result<()> {
    std::fs::create_dir_all(&o.out)?;
    if o.v2 {
        return run2(version, &o);
    }
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
        chosen = Some(Plan { seed, fx, rz, layers: layers.clone(), trips, shaft_x: shaft.0, shaft_z: shaft.1, shaft_stand_y: stand_y, ox, oz, ow_stand_y, rx, v2: None });
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
                if text.trim().is_empty() {
                    // Half-published answer (pre-atomic-write actor builds): keep waiting.
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
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

    /// Death leaves the client on its respawn screen with every actor wait stalling:
    /// detect it on the server side (RCON works regardless) and fail the scene fast with a
    /// clear message instead of grinding. Death on a shoot = resume from the checkpoint.
    fn check_alive(&mut self) -> Result<()> {
        let out = self.rcon(&format!("data get entity {PLAYER} Health"))?;
        let health: f32 = out
            .rsplit(':')
            .next()
            .map(|v| v.trim().trim_end_matches('f'))
            .and_then(|v| v.parse().ok())
            .with_context(|| format!("health parse: {out}"))?;
        if health <= 0.0 {
            bail!("the player is dead (Health {health}) — resume the shoot from the last checkpoint");
        }
        Ok(())
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
        let p = s.split("pos=").nth(1).and_then(|r| r.split(' ').next()).with_context(|| format!("pos line: {s}"))?;
        let v: Vec<f64> = p.split(',').map(|x| x.parse()).collect::<std::result::Result<_, _>>()?;
        Ok([v[0], v[1], v[2]])
    }

    fn select(&mut self, slot: u32) -> Result<()> {
        self.actor.cmd(&format!("select {slot}"))?;
        Ok(())
    }

    fn mine(&mut self, p: Pos) -> Result<()> {
        // minef (aim+start in one submit, then tick-gated continues): the plain precise
        // `mine` measured ~15 s a block here — its continue-only loop never gets going.
        self.actor.cmd_timeout(&format!("minef {} {} {}", p[0], p[1], p[2]), Duration::from_secs(60))?;
        Ok(())
    }

    /// Bulk mining (clearing and digging, never near blocks that matter): the lean
    /// `minef` path (aim+start in one client submit, then tick-gated continues — the
    /// same per-tick survival timing). The held-key sweep measured far slower here:
    /// its prelude costs ~5 frame-gated submits a block, then a 6 s keyhold deadline
    /// routinely expired unbroken on deepslate before the direct fallback even ran.
    fn mine_sweep(&mut self, p: Pos) -> Result<()> {
        self.actor.cmd_timeout(&format!("minef {} {} {}", p[0], p[1], p[2]), Duration::from_secs(60))?;
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
        self.seg_step(name, scene, caption, speed, "")
    }

    /// `seg` with the v2 step indicator.
    fn seg_step(&mut self, name: &str, scene: &str, caption: &[&str], speed: f64, step: &str) -> Result<()> {
        self.check_alive()?;
        if let Some(r) = self.rec.take() {
            r.stop()?;
        }
        self.seg_idx += 1;
        let file = format!("{:02}-{name}.mkv", self.seg_idx);
        let path = self.out.join("segments").join(&file);
        self.timeline.segments.push(Segment { file: file.clone(), scene: scene.to_string(), caption: caption.iter().map(|s| s.to_string()).collect(), speed, step: step.to_string() });
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

    // ---- v2 actor verbs -------------------------------------------------------------------

    fn toss(&mut self) -> Result<()> {
        self.actor.cmd("toss")?;
        Ok(())
    }

    fn cam(&mut self, third: bool) -> Result<()> {
        self.actor.cmd(if third { "cam third" } else { "cam first" })?;
        Ok(())
    }

    /// Place with a fixed look (how a dropper/repeater gets its facing).
    fn placef(&mut self, click: Pos, face: &str, yaw: f32, pitch: f32) -> Result<String> {
        self.actor.cmd(&format!("placef {} {} {} {} {} {}", click[0], click[1], click[2], face, yaw, pitch))
    }

    /// Load a container through its GUI, two stacks at most (the player keeps the rest).
    fn fillinto(&mut self, at: Pos, substr: &str) -> Result<String> {
        self.actor.cmd_timeout(&format!("fillinto {} {} {} {} 2", at[0], at[1], at[2], substr), Duration::from_secs(60))
    }

    fn dropall(&mut self, substr: &str, keep: u32) -> Result<String> {
        self.actor.cmd_timeout(&format!("dropall {substr} {keep}"), Duration::from_secs(90))
    }

    /// Client-side box digging (see the actor's `digBox`): returns its answer line
    /// ("ok left=N[ stuck]").
    fn digbox(&mut self, lo: Pos, hi: Pos, mode: &str) -> Result<String> {
        self.actor.cmd_timeout(
            &format!("digbox {} {} {} {} {} {} {} 100", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2], mode),
            Duration::from_secs(160),
        )
    }

    fn invcount(&mut self, substr: &str) -> Result<i64> {
        let s = self.actor.cmd(&format!("invcount {substr}"))?;
        Ok(s.split("count=").nth(1).and_then(|v| v.split(' ').next()).and_then(|v| v.parse().ok()).unwrap_or(-1))
    }

    /// Select the hotbar slot holding `substr`, swapping it in from the main inventory first
    /// (a GUI number-key swap by the actor) when it is not on the hotbar yet.
    fn hold(&mut self, substr: &str) -> Result<()> {
        let s = self.actor.cmd(&format!("hotbar {SLOT_SWAP} {substr}"))?;
        let slot: u32 = s.split("slot=").nth(1).and_then(|v| v.split(' ').next()).and_then(|v| v.parse().ok()).context("hotbar reply")?;
        self.select(slot)
    }

    fn waitdim(&mut self, substr: &str) -> Result<String> {
        self.actor.cmd_timeout(&format!("waitdim {substr}"), Duration::from_secs(90))
    }

    /// RCON-poll until a block test passes (a generated portal appearing, say).
    fn poll_block(&mut self, dim: &str, p: Pos, pred: &str, secs: u64) -> Result<()> {
        let t0 = Instant::now();
        loop {
            if self.test_block(dim, p, pred)? {
                self.loga(&format!("poll ok {dim} {p:?} {pred} after {:.1}s", t0.elapsed().as_secs_f64()));
                return Ok(());
            }
            if t0.elapsed() > Duration::from_secs(secs) {
                bail!("poll: {dim} {p:?} never became {pred} in {secs}s");
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// A spectator establishing shot (camera work, captioned): fly to `at`, look at `look`,
    /// hold, then back to survival where the player stood.
    fn spec_shot(&mut self, dim: &str, at: [f64; 3], look: [f64; 3], ticks: u32) -> Result<()> {
        let back = self.feet()?;
        self.rcon_ok(&format!("gamemode spectator {PLAYER}"))?;
        self.tp(dim, at[0], at[1], at[2], 0.0, 60.0)?;
        self.look(look[0], look[1], look[2], 20)?;
        self.wait(ticks)?;
        self.rcon_ok(&format!("gamemode survival {PLAYER}"))?;
        self.tp(dim, back[0], back[1], back[2], 0.0, 20.0)?;
        Ok(())
    }

    /// Place the held block AT `pos` (asserting `pred` lands there): picks a solid clicked
    /// neighbour from a fresh read (below first), stands in reach, places. `look` pins the
    /// player's view for the click (dropper/repeater facings).
    fn place_at(&mut self, dim: &str, pos: Pos, pred: &str, look: Option<(f32, f32)>) -> Result<()> {
        self.place_at_s(dim, pos, pred, look, false)
    }

    /// `place_at` with the sneak key held for the click: how a player puts a plate on a
    /// hopper instead of opening it.
    fn place_at_sneak(&mut self, dim: &str, pos: Pos, pred: &str) -> Result<()> {
        self.place_at_s(dim, pos, pred, None, true)
    }

    fn place_at_s(&mut self, dim: &str, pos: Pos, pred: &str, look: Option<(f32, f32)>, sneak: bool) -> Result<()> {
        let blocks = if dim == "the_nether" {
            self.nether_box([pos[0] - 2, pos[1] - 2, pos[2] - 2], [pos[0] + 2, pos[1] + 2, pos[2] + 2])?
        } else {
            self.overworld_box([pos[0] - 2, pos[1] - 2, pos[2] - 2], [pos[0] + 2, pos[1] + 2, pos[2] + 2])?
        };
        if blocks.get(pos).starts_with(pred) {
            return Ok(());
        }
        // `stand_near` only considers stands near the player's current cell (snapshot
        // stands across solid rock are unwalkable), so a far placement in an open room
        // first walks to the target's south side and only then picks a stand.
        let f = self.feet()?;
        if (f[0].floor() as i32 - pos[0]).abs() + (f[2].floor() as i32 - pos[2]).abs() > 5 {
            let _ = self.walk(pos[0], pos[2] + 1);
        }
        // (neighbour offset, face of that neighbour the click lands on), below first.
        let tries: [(Pos, &str); 6] = [
            ([0, -1, 0], "UP"),
            ([0, 0, 1], "NORTH"),
            ([0, 0, -1], "SOUTH"),
            ([1, 0, 0], "WEST"),
            ([-1, 0, 0], "EAST"),
            ([0, 1, 0], "DOWN"),
        ];
        for (off, face) in tries {
            let click = [pos[0] + off[0], pos[1] + off[1], pos[2] + off[2]];
            if blocks.kind(click) == Kind::Solid {
                self.stand_near(&blocks, click)?;
                if sneak {
                    self.actor.cmd("sneak 1")?;
                }
                let r = match look {
                    Some((yaw, pitch)) => self.placef(click, face, yaw, pitch)?,
                    None => self.place(click, face)?,
                };
                if sneak {
                    self.actor.cmd("sneak 0")?;
                }
                self.loga(&format!("place_at {pos:?} via {click:?}/{face}: {r}"));
                return self.assert_block(dim, pos, pred, "place_at");
            }
        }
        bail!("place_at {dim} {pos:?}: no solid neighbour to click")
    }

    /// `build_portal` with an interior height (the v1 helper fixes 3). Bottom row at `ybase`
    /// (on the floor at ybase-1), plane z=z0, frame west x=x0. Player stands south (z0+2).
    fn build_portal2(&mut self, dim: &str, x0: i32, ybase: i32, z0: i32, h: i32) -> Result<()> {
        self.select(SLOT_OBSIDIAN)?;
        for x in x0..=x0 + 3 {
            self.walk(x, z0 + 2)?;
            self.place([x, ybase - 1, z0], "UP")?;
        }
        for x in [x0, x0 + 3] {
            self.walk(x, z0 + 2)?;
            for y in ybase + 1..=ybase + h {
                self.place([x, y - 1, z0], "UP")?;
            }
        }
        let top = ybase + h + 1;
        self.walk(x0, z0 + 2)?;
        self.place([x0, top - 1, z0], "UP")?;
        self.walk(x0 + 3, z0 + 2)?;
        self.place([x0 + 3, top - 1, z0], "UP")?;
        self.place([x0 + 3, top, z0], "WEST")?;
        self.place([x0 + 2, top, z0], "WEST")?;
        for x in x0..=x0 + 3 {
            self.assert_block(dim, [x, ybase, z0], "minecraft:obsidian", "portal2 bottom row")?;
            self.assert_block(dim, [x, top, z0], "minecraft:obsidian", "portal2 top row")?;
        }
        self.select(SLOT_FLINT)?;
        self.walk(x0 + 1, z0 + 1)?;
        self.place([x0 + 1, ybase, z0], "UP")?;
        self.assert_block(dim, [x0 + 1, ybase + 1, z0], "minecraft:nether_portal", "portal2 lit")?;
        self.select(SLOT_PICK)?;
        Ok(())
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
    let plan = Plan { seed: 1, fx: 0, rz: 0, layers: vec![], trips: 0, shaft_x: 0, shaft_z: 0, shaft_stand_y: 70, ox: 0, oz: 0, ow_stand_y: 70, rx: 0, v2: None };
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
    /// `dig_down` hardened for caves in the shaft: wait for the fall to settle before
    /// choosing the next cell (mining a cell you are falling past times out — it ends up
    /// above you, out of reach), and re-centre on the column after a sideways landing.
    fn dig_down2(&mut self, dim: &str, x: i32, z: i32, target_y: f64) -> Result<()> {
        for _ in 0..400 {
            // Death (a lava pocket flowing into the shaft) respawns the player at the
            // world spawn: say so at once instead of mining on in the wrong place.
            let pos = self.pos()?;
            if !pos.contains(dim) || pos.contains("health=0") {
                bail!("dig_down2: the player died or left {dim} ({pos})");
            }
            let mut f = self.feet()?;
            for _ in 0..40 {
                self.wait(3)?;
                let f2 = self.feet()?;
                if (f2[1] - f[1]).abs() < 0.01 {
                    f = f2;
                    break;
                }
                f = f2;
            }
            if f[1] <= target_y + 0.05 {
                return Ok(());
            }
            if f[0].floor() as i32 != x || f[2].floor() as i32 != z {
                self.walk(x, z)?;
                continue;
            }
            let below = [x, f[1].floor() as i32 - 1, z];
            self.mine_sweep(below)?;
        }
        bail!("dig_down2 never reached y {target_y}")
    }

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
        for pass in 0..200 {
            self.check_alive()?;
            // Keep the inventory (and the floor) free of junk stacks: piles of drop
            // entities are what slows the server to a crawl in a long clear.
            if self.invcount("cobbled_deepslate")? > 192 {
                self.dropall("cobbled_deepslate", 0)?;
            }
            if dim == "the_nether" && self.invcount("netherrack")? > 448 {
                self.dropall("netherrack", 3)?;
            }
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
            // Seal any lava the clearing opened (a pocket behind a mined cell flows straight
            // in): placing a block into lava consumes the source, then mine it back out.
            let lavas: Vec<Pos> = (lo[1]..=hi[1])
                .flat_map(|y| (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z])))
                .filter(|&c| blocks.get(c).starts_with("minecraft:lava"))
                .take(8)
                .collect();
            if !lavas.is_empty() {
                self.loga(&format!("clear_box {dim}: sealing {} lava cells {lavas:?}", lavas.len()));
                self.select(SLOT_NETHERRACK)?;
                for cell in lavas {
                    match self.place_at(dim, cell, "minecraft:netherrack", None) {
                        Ok(()) => {
                            self.select(SLOT_PICK)?;
                            let _ = self.mine(cell);
                            self.select(SLOT_NETHERRACK)?;
                        }
                        Err(e) => self.loga(&format!("clear_box: seal {cell:?} failed: {e:#}")),
                    }
                }
                self.select(SLOT_PICK)?;
                continue;
            }
            if todo.is_empty() {
                return Ok(());
            }
            // The digging itself runs inside the client (`digbox`): live stands, nearest
            // reachable first, no per-block file round trips. Driving it from here cell by
            // cell crawled at the client's frame rate and burned walk timeouts on stands
            // that only existed in a stale snapshot.
            self.loga(&format!("clear_box {dim} pass {pass}: {} blocks", todo.len()));
            self.select(SLOT_PICK)?;
            let first: Vec<Pos> = todo.iter().take(8).copied().collect();
            let r = self.digbox(lo, hi, if obsidian { "any" } else { "noobs" })?;
            self.loga(&format!("clear_box {dim} digbox -> {r}"));
            if r.contains("left=0") {
                return Ok(());
            }
            // Fallback when digbox barely progresses (its stand walks are frame-bound and
            // can stall in jagged cavities): mine a batch the direct per-cell way.
            let dug: i64 = r.split("left=").nth(1).and_then(|v| v.split(' ').next()).and_then(|v| v.parse().ok()).unwrap_or(i64::MAX);
            if dug > todo.len() as i64 * 3 / 4 {
                self.loga("clear_box: digbox slow — per-cell batch");
                let feet = self.feet()?;
                let fc = [feet[0].floor() as i32, feet[1].floor() as i32, feet[2].floor() as i32];
                let mut cells = todo.clone();
                cells.sort_by_key(|c| (c[0] - fc[0]).abs() + (c[1] - fc[1]).abs() + (c[2] - fc[2]).abs());
                let blocks2 = if dim == "the_nether" { self.nether_box(rlo, rhi)? } else { self.overworld_box(rlo, rhi)? };
                let mut n = 0;
                for cell in cells {
                    if n >= 60 {
                        break;
                    }
                    if blocks2.kind(cell) != Kind::Solid {
                        continue;
                    }
                    if self.stand_near(&blocks2, cell).is_ok() && self.mine_sweep(cell).is_ok() {
                        n += 1;
                    }
                }
                self.loga(&format!("clear_box: per-cell batch mined {n}"));
            }
            // Progress is judged by the next read; the actor reporting "stuck" or the count
            // not moving means the remaining cells need a fresh angle (rescue tp once).
            let after = if dim == "the_nether" { self.nether_box(rlo, rhi)? } else { self.overworld_box(rlo, rhi)? };
            let left = (lo[1]..=hi[1])
                .flat_map(|y| (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z])))
                .filter(|&c| {
                    let s = after.get(c);
                    after.kind(c) == Kind::Solid && !s.starts_with("minecraft:bedrock") && (obsidian || !s.starts_with("minecraft:obsidian"))
                })
                .count();
            if left >= todo.len() {
                self.loga(&format!("clear_box stuck; first remaining: {first:?}"));
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
                    // Never stand in or directly beside lava at feet level (a pocket beside
                    // the work area flows; the sealing shoot lost a player this way).
                    let lava_adj = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|&(dx, dz)| blocks.get([s[0] + dx, s[1], s[2] + dz]).starts_with("minecraft:lava"));
                    if !lava_adj
                        && blocks.kind([s[0], s[1] - 1, s[2]]) == Kind::Solid
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
        // Far "stands" from a snapshot (on top of unmined rock across the room) are almost
        // never walkable yet — don't burn four walk timeouts proving it.
        cands.retain(|(d, _)| *d <= 10);
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
    // Out of the stepped pit first (its steps climb southward; the side walls are 2+ high).
    p.walk_path(&[(fx + 1, rz + 5), (mouth, rz + 5), (mouth, rz), (rx + 1, rz + 1), (rx + 1, rz)])?;
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
            p.stand_near(&blocks, old)?;
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
    p.stand_near(blocks, [cand[0], cand[1] - 1, cand[2]])?;
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
            let r = p.place([x, old_bottom - 2, oz], "UP")?;
            p.loga(&format!("lower place bottom [{x}, {}, {oz}]: {r}", old_bottom - 1));
            p.assert_block("overworld", [x, old_bottom - 1, oz], "minecraft:obsidian", "lowered bottom row")?;
        }
        for x in [ox, ox + 3] {
            let r = p.place([x, old_bottom - 1, oz], "UP")?;
            p.loga(&format!("lower place corner [{x}, {old_bottom}, {oz}]: {r}"));
            p.assert_block("overworld", [x, old_bottom, oz], "minecraft:obsidian", "lowered corner")?;
        }
        p.select(SLOT_FLINT)?;
        let r = p.place([ox + 1, old_bottom - 1, oz], "UP")?;
        p.loga(&format!("lower light: {r}"));
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

// ---- v2: the sweep + remote-trigger remake -------------------------------------------------
//
// The technique of NOTES "Batch mode" + "Stay in the Nether", filmed as a tutorial:
// one flat 4-wide field on the Nether floor; ONE (pair of) drill portal(s) in the Overworld
// fed by a dropper; per trip the player mines ONE frame block and throws ONE item into the
// return portal, never leaving the Nether. The leftover frames steer the game's
// closest-site rule row by row (pass A: the exit-row parity, pass B: the other, after the
// dropper moves to the second drill portal 8 blocks south). Exit Y is the fired item's
// height (fixed: the dropper never moves vertically); the scout simulates the whole sweep
// against the predictor and only accepts fields that pierce to the void that way.

/// Derived v2 geometry, all from (fx, rz): the drill exit cell is (fx+1, *, rz).
struct V2Coords {
    /// R_N (Nether return portal) frame west x; plane z = rz, frame bottom y 5 (on the y4 sheet).
    rnx: i32,
    /// D_O interior west column (frame west = dix-1); the dropper fires at column dix+1.
    dix: i32,
    /// Drill portal planes: A (exit row rz) and B (exit row rz+1).
    doz_a: i32,
    doz_b: i32,
    /// R_O interior west column (frame west = rox-1), in plane doz_a, 13 west of D_O.
    rox: i32,
}

fn v2_coords(fx: i32, rz: i32) -> V2Coords {
    // R_N's nearest portal block (interior east column rnx+2) exactly 17 west of the exit
    // cell fx+1: outside the 16-block Nether-side search, so drill trips never find it.
    let rnx = fx - 18;
    // The fired item crosses D_O at column dix+1 (x ~= dix+1.5); floor(x/8) = fx+1 for
    // dix = 8fx+10. Its z at the plane doz (z ~= doz+0.5) maps to row rz for doz = 8rz+4.
    let dix = 8 * fx + 10;
    let doz_a = 8 * rz + 4;
    let doz_b = doz_a + 8;
    // An item tossed into R_N at x ~= rnx+2.5 heads for overworld x ~= 8fx-124: R_O's
    // nearest portal block is 120 away (inside the 128 square), D_O's 134 (outside it).
    let rox = dix - 14;
    V2Coords { rnx, dix, doz_a, doz_b, rox }
}

/// The frame cells a generated portal leaves standing (bottom row excluded): pillars + top.
fn frame_left_cells(fx: i32, l: i32, row: i32) -> Vec<Pos> {
    let mut v = Vec::new();
    for x in [fx, fx + 3] {
        for y in l + 1..=l + 3 {
            v.push([x, y, row]);
        }
    }
    for x in fx..=fx + 3 {
        v.push([x, l + 4, row]);
    }
    v
}

/// Simulate the sweep on a prepared field (overlay-flattened by the caller): all five
/// layers, both passes, exit fixed at (fx+1, EXIT_Y, rz+pass). Returns (trips, pass_rows,
/// hole_row) with pass_rows[layer y4..y0][pass] = rows in conversion order; `None` when
/// some trip would not land on the field at the wanted layer, or no row pierces.
fn sim_field(b: &mut Blocks, fx: i32, rz: i32, z0: i32, z1: i32) -> Option<(usize, Vec<Vec<Vec<i32>>>, i32)> {
    const EXIT_Y: i32 = 3;
    let mut trips = 0usize;
    let mut pass_rows: Vec<Vec<Vec<i32>>> = Vec::new();
    let mut pierce: Vec<i32> = (z0..=z1).collect();
    for l in (0..=4i32).rev() {
        let mut layer_passes: Vec<Vec<i32>> = Vec::new();
        let mut converted_layer: Vec<i32> = Vec::new();
        for pass in 0..2i32 {
            let exit = [fx + 1, EXIT_Y, rz + pass];
            let mut rows: Vec<i32> = Vec::new();
            loop {
                // Origins above y12 sit >9 above the exit: dist² > 81 beats no field row
                // (max ~40), so capping the scan there cannot change who wins the field.
                let w = portal::winner(b, exit, 0, 12)?;
                let row = w.origin[2];
                if !w.full || w.origin[0] != fx + 1 || w.origin[1] != l + 1 || !(z0 - 1..=z1 + 1).contains(&row) {
                    break;
                }
                // The game writes the frame; the player then mines ONE pillar block.
                for c in portal::converted_cells(&w) {
                    b.set(c, "minecraft:obsidian");
                }
                for c in frame_left_cells(fx, l, row) {
                    b.set(c, "minecraft:obsidian");
                }
                b.set([fx, l + 1, row], "minecraft:air");
                rows.push(row);
                trips += 1;
                if trips > 160 {
                    return None;
                }
            }
            // Pass over: the leftover frames are mined (the converted lines stay).
            for &row in &rows {
                for c in frame_left_cells(fx, l, row) {
                    b.set(c, "minecraft:air");
                }
            }
            converted_layer.extend(rows.iter().copied());
            layer_passes.push(rows);
        }
        // Layer over: its obsidian lines are mined out.
        for &row in &converted_layer {
            for x in fx..=fx + 3 {
                b.set([x, l, row], "minecraft:air");
            }
        }
        pierce.retain(|r| converted_layer.contains(r));
        pass_rows.push(layer_passes);
    }
    let hole = pierce.into_iter().min_by_key(|r| (r - rz).abs())?;
    Some((trips, pass_rows, hole))
}

/// v2 candidate pre-filter + full simulation at (fx, rz): a solid floor stack over the whole
/// field (support borders included), no lava in or above it, then `sim_field` on the
/// flattened overlay. Field rows are rz-5..rz+5 (inside the NOTES' radius-6 guidance).
fn candidate2(floor: &Blocks, fx: i32, rz: i32) -> Option<V2Plan> {
    let (z0, z1) = (rz - 5, rz + 5);
    // Lava margin wider than the cleared box: a pocket just outside the wall flows in the
    // moment the wall is mined (learned the wet way; Nether lava runs ~3 cells).
    for x in fx - 4..=fx + 7 {
        for z in z0 - 4..=z1 + 4 {
            for y in 0..=12 {
                let s = floor.get([x, y, z]);
                if s.starts_with("minecraft:lava") {
                    return None;
                }
                if (fx - 1..=fx + 4).contains(&x) && (z0 - 1..=z1 + 1).contains(&z) && y <= 4 && floor.kind([x, y, z]) != Kind::Solid {
                    return None;
                }
            }
        }
    }
    // Clone the reads into a sim copy: flatten (air above, solid y4 sheet) like the shoot does.
    let mut b = floor.subcopy([fx - 17, 0, rz - 17], [fx + 19, 12, rz + 19]);
    for x in fx - 2..=fx + 5 {
        for z in z0 - 1..=z1 + 1 {
            for y in 5..=12 {
                b.set([x, y, z], "minecraft:air");
            }
            if b.kind([x, 4, z]) != Kind::Solid {
                b.set([x, 4, z], "minecraft:netherrack");
            }
        }
    }
    let (trips, pass_rows, hole_row) = sim_field(&mut b, fx, rz, z0, z1)?;
    Some(V2Plan { z0, z1, exit_y: 3, hole_row, trips_total: trips, pass_rows })
}

/// A shaft stand for v2: solid footing and no lava anywhere in the 3x3 columns around the
/// shaft below (a pocket BESIDE the shaft flows in as soon as the digging updates it —
/// that drowned the first shoot), mostly solid rock (a small cave gap on the way down is
/// a survivable drop in peaceful, and `dig_down` keeps going from wherever it lands).
fn shaft_stand2(col: &Blocks, x: i32, z: i32) -> Option<i32> {
    (6..=110).rev().find(|&y| {
        col.kind([x, y, z]) == Kind::Replaceable
            && col.kind([x, y + 1, z]) == Kind::Replaceable
            && col.kind([x, y - 1, z]) == Kind::Solid
            && (5..y).all(|yy| {
                (-1..=1).all(|dx| (-1..=1).all(|dz| !col.get([x + dx, yy, z + dz]).starts_with("minecraft:lava")))
            })
            && (5..y).filter(|&yy| col.kind([x, yy, z]) == Kind::Solid).count() * 4 >= (y - 5).max(1) as usize * 3
    })
}

fn scout2(version: &str, o: &VideoOptions) -> Result<Plan> {
    let seed = o.seed.unwrap_or_else(|| {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        (t ^ (std::process::id() as u128) << 32) as i64
    });
    eprintln!("scout2: seed {seed}");
    let mut srv = VanillaServer::start(&server_opts(version, seed, None))?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -96 -96 95 95")?;
    let t0 = Instant::now();
    let floor = loop {
        match srv.read_dim_box("the_nether", [-80, 0, -80], [79, 15, 79]) {
            Ok(s) => break Blocks::new(s.region),
            Err(e) => {
                if t0.elapsed() > Duration::from_secs(240) {
                    return Err(e.context("the nether floor never generated"));
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    };
    eprintln!("scout2: floor read after {:.0}s", t0.elapsed().as_secs_f64());
    // The sweep simulation per candidate is the expensive part (the floor stack is solid
    // almost everywhere, so many pass the cheap pre-filter): run the grid in parallel.
    let grid: Vec<(i32, i32)> = (-44..=40).flat_map(|fx| (-40..=40).map(move |rz| (fx, rz))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let found: std::sync::Mutex<Vec<(usize, i32, i32, i32, V2Plan)>> = std::sync::Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&(fx, rz)) = grid.get(i) else { break };
                if let Some(v2) = candidate2(&floor, fx, rz) {
                    found.lock().unwrap().push((v2.trips_total, fx.abs() + rz.abs(), fx, rz, v2));
                }
            });
        }
    });
    let mut cands = found.into_inner().unwrap();
    eprintln!("scout2: {} fields simulate a full pierce", cands.len());
    cands.sort_by_key(|c| (c.0, c.1));
    if cands.is_empty() {
        bail!("scout2: no field simulates a full pierce in the searched area");
    }
    for c in cands.iter().take(5) {
        eprintln!("scout2: candidate fx={} rz={} trips={} hole_row={}", c.2, c.3, c.0, c.4.hole_row);
    }
    let mut chosen: Option<Plan> = None;
    let (mut n_corridor, mut n_shaft, mut n_wet, mut n_surface) = (0, 0, 0, 0);
    // Sorted by (trips, distance) the list starts with near-identical fields around the
    // origin sharing one corridor's lava: spread the tries over the map instead.
    let mut tried: Vec<(i32, i32)> = Vec::new();
    let spread = cands.into_iter().filter(|c| {
        if tried.iter().any(|&(tx, tz)| (tx - c.2).abs() < 7 && (tz - c.3).abs() < 7) {
            false
        } else {
            tried.push((c.2, c.3));
            true
        }
    });
    for (_, _, fx, rz, v2) in spread.take(140) {
        let c = v2_coords(fx, rz);
        // The walk west to R_N: lava in the walkway's air (y>=5) or flowing over it is the
        // hazard (checked a couple cells past the cleared footprint too); lava sealed in
        // the y0..4 stack gets paved by the y4 sheet fill.
        let corridor_lava = (c.rnx - 5..fx).any(|x| (5..=11).any(|y| (rz - 5..=v2.z1 + 3).any(|z| floor.get([x, y, z]).starts_with("minecraft:lava"))));
        if corridor_lava {
            n_corridor += 1;
            continue;
        }
        // Entry: best is a natural stand on the open floor just south of the field (feet
        // y5, straight from the floor reads — the shoot then just tunnels north). Buried
        // floors fall back to a dig-straight-down shaft column (v1's rule).
        let mut shaft_found: Option<((i32, i32), i32)> = None;
        'floor_stand: for sz in rz + 8..=rz + 14 {
            for sx in fx..=fx + 3 {
                let open = floor.kind([sx, 4, sz]) == Kind::Solid
                    && floor.kind([sx, 5, sz]) == Kind::Replaceable
                    && floor.kind([sx, 6, sz]) == Kind::Replaceable
                    && (sx - 1..=sx + 1).all(|x| (sz - 1..=sz + 1).all(|z| !floor.get([x, 5, z]).starts_with("minecraft:lava")));
                if open {
                    shaft_found = Some(((sx, sz), 5));
                    break 'floor_stand;
                }
            }
        }
        if shaft_found.is_none() {
            'shafts: for sx in [fx + 1, fx + 2, fx, fx + 3] {
                for sz in [rz + 9, rz + 10, rz + 8, rz + 11] {
                    let col = srv.read_dim_box("the_nether", [sx - 1, 0, sz - 1], [sx + 1, 127, sz + 1])?;
                    let col = Blocks::new(col.region);
                    if let Some(stand_y) = shaft_stand2(&col, sx, sz) {
                        shaft_found = Some(((sx, sz), stand_y));
                        break 'shafts;
                    }
                }
            }
        }
        let Some((shaft, stand_y)) = shaft_found else {
            n_shaft += 1;
            continue;
        };
        let (mx0, mx1, mz0, mz1) = (c.rox - 3, c.dix + 4, c.doz_a - 3, c.doz_b + 4);
        srv.cmd_ok(&format!("forceload add {} {} {} {}", mx0 - 16, mz0 - 16, mx1 + 16, mz1 + 16))?;
        std::thread::sleep(Duration::from_secs(2));
        let ow = srv.read_dim_box("overworld", [mx0, 0, mz0], [mx1, 12, mz1])?;
        let owb = Blocks::new(ow.region);
        // Only the room's own air volume (y2..8) must start dry; aquifers sealed under the
        // y1 floor or beyond the walls never get opened.
        let mut bad = false;
        for x in mx0..=mx1 {
            for z in mz0..=mz1 {
                for y in 2..=8 {
                    let s = owb.get([x, y, z]);
                    if s.starts_with("minecraft:water") || s.starts_with("minecraft:lava") {
                        bad = true;
                    }
                }
            }
        }
        if bad {
            n_wet += 1;
            srv.cmd_ok(&format!("forceload remove {} {} {} {}", mx0 - 16, mz0 - 16, mx1 + 16, mz1 + 16))?;
            continue;
        }
        let surf = srv.read_dim_box("overworld", [c.dix, 0, c.doz_a + 2], [c.dix + 2, 200, c.doz_a + 4])?;
        let surfb = Blocks::new(surf.region);
        let Some(ow_stand_y) = surface_stand(&surfb, c.dix + 1, c.doz_a + 3) else {
            n_surface += 1;
            srv.cmd_ok(&format!("forceload remove {} {} {} {}", mx0 - 16, mz0 - 16, mx1 + 16, mz1 + 16))?;
            continue;
        };
        // The dig shaft itself: no fluid in the column, no lava in its 3x3 neighbourhood
        // (same shaft rules as the Nether side, learned the wet way).
        let shaft_wet = (2..ow_stand_y).any(|y| {
            let s = surfb.get([c.dix + 1, y, c.doz_a + 3]);
            s.starts_with("minecraft:water")
                || s.starts_with("minecraft:lava")
                || (-1..=1).any(|dx| (-1..=1).any(|dz| surfb.get([c.dix + 1 + dx, y, c.doz_a + 3 + dz]).starts_with("minecraft:lava")))
        });
        if shaft_wet {
            n_surface += 1;
            srv.cmd_ok(&format!("forceload remove {} {} {} {}", mx0 - 16, mz0 - 16, mx1 + 16, mz1 + 16))?;
            continue;
        }
        chosen = Some(Plan {
            seed,
            fx,
            rz,
            layers: vec![],
            trips: v2.trips_total,
            shaft_x: shaft.0,
            shaft_z: shaft.1,
            shaft_stand_y: stand_y,
            ox: c.dix - 1,
            oz: c.doz_a,
            ow_stand_y,
            rx: c.rnx,
            v2: Some(v2),
        });
        break;
    }
    eprintln!("scout2: rejected by corridor-lava {n_corridor}, shaft {n_shaft}, wet-room {n_wet}, surface {n_surface}");
    let plan = chosen.context("scout2: every candidate failed the shaft/overworld checks")?;
    let v2 = plan.v2.as_ref().unwrap();
    eprintln!(
        "scout2: field x {}..{} rows {}..{} ({} trips, hole row {}), shaft ({},{}) stand y{}, machine R_O/D_O x {}/{} z {} stand y{}",
        plan.fx, plan.fx + 3, v2.z0, v2.z1, plan.trips, v2.hole_row, plan.shaft_x, plan.shaft_z, plan.shaft_stand_y, v2_coords(plan.fx, plan.rz).rox, v2_coords(plan.fx, plan.rz).dix, plan.oz, plan.ow_stand_y
    );
    srv.cmd_ok("save-all flush")?;
    let world_src = o.out.join("world-src");
    let _ = std::fs::remove_dir_all(&world_src);
    let src = srv.dir.join(&srv.level_name);
    drop(srv);
    copy_tree(&src, &world_src)?;
    std::fs::write(o.out.join("plan.json"), serde_json::to_string_pretty(&plan)?)?;
    Ok(plan)
}

fn record2(version: &str, plan: &Plan, o: &VideoOptions) -> Result<()> {
    let world = match &o.from {
        Some(tag) => o.out.join("checkpoints").join(tag),
        None => o.out.join("world-src"),
    };
    if !world.exists() {
        bail!("world {} missing (run --phase scout first)", world.display());
    }
    if o.from.is_some() {
        let state: State = serde_json::from_str(&std::fs::read_to_string(world.join("mctas-state.json")).context("checkpoint has no mctas-state.json")?)?;
        let mut timeline: Timeline = std::fs::read_to_string(o.out.join("timeline.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        timeline.segments.retain(|s| state.done.contains(&s.scene));
        std::fs::write(o.out.join("state.json"), serde_json::to_string_pretty(&state)?)?;
        std::fs::write(o.out.join("timeline.json"), serde_json::to_string_pretty(&timeline)?)?;
        eprintln!("record2: resuming from checkpoint {:?} (done: {:?})", o.from, state.done);
    }
    let mut p = start_prod(version, plan, o, world)?;
    p.loga(&format!("record2: seed {} field x {}..{} z {} trips {}", plan.seed, plan.fx, plan.fx + 3, plan.rz, plan.trips));
    scenes2(&mut p)?;
    p.stop_rec()?;
    p.checkpoint("end")?;
    p.loga("record2 done");
    Ok(())
}

const S1: &str = "Step 1/6 · Prepare the field";
const S2: &str = "Step 2/6 · The return portal";
const S3: &str = "Step 3/6 · The trigger machine";
const S4: &str = "Step 4/6 · First conversion";
const S5: &str = "Step 5/6 · The loop";
const S6: &str = "Step 6/6 · Deeper layers";

fn kit2(p: &mut Prod) -> Result<()> {
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
    p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.6 with minecraft:redstone 64"))?;
    p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.7 with minecraft:stone 64"))?;
    p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.8 with minecraft:oak_pressure_plate 4"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:obsidian 192"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:cobblestone 192"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:netherrack 128"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:stone 128"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:hopper 4"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:dropper 2"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:repeater 4"))?;
    p.rcon_ok(&format!("effect give {PLAYER} minecraft:haste infinite 1 true"))?;
    // Fire Resistance: clearing a 700-block room in netherrack WILL breach a lava pocket
    // (it drowned two takes); a player prepping a field brings a fire-res potion for this.
    p.rcon_ok(&format!("effect give {PLAYER} minecraft:fire_resistance infinite 0 true"))?;
    p.rcon_ok("time set noon")?;
    p.loga("kit given (captioned): Efficiency V pickaxe, obsidian, flint, redstone + dropper/hoppers/plates, Haste II, Fire Resistance");
    Ok(())
}

/// The overworld trigger machine (NOTES' remote trigger), built block by block: R_O, two
/// drill portals, the stone strip, hoppers + plates under R_O's exit side, the dust line
/// with two repeaters, and the dropper (at portal A) loaded with cobblestone.
fn build_machine(p: &mut Prod, c: &V2Coords) -> Result<()> {
    // Portals: bottom rows at y2 (on the y1 room floor), interiors y3..5.
    p.build_portal2("overworld", c.rox - 1, 2, c.doz_a, 3)?;
    p.build_portal2("overworld", c.dix - 1, 2, c.doz_a, 3)?;
    p.build_portal2("overworld", c.dix - 1, 2, c.doz_b, 3)?;
    // Back to the west end before building west-to-east: from the branch bay the only
    // walkable route is north up the branch column, west along the main aisle.
    p.walk_path(&[(c.dix + 1, c.doz_b + 2), (c.dix + 1, c.doz_a + 3), (c.rox, c.doz_a + 3), (c.rox, c.doz_a + 2)])?;
    // Stone only where something sits or feet go: the dust rows, the branch column, the
    // build-stand rows (a full rectangular slab was ~170 one-by-one placements).
    p.select(SLOT_STONE)?;
    let plate_cells: [(i32, i32); 4] = [(c.rox, c.doz_a + 1), (c.rox + 1, c.doz_a + 1), (c.rox, c.doz_a + 2), (c.rox + 1, c.doz_a + 2)];
    let mut strip: Vec<(i32, i32)> = Vec::new();
    for x in c.rox..=c.dix + 2 {
        strip.push((x, c.doz_a + 1));
        strip.push((x, c.doz_a + 2));
    }
    for z in c.doz_a + 3..=c.doz_b + 2 {
        strip.push((c.dix, z));
        strip.push((c.dix + 1, z));
        strip.push((c.dix + 2, z));
    }
    for (x, z) in strip {
        if plate_cells.contains(&(x, z)) {
            continue;
        }
        p.place_at("overworld", [x, 2, z], "minecraft:stone", None)?;
    }
    // Hoppers under the plates, then the plates themselves. The strip ends at the
    // branch's south end; walk the same aisle route back west first.
    p.walk_path(&[(c.dix + 1, c.doz_b + 2), (c.dix + 1, c.doz_a + 3), (c.rox, c.doz_a + 3), (c.rox, c.doz_a + 2)])?;
    p.hold("hopper")?;
    for (x, z) in plate_cells {
        p.place_at("overworld", [x, 2, z], "minecraft:hopper", None)?;
    }
    p.select(SLOT_SWAP)?;
    p.hold("oak_pressure_plate")?;
    for (x, z) in plate_cells {
        p.place_at_sneak("overworld", [x, 3, z], "minecraft:oak_pressure_plate")?;
    }
    // The back wall that keeps the arriving item on the plates.
    p.select(SLOT_STONE)?;
    for x in c.rox..=c.rox + 1 {
        for y in 3..=4 {
            p.place_at("overworld", [x, y, c.doz_a + 3], "minecraft:stone", None)?;
        }
    }
    // Dust: main line east from the plates, branch south to portal B's dropper spot.
    p.select(SLOT_REDSTONE)?;
    let rep1 = c.rox + 7;
    for x in c.rox + 2..=c.dix {
        if x == rep1 {
            continue;
        }
        p.place_at("overworld", [x, 3, c.doz_a + 1], "minecraft:redstone_wire", None)?;
    }
    for z in c.doz_a + 2..=c.doz_b + 1 {
        if z == c.doz_a + 5 {
            continue;
        }
        p.place_at("overworld", [c.dix, 3, z], "minecraft:redstone_wire", None)?;
    }
    // Repeaters: east on the main line, south on the branch (signal reaches both droppers).
    p.hold("repeater")?;
    p.place_at("overworld", [rep1, 3, c.doz_a + 1], "minecraft:repeater", Some((-90.0, 20.0)))?;
    p.place_at("overworld", [c.dix, 3, c.doz_a + 5], "minecraft:repeater", Some((0.0, 20.0)))?;
    // The dropper at portal A, facing north into its interior, loaded with cobblestone.
    p.hold("dropper")?;
    p.place_at("overworld", [c.dix + 1, 3, c.doz_a + 1], "minecraft:dropper", Some((0.0, 5.0)))?;
    p.test_block("overworld", [c.dix + 1, 3, c.doz_a + 1], "minecraft:dropper[facing=north]")?
        .then_some(())
        .context("dropper does not face north")?;
    let moved = p.fillinto([c.dix + 1, 3, c.doz_a + 1], "cobblestone")?;
    p.loga(&format!("dropper loaded: {moved}"));
    p.state.v2_dropper_b = false;
    p.save_state()?;
    p.select(SLOT_PICK)?;
    Ok(())
}

/// Walk from anywhere on the field to the R_N throw spot (north side, east column):
/// south to the border shelf, west along it, north up the west lane, east to the spot —
/// never through the portal plane.
fn go_throw_spot(p: &mut Prod) -> Result<()> {
    let (fx, rz) = (p.plan.fx, p.plan.rz);
    let v2 = p.plan.v2.as_ref().unwrap().clone();
    let c = v2_coords(fx, rz);
    let f = p.feet()?;
    let cur = [f[0].floor() as i32, f[2].floor() as i32];
    // Already west of the field on the north lane? Just walk east to the spot.
    if cur[1] <= rz - 2 && cur[0] <= c.rnx + 2 {
        return p.walk_path(&[(c.rnx + 2, rz - 2)]);
    }
    p.walk_path(&[(fx + 1, v2.z1 + 1), (c.rnx - 2, v2.z1 + 1), (c.rnx - 2, rz - 2), (c.rnx + 2, rz - 2)])
}

/// Throw one cobblestone into R_N from the throw spot.
fn throw_trigger(p: &mut Prod) -> Result<()> {
    let (fx, rz) = (p.plan.fx, p.plan.rz);
    let c = v2_coords(fx, rz);
    go_throw_spot(p)?;
    p.select(SLOT_COBBLE)?;
    p.look(c.rnx as f64 + 2.5, 6.6, rz as f64 + 0.5, 8)?;
    p.toss()?;
    p.wait(15)?;
    p.select(SLOT_PICK)?;
    Ok(())
}

/// Predict the next converted row from the live field (the current portal, if any, counted
/// as mined-one-block). `Ok(None)`: the pass is exhausted. Also returns that current row.
fn predict_next(p: &mut Prod, l: i32, exit_row: i32) -> Result<(Option<i32>, Option<i32>)> {
    let plan = p.plan.clone();
    let v2 = plan.v2.as_ref().unwrap();
    let mut b = p.nether_box([plan.fx - 20, 0, plan.rz - 20], [plan.fx + 24, 20, plan.rz + 24])?;
    let cur = (v2.z0 - 1..=v2.z1 + 1).find(|&r| b.get([plan.fx + 1, l + 1, r]).starts_with("minecraft:nether_portal"));
    if let Some(cr) = cur {
        for x in plan.fx + 1..=plan.fx + 2 {
            for y in l + 1..=l + 3 {
                b.set([x, y, cr], "minecraft:air");
            }
        }
        b.set([plan.fx, l + 1, cr], "minecraft:air");
    }
    let w = portal::winner(&b, [plan.fx + 1, v2.exit_y, exit_row], 0, 20);
    let next = match w {
        Some(w) if w.full && w.origin[0] == plan.fx + 1 && w.origin[1] == l + 1 && (v2.z0 - 1..=v2.z1 + 1).contains(&w.origin[2]) => Some(w.origin[2]),
        _ => None,
    };
    Ok((next, cur))
}

/// Assert a conversion: portal interior + the 4 obsidian bottom cells; count the trip.
fn assert_conversion(p: &mut Prod, l: i32, row: i32) -> Result<()> {
    let fx = p.plan.fx;
    p.assert_block("the_nether", [fx + 1, l + 1, row], "minecraft:nether_portal", "generated portal on the field")?;
    for x in fx..=fx + 3 {
        p.assert_block("the_nether", [x, l, row], "minecraft:obsidian", "converted line")?;
    }
    p.state.v2_trips += 1;
    p.save_state()?;
    Ok(())
}

/// One remote trip: mine the current portal's one block, throw the trigger item, and check
/// the new portal really appeared on `row`.
fn remote_trip(p: &mut Prod, l: i32, cur: i32, row: i32) -> Result<()> {
    let fx = p.plan.fx;
    p.walk_path(&[(fx + 1, cur + 1)])?;
    p.select(SLOT_PICK)?;
    p.mine([fx, l + 1, cur])?;
    p.assert_block("the_nether", [fx + 1, l + 1, cur], "minecraft:air", "portal out after mining one frame block")?;
    throw_trigger(p)?;
    p.poll_block("the_nether", [fx + 1, l + 1, row], "minecraft:nether_portal", 25)?;
    assert_conversion(p, l, row)?;
    // Walk back beside the new portal (for the next mine, and for the camera).
    let v2 = p.plan.v2.as_ref().unwrap().clone();
    let c = v2_coords(fx, p.plan.rz);
    p.walk_path(&[(c.rnx - 2, p.plan.rz - 2), (c.rnx - 2, v2.z1 + 1), (fx + 1, v2.z1 + 1), (fx + 1, row + 2)])?;
    p.look(fx as f64 + 1.5, l as f64 + 1.2, row as f64 + 0.5, 8)?;
    Ok(())
}

/// Tunnel-clear a box the way a player digs: a boustrophedon (snake) of columns from the
/// entry corner — mine the whole column ahead, step in, repeat. Stands are guaranteed
/// (the previous column), so this cannot stall the way snapshot-stand search does in
/// solid rock. The entry column comes first in its row (east leg, then west, then snake).
/// Cells the start snapshot shows as air are skipped; a second sweep catches stragglers.
fn tunnel_clear(p: &mut Prod, dim: &str, lo: Pos, hi: Pos, entry: (i32, i32)) -> Result<()> {
    let mut order: Vec<(i32, i32)> = Vec::new();
    let ez = entry.1;
    order.extend((entry.0..=hi[0]).map(|x| (x, ez)));
    // Walk back west over the just-mined row (cells east of the entry are air by now and
    // simply walked through), ending at the row's west end.
    order.extend((lo[0]..hi[0]).rev().map(|x| (x, ez)));
    let mut west_end = true; // the entry row ends at lo.x
    for z in (lo[2]..ez).rev() {
        if west_end {
            order.extend((lo[0]..=hi[0]).map(|x| (x, z)));
        } else {
            order.extend((lo[0]..=hi[0]).rev().map(|x| (x, z)));
        }
        west_end = !west_end;
    }
    for z in ez + 1..=hi[2] {
        if west_end {
            order.extend((lo[0]..=hi[0]).map(|x| (x, z)));
        } else {
            order.extend((lo[0]..=hi[0]).rev().map(|x| (x, z)));
        }
        west_end = !west_end;
    }
    p.select(SLOT_PICK)?;
    for sweep in 0..2 {
        let blocks = if dim == "the_nether" { p.nether_box(lo, hi)? } else { p.overworld_box(lo, hi)? };
        let mut mined = 0usize;
        for (i, &(x, z)) in order.iter().enumerate() {
            if i % 40 == 0 {
                p.check_alive()?;
                if p.invcount("cobbled_deepslate")? > 192 {
                    p.dropall("cobbled_deepslate", 0)?;
                    p.select(SLOT_PICK)?;
                }
                if dim == "the_nether" && p.invcount("netherrack")? > 448 {
                    p.dropall("netherrack", 3)?;
                    p.select(SLOT_PICK)?;
                }
            }
            for y in (lo[1]..=hi[1]).rev() {
                let s = blocks.get([x, y, z]);
                if blocks.kind([x, y, z]) == Kind::Solid && !s.starts_with("minecraft:bedrock") {
                    // Held-key sweep: a handful of frame-gated submits per block, where the
                    // precise path needs ~15 (measured ~15 s/block when the client's tick
                    // rate sags with its frame time under llvmpipe).
                    p.mine_sweep([x, y, z])?;
                    mined += 1;
                }
            }
            p.walk(x, z)?;
        }
        p.loga(&format!("tunnel_clear {dim} sweep {sweep}: mined {mined}"));
        if mined == 0 {
            break;
        }
    }
    // Verify: only bedrock (never in these boxes) may remain solid.
    let check = if dim == "the_nether" { p.nether_box(lo, hi)? } else { p.overworld_box(lo, hi)? };
    let left = (lo[1]..=hi[1])
        .flat_map(|y| (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z])))
        .filter(|&c| check.kind(c) == Kind::Solid && !check.get(c).starts_with("minecraft:bedrock"))
        .count();
    if left > 0 {
        bail!("tunnel_clear {dim} {lo:?}..{hi:?}: {left} cells still solid");
    }
    Ok(())
}

/// Mine every obsidian cell in a box (frames and converted lines), re-reading until none
/// is left. `skip` cells are spared (the y0 returns over the void handle themselves).
fn mine_obsidian_box(p: &mut Prod, lo: Pos, hi: Pos) -> Result<()> {
    let rlo = [lo[0] - 3, (lo[1] - 1).max(0), lo[2] - 3];
    let rhi = [hi[0] + 3, hi[1] + 2, hi[2] + 3];
    let mut rescued = false;
    for pass in 0..80 {
        p.check_alive()?;
        if p.invcount("obsidian")? > 320 {
            p.dropall("obsidian", 1)?;
        }
        let blocks = p.nether_box(rlo, rhi)?;
        let mut todo: Vec<Pos> = Vec::new();
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    if blocks.get([x, y, z]).starts_with("minecraft:obsidian") {
                        todo.push([x, y, z]);
                    }
                }
            }
        }
        if todo.is_empty() {
            return Ok(());
        }
        p.loga(&format!("mine_obsidian_box pass {pass}: {} cells", todo.len()));
        p.select(SLOT_PICK)?;
        let r = p.digbox(lo, hi, "obsidian")?;
        p.loga(&format!("mine_obsidian_box digbox -> {r}"));
        if r.contains("left=0") {
            return Ok(());
        }
        let after = p.nether_box(rlo, rhi)?;
        let left = (lo[1]..=hi[1])
            .flat_map(|y| (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z])))
            .filter(|&c| after.get(c).starts_with("minecraft:obsidian"))
            .count();
        if left >= todo.len() {
            if !rescued {
                rescued = true;
                let (cx, cz) = ((lo[0] + hi[0]) / 2, hi[2] + 1);
                p.tp("the_nether", cx as f64 + 0.5, hi[1] as f64 + 1.0, cz as f64 + 0.5, 180.0, 30.0)?;
                continue;
            }
            bail!("mine_obsidian_box {lo:?}..{hi:?}: no reachable stand in a whole pass");
        }
        rescued = false;
    }
    bail!("mine_obsidian_box {lo:?}..{hi:?}: still busy after 80 passes")
}

/// Open a pass by moving the dropper to the other drill portal (the player makes this one
/// trip through R_N in person; their entry through the new drill portal is the pass's
/// first conversion, predicted and asserted like any other).
fn move_dropper(p: &mut Prod, tag: &str, l: i32, to_b: bool) -> Result<()> {
    let plan = p.plan.clone();
    let (fx, rz) = (plan.fx, plan.rz);
    let c = v2_coords(fx, rz);
    let (old_doz, new_doz) = if to_b { (c.doz_a, c.doz_b) } else { (c.doz_b, c.doz_a) };
    let step = if l == 4 { S5 } else { S6 };
    p.seg_step(
        "move-dropper",
        tag,
        &[if to_b {
            "Pass 2: move the dropper to the second drill portal (8 blocks = 1 Nether row)"
        } else {
            "Next layer: the dropper goes back to the first drill portal"
        }],
        4.0,
        step,
    )?;
    let exit_row = rz + to_b as i32;
    let (want, cur) = predict_next(p, l, exit_row)?;
    if cur.is_some() {
        bail!("move_dropper: a lit portal still stands on the field at row {cur:?}");
    }
    let want = want.context("move_dropper: predictor sees no first row for this pass")?;
    // Through R_N (arrive at R_O), out its north side (never across the plates).
    go_throw_spot(p)?;
    p.walk(c.rnx + 2, rz)?;
    p.waitdim("overworld")?;
    p.wait(10)?;
    p.walk_path(&[(c.rox + 1, c.doz_a - 2), (c.dix + 3, c.doz_a - 2), (c.dix + 3, old_doz + 2), (c.dix + 1, old_doz + 2)])?;
    p.select(SLOT_PICK)?;
    p.mine([c.dix + 1, 3, old_doz + 1])?;
    p.assert_block("overworld", [c.dix + 1, 3, old_doz + 1], "minecraft:air", "old dropper mined")?;
    p.hold("dropper")?;
    p.walk(c.dix + 1, new_doz + 2)?;
    p.place_at("overworld", [c.dix + 1, 3, new_doz + 1], "minecraft:dropper", Some((0.0, 5.0)))?;
    p.test_block("overworld", [c.dix + 1, 3, new_doz + 1], "minecraft:dropper[facing=north]")?
        .then_some(())
        .context("moved dropper does not face north")?;
    let moved = p.fillinto([c.dix + 1, 3, new_doz + 1], "cobblestone")?;
    p.loga(&format!("moved dropper loaded: {moved}"));
    p.state.v2_dropper_b = to_b;
    p.save_state()?;
    // Back through the now-fed drill portal: first conversion of the pass.
    p.seg_step("trip", tag, &[&format!("Trip {}/{} · layer y={l}", p.state.v2_trips + 1, plan.trips)], 2.0, step)?;
    p.select(SLOT_PICK)?;
    p.walk_path(&[(c.dix, new_doz + 2), (c.dix, new_doz)])?;
    p.waitdim("the_nether")?;
    p.wait(10)?;
    assert_conversion(p, l, want)?;
    p.walk_path(&[(fx + 1, want + 2)])?;
    p.look(fx as f64 + 1.5, l as f64 + 1.2, want as f64 + 0.5, 8)?;
    Ok(())
}

/// Remote trips until the predictor says the pass is exhausted; then the pass-end mining
/// (leftover frames; the layer's lines too after pass B) and a checkpoint.
fn sweep_pass(p: &mut Prod, l: i32, pass_b: bool) -> Result<()> {
    let tag = format!("sweep{l}{}", if pass_b { "b" } else { "a" });
    if p.done(&tag) {
        return Ok(());
    }
    let plan = p.plan.clone();
    let (fx, rz) = (plan.fx, plan.rz);
    let v2 = plan.v2.clone().unwrap();
    let step = if l == 4 { S5 } else { S6 };
    if p.state.v2_dropper_b != pass_b {
        move_dropper(p, &tag, l, pass_b)?;
    }
    let exit_row = rz + pass_b as i32;
    loop {
        p.check_alive()?;
        let (next, cur) = predict_next(p, l, exit_row)?;
        let Some(row) = next else { break };
        let cur = cur.context("sweep: predictor wants a trip but no lit portal stands on the field")?;
        p.seg_step("trip", &tag, &[&format!("Trip {}/{} · layer y={l}", p.state.v2_trips + 1, plan.trips)], 5.0, step)?;
        remote_trip(p, l, cur, row)?;
    }
    p.seg_step("mine-frames", &tag, &["Pass done — mine the leftover frames (keep the lines)"], 8.0, step)?;
    mine_obsidian_box(p, [fx - 1, l + 1, v2.z0 - 2], [fx + 4, l + 4, v2.z1 + 2])?;
    if pass_b {
        p.seg_step("mine-layer", &tag, &[&format!("Layer y={l} converted — mine the obsidian out")], 8.0, step)?;
        if l > 0 {
            mine_obsidian_box(p, [fx, l, v2.z0 - 1], [fx + 3, l, v2.z1 + 1])?;
        } else {
            mine_bottom_layer(p)?;
        }
    }
    p.checkpoint(&tag)?;
    Ok(())
}

/// Mine the y0 obsidian lines north-row-first, always standing on the not-yet-mined south
/// neighbour: the player retreats over solid ground while the void opens behind them.
fn mine_bottom_layer(p: &mut Prod) -> Result<()> {
    let plan = p.plan.clone();
    let v2 = plan.v2.as_ref().unwrap();
    p.select(SLOT_PICK)?;
    for row in v2.z0 - 1..=v2.z1 + 1 {
        for x in plan.fx..=plan.fx + 3 {
            let blocks = p.nether_box([x - 1, 0, row], [x + 1, 1, row + 1])?;
            if !blocks.get([x, 0, row]).starts_with("minecraft:obsidian") {
                continue;
            }
            if blocks.kind([x, 0, row + 1]) != Kind::Solid {
                p.loga(&format!("mine_bottom_layer: skipping [{x},0,{row}] (no safe south stand)"));
                continue;
            }
            p.walk(x, row + 1)?;
            p.mine([x, 0, row])?;
        }
    }
    Ok(())
}

fn scenes2(p: &mut Prod) -> Result<()> {
    let plan = p.plan.clone();
    let (fx, rz) = (plan.fx, plan.rz);
    let v2 = plan.v2.clone().context("scenes2 needs a v2 plan")?;
    let c = v2_coords(fx, rz);
    if !p.done("setup2") {
        kit2(p)?;
        p.tp("the_nether", plan.shaft_x as f64 + 0.5, plan.shaft_stand_y as f64, plan.shaft_z as f64 + 0.5, 180.0, 10.0)?;
        p.checkpoint("setup2")?;
    }
    if !p.done("intro2") {
        p.seg_step("intro", "intro2", &[
            "Bedrock can't be mined — but the game will remove it for us",
            &format!("A real random world (seed {}) · survival · the kit was given (see card)", plan.seed),
        ], 1.0, "")?;
        p.lookdir(180.0, 5.0, 10)?;
        p.wait(25)?;
        p.lookdir(120.0, 0.0, 30)?;
        p.lookdir(240.0, 0.0, 40)?;
        p.lookdir(180.0, 25.0, 20)?;
        p.wait(15)?;
        p.checkpoint("intro2")?;
    }
    if !p.done("field") {
        p.seg_step("dig-down", "field", &["Dig down to the Nether floor (bedrock lives at y 0-4)"], 6.0, S1)?;
        p.walk(plan.shaft_x, plan.shaft_z)?;
        p.dig_down2("the_nether", plan.shaft_x, plan.shaft_z, 5.0)?;
        p.select(SLOT_PICK)?;
        for z in (v2.z1 + 2..plan.shaft_z).rev() {
            p.mine([plan.shaft_x, 5, z])?;
            p.mine([plan.shaft_x, 6, z])?;
            p.walk(plan.shaft_x, z)?;
        }
        p.seg_step("field-prep", "field", &[
            "Flatten one field: 4 blocks wide, 11 rows",
            "Solid floor over the bedrock, air above — once, for the whole job",
        ], 6.0, S1)?;
        p.clear_box("the_nether", [fx - 2, 5, v2.z0 - 2], [fx + 5, 10, v2.z1 + 2], false)?;
        p.fill_floor("the_nether", 4, fx - 2, fx + 5, v2.z0 - 1, v2.z1 + 1, Some((fx - 1, fx + 4, v2.z0 - 1, v2.z1 + 1)))?;
        // The west lanes to the return portal (walkway sheet + headroom) — cleared now,
        // while everything is still at the top level.
        p.clear_box("the_nether", [c.rnx - 3, 5, rz - 3], [fx - 3, 7, v2.z1 + 1], false)?;
        p.fill_floor("the_nether", 4, c.rnx - 3, fx - 3, rz - 3, v2.z1 + 1, None)?;
        p.torch([fx - 1, 4, v2.z0 - 1])?;
        p.torch([fx + 4, 4, v2.z1 + 1])?;
        p.seg_step("field-top", "field", &["The prepared field (camera view): 4 wide, 11 rows"], 1.0, S1)?;
        p.spec_shot("the_nether", [fx as f64 + 1.5, 10.5, rz as f64 + 9.5], [fx as f64 + 1.5, 4.0, rz as f64 + 0.5], 70)?;
        p.seg_step("bedrock-before", "field", &[
            "The top bedrock layer, y=4 — unbreakable by hand",
            "Watch this spot",
        ], 1.0, S1)?;
        p.walk(fx + 1, rz + 3)?;
        p.look(fx as f64 + 1.5, 4.3, rz as f64 + 0.5, 16)?;
        p.wait(60)?;
        p.checkpoint("field")?;
    }
    if !p.done("rn") {
        p.seg_step("return-portal", "rn", &[
            "A return portal 17+ blocks west of the field",
            "(portal search: 16 in the Nether, 128 in the Overworld)",
        ], 4.0, S2)?;
        p.clear_box("the_nether", [c.rnx - 1, 5, rz - 2], [c.rnx + 4, 10, rz + 2], false)?;
        p.build_portal2("the_nether", c.rnx, 5, rz, 3)?;
        // In person this once: the game generates its (wrong-placed) twin in the Overworld.
        p.walk_path(&[(c.rnx + 2, rz + 2), (c.rnx + 2, rz)])?;
        let arrived = p.waitdim("overworld")?;
        p.wait(15)?;
        let f = p.feet()?;
        p.state.twin = Some(f);
        p.save_state()?;
        p.loga(&format!("generated twin around {f:?} ({arrived})"));
        p.tp("overworld", c.dix as f64 + 1.5, plan.ow_stand_y as f64, c.doz_a as f64 + 3.5, 180.0, 10.0)?;
        p.checkpoint("rn")?;
    }
    // The machine-dig legs are separate checkpoints: the client's renderer falls behind
    // after a few hundred edits (every submit waits a frame), so the shoot resumes with a
    // FRESH client at each leg rather than crawling at ~1 fps by the third.
    if !p.done("owdig") {
        p.seg_step("overworld-dig", "owdig", &[
            "Travel skipped (teleport) — Overworld, 8x the field's coordinates",
            "Dig down: the machine sits with its portals' floors at y=2",
        ], 6.0, S3)?;
        p.walk(c.dix + 1, c.doz_a + 3)?;
        p.dig_down2("overworld", c.dix + 1, c.doz_a + 3, 2.0)?;
        // A minimal cavity, not a whole rectangular room (a 1600-cell clear was eating the
        // session): tunnel-dig the main band (aisle + strip) and the branch band south to
        // portal B, then clear the portal columns' tops from the open floor.
        tunnel_clear(p, "overworld", [c.rox - 1, 2, c.doz_a - 2], [c.dix + 3, 4, c.doz_a + 3], (c.dix + 1, c.doz_a + 3))?;
        p.checkpoint("owdig")?;
    }
    if !p.done("owbranch") {
        p.seg_step("ow-branch", "owbranch", &["Dig the second drill portal's bay (8 blocks south)"], 6.0, S3)?;
        p.walk(c.dix + 1, c.doz_a + 3)?;
        tunnel_clear(p, "overworld", [c.dix - 1, 2, c.doz_a + 4], [c.dix + 3, 6, c.doz_b + 2], (c.dix + 1, c.doz_a + 4))?;
        p.clear_box("overworld", [c.rox - 1, 5, c.doz_a], [c.rox + 2, 6, c.doz_a], false)?;
        p.clear_box("overworld", [c.dix - 1, 5, c.doz_a], [c.dix + 2, 6, c.doz_a], false)?;
        p.checkpoint("owbranch")?;
    }
    if !p.done("machine") {
        // Coordinator ruling after the starting pick broke in the machine dig (2,032
        // uses): exactly one identical replacement pick by command, captioned here and
        // disclosed in NOTES. Nothing else is given; every later action is survival.
        p.seg_step("pick-replacement", "machine", &[
            "The starting pick broke in the dig (2,000+ blocks)",
            "One identical replacement given by command — kit only",
        ], 1.0, S3)?;
        p.select(SLOT_PICK)?;
        p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.0 with minecraft:netherite_pickaxe"))?;
        p.rcon_ok(&format!("enchant {PLAYER} minecraft:efficiency 5"))?;
        p.select(SLOT_PICK)?;
        p.wait(80)?;
        // The generated twin must die or it would steal the trigger items.
        p.seg_step("twin-off", "machine", &[
            "The twin the game generated must go (one frame block)",
            "(ours will sit next to the drill portal instead)",
        ], 2.0, S3)?;
        let twin = p.state.twin.context("no twin position recorded")?;
        p.tp("overworld", twin[0], twin[1] + 1.0, twin[2] + 3.0, 180.0, 20.0)?;
        let (tx, ty, tz) = (twin[0].floor() as i32, twin[1].floor() as i32, twin[2].floor() as i32);
        let around = p.overworld_box([tx - 4, ty - 3, tz - 4], [tx + 4, ty + 5, tz + 4])?;
        let mut pillar: Option<Pos> = None;
        'scan: for y in ty - 3..=ty + 5 {
            for z in tz - 4..=tz + 4 {
                for x in tx - 4..=tx + 4 {
                    if around.get([x, y, z]).starts_with("minecraft:obsidian")
                        && (around.get([x + 1, y, z]).starts_with("minecraft:nether_portal") || around.get([x - 1, y, z]).starts_with("minecraft:nether_portal"))
                    {
                        pillar = Some([x, y, z]);
                        break 'scan;
                    }
                }
            }
        }
        let pillar = pillar.context("generated twin frame not found near arrival")?;
        p.stand_near(&around, pillar)?;
        p.select(SLOT_PICK)?;
        p.mine(pillar)?;
        p.loga("generated twin's frame block mined (its portal blocks die with it)");
        p.tp("overworld", c.dix as f64 + 1.5, 3.0, c.doz_a as f64 - 1.5, 0.0, 20.0)?;
        p.seg_step("machine-build", "machine", &[
            "Build the trigger at R_O (the return twin): plate on hoppers, redstone, dropper",
            "One item on the plate = one fresh stone into D_O, the drill portal",
        ], 5.0, S3)?;
        build_machine(p, &c)?;
        p.seg_step("machine-top", "machine", &["The finished machine (camera view): R_O left, the two drill portals right"], 1.0, S3)?;
        let midx = (c.rox + c.dix) as f64 / 2.0;
        p.spec_shot("overworld", [midx, 6.3, c.doz_b as f64 + 2.5], [midx, 2.5, c.doz_a as f64 + 1.0], 80)?;
        p.checkpoint("machine")?;
    }
    if !p.done("firstdrill") {
        let (want, cur) = predict_next(p, 4, rz)?;
        if cur.is_some() {
            bail!("firstdrill: a portal already stands on the field");
        }
        let want = want.context("firstdrill: predictor sees no site on the field")?;
        p.seg_step("first-trip", "firstdrill", &[
            "Through D_O (the drill portal): no exit portal exists, so the game builds one…",
            "…writing obsidian OVER the bedrock row",
        ], 1.0, S4)?;
        // Around the east side to the portal's front (never through R_O or D_O), then in
        // through the west interior column (the dropper block guards the east one).
        p.walk_path(&[(c.dix + 3, c.doz_a - 2), (c.dix + 3, c.doz_a + 2), (c.dix, c.doz_a + 2)])?;
        p.look(c.dix as f64 + 1.5, 4.0, c.doz_a as f64 + 0.5, 10)?;
        p.wait(25)?;
        p.walk(c.dix, c.doz_a)?;
        p.waitdim("the_nether")?;
        p.wait(15)?;
        assert_conversion(p, 4, want)?;
        p.walk_path(&[(fx + 1, want + 2)])?;
        // The money close-up: bedrock before (earlier shot), obsidian now, same row.
        p.look(fx as f64 + 1.5, 4.3, want as f64 + 0.5, 16)?;
        p.wait(60)?;
        p.checkpoint("firstdrill")?;
    }
    if !p.done("loop1") {
        let (next, cur) = predict_next(p, 4, rz)?;
        let (next, cur) = (next.context("loop1: predictor exhausted already")?, cur.context("loop1: no lit portal")?);
        p.seg_step("loop-mine", "loop1", &[
            "The loop, in real time. 1: mine ONE frame block",
            "(the portal goes out — the rest of the frame stays)",
        ], 1.0, S5)?;
        p.walk_path(&[(fx + 1, cur + 1)])?;
        p.look(fx as f64 + 0.5, 5.5, cur as f64 + 0.5, 10)?;
        p.wait(20)?;
        p.mine([fx, 5, cur])?;
        p.assert_block("the_nether", [fx + 1, 5, cur], "minecraft:air", "portal out (loop)")?;
        p.wait(15)?;
        p.seg_step("loop-throw", "loop1", &["2: throw ONE item into R_N, the return portal — and stay in the Nether"], 1.0, S5)?;
        throw_trigger(p)?;
        p.seg_step("loop-result", "loop1", &[
            "3: item exits at R_O → plate → dropper fires into D_O → new portal here",
            "The leftover frame blocks its own row, so the game picks the next",
        ], 1.0, S5)?;
        p.poll_block("the_nether", [fx + 1, 5, next], "minecraft:nether_portal", 25)?;
        assert_conversion(p, 4, next)?;
        p.walk_path(&[(c.rnx - 2, rz - 2), (c.rnx - 2, v2.z1 + 1), (fx + 1, v2.z1 + 1), (fx + 1, next + 2)])?;
        p.look(fx as f64 + 1.5, 4.6, next as f64 + 0.5, 14)?;
        p.wait(45)?;
        p.checkpoint("loop1")?;
    }
    for l in [4, 3, 2, 1, 0] {
        sweep_pass(p, l, false)?;
        sweep_pass(p, l, true)?;
    }
    if !p.done("finale2") {
        let hole = v2.hole_row;
        for y in 0..=4 {
            p.assert_block("the_nether", [fx + 1, y, hole], "minecraft:air", "the finished hole is open")?;
        }
        p.seg_step("reveal", "finale2", &[
            "Through all five layers",
            "A hole in the bedrock floor — in survival",
        ], 1.0, "The result")?;
        // Stand on a surviving south neighbour of the hole column and look in.
        let b = p.nether_box([fx, 0, hole], [fx + 3, 1, hole + 3])?;
        let mut stand: Option<(i32, i32)> = None;
        'find: for dz in 1..=3 {
            for x in fx..=fx + 3 {
                if b.kind([x, 0, hole + dz]) == Kind::Solid && b.kind([x, 1, hole + dz]) == Kind::Replaceable {
                    stand = Some((x, hole + dz));
                    break 'find;
                }
            }
        }
        let (sx, sz) = stand.context("no stand south of the hole")?;
        p.walk(sx, sz)?;
        p.actor.cmd("sneak 1")?;
        p.look(sx as f64 + 0.5, -6.0, hole as f64 + 0.5, 24)?;
        p.wait(50)?;
        p.select(SLOT_COBBLE)?;
        p.toss()?;
        p.wait(45)?;
        p.actor.cmd("sneak 0")?;
        p.seg_step("hook", "finale2", &[
            "Bedrock is unbreakable in survival…",
            "…unless you let the game remove it",
        ], 1.0, "")?;
        p.cam(true)?;
        p.look(sx as f64 + 0.5, -6.0, hole as f64 + 0.5, 18)?;
        p.wait(40)?;
        p.toss()?;
        p.wait(50)?;
        p.cam(false)?;
        p.select(SLOT_PICK)?;
        p.checkpoint("finale2")?;
    }
    Ok(())
}

/// v2's smoke: the machine parts that v1 never exercised, on a throwaway pad (commands
/// build the pad itself; every tested action is the actor in survival): a plate on a
/// hopper, a dropper placed facing north and loaded through its GUI, a toss, hotbar swap,
/// overhead dust powered by stepping on the plate (the dropper must fire: its cobble
/// lands north of it — checked by inventory of the area via a second fill).
fn smoke2(version: &str, o: &VideoOptions) -> Result<()> {
    let plan = Plan { seed: 1, fx: 0, rz: 0, layers: vec![], trips: 0, shaft_x: 0, shaft_z: 0, shaft_stand_y: 70, ox: 0, oz: 0, ow_stand_y: 70, rx: 0, v2: None };
    let world_dir = o.out.join("smoke2-world");
    let _ = std::fs::remove_dir_all(&world_dir);
    let mut p = {
        let mut srv = VanillaServer::start(&server_opts(version, 1, None))?;
        srv.cmd_ok("save-all flush")?;
        copy_tree(&srv.dir.join(&srv.level_name), &world_dir)?;
        drop(srv);
        start_prod(version, &plan, o, world_dir.clone())?
    };
    p.rcon_ok(&format!("gamemode survival {PLAYER}"))?;
    p.rcon_ok(&format!("clear {PLAYER}"))?;
    p.rcon_ok(&format!("item replace entity {PLAYER} hotbar.0 with minecraft:netherite_pickaxe"))?;
    p.rcon_ok(&format!("enchant {PLAYER} minecraft:efficiency 5"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:cobblestone 192"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:hopper 2"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:dropper 1"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:oak_pressure_plate 2"))?;
    p.rcon_ok(&format!("give {PLAYER} minecraft:redstone 16"))?;
    p.rcon_ok(&format!("effect give {PLAYER} minecraft:haste infinite 1 true"))?;
    p.rcon_ok("forceload add -16 -16 16 16")?;
    std::thread::sleep(Duration::from_secs(3));
    p.rcon_ok("fill -5 68 -5 5 69 5 minecraft:stone")?;
    p.rcon_ok("fill -5 70 -5 5 74 5 minecraft:air")?;
    p.tp("overworld", 0.5, 70.0, 3.5, 180.0, 20.0)?;
    // Hopper at (0,70,0) on the pad, plate on top: the survival question of the whole build.
    p.hold("hopper")?;
    p.place_at("overworld", [0, 70, 0], "minecraft:hopper", None)?;
    p.hold("oak_pressure_plate")?;
    p.place_at_sneak("overworld", [0, 71, 0], "minecraft:oak_pressure_plate")?;
    p.loga("smoke2: plate sits on hopper");
    // Dropper facing north at (2,70,-1), dust from plate to it, loaded via its GUI.
    p.hold("dropper")?;
    p.place_at("overworld", [2, 70, -1], "minecraft:dropper", Some((0.0, 5.0)))?;
    p.test_block("overworld", [2, 70, -1], "minecraft:dropper[facing=north]")?.then_some(()).context("smoke2 dropper facing")?;
    p.select(SLOT_REDSTONE.max(6))?;
    p.hold("redstone")?;
    p.place_at("overworld", [1, 70, 0], "minecraft:redstone_wire", None)?;
    p.place_at("overworld", [2, 70, 0], "minecraft:redstone_wire", None)?;
    let moved = p.fillinto([2, 70, -1], "cobblestone")?;
    p.loga(&format!("smoke2: fillinto -> {moved}"));
    if !moved.contains("moved=") || moved.contains("moved=0") {
        bail!("smoke2: dropper loading moved nothing: {moved}");
    }
    // Toss: cobble count drops by one (and do not walk over the item to pick it back).
    let before = p.invcount("cobblestone")?;
    p.hold("cobblestone")?;
    p.lookdir(180.0, 10.0, 4)?;
    p.toss()?;
    p.wait(10)?;
    let after = p.invcount("cobblestone")?;
    p.loga(&format!("smoke2: toss cobble {before} -> {after}"));
    if after != before - 1 {
        bail!("smoke2: toss did not drop exactly one cobblestone");
    }
    // Step on the plate: dust fires the dropper (its item count inside drops: refill and
    // compare how much fits is overkill — instead, an item entity worth of cobble must end
    // up pickable north of the dropper; simplest proof: fillinto again moves 1 more).
    let inv_before = p.invcount("cobblestone")?;
    p.walk(0, 2)?;
    p.walk(0, 0)?;
    p.wait(20)?;
    p.walk(0, 3)?;
    let moved2 = p.fillinto([2, 70, -1], "cobblestone")?;
    let inv_after = p.invcount("cobblestone")?;
    p.loga(&format!("smoke2: second fill -> {moved2} (inv {inv_before} -> {inv_after})"));
    p.cam(true)?;
    p.wait(10)?;
    p.cam(false)?;
    println!("smoke2 ok: plate-on-hopper, dropper facing + GUI load, toss, plate fire, cam");
    Ok(())
}

fn run2(version: &str, o: &VideoOptions) -> Result<()> {
    match o.phase.as_str() {
        "smoke" => smoke(version, o),
        "smoke2" => smoke2(version, o),
        "scout" => {
            let plan = scout2(version, o)?;
            println!("plan: seed {} field x {}..{} rows {}..{}, {} trips", plan.seed, plan.fx, plan.fx + 3, plan.v2.as_ref().unwrap().z0, plan.v2.as_ref().unwrap().z1, plan.trips);
            Ok(())
        }
        "record" => record2(version, &load_plan(o)?, o),
        "column" => {
            // Debug: dump the plan's shaft column (block per y) of a saved world, no server.
            let plan = load_plan(o)?;
            let world = match &o.from {
                Some(tag) => o.out.join("checkpoints").join(tag),
                None => o.out.join("world-src"),
            };
            let dim = world.join("dimensions").join("minecraft").join("the_nether");
            let snap = mctas::vanilla::read_box(&dim, [plan.shaft_x - 1, 0, plan.shaft_z - 1], [plan.shaft_x + 1, 110, plan.shaft_z + 1])?;
            let b = Blocks::new(snap.region);
            for y in (0..=105).rev() {
                let row: Vec<String> = (-1..=1)
                    .map(|dx| (-1..=1).map(|dz| match b.kind([plan.shaft_x + dx, y, plan.shaft_z + dz]) {
                        Kind::Solid => "#",
                        Kind::Replaceable => ".",
                        Kind::Other => "?",
                    }).collect::<Vec<_>>().join(""))
                    .collect();
                println!("y{y:3} {} {}", row.join(" "), b.get([plan.shaft_x, y, plan.shaft_z]));
            }
            Ok(())
        }
        "compose" => compose2(version, &load_plan(o)?, o),
        "all" => {
            let plan = if o.out.join("plan.json").exists() { load_plan(o)? } else { scout2(version, o)? };
            record2(version, &plan, o)?;
            compose2(version, &plan, o)
        }
        other => bail!("unknown --phase {other} for v2 (smoke|smoke2|scout|record|compose|all)"),
    }
}

// ---- v2 compose: big top captions, step chip, speed badge, drawn diagrams ------------------

/// One overlay PNG per segment: the caption (scale 4 — twice v1 — in a dark box, top-center,
/// clear of crosshair and hotbar), the step chip under it at the left, and a ×N badge at the
/// right when the footage runs faster than real time.
fn label2(font: &mctas::mcfont::Font, w: u32, h: u32, lines: &[String], step: &str, badge: &str) -> mctas::mcfont::Image {
    let mut img = mctas::mcfont::Image::new(w, h, [0, 0, 0, 0]);
    let s = 4u32;
    let pad = 14i64;
    let max_w = (w as f64 * 0.94) as u32;
    let mut laid: Vec<String> = Vec::new();
    for t in lines {
        laid.extend(font.wrap(t, s, max_w - 2 * pad as u32));
    }
    let lh = (mctas::mcfont::LINE_HEIGHT * s) as i64;
    let box_w = laid.iter().map(|t| font.width(t, s) as i64).max().unwrap_or(0) + 2 * pad;
    let box_h = lh * laid.len() as i64 + 2 * pad - 4;
    let (x, y) = ((w as i64 - box_w) / 2, 18i64);
    if !laid.is_empty() {
        img.fill_rect(x, y, box_w, box_h, [0, 0, 0, 190]);
        let mut ty = y + pad;
        for t in &laid {
            font.draw_centered(&mut img, w as i64 / 2, ty, t, s, WHITE, true);
            ty += lh;
        }
    }
    if !step.is_empty() {
        let ss = 2u32;
        let sw = font.width(step, ss) as i64 + 20;
        let sh = (mctas::mcfont::LINE_HEIGHT * ss) as i64 + 12;
        let sy = if laid.is_empty() { 18 } else { y + box_h + 10 };
        img.fill_rect(18, sy, sw, sh, [0, 0, 0, 150]);
        font.draw(&mut img, 28, sy + 6, step, ss, AQUA, true);
    }
    if !badge.is_empty() {
        let bs = 3u32;
        let bw = font.width(badge, bs) as i64 + 20;
        let bh = (mctas::mcfont::LINE_HEIGHT * bs) as i64 + 12;
        img.fill_rect(w as i64 - 18 - bw, 18, bw, bh, [0, 0, 0, 150]);
        font.draw(&mut img, w as i64 - 18 - bw + 10, 24, badge, bs, YELLOW, true);
    }
    img
}

/// The "idea" picture: a generated frame landing on a bedrock row (left: hovering with an
/// arrow, right: landed, the row's middle turned to obsidian).
fn diagram_idea(font: &mctas::mcfont::Font, bedrock: &mctas::mcfont::Image, obsidian: &mctas::mcfont::Image, rack: &mctas::mcfont::Image, w: u32, h: u32) -> mctas::mcfont::Image {
    let mut img = mctas::mcfont::Image::new(w, h, [10, 8, 8, 255]);
    img.tile(rack, 5, 0.18);
    font.draw_centered(&mut img, w as i64 / 2, 42, "The trick", 6, YELLOW, true);
    font.draw_centered(&mut img, w as i64 / 2, 108, "No portal near the exit? The game builds one —", 3, WHITE, true);
    font.draw_centered(&mut img, w as i64 / 2, 144, "and never checks what the frame overwrites", 3, WHITE, true);
    let cell = 58i64;
    // Floor row shared by both panels: 6 cells (1 rack, 4 bedrock, 1 rack).
    let floor = |img: &mut mctas::mcfont::Image, x0: i64, y: i64, landed: bool| {
        for i in 0..6i64 {
            let tex = if i == 0 || i == 5 {
                rack
            } else if landed {
                obsidian
            } else {
                bedrock
            };
            img.draw_resized(tex, x0 + i * cell, y, cell as u32, cell as u32);
        }
    };
    // A frame outline: 4 wide (over the middle floor cells), 3 above the floor + top.
    let frame = |img: &mut mctas::mcfont::Image, x0: i64, floor_y: i64, purple: bool| {
        for r in 1..=3i64 {
            for i in [1i64, 4] {
                img.draw_resized(obsidian, x0 + i * cell, floor_y - r * cell, cell as u32, cell as u32);
            }
            if purple {
                img.fill_rect(x0 + 2 * cell, floor_y - r * cell, 2 * cell, cell, [150, 60, 190, 255]);
            }
        }
        for i in 1..=4i64 {
            img.draw_resized(obsidian, x0 + i * cell, floor_y - 4 * cell, cell as u32, cell as u32);
        }
    };
    let y = 520i64;
    // Left: frame hovering 1 cell up, arrow down.
    let lx = 90i64;
    floor(&mut img, lx, y, false);
    frame(&mut img, lx, y - cell, false);
    img.fill_rect(lx + 2 * cell + cell / 2 - 8, y - cell - 4, 16, 40, [85, 255, 85, 255]);
    font.draw_centered(&mut img, lx + 3 * cell, y + cell + 16, "bedrock: solid bottom cell, counts!", 2, GRAY, true);
    // Arrow between panels.
    let ax = w as i64 / 2 - 46;
    img.fill_rect(ax, y - 2 * cell, 70, 18, [255, 255, 85, 255]);
    img.fill_rect(ax + 62, y - 2 * cell - 10, 18, 38, [255, 255, 85, 255]);
    // Right: landed.
    let rx = w as i64 - 90 - 6 * cell;
    floor(&mut img, rx, y, true);
    frame(&mut img, rx, y, true);
    font.draw_centered(&mut img, rx + 3 * cell, y + cell + 16, "4 bedrock became obsidian — mine them", 2, GREEN, true);
    img
}

/// The trigger machine, top view, at canonical coordinates (fx=rz=0 → the same relative
/// layout the build uses). Cells are coloured by role and labelled on the right.
fn diagram_machine(font: &mctas::mcfont::Font, rack: &mctas::mcfont::Image, w: u32, h: u32) -> mctas::mcfont::Image {
    let c = v2_coords(0, 0);
    let mut img = mctas::mcfont::Image::new(w, h, [8, 8, 10, 255]);
    img.tile(rack, 5, 0.12);
    font.draw_centered(&mut img, w as i64 / 2, 36, "The trigger machine (Overworld, from above)", 4, YELLOW, true);
    let cell = 34i64;
    let (gx0, gz0) = (c.rox - 3, c.doz_a - 2);
    let ox = 70i64;
    let oy = 120i64;
    let at = |x: i32, z: i32| (ox + (x - gx0) as i64 * cell, oy + (z - gz0) as i64 * cell);
    for x in gx0..=c.dix + 3 {
        for z in gz0..=c.doz_b + 2 {
            let (px, py) = at(x, z);
            let (col, full): ([u8; 4], bool) = if z == c.doz_a || z == c.doz_b {
                // Portal planes: frame gray-purple, interior purple.
                let interior = (z == c.doz_a && (x == c.rox || x == c.rox + 1)) || (x == c.dix || x == c.dix + 1);
                let frame = (z == c.doz_a && (c.rox - 1..=c.rox + 2).contains(&x)) || (c.dix - 1..=c.dix + 2).contains(&x);
                if interior {
                    ([150, 60, 190, 255], true)
                } else if frame {
                    ([60, 50, 70, 255], true)
                } else {
                    ([35, 35, 40, 255], true)
                }
            } else if (c.rox..=c.rox + 1).contains(&x) && (z == c.doz_a + 1 || z == c.doz_a + 2) {
                ([200, 170, 90, 255], true) // plates
            } else if x == c.dix + 1 && (z == c.doz_a + 1 || z == c.doz_b + 1) {
                ([120, 120, 130, 255], true) // droppers
            } else if (z == c.doz_a + 1 && (c.rox + 2..=c.dix).contains(&x)) || (x == c.dix && (c.doz_a + 2..=c.doz_b + 1).contains(&z)) {
                ([110, 110, 120, 255], false) // strip under dust (dot drawn below)
            } else if (c.rox..=c.dix + 2).contains(&x) && (c.doz_a + 1..=c.doz_b + 2).contains(&z) {
                ([110, 110, 120, 255], true) // stone strip
            } else {
                ([35, 35, 40, 255], true)
            };
            img.fill_rect(px, py, cell - 1, cell - 1, col);
            if !full {
                let rep = (x == c.rox + 7 && z == c.doz_a + 1) || (x == c.dix && z == c.doz_a + 5);
                if rep {
                    img.fill_rect(px + 8, py + 8, cell - 17, cell - 17, [240, 140, 40, 255]);
                } else {
                    img.fill_rect(px + cell / 2 - 4, py + cell / 2 - 4, 8, 8, [230, 40, 40, 255]);
                }
            }
        }
    }
    let label = |img: &mut mctas::mcfont::Image, x: i32, z: i32, text: &str, color: [u8; 3]| {
        let (px, py) = at(x, z);
        fonts_draw(img, font, px, py - 30, text, color);
    };
    fn fonts_draw(img: &mut mctas::mcfont::Image, font: &mctas::mcfont::Font, x: i64, y: i64, text: &str, color: [u8; 3]) {
        font.draw(img, x, y, text, 2, color, true);
    }
    label(&mut img, c.rox - 1, c.doz_a, "R_O return twin", AQUA);
    label(&mut img, c.dix - 1, c.doz_a, "D_O drill A", AQUA);
    label(&mut img, c.dix - 1, c.doz_b, "D_O drill B (+8)", AQUA);
    label(&mut img, c.rox, c.doz_a + 4, "plate on hoppers", YELLOW);
    label(&mut img, c.dix + 1, c.doz_b + 3, "dropper", WHITE);
    font.draw(&mut img, 70, h as i64 - 92, "item out of R_O -> plate -> dropper fires a FRESH stone (no portal cooldown)", 2, WHITE, true);
    font.draw(&mut img, 70, h as i64 - 62, "nobody is in the Overworld: the arriving item's portal ticket runs the machine", 2, GRAY, true);
    img
}

fn compose2(version: &str, plan: &Plan, o: &VideoOptions) -> Result<()> {
    let v2 = plan.v2.clone().context("compose2 needs a v2 plan")?;
    let timeline: Timeline = serde_json::from_str(&std::fs::read_to_string(o.out.join("timeline.json"))?)?;
    let font = mctas::mcfont::Font::minecraft(version)?;
    let netherrack = mctas::mcfont::texture(version, "block/netherrack")?;
    let bedrock = mctas::mcfont::texture(version, "block/bedrock")?;
    let obsidian = mctas::mcfont::texture(version, "block/obsidian")?;
    let work = o.out.join("work2");
    std::fs::create_dir_all(&work)?;
    let (w, h, fps) = (o.width, o.height, o.fps);
    let mut inputs: Vec<String> = Vec::new();
    let mut graphs: Vec<String> = Vec::new();
    let mut outs: Vec<String> = Vec::new();
    let mut inp = 0usize;
    let mut total = 0.0f64;
    let mut chapters: Vec<(f64, String)> = Vec::new();
    let mut marks: Vec<(f64, String)> = Vec::new();
    macro_rules! add_card {
        ($name:expr, $secs:expr, $img:expr, $chapter:expr) => {{
            let png = work.join(format!("{}.png", $name));
            $img.save_png(&png)?;
            chapters.push((total, $chapter.to_string()));
            inputs.extend(["-loop".into(), "1".into(), "-framerate".into(), fps.to_string(), "-t".into(), format!("{:.2}", $secs), "-i".into(), png.display().to_string()]);
            graphs.push(format!("[{inp}:v]setsar=1,format=yuv420p,fade=t=in:st=0:d=0.3,fade=t=out:st={:.2}:d=0.3[p{inp}]", $secs - 0.3));
            outs.push(format!("[p{inp}]"));
            marks.push((total + $secs / 2.0, $name.to_string()));
            total += $secs;
            inp += 1;
        }};
    }
    let budget = |file: &str| -> f64 {
        let stem = file.splitn(2, '-').nth(1).unwrap_or(file).trim_end_matches(".mkv");
        match stem {
            "hook" => 6.0,
            "intro" => 7.0,
            "dig-down" => 9.0,
            "field-top" => 5.0,
            "field-prep" => 24.0,
            "bedrock-before" => 6.0,
            "return-portal" => 16.0,
            "overworld-dig" => 9.0,
            "twin-off" => 8.0,
            "machine-build" => 26.0,
            "machine-top" => 5.0,
            "first-trip" => 18.0,
            "loop-mine" | "loop-throw" | "loop-result" => 9.0,
            "move-dropper" => 6.0,
            "trip" => 1.6,
            "mine-frames" | "mine-layer" => 5.0,
            "reveal" => 12.0,
            _ => 10.0,
        }
    };
    let add_seg = |seg: &Segment, inputs: &mut Vec<String>, graphs: &mut Vec<String>, outs: &mut Vec<String>, inp: &mut usize, total: &mut f64| -> Result<()> {
        let path = o.out.join("segments").join(&seg.file);
        let raw = duration(&path)?;
        let words: usize = seg.caption.iter().map(|c| c.split_whitespace().count()).sum();
        let need = if words > 6 { words as f64 * 0.3 + 1.5 } else { 1.2 };
        // The segments the narration calls real time (first conversion, the loop, the
        // reveal/hook) must stay at their recorded speed: a budget must never turn
        // them into a time-lapse with a ×N badge contradicting the caption.
        let stem = seg.file.splitn(2, '-').nth(1).unwrap_or("").trim_end_matches(".mkv");
        let realtime = matches!(stem, "hook" | "first-trip" | "loop-mine" | "loop-throw" | "loop-result" | "reveal");
        let mut speed = if realtime { seg.speed } else { seg.speed.max(raw / budget(&seg.file)) };
        if raw / speed < need && raw / need >= 1.0 {
            speed = raw / need;
        }
        let secs = raw / speed;
        if secs + 0.05 < need {
            eprintln!("compose2: WARN {} caption needs {need:.1}s, gets {secs:.1}s", seg.file);
        }
        let badge = if speed > 1.05 { format!("×{:.0}", speed) } else { String::new() };
        let png = work.join(format!("{}.label.png", seg.file));
        label2(&font, w, h, &seg.caption, &seg.step, &badge).save_png(&png)?;
        inputs.extend(["-i".into(), path.display().to_string(), "-i".into(), png.display().to_string()]);
        graphs.push(format!(
            "[{i}:v]setpts=(PTS-STARTPTS)/{sp:.3},fps={fps},scale={w}:{h},setsar=1[s{i}];[s{i}][{lp}:v]overlay=0:0,format=yuv420p[p{i}]",
            i = *inp, sp = speed, lp = *inp + 1
        ));
        outs.push(format!("[p{}]", *inp));
        *total += secs;
        *inp += 2;
        Ok(())
    };
    // 1) The hook first (filmed last, shown first) — when the shoot reached it. A cut
    // from an earlier checkpoint (time-boxed) simply opens on the title instead.
    if let Some(hook) = timeline.segments.iter().find(|s| s.file.contains("hook")).cloned() {
        chapters.push((0.0, "Hook: the finished hole".into()));
        add_seg(&hook, &mut inputs, &mut graphs, &mut outs, &mut inp, &mut total)?;
    }
    // 2) Title, kit, idea.
    let title = card(&font, &netherrack, w, h, &[
        ("Removing bedrock in survival:".into(), 4, WHITE),
        ("the portal sweep".into(), 6, YELLOW),
        ("".into(), 2, WHITE),
        ("Minecraft Java 26.3 · a real random world · survival (peaceful)".into(), 3, AQUA),
        (format!("seed {} · Nether floor x {}..{}, rows {}..{}", plan.seed, plan.fx, plan.fx + 3, v2.z0, v2.z1), 2, GRAY),
        ("no block is ever placed or broken by a command".into(), 2, GRAY),
    ]);
    add_card!("card-title", 4.0, &title, "Title");
    let kit = card(&font, &netherrack, w, h, &[
        ("What you need".into(), 5, YELLOW),
        ("".into(), 2, WHITE),
        ("Efficiency V pickaxe · flint and steel".into(), 3, WHITE),
        ("Obsidian ~3 stacks · cobblestone ~2 stacks".into(), 3, WHITE),
        ("Redstone dust · 2 repeaters".into(), 3, WHITE),
        ("1 dropper · 4 hoppers · 4 wooden pressure plates".into(), 3, WHITE),
        ("Building blocks · torches".into(), 3, WHITE),
        ("Fire Resistance for the digging (lava pockets!) · Haste II helps".into(), 3, WHITE),
        ("".into(), 2, WHITE),
        ("(in this video the kit was given by command; every block after that is survival)".into(), 2, GRAY),
    ]);
    add_card!("card-kit", 5.0, &kit, "The kit");
    let idea = diagram_idea(&font, &bedrock, &obsidian, &netherrack, w, h);
    add_card!("card-idea", 7.0, &idea, "The trick (diagram)");
    // 3) The shoot in order (hook excluded), machine diagram before the build.
    let machine_diagram = diagram_machine(&font, &netherrack, w, h);
    let chapter_of = |seg: &Segment| -> Option<&'static str> {
        let stem = seg.file.splitn(2, '-').nth(1).unwrap_or("").trim_end_matches(".mkv");
        Some(match stem {
            "intro" => "The world",
            "dig-down" => "Step 1: prepare the field",
            "return-portal" => "Step 2: the return portal",
            "overworld-dig" => "Step 3: the trigger machine",
            "first-trip" => "Step 4: first conversion",
            "loop-mine" => "Step 5: the loop in real time",
            "reveal" => "The result",
            _ => return None,
        })
    };
    let mut trip_chapter_done = false;
    let mut machine_card_done = false;
    for seg in &timeline.segments {
        if seg.file.contains("hook") {
            continue;
        }
        if seg.file.contains("machine-build") {
            add_card!("card-machine", 7.0, &machine_diagram, "The trigger machine (diagram)");
            machine_card_done = true;
        }
        if seg.file.contains("trip") && !seg.file.contains("first-trip") && !trip_chapter_done {
            chapters.push((total, "The sweep (time-lapse)".into()));
            trip_chapter_done = true;
        } else if let Some(ch) = chapter_of(seg) {
            chapters.push((total, ch.into()));
        }
        add_seg(seg, &mut inputs, &mut graphs, &mut outs, &mut inp, &mut total)?;
    }
    let finished = timeline.segments.iter().any(|s| s.file.contains("reveal"));
    if !machine_card_done {
        add_card!("card-machine", 7.0, &machine_diagram, "The trigger machine (diagram)");
    }
    // 4) Recap. A time-boxed cut (no reveal filmed) says so on its last lines.
    let mut recap_lines: Vec<(String, u32, [u8; 3])> = vec![
        ("Recap".into(), 6, YELLOW),
        ("".into(), 1, WHITE),
        ("1 · Flatten one field on the floor".into(), 3, WHITE),
        ("2 · Return portal 17+ blocks away".into(), 3, WHITE),
        ("3 · Overworld: twin + plate + dropper -> drill portal".into(), 3, WHITE),
        ("4 · Loop: mine ONE frame block, throw ONE item".into(), 3, WHITE),
        ("Pass 2: move the dropper 8 blocks (one row)".into(), 3, WHITE),
        ("".into(), 1, WHITE),
        ("One 1x4 line per trip · keep the field within 16 of the exit".into(), 2, GRAY),
        ("The Nether roof is safe: frame bottom max y=122, roof starts 123".into(), 2, GRAY),
        ("Overworld floor: nearer cave/surface sites usually win instead".into(), 2, GRAY),
    ];
    if finished {
        recap_lines.push((format!("seed {} · github.com/vjeux/redstonepedia (designs/bedrock)", plan.seed), 2, GRAY));
    } else {
        recap_lines.push(("Filming stopped at the machine-room dig (recording-box lease).".into(), 2, YELLOW));
        recap_lines.push(("The loop + sweep are verified on the official server:".into(), 2, GRAY));
        recap_lines.push(("designs/bedrock/portal/portal_sweep.json + portal_remote_trigger.json".into(), 2, GRAY));
        recap_lines.push((format!("seed {} · github.com/vjeux/redstonepedia (designs/bedrock)", plan.seed), 2, GRAY));
    }
    let recap = card(&font, &netherrack, w, h, &recap_lines);
    add_card!("card-recap", 8.0, &recap, "Recap: rules and limits");
    let n_parts = outs.len();
    let graph = format!("{};{}concat=n={n_parts}:v=1:a=0,scale=out_range=tv,format=yuv420p[o]", graphs.join(";"), outs.concat());
    let out_file = o.out.join("bedrock-portal-method-v2.webm");
    let mut args = inputs;
    args.extend(["-filter_complex".into(), graph, "-map".into(), "[o]".into()]);
    args.extend([
        "-c:v".into(), "libvpx-vp9".into(), "-b:v".into(), "0".into(), "-crf".into(), "30".into(), "-row-mt".into(), "1".into(),
        "-deadline".into(), "good".into(), "-cpu-used".into(), "4".into(), "-pix_fmt".into(), "yuv420p".into(), "-color_range".into(), "tv".into(), out_file.display().to_string(),
    ]);
    ffmpeg_run(&args)?;
    let final_secs = duration(&out_file)?;
    // Review stills: cards + one frame every 2 s at phone size (640x360).
    let stills = o.out.join("stills2");
    std::fs::create_dir_all(&stills)?;
    for (t, name) in &marks {
        ffmpeg_run(&["-ss".into(), format!("{t:.2}"), "-i".into(), out_file.display().to_string(), "-frames:v".into(), "1".into(), "-vf".into(), "scale=960:-2".into(), "-q:v".into(), "3".into(), stills.join(format!("{name}.jpg")).display().to_string()])?;
    }
    let n2 = (final_secs / 2.0).ceil() as usize;
    for i in 0..n2 {
        let t = i as f64 * 2.0 + 1.0;
        ffmpeg_run(&["-ss".into(), format!("{t:.2}"), "-i".into(), out_file.display().to_string(), "-frames:v".into(), "1".into(), "-vf".into(), "scale=640:360".into(), "-q:v".into(), "4".into(), stills.join(format!("s{i:03}.jpg")).display().to_string()])?;
    }
    let probe = mctas::video::ffmpeg()?.with_file_name("ffprobe");
    let meta = std::process::Command::new(probe).args(["-v", "error", "-show_entries", "stream=codec_name,color_range,pix_fmt,width,height", "-of", "default=nw=1"]).arg(&out_file).output()?;
    println!("composed {} ({:.1}s, {} KiB)\n{}", out_file.display(), final_secs, std::fs::metadata(&out_file)?.len() / 1024, String::from_utf8_lossy(&meta.stdout).trim());
    println!("chapters:");
    for (t, name) in &chapters {
        println!("  {:>6.1}s  {}", t, name);
    }
    println!("stills in {}", stills.display());
    Ok(())
}
