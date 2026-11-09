//! The live viewer's HTML overlays: a circuit's tags (its inputs IN, A, B... in lime, outputs in
//! orange, a shulker design's ITEMS / BOXES chests, a BUD switch's TARGET; a shulker design's
//! same-kind chests that stand together share one tag), what the containers hold (an icon and a
//! count per kind: `[cobblestone] 1,344`, a shulker box `12/27`, at most two kinds and `+n`), and
//! the outputs' recent history.
//!
//! By default only what tells the story is labelled: the port chests (their contents inside
//! their tag) and the shulker boxes being filled or emptied. Any other container gets its label
//! when tapped (a tap again hides it); the bar's contents button labels them all. Placed every
//! frame by projecting their points with the player's camera, then laid out so that nothing
//! overlaps and no leader line crosses a tag or label (`mcview::labels::Layout`).

use crate::document;
use mcview::atlas::Atlas;
use mcview::labels::{self, Entry, Key, Layout, Rect};
use mcview::player::{HISTORY, Player};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlElement};

/// One container's label: its element, what it shows (to rebuild only on change), its size.
struct Label {
    el: HtmlElement,
    content: String,
    size: (f64, f64),
}

/// A tag (a port's name over its cells); the containers there (judge's chests) show their
/// contents inside the tag, after the name: one pill per port or group of ports.
/// How a label counts what a container holds: items (a design's own container), stacks (a judge's
/// chest it keeps full: how full), or the run's tally (a judge's chest it empties: what reached
/// it so far).
#[derive(Clone, Copy, PartialEq)]
enum Count {
    Items,
    Stacks,
    Tally,
}

struct Tag {
    el: HtmlElement,
    /// The world point it hangs at (over its cells' middle).
    point: [f32; 3],
    /// Its cells (the containers whose contents it shows).
    cells: Vec<[i32; 3]>,
    contents: HtmlElement,
    content: String,
    /// Its size, measured again when its contents change.
    size: (f64, f64),
    /// Its colour, and the output cell it names (its text red while that output is powered).
    colour: String,
    out: Option<[i32; 3]>,
    lit: bool,
    /// A judge chest's role note (`kept full`, `emptied`) and its element: an emptied chest's tag
    /// shows the run's tally (what reached it, `Session::received`) and says `received`.
    role: Option<&'static str>,
    note: Option<HtmlElement>,
    /// An item frame input's index (a combination lock's dial or code frame): the tag shows the
    /// frame's turn (0..7, the judge's digits) after its name.
    frame: Option<usize>,
    /// A display face's lamp (`circuit::display_face`): shown only with the Labels button on
    /// (the coordinator 2026-10-22: no OUT tag on every lamp by default).
    face: bool,
}

pub struct Overlays {
    layer: HtmlElement,
    /// The leader lines from moved tags and labels to their cells.
    leaders: Option<(HtmlCanvasElement, CanvasRenderingContext2d)>,
    tags: Vec<Tag>,
    /// The input tags (input index, tag): pressing one presses its input.
    pub input_tags: Vec<(usize, HtmlElement)>,
    labels: HashMap<Key, Label>,
    /// Item icons as data URLs (None: no texture: a coloured square), for the game assets
    /// `icons_of` (rebuilt when the item atlas arrives).
    icons: HashMap<String, Option<String>>,
    icons_of: (usize, bool),
    /// Every container labelled (the bar's contents button), not only the story's.
    pub show_all: bool,
    /// Containers labelled because they were tapped.
    pinned: HashSet<Key>,
    /// Where each container (besides the port chests) was on the last frame, for taps: its key,
    /// its box on the view, its distance.
    hits: Vec<(Key, Rect, f32)>,
    history: Option<(HtmlCanvasElement, CanvasRenderingContext2d)>,
    /// What the strip is (`output, last 2 s`), beside it.
    history_caption: Option<HtmlElement>,
    /// The viewer's own controls over the view (the Record button): no tag or label goes there.
    pub keep_clear: Vec<HtmlElement>,
    /// A TNT duper standing above its judge's ground: its height in words and the line from the
    /// design's bottom down to the ground (top and bottom points).
    drop: Option<(HtmlElement, [f32; 3], [f32; 3])>,
    /// The output strip's top (CSS px): under the time badge ([`Overlays::set_strip_top`]).
    strip_top: f64,
    /// The still blocks' cells (a tag never covers the block right above its cell), with the
    /// session's blocks version (and tick, where it does not log its writes) they were read at:
    /// read again only when the blocks change (a farm in its terrain draws tens of thousands of
    /// blocks: reading them every frame took ~4 ms of the phone's frame in WebKit).
    solid_cells: Option<((u64, u32), std::rc::Rc<HashSet<[i32; 3]>>)>,
}

fn div(css: &str) -> HtmlElement {
    let e: HtmlElement = document().create_element("div").unwrap().dyn_into().unwrap();
    let _ = e.set_attribute("style", css);
    e
}

fn span(css: &str, text: &str) -> HtmlElement {
    let e: HtmlElement = document().create_element("span").unwrap().dyn_into().unwrap();
    let _ = e.set_attribute("style", css);
    e.set_text_content(Some(text));
    e
}

