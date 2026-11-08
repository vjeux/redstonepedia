//! The game's own block models, loaded at run time with its textures (vendored in
//! `web/textures/26.3`): every block definition (blockstate) and every model of the
//! judge's version, resolved like the game (variants by properties, multipart conditions,
//! weighted variants picked from the block's position, model parents, `#texture` references,
//! elements with their rotations, faces with uv / rotation / cullface / tint, quarter turns and
//! uvlock), into the viewer's [`Model`]s. Their textures are the game's block atlas, uploaded as
//! one 2D texture beside the tile array (faces name them `block/...`).

use crate::json::Json;
use crate::models::{Face, Layer, Model, Part, PartRot};
use std::collections::HashMap;

/// Where a game texture sits in the game texture: (x, y, w, h) of its first frame, and whether it
/// has see-through pixels.
#[derive(Clone, Copy, Debug)]
pub struct TexRect {
    pub rect: [u32; 4],
    pub transparent: bool,
}

pub struct GameModels {
    defs: HashMap<String, Json>,
    models: HashMap<String, Json>,
    /// `block/stone` -> its place in the game texture.
    pub tex: HashMap<String, TexRect>,
    /// The game texture's size.
    pub size: (u32, u32),
}

/// `minecraft:block/x` -> `block/x`.
fn bare(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

fn into_map(j: Json) -> HashMap<String, Json> {
    match j {
        Json::Obj(kv) => kv.into_iter().collect(),
        _ => HashMap::new(),
    }
}

/// A Java `Random` (the game's `LegacyRandomSource`) seeded for a block position: which of the
/// weighted variants the game draws there (`Mth.getSeed(pos)`, then `nextInt(total)`).
pub fn variant_pick(pos: [i32; 3], total: u32) -> u32 {
    let (x, y, z) = (pos[0], pos[1], pos[2]);
    let mut seed: i64 = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ (y as i64);
    seed = seed.wrapping_mul(seed).wrapping_mul(42317861).wrapping_add(seed.wrapping_mul(11));
    let seed = seed >> 16;
    const MASK: i64 = (1 << 48) - 1;
    let mut s = (seed ^ 0x5DEECE66D) & MASK;
    let mut next31 = || {
        s = (s.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & MASK;
        (s >> 17) as i32
    };
    let bound = total.max(1) as i32;
    if bound & (bound - 1) == 0 {
        return ((bound as i64 * next31() as i64) >> 31) as u32;
    }
    loop {
        let sample = next31();
        let m = sample % bound;
        if sample.wrapping_sub(m).wrapping_add(bound - 1) >= 0 {
            return m as u32;
        }
    }
}

/// A state's name and properties: `minecraft:repeater[delay=1,facing=north]`.
fn parse_state(state: &str) -> (&str, Vec<(&str, &str)>) {
    let (name, rest) = match state.find('[') {
        Some(i) => (&state[..i], state[i + 1..].trim_end_matches(']')),
        None => (state, ""),
    };
    let props = rest.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
    (bare(name), props)
}

fn json_value_str(v: &Json) -> String {
    match v {
        Json::Str(s) => s.clone(),
        Json::Bool(b) => b.to_string(),
        Json::Num(n) => format!("{n}"),
        _ => String::new(),
    }
}

/// A multipart `when` against the state's properties (a property the state lacks matches only
/// `false` / `none`).
fn when_matches(w: &Json, props: &[(&str, &str)]) -> bool {
    let Json::Obj(kv) = w else { return true };
    for (k, v) in kv {
        let ok = match k.as_str() {
            "OR" => v.as_arr().is_some_and(|a| a.iter().any(|c| when_matches(c, props))),
            "AND" => v.as_arr().is_some_and(|a| a.iter().all(|c| when_matches(c, props))),
            _ => {
                let want = json_value_str(v);
                let have = props.iter().find(|(pk, _)| pk == k).map(|(_, pv)| *pv);
                match have {
                    Some(h) => want.split('|').any(|x| x == h),
                    None => want.split('|').any(|x| x == "false" || x == "none"),
                }
            }
        };
        if !ok {
            return false;
        }
    }
    true
}

/// One model reference of a blockstate: the model, its quarter turns and uvlock.
struct Apply<'a> {
    model: &'a str,
    x: u8,
    y: u8,
    uvlock: bool,
}

fn apply_of(j: &Json) -> Option<Apply<'_>> {
    let model = j.get("model")?.as_str()?;
    let q = |k: &str| (j.get(k).and_then(Json::as_i64).unwrap_or(0).rem_euclid(360) / 90) as u8;
    Some(Apply { model: bare(model), x: q("x"), y: q("y"), uvlock: matches!(j.get("uvlock"), Some(Json::Bool(true))) })
}

