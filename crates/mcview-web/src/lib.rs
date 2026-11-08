//! The in-browser viewer (docs/webrender.md). On start it renders every element with a
//! `data-mcview` attribute (or class `mcview`), and the ones added later (MutationObserver):
//!
//! * `data-mcview="thumb"` (default): a still picture, drawn lazily when the element becomes
//!   visible (IntersectionObserver) through ONE shared WebGL2 context and copied into the
//!   element's own canvas; cached by (source, size) so a re-added card paints at once.
//! * `data-mcview="full"` (or `live`): an interactive viewer with its own WebGL2 context: orbit /
//!   zoom / pan, play / pause / step the redsim simulation, click a lever to flip it.
//!
//! Attributes: `data-src` (URL of a design .txt, relative to the page) or `data-design` (the text),
//! optional `data-tick` (still-frame tick), `data-yaw` / `data-pitch` (degrees). The element gets
//! class `mcview-ready` once its canvas shows, `mcview-error` (and a `title`) if it cannot.

#![cfg(target_arch = "wasm32")]

mod gpu;
mod live;
mod overlay;
mod record;

use mcview::atlas::Atlas;
use mcview::render::Frame;
use mcview::{Options, Session, scene};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement, HtmlElement, ImageBitmap, IntersectionObserver, IntersectionObserverEntry, MutationObserver, MutationRecord, ResizeObserver, ResizeObserverEntry};

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Run `f` on the app (None when it is not up yet or already borrowed).
pub(crate) fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

pub(crate) fn log(s: &str) {
    web_sys::console::log_1(&JsValue::from_str(&format!("mcview: {s}")));
}

/// A visible note on the page when the game's textures fall back or the GPU errors: the
/// console log alone is invisible on a phone, and a silent fallback hid two incidents (the
/// dead mcmeta URLs, then this one). Fixed to the viewport's bottom-left, pointer-transparent;
/// a later call replaces its text, [`texture_note_clear`] removes it.
pub(crate) fn texture_note(msg: &str) {
    let Some(body) = document().body() else { return };
    let el = match body.query_selector("#mcview-texture-note").ok().flatten() {
        Some(e) => e,
        None => {
            let e = match document().create_element("div") {
                Ok(e) => e,
                Err(_) => return,
            };
            let _ = e.set_attribute("id", "mcview-texture-note");
            let _ = e.set_attribute("style", "position:fixed;left:8px;bottom:8px;z-index:9999;max-width:92vw;padding:6px 10px;border-radius:8px;background:#b91c1c;color:#fff;font:12px/1.4 system-ui,sans-serif;pointer-events:none;white-space:pre-wrap");
            if body.append_child(&e).is_err() {
                return;
            }
            e
        }
    };
    el.set_text_content(Some(&format!("mcview textures: {msg}")));
}

pub(crate) fn texture_note_clear() {
    if let Some(body) = document().body()
        && let Ok(Some(e)) = body.query_selector("#mcview-texture-note")
    {
        let _ = e.remove();
    }
}

/// `?mcview-debug` in the page URL: timings in the console.
pub(crate) fn debug() -> bool {
    thread_local! {
        static DEBUG: bool = web_sys::window().and_then(|w| w.location().search().ok()).is_some_and(|s| s.contains("mcview-debug"));
    }
    DEBUG.with(|d| *d)
}

/// `?mcview-perf` in the page URL: every 5 s of play, the frames' count and times in the console
/// (without `?mcview-debug`'s per-frame lines).
pub(crate) fn perf_log() -> bool {
    thread_local! {
        static PERF: bool = web_sys::window().and_then(|w| w.location().search().ok()).is_some_and(|s| s.contains("mcview-perf"));
    }
    PERF.with(|d| *d)
}

pub(crate) fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

/// Wait for a task of its own (a zero timeout): the page's steps that each take tens of
/// milliseconds (the WebGL context, the shaders, the textures, the block models) run in separate
/// tasks, the page answering in between (the coordinator 2026-10-22: the page's start-up ran
/// 120-190 ms in one task).
pub(crate) async fn next_task() {
    sleep_ms(0).await;
}

/// Wait `ms` milliseconds (a timeout).
pub(crate) async fn sleep_ms(ms: i32) {
    let p = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(p).await;
}

/// A piece of staged work (`mcview::staged`: an atlas decoded row by row) run over tasks, `ms` of
/// it a task, the page answering in between.
pub(crate) async fn run_staged<T>(ms: f64, make: impl FnOnce(mcview::staged::Budget) -> mcview::staged::Task<T>) -> T {
    let b = match window().performance() {
        Some(p) => mcview::staged::Budget::with_clock(Rc::new(move || p.now())),
        None => mcview::staged::Budget::none(),
    };
    let mut task = make(b.clone());
    loop {
        b.give(ms);
        if let Some(v) = task.poll() {
            return v;
        }
        next_task().await;
    }
}

/// A PNG's pixels decoded over tasks ([`run_staged`]).
pub(crate) async fn decode_png_over_tasks(bytes: Vec<u8>) -> Result<(Vec<u8>, u32, u32), String> {
    run_staged(12.0, |b| mcview::staged::Task::new(async move { mcview::atlas::vanilla::decode_png_staged(&bytes, &b).await })).await
}

pub(crate) fn document() -> web_sys::Document {
    window().document().expect("no document")
}

enum ThumbGpu {
    Pending,
    Ready(gpu::Gpu),
    Failed(String),
}

pub(crate) struct View {
    pub el: HtmlElement,
    pub canvas: HtmlCanvasElement,
    pub full: bool,
    pub src: Option<String>,
    pub inline: Option<Rc<str>>,
    pub tick: Option<u32>,
    /// `data-farm-rts`: a crop farm's judge's random tick speed (`circuit::farm::game_rts`, from
    /// its vanilla twin run): the view replays its run at it (a big farm is shown as built
    /// without it).
    pub farm_rts: Option<u32>,
    /// `data-still`: the still frame a thumbnail shows, found at build time by the site through
    /// `mcview::tools::still_attr` (`Session::still_attr`): one run to its tick instead of the
    /// period search. Absent or unreadable: the viewer finds it (`Session::go_to_still`).
    pub still: Option<String>,
    /// `data-asbuilt`: the site knows the browser's simulator cannot run this design (its build
    /// found it unmodeled: w86's bamboo sweepers, bamboo growing out of the modeled box): the view
    /// stays the still and never builds the session that would play (pressing play on the bamboo
    /// sweeper took one 550 ms task to find that out; `mctas web stall`).
    pub asbuilt: bool,
    /// `data-autoplay`: a live viewer starts playing once loaded.
    pub autoplay: bool,
    /// `data-stimulus`: a circuit's judge stimulus (the auto-demo with `data-autoplay`).
    pub stimulus: Option<String>,
    /// `data-events`: the judge's actions at cells (a BUD switch's target and footprint probes),
    /// replayed in the auto-demo.
    pub events: Option<String>,
    /// `data-levels`: a strength input's values (`<frame> <input> <value>; ...`: the judge's
    /// chest fills, the hex circuits), replayed by the auto-demo.
    pub levels: Option<String>,
    /// `data-phases`: the checker's phase labels.
    pub phases: Option<String>,
    /// `data-input-mode`: `pulse` | `toggle` (default from the circuit kind).
    pub input_mode: Option<String>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    /// Canvas size in device pixels.
    pub w: u32,
    pub h: u32,
    pub visible: bool,
    pub done: bool,
    pub live: Option<live::Live>,
}

