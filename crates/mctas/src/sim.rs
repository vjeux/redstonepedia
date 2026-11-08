//! Driving simulations.
//!
//! A [`Worker`] is one JVM running the bridge; it hosts any number of sandboxes (each its own
//! Minecraft level with its own clock). Requests are JSON lines; [`Worker::call`] sends one and
//! waits, [`Worker::send`] + [`Worker::recv`] pipeline many. A [`Pool`] runs one worker per
//! core-ish and spreads jobs over them.

use crate::bridge::{self, BridgeBuild, MAIN_CLASS};
use crate::paths;
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// A block position `[x, y, z]`.
pub type Pos = [i32; 3];

/// How blocks are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceMode {
    /// Exact states, no neighbor or shape updates, no `onPlace` (a structure-like paste).
    Raw,
    /// Like a player placing it: neighbor and shape updates, `onPlace` runs.
    Update,
    /// Shape updates only (flags 2: no block update to the neighbors), like a crop growing.
    Shape,
}

impl PlaceMode {
    fn as_str(self) -> &'static str {
        match self {
            PlaceMode::Raw => "raw",
            PlaceMode::Update => "update",
            PlaceMode::Shape => "shape",
        }
    }
}

/// JVM settings for a worker.
#[derive(Clone, Debug)]
pub struct WorkerConfig {
    pub version: String,
    /// Max heap (-Xmx) in MiB.
    pub heap_mb: u32,
    /// CPUs the JVM believes it has (sizes its GC/JIT/worker pools).
    pub cpus: u32,
    /// Minecraft log level on stderr (`warn`, `info`, ...).
    pub log_level: String,
    /// Extra JVM arguments.
    pub jvm_args: Vec<String>,
    /// Where worker stderr goes (`None` = inherit).
    pub stderr_log: Option<PathBuf>,
    /// Accept real game clients on this localhost port (video recording).
    pub listen: Option<u16>,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        WorkerConfig {
            version: crate::DEFAULT_VERSION.to_string(),
            heap_mb: 1024,
            cpus: 2,
            log_level: "warn".to_string(),
            jvm_args: Vec::new(),
            stderr_log: None,
            listen: None,
        }
    }
}

/// Options for a new sandbox.
#[derive(Clone, Debug)]
pub struct SandboxOptions {
    /// Force-loaded chunk range `[min_cx, min_cz, max_cx, max_cz]` (inclusive). The default
    /// `[-1, -1, 0, 0]` gives x and z in `-16..=15`.
    pub chunks: [i32; 4],
    /// Seed of the level's random (random ticks, etc.).
    pub seed: i64,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 }
    }
}

/// Physical facts about a palette entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct StateFlags {
    /// Has any collision shape (cannot be walked through).
    pub collides: bool,
    /// Its collision shape is a full cube.
    pub full: bool,
}

/// A box of block states: a palette plus one index per block (x fastest, then z, then y).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub from: Pos,
    pub size: [i32; 3],
    pub palette: Vec<String>,
    /// Same length as `palette` (empty if the bridge did not send flags).
    pub flags: Vec<StateFlags>,
    pub data: Vec<u16>,
}

impl Region {
    fn decode(from: Pos, size: [i32; 3], palette: Vec<String>, flags: Vec<StateFlags>, b64: &str) -> Result<Region> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
        let data: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        if data.len() != (size[0] * size[1] * size[2]) as usize {
            bail!("region data has {} entries for size {:?}", data.len(), size);
        }
        Ok(Region { from, size, palette, flags, data })
    }

    /// Flags of the block at a world position.
    pub fn flags_at(&self, p: Pos) -> Option<StateFlags> {
        self.index(p).map(|i| self.flags.get(self.data[i] as usize).copied().unwrap_or_default())
    }

    /// Index of a world position in `data`, if inside.
    pub fn index(&self, p: Pos) -> Option<usize> {
        let (x, y, z) = (p[0] - self.from[0], p[1] - self.from[1], p[2] - self.from[2]);
        if x < 0 || y < 0 || z < 0 || x >= self.size[0] || y >= self.size[1] || z >= self.size[2] {
            return None;
        }
        Some(((y * self.size[2] + z) * self.size[0] + x) as usize)
    }

    /// Block state at a world position (`None` outside the region).
    pub fn get(&self, p: Pos) -> Option<&str> {
        self.index(p).map(|i| self.palette[self.data[i] as usize].as_str())
    }

    /// All positions with their states.
    pub fn iter(&self) -> impl Iterator<Item = (Pos, &str)> + '_ {
        let [sx, _, sz] = self.size;
        self.data.iter().enumerate().map(move |(i, &id)| {
            let i = i as i32;
            let x = i % sx;
            let z = (i / sx) % sz;
            let y = i / (sx * sz);
            ([self.from[0] + x, self.from[1] + y, self.from[2] + z], self.palette[id as usize].as_str())
        })
    }

    /// Non-air blocks.
    pub fn blocks(&self) -> Vec<(Pos, String)> {
        self.iter()
            .filter(|(_, s)| !crate::blockstate::is_air(s))
            .map(|(p, s)| (p, s.to_string()))
            .collect()
    }
}

