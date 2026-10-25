//! The live viewer: its own WebGL2 context on the element's canvas, a touch-sized control bar
//! (reset, play / pause, one game tick, speed, flip the lever, the tick counter in gt and seconds)
//! and gestures: one finger or the mouse drags to orbit, two fingers pinch to zoom and drag to pan
//! (right-drag / shift-drag with a mouse), the wheel zooms, a tap or click on a lever or button
//! uses it, a double tap / double click resets the view. Keys: space play/pause, right arrow or
//! `.` step, `r` reset, `f` flip.

use crate::gpu::Gpu;
use crate::{App, document, ensure_frame, log, next_task, window, with_app};
use mcview::player::Player;
use mcview::render::Frame;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlButtonElement, HtmlElement, KeyboardEvent, PointerEvent, WheelEvent};

/// A pointer down on the canvas (finger, pen or mouse).
struct Touch {
    id: i32,
    x: f64,
    y: f64,
    /// Where it went down (a tap moves less than a few pixels).
    sx: f64,
    sy: f64,
}

pub struct Live {
    gpu: Option<Gpu>,
    player: Option<Player>,
    label: HtmlElement,
    bar: HtmlElement,
    status: HtmlElement,
    play: HtmlButtonElement,
    /// One tick back (the timelapse): disabled at the run's start.
    back: HtmlButtonElement,
    flip: HtmlButtonElement,
    speed: HtmlButtonElement,
    speed_i: usize,
    /// The person picked a speed (the speed button): a fast machine no longer starts slowed.
    speed_chosen: bool,
    touches: Vec<Touch>,
    /// The current gesture moved (orbit, pan, pinch): not a tap.
    moved: bool,
    /// Mouse right-drag or shift-drag: pan instead of orbit.
    pan_button: bool,
    /// Two-finger gesture: last finger distance and midpoint.
    pinch: Option<(f64, f64, f64)>,
    /// When the last tap ended (ms), for double taps.
    last_tap: f64,
    last_ts: Option<f64>,
    /// A press that began on a circuit input (its cell, device or tag): (input, when, pointer):
    /// held past [`HOLD_MS`] it holds the input on until released; it never orbits the camera.
    press: Option<(usize, f64, i32)>,
    /// The input held on by a long press.
    holding: Option<usize>,
    /// The design being swapped in place (`data-src` changed): the previous camera (kept when
    /// `keep`) and whether it was playing.
    prev: Option<(mcview::camera::Camera, bool, bool)>,
    /// The player built from its still, its camera and the rest made in the next task
    /// (`finish_load`): the player, the design's text and when its load began (ms).
    loading: Option<(Player, std::rc::Rc<str>, f64)>,
    pub(crate) redraw: bool,
    overlays: crate::overlay::Overlays,
    /// One bar button per circuit input, and the level it shows.
    inputs: Vec<(HtmlButtonElement, bool)>,
    containers: HtmlButtonElement,
    /// The badge's text when no demo runs.
    base_status: String,
    shown_status: String,
    /// The design's text and name (a recording replays it from tick 0, a file named after it),
    /// and the auto-demo's schedule (a recording plays it whole).
    text: Option<std::rc::Rc<str>>,
    name: String,
    demo: Option<mcview::circuit::Schedule>,
    /// The Record button, and the recording running (drawn instead of the live view meanwhile).
    record: HtmlButtonElement,
    rec: Option<Rec>,
    /// A narrow viewer (a phone, [`NARROW_PX`]): the time badge on two lines.
    narrow: bool,
    /// The left badge says why the machine runs slowed (until the person picks a speed).
    slowed_note: bool,
}

/// A viewer narrower than this (CSS px) is a phone's: its time badge goes on two lines (seconds
/// over ticks), so the badge on the left keeps room for its words.
const NARROW_PX: f64 = 520.0;

/// The time badge's text: `6.15 s · 123 ticks`, on a narrow viewer one part per line.
fn time_text(p: &Player, narrow: bool) -> String {
    let t = p.label();
    if narrow { t.replace(" · ", "\n") } else { t }
}

/// A badge's words wrap between clauses, never inside a short one (critique: `demo: A off, B` /
/// `off`): the spaces of a clause of 3 words or fewer become non-breaking; a tick count stays
/// whole (`2 ticks (0.1 s)`, never `2` / `ticks (0.1 s)`).
fn badge_words(s: &str) -> String {
    let clauses = s.split(", ").map(|c| if c.split(' ').count() <= 3 { c.replace(' ', "\u{a0}") } else { c.to_string() }).collect::<Vec<_>>().join(", ");
    let words: Vec<&str> = clauses.split(' ').collect();
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            let prev = words[i - 1];
            let num = !prev.is_empty() && prev.chars().all(|c| c.is_ascii_digit() || c == ',');
            let glue = (num && w.starts_with("tick")) || (prev.starts_with("tick") && w.starts_with('(')) || (prev.starts_with('(') && (w.starts_with("s)") || w.starts_with("min")));
            out.push(if glue { '\u{a0}' } else { ' ' });
        }
        out.push_str(w);
    }
    out
}

/// Size the badge once for all the texts it will show, so it never resizes while they change:
/// as wide as the widest (measured unrounded, rounded up), or the widest it may be when a text
/// would wrap to more lines at that width than at its own (a greedy wrap at a narrower width can
/// push words to a third line: critique, "demo: 1st cycle: out and" cut); then at least as tall
/// as the tallest at that width (a minimum, never a clip: critique, WebKit cut the text at the
/// pill's bottom).
fn fit_badge(status: &HtmlElement, texts: &[String]) {
    let s = status.style();
    let _ = s.remove_property("width");
    let _ = s.remove_property("height");
    let _ = s.remove_property("min-height");
    let measure = |status: &HtmlElement| -> Vec<(f64, f64)> {
        texts
            .iter()
            .map(|t| {
                status.set_text_content(Some(t));
                let r = status.get_bounding_client_rect();
                (r.width(), r.height())
            })
            .collect()
    };
    let free = measure(status);
    let widest = free.iter().map(|(w, _)| *w).fold(0.0, f64::max);
    if widest <= 0.0 {
        return;
    }
    // the box counts the paddings (8 px each side, 3 px top and bottom)
    let _ = s.set_property("width", &format!("{}px", (widest - 16.0).ceil() + 1.0));
    let mut fixed = measure(status);
    if fixed.iter().zip(&free).any(|((_, h), (_, h0))| *h > *h0 + 1.0) {
        // as wide as it may be (its max-width: both count the content box)
        let max = s.get_property_value("max-width").unwrap_or_default();
        let _ = s.set_property("width", if max.is_empty() { "100%" } else { &max });
        fixed = measure(status);
    }
    let tallest = fixed.iter().map(|(_, h)| *h).fold(0.0, f64::max);
    let _ = s.set_property("min-height", &format!("{}px", (tallest - 6.0).ceil()));
}

/// A recording (the Record button: vjeux 2026-10-20 "add a record video button using
/// mediabunny"): its own player from tick 0, stepped exactly `1 / FPS` s of game time per frame
/// (exact partial ticks, never real time), each frame drawn by the viewer and composed with the
/// page's overlays on `canvas` (even size), which the encoder reads.
struct Rec {
    player: Player,
    frames: u32,
    canvas: web_sys::HtmlCanvasElement,
    ctx: web_sys::CanvasRenderingContext2d,
    /// The person pressed the button again: stop, no file.
    cancel: bool,
}

/// Speeds the speed button cycles through: label, game ticks per second.
const SPEEDS: [(&str, f32); 7] = [("1×", 20.0), ("2×", 40.0), ("5×", 100.0), ("20×", 400.0), ("100×", 2000.0), ("¼×", 5.0), ("½×", 10.0)];
/// The slow-motion speed a fast machine starts at (¼×).
const SLOW: usize = 5;

/// A frame's time for a live view's ticks ([`mcview::player::Player::update_with`]): past it the
/// ticks it could not play are dropped (the coordinator 2026-10-21: never block the page over
/// ~50 ms a frame, with every card a live view).
const STEP_BUDGET_MS: f64 = 12.0;

/// A press on an input longer than this (ms) holds it on (shorter: a tap).
const HOLD_MS: f64 = 250.0;

fn now_ms() -> f64 {
    crate::window().performance().map(|p| p.now()).unwrap_or(0.0)
}

fn el(tag: &str) -> HtmlElement {
    document().create_element(tag).unwrap().dyn_into().unwrap()
}

fn style(e: &HtmlElement, props: &[(&str, &str)]) {
    let s = e.style();
    for (k, v) in props {
        let _ = s.set_property(k, v);
    }
}

