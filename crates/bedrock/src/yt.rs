//! `mctas bedrock yt`: the YouTube tutorial of the 11-block roof machine
//! (`designs/bedrock/machine-roof-11b.txt`, NOTES.md section 3), filmed in the REAL client on
//! the official server: a default-generated world of a seed, survival (peaceful), the actor mod
//! (`bridge/actor`) playing. Every block of the machine is placed by the player with the use key's
//! own code, aimed with the crosshair (`hit` is checked before every click); the press is a real
//! tap of ONE key bound to Attack and Use in the key binds screen. Commands only give the kit,
//! place the beacon's pyramid (captioned), move the camera between takes and freeze/step the game
//! for the slow-motion explanation (`/tick`, captioned).
//!
//! Phases (`--phase ...`):
//! * `scout`: generate the world, read the Nether roof and what lies under it from the region
//!   files, and pick the target T1 (a top-layer roof bedrock with no bedrock under it and a safe
//!   drop into a cavern), T2 (a second machine for the slow motion) and the beacon pyramid spot.
//! * `record` (`--from <checkpoint>` resumes): film the scenes (segments + a timeline with
//!   wall-clock events + game audio from OpenAL Soft's wave writer). `smoke` / `presslab`: the
//!   client actions and the press timing tried on a scratch copy, nothing kept.
//! * `review` (frames of every segment), `audiosync` (the presses' sounds against their wall
//!   times), `preview` (the overlays on real frames).
//! * `compose` / `short` / `thumb`: [`crate::yt_cut`]; `upload`: the three files to the
//!   `videos` release of vjeux/redstonepedia.

use crate::portal::{Blocks, Kind, Pos};
use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PLAYER: &str = "mctas";
/// The roof's top layer (the target bedrock's y).
pub const ROOF_Y: i32 = 127;

// The machine, relative to its target T (x east, z south), as the player builds it.
pub const A: Pos = [0, 1, 0];
pub const HEAD: Pos = [-1, 1, 0];
pub const TD: Pos = [0, 2, -1];
pub const PRESS: Pos = [0, 1, -1];
pub const X: Pos = [0, 2, 1];
pub const O: Pos = [1, 2, 1];
pub const D1: Pos = [1, 1, 1];
pub const D2: Pos = [2, 1, 1];
pub const D3: Pos = [2, 1, 2];
pub const SP: Pos = [2, 2, 0];
pub const RB: Pos = [2, 2, 1];
pub const O2: Pos = [2, 3, 1];
pub const L: Pos = [2, 3, 2];
/// Temporary netherrack: under the trapdoor (press spot), under the sticky piston, a step to
/// stand on for the top observer. All mined again.
pub const N1: Pos = [0, 1, -1];
pub const N2: Pos = [2, 1, 0];
pub const N3: Pos = [2, 1, -2];

// Hotbar of the machine kit.
const SLOT_PICK: u32 = 0;
const SLOT_EMPTY: u32 = 1; // pistons start here, then go to the offhand (F)
const SLOT_TRAPDOOR: u32 = 2;
const SLOT_NETHERRACK: u32 = 3;
const SLOT_DUST: u32 = 4;
const SLOT_STICKY: u32 = 5;
const SLOT_RBLOCK: u32 = 6;
const SLOT_OBSERVER: u32 = 7;
const SLOT_STONE: u32 = 8;