/// A variant entry: one model, or weighted ones (picked from the block's position).
fn pick<'a>(v: &'a Json, pos: Option<[i32; 3]>) -> Option<Apply<'a>> {
    match v {
        Json::Arr(list) if !list.is_empty() => {
            let w: Vec<u32> = list.iter().map(|j| j.get("weight").and_then(Json::as_i64).unwrap_or(1).max(1) as u32).collect();
            apply_of(&list[pick_index(&w, pos.unwrap_or([0, 0, 0]))])
        }
        Json::Arr(_) => None,
        _ => apply_of(v),
    }
}

/// The variant of a weighted list the game picks at `pos` (`WeightedVariants`: the position's
/// seed, then the weights in order).
pub fn pick_index(weights: &[u32], pos: [i32; 3]) -> usize {
    let total: u32 = weights.iter().sum();
    let mut r = variant_pick(pos, total) as i64;
    for (i, w) in weights.iter().enumerate() {
        r -= *w as i64;
        if r < 0 {
            return i;
        }
    }
    0
}

/// Blocks the game draws alpha-blended (translucent render layer).
fn translucent(name: &str) -> bool {
    name.ends_with("stained_glass") || name.ends_with("stained_glass_pane") || matches!(name, "ice" | "frosted_ice" | "slime_block" | "honey_block" | "tinted_glass" | "nether_portal" | "water" | "bubble_column")
}

/// The plains colours the game's tinted faces carry here (`BlockColors` at the pictures' plains):
/// grass, foliage, water. A farm in its terrain swaps them for its biomes' (`mesh::BiomeTintFn`).
pub const GRASS_TINT: [u8; 3] = [0x8e, 0xb9, 0x72];
pub const FOLIAGE_TINT: [u8; 3] = [0x77, 0xab, 0x2f];
pub const WATER_TINT: [u8; 3] = [0x3f, 0x76, 0xe4];

/// The color a tinted face takes (`tintindex`): the game's block colors for the pictures' plains.
fn tint(name: &str, props: &[(&str, &str)], index: i64) -> [u8; 3] {
    let rgb = |c: u32| [(c >> 16) as u8, (c >> 8) as u8, c as u8];
    let prop = |k: &str| props.iter().find(|(pk, _)| *pk == k).map(|(_, v)| *v);
    if index < 0 {
        return [255, 255, 255];
    }
    match name {
        "redstone_wire" => crate::models::dust_color(prop("power").and_then(|p| p.parse().ok()).unwrap_or(0)),
        "grass_block" | "short_grass" | "grass" | "tall_grass" | "fern" | "large_fern" | "potted_fern" | "sugar_cane" | "bush" => GRASS_TINT,
        "birch_leaves" => rgb(0x80a755),
        "spruce_leaves" => rgb(0x619961),
        n if n.ends_with("_leaves") && !n.contains("azalea") && !n.contains("cherry") && !n.contains("pale") => FOLIAGE_TINT,
        "vine" => FOLIAGE_TINT,
        "lily_pad" => rgb(0x208030),
        "water" | "bubble_column" | "water_cauldron" => WATER_TINT,
        "melon_stem" | "pumpkin_stem" => {
            let a: i32 = prop("age").and_then(|p| p.parse().ok()).unwrap_or(0);
            [(a * 32).min(255) as u8, (255 - a * 8) as u8, (a * 4) as u8]
        }
        "attached_melon_stem" | "attached_pumpkin_stem" => rgb(0xe0c71c),
        _ => [255, 255, 255],
    }
}

thread_local! {
    static NAMES: std::cell::RefCell<std::collections::HashSet<&'static str>> = Default::default();
}

