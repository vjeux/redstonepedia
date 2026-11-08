//! REDSTONEPEDIA's filtering UI, in the browser (Rust compiled to WebAssembly).
//!
//! index.html holds the empty shell (`#filters`, `#summary`, `#grid`, `#more`); this module
//! fetches `data/site.json` ([`site_model::Site`]), draws the filter panel, runs every
//! [`site_model::Query`] with the engine of crates/site-model (unit-tested natively) and draws
//! the result cards with the same markup as the static pages ([`site_model::html`]). The query
//! lives in the URL fragment, so every search is a link. The cards' 3D thumbnails are
//! `data-mcview` elements that webrender's viewer (viewer/mcview.js) renders when it is loaded.

use site_model::html::{chip_plural, esc, hit_card_run, nearest_searches};
use site_model::{FLAGS, Facets, Hit, Query, Range, Site, Sort, flag_label, gt_time};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, HtmlSelectElement};

mod swap;

/// Cards drawn per page (more on scroll).
const PAGE: usize = 60;

struct App {
    site: Site,
    q: Query,
    hits: Vec<Hit>,
    facets: Facets,
    shown: usize,
    /// The block-kind chips' own filter text.
    kind_filter: String,
    error: String,
    /// The chips most of the first hits share: said once in the count line, left off the cards.
    shared: Vec<String>,
    /// The kind line of the last card drawn (a run of cards shows it only where it changes).
    last_kind: Option<String>,
    /// The period of the last clock card drawn: a heading opens each period (vjeux 01:24Z: every
    /// clock as a card, "grouped by timing").
    last_period: Option<u32>,
    /// The hits per period (each heading's count), and the clocks per period for the filters but
    /// the time (the Clocks tab's jump bar: critique, the 5-tick heading sat 20 phone screens down).
    period_counts: BTreeMap<u32, usize>,
    jump_counts: BTreeMap<u32, usize>,
    /// Every group chip shown, not only the top ten.
    all_groups: bool,
    /// How many of the hits name what the search's words name (the rest only mention them).
    named: usize,
    /// What each tab's cards hold with no filter (Site::holds: doors, clocks, circuits, farms).
    holds: [usize; 4],
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn doc() -> Document {
    web_sys::window().and_then(|w| w.document()).expect("a document")
}

fn by_id(id: &str) -> Option<Element> {
    doc().get_element_by_id(id)
}

fn input(id: &str) -> Option<HtmlInputElement> {
    by_id(id).and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
}

fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

#[wasm_bindgen(start)]
pub fn start() {
    // the build and family pages: in-place swaps (tunable-group pickers, family member rows)
    if by_id("filters").is_none() {
        if let Err(e) = swap::install() {
            log(&format!("redstonepedia: {e:?}"));
        }
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = load().await {
            log(&format!("redstonepedia: {e:?}"));
            if let Some(f) = by_id("filters") {
                f.set_inner_html(&format!("<p class=\"error\">Could not load the data: {}. <a href=\"circuits/index.html\">The circuits</a> work without it.</p>", esc(&format!("{e:?}"))));
            }
        }
    });
}

async fn load() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    // the data's URL carries its content hash (cache-busting): #filters' data-src
    let src = by_id("filters").and_then(|f| f.get_attribute("data-src")).unwrap_or_else(|| "data/site.json".to_string());
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(&src)).await?.dyn_into()?;
    if !resp.ok() {
        return Err(JsValue::from_str(&format!("data/site.json: HTTP {}", resp.status())));
    }
    let text = JsFuture::from(resp.text()?).await?.as_string().ok_or("data/site.json: not text")?;
    let site: Site = serde_json::from_str(&text).map_err(|e| JsValue::from_str(&format!("data/site.json: {e}")))?;
    let hash = window.location().hash().unwrap_or_default();
    let mut q = Query::from_fragment(&hash);
    // the header's search form submitted from another page: index.html?kind=..&q=..
    let search = window.location().search().unwrap_or_default();
    let mut from_form = false;
    for part in search.trim_start_matches('?').split('&') {
        if let Some((k, v)) = part.split_once('=') {
            let v = site_model::decode(&v.replace('+', " "));
            match k {
                "kind" if !v.is_empty() => {
                    q.kind = v;
                    from_form = true;
                }
                "q" => {
                    route_search(&mut q, &v);
                    from_form = true;
                }
                _ => {}
            }
        }
    }
    let holds = site.holds();
    APP.with(|a| *a.borrow_mut() = Some(App { site, q, hits: Vec::new(), facets: Facets::default(), shown: 0, kind_filter: String::new(), error: String::new(), shared: Vec::new(), last_kind: None, last_period: None, period_counts: BTreeMap::new(), jump_counts: BTreeMap::new(), all_groups: false, named: 0, holds }));
    draw_panel()?;
    sync_inputs();
    refresh(true);
    if from_form || hash.contains("time=") {
        if let Some(t) = input("hq") {
            let _ = t.focus();
        }
    }
    Ok(())
}

