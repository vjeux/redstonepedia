//! What a container's label says (the live viewer's container overlay): its contents summed by
//! item kind, its shulker boxes with how many of their slots are filled, and where the label
//! points (the top of the block, or of the cart or chest boat). One short label per container.

use crate::atlas::Atlas;
use crate::session::Session;
use redsim::redstone::container::{Stack, item_name, payload_slots};
use redsim::World;

/// A container: a block's cell, or a container entity's id (a hopper minecart, a chest boat).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Block([i32; 3]),
    Entity(u32),
}

/// One line of a label.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// `count` items of `id` in all, in `stacks` stacks (slots).
    Items { id: &'static str, count: u32, stacks: u32 },
    /// `n` shulker boxes (`id`: the first one's item id, for its colour), `used` of their `slots`
    /// slots holding something.
    Boxes { id: &'static str, n: u32, used: u32, slots: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub key: Key,
    /// Where the label points: the top centre of the block, the top of the entity.
    pub anchor: [f32; 3],
    /// Shulker boxes first, then items by count (largest first), then by name. Empty only for a
    /// furnace burning with nothing in it.
    pub entries: Vec<Entry>,
    /// A placed shulker box (the box being filled or emptied: labelled by default).
    pub shulker_box: bool,
    /// A furnace: its fire and its arrow as the game's furnace screen draws them (labelled by
    /// default: a smelter's story).
    pub furnace: Option<Cooking>,
}

/// A furnace's progress as its screen shows it (`AbstractFurnaceMenu`): the flame, the fuel's burn
/// time left (`litTimeRemaining / litTotalTime`), and the arrow, the item's cooking progress
/// (`cookingTimer / cookingTotalTime`), each 0..1, in the screen's steps (the flame 14 px high,
/// the arrow 24 px wide: `getLitProgress`, `getBurnProgress`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cooking {
    /// Flame pixels lit, 0..=14 (0: out).
    pub flame: u8,
    /// Arrow pixels filled, 0..=24.
    pub arrow: u8,
}

impl Cooking {
    /// The screen's numbers for a furnace block entity.
    pub fn of(c: &redsim::redstone::container::ContainerBe) -> Cooking {
        // AbstractFurnaceMenu.getLitProgress (litTotalTime 0 counts as 200) and getBurnProgress (0
        // without a total or a timer); AbstractFurnaceScreen draws ceil(lit * 13) + 1 of the
        // flame's 14 px while lit and ceil(burn * 24) of the arrow's 24 px
        let total = if c.lit_total == 0 { 200 } else { c.lit_total };
        let flame = if c.lit_time > 0 { ((c.lit_time as f32 / total as f32).clamp(0.0, 1.0) * 13.0).ceil() as u8 + 1 } else { 0 };
        let arrow = if c.cook_total > 0 && c.cook_timer > 0 { ((c.cook_timer as f32 / c.cook_total as f32).clamp(0.0, 1.0) * 24.0).ceil() as u8 } else { 0 };
        Cooking { flame: flame.min(14), arrow: arrow.min(24) }
    }

    /// Anything to draw (a fire, an arrow)?
    pub fn shows(self) -> bool {
        self.flame > 0 || self.arrow > 0
    }
}