impl View {
    /// The cache key: source and size.
    fn key(&self) -> String {
        let src = match (&self.src, &self.inline) {
            (Some(s), _) => s.clone(),
            (None, Some(t)) => format!("inline:{:x}", fnv(t)),
            _ => String::new(),
        };
        format!("{src}|{}x{}|{:?}|{:?}|{:?}", self.w, self.h, self.tick, self.yaw, self.pitch)
    }
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

pub(crate) struct App {
    pub atlas: Rc<Atlas>,
    thumb: ThumbGpu,
    pub views: HashMap<u32, View>,
    next_id: u32,
    queue: VecDeque<u32>,
    texts: HashMap<String, Result<Rc<str>, String>>,
    fetching: HashSet<String>,
    cache: HashMap<String, ImageBitmap>,
    io: IntersectionObserver,
    ro: ResizeObserver,
    frame_scheduled: bool,
    work_scheduled: bool,
    /// The game's textures: still loading (thumbnails wait, up to a few seconds), or settled
    /// (applied, or unavailable: the hand-drawn tiles stay).
    textures_pending: bool,
    /// The game's textures finished loading (applied or failed): the stall watchdog's flag.
    textures_done: bool,
    /// The game's entity textures were asked for (the first entity drawn without them).
    entities_requested: bool,
    /// The page's scrolling: the last scroll position, its time (ms) and the speed (CSS px/s,
    /// smoothed). A fling past [`FLING`] defers the thumbnails until it slows (the cards fly by:
    /// drawing them made the scroll stutter).
    scroll_y: f64,
    scroll_ts: f64,
    scroll_v: f64,
    /// A deferred thumbnail pass is scheduled (after a fling).
    defer_scheduled: bool,
    /// A card's picture being made over several tasks ([`ThumbJob`]).
    thumb_job: Option<ThumbJob>,
    /// The thumbnails' canvas until its context is made (`start_thumb_gpu`: once a card comes).
    thumb_canvas: Option<HtmlCanvasElement>,
}

/// A card's picture made over several tasks (the coordinator 2026-10-22: no task over ~50-100
/// ms; melon-lattice8's card took 105 ms in one): its still, its camera, its mesh and its
/// triangles are each a step of [`thumb_picture`] (`mcview::staged`: it pauses between them once
/// the work slice's time is up), then it is drawn.
struct ThumbJob {
    id: u32,
    key: String,
    t0: f64,
    budget: mcview::staged::Budget,
    task: mcview::staged::Task<Result<ThumbPicture, String>>,
    picture: Option<Result<ThumbPicture, String>>,
}

/// A card's triangles, framed, ready to draw (and when each step ended, for `?mcview-debug`).
struct ThumbPicture {
    solid: Vec<mcview::mesh::Vertex>,
    translucent: Vec<mcview::mesh::Vertex>,
    cam: mcview::camera::Camera,
    w: u32,
    h: u32,
    marks: [f64; 3],
}

/// A card's picture up to its triangles, a step at a time ([`ThumbJob`]): a card is a still,
/// never simulated (vjeux 2026-10-21, "Why is it simulating everything?"; the coordinator: cards
/// never simulate): the design as the judge places it, its judge's chests, framed by the page's
/// `data-still` marks (its output cell and picture box, found at build time; its tick not run).
#[allow(clippy::too_many_arguments)]
async fn thumb_picture(atlas: Rc<Atlas>, text: Rc<str>, still: Option<String>, yaw: Option<f32>, pitch: Option<f32>, w: u32, h: u32, b: mcview::staged::Budget) -> Result<ThumbPicture, String> {
    let clock = || window().performance().map(|p| p.now()).unwrap_or(0.0);
    let mut s = Session::new(&text, &Options::still())?;
    if let Some(a) = still.as_deref() {
        let _ = s.take_still_marks(a);
    }
    let m0 = clock();
    b.pause().await;
    let mut look = scene::viewer_look(&s);
    let aspect = w as f32 / h as f32;
    // a thumbnail is a picture, nothing to press: no outlines over a small build (the live viewer
    // has them, with its tags) but a BUD switch's TARGET, and it frames what it draws
    scene::thumbnail_look(&s, &mut look);
    let mut cam = scene::thumb_camera(&atlas, &s, &look, aspect);
    if yaw.is_some() || pitch.is_some() {
        if let Some(y) = yaw {
            cam.yaw = y.to_radians();
        }
        if let Some(p) = pitch {
            cam.pitch = p.to_radians();
        }
        let (lo, hi) = scene::frame_bounds(&s, &look);
        cam.fit(lo, hi, aspect, 0.85);
    }
    let m1 = clock();
    b.pause().await;
    let mesh = scene::build(&atlas, &s, 0.0, &look);
    b.pause().await;
    let (solid, translucent) = scene::triangles(&mesh, &cam);
    Ok(ThumbPicture { solid, translucent, cam, w, h, marks: [m0, m1, clock()] })
}

/// A scroll faster than this (CSS px/s) is a fling: thumbnails wait until it slows or stops.
const FLING: f64 = 1200.0;
/// Scrolling stopped this long ago (ms): the speed no longer counts.
const SCROLL_REST_MS: f64 = 150.0;

/// Where the game's assets come from: `data-mcview-textures` on the page's root element (`none` =
/// the hand-drawn tiles and the viewer's models only; else the directory of `atlas.png`,
/// `data.min.json`, `block_definition.min.json` and `model.min.json`), by default the vendored
/// textures served with the site (`textures/26.3`, same origin as the page).
fn texture_urls() -> Option<[String; 4]> {
    use mcview::atlas::vanilla::*;
    let root = document().document_element()?;
    let base = match root.get_attribute("data-mcview-textures") {
        Some(v) if v.trim() == "none" => return None,
        Some(v) if !v.trim().is_empty() => v.trim().trim_end_matches('/').to_string(),
        _ => SITE_DIR.to_string(),
    };
    Some([format!("{base}/{ATLAS}"), format!("{base}/{INDEX}"), format!("{base}/{DEFS}"), format!("{base}/{MODELS}")])
}

/// The game's entity and item atlases (`entities.png`, `entities.min.json`, `items.png`,
/// `items.min.json` in a `data-mcview-textures` directory; the vendored ones by default), loaded
/// the first time a page draws an entity.
fn entity_urls() -> Option<[String; 4]> {
    use mcview::atlas::vanilla::*;
    let root = document().document_element()?;
    let base = match root.get_attribute("data-mcview-textures") {
        Some(v) if v.trim() == "none" => return None,
        Some(v) if !v.trim().is_empty() => v.trim().trim_end_matches('/').to_string(),
        _ => SITE_DIR.to_string(),
    };
    Some([format!("{base}/{ENTITY_ATLAS}"), format!("{base}/{ENTITY_INDEX}"), format!("{base}/{ITEM_ATLAS}"), format!("{base}/{ITEM_INDEX}")])
}

/// Load the game's entity and item textures into the game texture (boats, minecarts, armor
/// stands and dropped items drawn with the viewer's stand-ins until then), and its particle
/// sprites below them (an explosion's frames: `particle/<name>.png` in a `data-mcview-textures`
/// directory, the vendored ones by default).
async fn load_entity_textures() {
    let Some([ep, ei, ip, ii]) = entity_urls() else { return };
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    let (ep, ei, ip, ii) = futures_join4(fetch_cached(&ep), fetch_cached(&ei), fetch_cached(&ip), fetch_cached(&ii)).await;
    // the two atlases decoded over tasks of their own (the entity atlas took 55 ms in one)
    let now = || window().performance().map(|p| p.now()).unwrap_or(0.0);
    let t0 = now();
    let entities = match ep {
        Ok(b) => decode_png_over_tasks(b).await,
        Err(e) => Err(e),
    };
    let t1 = now();
    let items = match ip {
        Ok(b) => decode_png_over_tasks(b).await,
        Err(e) => Err(e),
    };
    if debug() {
        log(&format!("the game's entity and item atlases decoded over {:.0} + {:.0} ms", t1 - t0, now() - t1));
    }
    let got = (|| {
        let (er, ew, eh) = entities?;
        let (ir, iw, ih) = items?;
        Ok::<_, String>((er, ew, eh, text(ei?)?, ir, iw, ih, text(ii?)?))
    })();
    let base = document().document_element().and_then(|r| r.get_attribute("data-mcview-textures")).map(|v| v.trim().trim_end_matches('/').to_string()).filter(|v| !v.is_empty() && v != "none").map(|b| format!("{b}/{}/", mcview::atlas::vanilla::PARTICLE_DIR));
    let base = base.unwrap_or_else(|| format!("{}/{}/", mcview::atlas::vanilla::SITE_DIR, mcview::atlas::vanilla::PARTICLE_DIR));
    let mut sprites: Vec<(String, Vec<u8>, u32, u32)> = Vec::new();
    let mut sprite_err: Option<String> = None;
    for name in mcview::atlas::vanilla::particle_names() {
        match fetch_cached(&format!("{base}{name}.png")).await.and_then(|b| mcview::atlas::vanilla::decode_png(&b)) {
            Ok((rgba, w, h)) => sprites.push((name, rgba, w, h)),
            Err(e) => {
                sprite_err = Some(format!("{name}: {e}"));
                break;
            }
        }
    }
    // (added and uploaded in a task of its own)
    next_task().await;
    let t3 = now();
    with_app(|a| match got {
        Ok((er, ew, eh, ei, ir, iw, ih, ii)) => {
            let mut atlas = (*a.atlas).clone();
            match atlas.add_game_entities((&er, ew, eh, &ei), (&ir, iw, ih, &ii)) {
                Ok(n) => {
                    if debug() {
                        log(&format!("the game's entity textures ({n}) and items ({:.0} ms)", now() - t3));
                    }
                    match sprite_err {
                        None => {
                            if let Err(e) = atlas.add_game_particles(&sprites) {
                                log(&format!("the game's particle sprites: {e} (the viewer's stand-ins)"));
                            }
                        }
                        Some(e) => log(&format!("the game's particle sprites unavailable: {e} (the viewer's stand-ins)")),
                    }
                    a.set_atlas(atlas);
                }
                Err(e) => log(&format!("the game's entity textures: {e} (the viewer's stand-ins)")),
            }
        }
        Err(e) => log(&format!("the game's entity textures unavailable: {e} (the viewer's stand-ins)")),
    });
    ensure_frame();
}

/// A URL's bytes are what they claim (a PNG starts with its magic, a JSON starts with
/// `{` or `[`, anything else is non-empty): the Cache API cache never expires, so a
/// half-stored entry used to poison every later load.
fn looks_like(url: &str, b: &[u8]) -> bool {
    if b.is_empty() {
        return false;
    }
    if url.ends_with(".png") {
        return b.starts_with(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10]);
    }
    if url.ends_with(".json") {
        return b.iter().find(|c| !c.is_ascii_whitespace()).is_some_and(|c| *c == b'{' || *c == b'[');
    }
    true
}

