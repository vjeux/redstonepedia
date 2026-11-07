//! Videos rendered by the real Minecraft client.
//!
//! The client runs unmodified (plus a tiny launcher that hides the HUD) on a virtual X display
//! (Xvfb) with Mesa's software Vulkan/OpenGL, joins a simulator worker that listens on
//! localhost as a spectator, and ffmpeg records the display while the simulator steps the
//! contraption at a watchable tick rate. Linux only.

use crate::net::{self, Checksum};
use crate::paths;
use crate::setup::{self, ClientInstall};
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CLIENT_MAIN: &str = include_str!("../../../bridge/client/dev/vjeux/mctas/client/ClientMain.java");
const ACTOR_MAIN: &str = include_str!("../../../bridge/client/dev/vjeux/mctas/client/ActorMain.java");
const ACTOR_SRC: &str = include_str!("../../../bridge/actor/dev/vjeux/mctas/actor/Actor.java");
const FFMPEG_URL: &str = "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz";

/// Client install + our launcher class.
#[derive(Clone, Debug)]
pub struct ClientBuild {
    pub install: ClientInstall,
    pub classes: PathBuf,
}

impl ClientBuild {
    pub fn classpath(&self) -> String {
        let mut parts = vec![self.classes.display().to_string(), self.install.client_jar.display().to_string()];
        parts.extend(self.install.libs.iter().map(|p| p.display().to_string()));
        parts.join(":")
    }
}