/// The header's search text into the query: on the Clocks tab (or with an explicit time unit) a
/// period or a time range is the clock time search (`48`, `2.5 s`, `40-60 gt`); anything else is
/// the text search.
fn route_search(q: &mut Query, s: &str) {
    let t = s.trim();
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    let lower = t.to_lowercase();
    // the Circuits tab: a value (`12`, `L 12`, `N 3`, `10-20 gt`, `latency 0`) is the circuit
    // value search, which expands the tunable groups to the member that hits it
    if q.kind == "circuit" && has_digit && site_model::parse_value_query(t).is_ok() {
        q.value = t.to_string();
        q.text.clear();
        q.time.clear();
        return;
    }
    q.value.clear();
    let explicit_unit = ["gt", "tick", " s", "sec", "min", "rt"].iter().any(|u| lower.contains(u)) || lower.ends_with('s');
    let is_time = has_digit && site_model::parse_time_query(t).is_ok() && (q.kind == "clock" || explicit_unit);
    if is_time {
        if q.kind != "clock" {
            q.kind = "clock".into();
            q.groups.clear();
        }
        q.time = t.to_string();
        q.text.clear();
    } else {
        q.text = t.to_string();
        q.time.clear();
    }
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.borrow_mut().as_mut().map(f))
}

/// The static part of the panel (inputs keep their focus while typing), its listeners, and the
/// summary bar.
fn draw_panel() -> Result<(), JsValue> {
    let filters = by_id("filters").ok_or("no #filters")?;
    let range = |id: &str, label: &str| -> String {
        format!(
            "<div class=\"range-label\">{label}</div><div class=\"range\"><input id=\"f-{id}-min\" type=\"number\" min=\"0\" placeholder=\"min\" aria-label=\"{label} min\"><input id=\"f-{id}-max\" type=\"number\" min=\"0\" placeholder=\"max\" aria-label=\"{label} max\"></div>"
        )
    };
    let html = format!(
        "<button id=\"f-toggle\" class=\"ftoggle\" type=\"button\" aria-expanded=\"false\">Filters</button>\
         <h3 id=\"f-groups-title\">Category</h3><div id=\"f-groups\" class=\"chips\"></div>\
         <h3>Block kinds</h3><p class=\"hint\">Click: must use it. Again: must not. Again: either.</p><input id=\"f-kind-filter\" type=\"search\" placeholder=\"filter the block kinds\" aria-label=\"filter block kinds\"><div id=\"f-blocks\" class=\"chips\"></div>\
         <h3>Properties</h3><div id=\"f-flags\" class=\"chips\"></div>\
         <h3>Ranges</h3>{}{}{}{}{}\
         <label class=\"check\"><input id=\"f-variants\" type=\"checkbox\"> Show variants (designs that only add block kinds to a smaller one) and archived family members</label>\
         <button id=\"f-reset\" class=\"reset\" type=\"button\">Reset all filters</button>",
        range("blocks", "Blocks placed"),
        range("volume", "Volume"),
        range("period", "Clock period (gt)"),
        range("open", "Door opens in (gt)"),
        range("close", "Door closes in (gt)")
    );
    filters.set_inner_html(&html);
    // narrow screens: the panel starts folded under its "Filters" button
    let _ = filters.class_list().add_1("collapsed");
    if let Some(t) = by_id("f-toggle") {
        listen(&t, "click", |_| {
            if let Some(f) = by_id("filters") {
                let folded = f.class_list().toggle("collapsed").unwrap_or(false);
                if let Some(t) = by_id("f-toggle") {
                    let _ = t.set_attribute("aria-expanded", if folded { "false" } else { "true" });
                }
            }
        })?;
    }
    let sorts: String = Sort::ALL.iter().map(|(_, k, label)| format!("<option value=\"{k}\">{label}</option>")).collect();
    if let Some(s) = by_id("summary") {
        s.set_inner_html(&format!("<span id=\"f-count\"></span><label>Sort by <select id=\"f-sort\">{sorts}</select></label><nav id=\"f-periods\" class=\"periods\" aria-label=\"periods\" hidden></nav>"));
        // the jump bar's list of every period (rebuilt with each refresh: one delegated listener)
        listen(&s, "change", |e| {
            let Some(t) = e.target().and_then(|t| t.dyn_into::<HtmlSelectElement>().ok()) else { return };
            if t.id() != "f-period" {
                return;
            }
            let v = t.value();
            with_app(|a| {
                a.q.kind = "clock".into();
                a.q.time = v;
            });
            sync_inputs();
            refresh(true);
        })?;
    }
    // one delegated click listener for every chip and tab
    listen(&filters, "click", |e| {
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        let Ok(Some(el)) = t.closest("[data-act]") else { return };
        let act = el.get_attribute("data-act").unwrap_or_default();
        let val = el.get_attribute("data-val").unwrap_or_default();
        with_app(|a| {
            let q = &mut a.q;
            match act.as_str() {
                "kind" => {
                    // the main number of each kind leads (doors: size; clocks: period; circuits:
                    // kind and value); a sort that only means something for doors falls back to it
                    if val != "door" && (q.sort == Sort::Volume || q.sort == Sort::Open || q.sort == Sort::Close) {
                        q.sort = Sort::Main;
                    }
                    q.kind = val;
                    q.groups.clear();
                }
                "group" => toggle(&mut q.groups, &val),
                "block" => tri(&mut q.kinds_in, &mut q.kinds_out, &val),
                "flag" => tri(&mut q.flags_in, &mut q.flags_out, &val),
                _ => {}
            }
            if act == "groups-all" {
                a.all_groups = !a.all_groups;
            }
        });
        // a tab click changed the sort: show it
        if let Some(s) = by_id("f-sort").and_then(|e| e.dyn_into::<HtmlSelectElement>().ok()) {
            if let Some(k) = with_app(|a| a.q.sort.key()) {
                s.set_value(k);
            }
        }
        refresh(true);
    })?;
    for id in ["f-kind-filter"] {
        if let Some(el) = by_id(id) {
            listen(&el, "input", |_| {
                read_inputs();
                refresh(true);
            })?;
        }
    }
    // the header's search box: live on this page (the form only navigates from other pages)
    if let Some(el) = by_id("hq") {
        listen(&el, "input", |_| {
            let s = input("hq").map(|i| i.value()).unwrap_or_default();
            with_app(|a| route_search(&mut a.q, &s));
            refresh(true);
        })?;
    }
    if let Some(form) = by_id("hsearch") {
        listen(&form, "submit", |e| e.prevent_default())?;
    }
    for id in ["blocks", "volume", "period", "open", "close"] {
        for end in ["min", "max"] {
            if let Some(el) = by_id(&format!("f-{id}-{end}")) {
                listen(&el, "input", |_| {
                    read_inputs();
                    refresh(true);
                })?;
            }
        }
    }
    for id in ["f-variants", "f-sort"] {
        if let Some(el) = by_id(id) {
            listen(&el, "change", |_| {
                read_inputs();
                refresh(true);
            })?;
        }
    }
    if let Some(el) = by_id("f-reset") {
        listen(&el, "click", |_| {
            with_app(|a| {
                a.q = Query::default();
                a.kind_filter.clear();
            });
            sync_inputs();
            refresh(true);
        })?;
    }
    if let Some(el) = by_id("more") {
        listen(&el, "click", |_| more())?;
        // infinite scroll: the button loads the next page when it comes into view
        let cb = Closure::<dyn FnMut(js_sys::Array)>::new(|entries: js_sys::Array| {
            let visible = entries.iter().any(|e| e.dyn_into::<web_sys::IntersectionObserverEntry>().map(|x| x.is_intersecting()).unwrap_or(false));
            if visible {
                more();
            }
        });
        let opts = web_sys::IntersectionObserverInit::new();
        opts.set_root_margin("600px");
        let io = web_sys::IntersectionObserver::new_with_options(cb.as_ref().unchecked_ref(), &opts)?;
        io.observe(&el);
        cb.forget();
        std::mem::forget(io);
    }
    // nav links to index.html#... on this page (the header tabs), back/forward
    let window = web_sys::window().ok_or("no window")?;
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(|_e: web_sys::Event| {
        let hash = web_sys::window().map(|w| w.location().hash().unwrap_or_default()).unwrap_or_default();
        let q = Query::from_fragment(&hash);
        let same = with_app(|a| a.q == q).unwrap_or(true);
        if !same {
            with_app(|a| a.q = q);
            sync_inputs();
            refresh(false);
        }
    });
    window.add_event_listener_with_callback("hashchange", cb.as_ref().unchecked_ref())?;
    cb.forget();
    Ok(())
}