/// A block in motion (a piston's moving block entity, [`Worker::get_region_moving`]).
#[derive(Clone, Debug)]
pub struct MovingBlock {
    pub pos: Pos,
    /// The block it carries.
    pub state: String,
    /// Extending (it moves along `direction`) or retracting (against it).
    pub extending: bool,
    /// The piston's own head.
    pub source: bool,
    /// The piston's facing (`north`, `up`, ...).
    pub direction: String,
    pub progress: f64,
}

impl MovingBlock {
    /// The direction the block travels.
    pub fn motion(&self) -> Pos {
        let d = match self.direction.as_str() {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            "west" => [-1, 0, 0],
            "up" => [0, 1, 0],
            _ => [0, -1, 0],
        };
        if self.extending { d } else { [-d[0], -d[1], -d[2]] }
    }

    /// The cell it came from.
    pub fn origin(&self) -> Pos {
        let m = self.motion();
        [self.pos[0] - m[0], self.pos[1] - m[1], self.pos[2] - m[2]]
    }
}

/// Result of a step request for one sandbox.
#[derive(Clone, Debug)]
pub struct StepResult {
    pub sandbox: u32,
    pub ticked: u32,
    pub time: i64,
    /// With `until_idle`: whether it went idle before the cap.
    pub idle: Option<bool>,
    /// With `record`: the recorded region after every tick.
    pub frames: Vec<Region>,
    /// With `signals`: after every tick, `[best, direct]` per probed position (flattened): the
    /// best signal the position's neighbors give into it and the strong signal into it.
    pub signals: Vec<Vec<u8>>,
    /// With an entity watch: after every tick, whether a colliding entity overlapped the walk box.
    pub entity_blocked: Vec<bool>,
    /// With an entity watch: the first entity seen outside the watch box ("tick N: type at x y z").
    pub entity_escape: Option<String>,
    /// With an entity watch: the first entity the design did not declare (a drop...).
    pub entity_foreign: Option<String>,
    /// With `record` + block entities ([`Worker::step_record_be`]): after every tick, every block
    /// entity of the recorded region as text (one line `x y z <type> <SNBT>` each, block order,
    /// keys sorted: equal states give equal text; "" when there is none).
    pub block_entities: Vec<String>,
}

/// A block entity read from a sandbox ([`Worker::block_entities`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockEntityInfo {
    pub pos: Pos,
    /// Its type (`minecraft:hopper`).
    pub kind: String,
    /// Its data (`saveWithoutMetadata`, SNBT with sorted keys).
    pub nbt: String,
}

/// Parse a block-entity dump (`x y z <type> <SNBT>` per line, the bridge's
/// `Region.captureBlockEntities`).
pub fn parse_block_entities(dump: &str) -> Result<Vec<BlockEntityInfo>> {
    let mut out = Vec::new();
    for line in dump.lines().filter(|l| !l.is_empty()) {
        let mut it = line.splitn(5, ' ');
        let mut num = || -> Result<i32> { it.next().ok_or_else(|| anyhow!("short block-entity line {line:?}"))?.parse::<i32>().map_err(Into::into) };
        let pos = [num()?, num()?, num()?];
        let kind = it.next().ok_or_else(|| anyhow!("short block-entity line {line:?}"))?.to_string();
        let nbt = it.next().unwrap_or("{}").to_string();
        out.push(BlockEntityInfo { pos, kind, nbt });
    }
    Ok(out)
}

/// What is still pending in a sandbox.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Status {
    pub time: i64,
    pub block_ticks: u32,
    pub fluid_ticks: u32,
    pub block_events: u32,
    pub moving_pistons: u32,
    pub entities: u32,
    pub idle: bool,
}

/// An entity in a sandbox.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct EntityInfo {
    #[serde(rename = "type")]
    pub kind: String,
    pub pos: [f64; 3],
    pub uuid: String,
    /// `Entity.getId()`: the server's network id (a counter; some ticks read it).
    #[serde(default)]
    pub net_id: i64,
    #[serde(default)]
    pub motion: [f64; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
    /// Declared by the design ([`Worker::summon`]), not a drop or a stray.
    #[serde(default)]
    pub design: bool,
    /// At rest: the last tick did not move it and it has no motion left (see the bridge's
    /// `Sandbox.still`).
    #[serde(default)]
    pub still: bool,
    /// Bumps or gets shoved by a player walking (boats, minecarts): it blocks a walkway.
    #[serde(default)]
    pub collides: bool,
    #[serde(default)]
    pub on_ground: bool,
    /// Its box overlaps a block's collision shape (stuck inside a block).
    #[serde(default)]
    pub embedded: bool,
    /// Bounding box `[min x, min y, min z, max x, max y, max z]`.
    #[serde(default, rename = "box")]
    pub bbox: [f64; 6],
    /// Everything about it but position and motion (saved data without volatile fields, the rail
    /// a minecart is on).
    #[serde(default)]
    pub state: String,
    /// Resolved leash holder's sandbox network id, if any.
    #[serde(default)]
    pub leash_holder: Option<i32>,
    /// `LeashData.angularMomentum` (zero without leash data).
    #[serde(default)]
    pub angular_momentum: f64,
}