/// A URL's bytes through the Cache API (kept across visits; the plain fetch when the Cache API
/// is unavailable, e.g. on http pages). A cached entry that fails [`looks_like`] is dropped
/// and the network tried; bytes that fail it from the network are an error, not a texture.
async fn fetch_cached(url: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen_futures::JsFuture;
    let w = window();
    let cache: Option<web_sys::Cache> = match w.caches() {
        Ok(cs) => JsFuture::from(cs.open("mcview-textures-1")).await.ok().map(|c| c.unchecked_into()),
        Err(_) => None,
    };
    let bytes_of = async |resp: web_sys::Response| -> Result<Vec<u8>, String> {
        let buf = JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
        Ok(js_sys::Uint8Array::new(&buf).to_vec())
    };
    if let Some(c) = &cache
        && let Ok(r) = JsFuture::from(c.match_with_str(url)).await
        && !r.is_undefined()
    {
        let resp: web_sys::Response = r.unchecked_into();
        match bytes_of(resp).await {
            Ok(b) if looks_like(url, &b) => return Ok(b),
            _ => {
                // a half-stored entry: drop it so it cannot poison the next load either
                let _ = JsFuture::from(c.delete_with_str(url)).await;
            }
        }
    }
    let r: web_sys::Response = JsFuture::from(w.fetch_with_str(url)).await.map_err(|e| format!("fetch {url}: {e:?}"))?.unchecked_into();
    if !r.ok() {
        return Err(format!("fetch {url}: HTTP {}", r.status()));
    }
    if let Some(c) = &cache
        && let Ok(copy) = r.clone()
    {
        let _ = JsFuture::from(c.put_with_str(url, &copy)).await;
    }
    let b = bytes_of(r).await?;
    if !looks_like(url, &b) {
        return Err(format!("fetch {url}: bytes are not what the file claims ({} bytes)", b.len()));
    }
    Ok(b)
}

/// Fetch the game's textures, apply them, re-upload every context's tiles and redraw.
async fn load_textures() {
    let Some([png_url, index_url, defs_url, models_url]) = texture_urls() else {
        with_app(|a| {
            a.textures_pending = false;
            a.textures_done = true;
        });
        ensure_frame();
        return;
    };
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    // all four at once
    let (png, index, defs, models) = futures_join4(fetch_cached(&png_url), fetch_cached(&index_url), fetch_cached(&defs_url), fetch_cached(&models_url)).await;
    let got = (|| Ok::<_, String>((png?, text(index?)?)))();
    let game_text = (|| Ok::<_, String>((text(defs?)?, text(models?)?)))();
    // three tasks (the coordinator 2026-10-22: no task over ~50-100 ms): the block atlas decoded
    // and its tiles, the game's block models, then the texture uploaded to every context (the
    // game texture is only ever replaced here before `maybe_load_entities`, which waits for
    // `atlas.game`, adds the entities: nothing changes it in between)
    let now = || window().performance().map(|p| p.now()).unwrap_or(0.0);
    let base = with_app(|a| (*a.atlas).clone());
    // whether the game's tiles and the game's models both landed: only then is the
    // fallback note (if a watchdog set one) wrong and cleared
    let (mut tiles_ok, mut models_ok) = (false, false);
    if let Some(mut atlas) = base {
        match got {
            Ok((png, index)) => {
                let t0 = now();
                match decode_png_over_tasks(png).await {
                    Ok((rgba, w, h)) => {
                        next_task().await;
                        match atlas.apply_vanilla(&rgba, w, h, &index) {
                            Ok(n) => {
                                tiles_ok = true;
                                if debug() {
                                    log(&format!("the game's textures on {n} tiles ({:.0} ms)", now() - t0));
                                }
                            }
                            Err(e) => {
                                log(&format!("the game's textures: {e} (hand-drawn tiles)"));
                                texture_note(&format!("showing hand-drawn tiles: the game's textures failed: {e}"));
                            }
                        }
                        next_task().await;
                        // the game's own block models (the viewer's when they cannot be read)
                        let t1 = now();
                        match game_text.and_then(|(defs, models)| atlas.set_game(&rgba, w, h, &index, &defs, &models)) {
                            Ok(n) => {
                                models_ok = true;
                                if debug() {
                                    log(&format!("the game's block models ({n} textures, {:.0} ms)", now() - t1));
                                }
                            }
                            Err(e) => {
                                log(&format!("the game's block models unavailable: {e} (the viewer's models)"));
                                texture_note(&format!("showing the game's tiles on the viewer's models: the game's block models failed: {e}"));
                            }
                        }
                        next_task().await;
                        let t2 = now();
                        with_app(|a| a.set_atlas(atlas));
                        if tiles_ok && models_ok {
                            texture_note_clear();
                        }
                        if debug() {
                            log(&format!("the game's texture uploaded ({:.0} ms)", now() - t2));
                        }
                    }
                    Err(e) => {
                        log(&format!("the game's textures: {e} (hand-drawn tiles)"));
                        texture_note(&format!("showing hand-drawn tiles: the game's atlas failed to decode: {e}"));
                    }
                }
            }
            Err(e) => {
                log(&format!("the game's textures unavailable: {e} (hand-drawn tiles)"));
                texture_note(&format!("showing hand-drawn tiles: the game's textures failed to load: {e}"));
            }
        }
    }
    with_app(|a| {
        a.textures_pending = false;
        a.textures_done = true;
    });
    ensure_frame();
}

