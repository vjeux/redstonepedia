//! The design text format (`crates/mctas/src/design.rs` is the reference parser; this one
//! has no dependencies so it compiles to wasm, and `mctas web check` compares the two on every
//! archived design).
//!
//! ```text
//! # comment
//! name: tiny-demo
//! origin: 0 0 0                  # world position of row 0, column 0 (y adds to the layer y)
//! port lever: 2 1 0              # named positions; words after x y z are the port's args
//! entity: minecraft:minecart 0.5 1.0625 3.5 {snbt}
//! nbt: 1 0 0 {Items:[...]}
//! palette:
//!   S minecraft:stone
//! layer 0:
//!   S.
//! ```
//!
//! Rows run north to south (z grows downward), columns west to east (x grows rightward).

use std::collections::BTreeMap;

pub type Pos = [i32; 3];

/// An entity line: type, feet position (world coordinates), optional SNBT.
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub kind: String,
    pub pos: [f64; 3],
    pub nbt: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Design {
    pub name: String,
    /// Other `key: value` header lines.
    pub meta: BTreeMap<String, String>,
    /// `port <name>: x y z [args]`: position and the words after it (often empty).
    pub ports: BTreeMap<String, (Pos, String)>,
    /// Non-air blocks: position -> block state (`minecraft:` prefixed). Iteration order (x, then
    /// y, then z) is the judges' placement order.
    pub blocks: BTreeMap<Pos, String>,
    pub entities: Vec<Entity>,
    /// `nbt: x y z {snbt}` lines (block-entity data, merged after the raw paste).
    pub block_entities: BTreeMap<Pos, String>,
    /// Header comment lines (`# CLOCK: ...` carries the judge's period and warmup).
    pub comments: Vec<String>,
    /// `stop: x y z <state>` lines (circuits: environment blocks placed before the build, like a
    /// flying machine's stop wall), in file order.
    pub stops: Vec<(Pos, String)>,
    /// A mob farm's Nether part (`nether layer Y:` sections from `nether-origin:`, sharing the
    /// palette, as `mctas::design` reads them), in the file's coordinates: parsed so a design with
    /// one loads (the carousel station's page); the view draws the overworld blocks only.
    pub nether_blocks: BTreeMap<Pos, String>,
}