fn listen(el: &Element, event: &str, f: impl FnMut(web_sys::Event) + 'static) -> Result<(), JsValue> {
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    el.add_event_listener_with_callback(event, cb.as_ref().unchecked_ref())?;
    cb.forget();
    Ok(())
}

fn toggle(set: &mut BTreeSet<String>, v: &str) {
    if !set.remove(v) {
        set.insert(v.to_string());
    }
}

/// either -> must -> must not -> either.
fn tri(inc: &mut BTreeSet<String>, exc: &mut BTreeSet<String>, v: &str) {
    if inc.remove(v) {
        exc.insert(v.to_string());
    } else if !exc.remove(v) {
        inc.insert(v.to_string());
    }
}

fn num(id: &str) -> Option<i64> {
    input(id).and_then(|i| i.value().trim().parse::<i64>().ok())
}

/// Read the static inputs into the query (the header's search box is routed on its own).
fn read_inputs() {
    let kind_filter = input("f-kind-filter").map(|i| i.value()).unwrap_or_default();
    let variants = input("f-variants").map(|i| i.checked()).unwrap_or(false);
    let sort = by_id("f-sort").and_then(|e| e.dyn_into::<HtmlSelectElement>().ok()).map(|s| Sort::from_key(&s.value())).unwrap_or_default();
    let r = |id: &str| Range { min: num(&format!("f-{id}-min")), max: num(&format!("f-{id}-max")) };
    let (blocks, volume, period, open, close) = (r("blocks"), r("volume"), r("period"), r("open"), r("close"));
    with_app(|a| {
        let q = &mut a.q;
        q.variants = variants;
        q.sort = sort;
        q.blocks = blocks;
        q.volume = volume;
        q.period = period;
        q.open = open;
        q.close = close;
        a.kind_filter = kind_filter;
    });
}

