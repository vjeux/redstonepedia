//! `mctas web tools`: the site's Tools pages (crates/site/src/tools.rs, web/tools) driven in
//! headless Chrome like a player, every answer the page shows checked against crates/rng's own:
//! the enchanting calculator on the video's numbers (two seeds → the player seed → "drop 195, 15
//! bookshelves, the bottom slot"), then a whole run from readings of the table (a search of every
//! seed on the page's Web Workers, the next seed, the player seed, a plan), the librarian
//! calculator's sync and plan in a positive and a negative world seed, and every Tools page at
//! phone widths without a sideways scroll.

use crate::cdp::{Browser, base64_decode};
use anyhow::{Context, Result, bail};
use rng::Data;
use rng::java_random::JavaRandom;
use rng::player::{Goal, crack_player, plan};
use rng::table::offers;
use rng::villager::{Generations, Offer, Want, plan_rerolls, trade_set};
use rng::xoroshiro::SequenceSettings;
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};

struct Check {
    fails: usize,
}

impl Check {
    fn ok(&mut self, pass: bool, what: &str) {
        println!("{} {what}", if pass { "ok  " } else { "FAIL" });
        if !pass {
            self.fails += 1;
        }
    }
}

fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// Wait until `expr` is truthy (polling), up to `secs`.
fn wait_for(b: &mut Browser, expr: &str, secs: u64) -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        let v = b.eval(expr)?;
        if !(v.is_null() || v == json!(false) || v == json!("") || v == json!(0)) {
            return Ok(v);
        }
        if Instant::now() > deadline {
            bail!("timed out after {secs} s waiting for {expr}");
        }
        b.drain(Duration::from_millis(150));
    }
}

fn set(b: &mut Browser, id: &str, value: &str) -> Result<()> {
    b.eval(&format!("(() => {{ const e = document.getElementById({}); e.value = {}; e.dispatchEvent(new Event('change', {{bubbles: true}})); }})()", js_str(id), js_str(value)))?;
    Ok(())
}

fn text(b: &mut Browser, id: &str) -> Result<String> {
    Ok(b.eval(&format!("document.getElementById({}).innerText", js_str(id)))?.as_str().unwrap_or("").to_string())
}

fn value(b: &mut Browser, id: &str) -> Result<String> {
    Ok(b.eval(&format!("document.getElementById({}).value", js_str(id)))?.as_str().unwrap_or("").to_string())
}

fn click(b: &mut Browser, selector: &str) -> Result<()> {
    let r = b.eval(&format!("(() => {{ const e = document.querySelector({}); if (!e) return false; e.click(); return true; }})()", js_str(selector)))?;
    if r != json!(true) {
        bail!("nothing to click at {selector}");
    }
    Ok(())
}

/// Fill the reading rows of `container` (adding rows as needed) with the costs `seed` shows at
/// each bookshelf count.
fn fill_readings(b: &mut Browser, container: &str, seed: i32, shelves: &[i32]) -> Result<()> {
    let data = Data::load();
    let sword = &data.items[data.item("diamond_sword").context("diamond_sword")?];
    while b.eval(&format!("document.querySelectorAll('#{container} .obsrow').length"))?.as_u64().unwrap_or(0) < shelves.len() as u64 {
        click(b, &format!("[data-act=add-obs][data-target={container}]"))?;
    }
    for (i, &sh) in shelves.iter().enumerate() {
        let o = offers(data, seed, sword, sh);
        b.eval(&format!(
            "(() => {{ const r = document.querySelectorAll('#{container} .obsrow')[{i}]; r.querySelector('.sh').value = '{sh}'; r.querySelector('.c0').value = '{}'; r.querySelector('.c1').value = '{}'; r.querySelector('.c2').value = '{}'; }})()",
            o[0].cost, o[1].cost, o[2].cost
        ))?;
    }
    Ok(())
}

fn open(b: &mut Browser, url: &str, ready: &str) -> Result<()> {
    b.call("Page.navigate", json!({ "url": url }))?;
    wait_for(b, ready, 30).with_context(|| format!("{url}: the page's bundle never drew its forms"))?;
    Ok(())
}