/// Entity checks of a recorded step ([`Worker::step_record_watch`]): `watch` is the box every
/// entity must stay inside, `walk` the box no colliding entity may overlap (the opening and its
/// walkway), both inclusive block ranges.
#[derive(Clone, Copy, Debug)]
pub struct EntityWatch {
    pub watch: (Pos, Pos),
    pub walk: (Pos, Pos),
}

static UNIVERSE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// One bridge JVM.
pub struct Worker {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    universe: PathBuf,
    pub version: String,
}

impl Worker {
    /// Build the bridge if needed, start a JVM and wait until it is ready.
    pub fn spawn(cfg: &WorkerConfig) -> Result<Worker> {
        let build = bridge::build(&cfg.version)?;
        Self::spawn_with(&build, cfg)
    }

    /// Start a JVM from an existing bridge build.
    pub fn spawn_with(build: &BridgeBuild, cfg: &WorkerConfig) -> Result<Worker> {
        let universe = paths::run_dir().join(format!(
            "u-{}-{}",
            std::process::id(),
            UNIVERSE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&universe);
        std::fs::create_dir_all(&universe)?;
        let mut cmd = Command::new(&build.install.java);
        cmd.arg(format!("-Xmx{}m", cfg.heap_mb))
            .arg("-XX:+UseSerialGC")
            .arg(format!("-XX:ActiveProcessorCount={}", cfg.cpus.max(1)))
            .arg("-XX:+IgnoreUnrecognizedVMOptions")
            .arg("--enable-native-access=ALL-UNNAMED")
            .arg("--sun-misc-unsafe-memory-access=allow")
            // tickexact: the bridge seeds Collections.shuffle's static random (InsideBrownianWalk)
            .arg("--add-opens=java.base/java.util=ALL-UNNAMED")
            .arg(format!("-Dlog4j2.configurationFile={}", build.log4j_config.display()))
            .arg(format!("-Dmctas.loglevel={}", cfg.log_level))
            .args(&cfg.jvm_args)
            .arg("-cp")
            .arg(build.classpath())
            .arg(MAIN_CLASS)
            .arg("--universe")
            .arg(&universe)
            .args(cfg.listen.map(|p| vec!["--listen".to_string(), p.to_string()]).unwrap_or_default())
            .current_dir(&universe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        match &cfg.stderr_log {
            Some(path) => {
                cmd.stderr(std::fs::File::create(path)?);
            }
            None => {
                cmd.stderr(Stdio::inherit());
            }
        }
        let mut child = cmd.spawn().context("starting the bridge JVM")?;
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut worker = Worker { child, stdin, stdout, next_id: 1, universe, version: cfg.version.clone() };
        let ready = worker.read_line().context("waiting for the bridge to start")?;
        if ready["event"] != "ready" {
            bail!("unexpected first message from the bridge: {ready}");
        }
        Ok(worker)
    }

    fn read_line(&mut self) -> Result<Value> {
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line)?;
        if n == 0 {
            let status = self.child.try_wait().ok().flatten();
            bail!("bridge exited ({status:?}); see its stderr");
        }
        serde_json::from_str(&line).with_context(|| format!("bad line from the bridge: {line}"))
    }

    /// Queue a request (not flushed). Returns its id.
    pub fn send(&mut self, mut req: Value) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        req["id"] = json!(id);
        serde_json::to_writer(&mut self.stdin, &req)?;
        self.stdin.write_all(b"\n")?;
        Ok(id)
    }

    /// Flush queued requests to the JVM.
    pub fn flush(&mut self) -> Result<()> {
        self.stdin.flush()?;
        Ok(())
    }

    /// Read the next response; an `ok: false` response becomes an error.
    pub fn recv(&mut self) -> Result<Value> {
        let v = self.read_line()?;
        if v["ok"] != json!(true) {
            bail!("bridge error: {}", v["error"].as_str().unwrap_or("?"));
        }
        Ok(v)
    }

    /// Send one request and wait for its response.
    pub fn call(&mut self, req: Value) -> Result<Value> {
        let op = req["op"].as_str().unwrap_or("?").to_string();
        let id = self.send(req)?;
        self.flush()?;
        let v = self.recv().with_context(|| format!("op {op}"))?;
        if v["id"] != json!(id) {
            bail!("response id mismatch: sent {id}, got {}", v["id"]);
        }
        Ok(v)
    }

    pub fn info(&mut self) -> Result<Value> {
        self.call(json!({"op": "info"}))
    }