/// Inline SVG icons (glyphs like ⏭ are missing from many fonts).
const ICON_PLAY: &str = "M8 5v14l11-7z";
const ICON_PAUSE: &str = "M6 5h4v14H6zM14 5h4v14h-4z";
const ICON_STEP: &str = "M5 5v14l9-7zM15 5h3v14h-3z";
/// One tick back in time (the timelapse): the step icon mirrored.
const ICON_BACK: &str = "M9 5H6v14h3zM19 5v14l-9-7z";
const ICON_RESET: &str = "M12 5V1.5L7 6l5 4.5V7a5 5 0 1 1-5 5H5a7 7 0 1 0 7-7z";
const ICON_LEVER: &str = "M17 7H7a5 5 0 0 0 0 10h10a5 5 0 0 0 0-10zm0 8a3 3 0 1 1 0-6 3 3 0 0 1 0 6z";
const ICON_BOXES: &str = "M3 3h8v8H3zM13 3h8v8h-8zM3 13h8v8H3zM13 13h8v8h-8zM5 5v4h4V5zM15 5v4h4V5zM5 15v4h4v-4zM15 15v4h4v-4z";
/// The Record button's face: a red dot and its word; while recording, a stop square and the
/// progress.
const RECORD_FACE: &str = "<svg width=\"14\" height=\"14\" viewBox=\"0 0 14 14\" aria-hidden=\"true\" style=\"display:block\"><circle cx=\"7\" cy=\"7\" r=\"6\" fill=\"#ff3b30\"/></svg><span>Record</span>";
fn recording_face(pct: u32) -> String {
    format!("<svg width=\"12\" height=\"12\" viewBox=\"0 0 12 12\" aria-hidden=\"true\" style=\"display:block\"><rect x=\"1\" y=\"1\" width=\"10\" height=\"10\" rx=\"1.5\" fill=\"#fff\"/></svg><span style=\"font-variant-numeric:tabular-nums\">{pct}%</span>")
}

fn icon(path: &str) -> String {
    format!("<svg width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"currentColor\" aria-hidden=\"true\" style=\"display:block\"><path d=\"{path}\"/></svg>")
}

/// A touch-sized (44 px) button showing an icon (an SVG path) or a short text.
fn button(content: &str, title: &str) -> HtmlButtonElement {
    let b: HtmlButtonElement = document().create_element("button").unwrap().dyn_into().unwrap();
    if content.starts_with('M') && content.len() > 4 {
        b.set_inner_html(&icon(content));
    } else {
        b.set_text_content(Some(content));
    }
    b.set_title(title);
    let _ = b.set_attribute("aria-label", title);
    style(
        b.unchecked_ref(),
        &[
            ("min-width", "44px"),
            ("height", "44px"),
            ("padding", "0 10px"),
            ("flex", "0 0 auto"),
            ("display", "inline-flex"),
            ("align-items", "center"),
            ("justify-content", "center"),
            ("background", "rgba(20,22,28,0.72)"),
            ("color", "#fff"),
            ("border", "1px solid rgba(255,255,255,0.22)"),
            ("border-radius", "12px"),
            ("font", "600 18px/1 system-ui, sans-serif"),
            ("cursor", "pointer"),
            ("pointer-events", "auto"),
            // a long press (holding an input) never selects, zooms or opens the iOS callout
            ("touch-action", "none"),
            ("-webkit-tap-highlight-color", "transparent"),
            ("user-select", "none"),
            ("-webkit-user-select", "none"),
            ("-webkit-touch-callout", "none"),
        ],
    );
    on(&b, "contextmenu", |e: web_sys::Event| e.prevent_default());
    b
}

/// Run `f` on a live view and the atlas.
fn with_live<R>(id: u32, f: impl FnOnce(&mut Live, &mcview::atlas::Atlas, &mut crate::View) -> R) -> Option<R> {
    with_app(|a| {
        let atlas = a.atlas.clone();
        let v = a.views.get_mut(&id)?;
        let mut l = v.live.take()?;
        let r = f(&mut l, &atlas, v);
        v.live = Some(l);
        Some(r)
    })
    .flatten()
}

fn on<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(target: &web_sys::EventTarget, kind: &str, f: impl FnMut(E) + 'static) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(false);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(kind, cb.as_ref().unchecked_ref(), &opts);
    cb.forget();
}

/// A button that runs `f` on the live view, then draws.
fn on_click(b: &HtmlButtonElement, id: u32, f: fn(&mut Live)) {
    on(b, "click", move |e: web_sys::Event| {
        e.stop_propagation();
        with_live(id, |l, _, _| f(l));
        ensure_frame();
    });
}

