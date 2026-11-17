//! REDSTONEPEDIA's Tools pages in the browser (docs/site.md#tools): the enchanting table calculator
//! (tools/enchanting.html) and the librarian calculator (tools/librarian.html). The pages' text and
//! empty forms are static (crates/site/src/tools.rs); this module draws the rows, answers the
//! buttons with crates/rng (unit-tested natively, checked against the real game by `mctas rng
//! game`), and runs the search over every enchanting seed on Web Workers: each worker is this same
//! module (tools/pkg/worker.js, the one-line start every page uses), answering slices of the 2^32
//! seeds.

use rng::crack::{Observation, crack_range, filter};
use rng::data::Data;
use rng::player::{Goal, crack_player, next_seed_candidates, plan};
use rng::villager::{Generations, ObservedTrades, Offer, Want, plan_rerolls, sync, trade_set};
use rng::xoroshiro::SequenceSettings;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, Document, Element, HtmlElement, HtmlInputElement, HtmlProgressElement, HtmlSelectElement, MessageEvent, Worker, WorkerOptions, WorkerType};

const ROMAN: [&str; 6] = ["", "I", "II", "III", "IV", "V"];
/// Seeds per slice a worker answers between two progress messages.
const CHUNK: u64 = 1 << 22;
/// The candidates kept to narrow down with the next reading (more: search again).
const KEEP: usize = 2_000_000;

fn data() -> &'static Data {
    Data::load()
}

fn doc() -> Document {
    web_sys::window().and_then(|w| w.document()).expect("a document")
}

fn by_id(id: &str) -> Option<Element> {
    doc().get_element_by_id(id)
}

fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn words(name: &str) -> String {
    name.replace('_', " ")
}

fn hex8(s: i32) -> String {
    format!("{:08X}", s as u32)
}

/// The value of an input or a select.
fn value_of(e: &Element) -> String {
    if let Some(i) = e.dyn_ref::<HtmlInputElement>() {
        i.value()
    } else if let Some(s) = e.dyn_ref::<HtmlSelectElement>() {
        s.value()
    } else {
        String::new()
    }
    .trim()
    .to_string()
}

fn value(id: &str) -> String {
    by_id(id).map(|e| value_of(&e)).unwrap_or_default()
}

fn set_value(id: &str, v: &str) {
    if let Some(i) = by_id(id).and_then(|e| e.dyn_into::<HtmlInputElement>().ok()) {
        i.set_value(v);
    }
}

/// The value of the field `class` inside `row`.
fn field(row: &Element, class: &str) -> String {
    row.query_selector(&format!(".{class}")).ok().flatten().map(|e| value_of(&e)).unwrap_or_default()
}

fn set_hidden(id: &str, hidden: bool) {
    if let Some(e) = by_id(id).and_then(|e| e.dyn_into::<HtmlElement>().ok()) {
        e.set_hidden(hidden);
    }
}

/// Show a result box: `class` "good" (a green edge), "bad" (red) or "" (plain).
fn show(id: &str, html: &str, class: &str) {
    if let Some(e) = by_id(id) {
        e.set_inner_html(html);
        e.set_class_name(&format!("out {class}"));
        if let Some(h) = e.dyn_ref::<HtmlElement>() {
            h.set_hidden(false);
        }
    }
}