    /// OS process id of the JVM.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Resident memory of the JVM in KiB (Linux only; `None` elsewhere).
    pub fn rss_kib(&self) -> Option<u64> {
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.pid())).ok()?;
        let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
        line.split_whitespace().nth(1)?.parse().ok()
    }

    /// Create a sandbox; returns its id.
    pub fn create(&mut self, opts: &SandboxOptions) -> Result<u32> {
        let v = self.call(json!({"op": "create", "chunks": opts.chunks, "seed": opts.seed}))?;
        Ok(v["sandbox"].as_u64().ok_or_else(|| anyhow!("no sandbox id"))? as u32)
    }

    /// Reset a sandbox to empty void, time 0, reseeded.
    pub fn clear(&mut self, sandbox: u32) -> Result<()> {
        self.call(json!({"op": "clear", "sandbox": sandbox}))?;
        Ok(())
    }

    /// Save, unload and load again every chunk of a sandbox without ticking it (a player walking
    /// away and back, a server restart): `Sandbox.reload`.
    pub fn reload(&mut self, sandbox: u32) -> Result<()> {
        self.call(json!({"op": "reload", "sandbox": sandbox}))?;
        Ok(())
    }

    /// Destroy a sandbox (prefer [`Worker::clear`] to reuse one).
    pub fn drop_sandbox(&mut self, sandbox: u32) -> Result<()> {
        self.call(json!({"op": "drop", "sandbox": sandbox}))?;
        Ok(())
    }

    /// The sandbox's level random state (`LegacyRandomSource.seed`, the 48-bit LCG state), read
    /// without advancing it: redsim's level-random mirror is checked against it (trace-export's
    /// "rng").
    pub fn random_state(&mut self, sandbox: u32) -> Result<i64> {
        let v = self.call(json!({"op": "random_state", "sandbox": sandbox}))?;
        v["seed"].as_i64().context("random_state: no seed")
    }

    /// The level random's state and `Level.randValue` (the LCG of `getBlockRandomPos`: random
    /// ticks and precipitation; the bridge seeds it from the sandbox seed), read without advancing
    /// them (trace-export's "rng" and "rv").
    pub fn random_states(&mut self, sandbox: u32) -> Result<(i64, i32)> {
        let v = self.call(json!({"op": "random_state", "sandbox": sandbox}))?;
        let seed = v["seed"].as_i64().context("random_state: no seed")?;
        let rv = v["rand_value"].as_i64().context("random_state: no rand_value (an old bridge)")? as i32;
        Ok((seed, rv))
    }

    /// The chunks the sandbox's level ticks (`ServerLevel.tickChunk`), in the game's order: [x, z].
    pub fn ticking_chunks(&mut self, sandbox: u32) -> Result<Vec<[i32; 2]>> {
        let v = self.call(json!({"op": "ticking_chunks", "sandbox": sandbox}))?;
        let a = v["chunks"].as_array().context("ticking_chunks: no chunks")?;
        a.iter().map(|c| Ok([c[0].as_i64().context("chunk x")? as i32, c[1].as_i64().context("chunk z")? as i32])).collect()
    }

    /// Write blocks. Returns how many positions changed.
    pub fn set_blocks<S: AsRef<str>>(&mut self, sandbox: u32, blocks: &[(Pos, S)], mode: PlaceMode) -> Result<u32> {
        let list: Vec<Value> = blocks.iter().map(|(p, s)| json!([p[0], p[1], p[2], s.as_ref()])).collect();
        let v = self.call(json!({"op": "set", "sandbox": sandbox, "mode": mode.as_str(), "blocks": list}))?;
        Ok(v["changed"].as_u64().unwrap_or(0) as u32)
    }

    /// Fill a box with one state.
    pub fn fill(&mut self, sandbox: u32, from: Pos, to: Pos, state: &str, mode: PlaceMode) -> Result<u32> {
        let v = self.call(json!({"op": "fill", "sandbox": sandbox, "from": from, "to": to, "state": state, "mode": mode.as_str()}))?;
        Ok(v["changed"].as_u64().unwrap_or(0) as u32)
    }

    /// Read a box of block states (inclusive corners).
    pub fn get_region(&mut self, sandbox: u32, from: Pos, to: Pos) -> Result<Region> {
        let v = self.call(json!({"op": "get", "sandbox": sandbox, "from": from, "to": to}))?;
        parse_region(&v, &v["data"])
    }

    /// Every block entity in a box (inclusive corners), in block order (x fastest, then z, then
    /// y): its position, type and data (container items, a hopper's `TransferCooldown`...).
    pub fn block_entities(&mut self, sandbox: u32, from: Pos, to: Pos) -> Result<Vec<BlockEntityInfo>> {
        let v = self.call(json!({"op": "get", "sandbox": sandbox, "from": from, "to": to, "block_entities": true}))?;
        parse_block_entities(v["block_entities"].as_str().unwrap_or_default())
    }

    /// Merge SNBT into the block entity at `pos` exactly like `data merge block x y z <snbt>`
    /// (old data merged with the tag, then `loadWithComponents` + `setChanged` when it changed:
    /// comparators reading a container are notified). Fails when there is no block entity there
    /// or the SNBT does not parse. Returns whether anything changed.
    pub fn merge_block_entity(&mut self, sandbox: u32, pos: Pos, snbt: &str) -> Result<bool> {
        let v = self.call(json!({"op": "bedata", "sandbox": sandbox, "pos": pos, "nbt": snbt}))?;
        Ok(v["changed"].as_bool().unwrap_or(false))
    }

    /// [`Worker::get_region`] plus every block in motion inside the box (a piston's moving block
    /// entity): what it carries, its piston's facing, extending or retracting.
    pub fn get_region_moving(&mut self, sandbox: u32, from: Pos, to: Pos) -> Result<(Region, Vec<MovingBlock>)> {
        let v = self.call(json!({"op": "get", "sandbox": sandbox, "from": from, "to": to, "moving": true}))?;
        let region = parse_region(&v, &v["data"])?;
        let mut moving = Vec::new();
        for m in v["moving"].as_array().cloned().unwrap_or_default() {
            moving.push(MovingBlock {
                pos: pos_of(&m["pos"])?,
                state: m["state"].as_str().unwrap_or_default().to_string(),
                extending: m["extending"].as_bool().unwrap_or(false),
                source: m["source"].as_bool().unwrap_or(false),
                direction: m["direction"].as_str().unwrap_or_default().to_string(),
                progress: m["progress"].as_f64().unwrap_or(0.0),
            });
        }
        Ok((region, moving))
    }

    /// Positions in the box whose block cannot survive where it stands (`canSurvive` false):
    /// floating levers, torches, dust, repeaters... (raw placement keeps them).
    pub fn unsupported(&mut self, sandbox: u32, from: Pos, to: Pos) -> Result<Vec<Pos>> {
        Ok(self.static_problems(sandbox, from, to)?.0)
    }

    /// Block states turned `turns` quarter turns clockwise seen from above, by the game's own
    /// `BlockState.rotate` (facings, axes, dust connections, rails, levers...).
    pub fn rotate_states(&mut self, states: &[String], turns: u8) -> Result<Vec<String>> {
        self.transform_states(states, turns, false)
    }

    /// Block states mirrored (x -> -x, the game's `Mirror.FRONT_BACK`) when `mirror`, then turned
    /// `turns` quarter turns clockwise, by the game's own `BlockState.mirror` / `rotate`.
    pub fn transform_states(&mut self, states: &[String], turns: u8, mirror: bool) -> Result<Vec<String>> {
        let v = self.call(json!({"op": "rotate", "states": states, "turns": turns, "mirror": mirror}))?;
        let out: Vec<String> = v["states"].as_array().ok_or_else(|| anyhow!("rotate: no states"))?.iter().filter_map(|s| s.as_str().map(String::from)).collect();
        if out.len() != states.len() {
            bail!("rotate: {} states in, {} out", states.len(), out.len());
        }
        Ok(out)
    }

    /// (unsupported positions, randomly-ticking positions) in the box: blocks that could not
    /// stand where they are, and blocks that change by themselves over time (random ticks).
    pub fn static_problems(&mut self, sandbox: u32, from: Pos, to: Pos) -> Result<(Vec<Pos>, Vec<Pos>)> {
        let v = self.call(json!({"op": "unsupported", "sandbox": sandbox, "from": from, "to": to}))?;
        let list = |key: &str| -> Vec<Pos> {
            let mut out = Vec::new();
            for p in v[key].as_array().cloned().unwrap_or_default() {
                let c: Vec<i32> = p.as_array().map(|a| a.iter().filter_map(|n| n.as_i64()).map(|n| n as i32).collect()).unwrap_or_default();
                if c.len() == 3 {
                    out.push([c[0], c[1], c[2]]);
                }
            }
            out
        };
        Ok((list("positions"), list("random")))
    }

    /// Right-click a block (levers and buttons are toggled/pressed directly).
    pub fn use_block(&mut self, sandbox: u32, pos: Pos) -> Result<String> {
        let v = self.call(json!({"op": "use", "sandbox": sandbox, "pos": pos}))?;
        Ok(v["result"].as_str().unwrap_or_default().to_string())
    }

    /// Right-click the item frame in a block cell with an empty hand: its item turns one step
    /// (the comparators reading it update, as for a player). Exactly one frame must be there.
    pub fn use_entity(&mut self, sandbox: u32, pos: Pos) -> Result<String> {
        let v = self.call(json!({"op": "use_entity", "sandbox": sandbox, "pos": pos}))?;
        Ok(v["result"].as_str().unwrap_or_default().to_string())
    }

    /// Advance `ticks` game ticks.
    pub fn step(&mut self, sandbox: u32, ticks: u32) -> Result<StepResult> {
        let v = self.call(json!({"op": "step", "sandbox": sandbox, "ticks": ticks}))?;
        Ok(parse_steps(&v)?.remove(0))
    }

    /// Advance until nothing is pending (no scheduled ticks, block events or moving pistons),
    /// at most `max_ticks`.
    pub fn step_until_idle(&mut self, sandbox: u32, max_ticks: u32) -> Result<StepResult> {
        let v = self.call(json!({"op": "step", "sandbox": sandbox, "ticks": max_ticks, "until_idle": true}))?;
        Ok(parse_steps(&v)?.remove(0))
    }

    /// Advance and capture a region after every tick.
    pub fn step_record(&mut self, sandbox: u32, ticks: u32, until_idle: bool, from: Pos, to: Pos) -> Result<StepResult> {
        let v = self.call(json!({
            "op": "step", "sandbox": sandbox, "ticks": ticks, "until_idle": until_idle,
            "record": {"from": from, "to": to}
        }))?;
        Ok(parse_steps(&v)?.remove(0))
    }

    /// [`Worker::step_record`] that also captures every block entity of the region after every
    /// tick ([`StepResult::block_entities`]): what a periodic check of a design with containers
    /// must compare too (items, hopper cooldowns).
    pub fn step_record_be(&mut self, sandbox: u32, ticks: u32, until_idle: bool, from: Pos, to: Pos) -> Result<StepResult> {
        let v = self.call(json!({
            "op": "step", "sandbox": sandbox, "ticks": ticks, "until_idle": until_idle,
            "record": {"from": from, "to": to, "block_entities": true}
        }))?;
        Ok(parse_steps(&v)?.remove(0))
    }

    /// Advance several sandboxes of this worker in one request.
    pub fn step_many(&mut self, sandboxes: &[u32], ticks: u32, until_idle: bool) -> Result<Vec<StepResult>> {
        let v = self.call(json!({"op": "step", "sandboxes": sandboxes, "ticks": ticks, "until_idle": until_idle}))?;
        parse_steps(&v)
    }

    /// Advance `ticks` (or until idle, capped) recording, after every tick, the signal into each
    /// of `probes` (see [`StepResult::signals`]) and optionally a region.
    pub fn step_probe(&mut self, sandbox: u32, ticks: u32, until_idle: bool, probes: &[Pos], record: Option<(Pos, Pos)>) -> Result<StepResult> {
        self.step_probe_be(sandbox, ticks, until_idle, probes, record, false)
    }

    /// [`Worker::step_probe`] that also captures every block entity of the recorded region after
    /// every tick when `block_entities` ([`StepResult::block_entities`]).
    pub fn step_probe_be(&mut self, sandbox: u32, ticks: u32, until_idle: bool, probes: &[Pos], record: Option<(Pos, Pos)>, block_entities: bool) -> Result<StepResult> {
        let mut req = json!({"op": "step", "sandbox": sandbox, "ticks": ticks, "until_idle": until_idle, "signals": probes});
        if let Some((from, to)) = record {
            req["record"] = if block_entities { json!({"from": from, "to": to, "block_entities": true}) } else { json!({"from": from, "to": to}) };
        }
        let v = self.call(req)?;
        Ok(parse_steps(&v)?.remove(0))
    }

    pub fn status(&mut self, sandbox: u32) -> Result<Status> {
        let v = self.call(json!({"op": "status", "sandbox": sandbox}))?;
        Ok(serde_json::from_value(v)?)
    }

    pub fn entities(&mut self, sandbox: u32) -> Result<Vec<EntityInfo>> {
        let v = self.call(json!({"op": "entities", "sandbox": sandbox}))?;
        Ok(serde_json::from_value(v["entities"].clone())?)
    }

    /// Summon a design's entities (deterministic UUIDs and randoms, tagged as the design's; mobs
    /// are refused). Returns their UUIDs.
    pub fn summon(&mut self, sandbox: u32, entities: &[crate::design::EntitySpec]) -> Result<Vec<String>> {
        let list: Vec<Value> = entities.iter().map(|e| json!({"type": e.kind, "pos": e.pos, "nbt": e.nbt})).collect();
        let v = self.call(json!({"op": "summon", "sandbox": sandbox, "entities": list}))?;
        Ok(v["uuids"].as_array().map(|a| a.iter().filter_map(|u| u.as_str().map(String::from)).collect()).unwrap_or_default())
    }

    /// Entities' data turned like structure templates turn them (mirror first, then `turns`
    /// quarter turns clockwise): each entity's SNBT with its new `Rotation` (and a hanging entity's
    /// facing), `None` when it has no data left to write. Positions are turned by the caller.
    pub fn transform_entities(&mut self, entities: &[crate::design::EntitySpec], turns: u8, mirror: bool) -> Result<Vec<Option<String>>> {
        let list: Vec<Value> = entities.iter().map(|e| json!({"type": e.kind, "nbt": e.nbt})).collect();
        let v = self.call(json!({"op": "rotate", "states": [], "turns": turns, "mirror": mirror, "entities": list}))?;
        let out: Vec<Option<String>> =
            v["entities"].as_array().ok_or_else(|| anyhow!("rotate: no entities"))?.iter().map(|s| s.as_str().map(String::from)).collect();
        if out.len() != entities.len() {
            bail!("rotate: {} entities in, {} out", entities.len(), out.len());
        }
        Ok(out)
    }

    /// [`Worker::step_record`] plus entity checks after every tick ([`EntityWatch`]).
    pub fn step_record_watch(&mut self, sandbox: u32, ticks: u32, until_idle: bool, from: Pos, to: Pos, watch: &EntityWatch) -> Result<StepResult> {
        let v = self.call(json!({
            "op": "step", "sandbox": sandbox, "ticks": ticks, "until_idle": until_idle,
            "record": {"from": from, "to": to},
            "entity_watch": {"from": watch.watch.0, "to": watch.watch.1, "walk_from": watch.walk.0, "walk_to": watch.walk.1}
        }))?;
        Ok(parse_steps(&v)?.remove(0))
    }

    /// Run a vanilla command (e.g. `fill 0 0 0 3 3 3 stone`) inside a sandbox.
    pub fn exec(&mut self, sandbox: u32, command: &str) -> Result<()> {
        self.call(json!({"op": "exec", "sandbox": sandbox, "command": command}))?;
        Ok(())
    }

    /// Emit a game event (`step`: a player's footstep) at each position's bottom center, no
    /// source entity (sculk sensors in range hear it). Returns how many were emitted.
    pub fn game_event(&mut self, sandbox: u32, event: &str, positions: &[Pos]) -> Result<u32> {
        let v = self.call(json!({"op": "gameevent", "sandbox": sandbox, "event": event, "positions": positions}))?;
        Ok(v["emitted"].as_u64().unwrap_or(0) as u32)
    }

    /// [`Worker::game_event`] at each cell's CENTER when `center` (where `Level.gameEvent(.., BlockPos)`
    /// posts a block's own events: a lever's `block_activate`), else at its bottom center.
    pub fn game_event_at(&mut self, sandbox: u32, event: &str, positions: &[Pos], center: bool) -> Result<u32> {
        let v = self.call(json!({"op": "gameevent", "sandbox": sandbox, "event": event, "positions": positions, "at": if center { "center" } else { "feet" }}))?;
        Ok(v["emitted"].as_u64().unwrap_or(0) as u32)
    }

    /// Number of connected game clients.
    pub fn viewers(&mut self) -> Result<u32> {
        Ok(self.call(json!({"op": "viewers"}))?["count"].as_u64().unwrap_or(0) as u32)
    }

    /// Move connected clients (spectator mode) to a camera pose in a sandbox.
    pub fn camera(&mut self, sandbox: u32, pos: [f64; 3], yaw: f32, pitch: f32) -> Result<()> {
        self.call(json!({"op": "camera", "sandbox": sandbox, "pos": pos, "yaw": yaw, "pitch": pitch}))?;
        Ok(())
    }

    /// Every connected client's game mode (`creative`, `spectator`, ...) and, with `op`, operator
    /// status (a client mod pasting with commands needs both).
    pub fn viewer_mode(&mut self, mode: &str, op: bool) -> Result<u32> {
        Ok(self.call(json!({"op": "viewer_mode", "mode": mode, "operator": op}))?["changed"].as_u64().unwrap_or(0) as u32)
    }

    /// How many chunks around a viewer the sandbox's level sends (2 by default: a viewer sees a
    /// 5x5-chunk area; chunks outside the sandbox are empty).
    pub fn view_distance(&mut self, sandbox: u32, chunks: u32) -> Result<()> {
        self.call(json!({"op": "view", "sandbox": sandbox, "chunks": chunks}))?;
        Ok(())
    }

    /// Step at a wall-clock pace so connected clients can watch.
    pub fn play(&mut self, sandbox: u32, ticks: u32, tps: f64) -> Result<()> {
        self.call(json!({"op": "play", "sandbox": sandbox, "ticks": ticks, "tps": tps}))?;
        Ok(())
    }

    /// Serve connected clients for `ms` milliseconds without ticking.
    pub fn hold(&mut self, ms: u64) -> Result<()> {
        self.call(json!({"op": "hold", "ms": ms}))?;
        Ok(())
    }

    /// Signal probes now: `(best, direct)` per position: the best signal the position's neighbors
    /// give into it (what a piston or lamp there would see) and the strong signal into it.
    pub fn signals(&mut self, sandbox: u32, positions: &[Pos]) -> Result<Vec<(u8, u8)>> {
        let v = self.call(json!({"op": "signals", "sandbox": sandbox, "positions": positions}))?;
        let rows = v["signals"].as_array().ok_or_else(|| anyhow!("no signals"))?;
        Ok(rows
            .iter()
            .map(|r| {
                let a = r.as_array().cloned().unwrap_or_default();
                let n = |i: usize| a.get(i).and_then(|x| x.as_u64()).unwrap_or(0) as u8;
                (n(0), n(1))
            })
            .collect())
    }

    /// Manual block updates at `positions`: `neighbors` (notify the 6 neighbors), `self` (the
    /// block re-checks its inputs, e.g. a piston checks power) or `shape` (recompute
    /// connection-style properties without notifying anyone).
    pub fn update(&mut self, sandbox: u32, positions: &[Pos], kind: &str) -> Result<u32> {
        let v = self.call(json!({"op": "update", "sandbox": sandbox, "kind": kind, "positions": positions}))?;
        Ok(v["changed"].as_u64().unwrap_or(0) as u32)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.universe);
    }
}

