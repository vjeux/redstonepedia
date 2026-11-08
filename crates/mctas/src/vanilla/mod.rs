//! An UNMODIFIED vanilla dedicated server (`java -jar` on the official server jar), driven
//! over RCON, with a bot player for clicks, and world reads straight from the saved region
//! files. Used to check the simulator's verdicts against the real game.
//!
//! * The world is a void superflat in its own temp dir; RCON and the game port are free ports
//!   on 127.0.0.1; online-mode is off so the bot can log in.
//! * Time is frozen (`/tick freeze`) and advanced with `/tick step 1`, waiting for the game
//!   time to move, so ticks happen only when asked.
//! * [`VanillaServer::snapshot`] runs `save-all flush` and parses the region files: every block
//!   state in a box, the scheduled block ticks (`block_ticks`) and block entities (moving
//!   pistons), i.e. everything the simulator's `idle` looks at except falling blocks.

pub mod bot;
pub mod nbt;

use crate::sim::{Pos, Region};
use anyhow::{Context, Result, anyhow, bail};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A tiny RCON client (Source RCON protocol, as `RconClient` implements it).
pub struct Rcon {
    s: TcpStream,
    next_id: i32,
}

impl Rcon {
    pub fn connect(port: u16, password: &str) -> Result<Rcon> {
        let s = TcpStream::connect(("127.0.0.1", port))?;
        s.set_nodelay(true)?;
        s.set_read_timeout(Some(Duration::from_secs(120)))?;
        let mut r = Rcon { s, next_id: 1 };
        let id = r.send(3, password)?;
        let (rid, _, _) = r.read()?;
        if rid != id {
            bail!("RCON login refused");
        }
        Ok(r)
    }

    fn send(&mut self, kind: i32, body: &str) -> Result<i32> {
        let id = self.next_id;
        self.next_id += 1;
        let mut p = Vec::with_capacity(body.len() + 14);
        p.extend_from_slice(&((body.len() + 10) as i32).to_le_bytes());
        p.extend_from_slice(&id.to_le_bytes());
        p.extend_from_slice(&kind.to_le_bytes());
        p.extend_from_slice(body.as_bytes());
        p.extend_from_slice(&[0, 0]);
        self.s.write_all(&p)?;
        Ok(id)
    }

    fn read(&mut self) -> Result<(i32, i32, String)> {
        let mut len = [0u8; 4];
        self.s.read_exact(&mut len)?;
        let len = i32::from_le_bytes(len);
        if !(10..=1 << 20).contains(&len) {
            bail!("bad RCON packet length {len}");
        }
        let mut buf = vec![0u8; len as usize];
        self.s.read_exact(&mut buf)?;
        let id = i32::from_le_bytes(buf[0..4].try_into()?);
        let kind = i32::from_le_bytes(buf[4..8].try_into()?);
        let body = String::from_utf8_lossy(&buf[8..buf.len() - 2]).into_owned();
        Ok((id, kind, body))
    }

    /// Run one command (no leading slash needed) and return its output.
    pub fn cmd(&mut self, command: &str) -> Result<String> {
        let id = self.send(2, command)?;
        // The server answers in 4096-byte pieces; ours are short, but read until the id matches.
        loop {
            let (rid, _, body) = self.read()?;
            if rid == id {
                return Ok(body);
            }
        }
    }
}

fn free_port() -> Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// Knobs of [`VanillaServer::start`].
#[derive(Clone, Debug)]
pub struct VanillaOptions {
    pub version: String,
    pub heap_mb: u32,
    /// Keep the world directory after the run (for post-mortems).
    pub keep_dir: bool,
    /// Name of the bot player that clicks levers.
    pub bot_name: String,
    /// `/tick rate` while frozen: only how often the idle server loop wakes up (a `tick step`
    /// waits for the next wake-up), not what a tick does. Vanilla's default is 20.
    pub tick_rate: u32,
    /// Start on a copy of this saved world instead of a fresh void world.
    pub world: Option<PathBuf>,
    /// view-distance (chunks sent to players).
    pub view_distance: u32,
    /// `level-name`: the world folder's name, and the name the game lists for a new world.
    pub level_name: String,
}