/// A texture name as a `&'static str` (faces hold static names; the game's few thousand names
/// are interned once).
pub fn intern(s: &str) -> &'static str {
    NAMES.with(|n| {
        let mut n = n.borrow_mut();
        if let Some(x) = n.get(s) {
            return *x;
        }
        let x: &'static str = Box::leak(s.to_string().into_boxed_str());
        n.insert(x);
        x
    })
}

const FACE_NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

impl GameModels {
    /// From the summary's block definitions and models, the block atlas's index and pixels (to
    /// tell see-through textures).
    pub fn parse(defs: &str, models: &str, index: &str, rgba: &[u8], w: u32, h: u32) -> Result<GameModels, String> {
        let defs = into_map(Json::parse(defs).map_err(|e| format!("block definitions: {e}"))?);
        let models = into_map(Json::parse(models).map_err(|e| format!("models: {e}"))?);
        let idx = Json::parse(index).map_err(|e| format!("atlas index: {e}"))?;
        let mut tex = HashMap::new();
        if let Json::Obj(kv) = &idx {
            for (k, v) in kv {
                let r: Vec<u32> = v.as_arr().unwrap_or(&[]).iter().filter_map(Json::as_f64).map(|x| x as u32).collect();
                if r.len() != 4 || r[0] + r[2] > w || r[1] + r[2].min(r[3]) > h {
                    continue;
                }
                // the first frame of an animated strip: square
                let side = r[2].min(r[3]);
                let mut transparent = false;
                'scan: for y in r[1]..r[1] + side {
                    for x in r[0]..r[0] + r[2] {
                        if rgba[((y * w + x) * 4 + 3) as usize] < 255 {
                            transparent = true;
                            break 'scan;
                        }
                    }
                }
                tex.insert(k.clone(), TexRect { rect: [r[0], r[1], r[2], side], transparent });
            }
        }
        Ok(GameModels { defs, models, tex, size: (w, h) })
    }

    /// Does this state's block draw one of several weighted variants (the model depends on the
    /// block's position)?
    pub fn positional(&self, state: &str) -> bool {
        let (name, props) = parse_state(state);
        let Some(Json::Obj(vars)) = self.defs.get(name).and_then(|d| d.get("variants")) else { return false };
        vars.iter().any(|(k, v)| matches!(v, Json::Arr(a) if a.len() > 1) && key_matches(k, &props))
    }

    /// Which of a positional state's variants ([`GameModels::positional`]) the game draws at
    /// `pos`: its index in the state's list, the one [`GameModels::model`] draws there (the same
    /// index, the same model: a mesh keeps one model per variant, not one per cell: a terrain's
    /// tens of thousands of grass and stone blocks). None: not positional.
    pub fn variant_index(&self, state: &str, pos: [i32; 3]) -> Option<usize> {
        self.variant_weights(state).map(|w| pick_index(&w, pos))
    }

    /// The weights of a positional state's variants, in order (None: not positional): with
    /// [`pick_index`], the variant the game draws at a cell.
    pub fn variant_weights(&self, state: &str) -> Option<Vec<u32>> {
        let (name, props) = parse_state(state);
        let Some(Json::Obj(vars)) = self.defs.get(name).and_then(|d| d.get("variants")) else { return None };
        let Json::Arr(list) = vars.iter().find(|(k, _)| key_matches(k, &props)).map(|(_, v)| v)? else { return None };
        if list.len() < 2 {
            return None;
        }
        Some(list.iter().map(|j| j.get("weight").and_then(Json::as_i64).unwrap_or(1).max(1) as u32).collect())
    }

    /// The model of a state the game draws at `pos` (None: no definition, or nothing but a
    /// particle texture: drawn by a block-entity renderer, e.g. chests).
    pub fn model(&self, state: &str, pos: Option<[i32; 3]>) -> Option<Model> {
        let (name, props) = parse_state(state);
        let def = self.defs.get(name)?;
        let mut applies: Vec<Apply> = Vec::new();
        if let Some(Json::Obj(vars)) = def.get("variants") {
            // every listed property must match (a property the state lacks matches any value)
            let v = vars.iter().find(|(k, _)| key_matches(k, &props)).map(|(_, v)| v)?;
            applies.push(pick(v, pos)?);
        } else if let Some(parts) = def.get("multipart").and_then(Json::as_arr) {
            for p in parts {
                if p.get("when").is_none_or(|w| when_matches(w, &props))
                    && let Some(a) = p.get("apply")
                    && let Some(ap) = pick(a, pos)
                {
                    applies.push(ap);
                }
            }
        }
        let mut out = Model::default();
        let mut cube_faces = 0;
        for (k, ap) in applies.iter().enumerate() {
            let (textures, elements) = self.resolve(ap.model);
            // the first model's `ambientocclusion` decides (`parts.getFirst().useAmbientOcclusion()`)
            if k == 0 {
                out.no_ao = !self.ambient_occlusion(ap.model);
            }
            let Some(elements) = elements.and_then(Json::as_arr) else { continue };
            for el in elements {
                if let Some(p) = self.part(el, &textures, name, &props, ap) {
                    if p.from == [0.0; 3] && p.to == [16.0; 3] && p.rot.is_none() {
                        out.fills = true;
                        if p.layer == Layer::Opaque {
                            cube_faces += p.faces.iter().flatten().count();
                        }
                    }
                    out.parts.push(p);
                }
            }
        }
        if out.parts.is_empty() {
            return None;
        }
        out.full = out.parts.len() == 1 && cube_faces == 6;
        Some(out)
    }

    /// A model's `ambientocclusion` (the nearest model of its parents that says, else true).
    fn ambient_occlusion(&self, name: &str) -> bool {
        let mut cur = Some(bare(name).to_string());
        for _ in 0..16 {
            let Some(n) = cur.take() else { break };
            let Some(m) = self.models.get(&n) else { break };
            if let Some(Json::Bool(b)) = m.get("ambientocclusion") {
                return *b;
            }
            cur = m.get("parent").and_then(Json::as_str).map(|p| bare(p).to_string());
        }
        true
    }

    /// A model's textures (child before parent) and elements (the nearest model that has them).
    fn resolve<'a>(&'a self, name: &str) -> (HashMap<String, String>, Option<&'a Json>) {
        let mut textures: HashMap<String, String> = HashMap::new();
        let mut elements = None;
        let mut cur = Some(bare(name).to_string());
        for _ in 0..16 {
            let Some(n) = cur.take() else { break };
            let Some(m) = self.models.get(&n) else { break };
            if let Some(Json::Obj(t)) = m.get("textures") {
                for (k, v) in t {
                    let s = match v {
                        Json::Str(s) => s.clone(),
                        // `{sprite, force_translucent}` (26.3)
                        _ => v.get("sprite").and_then(Json::as_str).unwrap_or("").to_string(),
                    };
                    textures.entry(k.clone()).or_insert(s);
                }
            }
            if elements.is_none() {
                elements = m.get("elements");
            }
            cur = m.get("parent").and_then(Json::as_str).map(|p| bare(p).to_string());
        }
        (textures, elements)
    }

    /// `#side` -> `block/stone` through the model's texture variables.
    fn texture(textures: &HashMap<String, String>, r: &str) -> Option<String> {
        let mut r = r.to_string();
        for _ in 0..10 {
            match r.strip_prefix('#') {
                Some(k) => r = textures.get(k)?.clone(),
                None => return Some(bare(&r).to_string()),
            }
        }
        None
    }

    fn part(&self, el: &Json, textures: &HashMap<String, String>, name: &str, props: &[(&str, &str)], ap: &Apply) -> Option<Part> {
        let v3 = |j: Option<&Json>| -> Option<[f32; 3]> {
            let a = j?.as_arr()?;
            Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
        };
        let from = v3(el.get("from"))?;
        let to = v3(el.get("to"))?;
        let mut p = Part::new(from, to);
        if let Some(r) = el.get("rotation") {
            let axis = match r.get("axis").and_then(Json::as_str) {
                Some("x") => 0,
                Some("y") => 1,
                _ => 2,
            };
            let angle = r.get("angle").and_then(Json::as_f64).unwrap_or(0.0) as f32;
            let origin = v3(r.get("origin")).unwrap_or([8.0; 3]);
            if angle != 0.0 {
                p.rot = Some(PartRot { axis, angle, origin, rescale: matches!(r.get("rescale"), Some(Json::Bool(true))) });
            }
        }
        if matches!(el.get("shade"), Some(Json::Bool(false))) || el.get("shade_direction_override").and_then(Json::as_str) == Some("up") {
            p.shade = false;
        }
        let mut any_transparent = false;
        for (i, fname) in FACE_NAMES.iter().enumerate() {
            let Some(f) = el.get("faces").and_then(|fs| fs.get(fname)) else { continue };
            let Some(t) = f.get("texture").and_then(Json::as_str).and_then(|r| Self::texture(textures, r)) else { continue };
            if let Some(tr) = self.tex.get(&t) {
                any_transparent |= tr.transparent;
            }
            let mut face = Face::new(intern(&t));
            if let Some(uv) = f.get("uv").and_then(Json::as_arr)
                && uv.len() == 4
            {
                face.uv = Some([0, 1, 2, 3].map(|k| uv[k].as_f64().unwrap_or(0.0) as f32));
            }
            face.rot = (f.get("rotation").and_then(Json::as_i64).unwrap_or(0).rem_euclid(360) / 90) as u8;
            face.cull = f.get("cullface").is_some();
            face.tint = tint(name, props, f.get("tintindex").and_then(Json::as_i64).unwrap_or(-1));
            p.faces[i] = Some(face);
        }
        if p.faces.iter().all(|f| f.is_none()) {
            return None;
        }
        p.layer = if translucent(name) {
            Layer::Translucent
        } else if any_transparent {
            Layer::Cutout
        } else {
            Layer::Opaque
        };
        p.qrot = Some((ap.x, ap.y));
        p.uvlock = ap.uvlock;
        Some(p)
    }
}