/// Write the query into the static inputs (after a reset or a fragment change).
fn sync_inputs() {
    let Some((q, kf)) = with_app(|a| (a.q.clone(), a.kind_filter.clone())) else { return };
    let set = |id: &str, v: &str| {
        if let Some(i) = input(id) {
            if i.value() != v {
                i.set_value(v);
            }
        }
    };
    set("hq", if !q.value.is_empty() { &q.value } else if q.time.is_empty() { &q.text } else { &q.time });
    set("f-kind-filter", &kf);
    let opt = |v: Option<i64>| v.map(|x| x.to_string()).unwrap_or_default();
    for (id, r) in [("blocks", q.blocks), ("volume", q.volume), ("period", q.period), ("open", q.open), ("close", q.close)] {
        set(&format!("f-{id}-min"), &opt(r.min));
        set(&format!("f-{id}-max"), &opt(r.max));
    }
    if let Some(i) = input("f-variants") {
        i.set_checked(q.variants);
    }
    if let Some(s) = by_id("f-sort").and_then(|e| e.dyn_into::<HtmlSelectElement>().ok()) {
        s.set_value(q.sort.key());
    }
}

/// Run the query and redraw the dynamic parts; `write_hash`: record the query in the URL.
fn refresh(write_hash: bool) {
    let done = with_app(|a| {
        match a.site.run(&a.q) {
            Ok((hits, facets)) => {
                // the chips most of the first cards share, said once in the count line
                a.shared = site_model::html::hits_common_chips(&a.site, &hits);
                // where the cards that only mention the words begin
                a.named = a.site.named_count(&a.q, &hits);
                a.hits = hits;
                a.facets = facets;
                a.error.clear();
            }
            Err(e) => a.error = e,
        }
        a.shown = 0;
        a.last_kind = None;
        a.last_period = None;
        a.period_counts = count_periods(&a.site, &a.hits);
        a.jump_counts = if a.q.kind == "clock" {
            let mut q = a.q.clone();
            q.time.clear();
            a.site.run(&q).map(|(h, _)| count_periods(&a.site, &h)).unwrap_or_default()
        } else {
            BTreeMap::new()
        };
    });
    if done.is_none() {
        return;
    }
    if write_hash {
        let frag = with_app(|a| a.q.to_fragment()).unwrap_or_default();
        if let Some(w) = web_sys::window() {
            if let Ok(h) = w.history() {
                let path = w.location().pathname().unwrap_or_default();
                let url = if frag.is_empty() { path } else { format!("#{frag}") };
                let _ = h.replace_state_with_url(&JsValue::NULL, "", Some(&url));
            }
        }
    }
    draw_dynamic();
    if let Some(g) = by_id("grid") {
        g.set_inner_html("");
    }
    more();
}