fn set_disabled(id: &str, disabled: bool) {
    if let Some(e) = by_id(id) {
        if disabled {
            let _ = e.set_attribute("disabled", "");
        } else {
            let _ = e.remove_attribute("disabled");
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    // a worker: answer slices of the seed search
    if let Ok(scope) = js_sys::global().dyn_into::<DedicatedWorkerGlobalScope>() {
        worker_install(scope);
        return;
    }
    if let Some(root) = by_id("calc-enchant") {
        enchant_install(&root);
    }
    if let Some(root) = by_id("calc-librarian") {
        librarian_install(&root);
    }
}

// ------------------------------------------------------------------------------------------------
// observations as JSON (the page to its workers)

fn obs_to_json(obs: &[Observation]) -> Value {
    let d = data();
    Value::Array(
        obs.iter()
            .map(|o| {
                json!({
                    "shelves": o.shelves,
                    "costs": o.costs,
                    "item": o.item.map(|i| d.items[i].id.clone()),
                    "clues": o.clues.iter().map(|c| c.map(|(e, l)| json!([d.enchants[e as usize].id, l]))).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

fn obs_from_json(v: &Value) -> Vec<Observation> {
    let d = data();
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|o| Observation {
                    shelves: o["shelves"].as_i64().unwrap_or(0) as i32,
                    costs: std::array::from_fn(|i| o["costs"][i].as_i64().unwrap_or(-1) as i32),
                    item: o["item"].as_str().and_then(|s| d.item(s)),
                    clues: std::array::from_fn(|i| {
                        let c = &o["clues"][i];
                        Some((d.enchant(c[0].as_str()?)?, c[1].as_u64()? as u8))
                    }),
                })
                .collect()
        })
        .unwrap_or_default()
}

// ------------------------------------------------------------------------------------------------
// the workers

fn worker_install(scope: DedicatedWorkerGlobalScope) {
    let me = scope.clone();
    let on = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let Some(text) = e.data().as_string() else { return };
        let Ok(req) = serde_json::from_str::<Value>(&text) else { return };
        let obs = obs_from_json(&req["obs"]);
        let start = req["start"].as_u64().unwrap_or(0);
        let count = req["count"].as_u64().unwrap_or(0);
        let mut found: Vec<i32> = Vec::new();
        let mut off = 0;
        while off < count {
            let n = CHUNK.min(count - off);
            found.extend(crack_range(data(), &obs, (start + off) as u32, n as u32));
            off += n;
            let _ = me.post_message(&JsValue::from_str(&json!({"type": "progress", "n": n}).to_string()));
        }
        let _ = me.post_message(&JsValue::from_str(&json!({"type": "done", "found": found}).to_string()));
    });
    scope.set_onmessage(Some(on.as_ref().unchecked_ref()));
    on.forget();
    let _ = scope.post_message(&JsValue::from_str(r#"{"type":"ready"}"#));
}

type Done = Box<dyn FnOnce(Vec<i32>)>;

struct Scan {
    done: u64,
    left: usize,
    found: Vec<i32>,
    workers: Vec<Worker>,
    on_done: Option<Done>,
    failed: bool,
}

/// Search every seed for `obs` on Web Workers (one per core), showing progress in `progress` and
/// disabling `button` meanwhile; on the page's own thread, a slice at a time, where workers cannot
/// start.
fn scan_all(obs: Vec<Observation>, progress: &'static str, button: &'static str, on_done: impl FnOnce(Vec<i32>) + 'static) {
    const TOTAL: u64 = 1 << 32;
    set_hidden(progress, false);
    set_progress(progress, 0.0);
    set_disabled(button, true);
    let finish: Done = Box::new(move |mut found: Vec<i32>| {
        set_hidden(progress, true);
        set_disabled(button, false);
        found.sort_unstable();
        on_done(found);
    });
    let url = by_id("calc-enchant").and_then(|r| r.get_attribute("data-worker")).unwrap_or_default();
    let cores = web_sys::window().map(|w| w.navigator().hardware_concurrency()).unwrap_or(4.0) as usize;
    let n = cores.clamp(1, 32);
    let state = Rc::new(RefCell::new(Scan { done: 0, left: n, found: Vec::new(), workers: Vec::new(), on_done: Some(finish), failed: false }));
    let msg_obs = obs_to_json(&obs);
    let per = TOTAL / n as u64;
    for i in 0..n {
        let opts = WorkerOptions::new();
        opts.set_type(WorkerType::Module);
        let Ok(w) = (if url.is_empty() { Err(JsValue::NULL) } else { Worker::new_with_options(&url, &opts) }) else {
            fail_over(&state, obs, progress);
            return;
        };
        let start = i as u64 * per;
        let count = if i + 1 == n { TOTAL - start } else { per };
        let msg = json!({"obs": msg_obs, "start": start, "count": count}).to_string();
        let st = state.clone();
        let me = w.clone();
        let on = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            let v: Value = e.data().as_string().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
            match v["type"].as_str() {
                Some("ready") => {
                    let _ = me.post_message(&JsValue::from_str(&msg));
                }
                Some("progress") => {
                    let done = {
                        let mut s = st.borrow_mut();
                        s.done += v["n"].as_u64().unwrap_or(0);
                        s.done
                    };
                    set_progress(progress, done as f64 / TOTAL as f64);
                }
                Some("done") => {
                    me.terminate();
                    let finished = {
                        let mut s = st.borrow_mut();
                        s.found.extend(v["found"].as_array().into_iter().flatten().filter_map(|x| x.as_i64()).map(|x| x as i32));
                        s.left -= 1;
                        if s.left == 0 && !s.failed { s.on_done.take().map(|f| (f, std::mem::take(&mut s.found))) } else { None }
                    };
                    if let Some((f, found)) = finished {
                        f(found);
                    }
                }
                _ => {}
            }
        });
        w.set_onmessage(Some(on.as_ref().unchecked_ref()));
        on.forget();
        let st = state.clone();
        let obs_again = obs.clone();
        let on_err = Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| fail_over(&st, obs_again.clone(), progress));
        w.set_onerror(Some(on_err.as_ref().unchecked_ref()));
        on_err.forget();
        state.borrow_mut().workers.push(w);
    }
}

fn set_progress(id: &str, f: f64) {
    if let Some(p) = by_id(id).and_then(|e| e.dyn_into::<HtmlProgressElement>().ok()) {
        p.set_value(f);
    }
}

/// One turn of the page-thread search (it schedules the next).
type Step = Closure<dyn FnMut()>;

/// The workers could not start (a browser without module workers): stop them and search on the
/// page's thread instead, a slice per turn of the event loop so the page stays live.
fn fail_over(state: &Rc<RefCell<Scan>>, obs: Vec<Observation>, progress: &'static str) {
    let on_done = {
        let mut s = state.borrow_mut();
        if s.failed {
            return;
        }
        s.failed = true;
        for w in &s.workers {
            w.terminate();
        }
        s.on_done.take()
    };
    let Some(on_done) = on_done else { return };
    log("redstonepedia tools: workers unavailable, searching on the page");
    let step: Rc<RefCell<Option<Step>>> = Rc::new(RefCell::new(None));
    let next = step.clone();
    let mut at: u64 = 0;
    let mut found: Vec<i32> = Vec::new();
    let mut on_done = Some(on_done);
    *step.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
        found.extend(crack_range(data(), &obs, at as u32, CHUNK as u32));
        at += CHUNK;
        set_progress(progress, at as f64 / (1u64 << 32) as f64);
        if at >= 1 << 32 {
            if let Some(f) = on_done.take() {
                f(std::mem::take(&mut found));
            }
            return;
        }
        if let (Some(w), Some(cb)) = (web_sys::window(), next.borrow().as_ref()) {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 0);
        }
    }));
    if let (Some(w), Some(cb)) = (web_sys::window(), step.borrow().as_ref()) {
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 0);
    }
    std::mem::forget(step);
}