/// A furnace's fire and arrow after its label's contents, as its screen draws them
/// (`labels::Cooking`): the flame burning down with the fuel's time left (14 steps, 7 px), the
/// arrow filling as the item cooks (24 steps, 12 px).
fn furnace_marks(el: &HtmlElement, c: mcview::labels::Cooking) {
    let flame = div("position:relative;width:7px;height:7px;flex:none;border-radius:1px;background:#4a3a2a;overflow:hidden");
    let fire = div(&format!("position:absolute;left:0;right:0;bottom:0;height:{:.2}px;background:linear-gradient(#ffe066,#ff7a1a)", c.flame as f64 * 7.0 / 14.0));
    let _ = flame.append_child(&fire);
    let _ = el.append_child(&flame);
    let arrow = div("width:12px;height:4px;flex:none;border-radius:2px;background:rgba(255,255,255,0.25);overflow:hidden");
    let fill = div(&format!("height:100%;width:{:.2}px;background:#fff", c.arrow as f64 * 12.0 / 24.0));
    let _ = arrow.append_child(&fill);
    let _ = el.append_child(&arrow);
    let lit = if c.flame > 0 { "burning" } else { "out" };
    let title = el.get_attribute("aria-label").unwrap_or_default();
    let _ = el.set_attribute("aria-label", &format!("{title}{}furnace {lit}, cooking {}%", if title.is_empty() { "" } else { "; " }, c.arrow as u32 * 100 / 24));
}

/// The colour of an item without a texture (the items redsim models).
fn item_color(item: &str) -> &'static str {
    match item {
        "stone" | "cobblestone" => "#8f8f8f",
        "redstone" => "#d01c10",
        "snowball" => "#f2f6fa",
        "ender_pearl" => "#1f8a7a",
        "wooden_shovel" => "#a07a45",
        "diamond_sword" => "#4ad8d0",
        "paper" => "#f4f1e4",
        id if id.ends_with("shulker_box") => "#9a6ea6",
        _ => "#c0c0c0",
    }
}

/// The game texture set an icon was made from (its address, and whether the item atlas is in).
fn game_of(atlas: &Atlas) -> (usize, bool) {
    atlas.game.as_ref().map(|g| (std::rc::Rc::as_ptr(g) as usize, g.entities)).unwrap_or((0, false))
}

fn show_at(e: &HtmlElement, r: &Rect) {
    let s = e.style();
    let _ = s.set_property("display", "flex");
    let _ = s.set_property("left", &format!("{:.1}px", r.x0));
    let _ = s.set_property("top", &format!("{:.1}px", r.y0));
}

fn hide(e: &HtmlElement) {
    let _ = e.style().set_property("display", "none");
}

/// An element's size (laid out now if needed).
fn measure(e: &HtmlElement) -> (f64, f64) {
    let _ = e.style().set_property("display", "flex");
    (e.offset_width() as f64, e.offset_height() as f64)
}

impl Overlays {
    pub fn new(parent: &HtmlElement) -> Overlays {
        let layer = div("position:absolute;left:0;top:0;right:0;bottom:0;pointer-events:none;overflow:hidden;z-index:1;font:600 11px system-ui,sans-serif");
        // named for the page's tests (`mctas web qa`: the tags must not overlap)
        layer.set_class_name("mcview-tags");
        let _ = parent.append_child(&layer);
        let leaders = (|| {
            let c: HtmlCanvasElement = document().create_element("canvas").ok()?.dyn_into().ok()?;
            let _ = c.set_attribute("style", "position:absolute;left:0;top:0;width:100%;height:100%;pointer-events:none");
            let _ = layer.append_child(&c);
            let ctx = c.get_context("2d").ok()??.dyn_into::<CanvasRenderingContext2d>().ok()?;
            Some((c, ctx))
        })();
        Overlays {
            layer,
            leaders,
            tags: Vec::new(),
            input_tags: Vec::new(),
            labels: HashMap::new(),
            icons: HashMap::new(),
            icons_of: (0, false),
            show_all: false,
            pinned: HashSet::new(),
            hits: Vec::new(),
            history: None,
            history_caption: None,
            keep_clear: Vec::new(),
            drop: None,
            strip_top: 34.0,
            solid_cells: None,
        }
    }

    /// Put the output strip (and its caption) at `top` CSS px: under the time badge, however
    /// tall it is (two lines on a phone).
    pub fn set_strip_top(&mut self, top: f64) {
        self.strip_top = top;
        if let Some((c, _)) = &self.history {
            let _ = c.style().set_property("top", &format!("{top}px"));
        }
        if let Some(t) = &self.history_caption {
            let _ = t.style().set_property("top", &format!("{}px", top - 2.0));
        }
    }

    /// The layer holding the tags, labels, leaders and the output strip (a recording paints it).
    pub fn layer(&self) -> &HtmlElement {
        &self.layer
    }