/// Several containers' entries as one (a group of same-kind port chests): items summed by kind,
/// boxes summed.
pub fn merge<'a>(lists: impl IntoIterator<Item = &'a [Entry]>) -> Vec<Entry> {
    let mut items: Vec<(&'static str, u32, u32)> = Vec::new();
    let mut boxes: Option<(&'static str, u32, u32, u32)> = None;
    for list in lists {
        for e in list {
            match e {
                Entry::Items { id, count, stacks } => match items.iter_mut().find(|(i, _, _)| i == id) {
                    Some(x) => {
                        x.1 += count;
                        x.2 += stacks;
                    }
                    None => items.push((id, *count, *stacks)),
                },
                Entry::Boxes { id, n, used, slots } => {
                    let b = boxes.get_or_insert((id, 0, 0, 0));
                    b.1 += n;
                    b.2 += used;
                    b.3 += slots;
                }
            }
        }
    }
    items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let mut v = Vec::new();
    if let Some((id, n, used, slots)) = boxes {
        v.push(Entry::Boxes { id, n, used, slots });
    }
    v.extend(items.into_iter().map(|(id, count, stacks)| Entry::Items { id, count, stacks }));
    v
}

/// A container's slots summed by kind (shulker boxes first, then items by count).
pub fn summarize(w: &World, slots: &[Option<Stack>]) -> Vec<Entry> {
    let mut items: Vec<(&'static str, u32, u32)> = Vec::new();
    let mut boxes: Option<(&'static str, u32, u32, u32)> = None;
    for s in slots.iter().flatten() {
        let id = item_name(s.item);
        if id.ends_with("shulker_box") {
            let used = payload_slots(w, s.payload).iter().filter(|x| x.is_some()).count() as u32;
            let b = boxes.get_or_insert((id, 0, 0, 0));
            b.1 += s.count as u32;
            b.2 += used * s.count as u32;
            b.3 += 27 * s.count as u32;
        } else {
            match items.iter_mut().find(|(i, _, _)| *i == id) {
                Some(e) => {
                    e.1 += s.count as u32;
                    e.2 += 1;
                }
                None => items.push((id, s.count as u32, 1)),
            }
        }
    }
    items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let mut v = Vec::new();
    if let Some((id, n, used, slots)) = boxes {
        v.push(Entry::Boxes { id, n, used, slots });
    }
    v.extend(items.into_iter().map(|(id, count, stacks)| Entry::Items { id, count, stacks }));
    v
}

/// Does the session have containers at all (blocks or entities, empty ones included)?
pub fn has_containers(s: &Session) -> bool {
    s.simulated() && s.replay.is_none() && (!s.world.containers.map.is_empty() || !s.world.containers.entities.is_empty())
}

/// Every container holding something, with its label, at `partial` (0..1) through the tick
/// (entities interpolated like they are drawn). Empty containers have no label, but a furnace
/// with a fire or an arrow to show.
pub fn summaries(s: &Session, partial: f32) -> Vec<Summary> {
    if !s.simulated() || s.replay.is_some() {
        return Vec::new();
    }
    let w = &s.world;
    let mut v: Vec<Summary> = w
        .containers
        .map
        .iter()
        .filter_map(|(p, c)| {
            let entries = summarize(w, &c.slots);
            let shulker_box = matches!(w.get(*p), redsim::Block::ShulkerBox { .. });
            let furnace = c.furnace.then(|| Cooking::of(c));
            (!entries.is_empty() || furnace.is_some_and(Cooking::shows)).then(|| Summary { key: Key::Block(p.arr()), anchor: [p.x as f32 + 0.5, p.y as f32 + 1.0, p.z as f32 + 0.5], entries, shulker_box, furnace })
        })
        .collect();
    let lerp = |a: f64, b: f64| (a + (b - a) * partial as f64) as f32;
    for (id, c) in &w.containers.entities {
        let Some(e) = w.entities.list.iter().find(|e| e.id == *id && !e.removed) else { continue };
        let entries = summarize(w, &c.slots);
        if entries.is_empty() {
            continue;
        }
        let height = e.kind.dims().1;
        v.push(Summary { key: Key::Entity(*id), anchor: [lerp(e.xo, e.pos.x), lerp(e.yo, e.pos.y) + height, lerp(e.zo, e.pos.z)], entries, shulker_box: false, furnace: None });
    }
    v.sort_by_key(|s| s.key);
    v
}

/// `1344` -> `1,344`.
pub fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// What the label of one entry says next to its icon: `1,344 items`; one box `12/27 slots`;
/// several `3 boxes`.
pub fn entry_text(e: &Entry) -> String {
    match e {
        // with its unit (critique: "×27" read as stacks beside a hopper's "×9" items)
        Entry::Items { count: 1, .. } => "1 item".into(),
        Entry::Items { count, .. } => format!("{} items", thousands(*count)),
        Entry::Boxes { n: 1, used, slots, .. } => format!("{used}/{slots} slots"),
        Entry::Boxes { n, .. } => format!("{n} boxes"),
    }
}

/// [`entry_text`] for a run's tally (what reached a chest the judge empties): items, and boxes
/// counted as boxes (`1 box`, not one box's `27/27 slots`).
pub fn entry_text_tally(e: &Entry) -> String {
    match e {
        Entry::Boxes { n: 1, .. } => "1 box".into(),
        Entry::Boxes { n, .. } => format!("{} boxes", thousands(*n)),
        _ => entry_text(e),
    }
}

/// [`entry_text`] for a judge's port chest: its items in stacks (`27 stacks`: how full the chest
/// is, which is what the judge keeps; critique: a loader's ITEMS chest "should be 27 stacks").
pub fn entry_text_stacks(e: &Entry) -> String {
    match e {
        Entry::Items { stacks: 1, .. } => "1 stack".into(),
        Entry::Items { stacks, .. } => format!("{} stacks", thousands(*stacks)),
        _ => entry_text(e),
    }
}

/// An entry's spoken form (the label's tooltip): `1,344 cobblestone in 21 stacks`, `3 shulker
/// boxes, 81 of 81 slots filled`.
pub fn entry_title(e: &Entry) -> String {
    match e {
        Entry::Items { id, count, stacks } => format!("{} {} in {} stack{}", thousands(*count), id.replace('_', " "), thousands(*stacks), if *stacks == 1 { "" } else { "s" }),
        Entry::Boxes { n, used, slots, .. } => format!("{n} shulker box{}, {used} of {slots} slots filled", if *n == 1 { "" } else { "es" }),
    }
}

/// An item's 16 x 16 RGBA icon from the game's textures: its item sprite (once the item atlas is
/// loaded), else its block's texture (a block item: cobblestone, a shulker box's colour).
/// None: no texture for it (the overlay draws a coloured square).
pub fn item_icon(atlas: &Atlas, id: &str) -> Option<Vec<u8>> {
    let g = atlas.game.as_ref()?;
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    let names = [format!("item/{id}"), format!("block/{id}"), format!("block/{id}_front"), format!("block/{id}_side"), format!("block/{id}_top")];
    let r = names.iter().find_map(|n| g.tex.get(n))?.rect;
    let side = r[2].min(r[3]).max(1);
    let mut out = vec![0u8; 16 * 16 * 4];
    for y in 0..16u32 {
        for x in 0..16u32 {
            let (sx, sy) = (r[0] + x * side / 16, r[1] + y * side / 16);
            let i = ((sy * g.width + sx) * 4) as usize;
            let o = ((y * 16 + x) * 4) as usize;
            if let Some(px) = g.mips[0].get(i..i + 4) {
                out[o..o + 4].copy_from_slice(px);
            }
        }
    }
    Some(out)
}

/// An image as a `data:image/png;base64,` URL, encoded here (an item icon's 16x16 pixels: a
/// canvas's `toDataURL` read the GPU back, 300-450 ms the first time in the browser, one long
/// task the first time a label showed an item; the coordinator 2026-10-22).
pub fn png_data_url(rgba: &[u8], w: u32, h: u32) -> Option<String> {
    let mut bytes: Vec<u8> = Vec::new();
    {
        let mut e = png::Encoder::new(&mut bytes, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().ok()?;
        wr.write_image_data(rgba).ok()?;
    }
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(22 + bytes.len().div_ceil(3) * 4);
    out.push_str("data:image/png;base64,");
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    Some(out)
}

/// Does `id` wait for the item atlas (not loaded yet, and no block texture stands in for it)?
pub fn wants_item_atlas(atlas: &Atlas, id: &str) -> bool {
    let Some(g) = atlas.game.as_ref() else { return false };
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    !g.entities && ![format!("block/{id}"), format!("block/{id}_front"), format!("block/{id}_side"), format!("block/{id}_top")].iter().any(|n| g.tex.contains_key(n))
}

/// A box on the view, CSS px (the overlay's layout).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
    pub fn inside(&self, o: &Rect) -> bool {
        self.x0 >= o.x0 && self.x1 <= o.x1 && self.y0 >= o.y0 && self.y1 <= o.y1
    }
}

/// Where a `w` x `h` box hanging above the screen point (ax, ay) goes: right above it if free,
/// else higher up, else beside (the cheapest free spot: a row up costs 1, a column aside 2.5),
/// never over `placed`, within `safe`. The flag says it moved (a leader line ties it back).
pub fn spot(placed: &[Rect], safe: &Rect, ax: f64, ay: f64, w: f64, h: f64, rows: usize) -> Option<(Rect, bool)> {
    let mut l = Layout { obstacles: placed.to_vec(), ..Default::default() };
    l.place(safe, ax, ay, w, h, rows)
}

/// Where a leader line from the cell's point (ax, ay) meets a moved box: the nearest point of
/// its edge facing the point.
pub fn leader_end(ax: f64, ay: f64, r: &Rect) -> (f64, f64) {
    let x = ax.clamp(r.x0 + 3.0, (r.x1 - 3.0).max(r.x0 + 3.0));
    let y = if ay >= r.y1 { r.y1 } else if ay <= r.y0 { r.y0 } else { ay };
    (x, y)
}

/// Does the segment a-b cross the box (its inside, 1 px in from the edges)?
pub fn seg_hits(a: (f64, f64), b: (f64, f64), r: &Rect) -> bool {
    let (x0, y0, x1, y1) = (r.x0 + 1.0, r.y0 + 1.0, r.x1 - 1.0, r.y1 - 1.0);
    if x0 >= x1 || y0 >= y1 {
        return false;
    }
    // Liang-Barsky
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dy, a.1 - y0), (dy, y1 - a.1)] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// The overlay's layout of one frame: what is placed so far (boxes nothing may cover: outlined
/// cells, the history strip; the tags and labels; their leader lines).
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub obstacles: Vec<Rect>,
    pub texts: Vec<Rect>,
    pub lines: Vec<((f64, f64), (f64, f64))>,
}

impl Layout {
    /// A free spot for a `w` x `h` box hanging above (ax, ay) (see [`spot`]): it covers no
    /// obstacle, tag or label, no leader line crosses it, and its own leader line (when it moved)
    /// crosses no tag or label. Placed and recorded; None when nothing fits.
    pub fn place(&mut self, safe: &Rect, ax: f64, ay: f64, w: f64, h: f64, rows: usize) -> Option<(Rect, bool)> {
        const GAP: f64 = 2.0;
        let mut cands: Vec<(f64, usize, i32)> = Vec::new();
        for k in 0..rows {
            for c in [-1i32, 0, 1] {
                cands.push((k as f64 + 2.5 * c.abs() as f64, k, c));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        for (_, k, c) in cands {
            let y1 = ay - GAP - k as f64 * (h + GAP);
            // centred over its point, or a whole width aside; centred but past the view's edge:
            // slid back inside (a long tag near the edge)
            let centred = ax - w / 2.0;
            let x0 = if c == 0 && safe.x1 - safe.x0 >= w { centred.min(safe.x1 - w).max(safe.x0) } else { centred + c as f64 * (w + 4.0) };
            let r = Rect { x0, y0: y1 - h, x1: x0 + w, y1 };
            if !r.inside(safe) || self.obstacles.iter().chain(self.texts.iter()).any(|p| p.overlaps(&r)) || self.lines.iter().any(|(a, b)| seg_hits(*a, *b, &r)) {
                continue;
            }
            let moved = k > 0 || c != 0 || (x0 - centred).abs() > w / 4.0;
            if moved {
                let end = leader_end(ax, ay, &r);
                if self.texts.iter().any(|t| seg_hits((ax, ay), end, t)) {
                    continue;
                }
                self.lines.push(((ax, ay), end));
            }
            self.texts.push(r);
            return Some((r, moved));
        }
        None
    }

    /// [`Layout::place`] under its point instead (a tag whose cell has a block right above it:
    /// over the cell it would read as that block's), rows going down.
    pub fn place_below(&mut self, safe: &Rect, ax: f64, ay: f64, w: f64, h: f64, rows: usize) -> Option<(Rect, bool)> {
        const GAP: f64 = 2.0;
        let mut cands: Vec<(f64, usize, i32)> = Vec::new();
        for k in 0..rows {
            for c in [-1i32, 0, 1] {
                cands.push((k as f64 + 2.5 * c.abs() as f64, k, c));
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        for (_, k, c) in cands {
            let y0 = ay + GAP + k as f64 * (h + GAP);
            let centred = ax - w / 2.0;
            let x0 = if c == 0 && safe.x1 - safe.x0 >= w { centred.min(safe.x1 - w).max(safe.x0) } else { centred + c as f64 * (w + 4.0) };
            let r = Rect { x0, y0, x1: x0 + w, y1: y0 + h };
            if !r.inside(safe) || self.obstacles.iter().chain(self.texts.iter()).any(|p| p.overlaps(&r)) || self.lines.iter().any(|(a, b)| seg_hits(*a, *b, &r)) {
                continue;
            }
            let moved = k > 0 || c != 0 || (x0 - centred).abs() > w / 4.0;
            if moved {
                let end = leader_end(ax, ay, &r);
                if self.texts.iter().any(|t| seg_hits((ax, ay), end, t)) {
                    continue;
                }
                self.lines.push(((ax, ay), end));
            }
            self.texts.push(r);
            return Some((r, moved));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A furnace's fire and arrow in the furnace screen's steps (AbstractFurnaceScreen: ceil(lit /
    /// total * 13) + 1 of 14 px while lit, ceil(cooking / total * 24) of 24 px).
    #[test]
    fn a_furnace_shows_its_screens_fire_and_arrow() {
        let mut c = redsim::redstone::container::ContainerBe {
            slots: vec![None; 3],
            cooldown: -1,
            ticked: 0,
            hopper: false,
            bookshelf: false,
            last_slot: -1,
            crafter: false,
            disabled: 0,
            crafting_ticks: 0,
            jukebox: false,
            song: None,
            song_ticks: 0,
            furnace: true,
            lit_time: 0,
            lit_total: 0,
            speed_mult: 1,
            cook_timer: 0,
            cook_total: 0,
            smelted: 0,
            brewing: false,
            brew_time: 0,
            brew_total: 0,
            fuel_uses: 0,
            fuel_total: 0,
            brew_ingredient: None,
            last_bits: None,
            brews: 0,
        };
        assert_eq!(Cooking::of(&c), Cooking { flame: 0, arrow: 0 }, "out, nothing cooking");
        assert!(!Cooking::of(&c).shows());
        (c.lit_time, c.lit_total) = (1600, 1600);
        assert_eq!(Cooking::of(&c).flame, 14, "a coal just lit: the whole flame");
        c.lit_time = 1;
        assert_eq!(Cooking::of(&c).flame, 2, "its last tick: the flame's first step and the base");
        (c.lit_time, c.lit_total) = (100, 0);
        assert_eq!(Cooking::of(&c).flame, 8, "no total counts as 200: ceil(6.5) + 1");
        (c.cook_timer, c.cook_total) = (100, 200);
        assert_eq!(Cooking::of(&c).arrow, 12);
        c.cook_timer = 1;
        assert_eq!(Cooking::of(&c).arrow, 1, "the arrow's first pixel at the first tick (ceil)");
        c.cook_timer = 200;
        assert_eq!(Cooking::of(&c).arrow, 24);
    }

    #[test]
    fn labels_never_overlap() {
        let safe = Rect { x0: 0.0, y0: 0.0, x1: 390.0, y1: 400.0 };
        let mut placed: Vec<Rect> = Vec::new();
        // ten labels all hanging at the same point: each finds its own spot, none overlap
        for _ in 0..10 {
            if let Some((r, _)) = spot(&placed, &safe, 200.0, 300.0, 50.0, 16.0, 6) {
                assert!(placed.iter().all(|p| !p.overlaps(&r)));
                placed.push(r);
            }
        }
        assert!(placed.len() >= 8, "{}", placed.len());
        // the first one right above its point, the next one stacked right above it
        assert_eq!(placed[0], Rect { x0: 175.0, y0: 282.0, x1: 225.0, y1: 298.0 });
        assert_eq!(placed[1].y1, placed[0].y0 - 2.0);
        assert_eq!(placed[1].x0, placed[0].x0);
        // nothing fits outside the safe area
        assert!(spot(&[], &safe, 200.0, 10.0, 50.0, 16.0, 6).is_none());
    }

    /// A moved label's leader line never crosses another label, and no label covers a line.
    #[test]
    fn leaders_never_cross_labels() {
        let safe = Rect { x0: 0.0, y0: 0.0, x1: 390.0, y1: 400.0 };
        let mut l = Layout::default();
        // a label above (200, 300), then a second one for the same point: it goes up a row and
        // its leader would cross the first one, so it goes aside instead
        let (a, _) = l.place(&safe, 200.0, 300.0, 50.0, 16.0, 6).unwrap();
        let (b, moved) = l.place(&safe, 200.0, 300.0, 50.0, 16.0, 6).unwrap();
        assert!(moved);
        assert!(!seg_hits((200.0, 300.0), leader_end(200.0, 300.0, &b), &a), "{a:?} {b:?}");
        // a third box for a point under the second one's leader is not put across that line
        let (line_a, line_b) = l.lines[0];
        let mid = ((line_a.0 + line_b.0) / 2.0, (line_a.1 + line_b.1) / 2.0);
        if let Some((c, _)) = l.place(&safe, mid.0, mid.1 + 9.0, 30.0, 16.0, 6) {
            assert!(!seg_hits(line_a, line_b, &c), "{c:?} crosses {line_a:?}-{line_b:?}");
        }
        assert!(seg_hits((0.0, 0.0), (10.0, 10.0), &Rect { x0: 2.0, y0: 2.0, x1: 8.0, y1: 8.0 }));
        assert!(!seg_hits((0.0, 0.0), (10.0, 0.0), &Rect { x0: 2.0, y0: 2.0, x1: 8.0, y1: 8.0 }));
    }

    #[test]
    fn groups_merge() {
        let a = [Entry::Items { id: "iron_shovel", count: 27, stacks: 27 }];
        let b = [Entry::Items { id: "iron_shovel", count: 5, stacks: 5 }, Entry::Boxes { id: "shulker_box", n: 1, used: 3, slots: 27 }];
        let m = merge([&a[..], &b[..]]);
        assert_eq!(m, vec![Entry::Boxes { id: "shulker_box", n: 1, used: 3, slots: 27 }, Entry::Items { id: "iron_shovel", count: 32, stacks: 32 }]);
    }

    #[test]
    fn labels_sum_by_kind() {
        let mut w = World::new(redsim::BlockPos::new(0, 0, 0), [1, 1, 1]);
        let id = |name: &str| (0..=255u8).find(|i| item_name(*i) == name).unwrap_or_else(|| panic!("no item {name}"));
        let st = |item: u8, count: u8, payload: u16| Some(Stack { item, count, payload });
        let shovel = id("iron_shovel");
        let full = redsim::redstone::container::new_payload(&mut w, &(0..12).map(|k| (k, Stack { item: shovel, count: 1, payload: 0 })).collect::<Vec<_>>());
        let boxid = (0..=255u8).find(|i| item_name(*i).ends_with("shulker_box")).unwrap();
        let slots = vec![st(shovel, 1, 0), st(shovel, 1, 0), None, st(boxid, 1, full), st(id("redstone"), 64, 0), st(id("redstone"), 3, 0)];
        let e = summarize(&w, &slots);
        assert_eq!(e[0], Entry::Boxes { id: item_name(boxid), n: 1, used: 12, slots: 27 });
        assert_eq!(e[1], Entry::Items { id: "redstone", count: 67, stacks: 2 });
        assert_eq!(e[2], Entry::Items { id: "iron_shovel", count: 2, stacks: 2 });
        assert_eq!(entry_text(&e[0]), "12/27 slots");
        assert_eq!(entry_text(&e[1]), "67 items");
        assert_eq!(entry_text_stacks(&e[1]), "2 stacks");
        assert_eq!(entry_text_tally(&e[1]), "67 items");
        assert_eq!(entry_text_tally(&e[0]), "1 box");
        assert_eq!(entry_text(&Entry::Items { id: "redstone", count: 1, stacks: 1 }), "1 item");
        assert_eq!(entry_text(&Entry::Boxes { id: item_name(boxid), n: 27, used: 0, slots: 729 }), "27 boxes");
        assert_eq!(thousands(1344), "1,344");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000_000), "1,000,000");
        assert!(summarize(&w, &[None, None]).is_empty());
    }
}