fn pos_of(v: &Value) -> Result<Pos> {
    let a = v.as_array().ok_or_else(|| anyhow!("expected [x,y,z], got {v}"))?;
    Ok([a[0].as_i64().unwrap_or(0) as i32, a[1].as_i64().unwrap_or(0) as i32, a[2].as_i64().unwrap_or(0) as i32])
}

fn parse_region(v: &Value, data: &Value) -> Result<Region> {
    let palette: Vec<String> = serde_json::from_value(v["palette"].clone())?;
    let flags: Vec<StateFlags> = if v["flags"].is_array() { serde_json::from_value(v["flags"].clone())? } else { Vec::new() };
    Region::decode(pos_of(&v["from"])?, pos_of(&v["size"])?, palette, flags, data.as_str().unwrap_or_default())
}

fn parse_steps(v: &Value) -> Result<Vec<StepResult>> {
    let mut out = Vec::new();
    for r in v["results"].as_array().ok_or_else(|| anyhow!("no results"))? {
        let mut frames = Vec::new();
        if let Some(fs) = r["frames"].as_array() {
            for f in fs {
                frames.push(parse_region(r, f)?);
            }
        }
        let mut signals = Vec::new();
        if let Some(rows) = r["signals"].as_array() {
            for row in rows {
                signals.push(row.as_array().map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as u8).collect()).unwrap_or_default());
            }
        }
        let mut block_entities = Vec::new();
        if let (Some(pal), Some(ids)) = (r["be_palette"].as_array(), r["be_frames"].as_array()) {
            let pal: Vec<&str> = pal.iter().map(|s| s.as_str().unwrap_or_default()).collect();
            for id in ids {
                let i = id.as_u64().ok_or_else(|| anyhow!("bad be_frames entry {id}"))? as usize;
                block_entities.push(pal.get(i).ok_or_else(|| anyhow!("be_frames index {i} out of range"))?.to_string());
            }
        }
        out.push(StepResult {
            sandbox: r["sandbox"].as_u64().unwrap_or(0) as u32,
            ticked: r["ticked"].as_u64().unwrap_or(0) as u32,
            time: r["time"].as_i64().unwrap_or(0),
            idle: r["idle"].as_bool(),
            frames,
            signals,
            entity_blocked: r["entity_blocked"].as_array().map(|a| a.iter().map(|v| v.as_bool().unwrap_or(false)).collect()).unwrap_or_default(),
            entity_escape: entity_event(&r["entity_escape"]),
            entity_foreign: entity_event(&r["entity_foreign"]),
            block_entities,
        });
    }
    Ok(out)
}