impl Default for VanillaOptions {
    fn default() -> Self {
        VanillaOptions {
            version: crate::DEFAULT_VERSION.to_string(),
            heap_mb: 1536,
            keep_dir: false,
            bot_name: "mctas_bot".into(),
            tick_rate: 20,
            world: None,
            view_distance: 3,
            level_name: "world".into(),
        }
    }
}

/// Explicit experimental survival server: no protocol bot, ordinary game authority,
/// and optional read-only JVM instrumentation. Legacy callers keep the legacy profile.
#[derive(Clone, Debug)]
pub struct SurvivalProfile {
    pub observer_agent: Option<PathBuf>,
    pub trace_file: PathBuf,
}

/// What one `save-all` shows of a box.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Block states of the box (no collision flags: those come from the simulator).
    pub region: Region,
    /// Scheduled block ticks in the box's chunks: position, block id, ticks until due.
    pub block_ticks: Vec<(Pos, String, i64)>,
    pub fluid_ticks: usize,
    /// Block entities in the box's chunks: position and id (`minecraft:piston` = moving block).
    pub block_entities: Vec<(Pos, String)>,
}

impl Snapshot {
    pub fn moving_pistons(&self) -> usize {
        self.block_entities.iter().filter(|(_, id)| id == "minecraft:piston").count()
    }
}

/// Where the time goes.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub step: Duration,
    pub steps: u32,
    pub snapshot: Duration,
    pub snapshots: u32,
}

/// A running vanilla server.
pub struct VanillaServer {
    child: Child,
    pub dir: PathBuf,
    rcon: Rcon,
    pub bot: Option<bot::Bot>,
    game_port: u16,
    keep_dir: bool,
    /// Time spent stepping and snapshotting, and how many of each.
    pub stats: Stats,
    /// The `/tick rate` set at start (20 = vanilla default).
    pub tick_rate: u32,
    /// The last box a caller built in (to clear before the next build).
    pub dirty: Option<(Pos, Pos)>,
    /// Marker block state per chunk (see `snapshot`).
    markers: HashMap<(i32, i32), bool>,
    markers_seen: u64,
    /// The world folder's name under `dir` (`level-name`).
    pub level_name: String,
    /// The server's console log.
    pub log: PathBuf,
}

impl VanillaServer {
    /// Start the official server jar in a fresh world directory and connect RCON.
    pub fn start(opts: &VanillaOptions) -> Result<VanillaServer> {
        Self::start_profile(opts, None)
    }

    /// A dedicated server for explicit survival mechanics experiments, with no fake pilot.
    pub fn start_survival(opts: &VanillaOptions, profile: &SurvivalProfile) -> Result<VanillaServer> {
        Self::start_profile(opts, Some(profile))
    }