/// Build the controls, start the WebGL2 context and the design's fetch.
pub fn start(app: &mut App, id: u32) {
    let v = app.views.get_mut(&id).unwrap();
    let root = v.el.clone();
    let _ = root.set_attribute("tabindex", "0");
    let _ = root.style().set_property("outline", "none");
    // nothing in a viewer selects on a long press (labels, tags, badges) or opens a callout
    for (k, val) in [("user-select", "none"), ("-webkit-user-select", "none"), ("-webkit-touch-callout", "none"), ("-webkit-tap-highlight-color", "transparent")] {
        let _ = root.style().set_property(k, val);
    }
    on(&root, "contextmenu", |e: web_sys::Event| e.prevent_default());
    on(&root, "selectstart", |e: web_sys::Event| e.prevent_default());
    let bar = el("div");
    // the bar, the tick badge and the status line are named for the page's tests (`mctas web
    // qa`: no tag under them)
    bar.set_class_name("mcview-bar");
    style(
        &bar,
        &[
            ("position", "absolute"),
            ("left", "0"),
            ("right", "0"),
            ("bottom", "0"),
            ("display", "flex"),
            ("gap", "6px"),
            ("align-items", "center"),
            ("flex-wrap", "nowrap"),
            ("padding", "8px"),
            ("height", "60px"),
            ("overflow", "hidden"),
            // only the buttons take the pointer: a tap on the scene behind the bar (a lever near
            // the bottom of the view) reaches the canvas
            ("pointer-events", "none"),
            ("background", "linear-gradient(transparent, rgba(0,0,0,0.55))"),
            ("color", "#fff"),
            ("font", "13px system-ui, sans-serif"),
            ("z-index", "2"),
            ("box-sizing", "border-box"),
        ],
    );
    let reset = button(ICON_RESET, "Back to the start (r)");
    let play = button(ICON_PLAY, "Play / pause (space)");
    let _ = play.style().set_property("min-width", "52px");
    let back = button(ICON_BACK, "One game tick back (←)");
    back.set_disabled(true);
    let step = button(ICON_STEP, "One game tick (→)");
    let speed = button(SPEEDS[0].0, "Speed (1× = 20 game ticks per second, real time)");
    let _ = speed.style().set_property("font-size", "15px");
    let flip = button(ICON_LEVER, "Flip the lever (f), or tap the lever");
    let _ = flip.set_attribute("data-mcview-input", "lever");
    let containers = button(ICON_BOXES, "Show what every container holds (or tap one)");
    let _ = containers.style().set_property("display", "none");
    // the tick counter: a badge at the top right, fixed width (tabular digits), so nothing moves
    // while it runs and the bar's buttons fit a phone
    let label = el("div");
    label.set_class_name("mcview-badge");
    style(
        &label,
        &[
            ("position", "absolute"),
            ("right", "8px"),
            ("top", "8px"),
            ("min-width", "21ch"),
            ("height", "20px"),
            ("line-height", "20px"),
            ("padding", "0 8px"),
            ("box-sizing", "content-box"),
            ("border-radius", "8px"),
            ("background", "rgba(0,0,0,0.45)"),
            ("color", "#fff"),
            ("font", "13px system-ui, sans-serif"),
            ("font-variant-numeric", "tabular-nums"),
            ("text-align", "right"),
            ("white-space", "nowrap"),
            ("pointer-events", "none"),
            ("z-index", "2"),
        ],
    );
    label.set_text_content(Some("0.00 s · 0 ticks"));
    for c in [reset.unchecked_ref::<HtmlElement>(), play.unchecked_ref(), back.unchecked_ref(), step.unchecked_ref(), speed.unchecked_ref(), flip.unchecked_ref(), containers.unchecked_ref()] {
        let _ = bar.append_child(c);
    }
    let _ = root.append_child(&bar);
    let _ = root.append_child(&label);
    let _ = flip.style().set_property("display", "none");
    // what to do or what runs: up to two lines on a narrow viewer (a demo phase in words never
    // ends in an ellipsis), sized once from its longest text so it never resizes
    let status = el("div");
    status.set_class_name("mcview-status");
    style(
        &status,
        &[
            ("position", "absolute"),
            ("left", "8px"),
            ("top", "8px"),
            ("max-width", "calc(100% - 200px)"),
            ("line-height", "16px"),
            ("padding", "3px 8px"),
            ("border-radius", "8px"),
            ("background", "rgba(0,0,0,0.45)"),
            ("color", "#fff"),
            ("font", "12px system-ui, sans-serif"),
            ("white-space", "normal"),
            ("overflow", "hidden"),
            ("display", "-webkit-box"),
            ("-webkit-line-clamp", "2"),
            ("-webkit-box-orient", "vertical"),
            ("pointer-events", "none"),
            ("z-index", "2"),
        ],
    );
    let _ = root.append_child(&status);
    // the Record button: a labelled pill over the view's bottom right above the bar (a phone's
    // bar is full; critique: a bare red dot read as a logo or a "live" light), where the browser
    // can encode video (WebCodecs); while recording it shows the progress and a press stops it
    let record = button(RECORD_TITLE, RECORD_TITLE);
    record.set_inner_html(RECORD_FACE);
    style(
        record.unchecked_ref(),
        &[("position", "absolute"), ("right", "8px"), ("bottom", "68px"), ("height", "36px"), ("min-width", "0"), ("padding", "0 12px 0 10px"), ("gap", "6px"), ("border-radius", "18px"), ("font", "600 14px/1 system-ui, sans-serif"), ("z-index", "2")],
    );
    let _ = record.set_attribute("data-mcview-record", "");
    if !crate::record::supported() {
        let _ = record.style().set_property("display", "none");
    }
    on(&record, "click", move |e: web_sys::Event| {
        e.stop_propagation();
        let busy = with_live(id, |l, _, _| match l.rec.as_mut() {
            Some(r) => {
                r.cancel = true;
                true
            }
            None => l.player.is_none(),
        })
        .unwrap_or(true);
        if !busy {
            wasm_bindgen_futures::spawn_local(record_video(id));
        }
    });
    let _ = root.append_child(&record);
    let mut overlays = crate::overlay::Overlays::new(&root);
    // no tag or label under the Record button or the badges (critique: TARGET touched the demo
    // badge's bottom edge)
    overlays.keep_clear.push(record.clone().unchecked_into());
    overlays.keep_clear.push(status.clone());
    overlays.keep_clear.push(label.clone());
    v.live = Some(Live {
        gpu: None,
        player: None,
        label,
        bar: bar.clone(),
        status,
        play: play.clone(),
        back: back.clone(),
        flip: flip.clone(),
        speed: speed.clone(),
        speed_i: 0,
        speed_chosen: false,
        touches: Vec::new(),
        moved: false,
        pan_button: false,
        pinch: None,
        last_tap: -1e9,
        last_ts: None,
        press: None,
        holding: None,
        prev: None,
        loading: None,
        redraw: true,
        overlays,
        inputs: Vec::new(),
        containers: containers.clone(),
        base_status: String::new(),
        shown_status: String::from("\u{0}"),
        text: None,
        name: String::from("design"),
        demo: None,
        record,
        rec: None,
        narrow: false,
        slowed_note: false,
    });
    let canvas = v.canvas.clone();
    let _ = canvas.style().set_property("touch-action", "none");
    on_click(&reset, id, |l| l.reset());
    on_click(&play, id, |l| l.toggle_play());
    on_click(&back, id, |l| l.step_back());
    on_click(&step, id, |l| l.step());
    on_click(&flip, id, |l| l.flip());
    on_click(&speed, id, |l| l.next_speed());
    on_click(&containers, id, |l| l.toggle_containers());
    // gestures
    on(&canvas, "pointerdown", move |e: PointerEvent| {
        let started = with_live(id, |l, atlas, v| {
            l.pointer_down(&e, atlas, v);
            let _ = v.canvas.set_pointer_capture(e.pointer_id());
            let _ = v.el.focus();
            l.press.is_some()
        });
        e.prevent_default();
        if started == Some(true) {
            // a press on an input: held long enough, it holds the input on
            let cb = Closure::once_into_js(move || {
                with_live(id, |l, _, _| l.check_hold());
                ensure_frame();
            });
            let _ = crate::window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), HOLD_MS as i32 + 10);
        }
    });
    on(&canvas, "pointermove", move |e: PointerEvent| {
        with_live(id, |l, _, v| l.pointer_move(&e, v));
        ensure_frame();
    });
    on(&canvas, "pointerup", move |e: PointerEvent| {
        with_live(id, |l, atlas, v| l.pointer_up(&e, atlas, v, true));
        ensure_frame();
    });
    on(&canvas, "pointercancel", move |e: PointerEvent| {
        with_live(id, |l, atlas, v| l.pointer_up(&e, atlas, v, false));
        ensure_frame();
    });
    on(&canvas, "wheel", move |e: WheelEvent| {
        e.prevent_default();
        with_live(id, |l, _, _| l.zoom((e.delta_y() as f32 * 0.0015).exp()));
        ensure_frame();
    });
    on(&canvas, "dblclick", move |e: web_sys::MouseEvent| {
        e.prevent_default();
        with_live(id, |l, _, _| l.home());
        ensure_frame();
    });
    on(&canvas, "contextmenu", move |e: web_sys::Event| e.prevent_default());
    on(&root, "keydown", move |e: KeyboardEvent| {
        let handled = with_live(id, |l, _, _| {
            match e.key().as_str() {
                " " => l.toggle_play(),
                "ArrowRight" | "." => l.step(),
                "ArrowLeft" | "," => l.step_back(),
                "r" => l.reset(),
                "f" => l.flip(),
                _ => return false,
            }
            true
        })
        .unwrap_or(false);
        if handled {
            e.prevent_default();
            ensure_frame();
        }
    });
    // the context, then the design
    // (no captured atlas here: the app's atlas is read when the context is made below —
    // the game's textures replace it while this task waits, and a captured one left every
    // live view on the hand-drawn atlas's white game texture)
    // a lost WebGL context (iOS drops the oldest when a page holds too many; a GPU reset) comes
    // back: the viewer makes a new one on the restored canvas and draws again
    on(&canvas, "webglcontextlost", move |e: web_sys::Event| {
        e.prevent_default();
        with_live(id, |l, _, _| l.gpu = None);
        log("a viewer's WebGL context was lost; waiting for it to come back");
    });
    on(&canvas, "webglcontextrestored", move |_: web_sys::Event| {
        let got = with_app(|a| a.views.get(&id).map(|v| (v.canvas.clone(), a.atlas.clone()))).flatten();
        if let Some((canvas, atlas)) = got {
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(g) = Gpu::new(canvas, &atlas).await {
                    with_live(id, |l, _, _| {
                        l.gpu = Some(g);
                        if let Some(p) = l.player.as_mut() {
                            p.invalidate();
                        }
                        l.redraw = true;
                    });
                    ensure_frame();
                }
            });
        }
    });
    wasm_bindgen_futures::spawn_local(async move {
        // the context once the game's textures settled (the first picture waits for them anyway;
        // a context made in a page's first moments waited 50-125 ms for the browser's own GPU
        // set-up, one made later takes a few), then the design, each in a task of its own
        while with_app(|a| a.textures_pending).unwrap_or(false) {
            crate::sleep_ms(16).await;
        }
        next_task().await;
        // the app's atlas as of now (not the one captured when the view was set up: the game's
        // textures replaced it while this task waited)
        let Some(atlas) = with_app(|a| a.atlas.clone()) else { return };
        match Gpu::new(canvas, &atlas).await {
            Ok(g) => {
                with_live(id, |l, _, _| {
                    l.gpu = Some(g);
                    l.redraw = true;
                });
            }
            Err(e) => {
                with_app(|a| {
                    if let Some(v) = a.views.get(&id) {
                        let el = v.el.clone();
                        a.fail(&el, &format!("WebGL2 unavailable: {e}"));
                    }
                });
            }
        }
        next_task().await;
        with_app(|a| load(a, id));
        ensure_frame();
    });
}

/// Create the player once the design text is here: its still in this task, its camera and the
/// rest in the next ([`finish_load`]; the coordinator 2026-10-22, no task over ~50-100 ms: a big
/// farm's camera search alone took 50-60 ms of its page's start in a software GL).
pub fn load(app: &mut App, id: u32) {
    let Some(v) = app.views.get(&id) else { return };
    if v.live.as_ref().is_none_or(|l| l.player.is_some() || l.loading.is_some()) {
        return;
    }
    let el = v.el.clone();
    let text = match app.text_of(id) {
        None => return,
        Some(Err(e)) => {
            app.fail(&el, &e);
            if let Some(v) = app.views.get_mut(&id) {
                v.done = true;
            }
            return;
        }
        Some(Ok(t)) => t,
    };
    // (vjeux: the view shows only what redsim simulates exactly: no real-game recording is
    // played; a design redsim cannot run is shown as built, saying why)
    let v = app.views.get_mut(&id).unwrap();
    let aspect = v.w.max(1) as f32 / v.h.max(1) as f32;
    let t0 = crate::window().performance().map(|p| p.now()).unwrap_or(0.0);
    match Player::new_lazy_with(&text, aspect, v.still.as_deref(), v.farm_rts) {
        Ok(p) => {
            if let Some(l) = v.live.as_mut() {
                l.loading = Some((p, text, t0));
            }
            wasm_bindgen_futures::spawn_local(async move {
                next_task().await;
                with_app(|a| finish_load(a, id));
                ensure_frame();
            });
        }
        Err(e) => {
            let el = v.el.clone();
            app.fail(&el, &e);
            if let Some(v) = app.views.get_mut(&id) {
                v.done = true;
            }
        }
    }
}