    /// The tags and the history strip of a (new) player.
    pub fn setup(&mut self, p: &Player) {
        for t in self.tags.drain(..) {
            t.el.remove();
        }
        for (_, l) in self.labels.drain() {
            l.el.remove();
        }
        self.input_tags.clear();
        self.pinned.clear();
        self.solid_cells = None;
        // what tells the story: a shulker design's port chests and boxes (its hoppers and carts
        // on a tap); any other design's containers all (an item counter's droppers hold its count);
        // a display's face: its digit (its lamps' OUT tags and its chest's label with Labels on)
        let face = mcview::circuit::display_face(&p.session.design).is_some();
        self.show_all = !mcview::circuit::is_shulker(&p.session.design) && !face;
        self.hits.clear();
        let make = |layer: &HtmlElement, text: &str, color: &str, cells: Vec<[i32; 3]>| -> Tag {
            let el = div(&format!("position:absolute;display:none;align-items:center;gap:4px;padding:1px 5px;border-radius:6px;background:rgba(0,0,0,0.62);color:{color};white-space:nowrap;width:max-content;letter-spacing:0.5px;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none"));
            let _ = el.append_child(&span("", text));
            let contents = span("display:none;align-items:center;gap:2px;color:#fff;font:600 10px/14px system-ui,sans-serif;letter-spacing:0", "");
            let _ = el.append_child(&contents);
            let _ = layer.append_child(&el);
            // over the middle of its cells, at the top one's height
            let n = cells.len().max(1) as f32;
            let mid = |k: usize| cells.iter().map(|c| c[k] as f32 + 0.5).sum::<f32>() / n;
            let top = cells.iter().map(|c| c[1]).max().unwrap_or(0) as f32 + 1.05;
            Tag { el, point: [mid(0), top, mid(2)], cells, contents, content: String::new(), size: (0.0, 0.0), colour: color.to_string(), out: None, lit: false, role: None, note: None, frame: None, face: false }
        };
        for (i, ci) in p.session.circuit_inputs.iter().enumerate() {
            let name = mcview::circuit::input_label(&p.session.design, ci);
            let mut t = make(&self.layer, &name, "#a6f05a", vec![ci.pos]);
            if ci.how == mcview::circuit::InputHow::Frame {
                t.frame = Some(i);
            }
            // pressing the tag presses its input (a tap or a hold)
            let s = t.el.style();
            let _ = s.set_property("pointer-events", "auto");
            let _ = s.set_property("cursor", "pointer");
            let _ = s.set_property("touch-action", "none");
            let _ = t.el.set_attribute("data-mcview-input", &i.to_string());
            self.input_tags.push((i, t.el.clone()));
            self.tags.push(t);
        }
        // a circuit's other ports: its outputs, a shulker design's chests (same-kind neighbours
        // share one tag), a BUD switch's target; a shulker design's judge chests say what the
        // judge does to them (critique: an ITEMS count that never goes down): it keeps a loader's
        // sources and every box chest full, and empties the outputs, the rejects and an
        // unloader's item chests
        let kind = p.session.design.meta.get("circuit").map(|k| k.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default();
        for g in mcview::circuit::port_groups(&p.session.design) {
            let mut t = make(&self.layer, &g.label, g.colour, g.cells);
            t.face = face && g.label.starts_with("OUT");
            let role = match (kind.as_str(), g.label.split(' ').next().unwrap_or("")) {
                ("shulker-loader" | "sorter", "ITEMS") | ("shulker-loader" | "shulker-unloader", "BOXES") => Some("kept full"),
                ("shulker-loader" | "shulker-unloader", "OUT" | "REJECT") | ("shulker-unloader", "ITEMS") | ("sorter", "OUT" | "PASS") => Some("emptied"),
                _ => None,
            };
            if let Some(r) = role {
                let note = span("color:#d8d8d8;font-weight:500;letter-spacing:0", r);
                let _ = t.el.insert_before(&note, Some(&t.contents));
                t.role = Some(r);
                t.note = Some(note);
            }
            self.tags.push(t);
        }
        // (no tag over a TNT duper's water: vjeux 2026-10-21, "please remove the water label, we
        // can see it": the view's tags are its controls and outputs only)
        // a TNT duper standing above its judge's ground (critique: from above it looked like it
        // sat on the grass): how high, by a dashed line from its bottom down to the ground
        if let Some(d) = self.drop.take() {
            d.0.remove();
        }
        if let Some(f) = &p.session.floor {
            let (lo, hi) = f.design_box;
            let up = lo[1] - 1 - f.hi[1];
            if up > 0 {
                let el = div("position:absolute;display:none;align-items:center;padding:1px 5px;border-radius:6px;background:rgba(0,0,0,0.62);color:#fff;font:600 10px/14px system-ui,sans-serif;white-space:nowrap;width:max-content");
                el.set_text_content(Some(&format!("{up} block{} above the ground", if up == 1 { "" } else { "s" })));
                let _ = self.layer.append_child(&el);
                let (x, z) = ((lo[0] + hi[0] + 1) as f32 / 2.0, (lo[2] + hi[2] + 1) as f32 / 2.0);
                self.drop = Some((el, [x, lo[1] as f32, z], [x, f.hi[1] as f32 + 1.0, z]));
            }
        }
        // a clock's output: the circuits' OUT box and tag (vjeux: instead of the orange glass)
        let clock_out = !p.session.design.meta.contains_key("circuit") && !p.session.mode.is_door();
        if clock_out && let Some(o) = p.session.outputs.first() {
            let t = make(&self.layer, "OUT", "#ffb04a", vec![*o]);
            self.tags.push(t);
        }
        // the tags naming an output turn red while it is powered
        for t in self.tags.iter_mut() {
            t.out = t.cells.iter().copied().find(|c| p.session.outputs.contains(c));
        }
        if let Some((c, _)) = self.history.take() {
            c.remove();
        }
        if let Some(t) = self.history_caption.take() {
            t.remove();
        }
        if (!p.session.circuit_inputs.is_empty() || clock_out && p.session.simulated()) && !p.history.is_empty() {
            let c: HtmlCanvasElement = document().create_element("canvas").unwrap().dyn_into().unwrap();
            let rows = p.history.len() as u32;
            c.set_width(HISTORY as u32 * 3);
            c.set_height(rows * 8 + (rows - 1) * 2);
            // every edge set: a page's own rules for the viewer's canvases (`left:0`, a full
            // size) must not move the strip
            let _ = c.set_attribute("style", &format!("position:absolute;left:auto;bottom:auto;right:8px;top:34px;width:{}px;height:{}px;max-width:none;border-radius:3px;background:rgba(0,0,0,0.45);image-rendering:pixelated", HISTORY * 3, rows * 8 + (rows - 1) * 2));
            c.set_title("the output: powered (red) or not, over the last 40 game ticks");
            let _ = self.layer.append_child(&c);
            // what the strip is, beside it: the last 2 s (40 ticks)
            let caption = span(&format!("position:absolute;left:auto;bottom:auto;right:{}px;top:32px;padding:0 4px;border-radius:3px;background:rgba(0,0,0,0.45);color:#e8e8e8;font:500 10px/12px system-ui,sans-serif;white-space:nowrap", 8 + HISTORY * 3 + 4), if rows > 1 { "outputs, last 2 s" } else { "output, last 2 s" });
            let _ = self.layer.append_child(&caption);
            self.history_caption = Some(caption);
            if let Some(ctx) = c.get_context("2d").ok().flatten().and_then(|x| x.dyn_into::<CanvasRenderingContext2d>().ok()) {
                self.history = Some((c, ctx));
            }
        }
    }

    /// A tap at (x, y) CSS px on the view: the container there (a block under the tap: `picked`,
    /// or a cart or chest boat) shows or hides its label. False: no container there.
    pub fn toggle_pin_at(&mut self, x: f64, y: f64, picked: Option<[i32; 3]>) -> bool {
        let hit = self
            .hits
            .iter()
            .filter(|(k, r, _)| x >= r.x0 && x <= r.x1 && y >= r.y0 && y <= r.y1 && matches!(k, Key::Entity(_)) || matches!(k, Key::Block(c) if Some(*c) == picked))
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(k, _, _)| *k);
        let Some(k) = hit else { return false };
        if !self.pinned.remove(&k) {
            self.pinned.insert(k);
        }
        true
    }

