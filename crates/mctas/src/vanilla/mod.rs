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

    /// [`Rcon::cmd`] for a long answer (mobinfra: entity dumps). `RconClient.sendCmdResponse`
    /// splits an answer into packets of 4096 characters with the same id, and the server drops
    /// the connection when two requests arrive in one read (`pktsize != read - 4`), so no fence
    /// command can follow: after a full 4096-character packet the next one is awaited for 300 ms
    /// (an answer of an exact multiple of 4096 ends on that wait).
    pub fn cmd_long(&mut self, command: &str) -> Result<String> {
        let id = self.send(2, command)?;
        let mut out = String::new();
        loop {
            let (rid, _, body) = self.read()?;
            if rid != id {
                continue;
            }
            let full = body.len() == 4096;
            out.push_str(&body);
            if !full {
                return Ok(out);
            }
            self.s.set_read_timeout(Some(Duration::from_millis(300)))?;
            let mut len = [0u8; 4];
            let got = self.s.peek(&mut len);
            self.s.set_read_timeout(Some(Duration::from_secs(120)))?;
            match got {
                Ok(n) if n > 0 => continue,
                _ => return Ok(out),
            }
        }
    }
}

/// Two distinct free ports (the game's and RCON's): both listeners held until both are chosen,
/// so the kernel cannot hand out the same port twice here.
fn free_ports() -> Result<(u16, u16)> {
    let a = TcpListener::bind("127.0.0.1:0")?;
    let b = TcpListener::bind("127.0.0.1:0")?;
    Ok((a.local_addr()?.port(), b.local_addr()?.port()))
}

/// The server log's sign that a port it was given got taken between our choice and its bind
/// (another server started at the same moment: `mctas test` runs many at once): start again.
fn port_taken(log: &Path) -> bool {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.contains("FAILED TO BIND") || text.contains("Unable to initialise RCON") || text.contains("Address already in use")
}

/// The server's `server.properties`: the world kind (`(level-type, generator-settings,
/// generate-structures)`), the ports and RCON password chosen for this start, and the knobs of
/// `opts`.
fn write_properties(dir: &Path, opts: &VanillaOptions, world: (String, String, String), game_port: u16, rcon_port: u16, password: &str) -> Result<()> {
    let (level_type, gen_settings, structures) = world;
    let props = [
        ("level-name", opts.level_name.clone()),
        ("level-type", level_type),
        ("generator-settings", gen_settings),
        ("generate-structures", structures),
        ("level-seed", opts.seed.to_string()),
        ("online-mode", "false".to_string()),
        ("enforce-secure-profile", "false".to_string()),
        ("server-ip", "127.0.0.1".to_string()),
        ("server-port", game_port.to_string()),
        ("enable-rcon", "true".to_string()),
        ("rcon.port", rcon_port.to_string()),
        ("rcon.password", password.to_string()),
        ("broadcast-rcon-to-ops", "false".to_string()),
        ("enable-query", "false".to_string()),
        ("enable-status", "true".to_string()),
        ("spawn-protection", "0".to_string()),
        ("enable-command-block", opts.command_blocks.to_string()),
        ("difficulty", opts.difficulty.clone()),
        ("gamemode", opts.gamemode.clone()),
        ("force-gamemode", opts.force_gamemode.to_string()),
        ("allow-flight", "true".to_string()),
        ("max-players", "4".to_string()),
        ("max-tick-time", "-1".to_string()),
        ("pause-when-empty-seconds", "0".to_string()),
        ("view-distance", opts.view_distance.to_string()),
        ("simulation-distance", opts.simulation_distance.to_string()),
        ("network-compression-threshold", "-1".to_string()),
        ("sync-chunk-writes", "false".to_string()),
        // A real world compresses its region files (26.3's `deflate`; read_box reads it);
        // the void world's files stay raw (tiny, and snapshots parse them constantly).
        ("region-file-compression", if opts.default_world { "deflate".to_string() } else { "none".to_string() }),
        ("log-ips", "false".to_string()),
        ("white-list", "false".to_string()),
        ("enforce-whitelist", "false".to_string()),
    ];
    let text: String = props.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    std::fs::write(dir.join("server.properties"), text)?;
    Ok(())
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
    /// `level-seed` of a fresh world (0 by default; ignored for a copied-in `world`).
    pub seed: i64,
    /// A default-generated world (`minecraft:normal`, structures on) instead of the void
    /// superflat (the `bedrock video` tutorial: a real random world).
    pub default_world: bool,
    /// `gamemode` server property (players' default mode).
    pub gamemode: String,
    /// `force-gamemode` server property.
    pub force_gamemode: bool,
    /// `difficulty` server property.
    pub difficulty: String,
    /// Freeze time (`/tick freeze`, `save-off`) and step it by hand; false = real time.
    pub frozen: bool,
    /// Connect the network bot (a player in the world; the video's real client needs none).
    pub with_bot: bool,
    /// simulation-distance (ticking chunks around players).
    pub simulation_distance: u32,
    /// The void superflat's biome (`minecraft:the_void` by default; mobinfra: the mob-farm gate
    /// runs in `minecraft:plains`, where it rains).
    pub biome: String,
    /// mobinfra: the copied-in `world`'s weather, written into its `data/minecraft/weather.dat`
    /// before the server starts (the mob-farm gate in a terrain sample's kept world: the cycle's
    /// stationary state without warm-up ticks that would age the world).
    pub saved_weather: Option<SavedWeather>,
    /// A Java agent for the server (`-javaagent:`): the mob-farm gate's golem agent
    /// (`bridge::golem_agent`) tags every golem a villager summons with the villager's UUID.
    pub java_agent: Option<PathBuf>,
    /// `enable-command-block` (off, vanilla's default): on for a harness that counts ticks with
    /// in-game command blocks (mobai load-count).
    pub command_blocks: bool,
}