fn enchanting(b: &mut Browser, base: &str, shots: Option<&Path>, c: &mut Check) -> Result<()> {
    let data = Data::load();
    open(b, &format!("{base}tools/enchanting.html"), "document.querySelectorAll('#obs1 .obsrow').length == 2 && document.querySelectorAll('#tlevels .lv').length > 0")?;
    // A. the video's numbers
    set(b, "xp1", "5F94CCE0")?;
    set(b, "xp2", "BE8A2899")?;
    click(b, "#crack2")?;
    wait_for(b, "document.getElementById('out2').innerText.includes('Player seed')", 10)?;
    let want = crack_player(0x5F94CCE0u32 as i32, 0xBE8A2899u32 as i32, 0);
    let pseed = value(b, "pseed")?;
    c.ok(want.len() == 1 && pseed == format!("{:012X}", want[0].0) && pseed.starts_with("BE8A2899B11"), &format!("the video's seeds 5F94CCE0, BE8A2899 give the player seed {pseed} (the video: BE8A2899B11...)"));
    set(b, "tlevel", "999")?;
    for (e, l) in [("sharpness", 4), ("fire_aspect", 2), ("looting", 3)] {
        click(b, &format!("#tlevels .lv[data-ench='minecraft:{e}'] button[data-val='{l}']"))?;
    }
    click(b, "[data-act=calc]")?;
    let out = text(b, "out3")?;
    c.ok(
        out.contains("Drop 195 items") && out.contains("3 stacks of 64 and 3 more") && out.contains("Set 15 bookshelves") && out.contains("click slot 3"),
        &format!("Sharpness IV, Fire Aspect II, Looting III on a diamond sword: the video's \"drop 195, 15 bookshelves, slot 3\": {:?}", out.lines().next().unwrap_or("")),
    );
    // B. a whole run from readings: a player whose generator we know, its two enchanting seeds
    let mut player = JavaRandom::from_state(0x2C41_9D0E_57A3);
    let (xp1, xp2) = (player.next_int(), player.next_int());
    open(b, &format!("{base}tools/enchanting.html"), "document.querySelectorAll('#obs1 .obsrow').length == 2")?;
    let order = [15, 8, 3, 12, 1, 6, 10, 0, 14, 5, 2, 9];
    let t = Instant::now();
    let mut used = 0;
    for n in [7, 9, 12] {
        fill_readings(b, "obs1", xp1, &order[..n])?;
        click(b, "#crack1")?;
        wait_for(b, "!document.getElementById('crack1').disabled && !document.getElementById('out1').innerText.startsWith('Searching')", 300)?;
        used = n;
        if value(b, "xp1")? == format!("{:08X}", xp1 as u32) {
            break;
        }
    }
    let secs = t.elapsed().as_secs_f64();
    let workers_failed = b.console.iter().any(|l| l.contains("workers unavailable"));
    c.ok(value(b, "xp1")? == format!("{:08X}", xp1 as u32) && !workers_failed, &format!("seed {:08X} found from {used} readings of the costs, every seed searched on the page's workers in {secs:.1} s", xp1 as u32));
    let mut found_after = 0;
    for n in [2, 3, 4, 5, 6] {
        fill_readings(b, "obs2", xp2, &order[..n])?;
        click(b, "#crack2")?;
        wait_for(b, "!document.getElementById('crack2').disabled && !document.getElementById('out2').innerText.startsWith('Seed 2 does not follow')", 120)?;
        found_after = n;
        if text(b, "out2")?.contains("Player seed") || text(b, "out2")?.contains("not from enchants") {
            break;
        }
    }
    let state = crack_player(xp1, xp2, 0);
    let pseed = value(b, "pseed")?;
    let shown = text(b, "out2")?;
    c.ok(
        value(b, "xp2")? == format!("{:08X}", xp2 as u32) && state.len() == 1 && pseed == format!("{:012X}", state[0].0) && state[0].0 == player.state,
        &format!("the next seed {:08X} from {found_after} readings, and the player seed {pseed}: {shown:?}", xp2 as u32),
    );
    set(b, "titem", "minecraft:diamond_pickaxe")?;
    set(b, "tlevel", "40")?;
    for (e, l) in [("efficiency", 4), ("unbreaking", 3)] {
        click(b, &format!("#tlevels .lv[data-ench='minecraft:{e}'] button[data-val='{l}']"))?;
    }
    click(b, "#tlevels .lv[data-ench='minecraft:silk_touch'] button[data-val='no']")?;
    click(b, "[data-act=calc]")?;
    let out = text(b, "out3")?;
    let e = |n: &str| data.enchant(n).unwrap_or(0);
    let goal = Goal { item: data.item("diamond_pickaxe").unwrap_or(0), wanted: vec![(e("efficiency"), 4), (e("unbreaking"), 3)], unwanted: vec![e("silk_touch")], max_shelves: 15, max_drops: 2048, player_level: 40 };
    let want = plan(data, player.state, &goal).context("a plan")?;
    let words = format!("Set {} bookshelves, put in the diamond pickaxe, click slot {}", want.shelves, want.slot + 1);
    let drops = want.drops.map(|n| format!("Drop {n} item")).unwrap_or_default();
    c.ok(out.contains(&words) && out.contains(&drops) && !out.contains("silk touch"), &format!("Efficiency IV + Unbreaking III, no Silk Touch, on a diamond pickaxe: {:?} then {words:?}", want.drops));
    shot(b, shots, "enchanting-desktop.png", 1280)?;
    click(b, "[data-act=done]")?;
    c.ok(value(b, "pseed")? == format!("{:012X}", want.state_after), "\"I did it\" moves the player seed past the plan");
    Ok(())
}