/// Four futures driven together (no executor crate: a small poll loop over pinned futures).
async fn futures_join4<A: std::future::Future, B: std::future::Future, C: std::future::Future, D: std::future::Future>(a: A, b: B, c: C, d: D) -> (A::Output, B::Output, C::Output, D::Output) {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::Poll;
    let (mut a, mut b, mut c, mut d) = (Box::pin(a), Box::pin(b), Box::pin(c), Box::pin(d));
    let (mut ra, mut rb, mut rc, mut rd) = (None, None, None, None);
    std::future::poll_fn(move |cx| {
        if ra.is_none() && let Poll::Ready(v) = Pin::new(&mut a).poll(cx) {
            ra = Some(v);
        }
        if rb.is_none() && let Poll::Ready(v) = Pin::new(&mut b).poll(cx) {
            rb = Some(v);
        }
        if rc.is_none() && let Poll::Ready(v) = Pin::new(&mut c).poll(cx) {
            rc = Some(v);
        }
        if rd.is_none() && let Poll::Ready(v) = Pin::new(&mut d).poll(cx) {
            rd = Some(v);
        }
        if ra.is_some() && rb.is_some() && rc.is_some() && rd.is_some() {
            Poll::Ready((ra.take().unwrap(), rb.take().unwrap(), rc.take().unwrap(), rd.take().unwrap()))
        } else {
            Poll::Pending
        }
    })
    .await
}

/// The thumbnails' shared WebGL context, made the first time the page has a card to draw, once
/// the game's textures settled (`App::thumbs`; a design page with only its live view never makes
/// it: its context, adapter and renderer were 100-180 ms of the page's start in a software GL;
/// the coordinator 2026-10-22).
fn start_thumb_gpu(canvas: HtmlCanvasElement, atlas: Rc<Atlas>) {
    wasm_bindgen_futures::spawn_local(async move {
        // (a task of its own, after the page's start)
        next_task().await;
        let t = window().performance().map(|p| p.now()).unwrap_or(0.0);
        let g = gpu::Gpu::new(canvas, &atlas).await;
        if debug() {
            log(&format!("the thumbnails' WebGL context done in {:.1} ms", window().performance().map(|p| p.now()).unwrap_or(0.0) - t));
        }
        with_app(|a| {
            a.thumb = match g {
                Ok(g) => ThumbGpu::Ready(g),
                Err(e) => {
                    log(&format!("thumbnails disabled: {e}"));
                    ThumbGpu::Failed(e)
                }
            };
        });
        ensure_frame();
    });
}

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!("mcview panic: {info}")));
    }));
    let io_cb = Closure::<dyn FnMut(js_sys::Array, IntersectionObserver)>::new(|entries: js_sys::Array, _| {
        with_app(|a| a.on_intersect(entries));
        ensure_frame();
    });
    let io_init = web_sys::IntersectionObserverInit::new();
    io_init.set_root_margin("300px");
    let io = IntersectionObserver::new_with_options(io_cb.as_ref().unchecked_ref(), &io_init).expect("IntersectionObserver");
    io_cb.forget();
    let ro_cb = Closure::<dyn FnMut(js_sys::Array, ResizeObserver)>::new(|entries: js_sys::Array, _| {
        with_app(|a| a.on_resize(entries));
        ensure_frame();
    });
    let ro = ResizeObserver::new(ro_cb.as_ref().unchecked_ref()).expect("ResizeObserver");
    ro_cb.forget();
    let clock = || window().performance().map(|p| p.now()).unwrap_or(0.0);
    let t0 = clock();
    let atlas = Rc::new(Atlas::build());
    if debug() {
        log(&format!("start: the viewer's own atlas done in {:.1} ms", clock() - t0));
    }
    let app = App {
        atlas: atlas.clone(),
        thumb: ThumbGpu::Pending,
        views: HashMap::new(),
        next_id: 1,
        queue: VecDeque::new(),
        texts: HashMap::new(),
        fetching: HashSet::new(),
        cache: HashMap::new(),
        io,
        ro,
        frame_scheduled: false,
        work_scheduled: false,
        textures_pending: true,
        textures_done: false,
        entities_requested: false,
        scroll_y: 0.0,
        scroll_ts: f64::MIN,
        scroll_v: 0.0,
        defer_scheduled: false,
        thumb_job: None,
        thumb_canvas: None,
    };
    APP.with(|a| *a.borrow_mut() = Some(app));
    // the page's scroll speed (any scrolling element: captured on the window), for the fling gate
    {
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            let y = e.target().and_then(|t| t.dyn_into::<Element>().ok()).map(|el| el.scroll_top() as f64).unwrap_or_else(|| window().scroll_y().unwrap_or(0.0));
            let now = window().performance().map(|p| p.now()).unwrap_or(0.0);
            with_app(|a| {
                let dt = now - a.scroll_ts;
                if dt > 0.0 && dt < 500.0 {
                    let v = (y - a.scroll_y).abs() / dt * 1000.0;
                    a.scroll_v = 0.5 * a.scroll_v + 0.5 * v;
                } else {
                    a.scroll_v = 0.0;
                }
                a.scroll_y = y;
                a.scroll_ts = now;
            });
        });
        let opts = web_sys::AddEventListenerOptions::new();
        opts.set_passive(true);
        opts.set_capture(true);
        let _ = window().add_event_listener_with_callback_and_add_event_listener_options("scroll", cb.as_ref().unchecked_ref(), &opts);
        cb.forget();
    }
    // the game's textures (hand-drawn tiles if they do not come within a few seconds)
    wasm_bindgen_futures::spawn_local(load_textures());
    // a load that never finishes (a fetch that hangs): say so on the page, instead of
    // leaving the hand-drawn tiles with no explanation
    wasm_bindgen_futures::spawn_local(async {
        sleep_ms(15_000).await;
        if with_app(|a| !a.textures_done).unwrap_or(false) {
            texture_note("still loading after 15 s (the fetch or decode stalled)");
        }
    });
    {
        let cb = Closure::once_into_js(move || {
            let late = with_app(|a| std::mem::replace(&mut a.textures_pending, false)).unwrap_or(false);
            if late {
                log("the game's textures are slow: drawing with the hand-drawn tiles meanwhile");
                ensure_frame();
            }
        });
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 4000);
    }
    // the shared thumbnail context, on a canvas that is never shown
    let canvas: HtmlCanvasElement = document().create_element("canvas").unwrap().dyn_into().unwrap();
    canvas.set_width(240);
    canvas.set_height(180);
    // the shared thumbnail context lost (a GPU reset, too many contexts on iOS): thumbnails wait
    // and the context is made again once the canvas is restored (drawn pictures stay)
    {
        let lost = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            e.prevent_default();
            with_app(|a| a.thumb = ThumbGpu::Pending);
            log("the thumbnails' WebGL context was lost; waiting for it to come back");
        });
        let _ = canvas.add_event_listener_with_callback("webglcontextlost", lost.as_ref().unchecked_ref());
        lost.forget();
        let c2 = canvas.clone();
        let restored = Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| {
            let c3 = c2.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let Some(atlas) = with_app(|a| a.atlas.clone()) else { return };
                if let Ok(g) = gpu::Gpu::new(c3, &atlas).await {
                    with_app(|a| a.thumb = ThumbGpu::Ready(g));
                    ensure_frame();
                }
            });
        });
        let _ = canvas.add_event_listener_with_callback("webglcontextrestored", restored.as_ref().unchecked_ref());
        restored.forget();
    }
    // (its context made once the page has a card: `start_thumb_gpu`)
    with_app(|a| a.thumb_canvas = Some(canvas));
    // the page as it is, then whatever is added later
    let found = scan(&document().document_element().unwrap());
    with_app(|a| a.register_all(found));
    let mo_cb = Closure::<dyn FnMut(js_sys::Array, MutationObserver)>::new(|records: js_sys::Array, _| {
        let mut removed = false;
        let mut found: Vec<HtmlElement> = Vec::new();
        for r in records.iter() {
            let r: MutationRecord = r.unchecked_into();
            if r.type_() == "attributes" {
                if let Some(t) = r.target()
                    && let Ok(e) = t.dyn_into::<HtmlElement>()
                {
                    with_app(|a| a.src_changed(&e));
                }
                continue;
            }
            let added = r.added_nodes();
            for i in 0..added.length() {
                if let Some(n) = added.item(i)
                    && let Ok(e) = n.dyn_into::<Element>()
                {
                    found.extend(scan(&e));
                }
            }
            removed |= r.removed_nodes().length() > 0;
        }
        if !found.is_empty() {
            with_app(|a| a.register_all(found));
        }
        if removed {
            with_app(|a| a.sweep());
        }
        ensure_frame();
    });
    let mo = MutationObserver::new(mo_cb.as_ref().unchecked_ref()).expect("MutationObserver");
    mo_cb.forget();
    let init = web_sys::MutationObserverInit::new();
    init.set_child_list(true);
    init.set_subtree(true);
    // a viewer's design swapped in place (the site's variant picker): `data-src` set last
    init.set_attributes(true);
    init.set_attribute_filter(&js_sys::Array::of1(&JsValue::from_str("data-src")));
    let _ = mo.observe_with_options(&document().document_element().unwrap(), &init);
    ensure_frame();
    if debug() {
        log(&format!("start: done in {:.1} ms", clock() - t0));
    }
}

