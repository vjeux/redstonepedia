//! `mctas site serve` (a static file server for previewing a generated site) and `mctas site
//! smoke` (the browser check without a person: serve the site on a free local port, load pages
//! in a headless Chrome with `--dump-dom`, and check the DOM the WebAssembly UI built: result
//! cards, the clock time search expanding families, facet filters, build pages).

use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    }
}

fn respond(mut s: TcpStream, root: &Path, extra: &std::collections::HashMap<String, String>, inject: &str) -> Result<()> {
    let mut reader = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    // drain the headers
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
    }
    let raw = line.split_whitespace().nth(1).unwrap_or("/");
    let path = raw.split(['?', '#']).next().unwrap_or("/");
    let path = site_model::decode(path);
    // `MCTAS_RIG_LATENCY_MS`: every answer that many ms late (a phone's round trip to the site,
    // for timing what a page waits on the network for; unset: at once)
    if let Some(ms) = std::env::var("MCTAS_RIG_LATENCY_MS").ok().and_then(|v| v.trim().parse::<u64>().ok()) {
        std::thread::sleep(std::time::Duration::from_millis(ms.min(10_000)));
    }
    // `/__slow?ms=N`: answers after N real milliseconds (a pending request keeps a headless
    // Chrome's virtual clock from running out while a WebAssembly bundle compiles)
    if path == "/__slow" {
        let ms: u64 = raw.split("ms=").nth(1).and_then(|v| v.split('&').next()).and_then(|v| v.parse().ok()).unwrap_or(1000);
        std::thread::sleep(std::time::Duration::from_millis(ms.min(120_000)));
        write!(s, "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
        return Ok(());
    }
    if let Some(page) = extra.get(&path) {
        write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", page.len())?;
        s.write_all(page.as_bytes())?;
        return Ok(());
    }
    let rel = path.trim_start_matches('/');
    // (GitHub Pages serves the site under /redstonepedia/: the 404 page's absolute links work here
    // too)
    let rel = rel.strip_prefix(crate::deploy::SITE_PATH.trim_start_matches('/')).unwrap_or(rel);
    let mut file = root.join(rel);
    if rel.split('/').any(|c| c == "..") {
        file = root.join("__forbidden__");
    }
    if file.is_dir() {
        file = file.join("index.html");
    }
    match std::fs::read(&file) {
        Ok(mut body) => {
            if !inject.is_empty() && content_type(&file).starts_with("text/html") {
                let text = String::from_utf8_lossy(&body).to_string();
                body = match text.find("<head>") {
                    Some(i) => format!("{}{inject}{}", &text[..i + 6], &text[i + 6..]),
                    None => format!("{inject}{text}"),
                }
                .into_bytes();
            }
            write!(s, "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", content_type(&file), body.len())?;
            s.write_all(&body)?;
        }
        Err(_) => {
            let body = std::fs::read(root.join("404.html")).unwrap_or_else(|_| b"not found".to_vec());
            write!(s, "HTTP/1.1 404 Not Found\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())?;
            s.write_all(&body)?;
        }
    }
    Ok(())
}

/// Serve `root` on `listener` forever (one thread per connection); `extra` pages are served at
/// their paths too (the smoke test's probe page).
pub fn serve_on(listener: TcpListener, root: PathBuf) {
    serve_with(listener, root, Default::default())
}

pub fn serve_with(listener: TcpListener, root: PathBuf, extra: std::collections::HashMap<String, String>) {
    serve_injecting(listener, root, extra, String::new())
}

/// [`serve_with`], with `inject` inserted right after `<head>` in every HTML file served (the
/// WebKit smoke's console recorder: it must run before the page's own scripts).
pub fn serve_injecting(listener: TcpListener, root: PathBuf, extra: std::collections::HashMap<String, String>, inject: String) {
    let extra = std::sync::Arc::new(extra);
    let inject = std::sync::Arc::new(inject);
    for s in listener.incoming().flatten() {
        let root = root.clone();
        let extra = extra.clone();
        let inject = inject.clone();
        std::thread::spawn(move || {
            let _ = respond(s, &root, &extra, &inject);
        });
    }
}

pub fn serve(dir: &Path, port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).with_context(|| format!("binding 127.0.0.1:{port}"))?;
    println!("serving {} at http://127.0.0.1:{}/", dir.display(), listener.local_addr()?.port());
    serve_on(listener, dir.to_path_buf());
    Ok(())
}

/// The headless Chrome to use: the given path, $CHROME, else Chrome for Testing's headless shell
/// unpacked in ~/tools.
fn chrome(given: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = given {
        return Ok(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("CHROME") {
        return Ok(PathBuf::from(p));
    }
    for p in [chrome_cache_dir().join("chrome-headless-shell-linux64/chrome-headless-shell"), PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("tools/chrome-headless-shell-linux64/chrome-headless-shell")] {
        if p.is_file() {
            return Ok(p);
        }
    }
    bail!("no headless Chrome: run `mctas site chrome` (downloads Chrome for Testing's chrome-headless-shell into ~/.cache/mctas), pass --chrome, or set CHROME")
}

/// Where `mctas site chrome` puts the headless Chrome.
fn chrome_cache_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache/mctas")
}

/// Download Chrome for Testing's chrome-headless-shell (linux64, the stable channel) into
/// ~/.cache/mctas, for `site smoke` and `site publish`. Returns the binary.
pub fn install_chrome() -> Result<PathBuf> {
    let dir = chrome_cache_dir();
    std::fs::create_dir_all(&dir)?;
    let json = Command::new("curl").args(["-fsSL", "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json"]).output().context("running curl")?;
    if !json.status.success() {
        bail!("downloading the Chrome for Testing index failed: {}", String::from_utf8_lossy(&json.stderr).trim());
    }
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).context("the Chrome for Testing index")?;
    let url = v["channels"]["Stable"]["downloads"]["chrome-headless-shell"]
        .as_array()
        .and_then(|a| a.iter().find(|d| d["platform"] == "linux64"))
        .and_then(|d| d["url"].as_str())
        .context("no linux64 chrome-headless-shell in the Chrome for Testing index")?
        .to_string();
    let zip = dir.join("chrome-headless-shell-linux64.zip");
    println!("downloading {url}");
    let st = Command::new("curl").args(["-fsSL", "-o"]).arg(&zip).arg(&url).status().context("running curl")?;
    if !st.success() {
        bail!("downloading {url} failed");
    }
    let _ = std::fs::remove_dir_all(dir.join("chrome-headless-shell-linux64"));
    let st = Command::new("unzip").args(["-q", "-o"]).arg(&zip).arg("-d").arg(&dir).status().context("running unzip")?;
    if !st.success() {
        bail!("unzipping {} failed", zip.display());
    }
    let _ = std::fs::remove_file(&zip);
    let bin = dir.join("chrome-headless-shell-linux64/chrome-headless-shell");
    let o = Command::new(&bin).arg("--version").output().with_context(|| format!("running {}", bin.display()))?;
    println!("{}: {}", bin.display(), String::from_utf8_lossy(&o.stdout).trim());
    Ok(bin)
}