// ------------------------------------------------------------------------------------------------
// the enchanting table calculator

#[derive(Default)]
struct Enchant {
    /// The readings of the last full search and the seeds it left.
    cand1: Option<(Vec<Observation>, Vec<i32>)>,
    /// The player seed after the last plan (the "I did it" button).
    after: Option<u64>,
}

thread_local! {
    static ENCHANT: RefCell<Enchant> = RefCell::new(Enchant::default());
}

fn enchantable() -> Vec<usize> {
    let d = data();
    (0..d.items.len()).filter(|&i| d.items[i].is_enchantable).collect()
}

fn hint_options(item: usize) -> String {
    let d = data();
    let mut o = String::from("<option value=\"\">hint?</option>");
    for &e in &d.items[item].table_pool {
        let en = &d.enchants[e as usize];
        for l in en.min_level..=en.max_level {
            let lv = if en.max_level > 1 { format!(" {}", ROMAN[l as usize]) } else { String::new() };
            o += &format!("<option value=\"{}|{l}\">{}{lv}</option>", esc(&en.id), esc(&words(&en.name)));
        }
    }
    o
}

fn reading_row(n: usize) -> String {
    let d = data();
    let sword = d.item("diamond_sword").unwrap_or(0);
    let items: String = enchantable().iter().map(|&i| format!("<option value=\"{}\"{}>{}</option>", esc(&d.items[i].id), if i == sword { " selected" } else { "" }, esc(&words(&d.items[i].name)))).collect();
    let hints = hint_options(sword);
    let shelves = [15, 8, 3, 12, 1, 6][n % 6];
    format!(
        "<div class=\"row obsrow\"><label>bookshelves <input class=\"num sh\" inputmode=\"numeric\" value=\"{shelves}\"></label>\
         <span class=\"costs\">levels <input class=\"num c0\" inputmode=\"numeric\" aria-label=\"top slot's level\"> <input class=\"num c1\" inputmode=\"numeric\" aria-label=\"middle slot's level\"> <input class=\"num c2\" inputmode=\"numeric\" aria-label=\"bottom slot's level\"></span>\
         <label class=\"opt\">item <select class=\"it\">{items}</select></label>\
         <span class=\"opt\"><select class=\"h0\" aria-label=\"top slot's hint\">{hints}</select><select class=\"h1\" aria-label=\"middle slot's hint\">{hints}</select><select class=\"h2\" aria-label=\"bottom slot's hint\">{hints}</select></span>\
         <button type=\"button\" class=\"small\" data-act=\"rm-row\" aria-label=\"remove this reading\">×</button></div>"
    )
}