/// The rest of [`load`]: the player's camera, its bar, badges and labels.
fn finish_load(app: &mut App, id: u32) {
    let Some(v) = app.views.get_mut(&id) else { return };
    let loading = v.live.as_mut().and_then(|l| l.loading.take());
    let aspect = v.w.max(1) as f32 / v.h.max(1) as f32;
    // where the build goes: clear of the bar and the badges at this viewer's size, in CSS pixels
    // (the canvas counts device pixels: twice as many on a phone, which put the build under the
    // badges: critique, the e78st lever's box under the time badge); on a phone, clear of the
    // Record pill too when the build would come out under it (`scene::live_camera`)
    let css_h = match v.canvas.client_height() {
        h if h > 0 => h as f32,
        _ => v.h.max(1) as f32,
    };
    let css_w = match v.canvas.client_width() {
        w if w > 0 => w as f32,
        _ => v.w.max(1) as f32,
    };
    let narrow_view = (css_w as f64) < NARROW_PX;
    let mut window = mcview::scene::live_window_css(css_w, css_h);
    let clock = || crate::window().performance().map(|p| p.now()).unwrap_or(0.0);
    let mut marks = [clock(); 3];
    match loading {
        // (dropped meanwhile: another design swapped in)
        None => return,
        Some((mut p, text, t0)) => {
            // played, its session is built over frames (`Player::wake`: a slice a frame; the
            // coordinator 2026-10-22, no task over ~50-100 ms)
            if let Some(perf) = crate::window().performance() {
                p.set_clock(std::rc::Rc::new(move || perf.now()));
            }
            // (a design the site knows the browser cannot run: the still stays, nothing plays)
            if v.asbuilt {
                p.keep_still("the browser's simulator cannot run it yet");
            }
            if v.yaw.is_some() || v.pitch.is_some() {
                if let Some(y) = v.yaw {
                    p.camera.yaw = y.to_radians();
                }
                if let Some(pt) = v.pitch {
                    p.camera.pitch = pt.to_radians();
                }
                let (lo, hi) = mcview::scene::frame_bounds(&p.session, &p.look);
                p.camera.fit(lo, hi, aspect, 0.85);
                p.home = p.camera;
            } else {
                window = p.frame_live(&app.atlas, css_w, css_h);
            }
            marks[1] = clock();
            if let Some(t) = v.tick {
                p.run_on_wake(t);
            }
            let l = v.live.as_mut().unwrap();
            // a recording replays the design from tick 0 and is named after it
            l.text = Some(text.clone());
            l.name = file_name(v.src.as_deref(), p.session.design.meta.get("name").map(|s| s.as_str()));
            l.demo = None;
            // a narrow viewer (a phone): the time badge on two lines, seconds over ticks
            // (critique: at 390 px it took half the width and squeezed the demo badge), the badge
            // on the left as wide as the rest of the top
            l.narrow = narrow_view;
            let time_style: &[(&str, &str)] = if l.narrow {
                &[("height", "auto"), ("line-height", "16px"), ("padding", "2px 8px"), ("min-width", "11ch"), ("white-space", "pre-line")]
            } else {
                &[("height", "20px"), ("line-height", "20px"), ("padding", "0 8px"), ("min-width", "21ch"), ("white-space", "nowrap")]
            };
            style(&l.label, time_style);
            l.label.set_text_content(Some(&time_text(&p, l.narrow)));
            // the left badge ends 6 px short of it: its width counts no padding (16 px), and both
            // stand 8 px from the edges
            let _ = l.status.style().set_property("max-width", &format!("calc(100% - {}px)", l.label.offset_width() + 38));
            let circuit = !p.session.circuit_inputs.is_empty() && p.session.simulated();
            if p.session.simulated() && !circuit && (p.session.mode.is_door() || !p.session.inputs.is_empty()) {
                let _ = l.flip.style().set_property("display", "inline-flex");
            }
            if !p.session.playable() {
                l.play.set_disabled(true);
                let _ = l.play.style().set_property("opacity", "0.4");
            }
            if let Some(ph) = &v.phases {
                p.phases = mcview::circuit::parse_phases(ph);
            }
            if circuit {
                // the judge's way to drive each input, a button per input, the auto-demo
                if let Some(m) = &v.input_mode {
                    p.pulse_inputs = m.trim() == "pulse";
                }
                let names: Vec<String> = p.session.circuit_inputs.iter().map(|c| c.name.clone()).collect();
                let mut made: Vec<web_sys::HtmlButtonElement> = Vec::new();
                for (i, name) in names.iter().enumerate() {
                    let ci = p.session.circuit_inputs[i].clone();
                    let frame = ci.how == mcview::circuit::InputHow::Frame;
                    let full = mcview::circuit::input_label(&p.session.design, &ci);
                    // a lock's frame: "Dial 1" for its "DIAL 1" tag (critique 058332315: not the
                    // raw port names D0..K2); when the bar has no room for them all, the tags
                    // press the inputs instead (below)
                    let label: String = match full.split_once(' ') {
                        Some((w, n)) if frame => format!("{}{} {n}", &w[..1], w[1..].to_lowercase()),
                        _ => name.to_uppercase().chars().take(3).collect(),
                    };
                    let how = if frame {
                        "tap to turn it one step of 8 (a right-click)"
                    } else if p.pulse_inputs {
                        "hold for a pulse; a tap gives one of 2 ticks (0.1 s)"
                    } else {
                        "tap to turn on / off"
                    };
                    let b = button(&label, &format!("{full}: {how}, or tap its cell"));
                    let _ = b.style().set_property("color", "#a6f05a");
                    let _ = b.style().set_property("font-size", "14px");
                    // which input it presses (as its tag says: `data-mcview-input`), for tests
                    let _ = b.set_attribute("data-mcview-input", &i.to_string());
                    made.push(b.clone());
                    if p.pulse_inputs {
                        on(&b, "pointerdown", move |e: PointerEvent| {
                            e.prevent_default();
                            with_live(id, |l, _, _| l.input_down(i));
                            ensure_frame();
                        });
                        for kind in ["pointerup", "pointerleave", "pointercancel"] {
                            on(&b, kind, move |_: PointerEvent| {
                                with_live(id, |l, _, _| l.input_up(i));
                                ensure_frame();
                            });
                        }
                    } else {
                        on(&b, "click", move |e: web_sys::Event| {
                            e.stop_propagation();
                            with_live(id, |l, _, _| l.input_tap(i));
                            ensure_frame();
                        });
                    }
                    l.inputs.push((b, false));
                }
                // in the bar left to right as the view shows the inputs (critique: the AND
                // gate's A tag on the right, its A button on the left)
                for i in mcview::scene::inputs_left_to_right(&p.session, &p.camera) {
                    let _ = l.bar.append_child(&made[i]);
                }
            }
            // a new design (a swap): the badge sized again from its own texts
            let _ = l.status.style().remove_property("width");
            let _ = l.status.style().remove_property("height");
            let _ = l.status.style().remove_property("min-height");
            // the auto-demo: the judge's stimulus on the inputs and its actions at cells
            // (data-events: a BUD switch's target and footprint probes), played when the view is
            // played (site 2026-10-21: no page autoplays; "Press play under the view to watch our
            // test on a loop")
            if p.session.simulated() && (v.stimulus.is_some() || v.events.is_some() || v.levels.is_some()) {
                let names: Vec<String> = p.session.circuit_inputs.iter().map(|c| c.name.clone()).collect();
                let mut demo = match &v.stimulus {
                    Some(st) if !names.is_empty() => match mcview::circuit::parse_stimulus(st, &names, 40) {
                        Ok(s) => Some(s),
                        Err(e) => {
                            log(&format!("data-stimulus: {e}"));
                            None
                        }
                    },
                    _ => None,
                };
                if let Some(ev) = &v.events {
                    match mcview::circuit::parse_events(ev) {
                        Ok(events) if !events.is_empty() => {
                            let d = demo.get_or_insert_with(|| mcview::circuit::Schedule { edges: vec![Vec::new(); names.len()], ticks: 0, events: Vec::new(), levels: Vec::new() });
                            let last = events.iter().map(|(t, _, _)| *t).max().unwrap_or(0);
                            d.ticks = d.ticks.max(last + 40);
                            d.events = events;
                        }
                        Ok(_) => {}
                        Err(e) => log(&format!("data-events: {e}")),
                    }
                }
                // a strength input's values (data-levels: the judge's chest fills)
                if let Some(lv) = &v.levels {
                    match mcview::circuit::parse_levels(lv, &names) {
                        Ok(levels) if !levels.is_empty() => {
                            let d = demo.get_or_insert_with(|| mcview::circuit::Schedule { edges: vec![Vec::new(); names.len()], ticks: 0, events: Vec::new(), levels: Vec::new() });
                            let last = levels.iter().map(|(t, _, _)| *t).max().unwrap_or(0);
                            d.ticks = d.ticks.max(last + 40);
                            d.levels = levels;
                        }
                        Ok(_) => {}
                        Err(e) => log(&format!("data-levels: {e}")),
                    }
                }
                if let Some(d) = demo {
                    l.demo = Some(d.clone());
                    p.attach_demo(d);
                    p.skip_demo_wait();
                    // the badge shows each phase in turn: as wide (and as tall: two lines on a
                    // narrow viewer) as its longest label from the start, so it never resizes
                    // while the demo plays
                    let mut labels = vec![demo_text(&p, None)];
                    labels.extend(p.phases.iter().map(|(_, _, ph)| demo_text(&p, Some(ph))));
                    fit_badge(&l.status, &labels);
                    l.shown_status = String::from("\u{0}");
                }
            }
            // a door with data-autoplay: its lever flipped like the record videos (the door
            // demo), the badge sized once from its four texts
            if v.autoplay && p.session.mode.is_door() && p.session.simulated() {
                p.start_door_demo();
                let words: Vec<String> = mcview::player::DoorDemo::ALL_WORDS.iter().map(|t| badge_words(t)).collect();
                fit_badge(&l.status, &words);
                l.shown_status = String::from("\u{0}");
            }
            l.overlays.setup(&p);
            marks[2] = clock();
            // the output strip under the time badge, however tall it is
            l.overlays.set_strip_top(8.0 + l.label.offset_height() as f64 + 6.0);
            // the contents button: shown with containers, gold while every container is labelled
            let has = mcview::labels::has_containers(&p.session);
            let _ = l.containers.style().set_property("display", if has { "inline-flex" } else { "none" });
            let _ = l.containers.style().set_property("color", if l.overlays.show_all { "#ffd27a" } else { "#fff" });
            // the flip and contents buttons say what they are (the pages' "Try it" names them;
            // critique: a four-squares button with no label), as long as the bar has room: a word
            // the bar cannot hold goes, the flip button's first
            let word = |w: &str| format!("<span style=\"margin-left:4px;font-size:14px\">{w}</span>");
            l.flip.set_inner_html(&format!("{}{}", icon(ICON_LEVER), word("Flip")));
            l.containers.set_inner_html(&format!("{}{}", icon(ICON_BOXES), word("Labels")));
            if l.bar.scroll_width() > l.bar.client_width() {
                l.flip.set_inner_html(&icon(ICON_LEVER));
            }
            if l.bar.scroll_width() > l.bar.client_width() {
                l.containers.set_inner_html(&icon(ICON_BOXES));
            }
            // still no room (a combination lock's three dials): the inputs' buttons go, their
            // tags press them (each says which input it is)
            if l.bar.scroll_width() > l.bar.client_width() {
                for (b, _) in &l.inputs {
                    let _ = b.style().set_property("display", "none");
                }
            }
            // pressing an input's tag presses the input: a tap, or a hold while pressed
            for (i, t) in l.overlays.input_tags.clone() {
                on(&t, "pointerdown", move |e: PointerEvent| {
                    e.prevent_default();
                    e.stop_propagation();
                    if let Some(target) = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                        let _ = target.set_pointer_capture(e.pointer_id());
                    }
                    with_live(id, |l, _, _| {
                        l.press = Some((i, now_ms(), e.pointer_id()));
                        l.touches.clear();
                        l.touches.push(Touch { id: e.pointer_id(), x: e.client_x() as f64, y: e.client_y() as f64, sx: e.client_x() as f64, sy: e.client_y() as f64 });
                    });
                    let cb = Closure::once_into_js(move || {
                        with_live(id, |l, _, _| l.check_hold());
                        ensure_frame();
                    });
                    let _ = crate::window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), HOLD_MS as i32 + 10);
                });
                for (kind, up) in [("pointerup", true), ("pointercancel", false)] {
                    on(&t, kind, move |e: PointerEvent| {
                        e.stop_propagation();
                        with_live(id, |l, _, _| {
                            l.touches.retain(|t| t.id != e.pointer_id());
                            l.end_press(&e, up);
                        });
                        ensure_frame();
                    });
                }
            }
            l.base_status = match &p.session.unsimulated {
                Some(why) => {
                    let _ = l.status.set_attribute("title", why);
                    format!("not simulated: {}", short_reason(why))
                }
                // a combination lock: its frames are turned, not pressed (its DIAL tags or
                // buttons; the lamp lights at the CODE frames' turns)
                None if circuit && p.session.circuit_inputs.iter().any(|c| c.how == mcview::circuit::InputHow::Frame) => "tap a DIAL to turn it to its CODE".to_string(),
                None if circuit => format!("{} the inputs", if p.pulse_inputs { "press" } else { "tap" }),
                None => match p.session.mode {
                    mcview::Mode::Door { .. } => "tap the lever".to_string(),
                    _ => String::new(),
                },
            };
            l.slowed_note = false;
            l.player = Some(p);
            l.redraw = true;
            l.sync_status();
            // swapped in place: the person's camera (unless the new build is out of it), play
            // state and speed stay
            let mut playing = v.autoplay;
            if let Some((cam, was_playing, keep)) = l.prev.take() {
                let p = l.player.as_mut().unwrap();
                if keep {
                    // the person's camera stays (its direction always); when the new build does
                    // not fit it, the distance and target are fitted again (never cut)
                    let mut c = cam;
                    if !camera_fits(&c, &p.session, &p.look, aspect) {
                        mcview::scene::fit_view(&mut c, &mcview::scene::view_points_for(&p.session, &p.look, aspect), aspect, window);
                    }
                    p.camera = c;
                }
                playing |= was_playing;
            }
            // a machine whose blocks repeat in under 10 gt starts in slow motion: decided once it
            // plays (`Live::pick_cycle_speed`: the still a page loads as has no cycle yet)
            l.pick_cycle_speed();
            if let Some(p) = l.player.as_mut() {
                p.tps = SPEEDS[l.speed_i].1;
            }
            l.play.set_inner_html(&icon(ICON_PLAY));
            if playing {
                l.toggle_play();
            }
            let _ = v.el.class_list().add_1("mcview-live-src");
            if crate::debug() {
                log(&format!("live view {id} load (its camera's task): the camera {:.0}, the labels {:.0}, the bar {:.0}, done in {:.1} ms ({:.0} ms after the load began)", marks[1] - marks[0], marks[2] - marks[1], clock() - marks[2], clock() - marks[0], clock() - t0));
            }
        }
    }
    if let Some(v) = app.views.get_mut(&id) {
        v.done = true;
    }
}