    /// An item's icon as a data URL (cached; None: no texture for it), its PNG encoded here
    /// (`labels::png_data_url`: a canvas's `toDataURL` read the GPU back, a 300-450 ms task the
    /// first time a label showed an item).
    fn icon(&mut self, atlas: &Atlas, id: &str) -> Option<String> {
        if let Some(u) = self.icons.get(id) {
            return u.clone();
        }
        let url = labels::item_icon(atlas, id).and_then(|rgba| labels::png_data_url(&rgba, 16, 16));
        if url.is_none()
            && labels::wants_item_atlas(atlas, id)
            && let Some(g) = &atlas.game
        {
            // the item atlas loads (the page's next frame), then the icons are made again
            g.wants_entities.set(true);
        }
        self.icons.insert(id.to_string(), url.clone());
        url
    }

    /// A label's contents: per kind (two at most) its icon and count, then `+n` more kinds; a
    /// judge's chest it keeps full counts its items in stacks, a chest it empties shows the run's
    /// tally ([`Count`]).
    fn fill_label(&mut self, atlas: &Atlas, el: &HtmlElement, entries: &[Entry], count: Count) {
        el.set_inner_html("");
        for e in entries.iter().take(2) {
            let id = match e {
                Entry::Items { id, .. } | Entry::Boxes { id, .. } => *id,
            };
            let icon: HtmlElement = match self.icon(atlas, id) {
                Some(url) => {
                    let img: HtmlElement = document().create_element("img").unwrap().dyn_into().unwrap();
                    let _ = img.set_attribute("src", &url);
                    let _ = img.set_attribute("alt", "");
                    let _ = img.set_attribute("style", "width:12px;height:12px;image-rendering:pixelated;flex:none");
                    img
                }
                None => div(&format!("width:10px;height:10px;border-radius:2px;flex:none;background:{}", item_color(id))),
            };
            let _ = el.append_child(&icon);
            let text = match count {
                Count::Items => labels::entry_text(e),
                Count::Stacks => labels::entry_text_stacks(e),
                Count::Tally => labels::entry_text_tally(e),
            };
            let _ = el.append_child(&span("margin-right:2px", &text));
            // several boxes: how full they are, as a bar (not when all are empty: nothing to show;
            // not on a tally: they are gone)
            if let Entry::Boxes { n, used, slots, .. } = e
                && *n > 1
                && *used > 0
                && count != Count::Tally
            {
                let bar = div("width:14px;height:4px;border-radius:2px;background:rgba(255,255,255,0.25);overflow:hidden;flex:none;margin-right:2px");
                let fill = div(&format!("height:100%;width:{:.0}%;background:#d9a8ff", *used as f64 * 100.0 / (*slots).max(1) as f64));
                let _ = bar.append_child(&fill);
                let _ = el.append_child(&bar);
            }
        }
        if entries.len() > 2 {
            let _ = el.append_child(&span("color:#ddd", &format!("+{}", entries.len() - 2)));
        }
        let title: Vec<String> = entries.iter().map(labels::entry_title).collect();
        let _ = el.set_attribute("aria-label", &title.join(", "));
    }