fn entity_event(v: &Value) -> Option<String> {
    v.is_object().then(|| {
        format!("tick {}: {} at {}", v["tick"].as_u64().unwrap_or(0), v["type"].as_str().unwrap_or("?"), v["pos"].as_str().unwrap_or("?"))
    })
}

/// Several workers; jobs are spread over them by [`Pool::map`].
pub struct Pool {
    workers: Vec<Mutex<Worker>>,
}

impl Pool {
    /// Start `n` workers in parallel.
    pub fn spawn(n: usize, cfg: &WorkerConfig) -> Result<Pool> {
        // A box is shared by several agents: MCTAS_MAX_WORKERS caps every pool of one process.
        let n = match std::env::var("MCTAS_MAX_WORKERS").ok().and_then(|v| v.parse::<usize>().ok()) {
            Some(cap) if cap > 0 => n.min(cap),
            _ => n,
        };
        let build = bridge::build(&cfg.version)?;
        let results: Vec<Result<Worker>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..n).map(|_| s.spawn(|| Worker::spawn_with(&build, cfg))).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let workers = results.into_iter().collect::<Result<Vec<_>>>()?;
        Ok(Pool { workers: workers.into_iter().map(Mutex::new).collect() })
    }

    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    /// Run `f` on every item, each worker pulling items from a shared queue; results come back
    /// in input order. `f` gets the worker and its index (to keep per-worker sandboxes).
    pub fn map<T, R, F>(&self, items: Vec<T>, f: F) -> Vec<Result<R>>
    where
        T: Send,
        R: Send,
        F: Fn(&mut Worker, usize, T) -> Result<R> + Sync,
    {
        let n = items.len();
        let queue = Mutex::new(items.into_iter().enumerate().collect::<std::collections::VecDeque<_>>());
        let results: Mutex<Vec<Option<Result<R>>>> = Mutex::new((0..n).map(|_| None).collect());
        std::thread::scope(|s| {
            for (wi, worker) in self.workers.iter().enumerate() {
                let queue = &queue;
                let results = &results;
                let f = &f;
                s.spawn(move || {
                    let mut worker = worker.lock().unwrap();
                    loop {
                        let next = queue.lock().unwrap().pop_front();
                        let Some((i, item)) = next else { break };
                        let r = f(&mut worker, wi, item);
                        results.lock().unwrap()[i] = Some(r);
                    }
                });
            }
        });
        results.into_inner().unwrap().into_iter().map(|r| r.unwrap_or_else(|| Err(anyhow!("job not run")))).collect()
    }

    /// Exclusive access to one worker.
    pub fn worker(&self, i: usize) -> std::sync::MutexGuard<'_, Worker> {
        self.workers[i].lock().unwrap()
    }
}