/// For `mctas web check --wasm`: memory the host fills with a design's text.
#[unsafe(no_mangle)]
pub extern "C" fn mcview_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// For `mctas web check --wasm`: the clock period detector of THIS wasm (the code the browser
/// runs) on a design text written at `ptr` (taken back and freed). Returns state warmup, state
/// period, visible warmup and visible period as four u16 (low to high), or u64::MAX.
///
/// # Safety
/// `ptr` must come from `mcview_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcview_clock_period(ptr: *mut u8, len: usize, horizon: u32) -> u64 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&bytes);
    match Session::new(&text, &Options::default()).and_then(|s| s.period(horizon)) {
        Ok(p) if p.state_warmup < 65536 && p.state_period < 65536 => p.state_warmup as u64 | (p.state_period as u64) << 16 | (p.warmup as u64) << 32 | (p.period as u64) << 48,
        _ => u64::MAX,
    }
}

/// For `mctas web check --wasm`: a flying machine's end tick in THIS wasm's session (the judge's
/// corridor end), or u32::MAX. Takes back and frees the text like [`mcview_clock_period`].
///
/// # Safety
/// `ptr` must come from `mcview_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcview_fly_end(ptr: *mut u8, len: usize) -> u32 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&bytes);
    match Session::new(&text, &Options::default()) {
        Ok(s) if s.simulated() => s.end_tick.unwrap_or(u32::MAX),
        _ => u32::MAX,
    }
}

/// For `mctas web check --wasm`: the digest of this wasm's frames 0..=`ticks`
/// ([`mcview::session::Session::frames_digest`]), or 0.
///
/// # Safety
/// `ptr` must come from `mcview_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcview_frames_digest(ptr: *mut u8, len: usize, ticks: u32) -> u64 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&bytes);
    match Session::new(&text, &Options::default()) {
        Ok(s) if s.simulated() => s.frames_digest(ticks),
        _ => 0,
    }
}

/// For `mctas web check --wasm`: a mob farm's run in THIS wasm (the browser's session: its
/// placement, the water's settle, `ticks` judged ticks), as one number
/// ([`mcview::mobfarm::chain`] of every tick's world digest and the golems summoned), or 0.
///
/// # Safety
/// `ptr` must come from `mcview_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcview_mob_digest(ptr: *mut u8, len: usize, ticks: u32) -> u64 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&bytes);
    match mcview::mobfarm::view_digests(&text, ticks) {
        Ok((v, golems)) => mcview::mobfarm::chain(&v, golems),
        Err(_) => 0,
    }
}

/// The viewer elements in (and including) `root` not registered yet.
fn scan(root: &Element) -> Vec<HtmlElement> {
    let mut found: Vec<HtmlElement> = Vec::new();
    if root.matches("[data-mcview], .mcview").unwrap_or(false)
        && let Ok(h) = root.clone().dyn_into::<HtmlElement>()
    {
        found.push(h);
    }
    if let Ok(list) = root.query_selector_all("[data-mcview], .mcview") {
        for i in 0..list.length() {
            if let Some(n) = list.item(i)
                && let Ok(h) = n.dyn_into::<HtmlElement>()
            {
                found.push(h);
            }
        }
    }
    found.retain(|el| el.get_attribute("data-mcview-id").is_none());
    found
}

/// An entity was drawn without the game's entity textures: load them (once).
fn maybe_load_entities() {
    let want = with_app(|a| {
        let want = !a.entities_requested && a.atlas.game.as_ref().is_some_and(|g| !g.entities && g.wants_entities.get());
        if want {
            a.entities_requested = true;
        }
        want
    })
    .unwrap_or(false);
    if want {
        wasm_bindgen_futures::spawn_local(load_entity_textures());
    }
}