    /// Place everything for the player's camera (`w` x `h` CSS px view) and its current tick.
    pub fn update(&mut self, p: &Player, atlas: &Atlas, w: f64, h: f64) {
        let aspect = (w / h.max(1.0)) as f32;
        let (f, r, u) = p.camera.basis();
        let eye = p.camera.eye();
        let t = (p.camera.vfov.to_radians() / 2.0).tan();
        let project = |q: [f32; 3]| -> Option<(f64, f64)> {
            let v = [q[0] - eye[0], q[1] - eye[1], q[2] - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f);
            if z <= 0.05 {
                return None;
            }
            let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
            if x.abs() > 1.1 || y.abs() > 1.1 {
                return None;
            }
            Some(((x as f64 + 1.0) / 2.0 * w, (1.0 - y as f64) / 2.0 * h))
        };
        // a cell's box on the view (None: not wholly in view)
        let cell_box = |c: [i32; 3]| -> Option<Rect> {
            let mut b = Rect { x0: f64::MAX, y0: f64::MAX, x1: f64::MIN, y1: f64::MIN };
            for k in 0..8 {
                let (x, y) = project([c[0] as f32 + (k & 1) as f32, c[1] as f32 + ((k >> 1) & 1) as f32, c[2] as f32 + ((k >> 2) & 1) as f32])?;
                b = Rect { x0: b.x0.min(x), y0: b.y0.min(y), x1: b.x1.max(x), y1: b.y1.max(y) };
            }
            Some(b)
        };
        let mut layout = Layout::default();
        // the viewer's own controls over the view (the Record button) stay clear
        let base = self.layer.get_bounding_client_rect();
        for e in &self.keep_clear {
            if e.offset_width() > 0 && e.style().get_property_value("visibility").ok().as_deref() != Some("hidden") {
                let b = e.get_bounding_client_rect();
                layout.obstacles.push(Rect { x0: b.left() - base.left() - 4.0, y0: b.top() - base.top() - 4.0, x1: b.right() - base.left() + 4.0, y1: b.bottom() - base.top() + 4.0 });
            }
        }
        // the history strip and its caption stay clear
        if let Some((c, _)) = &self.history {
            let (cw, ch) = (c.offset_width() as f64, c.offset_height() as f64);
            let tw = self.history_caption.as_ref().map(|t| t.offset_width() as f64 + 4.0).unwrap_or(0.0);
            layout.obstacles.push(Rect { x0: w - 8.0 - cw - tw, y0: self.strip_top - 2.0, x1: w - 8.0, y1: self.strip_top + ch.max(12.0) });
        }
        // what the containers hold (nearest first)
        let game = game_of(atlas);
        if game != self.icons_of {
            self.icons.clear();
            self.icons_of = game;
        }
        let perf = crate::window().performance();
        let now_ms = || perf.as_ref().map(|q| q.now()).unwrap_or(0.0);
        let t_a = now_ms();
        let mut now = labels::summaries(&p.session, p.partial);
        let t_b = now_ms();
        let dist = |a: [f32; 3]| (a[0] - eye[0]).powi(2) + (a[1] - eye[1]).powi(2) + (a[2] - eye[2]).powi(2);
        now.sort_by(|a, b| dist(a.anchor).total_cmp(&dist(b.anchor)).then(a.key.cmp(&b.key)));
        // the tags first, always shown (at their point when no free spot is left); the
        // containers under a tag (port chests) show their contents in the tag; no tag covers an
        // input's cell (a tap there presses that input, not the tag's)
        // tags stay 4 px inside the view (critique: a long tag ran into the right edge)
        let view = Rect { x0: 4.0, y0: 4.0, x1: w - 4.0, y1: h - 4.0 };
        for ci in &p.session.circuit_inputs {
            if let Some((x, y)) = project([ci.pos[0] as f32 + 0.5, ci.pos[1] as f32 + 0.5, ci.pos[2] as f32 + 0.5]) {
                layout.obstacles.push(Rect { x0: x - 10.0, y0: y - 10.0, x1: x + 10.0, y1: y + 10.0 });
            }
        }
        // the still block right above each output tag's top cell (a tag never covers it): asked
        // of the session cell by cell (`Session::drawn_block_at`, what `render_blocks` draws
        // there: test `session::tests::drawn_block_at_is_what_render_blocks_draws`), the whole set
        // of still cells read only when it cannot tell (a moving block, a view of the design's
        // text), again when the blocks change (it was read every frame of a farm in its terrain
        // whose blocks change, its landscape's 50,000 cells: 2-3 ms of a frame in WebKit)
        let blocks_key = (p.session.blocks_version, if p.session.world.log_writes { 0 } else { p.session.tick });
        let mut above_solid: Vec<bool> = vec![false; self.tags.len()];
        let off = p.session.view_offset();
        for (i, tg) in self.tags.iter().enumerate() {
            let Some(top) = tg.cells.iter().max_by_key(|c| c[1]).copied().filter(|_| tg.out.is_some()) else { continue };
            let c = [top[0], top[1] + 1, top[2]];
            if tg.cells.contains(&c) {
                continue;
            }
            above_solid[i] = match p.session.drawn_block_at([c[0] + off[0], c[1] + off[1], c[2] + off[2]]) {
                Ok(b) => b.is_some(),
                Err(()) => {
                    if self.solid_cells.as_ref().is_none_or(|(k, _)| *k != blocks_key) {
                        let cells = p.session.render_blocks(0.0).into_iter().filter(|b| b.offset == [0.0; 3]).map(|b| b.cell).collect();
                        self.solid_cells = Some((blocks_key, std::rc::Rc::new(cells)));
                    }
                    self.solid_cells.as_ref().is_some_and(|(_, s)| s.contains(&c))
                }
            };
        }
        // every tag's point: no other tag covers it, or that tag's leader line could not reach
        // it (QA at phone size, lock2-alt: CODE 1 laid over OUT's point left OUT no free spot,
        // and OUT lay on CODE 1)
        let anchors: Vec<Option<Rect>> = self.tags.iter().map(|tg| if tg.face && !self.show_all { None } else { project(tg.point).map(|(x, y)| Rect { x0: x - 5.0, y0: y - 5.0, x1: x + 5.0, y1: y + 5.0 }) }).collect();
        let mut in_tags: Vec<Key> = Vec::new();
        for i in 0..self.tags.len() {
            // a display face's lamp tags: only with the Labels button on
            if self.tags[i].face && !self.show_all {
                hide(&self.tags[i].el);
                continue;
            }
            let held: Vec<&labels::Summary> = now.iter().filter(|s| matches!(s.key, Key::Block(c) if self.tags[i].cells.contains(&c))).collect();
            in_tags.extend(held.iter().map(|s| s.key));
            // a chest the judge empties: everything that reached it (the run's tally and what it
            // holds now), in items; a chest it keeps full: how full, in stacks
            let tally = self.tags[i].role == Some("emptied");
            let got: Vec<&[Entry]> = if tally { self.tags[i].cells.iter().filter_map(|c| p.session.received.get(c).map(|v| v.as_slice())).collect() } else { Vec::new() };
            let entries = labels::merge(held.iter().map(|s| s.entries.as_slice()).chain(got.iter().copied()));
            // an item frame input: its turn after its name ("DIAL 1 · 3")
            let turn = self.tags[i].frame.and_then(|fi| p.session.frame_turn(fi)).map(|r| format!("· {r}"));
            let content = if let Some(t) = &turn {
                t.clone()
            } else if entries.is_empty() {
                String::new()
            } else {
                format!("{entries:?}{:?}", self.icons_of)
            };
            if self.tags[i].content != content || self.tags[i].size.0 <= 0.0 {
                let contents = self.tags[i].contents.clone();
                if let Some(t) = &turn {
                    contents.set_text_content(Some(t));
                    let _ = contents.style().set_property("display", "inline-flex");
                } else if entries.is_empty() {
                    contents.set_inner_html("");
                    let _ = contents.style().set_property("display", "none");
                } else {
                    self.fill_label(atlas, &contents, &entries, if tally { Count::Tally } else { Count::Stacks });
                    let _ = contents.style().set_property("display", "inline-flex");
                }
                let tg = &mut self.tags[i];
                if tally && let Some(n) = &tg.note {
                    n.set_text_content(Some(if entries.is_empty() { "emptied" } else { "received" }));
                    if !entries.is_empty() {
                        let what: Vec<String> = entries.iter().map(labels::entry_title).collect();
                        let _ = tg.el.set_attribute("title", &format!("{} received so far (the judge empties it as it fills)", what.join(", ")));
                    }
                }
                tg.content = content;
                tg.size = measure(&tg.el);
            }
            let tg = &self.tags[i];
            let Some((ax, ay)) = project(tg.point) else {
                hide(&tg.el);
                continue;
            };
            // an output's tag: red while it is powered
            let lit = tg.out.is_some_and(|o| p.session.output_powered(o));
            if lit != tg.lit {
                let _ = tg.el.style().set_property("color", if lit { "#ff5a4f" } else { &tg.colour });
                self.tags[i].lit = lit;
            }
            let tg = &self.tags[i];
            let (wt, ht) = tg.size;
            // the other tags' points stay clear
            let n0 = layout.obstacles.len();
            layout.obstacles.extend(anchors.iter().enumerate().filter(|(j, _)| *j != i).filter_map(|(_, a)| *a));
            // an output's tag with a block right above its cell (a clock's observer over its OUT
            // lamp): the tag does not cover that block, it goes under the cell (critique: "OUT"
            // read as the observer's); an input's tag stays over its cell, torch or not (under
            // it, a gate's A tag landed on its OUT tag)
            let top = tg.cells.iter().max_by_key(|c| c[1]).copied().filter(|_| tg.out.is_some());
            let above = top.map(|c| [c[0], c[1] + 1, c[2]]).filter(|_| above_solid[i]).and_then(|c| cell_box(c));
            if let Some(b) = above {
                layout.obstacles.push(b);
            }
            // a block over the cell: under the cell instead (rows going down from its bottom face)
            let below = above.and_then(|_| {
                let c = top?;
                let n = tg.cells.len().max(1) as f32;
                let mid = |k: usize| tg.cells.iter().map(|c| c[k] as f32 + 0.5).sum::<f32>() / n;
                let low = tg.cells.iter().map(|c| c[1]).min().unwrap_or(c[1]) as f32 - 0.05;
                let (bx, by) = project([mid(0), low, mid(2)])?;
                layout.place_below(&view, bx, by, wt, ht, 3)
            });
            // no free spot over it within 5 rows: under its cells, then further rows either way,
            // before laying it on its point (critique on lock3-q85: OUT laid over DIAL 2)
            let under = || {
                let n = tg.cells.len().max(1) as f32;
                let mid = |k: usize| tg.cells.iter().map(|c| c[k] as f32 + 0.5).sum::<f32>() / n;
                let low = tg.cells.iter().map(|c| c[1]).min().unwrap_or(0) as f32 - 0.05;
                project([mid(0), low, mid(2)])
            };
            let placed = below
                .or_else(|| layout.place(&view, ax, ay, wt, ht, 5))
                .or_else(|| under().and_then(|(bx, by)| layout.place_below(&view, bx, by, wt, ht, 5)))
                .or_else(|| layout.place(&view, ax, ay, wt, ht, 14))
                .or_else(|| under().and_then(|(bx, by)| layout.place_below(&view, bx, by, wt, ht, 14)));
            // (the block over its cell and the other tags' points: obstacles for this tag only)
            layout.obstacles.truncate(n0);
            let rect = match placed {
                Some((rect, _)) => rect,
                None => {
                    // no free spot: at its point, inside the view (a long tag near the edge ran
                    // off it: critique, "ITEMS A–C kept full 81 item…" on a phone)
                    let x0 = (ax - wt / 2.0).min(w - 2.0 - wt).max(2.0);
                    let y0 = (ay - 2.0 - ht).min(h - 2.0 - ht).max(2.0);
                    let rect = Rect { x0, y0, x1: x0 + wt, y1: y0 + ht };
                    layout.texts.push(rect);
                    rect
                }
            };
            show_at(&tg.el, &rect);
        }
        // the outlined cells: no label covers one
        // (a TNT duper's height over its ground first: its dashed line and words beside it)
        let mut dashed: Vec<((f64, f64), (f64, f64))> = Vec::new();
        if let Some((el, top, bottom)) = &self.drop {
            // the ground may lie below the view (a precise drop standing 23 blocks up, framed down
            // to its burst point): the line runs to the view's bottom (above the bar) and the
            // words still say how high
            let free = |q: [f32; 3]| -> Option<(f64, f64)> {
                let v = [q[0] - eye[0], q[1] - eye[1], q[2] - eye[2]];
                let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
                let z = dot(f);
                (z > 0.05).then(|| (((dot(r) / (z * t * aspect)) as f64 + 1.0) / 2.0 * w, (1.0 - (dot(u) / (z * t)) as f64) / 2.0 * h))
            };
            let floor_y = h - 70.0;
            let b = free(*bottom).map(|b| {
                if b.1 > floor_y {
                    let a = free(*top).unwrap_or(b);
                    let k = if (b.1 - a.1).abs() > 1e-6 { (floor_y - a.1) / (b.1 - a.1) } else { 1.0 };
                    (a.0 + (b.0 - a.0) * k.clamp(0.0, 1.0), floor_y)
                } else {
                    b
                }
            });
            match (project(*top), b) {
                (Some(a), Some(b)) if (b.1 - a.1).abs() > 12.0 => {
                    dashed.push((a, b));
                    let (wt, ht) = measure(el);
                    let (mx, my) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                    let right = Rect { x0: mx + 6.0, y0: my - ht / 2.0, x1: mx + 6.0 + wt, y1: my + ht / 2.0 };
                    let r = if right.x1 > w - 4.0 { Rect { x0: mx - 6.0 - wt, y0: right.y0, x1: mx - 6.0, y1: right.y1 } } else { right };
                    layout.texts.push(r);
                    show_at(el, &r);
                }
                _ => hide(el),
            }
        }
        for c in p.look.outlines.iter().copied().chain(p.look.port_outlines.iter().map(|(c, _)| *c)) {
            if let Some(b) = cell_box(c) {
                layout.obstacles.push(b);
            }
        }
        // no container label covers a tag's point either
        layout.obstacles.extend(anchors.iter().filter_map(|a| *a));
        for (lo, hi) in &p.look.input_spans {
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        if let Some(b) = cell_box([x, y, z]) {
                            layout.obstacles.push(b);
                        }
                    }
                }
            }
        }
        // where each other container is, for taps
        let t_c = now_ms();
        self.hits.clear();
        for s in now.iter().filter(|s| !in_tags.contains(&s.key)) {
            let rect = match s.key {
                Key::Block(c) => cell_box(c),
                Key::Entity(_) => project([s.anchor[0], s.anchor[1] - 0.4, s.anchor[2]]).map(|(x, y)| Rect { x0: x - 16.0, y0: y - 14.0, x1: x + 16.0, y1: y + 14.0 }),
            };
            if let Some(rect) = rect {
                self.hits.push((s.key, rect, dist(s.anchor)));
            }
        }
        // the story's labels (the shulker boxes being filled or emptied), the tapped ones, or
        // all of them; clear of the badges (top) and the bar (bottom)
        let pinned = &self.pinned;
        let show_all = self.show_all;
        now.retain(|s| !in_tags.contains(&s.key) && (show_all || s.shulker_box || s.furnace.is_some() || pinned.contains(&s.key)));
        let keep: Vec<Key> = now.iter().map(|s| s.key).collect();
        self.labels.retain(|k, l| {
            let k = keep.contains(k);
            if !k {
                l.el.remove();
            }
            k
        });
        let safe = Rect { x0: 2.0, y0: 48.0, x1: w - 2.0, y1: h - 62.0 };
        let t_d = now_ms();
        for s in &now {
            let content = format!("{:?}{:?}{:?}", s.entries, s.furnace, self.icons_of);
            if !self.labels.contains_key(&s.key) {
                let el = div("position:absolute;display:none;align-items:center;gap:2px;padding:1px 4px;border-radius:5px;background:rgba(0,0,0,0.68);color:#fff;font:600 10px/14px system-ui,sans-serif;white-space:nowrap;width:max-content;pointer-events:none");
                let _ = self.layer.append_child(&el);
                self.labels.insert(s.key, Label { el, content: String::new(), size: (0.0, 0.0) });
            }
            if self.labels[&s.key].content != content {
                let el = self.labels[&s.key].el.clone();
                self.fill_label(atlas, &el, &s.entries, Count::Items);
                if let Some(c) = s.furnace {
                    furnace_marks(&el, c);
                }
                let size = measure(&el);
                let l = self.labels.get_mut(&s.key).unwrap();
                l.content = content;
                l.size = size;
            }
            let l = &self.labels[&s.key];
            match project(s.anchor).and_then(|(ax, ay)| layout.place(&safe, ax, ay, l.size.0, l.size.1, 6)) {
                Some((rect, _)) => show_at(&l.el, &rect),
                None => hide(&l.el),
            }
        }
        let t_e = now_ms();
        if crate::debug() && t_e - t_a > 30.0 {
            crate::log(&format!("labels: summaries {:.0} ({} containers), tags {:.0}, hits {:.0}, labels {:.0} ({} shown) done in {:.1} ms", t_b - t_a, now.len(), t_c - t_b, t_d - t_c, t_e - t_d, self.labels.len(), t_e - t_a));
        }
        self.draw_leaders(&layout.lines, &dashed, w, h);
        // the outputs' history
        if let Some((c, ctx)) = &self.history {
            let (cw, ch) = (c.width() as f64, c.height() as f64);
            ctx.clear_rect(0.0, 0.0, cw, ch);
            for (row, hist) in p.history.iter().enumerate() {
                let y = row as f64 * 10.0;
                let pad = HISTORY - hist.len().min(HISTORY);
                for (k, on) in hist.iter().enumerate() {
                    ctx.set_fill_style_str(if *on { "#ff5a4f" } else { "#3a3a3a" });
                    ctx.fill_rect(((pad + k) * 3) as f64, y + if *on { 0.0 } else { 5.0 }, 2.0, if *on { 8.0 } else { 3.0 });
                }
            }
        }
    }

    /// A thin line from each moved tag or label down to its cell, with a dot there; `dashed`: a
    /// TNT duper's height over its ground.
    fn draw_leaders(&self, lines: &[((f64, f64), (f64, f64))], dashed: &[((f64, f64), (f64, f64))], w: f64, h: f64) {
        let Some((c, ctx)) = &self.leaders else { return };
        let dpr = crate::window().device_pixel_ratio().clamp(1.0, 3.0);
        let (pw, ph) = ((w * dpr).round().max(1.0) as u32, (h * dpr).round().max(1.0) as u32);
        if c.width() != pw || c.height() != ph {
            c.set_width(pw);
            c.set_height(ph);
        }
        ctx.clear_rect(0.0, 0.0, pw as f64, ph as f64);
        if !dashed.is_empty() {
            ctx.set_line_width(1.5 * dpr);
            ctx.set_stroke_style_str("rgba(255,255,255,0.9)");
            let _ = ctx.set_line_dash(&js_sys::Array::of2(&(4.0 * dpr).into(), &(3.0 * dpr).into()));
            for ((ax, ay), (x, y)) in dashed {
                ctx.begin_path();
                ctx.move_to(ax * dpr, ay * dpr);
                ctx.line_to(x * dpr, y * dpr);
                ctx.stroke();
            }
            let _ = ctx.set_line_dash(&js_sys::Array::new());
        }
        if lines.is_empty() {
            return;
        }
        ctx.set_line_width(dpr);
        ctx.set_stroke_style_str("rgba(255,255,255,0.8)");
        ctx.set_fill_style_str("rgba(255,255,255,0.9)");
        for ((ax, ay), (x, y)) in lines {
            ctx.begin_path();
            ctx.move_to(ax * dpr, ay * dpr);
            ctx.line_to(x * dpr, y * dpr);
            ctx.stroke();
            ctx.begin_path();
            let _ = ctx.arc(ax * dpr, ay * dpr, 1.6 * dpr, 0.0, std::f64::consts::TAU);
            ctx.fill();
        }
    }
}