/// A world's saved weather (`WeatherData`: 26.x `data/minecraft/weather.dat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedWeather {
    pub clear_weather_time: i32,
    pub rain_time: i32,
    pub thunder_time: i32,
    pub raining: bool,
    pub thundering: bool,
}

/// Write `w` as world folder `world`'s saved weather (`data/minecraft/weather.dat`, gzipped NBT
/// `{data: {...}, DataVersion}`), keeping the file's other fields when it exists (its
/// DataVersion; else level.dat's).
pub fn write_saved_weather(world: &Path, w: &SavedWeather) -> Result<()> {
    use std::io::{Read, Write};
    let read_gz = |p: &Path| -> Result<nbt::Tag> {
        let bytes = std::fs::read(p).with_context(|| format!("read {}", p.display()))?;
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(&bytes[..]).read_to_end(&mut raw)?;
        nbt::parse_named(&raw)
    };
    let path = world.join("data").join("minecraft").join("weather.dat");
    let mut root = match read_gz(&path) {
        Ok(nbt::Tag::Compound(m)) => m,
        _ => {
            let level = read_gz(&world.join("level.dat"))?;
            let dv = level.get("Data").and_then(|d| d.get("DataVersion")).and_then(|v| v.as_i64()).context("level.dat without Data.DataVersion")?;
            let mut m = std::collections::BTreeMap::new();
            m.insert("DataVersion".to_string(), nbt::Tag::Int(dv as i32));
            m
        }
    };
    let mut data = std::collections::BTreeMap::new();
    data.insert("clear_weather_time".to_string(), nbt::Tag::Int(w.clear_weather_time));
    data.insert("rain_time".to_string(), nbt::Tag::Int(w.rain_time));
    data.insert("thunder_time".to_string(), nbt::Tag::Int(w.thunder_time));
    data.insert("raining".to_string(), nbt::Tag::Byte(w.raining as i8));
    data.insert("thundering".to_string(), nbt::Tag::Byte(w.thundering as i8));
    root.insert("data".to_string(), nbt::Tag::Compound(data));
    std::fs::create_dir_all(path.parent().context("weather.dat's folder")?)?;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&nbt::write_named("", &nbt::Tag::Compound(root)))?;
    std::fs::write(&path, gz.finish()?)?;
    Ok(())
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
            seed: 0,
            default_world: false,
            gamemode: "creative".into(),
            force_gamemode: true,
            difficulty: "peaceful".into(),
            frozen: true,
            with_bot: true,
            simulation_distance: 3,
            biome: "minecraft:the_void".into(),
            saved_weather: None,
            java_agent: None,
            command_blocks: false,
        }
    }
}