/// The DOM of `url` after its scripts ran (`--dump-dom` with a virtual time budget).
pub fn dump_dom(chrome: &Path, url: &str, budget_ms: u32) -> Result<String> {
    let o = Command::new(chrome)
        .args(["--headless", "--no-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--hide-scrollbars", "--window-size=1400,1000"])
        .arg(format!("--virtual-time-budget={budget_ms}"))
        .arg("--dump-dom")
        .arg(url)
        .output()
        .with_context(|| format!("running {}", chrome.display()))?;
    if !o.status.success() {
        bail!("chrome failed on {url}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A screenshot of `url` (PNG bytes): unlike `--dump-dom`, it produces frames, so WebGL and
/// requestAnimationFrame run (WebGL2 in software: SwiftShader).
pub fn screenshot(chrome: &Path, url: &str, budget_ms: u32, w: u32, h: u32) -> Result<Vec<u8>> {
    let file = std::env::temp_dir().join(format!("mctas-site-shot-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let o = Command::new(chrome)
        .args(["--headless", "--no-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--hide-scrollbars"])
        .arg(format!("--window-size={w},{h}"))
        .arg(format!("--virtual-time-budget={budget_ms}"))
        .arg(format!("--screenshot={}", file.display()))
        .arg(url)
        .output()
        .with_context(|| format!("running {}", chrome.display()))?;
    if !o.status.success() {
        bail!("chrome failed on {url}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""));
    }
    let png = std::fs::read(&file).with_context(|| format!("no screenshot of {url}"))?;
    let _ = std::fs::remove_file(&file);
    Ok(png)
}

/// Decode an 8-bit RGB or RGBA, non-interlaced PNG (what Chrome writes) into RGB pixels.
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<[u8; 3]>)> {
    use std::io::Read;
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        bail!("not a PNG");
    }
    let (mut w, mut h, mut channels) = (0u32, 0u32, 0usize);
    let mut idat = Vec::new();
    let mut i = 8;
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into()?) as usize;
        let kind = &bytes[i + 4..i + 8];
        let data = bytes.get(i + 8..i + 8 + len).context("truncated PNG chunk")?;
        match kind {
            b"IHDR" => {
                w = u32::from_be_bytes(data[0..4].try_into()?);
                h = u32::from_be_bytes(data[4..8].try_into()?);
                let (depth, color, interlace) = (data[8], data[9], data[12]);
                if depth != 8 || interlace != 0 {
                    bail!("PNG bit depth {depth} / interlace {interlace} not supported");
                }
                channels = match color {
                    2 => 3,
                    6 => 4,
                    c => bail!("PNG color type {c} not supported"),
                };
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        i += 12 + len;
    }
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&idat[..]).read_to_end(&mut raw)?;
    let stride = w as usize * channels;
    if raw.len() < (stride + 1) * h as usize {
        bail!("PNG data too short");
    }
    let mut prev = vec![0u8; stride];
    let mut cur = vec![0u8; stride];
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        let row = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let filter = row[0];
        for x in 0..stride {
            let a = if x >= channels { cur[x - channels] as i32 } else { 0 };
            let b = prev[x] as i32;
            let c = if x >= channels { prev[x - channels] as i32 } else { 0 };
            let pred = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
                }
                f => bail!("PNG filter {f}"),
            };
            cur[x] = (row[1 + x] as i32 + pred) as u8;
        }
        for x in 0..w as usize {
            px.push([cur[x * channels], cur[x * channels + 1], cur[x * channels + 2]]);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    Ok((w, h, px))
}

/// The probe page of the viewer check: one design filling the window as a thumbnail, over a
/// flat fallback with no text (one color), so any rendering shows as many colors; a slow request
/// holds the page open while the viewer compiles and draws.
fn probe_page(root: &str, design: &str, kind: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"en\" data-mcview-textures=\"{root}textures/26.3\"><head><meta charset=\"utf-8\"><title>probe</title><style>html,body{{margin:0;background:#808080}}#v{{position:fixed;inset:0}}.fallback{{position:absolute;inset:0;background:#808080}}.mcview-ready>.fallback{{display:none}}img{{position:fixed;width:1px;height:1px}}</style></head><body><div id=\"v\" data-mcview=\"thumb\" data-src=\"{root}{}\" data-kind=\"{kind}\"><div class=\"fallback\"></div></div><img src=\"__slow?ms=15000\" alt=\"\"><script type=\"module\">import init from \"{}viewer/mcview.js\"; init();</script></body></html>",
        site_model::html::esc(design),
        if root.is_empty() { "./" } else { root }
    )
}

/// The number in `<span id="f-count"><strong>N</strong>`.
/// GET a path from the local server: the body (status 200 only).
fn http_get_local(port: u16, path: &str) -> Result<Vec<u8>> {
    let mut s = TcpStream::connect(("127.0.0.1", port))?;
    write!(s, "GET /{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path.trim_start_matches('/'))?;
    let mut all = Vec::new();
    std::io::Read::read_to_end(&mut s, &mut all)?;
    let head_end = all.windows(4).position(|w| w == b"\r\n\r\n").context("no HTTP header")?;
    let head = String::from_utf8_lossy(&all[..head_end]).to_string();
    if !head.starts_with("HTTP/1.1 200") {
        bail!("GET /{path}: {}", head.lines().next().unwrap_or(""));
    }
    Ok(all[head_end + 4..].to_vec())
}

fn result_count(dom: &str) -> Option<usize> {
    let i = dom.find("id=\"f-count\"")?;
    let rest = &dom[i..];
    let s = rest.find("<strong>")? + 8;
    let e = rest[s..].find("</strong>")? + s;
    rest[s..e].trim().parse().ok()
}

/// Smoke-test a generated site in a headless browser. Returns the number of failed checks.
/// `remote`: check the deployed site at that URL instead (the pages and the bundles there; the
/// expected answers still come from `dir`'s data, the build that was deployed).
pub fn smoke(dir: &Path, chrome_path: Option<&Path>, budget_ms: u32, remote: Option<&str>) -> Result<usize> {
    let chrome = chrome(chrome_path)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let root = dir.to_path_buf();
    let site: site_model::Site = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json"))?)?;
    let remote = remote.map(|r| if r.ends_with('/') { r.to_string() } else { format!("{r}/") });
    // the viewer probes (served locally, loading the site's bundle and designs): one door, one
    // clock, each filling the window
    let mut extra = std::collections::HashMap::new();
    let probe_root = remote.clone().unwrap_or_default();
    let probes: Vec<(String, String)> = ["door", "clock"]
        .iter()
        .filter_map(|k| site.builds.iter().find(|b| b.kind == *k).map(|b| (format!("/__probe-{k}.html"), probe_page(&probe_root, &b.design, k))))
        .collect();
    for (p, html) in &probes {
        extra.insert(p.clone(), html.clone());
    }
    std::thread::spawn(move || serve_with(listener, root, extra));
    let local = format!("http://127.0.0.1:{port}/");
    let base = remote.clone().unwrap_or_else(|| local.clone());
    let mut fails = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{} {name}: {detail}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            fails += 1;
        }
    };
    // static checks (the files of the build): nothing community-derived is published (vjeux
    // 01:15Z), and the circuit pages explain themselves (vjeux 01:05Z)
    let community: Vec<&str> = site
        .builds
        .iter()
        .filter(|b| b.kind == "door")
        .filter(|b| crate::doors::community_seed(&b.name, &std::fs::read_to_string(dir.join(&b.design)).unwrap_or_default()).is_some())
        .map(|b| b.name.as_str())
        .collect();
    let stray = std::fs::read_dir(dir.join("designs/doors")).map(|rd| rd.flatten().filter(|e| crate::doors::community_seed(&e.file_name().to_string_lossy(), "").is_some()).count()).unwrap_or(0);
    check("no community door published (pages, data, design files)", community.is_empty() && stray == 0, format!("{} builds, {stray} files{}", community.len(), community.first().map(|n| format!(", e.g. {n}")).unwrap_or_default()));
    // every page ever published still answers (crates/site/published.tsv, the coordinator
    // 2026-10-21 11:05Z): a page, or a redirect to a page; a page vjeux deleted stays deleted
    let listed = crate::published::read(&crate::published::repo_root().join(crate::published::FILE));
    let (resolved, unresolved) = crate::published::check(dir, &listed);
    check(
        "every page ever published resolves (a page, or a redirect to one)",
        unresolved.is_empty() && !listed.is_empty(),
        format!("{resolved} of {} listed paths{}", listed.len(), if unresolved.is_empty() { String::new() } else { format!("; {} do not: {}", unresolved.len(), unresolved.iter().take(5).cloned().collect::<Vec<_>>().join(", ")) }),
    );
    // one card per mechanism and period (the coordinator 11:34Z): no period of the Clocks tab
    // lists two cards of one mechanism (crates/site/src/mechanism.rs: their designs as written or
    // their pictures at their still frames, swappable blocks aside)
    {
        let cards: Vec<site_model::Build> = site.builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.host.is_none())).cloned().collect();
        let keys: Vec<Vec<String>> = crate::circuits::par_map(&cards, |b| {
            let text = std::fs::read_to_string(dir.join(&b.design)).unwrap_or_default();
            let still: crate::simulate::Still = crate::cache::get_or(crate::cache::Domain::View, &[b"still", text.as_bytes()], || crate::simulate::still(&text));
            crate::mechanism::keys_of(&still, &text)
        });
        let twins = crate::mechanism::folds(&cards, &keys);
        let folded = site.builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.host.is_some())).count();
        let family = cards.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.family.is_some() || c.n.is_some())).count();
        check(
            "the Clocks tab: one card per mechanism and period, family members included",
            twins.is_empty() && folded > 0 && family > 0,
            if twins.is_empty() { format!("{} cards ({family} of a family), {folded} clocks folded under them", cards.len()) } else { twins.iter().take(5).map(|(a, h)| format!("{} = {}", cards[*a].name, cards[*h].name)).collect::<Vec<_>>().join(", ") },
        );
    }
    // one card per door mechanism (vjeux 13:10Z, "What about doors?"): no two door cards (board
    // leaders, the records table's rows) are one mechanism with a swapped block
    {
        let cards: Vec<&site_model::Build> = site.builds.iter().filter(|b| b.door.as_ref().is_some_and(|d| !d.leads.is_empty() && d.host.is_none())).collect();
        let keys: Vec<Vec<String>> = crate::circuits::par_map(&cards, |b| {
            let text = std::fs::read_to_string(dir.join(&b.design)).unwrap_or_default();
            let still: crate::simulate::Still = crate::cache::get_or(crate::cache::Domain::View, &[b"still", text.as_bytes()], || crate::simulate::still(&text));
            crate::mechanism::keys_of(&still, &text)
        });
        let mut seen: std::collections::BTreeMap<&String, usize> = std::collections::BTreeMap::new();
        let mut same: Vec<String> = Vec::new();
        for (i, ks) in keys.iter().enumerate() {
            for k in ks {
                match seen.get(k) {
                    Some(&j) if j != i => same.push(format!("{} = {}", cards[i].name, cards[j].name)),
                    _ => {
                        seen.insert(k, i);
                    }
                }
            }
        }
        same.dedup();
        check(
            "the Doors tab: one card per door mechanism",
            same.is_empty() && cards.len() > 1 && keys.iter().all(|k| !k.is_empty()),
            if same.is_empty() { format!("{} door cards, every one its own mechanism", cards.len()) } else { same.iter().take(5).cloned().collect::<Vec<_>>().join(", ") },
        );
    }
    // a clock that is hard to wire into a circuit (the judge's `usable`: `no: ...`): its page warns
    // (never hidden), its card says so, and on the Clocks tab it comes after the easy ones of its
    // period (the coordinator 2026-10-21)
    {
        let hard = |b: &site_model::Build| b.clock.as_ref().is_some_and(|c| c.usable.starts_with("no"));
        let unwarned: Vec<String> = site
            .builds
            .iter()
            .filter(|b| hard(b))
            .filter(|b| !std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default().contains(&site_model::html::esc(crate::plain::USABLE_WARNING)))
            .map(|b| b.name.clone())
            .collect();
        let hits = site.run(&site_model::Query::from_fragment("kind=clock")).map(|(h, _)| h).unwrap_or_default();
        let mut early: Vec<String> = Vec::new();
        for w in hits.windows(2) {
            if let (site_model::Hit::Build(a), site_model::Hit::Build(b)) = (w[0], w[1]) {
                let (a, b) = (&site.builds[a], &site.builds[b]);
                let same = a.clock.as_ref().map(|c| c.every()) == b.clock.as_ref().map(|c| c.every());
                if same && hard(a) && !hard(b) {
                    early.push(format!("{} before {}", a.name, b.name));
                }
            }
        }
        let n = site.builds.iter().filter(|b| hard(b)).count();
        check(
            "clocks hard to wire into a circuit: warned on their pages, after the easy ones of their period",
            unwarned.is_empty() && early.is_empty(),
            if unwarned.is_empty() && early.is_empty() {
                format!("{n} clocks warned")
            } else {
                format!("unwarned: {}; out of order: {}", unwarned.iter().take(3).cloned().collect::<Vec<_>>().join(", "), early.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
            },
        );
    }
    if let Some(b) = site.builds.iter().find(|b| b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("counter "))) {
        let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let ok = ["<p class=\"oneliner\">", "<h2>Try it</h2>", "<details class=\"more\">", "<h2>How to try it</h2>", "data-stimulus=\"", "data-input-mode=\"pulse\"", "<svg class=\"wave\"", "<h2>Measurements</h2>"].iter().all(|s| page.contains(s));
        check("circuit page explains itself: one plain sentence on top, try it under the view; the measurements in the details", ok, b.page.clone());
        // no "Why build it" line on any page (vjeux 23:52Z: "All these why build it have been
        // super useless. Please remove all of them")
        let with_why: Vec<String> = site
            .builds
            .iter()
            .map(|o| o.page.clone())
            .chain(crate::explain::studies().iter().map(|s| format!("circuits/{}/index.html", s.0)))
            .chain(["index.html".to_string(), "circuits/index.html".to_string()])
            .chain(site.cards.iter().map(|c| c.page.clone()))
            .filter(|p| std::fs::read_to_string(dir.join(p)).unwrap_or_default().contains("Why build it"))
            .collect();
        check("no page has a \"Why build it\" line", with_why.is_empty(), format!("{} pages{}", with_why.len(), with_why.first().map(|p| format!(", e.g. {p}")).unwrap_or_default()));
        // a view shown as built promises no play (the coordinator 2026-10-22: webrender's as-built
        // kinds still said "press play to watch our test"): a page whose viewer carries
        // data-asbuilt says the build only, nothing of a demo
        let promises = ["Press play", "press play", "on a loop", "runs in the view", "The view plants", "watch our test", "watch the judge"];
        let asbuilt: Vec<String> = site.builds.iter().map(|o| o.page.clone()).filter(|p| std::fs::read_to_string(dir.join(p)).unwrap_or_default().contains("data-asbuilt")).collect();
        let playing: Vec<&String> = asbuilt.iter().filter(|p| {
            let html = std::fs::read_to_string(dir.join(p)).unwrap_or_default();
            promises.iter().any(|w| html.contains(w)) || !html.contains("shows the build only")
        }).collect();
        check("as-built views: their pages say the build only, no demo", playing.is_empty(), format!("{} as-built pages{}", asbuilt.len(), playing.first().map(|p| format!(", e.g. {p}")).unwrap_or_default()));
        // no extendable family pages and no generated members (vjeux 00:33Z: "All these
        // variations are useless, remove them"): none written, none linked, none in the data
        let links_family = |html: &str| html.split("href=\"").skip(1).any(|s| s.split('"').next().is_some_and(|h| h.contains("families/")));
        let linking_families: Vec<String> = site
            .builds
            .iter()
            .map(|o| o.page.clone())
            .chain(crate::explain::studies().iter().map(|s| format!("circuits/{}/index.html", s.0)))
            .chain(["index.html".to_string(), "circuits/index.html".to_string()])
            .filter(|p| links_family(&std::fs::read_to_string(dir.join(p)).unwrap_or_default()))
            .collect();
        let written = std::fs::read_dir(dir.join("families")).map(|rd| rd.flatten().any(|e| !crate::published::is_redirect(&std::fs::read_to_string(e.path()).unwrap_or_default()))).unwrap_or(false) || dir.join("designs/circuits/members").exists();
        // (the verified clock members are clocks of their own now: no member list in the data;
        // one design file per extended clock: vjeux 01:24Z)
        let extended = site.builds.iter().filter(|b| crate::generate::is_member(b)).count();
        let member_files = std::fs::read_dir(dir.join("designs/clocks/members")).map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".txt")).count()).unwrap_or(0);
        check(
            "no family page, no member list; every member design file is an extended clock's",
            !written && linking_families.is_empty() && site.cards.is_empty() && site.members.is_empty() && member_files == extended,
            format!(
                "written: {written}, {} pages link one{}, {} cards, {} members, {member_files} member files for {extended} extended clocks",
                linking_families.len(),
                linking_families.first().map(|p| format!(" (e.g. {p})")).unwrap_or_default(),
                site.cards.len(),
                site.members.len()
            ),
        );
        // the redstone first (vjeux): the live view comes before the explanation (the heading's
        // two sentences aside)
        let first = page.find("data-mcview=\"full\"").zip(page.find("<h2>Try it</h2>")).is_some_and(|(v, w)| v < w);
        check("circuit page: the live 3D view before the explanation", first, b.page.clone());
    }
    let c = site.builds.iter().find(|b| b.kind == "clock" && b.page.ends_with("clk-obs-p4-obs-4b.html")).or_else(|| site.builds.iter().find(|b| b.kind == "clock"));
    if let Some(b) = c {
        let html = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        check("clock page: its output over time (start-up shaded, every period marked)", html.contains("<h2>Its output over time</h2>") && html.contains("class=\"wave fit\"") && html.contains("class=\"warm\""), b.page.clone());
    }
    // family pages: a title in words, no link to the deleted families index (a relative
    // `index.html` there resolves to it)
    let bad: Vec<String> = site
        .cards
        .iter()
        .filter(|c| {
            let html = std::fs::read_to_string(dir.join(&c.page)).unwrap_or_default();
            let h1 = html.split("<h1>").nth(1).and_then(|s| s.split("</h1>").next()).unwrap_or("");
            html.contains("href=\"index.html\"") || h1.contains(" = ") || h1.is_empty()
        })
        .map(|c| c.page.clone())
        .collect();
    check("family pages: a title in words, no dead link", bad.is_empty(), if bad.is_empty() { format!("{} pages", site.cards.len()) } else { bad.join(", ") });
    // the Doors tab opens on solid doors (sculk doors can be set off by footsteps: coordinator
    // 13:54Z)
    // the players' words find what they name, first (critique C12)
    for (text, want) in [("toggle", "Toggle switch"), ("one shot", "One-shot timer"), ("hidden door", "door"), ("double piston extender", "Piston extender"), ("timer", "One-shot timer")] {
        let q = site_model::Query::from_fragment(&format!("q={}", site_model::encode(text)));
        let first = site.run(&q).ok().and_then(|(h, _)| h.first().copied()).and_then(|h| match h {
            site_model::Hit::Build(i) => site.builds.get(i).map(|b| if b.kind == "door" { "door".to_string() } else { b.group.clone() }),
            _ => Some("family member".to_string()),
        });
        check(&format!("search \"{text}\": a match first{}", if want.is_empty() { String::new() } else { format!(", {want}") }), first.as_deref().is_some_and(|g| want.is_empty() || g.starts_with(want)), format!("first: {first:?}"));
    }
    // an exact gate function finds its own gate first (critique: "xor" led with OR and AND)
    for f in ["xor", "xnor", "nand"] {
        if !site.builds.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.kind == format!("gate {f}"))) {
            continue;
        }
        let q = site_model::Query::from_fragment(&format!("q={f}"));
        let first = site.run(&q).ok().and_then(|(h, _)| h.first().copied()).and_then(|h| match h {
            site_model::Hit::Build(i) => site.builds.get(i).and_then(|b| b.circuit.as_ref().map(|c| c.kind.clone())),
            _ => None,
        });
        check(&format!("search \"{f}\": its gate first"), first.as_deref() == Some(&format!("gate {f}")), format!("first: {first:?}"));
    }
    let dq = site_model::Query::from_fragment("kind=door");
    if let Ok((dhits, _)) = site.run(&dq) {
        let first: Vec<&str> = dhits.iter().take(3).filter_map(|h| match h {
            site_model::Hit::Build(i) => site.builds.get(*i).map(|b| b.group.as_str()),
            _ => None,
        }).collect();
        check("the Doors tab opens on solid doors", !first.is_empty() && first.iter().all(|g| *g == "solid"), format!("first: {first:?}"));
    }
    let study = std::fs::read_to_string(dir.join("circuits/memory/index.html")).unwrap_or_default();
    let plain_first = study.find(crate::explain::study_uses("memory")).zip(study.find("class=\"card")).is_some_and(|(u, c)| u < c);
    // (how we test and rank them is on no page a builder reads: vjeux 2026-11-08 10:10)
    check("study page: what you would use them for, then the designs; nothing on how we test", study.contains("<h1>Memory and counters</h1>") && plain_first && !study.contains("How we test"), "circuits/memory/index.html".into());
    // cards and pages lead with the properties (vjeux 12:09Z), kinds are base kinds (12:15Z): no
    // number of its own in a kind's name (`Counter 7`), a name's own digits aside (`7-segment
    // display decoder`)
    let numbered: Vec<&str> = site
        .builds
        .iter()
        .filter(|b| b.kind == "circuit" && (b.group.split_whitespace().any(|w| w.chars().all(|c| c.is_ascii_digit() || c == '.')) || b.group.starts_with(|c: char| c.is_lowercase())))
        .map(|b| b.group.as_str())
        .collect();
    check("circuit kinds are base kinds in words (no number)", numbered.is_empty(), if numbered.is_empty() { "all".into() } else { format!("{:?}", numbered.iter().take(5).collect::<Vec<_>>()) });
    let pext_page = std::fs::read_to_string(dir.join("circuits/pext/index.html")).unwrap_or_default();
    // one card per kind (vjeux 21:47Z): the piston extender (every reach on its page) and the sand
    // elevator, no design names
    let titles: Vec<&str> = pext_page.split("<div class=\"title\">").skip(1).filter_map(|s| s.split('<').next()).collect();
    check(
        "study cards: one card per kind, the property line leads, no design names",
        titles.len() == 2 && titles.iter().any(|t| t.replace('\u{a0}', " ").starts_with("reach 2 to ")) && !pext_page.contains("class=\"name\"") && !pext_page.contains(">pext-n"),
        format!("pext cards: {titles:?}"),
    );
    let mut named: Vec<String> = Vec::new();
    for b in site.builds.iter().filter(|b| b.kind == "circuit").take(30) {
        let html = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let details = html.find("<details class=\"more\">").unwrap_or(html.len());
        let head = &html[..details];
        if head.contains(&format!(">{}<", b.name)) || head.contains(&format!("({})", b.name)) || !html[details..].contains(&format!("<code>{}</code>", b.name)) {
            named.push(b.name.clone());
        }
    }
    check("design pages: the property line is the title, the design's name only in the details", named.is_empty(), if named.is_empty() { "30 pages".into() } else { named.join(", ") });
    // one design per kind (vjeux 21:47Z): no study lists two cards of one kind, and the machines'
    // variations tables are gone (the coordinator 21:55Z (F))
    let mut doubled: Vec<String> = Vec::new();
    for (study, _, _) in crate::explain::studies().iter() {
        let page = std::fs::read_to_string(dir.join(format!("circuits/{study}/index.html"))).unwrap_or_default();
        if page.contains("id=\"variations\"") {
            doubled.push(format!("{study}: a variations table"));
        }
    }
    let mut per_kind: std::collections::BTreeMap<(String, String), usize> = std::collections::BTreeMap::new();
    for b in site.builds.iter().filter(|b| b.circuit.is_some() && !b.flags.iter().any(|f| f == "size") && b.circuit.as_ref().is_some_and(|c| c.tunable.is_empty() || !c.span.is_empty())) {
        *per_kind.entry((b.circuit.as_ref().unwrap().study.clone(), crate::kinds::kind_key(b))).or_default() += 1;
    }
    doubled.extend(per_kind.iter().filter(|(_, n)| **n > 1).map(|((s, k), n)| format!("{s}/{k}: {n} cards")));
    let doors = site.builds.iter().filter(|b| b.door.is_some() && !b.flags.iter().any(|f| f == "size")).count();
    // (one door card per door size: the 3x3 records, the 2x2's, the coordinator 2026-10-21)
    let door_sizes = site.builds.iter().filter(|b| b.door.is_some()).map(site_model::door_side).collect::<std::collections::BTreeSet<u32>>().len();
    // (every clock is a card of its own under its period, vjeux 01:24Z, but one card per mechanism:
    // the coordinator 11:33Z; a folded clock keeps its page, listed on its card's)
    let clocks = site.builds.iter().filter(|b| b.clock.is_some()).count();
    let clock_cards = site.builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.host.is_none()) && !b.flags.iter().any(|f| f == "size")).count();
    let folded_clocks = site.builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| c.host.is_some())).count();
    check(
        "one card per kind: every study, one door card per door size, every clock a card (one per mechanism), no variations table",
        doubled.is_empty() && doors == door_sizes && clock_cards + folded_clocks == clocks && clocks >= 2,
        format!("{} circuit kinds, {doors} door cards for {door_sizes} door sizes, {clock_cards} clock cards for {clocks} clocks{}", per_kind.len(), if doubled.is_empty() { String::new() } else { format!("; {}", doubled.join(", ")) }),
    );
    // study 11: every kind explains itself
    if site.builds.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.study == "pulsemult")) {
        let mut bare: Vec<String> = Vec::new();
        for b in site.builds.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.study == "pulsemult")) {
            let html = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
            let what = html.split("<section class=\"what\">").nth(1).and_then(|s| s.split("</section>").next()).unwrap_or("");
            if what.matches("<b>").count() == 0 || what.contains("<p></p>") {
                bare.push(b.name.clone());
            }
        }
        check("study 11: every design page says what it does with its numbers", bare.is_empty(), if bare.is_empty() { "all".into() } else { bare.join(", ") });
    }
    // a parametric kind's page lists every size, one row each, its own marked (vjeux 21:47Z)
    if let Some(b) = site.builds.iter().find(|b| b.name == "pext-n2-rider7-flat").or_else(|| site.builds.iter().find(|b| b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("piston-extender")))) {
        let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let table = page.split("<table class=\"list kind-sizes\">").nth(1).and_then(|s| s.split("</table>").next()).unwrap_or("");
        let reaches: Vec<String> = table.split("<td>").skip(1).step_by(3).filter_map(|s| s.split("reach ").nth(1)?.split(|c: char| !c.is_ascii_digit()).next().map(String::from)).collect();
        let distinct: std::collections::BTreeSet<&String> = reaches.iter().collect();
        check(
            "circuit page: the kind's table lists every size once, this page's marked",
            reaches.len() >= 5 && distinct.len() == reaches.len() && table.contains("(this page)"),
            format!("{}: reaches {reaches:?}", b.page),
        );
    }
    // the judge's calibration / selftest designs are not published (vjeux 02:40Z): no page (a
    // redirect where one was published once: crates/site/published.tsv), no design file
    let calib = site.builds.iter().filter(|b| b.page.starts_with("circuits/calibration/") || b.page.starts_with("circuits/selftest/")).count();
    let real_pages = |d: &Path| std::fs::read_dir(d).map(|rd| rd.flatten().any(|e| !crate::published::is_redirect(&std::fs::read_to_string(e.path()).unwrap_or_default()))).unwrap_or(false);
    let calib_dir = real_pages(&dir.join("circuits/calibration")) || real_pages(&dir.join("circuits/selftest")) || dir.join("designs/circuits/calibration").exists();
    check("no calibration or selftest circuit published", calib == 0 && !calib_dir, format!("{calib} builds, folders: {calib_dir}"));
    // nothing crates/site/hidden.tsv keeps off the site (vjeux on the slime-free extender sweeps)
    let hidden: Vec<String> = site
        .builds
        .iter()
        .filter_map(|b| {
            let c = b.circuit.as_ref()?;
            crate::circuits::hidden(&c.study, &format!("{}.txt", b.name)).map(|_| b.name.clone())
        })
        .collect();
    let studies: std::collections::BTreeSet<String> = crate::circuits::hidden_rules().into_iter().map(|(s, _, _)| s).collect();
    let hidden_pages = studies
        .iter()
        .map(|s| {
            std::fs::read_dir(dir.join("circuits").join(s))
                .map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().strip_suffix(".html").is_some_and(|stem| crate::circuits::hidden(s, &format!("{stem}.txt")).is_some()) && !crate::published::is_redirect(&std::fs::read_to_string(e.path()).unwrap_or_default())).count())
                .unwrap_or(0)
        })
        .sum::<usize>();
    check("no hidden design published (crates/site/hidden.tsv)", hidden.is_empty() && hidden_pages == 0, if hidden.is_empty() { format!("{} rules, 0 builds, {hidden_pages} pages", crate::circuits::hidden_rules().len()) } else { hidden.join(", ") });
    // no simulator attribution on the pages (vjeux 02:01Z): design files (<pre>) and class names aside
    let mut attributed: Vec<String> = Vec::new();
    let sample: Vec<String> = ["index.html", "circuits/index.html"].iter().map(|s| s.to_string()).chain(crate::explain::studies().iter().map(|s| format!("circuits/{}/index.html", s.0))).chain(site.builds.iter().filter(|b| b.kind == "circuit").take(40).map(|b| b.page.clone())).chain(site.builds.iter().filter(|b| b.kind != "circuit").take(20).map(|b| b.page.clone())).chain(site.cards.iter().map(|c| c.page.clone())).collect();
    // the pages vjeux deleted (13:35Z) are neither written nor linked from anywhere
    let written: Vec<&str> = crate::generate::DELETED_PAGES.iter().copied().filter(|p| dir.join(p).exists()).collect();
    let linking: Vec<String> = sample
        .iter()
        .filter(|p| {
            let html = std::fs::read_to_string(dir.join(p)).unwrap_or_default();
            crate::generate::DELETED_PAGES.iter().any(|d| {
                // a link to the page itself (`"about.html"`, `../about.html#x`), not a longer name
                // that ends like it (`pulse-oneshot-fall.html`)
                html.match_indices(d).any(|(i, _)| {
                    let before = html[..i].chars().last();
                    let after = html[i + d.len()..].chars().next();
                    matches!(before, Some('/') | Some('"')) && matches!(after, Some('"') | Some('#'))
                })
            })
        })
        .cloned()
        .collect();
    check(
        "deleted pages (boards, families index, everything, about): none written, none linked",
        written.is_empty() && linking.is_empty(),
        format!("{} written{}, {} of {} pages link one{}", written.len(), written.first().map(|p| format!(" ({p})")).unwrap_or_default(), linking.len(), sample.len(), linking.first().map(|p| format!(", e.g. {p}")).unwrap_or_default()),
    );
    for p in &sample {
        let html = std::fs::read_to_string(dir.join(p)).unwrap_or_default();
        let mut visible = String::new();
        let mut rest = html.as_str();
        while let Some(i) = rest.find("<pre") {
            visible.push_str(&rest[..i]);
            rest = rest[i..].find("</pre>").map(|j| &rest[i + j..]).unwrap_or("");
        }
        visible.push_str(rest);
        if visible.replace("st-redsim", "").to_lowercase().contains("redsim") {
            attributed.push(p.clone());
        }
    }
    check("no simulator attribution on the pages", attributed.is_empty(), format!("{} of {} pages{}", attributed.len(), sample.len(), attributed.first().map(|p| format!(", e.g. {p}")).unwrap_or_default()));
    // the door card's page: every board's record of its door size; the clock card's: every period
    if let Some(b) = site.builds.iter().find(|b| b.door.is_some() && !b.flags.iter().any(|f| f == "size")) {
        let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let rows = page.split("id=\"boards\"").nth(1).and_then(|s| s.split("</table>").next()).map(|t| t.matches("<tr").count().saturating_sub(1)).unwrap_or(0);
        let records = site.builds.iter().filter(|o| o.door.is_some() && site_model::door_side(o) == site_model::door_side(b)).count();
        check("door card: its page lists every board's record", rows == records && records >= 2, format!("{}: {rows} rows, {records} records", b.page));
    }
    // every extended clock (a verified family member) is a clock of its own with a page, a 3D
    // view and its design file (vjeux 01:24Z)
    let extended: Vec<&site_model::Build> = site.builds.iter().filter(|b| b.flags.iter().any(|f| f == "extended") && crate::generate::is_member(b)).collect();
    let broken: Vec<&str> = extended
        .iter()
        .filter(|b| {
            let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
            !page.contains("data-mcview=\"full\"") || !dir.join(&b.design).is_file() || !dir.join(crate::generate::litematic_path(&b.design)).is_file()
        })
        .map(|b| b.name.as_str())
        .collect();
    check("extended clocks: each a page with its 3D view and its downloads", !extended.is_empty() && broken.is_empty(), format!("{} extended clocks, {} broken{}", extended.len(), broken.len(), broken.first().map(|n| format!(", e.g. {n}")).unwrap_or_default()));
    // a combination lock's page names its dials in words, never the raw port names (critique
    // 058332315: "Use the D0, D1, D2, K0, K1 and K2 buttons"), and sends a visitor to the view's
    // buttons only when they turn the frames
    for b in site.builds.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("lock"))) {
        let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let text: String = page.split("<section").skip(1).filter(|s| s.contains("<h2>Try it</h2>") || s.contains("<h2>How to try it</h2>")).collect();
        let raw = ["D0", "D1", "K0", "K1"].iter().any(|n| text.contains(n));
        // (the view's buttons named only when they turn the frames, by the view's own labels)
        let buttons = text.contains("under the view");
        let labels_ok = !crate::explain::viewer_turns_frames() || text.contains("DIAL 1");
        check(&format!("lock {}: its dials in words; its buttons named only when they turn frames", b.name), !raw && buttons == crate::explain::viewer_turns_frames() && labels_ok, format!("raw port names: {raw}, names the buttons: {buttons}, the view turns frames: {}, the view's labels: {labels_ok}", crate::explain::viewer_turns_frames()));
    }
    // a clock too fast for a lamp, or one relighting it within the tick, says so (critique
    // a417977c3; the coordinator 03:20Z), the 4-tick clock among them
    let fast: Vec<&site_model::Build> = site.builds.iter().filter(|b| b.clock.as_ref().is_some_and(|c| !c.lamp.is_empty())).collect();
    let silent: Vec<&str> = fast.iter().filter(|b| !std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default().contains("Redstone dust there follows every pulse.")).map(|b| b.name.as_str()).collect();
    let p4 = site.builds.iter().find(|b| b.name == "clk-obs-p4-obs-4b").and_then(|b| b.clock.as_ref()).map(|c| c.lamp.clone()).unwrap_or_default();
    check("clocks too fast for a lamp: each page says so", !fast.is_empty() && silent.is_empty() && (p4 == "stays lit" || !site.builds.iter().any(|b| b.name == "clk-obs-p4-obs-4b")), format!("{} clocks, {} silent{}, clk-obs-p4-obs-4b: {p4:?}", fast.len(), silent.len(), silent.first().map(|n| format!(", e.g. {n}")).unwrap_or_default()));
    if let Some(b) = site.builds.iter().find(|b| !b.flags.iter().any(|f| f == "size") && b.circuit.as_ref().is_some_and(|c| !c.span.is_empty() && !c.tunable.starts_with("kind:"))) {
        let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let picks = page.matches("class=\"pick").count();
        check("tunable group: the member picker, the current member marked", picks >= 2 && page.contains("aria-current=\"page\""), format!("{}: {picks} members", b.page));
    }
    // the index: the UI ran, counted and drew the first page of cards
    let dom = dump_dom(&chrome, &format!("{base}index.html"), budget_ms)?;
    let want = site.run(&site_model::Query::default()).map(|(h, _)| h.len()).unwrap_or(0);
    let got = result_count(&dom);
    let cards = dom.matches("class=\"card").count();
    check("index: the UI loaded data/site.json and counted the default results", got == Some(want), format!("{got:?} results, engine says {want}"));
    check("index: first page of cards drawn", cards == want.min(60), format!("{cards} cards"));
    // (a card without a 3D view keeps its thumbnail slot: the fallback headline, no data-mcview)
    let thumbs = dom.matches("data-mcview=\"thumb\"").count();
    check("index: every card has a thumbnail slot", dom.matches("<div class=\"thumb ").count() == cards, format!("{} thumbnail slots, {thumbs} with a 3D view", dom.matches("<div class=\"thumb ").count()));
    // each 3D thumbnail with its still frame (webrender's data-still: no search in the browser;
    // tick 0 since vjeux 21:29Z, no simulation on the grid)
    check("index: every 3D thumbnail carries its still frame", dom.matches(" data-still=\"").count() == thumbs, format!("{} of {thumbs}", dom.matches(" data-still=\"").count()));
    // the header: the title, the three kind tabs and the search box, nothing else (vjeux 01:13Z)
    let header = dom.split("<header class=\"top\">").nth(1).and_then(|h| h.split("</header>").next()).unwrap_or("");
    check(
        "index: the header is the title, Doors / Clocks / Circuits and the search box",
        header.matches("<a ").count() == 4 && header.contains("id=\"tab-door\"") && header.contains("id=\"tab-clock\"") && header.contains("id=\"tab-circuit\"") && header.contains("id=\"hq\"") && header.contains("class=\"count\""),
        format!("{} links, search box: {}", header.matches("<a ").count(), header.contains("id=\"hq\"")),
    );
    check("index: the filter panel was drawn", dom.contains("data-act=\"block\"") && !dom.contains("id=\"f-time\""), "block-kind chips, no second search box".into());
    // the clock time search expands extendable families to their members
    let frag = "kind=clock&time=48%20gt";
    let q = site_model::Query::from_fragment(frag);
    let (hits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
    let members = hits.iter().filter(|h| matches!(h, site_model::Hit::Member(_))).count();
    let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
    check("time search 48 gt: count", result_count(&dom) == Some(hits.len()), format!("{:?} results, engine says {} ({members} members)", result_count(&dom), hits.len()));
    check("time search 48 gt: one heading over its clocks", members == 0 && dom.matches("<h3 class=\"period\"").count() == 1 && dom.contains("id=\"p48\""), format!("{} headings, {members} members", dom.matches("<h3 class=\"period\"").count()));
    check("time search 48 gt: the count names the period", dom.contains("with a period of 48 ticks (2.4 s)"), "summary".into());
    // the Clocks tab (vjeux 01:24Z; critique e92a18d12: the 5-tick heading sat 20 phone screens
    // down): a jump bar of periods with counts and every period in a list, each heading a link
    // with its count, cards that lead with their blocks (the heading says the period)
    let dom = dump_dom(&chrome, &format!("{base}index.html#kind=clock"), budget_ms)?;
    let periods = site.clock_periods();
    let jump = dom.split("id=\"f-periods\"").nth(1).and_then(|s| s.split("</nav>").next()).unwrap_or("");
    let options = jump.matches("<option value=\"").count();
    check("clocks tab: a jump bar (landmark periods with counts, every period in a list)", jump.contains("class=\"chip") && options == periods + 1, format!("{} chips, {options} options for {periods} periods", jump.matches("class=\"chip").count()));
    let first = dom.split("<h3 class=\"period\"").nth(1).unwrap_or("");
    check("clocks tab: a period's heading links it and counts its clocks", first.contains("href=\"#kind=clock&amp;time=") && first.contains("<span class=\"count\">"), first.chars().take(120).collect());
    let title = first.split("<div class=\"title\">").nth(1).and_then(|s| s.split("</div>").next()).unwrap_or("");
    check("clocks tab: a card under its heading leads with its blocks, not the period", title.ends_with("blocks") && !title.contains("tick"), title.to_string());
    check("clocks tab: its own opening line", dom.contains(", smallest first."), "lede".into());
    // a time no clock has: the nearest periods that exist (critique: "nothing else"); found in the
    // data, since the clock tracks fill the gaps (a clock at exactly 1 minute arrived 10-21)
    let periods: std::collections::BTreeSet<u32> = site.builds.iter().filter_map(|b| b.clock.as_ref().map(|c| c.every())).collect();
    let gap = (1101..1_000_000u32).find(|p| !periods.contains(p)).unwrap_or(1_000_001);
    let dom = dump_dom(&chrome, &format!("{base}index.html#kind=clock&time={gap}"), budget_ms)?;
    check(&format!("time search {gap} ticks (no clock has it): the nearest periods"), dom.contains("Nearest periods that exist") && dom.contains(". Nearest: "), format!("{:?} results", result_count(&dom)));
    // a gate function with no design says so (critique: "xor" listed other gates silently)
    let missing_gate = ["xor", "xnor", "and", "nand", "nor", "or", "not"].into_iter().find(|f| !site.builds.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.kind == format!("gate {f}"))));
    if let Some(f) = missing_gate {
        let dom = dump_dom(&chrome, &format!("{base}index.html#kind=circuit&q={f}"), budget_ms)?;
        check(&format!("search {f}: says no {} gate is on the site", f.to_uppercase()), dom.contains(&format!("No {} gate on the site right now", f.to_uppercase())), "count line".into());
    }
    // the header's search form from another page (index.html?kind=clock&q=48): the Clocks tab's
    // search box takes a period
    let dom = dump_dom(&chrome, &format!("{base}index.html?kind=clock&q=48"), budget_ms)?;
    check("header search from another page (?kind=clock&q=48): the time search", result_count(&dom) == Some(hits.len()), format!("{:?} results, engine says {}", result_count(&dom), hits.len()));
    // the circuit value search expands the tunable groups to the member that hits it
    let frag = "kind=circuit&value=L%2012";
    let q = site_model::Query::from_fragment(frag);
    let (vhits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
    let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
    check("circuit value search L 12: count", result_count(&dom) == Some(vhits.len()) && !vhits.is_empty(), format!("{:?} results, engine says {}", result_count(&dom), vhits.len()));
    // ... and the extendable circuit families to the generated member (the first card's n_lo + 1)
    let probe = site.cards.iter().filter(|c| c.kind == "circuit").find_map(|c| {
        let f = c.families.first()?;
        let m = site.members.iter().find(|m| m.card == c.id && m.family == f.id && m.n == f.n_lo + 1 && m.build.is_none())?;
        let letter = m.axis.split_once(" = ").map(|(l, _)| l.to_string())?;
        Some((format!("{letter} {}", m.period), m.family.clone(), m.n, c.id.clone()))
    });
    if let Some((value, family, n, card)) = probe {
        let frag = format!("kind=circuit&value={}", site_model::encode(&value));
        let q = site_model::Query::from_fragment(&frag);
        let (fhits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
        let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
        let shown = dom.contains(&format!("families/{card}.html#n{n}\""));
        check(&format!("circuit value search {value}: the generated family member"), result_count(&dom) == Some(fhits.len()) && shown, format!("{family} n = {n}: {:?} results, engine says {}", result_count(&dom), fhits.len()));
    }
    // a facet filter from the URL
    let frag = "kind=door&is=flat,slime-free&sort=volume";
    let q = site_model::Query::from_fragment(frag);
    let (hits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
    let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
    check("doors, flat + slime-free, by volume: count", result_count(&dom) == Some(hits.len()), format!("{:?} results, engine says {}", result_count(&dom), hits.len()));
    if let Some(site_model::Hit::Build(i)) = hits.first() {
        let first = &site.builds[*i];
        check("doors, flat + slime-free, by volume: the smallest first", dom.find(&format!("href=\"{}\"", first.page)).is_some_and(|p| p < dom.find("class=\"card").map(|c| c + 400).unwrap_or(0)), first.name.clone());
    }
    // a build page and a family page
    if let Some(b) = site.builds.iter().find(|b| b.kind == "door") {
        let dom = dump_dom(&chrome, &format!("{base}{}", b.page), budget_ms)?;
        check("door page: live viewer slot + properties", dom.contains("data-mcview=\"full\"") && dom.contains("Open / close"), b.page.clone());
        // its .litematic: linked, served, a schematic of exactly the design file next to it
        let lpath = crate::generate::litematic_path(&b.design);
        let linked = dom.contains(&format!("href=\"../{lpath}\""));
        let verdict = http_get_local(port, &lpath).and_then(|bytes| {
            let s = litematic::read(&bytes)?;
            let d = mctas::design::Design::parse(&std::fs::read_to_string(dir.join(&b.design))?)?;
            litematic::compare(&d, &s)?;
            Ok(format!("{} bytes, {} blocks, Version {} for data version {}", bytes.len(), s.total_blocks.unwrap_or(0), s.version, s.data_version))
        });
        check("door page: .litematic linked, served and equal to the design", linked && verdict.is_ok(), match verdict {
            Ok(v) => format!("{lpath}: {v}"),
            Err(e) => format!("{lpath}: linked {linked}, {e:#}"),
        });
    }
    if let Some(c) = site.cards.first() {
        let dom = dump_dom(&chrome, &format!("{base}{}", c.page), budget_ms)?;
        check("family page: every member listed", dom.matches("<tr id=\"n").count() == site.members.iter().filter(|m| m.card == c.id).count(), c.page.clone());
    }
    // the 3D viewer (webrender's bundle), when the site has one: a probe page renders one
    // design over a one-color fallback; a render has thousands of colors
    if dir.join("viewer/mcview.js").exists() {
        for (path, _) in &probes {
            let png = screenshot(&chrome, &format!("{local}{}", path.trim_start_matches('/')), budget_ms.max(60_000), 480, 360)?;
            let (w, h, px) = decode_png(&png)?;
            let distinct: std::collections::HashSet<[u8; 3]> = px.iter().copied().collect();
            let gray = px.iter().filter(|p| **p == [128, 128, 128]).count();
            check(
                &format!("viewer renders {}", path.trim_start_matches("/__probe-").trim_end_matches(".html")),
                distinct.len() > 500 && gray * 2 < px.len(),
                format!("{w}x{h}: {} colors, {:.0}% fallback gray", distinct.len(), 100.0 * gray as f64 / px.len().max(1) as f64),
            );
        }
    } else {
        println!("SKIP viewer: no viewer/mcview.js in the site (build with --viewer)");
    }
    Ok(fails)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_parse() {
        assert_eq!(result_count("<span id=\"f-count\"><strong>42</strong> results</span>"), Some(42));
        assert_eq!(result_count("nothing"), None);
    }

    /// A 2x2 RGBA PNG with a Sub-filtered and an Up-filtered row decodes to its pixels.
    #[test]
    fn png_decodes() {
        use std::io::Write as _;
        let raw: Vec<u8> = vec![
            1, 10, 20, 30, 255, 5, 5, 5, 0, // Sub: (10,20,30) then (15,25,35)
            2, 1, 1, 1, 0, 2, 2, 2, 0, // Up: (11,21,31) then (17,27,37)
        ];
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&raw).unwrap();
        let idat = z.finish().unwrap();
        let chunk = |kind: &[u8], data: &[u8]| -> Vec<u8> {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(kind);
            c.extend_from_slice(data);
            c.extend_from_slice(&[0, 0, 0, 0]);
            c
        };
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(chunk(b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0]));
        png.extend(chunk(b"IDAT", &idat));
        png.extend(chunk(b"IEND", &[]));
        let (w, h, px) = decode_png(&png).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(px, vec![[10, 20, 30], [15, 25, 35], [11, 21, 31], [17, 27, 37]]);
    }

    #[test]
    fn serves_files() {
        let dir = std::env::temp_dir().join(format!("site-serve-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("pkg")).unwrap();
        std::fs::write(dir.join("index.html"), "<p>hi</p>").unwrap();
        std::fs::write(dir.join("pkg/a.wasm"), [0u8, 97, 115, 109]).unwrap();
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let root = dir.clone();
        std::thread::spawn(move || serve_on(l, root));
        let get = |path: &str| -> String {
            let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut out = Vec::new();
            std::io::Read::read_to_end(&mut s, &mut out).unwrap();
            String::from_utf8_lossy(&out).into_owned()
        };
        let r = get("/");
        assert!(r.starts_with("HTTP/1.1 200") && r.ends_with("<p>hi</p>"), "{r}");
        assert!(get("/pkg/a.wasm").contains("Content-Type: application/wasm"));
        assert!(get("/../etc/passwd").starts_with("HTTP/1.1 404"));
        assert!(get("/index.html?x=1#y").starts_with("HTTP/1.1 200"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