/// Download the client and compile the launcher.
pub fn build_client(version: &str) -> Result<ClientBuild> {
    let install = setup::install_client(version)?;
    let hash = &net::sha256_hex(format!("{version}{CLIENT_MAIN}").as_bytes())[..12];
    let dir = paths::version_dir(version).join(format!("client-launcher-{hash}"));
    let classes = dir.join("classes");
    let build = ClientBuild { install, classes: classes.clone() };
    if dir.join(".built").exists() {
        return Ok(build);
    }
    let src = dir.join("src/dev/vjeux/mctas/client/ClientMain.java");
    std::fs::create_dir_all(src.parent().unwrap())?;
    std::fs::write(&src, CLIENT_MAIN)?;
    std::fs::create_dir_all(&classes)?;
    let mut cp = vec![build.install.client_jar.display().to_string()];
    cp.extend(build.install.libs.iter().map(|p| p.display().to_string()));
    let major = setup::required_java(version)?;
    let out = Command::new(setup::javac_for(&build.install.java))
        .args(["--release", &major.to_string(), "-nowarn", "-cp", &cp.join(":"), "-d"])
        .arg(&classes)
        .arg(&src)
        .output()?;
    if !out.status.success() {
        bail!("javac (client launcher) failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    std::fs::write(dir.join(".built"), b"ok")?;
    Ok(build)
}

/// The client with the actor launcher (`dev.vjeux.mctas.client.ActorMain`: the scripted survival
/// player of `mctas bedrock video`, HUD visible; the same driver as the Fabric mod of
/// [`crate::fabric`], without the loader). Launch with
/// [`Client::launch_main`] and main `dev.vjeux.mctas.client.ActorMain`.
pub fn build_actor_client(version: &str) -> Result<ClientBuild> {
    let install = setup::install_client(version)?;
    let hash = &net::sha256_hex(format!("{version}{ACTOR_MAIN}{ACTOR_SRC}").as_bytes())[..12];
    let dir = paths::version_dir(version).join(format!("actor-launcher-{hash}"));
    let classes = dir.join("classes");
    let build = ClientBuild { install, classes: classes.clone() };
    if dir.join(".built").exists() {
        return Ok(build);
    }
    let main_src = dir.join("src/dev/vjeux/mctas/client/ActorMain.java");
    let actor_src = dir.join("src/dev/vjeux/mctas/actor/Actor.java");
    for (src, text) in [(&main_src, ACTOR_MAIN), (&actor_src, ACTOR_SRC)] {
        std::fs::create_dir_all(src.parent().unwrap())?;
        std::fs::write(src, text)?;
    }
    std::fs::create_dir_all(&classes)?;
    let mut cp = vec![build.install.client_jar.display().to_string()];
    cp.extend(build.install.libs.iter().map(|p| p.display().to_string()));
    let major = setup::required_java(version)?;
    let out = Command::new(setup::javac_for(&build.install.java))
        .args(["--release", &major.to_string(), "-nowarn", "-cp", &cp.join(":"), "-d"])
        .arg(&classes)
        .arg(&main_src)
        .arg(&actor_src)
        .output()?;
    if !out.status.success() {
        bail!("javac (actor launcher) failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    std::fs::write(dir.join(".built"), b"ok")?;
    Ok(build)
}

/// A static ffmpeg build (downloaded once).
pub fn ffmpeg() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("MCTAS_FFMPEG") {
        return Ok(PathBuf::from(p));
    }
    let dir = paths::downloads();
    let find = || -> Option<PathBuf> {
        std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|e| e.path().join("ffmpeg"))
            .find(|p| p.exists() && p.parent().is_some_and(|d| d.to_string_lossy().contains("static")))
    };
    if let Some(p) = find() {
        return Ok(p);
    }
    let tarball = dir.join("ffmpeg-static.tar.xz");
    net::download(FFMPEG_URL, &tarball, Checksum::None)?;
    let status = Command::new("tar").arg("-xf").arg(&tarball).arg("-C").arg(&dir).status()?;
    if !status.success() {
        bail!("extracting ffmpeg failed");
    }
    find().context("ffmpeg not found after extraction")
}

/// A virtual X display.
pub struct Display {
    child: Child,
    pub number: u32,
}

impl Display {
    pub fn start(width: u32, height: u32) -> Result<Display> {
        // One at a time per process, and only a display whose Xvfb is still alive counts (two
        // processes racing for the same number: the loser's Xvfb exits and it moves on).
        static START: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = START.lock().unwrap_or_else(|e| e.into_inner());
        // Locks left by an Xvfb that died without cleaning up (killed) are reclaimed: their
        // process is gone, so nobody uses those displays. (A server takes its lock before it
        // creates its socket, so a socket without a lock is an orphan too.)
        for number in 90..200u32 {
            let lock = PathBuf::from(format!("/tmp/.X{number}-lock"));
            let socket = PathBuf::from(format!("/tmp/.X11-unix/X{number}"));
            if lock.exists() && !lock_alive(&lock) {
                let _ = std::fs::remove_file(&lock);
                let _ = std::fs::remove_file(&socket);
            }
            if !lock.exists() && socket.exists() {
                let _ = std::fs::remove_file(&socket);
            }
        }
        for number in 90..200u32 {
            let lock = PathBuf::from(format!("/tmp/.X{number}-lock"));
            let socket = PathBuf::from(format!("/tmp/.X11-unix/X{number}"));
            if lock.exists() || socket.exists() {
                continue;
            }
            let mut child = Command::new("Xvfb")
                .arg(format!(":{number}"))
                .args(["-screen", "0", &format!("{width}x{height}x24"), "-nolisten", "tcp", "+extension", "GLX"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .context("starting Xvfb (install xorg-x11-server-Xvfb)")?;
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut died = false;
            while Instant::now() < deadline {
                if child.try_wait().ok().flatten().is_some() {
                    died = true;
                    break;
                }
                if socket.exists() {
                    std::thread::sleep(Duration::from_millis(200));
                    if child.try_wait().ok().flatten().is_some() {
                        died = true;
                        break;
                    }
                    return Ok(Display { child, number });
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            if died {
                continue;
            }
            let _ = child.kill();
            bail!("Xvfb :{number} did not come up");
        }
        bail!("no free X display number")
    }

    pub fn name(&self) -> String {
        format!(":{}", self.number)
    }
}

/// True when the process named in an X lock file is still running (or the file is unreadable).
fn lock_alive(lock: &Path) -> bool {
    match std::fs::read_to_string(lock).ok().and_then(|s| s.trim().parse::<u32>().ok()) {
        Some(pid) => Path::new(&format!("/proc/{pid}")).exists(),
        None => true,
    }
}

/// Stop a child politely (SIGTERM, so Xvfb removes its lock file and socket), then for good.
fn terminate(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    let _ = Command::new("kill").arg("-TERM").arg(child.id().to_string()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

impl Drop for Display {
    fn drop(&mut self) {
        terminate(&mut self.child);
    }
}

/// The real game client, joining `localhost:port`.
pub struct Client {
    child: Child,
    pub log: PathBuf,
}

impl Client {
    pub fn launch(build: &ClientBuild, display: &Display, port: u16, width: u32, height: u32, game_dir: &Path) -> Result<Client> {
        Client::launch_main(build, display, Some(port), width, height, game_dir, "dev.vjeux.mctas.client.ClientMain", &[], &[])
    }

    /// [`Client::launch`] with another main class, JVM arguments and classpath entries put before
    /// the client's (the Fabric loader: `fabric::launch`); without a port the client stays on its
    /// title screen (a mod may open a world).
    #[allow(clippy::too_many_arguments)]
    pub fn launch_main(build: &ClientBuild, display: &Display, port: Option<u16>, width: u32, height: u32, game_dir: &Path, main: &str, jvm: &[String], cp_first: &[PathBuf]) -> Result<Client> {
        std::fs::create_dir_all(game_dir)?;
        // First-launch screens and distractions off; small render distance (the scene is small);
        // particles minimal (`MCTAS_CLIENT_PARTICLES=0`: all, to picture an explosion's)
        let particles = std::env::var("MCTAS_CLIENT_PARTICLES").ok().filter(|v| matches!(v.as_str(), "0" | "1" | "2")).unwrap_or_else(|| "2".into());
        let render_distance = std::env::var("MCTAS_CLIENT_RENDER_DISTANCE").ok().and_then(|v| v.parse::<u32>().ok()).filter(|v| (2..=16).contains(v)).unwrap_or(6);
        std::fs::write(
            game_dir.join("options.txt"),
            format!(
                "onboardAccessibility:false\nskipMultiplayerWarning:true\njoinedFirstServer:true\ntutorialStep:none\n\
             pauseOnLostFocus:false\nrenderDistance:{render_distance}\nsimulationDistance:5\nfov:0.0\nnarrator:0\nsoundCategory_master:0.0\n\
             guiScale:2\nfovEffectScale:0.0\nfullscreen:false\nbobView:false\nrenderClouds:\"false\"\nparticles:{particles}\nentityShadows:true\n"
            ),
        )?;
        let natives = game_dir.join("natives");
        std::fs::create_dir_all(&natives)?;
        let log = game_dir.join("client.log");
        let mut cp: Vec<String> = cp_first.iter().map(|p| p.display().to_string()).collect();
        cp.push(build.classpath());
        let child = Command::new(&build.install.java)
            .arg("-Xmx3g")
            .arg("--enable-native-access=ALL-UNNAMED")
            .arg("--sun-misc-unsafe-memory-access=allow")
            .arg(format!("-Djava.library.path={}", natives.display()))
            .arg(format!("-Dorg.lwjgl.system.SharedLibraryExtractPath={}", natives.display()))
            .args(jvm)
            .arg("-cp")
            .arg(cp.join(":"))
            .arg(main)
            .args(["--username", "mctas", "--version", &build.install.version])
            .arg("--gameDir")
            .arg(game_dir)
            .arg("--assetsDir")
            .arg(&build.install.assets_dir)
            .args(["--assetIndex", &build.install.asset_index])
            .args(["--uuid", "8d2b3e4f-0000-4000-8000-00000000c0de", "--accessToken", "0", "--versionType", "release"])
            .args(["--width", &width.to_string(), "--height", &height.to_string()])
            .args(match port {
                Some(port) => vec!["--quickPlayMultiplayer".to_string(), format!("127.0.0.1:{port}")],
                None => Vec::new(),
            })
            .env("DISPLAY", display.name())
            // Software Vulkan (llvmpipe) would start one thread per CPU; several clients share a box.
            .env("LP_NUM_THREADS", std::env::var("LP_NUM_THREADS").unwrap_or_else(|_| "12".into()))
            .current_dir(game_dir)
            .stdout(std::fs::File::create(&log)?)
            .stderr(Stdio::from(std::fs::File::options().append(true).open(&log)?))
            .spawn()
            .context("starting the Minecraft client")?;
        Ok(Client { child, log })
    }

    pub fn alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// ffmpeg grabbing the display into a lossless intermediate file.
pub struct Recorder {
    child: Child,
    pub raw: PathBuf,
}

impl Recorder {
    pub fn start(display: &Display, width: u32, height: u32, fps: u32, raw: &Path) -> Result<Recorder> {
        Recorder::start_crf(display, width, height, fps, raw, None)
    }

    /// [`Recorder::start`] at x264 `crf` instead of lossless qp 0 (long recordings: the bedrock
    /// tutorial's half hour of mining would be tens of gigabytes lossless).
    pub fn start_crf(display: &Display, width: u32, height: u32, fps: u32, raw: &Path, crf: Option<u32>) -> Result<Recorder> {
        let mut cmd = Command::new(ffmpeg()?);
        cmd.args(["-loglevel", "error", "-y", "-f", "x11grab", "-draw_mouse", "0"])
            .args(["-framerate", &fps.to_string(), "-video_size", &format!("{width}x{height}")])
            .args(["-i", &display.name()])
            .args(["-c:v", "libx264", "-preset", "ultrafast"]);
        match crf {
            Some(c) => cmd.args(["-crf", &c.to_string(), "-pix_fmt", "yuv420p"]),
            None => cmd.args(["-qp", "0"]),
        };
        let child = cmd.arg(raw).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().context("starting ffmpeg")?;
        Ok(Recorder { child, raw: raw.to_path_buf() })
    }

    /// Ask ffmpeg to finish the file ('q' on stdin) and wait for it.
    pub fn stop(mut self) -> Result<PathBuf> {
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q");
        }
        let _ = self.child.wait();
        Ok(self.raw.clone())
    }
}

/// Encode a browser-playable H.264 MP4 (optionally sped up by `speed`).
pub fn encode(raw: &Path, out: &Path, speed: f64) -> Result<()> {
    encode_crf(raw, out, speed, 20)
}

/// [`encode`] at x264 quality `crf` (lower = better: 16 for clips that are cut again).
pub fn encode_crf(raw: &Path, out: &Path, speed: f64, crf: u32) -> Result<()> {
    let mut cmd = Command::new(ffmpeg()?);
    cmd.args(["-loglevel", "error", "-y", "-i"]).arg(raw);
    if (speed - 1.0).abs() > 1e-6 {
        cmd.args(["-filter:v", &format!("setpts=PTS/{speed}")]);
    }
    let status = cmd
        .args(["-c:v", "libx264", "-preset", "slow", "-crf", &crf.to_string(), "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(out)
        .status()?;
    if !status.success() {
        bail!("ffmpeg encode failed");
    }
    Ok(())
}

/// One frame of the display as PNG (for checks and thumbnails).
pub fn screenshot(display: &Display, width: u32, height: u32, out: &Path) -> Result<()> {
    let status = Command::new(ffmpeg()?)
        .args(["-loglevel", "error", "-y", "-f", "x11grab", "-video_size", &format!("{width}x{height}")])
        .args(["-i", &display.name(), "-frames:v", "1"])
        .arg(out)
        .status()?;
    if !status.success() {
        bail!("screenshot failed");
    }
    Ok(())
}

/// Yaw/pitch (Minecraft degrees) to look from `eye` at `target`.
pub fn look_at(eye: [f64; 3], target: [f64; 3]) -> (f32, f32) {
    let (dx, dy, dz) = (target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]);
    let yaw = (-dx.atan2(dz)).to_degrees();
    let pitch = (-dy.atan2((dx * dx + dz * dz).sqrt())).to_degrees();
    (yaw as f32, pitch as f32)
}

/// Upload (or replace) a file as an asset of a GitHub release, creating the release if needed.
/// Uses the `gh` CLI. Returns the asset's download URL.
pub fn upload_release_asset(repo: &str, tag: &str, file: &Path) -> Result<String> {
    let view = Command::new("gh").args(["release", "view", tag, "--repo", repo]).output()?;
    if !view.status.success() {
        let status = Command::new("gh")
            .args(["release", "create", tag, "--repo", repo, "--title", tag, "--notes", "Videos recorded with mctas (real Minecraft client)."])
            .status()?;
        if !status.success() {
            bail!("gh release create failed");
        }
    }
    let status = Command::new("gh").args(["release", "upload", tag, "--repo", repo, "--clobber"]).arg(file).status()?;
    if !status.success() {
        bail!("gh release upload failed");
    }
    let name = file.file_name().unwrap().to_string_lossy();
    Ok(format!("https://github.com/{repo}/releases/download/{tag}/{name}"))
}

/// Camera pose (eye position, yaw, pitch) that frames an axis-aligned box: the camera looks at
/// the box center from direction `dir` (from the center toward the eye) and backs off until
/// the projected box fills `fill` of the frame. `vfov_deg` is the vertical field of view.
pub fn frame_box(lo: [f64; 3], hi: [f64; 3], dir: [f64; 3], vfov_deg: f64, aspect: f64, fill: f64) -> ([f64; 3], f32, f32) {
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let d = [dir[0] / n, dir[1] / n, dir[2] / n];
    let tan_v = (vfov_deg.to_radians() / 2.0).tan();
    let tan_h = tan_v * aspect;
    let corners: Vec<[f64; 3]> = (0..8)
        .map(|i| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }])
        .collect();
    // Camera basis looking along -d.
    let f = [-d[0], -d[1], -d[2]];
    let up0 = [0.0, 1.0, 0.0];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |a: [f64; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let right = norm(cross(f, up0));
    let up = cross(right, f);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let extent = |dist: f64| -> f64 {
        let eye = [center[0] + d[0] * dist, center[1] + d[1] * dist, center[2] + d[2] * dist];
        let mut m: f64 = 0.0;
        for c in &corners {
            let v = [c[0] - eye[0], c[1] - eye[1], c[2] - eye[2]];
            let z = dot(v, f);
            if z <= 0.1 {
                return f64::INFINITY;
            }
            m = m.max((dot(v, right) / z).abs() / tan_h).max((dot(v, up) / z).abs() / tan_v);
        }
        m
    };
    // extent() decreases with distance: bisect for extent == fill.
    let (mut a, mut b) = (0.5, 1000.0);
    for _ in 0..80 {
        let mid = (a + b) / 2.0;
        if extent(mid) > fill { a = mid } else { b = mid }
    }
    let eye = [center[0] + d[0] * b, center[1] + d[1] * b, center[2] + d[2] * b];
    let (yaw, pitch) = look_at(eye, center);
    (eye, yaw, pitch)
}

#[cfg(test)]
mod tests {
    #[test]
    fn frame_box_fills() {
        let (eye, yaw, pitch) = super::frame_box([-4.0, -4.0, -1.0], [14.0, 9.0, 9.0], [-0.55, 0.45, -1.0], 70.0, 16.0 / 9.0, 0.9);
        println!("eye {eye:?} yaw {yaw} pitch {pitch}");
        let c = [5.0, 2.5, 4.0];
        let dist = ((eye[0] - c[0]).powi(2) + (eye[1] - c[1]).powi(2) + (eye[2] - c[2]).powi(2)).sqrt();
        println!("distance {dist}");
        assert!(dist < 40.0);
    }
}

/// Camera pose framing a set of points (e.g. every block corner of a build) as tightly as
/// possible: viewing direction `dir` (from the scene toward the eye), the points' screen
/// bounding box centered and filling `fill` of the frame in its tighter dimension.
pub fn frame_points(points: &[[f64; 3]], dir: [f64; 3], vfov_deg: f64, aspect: f64, fill: f64) -> ([f64; 3], f32, f32) {
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let d = [dir[0] / n, dir[1] / n, dir[2] / n];
    let f = [-d[0], -d[1], -d[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let r0 = cross(f, [0.0, 1.0, 0.0]);
    let rl = dot(r0, r0).sqrt();
    let right = [r0[0] / rl, r0[1] / rl, r0[2] / rl];
    let up = cross(right, f);
    let tan_v = (vfov_deg.to_radians() / 2.0).tan();
    let tan_h = tan_v * aspect;
    let count = points.len().max(1) as f64;
    let mut target = [0.0; 3];
    for p in points {
        for i in 0..3 {
            target[i] += p[i] / count;
        }
    }
    // Screen-space bounds (normalized: 1.0 = frame edge) seen from distance `dist`.
    let bounds = |target: [f64; 3], dist: f64| -> Option<(f64, f64, f64, f64)> {
        let eye = [target[0] + d[0] * dist, target[1] + d[1] * dist, target[2] + d[2] * dist];
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in points {
            let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
            let z = dot(v, f);
            if z <= 0.2 {
                return None;
            }
            let (x, y) = (dot(v, right) / z / tan_h, dot(v, up) / z / tan_v);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        Some((x0, x1, y0, y1))
    };
    let mut dist = 20.0;
    for _ in 0..6 {
        // Distance so the larger half-extent around the bounds' center is `fill`.
        let (mut a, mut b) = (0.5, 2000.0);
        for _ in 0..60 {
            let mid = (a + b) / 2.0;
            match bounds(target, mid) {
                Some((x0, x1, y0, y1)) if ((x1 - x0) / 2.0).max((y1 - y0) / 2.0) <= fill => b = mid,
                _ => a = mid,
            }
        }
        dist = b;
        // Re-center: shift the target by the screen offset of the bounds' center.
        if let Some((x0, x1, y0, y1)) = bounds(target, dist) {
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            for i in 0..3 {
                target[i] += right[i] * cx * dist * tan_h + up[i] * cy * dist * tan_v;
            }
        }
    }
    let eye = [target[0] + d[0] * dist, target[1] + d[1] * dist, target[2] + d[2] * dist];
    let (yaw, pitch) = look_at(eye, target);
    (eye, yaw, pitch)
}