/// What one `save-all` shows of a box.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Block states of the box (no collision flags: those come from the simulator).
    pub region: Region,
    /// Scheduled block ticks in the box's chunks: position, block id, ticks until due.
    pub block_ticks: Vec<(Pos, String, i64)>,
    pub fluid_ticks: usize,
    /// (tickexact 2026-11-13, ironspec 02:28 PT (f2)) Every scheduled fluid tick's own position in
    /// the box's chunks, so a check can ask about one cell rather than a total.
    pub fluid_tick_cells: Vec<Pos>,
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
    /// Start the official server jar in a fresh world directory and connect RCON (again on new
    /// ports, up to 3 times, when a port was taken before the server bound it).
    pub fn start(opts: &VanillaOptions) -> Result<VanillaServer> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match Self::start_once(opts) {
                Err(e) if attempt < 3 && e.to_string().contains("port taken") => eprintln!("vanilla server: {e:#}; starting again on new ports"),
                r => return r,
            }
        }
    }

    fn start_once(opts: &VanillaOptions) -> Result<VanillaServer> {
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
        let (game_port, rcon_port) = free_ports()?;
        let password = format!("p{:x}", rand_u64());
        std::fs::write(dir.join("eula.txt"), "eula=true\n")?;
        if let Some(src) = &opts.world {
            copy_tree(src, &dir.join(&opts.level_name))?;
            if let Some(w) = &opts.saved_weather {
                write_saved_weather(&dir.join(&opts.level_name), w)?;
            }
        } else if opts.saved_weather.is_some() {
            bail!("saved_weather needs a copied-in world");
        }
        let (level_type, gen_settings, structures) = if opts.default_world {
            ("minecraft\\:normal".to_string(), "{}".to_string(), "true".to_string())
        } else {
            (
                "minecraft\\:flat".to_string(),
                format!(r#"{{"layers":[{{"block":"minecraft:air","height":1}}],"biome":"{}","features":false,"lakes":false}}"#, opts.biome),
                "false".to_string(),
            )
        };
        write_properties(&dir, opts, (level_type, gen_settings, structures), game_port, rcon_port, &password)?;
        let log = dir.join("server.log");
        let out = std::fs::File::create(&log)?;
        let repo = crate::paths::version_dir(&opts.version).join("vanilla-bundler-repo");
        std::fs::create_dir_all(&repo)?;
        // (farmopt 2026-11-11) the bundler unpacks its libraries into the shared repo on its first
        // start; servers started together on a fresh box raced on it (8 at once: 3 exited with
        // "Failed to extract server libraries", a FileAlreadyExistsException on a jar another had
        // just written). Until the repo holds its marker, a start holds the repo's lock until its
        // server is up (the bundler done), then writes the marker; a later start sees the marker
        // and takes no lock (the bundler then only checks the jars). The kernel drops the lock of
        // a process that dies.
        let extract_guard = bundler_extract_guard(&repo)?;
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
        command
            .arg(format!("-Xmx{}m", opts.heap_mb))
            .arg("-XX:+UseSerialGC")
            .arg("-XX:ActiveProcessorCount=2")
            .arg(format!("-DbundlerRepoDir={}", repo.display()));
        if let Some(agent) = &opts.java_agent {
            command.arg(format!("-javaagent:{}", agent.display()));
        }
        command
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
                if port_taken(&log) {
                    let _ = std::fs::remove_dir_all(&dir);
                    bail!("port taken: the vanilla server could not bind {game_port} / {rcon_port} ({status})");
                }
                bail!("the vanilla server exited ({status}); see {}", log.display());
            }
            if port_taken(&log) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(&dir);
                bail!("port taken: the vanilla server could not bind {game_port} / {rcon_port}");
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
        // (the server is up: the bundler's libraries are all in the repo)
        if let Some(lock) = extract_guard {
            let _ = std::fs::write(repo.join(BUNDLER_MARKER), b"");
            drop(lock);
        }
        let mut server = VanillaServer { child, dir, rcon, bot: None, game_port, keep_dir: opts.keep_dir, log, stats: Stats::default(), tick_rate: opts.tick_rate, dirty: None, markers: HashMap::new(), markers_seen: 0, level_name: opts.level_name.clone() };
        for c in [
            "gamerule spawn_mobs false",
            "gamerule spawn_monsters false",
            "gamerule advance_weather false",
            "gamerule advance_time false",
            "gamerule log_admin_commands false",
        ] {
            server.cmd_ok(c)?;
        }
        if opts.frozen {
            server.cmd_ok("tick freeze")?;
            server.cmd_ok("save-off")?;
            if opts.tick_rate != 20 {
                server.cmd_ok(&format!("tick rate {}", opts.tick_rate))?;
            }
        }
        if opts.with_bot {
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

    /// Run a command whose answer may span several RCON packets ([`Rcon::cmd_long`]).
    pub fn cmd_long(&mut self, command: &str) -> Result<String> {
        self.rcon.cmd_long(command).with_context(|| format!("RCON `{command}`"))
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

    /// (tickexact 2026-11-13, ironspec 02:27 PT) Stop the server with the bot still connected:
    /// MinecraftServer.stopServer saves every player (PlayerList.saveAll) before it removes them,
    /// the way a dedicated server's restart does; the bot is dropped once the process has exited.
    /// (Dropping the server instead disconnects the bot first: the player's own logout path.)
    pub fn stop_with_bot(&mut self) -> Result<()> {
        let _ = self.rcon.cmd("stop");
        let t0 = Instant::now();
        while self.child.try_wait()?.is_none() {
            if t0.elapsed() > Duration::from_secs(60) {
                bail!("the server did not stop within 60 s");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        self.bot = None;
        Ok(())
    }

    /// Kill the server process without a stop (a crash or power loss): nothing after the last save
    /// reaches the disk.
    pub fn kill_hard(&mut self) -> Result<()> {
        self.child.kill()?;
        self.child.wait()?;
        self.bot = None;
        Ok(())
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
        read_box(&self.dim_dir("overworld"), lo, hi)
    }

    /// A dimension's folder of this server's world (26.x: `world/dimensions/minecraft/<dim>/`,
    /// e.g. `the_nether`; the overworld of older layouts is the world folder itself).
    pub fn dim_dir(&self, dim: &str) -> PathBuf {
        let modern = self.dir.join(&self.level_name).join("dimensions").join("minecraft").join(dim);
        if modern.exists() || dim != "overworld" { modern } else { self.dir.join(&self.level_name) }
    }

    /// `save-all flush`, then read a box of a dimension from its region files. Unlike
    /// [`VanillaServer::snapshot`] no marker blocks are flipped: right for plain block reads of
    /// a live world, where the interesting chunks were just changed by players (and so are
    /// written), but not for fresh scheduled-tick lists of untouched chunks.
    pub fn read_dim_box(&mut self, dim: &str, lo: Pos, hi: Pos) -> Result<Snapshot> {
        let out = self.cmd("save-all flush")?;
        if !out.to_lowercase().contains("saved") {
            bail!("save-all flush: {out}");
        }
        read_box(&self.dim_dir(dim), lo, hi)
    }
}

/// The layer where [`VanillaServer::snapshot`] flips its marker blocks (the overworld floor).
pub const MARKER_Y: i32 = -64;

/// The marker a bundler repo holds once a server started on it (its libraries all unpacked).
pub const BUNDLER_MARKER: &str = ".extracted";

/// A server start's lock on the bundler repo while the repo has no marker
/// ([`BUNDLER_MARKER`]): the lock file `.extract.lock` held exclusively (`File::lock`, blocking),
/// or None once the marker is there (also when it appeared while this start waited).
pub fn bundler_extract_guard(repo: &Path) -> Result<Option<std::fs::File>> {
    if repo.join(BUNDLER_MARKER).exists() {
        return Ok(None);
    }
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(repo.join(".extract.lock")).with_context(|| format!("the bundler repo's lock in {}", repo.display()))?;
    f.lock().with_context(|| format!("locking {}", repo.join(".extract.lock").display()))?;
    if repo.join(BUNDLER_MARKER).exists() {
        return Ok(None);
    }
    Ok(Some(f))
}

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
    fluid_tick_cells: Vec<Pos>,
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

fn read_chunk_root(world: &Path, cx: i32, cz: i32, cache: &mut HashMap<(i32, i32), Vec<u8>>) -> Result<Option<nbt::Tag>> {
    read_region_root(&world.join("region"), cx, cz, cache)
}

/// One chunk's root compound from the `r.X.Z.mca` files of a folder (`region/` for blocks,
/// `entities/` for entities, `poi/`), `None` when the chunk was never saved; `cache` keeps the
/// region files read so far.
pub fn read_region_root(folder: &Path, cx: i32, cz: i32, cache: &mut HashMap<(i32, i32), Vec<u8>>) -> Result<Option<nbt::Tag>> {
    let (rx, rz) = (cx.div_euclid(32), cz.div_euclid(32));
    if let std::collections::hash_map::Entry::Vacant(e) = cache.entry((rx, rz)) {
        let path = folder.join(format!("r.{rx}.{rz}.mca"));
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
    Ok(Some(nbt::parse_named(&bytes)?))
}

fn read_chunk(world: &Path, cx: i32, cz: i32, cache: &mut HashMap<(i32, i32), Vec<u8>>) -> Result<Option<Chunk>> {
    let Some(root) = read_chunk_root(world, cx, cz, cache)? else {
        return Ok(None);
    };
    let mut chunk = Chunk { sections: HashMap::new(), block_ticks: Vec::new(), fluid_ticks: 0, fluid_tick_cells: Vec::new(), block_entities: Vec::new() };
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
    chunk.fluid_tick_cells = root.get("fluid_ticks").map(|t| t.as_list()).unwrap_or(&[]).iter().map(pos_of).collect();
    for t in root.get("block_entities").map(|t| t.as_list()).unwrap_or(&[]) {
        let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        chunk.block_entities.push((pos_of(t), id));
    }
    Ok(Some(chunk))
}

/// One saved heightmap value (`Heightmaps.<name>`) for a column of a saved dimension
/// directory: the first-available y exactly as the game's `getHeight` reports it.
/// `height` is the chunk's HeightAccessor height (256 in the Nether: y 0..255,
/// even though its logical height is 128), fixing the packed bits per value the
/// way `Heightmap`'s constructor does (`ceillog2(height + 1)`).
pub fn read_heightmap(world: &Path, name: &str, x: i32, z: i32, min_y: i32, height: i32) -> Result<Option<i32>> {
    let mut cache = HashMap::new();
    let Some(root) = read_chunk_root(world, x.div_euclid(16), z.div_euclid(16), &mut cache)? else {
        return Ok(None);
    };
    let Some(tag) = root.get("Heightmaps").and_then(|h| h.get(name)) else {
        return Ok(None);
    };
    let data = tag.as_long_array();
    let bits = 32 - (height as u32).leading_zeros();
    let per_long = 64 / bits as usize;
    let idx = (x.rem_euclid(16) + z.rem_euclid(16) * 16) as usize;
    let word = data.get(idx / per_long).copied().unwrap_or(0) as u64;
    let v = (word >> ((idx % per_long) as u32 * bits)) & ((1u64 << bits) - 1);
    if std::env::var_os("MCTAS_DEBUG_HEIGHTMAP").is_some() {
        eprintln!("heightmap {name} [{x},{z}] data_len={} bits={bits} per_long={per_long} idx={idx} word={word:#x} v={v} first_words={:?}", data.len(), data.iter().take(6).collect::<Vec<_>>());
    }
    if data.is_empty() {
        return Ok(None);
    }
    Ok(Some(v as i32 + min_y))
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
    let mut fluid_tick_cells = Vec::new();
    let mut block_entities = Vec::new();
    for c in chunks.values() {
        block_ticks.extend(c.block_ticks.iter().cloned());
        fluid_ticks += c.fluid_ticks;
        fluid_tick_cells.extend(c.fluid_tick_cells.iter().cloned());
        block_entities.extend(c.block_entities.iter().cloned());
    }
    block_ticks.sort();
    fluid_tick_cells.sort();
    block_entities.sort();
    Ok(Snapshot { region: Region { from: lo, size, palette, flags: Vec::new(), data }, block_ticks, fluid_ticks, fluid_tick_cells, block_entities })
}

#[cfg(test)]
mod bundler_tests {
    use super::*;

    /// The bundler repo's first-start lock (farmopt 2026-11-11: servers started together on a
    /// fresh box raced to unpack the bundler's jars): without the marker a start holds the repo's
    /// lock (another handle cannot take it meanwhile); once the marker is written and the lock
    /// dropped, a start takes no lock.
    #[test]
    fn the_first_start_holds_the_bundler_repo_until_its_marker() {
        let repo = std::env::temp_dir().join(format!("mctas-bundler-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(&repo).unwrap();
        let first = bundler_extract_guard(&repo).unwrap().expect("no marker: the first start locks");
        let other = std::fs::OpenOptions::new().write(true).open(repo.join(".extract.lock")).unwrap();
        assert!(other.try_lock().is_err(), "a second start waits while the first unpacks");
        std::fs::write(repo.join(BUNDLER_MARKER), b"").unwrap();
        drop(first);
        assert!(other.try_lock().is_ok(), "the lock is free once the first start is up");
        drop(other);
        assert!(bundler_extract_guard(&repo).unwrap().is_none(), "the marker there: no lock");
        let _ = std::fs::remove_dir_all(&repo);
    }
}