#[derive(Clone, Debug)]
pub struct YtOptions {
    pub seed: Option<i64>,
    pub out: PathBuf,
    pub phase: String,
    pub driver: String,
    pub from: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YtPlan {
    pub seed: i64,
    /// The tutorial machine's target (top roof layer) and the slow-motion machine's.
    pub t1: Pos,
    pub t2: Pos,
    /// Where the beacon goes (on top of its pyramid); the pyramid is under it.
    pub beacon: Pos,
    /// Under T1: the last solid block of the shaft and the cavern floor (the drop).
    pub shaft_bottom: i32,
    pub floor: i32,
}

/// One recorded segment: wall-clock span (seconds since the epoch, from ffmpeg's first frame),
/// the audio file's size at both ends, and events (wall time, label) for the cut.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Seg {
    pub file: String,
    pub name: String,
    pub scene: String,
    pub start: f64,
    pub end: f64,
    pub wav: String,
    pub wav_start: u64,
    pub wav_end: u64,
    /// Wall time of the `wav_start` sample (the end sample's is `end`).
    #[serde(default)]
    pub wav_t_start: f64,
    pub events: Vec<(f64, String)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub segments: Vec<Seg>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct State {
    done: Vec<String>,
}

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn run(version: &str, o: YtOptions) -> Result<()> {
    std::fs::create_dir_all(&o.out)?;
    match o.phase.as_str() {
        "scout" => {
            let plan = scout(version, &o)?;
            println!("plan: {}", serde_json::to_string(&plan)?);
            Ok(())
        }
        "record" => record(version, &load_plan(&o)?, &o),
        "smoke" => smoke(version, &load_plan(&o)?, &o),
        "presslab" => presslab(version, &load_plan(&o)?, &o),
        "compose" => crate::yt_cut::compose(version, &load_plan(&o)?, &o),
        "review" => crate::yt_cut::review(&o),
        "audiosync" => crate::yt_cut::audiosync(version, &o),
        "preview" => crate::yt_cut::preview(version, &load_plan(&o)?, &o),
        "upload" => {
            for f in ["bedrock-roof-11-blocks.webm", "bedrock-roof-11-blocks-short.webm", "bedrock-roof-11-blocks-thumb.png"] {
                let url = mctas::video::upload_release_asset("vjeux/redstonepedia", "videos", &o.out.join(f))?;
                println!("{f}: {url}");
            }
            Ok(())
        }
        "short" => crate::yt_cut::short(version, &load_plan(&o)?, &o),
        "thumb" => crate::yt_cut::thumb(version, &load_plan(&o)?, &o),
        "all" => {
            let plan = if o.out.join("plan.json").exists() { load_plan(&o)? } else { scout(version, &o)? };
            record(version, &plan, &o)?;
            crate::yt_cut::compose(version, &plan, &o)?;
            crate::yt_cut::short(version, &plan, &o)?;
            crate::yt_cut::thumb(version, &plan, &o)
        }
        other => bail!("unknown --phase {other} (scout|record|smoke|presslab|review|audiosync|preview|compose|short|thumb|upload|all)"),
    }
}

pub fn load_plan(o: &YtOptions) -> Result<YtPlan> {
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
        view_distance: 8,
        simulation_distance: 6,
        ..Default::default()
    }
}