/// Does a camera still show a session's build (its frame box's centre inside the view)?
/// Does the camera show the session's whole build (every point of [`mcview::scene::view_points`]
/// inside the view)?
fn camera_fits(cam: &mcview::camera::Camera, s: &mcview::session::Session, look: &mcview::scene::Look, aspect: f32) -> bool {
    let (f, r, u) = cam.basis();
    let eye = cam.eye();
    let t = (cam.vfov.to_radians() / 2.0).tan();
    mcview::scene::view_points_for(s, look, aspect).iter().all(|c| {
        let v = [c[0] - eye[0], c[1] - eye[1], c[2] - eye[2]];
        let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
        let z = dot(f);
        z > 0.1 && (dot(r) / (z * t * aspect)).abs() <= 0.97 && (dot(u) / (z * t)).abs() <= 0.97
    })
}

/// The demo badge's text for a phase label: `demo: A on, B off`.
fn demo_text(p: &Player, phase: Option<&str>) -> String {
    let names: Vec<String> = p.session.circuit_inputs.iter().map(|c| c.name.clone()).collect();
    let kind = p.session.design.meta.get("circuit").map(|s| s.as_str()).unwrap_or("");
    let words = match phase {
        Some(ph) => format!("demo: {}", mcview::circuit::phase_words(kind, ph, &names)),
        None => "demo: the test sequence".to_string(),
    };
    badge_words(&words)
}

/// The badge's text for a player: why it stopped, the demo's phase, the door demo's step, else
/// `base` (what to do).
fn status_for(p: &Player, base: &str) -> String {
    match () {
        _ if p.session.halted.is_some() => halted_words(p.session.halted.as_deref().unwrap_or_default()),
        _ if p.session.demo.is_some() => demo_text(p, p.phase()),
        _ if p.door_demo.is_some() => badge_words(p.door_demo.map(|d| d.words()).unwrap_or_default()),
        _ => base.to_string(),
    }
}

/// Why redsim stopped, in a few words (`tick 44: dropper at (0,1,0) picks a random slot among 9
/// stacks (not modeled)` -> `stopped at tick 44: dropper picks a random slot among 9 stacks`):
/// the page said only "not simulated after tick 44" (QA at phone size, counter576-items).
fn halted_words(h: &str) -> String {
    let (when, why) = h.split_once(": ").unwrap_or(("this tick", h));
    let mut why = why.to_string();
    while let Some(i) = why.find(" at (") {
        match why[i..].find(')') {
            Some(j) => why.replace_range(i..i + j + 1, ""),
            None => break,
        }
    }
    for tail in [" (not modeled)", ", not modeled", " (item entity, not modeled)"] {
        if let Some(s) = why.strip_suffix(tail) {
            why = s.to_string();
        }
    }
    format!("stopped at {when}: {}", why.trim())
}

/// A recording's file name: the design's file name (`designs/clk-x.txt` -> `clk-x`), else its
/// `name:`, else `design`; only letters, digits, `.`, `_` and `-`.
fn file_name(src: Option<&str>, name: Option<&str>) -> String {
    let stem = src.and_then(|s| s.split(['?', '#']).next()).and_then(|s| s.rsplit('/').next()).map(|s| s.strip_suffix(".txt").unwrap_or(s)).filter(|s| !s.is_empty()).or(name).unwrap_or("design");
    let clean: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }).collect();
    if clean.is_empty() { "design".into() } else { clean }
}

/// The Record button's tooltip.
const RECORD_TITLE: &str = "Record a video of the run from the start";