/// The whole page as it stands into `shots` (when given), `width` px wide.
fn shot(b: &mut Browser, shots: Option<&Path>, name: &str, width: u32) -> Result<()> {
    let Some(dir) = shots else { return Ok(()) };
    std::fs::create_dir_all(dir)?;
    let height = b.eval("document.documentElement.scrollHeight")?.as_f64().unwrap_or(900.0).min(4000.0);
    let r = b.call("Page.captureScreenshot", json!({ "format": "png", "captureBeyondViewport": true, "clip": { "x": 0, "y": 0, "width": width, "height": height, "scale": 1 } }))?;
    std::fs::write(dir.join(name), base64_decode(r["data"].as_str().unwrap_or(""))?)?;
    Ok(())
}

fn book_row(data: &Data, offers: &[Offer]) -> (String, String, String) {
    match offers.iter().find_map(|o| o.book.map(|b| (b, o.cost_a.1))) {
        Some(((e, l), price)) => (data.enchants[e as usize].id.clone(), l.to_string(), price.to_string()),
        None => (String::new(), String::new(), String::new()),
    }
}

fn librarian(b: &mut Browser, base: &str, shots: Option<&Path>, c: &mut Check) -> Result<()> {
    let data = Data::load();
    let set_ = trade_set(data, "librarian", 1).map_err(anyhow::Error::msg)?;
    for (world, first) in [(8675309i64, 150u64), (-5584399522669591931, 10)] {
        open(b, &format!("{base}tools/librarian.html"), "document.querySelectorAll('#seen .seenrow').length == 2")?;
        let mut g = Generations::new(data, set_, world, SequenceSettings::default()).map_err(anyhow::Error::msg)?;
        let mut screens = Vec::new();
        while g.index < first + 2 {
            let i = g.index;
            let o = g.next_generation().map_err(anyhow::Error::msg)?;
            if i >= first {
                screens.push(o);
            }
        }
        set(b, "wseed", &world.to_string())?;
        for (k, o) in screens.iter().enumerate() {
            let (id, level, price) = book_row(data, o);
            b.eval(&format!(
                "(() => {{ const r = document.querySelectorAll('#seen .seenrow')[{k}]; r.querySelector('.b').value = {}; r.querySelector('.l').value = {}; r.querySelector('.p').value = {}; }})()",
                js_str(&id),
                js_str(if level.is_empty() { "1" } else { &level }),
                js_str(&price)
            ))?;
        }
        click(b, "[data-act=vsync]")?;
        let after = value(b, "vafter")?;
        c.ok(after == (first + 1).to_string(), &format!("world {world}: screens {first} and {} find the place (the last screen: entry {after})", first + 1));
        set(b, "vwant", "minecraft:mending")?;
        click(b, "[data-act=vplan]")?;
        let out = text(b, "vout2")?;
        let want = Want { enchant: data.enchant("mending").unwrap_or(0), min_level: 1, max_price: None };
        let (n, _) = plan_rerolls(data, set_, world, SequenceSettings::default(), first + 1, &want, 100_000).map_err(anyhow::Error::msg)?.context("mending")?;
        c.ok(out.contains(&format!("Reroll {n} time")), &format!("world {world}: Mending in {n} rerolls"));
        if world > 0 {
            shot(b, shots, "librarian-desktop.png", 1280)?;
        }
    }
    Ok(())
}