/// Make sure the pending work runs: thumbnails in a task (no vsync needed, so they also draw in
/// background tabs and headless browsers), live viewers in animation frames.
pub(crate) fn ensure_frame() {
    maybe_load_entities();
    let work = with_app(|a| !std::mem::replace(&mut a.work_scheduled, true)).unwrap_or(false);
    if work {
        let cb = Closure::once_into_js(move || {
            let more = with_app(|a| {
                a.work_scheduled = false;
                a.thumbs()
            })
            .unwrap_or(false);
            maybe_load_entities();
            if more {
                ensure_frame();
            }
        });
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 0);
    }
    let go = with_app(|a| a.views.values().any(|v| v.full) && !std::mem::replace(&mut a.frame_scheduled, true)).unwrap_or(false);
    if go {
        let cb = Closure::once_into_js(move |ts: f64| {
            let more = with_app(|a| {
                a.frame_scheduled = false;
                a.live_frames(ts)
            })
            .unwrap_or(false);
            maybe_load_entities();
            if more {
                ensure_frame();
            }
        });
        let _ = window().request_animation_frame(cb.unchecked_ref());
    }
}

/// Fetch a URL's text.
async fn fetch_text(url: &str) -> Result<String, String> {
    let p = window().fetch_with_str(url);
    let resp: web_sys::Response = wasm_bindgen_futures::JsFuture::from(p).await.map_err(|e| format!("fetch {url}: {e:?}"))?.unchecked_into();
    if !resp.ok() {
        return Err(format!("fetch {url}: HTTP {}", resp.status()));
    }
    let t = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
    t.as_string().ok_or_else(|| "not text".into())
}

fn attr_f32(el: &Element, k: &str) -> Option<f32> {
    el.get_attribute(k).and_then(|v| v.trim().parse().ok())
}

impl App {
    /// A viewer's `data-src` changed (a design swapped in place): every `data-*` attribute is read
    /// again; a live viewer loads the new design keeping its camera when `data-keep-camera` is
    /// set (and its play state and speed), its buttons, tags and overlays rebuilt; a thumbnail is
    /// drawn again.
    fn src_changed(&mut self, el: &HtmlElement) {
        let Some(id) = el.get_attribute("data-mcview-id").and_then(|s| s.parse::<u32>().ok()) else { return };
        let Some(v) = self.views.get_mut(&id) else { return };
        let src = el.get_attribute("data-src").filter(|s| !s.is_empty());
        if src.is_none() || (src == v.src && v.live.as_ref().is_some_and(|l| l.has_player())) {
            return;
        }
        v.src = src;
        v.inline = el.get_attribute("data-design").filter(|s| !s.is_empty()).map(Rc::from);
        v.tick = el.get_attribute("data-tick").and_then(|t| t.trim().parse().ok());
        v.farm_rts = el.get_attribute("data-farm-rts").and_then(|t| t.trim().parse().ok());
        v.still = el.get_attribute("data-still").filter(|s| !s.trim().is_empty());
        v.asbuilt = el.has_attribute("data-asbuilt");
        v.autoplay = el.has_attribute("data-autoplay");
        v.stimulus = el.get_attribute("data-stimulus").filter(|s| !s.trim().is_empty());
        v.events = el.get_attribute("data-events").filter(|s| !s.trim().is_empty());
        v.levels = el.get_attribute("data-levels").filter(|s| !s.trim().is_empty());
        v.phases = el.get_attribute("data-phases").filter(|s| !s.trim().is_empty());
        v.input_mode = el.get_attribute("data-input-mode").filter(|s| !s.trim().is_empty());
        v.yaw = attr_f32(el, "data-yaw");
        v.pitch = attr_f32(el, "data-pitch");
        let keep = el.has_attribute("data-keep-camera");
        if let Some(l) = v.live.as_mut() {
            l.begin_reload(keep);
        }
        let _ = el.class_list().remove_1("mcview-error");
        v.done = false;
        if v.full || v.visible {
            self.queue.push_back(id);
        }
    }

    /// New tiles (the game's textures arrived): every context uploads them, every picture is
    /// drawn again.
    fn set_atlas(&mut self, atlas: Atlas) {
        self.atlas = Rc::new(atlas);
        if let ThumbGpu::Ready(g) = &mut self.thumb {
            g.r.upload_atlas(&self.atlas);
        }
        self.cache.clear();
        let atlas = self.atlas.clone();
        let mut again = Vec::new();
        for (id, v) in self.views.iter_mut() {
            if let Some(l) = v.live.as_mut() {
                l.atlas_changed(&atlas);
            } else if v.done {
                v.done = false;
                if v.visible {
                    again.push(*id);
                }
            }
        }
        for id in again {
            if !self.queue.contains(&id) {
                self.queue.push_back(id);
            }
        }
    }

    /// Register viewer elements: every one's layout read first, then each one set up (a page of
    /// 60 cards added at once: one layout, not one per card; a read after each card's new canvas
    /// forced a layout of the whole growing page every time).
    fn register_all(&mut self, els: Vec<HtmlElement>) {
        let measured: Vec<(HtmlElement, bool, (u32, u32))> = els
            .into_iter()
            .filter(|el| el.get_attribute("data-mcview-id").is_none())
            .map(|el| {
                let stat = window().get_computed_style(&el).ok().flatten().and_then(|s| s.get_property_value("position").ok()).is_some_and(|p| p == "static" || p.is_empty());
                let size = pixel_size(&el);
                (el, stat, size)
            })
            .collect();
        // (an element met twice in one batch, a node and its parent both added: set up once)
        for (el, stat, size) in measured {
            if el.get_attribute("data-mcview-id").is_none() {
                self.register(el, stat, size);
            }
        }
    }