/// Record a video of a live view (the Record button): its own player replayed from tick 0 for
/// the whole run ([`Player::recording_ticks`]: the demo, a door's cycle, clock periods, at most
/// 20 s at the current speed), every frame drawn and composed with the overlays, encoded by the
/// vendored mediabunny and downloaded; a press on the button meanwhile stops it. The live view
/// comes back as it was.
async fn record_video(id: u32) {
    let started = with_live(id, |l, atlas, v| l.rec_start(atlas, v)).unwrap_or_else(|| Err("the viewer is gone".into()));
    let (canvas, w, h, frames, name) = match started {
        Ok(x) => x,
        Err(e) => {
            log(&format!("record: {e}"));
            return;
        }
    };
    let enc = match crate::record::Encoder::new(&canvas, w, h).await {
        Ok(e) => e,
        Err(e) => {
            with_live(id, |l, _, _| l.rec_end(Some(&e)));
            ensure_frame();
            return;
        }
    };
    for k in 0..frames {
        if !with_live(id, |l, atlas, v| l.rec_frame(atlas, v, k)).unwrap_or(false) {
            enc.cancel().await;
            with_live(id, |l, _, _| l.rec_end(None));
            ensure_frame();
            return;
        }
        if let Err(e) = enc.add(k).await {
            enc.cancel().await;
            with_live(id, |l, _, _| l.rec_end(Some(&e)));
            ensure_frame();
            return;
        }
    }
    with_live(id, |l, _, _| l.record.set_inner_html("<span>Saving…</span>"));
    // with ?mcview-debug: the last frame's colours as the encoder read them (`mctas web recqa`)
    let grid = if crate::debug() { crate::record::grid_colours(&canvas) } else { String::new() };
    let (ext, mime) = (enc.ext, enc.mime);
    let file = format!("{name}.{ext}");
    let saved = match enc.finish().await {
        Ok(buf) => {
            // what was saved (the console; the tests read the viewer's attribute with
            // `?mcview-debug`)
            let note = format!("{file} {} bytes {frames} frames {w}x{h} {mime}", buf.byte_length());
            log(&format!("record: saved {note}"));
            if crate::debug() {
                let _ = with_live(id, |_, _, v| v.el.set_attribute("data-mcview-recorded", &format!("{note} grid={grid}")));
            }
            crate::record::download(&buf, mime, &file)
        }
        Err(e) => Err(e),
    };
    with_live(id, |l, _, _| l.rec_end(saved.err().as_deref()));
    ensure_frame();
}

/// Why redsim cannot run a design, short (the badge; the whole reason in its tooltip):
/// `block "oak_fence" is not modeled by redsim; ...` -> `oak_fence`.
fn short_reason(why: &str) -> String {
    let first = why.split(';').next().unwrap_or(why).trim();
    if let Some(rest) = first.strip_prefix("block \"")
        && let Some(name) = rest.split('"').next()
    {
        return format!("{name} (redsim does not model it)");
    }
    first.split(':').next().unwrap_or(first).trim().to_string()
}

/// Advance and draw one frame; true while it needs more frames (playing, dragging).
pub fn frame(app: &mut App, id: u32, ts: f64) -> bool {
    let atlas = app.atlas.clone();
    let mut need_ready = false;
    let more = {
        let Some(v) = app.views.get_mut(&id) else { return false };
        let (w, h) = (v.w.max(1), v.h.max(1));
        let visible = v.visible;
        let Some(l) = v.live.as_mut() else { return false };
        // a recording draws its own frames meanwhile (the live view resumes after)
        if l.rec.is_some() {
            return false;
        }
        // off screen: neither drawn nor played (coordinator: only on-screen live viewers render
        // and animate); its time stands still until it is back (`App::on_intersect`)
        if !visible {
            l.last_ts = None;
            return false;
        }
        let dt = l.last_ts.map(|t| ((ts - t) / 1000.0) as f32).unwrap_or(0.0);
        l.last_ts = Some(ts);
        let Some(p) = l.player.as_mut() else { return false };
        // a frame's time for its ticks (the coordinator 2026-10-21: never block the page over
        // ~50 ms a frame): the ticks past it are dropped, never queued
        let perf = window().performance();
        let t0 = perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
        let mut out_of_time = || perf.as_ref().is_some_and(|q| q.now() - t0 > STEP_BUDGET_MS);
        let mut changed = p.update_with(dt, &mut out_of_time) | std::mem::take(&mut l.redraw);
        let t_update = perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
        // the view just woke up (played or pressed): its speed from the session that plays
        if p.take_woke() {
            l.pick_cycle_speed();
            changed = true;
        }
        let t_pick = perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
        let (mut t_mesh, mut t_draw, mut t_over) = (t_pick, t_pick, t_pick);
        let (Some(p), Some(g)) = (l.player.as_mut(), l.gpu.as_mut()) else { return false };
        p.tps = SPEEDS[l.speed_i].1;
        if changed {
            let (solid, translucent) = p.triangles(&atlas);
            t_mesh = perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
            match g.draw(&Frame { solid: &solid, translucent: &translucent, camera: p.camera, width: w, height: h }) {
                Ok(()) => need_ready = true,
                Err(e) => log(&e),
            }
            t_draw = perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
            l.label.set_text_content(Some(&time_text(p, l.narrow)));
            // the back button is live once the run has a tick behind it
            l.back.set_disabled(!p.can_step_back());
            if crate::debug() {
                // for `mctas web layout`: nothing may move while the counter runs (not counted
                // while its session is built over frames: the still it shows then is not the view
                // that plays, whose speed badge comes with the session)
                let r = |e: &HtmlElement| {
                    let b = e.get_bounding_client_rect();
                    format!("{:.1},{:.1},{:.1},{:.1}", b.x(), b.y(), b.width(), b.height())
                };
                if !p.is_waking() {
                    log(&format!("layout view {id} tick {} el {} bar {} label {} status {} cam {:?} {} {} {} vfov {}", p.session.tick, r(&v.el), r(&l.bar), r(&l.label), r(&l.status), p.camera.target, p.camera.yaw, p.camera.pitch, p.camera.dist, p.camera.vfov));
                }
                // for `mctas web click`: where a click uses each input (fractions of the canvas),
                // and what the sim shows
                let cr = v.canvas.get_bounding_client_rect();
                let aspect = (cr.width() / cr.height().max(1.0)) as f32;
                let inputs: Vec<String> = p
                    .session
                    .inputs
                    .clone()
                    .iter()
                    .map(|c| {
                        // a circuit input: its own state (a strength chest is always a block);
                        // a lever or button of a door: its cell's
                        let on = match p.session.circuit_inputs.iter().position(|ci| ci.pos == *c) {
                            Some(i) => p.session.input_on.get(i).copied().unwrap_or(false),
                            None => p.session.is_on(*c),
                        } as u8;
                        match p.screen_point(&atlas, *c, aspect) {
                            Some((x, y)) => format!("{:.4},{:.4},{on}", (x + 1.0) / 2.0, (1.0 - y) / 2.0),
                            None => format!("-,-,{on}"),
                        }
                    })
                    .collect();
                let door = p.session.door_blocks().map(|n| n.to_string()).unwrap_or_else(|| "-".into());
                let active = p.session.design.blocks.keys().filter(|c| !p.session.inputs.contains(c) && p.session.is_on(**c)).count();
                // a lock's frames: each one's turn (a tap turns it, nothing else may show it)
                let frames: Vec<String> = (0..p.session.circuit_inputs.len()).filter_map(|i| p.session.frame_turn(i).map(|t| t.to_string())).collect();
                let frames = if frames.is_empty() { "-".to_string() } else { frames.join(",") };
                let c = &p.camera;
                let cam = format!("{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}", c.yaw, c.pitch, c.dist, c.target[0], c.target[1], c.target[2]);
                let _ = v.el.set_attribute("data-mcview-state", &format!("tick={} playing={} door={door} active={active} frames={frames} cam={cam} inputs={}", p.session.tick, p.playing as u8, inputs.join("|")));
            }
            l.overlays.update(p, &atlas, v.canvas.client_width() as f64, v.canvas.client_height() as f64);
            t_over = perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
        }
        let playing = p.playing;
        if changed {
            l.sync_status();
            l.sync_inputs();
        }
        // with `?mcview-debug`, a slow frame's steps (for `mctas web stall`)
        let t_end = perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
        if crate::debug() && t_end - t0 > 30.0 {
            log(&format!(
                "live view {id} tick {}: play {:.0}, cycle pick {:.0}, picture {:.0}, draw {:.0}, labels {:.0}, badges {:.0} done in {:.1} ms",
                l.player.as_ref().map(|p| p.session.tick).unwrap_or(0),
                t_update - t0,
                t_pick - t_update,
                t_mesh - t_pick,
                t_draw - t_mesh,
                t_over - t_draw,
                t_end - t_over.max(t_pick),
                t_end - t0
            ));
        }
        playing
    };
    if need_ready {
        app.ready(id);
    }
    more
}

