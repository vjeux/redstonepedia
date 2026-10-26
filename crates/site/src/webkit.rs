//! The WebKit smoke (`mctas site smoke --engine webkit`): the pages that matter rendered by
//! WebKitGTK (the engine family of iOS Safari) through WebKitWebDriver (W3C WebDriver over HTTP)
//! in an Xvfb display, on a desktop window and on a phone-sized window (iPhone user agent, device
//! scale 2): every 3D view must become ready and draw a real picture (enough colors, not black).
//!
//! Needs WebKitGTK's WebKitWebDriver and MiniBrowser (CentOS: `dnf install webkit2gtk3
//! webkit2gtk3-devel`) and Xvfb (`xorg-x11-server-Xvfb`).

use anyhow::{Context, Result, bail};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const MINIBROWSER: &str = "/usr/libexec/webkit2gtk-4.0/MiniBrowser";
const IPHONE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

/// The console recorder the local server puts at the top of every page (test harness only, never
/// deployed): every console line, uncaught error and unhandled rejection goes to
/// `window.__mcLog`, so the smoke can report the viewer's own messages (`GPU error`, a panic, an
/// asset `unavailable`).
const CONSOLE_HOOK: &str = "<script>(function(){var L=window.__mcLog=[];function s(a){try{return typeof a==='string'?a:(a&&a.stack)?String(a.stack):JSON.stringify(a)}catch(e){return String(a)}}['log','info','warn','error','debug'].forEach(function(k){var o=console[k];console[k]=function(){try{L.push(k+': '+Array.prototype.map.call(arguments,s).join(' '))}catch(e){}return o.apply(console,arguments)}});window.addEventListener('error',function(e){L.push('uncaught: '+(e.message||String(e))+(e.filename?' @ '+e.filename+':'+e.lineno:''))});window.addEventListener('unhandledrejection',function(e){L.push('unhandled rejection: '+s(e.reason))})})();</script>";

/// The page's recorded console lines (`None`: no recorder, e.g. the deployed site).
fn console_lines(s: &Session) -> Option<Vec<String>> {
    let v = s.script("return window.__mcLog ? window.__mcLog.slice() : null;").ok()?;
    v.as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
}

/// Console lines WebKitGTK writes itself, not the page: its built-in media-controls script (any
/// `<video controls>`: the door pages' record video) formats durations with Intl.DurationFormat,
/// which WebKitGTK 2.54's JavaScriptCore lacks (Safari has it; webrender 2026-10-21). Named in the
/// report, never a failure.
fn webkitgtk_own(low: &str) -> bool {
    low.contains("intl.durationformat") || low.contains("this._formatter.format")
}

/// The console lines worth a report (the viewer's `GPU error`, a panic, an `unavailable` asset,
/// uncaught errors), and whether they fail the check (everything but `unavailable` and
/// WebKitGTK's own media-controls errors).
fn console_flags(lines: &[String]) -> (Vec<String>, bool) {
    let mut flagged = Vec::new();
    let mut fail = false;
    let mut own = false;
    for l in lines {
        let low = l.to_lowercase();
        if webkitgtk_own(&low) {
            if !own {
                flagged.push("WebKitGTK's own media controls (no Intl.DurationFormat in its JavaScriptCore; not the page's code)".to_string());
                own = true;
            }
            continue;
        }
        let bad = low.contains("gpu error") || low.contains("panic") || low.starts_with("uncaught") || low.starts_with("unhandled rejection");
        if bad || low.contains("unavailable") {
            flagged.push(l.chars().take(220).collect::<String>());
            fail |= bad;
        }
    }
    (flagged, fail)
}

/// The console part of a check: a short summary, or an error when the console has a GPU error,
/// a panic or an uncaught error.
fn console_check(s: &Session) -> Result<String> {
    let Some(lines) = console_lines(s) else { return Ok("console not recorded".to_string()) };
    let (flagged, fail) = console_flags(&lines);
    if fail {
        bail!("console: {}", flagged.join(" || "));
    }
    Ok(if flagged.is_empty() { format!("console: {} lines, none flagged", lines.len()) } else { format!("console: {} lines; {}", lines.len(), flagged.join(" || ")) })
}

/// The game assets the viewer fetched (the vendored textures served with the site):
/// this page's resource timings plus the session's Cache API contents.
fn fetched_assets(s: &Session) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Ok(v) = s.script("return performance.getEntriesByType('resource').map(function(e){return e.name}).filter(function(n){return n.indexOf('textures/26.3') >= 0});") {
        out.extend(v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)));
    }
    let js = "var done = arguments[arguments.length - 1]; if (!window.caches) { done([]); return; } caches.keys().then(function(ks){ return Promise.all(ks.map(function(k){ return caches.open(k).then(function(c){ return c.keys(); }).then(function(rs){ return rs.map(function(r){ return r.url; }); }); })); }).then(function(x){ done([].concat.apply([], x)); }).catch(function(){ done([]); });";
    if let Ok(v) = s.post("execute/async", serde_json::json!({"script": js, "args": []})) {
        out.extend(v["value"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).filter(|u| u.contains("textures/26.3")));
    }
    let mut short: Vec<String> = out.iter().map(|u| u.split("textures/26.3/").nth(1).map(|r| r.to_string()).unwrap_or_else(|| u.clone())).collect();
    short.sort();
    short.dedup();
    short
}