/// Append the next page of cards (or say why there is none).
fn more() {
    let Some((html, shown, total)) = with_app(|a| {
        if !a.error.is_empty() {
            return (String::new(), 0, 0);
        }
        if a.hits.is_empty() {
            return (empty_message(a), 0, 0);
        }
        let end = (a.shown + PAGE).min(a.hits.len());
        // (a run of cards: the home page shows a kind line only where the kind changes)
        let mut prev = a.last_kind.take();
        let mut html = String::new();
        // (clocks sorted by period: one heading per period, vjeux 01:24Z)
        let by_period = matches!(a.q.sort, site_model::Sort::Main | site_model::Sort::Period);
        for (k, h) in a.hits[a.shown..end].iter().enumerate() {
            // the cards that only mention the search's words, under their own line
            if a.named > 0 && a.shown + k == a.named {
                html.push_str(&format!("<p class=\"also\">Also mentioning “{}”:</p>", esc(a.q.text.trim())));
                prev = None;
            }
            if by_period {
                if let Some(p) = a.site.hit_period(*h).filter(|p| a.last_period != Some(*p)) {
                    // (each heading links its period and says how many clocks it has: critique)
                    let n = a.period_counts.get(&p).copied().unwrap_or(0);
                    html.push_str(&format!(
                        "<h3 class=\"period\" id=\"p{p}\"><a href=\"#kind=clock&amp;time={p}\">Every {}</a> <span class=\"count\">{n} clock{}</span></h3>",
                        esc(&site_model::html::keep_units(&site_model::gt_time(p))),
                        if n == 1 { "" } else { "s" }
                    ));
                    a.last_period = Some(p);
                    prev = None;
                }
                // under its period's heading a clock's card leads with its blocks and parts
                if let site_model::Hit::Build(i) = h {
                    if let Some(b) = a.site.builds.get(*i).filter(|b| b.clock.is_some()) {
                        html.push_str(&site_model::html::clock_card_in_period(b, "", &a.shared));
                        continue;
                    }
                }
            }
            html.push_str(&hit_card_run(&a.site, *h, "", &a.q.kind, &a.shared, &mut prev));
        }
        a.last_kind = prev;
        a.shown = end;
        (html, end, a.hits.len())
    }) else {
        return;
    };
    if let Some(g) = by_id("grid") {
        if !html.is_empty() {
            let _ = g.insert_adjacent_html("beforeend", &html);
        }
    }
    if let Some(m) = by_id("more").and_then(|e| e.dyn_into::<HtmlElement>().ok()) {
        m.set_hidden(shown >= total);
        m.set_text_content(Some(&format!("Show more ({} more)", total.saturating_sub(shown))));
    }
}

/// The hits per period (a clock's), for the Clocks tab's headings and jump bar.
fn count_periods(site: &site_model::Site, hits: &[Hit]) -> BTreeMap<u32, usize> {
    let mut m = BTreeMap::new();
    for h in hits {
        if let Some(p) = site.hit_period(*h) {
            *m.entry(p).or_insert(0) += 1;
        }
    }
    m
}

/// The Clocks tab's jump bar (critique: after "Show more", the second heading sat ~16,800 px
/// down): a chip for each landmark period that has clocks, with its count, then every period in
/// a list; nothing folded.
fn jump_bar(counts: &BTreeMap<u32, usize>, current: Option<u32>) -> String {
    if counts.len() < 2 {
        return String::new();
    }
    const MARKS: [u32; 15] = [4, 5, 6, 8, 10, 20, 40, 100, 200, 400, 1200, 2400, 6000, 12000, 72000];
    let chips: String = MARKS
        .iter()
        .filter_map(|p| counts.get(p).map(|n| (*p, *n)))
        .map(|(p, n)| {
            let label = if p < 20 { format!("{p} ticks") } else { site_model::real_time(p) };
            format!("<a class=\"chip{}\" href=\"#kind=clock&amp;time={p}\">{} <span class=\"n\">{n}</span></a>", if current == Some(p) { " on" } else { "" }, esc(&site_model::html::keep_units(&label)))
        })
        .collect();
    let options: String = counts
        .iter()
        .map(|(p, n)| format!("<option value=\"{p}\"{}>{} · {n} clock{}</option>", if current == Some(*p) { " selected" } else { "" }, esc(&gt_time(*p)), if *n == 1 { "" } else { "s" }))
        .collect();
    // (the list first, the landmarks after it in a strip that scrolls sideways on a phone: one
    // line, so the tab's first phone screen shows cards, webrender's profile)
    format!("<select id=\"f-period\" aria-label=\"Jump to a period\"><option value=\"\">Jump to a period…</option>{options}</select><span class=\"jump-chips\">{chips}</span>")
}