impl Live {
    /// Start a recording: its player (the live one's camera, look, speed and demo, from tick 0),
    /// its length and its canvas. Returns (canvas, width, height, frames, file name).
    fn rec_start(&mut self, atlas: &mcview::atlas::Atlas, v: &crate::View) -> Result<(web_sys::HtmlCanvasElement, u32, u32, u32, String), String> {
        let _ = atlas;
        if self.rec.is_some() {
            return Err("already recording".into());
        }
        let live = self.player.as_ref().ok_or("not loaded yet")?;
        let text = self.text.clone().ok_or("no design")?;
        let (cw, ch) = (v.w.max(2), v.h.max(2));
        let mut p = Player::new_web_with(&text, cw as f32 / ch as f32, v.farm_rts)?;
        // the live view's marks (its page's output cell and picture box)
        if p.session.outputs.is_empty() {
            p.session.outputs = live.session.outputs.clone();
        }
        if p.session.picture.is_none() {
            p.session.picture = live.session.picture;
        }
        p.camera = live.camera;
        p.look = live.look.clone();
        p.tps = live.tps;
        p.phases = live.phases.clone();
        p.pulse_inputs = live.pulse_inputs;
        if let Some(d) = &self.demo {
            p.attach_demo(d.clone());
            p.skip_demo_wait();
        } else if let Some((d, phases)) = mcview::player::lock_demo(&p.session) {
            // a lock without a demo (its page stays paused): its dials turned to the code, open,
            // shut again
            p.attach_demo(d);
            p.phases = phases;
        }
        // a door: its demo (closed, opening, open, closing), one cycle
        let door = p.session.mode.is_door() && p.session.simulated() && p.session.demo.is_none();
        let cycle = if door { mcview::player::door_demo_cycle(&text) } else { None };
        if door {
            p.start_door_demo();
        }
        p.playing = p.session.playable();
        let fps = crate::record::FPS;
        let max_ticks = (crate::record::MAX_SECONDS * p.tps as f64).floor() as u32;
        let ticks = p.recording_ticks(cycle, max_ticks);
        let secs = (ticks as f64 / p.tps.max(0.1) as f64).clamp(1.0, crate::record::MAX_SECONDS);
        let frames = (secs * fps).ceil() as u32;
        // the view's own pixels, at most 1920 x 1080, both even (4:2:0 video)
        let (mw, mh) = crate::record::MAX_SIZE;
        let s = (mw / cw as f64).min(mh / ch as f64).min(1.0);
        let w = (((cw as f64 * s) as u32) / 2 * 2).max(2);
        let h = (((ch as f64 * s) as u32) / 2 * 2).max(2);
        let canvas: web_sys::HtmlCanvasElement = document().create_element("canvas").map_err(|_| "no canvas")?.dyn_into().map_err(|_| "no canvas")?;
        canvas.set_width(w);
        canvas.set_height(h);
        let ctx: web_sys::CanvasRenderingContext2d = canvas.get_context("2d").ok().flatten().and_then(|c| c.dyn_into().ok()).ok_or("no 2D context")?;
        self.rec = Some(Rec { player: p, frames, canvas: canvas.clone(), ctx, cancel: false });
        self.record.set_inner_html(&recording_face(0));
        let _ = self.record.style().set_property("background", "rgba(200,30,20,0.85)");
        self.record.set_title("Recording: press to stop");
        Ok((canvas, w, h, frames, self.name.clone()))
    }

    /// Draw frame `k` of the recording (game time `k / FPS` s after its start) into its canvas:
    /// the 3D view, then the tags, labels and badges where the page lays them out. False: stop
    /// (cancelled, or nothing to draw with).
    fn rec_frame(&mut self, atlas: &mcview::atlas::Atlas, v: &crate::View, k: u32) -> bool {
        let Live { rec, gpu, overlays, label, status, base_status, record, narrow, .. } = self;
        let Some(r) = rec.as_mut() else { return false };
        let Some(g) = gpu.as_mut() else { return false };
        if r.cancel {
            return false;
        }
        if k > 0 {
            r.player.update((1.0 / crate::record::FPS) as f32);
        }
        let (solid, translucent) = r.player.triangles(atlas);
        if let Err(e) = g.draw(&Frame { solid: &solid, translucent: &translucent, camera: r.player.camera, width: v.w.max(1), height: v.h.max(1) }) {
            log(&format!("record: {e}"));
            return false;
        }
        overlays.update(&r.player, atlas, v.canvas.client_width() as f64, v.canvas.client_height() as f64);
        label.set_text_content(Some(&time_text(&r.player, *narrow)));
        let st = status_for(&r.player, base_status);
        status.set_text_content(Some(&st));
        let _ = status.style().set_property("visibility", if st.is_empty() { "hidden" } else { "visible" });
        let (fw, fh) = (r.canvas.width() as f64, r.canvas.height() as f64);
        r.ctx.set_image_smoothing_enabled(true);
        if r.ctx.draw_image_with_html_canvas_element_and_dw_and_dh(&v.canvas, 0.0, 0.0, fw, fh).is_err() {
            return false;
        }
        let cr = v.canvas.get_bounding_client_rect();
        let (origin, scale) = ((cr.left(), cr.top()), fw / cr.width().max(1.0));
        crate::record::paint(&r.ctx, overlays.layer(), origin, scale);
        crate::record::paint(&r.ctx, status, origin, scale);
        crate::record::paint(&r.ctx, label, origin, scale);
        record.set_inner_html(&recording_face((k + 1) * 100 / r.frames.max(1)));
        true
    }

    /// The recording is over (saved, stopped or failed): the live view comes back as it was.
    fn rec_end(&mut self, error: Option<&str>) {
        self.rec = None;
        self.record.set_inner_html(RECORD_FACE);
        let _ = self.record.style().set_property("background", "rgba(20,22,28,0.72)");
        match error {
            Some(e) => {
                log(&format!("record: {e}"));
                self.record.set_title(&format!("Recording failed: {e}"));
            }
            None => self.record.set_title(RECORD_TITLE),
        }
        self.redraw = true;
        self.last_ts = None;
        self.shown_status = String::from("\u{0}");
    }

    /// A machine whose blocks repeat in under 10 gt (a 4 gt clock flickers at 5 Hz at 1x) plays in
    /// slow motion, ¼x, until the person picks a speed (exact frames: nothing is interpolated
    /// between them but moving blocks and entities); not a TNT design (its point is the flight
    /// and the blast: a launcher repeating every 6 gt at ¼x took 16 s to its first explosion), nor
    /// an item sorter, smelter or brewer (their judge's stream moves at a hopper's pace). Decided
    /// on the session that plays (a still has no cycle: `Session::short_cycle` is None there):
    /// at load and again when the view wakes.
    fn pick_cycle_speed(&mut self) {
        if self.speed_chosen {
            return;
        }
        let Some(p) = self.player.as_ref() else { return };
        // looked for as the view woke, over its frames (`Player::wake`; a still has none)
        let fast = p.cycle_found().flatten();
        self.set_speed(if fast.is_some() { SLOW } else { 0 });
        // why it runs slowed, and how to change it (critique: "¼×" with no explanation), until
        // the person picks a speed
        if let Some(n) = fast
            && self.base_status.is_empty()
        {
            self.base_status = badge_words(&format!("slowed (tap {} to change): it repeats every {}", SPEEDS[SLOW].0, mcview::circuit::ticks_words(n)));
            self.slowed_note = true;
            self.sync_status();
        }
    }

    fn toggle_play(&mut self) {
        if let Some(p) = self.player.as_mut() {
            if !p.session.playable() {
                return;
            }
            p.playing = !p.playing;
            // a door's play is its test on a loop (the door demo), from the still it loaded as
            if p.playing && p.is_still() && p.session.mode.is_door() && p.door_demo.is_none() {
                p.start_door_demo();
            }
            self.play.set_inner_html(&icon(if p.playing { ICON_PAUSE } else { ICON_PLAY }));
            self.last_ts = None;
            self.redraw = true;
        }
    }