    /// Set up one viewer element (`stat`: its CSS position is static; `size`: its pixel size).
    fn register(&mut self, el: HtmlElement, stat: bool, size: (u32, u32)) {
        let id = self.next_id;
        self.next_id += 1;
        let _ = el.set_attribute("data-mcview-id", &id.to_string());
        let mode = el.get_attribute("data-mcview").filter(|m| !m.is_empty()).or_else(|| el.get_attribute("data-mode")).unwrap_or_else(|| "thumb".into());
        let full = matches!(mode.as_str(), "full" | "live");
        // keep the element's own children (a fallback card): the canvas covers them once ready
        if stat {
            let _ = el.style().set_property("position", "relative");
        }
        let canvas: HtmlCanvasElement = document().create_element("canvas").unwrap().dyn_into().unwrap();
        let st = canvas.style();
        let _ = st.set_property("position", "absolute");
        let _ = st.set_property("left", "0");
        let _ = st.set_property("top", "0");
        let _ = st.set_property("width", "100%");
        let _ = st.set_property("height", "100%");
        let _ = st.set_property("display", "block");
        let _ = st.set_property("visibility", "hidden");
        let _ = el.append_child(&canvas);
        let (w, h) = size;
        canvas.set_width(w.max(1));
        canvas.set_height(h.max(1));
        let view = View {
            src: el.get_attribute("data-src").filter(|s| !s.is_empty()),
            inline: el.get_attribute("data-design").filter(|s| !s.is_empty()).map(Rc::from),
            tick: el.get_attribute("data-tick").and_then(|t| t.trim().parse().ok()),
            farm_rts: el.get_attribute("data-farm-rts").and_then(|t| t.trim().parse().ok()),
            still: el.get_attribute("data-still").filter(|s| !s.trim().is_empty()),
            asbuilt: el.has_attribute("data-asbuilt"),
            autoplay: el.has_attribute("data-autoplay"),
            stimulus: el.get_attribute("data-stimulus").filter(|s| !s.trim().is_empty()),
            events: el.get_attribute("data-events").filter(|s| !s.trim().is_empty()),
            levels: el.get_attribute("data-levels").filter(|s| !s.trim().is_empty()),
            phases: el.get_attribute("data-phases").filter(|s| !s.trim().is_empty()),
            input_mode: el.get_attribute("data-input-mode").filter(|s| !s.trim().is_empty()),
            yaw: attr_f32(&el, "data-yaw"),
            pitch: attr_f32(&el, "data-pitch"),
            el: el.clone(),
            canvas,
            full,
            w,
            h,
            visible: false,
            done: false,
            live: None,
        };
        if view.src.is_none() && view.inline.is_none() {
            self.fail(&el, "no data-src or data-design");
            return;
        }
        self.ro.observe(&el);
        // every view's visibility is followed: a thumbnail is drawn on screen, a live viewer
        // only draws and plays there
        self.io.observe(&el);
        if full {
            self.views.insert(id, view);
            live::start(self, id);
        } else {
            let click_src = view.src.clone().unwrap_or_default();
            let target = el.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| {
                let init = web_sys::CustomEventInit::new();
                init.set_bubbles(true);
                init.set_detail(&JsValue::from_str(&click_src));
                if let Ok(ev) = web_sys::CustomEvent::new_with_event_init_dict("mcview-open", &init) {
                    let _ = target.dispatch_event(&ev);
                }
            });
            let _ = el.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
            cb.forget();
            self.views.insert(id, view);
        }
    }

    pub(crate) fn fail(&mut self, el: &HtmlElement, why: &str) {
        let _ = el.class_list().add_1("mcview-error");
        let _ = el.set_attribute("title", why);
        log(why);
    }

    fn id_of(el: &Element) -> Option<u32> {
        el.get_attribute("data-mcview-id").and_then(|s| s.parse().ok())
    }

    fn on_intersect(&mut self, entries: js_sys::Array) {
        for e in entries.iter() {
            let e: IntersectionObserverEntry = e.unchecked_into();
            let Some(id) = App::id_of(&e.target()) else { continue };
            let Some(v) = self.views.get_mut(&id) else { continue };
            v.visible = e.is_intersecting();
            // a live viewer only draws and plays on screen (`live::frame`): back on screen, it
            // goes on where it was
            if v.full {
                if let Some(l) = v.live.as_mut() {
                    l.redraw = true;
                }
                continue;
            }
            if v.visible && !v.done && !self.queue.contains(&id) {
                self.queue.push_back(id);
            }
        }
    }

    fn on_resize(&mut self, entries: js_sys::Array) {
        for e in entries.iter() {
            let e: ResizeObserverEntry = e.unchecked_into();
            let Some(id) = App::id_of(&e.target()) else { continue };
            let Some(v) = self.views.get_mut(&id) else { continue };
            let (w, h) = pixel_size(&v.el);
            if (w, h) == (v.w, v.h) || w == 0 || h == 0 {
                continue;
            }
            v.w = w;
            v.h = h;
            if v.full {
                if let Some(l) = v.live.as_mut() {
                    l.resized();
                }
            } else {
                v.canvas.set_width(w);
                v.canvas.set_height(h);
                v.done = false;
                if v.visible && !self.queue.contains(&id) {
                    self.queue.push_back(id);
                }
            }
        }
    }

    /// Forget the views whose element left the page.
    fn sweep(&mut self) {
        let gone: Vec<u32> = self.views.iter().filter(|(_, v)| !v.el.is_connected()).map(|(id, _)| *id).collect();
        for id in gone {
            if let Some(v) = self.views.remove(&id) {
                self.io.unobserve(&v.el);
                self.ro.unobserve(&v.el);
                let _ = v.el.remove_attribute("data-mcview-id");
                v.canvas.remove();
            }
            self.queue.retain(|q| *q != id);
        }
    }

    /// The design text of a view, starting its fetch if needed (None while it loads).
    pub(crate) fn text_of(&mut self, id: u32) -> Option<Result<Rc<str>, String>> {
        let v = self.views.get(&id)?;
        if let Some(t) = &v.inline {
            return Some(Ok(t.clone()));
        }
        let src = v.src.clone()?;
        self.url_text(&src)
    }

    /// The text at a URL (cached), starting its fetch if needed: None while it loads. When it
    /// arrives, every view using it is queued again.
    fn url_text(&mut self, url: &str) -> Option<Result<Rc<str>, String>> {
        if let Some(r) = self.texts.get(url) {
            return Some(r.clone());
        }
        let src = url.to_string();
        if self.fetching.insert(src.clone()) {
            wasm_bindgen_futures::spawn_local(async move {
                let r = fetch_text(&src).await.map(|t| Rc::<str>::from(t.as_str()));
                with_app(|a| {
                    a.fetching.remove(&src);
                    a.texts.insert(src.clone(), r);
                    let uses = |v: &View| v.src.as_deref() == Some(src.as_str());
                    let waiting: Vec<u32> = a.views.iter().filter(|(_, v)| uses(v) && (!v.done || v.full) && (v.visible || v.full)).map(|(id, _)| *id).collect();
                    for id in waiting {
                        if !a.queue.contains(&id) {
                            a.queue.push_back(id);
                        }
                    }
                });
                ensure_frame();
            });
        }
        None
    }

    /// Thumbnails (and live viewers waiting for their text) within a time budget. Returns whether
    /// work is left. Only the thumbnails still on screen (or near it) are drawn: one that left
    /// before its turn is drawn when it comes back (critique: of 95 drawn during a fling on a
    /// phone, 83 were already off screen), and none during a fling (`FLING`): they wait until the
    /// scroll slows or stops (a deferred pass then).
    fn thumbs(&mut self) -> bool {
        if self.textures_pending {
            // drawn once, with the game's textures (load_textures or its timeout wakes us)
            return false;
        }
        let perf = window().performance();
        let t0 = perf.as_ref().map(|p| p.now()).unwrap_or(0.0);
        if debug() && !self.queue.is_empty() {
            log(&format!("work at {t0:.0}: {} queued", self.queue.len()));
        }
        // flinging: come back once it has slowed
        if t0 - self.scroll_ts < SCROLL_REST_MS && self.scroll_v > FLING && self.queue.iter().any(|id| self.views.get(id).is_some_and(|v| !v.full)) {
            if !self.defer_scheduled {
                self.defer_scheduled = true;
                let cb = Closure::once_into_js(move || {
                    with_app(|a| a.defer_scheduled = false);
                    ensure_frame();
                });
                let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), SCROLL_REST_MS as i32);
            }
            // live viewers waiting for their text still load
            let live: Vec<u32> = self.queue.iter().copied().filter(|id| self.views.get(id).is_some_and(|v| v.full && !v.done)).collect();
            for id in live {
                self.queue.retain(|q| *q != id);
                live::load(self, id);
            }
            return false;
        }
        let mut waiting = VecDeque::new();
        let slice_end = t0 + 12.0;
        // a card's picture being made: on with it first
        if self.thumb_job.is_some() && !self.advance_thumb(slice_end) {
            return true;
        }
        while let Some(id) = self.queue.pop_front() {
            if perf.as_ref().is_some_and(|p| p.now() - t0 > 12.0) {
                self.queue.push_front(id);
                break;
            }
            let Some(v) = self.views.get(&id) else { continue };
            if v.done {
                continue;
            }
            if v.full {
                live::load(self, id);
                continue;
            }
            // left the screen before its turn: drawn when it comes back (`on_intersect`)
            if !v.visible {
                continue;
            }
            let el = v.el.clone();
            match self.thumb {
                ThumbGpu::Pending => {
                    // the page's first card to draw: the thumbnails' context is made now (once
                    // the game's textures settled: a context made in a page's first moments
                    // waited 50-125 ms for the browser's own GPU set-up)
                    if let Some(c) = self.thumb_canvas.take() {
                        start_thumb_gpu(c, self.atlas.clone());
                    }
                    waiting.push_back(id);
                    continue;
                }
                ThumbGpu::Failed(ref e) => {
                    let e = e.clone();
                    self.fail(&el, &format!("WebGL2 unavailable: {e}"));
                    if let Some(v) = self.views.get_mut(&id) {
                        v.done = true;
                    }
                    continue;
                }
                ThumbGpu::Ready(_) => {}
            }
            match self.text_of(id) {
                None => {}
                Some(Err(e)) => {
                    self.fail(&el, &e);
                    if let Some(v) = self.views.get_mut(&id) {
                        v.done = true;
                    }
                }
                Some(Ok(text)) => {
                    self.thumb(id, text, slice_end);
                    // its next steps in the next slices
                    if self.thumb_job.is_some() {
                        break;
                    }
                }
            }
        }
        self.queue.extend(waiting);
        (!self.queue.is_empty() || self.thumb_job.is_some()) && !matches!(self.thumb, ThumbGpu::Pending)
    }

    /// One animation frame of every live viewer; true while one needs more (playing).
    fn live_frames(&mut self, ts: f64) -> bool {
        let mut more = false;
        let ids: Vec<u32> = self.views.iter().filter(|(_, v)| v.full).map(|(id, _)| *id).collect();
        for id in ids {
            more |= live::frame(self, id, ts);
        }
        more
    }

    /// Draw a thumbnail (from the cache when it has this source at this size).
    fn thumb(&mut self, id: u32, text: Rc<str>, slice_end: f64) {
        let t0 = window().performance().map(|p| p.now()).unwrap_or(0.0);
        let Some(v) = self.views.get(&id) else { return };
        let key = v.key();
        if debug() {
            log(&format!("thumb {key} start"));
        }
        let (w, h) = (v.w.max(1), v.h.max(1));
        if let Some(bmp) = self.cache.get(&key) {
            let ctx: Option<CanvasRenderingContext2d> = v.canvas.get_context("2d").ok().flatten().and_then(|c| c.dyn_into().ok());
            let Some(ctx) = ctx else { return };
            let _ = ctx.draw_image_with_image_bitmap(bmp, 0.0, 0.0);
            self.ready(id);
            return;
        }
        // its picture a step at a time (`ThumbJob`): as many steps as this slice has time for,
        // the rest in the next slices
        let budget = match window().performance() {
            Some(p) => mcview::staged::Budget::with_clock(Rc::new(move || p.now())),
            None => mcview::staged::Budget::none(),
        };
        let task = mcview::staged::Task::new(thumb_picture(self.atlas.clone(), text, v.still.clone(), v.yaw, v.pitch, w, h, budget.clone()));
        self.thumb_job = Some(ThumbJob { id, key, t0, budget, task, picture: None });
        self.advance_thumb(slice_end);
    }

    /// The card's picture being made ([`ThumbJob`]): its next steps until `slice_end` (ms), drawn
    /// once its triangles are ready (and this slice still has time). True once it is over.
    fn advance_thumb(&mut self, slice_end: f64) -> bool {
        let clock = || window().performance().map(|p| p.now()).unwrap_or(0.0);
        let Some(job) = self.thumb_job.as_mut() else { return true };
        if job.picture.is_none() {
            job.budget.give(slice_end - clock());
            job.picture = job.task.poll();
            if job.picture.is_none() {
                return false;
            }
        }
        // drawn in a slice of its own when this one's time is spent (a draw takes 20-30 ms in a
        // software GL)
        if clock() > slice_end && job.picture.as_ref().is_some_and(|p| p.is_ok()) {
            return false;
        }
        let Some(job) = self.thumb_job.take() else { return true };
        let (id, key, t0) = (job.id, job.key, job.t0);
        // the card changed (another design, another size) or left the page meanwhile: drawn again
        // from its new source, if it is still here; the thumbnails' context lost: drawn again once
        // it is back
        let Some(v) = self.views.get(&id) else { return true };
        if v.key() != key || v.done || matches!(self.thumb, ThumbGpu::Pending) {
            if !v.done && !self.queue.contains(&id) {
                self.queue.push_back(id);
            }
            return true;
        }
        let ctx: Option<CanvasRenderingContext2d> = v.canvas.get_context("2d").ok().flatten().and_then(|c| c.dyn_into().ok());
        let Some(ctx) = ctx else { return true };
        let mut marks = [0.0f64; 3];
        let mut tris = (0usize, 0usize);
        let mut steps = [0.0f64; 3];
        let drawn = job.picture.unwrap_or_else(|| Err("no picture".into())).and_then(|p| {
            tris = (p.solid.len(), p.translucent.len());
            steps = p.marks;
            let ThumbGpu::Ready(g) = &mut self.thumb else { return Err("no WebGL2".into()) };
            marks[0] = clock();
            g.draw(&Frame { solid: &p.solid, translucent: &p.translucent, camera: p.cam, width: p.w, height: p.h })?;
            marks[1] = clock();
            ctx.clear_rect(0.0, 0.0, p.w as f64, p.h as f64);
            ctx.draw_image_with_html_canvas_element(&g.canvas, 0.0, 0.0).map_err(|e| format!("{e:?}"))?;
            marks[2] = clock();
            Ok(())
        });
        match drawn {
            Ok(()) => {
                if debug() {
                    log(&format!(
                        "thumb {key} (still {:.0}, camera {:.0}, mesh {:.0}, draw {:.0}, copy {:.0}; {} solid + {} translucent triangles) done over {:.1} ms",
                        steps[0] - t0,
                        steps[1] - steps[0],
                        steps[2] - steps[1],
                        marks[1] - marks[0],
                        marks[2] - marks[1],
                        tris.0,
                        tris.1,
                        clock() - t0
                    ));
                }
                self.ready(id);
                let canvas = self.views[&id].canvas.clone();
                if let Ok(p) = window().create_image_bitmap_with_html_canvas_element(&canvas) {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(b) = wasm_bindgen_futures::JsFuture::from(p).await {
                            let b: ImageBitmap = b.unchecked_into();
                            with_app(|a| {
                                if a.cache.len() > 600 {
                                    a.cache.clear();
                                }
                                a.cache.insert(key, b);
                            });
                        }
                    });
                }
            }
            Err(e) => {
                let el = self.views[&id].el.clone();
                self.fail(&el, &e);
                if let Some(v) = self.views.get_mut(&id) {
                    v.done = true;
                }
            }
        }
        true
    }

    pub(crate) fn ready(&mut self, id: u32) {
        if let Some(v) = self.views.get_mut(&id) {
            v.done = true;
            let _ = v.canvas.style().set_property("visibility", "visible");
            let _ = v.el.class_list().add_1("mcview-ready");
        }
    }
}

/// An element's size in device pixels (capped at 2x).
pub(crate) fn pixel_size(el: &HtmlElement) -> (u32, u32) {
    let r = el.get_bounding_client_rect();
    let dpr = window().device_pixel_ratio().clamp(1.0, 2.0);
    ((r.width() * dpr).round() as u32, (r.height() * dpr).round() as u32)
}