/// The one period a time search names, if it names one.
fn current_period(q: &Query) -> Option<u32> {
    match site_model::parse_time_query(&q.time) {
        Ok((lo, hi)) if lo == hi => Some(lo),
        _ => None,
    }
}

/// A search that names one gate function (`xor`, `xor gate`, `nand`): the function.
fn gate_word(text: &str) -> Option<&'static str> {
    let t = text.trim().to_lowercase();
    let t = t.strip_suffix(" gate").unwrap_or(&t).trim().to_string();
    ["xor", "xnor", "and", "nand", "or", "nor", "not", "imply", "nimply"].into_iter().find(|f| *f == t)
}

/// No result: for a time search, the nearest periods that exist (links); else a hint.
fn empty_message(a: &App) -> String {
    if !a.q.time.trim().is_empty() {
        if let Ok((lo, hi)) = site_model::parse_time_query(&a.q.time) {
            let near = a.site.nearest_periods(lo, hi, 6);
            let links: Vec<String> = near
                .iter()
                .map(|(p, n)| {
                    let mut q = a.q.clone();
                    q.time = format!("{p} gt");
                    format!("<a href=\"#{}\">{}</a> ({n})", esc(&q.to_fragment()), esc(&gt_time(*p)))
                })
                .collect();
            let what = if lo == hi { gt_time(lo) } else { format!("{} to {}", gt_time(lo), gt_time(hi)) };
            return format!("<p class=\"empty\">No clock or family member with a period of {} matches. Nearest periods that exist: {}.</p>", esc(&what), links.join(", "));
        }
    }
    "<p class=\"empty\">Nothing matches these filters. Remove one, or reset them all.</p>".to_string()
}

fn chip(act: &str, val: &str, label: &str, count: usize, state: &str) -> String {
    format!(
        "<button type=\"button\" class=\"chip {state}{}\" data-act=\"{act}\" data-val=\"{}\">{}<span class=\"count\">{count}</span></button>",
        if count == 0 && state.is_empty() { " zero" } else { "" },
        esc(val),
        esc(label)
    )
}