    fn step(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.playing = false;
            self.play.set_inner_html(&icon(ICON_PLAY));
            p.step();
            self.redraw = true;
        }
    }

    /// One tick back in time (the timelapse): pauses first, like [`Live::step`].
    fn step_back(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.playing = false;
            self.play.set_inner_html(&icon(ICON_PLAY));
            p.step_back();
            self.redraw = true;
        }
    }

    fn reset(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.reset();
            self.redraw = true;
        }
    }

    fn flip(&mut self) {
        if let Some(p) = self.player.as_mut()
            && p.flip()
        {
            self.redraw = true;
            self.play_after_input();
        }
    }

    /// After using a lever or button: run the simulation so it reacts (a door opens) —
    /// playing, but NOT the door demo. An explicit press of Play starts the demo; a flip
    /// is one toggle, so the door toggles once and settles (before, a flip on the still
    /// the view loaded as started the demo through `toggle_play`, and the door looped
    /// open/closed forever instead of settling: e78k-h5-41b).
    fn play_after_input(&mut self) {
        if let Some(p) = self.player.as_mut() {
            if !p.session.playable() || p.playing {
                return;
            }
            p.playing = true;
            self.play.set_inner_html(&icon(ICON_PAUSE));
            self.last_ts = None;
            self.redraw = true;
        }
    }

    /// A circuit input's bar button pressed (pulse inputs: on while held).
    fn input_down(&mut self, i: usize) {
        if let Some(p) = self.player.as_mut()
            && p.input_down(i)
        {
            self.redraw = true;
            self.play_after_input();
        }
    }

    fn input_up(&mut self, i: usize) {
        if let Some(p) = self.player.as_mut() {
            p.input_up(i);
            self.redraw = true;
        }
    }

    /// A circuit input's bar button tapped (level inputs: flips).
    fn input_tap(&mut self, i: usize) {
        if let Some(p) = self.player.as_mut()
            && p.tap_input(i)
        {
            self.redraw = true;
            self.play_after_input();
        }
    }

    fn toggle_containers(&mut self) {
        self.overlays.show_all = !self.overlays.show_all;
        let _ = self.containers.style().set_property("color", if self.overlays.show_all { "#ffd27a" } else { "#fff" });
        self.redraw = true;
    }

    /// The badge: the demo's phase (in words) while it runs, else what runs; a run redsim could
    /// not follow stops and says so.
    fn sync_status(&mut self) {
        let text = match &self.player {
            Some(p) => {
                if let Some(why) = &p.session.halted {
                    let _ = self.status.set_attribute("title", why);
                }
                status_for(p, &self.base_status)
            }
            None => self.base_status.clone(),
        };
        if text != self.shown_status {
            self.status.set_text_content(Some(&text));
            // nothing to say: no badge
            let _ = self.status.style().set_property("visibility", if text.is_empty() { "hidden" } else { "visible" });
            self.shown_status = text;
        }
    }

    /// The input buttons show their input's level (pressed = on).
    fn sync_inputs(&mut self) {
        let Some(p) = &self.player else { return };
        for (i, (b, shown)) in self.inputs.iter_mut().enumerate() {
            let on = p.session.input_level(i);
            if on != *shown {
                *shown = on;
                let s = b.style();
                let _ = s.set_property("background", if on { "rgba(110,200,40,0.85)" } else { "rgba(20,22,28,0.72)" });
                let _ = s.set_property("color", if on { "#10200a" } else { "#a6f05a" });
            }
        }
    }

    fn next_speed(&mut self) {
        self.speed_chosen = true;
        self.set_speed((self.speed_i + 1) % SPEEDS.len());
        // the person picked a speed: the slow-motion note goes
        if std::mem::take(&mut self.slowed_note) {
            self.base_status.clear();
            self.sync_status();
        }
    }

    /// Show and use speed `i` of [`SPEEDS`].
    fn set_speed(&mut self, i: usize) {
        self.speed_i = i;
        let (name, tps) = SPEEDS[self.speed_i];
        self.speed.set_text_content(Some(name));
        self.speed.set_title(&format!("Speed: {name} = {tps} game ticks per second"));
        if let Some(p) = self.player.as_mut() {
            p.tps = tps;
        }
    }

    fn home(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.camera = p.home;
            self.redraw = true;
        }
    }

    /// Zoom by a distance factor (> 1: farther).
    fn zoom(&mut self, f: f32) {
        if let Some(p) = self.player.as_mut() {
            p.camera.dist = (p.camera.dist * f).clamp(1.0, 2000.0);
            p.keep_camera_above_ground();
            self.redraw = true;
        }
    }

    pub fn resized(&mut self) {
        self.redraw = true;
    }

    /// The tiles changed (the game's textures arrived): upload them, draw again.
    pub fn atlas_changed(&mut self, atlas: &mcview::atlas::Atlas) {
        if let Some(g) = &mut self.gpu {
            g.r.upload_atlas(atlas);
        }
        if let Some(p) = self.player.as_mut() {
            p.invalidate();
        }
        self.redraw = true;
    }

    pub fn has_player(&self) -> bool {
        self.player.is_some()
    }

    /// Before another design loads in place: drop the player (remembering its camera and play
    /// state), its input buttons and per-design controls; the canvas keeps its last frame.
    pub fn begin_reload(&mut self, keep_camera: bool) {
        if let Some(p) = self.player.take() {
            self.prev = Some((p.camera, p.playing, keep_camera));
        }
        // (a load of the previous design not finished yet: dropped)
        self.loading = None;
        for (b, _) in self.inputs.drain(..) {
            b.remove();
        }
        let _ = self.flip.style().set_property("display", "none");
        let _ = self.containers.style().set_property("display", "none");
        self.play.set_disabled(false);
        let _ = self.play.style().set_property("opacity", "1");
        let _ = self.status.remove_attribute("title");
        self.press = None;
        self.holding = None;
        self.redraw = true;
    }

    fn pointer_down(&mut self, e: &PointerEvent, atlas: &mcview::atlas::Atlas, v: &crate::View) {
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        self.touches.retain(|t| t.id != e.pointer_id());
        self.touches.push(Touch { id: e.pointer_id(), x, y, sx: x, sy: y });
        match self.touches.len() {
            1 => {
                self.moved = false;
                self.pan_button = e.button() != 0 || e.shift_key();
                self.pinch = None;
                // a press that begins on a circuit input (its cell or device) is the input's:
                // a tap or a hold, never an orbit
                self.press = None;
                if !self.pan_button
                    && let Some(p) = &self.player
                {
                    let r = v.canvas.get_bounding_client_rect();
                    let fx = ((x - r.left()) / r.width().max(1.0) * 2.0 - 1.0) as f32;
                    let fy = (1.0 - (y - r.top()) / r.height().max(1.0) * 2.0) as f32;
                    let aspect = (r.width() / r.height().max(1.0)) as f32;
                    if let Some(i) = p.input_at(atlas, fx, fy, aspect) {
                        self.press = Some((i, now_ms(), e.pointer_id()));
                    }
                }
            }
            _ => {
                // two fingers: pinch and pan, never a tap
                self.moved = true;
                self.press = None;
                self.pinch = Some(self.two_finger());
            }
        }
    }

    /// A press on an input lasting [`HOLD_MS`] holds it on (until the pointer lifts).
    pub fn check_hold(&mut self) {
        let Some((i, t0, _)) = self.press else { return };
        if self.holding.is_some() || self.touches.len() != 1 || now_ms() - t0 < HOLD_MS - 1.0 {
            return;
        }
        if let Some(p) = self.player.as_mut()
            && p.hold_input(i, true)
        {
            self.holding = Some(i);
            self.redraw = true;
            self.play_after_input();
        }
    }

    /// The pointer of a press lifted or cancelled: a held input goes off; a short press is a tap.
    /// Returns whether the press was an input's (then nothing else happens).
    fn end_press(&mut self, e: &PointerEvent, up: bool) -> bool {
        let Some((i, _, pid)) = self.press else { return false };
        if pid != e.pointer_id() {
            return false;
        }
        self.press = None;
        if let Some(h) = self.holding.take() {
            if let Some(p) = self.player.as_mut() {
                p.hold_input(h, false);
            }
            self.redraw = true;
            return true;
        }
        if up
            && let Some(p) = self.player.as_mut()
            && p.tap_input(i)
        {
            // a tap on the input: today's pulse or flip
            self.redraw = true;
            self.play_after_input();
        }
        true
    }

    /// The first two pointers' distance and midpoint.
    fn two_finger(&self) -> (f64, f64, f64) {
        let (a, b) = (&self.touches[0], &self.touches[1]);
        (((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt().max(1.0), (a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }

    fn pointer_move(&mut self, e: &PointerEvent, v: &crate::View) {
        let Some(i) = self.touches.iter().position(|t| t.id == e.pointer_id()) else { return };
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        let (dx, dy) = (x - self.touches[i].x, y - self.touches[i].y);
        self.touches[i].x = x;
        self.touches[i].y = y;
        let h = v.el.get_bounding_client_rect().height().max(1.0);
        // a press on an input never moves the camera
        if self.press.is_some() && self.touches.len() < 2 {
            return;
        }
        let Some(p) = self.player.as_mut() else { return };
        if self.touches.len() >= 2 {
            let (d0, mx0, my0) = self.pinch.unwrap_or((1.0, x, y));
            let (a, b) = (&self.touches[0], &self.touches[1]);
            let (d1, mx1, my1) = (((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt().max(1.0), (a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            p.camera.dist = (p.camera.dist * (d0 / d1) as f32).clamp(1.0, 2000.0);
            p.camera.pan(((mx1 - mx0) / h) as f32, ((my1 - my0) / h) as f32);
            self.pinch = Some((d1, mx1, my1));
        } else {
            let t = &self.touches[i];
            if (t.x - t.sx).abs() + (t.y - t.sy).abs() > 6.0 {
                self.moved = true;
            }
            if !self.moved {
                return;
            }
            if self.pan_button {
                p.camera.pan((dx / h) as f32, (dy / h) as f32);
            } else {
                p.camera.orbit(-(dx as f32) * 0.008, (dy as f32) * 0.008);
            }
        }
        p.keep_camera_above_ground();
        self.redraw = true;
    }

    /// A pointer lifted (`up`) or cancelled: a tap uses the lever or button under it; a double
    /// tap resets the view.
    fn pointer_up(&mut self, e: &PointerEvent, atlas: &mcview::atlas::Atlas, v: &crate::View, up: bool) {
        let was_single = self.touches.len() == 1;
        self.touches.retain(|t| t.id != e.pointer_id());
        if self.touches.len() == 1 {
            self.pinch = None;
        }
        if self.end_press(e, up) {
            return;
        }
        if !up || !was_single || self.moved {
            return;
        }
        let now = crate::window().performance().map(|p| p.now()).unwrap_or(0.0);
        if e.pointer_type() == "touch" && now - self.last_tap < 320.0 {
            self.last_tap = -1e9;
            self.home();
            return;
        }
        self.last_tap = now;
        let r = v.canvas.get_bounding_client_rect();
        let x = ((e.client_x() as f64 - r.left()) / r.width().max(1.0) * 2.0 - 1.0) as f32;
        let y = (1.0 - (e.client_y() as f64 - r.top()) / r.height().max(1.0) * 2.0) as f32;
        let aspect = (r.width() / r.height().max(1.0)) as f32;
        // a container tapped (not an input): its label shows or hides
        if let Some(p) = self.player.as_ref()
            && p.input_at(atlas, x, y, aspect).is_none()
            && self.overlays.toggle_pin_at(e.client_x() as f64 - r.left(), e.client_y() as f64 - r.top(), p.pick(atlas, x, y, aspect))
        {
            self.redraw = true;
            return;
        }
        if let Some(p) = self.player.as_mut()
            && p.click(atlas, x, y, aspect).is_some()
        {
            self.redraw = true;
            self.play_after_input();
        }
    }
}