pub fn copy_tree(from: &Path, to: &Path) -> Result<()> {
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

// ---- scout ---------------------------------------------------------------------------------

fn is_lava(s: &str) -> bool {
    s.starts_with("minecraft:lava") || s.starts_with("minecraft:fire") || s.starts_with("minecraft:magma_block")
}

/// Under the roof top at (x, z): the shaft (solid, no bedrock, from y 126 down to `bottom`), the
/// cavern below it and its floor; None when the column is unusable for a safe drop.
fn drop_column(b: &Blocks, x: i32, z: i32) -> Option<(i32, i32, usize)> {
    if !b.get([x, ROOF_Y, z]).starts_with("minecraft:bedrock") {
        return None;
    }
    // No bedrock under the top layer, no lava next to the shaft.
    let mut y = ROOF_Y - 1;
    while y > 40 && b.kind([x, y, z]) == Kind::Solid {
        if b.get([x, y, z]).starts_with("minecraft:bedrock") && y < ROOF_Y {
            return None;
        }
        y -= 1;
    }
    let bottom = y + 1;
    if bottom > ROOF_Y - 3 || b.kind([x, y, z]) != Kind::Replaceable || is_lava(b.get([x, y, z])) {
        return None;
    }
    // The cavern: air down to a solid floor (the drop is made with Slow Falling).
    let mut f = y;
    while f > 20 && b.kind([x, f, z]) == Kind::Replaceable && !is_lava(b.get([x, f, z])) {
        f -= 1;
    }
    let fall = bottom - f - 1;
    if !(10..=45).contains(&fall) || b.kind([x, f, z]) != Kind::Solid || is_lava(b.get([x, f, z])) {
        return None;
    }
    // No lava next to the shaft (it would pour in) nor around the landing.
    for yy in bottom - 1..=ROOF_Y - 1 {
        for dx in -2..=2 {
            for dz in -2..=2 {
                if is_lava(b.get([x + dx, yy, z + dz])) {
                    return None;
                }
            }
        }
    }
    for yy in f..f + 4 {
        for dx in -3..=3 {
            for dz in -3..=3 {
                if is_lava(b.get([x + dx, yy, z + dz])) {
                    return None;
                }
            }
        }
    }
    // A roomy cavern reads better on video: air cells around the landing.
    let mut air = 0;
    for yy in f + 1..bottom {
        for dx in -4..=4 {
            for dz in -4..=4 {
                if b.kind([x + dx, yy, z + dz]) == Kind::Replaceable {
                    air += 1;
                }
            }
        }
    }
    Some((bottom, f, air))
}

fn scout(version: &str, o: &YtOptions) -> Result<YtPlan> {
    let seed = o.seed.unwrap_or_else(|| {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        (t ^ ((std::process::id() as u128) << 32)) as i64
    });
    eprintln!("scout: seed {seed}");
    let mut srv = VanillaServer::start(&server_opts(version, seed, None))?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -64 -64 63 63")?;
    let t0 = Instant::now();
    let b = loop {
        match srv.read_dim_box("the_nether", [-64, 30, -64], [63, ROOF_Y, 63]) {
            Ok(s) => break Blocks::new(s.region),
            Err(e) => {
                if t0.elapsed() > Duration::from_secs(300) {
                    return Err(e.context("the nether never generated"));
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    };
    eprintln!("scout: nether read after {:.0}s", t0.elapsed().as_secs_f64());
    let mut cands = Vec::new();
    for x in -30..=24 {
        for z in -30..=30 {
            if let Some((bottom, floor, air)) = drop_column(&b, x, z) {
                // A short dig first (the shaft under the roof), then a roomy cavern.
                if bottom >= 110 {
                    cands.push((-bottom, std::cmp::Reverse(air.min(400)), x.abs() + z.abs(), x, z, bottom, floor));
                }
            }
        }
    }
    cands.sort();
    for c in cands.iter().take(5) {
        eprintln!("scout: candidate ({}, {}) shaft to y{} floor y{} air {}", c.3, c.4, c.5, c.6, (c.1).0);
    }
    let c = cands.first().context("scout: no roof column with a safe drop near the origin")?;
    let t1 = [c.3, ROOF_Y, c.4];
    let plan = YtPlan { seed, t1, t2: add(t1, [12, 0, 0]), beacon: add(t1, [-12, 5, 1]), shaft_bottom: c.5, floor: c.6 };
    srv.cmd_ok("save-all flush")?;
    let world_src = o.out.join("world-src");
    let _ = std::fs::remove_dir_all(&world_src);
    let src = srv.dir.join(&srv.level_name);
    drop(srv);
    copy_tree(&src, &world_src)?;
    std::fs::write(o.out.join("plan.json"), serde_json::to_string_pretty(&plan)?)?;
    Ok(plan)
}

// ---- the actor and the recorder --------------------------------------------------------------

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
                if !text.trim().is_empty() {
                    let _ = std::fs::remove_file(&done);
                    let text = text.trim().to_string();
                    if let Some(err) = text.strip_prefix("error ") {
                        bail!("actor `{line}`: {err}");
                    }
                    return Ok(text);
                }
            }
            if t0.elapsed() > timeout {
                bail!("actor `{line}` timed out after {timeout:?}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// ffmpeg grabbing the X display; its log names the first frame's wall-clock time.
struct Recorder {
    child: std::process::Child,
    log: PathBuf,
}

impl Recorder {
    fn start(display: &mctas::video::Display, w: u32, h: u32, fps: u32, out: &Path) -> Result<Recorder> {
        let log = out.with_extension("log");
        let child = std::process::Command::new(mctas::video::ffmpeg()?)
            .args(["-loglevel", "info", "-y", "-f", "x11grab", "-draw_mouse", "0"])
            .args(["-framerate", &fps.to_string(), "-video_size", &format!("{w}x{h}")])
            .args(["-i", &display.name(), "-c:v", "libx264", "-preset", "ultrafast", "-crf", "14", "-pix_fmt", "yuv420p"])
            .arg(out)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(&log)?)
            .spawn()
            .context("starting ffmpeg")?;
        Ok(Recorder { child, log })
    }

    /// Stop; returns the first frame's wall time (seconds since the epoch).
    fn stop(mut self) -> Result<f64> {
        use std::io::Write;
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q");
        }
        let _ = self.child.wait();
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        let start = text
            .split("start: ")
            .nth(1)
            .and_then(|r| r.split(',').next())
            .and_then(|v| v.trim().parse::<f64>().ok())
            .with_context(|| format!("no start time in {}", self.log.display()))?;
        Ok(start)
    }
}

// ---- production ------------------------------------------------------------------------------

pub struct Prod {
    srv: VanillaServer,
    actor: Actor,
    display: mctas::video::Display,
    _client: mctas::video::Client,
    rec: Option<Recorder>,
    out: PathBuf,
    pub plan: YtPlan,
    timeline: Timeline,
    state: State,
    log: PathBuf,
    wav: PathBuf,
    width: u32,
    height: u32,
    fps: u32,
}

fn launch_client(version: &str, o: &YtOptions, display: &mctas::video::Display, port: u16, game_dir: &Path) -> Result<mctas::video::Client> {
    match o.driver.as_str() {
        "fabric" => {
            let build = mctas::video::build_client(version)?;
            let mut fabric = mctas::fabric::install_loader(version)?;
            let jar = mctas::fabric::build_actor(version, &build, &fabric.libs)?;
            fabric.mods = vec![jar];
            mctas::fabric::launch(&build, &fabric, display, Some(port), o.width, o.height, game_dir)
        }
        "launcher" => {
            let build = mctas::video::build_actor_client(version)?;
            mctas::video::Client::launch_main(&build, display, Some(port), o.width, o.height, game_dir, "dev.vjeux.mctas.client.ActorMain", &[], &[])
        }
        other => bail!("unknown --driver {other} (fabric|launcher)"),
    }
}

fn start_prod(version: &str, plan: &YtPlan, o: &YtOptions, world: PathBuf) -> Result<Prod> {
    let audio = o.out.join("audio");
    std::fs::create_dir_all(&audio)?;
    let wav = audio.join(format!("session-{}.wav", now() as u64));
    let conf = o.out.join("alsoft.conf");
    std::fs::write(&conf, format!("[general]\ndrivers = wave\nfrequency = 48000\n[wave]\nfile = {}\n", wav.display()))?;
    // SAFETY: single-threaded here; the server and the client are child processes started below.
    unsafe {
        std::env::set_var("ALSOFT_DRIVERS", "wave");
        std::env::set_var("ALSOFT_CONF", &conf);
        if std::env::var_os("LP_NUM_THREADS").is_none() {
            std::env::set_var("LP_NUM_THREADS", "24");
        }
        std::env::set_var(
            "MCTAS_CLIENT_OPTIONS_EXTRA",
            "soundCategory_master:1.0;soundCategory_music:0.0;soundCategory_record:0.0;soundCategory_ambient:0.35;soundCategory_weather:0.0;\
             soundCategory_block:1.0;soundCategory_player:0.8;soundCategory_ui:0.6;gamma:1.0;chatVisibility:2",
        );
    }
    let srv = VanillaServer::start(&server_opts(version, plan.seed, Some(world)))?;
    let port = srv.game_port();
    let display = mctas::video::Display::start(o.width, o.height)?;
    eprintln!("[yt] display :{} server port {port}", display.number);
    let game_dir = o.out.join("client");
    let mut actor = Actor::new(&game_dir)?;
    let client = launch_client(version, o, &display, port, &game_dir)?;
    let t0 = Instant::now();
    loop {
        match actor.cmd_timeout("ping", Duration::from_secs(20)) {
            Ok(s) if s.contains("pos=") => break,
            _ => {}
        }
        if t0.elapsed() > Duration::from_secs(300) {
            bail!("the client never joined; see {}", client.log.display());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    std::fs::create_dir_all(o.out.join("segments"))?;
    let timeline: Timeline = std::fs::read_to_string(o.out.join("timeline.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let state: State = std::fs::read_to_string(o.out.join("state.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    Ok(Prod {
        srv,
        actor,
        display,
        _client: client,
        rec: None,
        out: o.out.clone(),
        plan: plan.clone(),
        timeline,
        state,
        log: o.out.join("production.log"),
        wav,
        width: o.width,
        height: o.height,
        fps: o.fps,
    })
}

impl Prod {
    pub fn loga(&self, line: &str) {
        eprintln!("[yt] {line}");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.log) {
            use std::io::Write;
            let _ = writeln!(f, "{:.3} {line}", now());
        }
    }

    fn rcon(&mut self, c: &str) -> Result<String> {
        self.srv.cmd(c)
    }

    fn rcon_ok(&mut self, c: &str) -> Result<String> {
        self.srv.cmd_ok(c)
    }

    fn nether(&mut self, c: &str) -> Result<String> {
        self.srv.cmd_ok(&format!("execute in minecraft:the_nether run {c}"))
    }

    fn test(&mut self, p: Pos, pred: &str) -> Result<bool> {
        let out = self.rcon(&format!("execute in minecraft:the_nether if block {} {} {} {pred}", p[0], p[1], p[2]))?;
        if out.contains("Test passed") {
            Ok(true)
        } else if out.contains("Test failed") {
            Ok(false)
        } else {
            bail!("execute if block: {out}")
        }
    }

    fn expect(&mut self, p: Pos, pred: &str, what: &str) -> Result<()> {
        if self.test(p, pred)? {
            self.loga(&format!("ok {p:?} is {pred} ({what})"));
            Ok(())
        } else {
            bail!("{what}: {p:?} is not {pred}")
        }
    }

    fn a(&mut self, line: &str) -> Result<String> {
        self.actor.cmd_timeout(line, Duration::from_secs(120))
    }

    fn wait(&mut self, ticks: u32) -> Result<()> {
        self.a(&format!("wait {ticks}"))?;
        Ok(())
    }

    fn select(&mut self, slot: u32) -> Result<()> {
        self.a(&format!("select {slot}"))?;
        Ok(())
    }

    fn look_at(&mut self, p: [f64; 3], ticks: u32) -> Result<()> {
        self.a(&format!("look {:.3} {:.3} {:.3} {ticks}", p[0], p[1], p[2]))?;
        Ok(())
    }

    fn lookdir(&mut self, yaw: f32, pitch: f32, ticks: u32) -> Result<()> {
        self.a(&format!("lookdir {yaw} {pitch} {ticks}"))?;
        Ok(())
    }

    fn walk(&mut self, x: i32, z: i32) -> Result<()> {
        self.actor.cmd_timeout(&format!("walk {x} {z}"), Duration::from_secs(60))?;
        Ok(())
    }

    fn walk_jump(&mut self, x: i32, z: i32) -> Result<()> {
        self.actor.cmd_timeout(&format!("walk {x} {z} jump"), Duration::from_secs(60))?;
        Ok(())
    }

    /// Walk relative-to-T waypoints (x, z) of machine `t`.
    fn route(&mut self, t: Pos, pts: &[(i32, i32)]) -> Result<()> {
        for &(x, z) in pts {
            self.walk(t[0] + x, t[2] + z)?;
        }
        Ok(())
    }

    fn feet(&mut self) -> Result<[f64; 3]> {
        let s = self.a("pos")?;
        let p = s.split("pos=").nth(1).and_then(|r| r.split(' ').next()).with_context(|| format!("pos line: {s}"))?;
        let v: Vec<f64> = p.split(',').map(|x| x.parse()).collect::<std::result::Result<_, _>>()?;
        Ok([v[0], v[1], v[2]])
    }

    fn tp(&mut self, x: f64, y: f64, z: f64, yaw: f32, pitch: f32) -> Result<()> {
        self.nether(&format!("tp {PLAYER} {x:.3} {y:.3} {z:.3} {yaw} {pitch}"))?;
        self.wait(10)
    }

    pub fn event(&mut self, label: &str) {
        let t = now();
        if let Some(s) = self.timeline.segments.last_mut() {
            s.events.push((t, label.to_string()));
        }
        self.loga(&format!("event {label}"));
    }

    fn wav_size(&self) -> u64 {
        std::fs::metadata(&self.wav).map(|m| m.len()).unwrap_or(0)
    }

    /// Start a new segment (stopping the current one).
    fn seg(&mut self, name: &str, scene: &str) -> Result<()> {
        self.stop_rec()?;
        let idx = self.timeline.segments.len() + 1;
        let file = format!("{idx:02}-{name}.mkv");
        let path = self.out.join("segments").join(&file);
        let wav_t_start = now();
        let wav_start = self.wav_size();
        self.rec = Some(Recorder::start(&self.display, self.width, self.height, self.fps, &path)?);
        self.timeline.segments.push(Seg {
            file: file.clone(),
            name: name.into(),
            scene: scene.into(),
            wav: self.wav.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            wav_start,
            wav_t_start,
            ..Default::default()
        });
        self.loga(&format!("segment {file}"));
        // ffmpeg needs a moment before its first frame.
        std::thread::sleep(Duration::from_millis(400));
        Ok(())
    }

    fn stop_rec(&mut self) -> Result<()> {
        if let Some(r) = self.rec.take() {
            let end = now();
            let wav_end = self.wav_size();
            let start = r.stop()?;
            if let Some(s) = self.timeline.segments.last_mut() {
                s.start = start;
                s.end = end;
                s.wav_end = wav_end;
            }
            std::fs::write(self.out.join("timeline.json"), serde_json::to_string_pretty(&self.timeline)?)?;
        }
        Ok(())
    }

    fn save_state(&self) -> Result<()> {
        std::fs::write(self.out.join("state.json"), serde_json::to_string_pretty(&self.state)?)?;
        Ok(())
    }

    fn done(&self, tag: &str) -> bool {
        self.state.done.iter().any(|t| t == tag)
    }

    /// A resumable point: the world, the timeline and the state as of now.
    fn checkpoint(&mut self, tag: &str) -> Result<()> {
        self.stop_rec()?;
        self.rcon_ok("save-off")?;
        self.rcon_ok("save-all flush")?;
        let dst = self.out.join("checkpoints").join(tag);
        let _ = std::fs::remove_dir_all(&dst);
        let r = copy_tree(&self.srv.dir.join(&self.srv.level_name), &dst.join("world"));
        let _ = self.rcon_ok("save-on");
        r?;
        if !self.done(tag) {
            self.state.done.push(tag.to_string());
        }
        self.save_state()?;
        std::fs::write(dst.join("state.json"), serde_json::to_string_pretty(&self.state)?)?;
        std::fs::write(dst.join("timeline.json"), serde_json::to_string_pretty(&self.timeline)?)?;
        let pose = self.a("pose")?;
        let feet = self.feet()?;
        std::fs::write(dst.join("player.txt"), format!("{} {} {}\n{pose}\n", feet[0], feet[1], feet[2]))?;
        self.loga(&format!("checkpoint {tag}"));
        Ok(())
    }

    /// The crosshair must be on `block`'s `face` (what the next click will touch).
    fn check_hit(&mut self, block: Pos, face: &str) -> Result<String> {
        let h = self.a("hit")?;
        let want = format!("pos={},{},{} face={face}", block[0], block[1], block[2]);
        if !h.contains(&want) {
            bail!("the crosshair is not on {block:?} {face}: {h}");
        }
        Ok(h)
    }

    /// Aim at `point` (absolute), check the crosshair is on `block`'s `face`, then click the use
    /// key (sneaking when asked) and check `expect` holds `pred`.
    #[allow(clippy::too_many_arguments)]
    fn place(&mut self, slot: u32, block: Pos, face: &str, point: [f64; 3], sneak: bool, expect: Pos, pred: &str, what: &str) -> Result<()> {
        self.select(slot)?;
        // Sneak BEFORE aiming: the eye drops 0.35 when sneaking, which moves what the crosshair
        // touches.
        if sneak {
            self.a("sneak 1")?;
            self.wait(4)?;
        }
        self.look_at(point, 12)?;
        self.wait(6)?;
        self.check_hit(block, face)?;
        self.a("click use")?;
        if sneak {
            self.wait(2)?;
            self.a("sneak 0")?;
        }
        self.wait(4)?;
        self.event(&format!("placed {what}"));
        self.expect(expect, pred, what)
    }

    /// Aim at `point`, check the crosshair is on `block`, click the attack key: an instant break.
    fn mine(&mut self, block: Pos, face: &str, point: [f64; 3], what: &str) -> Result<()> {
        self.select(SLOT_PICK)?;
        self.look_at(point, 10)?;
        self.wait(4)?;
        self.check_hit(block, face)?;
        self.a("click attack")?;
        self.wait(4)?;
        self.event(&format!("mined {what}"));
        self.expect(block, "minecraft:air", what)
    }
}

/// The absolute center of `face` of block `p`, nudged by (u, v) inside the face (fractions).
pub fn face_point(p: Pos, face: &str, nudge: [f64; 3]) -> [f64; 3] {
    let c = [p[0] as f64 + 0.5 + nudge[0], p[1] as f64 + 0.5 + nudge[1], p[2] as f64 + 0.5 + nudge[2]];
    match face {
        "up" => [c[0], p[1] as f64 + 1.0, c[2]],
        "down" => [c[0], p[1] as f64, c[2]],
        "north" => [c[0], c[1], p[2] as f64],
        "south" => [c[0], c[1], p[2] as f64 + 1.0],
        "west" => [p[0] as f64, c[1], c[2]],
        _ => [p[0] as f64 + 1.0, c[1], c[2]],
    }
}

// ---- record ----------------------------------------------------------------------------------

fn record(version: &str, plan: &YtPlan, o: &YtOptions) -> Result<()> {
    let world = match &o.from {
        Some(tag) => {
            let dir = o.out.join("checkpoints").join(tag);
            for f in ["state.json", "timeline.json"] {
                std::fs::copy(dir.join(f), o.out.join(f))?;
            }
            let w = o.out.join("work-world");
            let _ = std::fs::remove_dir_all(&w);
            copy_tree(&dir.join("world"), &w)?;
            w
        }
        None => {
            let _ = std::fs::remove_file(o.out.join("state.json"));
            let _ = std::fs::remove_file(o.out.join("timeline.json"));
            let w = o.out.join("work-world");
            let _ = std::fs::remove_dir_all(&w);
            copy_tree(&o.out.join("world-src"), &w)?;
            w
        }
    };
    let mut p = start_prod(version, plan, o, world)?;
    let r = crate::yt_scenes::scenes(&mut p);
    let _ = p.stop_rec();
    if let Err(e) = &r {
        p.loga(&format!("FAILED: {e:#}"));
    }
    r
}

/// A quick check of the actor's new verbs on the real roof (no filming).
fn smoke(version: &str, plan: &YtPlan, o: &YtOptions) -> Result<()> {
    let w = o.out.join("smoke-world");
    let _ = std::fs::remove_dir_all(&w);
    copy_tree(&o.out.join("world-src"), &w)?;
    let mut p = start_prod(version, plan, o, w)?;
    crate::yt_scenes::smoke(&mut p)
}

/// The press timing lab on a copy of a checkpoint (`--from`), nothing filmed or kept.
fn presslab(version: &str, plan: &YtPlan, o: &YtOptions) -> Result<()> {
    let tag = o.from.as_deref().context("presslab needs --from <checkpoint>")?;
    let w = o.out.join("lab-world");
    let _ = std::fs::remove_dir_all(&w);
    copy_tree(&o.out.join("checkpoints").join(tag).join("world"), &w)?;
    let mut p = start_prod(version, plan, o, w)?;
    crate::yt_scenes::presslab(&mut p)
}

// Accessors for the scenes module.
impl Prod {
    pub fn cmd(&mut self, c: &str) -> Result<String> {
        self.rcon_ok(c)
    }
    pub fn cmd_raw(&mut self, c: &str) -> Result<String> {
        self.rcon(c)
    }
    pub fn in_nether(&mut self, c: &str) -> Result<String> {
        self.nether(c)
    }
    pub fn is(&mut self, p: Pos, pred: &str) -> Result<bool> {
        self.test(p, pred)
    }
    pub fn must(&mut self, p: Pos, pred: &str, what: &str) -> Result<()> {
        self.expect(p, pred, what)
    }
    pub fn act(&mut self, line: &str) -> Result<String> {
        self.a(line)
    }
    pub fn act_long(&mut self, line: &str, secs: u64) -> Result<String> {
        self.actor.cmd_timeout(line, Duration::from_secs(secs))
    }
    pub fn ticks(&mut self, n: u32) -> Result<()> {
        self.wait(n)
    }
    pub fn slot(&mut self, s: u32) -> Result<()> {
        self.select(s)
    }
    pub fn look(&mut self, pt: [f64; 3], ticks: u32) -> Result<()> {
        self.look_at(pt, ticks)
    }
    pub fn turn(&mut self, yaw: f32, pitch: f32, ticks: u32) -> Result<()> {
        self.lookdir(yaw, pitch, ticks)
    }
    pub fn go(&mut self, x: i32, z: i32) -> Result<()> {
        self.walk(x, z)
    }
    pub fn go_jump(&mut self, x: i32, z: i32) -> Result<()> {
        self.walk_jump(x, z)
    }
    pub fn go_route(&mut self, t: Pos, pts: &[(i32, i32)]) -> Result<()> {
        self.route(t, pts)
    }
    pub fn pos(&mut self) -> Result<[f64; 3]> {
        self.feet()
    }
    pub fn teleport(&mut self, x: f64, y: f64, z: f64, yaw: f32, pitch: f32) -> Result<()> {
        self.tp(x, y, z, yaw, pitch)
    }
    pub fn segment(&mut self, name: &str, scene: &str) -> Result<()> {
        self.seg(name, scene)
    }
    pub fn cut(&mut self) -> Result<()> {
        self.stop_rec()
    }
    pub fn is_done(&self, tag: &str) -> bool {
        self.done(tag)
    }
    pub fn mark(&mut self, tag: &str) -> Result<()> {
        self.checkpoint(tag)
    }
    pub fn hit_on(&mut self, block: Pos, face: &str) -> Result<String> {
        self.check_hit(block, face)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn put(&mut self, slot: u32, block: Pos, face: &str, point: [f64; 3], sneak: bool, expect: Pos, pred: &str, what: &str) -> Result<()> {
        self.place(slot, block, face, point, sneak, expect, pred, what)
    }
    pub fn dig(&mut self, block: Pos, face: &str, point: [f64; 3], what: &str) -> Result<()> {
        self.mine(block, face, point, what)
    }
    pub fn out_dir(&self) -> PathBuf {
        self.out.clone()
    }
}

pub const SLOTS: [u32; 9] = [SLOT_PICK, SLOT_EMPTY, SLOT_TRAPDOOR, SLOT_NETHERRACK, SLOT_DUST, SLOT_STICKY, SLOT_RBLOCK, SLOT_OBSERVER, SLOT_STONE];