/// Every Tools page at phone widths: nothing wider than the window.
fn phones(b: &mut Browser, base: &str, shots: Option<&Path>, c: &mut Check) -> Result<()> {
    for (w, h) in [(375, 812), (320, 640)] {
        b.call("Emulation.setDeviceMetricsOverride", json!({ "width": w, "height": h, "deviceScaleFactor": 2, "mobile": true }))?;
        for page in ["tools/index.html", "tools/enchanting.html", "tools/librarian.html"] {
            b.call("Page.navigate", json!({ "url": format!("{base}{page}") }))?;
            let ready = match page {
                "tools/enchanting.html" => "document.querySelectorAll('#obs1 .obsrow').length == 2",
                "tools/librarian.html" => "document.querySelectorAll('#seen .seenrow').length == 2",
                _ => "document.readyState == 'complete'",
            };
            wait_for(b, ready, 30)?;
            b.drain(Duration::from_millis(300));
            let wide = b.eval(&format!("(() => {{ const out = []; for (const e of document.querySelectorAll('body *')) {{ const r = e.getBoundingClientRect(); const spills = e.clientWidth > 0 && e.scrollWidth > e.clientWidth + 1 && getComputedStyle(e).overflowX == 'visible'; if ((r.right > {w} + 1 && r.width > 0) || spills) out.push(e.tagName + '.' + e.className + (e.id ? '#' + e.id : '') + ' ' + Math.round(r.right) + (spills ? ' (its text spills over)' : '')); }} return [document.documentElement.scrollWidth, out.slice(0, 6)]; }})()"))?;
            // (against the phone's width: a page wider than it widens the window itself)
            let sw = wide[0].as_f64().unwrap_or(0.0);
            c.ok(sw <= w as f64, &format!("{page} at {w} px: {sw} px wide{}", if sw > w as f64 { format!(", too wide: {}", wide[1]) } else { String::new() }));
            if let Some(dir) = shots {
                std::fs::create_dir_all(dir)?;
                let height = b.eval("document.documentElement.scrollHeight")?.as_f64().unwrap_or(h as f64).min(4000.0);
                let r = b.call("Page.captureScreenshot", json!({ "format": "png", "captureBeyondViewport": true, "clip": { "x": 0, "y": 0, "width": w, "height": height, "scale": 1 } }))?;
                let name = format!("{}-{w}.png", page.trim_start_matches("tools/").trim_end_matches(".html"));
                std::fs::write(dir.join(&name), base64_decode(r["data"].as_str().unwrap_or(""))?)?;
            }
        }
    }
    b.call("Emulation.clearDeviceMetricsOverride", json!({}))?;
    Ok(())
}

pub fn cmd_tools(dir: &Path, url: Option<&str>, shots: Option<&Path>, browser: Option<&Path>) -> Result<()> {
    let base = match url {
        Some(u) => format!("{}/", u.trim_end_matches('/')),
        None => {
            if !dir.join("tools/enchanting.html").is_file() {
                bail!("{}: no tools/enchanting.html (a `mctas site build` output?)", dir.display());
            }
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
            let port = listener.local_addr()?.port();
            let root = dir.to_path_buf();
            std::thread::spawn(move || site::serve::serve_on(listener, root));
            format!("http://127.0.0.1:{port}/")
        }
    };
    let chrome = match browser {
        Some(p) => p.to_path_buf(),
        None => site::serve::install_chrome()?,
    };
    let mut b = Browser::start(&chrome, "1280,900", 1.0, "about:blank")?;
    b.attach_page()?;
    b.call("Page.enable", json!({}))?;
    b.call("Runtime.enable", json!({}))?;
    let mut c = Check { fails: 0 };
    if let Err(e) = enchanting(&mut b, &base, shots, &mut c) {
        c.ok(false, &format!("the enchanting calculator: {e:#}"));
    }
    if let Err(e) = librarian(&mut b, &base, shots, &mut c) {
        c.ok(false, &format!("the librarian calculator: {e:#}"));
    }
    if let Err(e) = phones(&mut b, &base, shots, &mut c) {
        c.ok(false, &format!("phone widths: {e:#}"));
    }
    for l in b.console.iter().filter(|l| l.starts_with("exception") || l.starts_with("error")) {
        println!("console {l}");
        c.fails += 1;
    }
    if c.fails > 0 {
        bail!("{} Tools check(s) failed ({base})", c.fails);
    }
    println!("all Tools checks passed ({base})");
    Ok(())
}