fn add_row(container: &str, html_of: impl Fn(usize) -> String) {
    if let Some(c) = by_id(container) {
        let n = c.child_element_count() as usize;
        let _ = c.insert_adjacent_html("beforeend", &html_of(n));
    }
}

fn levels_html(item: usize) -> String {
    let d = data();
    let mut s = String::new();
    for &e in &d.items[item].table_pool {
        let en = &d.enchants[e as usize];
        let mut buttons = String::from(
            "<button type=\"button\" data-act=\"lv\" data-val=\"no\" class=\"no\" aria-label=\"must not have it\">×</button><button type=\"button\" data-act=\"lv\" data-val=\"any\" class=\"on\" aria-label=\"either way\">–</button>",
        );
        for l in 1..=en.max_level {
            buttons += &format!("<button type=\"button\" data-act=\"lv\" data-val=\"{l}\">{}</button>", ROMAN[l as usize]);
        }
        s += &format!("<div class=\"lvname\">{}</div><div class=\"lv\" data-ench=\"{}\">{buttons}</div>", esc(&words(&en.name)), esc(&en.id));
    }
    s
}

fn read_obs(container: &str) -> Result<Vec<Observation>, String> {
    let d = data();
    let mut out = Vec::new();
    let Some(c) = by_id(container) else { return Ok(out) };
    let rows = c.query_selector_all(".obsrow").map_err(|_| "no rows")?;
    for i in 0..rows.length() {
        let Some(row) = rows.item(i).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
        let cost = |k: &str| field(&row, k);
        if cost("c0").is_empty() && cost("c1").is_empty() && cost("c2").is_empty() {
            continue;
        }
        let num = |s: String, what: &str| -> Result<i32, String> { if s.is_empty() { Ok(0) } else { s.parse::<i32>().map_err(|_| format!("{what} \"{s}\" is not a number")) } };
        let shelves = num(field(&row, "sh"), "bookshelves")?;
        if !(0..=15).contains(&shelves) {
            return Err(format!("bookshelves must be 0 to 15, not {shelves}"));
        }
        let costs = [num(cost("c0"), "a level")?, num(cost("c1"), "a level")?, num(cost("c2"), "a level")?];
        let mut o = Observation::costs_only(shelves, costs);
        let clues: [Option<(u8, u8)>; 3] = std::array::from_fn(|k| {
            let v = field(&row, &format!("h{k}"));
            let (id, l) = v.split_once('|')?;
            Some((d.enchant(id)?, l.parse().ok()?))
        });
        if clues.iter().any(|c| c.is_some()) {
            o.item = d.item(&field(&row, "it"));
            o.clues = clues;
        }
        out.push(o);
    }
    Ok(out)
}

fn has_clues(obs: &[Observation]) -> bool {
    obs.iter().any(|o| o.clues.iter().any(|c| c.is_some()))
}

fn parse_seed(id: &str) -> Result<Option<i32>, String> {
    let t = value(id);
    if t.is_empty() {
        return Ok(None);
    }
    rng::parse_xp_seed(&t).map(Some).map_err(|_| format!("\"{}\" is not a seed (8 hex digits, like 5F94CCE0)", esc(&t)))
}