/// Tabs, chips, the time hint and the result count.
fn draw_dynamic() {
    let Some(parts) = with_app(|a| {
        let q = &a.q;
        let f = &a.facets;
        // the header's tabs: the active kind and each kind's count under the other filters; with
        // no filter, what each tab's cards hold (11 door records, 172 clock periods, 39 circuit
        // kinds: critique, "Doors 1 · Clocks 1" read as one door and one clock)
        let r0 = |r: &site_model::Range| r.min.is_none() && r.max.is_none();
        let plain = q.text.trim().is_empty() && q.time.trim().is_empty() && q.value.trim().is_empty() && q.groups.is_empty() && q.kinds_in.is_empty() && q.kinds_out.is_empty() && q.flags_in.is_empty() && q.flags_out.is_empty() && r0(&q.blocks) && r0(&q.volume) && r0(&q.period) && r0(&q.open) && r0(&q.close);
        let tabs: Vec<(String, bool, usize)> = ["door", "clock", "circuit", "farm"].iter().enumerate().map(|(i, k)| (k.to_string(), q.kind == *k, if plain { a.holds[i] } else { f.kinds.get(*k).copied().unwrap_or(0) })).collect();
        // groups: the selected first, then by count
        let mut groups: Vec<(&String, usize)> = f.groups.iter().map(|(k, v)| (k, *v)).collect();
        for g in &q.groups {
            if !f.groups.contains_key(g) {
                groups.push((g, 0));
            }
        }
        groups.sort_by(|x, y| (!q.groups.contains(x.0), std::cmp::Reverse(x.1), x.0).cmp(&(!q.groups.contains(y.0), std::cmp::Reverse(y.1), y.0)));
        // in words, the top ten unless asked for all (critique: two phone screens of raw ids
        // like "calibrated_sculk_sensor + note_block")
        let label = |g: &str| -> String {
            match q.kind.as_str() {
                "clock" => site_model::html::human_blocks(g),
                "door" => {
                    let w = site_model::html::door_category(g);
                    if w.is_empty() { g.to_string() } else { w.to_string() }
                }
                _ => g.replace('+', " + "),
            }
        };
        let limit = if a.all_groups { usize::MAX } else { 10 };
        // (every category one card: the facet says nothing, it goes; critique, "show all 31"
        // categories of one card each)
        let useful = groups.iter().any(|(_, n)| *n > 1) || !q.groups.is_empty();
        let mut groups_html: String = if useful { groups.iter().enumerate().filter(|(k, (g, _))| *k < limit || q.groups.contains(*g)).map(|(_, (g, n))| chip("group", g, &label(g), *n, if q.groups.contains(*g) { "in" } else { "" })).collect() } else { String::new() };
        if useful && groups.len() > 10 {
            groups_html.push_str(&format!("<button type=\"button\" class=\"chip more\" data-act=\"groups-all\" data-val=\"\">{}</button>", if a.all_groups { "show fewer".to_string() } else { format!("show all {}", groups.len()) }));
        }
        let kf = a.kind_filter.trim().to_lowercase();
        let mut blocks: Vec<(&String, usize)> = f.block_kinds.iter().map(|(k, v)| (k, *v)).collect();
        for k in q.kinds_in.iter().chain(&q.kinds_out) {
            if !f.block_kinds.contains_key(k) {
                blocks.push((k, 0));
            }
        }
        let state = |k: &String| if q.kinds_in.contains(k) { "in" } else if q.kinds_out.contains(k) { "out" } else { "" };
        blocks.retain(|(k, _)| !state(k).is_empty() || kf.is_empty() || k.replace('_', " ").contains(&kf) || k.contains(&kf));
        blocks.sort_by(|x, y| (state(x.0).is_empty(), std::cmp::Reverse(x.1), x.0).cmp(&(state(y.0).is_empty(), std::cmp::Reverse(y.1), y.0)));
        let blocks_html: String = blocks.iter().map(|(k, n)| chip("block", k, &k.replace('_', " "), *n, state(k))).collect();
        let flags_html: String = FLAGS
            .iter()
            .filter(|(k, _, kind)| q.kind.is_empty() || kind.split(',').any(|x| x == q.kind) || q.flags_in.contains(*k) || q.flags_out.contains(*k))
            .filter_map(|(k, _, _)| {
                let n = f.flags.get(*k).copied().unwrap_or(0);
                let st = if q.flags_in.contains(*k) { "in" } else if q.flags_out.contains(*k) { "out" } else { "" };
                (n > 0 || !st.is_empty()).then(|| chip("flag", k, flag_label(k), n, st))
            })
            .collect();
        let groups_title = match q.kind.as_str() {
            _ if !useful => "",
            "door" => "Category",
            "clock" => "Made of (the blocks that change state)",
            "circuit" => "What it does",
            _ => "Category or family",
        };
        let time_msg: Option<(String, bool)> = if !a.error.is_empty() {
            Some((a.error.clone(), true))
        } else if !q.value.trim().is_empty() {
            Some(match site_model::parse_value_query(&q.value) {
                Ok((field, lo, hi)) => {
                    let what = match field {
                        site_model::ValueField::Latency => "a latency",
                        site_model::ValueField::L => "L",
                        site_model::ValueField::N => "N",
                        site_model::ValueField::T => "T",
                        site_model::ValueField::P => "P",
                        site_model::ValueField::Axis => "L / N / T / P",
                    };
                    let range = if lo == hi { format!("= {lo}") } else if hi == u32::MAX { format!("from {lo}") } else { format!("from {lo} to {hi}") };
                    (format!("with {what} {range}"), false)
                }
                Err(e) => (e, true),
            })
        } else if q.time.trim().is_empty() {
            None
        } else {
            Some(match site_model::parse_time_query(&q.time) {
                Ok((lo, hi)) if lo == hi => (format!("with a period of {}", gt_time(lo)), false),
                Ok((lo, hi)) if hi == u32::MAX => (format!("with a period from {}", gt_time(lo)), false),
                Ok((lo, hi)) => (format!("with a period from {} to {}", gt_time(lo), gt_time(hi)), false),
                Err(e) => (e, true),
            })
        };
        let members = a.hits.iter().filter(|h| matches!(h, Hit::Member(_))).count();
        let count = if a.error.is_empty() {
            let mut s = format!("<strong>{}</strong> result{}", a.hits.len(), if a.hits.len() == 1 { "" } else { "s" });
            if let Some((m, false)) = &time_msg {
                s.push_str(&format!(" {}", esc(m)));
            }
            if members > 0 {
                s.push_str(&format!(" ({members} of them generated family member{})", if members == 1 { "" } else { "s" }));
            }
            // the chips most cards share, once (critique: six cards in a row repeated "no slime ·
            // 1 block thick · works facing any way")
            if !a.shared.is_empty() {
                let parts: Vec<String> = a.shared.iter().map(|c| chip_plural(c)).collect();
                let list = match parts.len() {
                    1 => parts[0].clone(),
                    n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
                };
                s.push_str(&format!(". Most {}", esc(&list)));
            }
            // what a tick is, once (critique: players count repeater ticks, 1 = 2 game ticks;
            // short: the Clocks tab's first phone screen, webrender's profile)
            if q.kind == "clock" {
                s.push_str(". 20 game ticks = 1 s (a repeater's tick is 2)");
            }
            // nothing found for a text: the nearest kinds instead of a bare 0 (critique C12)
            if a.hits.is_empty() && !q.text.trim().is_empty() {
                s.push_str(&format!(". Try: {}", nearest_searches(&q.text)));
            }
            // nothing found for a time: the nearest period in the count line too (critique: "1
            // minute" gave "0 results" and nothing else on its line)
            if a.hits.is_empty() && !q.time.trim().is_empty() {
                if let Ok((lo, hi)) = site_model::parse_time_query(&q.time) {
                    if let Some((p, _)) = a.site.nearest_periods(lo, hi, 1).first() {
                        s.push_str(&format!(". Nearest: {}", gt_time(*p)));
                    }
                }
            }
            // a gate function with no design on the site (being re-tested) says so (critique:
            // "xor" listed 13 other gates)
            if let Some(f) = gate_word(&q.text) {
                let has = a.site.builds.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.kind == format!("gate {f}")));
                if !has {
                    s.push_str(&format!(". No {} gate on the site right now (its designs are being re-tested in the game); other logic gates", f.to_uppercase()));
                }
            }
            s
        } else {
            format!("<span class=\"error\">The search did not parse: {}</span>", esc(&a.error))
        };
        let placeholder = match q.kind.as_str() {
            "clock" => "Search: 48 ticks, 2.5 s…",
            "door" => "Search doors",
            "circuit" => "Search: toggle, timer, 12…",
            "farm" => "Search: iron, sugar cane, melon…",
            _ => "Search every build",
        };
        (tabs, groups_html, groups_title, blocks_html, flags_html, placeholder, count)
    }) else {
        return;
    };
    let (tabs, groups_html, groups_title, blocks_html, flags_html, placeholder, count) = parts;
    let set = |id: &str, html: &str| {
        if let Some(e) = by_id(id) {
            e.set_inner_html(html);
        }
    };
    for (k, on, n) in &tabs {
        if let Some(e) = by_id(&format!("tab-{k}")) {
            let label = match k.as_str() {
                "door" => "Doors",
                "clock" => "Clocks",
                "farm" => "Farms",
                _ => "Circuits",
            };
            e.set_inner_html(&format!("{label} <span class=\"count\">{n}</span>"));
            let _ = if *on { e.class_list().add_1("on") } else { e.class_list().remove_1("on") };
        }
    }
    if let Some(i) = input("hq") {
        i.set_placeholder(placeholder);
    }
    set("f-groups", &groups_html);
    set("f-blocks", &blocks_html);
    set("f-flags", &if flags_html.is_empty() { "<span class=\"hint\">none for this kind</span>".to_string() } else { flags_html });
    set("f-count", &count);
    if let Some(e) = by_id("f-groups-title") {
        e.set_text_content(Some(groups_title));
    }
    // the Clocks tab's jump bar, and each tab's own opening line (critique: the Clocks tab's
    // intro spoke of doors and circuits)
    if let Some((jump, lede)) = with_app(|a| (if a.q.kind == "clock" { jump_bar(&a.jump_counts, current_period(&a.q)) } else { String::new() }, site_model::html::lede(&a.site, &a.q.kind))) {
        if let Some(e) = by_id("f-periods").and_then(|e| e.dyn_into::<HtmlElement>().ok()) {
            e.set_inner_html(&jump);
            e.set_hidden(jump.is_empty());
        }
        if let Some(e) = by_id("lede") {
            e.set_text_content(Some(&lede));
        }
    }
}
