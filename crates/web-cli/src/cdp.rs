//! `mctas web click`: drives headless Chromium through the DevTools protocol over a pipe
//! (`--remote-debugging-pipe`: commands on fd 3, replies on fd 4, NUL-separated JSON) to click or
//! tap a page's live viewers like a person, and reads what the viewer reports in debug mode
//! (`?mcview-debug`: `data-mcview-state` on each viewer: tick, playing, a door's blocks, each
//! input's click point and state).

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

struct Browser {
    child: std::process::Child,
    to: std::fs::File,
    from: Receiver<Value>,
    next: u64,
    session: Option<String>,
    /// The page's console lines and exceptions seen so far.
    console: Vec<String>,
}

impl Browser {
    fn start(browser: &Path, window: &str, scale: f32, url: &str) -> Result<Browser> {
        let mut a = [0i32; 2];
        let mut b = [0i32; 2];
        // SAFETY: plain pipe(2) calls on local arrays.
        if unsafe { libc::pipe(a.as_mut_ptr()) } != 0 || unsafe { libc::pipe(b.as_mut_ptr()) } != 0 {
            bail!("pipe: {}", std::io::Error::last_os_error());
        }
        let (child_in, child_out) = (a[0], b[1]);
        let mut cmd = std::process::Command::new(browser);
        cmd.args(["--no-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--remote-debugging-pipe"])
            .arg(format!("--window-size={window}"))
            .arg(format!("--force-device-scale-factor={scale}"));
        // the page may fetch the game's textures from GitHub: through the box's proxy
        if let Some(p) = std::env::var_os("https_proxy").or_else(|| std::env::var_os("HTTPS_PROXY")) {
            cmd.arg(format!("--proxy-server={}", p.to_string_lossy())).arg("--proxy-bypass-list=127.0.0.1;localhost");
        }
        cmd.arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        // SAFETY: only async-signal-safe dup2 calls between fork and exec.
        unsafe {
            cmd.pre_exec(move || {
                if libc::dup2(child_in, 3) < 0 || libc::dup2(child_out, 4) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().with_context(|| format!("starting {}", browser.display()))?;
        // SAFETY: the child has its copies; these fds are ours to close or own.
        unsafe {
            libc::close(child_in);
            libc::close(child_out);
        }
        let to = unsafe { std::fs::File::from_raw_fd(a[1]) };
        let mut from_pipe = unsafe { std::fs::File::from_raw_fd(b[0]) };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 65536];
            while let Ok(n) = from_pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                while let Some(i) = buf.iter().position(|c| *c == 0) {
                    let msg: Vec<u8> = buf.drain(..=i).collect();
                    if let Ok(v) = serde_json::from_slice::<Value>(&msg[..msg.len() - 1])
                        && tx.send(v).is_err()
                    {
                        return;
                    }
                }
            }
        });
        Ok(Browser { child, to, from: rx, next: 1, session: None, console: Vec::new() })
    }

    /// Send a command (to the attached page when there is one) and wait for its reply.
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next;
        self.next += 1;
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = &self.session
            && !method.starts_with("Target.")
        {
            msg["sessionId"] = json!(s);
        }
        let mut bytes = serde_json::to_vec(&msg)?;
        bytes.push(0);
        self.to.write_all(&bytes)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let v = self.from.recv_timeout(left).with_context(|| format!("no reply to {method}"))?;
            if v["id"].as_u64() == Some(id) {
                if let Some(e) = v.get("error") {
                    bail!("{method}: {e}");
                }
                return Ok(v["result"].clone());
            }
            self.event(&v);
        }
    }

    /// Keep the page's console lines and exceptions (events met while waiting for replies).
    fn event(&mut self, v: &Value) {
        match v["method"].as_str() {
            Some("Runtime.consoleAPICalled") => {
                let args: Vec<String> = v["params"]["args"].as_array().map(|a| a.iter().map(|x| x["value"].as_str().map(str::to_string).unwrap_or_else(|| x["description"].as_str().unwrap_or("?").to_string())).collect()).unwrap_or_default();
                self.console.push(format!("{}: {}", v["params"]["type"].as_str().unwrap_or("log"), args.join(" ")));
            }
            Some("Runtime.exceptionThrown") => {
                self.console.push(format!("exception: {}", v["params"]["exceptionDetails"]["exception"]["description"].as_str().unwrap_or("?")));
            }
            _ => {}
        }
    }

    /// Take the events that arrived (console lines), waiting up to `wait`.
    fn drain(&mut self, wait: Duration) {
        let deadline = Instant::now() + wait;
        while let Ok(v) = self.from.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            self.event(&v);
        }
    }

    /// Attach to the page tab (the first `page` target).
    fn attach_page(&mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let t = self.call("Target.getTargets", json!({}))?;
            if let Some(page) = t["targetInfos"].as_array().and_then(|a| a.iter().find(|i| i["type"] == "page")) {
                let id = page["targetId"].as_str().unwrap_or_default().to_string();
                let r = self.call("Target.attachToTarget", json!({ "targetId": id, "flatten": true }))?;
                self.session = r["sessionId"].as_str().map(str::to_string);
                return Ok(());
            }
            if Instant::now() > deadline {
                bail!("no page target");
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn eval(&mut self, expr: &str) -> Result<Value> {
        let r = self.call("Runtime.evaluate", json!({ "expression": expr, "returnByValue": true }))?;
        Ok(r["result"]["value"].clone())
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A viewer's `data-mcview-state`: `tick=T playing=0|1 door=N|- inputs=x,y,on|...`.
#[derive(Debug, Clone, PartialEq)]
struct State {
    tick: u32,
    playing: bool,
    door: Option<u32>,
    inputs: Vec<(Option<(f64, f64)>, bool)>,
    active: Option<u32>,
    /// The camera: yaw, pitch, distance, target x, y, z.
    cam: Vec<f64>,
}

fn parse_state(s: &str) -> Option<State> {
    if !s.contains("tick=") {
        return None;
    }
    let mut st = State { tick: 0, playing: false, door: None, inputs: Vec::new(), active: None, cam: Vec::new() };
    for w in s.split_whitespace() {
        let (k, v) = w.split_once('=')?;
        match k {
            "tick" => st.tick = v.parse().ok()?,
            "playing" => st.playing = v == "1",
            "door" => st.door = v.parse().ok(),
            "active" => st.active = v.parse().ok(),
            "cam" => st.cam = v.split(',').filter_map(|x| x.parse().ok()).collect(),
            "inputs" => {
                for i in v.split('|').filter(|i| !i.is_empty()) {
                    let p: Vec<&str> = i.split(',').collect();
                    if p.len() != 3 {
                        continue;
                    }
                    let xy = match (p[0].parse::<f64>(), p[1].parse::<f64>()) {
                        (Ok(x), Ok(y)) => Some((x, y)),
                        _ => None,
                    };
                    st.inputs.push((xy, p[2] == "1"));
                }
            }
            _ => {}
        }
    }
    Some(st)
}

fn states(b: &mut Browser) -> Result<Vec<Option<State>>> {
    let v = b.eval("Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb').map(e => e.getAttribute('data-mcview-state') || '')")?;
    Ok(v.as_array().map(|a| a.iter().map(|s| s.as_str().and_then(parse_state)).collect()).unwrap_or_default())
}

/// Every live viewer's first reported state: a viewer only draws (and reports) on screen, so
/// each one not reported yet is brought on screen in turn; None when they never all report
/// within 60 s (the count seen then).
fn all_states(b: &mut Browser) -> Result<std::result::Result<Vec<State>, usize>> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let s = states(b)?;
        if !s.is_empty() && s.iter().all(|x| x.is_some()) {
            return Ok(Ok(s.into_iter().map(|x| x.unwrap()).collect()));
        }
        if Instant::now() > deadline {
            return Ok(Err(s.len()));
        }
        if let Some(i) = s.iter().position(|x| x.is_none()) {
            let _ = b.eval(&format!("(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[{i}]; if (v) v.scrollIntoView({{block: 'center'}}); return true; }})()"));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// A screenshot of a page after `seconds` of real time (animation frames run: playing viewers
/// move, demos advance), scrolled to `scroll_y` CSS px.
#[allow(clippy::too_many_arguments)]
pub fn cmd_snap(url: &str, window: &str, scale: f32, seconds: f64, scroll_y: f64, eval: Option<&str>, before: Option<(&str, f64)>, out: &Path, browser: &Path) -> Result<()> {
    let mut b = Browser::start(browser, window, scale, url)?;
    b.attach_page()?;
    b.call("Page.enable", json!({}))?;
    b.call("Runtime.enable", json!({}))?;
    std::thread::sleep(Duration::from_millis(500));
    if scroll_y > 0.0 {
        b.eval(&format!("window.scrollTo(0, {scroll_y})"))?;
    }
    if let Some((js, at)) = before {
        b.drain(Duration::from_secs_f64(at));
        println!("before {js} = {}", b.eval(js)?);
    }
    b.drain(Duration::from_secs_f64(seconds));
    let r = b.call("Page.captureScreenshot", json!({ "format": "png" }))?;
    let data = r["data"].as_str().context("no screenshot data")?;
    std::fs::write(out, base64_decode(data)?)?;
    for l in &b.console {
        println!("console {l}");
    }
    if let Some(js) = eval {
        println!("eval {js} = {}", b.eval(js)?);
    }
    println!("{} ({seconds} s of {url})", out.display());
    Ok(())
}

/// `mctas web record`: press the Record button of a page's live viewer `index` and keep the video
/// it downloads (`<design>.mp4|.webm`) in `out`; prints the file and the page's console.
pub fn cmd_record(url: &str, out: &Path, index: usize, window: &str, scale: f32, timeout: u64, browser: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let dir = std::fs::canonicalize(out)?;
    let before: Vec<std::ffi::OsString> = std::fs::read_dir(&dir)?.flatten().map(|e| e.file_name()).collect();
    let mut b = Browser::start(browser, window, scale, url)?;
    // downloads land in `out` (a browser-wide setting: sent before attaching to the page)
    b.call("Browser.setDownloadBehavior", json!({ "behavior": "allow", "downloadPath": dir.to_string_lossy(), "eventsEnabled": true }))?;
    b.attach_page()?;
    b.call("Page.enable", json!({}))?;
    b.call("Runtime.enable", json!({}))?;
    let t0 = Instant::now();
    loop {
        let n = b.eval("document.querySelectorAll('.mcview-live-src').length")?.as_u64().unwrap_or(0) as usize;
        if n > index {
            break;
        }
        if t0.elapsed() > Duration::from_secs(90) {
            bail!("no live viewer {index} on {url} after 90 s; console: {:?}", b.console);
        }
        b.drain(Duration::from_millis(500));
    }
    b.drain(Duration::from_secs(1));
    let pressed = b.eval(&format!("(() => {{ const r = document.querySelectorAll('[data-mcview-record]')[{index}]; if (!r) return 'none'; if (getComputedStyle(r).display === 'none') return 'hidden'; r.click(); return 'pressed'; }})()"))?;
    match pressed.as_str() {
        Some("pressed") => {}
        Some("hidden") => bail!("the Record button is hidden: this browser cannot encode video (no VideoEncoder)"),
        other => bail!("no Record button {index}: {other:?}"),
    }
    let start = Instant::now();
    let mut last_note = Instant::now();
    loop {
        b.drain(Duration::from_millis(500));
        let done: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)?
            .flatten()
            .filter(|e| !before.contains(&e.file_name()))
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "mp4" || x == "webm"))
            .collect();
        if let Some(p) = done.first() {
            // complete once its size holds (the browser writes it in one go from the blob)
            let n0 = std::fs::metadata(p)?.len();
            b.drain(Duration::from_millis(800));
            if n0 > 0 && std::fs::metadata(p)?.len() == n0 {
                for l in b.console.iter().filter(|l| l.contains("record") || l.starts_with("exception") || l.starts_with("error")) {
                    println!("console {l}");
                }
                println!("{} ({} KB, {:.1} s after the press)", p.display(), n0 / 1024, start.elapsed().as_secs_f64());
                return Ok(());
            }
        }
        if last_note.elapsed() > Duration::from_secs(10) {
            last_note = Instant::now();
            println!("recording: {}", b.eval(&format!("(document.querySelectorAll('[data-mcview-record]')[{index}] || {{}}).textContent"))?);
        }
        let failed: Vec<&String> = b.console.iter().filter(|l| l.contains("record:") && !l.contains("record: saved")).collect();
        if !failed.is_empty() {
            bail!("the recording failed: {failed:?}");
        }
        if start.elapsed() > Duration::from_secs(timeout) {
            bail!("no video after {timeout} s; console: {:?}", b.console.iter().filter(|l| l.contains("record") || l.starts_with("exception")).collect::<Vec<_>>());
        }
    }
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let bytes: Vec<u8> = s.bytes().filter(|c| *c != b'=' && !c.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c).context("bad base64")? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// The mean luma (0-255) of an RGBA image and the share of its pixels darker than 16.
pub fn luma_stats(rgba: &[u8]) -> (f64, f64) {
    let n = (rgba.len() / 4).max(1) as f64;
    let (mut sum, mut dark) = (0f64, 0f64);
    for p in rgba.chunks(4) {
        let l = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        sum += l;
        if l < 16.0 {
            dark += 1.0;
        }
    }
    (sum / n, dark / n)
}

/// Every viewer of a page (thumbnails and live viewers) in headless Chromium through the real
/// bundle: each scrolled into view once drawn, its on-screen pixels measured; fails when one is
/// mostly black (mean luma under `min` or over half its pixels near black): the WebGL failures
/// the native renderer cannot see.
pub fn cmd_luma(url: &str, window: &str, min: f64, browser: &Path) -> Result<()> {
    let mut b = Browser::start(browser, window, 1.0, url)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    b.call("Page.enable", json!({}))?;
    let count = |b: &mut Browser, sel: &str| -> Result<u64> { Ok(b.eval(&format!("document.querySelectorAll('{sel}').length"))?.as_u64().unwrap_or(0)) };
    // scroll through the page until every viewer has drawn (thumbnails draw near the viewport)
    let deadline = Instant::now() + Duration::from_secs(240);
    let total = loop {
        let total = count(&mut b, "[data-mcview]")?;
        if total > 0 {
            break total;
        }
        if Instant::now() > deadline {
            bail!("no viewer on the page");
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    loop {
        let height = b.eval("document.documentElement.scrollHeight")?.as_f64().unwrap_or(0.0);
        let view = b.eval("window.innerHeight")?.as_f64().unwrap_or(800.0).max(200.0);
        let mut y = 0.0;
        while y < height {
            b.eval(&format!("window.scrollTo(0, {y})"))?;
            b.drain(Duration::from_millis(120));
            y += view * 0.8;
        }
        let done = count(&mut b, "[data-mcview].mcview-ready, [data-mcview].mcview-error")?;
        if done >= total {
            break;
        }
        if Instant::now() > deadline {
            println!("web luma: only {done} of {total} viewers drew within 240 s");
            break;
        }
    }
    let (mut failed, mut worst) = (Vec::new(), (f64::MAX, String::new()));
    for i in 0..total {
        let r = b.eval(&format!(
            "(() => {{ const v = document.querySelectorAll('[data-mcview]')[{i}]; v.scrollIntoView({{block: 'center'}}); const r = v.getBoundingClientRect(); return [r.left + window.scrollX, r.top + window.scrollY, r.width, r.height, v.getAttribute('data-src') || '', v.className]; }})()"
        ))?;
        let a = r.as_array().cloned().unwrap_or_default();
        let num = |k: usize| a.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let (x, y, w, h) = (num(0), num(1), num(2), num(3));
        let src = a.get(4).and_then(Value::as_str).unwrap_or("").to_string();
        let class = a.get(5).and_then(Value::as_str).unwrap_or("").to_string();
        if w < 8.0 || h < 8.0 {
            continue;
        }
        b.drain(Duration::from_millis(80));
        let shot = b.call("Page.captureScreenshot", json!({ "format": "png", "clip": { "x": x + 2.0, "y": y + 2.0, "width": w - 4.0, "height": h - 4.0, "scale": 0.5 } }))?;
        let png = base64_decode(shot["data"].as_str().unwrap_or(""))?;
        let (rgba, _, _) = mcview::tools::decode_png(&png).map_err(anyhow::Error::msg)?;
        let (mean, dark) = luma_stats(&rgba);
        if mean < worst.0 {
            worst = (mean, src.clone());
        }
        if mean < min || dark > 0.5 || class.contains("mcview-error") {
            if failed.len() < 3 {
                // the first dark ones, to look at
                let _ = std::fs::write(format!("/tmp/mctas-luma-dark-{}.png", failed.len()), &png);
            }
            failed.push(format!("{src}: mean luma {mean:.0}, {:.0}% near black{}", dark * 100.0, if class.contains("mcview-error") { ", mcview-error" } else { "" }));
        }
    }
    for f in &failed {
        println!("DARK {f}");
    }
    for l in b.console.iter().filter(|l| l.starts_with("error") || l.starts_with("exception") || l.contains("GPU error") || l.contains("panic")) {
        println!("console {l}");
    }
    println!("web luma: {total} viewers, {} dark (darkest {:.0}: {})", failed.len(), worst.0, worst.1);
    if !failed.is_empty() {
        bail!("{} viewers are mostly black", failed.len());
    }
    Ok(())
}

/// Press and hold each live viewer's first circuit input in the 3D view (mouse or touch): the
/// input must turn on while held (after the hold delay), stay on, turn off once released, and
/// the camera must not move at any point.
pub fn cmd_hold(url: &str, window: &str, scale: f32, touch: bool, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut b = Browser::start(browser, window, scale, &page)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    if touch {
        b.call("Emulation.setTouchEmulationEnabled", json!({ "enabled": true, "maxTouchPoints": 5 }))?;
    }
    let initial = match all_states(&mut b)? {
        Ok(s) => s,
        Err(_) => bail!("the live viewers never reported (served? ?mcview-debug?)"),
    };
    let (mut tested, mut failed) = (0, 0);
    for (i, st) in initial.iter().enumerate() {
        // circuits only: a door's lever flips on a tap, it has no hold
        if st.door.is_some() {
            continue;
        }
        let Some(((fx, fy), _)) = st.inputs.iter().find_map(|(xy, on)| xy.map(|p| (p, *on))) else { continue };
        let rect = b.eval(&format!(
            "(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[{i}]; v.scrollIntoView({{block: 'center'}}); const r = v.querySelector('canvas').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; }})()"
        ))?;
        let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        if r.len() != 4 {
            continue;
        }
        std::thread::sleep(Duration::from_millis(300));
        let Some(Some(before)) = states(&mut b)?.get(i).cloned() else { continue };
        let Some(((fx, fy), on0)) = before.inputs.iter().find_map(|(xy, on)| xy.map(|p| (p, *on))).or(Some(((fx, fy), false))) else { continue };
        let (x, y) = (r[0] + fx * r[2], r[1] + fy * r[3]);
        tested += 1;
        // press, with a little jitter (a finger), and hold
        if touch {
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchStart", "touchPoints": [{ "x": x, "y": y }] }))?;
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchMove", "touchPoints": [{ "x": x + 2.0, "y": y + 1.0 }] }))?;
        } else {
            b.call("Input.dispatchMouseEvent", json!({ "type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1 }))?;
            b.call("Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x + 2.0, "y": y + 1.0, "button": "left" }))?;
        }
        std::thread::sleep(Duration::from_millis(700));
        let held = states(&mut b)?.get(i).cloned().flatten();
        std::thread::sleep(Duration::from_millis(500));
        let still = states(&mut b)?.get(i).cloned().flatten();
        if touch {
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchEnd", "touchPoints": [] }))?;
        } else {
            b.call("Input.dispatchMouseEvent", json!({ "type": "mouseReleased", "x": x + 2.0, "y": y + 1.0, "button": "left", "clickCount": 1 }))?;
        }
        std::thread::sleep(Duration::from_millis(600));
        let after = states(&mut b)?.get(i).cloned().flatten();
        let input_on = |s: &Option<State>| s.as_ref().and_then(|s| s.inputs.iter().find_map(|(xy, on)| xy.map(|_| *on)));
        let same_cam = |s: &Option<State>| s.as_ref().is_some_and(|s| s.cam.len() == 6 && s.cam.iter().zip(&before.cam).all(|(a, b)| (a - b).abs() < 1e-3));
        let ok = !on0 && input_on(&held) == Some(true) && input_on(&still) == Some(true) && input_on(&after) == Some(false) && same_cam(&held) && same_cam(&still) && same_cam(&after);
        let how = if touch { "touch" } else { "mouse" };
        if ok {
            println!("viewer {i}: {how} hold at ({x:.0}, {y:.0}): input on while held, off after release, camera unchanged: PASS");
        } else {
            failed += 1;
            println!(
                "viewer {i}: {how} hold at ({x:.0}, {y:.0}): FAIL (input before {on0}, held {:?}, still {:?}, after {:?}; camera before {:?}, after {:?})",
                input_on(&held),
                input_on(&still),
                input_on(&after),
                before.cam,
                after.as_ref().map(|s| s.cam.clone())
            );
        }
    }
    println!("web click --hold: {} of {tested} viewers hold their input while pressed ({window})", tested - failed);
    if tested == 0 {
        bail!("no live viewer with an input");
    }
    if failed > 0 {
        bail!("{failed} viewers failed the hold");
    }
    Ok(())
}

/// The site's in-place swap (`data-src` changed on a live viewer, `data-keep-camera` set): the
/// person orbits, the design is swapped, and the new design must show (not black, the viewer
/// marked `mcview-live-src`) under the very same camera.
pub fn cmd_swap(url: &str, to: &str, window: &str, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut b = Browser::start(browser, window, 1.0, &page)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let st0 = loop {
        if let Some(Some(s)) = states(&mut b)?.first().cloned() {
            break s;
        }
        if Instant::now() > deadline {
            bail!("no live viewer reported");
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let rect = b.eval("(() => { const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0]; v.scrollIntoView({block: 'center'}); const r = v.querySelector('canvas').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; })()")?;
    let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if r.len() != 4 {
        bail!("no canvas");
    }
    // the person orbits away from the default camera (a drag on the sky corner, not an input)
    let (x, y) = (r[0] + r[2] * 0.1, r[1] + r[3] * 0.1);
    b.call("Input.dispatchMouseEvent", json!({ "type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1 }))?;
    for k in 1..=6 {
        b.call("Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x + 15.0 * k as f64, "y": y + 4.0 * k as f64, "button": "left" }))?;
    }
    b.call("Input.dispatchMouseEvent", json!({ "type": "mouseReleased", "x": x + 90.0, "y": y + 24.0, "button": "left", "clickCount": 1 }))?;
    std::thread::sleep(Duration::from_millis(400));
    let st1 = states(&mut b)?.first().cloned().flatten().context("state after the orbit")?;
    if st1.cam == st0.cam {
        bail!("the drag did not orbit the camera ({:?})", st1.cam);
    }
    let marked = b.eval("document.querySelectorAll('.mcview-live-src').length")?.as_u64().unwrap_or(0);
    b.eval(&format!(
        "(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0]; v.setAttribute('data-keep-camera', '1'); v.setAttribute('data-src', {}); return 1; }})()",
        serde_json::to_string(to)?
    ))?;
    std::thread::sleep(Duration::from_millis(2500));
    let st2 = states(&mut b)?.first().cloned().flatten().context("state after the swap")?;
    let same_cam = st2.cam.len() == 6 && st2.cam.iter().zip(&st1.cam).all(|(a, b)| (a - b).abs() < 1e-3);
    // the person's orientation always stays; the distance and target too unless the new build
    // does not fit that camera (then they are fitted again: never cut)
    let same_dir = st2.cam.len() == 6 && st1.cam.len() == 6 && (st2.cam[0] - st1.cam[0]).abs() < 1e-3 && (st2.cam[1] - st1.cam[1]).abs() < 1e-3;
    // the viewer's pixels: the new design drawn, not black
    let shot = b.call("Page.captureScreenshot", json!({ "format": "png", "clip": { "x": r[0] + 2.0, "y": r[1] + 2.0, "width": r[2] - 4.0, "height": r[3] - 4.0, "scale": 0.5 } }))?;
    let (rgba, _, _) = mcview::tools::decode_png(&base64_decode(shot["data"].as_str().unwrap_or(""))?).map_err(anyhow::Error::msg)?;
    let (luma, dark) = luma_stats(&rgba);
    let changed = st2.inputs.len() != st1.inputs.len() || st2.tick < st1.tick.max(1) || st2.active != st1.active;
    println!(
        "swap to {to}: marked mcview-live-src {}, camera {} ({:?} -> {:?}), design changed {changed}, luma {luma:.0} ({:.0}% near black)",
        marked > 0,
        if same_cam {
            "kept"
        } else if same_dir {
            "kept, re-fitted to the bigger build"
        } else {
            "TURNED"
        },
        st1.cam,
        st2.cam,
        dark * 100.0
    );
    if marked == 0 || !same_dir || !changed || luma < 40.0 || dark > 0.5 {
        bail!("the in-place swap failed");
    }
    println!("web swap: PASS");
    Ok(())
}

/// A live viewer's WebGL context lost and restored (WEBGL_lose_context, what iOS does to the
/// oldest contexts): it must draw again (not black) once the canvas is restored.
pub fn cmd_lose(url: &str, window: &str, browser: &Path) -> Result<()> {
    let mut b = Browser::start(browser, window, 1.0, url)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if b.eval("document.querySelectorAll('[data-mcview].mcview-ready').length")?.as_u64().unwrap_or(0) > 0 {
            break;
        }
        if Instant::now() > deadline {
            bail!("no viewer drew");
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    std::thread::sleep(Duration::from_millis(800));
    let rect = b.eval("(() => { const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0]; v.scrollIntoView({block: 'center'}); const r = v.getBoundingClientRect(); return [r.left + window.scrollX, r.top + window.scrollY, r.width, r.height]; })()")?;
    let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if r.len() != 4 {
        bail!("no live viewer");
    }
    let measure = |b: &mut Browser| -> Result<f64> {
        b.drain(Duration::from_millis(300));
        let shot = b.call("Page.captureScreenshot", json!({ "format": "png", "clip": { "x": r[0] + 2.0, "y": r[1] + 2.0, "width": r[2] - 4.0, "height": r[3] - 4.0, "scale": 0.5 } }))?;
        let (rgba, _, _) = mcview::tools::decode_png(&base64_decode(shot["data"].as_str().unwrap_or(""))?).map_err(anyhow::Error::msg)?;
        Ok(luma_stats(&rgba).0)
    };
    let before = measure(&mut b)?;
    b.eval("(() => { const c = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0].querySelector('canvas'); window.__lose = c.getContext('webgl2').getExtension('WEBGL_lose_context'); window.__lose.loseContext(); return 1; })()")?;
    std::thread::sleep(Duration::from_millis(500));
    b.eval("window.__lose.restoreContext(), 1")?;
    std::thread::sleep(Duration::from_millis(1500));
    // a nudge (a wheel) so a paused viewer draws a frame
    let sy = b.eval("window.scrollY")?.as_f64().unwrap_or(0.0);
    b.call("Input.dispatchMouseEvent", json!({ "type": "mouseWheel", "x": r[0] + r[2] / 2.0, "y": r[1] + r[3] / 2.0 - sy, "deltaX": 0, "deltaY": 1 }))?;
    let after = measure(&mut b)?;
    for l in b.console.iter().filter(|l| l.contains("lost") || l.contains("error") || l.contains("panic")) {
        println!("console {l}");
    }
    println!("web lose: luma {before:.0} before the context loss, {after:.0} after it came back");
    if before < 40.0 || after < 40.0 {
        bail!("the viewer did not draw again after its context came back");
    }
    println!("web lose: PASS");
    Ok(())
}

pub fn cmd_click(url: &str, window: &str, scale: f32, touch: bool, wait: u64, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut b = Browser::start(browser, window, scale, &page)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    if touch {
        b.call("Emulation.setTouchEmulationEnabled", json!({ "enabled": true, "maxTouchPoints": 5 }))?;
    }
    // wait for every live viewer to report its state
    let initial = match all_states(&mut b)? {
        Ok(s) => s,
        Err(n) => bail!("the live viewers never reported (served? ?mcview-debug? {n} viewers)"),
    };
    let (mut tested, mut failed) = (0, 0);
    for (i, st) in initial.iter().enumerate() {
        let Some(((fx, fy), on)) = st.inputs.iter().find_map(|(xy, on)| xy.map(|p| (p, *on))) else {
            println!("viewer {i}: no clickable input, skipped ({st:?})");
            continue;
        };
        tested += 1;
        if st.playing {
            println!("viewer {i}: already playing before the click (expected paused)");
        }
        // bring the viewer into view, then click where its input is on its canvas now
        let rect = b.eval(&format!(
            "(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[{i}]; v.scrollIntoView({{block: 'center'}}); const r = v.querySelector('canvas').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; }})()"
        ))?;
        let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        if r.len() != 4 {
            bail!("viewer {i}: no canvas");
        }
        let (x, y) = (r[0] + fx * r[2], r[1] + fy * r[3]);
        let hit = b.eval(&format!("(() => {{ const e = document.elementFromPoint({x}, {y}); return e ? e.tagName + '.' + e.className : 'nothing'; }})()"))?;
        if hit.as_str() != Some("CANVAS.") {
            println!("viewer {i}: the point ({x:.0}, {y:.0}) is covered by {hit}");
        }
        if touch {
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchStart", "touchPoints": [{ "x": x, "y": y }] }))?;
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchEnd", "touchPoints": [] }))?;
        } else {
            for kind in ["mousePressed", "mouseReleased"] {
                b.call("Input.dispatchMouseEvent", json!({ "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1 }))?;
            }
        }
        // the input flips at once, the simulation starts, the build reacts
        let deadline = Instant::now() + Duration::from_secs(wait);
        let mut last = st.clone();
        let (mut saw_flip, mut saw_react) = (false, false);
        let verdict = loop {
            if let Some(Some(now)) = states(&mut b)?.get(i).cloned() {
                // a pulse input is on for 2 gt only, and a reaction can be brief: seen once is enough
                saw_flip |= now.inputs.iter().find_map(|(xy, on)| xy.map(|_| *on)) == Some(!on);
                saw_react |= match (st.door, now.door) {
                    (Some(a), Some(c)) => a != c,
                    _ => now.tick > st.tick + 1 && (now.active != st.active || now.active.is_none()),
                };
                last = now.clone();
                if saw_flip && now.playing && saw_react {
                    break Ok(now);
                }
            }
            if Instant::now() > deadline {
                break Err(last.clone());
            }
            std::thread::sleep(Duration::from_millis(40));
        };
        let how = if touch { "tap" } else { "click" };
        match verdict {
            Ok(now) => println!("viewer {i}: {how} at ({x:.0}, {y:.0}): input {} -> {}, playing, tick {} -> {}, door blocks {:?} -> {:?}: PASS", on as u8, !on as u8, st.tick, now.tick, st.door, now.door),
            Err(now) => {
                failed += 1;
                println!("viewer {i}: {how} at ({x:.0}, {y:.0}): FAIL (before {st:?}, after {now:?})");
            }
        }
    }
    if tested == 0 {
        bail!("no viewer had a clickable input");
    }
    if failed > 0 {
        bail!("{failed} of {tested} viewers did not react to a {}", if touch { "tap" } else { "click" });
    }
    println!("web click: {tested} viewers react to a {} on their input while paused ({window})", if touch { "tap" } else { "click" });
    Ok(())
}

/// The probe `web perf` installs before a page's own scripts: every animation frame's time, the
/// long tasks (where the browser reports them), and when each 3D view became ready (and whether
/// it was on screen then); `sample()` counts the views, `scroll()` scrolls at a speed.
const PERF_PROBE: &str = r#"(function(){
  if (window.__perf) return;
  var P = window.__perf = {frames: [], long: [], ready: [], logs: []};
  // the console's lines with their times (the viewer's `?mcview-debug` steps, matched to the long
  // tasks they ran in: `web stall --show`)
  try {
    var ol = console.log;
    console.log = function() { try { P.logs.push([performance.now(), Array.prototype.join.call(arguments, ' ')]); } catch (e) {} return ol.apply(console, arguments); };
  } catch (e) {}
  function raf(ts) { P.frames.push(ts); requestAnimationFrame(raf); }
  requestAnimationFrame(raf);
  try { new PerformanceObserver(function(l) { l.getEntries().forEach(function(e) { P.long.push([e.startTime, e.duration]); }); }).observe({entryTypes: ['longtask']}); } catch (e) {}
  function watch() {
    new MutationObserver(function(ms) {
      ms.forEach(function(m) {
        var t = m.target;
        if (t.classList && (t.classList.contains('mcview-ready') || t.classList.contains('mcview-error')) && !t.__pr) {
          t.__pr = 1;
          var r = t.getBoundingClientRect();
          P.ready.push([performance.now(), (r.bottom > 0 && r.top < innerHeight) ? 1 : 0]);
        }
      });
    }).observe(document.documentElement, {subtree: true, attributes: true, attributeFilter: ['class']});
  }
  if (document.documentElement) watch(); else document.addEventListener('DOMContentLoaded', watch);
  P.sample = function() {
    var els = document.querySelectorAll('[data-mcview]');
    var vis = 0, visReady = 0, ready = 0, seen = 0, seenReady = 0;
    for (var i = 0; i < els.length; i++) {
      var e = els[i];
      var rd = e.classList.contains('mcview-ready') || e.classList.contains('mcview-error');
      if (rd) ready++;
      var r = e.getBoundingClientRect();
      if (r.bottom > 0 && r.top < innerHeight && r.width > 0) { vis++; e.__seen = 1; if (rd) visReady++; }
      if (e.__seen) { seen++; if (rd) seenReady++; }
    }
    return {t: performance.now(), n: els.length, ready: ready, vis: vis, visReady: visReady, seen: seen, seenReady: seenReady, y: scrollY, frames: P.frames.length, canvases: document.querySelectorAll('canvas').length, heap: performance.memory ? performance.memory.usedJSHeapSize : 0};
  };
  P.scroll = function(seconds, speed) {
    return new Promise(function(done) {
      var start = null, last = null, gaps = [], y0 = scrollY, t0 = performance.now();
      function step(ts) {
        if (start === null) { start = ts; last = ts; }
        else { gaps.push(ts - last); window.scrollBy(0, (ts - last) * speed / 1000); last = ts; }
        if (ts - start < seconds * 1000) requestAnimationFrame(step);
        else done({gaps: gaps, dist: scrollY - y0, from: t0, to: performance.now()});
      }
      requestAnimationFrame(step);
    });
  };
  P.scrollStart = function(seconds, speed) { P.scrolled = null; P.scroll(seconds, speed).then(function(r) { P.scrolled = r; }); };
})();"#;

const IPHONE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

/// `web stall`: no page or card view may block the page (vjeux 2026-10-21, "Freezes my browser";
/// the coordinator's gate: no main-thread task over ~200 ms, never a page that stops answering).
/// Each page in headless Chromium (`window`, `scale`, the CPU slowed `cpu` times): `load` seconds
/// to load and draw its views (every card a still, every page's view a still until played), then
/// every live view's play pressed and `play` seconds of playing. Fails when the page stops
/// answering (a blocked main thread), when a long task passes `budget` ms, or when a view never
/// gets drawn (it never settles).
#[allow(clippy::too_many_arguments)]
pub fn cmd_stall(urls: &[String], window: &str, scale: f32, cpu: f64, load: f64, play: f64, budget: f64, show: Option<f64>, browser: &Path) -> Result<()> {
    let mut bad = 0;
    for url in urls {
        match stall_one(url, window, scale, cpu, load, play, budget, show, browser) {
            Ok(line) => println!("ok   {url}: {line}"),
            Err(e) => {
                bad += 1;
                println!("FAIL {url}: {e:#}");
            }
        }
    }
    println!("web stall: {} pages, {bad} blocking or never drawn (budget {budget:.0} ms a task)", urls.len());
    if bad > 0 {
        bail!("{bad} pages block the main thread or never draw their views");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stall_one(url: &str, window: &str, scale: f32, cpu: f64, load: f64, play: f64, budget: f64, show: Option<f64>, browser: &Path) -> Result<String> {
    let mut b = Browser::start(browser, window, scale, "about:blank")?;
    b.attach_page()?;
    b.call("Page.enable", json!({}))?;
    b.call("Runtime.enable", json!({}))?;
    if cpu > 1.0 {
        b.call("Emulation.setCPUThrottlingRate", json!({ "rate": cpu }))?;
    }
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({ "source": PERF_PROBE }))?;
    b.call("Page.navigate", json!({ "url": url }))?;
    let watch = |b: &mut Browser, secs: f64, what: &str| -> Result<Value> {
        let start = Instant::now();
        let mut last = Value::Null;
        while start.elapsed().as_secs_f64() < secs {
            b.drain(Duration::from_millis(250));
            last = b.eval("window.__perf ? __perf.sample() : null").map_err(|e| anyhow!("the page stopped answering {what} ({e:#}): its main thread is blocked"))?;
        }
        Ok(last)
    };
    let loaded = watch(&mut b, load, "while it loaded")?;
    let f = |v: &Value, k: &str| v[k].as_f64().unwrap_or(0.0);
    let (vis, vis_ready) = (f(&loaded, "vis"), f(&loaded, "visReady"));
    if vis_ready < vis {
        bail!("{} of the {vis:.0} views on screen never drawn within {load:.0} s", vis - vis_ready);
    }
    // the whole page scrolled through (every card drawn as it comes on screen), then back up
    let height = b.eval("document.documentElement.scrollHeight")?.as_f64().unwrap_or(0.0);
    let mut y = 0.0;
    while y < height {
        y += 600.0;
        b.eval(&format!("window.scrollTo(0, {y})")).map_err(|e| anyhow!("the page stopped answering while it scrolled ({e:#}): its main thread is blocked"))?;
        watch(&mut b, 0.5, "while it scrolled")?;
    }
    let scrolled = watch(&mut b, 3.0, "after it scrolled")?;
    // (the views that came on screen: the index adds a page of cards near its bottom, below
    // what the scroll covered, and a card never on screen is never drawn, by design)
    let (n, ready, all) = (f(&scrolled, "seen"), f(&scrolled, "seenReady"), f(&scrolled, "n"));
    b.eval("window.scrollTo(0, 0)")?;
    watch(&mut b, 1.0, "back at its top")?;
    // every live view played (its play button), then watched
    let played = b.eval("(function(){ var n = 0; document.querySelectorAll('button[aria-label^=\"Play / pause\"]').forEach(function(x){ x.click(); n++; }); return n; })()")?.as_f64().unwrap_or(0.0);
    watch(&mut b, play, "while its views played")?;
    let probe = b.eval("({long: __perf.long, logs: __perf.logs.slice(-4000)})")?;
    let long = pairs(&probe["long"]);
    let logs: Vec<(f64, String)> = probe["logs"].as_array().map(|a| a.iter().filter_map(|x| Some((x[0].as_f64()?, x[1].as_str()?.to_string()))).collect()).unwrap_or_default();
    // what the viewer logged while a task ran (its `?mcview-debug` steps)
    let during = |t: f64, d: f64| -> Vec<String> { logs.iter().filter(|(lt, _)| *lt >= t - 1.0 && *lt <= t + d + 1.0).map(|(lt, l)| format!("[{:.0} ms in] {}", lt - t, l.chars().take(240).collect::<String>())).collect() };
    if let Some(min) = show {
        for (t, d) in long.iter().filter(|x| x.1 > min) {
            println!("  task of {d:.0} ms at {:.2} s", t / 1000.0);
            for l in during(*t, *d) {
                println!("    {l}");
            }
        }
        // (every line the viewer logged in the first seconds, timed: what ran between the tasks)
        for (lt, l) in logs.iter().filter(|(lt, _)| *lt < 3000.0) {
            println!("  log at {:.3} s: {}", lt / 1000.0, l.chars().take(200).collect::<String>());
        }
    }
    let worst = long.iter().map(|x| x.1).fold(0.0, f64::max);
    let over: Vec<String> = long
        .iter()
        .filter(|x| x.1 > budget)
        .map(|(t, d)| {
            // with `?mcview-debug`, what the viewer logged while it ran (a card's steps, a view's load)
            let steps = during(*t, *d);
            format!("{d:.0} ms at {:.1} s{}", t / 1000.0, if steps.is_empty() { String::new() } else { format!(" (the viewer: {})", steps.join("; ")) })
        })
        .collect();
    if !over.is_empty() {
        bail!("{} main-thread task(s) over {budget:.0} ms: {}", over.len(), over.join(", "));
    }
    if n > 0.0 && ready < n {
        bail!("{} of the {n:.0} views that came on screen never drawn (the page scrolled through)", n - ready);
    }
    Ok(format!("{n:.0} views on screen drawn (of {all:.0} in the page), {played:.0} played for {play:.0} s, {} long tasks, the longest {worst:.0} ms", long.len()))
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * q).round() as usize]
}

/// What a `web perf` run collected, in either engine: the load window's samples, the navigation
/// times, every frame's time, the long tasks (None: the engine does not report them) and when
/// each view became ready; the scroll's.
struct PerfRun {
    label: String,
    samples: Vec<Value>,
    nav: Value,
    frames: Vec<f64>,
    long: Option<Vec<(f64, f64)>>,
    ready: Vec<(f64, f64)>,
    scroll: Option<ScrollRun>,
    console: Vec<String>,
}

/// The scroll part of a [`PerfRun`]: the probe's result, the sample when it stopped, the samples
/// after (every 100 ms), the long tasks and ready times at the end.
struct ScrollRun {
    secs: f64,
    speed: f64,
    result: Value,
    stop: Value,
    settle: Vec<Value>,
    long: Option<Vec<(f64, f64)>>,
    ready: Vec<(f64, f64)>,
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default()
}

fn pairs(v: &Value) -> Vec<(f64, f64)> {
    v.as_array().map(|a| a.iter().filter_map(|x| Some((x[0].as_f64()?, x[1].as_f64()?))).collect()).unwrap_or_default()
}

const NAV_JS: &str = "(function(){ var n = performance.getEntriesByType('navigation')[0]; return n ? [n.domContentLoadedEventEnd, n.loadEventEnd] : [0, 0]; })()";

/// `mctas web perf`: a page on a phone profile: how long it takes to show its cards and to draw
/// the on-screen 3D views, the long tasks and the slow frames meanwhile; then a scroll at `speed`
/// CSS px/s for `scroll` seconds: its frame times, the views drawn (and how many were already off
/// screen when drawn), and how long the views on screen when it stops take to be drawn. In
/// headless Chromium (the window's CSS size, the device scale, a touch screen, an iPhone user
/// agent, the CPU slowed `cpu` times) or in WebKitGTK (`engine` `webkit`: the site folder `dir`
/// served, `url` a page path in it; an iPhone user agent, device scale 2).
#[allow(clippy::too_many_arguments)]
pub fn cmd_perf(url: &str, engine: &str, dir: Option<&Path>, window: &str, scale: f32, cpu: f64, load: f64, scroll: f64, speed: f64, browser: &Path) -> Result<()> {
    let (w, h) = window.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))).context("--window w,h")?;
    let run = match engine {
        "webkit" => perf_webkit(url, dir.context("--engine webkit needs --dir (the site folder)")?, (w, h), load, scroll, speed)?,
        _ => perf_chromium(url, window, (w, h), scale, cpu, load, scroll, speed, browser)?,
    };
    perf_report(&run);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn perf_chromium(url: &str, window: &str, (w, h): (u32, u32), scale: f32, cpu: f64, load: f64, scroll: f64, speed: f64, browser: &Path) -> Result<PerfRun> {
    let mut b = Browser::start(browser, window, scale, "about:blank")?;
    b.attach_page()?;
    b.call("Page.enable", json!({}))?;
    b.call("Runtime.enable", json!({}))?;
    b.call("Emulation.setDeviceMetricsOverride", json!({ "width": w, "height": h, "deviceScaleFactor": scale, "mobile": true }))?;
    b.call("Emulation.setTouchEmulationEnabled", json!({ "enabled": true, "maxTouchPoints": 5 }))?;
    b.call("Emulation.setUserAgentOverride", json!({ "userAgent": IPHONE_UA }))?;
    if cpu > 1.0 {
        b.call("Emulation.setCPUThrottlingRate", json!({ "rate": cpu }))?;
    }
    b.call("Page.addScriptToEvaluateOnNewDocument", json!({ "source": PERF_PROBE }))?;
    b.call("Page.navigate", json!({ "url": url }))?;
    let start = Instant::now();
    let mut samples: Vec<Value> = Vec::new();
    while start.elapsed().as_secs_f64() < load {
        b.drain(Duration::from_millis(250));
        if let Ok(v) = b.eval("window.__perf ? __perf.sample() : null")
            && !v.is_null()
        {
            samples.push(v);
        }
    }
    let nav = b.eval(NAV_JS)?;
    let probe = b.eval("({frames: __perf.frames, long: __perf.long, ready: __perf.ready})")?;
    let mut run = PerfRun { label: format!("Chromium {window} at {scale}x, CPU /{cpu}"), samples, nav, frames: nums(&probe["frames"]), long: Some(pairs(&probe["long"])), ready: pairs(&probe["ready"]), scroll: None, console: Vec::new() };
    if scroll > 0.0 {
        let r = b.call("Runtime.evaluate", json!({ "expression": format!("__perf.scroll({scroll}, {speed})"), "returnByValue": true, "awaitPromise": true }))?;
        let result = r["result"]["value"].clone();
        let stop = b.eval("__perf.sample()")?;
        let settle0 = Instant::now();
        let mut settle = Vec::new();
        while settle0.elapsed().as_secs_f64() < 15.0 {
            let s = b.eval("__perf.sample()")?;
            let done = s["visReady"].as_f64() >= s["vis"].as_f64();
            settle.push(s);
            if done {
                break;
            }
            b.drain(Duration::from_millis(100));
        }
        let probe = b.eval("({long: __perf.long, ready: __perf.ready})")?;
        run.scroll = Some(ScrollRun { secs: scroll, speed, result, stop, settle, long: Some(pairs(&probe["long"])), ready: pairs(&probe["ready"]) });
    }
    run.console = b.console.clone();
    Ok(run)
}

fn perf_webkit(page: &str, dir: &Path, (w, h): (u32, u32), load: f64, scroll: f64, speed: f64) -> Result<PerfRun> {
    let sample = "return __perf.sample();".to_string();
    let mut steps: Vec<(String, u64)> = vec![(format!("{PERF_PROBE}; return true;"), 0)];
    let n = (load / 0.25).ceil() as usize;
    steps.extend(std::iter::repeat_n((sample.clone(), 250), n));
    steps.push((format!("return {{nav: {NAV_JS}, frames: __perf.frames, ready: __perf.ready}};"), 0));
    if scroll > 0.0 {
        steps.push((format!("__perf.scrollStart({scroll}, {speed}); return true;"), 0));
        steps.push(("return __perf.scrolled;".to_string(), (scroll * 1000.0) as u64 + 400));
        steps.push((sample.clone(), 0));
        steps.extend(std::iter::repeat_n((sample.clone(), 100), 150));
        steps.push(("return {ready: __perf.ready};".to_string(), 0));
    }
    let (out, console) = site::webkit::run_steps(dir, None, page, (w, h), &steps)?;
    let samples: Vec<Value> = out[1..=n].iter().filter(|v| !v.is_null()).cloned().collect();
    let end = &out[n + 1];
    let mut run = PerfRun { label: format!("WebKitGTK {w},{h} at 2x"), samples, nav: end["nav"].clone(), frames: nums(&end["frames"]), long: None, ready: pairs(&end["ready"]), scroll: None, console };
    if scroll > 0.0 {
        let k = n + 2;
        let settle: Vec<Value> = out[k + 3..k + 3 + 150].to_vec();
        // the samples up to the first with every view on screen drawn
        let cut = settle.iter().position(|s| s["visReady"].as_f64() >= s["vis"].as_f64()).map(|i| i + 1).unwrap_or(settle.len());
        run.scroll = Some(ScrollRun { secs: scroll, speed, result: out[k + 1].clone(), stop: out[k + 2].clone(), settle: settle[..cut].to_vec(), long: None, ready: pairs(&out[k + 3 + 150]["ready"]) });
    }
    Ok(run)
}

fn perf_report(run: &PerfRun) {
    let f = |v: &Value, k: &str| v[k].as_f64().unwrap_or(0.0);
    let secs = |ms: f64| format!("{:.2} s", ms / 1000.0);
    let slow = |g: &[f64], ms: f64| g.iter().filter(|x| **x > ms).count();
    let first_cards = run.samples.iter().find(|s| f(s, "n") > 0.0).map(|s| f(s, "t"));
    let screen = run.samples.iter().find(|s| f(s, "vis") > 0.0 && f(s, "visReady") >= f(s, "vis")).map(|s| (f(s, "t"), f(s, "vis")));
    let first_thumb = run.ready.first().map(|r| r.0);
    let load_end = run.samples.last().map(|s| f(s, "t")).unwrap_or(0.0);
    let gaps: Vec<f64> = run.frames.windows(2).map(|p| p[1] - p[0]).collect();
    let long_text = |long: &Option<Vec<(f64, f64)>>, a: f64, z: f64| match long {
        Some(l) => {
            let v: Vec<f64> = l.iter().filter(|(s, _)| *s >= a && *s < z).map(|(_, d)| *d).collect();
            format!("long tasks {} ({} in all, the longest {:.0} ms)", v.len(), secs(v.iter().sum()), v.iter().cloned().fold(0.0, f64::max))
        }
        None => "long tasks not reported by this engine".to_string(),
    };
    println!(
        "load ({}): DOMContentLoaded {}, load {}, first cards {}, first 3D view drawn {}, the first screen's views all drawn {}; {}; frames over 50 ms {} (the slowest {:.0} ms)",
        run.label,
        secs(run.nav[0].as_f64().unwrap_or(0.0)),
        secs(run.nav[1].as_f64().unwrap_or(0.0)),
        first_cards.map(secs).unwrap_or("never".into()),
        first_thumb.map(secs).unwrap_or("never".into()),
        screen.map(|(t, n)| format!("{} ({n:.0} views)", secs(t))).unwrap_or("not within the load window".into()),
        long_text(&run.long, 0.0, load_end),
        slow(&gaps, 50.0),
        gaps.iter().cloned().fold(0.0, f64::max),
    );
    if let Some(s) = run.samples.last() {
        println!("  after {}: {:.0} views in the page, {:.0} drawn, {:.0} on screen ({:.0} drawn), {:.0} canvases, JS heap {:.0} MB", secs(f(s, "t")), f(s, "n"), f(s, "ready"), f(s, "vis"), f(s, "visReady"), f(s, "canvases"), f(s, "heap") / 1e6);
    }
    if let Some(sc) = &run.scroll {
        let r = &sc.result;
        let mut g = nums(&r["gaps"]);
        let (from, to) = (f(r, "from"), f(r, "to"));
        let n = g.len();
        let mean = g.iter().sum::<f64>() / n.max(1) as f64;
        let (s33, s50) = (slow(&g, 34.0), slow(&g, 50.0));
        let (p50, p95, max) = (pct(&mut g, 0.5), pct(&mut g, 0.95), pct(&mut g, 1.0));
        let during: Vec<&(f64, f64)> = sc.ready.iter().filter(|(t, _)| *t >= from && *t <= to).collect();
        let off = during.iter().filter(|(_, v)| *v == 0.0).count();
        println!(
            "scroll {} s at {} px/s: {:.0} px; {n} frames (mean {mean:.0} ms, median {p50:.0}, p95 {p95:.0}, the slowest {max:.0}; {s33} over 33 ms, {s50} over 50 ms); {}; 3D views drawn meanwhile {} ({off} already off screen when drawn)",
            sc.secs,
            sc.speed,
            f(r, "dist"),
            long_text(&sc.long, from, to),
            during.len(),
        );
        let last = sc.settle.last().unwrap_or(&sc.stop);
        let settled = sc.settle.iter().find(|s| f(s, "visReady") >= f(s, "vis")).map(|s| f(s, "t") - to);
        println!(
            "  when it stopped: {:.0} views on screen, {:.0} drawn; all drawn {}; {:.0} views in the page, {:.0} canvases, JS heap {:.0} MB",
            f(&sc.stop, "vis"),
            f(&sc.stop, "visReady"),
            settled.map(|t| format!("{} later", secs(t))).unwrap_or_else(|| format!("not within 15 s ({:.0} of {:.0})", f(last, "visReady"), f(last, "vis"))),
            f(last, "n"),
            f(last, "canvases"),
            f(last, "heap") / 1e6
        );
    }
    let errors: Vec<&String> = run.console.iter().filter(|l| l.starts_with("exception") || l.starts_with("error") || l.starts_with("uncaught")).collect();
    if !errors.is_empty() {
        println!("  console errors: {}", errors.iter().take(5).map(|s| s.chars().take(160).collect::<String>()).collect::<Vec<_>>().join(" || "));
    }
    // with `?mcview-debug`: the viewer's own timing of the game's assets and of each thumbnail
    for l in run.console.iter().filter(|l| l.contains("the game's")).take(6) {
        println!("  console: {}", l.chars().take(160).collect::<String>());
    }
    let mut done: Vec<f64> = run.console.iter().filter_map(|l| l.split(" done in ").nth(1)).filter_map(|r| r.trim_end_matches(" ms").trim().parse::<f64>().ok()).collect();
    if !done.is_empty() {
        let n = done.len();
        let sum: f64 = done.iter().sum();
        println!("  thumbnails (the viewer's debug log): {n} drawn, {:.0} ms each on average, median {:.0}, p95 {:.0}, the slowest {:.0}", sum / n as f64, pct(&mut done, 0.5), pct(&mut done, 0.95), pct(&mut done, 1.0));
    }
}

/// A Chromium session on the phone profile (the window's CSS size, the device scale, touch, an
/// iPhone user agent) at `url`.
fn phone_browser(url: &str, window: &str, scale: f32, browser: &Path) -> Result<Browser> {
    let (w, h) = window.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))).context("--window w,h")?;
    let mut b = Browser::start(browser, window, scale, "about:blank")?;
    b.attach_page()?;
    b.call("Page.enable", json!({}))?;
    b.call("Runtime.enable", json!({}))?;
    b.call("Emulation.setDeviceMetricsOverride", json!({ "width": w, "height": h, "deviceScaleFactor": scale, "mobile": true }))?;
    b.call("Emulation.setTouchEmulationEnabled", json!({ "enabled": true, "maxTouchPoints": 5 }))?;
    b.call("Emulation.setUserAgentOverride", json!({ "userAgent": IPHONE_UA }))?;
    b.call("Page.navigate", json!({ "url": url }))?;
    Ok(b)
}

/// Run a function body (`return ...`) in the page; poll it every 250 ms until truthy (at most
/// `secs`): its value, else Null.
fn poll_body(b: &mut Browser, body: &str, secs: u64) -> Result<Value> {
    let t0 = Instant::now();
    loop {
        let v = b.eval(&format!("(() => {{ {body} }})()"))?;
        let truthy = !(v.is_null() || v == Value::Bool(false) || v.as_str() == Some("") || v.as_f64() == Some(0.0));
        if truthy {
            return Ok(v);
        }
        if t0.elapsed() > Duration::from_secs(secs) {
            return Ok(Value::Null);
        }
        b.drain(Duration::from_millis(250));
    }
}

/// `mctas web recqa`'s Chromium half: `page` (served at `base`) on the phone profile, its live
/// viewer's Record button pressed, the file it offers caught and saved (`recqa::save`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn chromium_record(engine: &str, base: &str, dir: &Path, out: &Path, page: &str, window: &str, scale: f32, timeout: u64, browser: &Path) -> Result<crate::recqa::Capture> {
    use crate::recqa::{HOOK, POLL, PRESS, READ, READ_POLL, READY};
    let url = format!("{base}{page}{}mcview-debug", if page.contains('?') { "&" } else { "?" });
    let mut b = phone_browser(&url, window, scale, browser)?;
    let mut c = crate::recqa::Capture { engine: engine.into(), page: page.to_string(), encoder: String::new(), file: None, note: String::new(), secs: 0.0, error: None };
    if poll_body(&mut b, READY, 90)?.is_null() {
        c.error = Some("the viewer was never ready".into());
        return Ok(c);
    }
    b.drain(Duration::from_millis(500));
    let enc = b.eval(&format!("(() => {{ {HOOK} }})()"))?;
    let press = b.eval(&format!("(() => {{ {PRESS} }})()"))?;
    c.encoder = format!("VideoEncoder {}; {}", enc.as_str().unwrap_or("?"), press.as_str().unwrap_or("?"));
    if press.as_str() != Some("pressed") {
        c.error = Some(format!("Record: {}", press.as_str().unwrap_or("?")));
        return Ok(c);
    }
    let poll = poll_body(&mut b, POLL, timeout)?;
    if poll.is_null() {
        c.error = Some(format!("no file within {timeout} s; console: {:?}", b.console.iter().filter(|l| l.contains("record")).take(3).collect::<Vec<_>>()));
        return Ok(c);
    }
    if poll["done"].as_i64() != Some(1) {
        c.error = Some(format!("the recording failed: {}", poll["error"].as_str().unwrap_or("?")));
        return Ok(c);
    }
    c.note = poll["note"].as_str().unwrap_or("").to_string();
    c.secs = poll["secs"].as_f64().unwrap_or(0.0);
    b.eval(&format!("(() => {{ {READ} }})()"))?;
    let b64 = poll_body(&mut b, READ_POLL, 60)?;
    match b64.as_str() {
        Some(s) if s != "ERROR" => c.file = Some(crate::recqa::save(out, dir, engine, page, s, poll["type"].as_str().unwrap_or(""))?),
        _ => c.error = Some("could not read the file back".into()),
    }
    Ok(c)
}

/// `mctas web recqa`'s Chromium playback: each file (`qa-rec/<name>` at `base`, its MIME type) in
/// a `<video>` on the phone profile: the page's answer (`recqa::PLAY`).
pub(crate) fn chromium_play(base: &str, files: &[(String, String)], window: &str, scale: f32, browser: &Path) -> Result<Vec<Value>> {
    use crate::recqa::{PLAY, PLAY_POLL};
    let mut b = phone_browser(&format!("{base}404.html"), window, scale, browser)?;
    b.drain(Duration::from_millis(1500));
    let mut out = Vec::new();
    for (name, mime) in files {
        // each on a fresh page
        b.call("Page.navigate", json!({ "url": format!("{base}404.html") }))?;
        b.drain(Duration::from_millis(800));
        let js = PLAY.replace("__SRC__", &format!("qa-rec/{name}")).replace("__TYPE__", &mime.replace('"', "\\\""));
        b.eval(&format!("(() => {{ {js} }})()"))?;
        out.push(poll_body(&mut b, PLAY_POLL, 40)?);
    }
    Ok(out)
}

/// `mctas web layout`: the page's live viewers brought on screen one after the other (a viewer
/// only draws and plays on screen), each watched until it ran `min_ticks` ticks (at most
/// `seconds`): every frame's boxes of the viewer, its bar, counter and badge (the viewer's debug
/// log, `?mcview-debug`) must never move.
pub fn cmd_layout(url: &str, seconds: u64, window: &str, scale: f32, min_ticks: u32, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut b = Browser::start(browser, window, scale, &page)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    b.drain(Duration::from_millis(1500));
    let n = b.eval("Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb').length")?.as_u64().unwrap_or(0) as usize;
    if n == 0 {
        bail!("no live viewer on the page");
    }
    // "view 1 tick 12 el x,y,w,h bar x,y,w,h label ... status ... cam ...": the boxes made relative
    // to the viewer's own (the page scrolls between viewers), the viewer's by its size
    let parse = |console: &[String]| -> std::collections::BTreeMap<String, Vec<(u32, String)>> {
        let mut m: std::collections::BTreeMap<String, Vec<(u32, String)>> = Default::default();
        let rect = |s: &str| -> Option<[f64; 4]> {
            let v: Vec<f64> = s.split(',').filter_map(|x| x.parse().ok()).collect();
            (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
        };
        for l in console {
            let Some(i) = l.find("mcview: layout ") else { continue };
            let rest = &l[i + "mcview: layout ".len()..];
            let w: Vec<&str> = rest.split_whitespace().collect();
            if w.len() < 6 || w[0] != "view" || w[2] != "tick" || w[4] != "el" {
                continue;
            }
            let Some(el) = rect(w[5]) else { continue };
            let mut parts = vec![format!("el {:.1}x{:.1}", el[2], el[3])];
            let mut k = 6;
            while k + 1 < w.len() && ["bar", "label", "status"].contains(&w[k]) {
                match rect(w[k + 1]) {
                    // the tick badge hugs its text: its left edge moves as it grows, so only
                    // its right edge (anchored 8 px from the viewer's) and its height count;
                    // the status line narrows as the badge grows, so only its left edge and
                    // height count
                    Some(r) if w[k] == "label" => parts.push(format!("label {:.1},{:.1},{:.1}", r[0] + r[2] - el[0], r[1] - el[1], r[3])),
                    Some(r) if w[k] == "status" => parts.push(format!("status {:.1},{:.1},{:.1}", r[0] - el[0], r[1] - el[1], r[3])),
                    Some(r) => parts.push(format!("{} {:.1},{:.1},{:.1},{:.1}", w[k], r[0] - el[0], r[1] - el[1], r[2], r[3])),
                    None => parts.push(format!("{} {}", w[k], w[k + 1])),
                }
                k += 2;
            }
            parts.push(w[k..].join(" "));
            m.entry(w[1].to_string()).or_default().push((w[3].parse().unwrap_or(0), parts.join(" ")));
        }
        m
    };
    for i in 0..n {
        b.eval(&format!("(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[{i}]; v.scrollIntoView({{block: 'center'}}); return v.getAttribute('data-mcview-id'); }})()"))?;
        let id = b.eval(&format!("Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[{i}].getAttribute('data-mcview-id')"))?.as_str().unwrap_or("").to_string();
        let t0 = Instant::now();
        while t0.elapsed().as_secs() < seconds {
            b.drain(Duration::from_millis(500));
            let views = parse(&b.console);
            if views.get(&id).is_some_and(|f| f.iter().map(|x| x.0).max().unwrap_or(0) - f.iter().map(|x| x.0).min().unwrap_or(0) >= min_ticks) {
                break;
            }
        }
    }
    let views = parse(&b.console);
    if views.is_empty() {
        bail!("no layout lines: no live viewer drew (is the page right, and served?)");
    }
    let mut bad = 0;
    for (id, frames) in &views {
        let first = &frames[0].1;
        let mut moved = 0;
        for (tick, rects) in frames {
            if rects != first {
                moved += 1;
                println!("view {id} moved at tick {tick}: {rects} (first: {first})");
            }
        }
        // the frames while it was on screen and playing: their ticks' span
        let (t0, t1) = (frames.iter().map(|f| f.0).min().unwrap_or(0), frames.iter().map(|f| f.0).max().unwrap_or(0));
        println!("view {id}: {} frames, ticks {t0}..{t1}, {moved} moved; {first}", frames.len());
        if moved > 0 || t1 - t0 < min_ticks {
            bad += 1;
        }
    }
    if views.len() < n {
        bail!("{} of {n} viewers drew", views.len());
    }
    if bad > 0 {
        bail!("{bad} viewers moved or ran fewer than {min_ticks} ticks");
    }
    println!("web layout: {} viewers, nothing moved over {min_ticks}+ ticks at {window}", views.len());
    Ok(())
}