    fn start_profile(opts: &VanillaOptions, profile: Option<&SurvivalProfile>) -> Result<VanillaServer> {
        let install = crate::setup::install_server(&opts.version)?;
        let bundle = crate::paths::downloads().join(format!("server-bundle-{}.jar", opts.version));
        if !bundle.exists() {
            bail!("{} is missing (run `mctas setup`)", bundle.display());
        }
        // The world lives in memory when it can (/dev/shm): `save-all flush` fsyncs every region
        // file, which is slow on a busy disk and pointless for a scratch world.
        let base = match std::env::var_os("MCTAS_VANILLA_DIR") {
            Some(d) => PathBuf::from(d),
            None if Path::new("/dev/shm").is_dir() => PathBuf::from("/dev/shm"),
            None => crate::paths::run_dir(),
        };
        let dir = base.join(format!("mctas-vanilla-{}-{:x}", std::process::id(), rand_u64()));
        std::fs::create_dir_all(&dir)?;
        let game_port = free_port()?;
        let rcon_port = free_port()?;
        let password = format!("p{:x}", rand_u64());
        std::fs::write(dir.join("eula.txt"), "eula=true\n")?;
        if let Some(src) = &opts.world {
            copy_tree(src, &dir.join(&opts.level_name))?;
        }
        let props = [
            ("level-name", opts.level_name.clone()),
            ("level-type", "minecraft\\:flat".to_string()),
            ("generator-settings", r#"{"layers":[{"block":"minecraft:air","height":1}],"biome":"minecraft:the_void","features":false,"lakes":false}"#.to_string()),
            ("generate-structures", "false".to_string()),
            ("level-seed", "0".to_string()),
            ("online-mode", "false".to_string()),
            ("enforce-secure-profile", "false".to_string()),
            ("server-ip", "127.0.0.1".to_string()),
            ("server-port", game_port.to_string()),
            ("enable-rcon", "true".to_string()),
            ("rcon.port", rcon_port.to_string()),
            ("rcon.password", password.clone()),
            ("broadcast-rcon-to-ops", "false".to_string()),
            ("enable-query", "false".to_string()),
            ("enable-status", "true".to_string()),
            ("spawn-protection", "0".to_string()),
            ("difficulty", "peaceful".to_string()),
            ("gamemode", if profile.is_some() { "survival" } else { "creative" }.to_string()),
            ("force-gamemode", "true".to_string()),
            ("allow-flight", if profile.is_some() { "false" } else { "true" }.to_string()),
            ("max-players", "4".to_string()),
            ("max-tick-time", "-1".to_string()),
            ("pause-when-empty-seconds", "0".to_string()),
            ("view-distance", opts.view_distance.to_string()),
            ("simulation-distance", "3".to_string()),
            ("network-compression-threshold", "-1".to_string()),
            ("sync-chunk-writes", "false".to_string()),
            ("region-file-compression", "none".to_string()),
            ("log-ips", "false".to_string()),
            ("white-list", "false".to_string()),
            ("enforce-whitelist", "false".to_string()),
        ];
        let text: String = props.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        std::fs::write(dir.join("server.properties"), text)?;
        let log = dir.join("server.log");
        let out = std::fs::File::create(&log)?;
        let repo = crate::paths::version_dir(&opts.version).join("vanilla-bundler-repo");
        std::fs::create_dir_all(&repo)?;
        // Never leave an orphan server behind if this process dies without running Drop:
        // util-linux `setpriv --pdeathsig TERM` (when installed) makes the kernel stop it.
        let setpriv = Path::new("/usr/bin/setpriv");
        let mut command = if setpriv.exists() {
            let mut c = Command::new(setpriv);
            c.arg("--pdeathsig").arg("TERM").arg(&install.java);
            c
        } else {
            Command::new(&install.java)
        };
        if let Some(profile) = profile {
            if let Some(agent) = &profile.observer_agent {
                command.arg(format!("-javaagent:{}", agent.display()));
                command.arg(format!("-Dmctas.probe.trace={}", profile.trace_file.display()));
            }
        }
        command
            .arg(format!("-Xmx{}m", opts.heap_mb))
            .arg("-XX:+UseSerialGC")
            .arg("-XX:ActiveProcessorCount=2")
            .arg(format!("-DbundlerRepoDir={}", repo.display()))
            .arg("-jar")
            .arg(&bundle)
            .arg("nogui")
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(out.try_clone()?)
            .stderr(out);
        let child = command.spawn().context("starting the vanilla server")?;
        let mut child = child;
        // Wait for RCON.
        let t0 = Instant::now();
        let rcon = loop {
            if let Some(status) = child.try_wait()? {
                bail!("the vanilla server exited ({status}); see {}", log.display());
            }
            let ready = std::fs::read_to_string(&log).unwrap_or_default().contains("RCON running on");
            if ready {
                match Rcon::connect(rcon_port, &password) {
                    Ok(r) => break r,
                    Err(e) if t0.elapsed() > Duration::from_secs(180) => return Err(e.context("RCON connect")),
                    Err(_) => {}
                }
            }
            if t0.elapsed() > Duration::from_secs(180) {
                let _ = child.kill();
                bail!("the vanilla server did not start RCON within 180 s; see {}", log.display());
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        let mut server = VanillaServer { child, dir, rcon, bot: None, game_port, keep_dir: opts.keep_dir, log, stats: Stats::default(), tick_rate: opts.tick_rate, dirty: None, markers: HashMap::new(), markers_seen: 0, level_name: opts.level_name.clone() };
        for c in [
            "tick freeze",
            "save-off",
            "gamerule spawn_mobs false",
            "gamerule spawn_monsters false",
            "gamerule advance_weather false",
            "gamerule advance_time false",
            "gamerule log_admin_commands false",
        ] {
            server.cmd_ok(c)?;
        }
        if opts.tick_rate != 20 {
            server.cmd_ok(&format!("tick rate {}", opts.tick_rate))?;
        }
        if profile.is_none() {
            let bot = bot::Bot::connect(game_port, &opts.bot_name)?;
            server.bot = Some(bot);
            server.cmd_ok(&format!("gamemode creative {}", opts.bot_name))?;
        }
        Ok(server)
    }

    pub fn game_port(&self) -> u16 {
        self.game_port
    }

    /// Run a command; returns its output.
    pub fn cmd(&mut self, command: &str) -> Result<String> {
        self.rcon.cmd(command).with_context(|| format!("RCON `{command}`"))
    }

    /// Run a command and fail on the usual error replies.
    pub fn cmd_ok(&mut self, command: &str) -> Result<String> {
        let out = self.cmd(command)?;
        let lower = out.to_lowercase();
        let bad = ["unknown or incomplete command", "incorrect argument", "could not", "invalid", "expected", "unknown", "error", "failed", "not loaded", "cannot", "too many"];
        if bad.iter().any(|b| lower.contains(b)) {
            bail!("`{command}` -> {out}");
        }
        Ok(out)
    }

    pub fn game_time(&mut self) -> Result<i64> {
        let out = self.cmd("time query gametime")?;
        let num: String = out.chars().filter(|c| c.is_ascii_digit()).collect();
        num.parse().map_err(|_| anyhow!("cannot read the game time from `{out}`"))
    }

    /// Advance exactly `n` game ticks (the world is frozen otherwise).
    pub fn step(&mut self, n: u32) -> Result<()> {
        let started = Instant::now();
        let r = self.step_inner(n);
        self.stats.step += started.elapsed();
        self.stats.steps += 1;
        r
    }

    fn step_inner(&mut self, n: u32) -> Result<()> {
        let t0 = self.game_time()?;
        self.cmd_ok(&format!("tick step {n}"))?;
        let start = Instant::now();
        loop {
            let t = self.game_time()?;
            if t == t0 + n as i64 {
                return Ok(());
            }
            if t > t0 + n as i64 {
                bail!("game time jumped from {t0} to {t} for a {n}-tick step");
            }
            if start.elapsed() > Duration::from_secs(60) {
                bail!("game time stuck at {t} (wanted {})", t0 + n as i64);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// True when a block test passes (`execute if block <pos> <predicate>`).
    pub fn test_block(&mut self, p: Pos, predicate: &str) -> Result<bool> {
        let out = self.cmd(&format!("execute if block {} {} {} {predicate}", p[0], p[1], p[2]))?;
        if out.contains("Test passed") {
            Ok(true)
        } else if out.contains("Test failed") {
            Ok(false)
        } else {
            bail!("execute if block: {out}")
        }
    }

    /// Number of entities matching a selector (`execute if entity`).
    pub fn count_entities(&mut self, selector: &str) -> Result<u32> {
        let out = self.cmd(&format!("execute if entity {selector}"))?;
        if out.contains("Test failed") {
            return Ok(0);
        }
        let n: String = out.rsplit(':').next().unwrap_or("").chars().filter(|c| c.is_ascii_digit()).collect();
        n.parse().map_err(|_| anyhow!("execute if entity: {out}"))
    }

    /// Teleport the bot and wait until it has accepted the teleport.
    pub fn teleport_bot(&mut self, x: f64, y: f64, z: f64) -> Result<()> {
        self.teleport_bot_to(x, y, z, None)
    }

    /// Teleport the bot (feet position) and set where it looks (yaw, pitch in degrees: the
    /// server stores them as body and head rotation, what block placement reads), then wait
    /// until the bot has accepted the teleport (clicks sent after that are handled after it).
    pub fn teleport_bot_to(&mut self, x: f64, y: f64, z: f64, look: Option<(f32, f32)>) -> Result<()> {
        let bot = self.bot.as_ref().context("no bot")?;
        let (name, before) = (bot.name.clone(), bot.teleports_accepted());
        let rot = look.map(|(yaw, pitch)| format!(" {yaw} {pitch}")).unwrap_or_default();
        self.cmd_ok(&format!("tp {name} {x:.5} {y:.5} {z:.5}{rot}"))?;
        let t0 = Instant::now();
        loop {
            let bot = self.bot.as_ref().unwrap();
            if let Some(e) = bot.error() {
                bail!("bot: {e}");
            }
            if bot.teleports_accepted() > before {
                break;
            }
            if t0.elapsed() > Duration::from_secs(20) {
                bail!("the bot never acknowledged its teleport");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if look.is_none() {
            // The acceptance is handled on the server thread: make sure it ran before any click.
            self.cmd("list")?;
        }
        Ok(())
    }

    /// Put an item in the bot's main hand (the selected hotbar slot), or empty it.
    pub fn set_bot_main_hand(&mut self, item: Option<&str>) -> Result<()> {
        let name = self.bot.as_ref().context("no bot")?.name.clone();
        let item = item.unwrap_or("minecraft:air");
        self.cmd_ok(&format!("item replace entity {name} weapon.mainhand with {item}"))?;
        Ok(())
    }

    /// Right-click a block with the bot (empty hand), e.g. flip a lever.
    pub fn bot_use(&mut self, p: Pos, face: u8) -> Result<()> {
        self.bot.as_mut().context("no bot")?.use_item_on(p, face)
    }

    /// Save the world and read a box back from the region files.
    pub fn snapshot(&mut self, lo: Pos, hi: Pos) -> Result<Snapshot> {
        let started = Instant::now();
        let r = self.snapshot_inner(lo, hi);
        self.stats.snapshot += started.elapsed();
        self.stats.snapshots += 1;
        r
    }

    fn snapshot_inner(&mut self, lo: Pos, hi: Pos) -> Result<Snapshot> {
        // `save-all` only writes chunks marked unsaved, and scheduling or running a block tick
        // does not mark a chunk unsaved: a chunk whose blocks did not change keeps a STALE tick
        // list on disk. Flip a marker block at the bottom of the world in every chunk of the
        // box first (strict: no updates of any kind), so every chunk is saved fresh.
        if lo[1] <= MARKER_Y {
            bail!("box {lo:?}..{hi:?} reaches the marker layer y={MARKER_Y}");
        }
        self.markers_seen += 1;
        for cx in lo[0].div_euclid(16)..=hi[0].div_euclid(16) {
            for cz in lo[2].div_euclid(16)..=hi[2].div_euclid(16) {
                let on = self.markers.entry((cx, cz)).or_insert(false);
                *on = !*on;
                let marker = if *on { "minecraft:stone" } else { "minecraft:air" };
                let out = self.cmd(&format!("setblock {} {MARKER_Y} {} {marker} strict", cx * 16, cz * 16))?;
                if !out.contains("Changed the block") {
                    bail!("marker setblock in chunk {cx},{cz}: {out}");
                }
            }
        }
        let out = self.cmd("save-all flush")?;
        if !out.to_lowercase().contains("saved") {
            bail!("save-all flush: {out}");
        }
        // 26.x keeps each dimension under world/dimensions/<namespace>/<name>/.
        let modern = self.dir.join(&self.level_name).join("dimensions").join("minecraft").join("overworld");
        let dim = if modern.exists() { modern } else { self.dir.join(&self.level_name) };
        read_box(&dim, lo, hi)
    }
}

/// The layer where [`VanillaServer::snapshot`] flips its marker blocks (the overworld floor).
pub const MARKER_Y: i32 = -64;

impl Drop for VanillaServer {
    fn drop(&mut self) {
        self.bot = None;
        let _ = self.rcon.cmd("stop");
        let t0 = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                _ if t0.elapsed() > Duration::from_secs(30) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
                _ => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        if !self.keep_dir {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// Copy a directory tree (a saved world), skipping `session.lock`.
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

fn rand_u64() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) as u64;
    let mut x = t ^ (std::process::id() as u64).rotate_left(32) ^ 0x9e37_79b9_7f4a_7c15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x
}

/// Decoded chunk: sections by Y, plus ticks and block entities.
struct Chunk {
    /// section y -> (palette, packed data, bits)
    sections: HashMap<i32, (Vec<String>, Vec<i64>)>,
    block_ticks: Vec<(Pos, String, i64)>,
    fluid_ticks: usize,
    block_entities: Vec<(Pos, String)>,
}

impl Chunk {
    fn state(&self, x: i32, y: i32, z: i32) -> &str {
        let Some((palette, data)) = self.sections.get(&y.div_euclid(16)) else {
            return "minecraft:air";
        };
        if palette.len() <= 1 || data.is_empty() {
            return palette.first().map(|s| s.as_str()).unwrap_or("minecraft:air");
        }
        let bits = (usize::BITS - (palette.len() - 1).leading_zeros()).max(4) as usize;
        let per_long = 64 / bits;
        let idx = ((y.rem_euclid(16) * 16 + z.rem_euclid(16)) * 16 + x.rem_euclid(16)) as usize;
        let word = data[idx / per_long] as u64;
        let v = (word >> ((idx % per_long) * bits)) & ((1u64 << bits) - 1);
        if v as usize >= palette.len() && std::env::var_os("MCTAS_DEBUG_ANVIL").is_some() {
            eprintln!("anvil: ({x},{y},{z}) idx {idx} bits {bits} value {v} palette {palette:?} data len {}", data.len());
        }
        palette.get(v as usize).map(|s| s.as_str()).unwrap_or("?")
    }
}

fn read_chunk(world: &Path, cx: i32, cz: i32, cache: &mut HashMap<(i32, i32), Vec<u8>>) -> Result<Option<Chunk>> {
    let (rx, rz) = (cx.div_euclid(32), cz.div_euclid(32));
    if let std::collections::hash_map::Entry::Vacant(e) = cache.entry((rx, rz)) {
        let path = world.join("region").join(format!("r.{rx}.{rz}.mca"));
        e.insert(std::fs::read(&path).unwrap_or_default());
    }
    let file = &cache[&(rx, rz)];
    if file.len() < 8192 {
        return Ok(None);
    }
    let slot = 4 * ((cx.rem_euclid(32) + cz.rem_euclid(32) * 32) as usize);
    let loc = u32::from_be_bytes(file[slot..slot + 4].try_into()?);
    let (offset, count) = ((loc >> 8) as usize * 4096, (loc & 0xff) as usize);
    if offset == 0 || count == 0 {
        return Ok(None);
    }
    let len = u32::from_be_bytes(file.get(offset..offset + 4).context("region truncated")?.try_into()?) as usize;
    let kind = *file.get(offset + 4).context("region truncated")?;
    let raw = file.get(offset + 5..offset + 4 + len).context("region chunk truncated")?;
    if kind & 0x80 != 0 {
        bail!("chunk {cx},{cz} is stored in an external .mcc file (not supported)");
    }
    let bytes: Vec<u8> = match kind {
        3 => raw.to_vec(),
        2 => {
            let mut v = Vec::new();
            flate2::read::ZlibDecoder::new(raw).read_to_end(&mut v)?;
            v
        }
        1 => {
            let mut v = Vec::new();
            flate2::read::GzDecoder::new(raw).read_to_end(&mut v)?;
            v
        }
        k => bail!("chunk {cx},{cz}: unsupported compression {k}"),
    };
    let root = nbt::parse_named(&bytes)?;
    let mut chunk = Chunk { sections: HashMap::new(), block_ticks: Vec::new(), fluid_ticks: 0, block_entities: Vec::new() };
    for sec in root.get("sections").map(|t| t.as_list()).unwrap_or(&[]) {
        let y = sec.get("Y").and_then(|t| t.as_i64()).unwrap_or(0) as i32;
        if let Some(bs) = sec.get("block_states") {
            if std::env::var_os("MCTAS_DEBUG_ANVIL").is_some() {
                if let Some(p) = bs.get("palette") {
                    if p.as_list().len() > 1 {
                        eprintln!("anvil: section {y} palette {:?}", p.as_list().iter().take(4).collect::<Vec<_>>());
                    }
                }
            }
            let palette: Vec<String> = bs.get("palette").map(|t| t.as_list()).unwrap_or(&[]).iter().map(nbt::block_state_string).collect();
            let data = bs.get("data").map(|t| t.as_long_array().to_vec()).unwrap_or_default();
            chunk.sections.insert(y, (palette, data));
        }
    }
    let pos_of = |t: &nbt::Tag| -> Pos {
        [
            t.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            t.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            t.get("z").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        ]
    };
    for t in root.get("block_ticks").map(|t| t.as_list()).unwrap_or(&[]) {
        let id = t.get("i").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        chunk.block_ticks.push((pos_of(t), id, t.get("t").and_then(|v| v.as_i64()).unwrap_or(0)));
    }
    chunk.fluid_ticks = root.get("fluid_ticks").map(|t| t.as_list().len()).unwrap_or(0);
    for t in root.get("block_entities").map(|t| t.as_list()).unwrap_or(&[]) {
        let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        chunk.block_entities.push((pos_of(t), id));
    }
    Ok(Some(chunk))
}

/// Read a box of a saved world (overworld region files).
pub fn read_box(world: &Path, lo: Pos, hi: Pos) -> Result<Snapshot> {
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut cache = HashMap::new();
    let mut chunks: HashMap<(i32, i32), Chunk> = HashMap::new();
    for cx in lo[0].div_euclid(16)..=hi[0].div_euclid(16) {
        for cz in lo[2].div_euclid(16)..=hi[2].div_euclid(16) {
            let c = read_chunk(world, cx, cz, &mut cache)?.with_context(|| format!("chunk {cx},{cz} was never saved (not loaded?)"))?;
            chunks.insert((cx, cz), c);
        }
    }
    let mut palette: Vec<String> = Vec::new();
    let mut index: HashMap<String, u16> = HashMap::new();
    let mut data = Vec::with_capacity((size[0] * size[1] * size[2]) as usize);
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let c = &chunks[&(x.div_euclid(16), z.div_euclid(16))];
                let s = c.state(x, y, z);
                let id = match index.get(s) {
                    Some(&i) => i,
                    None => {
                        let i = palette.len() as u16;
                        palette.push(s.to_string());
                        index.insert(s.to_string(), i);
                        i
                    }
                };
                data.push(id);
            }
        }
    }
    // Ticks and block entities of the whole chunks: the simulator's `idle` is level-wide too.
    let mut block_ticks = Vec::new();
    let mut fluid_ticks = 0;
    let mut block_entities = Vec::new();
    for c in chunks.values() {
        block_ticks.extend(c.block_ticks.iter().cloned());
        fluid_ticks += c.fluid_ticks;
        block_entities.extend(c.block_entities.iter().cloned());
    }
    block_ticks.sort();
    block_entities.sort();
    Ok(Snapshot { region: Region { from: lo, size, palette, flags: Vec::new(), data }, block_ticks, fluid_ticks, block_entities })
}