fn enchant_install(root: &Element) {
    add_row("obs1", reading_row);
    add_row("obs1", reading_row);
    add_row("obs2", reading_row);
    let item = data().item(&value("titem")).or_else(|| data().item("diamond_sword")).unwrap_or(0);
    if let Some(l) = by_id("tlevels") {
        l.set_inner_html(&levels_html(item));
    }
    let on_click = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        let Some(b) = t.closest("[data-act]").ok().flatten() else { return };
        let act = b.get_attribute("data-act").unwrap_or_default();
        match act.as_str() {
            "add-obs" => {
                if let Some(target) = b.get_attribute("data-target") {
                    add_row(&target, reading_row);
                }
            }
            "rm-row" => {
                if let Some(row) = b.closest(".row").ok().flatten() {
                    row.remove();
                }
            }
            "lv" => {
                if let Some(group) = b.parent_element() {
                    let all = group.children();
                    for i in 0..all.length() {
                        if let Some(x) = all.item(i) {
                            let _ = x.class_list().remove_1("on");
                        }
                    }
                    let _ = b.class_list().add_1("on");
                }
            }
            "crack1" => crack1(),
            "crack2" => crack2(),
            "calc" => calculate(),
            "done" => {
                if let Some(after) = ENCHANT.with(|s| s.borrow_mut().after.take()) {
                    set_value("pseed", &format!("{after:012X}"));
                    set_hidden("donerow", true);
                    show("out3", &format!("Player seed updated to <span class=\"mono\">{after:012X}</span>. Pick the next item."), "good");
                }
            }
            _ => {}
        }
    });
    let _ = root.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
    on_click.forget();
    let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        if t.id() == "titem" {
            if let (Some(l), Some(item)) = (by_id("tlevels"), data().item(&value_of(&t))) {
                l.set_inner_html(&levels_html(item));
            }
        } else if t.class_list().contains("it")
            && let (Some(row), Some(item)) = (t.closest(".obsrow").ok().flatten(), data().item(&value_of(&t)))
        {
            for k in 0..3 {
                if let Ok(Some(h)) = row.query_selector(&format!(".h{k}")) {
                    h.set_inner_html(&hint_options(item));
                }
            }
        }
    });
    let _ = root.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
    on_change.forget();
}

fn crack1() {
    let obs = match read_obs("obs1") {
        Ok(o) => o,
        Err(e) => return show("out1", &esc(&e), "bad"),
    };
    if obs.is_empty() {
        return show("out1", "Add at least one reading.", "bad");
    }
    let prior = ENCHANT.with(|s| s.borrow().cand1.clone());
    if let Some((before, seeds)) = prior.filter(|(before, _)| before.len() <= obs.len() && obs[..before.len()] == before[..]) {
        let _ = before;
        let found = filter(data(), &seeds, &obs);
        return finish1(obs, found);
    }
    if obs.len() < 2 && !has_clues(&obs) {
        return show("out1", "One reading without hints leaves millions of seeds. Add a second reading with another bookshelf count.", "bad");
    }
    show("out1", "Searching all 4,294,967,296 seeds…", "");
    let kept = obs.clone();
    scan_all(obs, "prog1", "crack1", move |found| finish1(kept, found));
}

fn finish1(obs: Vec<Observation>, found: Vec<i32>) {
    let n = found.len();
    if n == 1 {
        set_value("xp1", &hex8(found[0]));
    }
    ENCHANT.with(|s| s.borrow_mut().cand1 = (n <= KEEP).then(|| (obs, found.clone())));
    match n {
        1 => show("out1", &format!("Enchanting seed 1 = <span class=\"mono big\">{}</span>. Now do step 2.", hex8(found[0])), "good"),
        0 => show("out1", "No seed matches. Check the numbers, and that nothing was enchanted between the readings.", "bad"),
        _ => show("out1", &format!("{} seeds still match. Add another reading (another bookshelf count) and press again.", group(n as u64)), ""),
    }
}

fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn crack2() {
    let xp1 = match parse_seed("xp1") {
        Ok(Some(s)) => s,
        Ok(None) => return show("out2", "Find or enter enchanting seed 1 first.", "bad"),
        Err(e) => return show("out2", &e, "bad"),
    };
    let xp2 = match parse_seed("xp2") {
        Ok(s) => s,
        Err(e) => return show("out2", &e, "bad"),
    };
    let obs = match read_obs("obs2") {
        Ok(o) => o,
        Err(e) => return show("out2", &esc(&e), "bad"),
    };
    if !obs.is_empty() || xp2.is_none() {
        if obs.is_empty() {
            return show("out2", "Add a reading after the cheap enchant (or enter enchanting seed 2).", "bad");
        }
        let mut c = next_seed_candidates(xp1);
        c.sort_unstable();
        c.dedup();
        let found = filter(data(), &c, &obs);
        match found.len() {
            1 => {
                set_value("xp2", &hex8(found[0]));
                return player_seed(xp1, found[0]);
            }
            n if n > 1 => return show("out2", &format!("{n} seeds still match. Add another reading."), ""),
            _ => {}
        }
        if obs.len() < 2 && !has_clues(&obs) {
            return show("out2", "Seed 2 does not follow seed 1 directly. Add a second reading so a full search can find it.", "bad");
        }
        show("out2", "Seed 2 does not follow seed 1 directly (something else used the player's random generator in between). Searching all seeds…", "");
        scan_all(obs, "prog2", "crack2", move |found| {
            if found.len() == 1 {
                set_value("xp2", &hex8(found[0]));
                player_seed(xp1, found[0]);
            } else {
                show("out2", &format!("{} seeds match. Add another reading.", group(found.len() as u64)), if found.is_empty() { "bad" } else { "" });
            }
        });
        return;
    }
    if let Some(xp2) = xp2 {
        player_seed(xp1, xp2);
    }
}

fn player_seed(xp1: i32, xp2: i32) {
    let mut res = crack_player(xp1, xp2, 0);
    let mut note = String::new();
    if res.is_empty() {
        res = crack_player(xp1, xp2, 2000);
        if !res.is_empty() {
            note = format!(" (something else drew {} times from the player's random generator between the two enchants; that is fine)", res.iter().map(|r| r.1.to_string()).collect::<Vec<_>>().join(" or "));
        }
    }
    match res.len() {
        1 => {
            set_value("pseed", &format!("{:012X}", res[0].0));
            show("out2", &format!("Enchanting seed 2 = <span class=\"mono\">{}</span>. Player seed = <span class=\"mono big\">{:012X}</span>{note}. Now do step 3.", hex8(xp2), res[0].0), "good");
        }
        0 => show("out2", "These two seeds are not from enchants in a row. Enchant once more and use seed 2 as the new seed 1.", "bad"),
        _ => show("out2", "Several player seeds fit. Enchant once more and use the last two seeds.", "bad"),
    }
}