/// A design's Nether part as a design text of its own (a page's `data-part="nether"` viewer, as
/// `mctas mobfarm nether-part` writes it): its `nether layer Y:` sections as the layers from
/// `nether-origin:`, its `nether nbt:` lines as `nbt:`, its `port nether-<name>:` lines as
/// `port <name>:`; the overworld's layers, entities, ports, data and `circuit:` line dropped (the
/// part does not run alone: it shows as built). Err without a Nether part.
pub fn nether_part_text(text: &str) -> Result<String, String> {
    let mut out = String::new();
    // (the rows that follow a kept header: the palette's and the Nether layers')
    let mut keep = false;
    let mut layers = 0;
    for line in text.lines() {
        if line.starts_with("  ") {
            if keep {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        let l = line.trim();
        keep = false;
        if l.is_empty() || l.starts_with('#') {
            out.push_str(line);
            out.push('\n');
        } else if l == "palette:" {
            keep = true;
            out.push_str("palette:\n");
        } else if let Some(rest) = l.strip_prefix("nether layer ") {
            keep = true;
            layers += 1;
            out.push_str(&format!("layer {rest}\n"));
        } else if let Some(rest) = l.strip_prefix("nether-origin:") {
            out.push_str(&format!("origin:{rest}\n"));
        } else if let Some(rest) = l.strip_prefix("nether nbt:") {
            out.push_str(&format!("nbt:{rest}\n"));
        } else if let Some(rest) = l.strip_prefix("port nether-") {
            out.push_str(&format!("port {rest}\n"));
        } else if let Some(rest) = l.strip_prefix("name:") {
            out.push_str(&format!("name: {} nether part\n", rest.trim()));
        } else if ["layer ", "entity:", "nbt:", "port ", "origin:", "circuit:", "stop:"].iter().any(|p| l.starts_with(p)) {
            // (the overworld's)
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if layers == 0 {
        return Err("no Nether part (`nether layer Y:` sections)".into());
    }
    Ok(out)
}

/// `+x`, `-y`, `east`, `up`, or `dx dy dz` (a unit vector along one axis), like
/// `circuit_fly::machine::parse_travel`.
pub fn parse_travel(s: &str) -> Option<Pos> {
    let v = match s.trim() {
        "+x" | "x" | "east" => [1, 0, 0],
        "-x" | "west" => [-1, 0, 0],
        "+y" | "y" | "up" => [0, 1, 0],
        "-y" | "down" => [0, -1, 0],
        "+z" | "z" | "south" => [0, 0, 1],
        "-z" | "north" => [0, 0, -1],
        t => {
            let w: Vec<i32> = t.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()).map(|w| w.parse::<i32>()).collect::<Result<_, _>>().ok()?;
            if w.len() != 3 {
                return None;
            }
            [w[0], w[1], w[2]]
        }
    };
    (v.iter().map(|c| c.abs()).sum::<i32>() == 1).then_some(v)
}

fn parse_pos(s: &str) -> Result<Pos, String> {
    let v: Vec<i32> = s.split_whitespace().map(|t| t.parse::<i32>().map_err(|e| format!("bad coordinate {t:?}: {e}"))).collect::<Result<_, _>>()?;
    if v.len() != 3 {
        return Err(format!("expected 'x y z', got {s:?}"));
    }
    Ok([v[0], v[1], v[2]])
}

/// `stone` -> `minecraft:stone`; properties kept as written (redsim reads any order).
pub fn normalize_state(s: &str) -> Result<String, String> {
    let s = s.trim();
    let (name, rest) = match s.find('[') {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    };
    if name.is_empty() {
        return Err(format!("empty block name in {s:?}"));
    }
    if !rest.is_empty() && !rest.ends_with(']') {
        return Err(format!("missing ']' in {s:?}"));
    }
    Ok(if name.contains(':') { format!("{name}{rest}") } else { format!("minecraft:{name}{rest}") })
}

fn parse_entity(s: &str) -> Result<Entity, String> {
    let s = s.trim();
    let mut rest = s;
    let mut next = || -> &str {
        let r = rest.trim_start();
        let end = r.find(char::is_whitespace).unwrap_or(r.len());
        let (w, tail) = r.split_at(end);
        rest = tail;
        w
    };
    let kind = next().to_string();
    if kind.is_empty() {
        return Err(format!("expected 'entity: <type> <x> <y> <z> [snbt]', got {s:?}"));
    }
    let mut pos = [0.0f64; 3];
    for p in pos.iter_mut() {
        let w = next();
        *p = w.parse::<f64>().map_err(|_| format!("bad entity coordinate {w:?} in {s:?}"))?;
        if !p.is_finite() {
            return Err(format!("bad entity coordinate {w:?} in {s:?}"));
        }
    }
    let nbt = rest.trim();
    if !nbt.is_empty() && !nbt.starts_with('{') {
        return Err(format!("entity data must be SNBT in braces, got {nbt:?}"));
    }
    let kind = if kind.contains(':') { kind } else { format!("minecraft:{kind}") };
    Ok(Entity { kind, pos, nbt: (!nbt.is_empty()).then(|| nbt.to_string()) })
}

impl Design {
    pub fn parse(text: &str) -> Result<Design, String> {
        enum Section {
            Header,
            Palette,
            Layer(i32),
            NetherLayer(i32),
        }
        let mut d = Design::default();
        let mut origin: Pos = [0, 0, 0];
        let mut nether_origin: Pos = [0, 0, 0];
        let mut palette: BTreeMap<char, String> = BTreeMap::new();
        let mut section = Section::Header;
        let mut row = 0i32;
        for (lineno, raw) in text.lines().enumerate() {
            let err = |m: &str| format!("line {}: {m}: {raw:?}", lineno + 1);
            let line = match raw.find('#') {
                // '#' starts a comment except inside a layer row, where it can be a palette char.
                Some(i) if !matches!(section, Section::Layer(_)) || !raw.starts_with("  ") => {
                    if i == 0 || raw[..i].trim().is_empty() {
                        d.comments.push(raw[i..].to_string());
                    }
                    &raw[..i]
                }
                _ => raw,
            };
            if line.trim().is_empty() {
                continue;
            }
            if !line.starts_with("  ") {
                let l = line.trim();
                if l == "palette:" {
                    section = Section::Palette;
                } else if let Some(y) = l.strip_prefix("layer ").and_then(|r| r.strip_suffix(':')) {
                    section = Section::Layer(y.trim().parse().map_err(|_| err("bad layer"))?);
                    row = 0;
                } else if let Some(y) = l.strip_prefix("nether layer ").and_then(|r| r.strip_suffix(':')) {
                    section = Section::NetherLayer(y.trim().parse().map_err(|_| err("bad nether layer"))?);
                    row = 0;
                } else if l.starts_with("nether nbt:") {
                    // (the Nether part's block-entity data: not drawn)
                    section = Section::Header;
                } else if let Some(rest) = l.strip_prefix("port ") {
                    let (name, pos) = rest.split_once(':').ok_or_else(|| err("bad port"))?;
                    let words: Vec<&str> = pos.split_whitespace().collect();
                    let (xyz, args) = if words.len() > 3 { (words[..3].join(" "), words[3..].join(" ")) } else { (pos.to_string(), String::new()) };
                    let p = parse_pos(&xyz).map_err(|e| err(&e))?;
                    d.ports.insert(name.trim().to_string(), (p, args));
                    section = Section::Header;
                } else if let Some(rest) = l.strip_prefix("entity:") {
                    d.entities.push(parse_entity(rest).map_err(|e| err(&e))?);
                    section = Section::Header;
                } else if let Some(rest) = l.strip_prefix("nbt:") {
                    let rest = rest.trim();
                    let brace = rest.find('{').ok_or_else(|| err("expected 'nbt: <x> <y> <z> {snbt}'"))?;
                    let p = parse_pos(&rest[..brace]).map_err(|e| err(&e))?;
                    let nbt = rest[brace..].trim();
                    if !nbt.ends_with('}') {
                        return Err(err("expected 'nbt: <x> <y> <z> {snbt}'"));
                    }
                    if d.block_entities.insert(p, nbt.to_string()).is_some() {
                        return Err(err("two nbt: lines for one position"));
                    }
                    section = Section::Header;
                } else if let Some((k, v)) = l.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    match k {
                        "name" => d.name = v.to_string(),
                        "origin" => origin = parse_pos(v).map_err(|e| err(&e))?,
                        "nether-origin" => nether_origin = parse_pos(v).map_err(|e| err(&e))?,
                        "stop" => {
                            let words: Vec<&str> = v.split_whitespace().collect();
                            if words.len() != 4 {
                                return Err(err("expected 'stop: x y z <state>'"));
                            }
                            let p = parse_pos(&words[..3].join(" ")).map_err(|e| err(&e))?;
                            d.stops.push((p, normalize_state(words[3]).map_err(|e| err(&e))?));
                        }
                        _ => {
                            d.meta.insert(k.to_string(), v.to_string());
                        }
                    }
                    section = Section::Header;
                } else {
                    return Err(err("unexpected line"));
                }
                continue;
            }
            match section {
                Section::Header => return Err(err("indented line outside palette/layer")),
                Section::Palette => {
                    let body = line.trim();
                    let mut chars = body.chars();
                    let c = chars.next().unwrap();
                    let state = chars.as_str().trim();
                    if c == '.' || state.is_empty() {
                        return Err(err("bad palette entry"));
                    }
                    palette.insert(c, normalize_state(state).map_err(|e| err(&e))?);
                }
                Section::Layer(y) => {
                    for (col, c) in line[2..].chars().enumerate() {
                        if c == '.' || c == ' ' {
                            continue;
                        }
                        let state = palette.get(&c).ok_or_else(|| err(&format!("unknown palette char {c:?}")))?;
                        d.blocks.insert([origin[0] + col as i32, origin[1] + y, origin[2] + row], state.clone());
                    }
                    row += 1;
                }
                Section::NetherLayer(y) => {
                    for (col, c) in line[2..].chars().enumerate() {
                        if c == '.' || c == ' ' {
                            continue;
                        }
                        let state = palette.get(&c).ok_or_else(|| err(&format!("unknown palette char {c:?}")))?;
                        d.nether_blocks.insert([nether_origin[0] + col as i32, nether_origin[1] + y, nether_origin[2] + row], state.clone());
                    }
                    row += 1;
                }
            }
        }
        Ok(d)
    }

    /// Inclusive bounding box of the blocks (entities' cells and ports included).
    pub fn bounds(&self) -> Option<(Pos, Pos)> {
        let mut pts: Vec<Pos> = self.blocks.keys().copied().collect();
        for e in &self.entities {
            pts.push([e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]);
        }
        let first = *pts.first()?;
        let (mut lo, mut hi) = (first, first);
        for p in pts {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        Some((lo, hi))
    }

    pub fn port(&self, name: &str) -> Option<Pos> {
        self.ports.get(name).map(|(p, _)| *p)
    }

    /// The judge's numbers from a `# CLOCK: <name> period P gt ..., warmup W, ...` line.
    pub fn clock_hint(&self) -> Option<(u32, u32)> {
        let line = self.comments.iter().find(|l| l.starts_with("# CLOCK:"))?;
        let num_after = |key: &str| -> Option<u32> {
            let i = line.find(key)? + key.len();
            line[i..].split(|ch: char| !ch.is_ascii_digit()).next()?.parse().ok()
        };
        Some((num_after("warmup ")?, num_after(" period ")?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ports_layers_and_hints() {
        let text = "# CLOCK: x period 12 gt, on 4/12, edges 2, warmup 3, family f\nname: demo\nport start: 0 0 0 last\nport lever: 1 1 0\norigin: 5 0 -2\npalette:\n  S stone\n  P sticky_piston[facing=up]\nlayer 0:\n  S.\n  .S\nlayer 1:\n  P\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.name, "demo");
        assert_eq!(d.blocks.len(), 3);
        assert_eq!(d.blocks[&[5, 1, -2]], "minecraft:sticky_piston[facing=up]");
        assert_eq!(d.blocks[&[6, 0, -1]], "minecraft:stone");
        assert_eq!(d.ports["start"], ([0, 0, 0], "last".to_string()));
        assert_eq!(d.port("lever"), Some([1, 1, 0]));
        assert_eq!(d.clock_hint(), Some((3, 12)));
    }

    #[test]
    fn comments_and_entities() {
        let text = "# hello\nname: e # trailing\nentity: minecart 0.5 1 2.5 {Rotation:[90f,0f]}\nnbt: 1 0 0 {Items:[]}\npalette:\n  S stone\nlayer 0:\n  S\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.name, "e");
        assert_eq!(d.comments[0], "# hello");
        assert_eq!(d.entities[0].kind, "minecraft:minecart");
        assert_eq!(d.entities[0].nbt.as_deref(), Some("{Rotation:[90f,0f]}"));
        assert_eq!(d.block_entities[&[1, 0, 0]], "{Items:[]}");
    }

    #[test]
    fn a_nether_part_loads() {
        // (mctas::design's own test text: the overworld blocks drawn, the Nether part kept apart)
        let text = "name: n\nport output: 0 0 0\npalette:\n  S stone\n  P nether_portal[axis=z]\n  H hopper\nlayer 0:\n  S\nnether-origin: 2 0 3\nnether layer 64:\n  SPH\nport nether-output: 4 64 3\nnether nbt: 4 64 3 {Items:[{Slot:0b,id:\"minecraft:paper\",count:1}]}\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.blocks.len(), 1);
        assert_eq!(d.blocks[&[0, 0, 0]], "minecraft:stone");
        assert_eq!(d.nether_blocks.len(), 3);
        assert_eq!(d.nether_blocks[&[3, 64, 3]], "minecraft:nether_portal[axis=z]");
        assert_eq!(d.nether_blocks[&[4, 64, 3]], "minecraft:hopper");
        assert_eq!(d.port("nether-output"), Some([4, 64, 3]));
        assert!(d.block_entities.is_empty());
    }

    #[test]
    fn the_nether_part_is_a_design_of_its_own() {
        let text = "name: n\ncircuit: mob-farm iron_ingot\nport output: 0 0 0\nentity: minecraft:villager 0.5 1 0.5\npalette:\n  S stone\n  P nether_portal[axis=z]\n  H hopper\nlayer 0:\n  S\nnether-origin: 2 0 3\nnether layer 64:\n  SPH\nport nether-output: 4 64 3\nnether nbt: 4 64 3 {Items:[{Slot:0b,id:\"minecraft:paper\",count:1}]}\n";
        let n = Design::parse(&nether_part_text(text).unwrap()).unwrap();
        assert_eq!(n.name, "n nether part");
        assert_eq!(n.blocks, Design::parse(text).unwrap().nether_blocks);
        assert!(n.entities.is_empty() && n.nether_blocks.is_empty() && !n.meta.contains_key("circuit"));
        assert_eq!(n.ports.keys().collect::<Vec<_>>(), ["output"]);
        assert_eq!(n.port("output"), Some([4, 64, 3]));
        assert_eq!(n.block_entities[&[4, 64, 3]], "{Items:[{Slot:0b,id:\"minecraft:paper\",count:1}]}");
        assert!(nether_part_text("name: x\npalette:\n  S stone\nlayer 0:\n  S\n").is_err());
    }
}