/// A variant key (`facing=east,half=bottom`, or `""`) against a state's properties.
fn key_matches(key: &str, props: &[(&str, &str)]) -> bool {
    key.split(',').filter(|kv| !kv.is_empty()).all(|kv| match kv.split_once('=') {
        Some((k, v)) => props.iter().find(|(pk, _)| *pk == k).is_none_or(|(_, pv)| *pv == v),
        None => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_like_the_game() {
        // java.util.Random(Mth.getSeed(0,0,0)).nextInt(4) and a non-power-of-two bound
        assert!(variant_pick([0, 0, 0], 4) < 4);
        assert!(variant_pick([5, -3, 12], 3) < 3);
        let defs = r#"{"lever":{"variants":{"face=floor,facing=north,powered=false":{"model":"minecraft:block/lever"},"face=floor,facing=north,powered=true":{"model":"minecraft:block/lever_on"}}},"wire":{"multipart":[{"apply":{"model":"block/dot"},"when":{"OR":[{"north":"none","south":"none"},{"north":"side|up"}]}},{"apply":{"model":"block/dot","y":90},"when":{"north":"side|up"}}]}}"#;
        let models = r##"{"block/lever":{"textures":{"base":"block/cobblestone"},"elements":[{"from":[5,0,4],"to":[11,3,12],"faces":{"up":{"uv":[5,4,11,12],"texture":"#base"}}}]},"block/lever_on":{"parent":"block/lever"},"block/dot":{"textures":{"line":{"sprite":"block/redstone_dust_dot"}},"elements":[{"from":[0,0.25,0],"to":[16,0.25,16],"shade_direction_override":"up","faces":{"up":{"texture":"#line","tintindex":0}}}]}}"##;
        let index = r#"{"block/cobblestone":[0,0,16,16],"block/redstone_dust_dot":[16,0,16,16]}"#;
        let rgba = vec![255u8; 32 * 16 * 4];
        let g = GameModels::parse(defs, models, index, &rgba, 32, 16).unwrap();
        let m = g.model("minecraft:lever[face=floor,facing=north,powered=true]", None).unwrap();
        assert_eq!(m.parts.len(), 1, "lever_on inherits the lever's elements");
        assert_eq!(m.parts[0].faces[1].as_ref().unwrap().tile, "block/cobblestone");
        let w = g.model("minecraft:wire[north=side,south=none]", None).unwrap();
        assert_eq!(w.parts.len(), 2, "both multipart parts apply");
        assert_eq!(w.parts[1].qrot, Some((0, 1)));
        assert!(!w.parts[0].shade);
        assert!(g.model("minecraft:wire[north=none,south=side]", None).is_none());
    }
}