/// A child process killed when dropped.
struct Proc(Child);

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Xvfb, stopped with SIGTERM when dropped (so it removes its lock file and socket).
struct Xvfb(Child);

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = Command::new("kill").args(["-TERM", &self.0.id().to_string()]).status();
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(5) {
            if let Ok(Some(_)) = self.0.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A display number in :90..:99 that no live X server holds (a stale lock of a dead server is
/// removed with its socket).
fn free_display() -> Option<u32> {
    (90..100).find(|n| {
        let lock = PathBuf::from(format!("/tmp/.X{n}-lock"));
        let sock = PathBuf::from(format!("/tmp/.X11-unix/X{n}"));
        match std::fs::read_to_string(&lock) {
            Err(_) => !sock.exists(),
            Ok(pid) => {
                let alive = Path::new(&format!("/proc/{}", pid.trim())).exists();
                if !alive {
                    let _ = std::fs::remove_file(&lock);
                    let _ = std::fs::remove_file(&sock);
                }
                !alive
            }
        }
    })
}

fn free_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

/// One HTTP request to the local WebDriver; the JSON body of the answer.
fn http(port: u16, method: &str, path: &str, body: Option<&serde_json::Value>) -> Result<serde_json::Value> {
    http_within(port, method, path, body, Duration::from_secs(120))
}

/// [`http`], waiting at most `timeout` for the answer (a frozen page never answers).
fn http_within(port: u16, method: &str, path: &str, body: Option<&serde_json::Value>, timeout: Duration) -> Result<serde_json::Value> {
    let mut s = TcpStream::connect(("127.0.0.1", port))?;
    s.set_read_timeout(Some(timeout))?;
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    s.write_all(req.as_bytes())?;
    let mut all = Vec::new();
    s.read_to_end(&mut all)?;
    let head_end = all.windows(4).position(|w| w == b"\r\n\r\n").context("no HTTP header from WebKitWebDriver")?;
    let head = String::from_utf8_lossy(&all[..head_end]).to_lowercase();
    let mut body = all[head_end + 4..].to_vec();
    if head.contains("transfer-encoding: chunked") {
        let mut out = Vec::new();
        let mut rest = &body[..];
        while let Some(i) = rest.windows(2).position(|w| w == b"\r\n") {
            let n = usize::from_str_radix(String::from_utf8_lossy(&rest[..i]).trim(), 16).unwrap_or(0);
            if n == 0 {
                break;
            }
            let start = i + 2;
            out.extend_from_slice(&rest[start..(start + n).min(rest.len())]);
            rest = &rest[(start + n + 2).min(rest.len())..];
        }
        body = out;
    }
    let v: serde_json::Value = serde_json::from_slice(&body).with_context(|| format!("{method} {path}: {}", String::from_utf8_lossy(&body).chars().take(300).collect::<String>()))?;
    if let Some(e) = v["value"]["error"].as_str() {
        bail!("{method} {path}: {e}: {}", v["value"]["message"].as_str().unwrap_or(""));
    }
    Ok(v)
}

struct Session {
    port: u16,
    id: String,
    _driver: Proc,
}

impl Session {
    fn start(display: &str, phone: bool) -> Result<Session> {
        Session::start_sized(display, phone, None)
    }

    /// [`Session::start`] with this window size (CSS px) instead of the default for its form.
    fn start_sized(display: &str, phone: bool, size: Option<(u32, u32)>) -> Result<Session> {
        let port = free_port()?;
        let mut cmd = Command::new("WebKitWebDriver");
        cmd.arg(format!("--port={port}")).env("DISPLAY", display).stdout(Stdio::null()).stderr(Stdio::null());
        if phone {
            cmd.env("GDK_SCALE", "2");
        }
        let driver = Proc(cmd.spawn().context("running WebKitWebDriver (dnf install webkit2gtk3)")?);
        let t0 = Instant::now();
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            if t0.elapsed() > Duration::from_secs(20) {
                bail!("WebKitWebDriver did not start");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let mut args = vec!["--automation".to_string(), "--enable-webgl=true".to_string()];
        if phone {
            args.push(format!("--user-agent={IPHONE_UA}"));
        }
        let caps = serde_json::json!({"capabilities": {"alwaysMatch": {"webkitgtk:browserOptions": {"binary": MINIBROWSER, "args": args}}}});
        let v = http(port, "POST", "/session", Some(&caps))?;
        let id = v["value"]["sessionId"].as_str().context("no session id")?.to_string();
        let s = Session { port, id, _driver: driver };
        let (w, h) = size.unwrap_or(if phone { (390, 844) } else { (1280, 900) });
        let _ = s.post("window/rect", serde_json::json!({"width": w, "height": h}));
        Ok(s)
    }

    fn post(&self, what: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        http(self.port, "POST", &format!("/session/{}/{what}", self.id), Some(&body))
    }

    fn script(&self, js: &str) -> Result<serde_json::Value> {
        Ok(self.post("execute/sync", serde_json::json!({"script": js, "args": []}))?["value"].clone())
    }

    /// [`Session::script`], waiting at most `secs` for the page to run it.
    fn script_within(&self, js: &str, secs: u64) -> Result<serde_json::Value> {
        Ok(http_within(self.port, "POST", &format!("/session/{}/execute/sync", self.id), Some(&serde_json::json!({"script": js, "args": []})), Duration::from_secs(secs))?["value"].clone())
    }

    fn screenshot(&self) -> Result<(u32, u32, Vec<[u8; 3]>)> {
        let v = http(self.port, "GET", &format!("/session/{}/screenshot", self.id), None)?;
        self.decode_shot(&v)
    }

    fn decode_shot(&self, v: &serde_json::Value) -> Result<(u32, u32, Vec<[u8; 3]>)> {
        let b64 = v["value"].as_str().context("no screenshot")?;
        use base64::Engine as _;
        let png = base64::engine::general_purpose::STANDARD.decode(b64)?;
        // MCTAS_WEBKIT_SHOTS=<dir>: keep every screenshot (shot-1.png, shot-2.png...)
        if let Some(dir) = std::env::var_os("MCTAS_WEBKIT_SHOTS") {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let k = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(Path::new(&dir).join(format!("shot-{k}.png")), &png);
        }
        crate::serve::decode_png(&png)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = http(self.port, "DELETE", &format!("/session/{}", self.id), None);
    }
}

/// Colors, mean luma and the black fraction of a region of a screenshot.
fn picture_stats(shot: &(u32, u32, Vec<[u8; 3]>), x: f64, y: f64, w: f64, h: f64) -> (usize, f64, f64, f64) {
    let (sw, sh, px) = shot;
    let (x0, y0) = (x.max(0.0) as u32, y.max(0.0) as u32);
    let (x1, y1) = (((x + w) as u32).min(*sw), ((y + h) as u32).min(*sh));
    let mut colors = std::collections::HashSet::new();
    let (mut sum, mut black, mut n) = (0f64, 0usize, 0usize);
    // high-frequency variation: the mean channel step between horizontally adjacent pixels.
    // A textured build steps hard (the stone's speckle); flat shading (a view whose GPU never
    // got the game texture) steps ~1 per pixel, however many colors the sky gradient adds.
    let (mut step, mut steps) = (0f64, 0usize);
    for yy in y0..y1 {
        for xx in x0..x1 {
            let p = px[(yy * sw + xx) as usize];
            let l = 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
            sum += l;
            if l < 16.0 {
                black += 1;
            }
            n += 1;
            colors.insert(p);
            if xx + 1 < x1 {
                let q = px[(yy * sw + xx + 1) as usize];
                step += (0..3).map(|c| (p[c] as f64 - q[c] as f64).abs()).sum::<f64>() / 3.0;
                steps += 1;
            }
        }
    }
    if n == 0 {
        return (0, 0.0, 1.0, 0.0);
    }
    (colors.len(), sum / n as f64, black as f64 / n as f64, if steps == 0 { 0.0 } else { step / steps as f64 })
}

/// Check one page: its first viewer (`selector`) becomes ready and draws a picture.
fn check_page(s: &Session, base: &str, page: &str, selector: &str) -> Result<String> {
    s.post("url", serde_json::json!({"url": format!("{base}{page}")}))?;
    let t0 = Instant::now();
    let state = loop {
        let js = format!("const v = document.querySelector('{selector}'); if (!v) return 'none'; v.scrollIntoView({{block: 'center'}}); return v.className;");
        let c = s.script(&js)?.as_str().unwrap_or("").to_string();
        // the index draws its cards (and their views) only after loading the data: wait for them
        if c.contains("mcview-ready") || c.contains("mcview-error") {
            break c;
        }
        if t0.elapsed() > Duration::from_secs(90) {
            break format!("timeout ({c})");
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    if !state.contains("mcview-ready") {
        let title = s.script(&format!("const v = document.querySelector('{selector}'); return v ? (v.getAttribute('title') || '') : '';")).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        bail!("the view did not become ready: {state} {title}");
    }
    // let it draw a few frames (software GL is slow)
    std::thread::sleep(Duration::from_secs(4));
    let detail = picture(s, selector)?;
    // the entity clocks draw their entities through a texture fetched lazily: give it time
    Ok(format!("{detail}; {}", console_check(s)?))
}

/// The longest a trivial script may wait for a group page's main thread (seconds).
const RESPONSIVE_S: f64 = 2.0;

/// A group page (a tab of the index, a study's cards) must stay responsive while its thumbnails
/// draw (the coordinator 2026-10-21: the Auto-brewer group page froze vjeux's browser, its cards
/// simulating their designs up to their still frames on the page's one thread): loaded, then
/// scrolled through top to bottom (the thumbnails draw as they come into view), a trivial script
/// must come back within [`RESPONSIVE_S`] at every step. A page frozen for good fails on
/// WebDriver's own script timeout.
fn check_responsive(s: &Session, base: &str, page: &str) -> Result<String> {
    // (a page that never answers: its main thread stayed busy; said so, not as a socket error)
    const GIVE_UP_S: u64 = 20;
    let frozen = |e: anyhow::Error, at: &str| -> anyhow::Error {
        let m = format!("{e:#}");
        if m.contains("os error 11") || m.contains("timed out") || m.contains("WouldBlock") {
            anyhow::anyhow!("not responsive: the page did not answer a one-line script within {GIVE_UP_S} s {at} (its main thread stayed busy)")
        } else {
            e
        }
    };
    let t = Instant::now();
    http_within(s.port, "POST", &format!("/session/{}/url", s.id), Some(&serde_json::json!({"url": format!("{base}{page}")})), Duration::from_secs(60)).map_err(|e| frozen(e, "loading"))?;
    let load = t.elapsed().as_secs_f64();
    // (the index draws its cards after loading its data: give it a moment, then measure)
    std::thread::sleep(Duration::from_millis(800));
    let height = s.script_within("return document.documentElement.scrollHeight", GIVE_UP_S).map_err(|e| frozen(e, "after loading"))?.as_f64().unwrap_or(0.0);
    let steps = 6;
    let mut worst = 0.0f64;
    for k in 0..=steps {
        let t = Instant::now();
        s.script_within(&format!("window.scrollTo(0, {}); return document.readyState;", (height * k as f64 / steps as f64).round()), GIVE_UP_S).map_err(|e| frozen(e, &format!("at scroll step {k}/{steps}")))?;
        let rt = t.elapsed().as_secs_f64();
        worst = worst.max(rt);
        if rt > RESPONSIVE_S {
            bail!("not responsive: a one-line script waited {rt:.1} s for the page at scroll step {k}/{steps} (loaded in {load:.1} s)");
        }
        std::thread::sleep(Duration::from_millis(600));
    }
    let drawn = s.script_within("return document.querySelectorAll('.thumb.mcview-ready').length + ' of ' + document.querySelectorAll('.thumb[data-mcview]').length", GIVE_UP_S).map_err(|e| frozen(e, "at the end"))?.as_str().unwrap_or("").to_string();
    Ok(format!("loaded in {load:.1} s, the slowest script round trip {worst:.2} s scrolling through, thumbnails drawn: {drawn}; {}", console_check(s)?))
}

/// Poll a script until it returns true (or time out).
fn wait_true(s: &Session, js: &str, secs: u64) -> Result<bool> {
    let t0 = Instant::now();
    loop {
        if s.script(js)?.as_bool().unwrap_or(false) {
            return Ok(true);
        }
        if t0.elapsed() > Duration::from_secs(secs) {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Click the element `css` through WebDriver (a real click: the page's own handlers run).
fn click(s: &Session, css: &str) -> Result<()> {
    let v = s.post("element", serde_json::json!({"using": "css selector", "value": css}))?;
    let id = v["value"].as_object().and_then(|o| o.values().next()).and_then(|x| x.as_str()).context("no element id")?.to_string();
    s.post(&format!("element/{id}/click"), serde_json::json!({}))?;
    Ok(())
}

/// The in-place swap of a tunable group's picker (vjeux: "Don't reload the page when clicking on
/// variants, update inline and keep the current camera"): after a click on another version the
/// viewer is the same element (no reload), its design is the version's, the URL and the title are
/// the version's, and it still draws a real picture.
fn check_picker_swap(s: &Session, base: &str, page: &str, other_page: &str, other_design: &str, other_name: &str) -> Result<String> {
    check_page(s, base, page, VIEW)?;
    let live = s.script(&format!("const v = document.querySelector('{VIEW}'); v.__probe = 'kept'; return v.classList.contains('mcview-live-src');"))?.as_bool().unwrap_or(false);
    if !live {
        bail!("the viewer does not mark itself mcview-live-src (no in-place swaps with this bundle)");
    }
    click(s, &format!("nav.picker a.pick[href$=\"{other_page}\"]"))?;
    let js = format!(
        "const v = document.querySelector('{VIEW}'); return !!v && location.pathname.endsWith('{other_page}') && (v.getAttribute('data-src') || '').endsWith('{other_design}') && document.title.includes('{other_name}');"
    );
    if !wait_true(s, &js, 20)? {
        let state = s.script(&format!("const v = document.querySelector('{VIEW}'); return [location.pathname, v ? v.getAttribute('data-src') : 'no viewer', document.title].join(' | ');"))?;
        bail!("no swap after the click: {state}");
    }
    let same = s.script(&format!("return document.querySelector('{VIEW}').__probe === 'kept';"))?.as_bool().unwrap_or(false);
    if !same {
        bail!("the page reloaded (the viewer element is a new one)");
    }
    std::thread::sleep(Duration::from_secs(4));
    let detail = picture(s, VIEW)?;
    let console = console_check(s)?;
    Ok(format!("same viewer, URL + title + design swapped, {detail}; {console}"))
}

/// A family page's member row: the family view shows that member (same element, its design).
fn check_family_swap(s: &Session, base: &str, page: &str) -> Result<String> {
    check_page(s, base, page, "#member-view")?;
    s.script("document.querySelector('#member-view').__probe = 'kept'; return true;")?;
    let target = s.script("const a = document.querySelectorAll('a.view[data-src]'); return a.length > 1 ? [a[1].getAttribute('href'), a[1].getAttribute('data-src')] : null;")?;
    let (Some(href), Some(src)) = (target[0].as_str().map(String::from), target[1].as_str().map(String::from)) else { bail!("no second member row to click") };
    click(s, &format!("a.view[href=\"{href}\"]"))?;
    let js = format!("const v = document.querySelector('#member-view'); return v.getAttribute('data-src') === '{src}' && location.hash === '{href}' && v.__probe === 'kept';");
    let mut how = "click";
    if !wait_true(s, &js, 5)? {
        // the page must not hide the link (what is at its center?); WebKitWebDriver's synthesized
        // clicks can miss small targets at device scale 2: then a script click (the page's own
        // handler still runs)
        let hit = s.script(&format!("const a = document.querySelector('a.view[href=\"{href}\"]'); a.scrollIntoView({{block: 'center'}}); const r = a.getBoundingClientRect(); const e = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return !!e && (e === a || a.contains(e));"))?;
        if !hit.as_bool().unwrap_or(false) {
            bail!("the member link {href} is covered by another element");
        }
        s.script(&format!("document.querySelector('a.view[href=\"{href}\"]').click(); return true;"))?;
        how = "script click (the link is not covered)";
    }
    if !wait_true(s, &js, 10)? {
        let state = s.script("const v = document.querySelector('#member-view'); return [v ? v.getAttribute('data-src') : 'no view', v ? v.className : '', location.hash, v ? String(v.__probe) : ''].join(' | ');")?;
        bail!("the family view did not swap to {src}: {state}");
    }
    std::thread::sleep(Duration::from_secs(4));
    let detail = picture(s, "#member-view")?;
    let console = console_check(s)?;
    Ok(format!("same view, member {href} shown ({how}), {detail}; {console}"))
}

/// The view `selector` must draw a real picture: enough colors, not dark, not blank (a cleared or
/// white canvas), measured on a screenshot of the window cropped to the view (what a user sees).
fn picture(s: &Session, selector: &str) -> Result<String> {
    s.script(&format!("document.querySelector('{selector}').scrollIntoView({{block: 'center', inline: 'nearest'}}); return true;"))?;
    std::thread::sleep(Duration::from_millis(800));
    let r = s.script(&format!("const v = document.querySelector('{selector}'); const r = v.getBoundingClientRect(); return [r.x, r.y, r.width, r.height, window.devicePixelRatio, window.innerWidth];"))?;
    let f = |i: usize| r[i].as_f64().unwrap_or(0.0);
    let dpr = f(4).max(1.0);
    let shot = s.screenshot()?;
    // the screenshot is in device pixels when it is wider than the CSS viewport
    let scale = if shot.0 as f64 > f(5) * 1.5 { dpr } else { 1.0 };
    let (colors, luma, black, _) = picture_stats(&shot, f(0) * scale, f(1) * scale, f(2) * scale, f(3) * scale);
    // the build sits in the view's center third: its texture must vary there, not just the sky
    let (center_colors, _, _, center_step) = picture_stats(&shot, (f(0) + f(2) / 3.0) * scale, (f(1) + f(3) / 3.0) * scale, f(2) * scale / 3.0, f(3) * scale / 3.0);
    let detail = format!("{colors} colors, mean luma {luma:.0}, {:.0}% black, center {center_colors} colors, step {center_step:.1} ({}x{} css px, dpr {dpr})", black * 100.0, f(2) as u32, f(3) as u32);
    if colors < 40 || luma < 25.0 || luma > 225.0 || black > 0.1 || center_step < CENTER_STEP_MIN {
        bail!("the view draws no real picture: {detail}");
    }
    Ok(detail)
}

/// The center third's mean adjacent-pixel step below which the build is flat shading, not
/// texture (the 2026-10-24 live-view stale-atlas bug: game-model faces on the white texture).
/// Calibrated: flat 0.4–0.7; textured 1.3 (a 2-block build under its wireframe) to 4.1.
const CENTER_STEP_MIN: f64 = 1.0;

const VIEW: &str = "main > [data-mcview=\"full\"]";

fn wait_for(path: &Path, secs: u64) -> bool {
    let t0 = Instant::now();
    while !path.exists() {
        if t0.elapsed() > Duration::from_secs(secs) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}

/// What both the smoke and the film need: the site's data, its base URL (served locally with
/// the console recorder, or the deployed site), and an X display of our own.
struct Rig {
    site: site_model::Site,
    base: String,
    display: String,
    _xvfb: Xvfb,
}

fn rig(dir: &Path, remote: Option<&str>) -> Result<Rig> {
    if !Path::new(MINIBROWSER).is_file() {
        bail!("no {MINIBROWSER}: dnf install webkit2gtk3 webkit2gtk3-devel xorg-x11-server-Xvfb");
    }
    let (site, base): (site_model::Site, String) = match remote {
        Some(url) => {
            let url = if url.ends_with('/') { url.to_string() } else { format!("{url}/") };
            let o = Command::new("curl").args(["-fsSL", &format!("{url}data/site.json")]).output().context("running curl")?;
            if !o.status.success() {
                bail!("{url}data/site.json: {}", String::from_utf8_lossy(&o.stderr).trim());
            }
            (serde_json::from_slice(&o.stdout).context("the deployed data/site.json")?, url)
        }
        None => {
            let site = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json"))?)?;
            // serve the folder
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            let port = listener.local_addr()?.port();
            let root: PathBuf = dir.to_path_buf();
            std::thread::spawn(move || crate::serve::serve_injecting(listener, root, Default::default(), CONSOLE_HOOK.to_string()));
            (site, format!("http://127.0.0.1:{port}/"))
        }
    };
    // a display of our own
    let n = free_display().context("no free X display in :90..:99")?;
    let display = format!(":{n}");
    let xvfb = Xvfb(Command::new("Xvfb").args([display.as_str(), "-screen", "0", "1400x1000x24", "-nolisten", "tcp"]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().context("running Xvfb (dnf install xorg-x11-server-Xvfb)")?);
    if !wait_for(Path::new(&format!("/tmp/.X11-unix/X{n}")), 10) {
        bail!("Xvfb {display} did not start");
    }
    Ok(Rig { site, base, display, _xvfb: xvfb })
}

/// The fraction of pixels of the region (`x, y, w, h` in screenshot pixels) that differ between
/// two screenshots of the same size (any channel by more than 24).
fn changed(a: &(u32, u32, Vec<[u8; 3]>), b: &(u32, u32, Vec<[u8; 3]>), r: (f64, f64, f64, f64)) -> f64 {
    if a.0 != b.0 || a.1 != b.1 {
        return 1.0;
    }
    let (x0, y0) = (r.0.max(0.0) as u32, r.1.max(0.0) as u32);
    let (x1, y1) = (((r.0 + r.2) as u32).min(a.0), ((r.1 + r.3) as u32).min(a.1));
    let (mut n, mut d) = (0usize, 0usize);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * a.0 + x) as usize;
            let (p, q) = (a.2[i], b.2[i]);
            n += 1;
            if (0..3).any(|c| (p[c] as i32 - q[c] as i32).abs() > 24) {
                d += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { d as f64 / n as f64 }
}

/// Click the button labelled `label` (a viewer's bar button: `INP`, `MOD`, `IN`...): a real
/// WebDriver click, else a scripted one.
fn click_button(s: &Session, label: &str) -> Result<()> {
    let xpath = format!("//button[normalize-space(.)='{label}']");
    let v = s.post("element", serde_json::json!({"using": "xpath", "value": xpath}))?;
    let id = v["value"].as_object().and_then(|o| o.values().next()).and_then(|x| x.as_str()).map(String::from);
    if let Some(id) = id {
        if s.post(&format!("element/{id}/click"), serde_json::json!({})).is_ok() {
            return Ok(());
        }
    }
    let ok = s.script(&format!(
        "const b = [...document.querySelectorAll('button')].find(b => b.textContent.trim() === '{label}'); if (!b) return false; for (const t of ['pointerdown', 'pointerup']) b.dispatchEvent(new PointerEvent(t, {{bubbles: true}})); b.click(); return true;"
    ))?;
    if ok.as_bool() != Some(true) {
        bail!("no button labelled {label}");
    }
    Ok(())
}

/// Open `page` of the site in WebKit on a phone (an iPhone user agent, device scale 2, a window of
/// `size` CSS px) and run `steps` in order, each after `wait` ms: a script (a function body: its
/// `return` value is kept), or `@url <path>` (open another page of the site), `@shot` (a
/// screenshot: its PNG, base64), `@until <script>` (run it every 250 ms until it returns
/// something truthy, at most 60 s, or `@until<N>` at most N s: that value, else null). The page's
/// console lines last. For webrender's `mctas web perf --engine webkit` (the phone profile of the
/// long pages), `mctas web qa` (the viewer's visual QA at phone size) and `mctas web recqa` (the
/// Record button's files).
pub fn run_steps(dir: &Path, remote: Option<&str>, page: &str, size: (u32, u32), steps: &[(String, u64)]) -> Result<(Vec<serde_json::Value>, Vec<String>)> {
    let r = rig(dir, remote)?;
    let s = Session::start_sized(&r.display, true, Some(size))?;
    s.post("url", serde_json::json!({"url": format!("{}{page}", r.base)}))?;
    let mut out = Vec::new();
    let truthy = |v: &serde_json::Value| !(v.is_null() || v == &serde_json::Value::Bool(false) || v.as_str() == Some("") || v.as_f64() == Some(0.0));
    for (step, wait) in steps {
        std::thread::sleep(Duration::from_millis(*wait));
        let until = step.strip_prefix("@until").and_then(|rest| {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let js = rest[digits.len()..].strip_prefix(' ')?;
            Some((digits.parse::<u64>().unwrap_or(60), js))
        });
        let v = if let Some(p) = step.strip_prefix("@url ") {
            s.post("url", serde_json::json!({"url": format!("{}{}", r.base, p.trim())})).map(|_| serde_json::Value::Bool(true)).unwrap_or(serde_json::Value::Null)
        } else if step == "@shot" {
            http(s.port, "GET", &format!("/session/{}/screenshot", s.id), None).map(|v| v["value"].clone()).unwrap_or(serde_json::Value::Null)
        } else if let Some((secs, js)) = until {
            let t0 = Instant::now();
            loop {
                let v = s.script(js).unwrap_or(serde_json::Value::Null);
                if truthy(&v) {
                    break v;
                }
                if t0.elapsed() > Duration::from_secs(secs) {
                    break serde_json::Value::Null;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        } else {
            s.script(step).unwrap_or(serde_json::Value::Null)
        };
        out.push(v);
    }
    Ok((out, console_lines(&s).unwrap_or_default()))
}

/// Film pages in WebKit (`site smoke --engine webkit --page <path>`): `frames` screenshots of
/// each page's first 3D view, `interval` ms apart, desktop and phone, saved as
/// `<page>-<desktop|phone>-<k>.png` in $MCTAS_WEBKIT_SHOTS (default /tmp/mctas-webkit-shots),
/// with each frame's colors and brightness in the view, how much of the view changed since the
/// previous frame and since the first, and the page's console. `click`: press the buttons with
/// these labels first, in order (a viewer's input button: `INP` turns a shulker machine's start
/// lever on; `20×` picks that speed in the speed dropdown; `css:<selector>`: any element).
pub fn film(dir: &Path, remote: Option<&str>, pages: &[String], frames: u32, interval: u64, click: &[String]) -> Result<()> {
    let r = rig(dir, remote)?;
    let out = std::env::var_os("MCTAS_WEBKIT_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp/mctas-webkit-shots"));
    std::fs::create_dir_all(&out)?;
    for phone in [false, true] {
        let tag = if phone { "phone" } else { "desktop" };
        let s = Session::start(&r.display, phone)?;
        for page in pages {
            let sel = if page.starts_with("index.html") { ".thumb[data-mcview]" } else { "[data-mcview=\"full\"]" };
            match check_page(&s, &r.base, page, sel) {
                Ok(d) => println!("{tag} {page}: ready, {d}"),
                Err(e) => println!("{tag} {page}: {e:#}"),
            }
            for label in click {
                match press(&s, label) {
                    Ok(()) => println!("  pressed the {label} button"),
                    Err(e) => println!("  could not press {label}: {e:#}"),
                }
                std::thread::sleep(Duration::from_millis(300));
            }
            // the view's rectangle in screenshot pixels
            s.script(&format!("document.querySelector('{sel}').scrollIntoView({{block: 'center', inline: 'nearest'}}); return true;"))?;
            std::thread::sleep(Duration::from_millis(300));
            let rv = s.script(&format!("const v = document.querySelector('{sel}'); const r = v.getBoundingClientRect(); return [r.x, r.y, r.width, r.height, window.devicePixelRatio, window.innerWidth];"))?;
            let f = |i: usize| rv[i].as_f64().unwrap_or(0.0);
            let stem = page.trim_end_matches(".html").replace(['/', '#', '?', '&', '='], "_");
            let (mut first, mut prev): (Option<(u32, u32, Vec<[u8; 3]>)>, Option<(u32, u32, Vec<[u8; 3]>)>) = (None, None);
            for k in 1..=frames {
                std::thread::sleep(Duration::from_millis(interval));
                let v = http(s.port, "GET", &format!("/session/{}/screenshot", s.id), None)?;
                let png = out.join(format!("{stem}-{tag}-{k}.png"));
                if let Some(b64) = v["value"].as_str() {
                    use base64::Engine as _;
                    std::fs::write(&png, base64::engine::general_purpose::STANDARD.decode(b64)?)?;
                }
                let shot = s.decode_shot(&v)?;
                let scale = if shot.0 as f64 > f(5) * 1.5 { f(4).max(1.0) } else { 1.0 };
                let rect = (f(0) * scale, f(1) * scale, f(2) * scale, f(3) * scale);
                let (colors, luma, black, step) = picture_stats(&shot, rect.0, rect.1, rect.2, rect.3);
                let (_, _, _, center_step) = picture_stats(&shot, rect.0 + rect.2 / 3.0, rect.1 + rect.3 / 3.0, rect.2 / 3.0, rect.3 / 3.0);
                let moved = match (&prev, &first) {
                    (Some(p), Some(f0)) => format!(", {:.1}% of the view changed since the previous frame, {:.1}% since the first", changed(p, &shot, rect) * 100.0, changed(f0, &shot, rect) * 100.0),
                    _ => String::new(),
                };
                println!("  frame {k}: {} ({colors} colors, mean luma {luma:.0}, {:.0}% black, step {step:.1}, center step {center_step:.1}{moved})", png.display(), black * 100.0);
                if first.is_none() {
                    first = Some(shot.clone());
                }
                prev = Some(shot);
            }
            if let Some(lines) = console_lines(&s) {
                for l in lines.iter().take(30) {
                    println!("  console: {}", l.chars().take(200).collect::<String>());
                }
            }
        }
    }
    Ok(())
}

/// Press `label`: a viewer bar button by its label, or `css:<selector>` (any element: a tab, a
/// chip, a card, a picker version).
fn press(s: &Session, label: &str) -> Result<()> {
    match label.strip_prefix("css:") {
        Some(css) => {
            s.script(&format!("const e = document.querySelector('{css}'); if (e) e.scrollIntoView({{block: 'center'}}); return true;"))?;
            click(s, css)
        }
        None => {
            // a speed: pick it in the speed dropdown (a select, not a button)
            let picked = s.script(&format!(
                "const s = document.querySelector('select[aria-label=\"Speed\"]'); if (!s) return false; const o = [...s.options].find(o => o.text === '{label}'); if (!o) return false; s.value = o.value; s.dispatchEvent(new Event('change', {{bubbles: true}})); return true;"
            ))?;
            if picked.as_bool() == Some(true) {
                return Ok(());
            }
            click_button(s, label)
        }
    }
}

/// Read pages the way a visitor does (`site smoke --engine webkit --page <path> --full`): each
/// page from its top, one window-full at a time (at most `screens`, 10% overlap), once it settled
/// (loaded, its first 3D view ready or failed, then `settle` ms), saved as
/// `<page>-<desktop|phone>-s<k>.png` in $MCTAS_WEBKIT_SHOTS (default /tmp/mctas-webkit-shots).
/// Prints each page's title and height, and whatever makes it wider than the window (a phone
/// page that scrolls sideways). `click`: pressed first, in order (`press`). `only`: `phone` or
/// `desktop`.
pub fn look(dir: &Path, remote: Option<&str>, pages: &[String], screens: u32, settle: u64, click: &[String], only: Option<&str>) -> Result<()> {
    let r = rig(dir, remote)?;
    let out = std::env::var_os("MCTAS_WEBKIT_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp/mctas-webkit-shots"));
    std::fs::create_dir_all(&out)?;
    for phone in [false, true] {
        let tag = if phone { "phone" } else { "desktop" };
        if only.is_some_and(|o| o != tag) {
            continue;
        }
        let s = Session::start(&r.display, phone)?;
        for page in pages {
            if let Err(e) = look_page(&s, &r.base, page, tag, &out, screens.max(1), settle, click) {
                println!("{tag} {page}: {e:#}");
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn look_page(s: &Session, base: &str, page: &str, tag: &str, out: &Path, screens: u32, settle: u64, click: &[String]) -> Result<()> {
    s.post("url", serde_json::json!({"url": format!("{base}{page}")}))?;
    let settled = wait_true(s, "if (document.readyState !== 'complete') return false; const v = document.querySelector('[data-mcview]'); return !v || /mcview-(ready|error)/.test(v.className);", 60)?;
    std::thread::sleep(Duration::from_millis(settle));
    for label in click {
        match press(s, label) {
            Ok(()) => println!("  pressed {label}"),
            Err(e) => println!("  could not press {label}: {e:#}"),
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    let info = s.script(
        "window.scrollTo(0, 0); const w = window.innerWidth, se = document.scrollingElement; const wide = se.scrollWidth > w + 1 ? [...document.querySelectorAll('body *')].filter(e => { const r = e.getBoundingClientRect(); return r.width > 0 && r.right > w + 1; }).slice(0, 6).map(e => e.tagName.toLowerCase() + (e.id ? '#' + e.id : '') + (typeof e.className === 'string' && e.className.trim() ? '.' + e.className.trim().split(/\\s+/).join('.') : '') + ' to ' + Math.round(e.getBoundingClientRect().right) + ' px') : []; const v = document.querySelector('[data-mcview]'); return [se.scrollHeight, window.innerHeight, se.scrollWidth, w, document.title, wide, v ? v.className : ''];",
    )?;
    let num = |i: usize| info[i].as_f64().unwrap_or(0.0);
    let (height, h, sw, w) = (num(0), num(1).max(1.0), num(2), num(3));
    let wide: Vec<String> = info[5].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    let view = info[6].as_str().unwrap_or("");
    println!(
        "{tag} {page}: \"{}\", {height:.0} px tall ({:.1} windows of {h:.0}){}{}{}",
        info[4].as_str().unwrap_or(""),
        height / h,
        if settled { "" } else { ", NOT settled after 60 s" },
        if view.is_empty() { String::new() } else { format!(", first 3D view: {view}") },
        if sw > w + 1.0 { format!(", SCROLLS SIDEWAYS ({sw:.0} px wide in a {w:.0} px window: {})", wide.join(", ")) } else { String::new() }
    );
    let stem = page.trim_end_matches(".html").replace(['/', '#', '?', '&', '=', ' ', '%'], "_");
    let stem = if stem.is_empty() { "home".to_string() } else { stem };
    let mut y = 0.0f64;
    for k in 1..=screens {
        s.script(&format!("window.scrollTo(0, {y}); return true;"))?;
        std::thread::sleep(Duration::from_millis(900));
        let v = http(s.port, "GET", &format!("/session/{}/screenshot", s.id), None)?;
        let png = out.join(format!("{stem}-{tag}-s{k}.png"));
        if let Some(b64) = v["value"].as_str() {
            use base64::Engine as _;
            std::fs::write(&png, base64::engine::general_purpose::STANDARD.decode(b64)?)?;
        }
        let now = s.script("return [window.scrollY, document.scrollingElement.scrollHeight];")?;
        let (at, max_y) = (now[0].as_f64().unwrap_or(y), (now[1].as_f64().unwrap_or(height) - h).max(0.0));
        println!("  {} (at {at:.0} px of {:.0})", png.display(), max_y + h);
        if at >= max_y - 1.0 {
            break;
        }
        y = (at + h * 0.9).min(max_y);
    }
    Ok(())
}

/// Every group page: the index's tabs and each study's cards ([`check_responsive`]).
fn group_pages(site: &site_model::Site) -> Vec<String> {
    let mut groups: Vec<String> = ["index.html#kind=door", "index.html#kind=clock", "index.html#kind=circuit"].iter().map(|s| s.to_string()).collect();
    let mut studies: Vec<String> = site.builds.iter().filter_map(|b| b.circuit.as_ref().map(|c| c.study.clone())).filter(|s| !s.is_empty()).collect();
    studies.sort();
    studies.dedup();
    groups.extend(studies.iter().map(|s| format!("circuits/{s}/index.html")));
    groups
}

/// Just the group pages' responsiveness (`mctas site smoke --engine webkit --responsive`), on a
/// desktop window and a phone one (`only`: one of them). Returns the number of failed checks.
pub fn responsive(dir: &Path, remote: Option<&str>, only: Option<&str>) -> Result<usize> {
    let r = rig(dir, remote)?;
    let groups = group_pages(&r.site);
    let mut fails = 0;
    for phone in [false, true] {
        if only.is_some_and(|o| o != if phone { "phone" } else { "desktop" }) {
            continue;
        }
        let label = if phone { "phone 390x844 @2x, iPhone UA" } else { "desktop 1280x900" };
        // (a fresh browser for each page: a frozen page keeps its browser busy, and the next
        // page's check would fail with it)
        for page in &groups {
            let s = Session::start(&r.display, phone)?;
            match check_responsive(&s, &r.base, page) {
                Ok(d) => println!("PASS webkit {label}: group page responsive ({page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: group page responsive ({page}): {e:#}");
                    fails += 1;
                }
            }
        }
    }
    Ok(fails)
}

/// Run the WebKit smoke on a generated site folder, or on the deployed site at `remote` (its
/// pages and bundles; the pages to check come from its own data/site.json). Returns the number of
/// failed checks.
pub fn smoke(dir: &Path, remote: Option<&str>) -> Result<usize> {
    let r = rig(dir, remote)?;
    let (site, base, display) = (&r.site, r.base.clone(), r.display.clone());
    let pick = |kind: &str| site.builds.iter().find(|b| b.kind == kind && b.trace.is_none()).map(|b| b.page.clone());
    let mut pages: Vec<(String, String, &str)> = vec![("index".into(), "index.html#kind=clock".into(), ".thumb[data-mcview]")];
    for kind in ["door", "clock", "circuit"] {
        if let Some(p) = pick(kind) {
            pages.push((kind.to_string(), p, "[data-mcview=\"full\"]"));
        }
    }
    if let Some(c) = site.cards.first() {
        pages.push(("family".into(), c.page.clone(), "[data-mcview=\"full\"]"));
    }
    // entity clocks: the viewer fetches the entity and item atlases the first time it draws one
    let mut entity = 0;
    for name in ["clk-misc-p412-oak_boat-oak_pressure_plate-stone-water-82b-e58845", "clk-misc-cart-loop3"] {
        if let Some(b) = site.builds.iter().find(|b| b.name == name) {
            pages.push((format!("entity clock {name}"), b.page.clone(), "[data-mcview=\"full\"]"));
            entity += 1;
        }
    }
    // the machines' studies: TNT and dead coral fans, BUD switches, and the shulker machines that
    // run live with the judge's chests (a loader, an unloader, the combo with its mode lever, and
    // the fastest of each: dropper choices, hoppers choosing among minecarts)
    let combo = |c: &site_model::Circuit| c.metrics.iter().any(|(k, v)| k == "badges" && v.split('+').any(|x| x == "combo"));
    let items_s = |b: &site_model::Build| -> i64 { b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(k, _)| k == "items/s")).and_then(|(_, v)| v.split_whitespace().next()?.parse::<f64>().ok()).map(|v| (v * 1000.0) as i64).unwrap_or(0) };
    let fastest = |loader: bool| -> Option<&site_model::Build> {
        site.builds
            .iter()
            .filter(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && c.kind.starts_with(if loader { "shulker-loader" } else { "shulker-unloader" })))
            .max_by_key(|b| (items_s(b), std::cmp::Reverse(b.blocks)))
    };
    let machines: [(&str, Box<dyn Fn(&site_model::Circuit) -> bool>); 5] = [
        ("tnt", Box::new(|c: &site_model::Circuit| c.study == "tnt")),
        ("bud", Box::new(|c: &site_model::Circuit| c.study == "bud")),
        ("shulker loader", Box::new(move |c: &site_model::Circuit| c.kind.starts_with("shulker-loader") && !combo(c))),
        ("shulker unloader", Box::new(|c: &site_model::Circuit| c.kind.starts_with("shulker-unloader"))),
        ("shulker combo", Box::new(move |c: &site_model::Circuit| combo(c))),
    ];
    for (what, pick) in &machines {
        if let Some(b) = site.builds.iter().find(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && pick(c))) {
            pages.push((format!("{what} {}", b.name), b.page.clone(), "[data-mcview=\"full\"]"));
        }
    }
    for (what, loader) in [("fastest shulker loader", true), ("fastest shulker unloader", false)] {
        if let Some(b) = fastest(loader) {
            if !pages.iter().any(|p| p.1 == b.page) {
                pages.push((format!("{what} {}", b.name), b.page.clone(), "[data-mcview=\"full\"]"));
            }
        }
    }
    // study 11: a divider (its demo is a running clock) and a monostable (pulse pairs), else a
    // burst
    let first_of = |kind: &str| site.builds.iter().find(|b| b.circuit.as_ref().is_some_and(|c| c.study == "pulsemult" && c.host.is_none() && c.layout_of.is_none() && c.kind.split_whitespace().next() == Some(kind)));
    for (what, b) in [("study 11 divider", first_of("divider")), ("study 11 timer", first_of("monostable").or_else(|| first_of("burst")))] {
        if let Some(b) = b {
            pages.push((format!("{what} {}", b.name), b.page.clone(), "[data-mcview=\"full\"]"));
        }
    }
    if entity == 0 {
        if let Some(b) = site.builds.iter().find(|b| b.kind == "clock" && b.trace.is_some()) {
            pages.push((format!("entity clock {}", b.name), b.page.clone(), "[data-mcview=\"full\"]"));
        }
    }
    let mut fails = 0;
    // a tunable group with two versions (the in-place swap check): a `kind:` group renders a
    // table of sizes, not a `nav.picker`, so it has no picker to swap
    let group_pair = site.builds.iter().find_map(|b| {
        let c = b.circuit.as_ref()?;
        if c.tunable.is_empty() || c.tunable.starts_with("kind:") || c.host.is_some() || c.layout_of.is_some() {
            return None;
        }
        let other = site.builds.iter().find(|o| o.id != b.id && o.circuit.as_ref().is_some_and(|oc| oc.tunable == c.tunable && oc.host.is_none() && oc.layout_of.is_none()))?;
        let file = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
        Some((b.page.clone(), file(&other.page), file(&other.design), crate::pages::page_title(other).replace('\'', "\\'")))
    });
    let family_page = site.cards.first().map(|c| c.page.clone());
    let groups = group_pages(site);
    for phone in [false, true] {
        let label = if phone { "phone 390x844 @2x, iPhone UA" } else { "desktop 1280x900" };
        let s = match Session::start(&display, phone) {
            Ok(s) => s,
            Err(e) => {
                println!("FAIL webkit {label}: no browser session: {e:#}");
                fails += 1;
                continue;
            }
        };
        let mut assets: Vec<String> = Vec::new();
        for (what, page, selector) in &pages {
            match check_page(&s, &base, page, selector) {
                Ok(d) => println!("PASS webkit {label}: {what} ({page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: {what} ({page}): {e:#}");
                    fails += 1;
                }
            }
            assets.extend(fetched_assets(&s));
        }
        assets.sort();
        assets.dedup();
        println!("     webkit {label}: game assets fetched (vendored textures): {}", if assets.is_empty() { "none".to_string() } else { assets.join(", ") });
        if let Some((page, other_page, other_design, other_name)) = &group_pair {
            match check_picker_swap(&s, &base, page, other_page, other_design, other_name) {
                Ok(d) => println!("PASS webkit {label}: picker swap in place ({page} -> {other_page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: picker swap in place ({page} -> {other_page}): {e:#}");
                    fails += 1;
                }
            }
        }
        if let Some(page) = &family_page {
            match check_family_swap(&s, &base, page) {
                Ok(d) => println!("PASS webkit {label}: family member swap ({page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: family member swap ({page}): {e:#}");
                    fails += 1;
                }
            }
        }
        for page in &groups {
            match check_responsive(&s, &base, page) {
                Ok(d) => println!("PASS webkit {label}: group page responsive ({page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: group page responsive ({page}): {e:#}");
                    fails += 1;
                }
            }
        }
    }
    Ok(fails)
}

#[cfg(test)]
mod tests {
    use super::console_flags;

    #[test]
    fn webkitgtk_media_controls_are_named_not_failed() {
        let lines = vec![
            "uncaught: TypeError: undefined is not a constructor (evaluating 'new Intl.DurationFormat(...)')".to_string(),
            "uncaught: TypeError: undefined is not an object (evaluating 'this._formatter.format')".to_string(),
            "viewer ready".to_string(),
        ];
        let (flagged, fail) = console_flags(&lines);
        assert!(!fail, "{flagged:?}");
        assert_eq!(flagged.len(), 1, "{flagged:?}");
        assert!(flagged[0].starts_with("WebKitGTK's own media controls"), "{flagged:?}");
        // the page's own uncaught errors still fail
        let (_, fail) = console_flags(&["uncaught: ReferenceError: x is not defined".to_string()]);
        assert!(fail);
    }
}