fn calculate() {
    let d = data();
    let state = value("pseed");
    let Ok(state) = u64::from_str_radix(&state, 16).map_err(|_| ()).and_then(|s| if value("pseed").len() == 12 { Ok(s) } else { Err(()) }) else {
        return show("out3", "Find (or enter) the player seed in step 2 first: 12 hex digits.", "bad");
    };
    let Some(item) = d.item(&value("titem")) else { return show("out3", "Pick an item.", "bad") };
    let (mut wanted, mut unwanted) = (Vec::new(), Vec::new());
    if let Ok(groups) = doc().query_selector_all("#tlevels .lv") {
        for i in 0..groups.length() {
            let Some(g) = groups.item(i).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
            let Some(e) = g.get_attribute("data-ench").and_then(|id| d.enchant(&id)) else { continue };
            let on = g.query_selector(".on").ok().flatten().and_then(|b| b.get_attribute("data-val")).unwrap_or_default();
            match on.as_str() {
                "no" => unwanted.push(e),
                "any" | "" => {}
                l => wanted.push((e, l.parse::<i32>().unwrap_or(1))),
            }
        }
    }
    if wanted.is_empty() {
        return show("out3", "Pick at least one enchantment level.", "bad");
    }
    let num = |id: &str, dflt: i64| value(id).parse::<i64>().unwrap_or(dflt);
    let max_drops = num("tmaxdrops", 2048).clamp(0, 200_000) as u32;
    let goal = Goal { item, wanted, unwanted, max_shelves: num("tmaxsh", 15).clamp(0, 15) as i32, max_drops, player_level: num("tlevel", 999) as i32 };
    let Some(p) = plan(d, state, &goal) else {
        set_hidden("donerow", true);
        return show("out3", &format!("Not possible within {} drops. Raise the most drops or ask for less.", group(max_drops as u64)), "bad");
    };
    let mut steps = Vec::new();
    if let Some(n) = p.drops {
        let stacks = if n >= 64 { format!(": {} stack{} of 64 and {} more; hold them and press Q until they are gone", n / 64, if n >= 128 { "s" } else { "" }, n % 64) } else { String::new() };
        steps.push(format!("Drop <b>{n}</b> item{} one at a time with Q{stacks} (any item; not Ctrl+Q, which throws a whole stack at once).", if n == 1 { "" } else { "s" }));
        steps.push("Enchant any cheap item once (a throwaway).".to_string());
    }
    steps.push(format!(
        "Set <b>{}</b> bookshelves, put in the {}, click slot <b>{}</b> (it needs {} levels and uses {} lapis).",
        p.shelves,
        esc(&words(&d.items[item].name)),
        p.slot + 1,
        p.cost,
        p.slot + 1
    ));
    let got: Vec<String> = p.enchants.iter().map(|&e| esc(&words(&rng::table::format_ench(d, e)))).collect();
    let list: String = steps.iter().map(|s| format!("<li>{s}</li>")).collect();
    show("out3", &format!("<ol>{list}</ol>You get: <b>{}</b>", got.join(", ")), "good");
    ENCHANT.with(|s| s.borrow_mut().after = Some(p.state_after));
    set_hidden("donerow", false);
}

// ------------------------------------------------------------------------------------------------
// the librarian calculator

fn seen_row(n: usize) -> String {
    let d = data();
    let books: String = d.tags.get("minecraft:tradeable").into_iter().flatten().map(|&e| format!("<option value=\"{}\">{}</option>", esc(&d.enchants[e as usize].id), esc(&words(&d.enchants[e as usize].name)))).collect();
    format!(
        "<div class=\"row seenrow\"><span class=\"n\">screen {}:</span> <label>book <select class=\"b\"><option value=\"\">no book</option>{books}</select></label> <label>level <input class=\"num l\" inputmode=\"numeric\" value=\"1\"></label> <label>price <input class=\"num p\" inputmode=\"numeric\" placeholder=\"emeralds\"></label> <button type=\"button\" class=\"small\" data-act=\"rm-row\" aria-label=\"remove this screen\">×</button></div>",
        n + 1
    )
}

fn librarian_install(root: &Element) {
    add_row("seen", seen_row);
    add_row("seen", seen_row);
    let on_click = Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| {
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        let Some(b) = t.closest("[data-act]").ok().flatten() else { return };
        match b.get_attribute("data-act").unwrap_or_default().as_str() {
            "add-seen" => add_row("seen", seen_row),
            "rm-row" => {
                if let Some(row) = b.closest(".row").ok().flatten() {
                    row.remove();
                }
            }
            "vsync" => vsync(),
            "vplan" => vplan(),
            "vlist" => vlist(),
            _ => {}
        }
    });
    let _ = root.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
    on_click.forget();
}

fn world() -> Result<(i64, i32), String> {
    let ws = value("wseed");
    let seed = ws.parse::<i64>().map_err(|_| "Enter the world seed: the number /seed prints.".to_string())?;
    Ok((seed, value("vlevel").parse::<i32>().unwrap_or(1)))
}

fn offer_text(o: &Offer) -> String {
    esc(&words(&o.describe(data())))
}

fn screens(rows: &[(String, Vec<Offer>)], last: bool) -> String {
    let body: String = rows
        .iter()
        .enumerate()
        .map(|(i, (label, offers))| format!("<tr{}><td class=\"mono\">{}</td><td>{}</td></tr>", if last && i + 1 == rows.len() { " class=\"hl\"" } else { "" }, esc(label), offers.iter().map(offer_text).collect::<Vec<_>>().join("<br>")))
        .collect();
    format!("<table class=\"screens\">{body}</table>")
}

fn vsync() {
    let d = data();
    let (seed, level) = match world() {
        Ok(w) => w,
        Err(e) => return show("vout1", &e, "bad"),
    };
    let Ok(set) = trade_set(d, "librarian", level) else { return show("vout1", "No librarian trades at that level.", "bad") };
    let mut seen = Vec::new();
    if let Ok(rows) = doc().query_selector_all("#seen .seenrow") {
        for i in 0..rows.length() {
            let Some(row) = rows.item(i).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
            let mut o = ObservedTrades::default();
            match d.enchant(&field(&row, "b")) {
                Some(e) => o.book = Some((e, field(&row, "l").parse::<u8>().unwrap_or(1), field(&row, "p").parse::<i32>().ok())),
                None => o.no_book = true,
            }
            seen.push(o);
        }
    }
    if seen.is_empty() {
        return show("vout1", "Add the screens you saw.", "bad");
    }
    let hits = match sync(d, set, seed, SequenceSettings::default(), &seen, 200_000) {
        Ok(h) => h,
        Err(e) => return show("vout1", &esc(&e), "bad"),
    };
    let Some(&first) = hits.first() else {
        return show("vout1", "No match in the first 200,000 rerolls of this world. Check the world seed, the villager's level and the screens (a price with a discount: leave it empty).", "bad");
    };
    let last = first + seen.len() as u64 - 1;
    set_value("vafter", &last.to_string());
    if hits.len() == 1 {
        show("vout1", &format!("Found it: your screens were entries {first} to {last}. The next reroll gives entry {}.", last + 1), "good");
    } else {
        let some: Vec<String> = hits.iter().take(8).map(|h| h.to_string()).collect();
        show("vout1", &format!("{} places match ({}{}). Using the first; add one more screen to be sure.", hits.len(), some.join(", "), if hits.len() > 8 { ", …" } else { "" }), "");
    }
}

fn vplan() {
    let d = data();
    let (seed, level) = match world() {
        Ok(w) => w,
        Err(e) => return show("vout2", &e, "bad"),
    };
    let Ok(after) = value("vafter").parse::<u64>() else { return show("vout2", "Find your place first (or type the last entry you saw).", "bad") };
    let Some(want) = d.enchant(&value("vwant")) else { return show("vout2", "Pick a book.", "bad") };
    let min_level = value("vminlvl").parse::<u8>().unwrap_or(1);
    let max_price = value("vmaxprice").parse::<i32>().ok();
    let Ok(set) = trade_set(d, "librarian", level) else { return show("vout2", "No librarian trades at that level.", "bad") };
    match plan_rerolls(d, set, seed, SequenceSettings::default(), after, &Want { enchant: want, min_level, max_price }, 100_000) {
        Err(e) => show("vout2", &esc(&e), "bad"),
        Ok(None) => show("vout2", "Not within 100,000 rerolls.", "bad"),
        Ok(Some((n, path))) => {
            let mut rows: Vec<(String, Vec<Offer>)> = path.iter().enumerate().take(12).map(|(i, o)| (format!("reroll {} (entry {})", i + 1, after + 1 + i as u64), o.clone())).collect();
            if path.len() > 12 {
                rows.truncate(11);
                rows.push(("…".to_string(), Vec::new()));
                rows.push((format!("reroll {n} (entry {})", after + n), path[path.len() - 1].clone()));
            }
            show("vout2", &format!("<span class=\"big\">Reroll {n} time{}.</span> Open the trade screen after every reroll so each one counts, and check that it matches:{}", if n == 1 { "" } else { "s" }, screens(&rows, true)), "good");
        }
    }
}

fn vlist() {
    let d = data();
    let (seed, level) = match world() {
        Ok(w) => w,
        Err(e) => return show("vout1", &e, "bad"),
    };
    let Ok(set) = trade_set(d, "librarian", level) else { return show("vout1", "No librarian trades at that level.", "bad") };
    let Ok(mut g) = Generations::new(d, set, seed, SequenceSettings::default()) else { return show("vout1", "No trades for that level.", "bad") };
    let mut rows = Vec::new();
    for _ in 0..20 {
        let i = g.index;
        match g.next_generation() {
            Ok(o) => rows.push((format!("entry {i}"), o)),
            Err(e) => return show("vout1", &esc(&e), "bad"),
        }
    }
    show("vout1", &screens(&rows, false), "");
}
